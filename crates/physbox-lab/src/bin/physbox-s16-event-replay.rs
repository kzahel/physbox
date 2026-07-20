use std::{
    env, fs,
    io::Read,
    path::{Path, PathBuf},
};

use flate2::read::GzDecoder;
use physbox_contracts::{ScenarioV1, Validate, canonical_hash, sha256_bytes};
use physbox_lab::{
    authority_response::{
        AuthorityBandsV1, AuthorityBodyPairV1, AuthorityFailureVectorV1, AuthorityQualityV1,
        authority_failure_vector_v1, full_f64_active_support_pairs_v1,
        full_f64_authority_quality_v1, scalar_active_support_pairs_v1, scalar_authority_quality_v1,
    },
    authority_structure::{
        AuthorityEventSolveResultV1, RoutedAuthorityEventSolveResultV1,
        solve_authority_event_stream_v1, solve_routed_authority_event_stream_v1,
    },
    event_structure::{
        EventFamilyV1, EventLaneV1, EventStreamV1, NormalDecisionEventV1, PreparedContactEventV1,
    },
    precision_margin::{
        FullF64EventSolveResultV1, RoutedFullF64EventSolveResultV1,
        solve_full_f64_authority_events_v1, solve_routed_full_f64_authority_events_v1,
    },
};
use serde::{Deserialize, Serialize};

const S12_MANIFEST: &str = "evidence/p5.2b-s12.2-authority-endpoints/manifest.json";
const A1_CAPTURE_ROOT: &str = "evidence/p5.2b-s16.2-first-event-divergence/captures";

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S12Manifest {
    artifact_kind: String,
    authority_bands: AuthorityBandsV1,
    cases: Vec<S12Case>,
}

