use physbox_contracts::{
    BodyId, BodyStateV1, CONSTRUCTION_INTENT_CONTRACT_VERSION, ConstructionIntentBatchV1,
    ConstructionIntentV1, ContractError, CoupledScenarioV1, CoupledScheduleV1, CoupledSettingsV1,
    FiniteF32, GrabTuningV1, MaterialV1, NeutralActionV1, ParticleMaterialV1, ParticleRegionV1,
    SCHEMA_VERSION, ScenarioV1, ShapeV1, SolverSettingsV1, Validate, Vec2, canonical_hash,
};
use serde::{Deserialize, Serialize};

use crate::ScenarioBuildError;

pub const COUPLED_CANONICAL_SOURCE_COUNT: usize = 7;
pub const FLUID_SOLID_OVERLAP_SOURCE_VERSION: u32 = 1;
pub const FLUID_SOLID_OVERLAP_PHASE_COUNT: usize = 4;
pub const FLUID_SOLID_OVERLAP_SETTLE_STEPS: u32 = 120;
pub const FLUID_SOLID_OVERLAP_RECOVERY_STEPS: u32 = 120;
pub const FLUID_SOLID_OVERLAP_SAMPLE_STRIDE: u32 = 4;
pub const FLUID_SOLID_OVERLAP_PHASE_OFFSETS: [f32; FLUID_SOLID_OVERLAP_PHASE_COUNT] =
    [0.0, 0.025, 0.05, 0.075];
pub const LIQUID_SCALE_BENCHMARK_VERSION: u32 = 1;
pub const LIQUID_SCALE_PARTICLE_COUNTS: [u32; 5] = [1_024, 4_096, 8_192, 16_384, 32_768];

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LiquidScaleSolidTier {
    WaterOnly,
    Solids20,
    Solids100,
}

impl LiquidScaleSolidTier {
    pub const ALL: [Self; 3] = [Self::WaterOnly, Self::Solids20, Self::Solids100];

