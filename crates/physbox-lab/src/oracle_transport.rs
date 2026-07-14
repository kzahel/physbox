use std::{
    ffi::OsString,
    fmt::{Display, Formatter},
    fs::{self, File, OpenOptions},
    io::{Read, Write},
    path::{Path, PathBuf},
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};

use physbox_contracts::{
    FailureKindV1, ORACLE_SUCCESS_SENTINEL, OracleRequestV1, OracleResultV1, Validate,
    canonical_json, sha256_bytes,
};

const REQUEST_FILE: &str = "oracle-request.json";
const STAGING_RESULT_FILE: &str = "oracle-result.partial.json";
const REJECTED_RESULT_FILE: &str = "oracle-result.rejected.json";
const FINAL_RESULT_FILE: &str = "oracle-result.json";
const STDOUT_FILE: &str = "oracle-stdout.log";
const STDERR_FILE: &str = "oracle-stderr.log";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleSourceIdentity {
    pub revision: String,
    pub dirty: bool,
    pub patch_digest: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleProcess {
    pub program: PathBuf,
    pub arguments: Vec<OsString>,
}

#[derive(Clone, Debug)]
pub struct OracleRunConfig {
    pub artifact_root: PathBuf,
    pub timeout: Duration,
    pub source: OracleSourceIdentity,
    pub process: OracleProcess,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleRunArtifacts {
    pub directory: PathBuf,
    pub request: PathBuf,
    pub result: PathBuf,
    pub stdout: PathBuf,
    pub stderr: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OracleRun {
    pub projection_hash: String,
    pub result: OracleResultV1,
    pub artifacts: OracleRunArtifacts,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OracleRunErrorKind {
    InvalidRequest,
    StaleResult,
    Spawn,
    Timeout,
    ProcessFailed,
    MissingSentinel,
    PartialResult,
    HashMismatch,
    WrongSourcePin,
    UnsupportedFeature,
    OracleReportedFailure,
    Artifact,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OracleRunError {
    pub kind: OracleRunErrorKind,
    pub reason: String,
    pub artifact_directory: Option<PathBuf>,
}

impl Display for OracleRunError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "{:?}: {}", self.kind, self.reason)
    }
}

impl std::error::Error for OracleRunError {}

pub fn run_oracle(
    config: &OracleRunConfig,
    request: &OracleRequestV1,
) -> Result<OracleRun, OracleRunError> {
    let request_bytes = canonical_json(request)
        .map_err(|error| error_without_artifacts(OracleRunErrorKind::InvalidRequest, error.0))?;
    validate_run_id(&request.run_id)?;
    let projection_hash = sha256_bytes(&request_bytes);
    let directory = config.artifact_root.join(&request.run_id);
    fs::create_dir_all(&config.artifact_root).map_err(|error| {
        error_without_artifacts(
            OracleRunErrorKind::Artifact,
            format!("create artifact root: {error}"),
        )
    })?;
    match fs::create_dir(&directory) {
        Ok(()) => {}
        Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {
            return Err(error_with_artifacts(
                OracleRunErrorKind::StaleResult,
                "run artifact directory already exists; refusing to reuse it",
                &directory,
            ));
        }
        Err(error) => {
            return Err(error_without_artifacts(
                OracleRunErrorKind::Artifact,
                format!("create run artifact directory: {error}"),
            ));
        }
    }

    let request_path = directory.join(REQUEST_FILE);
    let staging_path = directory.join(STAGING_RESULT_FILE);
    let rejected_path = directory.join(REJECTED_RESULT_FILE);
    let result_path = directory.join(FINAL_RESULT_FILE);
    let stdout_path = directory.join(STDOUT_FILE);
    let stderr_path = directory.join(STDERR_FILE);
    atomic_write(&request_path, &request_bytes)
        .map_err(|error| artifact_error(&directory, "write canonical request", error))?;

    let stdout_file = create_new_file(&stdout_path)
        .map_err(|error| artifact_error(&directory, "create stdout log", error))?;
    let stderr_file = create_new_file(&stderr_path)
        .map_err(|error| artifact_error(&directory, "create stderr log", error))?;
    let mut command = Command::new(&config.process.program);
    command
        .args(&config.process.arguments)
        .arg("--request")
        .arg(&request_path)
        .arg("--result")
        .arg(&staging_path)
        .stdin(Stdio::null())
        .stdout(Stdio::from(stdout_file))
        .stderr(Stdio::from(stderr_file));
    let mut child = command.spawn().map_err(|error| {
        error_with_artifacts(
            OracleRunErrorKind::Spawn,
            format!("launch oracle process: {error}"),
            &directory,
        )
    })?;
    let started = Instant::now();
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status,
            Ok(None) if started.elapsed() < config.timeout => {
                thread::sleep(Duration::from_millis(2));
            }
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error_with_artifacts(
                    OracleRunErrorKind::Timeout,
                    format!("oracle exceeded {} ms", config.timeout.as_millis()),
                    &directory,
                ));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error_with_artifacts(
                    OracleRunErrorKind::ProcessFailed,
                    format!("wait for oracle process: {error}"),
                    &directory,
                ));
            }
        }
    };
    if !status.success() {
        return Err(error_with_artifacts(
            OracleRunErrorKind::ProcessFailed,
            format!("oracle exited with {status}"),
            &directory,
        ));
    }

    let stdout = read_file(&stdout_path)
        .map_err(|error| artifact_error(&directory, "read stdout log", error))?;
    let stdout_text = String::from_utf8_lossy(&stdout);
    if !stdout_text
        .lines()
        .any(|line| line == ORACLE_SUCCESS_SENTINEL)
    {
        reject_if_present(&staging_path, &rejected_path);
        return Err(error_with_artifacts(
            OracleRunErrorKind::MissingSentinel,
            "oracle exited successfully without the exact success sentinel",
            &directory,
        ));
    }

    let result_bytes = read_file(&staging_path).map_err(|error| {
        error_with_artifacts(
            OracleRunErrorKind::PartialResult,
            format!("read staged oracle result: {error}"),
            &directory,
        )
    })?;
    let result: OracleResultV1 = serde_json::from_slice(&result_bytes).map_err(|error| {
        error_with_artifacts(
            OracleRunErrorKind::PartialResult,
            format!("parse staged oracle result: {error}"),
            &directory,
        )
    })?;
    if let Err(error) = result.validate() {
        reject_if_present(&staging_path, &rejected_path);
        return Err(error_with_artifacts(
            OracleRunErrorKind::PartialResult,
            format!("validate staged oracle result: {error}"),
            &directory,
        ));
    }

    let identity_check = validate_result_identity(config, request, &projection_hash, &result);
    if let Err((kind, reason)) = identity_check {
        reject_if_present(&staging_path, &rejected_path);
        return Err(error_with_artifacts(kind, reason, &directory));
    }
    if let Some(failure) = &result.failure {
        reject_if_present(&staging_path, &rejected_path);
        let kind = if failure.kind == FailureKindV1::UnsupportedFeature {
            OracleRunErrorKind::UnsupportedFeature
        } else {
            OracleRunErrorKind::OracleReportedFailure
        };
        return Err(error_with_artifacts(kind, &failure.reason, &directory));
    }

    fs::rename(&staging_path, &result_path)
        .map_err(|error| artifact_error(&directory, "atomically publish oracle result", error))?;
    sync_directory(&directory)
        .map_err(|error| artifact_error(&directory, "sync run artifact directory", error))?;
    Ok(OracleRun {
        projection_hash,
        result,
        artifacts: OracleRunArtifacts {
            directory,
            request: request_path,
            result: result_path,
            stdout: stdout_path,
            stderr: stderr_path,
        },
    })
}

