use std::{
    fs::{self, OpenOptions},
    io::Write,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{
    CONTACT_SOFTNESS_POLICY_VERSION, ContactIdentityPolicyV1, ContactSoftnessPolicyV1, FiniteF32,
    SCHEMA_VERSION, ScenarioV1, SettlingSummaryV2, canonical_hash, sha256_bytes,
};
use physbox_engine::{
    GpuGeneratedContactRunner, GpuGeneratedContactSeriesV1, GpuSoftFeedbackPolicyV1,
    GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1, P5_2_SETTLING_EVIDENCE_STEPS,
    p5_2_centered_rectangle_stack_scenario, p5_2_stale_reseed_reproducer_scenario,
};
use physbox_lab::{
    pass_energy::{ImpulseOperatorMechanismV1, analyze_impulse_operator},
    settling::{
        awake_settling_measurement_policy_v2, centered_stack_structure,
        observe_generated_settling_series_v2, summarize_settling_v2,
    },
    stationarity::{
        StationarityAnalysisV1, StationarityClassComparisonV1, StationarityClassVerdictV1,
        StationarityManifoldClassV1, StationaritySnapshotInputV1, analyze_stationarity_v1,
        compare_stationarity_v1, stationarity_capture_v1, stationarity_metric_contract_v1,
    },
};
use serde::{Deserialize, Serialize};

const TARGET_STEP: u32 = 904;
const HARD_DEPTHS: [u32; 11] = [2, 4, 8, 9, 10, 11, 12, 13, 14, 15, 16];
const S10_2_REVISION: &str = "41bdaaf3ccc1cc4f6bbb34a56da19b86510f1905";
const SOFT_CANDIDATE_ID: &str = "soft-hz30-d10-static1-prior";

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HardManifest {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    host: &'static str,
    target_step: u32,
    depths: Vec<HardDepthEvidence>,
    smallest_harmful_depth: u32,
    permanent_reproducer_source_hash: String,
    permanent_reproducer_exact_alias: bool,
    mechanism_verdict: HardMechanismVerdict,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HardDepthEvidence {
    depth: u32,
    label: String,
    source_scenario_hash: String,
    source: ArtifactIdentity,
    summary: SettlingSummaryV2,
    settled_speed_gate_failed: bool,
    energy_envelope_gate_failed: bool,
    harmful_signature: bool,
    structural_pass: bool,
    structural_reason: String,
    capacity_clean: bool,
    raw_series_hash: String,
    replay_raw_series_hash: String,
    exact_series_replay: bool,
    pass_trace_hash: String,
    replay_pass_trace_hash: String,
    exact_pass_trace_replay: bool,
    operator_report_hash: String,
    replay_operator_report_hash: String,
    exact_operator_replay: bool,
    trace: ArtifactIdentity,
    operator: ArtifactIdentity,
    mechanism: ImpulseOperatorMechanismV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct HardMechanismVerdict {
    named_mechanism: &'static str,
    classification: &'static str,
    smallest_reproducer: &'static str,
    correction_must_change: &'static str,
    correction_must_preserve: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SoftManifest {
    schema_version: u32,
    artifact_kind: &'static str,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
    host: &'static str,
    observation_runs: u32,
    inputs: SoftInputs,
    stationarity_contract: physbox_lab::stationarity::StationarityMetricContractV1,
    source_scenario_hash: String,
    source: ArtifactIdentity,
    capture: ArtifactIdentity,
    adapter: physbox_engine::AdapterDiagnostics,
    current_series_hash: String,
    legacy_series_projection_hash: String,
    frozen_raw_series_hash: String,
    frozen_legacy_projection_hash_exact: bool,
    fresh_summary: SettlingSummaryV2,
    frozen_summary: SettlingSummaryV2,
    frozen_summary_exact: bool,
    fresh_rising_energy_mode: bool,
    frozen_rising_energy_mode: bool,
    rising_energy_mode_preserved: bool,
    reobservation_context: &'static str,
    capacity_clean: bool,
    structural_pass: bool,
    structural_reason: String,
    soft_analysis: StationarityAnalysisV1,
    authority_analysis: StationarityAnalysisV1,
    class_comparisons: Vec<StationarityClassComparisonV1>,
    h3: H3Verdict,
    product_constructor_changed: bool,
    physical_defaults_changed: bool,
    numerical_tolerances_changed: bool,
    oracle_precedence_changed: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct SoftInputs {
    s6_4_run_id: String,
    s6_4_source_revision: String,
    s6_4_manifest_sha256: String,
    s6_4_frozen_trace: ArtifactIdentity,
    s10_2_run_id: String,
    s10_2_source_revision: String,
    s10_2_manifest_sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct H3Verdict {
    hard_normal_point_signature: bool,
    soft_normal_point_signature: bool,
    hard_normal_manifold_signature: bool,
    soft_normal_manifold_signature: bool,
    hard_active_or_redistribution_signature: bool,
    soft_active_or_redistribution_signature: bool,
    hard_tangent_signature: bool,
    soft_tangent_signature: bool,
    shared_point_normal_mode: bool,
    full_hard_signature_shared: bool,
    verdict: &'static str,
    rationale: &'static str,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S10_2Input {
    artifact_kind: String,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    physbox_analysis: StationarityAnalysisV1,
    authority_analysis: StationarityAnalysisV1,
    class_comparisons: Vec<StationarityClassComparisonV1>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S6_4Input {
    artifact_kind: String,
    run_id: String,
    source_revision: String,
    source_dirty: bool,
    full_results: Vec<S6_4Scene>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S6_4Scene {
    label: String,
    source: InputArtifactIdentity,
    candidates: Vec<S6_4Candidate>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S6_4Candidate {
    candidate: S6_4CandidateIdentity,
    candidate_scenario_hash: String,
    raw_trace_hash: String,
    trace: InputArtifactIdentity,
    replay_exact: bool,
    instrumentation_final_exact: bool,
    capacity_clean: bool,
    structural_pass: bool,
    summary: SettlingSummaryV2,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S6_4CandidateIdentity {
    id: String,
    identity_policy: ContactIdentityPolicyV1,
    softness: Option<ContactSoftnessPolicyV1>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct InputArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct S6_4LegacySeriesProjection<'a> {
    version: u32,
    adapter: &'a physbox_engine::AdapterDiagnostics,
    source_scenario_hash: &'a str,
    observation_interval_steps: u32,
    completed_steps: u32,
    solver_settings: &'a physbox_contracts::SolverSettingsV1,
    solver_execution: GpuSolverExecutionV4,
    solver_scheduling_policy: GpuSolverSchedulingPolicyV1,
    contact_identity_policy: ContactIdentityPolicyV1,
    dispatches_per_step: u32,
    allocated_bytes: u64,
    snapshots: &'a [physbox_engine::GpuGeneratedContactSnapshotV1],
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s10-mechanism error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let arguments = Arguments::parse()?;
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let (source_revision, source_dirty, source_patch_digest) = repository_state(&repository)?;
    if arguments.require_clean && source_dirty {
        return Err("S10.3 evidence requires a clean source tree".to_owned());
    }
    match arguments.phase {
        Phase::Hard => run_hard(
            &arguments,
            source_revision,
            source_dirty,
            source_patch_digest,
        ),
        Phase::Soft => run_soft(
            &arguments,
            source_revision,
            source_dirty,
            source_patch_digest,
        ),
    }
}

fn run_hard(
    arguments: &Arguments,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
) -> Result<(), String> {
    let staging = prepare_staging(&arguments.output, &["sources", "traces", "operators"])?;
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    let policy = awake_settling_measurement_policy_v2();
    let mut depths = Vec::new();
    for depth in HARD_DEPTHS {
        eprintln!("S10.3 hard depth {depth}");
        let source = p5_2_centered_rectangle_stack_scenario(depth, P5_2_SETTLING_EVIDENCE_STEPS)
            .map_err(|error| error.to_string())?;
        let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
        let source_artifact = write_json_artifact(
            &staging,
            &format!("sources/centered-stack-{depth}.json"),
            &source,
        )?;
        let raw = runner.run_selected_observation_series(&source, 1, 1_200)?;
        let replay = runner.run_selected_observation_series(&source, 1, 1_200)?;
        let raw_series_hash = serialized_hash(&raw)?;
        let replay_raw_series_hash = serialized_hash(&replay)?;
        let exact_series_replay = raw_series_hash == replay_raw_series_hash;
        let capacity_clean = raw.snapshots.iter().all(snapshot_capacity_clean)
            && replay.snapshots.iter().all(snapshot_capacity_clean);
        let mut structural_pass = true;
        let mut structural_reason = "structure not evaluated".to_owned();
        for snapshot in &raw.snapshots {
            let result = centered_stack_structure(
                &source,
                &snapshot.bodies,
                !snapshot_capacity_clean(snapshot),
                0.0,
            );
            structural_pass &= result.0;
            if snapshot.completed_steps == 1_200 {
                structural_reason = result.1;
            }
        }
        let observations = observe_generated_settling_series_v2(&source, &raw)?;
        let summary = summarize_settling_v2(&observations, policy)?;
        let trace = runner.run_contact_policy_pass_trace(
            &source,
            TARGET_STEP,
            ContactIdentityPolicyV1::SELECTED,
            physbox_engine::GpuSoftFeedbackPolicyV1::SELECTED,
        )?;
        let replay_trace = runner.run_contact_policy_pass_trace(
            &source,
            TARGET_STEP,
            ContactIdentityPolicyV1::SELECTED,
            physbox_engine::GpuSoftFeedbackPolicyV1::SELECTED,
        )?;
        let pass_trace_hash = serialized_hash(&trace)?;
        let replay_pass_trace_hash = serialized_hash(&replay_trace)?;
        let operator = analyze_impulse_operator(&source, &trace)?;
        let replay_operator = analyze_impulse_operator(&source, &replay_trace)?;
        let operator_report_hash = serialized_hash(&operator)?;
        let replay_operator_report_hash = serialized_hash(&replay_operator)?;
        let trace_artifact = write_gzip_json_artifact(
            &staging,
            &format!("traces/centered-stack-{depth}-step-{TARGET_STEP}.json.gz"),
            &trace,
        )?;
        let operator_artifact = write_gzip_json_artifact(
            &staging,
            &format!("operators/centered-stack-{depth}-step-{TARGET_STEP}.json.gz"),
            &operator,
        )?;
        let settled_speed_gate_failed =
            summary.settled_linear_speed_max.get() > policy.rest_linear_speed_limit.get();
        let energy_envelope_gate_failed = !summary.energy_envelope_nonincreasing;
        let harmful_signature = operator.mechanism.stale_reseed_signature
            && (settled_speed_gate_failed || energy_envelope_gate_failed);
        if !exact_series_replay
            || !capacity_clean
            || !structural_pass
            || pass_trace_hash != replay_pass_trace_hash
            || operator_report_hash != replay_operator_report_hash
        {
            return Err(format!(
                "S10.3 hard depth {depth} failed replay, structure, or capacity"
            ));
        }
        depths.push(HardDepthEvidence {
            depth,
            label: format!("centered-stack-{depth}"),
            source_scenario_hash,
            source: source_artifact,
            summary,
            settled_speed_gate_failed,
            energy_envelope_gate_failed,
            harmful_signature,
            structural_pass,
            structural_reason,
            capacity_clean,
            raw_series_hash,
            replay_raw_series_hash,
            exact_series_replay,
            pass_trace_hash,
            replay_pass_trace_hash,
            exact_pass_trace_replay: true,
            operator_report_hash,
            replay_operator_report_hash,
            exact_operator_replay: true,
            trace: trace_artifact,
            operator: operator_artifact,
            mechanism: operator.mechanism,
        });
    }
    let smallest_harmful_depth = depths
        .iter()
        .find(|row| row.harmful_signature)
        .map(|row| row.depth)
        .ok_or_else(|| "S10.3 depth sweep found no harmful stale-reseed signature".to_owned())?;
    if smallest_harmful_depth != 4 {
        return Err(format!(
            "S10.3 permanent reproducer changed from depth 4 to {smallest_harmful_depth}"
        ));
    }
    let permanent = p5_2_stale_reseed_reproducer_scenario().map_err(|error| error.to_string())?;
    let permanent_reproducer_source_hash =
        canonical_hash(&permanent).map_err(|error| error.to_string())?;
    let depth_four = depths
        .iter()
        .find(|row| row.depth == 4)
        .ok_or_else(|| "S10.3 depth sweep omitted depth 4".to_owned())?;
    let permanent_reproducer_exact_alias =
        permanent_reproducer_source_hash == depth_four.source_scenario_hash;
    if !permanent_reproducer_exact_alias {
        return Err("S10.3 permanent reproducer alias changed the source".to_owned());
    }
    let manifest = HardManifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s10.3-hard-operator-and-depth",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        host: "native-headless-metal",
        target_step: TARGET_STEP,
        depths,
        smallest_harmful_depth,
        permanent_reproducer_source_hash,
        permanent_reproducer_exact_alias,
        mechanism_verdict: HardMechanismVerdict {
            named_mechanism: "inter-substep-stale-persistence-reseed",
            classification: "oscillatory warm-start/bias composition with exact cache-vector overwrite at all three inter-substep refreshes",
            smallest_reproducer: "centered-stack-4 at exact completed-step prefix 904",
            correction_must_change: "refresh anchors and effective masses without replacing current within-step accumulated impulses with the older outer-step persistence seed",
            correction_must_preserve: "unilateral active sets, penetration correction, deterministic color support transfer, refreshed geometry, and final persistence",
        },
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    publish_manifest(&staging, &arguments.output, &manifest)?;
    print_success(arguments, "s10.3-hard-operator-and-depth")
}

fn run_soft(
    arguments: &Arguments,
    source_revision: String,
    source_dirty: bool,
    source_patch_digest: Option<String>,
) -> Result<(), String> {
    let s6_4_path = arguments
        .s6_4
        .as_ref()
        .ok_or_else(|| "soft phase requires --s6-4".to_owned())?;
    let s10_2_path = arguments
        .s10_2
        .as_ref()
        .ok_or_else(|| "soft phase requires --s10-2".to_owned())?;
    let s6_4_bytes = fs::read(s6_4_path).map_err(|error| error.to_string())?;
    let s10_2_bytes = fs::read(s10_2_path).map_err(|error| error.to_string())?;
    let s6_4: S6_4Input = serde_json::from_slice(&s6_4_bytes).map_err(|error| error.to_string())?;
    let s10_2: S10_2Input =
        serde_json::from_slice(&s10_2_bytes).map_err(|error| error.to_string())?;
    validate_soft_inputs(&s6_4, &s10_2)?;
    let s6_4_root = s6_4_path
        .parent()
        .ok_or_else(|| "S6.4 manifest has no parent".to_owned())?;
    let scene = s6_4
        .full_results
        .iter()
        .find(|scene| scene.label == "centered-stack-16")
        .ok_or_else(|| "S6.4 omits centered-stack-16".to_owned())?;
    let candidate = scene
        .candidates
        .iter()
        .find(|candidate| candidate.candidate.id == SOFT_CANDIDATE_ID)
        .ok_or_else(|| "S6.4 omits the frozen soft candidate".to_owned())?;
    validate_frozen_candidate(candidate)?;
    let source_bytes =
        fs::read(s6_4_root.join(&scene.source.path)).map_err(|error| error.to_string())?;
    if sha256_bytes(&source_bytes) != scene.source.sha256 {
        return Err("S6.4 centered-stack source digest differs".to_owned());
    }
    let mut source: ScenarioV1 =
        serde_json::from_slice(&source_bytes).map_err(|error| error.to_string())?;
    source.settings.contact_softness = candidate.candidate.softness;
    let source_scenario_hash = canonical_hash(&source).map_err(|error| error.to_string())?;
    if source_scenario_hash != candidate.candidate_scenario_hash {
        return Err("frozen S6.4 soft candidate source hash changed".to_owned());
    }
    let frozen_trace_bytes =
        fs::read(s6_4_root.join(&candidate.trace.path)).map_err(|error| error.to_string())?;
    if sha256_bytes(&frozen_trace_bytes) != candidate.trace.sha256 {
        return Err("frozen S6.4 soft trace digest differs".to_owned());
    }

    let staging = prepare_staging(&arguments.output, &["sources", "captures"])?;
    let source_artifact = write_json_artifact(&staging, "sources/soft-candidate-16.json", &source)?;
    let runner = pollster::block_on(GpuGeneratedContactRunner::new())?;
    eprintln!("S10.3 single authorized soft 16-stack observation");
    let raw = runner
        .run_contact_policy_observation_series_with_execution_scheduling_and_soft_feedback(
            &source,
            1,
            P5_2_SETTLING_EVIDENCE_STEPS,
            ContactIdentityPolicyV1::PriorContactHysteresis,
            64,
            GpuSolverExecutionV4::SELECTED,
            GpuSolverSchedulingPolicyV1::SELECTED,
            GpuSoftFeedbackPolicyV1::SELECTED,
        )?;
    let current_series_hash = serialized_hash(&raw)?;
    let legacy_series_projection_hash = legacy_series_projection_hash(&raw)?;
    let frozen_legacy_projection_hash_exact =
        legacy_series_projection_hash == candidate.raw_trace_hash;
    let capacity_clean = raw.snapshots.iter().all(snapshot_capacity_clean);
    let final_snapshot = raw
        .snapshots
        .last()
        .ok_or_else(|| "soft observation series is empty".to_owned())?;
    let (structural_pass, structural_reason) =
        centered_stack_structure(&source, &final_snapshot.bodies, !capacity_clean, 0.0);
    let observations = observe_generated_settling_series_v2(&source, &raw)?;
    let fresh_summary =
        summarize_settling_v2(&observations, awake_settling_measurement_policy_v2())?;
    let frozen_summary_exact = fresh_summary == candidate.summary;
    let fresh_rising_energy_mode = rising_energy_mode(&fresh_summary);
    let frozen_rising_energy_mode = rising_energy_mode(&candidate.summary);
    let rising_energy_mode_preserved = fresh_rising_energy_mode && frozen_rising_energy_mode;
    let capture = stationarity_capture_v1(
        "physbox-frozen-soft-hz30-d10-static1-prior",
        "centered-stack-16",
        &source,
        &source_scenario_hash,
        raw.snapshots
            .iter()
            .map(|snapshot| StationaritySnapshotInputV1 {
                completed_steps: snapshot.completed_steps,
                bodies: &snapshot.bodies,
                manifolds: &snapshot.manifolds,
                cache: &snapshot.cache,
            }),
    )?;
    let capture_artifact = write_gzip_json_artifact(
        &staging,
        "captures/centered-stack-16-soft.json.gz",
        &capture,
    )?;
    let soft_analysis = analyze_stationarity_v1(std::slice::from_ref(&capture))?;
    let class_comparisons = compare_stationarity_v1(&soft_analysis, &s10_2.authority_analysis)?;
    let hard_normal_point_signature = has_signature(&s10_2.class_comparisons, "normal-point-");
    let soft_normal_point_signature = has_signature(&class_comparisons, "normal-point-");
    let hard_normal_manifold_signature =
        has_signature(&s10_2.class_comparisons, "normal-manifold-");
    let soft_normal_manifold_signature = has_signature(&class_comparisons, "normal-manifold-");
    let hard_active_or_redistribution_signature =
        has_signature(&s10_2.class_comparisons, "active-set-")
            || has_signature(&s10_2.class_comparisons, "redistribution-");
    let soft_active_or_redistribution_signature = has_signature(&class_comparisons, "active-set-")
        || has_signature(&class_comparisons, "redistribution-");
    let hard_tangent_signature = has_signature(&s10_2.class_comparisons, "tangent-");
    let soft_tangent_signature = has_signature(&class_comparisons, "tangent-");
    let shared_point_normal_mode = hard_normal_point_signature
        && soft_normal_point_signature
        && hard_tangent_signature == soft_tangent_signature;
    let full_hard_signature_shared = shared_point_normal_mode
        && hard_normal_manifold_signature
        && soft_normal_manifold_signature
        && hard_active_or_redistribution_signature
        && soft_active_or_redistribution_signature;
    if !rising_energy_mode_preserved || !capacity_clean || !structural_pass {
        return Err(format!(
            "single S10.3 soft observation failed: rising_mode_preserved={rising_energy_mode_preserved} capacity_clean={capacity_clean} structural_pass={structural_pass}"
        ));
    }
    let manifest = SoftManifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2b-s10.3-soft-stationarity-unification",
        run_id: arguments.run_id.clone(),
        source_revision,
        source_dirty,
        source_patch_digest,
        host: "native-headless-metal",
        observation_runs: 1,
        inputs: SoftInputs {
            s6_4_run_id: s6_4.run_id,
            s6_4_source_revision: s6_4.source_revision,
            s6_4_manifest_sha256: sha256_bytes(&s6_4_bytes),
            s6_4_frozen_trace: ArtifactIdentity {
                path: candidate.trace.path.clone(),
                sha256: candidate.trace.sha256.clone(),
            },
            s10_2_run_id: s10_2.run_id,
            s10_2_source_revision: s10_2.source_revision,
            s10_2_manifest_sha256: sha256_bytes(&s10_2_bytes),
        },
        stationarity_contract: stationarity_metric_contract_v1(),
        source_scenario_hash,
        source: source_artifact,
        capture: capture_artifact,
        adapter: raw.adapter,
        current_series_hash,
        legacy_series_projection_hash,
        frozen_raw_series_hash: candidate.raw_trace_hash.clone(),
        frozen_legacy_projection_hash_exact,
        fresh_summary,
        frozen_summary: candidate.summary.clone(),
        frozen_summary_exact,
        fresh_rising_energy_mode,
        frozen_rising_energy_mode,
        rising_energy_mode_preserved,
        reobservation_context: "one current-source observation of the exact frozen softness/identity and historical per-color scheduling policy; historical raw hash and magnitude are context because later solver-policy schema and implementation changes make this a bounded re-observation, not a baseline replay",
        capacity_clean,
        structural_pass,
        structural_reason,
        soft_analysis,
        authority_analysis: s10_2.authority_analysis,
        class_comparisons,
        h3: H3Verdict {
            hard_normal_point_signature,
            soft_normal_point_signature,
            hard_normal_manifold_signature,
            soft_normal_manifold_signature,
            hard_active_or_redistribution_signature,
            soft_active_or_redistribution_signature,
            hard_tangent_signature,
            soft_tangent_signature,
            shared_point_normal_mode,
            full_hard_signature_shared,
            verdict: if full_hard_signature_shared {
                "full-hard-signature-shared"
            } else if shared_point_normal_mode {
                "partial-unification-point-normal-only"
            } else {
                "no-stationarity-unification"
            },
            rationale: if full_hard_signature_shared {
                "the soft rising-energy lane reproduces the hard lane's point, manifold, active-set or redistribution, and tangent authority-band dispositions"
            } else if shared_point_normal_mode {
                "both lanes exceed floor/interior point-normal stationarity bands and keep tangent below authority, but soft manifold totals, active sets, and redistribution do not reproduce the hard signature"
            } else {
                "the soft rising-energy lane does not reproduce even the hard point-normal/tangent stationarity signature"
            },
        },
        product_constructor_changed: false,
        physical_defaults_changed: false,
        numerical_tolerances_changed: false,
        oracle_precedence_changed: false,
    };
    publish_manifest(&staging, &arguments.output, &manifest)?;
    print_success(arguments, "s10.3-soft-stationarity-unification")
}

fn has_signature(comparisons: &[StationarityClassComparisonV1], prefix: &str) -> bool {
    comparisons.iter().any(|comparison| {
        matches!(
            comparison.class,
            StationarityManifoldClassV1::FloorSupport | StationarityManifoldClassV1::Interior
        ) && comparison.verdict == StationarityClassVerdictV1::Oscillating
            && comparison
                .metrics
                .iter()
                .any(|metric| metric.metric.starts_with(prefix) && metric.exceeds_authority)
    })
}

fn rising_energy_mode(summary: &SettlingSummaryV2) -> bool {
    !summary.energy_envelope_nonincreasing
        && summary.positive_energy_run_max > 5
        && summary.last_energy_quartile_max.get() > summary.first_energy_quartile_max.get()
}

fn validate_soft_inputs(s6_4: &S6_4Input, s10_2: &S10_2Input) -> Result<(), String> {
    if s6_4.artifact_kind != "p5.2b-s6.4-soft-contact-factorial"
        || s6_4.source_dirty
        || s10_2.artifact_kind != "p5.2b-s10.2-impulse-stationarity"
        || s10_2.source_dirty
        || s10_2.source_revision != S10_2_REVISION
        || s10_2.class_comparisons.len() != 3
        || s10_2.physbox_analysis.classes.len() != 3
        || s10_2.authority_analysis.classes.len() != 3
    {
        return Err("S10.3 soft inputs are not the governed clean evidence".to_owned());
    }
    Ok(())
}

fn validate_frozen_candidate(candidate: &S6_4Candidate) -> Result<(), String> {
    let expected_softness = ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: FiniteF32::new(30.0).unwrap(),
        damping_ratio: FiniteF32::new(10.0).unwrap(),
        static_contact_hertz_multiplier: FiniteF32::new(1.0).unwrap(),
    };
    if candidate.candidate.id != SOFT_CANDIDATE_ID
        || candidate.candidate.identity_policy != ContactIdentityPolicyV1::PriorContactHysteresis
        || candidate.candidate.softness != Some(expected_softness)
        || !candidate.replay_exact
        || !candidate.instrumentation_final_exact
        || !candidate.capacity_clean
        || !candidate.structural_pass
    {
        return Err("S6.4 soft candidate contract changed".to_owned());
    }
    Ok(())
}

fn snapshot_capacity_clean(snapshot: &physbox_engine::GpuGeneratedContactSnapshotV1) -> bool {
    snapshot.failure.is_none()
        && snapshot
            .capacities
            .iter()
            .all(|capacity| capacity.accepted && !capacity.overflowed)
}

fn serialized_hash(value: &impl Serialize) -> Result<String, String> {
    serde_json::to_vec(value)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(|error| error.to_string())
}

fn legacy_series_projection_hash(series: &GpuGeneratedContactSeriesV1) -> Result<String, String> {
    serialized_hash(&S6_4LegacySeriesProjection {
        version: series.version,
        adapter: &series.adapter,
        source_scenario_hash: &series.source_scenario_hash,
        observation_interval_steps: series.observation_interval_steps,
        completed_steps: series.completed_steps,
        solver_settings: &series.solver_settings,
        solver_execution: series.solver_execution,
        solver_scheduling_policy: series.solver_scheduling_policy,
        contact_identity_policy: series.contact_identity_policy,
        dispatches_per_step: series.dispatches_per_step,
        allocated_bytes: series.allocated_bytes,
        snapshots: &series.snapshots,
    })
}

fn prepare_staging(output: &Path, directories: &[&str]) -> Result<PathBuf, String> {
    let mut staging_name = output.as_os_str().to_owned();
    staging_name.push(".partial");
    let staging = PathBuf::from(staging_name);
    if output.exists() || staging.exists() {
        return Err(format!(
            "refusing to replace S10.3 output or staging {}",
            output.display()
        ));
    }
    for directory in directories {
        fs::create_dir_all(staging.join(directory)).map_err(|error| error.to_string())?;
    }
    Ok(staging)
}

fn publish_manifest(
    staging: &Path,
    output: &Path,
    manifest: &impl Serialize,
) -> Result<(), String> {
    atomic_json(&staging.join("manifest.json"), manifest)?;
    sync_directory(staging)?;
    fs::rename(staging, output).map_err(|error| error.to_string())?;
    sync_directory(output.parent().unwrap_or(Path::new(".")))
}

fn print_success(arguments: &Arguments, scenario: &str) -> Result<(), String> {
    let manifest =
        fs::read(arguments.output.join("manifest.json")).map_err(|error| error.to_string())?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=native scenario={} run_id={} manifest_sha256={} output={}",
        scenario,
        arguments.run_id,
        sha256_bytes(&manifest),
        arguments.output.display()
    );
    Ok(())
}

fn write_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    write_artifact(root, relative, &bytes)
}

fn write_gzip_json_artifact(
    root: &Path,
    relative: &str,
    value: &impl Serialize,
) -> Result<ArtifactIdentity, String> {
    let bytes = serde_json::to_vec(value).map_err(|error| error.to_string())?;
    let mut encoder = GzBuilder::new()
        .mtime(0)
        .write(Vec::new(), Compression::best());
    encoder
        .write_all(&bytes)
        .map_err(|error| error.to_string())?;
    let compressed = encoder.finish().map_err(|error| error.to_string())?;
    write_artifact(root, relative, &compressed)
}

fn write_artifact(root: &Path, relative: &str, bytes: &[u8]) -> Result<ArtifactIdentity, String> {
    atomic_write(&root.join(relative), bytes)?;
    Ok(ArtifactIdentity {
        path: relative.to_owned(),
        sha256: sha256_bytes(bytes),
    })
}

fn atomic_json(path: &Path, value: &impl Serialize) -> Result<(), String> {
    let mut bytes = serde_json::to_vec_pretty(value).map_err(|error| error.to_string())?;
    bytes.push(b'\n');
    atomic_write(path, &bytes)
}

fn atomic_write(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let staging = path.with_extension("partial");
    if path.exists() || staging.exists() {
        return Err(format!("refusing to replace artifact {}", path.display()));
    }
    let mut file = OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&staging)
        .map_err(|error| error.to_string())?;
    file.write_all(bytes).map_err(|error| error.to_string())?;
    file.sync_all().map_err(|error| error.to_string())?;
    drop(file);
    fs::rename(staging, path).map_err(|error| error.to_string())
}

fn sync_directory(path: &Path) -> Result<(), String> {
    fs::File::open(path)
        .and_then(|directory| directory.sync_all())
        .map_err(|error| error.to_string())
}

fn repository_state(repository: &Path) -> Result<(String, bool, Option<String>), String> {
    let revision = command_output(repository, "git", &["rev-parse", "HEAD"])?;
    let status = command_output(
        repository,
        "git",
        &["status", "--porcelain=v1", "--untracked-files=all"],
    )?;
    let dirty = !status.is_empty();
    let patch_digest = if dirty {
        let output = Command::new("git")
            .args(["diff", "--binary", "HEAD"])
            .current_dir(repository)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()
            .map_err(|error| error.to_string())?;
        if !output.status.success() {
            return Err("git diff failed while recording provenance".to_owned());
        }
        Some(sha256_bytes(&output.stdout))
    } else {
        None
    };
    Ok((revision, dirty, patch_digest))
}

fn command_output(directory: &Path, program: &str, arguments: &[&str]) -> Result<String, String> {
    let output = Command::new(program)
        .args(arguments)
        .current_dir(directory)
        .output()
        .map_err(|error| error.to_string())?;
    if !output.status.success() {
        return Err(format!("{program} {} failed", arguments.join(" ")));
    }
    String::from_utf8(output.stdout)
        .map(|value| value.trim().to_owned())
        .map_err(|error| error.to_string())
}

#[derive(Clone, Copy)]
enum Phase {
    Hard,
    Soft,
}

struct Arguments {
    phase: Phase,
    run_id: String,
    output: PathBuf,
    s6_4: Option<PathBuf>,
    s10_2: Option<PathBuf>,
    require_clean: bool,
}

impl Arguments {
    fn parse() -> Result<Self, String> {
        let mut phase = None;
        let mut run_id = None;
        let mut output = None;
        let mut s6_4 = None;
        let mut s10_2 = None;
        let mut require_clean = false;
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
                "--phase" => {
                    phase = Some(match value.as_str() {
                        "hard" => Phase::Hard,
                        "soft" => Phase::Soft,
                        _ => return Err(format!("invalid S10.3 phase {value}")),
                    })
                }
                "--run-id" => run_id = Some(value),
                "--output" => output = Some(PathBuf::from(value)),
                "--s6-4" => s6_4 = Some(PathBuf::from(value)),
                "--s10-2" => s10_2 = Some(PathBuf::from(value)),
                _ => return Err(format!("unknown argument {argument}")),
            }
        }
        Ok(Self {
            phase: phase.ok_or_else(|| "missing --phase".to_owned())?,
            run_id: run_id.ok_or_else(|| "missing --run-id".to_owned())?,
            output: output.ok_or_else(|| "missing --output".to_owned())?,
            s6_4,
            s10_2,
            require_clean,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn s10_3_depth_and_soft_contracts_are_frozen() {
        assert_eq!(HARD_DEPTHS, [2, 4, 8, 9, 10, 11, 12, 13, 14, 15, 16]);
        assert_eq!(TARGET_STEP, 904);
        assert_eq!(SOFT_CANDIDATE_ID, "soft-hz30-d10-static1-prior");
        assert_eq!(
            canonical_hash(&p5_2_stale_reseed_reproducer_scenario().unwrap()).unwrap(),
            canonical_hash(
                &p5_2_centered_rectangle_stack_scenario(4, P5_2_SETTLING_EVIDENCE_STEPS).unwrap()
            )
            .unwrap()
        );
    }
}
