use std::path::{Path, PathBuf};

use physbox_contracts::{NamedValueV1, ProvenanceV1};
use physbox_engine::{
    LIQUID_SCALE_SUCCESS_SENTINEL, LiquidScaleBenchmarkConfigV1, LiquidScaleHostKindV1,
    LiquidScaleSolidTier, run_liquid_scale_full_frame_benchmark,
    run_liquid_scale_physics_benchmark,
};

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-liquid-scale error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let config = if arguments.full_frame {
        LiquidScaleBenchmarkConfigV1::full_frame(
            arguments.solid_tier,
            arguments.particle_count,
            arguments.warmup,
            arguments.samples,
            arguments.render_extent,
            arguments.interactive_duration_seconds,
        )
        .with_particle_capacity(arguments.particle_capacity)
    } else {
        LiquidScaleBenchmarkConfigV1::physics_only(
            arguments.solid_tier,
            arguments.particle_count,
            arguments.warmup,
            arguments.samples,
        )
        .with_particle_capacity(arguments.particle_capacity)
    };
    let provenance = ProvenanceV1 {
        run_id: arguments.run_id.clone(),
        source_revision: arguments.source_revision,
        source_dirty: arguments.source_dirty,
        source_patch_digest: arguments.source_patch_digest,
        host: arguments.host,
        adapter: "runtime-discovered".to_owned(),
        backend: "runtime-discovered".to_owned(),
        driver: "runtime-discovered".to_owned(),
        toolchain: arguments.toolchain,
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
                value: if arguments.full_frame {
                    "full-frame-interactive"
                } else {
                    "physics-only-explicit-steps"
                }
                .to_owned(),
            },
        ],
    };
    let run = if arguments.full_frame {
        pollster::block_on(run_liquid_scale_full_frame_benchmark(
            config,
            LiquidScaleHostKindV1::AndroidNative,
            provenance,
        ))?
    } else {
        pollster::block_on(run_liquid_scale_physics_benchmark(
            config,
            LiquidScaleHostKindV1::AndroidNative,
            provenance,
        ))?
    };
    let json = serde_json::to_vec_pretty(&run.artifact).map_err(|error| error.to_string())?;
    write_atomic(&arguments.output, &json, &arguments.run_id)?;
    println!(
        "PHYSBOX5_LIQUID_SCALE_ADAPTER {}",
        serde_json::to_string(&run.adapter).map_err(|error| error.to_string())?
    );
    println!(
        "PHYSBOX5_LIQUID_SCALE_ROW_SUCCESS scenario={} particles={} dynamicBodies={} hostP95Ms={:.3} gpuStatus={:?}",
        run.artifact.scenario_id,
        run.artifact.work.live_particles,
        run.artifact.work.dynamic_bodies,
        run.artifact.timings.host_completion.p95_seconds * 1_000.0,
        run.artifact.timings.gpu_timestamp_status,
    );
    println!("{LIQUID_SCALE_SUCCESS_SENTINEL}");
    Ok(())
}

