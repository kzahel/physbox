use std::{env, fs, fs::File, io::Write, path::PathBuf};

use flate2::{Compression, GzBuilder};
use physbox_contracts::{ScenarioV1, Validate, canonical_hash, sha256_bytes};
use physbox_lab::{
    authority_structure::{AuthorityEventSolveResultV1, solve_authority_event_stream_v1},
    event_structure::{EventLaneV1, EventStreamV1},
    precision_margin::{FullF64EventSolveResultV1, solve_full_f64_authority_events_v1},
};
use serde::Serialize;

const VERSION: u32 = 1;
const ALLOWED_LABELS: [&str; 2] = [
    "s11.2-centered-eleven-soft",
    "governed-centered-sixteen-soft",
];

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct LaneCapture {
    lane: EventLaneV1,
    exact_replay: bool,
    observation_hash: String,
    prepared_contact_events: usize,
    normal_decision_events: usize,
    events: EventStreamV1,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Capture {
    version: u32,
    artifact_kind: &'static str,
    source_label: String,
    source_scenario_hash: String,
    requested_steps: u32,
    substeps: u32,
    dynamic_depth: usize,
    streams: Vec<LaneCapture>,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-s16-event-divergence: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let (source_path, label, output_path) = arguments()?;
    if !ALLOWED_LABELS.contains(&label.as_str()) {
        return Err(format!("S16 A1 source {label} is not governed"));
    }
    let source_bytes = fs::read(&source_path).map_err(display)?;
    let source: ScenarioV1 = serde_json::from_slice(&source_bytes).map_err(display)?;
    source.validate().map_err(display)?;
    let dynamic_depth = source
        .bodies
        .iter()
        .filter(|body| body.inverse_mass.get() != 0.0)
        .count();
    let expected_depth = if label == ALLOWED_LABELS[0] { 11 } else { 16 };
    if source.requested_steps != 1_200
        || source.settings.substeps != 4
        || dynamic_depth != expected_depth
        || source.settings.contact_softness.is_none()
        || !source.actions.is_empty()
        || !source.manifolds.is_empty()
        || !source.contact_cache.is_empty()
    {
        return Err("S16 A1 governed source contract drift".to_owned());
    }
    let source_scenario_hash = canonical_hash(&source).map_err(display)?;

    let exact_primary =
        solve_authority_event_stream_v1(&source, EventLaneV1::ExactS12F32).map_err(display)?;
    let exact_replay =
        solve_authority_event_stream_v1(&source, EventLaneV1::ExactS12F32).map_err(display)?;
    let exact = scalar_lane(exact_primary, exact_replay)?;

    let path_primary = solve_authority_event_stream_v1(&source, EventLaneV1::CancellationPathF64)
        .map_err(display)?;
    let path_replay = solve_authority_event_stream_v1(&source, EventLaneV1::CancellationPathF64)
        .map_err(display)?;
    let path = scalar_lane(path_primary, path_replay)?;

    let full_primary = solve_full_f64_authority_events_v1(&source).map_err(display)?;
    let full_replay = solve_full_f64_authority_events_v1(&source).map_err(display)?;
    let full = full_lane(full_primary, full_replay)?;

    let capture = Capture {
        version: VERSION,
        artifact_kind: "p5.2b-s16.2-first-event-divergence-capture",
        source_label: label,
        source_scenario_hash,
        requested_steps: source.requested_steps,
        substeps: source.settings.substeps,
        dynamic_depth,
        streams: vec![exact, path, full],
    };
    let bytes = serde_json::to_vec(&capture).map_err(display)?;
    let file = File::create(&output_path).map_err(display)?;
    let mut encoder = GzBuilder::new().mtime(0).write(file, Compression::best());
    encoder.write_all(&bytes).map_err(display)?;
    encoder.finish().map_err(display)?;
    println!(
        "PHYSBOX5_VALIDATION_SUCCESS host=scalar scenario=s16.2-first-event-divergence source={} depth={} streams=3 replay=exact output={}",
        capture.source_label,
        capture.dynamic_depth,
        output_path.display(),
    );
    Ok(())
}

fn scalar_lane(
    primary: AuthorityEventSolveResultV1,
    replay: AuthorityEventSolveResultV1,
) -> Result<LaneCapture, String> {
    let exact_replay = primary == replay;
    if !exact_replay {
        return Err(format!(
            "{:?} scalar event replay differs",
            primary.events.lane
        ));
    }
    let observation_hash =
        sha256_bytes(&serde_json::to_vec(&primary.solve.observations).map_err(display)?);
    let prepared_contact_events = primary.events.prepared_contacts.len();
    let normal_decision_events = primary.events.normal_decisions.len();
    Ok(LaneCapture {
        lane: primary.events.lane,
        exact_replay,
        observation_hash,
        prepared_contact_events,
        normal_decision_events,
        events: primary.events,
    })
}

fn full_lane(
    primary: FullF64EventSolveResultV1,
    replay: FullF64EventSolveResultV1,
) -> Result<LaneCapture, String> {
    let exact_replay = primary == replay;
    if !exact_replay {
        return Err("full-solve-f64 scalar event replay differs".to_owned());
    }
    let observation_hash =
        sha256_bytes(&serde_json::to_vec(&primary.solve.observations).map_err(display)?);
    let prepared_contact_events = primary.events.prepared_contacts.len();
    let normal_decision_events = primary.events.normal_decisions.len();
    Ok(LaneCapture {
        lane: primary.events.lane,
        exact_replay,
        observation_hash,
        prepared_contact_events,
        normal_decision_events,
        events: primary.events,
    })
}

fn arguments() -> Result<(PathBuf, String, PathBuf), String> {
    let mut arguments = env::args().skip(1);
    match (
        arguments.next().as_deref(),
        arguments.next(),
        arguments.next().as_deref(),
        arguments.next(),
        arguments.next().as_deref(),
        arguments.next(),
        arguments.next(),
    ) {
        (
            Some("--source"),
            Some(source),
            Some("--label"),
            Some(label),
            Some("--output"),
            Some(output),
            None,
        ) => Ok((PathBuf::from(source), label, PathBuf::from(output))),
        _ => Err(
            "usage: physbox-s16-event-divergence --source PATH --label LABEL --output PATH"
                .to_owned(),
        ),
    }
}

fn display(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn governed_source_labels_are_frozen() {
        assert_eq!(ALLOWED_LABELS.len(), 2);
        assert_eq!(ALLOWED_LABELS[0], "s11.2-centered-eleven-soft");
        assert_eq!(ALLOWED_LABELS[1], "governed-centered-sixteen-soft");
    }
}
