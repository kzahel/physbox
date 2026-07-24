use std::fs::OpenOptions;
use std::io::Write;
use std::path::PathBuf;
use std::process::Command;

use physbox_engine::{
    GpuFluidSolidOverlapEvidenceV1, SolverProfileConfig, run_gpu_fluid_solid_overlap_evidence,
};
use physbox_launch::FluidSolidCouplingQuality;
use serde::Serialize;
use sha2::{Digest, Sha256};

const ARTIFACT_VERSION: u32 = 1;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactV1 {
    artifact_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_sha256: Option<String>,
    host: &'static str,
    passed: bool,
    failure: Option<String>,
    evidence: GpuFluidSolidOverlapEvidenceV1,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-fluid-solid-overlap error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let source_revision = git(&["rev-parse", "HEAD"])?;
    let status = git(&["status", "--porcelain=v1", "--untracked-files=all"])?;
    let source_dirty = !status.is_empty();
    if arguments.require_clean && source_dirty {
        return Err("fluid/solid overlap evidence requires a clean source tree".to_owned());
    }
    let source_patch_sha256 = source_dirty
        .then(|| {
            git_bytes(&["diff", "--binary", "HEAD"])
                .map(|bytes| format!("{:x}", Sha256::digest(bytes)))
        })
        .transpose()?;
    let evidence = pollster::block_on(run_gpu_fluid_solid_overlap_evidence(
        SolverProfileConfig::soft_coupled(),
        arguments.coupling_quality,
    ))?;
    let failure = if evidence.capacity_clean_rows != evidence.row_count {
        Some(format!(
            "{} of {} overlap rows were finite and capacity-clean",
            evidence.capacity_clean_rows, evidence.row_count
        ))
    } else if arguments.coupling_quality == FluidSolidCouplingQuality::Standard
        && evidence.wet_only_issue_sources == 0
    {
        Some("Standard did not reproduce a wet-only sustained-overlap or re-entry row".to_owned())
    } else {
        None
    };
    let artifact = ArtifactV1 {
        artifact_version: ARTIFACT_VERSION,
        artifact_kind: "physbox5-s35-fluid-solid-overlap-corpus",
        run_id: arguments.run_id,
        source_revision,
        source_dirty,
        source_patch_sha256,
        host: "native-headless-webgpu",
        passed: failure.is_none(),
        failure,
        evidence,
    };
    let encoded = serde_json::to_vec_pretty(&artifact).map_err(|error| error.to_string())?;
    let mut output = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&arguments.output)
        .map_err(|error| {
            format!(
                "create fresh overlap artifact {}: {error}",
                arguments.output.display()
            )
        })?;
    output
        .write_all(&encoded)
        .map_err(|error| format!("write {}: {error}", arguments.output.display()))?;
    output
        .sync_all()
        .map_err(|error| format!("sync {}: {error}", arguments.output.display()))?;
    if let Some(failure) = &artifact.failure {
        return Err(format!(
            "{failure}; retained artifact {}",
            arguments.output.display()
        ));
    }
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s35-fluid-solid-overlap run_id={} coupling_quality={} wet_only_issue_rows={} wet_only_issue_sources={} output={}",
        artifact.run_id,
        artifact.evidence.coupling_quality,
        artifact.evidence.wet_only_issue_rows,
        artifact.evidence.wet_only_issue_sources,
        arguments.output.display()
    );
    Ok(())
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    require_clean: bool,
    coupling_quality: FluidSolidCouplingQuality,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut values = std::env::args().skip(1);
        let mut run_id = None;
        let mut output = None;
        let mut require_clean = false;
        let mut coupling_quality = FluidSolidCouplingQuality::Standard;
        while let Some(argument) = values.next() {
            match argument.as_str() {
                "--run-id" => {
                    run_id = Some(
                        values
                            .next()
                            .ok_or_else(|| "--run-id requires a value".to_owned())?,
                    );
                }
                "--output" => {
                    output = Some(PathBuf::from(
                        values
                            .next()
                            .ok_or_else(|| "--output requires a value".to_owned())?,
                    ));
                }
                "--require-clean" => require_clean = true,
                "--coupling-quality" => {
                    coupling_quality = match values
                        .next()
                        .ok_or_else(|| "--coupling-quality requires a value".to_owned())?
                        .as_str()
                    {
                        "standard" => FluidSolidCouplingQuality::Standard,
                        "stabilized" => FluidSolidCouplingQuality::Stabilized,
                        value => {
                            return Err(format!(
                                "unknown coupling quality {value}; expected standard or stabilized"
                            ));
                        }
                    };
                }
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        let run_id = run_id.ok_or_else(|| "missing --run-id".to_owned())?;
        if run_id.is_empty() {
            return Err("--run-id must not be empty".to_owned());
        }
        Ok(Self {
            run_id,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            require_clean,
            coupling_quality,
        })
    }
}

fn git(arguments: &[&str]) -> Result<String, String> {
    let output = git_bytes(arguments)?;
    String::from_utf8(output)
        .map(|value| value.trim().to_owned())
        .map_err(|error| format!("git output was not UTF-8: {error}"))
}

fn git_bytes(arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .args(arguments)
        .output()
        .map_err(|error| format!("run git {}: {error}", arguments.join(" ")))?;
    if !output.status.success() {
        return Err(format!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(output.stdout)
}
