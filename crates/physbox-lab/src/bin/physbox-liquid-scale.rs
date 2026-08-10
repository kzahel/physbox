use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{SystemTime, UNIX_EPOCH};

use physbox_contracts::{NamedValueV1, ProvenanceV1};
use physbox_engine::{
    LIQUID_SCALE_SUCCESS_SENTINEL, LiquidScaleBenchmarkConfigV1, LiquidScaleHostKindV1,
    LiquidScaleSolidTier, run_liquid_scale_physics_benchmark,
};
use serde::Serialize;
use sha2::{Digest, Sha256};

const SUITE_VERSION: u32 = 1;
const DEFAULT_WARMUP_STEPS: u32 = 16;
const DEFAULT_RETAINED_SAMPLES: u32 = 30;

#[derive(Clone, Debug)]
struct Arguments {
    matrix: Matrix,
    output: PathBuf,
    warmup_steps: u32,
    retained_samples: u32,
}

#[derive(Clone, Debug)]
enum Matrix {
    Selected,
    Full,
    Row {
        solid_tier: LiquidScaleSolidTier,
        particle_count: u32,
        particle_capacity: u32,
    },
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SuiteManifest {
    version: u32,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    warmup_steps: u32,
    retained_samples: u32,
    accepted_rows: u32,
    artifacts: Vec<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-liquid-scale error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = parse_arguments()?;
    if arguments.output.exists() {
        return Err(format!(
            "liquid scale output must be fresh: {}",
            arguments.output.display()
        ));
    }
    let run_id = run_id();
    let staging = staging_path(&arguments.output, &run_id)?;
    fs::create_dir(&staging)
        .map_err(|error| format!("create liquid staging {}: {error}", staging.display()))?;
    let result = execute_matrix(&arguments, &run_id, &staging);
    match result {
        Ok(manifest) => {
            atomic_json(&staging.join("manifest.json"), &manifest)?;
            fs::rename(&staging, &arguments.output).map_err(|error| {
                format!(
                    "publish liquid evidence {} -> {}: {error}",
                    staging.display(),
                    arguments.output.display()
                )
            })?;
            println!(
                "{LIQUID_SCALE_SUCCESS_SENTINEL} rows={} output={}",
                manifest.accepted_rows,
                arguments.output.display()
            );
            Ok(())
        }
        Err(error) => {
            let _ = fs::remove_dir_all(&staging);
            Err(error)
        }
    }
}

fn execute_matrix(
    arguments: &Arguments,
    run_id: &str,
    staging: &Path,
) -> Result<SuiteManifest, String> {
    let source_revision = git_text(&["rev-parse", "HEAD"])
        .ok_or_else(|| "liquid benchmark requires a Git source revision".to_owned())?;
    let source_dirty =
        git_bytes(&["status", "--porcelain=v1", "-z"]).is_none_or(|status| !status.is_empty());
    let source_patch_digest = source_dirty.then(dirty_patch_digest).transpose()?;
    let toolchain = command_text("rustc", &["--version"])
        .ok_or_else(|| "liquid benchmark requires rustc provenance".to_owned())?;
    let rows = matrix_rows(&arguments.matrix);
    let mut artifacts = Vec::with_capacity(rows.len());
    for (solid_tier, particle_count, particle_capacity) in rows {
        let config = LiquidScaleBenchmarkConfigV1::physics_only(
            solid_tier,
            particle_count,
            arguments.warmup_steps,
            arguments.retained_samples,
        )
        .with_particle_capacity(particle_capacity);
        let provenance = ProvenanceV1 {
            run_id: run_id.to_owned(),
            source_revision: source_revision.clone(),
            source_dirty,
            source_patch_digest: source_patch_digest.clone(),
            host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            adapter: "runtime-discovered".to_owned(),
            backend: "runtime-discovered".to_owned(),
            driver: "runtime-discovered".to_owned(),
            toolchain: toolchain.clone(),
            build_options: vec![
                NamedValueV1 {
                    name: "box2d-comparator".to_owned(),
                    value: "false".to_owned(),
                },
                NamedValueV1 {
                    name: "profile".to_owned(),
                    value: "release".to_owned(),
                },
            ],
            runtime_settings: vec![
                NamedValueV1 {
                    name: "coupling-quality".to_owned(),
                    value: "standard".to_owned(),
                },
                NamedValueV1 {
                    name: "outer-dt-seconds".to_owned(),
                    value: format!("{:.17}", config.outer_dt_seconds),
                },
                NamedValueV1 {
                    name: "timing-mode".to_owned(),
                    value: "physics-only-explicit-steps".to_owned(),
                },
            ],
        };
        let host_kind = if cfg!(target_os = "android") {
            LiquidScaleHostKindV1::AndroidNative
        } else {
            LiquidScaleHostKindV1::NativeDesktop
        };
        let run = pollster::block_on(run_liquid_scale_physics_benchmark(
            config, host_kind, provenance,
        ))?;
        let name = format!("{}.json", run.artifact.scenario_id);
        atomic_json(&staging.join(&name), &run.artifact)?;
        println!(
            "PHYSBOX5_LIQUID_SCALE_ROW_SUCCESS scenario={} particles={} dynamicBodies={} hostP95Ms={:.3} gpuStatus={:?}",
            run.artifact.scenario_id,
            run.artifact.work.live_particles,
            run.artifact.work.dynamic_bodies,
            run.artifact.timings.host_completion.p95_seconds * 1_000.0,
            run.artifact.timings.gpu_timestamp_status,
        );
        artifacts.push(name);
    }
    Ok(SuiteManifest {
        version: SUITE_VERSION,
        run_id: run_id.to_owned(),
        source_revision,
        source_dirty,
        warmup_steps: arguments.warmup_steps,
        retained_samples: arguments.retained_samples,
        accepted_rows: artifacts.len() as u32,
        artifacts,
    })
}

fn parse_arguments() -> Result<Arguments, String> {
    let mut values = std::env::args().skip(1).collect::<Vec<_>>();
    let mut warmup_steps = DEFAULT_WARMUP_STEPS;
    let mut retained_samples = DEFAULT_RETAINED_SAMPLES;
    while values.len() >= 2 {
        let option_index = values.len() - 2;
        match values[option_index].as_str() {
            "--warmup" => {
                warmup_steps = positive_u32("warmup", &values[option_index + 1])?;
                values.truncate(option_index);
            }
            "--samples" => {
                retained_samples = positive_u32("samples", &values[option_index + 1])?;
                values.truncate(option_index);
            }
            _ => break,
        }
    }
    let (matrix, output) = match values.as_slice() {
        [mode, output] if mode == "selected" => (Matrix::Selected, PathBuf::from(output)),
        [mode, output] if mode == "full" => (Matrix::Full, PathBuf::from(output)),
        [mode, tier, particles, output] if mode == "row" => (
            Matrix::Row {
                solid_tier: parse_tier(tier)?,
                particle_count: parse_particles(particles)?,
                particle_capacity: parse_particles(particles)?,
            },
            PathBuf::from(output),
        ),
        [mode, tier, particles, capacity, output] if mode == "fixed-reserve" => (
            Matrix::Row {
                solid_tier: parse_tier(tier)?,
                particle_count: parse_fixed_reserve_live_particles(particles)?,
                particle_capacity: parse_particles(capacity)?,
            },
            PathBuf::from(output),
        ),
        _ => {
            return Err(
                "usage: physbox-liquid-scale selected|full OUTPUT [--warmup N] [--samples N]\n       physbox-liquid-scale row water-only|20|100 1024|4096|8192|16384|32768 OUTPUT [--warmup N] [--samples N]\n       physbox-liquid-scale fixed-reserve water-only|20|100 0|1|1024|4096|8192 8192 OUTPUT [--warmup N] [--samples N]"
                    .to_owned(),
            );
        }
    };
    Ok(Arguments {
        matrix,
        output,
        warmup_steps,
        retained_samples,
    })
}

fn matrix_rows(matrix: &Matrix) -> Vec<(LiquidScaleSolidTier, u32, u32)> {
    match matrix {
        Matrix::Selected => vec![
            (LiquidScaleSolidTier::Solids20, 8_192, 8_192),
            (LiquidScaleSolidTier::Solids100, 8_192, 8_192),
        ],
        Matrix::Full => {
            let mut rows = [1_024, 4_096, 8_192, 16_384, 32_768]
                .into_iter()
                .map(|particles| (LiquidScaleSolidTier::WaterOnly, particles, particles))
                .collect::<Vec<_>>();
            for tier in [
                LiquidScaleSolidTier::Solids20,
                LiquidScaleSolidTier::Solids100,
            ] {
                rows.extend(
                    [4_096, 8_192, 16_384]
                        .into_iter()
                        .map(|particles| (tier, particles, particles)),
                );
            }
            rows
        }
        Matrix::Row {
            solid_tier,
            particle_count,
            particle_capacity,
        } => vec![(*solid_tier, *particle_count, *particle_capacity)],
    }
}

fn parse_tier(value: &str) -> Result<LiquidScaleSolidTier, String> {
    match value {
        "water-only" => Ok(LiquidScaleSolidTier::WaterOnly),
        "20" => Ok(LiquidScaleSolidTier::Solids20),
        "100" => Ok(LiquidScaleSolidTier::Solids100),
        _ => Err("liquid solid tier must be water-only, 20, or 100".to_owned()),
    }
}

fn parse_particles(value: &str) -> Result<u32, String> {
    let particles = positive_u32("particles", value)?;
    [1_024, 4_096, 8_192, 16_384, 32_768]
        .contains(&particles)
        .then_some(particles)
        .ok_or_else(|| "liquid particles must be 1024, 4096, 8192, 16384, or 32768".to_owned())
}

fn parse_fixed_reserve_live_particles(value: &str) -> Result<u32, String> {
    let particles = value
        .parse::<u32>()
        .map_err(|_| "liquid live particles must be 0, 1, 1024, 4096, or 8192".to_owned())?;
    [0, 1, 1_024, 4_096, 8_192]
        .contains(&particles)
        .then_some(particles)
        .ok_or_else(|| "liquid live particles must be 0, 1, 1024, 4096, or 8192".to_owned())
}

fn positive_u32(label: &str, value: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value != 0)
        .ok_or_else(|| format!("liquid {label} must be a positive integer"))
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let name = path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("liquid output has no UTF-8 filename: {}", path.display()))?;
    let temporary = path.with_file_name(format!(".{name}.tmp-{}", std::process::id()));
    fs::write(&temporary, bytes)
        .map_err(|error| format!("write liquid temporary {}: {error}", temporary.display()))?;
    fs::rename(&temporary, path).map_err(|error| {
        format!(
            "publish liquid JSON {} -> {}: {error}",
            temporary.display(),
            path.display()
        )
    })
}