pub fn verify_clean_git_pin(
    checkout: &Path,
    expected_revision: &str,
) -> Result<OracleSourceIdentity, OracleRunError> {
    let revision = git_output(checkout, &["rev-parse", "HEAD"])?;
    if revision != expected_revision {
        return Err(error_without_artifacts(
            OracleRunErrorKind::WrongSourcePin,
            format!("oracle checkout is at {revision}, expected {expected_revision}"),
        ));
    }
    let status = git_output(
        checkout,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    if !status.is_empty() {
        return Err(error_without_artifacts(
            OracleRunErrorKind::WrongSourcePin,
            "oracle authority checkout is dirty",
        ));
    }
    Ok(OracleSourceIdentity {
        revision,
        dirty: false,
        patch_digest: None,
    })
}

fn validate_result_identity(
    config: &OracleRunConfig,
    request: &OracleRequestV1,
    projection_hash: &str,
    result: &OracleResultV1,
) -> Result<(), (OracleRunErrorKind, String)> {
    if result.run_id != request.run_id || result.provenance.run_id != request.run_id {
        return Err((
            OracleRunErrorKind::StaleResult,
            "result or provenance run id does not match the fresh request".to_owned(),
        ));
    }
    if result.source_scenario_hash != request.source_scenario_hash {
        return Err((
            OracleRunErrorKind::HashMismatch,
            "result source scenario hash does not match the request".to_owned(),
        ));
    }
    if result.projection_hash != projection_hash {
        return Err((
            OracleRunErrorKind::HashMismatch,
            "result projection hash does not match the exact request bytes".to_owned(),
        ));
    }
    if result.provenance.source_revision != config.source.revision
        || result.provenance.source_dirty != config.source.dirty
        || result.provenance.source_patch_digest != config.source.patch_digest
    {
        return Err((
            OracleRunErrorKind::WrongSourcePin,
            "result source revision, dirty state, or patch digest does not match the verified oracle source"
                .to_owned(),
        ));
    }
    Ok(())
}

fn validate_run_id(run_id: &str) -> Result<(), OracleRunError> {
    if run_id.len() <= 96
        && run_id
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'))
    {
        Ok(())
    } else {
        Err(error_without_artifacts(
            OracleRunErrorKind::InvalidRequest,
            "run id must be 1-96 portable filename characters",
        ))
    }
}

fn atomic_write(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let partial = path.with_extension("partial");
    let mut file = create_new_file(&partial)?;
    file.write_all(bytes)?;
    file.sync_all()?;
    fs::rename(partial, path)?;
    if let Some(parent) = path.parent() {
        sync_directory(parent)?;
    }
    Ok(())
}

fn create_new_file(path: &Path) -> std::io::Result<File> {
    OpenOptions::new().write(true).create_new(true).open(path)
}

fn read_file(path: &Path) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    File::open(path)?.read_to_end(&mut bytes)?;
    Ok(bytes)
}

