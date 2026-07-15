use std::{
    fs::{self, File, OpenOptions},
    io::{BufWriter, Write},
    path::{Path, PathBuf},
    process::Command,
};

use physbox_contracts::{ConstructionResultV1, ConstructionStateV1, SCHEMA_VERSION, sha256_bytes};
use physbox_engine::{
    AdapterDiagnostics, ConstructionValidationScriptV1, EngineConfig, FrameInput, FrameReport,
    GpuConstructionGraphEvidence, OffscreenRenderer, PacingMode,
    construction_capacity_validation_script, construction_deep_overlap_validation_script,
    construction_interaction_validation_script, construction_invalidation_validation_script,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    adapter: AdapterDiagnostics,
    contract: ContractEvidence,
    interaction_script: ConstructionValidationScriptV1,
    interaction_trace: Vec<FrameEvidence>,
    interaction_exact_replay: bool,
    invalidation_script: ConstructionValidationScriptV1,
    invalidation_trace: Vec<FrameEvidence>,
    invalidation_exact_replay: bool,
    deep_overlap_script: ConstructionValidationScriptV1,
    deep_overlap: FailureEvidence,
    capacity_script: ConstructionValidationScriptV1,
    capacity: FailureEvidence,
    captures: Vec<String>,
    platform_artifacts: Vec<PlatformArtifact>,
    physical_device_gap: Option<String>,
    outcome: Outcome,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContractEvidence {
    intent_bytes: u32,
    intent_capacity: u32,
    state_bytes: u32,
    circle_radius: f32,
    rectangle_half_extents: [f32; 2],
    diagnostic_source: &'static str,
    diagnostic_readback_blocks_advancement: bool,
    product_cpu_occupancy_mirror: bool,
    validation_readback_scope: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FrameEvidence {
    label: &'static str,
    requested_exact_steps: u32,
    frame: FrameReport,
    construction: ConstructionStateV1,
    graph: GpuConstructionGraphEvidence,
    body_slots: Vec<physbox_engine::GpuConstructionBodySlotEvidence>,
    capture_body_zero: [f32; 8],
    capture_identity_zero: [u32; 4],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct FailureEvidence {
    exact_replay: bool,
    final_frame: FrameEvidence,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PlatformArtifact {
    label: String,
    path: String,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Outcome {
    interaction_frames: usize,
    interaction_pointer_events: u64,
    high_cadence_intents_in_one_step: u64,
    capacity_body_slots: u32,
    successful_capacity_placements: u64,
    explicit_capacity_failures: u64,
    explicit_deep_overlap_failures: u64,
    reused_slot: u32,
    reused_generation: u32,
    stale_graph_counts_after_erase: u32,
    stale_graph_counts_after_reuse: u32,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
}

struct TraceRun {
    adapter: AdapterDiagnostics,
    frames: Vec<FrameEvidence>,
}

struct Args {
    run_id: String,
    output: PathBuf,
    require_clean: bool,
    platforms: Vec<(String, PathBuf)>,
    physical_device_gap: Option<String>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-construction-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let args = parse_args()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if args.require_clean && source_dirty {
        return Err("construction evidence requires a clean source tree".to_owned());
    }
    let staging = args.output.with_extension("partial");
    if args.output.exists() || staging.exists() {
        return Err("output or staging evidence directory already exists".to_owned());
    }
    fs::create_dir_all(staging.join("captures")).map_err(|error| error.to_string())?;
    fs::create_dir_all(staging.join("platform")).map_err(|error| error.to_string())?;

    let interaction_script = construction_interaction_validation_script();
    let interaction = run_script(&interaction_script)?;
    let interaction_replay = run_script(&interaction_script)?;
    let interaction_exact_replay = exact_frames(&interaction.frames, &interaction_replay.frames)?;
    if !interaction_exact_replay {
        return Err("construction interaction replay differed".to_owned());
    }
    validate_interaction(&interaction.frames)?;

    let invalidation_script = construction_invalidation_validation_script();
    let invalidation = run_script(&invalidation_script)?;
    let invalidation_replay = run_script(&invalidation_script)?;
    let invalidation_exact_replay =
        exact_frames(&invalidation.frames, &invalidation_replay.frames)?;
    if !invalidation_exact_replay {
        return Err("construction invalidation replay differed".to_owned());
    }
    validate_invalidation(&invalidation.frames)?;

    let deep_overlap_script = construction_deep_overlap_validation_script();
    let deep_overlap = failure_run(&deep_overlap_script)?;
    if deep_overlap.final_frame.construction.deep_overlap_failures != 1
        || deep_overlap.final_frame.construction.last_result != ConstructionResultV1::DeepOverlap
    {
        return Err("deep-overlap lane did not reject exactly once".to_owned());
    }

    let capacity_script = construction_capacity_validation_script();
    let capacity = failure_run(&capacity_script)?;
    if capacity.final_frame.construction.live_bodies != 64
        || capacity.final_frame.construction.succeeded_intents != 62
        || capacity.final_frame.construction.capacity_failures != 1
        || capacity.final_frame.construction.last_result != ConstructionResultV1::CapacityFull
    {
        return Err(format!(
            "capacity lane did not fill atomically: {:?}",
            capacity.final_frame.construction
        ));
    }

    let mut captures = capture_checkpoints(
        &interaction_script,
        &interaction.frames,
        &staging,
        &["reuse-fall-stack", "high-cadence-place", "two-touch-cancel"],
    )?;
    captures.extend(capture_checkpoints(
        &invalidation_script,
        &invalidation.frames,
        &staging,
        &[
            "warm-contact",
            "reuse-empty-generation",
            "generation-recontact",
        ],
    )?);
    captures.sort();
    let platform_artifacts = copy_platform_artifacts(&args.platforms, &staging)?;
    let reused = find(&invalidation.frames, "reuse-empty-generation")?
        .construction
        .last_body
        .ok_or_else(|| "reuse checkpoint omitted its body identity".to_owned())?;
    let outcome = Outcome {
        interaction_frames: interaction.frames.len(),
        interaction_pointer_events: interaction_script
            .frames
            .iter()
            .flat_map(|frame| &frame.events)
            .filter(|event| matches!(event, physbox_engine::InputEvent::Pointer(_)))
            .count() as u64,
        high_cadence_intents_in_one_step: 8,
        capacity_body_slots: capacity.final_frame.construction.capacity_bodies,
        successful_capacity_placements: capacity.final_frame.construction.succeeded_intents,
        explicit_capacity_failures: capacity.final_frame.construction.capacity_failures,
        explicit_deep_overlap_failures: deep_overlap.final_frame.construction.deep_overlap_failures,
        reused_slot: reused.slot,
        reused_generation: reused.generation,
        stale_graph_counts_after_erase: graph_total(
            &find(&invalidation.frames, "erase-warm-body")?.graph,
        ),
        stale_graph_counts_after_reuse: graph_total(
            &find(&invalidation.frames, "reuse-empty-generation")?.graph,
        ),
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
    };
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s5.3-gpu-construction",
        run_id: args.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        adapter: interaction.adapter,
        contract: ContractEvidence {
            intent_bytes: physbox_engine::CONSTRUCTION_INTENT_BYTES,
            intent_capacity: physbox_engine::CONSTRUCTION_INTENT_CAPACITY,
            state_bytes: physbox_engine::CONSTRUCTION_STATE_BYTES,
            circle_radius: physbox_engine::CONSTRUCTION_CIRCLE_RADIUS,
            rectangle_half_extents: physbox_engine::CONSTRUCTION_RECTANGLE_HALF_EXTENTS,
            diagnostic_source: "async-gpu",
            diagnostic_readback_blocks_advancement: false,
            product_cpu_occupancy_mirror: false,
            validation_readback_scope: "bounded headless evidence after an exact requested frame",
        },
        interaction_script,
        interaction_trace: interaction.frames,
        interaction_exact_replay,
        invalidation_script,
        invalidation_trace: invalidation.frames,
        invalidation_exact_replay,
        deep_overlap_script,
        deep_overlap,
        capacity_script,
        capacity,
        captures,
        platform_artifacts,
        physical_device_gap: args.physical_device_gap,
        outcome,
    };
    atomic_json(&staging.join("manifest.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &args.output).map_err(|error| error.to_string())?;
    sync_directory(args.output.parent().unwrap_or(Path::new(".")))?;
    let bytes = fs::read(args.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario=s5.3-construction run_id={} manifest_sha256={} output={}",
        args.run_id,
        sha256_bytes(&bytes),
        args.output.display()
    );
    Ok(())
}

fn run_script(script: &ConstructionValidationScriptV1) -> Result<TraceRun, String> {
    script.validate()?;
    let mut renderer = pollster::block_on(OffscreenRenderer::new(
        script.target_size[0],
        script.target_size[1],
        EngineConfig {
            scenario: script.initial_scenario,
        },
    ))?;
    let adapter = renderer.diagnostics().clone();
    let mut frames = Vec::with_capacity(script.frames.len());
    for requested in &script.frames {
        let frame = renderer.render(
            FrameInput::exact(requested.exact_steps).with_input_events(requested.events.clone()),
        );
        if frame.fixed_steps_this_frame != requested.exact_steps
            || frame.pacing.mode != PacingMode::ExplicitStep
            || frame
                .construction_diagnostic
                .blocking_readback_for_advancement
            || frame.body_commands.gpu_selection_readback
        {
            return Err(format!(
                "{} violated exact-step or nonblocking construction policy",
                requested.label
            ));
        }
        frames.push(FrameEvidence {
            label: requested.label,
            requested_exact_steps: requested.exact_steps,
            frame,
            construction: renderer.read_construction_state()?,
            graph: renderer.read_construction_graph_evidence()?,
            body_slots: renderer.read_construction_body_evidence()?,
            capture_body_zero: renderer.read_capture_state()?,
            capture_identity_zero: renderer.read_capture_identity()?,
        });
    }
    Ok(TraceRun { adapter, frames })
}

fn failure_run(script: &ConstructionValidationScriptV1) -> Result<FailureEvidence, String> {
    let first = run_script(script)?;
    let replay = run_script(script)?;
    let exact_replay = exact_frames(&first.frames, &replay.frames)?;
    Ok(FailureEvidence {
        exact_replay,
        final_frame: first
            .frames
            .into_iter()
            .last()
            .ok_or_else(|| "failure script emitted no frames".to_owned())?,
    })
}

fn capture_checkpoints(
    script: &ConstructionValidationScriptV1,
    expected: &[FrameEvidence],
    staging: &Path,
    labels: &[&str],
) -> Result<Vec<String>, String> {
    let mut output = Vec::new();
    for &label in labels {
        let mut renderer = pollster::block_on(OffscreenRenderer::new(
            script.target_size[0],
            script.target_size[1],
            EngineConfig {
                scenario: script.initial_scenario,
            },
        ))?;
        let mut found = false;
        for requested in &script.frames {
            if requested.exact_steps <= 1 {
                renderer.render(
                    FrameInput::exact(requested.exact_steps)
                        .with_input_events(requested.events.clone()),
                );
            } else {
                renderer.render(FrameInput::exact(1).with_input_events(requested.events.clone()));
                for _ in 1..requested.exact_steps {
                    renderer.render(FrameInput::exact(1));
                }
            }
            if requested.label == label {
                found = true;
                break;
            }
        }
        if !found {
            return Err(format!(
                "capture checkpoint {label} is not in the shared script"
            ));
        }
        let expected_frame = find(expected, label)?;
        let actual_slots = renderer.read_construction_body_evidence()?;
        if actual_slots != expected_frame.body_slots {
            return Err(format!(
                "one-step capture replay differed from batched exact state at {label}"
            ));
        }
        let presentation = renderer.render(FrameInput::exact(0));
        if presentation.fixed_steps_this_frame != 0 {
            return Err(format!("capture checkpoint {label} advanced simulation"));
        }
        let relative = format!("captures/{label}.png");
        write_png(
            &staging.join(&relative),
            script.target_size[0],
            script.target_size[1],
            &renderer.read_pixels()?,
        )?;
        output.push(relative);
    }
    Ok(output)
}

fn exact_frames(left: &[FrameEvidence], right: &[FrameEvidence]) -> Result<bool, String> {
    let left = serde_json::to_vec(left).map_err(|error| error.to_string())?;
    let right = serde_json::to_vec(right).map_err(|error| error.to_string())?;
    Ok(left == right)
}

fn validate_interaction(frames: &[FrameEvidence]) -> Result<(), String> {
    let placed_circle = find(frames, "place-circle")?;
    let placed_rectangle = find(frames, "place-rectangle")?;
    let erased = find(frames, "erase-rectangle")?;
    let reused = find(frames, "place-reused-circle")?;
    let high_cadence = find(frames, "high-cadence-place")?;
    let cancelled = find(frames, "two-touch-cancel")?;
    let panned = find(frames, "secondary-pan")?;
    let zoomed = find(frames, "wheel-zoom")?;
    let recentered = find(frames, "recenter")?;
    let restarted = find(frames, "restart")?;
    let switched = find(frames, "switch-rectangle-stack")?;
    let returned = find(frames, "switch-bootstrap")?;
    if placed_circle.construction.placed_circles != 1
        || placed_rectangle.construction.placed_rectangles != 1
        || erased.construction.erased_bodies != 1
        || reused.construction.last_body
            != Some(physbox_contracts::BodyId {
                slot: 3,
                generation: 2,
            })
        || high_cadence.construction.attempted_intents != 12
        || high_cadence.construction.succeeded_intents != 12
        || high_cadence.construction.live_bodies != 12
        || cancelled.construction != high_cadence.construction
        || panned.frame.camera.pan_changes_total != 1
        || zoomed.frame.camera.zoom_changes_total != 1
        || recentered.frame.camera.recenter_changes_total != 1
        || restarted.construction.attempted_intents != 0
        || restarted.construction.live_bodies != 2
        || switched.frame.scene_contract.scenario != "rectangle-stack"
        || switched.construction.live_bodies != 5
        || returned.frame.scene_contract.scenario != "bootstrap"
        || returned.construction.live_bodies != 2
    {
        return Err("construction interaction trace failed a lifecycle invariant".to_owned());
    }
    Ok(())
}

fn validate_invalidation(frames: &[FrameEvidence]) -> Result<(), String> {
    let warm = find(frames, "warm-contact")?;
    let erased = find(frames, "erase-warm-body")?;
    let reused = find(frames, "reuse-empty-generation")?;
    let rebuilt = find(frames, "generation-recontact")?;
    if graph_total(&warm.graph) == 0
        || graph_total(&erased.graph) != 0
        || graph_total(&reused.graph) != 0
        || graph_total(&rebuilt.graph) == 0
        || reused.construction.last_body
            != Some(physbox_contracts::BodyId {
                slot: 0,
                generation: 2,
            })
        || [warm, erased, reused, rebuilt]
            .iter()
            .any(|frame| frame.graph.overflowed)
    {
        return Err("construction invalidation trace retained stale graph state".to_owned());
    }
    Ok(())
}

fn find<'a>(frames: &'a [FrameEvidence], label: &str) -> Result<&'a FrameEvidence, String> {
    frames
        .iter()
        .find(|frame| frame.label == label)
        .ok_or_else(|| format!("construction evidence omitted {label}"))
}

fn graph_total(graph: &GpuConstructionGraphEvidence) -> u32 {
    graph
        .candidate_pairs
        .saturating_add(graph.manifolds)
        .saturating_add(graph.contact_points)
        .saturating_add(graph.persistent_pairs)
}

fn copy_platform_artifacts(
    inputs: &[(String, PathBuf)],
    staging: &Path,
) -> Result<Vec<PlatformArtifact>, String> {
    let mut output = Vec::new();
    for (label, source) in inputs {
        if !source.is_file() {
            return Err(format!(
                "platform artifact is missing: {}",
                source.display()
            ));
        }
        let extension = source
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap_or("bin");
        let filename = format!(
            "{}.{}",
            label.replace(|ch: char| !ch.is_ascii_alphanumeric(), "-"),
            extension
        );
        let relative = format!("platform/{filename}");
        let bytes = fs::read(source).map_err(|error| error.to_string())?;
        atomic_write(&staging.join(&relative), &bytes)?;
        output.push(PlatformArtifact {
            label: label.clone(),
            path: relative,
            sha256: sha256_bytes(&bytes),
        });
    }
    Ok(output)
}

fn write_png(path: &Path, width: u32, height: u32, pixels: &[u8]) -> Result<(), String> {
    let expected_bytes = width as usize * height as usize * 4;
    if pixels.len() != expected_bytes {
        return Err(format!(
            "capture {} has {} bytes, expected {expected_bytes}",
            path.display(),
            pixels.len()
        ));
    }
    if pixels
        .chunks_exact(4)
        .any(|pixel| pixel[3] != u8::MAX || pixel[..3] == [0, 0, 0])
    {
        return Err(format!(
            "capture {} contains transparent or unrendered black pixels",
            path.display()
        ));
    }
    let file = File::create(path).map_err(|error| error.to_string())?;
    let mut encoder = png::Encoder::new(BufWriter::new(file), width, height);
    encoder.set_color(png::ColorType::Rgba);
    encoder.set_depth(png::BitDepth::Eight);
    let mut writer = encoder.write_header().map_err(|error| error.to_string())?;
    writer
        .write_image_data(pixels)
        .map_err(|error| error.to_string())
}

fn parse_args() -> Result<Args, String> {
    let mut run_id = None;
    let mut output = None;
    let mut require_clean = false;
    let mut platforms = Vec::new();
    let mut physical_device_gap = None;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        if argument == "--require-clean" {
            require_clean = true;
            continue;
        }
        let value = arguments
            .next()
            .ok_or_else(|| format!("missing value for {argument}"))?;
        match argument.as_str() {
            "--run-id" => run_id = Some(value),
            "--output" => output = Some(PathBuf::from(value)),
            "--platform" => {
                let (label, path) = value
                    .split_once('=')
                    .ok_or_else(|| "platform must be LABEL=PATH".to_owned())?;
                platforms.push((label.to_owned(), PathBuf::from(path)));
            }
            "--physical-device-gap" => physical_device_gap = Some(value),
            _ => return Err(format!("unknown argument {argument}")),
        }
    }
    Ok(Args {
        run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
        output: output.ok_or_else(|| "missing --output".to_owned())?,
        require_clean,
        platforms,
        physical_device_gap,
    })
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = git(repository, &["rev-parse", "HEAD"])?;
    let status = git_bytes(
        repository,
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let dirty = !status.is_empty();
    let digest = if dirty {
        let mut patch = status;
        patch.extend(git_bytes(repository, &["diff", "--binary", "HEAD", "--"])?);
        let untracked = git_bytes(
            repository,
            &["ls-files", "--others", "--exclude-standard", "-z"],
        )?;
        for raw_path in untracked
            .split(|byte| *byte == 0)
            .filter(|path| !path.is_empty())
        {
            let relative = std::str::from_utf8(raw_path).map_err(|error| error.to_string())?;
            patch.extend_from_slice(relative.as_bytes());
            patch.push(0);
            patch.extend(fs::read(repository.join(relative)).map_err(|error| error.to_string())?);
            patch.push(0);
        }
        Some(sha256_bytes(&patch))
    } else {
        None
    };
    Ok((revision, dirty, digest))
}

fn git(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    Ok(String::from_utf8_lossy(&git_bytes(repository, arguments)?)
        .trim()
        .to_owned())
}

fn git_bytes(repository: &Path, arguments: &[&str]) -> Result<Vec<u8>, String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).into_owned());
    }
    Ok(output.stdout)
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let partial = path.with_extension("partial");
    if path.exists() || partial.exists() {
        return Err(format!("fresh artifact already exists: {}", path.display()));
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&partial)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    fs::rename(partial, path).map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}
