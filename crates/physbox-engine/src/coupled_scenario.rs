use physbox_contracts::{
    BodyId, BodyStateV1, CoupledScenarioV1, CoupledScheduleV1, CoupledSettingsV1, FiniteF32,
    GrabTuningV1, MaterialV1, NeutralActionV1, ParticleMaterialV1, ParticleRegionV1,
    SCHEMA_VERSION, ScenarioV1, ShapeV1, SolverSettingsV1, Validate, Vec2, canonical_hash,
};

use crate::ScenarioBuildError;

pub const COUPLED_CANONICAL_SOURCE_COUNT: usize = 7;
pub const COUPLED_CANONICAL_SOURCE_HASHES_V1: [(&str, &str); COUPLED_CANONICAL_SOURCE_COUNT] = [
    (
        "coupled-contained-water-v1",
        "21a50b90d3c4d82c6b86e2c7cfb5d379bce46c28d6bb589ff44bf5e0c459cc4c",
    ),
    (
        "coupled-displacement-v1",
        "5b3fe7f26670d56cb44e700e654f6d732b7b1d9659e689abb062d50a3e724380",
    ),
    (
        "coupled-float-sink-v1",
        "2d13119f7bb6eb76eadd0f1859adeb61041ab5ab2d4f3c0a7cd07b26bf98cb83",
    ),
    (
        "coupled-off-center-torque-v1",
        "2fbb05ace68857fa08e1ff07454002f0ce4cf47f3ca375193209c4e35ee698f6",
    ),
    (
        "coupled-dam-break-v1",
        "222058df5d447014878a366b212cf3a9163cc282eea6f35df04c57a669548e04",
    ),
    (
        "coupled-grabbed-boundary-v1",
        "db0fe6e2851ae48e2911435096a5114a14f8df8b3522ed08675363fbab0f9c94",
    ),
    (
        "coupled-fluid-wake-v1",
        "4668b95de75b5a561bea856fdbc5e938bbb512c8267ca82c0db0e628877782a2",
    ),
];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub enum CoupledScenarioKind {
    ContainedWater,
    Displacement,
    FloatSink,
    OffCenterTorque,
    DamBreak,
    GrabbedBoundary,
    FluidWake,
}

impl CoupledScenarioKind {
    pub const ALL: [Self; COUPLED_CANONICAL_SOURCE_COUNT] = [
        Self::ContainedWater,
        Self::Displacement,
        Self::FloatSink,
        Self::OffCenterTorque,
        Self::DamBreak,
        Self::GrabbedBoundary,
        Self::FluidWake,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::ContainedWater => "coupled-contained-water-v1",
            Self::Displacement => "coupled-displacement-v1",
            Self::FloatSink => "coupled-float-sink-v1",
            Self::OffCenterTorque => "coupled-off-center-torque-v1",
            Self::DamBreak => "coupled-dam-break-v1",
            Self::GrabbedBoundary => "coupled-grabbed-boundary-v1",
            Self::FluidWake => "coupled-fluid-wake-v1",
        }
    }
}