fn staging_path(output: &Path, run_id: &str) -> Result<PathBuf, String> {
    let name = output
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| format!("liquid output has no UTF-8 filename: {}", output.display()))?;
    Ok(output.with_file_name(format!(".{name}.staging-{run_id}")))
}

fn dirty_patch_digest() -> Result<String, String> {
    let mut hasher = Sha256::new();
    let status = git_bytes(&["status", "--porcelain=v1", "-z"])
        .ok_or_else(|| "read dirty Git status for liquid provenance".to_owned())?;
    hasher.update(&status);
    let diff = git_bytes(&["diff", "--binary", "HEAD"])
        .ok_or_else(|| "read dirty Git patch for liquid provenance".to_owned())?;
    hasher.update(&diff);
    let untracked = git_bytes(&["ls-files", "--others", "--exclude-standard", "-z"])
        .ok_or_else(|| "read untracked files for liquid provenance".to_owned())?;
    for path in untracked
        .split(|byte| *byte == 0)
        .filter(|path| !path.is_empty())
    {
        hasher.update(path);
        let path = std::str::from_utf8(path)
            .map_err(|error| format!("decode untracked liquid path: {error}"))?;
        if Path::new(path).is_file() {
            hasher.update(
                fs::read(path).map_err(|error| format!("read untracked file {path}: {error}"))?,
            );
        }
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn git_text(arguments: &[&str]) -> Option<String> {
    git_bytes(arguments).map(|bytes| String::from_utf8_lossy(&bytes).trim().to_owned())
}

fn git_bytes(arguments: &[&str]) -> Option<Vec<u8>> {
    let output = Command::new("git").args(arguments).output().ok()?;
    output.status.success().then_some(output.stdout)
}

fn command_text(command: &str, arguments: &[&str]) -> Option<String> {
    let output = Command::new(command).args(arguments).output().ok()?;
    output
        .status
        .success()
        .then(|| String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn run_id() -> String {
    let nanos = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |duration| duration.as_nanos());
    format!("{nanos}-{}", std::process::id())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn full_matrix_keeps_32k_at_the_raw_capacity_frontier() {
        let rows = matrix_rows(&Matrix::Full);
        assert_eq!(rows.len(), 11);
        assert!(rows.contains(&(LiquidScaleSolidTier::WaterOnly, 32_768)));
        assert!(!rows.contains(&(LiquidScaleSolidTier::Solids20, 32_768)));
        assert!(!rows.contains(&(LiquidScaleSolidTier::Solids100, 32_768)));
    }
}
