use std::fmt::{Display, Formatter, Write};

use physbox_contracts::{
    BodyStateV1, FiniteF32, OracleResultV1, PlaybackArtifactV1, PlaybackSampleV1, ShapeV1,
    Validate, sha256_bytes,
};

use crate::scalar::ScalarSolveResult;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PlaybackError(pub String);

impl Display for PlaybackError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for PlaybackError {}

pub fn synchronized_sample(
    completed_steps: u32,
    scalar: ScalarSolveResult,
    oracle: &OracleResultV1,
    oracle_artifact: String,
) -> Result<PlaybackSampleV1, PlaybackError> {
    let (position, angle, linear_velocity, angular_velocity, dynamic_sleep_state_match) =
        body_deltas(&scalar.bodies, &oracle.bodies)?;
    let trace_bytes = serde_json::to_vec(&scalar.traces)
        .map_err(|error| PlaybackError(format!("serialize scalar trace: {error}")))?;
    let sample = PlaybackSampleV1 {
        completed_steps,
        scalar_bodies: scalar.bodies,
        authority_bodies: oracle.bodies.clone(),
        scalar_trace_records: scalar.traces.len() as u64,
        scalar_trace_hash: sha256_bytes(&trace_bytes),
        oracle_scenario_hash: oracle.source_scenario_hash.clone(),
        oracle_projection_hash: oracle.projection_hash.clone(),
        oracle_provenance: oracle.provenance.clone(),
        oracle_artifact,
        position_delta_max: finite(position, "position delta")?,
        angle_delta_max: finite(angle, "angle delta")?,
        linear_velocity_delta_max: finite(linear_velocity, "linear velocity delta")?,
        angular_velocity_delta_max: finite(angular_velocity, "angular velocity delta")?,
        dynamic_sleep_state_match,
    };
    sample
        .validate()
        .map_err(|error| PlaybackError(error.to_string()))?;
    Ok(sample)
}

pub fn render_svg(artifact: &PlaybackArtifactV1) -> Result<String, PlaybackError> {
    artifact
        .validate()
        .map_err(|error| PlaybackError(error.to_string()))?;
    const LABEL_WIDTH: f32 = 158.0;
    const CELL_WIDTH: f32 = 148.0;
    const TRACK_HEIGHT: f32 = 230.0;
    let sample_count = artifact
        .tracks
        .iter()
        .map(|track| track.samples.len())
        .max()
        .unwrap_or(1);
    let width = LABEL_WIDTH + CELL_WIDTH * sample_count as f32 + 20.0;
    let height = 92.0 + TRACK_HEIGHT * artifact.tracks.len() as f32;
    let mut output = String::new();
    writeln!(
        output,
        "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"{width}\" height=\"{height}\" viewBox=\"0 0 {width} {height}\">"
    )
    .unwrap();
    output.push_str(
        "<style>text{font-family:ui-monospace,SFMono-Regular,monospace;fill:#dce8f2}.title{font:700 18px system-ui}.track{font:700 14px system-ui}.meta{font-size:10px;fill:#91a8ba}.lane{font:700 10px system-ui}.delta{font-size:9px;fill:#b5c7d4}.plot{fill:#132738;stroke:#355166}.static{fill:#60798c}.scalar{fill:#5ee1dc}.authority{fill:#ffc267}</style>\n",
    );
    output.push_str("<rect width=\"100%\" height=\"100%\" fill=\"#0b1823\"/>\n");
    write!(
        output,
        "<text class=\"title\" x=\"18\" y=\"28\">PhysBox 5 synchronized scalar / Box2D playback</text>\n<text class=\"meta\" x=\"18\" y=\"48\">run {}</text>\n<text class=\"meta\" x=\"18\" y=\"64\">PhysBox {}{} · authority {}</text>\n",
        escape(&artifact.run_id),
        escape(&artifact.harness_provenance.source_revision),
        if artifact.harness_provenance.source_dirty {
            " + dirty"
        } else {
            ""
        },
        escape(
            &artifact.tracks[0].samples[0]
                .oracle_provenance
                .source_revision
        )
    )
    .unwrap();

    for (track_index, track) in artifact.tracks.iter().enumerate() {
        let top = 82.0 + TRACK_HEIGHT * track_index as f32;
        write!(
            output,
            "<text class=\"track\" x=\"18\" y=\"{}\">{}</text>\n<text class=\"meta\" x=\"18\" y=\"{}\">scenario {}</text>\n<text class=\"lane\" x=\"18\" y=\"{}\" fill=\"#5ee1dc\">SCALAR</text>\n<text class=\"lane\" x=\"18\" y=\"{}\" fill=\"#ffc267\">BOX2D</text>\n",
            top + 18.0,
            escape(&track.id),
            top + 34.0,
            &track.source_scenario_hash[..12],
            top + 79.0,
            top + 151.0,
        )
        .unwrap();
        for (sample_index, sample) in track.samples.iter().enumerate() {
            let x = LABEL_WIDTH + CELL_WIDTH * sample_index as f32;
            writeln!(
                output,
                "<text class=\"lane\" x=\"{}\" y=\"{}\">step {}</text>",
                x + 4.0,
                top + 18.0,
                sample.completed_steps
            )
            .unwrap();
            draw_lane(
                &mut output,
                track_index,
                sample_index,
                0,
                x,
                top + 42.0,
                &sample.scalar_bodies,
                "scalar",
            );
            draw_lane(
                &mut output,
                track_index,
                sample_index,
                1,
                x,
                top + 114.0,
                &sample.authority_bodies,
                "authority",
            );
            write!(
                output,
                "<text class=\"delta\" x=\"{}\" y=\"{}\">Δp {:.4}  Δa {:.4}</text>\n<text class=\"delta\" x=\"{}\" y=\"{}\">Δv {:.4}  sleep {}</text>\n",
                x + 4.0,
                top + 194.0,
                sample.position_delta_max.get(),
                sample.angle_delta_max.get(),
                x + 4.0,
                top + 208.0,
                sample.linear_velocity_delta_max.get(),
                if sample.dynamic_sleep_state_match {
                    "same"
                } else {
                    "diff"
                }
            )
            .unwrap();
        }
    }
    output.push_str("</svg>\n");
    Ok(output)
}