    pub const fn id(self) -> &'static str {
        match self {
            Self::WaterOnly => "water-only",
            Self::Solids20 => "solids-20",
            Self::Solids100 => "solids-100",
        }
    }

    pub const fn dynamic_solid_count(self) -> u32 {
        match self {
            Self::WaterOnly => 0,
            Self::Solids20 => 20,
            Self::Solids100 => 100,
        }
    }

    pub const fn expected_body_capacity(self) -> u32 {
        match self {
            Self::WaterOnly | Self::Solids20 => 64,
            Self::Solids100 => 128,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LiquidScaleBenchmarkSourceV1 {
    pub version: u32,
    pub id: String,
    pub scenario_hash: String,
    pub solid_tier: LiquidScaleSolidTier,
    pub dynamic_solid_count: u32,
    pub fixed_solid_count: u32,
    pub particle_count: u32,
    pub expected_body_capacity: u32,
    pub scenario: CoupledScenarioV1,
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FluidSolidOverlapArrangement {
    CirclePair,
    CircleThreeBody,
    RectanglePair,
    MixedPair,
    MixedThreeBody,
}

impl FluidSolidOverlapArrangement {
    pub const ALL: [Self; 5] = [
        Self::CirclePair,
        Self::CircleThreeBody,
        Self::RectanglePair,
        Self::MixedPair,
        Self::MixedThreeBody,
    ];

    pub const fn id(self) -> &'static str {
        match self {
            Self::CirclePair => "circle-pair",
            Self::CircleThreeBody => "circle-three-body",
            Self::RectanglePair => "rectangle-pair",
            Self::MixedPair => "mixed-pair",
            Self::MixedThreeBody => "mixed-three-body",
        }
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FluidSolidOverlapSourceV1 {
    pub version: u32,
    pub id: String,
    pub arrangement: FluidSolidOverlapArrangement,
    pub particle_phase_index: u32,
    pub particle_phase_offset: Vec2,
    pub settle_steps: u32,
    pub recovery_steps: u32,
    pub sample_stride: u32,
    pub wet: CoupledScenarioV1,
    pub dry: ScenarioV1,
    pub construction: ConstructionIntentBatchV1,
    pub constructed_bodies: Vec<BodyStateV1>,
    pub monitored_pairs: Vec<[BodyId; 2]>,
}
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

pub fn fluid_solid_overlap_source(
    arrangement: FluidSolidOverlapArrangement,
    particle_phase_index: u32,
) -> Result<FluidSolidOverlapSourceV1, ScenarioBuildError> {
    let phase_x = *FLUID_SOLID_OVERLAP_PHASE_OFFSETS
        .get(particle_phase_index as usize)
        .ok_or_else(|| {
            ScenarioBuildError(format!(
                "fluid/solid overlap particle phase {particle_phase_index} is outside 0..{}",
                FLUID_SOLID_OVERLAP_PHASE_COUNT
            ))
        })?;
    let id = format!(
        "fluid-solid-overlap-{}-phase-{particle_phase_index}-v1",
        arrangement.id()
    );
    let plan = overlap_construction_plan(arrangement, phase_x);
    let requested_steps = FLUID_SOLID_OVERLAP_SETTLE_STEPS + 1 + FLUID_SOLID_OVERLAP_RECOVERY_STEPS;
    let mut wet = liquid_sandbox_scenario()?;
    wet.id = format!("{id}-wet");
    wet.seed = 0x5035_3500 + (arrangement as u64) * 16 + u64::from(particle_phase_index);
    wet.requested_steps = requested_steps;
    wet.rigid.id = format!("{id}-wet-rigid");
    wet.rigid.seed = wet.seed;
    wet.rigid.requested_steps = requested_steps;
    wet.rigid.bodies.retain(|body| body.id.slot < 3);
    wet.validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;

    let mut intents = Vec::with_capacity(plan.len());
    let mut constructed_bodies = Vec::with_capacity(plan.len());
    for (index, (shape, position)) in plan.into_iter().enumerate() {
        let sequence = index as u32 + 1;
        intents.push(match shape {
            OverlapConstructionShape::Circle => ConstructionIntentV1::PlaceCircle {
                sequence,
                world_target: position,
            },
            OverlapConstructionShape::Rectangle => ConstructionIntentV1::PlaceRectangle {
                sequence,
                world_target: position,
            },
        });
        constructed_bodies.push(construction_body(index as u32 + 3, shape, position));
    }
    let construction = ConstructionIntentBatchV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: CONSTRUCTION_INTENT_CONTRACT_VERSION,
        capacity_intents: crate::construction::CONSTRUCTION_INTENT_CAPACITY,
        attempted_intents: intents.len() as u64,
        overflowed: false,
        intents,
    };
    construction
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;

    let mut monitored_pairs = Vec::new();
    for left in 0..constructed_bodies.len() {
        for right in left + 1..constructed_bodies.len() {
            monitored_pairs.push([constructed_bodies[left].id, constructed_bodies[right].id]);
        }
    }
    let mut dry = wet.rigid.clone();
    dry.id = format!("{id}-dry-rigid");
    dry.seed = wet.seed ^ 0x4452_5900;
    dry.validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;

    let source = FluidSolidOverlapSourceV1 {
        version: FLUID_SOLID_OVERLAP_SOURCE_VERSION,
        id,
        arrangement,
        particle_phase_index,
        particle_phase_offset: Vec2::new(phase_x, 0.0),
        settle_steps: FLUID_SOLID_OVERLAP_SETTLE_STEPS,
        recovery_steps: FLUID_SOLID_OVERLAP_RECOVERY_STEPS,
        sample_stride: FLUID_SOLID_OVERLAP_SAMPLE_STRIDE,
        wet,
        dry,
        construction,
        constructed_bodies,
        monitored_pairs,
    };
    source
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(source)
}

pub fn fluid_solid_overlap_sources() -> Result<Vec<FluidSolidOverlapSourceV1>, ScenarioBuildError> {
    FluidSolidOverlapArrangement::ALL
        .into_iter()
        .flat_map(|arrangement| {
            (0..FLUID_SOLID_OVERLAP_PHASE_COUNT as u32)
                .map(move |phase| fluid_solid_overlap_source(arrangement, phase))
        })
        .collect()
}

impl Validate for FluidSolidOverlapSourceV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.version != FLUID_SOLID_OVERLAP_SOURCE_VERSION
            || self.id.is_empty()
            || self.particle_phase_index >= FLUID_SOLID_OVERLAP_PHASE_COUNT as u32
            || self.settle_steps != FLUID_SOLID_OVERLAP_SETTLE_STEPS
            || self.recovery_steps != FLUID_SOLID_OVERLAP_RECOVERY_STEPS
            || self.sample_stride == 0
            || !self.recovery_steps.is_multiple_of(self.sample_stride)
            || self.construction.intents.len() != self.constructed_bodies.len()
        {
            return Err(ContractError(
                "invalid fluid/solid overlap source identity or bounds".to_owned(),
            ));
        }
        self.particle_phase_offset.validate()?;
        self.wet.validate()?;
        self.dry.validate()?;
        self.construction.validate()?;
        for body in &self.constructed_bodies {
            body.validate()?;
        }
        let expected_pairs =
            self.constructed_bodies.len() * self.constructed_bodies.len().saturating_sub(1) / 2;
        if self.monitored_pairs.len() != expected_pairs
            || self
                .monitored_pairs
                .windows(2)
                .any(|pair| pair[0] >= pair[1])
            || self.monitored_pairs.iter().any(|pair| {
                pair[0] >= pair[1]
                    || pair.iter().any(|body| {
                        self.constructed_bodies
                            .binary_search_by_key(body, |candidate| candidate.id)
                            .is_err()
                    })
            })
        {
            return Err(ContractError(
                "fluid/solid overlap monitored pairs must be complete, sorted, and live".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy)]
enum OverlapConstructionShape {
    Circle,
    Rectangle,
}

fn overlap_construction_plan(
    arrangement: FluidSolidOverlapArrangement,
    phase_x: f32,
) -> Vec<(OverlapConstructionShape, Vec2)> {
    const Y: f32 = 2.0;
    match arrangement {
        FluidSolidOverlapArrangement::CirclePair => vec![
            (
                OverlapConstructionShape::Circle,
                Vec2::new(phase_x - 0.17, Y),
            ),
            (
                OverlapConstructionShape::Circle,
                Vec2::new(phase_x + 0.17, Y),
            ),
        ],
        FluidSolidOverlapArrangement::CircleThreeBody => vec![
            (
                OverlapConstructionShape::Circle,
                Vec2::new(phase_x - 0.17, Y - 0.10),
            ),
            (
                OverlapConstructionShape::Circle,
                Vec2::new(phase_x + 0.17, Y - 0.10),
            ),
            (
                OverlapConstructionShape::Circle,
                Vec2::new(phase_x, Y + 0.194),
            ),
        ],
        FluidSolidOverlapArrangement::RectanglePair => vec![
            (
                OverlapConstructionShape::Rectangle,
                Vec2::new(phase_x - 0.21, Y),
            ),
            (
                OverlapConstructionShape::Rectangle,
                Vec2::new(phase_x + 0.21, Y),
            ),
        ],
        FluidSolidOverlapArrangement::MixedPair => vec![
            (
                OverlapConstructionShape::Circle,
                Vec2::new(phase_x - 0.185, Y),
            ),
            (
                OverlapConstructionShape::Rectangle,
                Vec2::new(phase_x + 0.185, Y),
            ),
        ],
        FluidSolidOverlapArrangement::MixedThreeBody => vec![
            (OverlapConstructionShape::Rectangle, Vec2::new(phase_x, Y)),
            (
                OverlapConstructionShape::Circle,
                Vec2::new(phase_x - 0.34, Y),
            ),
            (
                OverlapConstructionShape::Circle,
                Vec2::new(phase_x + 0.34, Y),
            ),
        ],
    }
}

fn construction_body(slot: u32, shape: OverlapConstructionShape, position: Vec2) -> BodyStateV1 {
    let (shape, inverse_mass, inverse_inertia) = match shape {
        OverlapConstructionShape::Circle => (
            ShapeV1::Circle {
                radius: finite(crate::construction::CONSTRUCTION_CIRCLE_RADIUS),
            },
            crate::construction::CONSTRUCTION_CIRCLE_INVERSE_MASS,
            crate::construction::CONSTRUCTION_CIRCLE_INVERSE_INERTIA,
        ),
        OverlapConstructionShape::Rectangle => (
            ShapeV1::Rectangle {
                half_extents: Vec2::new(
                    crate::construction::CONSTRUCTION_RECTANGLE_HALF_EXTENTS[0],
                    crate::construction::CONSTRUCTION_RECTANGLE_HALF_EXTENTS[1],
                ),
            },
            crate::construction::CONSTRUCTION_RECTANGLE_INVERSE_MASS,
            crate::construction::CONSTRUCTION_RECTANGLE_INVERSE_INERTIA,
        ),
    };
    let mut body = body(slot, shape, position, inverse_mass, inverse_inertia, false);
    body.material.friction = finite(crate::construction::CONSTRUCTION_FRICTION);
    body.material.restitution = finite(crate::construction::CONSTRUCTION_RESTITUTION);
    body
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

pub fn liquid_scale_benchmark_source(
    solid_tier: LiquidScaleSolidTier,
    particle_count: u32,
) -> Result<LiquidScaleBenchmarkSourceV1, ScenarioBuildError> {
    let (columns, rows) = liquid_scale_particle_grid(particle_count)?;
    let spacing = 0.1;
    let width = columns as f32 * spacing;
    let height = rows as f32 * spacing;
    let lower = Vec2::new(-0.5 * width, -6.0);
    // Keep the intended final column despite accumulated f32 stepping error
    // in the shared particle-region decoder at the 256-column frontier.
    let upper_x_guard = if columns >= 256 {
        spacing * 1.0e-3
    } else {
        0.0
    };
    let upper = Vec2::new(lower.x + width + upper_x_guard, lower.y + height);
    let center_y = 0.5 * (lower.y + upper.y);
    let id = format!("liquid-scale-{}-{particle_count}-v1", solid_tier.id());
    let seed = 0x5035_4300
        + u64::from(particle_count)
        + u64::from(solid_tier.dynamic_solid_count()) * 0x1_0000;
    let requested_steps = 512;
    let mut bodies = vec![
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
    append_liquid_scale_dynamic_bodies(&mut bodies, solid_tier, lower, upper);
    bodies.sort_by_key(|body| body.id);
    let rigid = ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: format!("{id}-rigid"),
        seed,
        requested_steps,
        settings: default_settings(),
        bodies,
        manifolds: Vec::new(),
        contact_cache: Vec::new(),
        actions: Vec::new(),
    };
    let mut water = ParticleMaterialV1::water_v1(0);
    water.radius = finite(0.05);
    let scenario = CoupledScenarioV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: physbox_contracts::COUPLED_CONTRACT_VERSION,
        id: id.clone(),
        seed,
        requested_steps,
        particle_capacity: particle_count,
        rigid,
        settings: CoupledSettingsV1 {
            schedule: CoupledScheduleV1::RigidThenCoupledBatch,
            ..CoupledSettingsV1::initial()
        },
        materials: vec![water],
        regions: vec![ParticleRegionV1 {
            id: 0,
            material_id: 0,
            lower,
            upper,
            spacing: finite(spacing),
            initial_velocity: Vec2::ZERO,
        }],
        actions: Vec::new(),
    };
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    let scenario_hash =
        canonical_hash(&scenario).map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(LiquidScaleBenchmarkSourceV1 {
        version: LIQUID_SCALE_BENCHMARK_VERSION,
        id,
        scenario_hash,
        solid_tier,
        dynamic_solid_count: solid_tier.dynamic_solid_count(),
        fixed_solid_count: 3,
        particle_count,
        expected_body_capacity: solid_tier.expected_body_capacity(),
        scenario,
    })
}

fn liquid_scale_particle_grid(particle_count: u32) -> Result<(u32, u32), ScenarioBuildError> {
    match particle_count {
        1_024 => Ok((32, 32)),
        4_096 => Ok((64, 64)),
        8_192 => Ok((128, 64)),
        16_384 => Ok((128, 128)),
        32_768 => Ok((256, 128)),
        _ => Err(ScenarioBuildError(format!(
            "liquid scale benchmark particle count {particle_count} is outside 1024/4096/8192/16384/32768"
        ))),
    }
}

fn append_liquid_scale_dynamic_bodies(
    bodies: &mut Vec<BodyStateV1>,
    solid_tier: LiquidScaleSolidTier,
    lower: Vec2,
    upper: Vec2,
) {
    let dynamic_count = solid_tier.dynamic_solid_count();
    if dynamic_count == 0 {
        return;
    }
    let columns = if dynamic_count == 20 { 5 } else { 10 };
    let rows = dynamic_count / columns;
    let separation = if dynamic_count == 20 { 0.42 } else { 0.30 };
    let span_x = (columns - 1) as f32 * separation;
    let span_y = (rows - 1) as f32 * separation;
    let center_x = 0.5 * (lower.x + upper.x);
    let center_y = 0.5 * (lower.y + upper.y);
    for index in 0..dynamic_count {
        let column = index % columns;
        let row = index / columns;
        let position = Vec2::new(
            center_x - 0.5 * span_x + column as f32 * separation,
            center_y - 0.5 * span_y + row as f32 * separation,
        );
        let slot = index + 3;
        let body = if index % 2 == 0 {
            dynamic_circle(slot, position, 0.13, 1.0, false)
        } else {
            dynamic_rectangle(slot, position, Vec2::new(0.14, 0.10), 1.0, false)
        };
        bodies.push(body);
    }
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
    fn liquid_scale_sources_are_deterministic_exact_and_capacity_bracketed() {
        let mut ids = BTreeSet::new();
        let mut hashes = BTreeSet::new();
        for solid_tier in LiquidScaleSolidTier::ALL {
            for particle_count in LIQUID_SCALE_PARTICLE_COUNTS {
                let source = liquid_scale_benchmark_source(solid_tier, particle_count).unwrap();
                source.scenario.validate().unwrap();

                let repeated = liquid_scale_benchmark_source(solid_tier, particle_count).unwrap();
                assert_eq!(source.scenario_hash, repeated.scenario_hash);
                assert!(ids.insert(source.id.clone()));
                assert!(hashes.insert(source.scenario_hash.clone()));

                assert_eq!(source.version, LIQUID_SCALE_BENCHMARK_VERSION);
                assert_eq!(source.particle_count, particle_count);
                assert_eq!(source.scenario.particle_capacity, particle_count);
                assert_eq!(source.dynamic_solid_count, solid_tier.dynamic_solid_count());
                assert_eq!(source.fixed_solid_count, 3);
                assert_eq!(
                    source.scenario.rigid.bodies.len() as u32,
                    source.dynamic_solid_count + source.fixed_solid_count
                );
                assert_eq!(
                    source.expected_body_capacity,
                    solid_tier.expected_body_capacity()
                );

                let region = source.scenario.regions[0];
                let spacing = region.spacing.get();
                let columns = ((region.upper.x - region.lower.x) / spacing).round() as u32;
                let rows = ((region.upper.y - region.lower.y) / spacing).round() as u32;
                assert_eq!(columns * rows, particle_count);
                let radius = source.scenario.materials[0].radius.get();
                let mut decoded_count = 0;
                let mut y = region.lower.y + radius;
                while y <= region.upper.y - radius + 1.0e-6 {
                    let mut x = region.lower.x + radius;
                    while x <= region.upper.x - radius + 1.0e-6 {
                        decoded_count += 1;
                        x += spacing;
                    }
                    y += spacing;
                }
                assert_eq!(decoded_count, particle_count);
            }
        }

        assert_eq!(LiquidScaleSolidTier::Solids20.expected_body_capacity(), 64);
        assert_eq!(
            LiquidScaleSolidTier::Solids100.expected_body_capacity(),
            128
        );
    }

    #[test]
    fn liquid_scale_source_rejects_an_unsupported_particle_count() {
        assert!(liquid_scale_benchmark_source(LiquidScaleSolidTier::WaterOnly, 2_048).is_err());
    }

    #[test]
    fn fluid_solid_overlap_sources_are_complete_permissive_and_phase_distinct() {
        let sources = fluid_solid_overlap_sources().unwrap();
        assert_eq!(
            sources.len(),
            FluidSolidOverlapArrangement::ALL.len() * FLUID_SOLID_OVERLAP_PHASE_COUNT
        );
        let mut ids = BTreeSet::new();
        let mut hashes = BTreeSet::new();
        for source in &sources {
            source.validate().unwrap();
            assert!(ids.insert(source.id.clone()), "duplicate {}", source.id);
            assert!(
                hashes.insert(canonical_hash(source).unwrap()),
                "duplicate source projection {}",
                source.id
            );
            assert_eq!(source.wet.rigid.bodies.len(), 3);
            assert_eq!(source.dry.bodies.len(), 3);
            assert!(
                source.construction.intents.iter().all(|intent| matches!(
                    intent,
                    ConstructionIntentV1::PlaceCircle { .. }
                        | ConstructionIntentV1::PlaceRectangle { .. }
                )),
                "{} bypasses the permissive product placement path",
                source.id
            );
            for (index, body) in source.constructed_bodies.iter().enumerate() {
                assert!(
                    source.constructed_bodies[..index]
                        .iter()
                        .all(|placed| !axis_aligned_body_contains(placed, body.position)),
                    "{} would be rejected because a placement center lies inside an earlier body",
                    source.id
                );
            }
            let overlapping_pairs = source
                .monitored_pairs
                .iter()
                .filter(|pair| {
                    let left = source
                        .constructed_bodies
                        .iter()
                        .find(|body| body.id == pair[0])
                        .unwrap();
                    let right = source
                        .constructed_bodies
                        .iter()
                        .find(|body| body.id == pair[1])
                        .unwrap();
                    axis_aligned_separation(left, right) < 0.0
                })
                .count();
            assert_eq!(
                overlapping_pairs,
                match source.arrangement {
                    FluidSolidOverlapArrangement::CircleThreeBody => 3,
                    FluidSolidOverlapArrangement::MixedThreeBody => 2,
                    FluidSolidOverlapArrangement::CirclePair
                    | FluidSolidOverlapArrangement::RectanglePair
                    | FluidSolidOverlapArrangement::MixedPair => 1,
                },
                "{} does not author the intended initial overlap graph",
                source.id
            );
        }
    }

    fn axis_aligned_body_contains(body: &BodyStateV1, point: Vec2) -> bool {
        let delta = Vec2::new(point.x - body.position.x, point.y - body.position.y);
        match body.shape {
            ShapeV1::Circle { radius } => {
                delta.x * delta.x + delta.y * delta.y <= radius.get() * radius.get()
            }
            ShapeV1::Rectangle { half_extents } => {
                delta.x.abs() <= half_extents.x && delta.y.abs() <= half_extents.y
            }
        }
    }

    fn axis_aligned_separation(left: &BodyStateV1, right: &BodyStateV1) -> f32 {
        let delta_x = (right.position.x - left.position.x).abs();
        let delta_y = (right.position.y - left.position.y).abs();
        match (left.shape, right.shape) {
            (ShapeV1::Circle { radius: left }, ShapeV1::Circle { radius: right }) => {
                (delta_x * delta_x + delta_y * delta_y).sqrt() - left.get() - right.get()
            }
            (
                ShapeV1::Rectangle { half_extents: left },
                ShapeV1::Rectangle {
                    half_extents: right,
                },
            ) => (delta_x - left.x - right.x).max(delta_y - left.y - right.y),
            (ShapeV1::Circle { radius }, ShapeV1::Rectangle { half_extents })
            | (ShapeV1::Rectangle { half_extents }, ShapeV1::Circle { radius }) => {
                let outside_x = (delta_x - half_extents.x).max(0.0);
                let outside_y = (delta_y - half_extents.y).max(0.0);
                let inside = (delta_x - half_extents.x)
                    .max(delta_y - half_extents.y)
                    .min(0.0);
                (outside_x * outside_x + outside_y * outside_y).sqrt() + inside - radius.get()
            }
        }
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