pub fn canonical_coupled_scenario(
    kind: CoupledScenarioKind,
) -> Result<CoupledScenarioV1, ScenarioBuildError> {
    let requested_steps = match kind {
        CoupledScenarioKind::ContainedWater => 360,
        CoupledScenarioKind::Displacement => 480,
        CoupledScenarioKind::FloatSink => 600,
        CoupledScenarioKind::OffCenterTorque => 360,
        CoupledScenarioKind::DamBreak => 480,
        CoupledScenarioKind::GrabbedBoundary => 360,
        CoupledScenarioKind::FluidWake => 480,
    };
    let mut bodies = container();
    let mut actions = Vec::new();
    let regions = match kind {
        CoupledScenarioKind::ContainedWater => {
            vec![region(0, Vec2::new(-3.8, -0.0), Vec2::new(3.8, 2.4))]
        }
        CoupledScenarioKind::Displacement => {
            bodies.push(dynamic_rectangle(
                3,
                Vec2::new(0.0, 3.4),
                Vec2::new(0.8, 0.6),
                1.0,
                false,
            ));
            vec![region(0, Vec2::new(-3.8, 0.0), Vec2::new(3.8, 1.8))]
        }
        CoupledScenarioKind::FloatSink => {
            bodies.extend([
                dynamic_rectangle(3, Vec2::new(-2.2, 2.8), Vec2::new(0.55, 0.4), 4.0, false),
                dynamic_rectangle(4, Vec2::new(0.0, 2.8), Vec2::new(0.55, 0.4), 1.0, false),
                dynamic_rectangle(5, Vec2::new(2.2, 2.8), Vec2::new(0.55, 0.4), 0.25, false),
            ]);
            vec![region(0, Vec2::new(-3.8, 0.0), Vec2::new(3.8, 2.2))]
        }
        CoupledScenarioKind::OffCenterTorque => {
            bodies.push(dynamic_rectangle(
                3,
                Vec2::new(1.2, 1.5),
                Vec2::new(1.0, 0.25),
                0.75,
                false,
            ));
            vec![ParticleRegionV1 {
                initial_velocity: Vec2::new(4.0, 0.0),
                ..region(0, Vec2::new(-4.0, 0.4), Vec2::new(-1.6, 1.8))
            }]
        }
        CoupledScenarioKind::DamBreak => {
            bodies.push(dynamic_circle(3, Vec2::new(2.6, 0.8), 0.55, 1.0, false));
            vec![region(0, Vec2::new(-4.2, 0.0), Vec2::new(-0.6, 3.6))]
        }
        CoupledScenarioKind::GrabbedBoundary => {
            let body = dynamic_rectangle(3, Vec2::new(0.0, 2.0), Vec2::new(0.8, 0.3), 1.0, false);
            let body_id = body.id;
            bodies.push(body);
            actions.extend([
                NeutralActionV1::GrabBegin {
                    step: 30,
                    body: body_id,
                    world_anchor: Vec2::new(0.0, 2.0),
                    world_target: Vec2::new(0.0, 2.0),
                    tuning: grab_tuning(),
                },
                NeutralActionV1::GrabTarget {
                    step: 90,
                    world_target: Vec2::new(-2.0, 0.8),
                },
                NeutralActionV1::GrabTarget {
                    step: 180,
                    world_target: Vec2::new(2.0, 0.8),
                },
                NeutralActionV1::GrabEnd { step: 240 },
            ]);
            vec![region(0, Vec2::new(-3.8, 0.0), Vec2::new(3.8, 1.8))]
        }
        CoupledScenarioKind::FluidWake => {
            bodies.push(dynamic_rectangle(
                3,
                Vec2::new(1.8, 0.65),
                Vec2::new(0.65, 0.35),
                1.0,
                true,
            ));
            vec![ParticleRegionV1 {
                initial_velocity: Vec2::new(3.0, 0.0),
                ..region(0, Vec2::new(-3.8, 0.2), Vec2::new(-1.2, 1.6))
            }]
        }
    };
    bodies.sort_by_key(|body| body.id);
    let rigid = ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: format!("{}-rigid", kind.id()),
        seed: 0x5035_3100 + kind as u64,
        requested_steps,
        settings: default_settings(),
        bodies,
        manifolds: Vec::new(),
        contact_cache: Vec::new(),
        actions,
    };
    let scenario = CoupledScenarioV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: physbox_contracts::COUPLED_CONTRACT_VERSION,
        id: kind.id().to_owned(),
        seed: 0x5035_3100 + kind as u64,
        requested_steps,
        particle_capacity: 4_096,
        rigid,
        settings: CoupledSettingsV1::initial(),
        materials: vec![ParticleMaterialV1::water_v1(0)],
        regions,
        actions: Vec::new(),
    };
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