fn body_deltas(
    scalar: &[BodyStateV1],
    authority: &[BodyStateV1],
) -> Result<(f32, f32, f32, f32, bool), PlaybackError> {
    if scalar.len() != authority.len()
        || scalar
            .iter()
            .zip(authority)
            .any(|(left, right)| left.id != right.id)
    {
        return Err(PlaybackError(
            "scalar and authority body identities differ".to_owned(),
        ));
    }
    let mut position: f32 = 0.0;
    let mut angle: f32 = 0.0;
    let mut linear_velocity: f32 = 0.0;
    let mut angular_velocity: f32 = 0.0;
    let mut dynamic_sleep_state_match = true;
    for (left, right) in scalar.iter().zip(authority) {
        position = position.max(length(
            left.position.x - right.position.x,
            left.position.y - right.position.y,
        ));
        let angle_difference = left.angle.get() - right.angle.get();
        angle = angle.max(angle_difference.sin().atan2(angle_difference.cos()).abs());
        linear_velocity = linear_velocity.max(length(
            left.linear_velocity.x - right.linear_velocity.x,
            left.linear_velocity.y - right.linear_velocity.y,
        ));
        angular_velocity = angular_velocity
            .max((left.angular_velocity.get() - right.angular_velocity.get()).abs());
        if left.inverse_mass.get() > 0.0 {
            dynamic_sleep_state_match &= left.sleeping == right.sleeping;
        }
    }
    Ok((
        position,
        angle,
        linear_velocity,
        angular_velocity,
        dynamic_sleep_state_match,
    ))
}

#[allow(clippy::too_many_arguments)]
fn draw_lane(
    output: &mut String,
    track_index: usize,
    sample_index: usize,
    lane_index: usize,
    x: f32,
    y: f32,
    bodies: &[BodyStateV1],
    class: &str,
) {
    const PLOT_WIDTH: f32 = 136.0;
    const PLOT_HEIGHT: f32 = 60.0;
    const WORLD_LEFT: f32 = -3.0;
    const WORLD_BOTTOM: f32 = -0.55;
    const WORLD_WIDTH: f32 = 6.0;
    const WORLD_HEIGHT: f32 = 2.4;
    let clip = format!("clip-{track_index}-{sample_index}-{lane_index}");
    write!(
        output,
        "<defs><clipPath id=\"{clip}\"><rect x=\"{}\" y=\"{y}\" width=\"{PLOT_WIDTH}\" height=\"{PLOT_HEIGHT}\"/></clipPath></defs><rect class=\"plot\" x=\"{}\" y=\"{y}\" width=\"{PLOT_WIDTH}\" height=\"{PLOT_HEIGHT}\"/>\n<g clip-path=\"url(#{clip})\">\n",
        x + 2.0,
        x + 2.0,
    )
    .unwrap();
    for body in bodies {
        let center_x = x + 2.0 + (body.position.x - WORLD_LEFT) / WORLD_WIDTH * PLOT_WIDTH;
        let center_y =
            y + PLOT_HEIGHT - (body.position.y - WORLD_BOTTOM) / WORLD_HEIGHT * PLOT_HEIGHT;
        let body_class = if body.inverse_mass.get() == 0.0 {
            "static"
        } else {
            class
        };
        match body.shape {
            ShapeV1::Circle { radius } => {
                writeln!(
                    output,
                    "<circle class=\"{body_class}\" cx=\"{center_x}\" cy=\"{center_y}\" r=\"{}\"/>",
                    radius.get() / WORLD_HEIGHT * PLOT_HEIGHT
                )
                .unwrap();
            }
            ShapeV1::Rectangle { half_extents } => {
                let width = half_extents.x * 2.0 / WORLD_WIDTH * PLOT_WIDTH;
                let height = half_extents.y * 2.0 / WORLD_HEIGHT * PLOT_HEIGHT;
                writeln!(
                    output,
                    "<rect class=\"{body_class}\" x=\"{}\" y=\"{}\" width=\"{width}\" height=\"{height}\" transform=\"rotate({} {center_x} {center_y})\"/>",
                    center_x - width * 0.5,
                    center_y - height * 0.5,
                    -body.angle.get().to_degrees(),
                )
                .unwrap();
            }
        }
    }
    output.push_str("</g>\n");
}