fn sync_directory(path: &Path) -> std::io::Result<()> {
    File::open(path)?.sync_all()
}

fn reject_if_present(staging: &Path, rejected: &Path) {
    if staging.exists() {
        let _ = fs::rename(staging, rejected);
    }
}

fn git_output(checkout: &Path, arguments: &[&str]) -> Result<String, OracleRunError> {
    let output = Command::new("git")
        .arg("-C")
        .arg(checkout)
        .args(arguments)
        .output()
        .map_err(|error| {
            error_without_artifacts(
                OracleRunErrorKind::WrongSourcePin,
                format!("run git pin check: {error}"),
            )
        })?;
    if !output.status.success() {
        return Err(error_without_artifacts(
            OracleRunErrorKind::WrongSourcePin,
            format!(
                "git pin check failed: {}",
                String::from_utf8_lossy(&output.stderr).trim()
            ),
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn artifact_error(directory: &Path, action: &str, error: std::io::Error) -> OracleRunError {
    error_with_artifacts(
        OracleRunErrorKind::Artifact,
        format!("{action}: {error}"),
        directory,
    )
}

fn error_without_artifacts(kind: OracleRunErrorKind, reason: impl Into<String>) -> OracleRunError {
    OracleRunError {
        kind,
        reason: reason.into(),
        artifact_directory: None,
    }
}

fn error_with_artifacts(
    kind: OracleRunErrorKind,
    reason: impl Into<String>,
    directory: &Path,
) -> OracleRunError {
    OracleRunError {
        kind,
        reason: reason.into(),
        artifact_directory: Some(directory.to_owned()),
    }
}
