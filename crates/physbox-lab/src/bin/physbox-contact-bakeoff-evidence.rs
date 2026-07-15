use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

use physbox_contracts::{FailureV1, SCHEMA_VERSION, ScenarioV1, canonical_hash, sha256_bytes};
use physbox_engine::{
    GpuGatherTopology, GpuGeneratedContactRunner, GpuPairSlotPersistenceRunner,
    P5_2CandidatePattern, P5_2GeneratedScenarioKind, authored_chain_scenario,
    benchmark_gpu_authored_gather_topology, benchmark_gpu_generated_contacts_with_gather_topology,
    benchmark_gpu_pair_slot_persistence, p5_2_candidate_scenario, p5_2_contact_policy_candidate,
    p5_2_generated_performance_scenario, p5_2_generated_scenario,
};
use physbox_lab::{
    analytic_narrowphase::generate_scalar_narrowphase,
    collision_candidates::enumerate_scalar_candidates,
};
use serde::Serialize;

const RANDOMIZED_SEEDS: u64 = 16;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Manifest {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    adapter: physbox_engine::AdapterDiagnostics,
    policy: PolicyEvidence,
    persistence: Vec<PersistenceRow>,
    generated_topology: Vec<GeneratedTopologyRow>,
    contact_rich_topology: Vec<ContactRichRow>,
    randomized: RandomizedEvidence,
    capacity: CapacityEvidence,
    selection_made: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PolicyEvidence {
    speculative_distance: f32,
    maximum_depenetration_velocity: f32,
    speculative_gap_baseline_contacts: usize,
    speculative_gap_candidate_contacts: usize,
    speculative_gap_baseline_penetration: f32,
    speculative_gap_candidate_penetration: f32,
    coincident_baseline_maximum_speed: f32,
    coincident_candidate_maximum_speed: f32,
    behavior_failures: u32,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistenceRow {
    bodies: u32,
    expected_contacts: u32,
    source_scenario_hash: String,
    baseline_dispatches: u64,
    pair_slot_dispatches: u32,
    baseline_allocated_bytes: u64,
    pair_slot_allocated_bytes: u64,
    baseline_persistence: Option<Timing>,
    pair_slot_persistence: Option<Timing>,
    pair_slot_pipeline: Option<Timing>,
    baseline_capacity_accepted: bool,
    pair_slot_capacity_accepted: bool,
    pair_slot_correctness_qualified: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct GeneratedTopologyRow {
    bodies: u32,
    expected_contacts: u32,
    source_scenario_hash: String,
    result_exact: bool,
    baseline_dispatches: u32,
    stable_edge_dispatches: u32,
    baseline_topology_bytes: u64,
    stable_edge_topology_bytes: u64,
    baseline_end_to_end: Option<Timing>,
    stable_edge_end_to_end: Option<Timing>,
    baseline_capacity_accepted: bool,
    stable_edge_capacity_accepted: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContactRichRow {
    contacts: u32,
    bodies: u32,
    source_scenario_hash: String,
    result_exact: bool,
    baseline_dispatches: u32,
    stable_edge_dispatches: u32,
    baseline_allocated_bytes: u64,
    stable_edge_allocated_bytes: u64,
    baseline_topology_bytes: u64,
    stable_edge_topology_bytes: u64,
    baseline_total: Option<Timing>,
    stable_edge_total: Option<Timing>,
    baseline_index_build: Option<Timing>,
    stable_edge_index_build: Option<Timing>,
    baseline_offsets: Option<Timing>,
    stable_edge_offsets: Option<Timing>,
    baseline_gather: Option<Timing>,
    stable_edge_gather: Option<Timing>,
    baseline_solve: Option<Timing>,
    stable_edge_solve: Option<Timing>,
    baseline_scan_attempts: u64,
    stable_edge_scan_attempts: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RandomizedEvidence {
    seeds: u64,
    cases: u64,
    topology_differences: u64,
    pair_slot_replay_differences: u64,
    pair_slot_permutation_differences: u64,
    accepted_overflows: u64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapacityEvidence {
    baseline_failure: Option<FailureV1>,
    stable_edge_failure: Option<FailureV1>,
    pair_slot_failure: Option<FailureV1>,
    baseline_committed_cache_points: usize,
    stable_edge_committed_cache_points: usize,
    pair_slot_committed_cache_points: usize,
    failures_equivalent: bool,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct Timing {
    samples: usize,
    minimum_seconds: f64,
    p50_seconds: f64,
    p95_seconds: f64,
    p99_seconds: f64,
    maximum_seconds: f64,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-contact-bakeoff-evidence error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("contact bakeoff evidence requires a clean source tree".to_owned());
    }
    let staging = arguments.output.with_extension("partial");
    if arguments.output.exists() || staging.exists() {
        return Err("contact bakeoff output and staging paths must be fresh".to_owned());
    }
    fs::create_dir_all(&staging).map_err(|error| error.to_string())?;

    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let policy = policy_evidence(&runner)?;
    let mut persistence = Vec::new();
    let mut generated_topology = Vec::new();
    for bodies in [32_u32, 256, 1024] {
        let scenario =
            p5_2_generated_performance_scenario(bodies).map_err(|error| error.to_string())?;
        let baseline = pollster::block_on(benchmark_gpu_generated_contacts_with_gather_topology(
            &scenario,
            arguments.warmup,
            arguments.samples,
            GpuGatherTopology::CanonicalBodyScan,
        ))?;
        let stable = pollster::block_on(benchmark_gpu_generated_contacts_with_gather_topology(
            &scenario,
            arguments.warmup,
            arguments.samples,
            GpuGatherTopology::StableBodyEdge,
        ))?;
        let pair = pollster::block_on(benchmark_gpu_pair_slot_persistence(
            &scenario,
            arguments.warmup,
            arguments.samples,
        ))?;
        let source_hash = canonical_hash(&scenario).map_err(|error| error.to_string())?;
        if baseline.source_scenario_hash != source_hash
            || stable.source_scenario_hash != source_hash
            || pair.source_scenario_hash != source_hash
        {
            return Err(format!("{bodies}-body benchmark source hashes differ"));
        }
        let pair_step = pair
            .correctness
            .steps
            .first()
            .ok_or_else(|| "pair-slot benchmark omitted correctness step".to_owned())?;
        persistence.push(PersistenceRow {
            bodies,
            expected_contacts: bodies / 2,
            source_scenario_hash: source_hash.clone(),
            baseline_dispatches: baseline.correctness.work.dispatches,
            pair_slot_dispatches: pair.dispatches_per_step,
            baseline_allocated_bytes: baseline.allocated_bytes,
            pair_slot_allocated_bytes: pair.allocated_bytes,
            baseline_persistence: optional_timing(&baseline.gpu_persistence_seconds)?,
            pair_slot_persistence: optional_timing(&pair.gpu_persistence_seconds)?,
            pair_slot_pipeline: optional_timing(&pair.gpu_pipeline_seconds)?,
            baseline_capacity_accepted: capacities_accepted(&baseline.correctness),
            pair_slot_capacity_accepted: pair_step.capacities.iter().all(|value| value.accepted),
            pair_slot_correctness_qualified: pair_step.failure.is_none()
                && pair.correctness.mapped_after_final_step_only,
        });
        let result_exact = baseline.correctness.bodies == stable.correctness.bodies
            && baseline.correctness.cache == stable.correctness.cache
            && baseline.correctness.work == stable.correctness.work
            && baseline.correctness.failure == stable.correctness.failure;
        generated_topology.push(GeneratedTopologyRow {
            bodies,
            expected_contacts: bodies / 2,
            source_scenario_hash: source_hash,
            result_exact,
            baseline_dispatches: baseline.dispatches_per_step,
            stable_edge_dispatches: stable.dispatches_per_step,
            baseline_topology_bytes: baseline.correctness.gather_topology.allocated_bytes,
            stable_edge_topology_bytes: stable.correctness.gather_topology.allocated_bytes,
            baseline_end_to_end: optional_timing(&baseline.gpu_end_to_end_seconds)?,
            stable_edge_end_to_end: optional_timing(&stable.gpu_end_to_end_seconds)?,
            baseline_capacity_accepted: capacities_accepted(&baseline.correctness),
            stable_edge_capacity_accepted: capacities_accepted(&stable.correctness),
        });
    }

    let mut contact_rich_topology = Vec::new();
    for contacts in [1_000_u32, 10_000] {
        let scenario =
            authored_chain_scenario(contacts + 1, 0x5a17).map_err(|error| error.to_string())?;
        let baseline = pollster::block_on(benchmark_gpu_authored_gather_topology(
            &scenario,
            arguments.warmup,
            arguments.samples,
            GpuGatherTopology::CanonicalBodyScan,
        ))?;
        let stable = pollster::block_on(benchmark_gpu_authored_gather_topology(
            &scenario,
            arguments.warmup,
            arguments.samples,
            GpuGatherTopology::StableBodyEdge,
        ))?;
        contact_rich_topology.push(ContactRichRow {
            contacts,
            bodies: contacts + 1,
            source_scenario_hash: baseline.source_scenario_hash.clone(),
            result_exact: baseline.result_digest == stable.result_digest
                && baseline.replay_exact
                && stable.replay_exact,
            baseline_dispatches: baseline.dispatches_per_step,
            stable_edge_dispatches: stable.dispatches_per_step,
            baseline_allocated_bytes: baseline.allocated_bytes,
            stable_edge_allocated_bytes: stable.allocated_bytes,
            baseline_topology_bytes: baseline.topology.allocated_bytes,
            stable_edge_topology_bytes: stable.topology.allocated_bytes,
            baseline_total: optional_timing(&baseline.gpu_total_seconds)?,
            stable_edge_total: optional_timing(&stable.gpu_total_seconds)?,
            baseline_index_build: optional_timing(&baseline.instrumented_index_build_seconds)?,
            stable_edge_index_build: optional_timing(&stable.instrumented_index_build_seconds)?,
            baseline_offsets: optional_timing(&baseline.instrumented_offset_construction_seconds)?,
            stable_edge_offsets: optional_timing(&stable.instrumented_offset_construction_seconds)?,
            baseline_gather: optional_timing(&baseline.instrumented_gather_seconds)?,
            stable_edge_gather: optional_timing(&stable.instrumented_gather_seconds)?,
            baseline_solve: optional_timing(&baseline.instrumented_solve_seconds)?,
            stable_edge_solve: optional_timing(&stable.instrumented_solve_seconds)?,
            baseline_scan_attempts: baseline.topology.canonical_scan_attempts,
            stable_edge_scan_attempts: stable.topology.canonical_scan_attempts,
        });
    }
    let randomized = randomized_evidence(&runner)?;
    let capacity = capacity_evidence(&runner)?;
    let adapter = runner.adapter().clone();
    let manifest = Manifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2-s3.5a-contact-bakeoff",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        adapter,
        policy,
        persistence,
        generated_topology,
        contact_rich_topology,
        randomized,
        capacity,
        selection_made: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
    };
    validate_manifest(&manifest, arguments.samples)?;
    atomic_json(&staging.join("matrix.json"), &manifest)?;
    sync_directory(&staging)?;
    fs::rename(&staging, &arguments.output).map_err(|error| error.to_string())?;
    sync_directory(arguments.output.parent().unwrap_or(Path::new(".")))?;
    println!(
        "PHYSBOX5_CONTACT_BAKEOFF_EVIDENCE_SUCCESS run_id={} output={}",
        arguments.run_id,
        arguments.output.display()
    );
    Ok(())
}

fn validate_manifest(manifest: &Manifest, samples: u32) -> Result<(), String> {
    let expected_samples = samples as usize;
    let timing_valid = |value: &Option<Timing>| {
        value
            .as_ref()
            .is_some_and(|timing| timing.samples == expected_samples)
    };
    if manifest.selection_made
        || manifest.physical_defaults_changed
        || manifest.numerical_tolerances_changed
    {
        return Err("bakeoff automation must not select a representation or default".to_owned());
    }
    if manifest.policy.speculative_gap_baseline_contacts != 0
        || manifest.policy.speculative_gap_candidate_contacts == 0
        || manifest.policy.speculative_gap_candidate_penetration
            >= manifest.policy.speculative_gap_baseline_penetration
        || manifest.policy.coincident_candidate_maximum_speed
            >= manifest.policy.coincident_baseline_maximum_speed
        || manifest.policy.behavior_failures != 0
    {
        return Err("contact-policy evidence failed its preserved reproducers".to_owned());
    }
    if manifest.persistence.len() != 3
        || manifest.persistence.iter().any(|row| {
            !row.baseline_capacity_accepted
                || !row.pair_slot_capacity_accepted
                || !row.pair_slot_correctness_qualified
                || !timing_valid(&row.baseline_persistence)
                || !timing_valid(&row.pair_slot_persistence)
                || !timing_valid(&row.pair_slot_pipeline)
        })
    {
        return Err("persistence matrix failed correctness, capacity, or timing gates".to_owned());
    }
    if manifest.generated_topology.len() != 3
        || manifest.generated_topology.iter().any(|row| {
            !row.result_exact
                || !row.baseline_capacity_accepted
                || !row.stable_edge_capacity_accepted
                || !timing_valid(&row.baseline_end_to_end)
                || !timing_valid(&row.stable_edge_end_to_end)
        })
    {
        return Err(
            "generated topology matrix failed exactness, capacity, or timing gates".to_owned(),
        );
    }
    if manifest.contact_rich_topology.len() != 2
        || manifest.contact_rich_topology.iter().any(|row| {
            !row.result_exact
                || row.stable_edge_scan_attempts != 0
                || !timing_valid(&row.baseline_total)
                || !timing_valid(&row.stable_edge_total)
                || !timing_valid(&row.baseline_index_build)
                || !timing_valid(&row.stable_edge_index_build)
                || !timing_valid(&row.baseline_offsets)
                || !timing_valid(&row.stable_edge_offsets)
                || !timing_valid(&row.baseline_gather)
                || !timing_valid(&row.stable_edge_gather)
                || !timing_valid(&row.baseline_solve)
                || !timing_valid(&row.stable_edge_solve)
        })
    {
        return Err("contact-rich topology matrix failed exactness or timing gates".to_owned());
    }
    if manifest.randomized.topology_differences != 0
        || manifest.randomized.pair_slot_replay_differences != 0
        || manifest.randomized.pair_slot_permutation_differences != 0
        || manifest.randomized.accepted_overflows != 0
        || !manifest.capacity.failures_equivalent
    {
        return Err("randomized replay or atomic capacity evidence failed".to_owned());
    }
    Ok(())
}

fn policy_evidence(runner: &GpuGeneratedContactRunner) -> Result<PolicyEvidence, String> {
    let mut gap = p5_2_generated_scenario(P5_2GeneratedScenarioKind::RectangleFloor, 1)
        .map_err(|error| error.to_string())?;
    gap.id = "p5.2-s3.5a-speculative-gap".to_owned();
    gap.bodies[0].position.y = 0.205;
    gap.bodies[0].linear_velocity.y = -0.5;
    gap.settings.contact_policy = None;
    let baseline_gap = runner.run(&gap, 1)?;
    let mut candidate_gap = gap.clone();
    candidate_gap.settings.contact_policy = Some(p5_2_contact_policy_candidate());
    let candidate_gap_output = runner.run(&candidate_gap, 1)?;
    let overlap = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 6, 19)
        .map_err(|error| error.to_string())?;
    let baseline_overlap = runner.run(&overlap, 1)?;
    let mut candidate_overlap = overlap;
    candidate_overlap.settings.contact_policy = Some(p5_2_contact_policy_candidate());
    let candidate_overlap_output = runner.run(&candidate_overlap, 1)?;
    let mut behavior_failures = 0;
    for kind in P5_2GeneratedScenarioKind::ALL {
        let mut scenario = p5_2_generated_scenario(kind, 1).map_err(|error| error.to_string())?;
        scenario.settings.contact_policy = Some(p5_2_contact_policy_candidate());
        let output = runner.run(&scenario, 1)?;
        behavior_failures += u32::from(output.failure.is_some());
    }
    let policy = p5_2_contact_policy_candidate();
    Ok(PolicyEvidence {
        speculative_distance: policy.speculative_distance.get(),
        maximum_depenetration_velocity: policy.max_depenetration_velocity.get(),
        speculative_gap_baseline_contacts: baseline_gap.cache.len(),
        speculative_gap_candidate_contacts: candidate_gap_output.cache.len(),
        speculative_gap_baseline_penetration: maximum_penetration(&gap, &baseline_gap.bodies)?,
        speculative_gap_candidate_penetration: maximum_penetration(
            &candidate_gap,
            &candidate_gap_output.bodies,
        )?,
        coincident_baseline_maximum_speed: maximum_speed(&baseline_overlap.bodies),
        coincident_candidate_maximum_speed: maximum_speed(&candidate_overlap_output.bodies),
        behavior_failures,
    })
}

fn randomized_evidence(runner: &GpuGeneratedContactRunner) -> Result<RandomizedEvidence, String> {
    let pair_runner = pollster::block_on(GpuPairSlotPersistenceRunner::new())?;
    let patterns = [
        P5_2CandidatePattern::Random,
        P5_2CandidatePattern::Touching,
        P5_2CandidatePattern::RotatedThin,
        P5_2CandidatePattern::FixedOnly,
    ];
    let mut topology_differences = 0;
    let mut pair_slot_replay_differences = 0;
    let mut pair_slot_permutation_differences = 0;
    let mut accepted_overflows = 0;
    for seed in 0..RANDOMIZED_SEEDS {
        for pattern in patterns {
            let scenario =
                p5_2_candidate_scenario(pattern, 16, seed).map_err(|error| error.to_string())?;
            let baseline = runner.run(&scenario, 2)?;
            let stable = runner.run_with_gather_topology(
                &scenario,
                2,
                GpuGatherTopology::StableBodyEdge,
                false,
            )?;
            topology_differences += u64::from(
                baseline.bodies != stable.bodies
                    || baseline.cache != stable.cache
                    || baseline.work != stable.work
                    || baseline.failure != stable.failure,
            );
            let pair = pair_runner.run_sequence(
                std::slice::from_ref(&scenario),
                &[],
                None,
                false,
                u32::MAX,
            )?;
            let replay = pair_runner.run_sequence(
                std::slice::from_ref(&scenario),
                &[],
                None,
                false,
                u32::MAX,
            )?;
            let permuted = pair_runner.run_sequence(
                std::slice::from_ref(&scenario),
                &[],
                None,
                true,
                u32::MAX,
            )?;
            pair_slot_replay_differences += u64::from(pair.steps != replay.steps);
            pair_slot_permutation_differences += u64::from(pair.steps != permuted.steps);
            accepted_overflows += u64::from(
                baseline.failure.is_none()
                    && baseline
                        .capacities
                        .iter()
                        .any(|capacity| capacity.overflowed),
            );
        }
    }
    Ok(RandomizedEvidence {
        seeds: RANDOMIZED_SEEDS,
        cases: RANDOMIZED_SEEDS * patterns.len() as u64,
        topology_differences,
        pair_slot_replay_differences,
        pair_slot_permutation_differences,
        accepted_overflows,
    })
}

fn capacity_evidence(runner: &GpuGeneratedContactRunner) -> Result<CapacityEvidence, String> {
    let scenario = p5_2_candidate_scenario(P5_2CandidatePattern::Coincident, 34, 97)
        .map_err(|error| error.to_string())?;
    let baseline = runner.run(&scenario, 1)?;
    let stable =
        runner.run_with_gather_topology(&scenario, 1, GpuGatherTopology::StableBodyEdge, false)?;
    let pair_runner = pollster::block_on(GpuPairSlotPersistenceRunner::new())?;
    let pair = pair_runner.run_sequence_with_candidate_capacity(
        std::slice::from_ref(&scenario),
        &[],
        Some(34 * 16),
        None,
        false,
        u32::MAX,
    )?;
    let pair_step = pair
        .steps
        .first()
        .ok_or_else(|| "capacity pair-slot lane omitted its step".to_owned())?;
    let failures_equivalent = baseline.failure == stable.failure
        && baseline.cache.is_empty()
        && stable.cache.is_empty()
        && pair_step.cache.is_empty()
        && pair_step.failure.is_some();
    Ok(CapacityEvidence {
        baseline_failure: baseline.failure,
        stable_edge_failure: stable.failure,
        pair_slot_failure: pair_step.failure.clone(),
        baseline_committed_cache_points: baseline.cache.len(),
        stable_edge_committed_cache_points: stable.cache.len(),
        pair_slot_committed_cache_points: pair_step.cache.len(),
        failures_equivalent,
    })
}

fn maximum_penetration(
    scenario: &ScenarioV1,
    bodies: &[physbox_contracts::BodyStateV1],
) -> Result<f32, String> {
    let mut final_scenario = scenario.clone();
    final_scenario.bodies.clone_from_slice(bodies);
    let candidates = enumerate_scalar_candidates(&final_scenario)?;
    let manifolds = generate_scalar_narrowphase(&final_scenario, &candidates.candidates)?;
    Ok(manifolds
        .iter()
        .flat_map(|manifold| &manifold.points)
        .map(|point| (-point.separation.get()).max(0.0))
        .fold(0.0, f32::max))
}

fn maximum_speed(bodies: &[physbox_contracts::BodyStateV1]) -> f32 {
    bodies
        .iter()
        .map(|body| body.linear_velocity.x.hypot(body.linear_velocity.y))
        .fold(0.0, f32::max)
}

fn capacities_accepted(output: &physbox_engine::GpuGeneratedContactOutput) -> bool {
    output.failure.is_none() && output.capacities.iter().all(|capacity| capacity.accepted)
}

fn optional_timing(samples: &Option<Vec<f64>>) -> Result<Option<Timing>, String> {
    samples.as_deref().map(timing).transpose()
}

fn timing(samples: &[f64]) -> Result<Timing, String> {
    if samples.is_empty()
        || samples
            .iter()
            .any(|sample| !sample.is_finite() || *sample < 0.0)
    {
        return Err("timing samples must be nonempty, finite, and nonnegative".to_owned());
    }
    let mut sorted = samples.to_vec();
    sorted.sort_by(f64::total_cmp);
    let percentile = |value: f64| sorted[((sorted.len() - 1) as f64 * value).ceil() as usize];
    Ok(Timing {
        samples: sorted.len(),
        minimum_seconds: sorted[0],
        p50_seconds: percentile(0.50),
        p95_seconds: percentile(0.95),
        p99_seconds: percentile(0.99),
        maximum_seconds: *sorted.last().expect("nonempty timing samples"),
    })
}

struct Arguments {
    run_id: String,
    output: PathBuf,
    warmup: u32,
    samples: u32,
    require_clean: bool,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut values = std::env::args().skip(1);
        let mut run_id = None;
        let mut output = None;
        let mut warmup = 5;
        let mut samples = 30;
        let mut require_clean = false;
        while let Some(argument) = values.next() {
            match argument.as_str() {
                "--run-id" => run_id = values.next(),
                "--output" => output = values.next().map(PathBuf::from),
                "--warmup" => warmup = parse_u32(values.next(), "warmup")?,
                "--samples" => samples = parse_u32(values.next(), "samples")?,
                "--require-clean" => require_clean = true,
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        let run_id = run_id.ok_or_else(usage)?;
        let output = output.ok_or_else(usage)?;
        if run_id.is_empty() || warmup == 0 || samples == 0 {
            return Err(usage());
        }
        Ok(Self {
            run_id,
            output,
            warmup,
            samples,
            require_clean,
        })
    }
}

fn parse_u32(value: Option<String>, name: &str) -> Result<u32, String> {
    value
        .ok_or_else(usage)?
        .parse()
        .map_err(|_| format!("invalid {name}"))
}

fn usage() -> String {
    "usage: physbox-contact-bakeoff-evidence --run-id ID --output DIRECTORY [--warmup N] [--samples N] [--require-clean]".to_owned()
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = git(repository, &["rev-parse", "HEAD"])?;
    let status = git(repository, &["status", "--porcelain=v1"])?;
    let dirty = !status.is_empty();
    let patch_digest = dirty.then(|| sha256_bytes(status.as_bytes()));
    Ok((revision, dirty, patch_digest))
}

fn git(repository: &Path, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new("git")
        .current_dir(repository)
        .args(arguments)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(String::from_utf8_lossy(&output.stderr).to_string());
    }
    Ok(String::from_utf8_lossy(&output.stdout).trim().to_owned())
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| error.to_string())?;
    file.write_all(&bytes).map_err(|error| error.to_string())?;
    file.write_all(b"\n").map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}