fn finite(value: f32, name: &str) -> Result<FiniteF32, PlaybackError> {
    FiniteF32::new(value).map_err(|_| PlaybackError(format!("{name} is non-finite")))
}

fn length(x: f32, y: f32) -> f32 {
    (x * x + y * y).sqrt()
}

fn escape(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{
        MetricsV1, NamedValueV1, ProvenanceV1, SCHEMA_VERSION, WorkVectorV1, canonical_hash,
    };
    use physbox_engine::{AnalyticFixtureKind, analytic_fixture_scenario};

    use super::*;

    #[test]
    fn sample_reports_objective_deltas_and_svg_contains_both_lanes() {
        let scenario = analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 1).unwrap();
        let scalar = ScalarSolveResult {
            bodies: scenario.bodies.clone(),
            contact_cache: Vec::new(),
            traces: Vec::new(),
        };
        let mut oracle_bodies = scenario.bodies.clone();
        oracle_bodies[0].position.x = 0.25;
        let oracle = OracleResultV1 {
            schema_version: SCHEMA_VERSION,
            run_id: "oracle-1".to_owned(),
            source_scenario_hash: canonical_hash(&scenario).unwrap(),
            projection_hash: "1".repeat(64),
            provenance: provenance("oracle-1"),
            bodies: oracle_bodies,
            manifolds: Vec::new(),
            traces: Vec::new(),
            metrics: empty_metrics(),
            failure: None,
        };
        let sample =
            synchronized_sample(1, scalar, &oracle, "oracles/oracle-1".to_owned()).unwrap();
        assert_eq!(sample.position_delta_max.get(), 0.25);
        let track = physbox_contracts::PlaybackTrackV1 {
            id: "circle-floor".to_owned(),
            source_scenario_hash: canonical_hash(&scenario).unwrap(),
            source_scenario: scenario,
            samples: vec![sample],
        };
        let artifact = PlaybackArtifactV1 {
            schema_version: SCHEMA_VERSION,
            run_id: "playback-1".to_owned(),
            harness_provenance: provenance("playback-1"),
            tracks: vec![track],
        };
        let svg = render_svg(&artifact).unwrap();
        assert!(svg.contains("SCALAR"));
        assert!(svg.contains("BOX2D"));
        assert!(svg.contains("Δp 0.2500"));

        let mut broken = artifact;
        broken.tracks[0].samples[0].oracle_artifact = "../stale-result".to_owned();
        assert!(broken.validate().is_err());
    }

    fn provenance(run_id: &str) -> ProvenanceV1 {
        ProvenanceV1 {
            run_id: run_id.to_owned(),
            source_revision: "revision".to_owned(),
            source_dirty: false,
            source_patch_digest: None,
            host: "test-host".to_owned(),
            adapter: "test-adapter".to_owned(),
            backend: "native-cpu".to_owned(),
            driver: "test".to_owned(),
            toolchain: "rustc test".to_owned(),
            build_options: vec![NamedValueV1 {
                name: "profile".to_owned(),
                value: "test".to_owned(),
            }],
            runtime_settings: Vec::new(),
        }
    }

    fn empty_metrics() -> MetricsV1 {
        let zero = FiniteF32::new(0.0).unwrap();
        MetricsV1 {
            work: WorkVectorV1 {
                bodies_total: 2,
                bodies_awake: 1,
                circles: 1,
                rectangles: 1,
                candidate_pairs: 0,
                overlapping_pairs: 0,
                manifolds: 0,
                contact_points: 0,
                solver_work: 0,
                dispatches: 0,
            },
            total_physics: None,
            cpu_encode_submit: None,
            capacities: Vec::new(),
            penetration_max: zero,
            jitter_rms: zero,
            energy_delta: zero,
        }
    }
}