#[derive(Clone, Copy, Deserialize, Eq, PartialEq)]
#[serde(rename_all = "kebab-case")]
enum SourceRole {
    ResponseDiagnostic,
    Context,
    Governed,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S12Case {
    label: String,
    role: SourceRole,
    requested_steps: u32,
    source_scenario_hash: String,
    source: ArtifactIdentity,
    authority: S12Authority,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct ArtifactIdentity {
    path: String,
    sha256: String,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct S12Authority {
    quality: AuthorityQualityV1,
    active_support_pairs: Vec<AuthorityBodyPairV1>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaneEnvelope {
    source_label: String,
    source_scenario_hash: String,
    dynamic_depth: usize,
    stream: LaneCapture,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct LaneCapture {
    lane: EventLaneV1,
    exact_replay: bool,
    events: EventStreamV1,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RouteDirection {
    F64FromF32,
    F32FromF64,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ReplayOutput {
    version: u32,
    artifact_kind: &'static str,
    continuous_recurrence: &'static str,
    candidate_count: u32,
    authority_bands: AuthorityBandsV1,
    directions: [RouteDirection; 2],
    families: [EventFamilyV1; 6],
    cases: Vec<CaseOutput>,
    rows: usize,
    individually_causal_families: Vec<EventFamilyV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CaseOutput {
    label: String,
    depth: usize,
    source_scenario_hash: String,
    immutable_owner_stream_hashes: OwnerHashes,
    authority_quality: AuthorityQualityV1,
    authority_active_support_pairs: Vec<AuthorityBodyPairV1>,
    baselines: [BaselineOutput; 2],
    rows: Vec<RowOutput>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct OwnerHashes {
    exact_s12_f32: String,
    full_solve_f64: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct BaselineOutput {
    lane: EventLaneV1,
    result_hash: String,
    quality: AuthorityQualityV1,
    active_support_pairs: Vec<AuthorityBodyPairV1>,
    failure_vector: AuthorityFailureVectorV1,
    authority_qualified: bool,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RowOutput {
    direction: RouteDirection,
    family: EventFamilyV1,
    owner_lane: EventLaneV1,
    receiving_lane: EventLaneV1,
    disposition: RouteDisposition,
    exact_replay: bool,
    primary_hash: Option<String>,
    replay_hash: Option<String>,
    route_error: Option<String>,
    quality: Option<AuthorityQualityV1>,
    active_support_pairs: Option<Vec<AuthorityBodyPairV1>>,
    failure_vector: Option<AuthorityFailureVectorV1>,
    authority_qualified: bool,
    baseline_first_failure: Option<&'static str>,
    improves_baseline_first_failure: bool,
    route_effect: Option<RouteEffect>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
enum RouteDisposition {
    Completed,
    OwnerChoiceUnavailable,
    SolverError,
    ReplayMismatch,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct RouteEffect {
    owner_receiver_token_difference: bool,
    owner_token_adopted: bool,
    event_kind: Option<&'static str>,
    event_index: Option<usize>,
    completed_step: Option<u32>,
    first_downstream_impulse_delta: Option<f64>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s16-event-replay: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let output_path = arguments()?;
    let s12_bytes = fs::read(S12_MANIFEST).map_err(display)?;
    let s12: S12Manifest = serde_json::from_slice(&s12_bytes).map_err(display)?;
    if s12.artifact_kind != "p5.2b-s12.2-authority-endpoints" {
        return Err("S12 authority manifest drift".to_owned());
    }
    let governed = s12
        .cases
        .into_iter()
        .filter(|case| case.role == SourceRole::Governed)
        .collect::<Vec<_>>();
    if governed.len() != 2 {
        return Err("S16 A2 requires exactly two governed S12 cases".to_owned());
    }

    let mut cases = Vec::new();
    for case in governed {
        cases.push(run_case(&case, &s12.authority_bands)?);
    }
    let individually_causal_families = EventFamilyV1::ALL
        .into_iter()
        .filter(|family| individually_causal(&cases, *family))
        .collect::<Vec<_>>();
    let rows = cases.iter().map(|case| case.rows.len()).sum();
    if rows != 24 {
        return Err(format!("S16 A2 produced {rows} rows instead of 24"));
    }
    let output = ReplayOutput {
        version: 1,
        artifact_kind: "p5.2b-s16.3-event-owner-replay-summary",
        continuous_recurrence: "unchanged-X1-X4-current-X5",
        candidate_count: 0,
        authority_bands: s12.authority_bands,
        directions: [RouteDirection::F64FromF32, RouteDirection::F32FromF64],
        families: EventFamilyV1::ALL,
        cases,
        rows,
        individually_causal_families,
    };
    fs::write(
        &output_path,
        serde_json::to_vec_pretty(&output).map_err(display)?,
    )
    .map_err(display)?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=scalar scenario=s16.3-event-owner-replay rows={} causal_families={} output={}",
        output.rows,
        output.individually_causal_families.len(),
        output_path.display(),
    );
    Ok(())
}

fn run_case(case: &S12Case, bands: &AuthorityBandsV1) -> Result<CaseOutput, String> {
    if case.requested_steps != 1_200 {
        return Err(format!("{} requested-step drift", case.label));
    }
    let source_path = Path::new(S12_MANIFEST)
        .parent()
        .expect("S12 manifest has a parent")
        .join(&case.source.path);
    let source_bytes = fs::read(&source_path).map_err(display)?;
    if sha256_bytes(&source_bytes) != case.source.sha256 {
        return Err(format!("{} source digest drift", case.label));
    }
    let source: ScenarioV1 = serde_json::from_slice(&source_bytes).map_err(display)?;
    source.validate().map_err(display)?;
    if canonical_hash(&source).map_err(display)? != case.source_scenario_hash {
        return Err(format!("{} scenario hash drift", case.label));
    }
    let depth = source
        .bodies
        .iter()
        .filter(|body| body.inverse_mass.get() != 0.0)
        .count();
    if ![11, 16].contains(&depth) {
        return Err(format!("{} governed depth drift", case.label));
    }

    let exact_owner = read_owner(&case.label, depth, EventLaneV1::ExactS12F32)?;
    let full_owner = read_owner(&case.label, depth, EventLaneV1::FullSolveF64)?;
    let exact_baseline_result =
        solve_authority_event_stream_v1(&source, EventLaneV1::ExactS12F32).map_err(display)?;
    let full_baseline_result = solve_full_f64_authority_events_v1(&source).map_err(display)?;
    if exact_baseline_result.events != exact_owner || full_baseline_result.events != full_owner {
        return Err(format!(
            "{} regenerated endpoint differs from A1 owner",
            case.label
        ));
    }
    let exact_baseline_output =
        exact_baseline(&source, exact_baseline_result, &case.authority, bands)?;
    let full_baseline_output =
        full_baseline(&source, full_baseline_result, &case.authority, bands)?;

    let mut rows = Vec::with_capacity(12);
    for family in EventFamilyV1::ALL {
        rows.push(run_full_route(
            &source,
            family,
            &exact_owner,
            &full_owner,
            &case.authority,
            bands,
            &full_baseline_output,
        )?);
        rows.push(run_exact_route(
            &source,
            family,
            &full_owner,
            &exact_owner,
            &case.authority,
            bands,
            &exact_baseline_output,
        )?);
    }

    Ok(CaseOutput {
        label: case.label.clone(),
        depth,
        source_scenario_hash: case.source_scenario_hash.clone(),
        immutable_owner_stream_hashes: OwnerHashes {
            exact_s12_f32: event_hash(&exact_owner)?,
            full_solve_f64: event_hash(&full_owner)?,
        },
        authority_quality: case.authority.quality.clone(),
        authority_active_support_pairs: case.authority.active_support_pairs.clone(),
        baselines: [exact_baseline_output, full_baseline_output],
        rows,
    })
}

fn read_owner(label: &str, depth: usize, lane: EventLaneV1) -> Result<EventStreamV1, String> {
    let lane_name = match lane {
        EventLaneV1::ExactS12F32 => "exact-s12-f32",
        EventLaneV1::FullSolveF64 => "full-solve-f64",
        EventLaneV1::CancellationPathF64 => {
            return Err("cancellation-path lane is not an A2 owner".to_owned());
        }
    };
    let path = Path::new(A1_CAPTURE_ROOT).join(format!("{label}-{lane_name}.json.gz"));
    let mut decoder = GzDecoder::new(fs::File::open(&path).map_err(display)?);
    let mut bytes = Vec::new();
    decoder.read_to_end(&mut bytes).map_err(display)?;
    let capture: LaneEnvelope = serde_json::from_slice(&bytes).map_err(display)?;
    if capture.source_label != label
        || capture.dynamic_depth != depth
        || capture.stream.lane != lane
        || capture.stream.events.lane != lane
        || !capture.stream.exact_replay
        || capture.source_scenario_hash.is_empty()
    {
        return Err(format!(
            "A1 owner capture contract drift: {}",
            path.display()
        ));
    }
    Ok(capture.stream.events)
}

fn exact_baseline(
    source: &ScenarioV1,
    result: AuthorityEventSolveResultV1,
    authority: &S12Authority,
    bands: &AuthorityBandsV1,
) -> Result<BaselineOutput, String> {
    let result_hash = scalar_result_hash(&result.solve, &result.events)?;
    let quality = scalar_authority_quality_v1(source, &result.solve).map_err(display)?;
    let active_support_pairs = scalar_active_support_pairs_v1(&result.solve);
    let failure_vector = authority_failure_vector_v1(
        &quality,
        &active_support_pairs,
        &authority.quality,
        &authority.active_support_pairs,
        bands,
        true,
    );
    Ok(BaselineOutput {
        lane: EventLaneV1::ExactS12F32,
        result_hash,
        quality,
        active_support_pairs,
        authority_qualified: failure_vector.ordered_failures.is_empty(),
        failure_vector,
    })
}

fn full_baseline(
    source: &ScenarioV1,
    result: FullF64EventSolveResultV1,
    authority: &S12Authority,
    bands: &AuthorityBandsV1,
) -> Result<BaselineOutput, String> {
    let result_hash = full_result_hash(&result.solve, &result.events)?;
    let quality = full_f64_authority_quality_v1(source, &result.solve).map_err(display)?;
    let active_support_pairs = full_f64_active_support_pairs_v1(&result.solve);
    let failure_vector = authority_failure_vector_v1(
        &quality,
        &active_support_pairs,
        &authority.quality,
        &authority.active_support_pairs,
        bands,
        true,
    );
    Ok(BaselineOutput {
        lane: EventLaneV1::FullSolveF64,
        result_hash,
        quality,
        active_support_pairs,
        authority_qualified: failure_vector.ordered_failures.is_empty(),
        failure_vector,
    })
}

fn run_full_route(
    source: &ScenarioV1,
    family: EventFamilyV1,
    owner: &EventStreamV1,
    receiver_baseline: &EventStreamV1,
    authority: &S12Authority,
    bands: &AuthorityBandsV1,
    baseline: &BaselineOutput,
) -> Result<RowOutput, String> {
    let primary = solve_routed_full_f64_authority_events_v1(source, owner, &[family]);
    let replay = solve_routed_full_f64_authority_events_v1(source, owner, &[family]);
    routed_full_row(
        source,
        family,
        primary,
        replay,
        owner,
        receiver_baseline,
        authority,
        bands,
        baseline,
    )
}

#[allow(clippy::too_many_arguments)]
fn routed_full_row(
    source: &ScenarioV1,
    family: EventFamilyV1,
    primary: Result<RoutedFullF64EventSolveResultV1, physbox_lab::scalar::ScalarError>,
    replay: Result<RoutedFullF64EventSolveResultV1, physbox_lab::scalar::ScalarError>,
    owner: &EventStreamV1,
    receiver_baseline: &EventStreamV1,
    authority: &S12Authority,
    bands: &AuthorityBandsV1,
    baseline: &BaselineOutput,
) -> Result<RowOutput, String> {
    match (primary, replay) {
        (Ok(primary), Ok(replay)) => {
            let primary_hash = full_result_hash(&primary.solve, &primary.events)?;
            let replay_hash = full_result_hash(&replay.solve, &replay.events)?;
            let exact_replay = primary == replay && primary_hash == replay_hash;
            let quality = full_f64_authority_quality_v1(source, &primary.solve).map_err(display)?;
            let active_support_pairs = full_f64_active_support_pairs_v1(&primary.solve);
            let failure_vector = authority_failure_vector_v1(
                &quality,
                &active_support_pairs,
                &authority.quality,
                &authority.active_support_pairs,
                bands,
                exact_replay,
            );
            let improves = improves_first_failure(&baseline.failure_vector, &failure_vector);
            Ok(RowOutput {
                direction: RouteDirection::F64FromF32,
                family,
                owner_lane: EventLaneV1::ExactS12F32,
                receiving_lane: EventLaneV1::FullSolveF64,
                disposition: if exact_replay {
                    RouteDisposition::Completed
                } else {
                    RouteDisposition::ReplayMismatch
                },
                exact_replay,
                primary_hash: Some(primary_hash),
                replay_hash: Some(replay_hash),
                route_error: None,
                quality: Some(quality),
                active_support_pairs: Some(active_support_pairs),
                authority_qualified: failure_vector.ordered_failures.is_empty(),
                baseline_first_failure: baseline.failure_vector.ordered_failures.first().copied(),
                improves_baseline_first_failure: improves,
                route_effect: Some(route_effect(
                    family,
                    receiver_baseline,
                    owner,
                    &primary.events,
                )),
                failure_vector: Some(failure_vector),
            })
        }
        (Err(primary), Err(replay)) => error_row(
            RouteDirection::F64FromF32,
            family,
            EventLaneV1::ExactS12F32,
            EventLaneV1::FullSolveF64,
            primary.to_string(),
            replay.to_string(),
            baseline,
        ),
        _ => Err(format!("f64<-f32/{family:?} replay outcome differs")),
    }
}

fn run_exact_route(
    source: &ScenarioV1,
    family: EventFamilyV1,
    owner: &EventStreamV1,
    receiver_baseline: &EventStreamV1,
    authority: &S12Authority,
    bands: &AuthorityBandsV1,
    baseline: &BaselineOutput,
) -> Result<RowOutput, String> {
    let primary = solve_routed_authority_event_stream_v1(source, owner, &[family]);
    let replay = solve_routed_authority_event_stream_v1(source, owner, &[family]);
    match (primary, replay) {
        (Ok(primary), Ok(replay)) => routed_exact_completed(
            source,
            family,
            primary,
            replay,
            owner,
            receiver_baseline,
            authority,
            bands,
            baseline,
        ),
        (Err(primary), Err(replay)) => error_row(
            RouteDirection::F32FromF64,
            family,
            EventLaneV1::FullSolveF64,
            EventLaneV1::ExactS12F32,
            primary.to_string(),
            replay.to_string(),
            baseline,
        ),
        _ => Err(format!("f32<-f64/{family:?} replay outcome differs")),
    }
}

#[allow(clippy::too_many_arguments)]
fn routed_exact_completed(
    source: &ScenarioV1,
    family: EventFamilyV1,
    primary: RoutedAuthorityEventSolveResultV1,
    replay: RoutedAuthorityEventSolveResultV1,
    owner: &EventStreamV1,
    receiver_baseline: &EventStreamV1,
    authority: &S12Authority,
    bands: &AuthorityBandsV1,
    baseline: &BaselineOutput,
) -> Result<RowOutput, String> {
    let primary_hash = scalar_result_hash(&primary.solve, &primary.events)?;
    let replay_hash = scalar_result_hash(&replay.solve, &replay.events)?;
    let exact_replay = primary == replay && primary_hash == replay_hash;
    let quality = scalar_authority_quality_v1(source, &primary.solve).map_err(display)?;
    let active_support_pairs = scalar_active_support_pairs_v1(&primary.solve);
    let failure_vector = authority_failure_vector_v1(
        &quality,
        &active_support_pairs,
        &authority.quality,
        &authority.active_support_pairs,
        bands,
        exact_replay,
    );
    Ok(RowOutput {
        direction: RouteDirection::F32FromF64,
        family,
        owner_lane: EventLaneV1::FullSolveF64,
        receiving_lane: EventLaneV1::ExactS12F32,
        disposition: if exact_replay {
            RouteDisposition::Completed
        } else {
            RouteDisposition::ReplayMismatch
        },
        exact_replay,
        primary_hash: Some(primary_hash),
        replay_hash: Some(replay_hash),
        route_error: None,
        quality: Some(quality),
        active_support_pairs: Some(active_support_pairs),
        authority_qualified: failure_vector.ordered_failures.is_empty(),
        baseline_first_failure: baseline.failure_vector.ordered_failures.first().copied(),
        improves_baseline_first_failure: improves_first_failure(
            &baseline.failure_vector,
            &failure_vector,
        ),
        route_effect: Some(route_effect(
            family,
            receiver_baseline,
            owner,
            &primary.events,
        )),
        failure_vector: Some(failure_vector),
    })
}

fn error_row(
    direction: RouteDirection,
    family: EventFamilyV1,
    owner_lane: EventLaneV1,
    receiving_lane: EventLaneV1,
    primary_error: String,
    replay_error: String,
    baseline: &BaselineOutput,
) -> Result<RowOutput, String> {
    let exact_replay = primary_error == replay_error;
    let unavailable = primary_error.starts_with("owner-choice-unavailable:");
    Ok(RowOutput {
        direction,
        family,
        owner_lane,
        receiving_lane,
        disposition: if !exact_replay {
            RouteDisposition::ReplayMismatch
        } else if unavailable {
            RouteDisposition::OwnerChoiceUnavailable
        } else {
            RouteDisposition::SolverError
        },
        exact_replay,
        primary_hash: None,
        replay_hash: None,
        route_error: Some(primary_error),
        quality: None,
        active_support_pairs: None,
        failure_vector: None,
        authority_qualified: false,
        baseline_first_failure: baseline.failure_vector.ordered_failures.first().copied(),
        improves_baseline_first_failure: false,
        route_effect: None,
    })
}

fn improves_first_failure(
    baseline: &AuthorityFailureVectorV1,
    candidate: &AuthorityFailureVectorV1,
) -> bool {
    let Some(first) = baseline.ordered_failures.first() else {
        return false;
    };
    let order = [
        "non-finite",
        "structure",
        "authority-response",
        "penetration-support",
        "awake-speed",
        "positive-run",
        "energy-envelope",
        "replay",
    ];
    let rank = order.iter().position(|label| label == first).unwrap();
    !candidate.ordered_failures.contains(first)
        && candidate
            .ordered_failures
            .iter()
            .all(|failure| order.iter().position(|label| label == failure).unwrap() > rank)
}

fn individually_causal(cases: &[CaseOutput], family: EventFamilyV1) -> bool {
    let full_rows = cases
        .iter()
        .flat_map(|case| &case.rows)
        .filter(|row| row.family == family && row.direction == RouteDirection::F64FromF32);
    let exact_rows = cases
        .iter()
        .flat_map(|case| &case.rows)
        .filter(|row| row.family == family && row.direction == RouteDirection::F32FromF64);
    full_rows.count() == 2
        && cases
            .iter()
            .flat_map(|case| &case.rows)
            .filter(|row| row.family == family && row.direction == RouteDirection::F64FromF32)
            .all(|row| {
                row.disposition == RouteDisposition::Completed
                    && row.exact_replay
                    && row.improves_baseline_first_failure
            })
        && exact_rows.count() == 2
        && cases
            .iter()
            .flat_map(|case| &case.rows)
            .filter(|row| row.family == family && row.direction == RouteDirection::F32FromF64)
            .all(|row| {
                row.exact_replay
                    && (row.disposition == RouteDisposition::OwnerChoiceUnavailable
                        || row
                            .route_effect
                            .as_ref()
                            .is_some_and(|effect| effect.owner_token_adopted))
            })
}

fn route_effect(
    family: EventFamilyV1,
    receiver: &EventStreamV1,
    owner: &EventStreamV1,
    routed: &EventStreamV1,
) -> RouteEffect {
    match family {
        EventFamilyV1::SeparationPositiveAndSpeculativeBranch
        | EventFamilyV1::UnilateralActiveSetProjection => {
            normal_route_effect(family, receiver, owner, routed)
        }
        _ => prepared_route_effect(family, receiver, owner, routed),
    }
}

fn prepared_route_effect(
    family: EventFamilyV1,
    receiver: &EventStreamV1,
    owner: &EventStreamV1,
    routed: &EventStreamV1,
) -> RouteEffect {
    let limit = receiver
        .prepared_contacts
        .len()
        .min(owner.prepared_contacts.len());
    for index in 0..limit {
        let receiver_event = &receiver.prepared_contacts[index];
        let owner_event = &owner.prepared_contacts[index];
        if prepared_token(family, receiver_event) != prepared_token(family, owner_event) {
            let adopted = routed.prepared_contacts.get(index).is_some_and(|event| {
                prepared_key(event) == prepared_key(owner_event)
                    && prepared_token(family, event) == prepared_token(family, owner_event)
            });
            return RouteEffect {
                owner_receiver_token_difference: true,
                owner_token_adopted: adopted,
                event_kind: Some("prepared-contact"),
                event_index: Some(index),
                completed_step: Some(owner_event.completed_step),
                first_downstream_impulse_delta: first_impulse_delta(
                    &receiver.normal_decisions,
                    &routed.normal_decisions,
                    0,
                ),
            };
        }
    }
    RouteEffect {
        owner_receiver_token_difference: receiver.prepared_contacts.len()
            != owner.prepared_contacts.len(),
        owner_token_adopted: false,
        event_kind: None,
        event_index: None,
        completed_step: None,
        first_downstream_impulse_delta: None,
    }
}

fn normal_route_effect(
    family: EventFamilyV1,
    receiver: &EventStreamV1,
    owner: &EventStreamV1,
    routed: &EventStreamV1,
) -> RouteEffect {
    let limit = receiver
        .normal_decisions
        .len()
        .min(owner.normal_decisions.len());
    for index in 0..limit {
        let receiver_event = &receiver.normal_decisions[index];
        let owner_event = &owner.normal_decisions[index];
        if normal_token(family, receiver_event) != normal_token(family, owner_event) {
            let adopted = routed.normal_decisions.get(index).is_some_and(|event| {
                normal_key(event) == normal_key(owner_event)
                    && normal_token(family, event) == normal_token(family, owner_event)
            });
            return RouteEffect {
                owner_receiver_token_difference: true,
                owner_token_adopted: adopted,
                event_kind: Some("normal-decision"),
                event_index: Some(index),
                completed_step: Some(owner_event.completed_step),
                first_downstream_impulse_delta: first_impulse_delta(
                    &receiver.normal_decisions,
                    &routed.normal_decisions,
                    index,
                ),
            };
        }
    }
    RouteEffect {
        owner_receiver_token_difference: receiver.normal_decisions.len()
            != owner.normal_decisions.len(),
        owner_token_adopted: false,
        event_kind: None,
        event_index: None,
        completed_step: None,
        first_downstream_impulse_delta: None,
    }
}

fn prepared_token(family: EventFamilyV1, event: &PreparedContactEventV1) -> serde_json::Value {
    match family {
        EventFamilyV1::ContactAdmissibilityAndPointCount => {
            serde_json::json!([event.pair, event.admitted, event.point_count])
        }
        EventFamilyV1::ReferenceOwnerAndAuthorityFeatureIdentity => serde_json::json!([
            event.pair,
            event.reference_owner,
            event.reference_face,
            event.normal_orientation,
            event
                .points
                .iter()
                .map(|point| point.identity)
                .collect::<Vec<_>>()
        ]),
        EventFamilyV1::WarmCacheMatchAndImpulseOwnership => serde_json::json!([
            event.pair,
            event
                .points
                .iter()
                .map(|point| (point.cache_hit, point.cache_owner))
                .collect::<Vec<_>>()
        ]),
        EventFamilyV1::SupportGraphColorAndOverflowOrder => {
            serde_json::json!([event.pair, event.graph_class, event.visit_rank])
        }
        _ => serde_json::Value::Null,
    }
}

fn normal_token(family: EventFamilyV1, event: &NormalDecisionEventV1) -> bool {
    match family {
        EventFamilyV1::SeparationPositiveAndSpeculativeBranch => event.separation_positive,
        EventFamilyV1::UnilateralActiveSetProjection => event.candidate_selected,
        _ => false,
    }
}

fn prepared_key(event: &PreparedContactEventV1) -> serde_json::Value {
    serde_json::json!([event.completed_step, event.pair])
}

fn normal_key(event: &NormalDecisionEventV1) -> serde_json::Value {
    serde_json::json!([
        event.completed_step,
        event.substep,
        event.pass,
        event.visit_rank,
        event.pair,
        event.point_ordinal,
        event.identity
    ])
}

fn first_impulse_delta(
    receiver: &[NormalDecisionEventV1],
    routed: &[NormalDecisionEventV1],
    start: usize,
) -> Option<f64> {
    receiver
        .iter()
        .zip(routed)
        .skip(start)
        .find_map(|(before, after)| {
            (before.applied_impulse != after.applied_impulse)
                .then_some(after.applied_impulse - before.applied_impulse)
        })
}

fn event_hash(events: &EventStreamV1) -> Result<String, String> {
    serde_json::to_vec(events)
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(display)
}

fn scalar_result_hash(
    solve: &physbox_lab::scalar::ScalarObservedSolveResultV1,
    events: &EventStreamV1,
) -> Result<String, String> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct Projection<'a> {
        observations: &'a [physbox_lab::scalar::ScalarStepObservationV1],
        final_bodies: &'a [physbox_contracts::BodyStateV1],
        final_cache: &'a [physbox_contracts::ContactCachePointV1],
        events: &'a EventStreamV1,
    }
    serde_json::to_vec(&Projection {
        observations: &solve.observations,
        final_bodies: &solve.result.bodies,
        final_cache: &solve.result.contact_cache,
        events,
    })
    .map(|bytes| sha256_bytes(&bytes))
    .map_err(display)
}

fn full_result_hash(
    solve: &physbox_lab::precision_margin::FullF64SolveResultV1,
    events: &EventStreamV1,
) -> Result<String, String> {
    serde_json::to_vec(&(solve, events))
        .map(|bytes| sha256_bytes(&bytes))
        .map_err(display)
}

fn arguments() -> Result<PathBuf, String> {
    let mut args = env::args().skip(1);
    match (args.next().as_deref(), args.next(), args.next()) {
        (Some("--output"), Some(path), None) => Ok(PathBuf::from(path)),
        _ => Err("usage: physbox-s16-event-replay --output PATH".to_owned()),
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matrix_is_exactly_two_by_six_by_two() {
        assert_eq!(EventFamilyV1::ALL.len() * 2 * 2, 24);
    }
}
