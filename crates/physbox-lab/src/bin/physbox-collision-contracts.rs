use physbox_contracts::{
    ANALYTIC_SHAPE_FLAG_CIRCLE, ANALYTIC_SHAPE_FLAG_RECTANGLE, AnalyticShapeEncodingV1,
    CollisionLayoutV1, SCHEMA_VERSION, Validate, canonical_hash, collision_layout_v1,
};
use physbox_engine::{
    BODY_BYTES_PER_SLOT, BODY_COMMAND_BYTES, P5_2CollisionScenarioKind, analytic_shape_encoding,
    p5_2_box2d_narrowphase_projection, p5_2_collision_scenario,
};
use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ContractManifest {
    schema_version: u32,
    artifact_kind: &'static str,
    body_bytes_per_slot: u32,
    command_bytes: u32,
    rectangle_flag: u32,
    circle_flag: u32,
    layout: CollisionLayoutV1,
    scenarios: Vec<ScenarioRecord>,
    mixed_shape_encodings: Vec<AnalyticShapeEncodingV1>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ScenarioRecord {
    name: &'static str,
    source_scenario_hash: String,
    projection_hash: String,
    bodies: usize,
    circles: usize,
    rectangles: usize,
}

fn main() {
    if let Err(error) = run() {
        eprintln!("physbox-collision-contracts error: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), String> {
    let layout = collision_layout_v1();
    layout.validate().map_err(|error| error.to_string())?;
    let mut scenarios = Vec::new();
    let mut mixed_shape_encodings = Vec::new();
    for kind in P5_2CollisionScenarioKind::ALL {
        let source = p5_2_collision_scenario(kind, 120, 17).map_err(|error| error.to_string())?;
        let projection =
            p5_2_box2d_narrowphase_projection(&source).map_err(|error| error.to_string())?;
        scenarios.push(ScenarioRecord {
            name: kind.name(),
            source_scenario_hash: canonical_hash(&source).map_err(|error| error.to_string())?,
            projection_hash: canonical_hash(&projection).map_err(|error| error.to_string())?,
            bodies: source.bodies.len(),
            circles: source
                .bodies
                .iter()
                .filter(|body| matches!(body.shape, physbox_contracts::ShapeV1::Circle { .. }))
                .count(),
            rectangles: source
                .bodies
                .iter()
                .filter(|body| matches!(body.shape, physbox_contracts::ShapeV1::Rectangle { .. }))
                .count(),
        });
        if kind == P5_2CollisionScenarioKind::MixedSmallWorld {
            mixed_shape_encodings = source
                .bodies
                .iter()
                .map(analytic_shape_encoding)
                .collect::<Result<Vec<_>, _>>()?;
        }
    }
    let manifest = ContractManifest {
        schema_version: SCHEMA_VERSION,
        artifact_kind: "p5.2-s3.1-collision-contracts",
        body_bytes_per_slot: BODY_BYTES_PER_SLOT,
        command_bytes: BODY_COMMAND_BYTES,
        rectangle_flag: ANALYTIC_SHAPE_FLAG_RECTANGLE,
        circle_flag: ANALYTIC_SHAPE_FLAG_CIRCLE,
        layout,
        scenarios,
        mixed_shape_encodings,
    };
    println!(
        "{}",
        serde_json::to_string_pretty(&manifest).map_err(|error| error.to_string())?
    );
    eprintln!("PHYSBOX5_COLLISION_CONTRACTS_SUCCESS");
    Ok(())
}