pub fn liquid_sandbox_scenario() -> Result<CoupledScenarioV1, ScenarioBuildError> {
    let mut scenario = canonical_coupled_scenario(CoupledScenarioKind::ContainedWater)?;
    scenario.id = "liquid-sandbox-v1".to_owned();
    scenario.seed = 0x5035_31ff;
    scenario.requested_steps = 600;
    scenario.particle_capacity = crate::particle_world::LIQUID_SANDBOX_PARTICLE_CAPACITY;
    scenario.rigid.id = "liquid-sandbox-v1-rigid".to_owned();
    scenario.rigid.seed = scenario.seed;
    scenario.rigid.requested_steps = scenario.requested_steps;
    scenario.settings.schedule = CoupledScheduleV1::RigidThenCoupledBatch;
    scenario
        .rigid
        .bodies
        .push(dynamic_circle(3, Vec2::new(0.0, 5.45), 0.45, 2.0, false));
    scenario.rigid.bodies.sort_by_key(|body| body.id);
    let mut water = ParticleMaterialV1::water_v1(0);
    water.radius = finite(0.05);
    scenario.materials = vec![water];
    scenario.regions = vec![ParticleRegionV1 {
        id: 0,
        material_id: 0,
        lower: Vec2::new(-3.8, 0.0),
        upper: Vec2::new(3.8, 5.2),
        spacing: finite(0.1),
        initial_velocity: Vec2::ZERO,
    }];
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

pub fn coupled_capacity_scenario(
    particle_count: u32,
) -> Result<CoupledScenarioV1, ScenarioBuildError> {
    let columns = match particle_count {
        1_024 => 32,
        4_096 => 64,
        16_384 => 128,
        32_768 => 256,
        _ => {
            return Err(ScenarioBuildError(
                "coupled capacity evidence supports exactly 1024, 4096, 16384, or 32768 particles"
                    .to_owned(),
            ));
        }
    };
    let rows = particle_count / columns;
    let spacing = 0.1;
    let width = columns as f32 * spacing;
    let height = rows as f32 * spacing;
    let lower = Vec2::new(-0.5 * width, -6.0);
    let upper = Vec2::new(lower.x + width, lower.y + height);
    let center_y = 0.5 * (lower.y + upper.y);
    let mut scenario = canonical_coupled_scenario(CoupledScenarioKind::ContainedWater)?;
    scenario.id = format!("coupled-capacity-{particle_count}-v1");
    scenario.seed = 0x5035_3200 + u64::from(particle_count);
    scenario.requested_steps = 120;
    scenario.particle_capacity = particle_count;
    scenario.rigid.id = format!("{}-rigid", scenario.id);
    scenario.rigid.seed = scenario.seed;
    scenario.rigid.requested_steps = scenario.requested_steps;
    scenario.rigid.bodies = vec![
        fixed_rectangle(
            0,
            Vec2::new(0.0, lower.y - 0.5),
            Vec2::new(0.5 * width + 1.0, 0.5),
        ),
        fixed_rectangle(
            1,
            Vec2::new(lower.x - 0.5, center_y),
            Vec2::new(0.5, 0.5 * height + 0.5),
        ),
        fixed_rectangle(
            2,
            Vec2::new(upper.x + 0.5, center_y),
            Vec2::new(0.5, 0.5 * height + 0.5),
        ),
    ];
    let mut water = ParticleMaterialV1::water_v1(0);
    water.radius = finite(0.05);
    scenario.materials = vec![water];
    scenario.regions = vec![ParticleRegionV1 {
        id: 0,
        material_id: 0,
        lower,
        upper,
        spacing: finite(spacing),
        initial_velocity: Vec2::ZERO,
    }];
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

pub fn canonical_coupled_scenario_hashes() -> Result<Vec<(&'static str, String)>, ScenarioBuildError>
{
    CoupledScenarioKind::ALL
        .into_iter()
        .map(|kind| {
            let scenario = canonical_coupled_scenario(kind)?;
            canonical_hash(&scenario)
                .map(|hash| (kind.id(), hash))
                .map_err(|error| ScenarioBuildError(error.to_string()))
        })
        .collect()
}

fn container() -> Vec<BodyStateV1> {
    vec![
        fixed_rectangle(0, Vec2::new(0.0, -0.5), Vec2::new(6.0, 0.5)),
        fixed_rectangle(1, Vec2::new(-5.5, 2.5), Vec2::new(0.5, 3.0)),
        fixed_rectangle(2, Vec2::new(5.5, 2.5), Vec2::new(0.5, 3.0)),
    ]
}

fn region(id: u32, lower: Vec2, upper: Vec2) -> ParticleRegionV1 {
    ParticleRegionV1 {
        id,
        material_id: 0,
        lower,
        upper,
        spacing: finite(0.2),
        initial_velocity: Vec2::ZERO,
    }
}

fn fixed_rectangle(slot: u32, position: Vec2, half_extents: Vec2) -> BodyStateV1 {
    body(
        slot,
        ShapeV1::Rectangle { half_extents },
        position,
        0.0,
        0.0,
        false,
    )
}

fn dynamic_rectangle(
    slot: u32,
    position: Vec2,
    half_extents: Vec2,
    inverse_mass: f32,
    sleeping: bool,
) -> BodyStateV1 {
    let mass = 1.0 / inverse_mass;
    let inertia = mass * (half_extents.x * half_extents.x + half_extents.y * half_extents.y) / 3.0;
    body(
        slot,
        ShapeV1::Rectangle { half_extents },
        position,
        inverse_mass,
        1.0 / inertia,
        sleeping,
    )
}

fn dynamic_circle(
    slot: u32,
    position: Vec2,
    radius: f32,
    inverse_mass: f32,
    sleeping: bool,
) -> BodyStateV1 {
    let mass = 1.0 / inverse_mass;
    let inertia = 0.5 * mass * radius * radius;
    body(
        slot,
        ShapeV1::Circle {
            radius: finite(radius),
        },
        position,
        inverse_mass,
        1.0 / inertia,
        sleeping,
    )
}

fn body(
    slot: u32,
    shape: ShapeV1,
    position: Vec2,
    inverse_mass: f32,
    inverse_inertia: f32,
    sleeping: bool,
) -> BodyStateV1 {
    BodyStateV1 {
        id: BodyId {
            slot,
            generation: 1,
        },
        shape,
        position,
        angle: finite(0.0),
        linear_velocity: Vec2::ZERO,
        angular_velocity: finite(0.0),
        inverse_mass: finite(inverse_mass),
        inverse_inertia: finite(inverse_inertia),
        material: MaterialV1 {
            friction: finite(0.2),
            restitution: finite(0.0),
        },
        sleeping,
    }
}

fn default_settings() -> SolverSettingsV1 {
    SolverSettingsV1 {
        fixed_dt: finite(1.0 / 60.0),
        gravity: Vec2::new(0.0, -10.0),
        substeps: 4,
        iterations: 1,
        bias_rate: finite(0.2),
        mass_coefficient: finite(1.0),
        impulse_coefficient: finite(0.0),
        contact_softness: None,
        restitution_threshold: finite(1.0),
        contact_policy: None,
    }
}

fn grab_tuning() -> GrabTuningV1 {
    GrabTuningV1 {
        spring_acceleration: finite(120.0),
        damping_rate: finite(18.0),
        max_acceleration: finite(400.0),
        max_linear_speed: finite(25.0),
        max_angular_speed: finite(20.0),
    }
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("coupled scenario constants are finite")
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeSet;

    #[test]
    fn canonical_coupled_sources_are_valid_unique_and_bounded() {
        let mut ids = BTreeSet::new();
        let mut hashes = BTreeSet::new();
        for kind in CoupledScenarioKind::ALL {
            let scenario = canonical_coupled_scenario(kind).unwrap();
            scenario.validate().unwrap();
            assert!(ids.insert(scenario.id.clone()));
            assert!(hashes.insert(canonical_hash(&scenario).unwrap()));
            assert_eq!(scenario.particle_capacity, 4_096);
            assert_eq!(scenario.materials.len(), 1);
        }
    }

    #[test]
    fn canonical_coupled_hashes_match_frozen_v1_sources() {
        let actual = canonical_coupled_scenario_hashes().unwrap();
        assert_eq!(actual.len(), COUPLED_CANONICAL_SOURCE_HASHES_V1.len());
        for ((actual_id, actual_hash), (expected_id, expected_hash)) in
            actual.iter().zip(COUPLED_CANONICAL_SOURCE_HASHES_V1)
        {
            assert_eq!(*actual_id, expected_id);
            assert_eq!(actual_hash, expected_hash);
        }
    }

    #[test]
    fn liquid_sandbox_keeps_painting_headroom_below_the_capacity_frontier() {
        let scenario = liquid_sandbox_scenario().unwrap();
        assert_eq!(
            scenario.particle_capacity,
            crate::particle_world::LIQUID_SANDBOX_PARTICLE_CAPACITY
        );
        assert!(scenario.particle_capacity < crate::particle_world::MAX_PRODUCT_PARTICLE_CAPACITY);
        assert_eq!(scenario.regions.len(), 1);
    }

    #[test]
    fn capacity_sources_author_exact_declared_particle_rows() {
        for (count, columns) in [(1_024, 32), (4_096, 64), (16_384, 128), (32_768, 256)] {
            let scenario = coupled_capacity_scenario(count).unwrap();
            let region = scenario.regions[0];
            let spacing = region.spacing.get();
            let authored_columns = ((region.upper.x - region.lower.x) / spacing).round() as u32;
            let authored_rows = ((region.upper.y - region.lower.y) / spacing).round() as u32;
            assert_eq!(authored_columns, columns);
            assert_eq!(authored_columns * authored_rows, count);
            assert_eq!(scenario.particle_capacity, count);
        }
        assert!(coupled_capacity_scenario(2_048).is_err());
    }
}