struct Arguments {
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    host: String,
    toolchain: String,
    solid_tier: LiquidScaleSolidTier,
    particle_count: u32,
    particle_capacity: u32,
    warmup: u32,
    samples: u32,
    full_frame: bool,
    render_extent: [u32; 2],
    interactive_duration_seconds: f64,
    output: PathBuf,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut run_id = None;
        let mut source_revision = None;
        let mut source_dirty = None;
        let mut source_patch_digest = None;
        let mut host = None;
        let mut toolchain = None;
        let mut solid_tier = None;
        let mut particle_count = None;
        let mut particle_capacity = None;
        let mut warmup = None;
        let mut samples = None;
        let mut timing_mode = None;
        let mut render_width = None;
        let mut render_height = None;
        let mut interactive_duration_seconds = None;
        let mut output = None;
        let mut arguments = std::env::args().skip(1);
        while let Some(argument) = arguments.next() {
            let value = arguments
                .next()
                .ok_or_else(|| format!("missing value for {argument}"))?;
            match argument.as_str() {
                "--run-id" => run_id = Some(value),
                "--source-revision" => source_revision = Some(value),
                "--source-dirty" => source_dirty = Some(parse_bool(&value)?),
                "--source-patch-digest" => source_patch_digest = (value != "none").then_some(value),
                "--host" => host = Some(value),
                "--toolchain" => toolchain = Some(value),
                "--solid-tier" => solid_tier = Some(parse_tier(&value)?),
                "--particles" => particle_count = Some(parse_nonnegative_u32(&value, "particles")?),
                "--capacity" => particle_capacity = Some(parse_u32(&value, "capacity")?),
                "--warmup" => warmup = Some(parse_u32(&value, "warmup")?),
                "--samples" => samples = Some(parse_u32(&value, "samples")?),
                "--timing-mode" => timing_mode = Some(parse_timing_mode(&value)?),
                "--render-width" => render_width = Some(parse_u32(&value, "render width")?),
                "--render-height" => render_height = Some(parse_u32(&value, "render height")?),
                "--duration-seconds" => {
                    interactive_duration_seconds = Some(parse_positive_f64(&value, "duration")?)
                }
                "--output" => output = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        let particle_count = required(particle_count, "--particles")?;
        Ok(Self {
            run_id: required(run_id, "--run-id")?,
            source_revision: required(source_revision, "--source-revision")?,
            source_dirty: required(source_dirty, "--source-dirty")?,
            source_patch_digest,
            host: required(host, "--host")?,
            toolchain: required(toolchain, "--toolchain")?,
            solid_tier: required(solid_tier, "--solid-tier")?,
            particle_count,
            particle_capacity: particle_capacity.unwrap_or(particle_count),
            warmup: required(warmup, "--warmup")?,
            samples: required(samples, "--samples")?,
            full_frame: timing_mode.unwrap_or(false),
            render_extent: [render_width.unwrap_or(960), render_height.unwrap_or(600)],
            interactive_duration_seconds: interactive_duration_seconds.unwrap_or(30.0),
            output: required(output, "--output")?,
        })
    }
}

fn required<T>(value: Option<T>, option: &str) -> Result<T, String> {
    value.ok_or_else(|| format!("missing {option}"))
}

fn parse_tier(value: &str) -> Result<LiquidScaleSolidTier, String> {
    match value {
        "water-only" => Ok(LiquidScaleSolidTier::WaterOnly),
        "20" => Ok(LiquidScaleSolidTier::Solids20),
        "100" => Ok(LiquidScaleSolidTier::Solids100),
        _ => Err("solid tier must be water-only, 20, or 100".to_owned()),
    }
}

fn parse_u32(value: &str, label: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .ok()
        .filter(|value| *value != 0)
        .ok_or_else(|| format!("{label} must be a positive integer"))
}

fn parse_nonnegative_u32(value: &str, label: &str) -> Result<u32, String> {
    value
        .parse::<u32>()
        .map_err(|_| format!("{label} must be a nonnegative integer"))
}

fn parse_bool(value: &str) -> Result<bool, String> {
    match value {
        "true" => Ok(true),
        "false" => Ok(false),
        _ => Err(format!("invalid boolean: {value}")),
    }
}

fn parse_timing_mode(value: &str) -> Result<bool, String> {
    match value {
        "physics-only" => Ok(false),
        "full-frame" => Ok(true),
        _ => Err("timing mode must be physics-only or full-frame".to_owned()),
    }
}

fn parse_positive_f64(value: &str, label: &str) -> Result<f64, String> {
    value
        .parse::<f64>()
        .ok()
        .filter(|value| value.is_finite() && *value > 0.0)
        .ok_or_else(|| format!("{label} must be finite and positive"))
}

fn write_atomic(path: &Path, bytes: &[u8], run_id: &str) -> Result<(), String> {
    if path.exists() {
        return Err(format!(
            "liquid scale output must be fresh: {}",
            path.display()
        ));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|error| format!("create liquid benchmark output parent: {error}"))?;
    }
    let temporary = path.with_extension(format!("tmp-{run_id}"));
    let _ = std::fs::remove_file(&temporary);
    std::fs::write(&temporary, bytes)
        .map_err(|error| format!("write liquid benchmark temporary artifact: {error}"))?;
    std::fs::rename(&temporary, path)
        .map_err(|error| format!("publish liquid benchmark artifact: {error}"))
}
