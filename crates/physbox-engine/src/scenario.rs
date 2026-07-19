use std::fmt::{Display, Formatter};

use physbox_contracts::{
    AuthoredContactPointV1, AuthoredManifoldV1, BodyCommandScenarioV1, BodyCommandV1, BodyId,
    BodyStateV1, CONTACT_SOFTNESS_POLICY_VERSION, ContactIdentityPolicyV1, ContactPolicyV1,
    ContactSoftnessPolicyV1, FeatureIdV1, FiniteF32, MaterialV1, SCHEMA_VERSION, ScenarioV1,
    ScheduledBodyCommandV1, ShapeV1, SolverSettingsV1, Validate, Vec2,
};

pub const P5_2_CENTERED_STACK_EVIDENCE_STEPS: u32 = 1_200;
pub const P5_2_BUCKET_EVIDENCE_STEPS: u32 = 600;
pub const P5_2_COLORING_CHURN_VERSION: u32 = 1;
pub const P5_2_MANIFOLD_COLOR_VALIDATION_STEPS: u32 = 8;
pub const P5_2_SELECTED_SOLVER_ITERATIONS: u32 = 2;
pub const P5_2_SETTLING_EVIDENCE_STEPS: u32 = 1_200;

#[derive(Clone, Debug, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct P5_2ColoringChurnSnapshotV1 {
    pub version: u32,
    pub label: String,
    pub constructed: Vec<BodyId>,
    pub erased: Vec<BodyId>,
    pub scenario: ScenarioV1,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioBuildError(pub String);

impl Display for ScenarioBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ScenarioBuildError {}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum AnalyticFixtureKind {
    CircleFloor,
    RectangleFloor,
    BoxOnBox,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum P5_2CollisionScenarioKind {
    CircleCircle,
    CircleRectangle,
    RectangleRectangle,
    MixedSmallWorld,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum P5_2GeneratedScenarioKind {
    RectangleFloor,
    CircleFloor,
    BilateralCircles,
    OffCenterRectangles,
    SmallStack,
    FrictionSlide,
    Restitution,
    PairChurn,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum P5_2BucketShape {
    Circle,
    Rectangle,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum P5_2SettlingFixtureKind {
    NearFlatFloorDrop,
    BoxOnRestingBoxDrop,
    SequentialRectangles3,
    SequentialRectangles4,
    SequentialRectangles5,
    SlideToRest,
    SmallMixedPile,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, serde::Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum P5_2SettlingReviewPolicy {
    HardControl,
    IdentityOnly,
    SoftnessOnly,
    SoftCandidate,
}

impl P5_2SettlingReviewPolicy {
    pub const fn name(self) -> &'static str {
        match self {
            Self::HardControl => "hard",
            Self::IdentityOnly => "identity-only",
            Self::SoftnessOnly => "softness-only",
            Self::SoftCandidate => "candidate",
        }
    }

    pub const fn identity_policy(self) -> ContactIdentityPolicyV1 {
        match self {
            Self::HardControl | Self::SoftnessOnly => {
                ContactIdentityPolicyV1::DirectMaximumSeparation
            }
            Self::IdentityOnly | Self::SoftCandidate => {
                ContactIdentityPolicyV1::PriorContactHysteresis
            }
        }
    }
}

impl P5_2SettlingFixtureKind {
    pub const ALL: [Self; 7] = [
        Self::NearFlatFloorDrop,
        Self::BoxOnRestingBoxDrop,
        Self::SequentialRectangles3,
        Self::SequentialRectangles4,
        Self::SequentialRectangles5,
        Self::SlideToRest,
        Self::SmallMixedPile,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::NearFlatFloorDrop => "near-flat-floor-drop",
            Self::BoxOnRestingBoxDrop => "box-on-resting-box-drop",
            Self::SequentialRectangles3 => "sequential-rectangles-3",
            Self::SequentialRectangles4 => "sequential-rectangles-4",
            Self::SequentialRectangles5 => "sequential-rectangles-5",
            Self::SlideToRest => "slide-to-rest",
            Self::SmallMixedPile => "small-mixed-pile",
        }
    }

    const fn sequential_count(self) -> Option<u32> {
        match self {
            Self::SequentialRectangles3 => Some(3),
            Self::SequentialRectangles4 => Some(4),
            Self::SequentialRectangles5 => Some(5),
            _ => None,
        }
    }
}

impl P5_2BucketShape {
    pub const ALL: [Self; 2] = [Self::Circle, Self::Rectangle];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Circle => "circle",
            Self::Rectangle => "rectangle",
        }
    }

    const fn seed(self) -> u64 {
        match self {
            Self::Circle => 5_100,
            Self::Rectangle => 5_200,
        }
    }
}

impl P5_2GeneratedScenarioKind {
    pub const ALL: [Self; 8] = [
        Self::RectangleFloor,
        Self::CircleFloor,
        Self::BilateralCircles,
        Self::OffCenterRectangles,
        Self::SmallStack,
        Self::FrictionSlide,
        Self::Restitution,
        Self::PairChurn,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::RectangleFloor => "p5.2-generated-rectangle-floor",
            Self::CircleFloor => "p5.2-generated-circle-floor",
            Self::BilateralCircles => "p5.2-generated-bilateral-circles",
            Self::OffCenterRectangles => "p5.2-generated-off-center-rectangles",
            Self::SmallStack => "p5.2-generated-small-stack",
            Self::FrictionSlide => "p5.2-generated-friction-slide",
            Self::Restitution => "p5.2-generated-restitution",
            Self::PairChurn => "p5.2-generated-pair-churn",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum P5_2CandidatePattern {
    Random,
    Touching,
    RotatedThin,
    Coincident,
    FixedOnly,
}

impl P5_2CandidatePattern {
    pub const ALL: [Self; 5] = [
        Self::Random,
        Self::Touching,
        Self::RotatedThin,
        Self::Coincident,
        Self::FixedOnly,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Random => "random",
            Self::Touching => "touching",
            Self::RotatedThin => "rotated-thin",
            Self::Coincident => "coincident",
            Self::FixedOnly => "fixed-only",
        }
    }
}

impl P5_2CollisionScenarioKind {
    pub const ALL: [Self; 4] = [
        Self::CircleCircle,
        Self::CircleRectangle,
        Self::RectangleRectangle,
        Self::MixedSmallWorld,
    ];

    pub const fn name(self) -> &'static str {
        match self {
            Self::CircleCircle => "p5.2-circle-circle",
            Self::CircleRectangle => "p5.2-circle-rectangle",
            Self::RectangleRectangle => "p5.2-rectangle-rectangle",
            Self::MixedSmallWorld => "p5.2-mixed-small-world",
        }
    }
}

pub fn p5_1_rectangle_floor_scenario(
    requested_steps: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if requested_steps == 0 {
        return Err(ScenarioBuildError(
            "P5.1 rectangle-floor scenario requires at least one requested step".to_owned(),
        ));
    }
    let dynamic_id = BodyId {
        slot: 0,
        generation: 1,
    };
    let floor_id = BodyId {
        slot: 1,
        generation: 1,
    };
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.0),
    };
    let dynamic_position = Vec2::new(0.0, 0.65);
    let floor_position = Vec2::new(0.0, -0.1);
    let dynamic_half_extents = Vec2::new(0.35, 0.2);
    let floor_half_extents = Vec2::new(8.0, 0.1);
    let contact_y = 0.0;
    let separation = dynamic_position.y - dynamic_half_extents.y - contact_y;
    Ok(ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: "p5.1-rectangle-floor".to_owned(),
        seed: 0,
        requested_steps,
        settings: default_settings(),
        bodies: vec![
            BodyStateV1 {
                id: dynamic_id,
                shape: ShapeV1::Rectangle {
                    half_extents: dynamic_half_extents,
                },
                position: dynamic_position,
                angle: finite(0.0),
                linear_velocity: Vec2::ZERO,
                angular_velocity: finite(0.0),
                inverse_mass: finite(1.0),
                inverse_inertia: finite(18.461_538),
                material,
                sleeping: false,
            },
            BodyStateV1 {
                id: floor_id,
                shape: ShapeV1::Rectangle {
                    half_extents: floor_half_extents,
                },
                position: floor_position,
                angle: finite(0.0),
                linear_velocity: Vec2::ZERO,
                angular_velocity: finite(0.0),
                inverse_mass: finite(0.0),
                inverse_inertia: finite(0.0),
                material,
                sleeping: false,
            },
        ],
        manifolds: vec![AuthoredManifoldV1 {
            id: 0,
            body_a: dynamic_id,
            body_b: floor_id,
            normal: Vec2::new(0.0, -1.0),
            material,
            points: [-dynamic_half_extents.x, dynamic_half_extents.x]
                .into_iter()
                .enumerate()
                .map(|(index, x)| AuthoredContactPointV1 {
                    local_anchor_a: Vec2::new(x, -dynamic_half_extents.y),
                    local_anchor_b: Vec2::new(x, floor_half_extents.y),
                    world_point: Vec2::new(x, contact_y),
                    separation: finite(separation),
                    feature_a: FeatureIdV1 {
                        feature_type: 1,
                        feature_index: 2,
                    },
                    feature_b: FeatureIdV1 {
                        feature_type: 1,
                        feature_index: index as u32,
                    },
                })
                .collect(),
        }],
        contact_cache: Vec::new(),
        actions: Vec::new(),
    })
}

pub fn p5_1_free_flight_scenario(
    body_count: u32,
    requested_steps: u32,
    seed: u64,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !(1..=100_000).contains(&body_count) {
        return Err(ScenarioBuildError(
            "P5.1 free-flight body count must be between 1 and 100000".to_owned(),
        ));
    }
    if requested_steps == 0 {
        return Err(ScenarioBuildError(
            "P5.1 free-flight scenario requires at least one requested step".to_owned(),
        ));
    }
    let mut random = StableRandom::new(seed);
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.0),
    };
    let columns = ((body_count as f32 * 1.6).sqrt().ceil() as u32).max(1);
    let rows = body_count.div_ceil(columns);
    let spacing_x = 3.0 / columns as f32;
    let spacing_y = 1.8 / rows as f32;
    let half_extents = Vec2::new((spacing_x * 0.32).min(0.16), (spacing_y * 0.32).min(0.105));
    let bodies = (0..body_count)
        .map(|slot| {
            let column = slot % columns;
            let row = slot / columns;
            BodyStateV1 {
                id: BodyId {
                    slot,
                    generation: 1,
                },
                shape: ShapeV1::Rectangle { half_extents },
                position: Vec2::new(
                    -1.5 + (column as f32 + 0.5) * spacing_x,
                    -0.9 + (row as f32 + 0.5) * spacing_y,
                ),
                angle: finite(random.signed(0.2)),
                linear_velocity: Vec2::new(random.signed(0.3), 0.2 + random.signed(0.05)),
                angular_velocity: finite(random.signed(0.6)),
                inverse_mass: finite(1.0),
                inverse_inertia: finite(1.0),
                material,
                sleeping: false,
            }
        })
        .collect();
    Ok(ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: format!("p5.1-free-flight-{body_count}"),
        seed,
        requested_steps,
        settings: default_settings(),
        bodies,
        manifolds: Vec::new(),
        contact_cache: Vec::new(),
        actions: Vec::new(),
    })
}

pub fn p5_1_reset_grab_scenario() -> Result<BodyCommandScenarioV1, ScenarioBuildError> {
    let scenario = p5_1_rectangle_floor_scenario(150)?;
    let body = scenario.bodies[0].id;
    let initial = scenario.bodies[0].clone();
    Ok(BodyCommandScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: "p5.1-reset-grab".to_owned(),
        scenario,
        scheduled_commands: vec![
            ScheduledBodyCommandV1 {
                step: 0,
                command: BodyCommandV1::Reset {
                    sequence: 1,
                    body,
                    position: initial.position,
                    angle: initial.angle,
                    linear_velocity: initial.linear_velocity,
                    angular_velocity: initial.angular_velocity,
                },
            },
            ScheduledBodyCommandV1 {
                step: 10,
                command: BodyCommandV1::GrabBegin {
                    sequence: 2,
                    body,
                    world_target: Vec2::new(0.0, 0.65),
                },
            },
            ScheduledBodyCommandV1 {
                step: 20,
                command: BodyCommandV1::GrabTarget {
                    sequence: 3,
                    body,
                    world_target: Vec2::new(0.45, 0.75),
                },
            },
            ScheduledBodyCommandV1 {
                step: 30,
                command: BodyCommandV1::GrabTarget {
                    sequence: 4,
                    body,
                    world_target: Vec2::new(-0.4, 0.55),
                },
            },
            ScheduledBodyCommandV1 {
                step: 40,
                command: BodyCommandV1::GrabEnd { sequence: 5, body },
            },
            ScheduledBodyCommandV1 {
                step: 80,
                command: BodyCommandV1::Reset {
                    sequence: 6,
                    body,
                    position: initial.position,
                    angle: initial.angle,
                    linear_velocity: initial.linear_velocity,
                    angular_velocity: initial.angular_velocity,
                },
            },
        ],
    })
}

pub fn p5_2_generated_floor_scenario(
    requested_steps: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    let mut scenario = p5_1_rectangle_floor_scenario(requested_steps)?;
    scenario.id = "p5.2-generated-rectangle-floor".to_owned();
    scenario.manifolds.clear();
    scenario.contact_cache.clear();
    scenario.settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    scenario.settings.contact_policy = Some(p5_2_contact_policy_candidate());
    Ok(scenario)
}

pub fn p5_2_centered_rectangle_stack_scenario(
    rectangle_count: u32,
    requested_steps: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !matches!(rectangle_count, 2 | 4 | 32 | 64) && !(8..=16).contains(&rectangle_count) {
        return Err(ScenarioBuildError(
            "centered rectangle stack requires 2, 4, 8 through 16, 32, or 64 rectangles".to_owned(),
        ));
    }
    let mut scenario = p5_2_generated_floor_scenario(requested_steps)?;
    let dynamic_template = scenario.bodies[0].clone();
    let mut floor = scenario.bodies[1].clone();
    floor.id.slot = 0;
    let ShapeV1::Rectangle { .. } = dynamic_template.shape else {
        unreachable!("generated floor dynamic template is a rectangle")
    };
    let half_extents = Vec2::new(0.35, 0.125);
    let mut bodies = vec![floor];
    for level in 0..rectangle_count {
        let mut body = dynamic_template.clone();
        body.id.slot = level + 1;
        body.shape = ShapeV1::Rectangle { half_extents };
        body.position = Vec2::new(0.0, half_extents.y + level as f32 * 2.0 * half_extents.y);
        body.angle = finite(0.0);
        body.linear_velocity = Vec2::ZERO;
        body.angular_velocity = finite(0.0);
        body.inverse_inertia = finite(21.719_458);
        body.sleeping = false;
        bodies.push(body);
    }
    scenario.id = format!("p5.2-centered-rectangle-stack-{rectangle_count}");
    scenario.seed = 0;
    scenario.bodies = bodies;
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

/// Permanent S10 centered four-body reproducer for the selected hard-contact
/// inter-substep stale-persistence reseed. The alias intentionally preserves
/// the governed centered-stack identity and exact 1,200-step source hash.
pub fn p5_2_stale_reseed_reproducer_scenario() -> Result<ScenarioV1, ScenarioBuildError> {
    p5_2_centered_rectangle_stack_scenario(4, P5_2_SETTLING_EVIDENCE_STEPS)
}

pub fn p5_2_settling_review_scenario(
    rectangle_count: u32,
    policy: P5_2SettlingReviewPolicy,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !(8..=16).contains(&rectangle_count) {
        return Err(ScenarioBuildError(
            "settling review requires between 8 and 16 rectangles".to_owned(),
        ));
    }
    let mut scenario = p5_2_centered_rectangle_stack_scenario(
        rectangle_count,
        P5_2_CENTERED_STACK_EVIDENCE_STEPS,
    )?;
    scenario.id = format!("p5.2-settling-review-{}-{rectangle_count}", policy.name());
    scenario.settings.contact_softness = match policy {
        P5_2SettlingReviewPolicy::HardControl | P5_2SettlingReviewPolicy::IdentityOnly => None,
        P5_2SettlingReviewPolicy::SoftnessOnly | P5_2SettlingReviewPolicy::SoftCandidate => {
            Some(ContactSoftnessPolicyV1 {
                version: CONTACT_SOFTNESS_POLICY_VERSION,
                contact_hertz: finite(30.0),
                damping_ratio: finite(10.0),
                static_contact_hertz_multiplier: finite(1.0),
            })
        }
    };
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

/// Shared soft-contact stack source for the frozen S6.5d 4/8/16 structural
/// matrix. The relaxation recurrence remains an execution policy and is not
/// authored into the scenario.
pub fn p5_2_settling_soft_stack_matrix_scenario(
    rectangle_count: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !matches!(rectangle_count, 4 | 8 | 16) {
        return Err(ScenarioBuildError(
            "settling soft-stack matrix requires 4, 8, or 16 rectangles".to_owned(),
        ));
    }
    let mut scenario = p5_2_centered_rectangle_stack_scenario(
        rectangle_count,
        P5_2_CENTERED_STACK_EVIDENCE_STEPS,
    )?;
    scenario.id = format!("p5.2-settling-soft-stack-matrix-{rectangle_count}");
    scenario.settings.contact_softness = Some(ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: finite(30.0),
        damping_ratio: finite(10.0),
        static_contact_hertz_multiplier: finite(1.0),
    });
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

/// Shared soft-contact depth source for the bounded S11.2 mechanism sweep.
/// The catalog deliberately keeps the accepted small controls and every
/// depth from eight through the governed 16-stack derivation gate.
pub fn p5_2_s11_soft_depth_sweep_scenario(
    rectangle_count: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !matches!(rectangle_count, 2 | 4 | 8..=16) {
        return Err(ScenarioBuildError(
            "S11 soft depth sweep requires 2, 4, or 8 through 16 rectangles".to_owned(),
        ));
    }
    let mut scenario = p5_2_centered_rectangle_stack_scenario(
        rectangle_count,
        P5_2_CENTERED_STACK_EVIDENCE_STEPS,
    )?;
    scenario.id = format!("p5.2-s11-soft-depth-sweep-{rectangle_count}");
    scenario.settings.contact_softness = Some(ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: finite(30.0),
        damping_ratio: finite(10.0),
        static_contact_hertz_multiplier: finite(1.0),
    });
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

/// Exact alias for the smallest S11 soft composition reproducer established
/// by the frozen depth sweep. Keeping the depth-source identity unchanged
/// makes later candidate comparisons byte-comparable with the minimization
/// evidence.
pub fn p5_2_s11_soft_composition_reproducer_scenario() -> Result<ScenarioV1, ScenarioBuildError> {
    p5_2_s11_soft_depth_sweep_scenario(11)
}

/// Returns the permanent four-dynamic-body rising-energy reproducer retained
/// by the S6.4 failure minimizer. The source is rebuilt from the shared seeded
/// fixture so native and Android diagnostics cannot fork the scenario.
pub fn p5_2_minimized_mixed_pile_energy_scenario() -> Result<ScenarioV1, ScenarioBuildError> {
    let mut scenario = p5_2_settling_fixture_scenario(
        P5_2SettlingFixtureKind::SmallMixedPile,
        0x6002,
        P5_2_SETTLING_EVIDENCE_STEPS,
    )?;
    scenario
        .bodies
        .retain(|body| matches!(body.id.slot, 0 | 1 | 2 | 4 | 7 | 9 | 10));
    for (slot, body) in scenario.bodies.iter_mut().enumerate() {
        body.id.slot = slot as u32;
    }
    scenario.bodies[3].angle = finite(0.0);
    scenario.bodies[3].angular_velocity = finite(0.0);
    scenario.bodies[4].position.x = 0.0;
    scenario.bodies[4].angular_velocity = finite(0.0);
    scenario.bodies[5].position.x = 0.0;
    scenario.bodies[6].position.x = 0.0;
    scenario.bodies[6].angle = finite(0.0);
    scenario.bodies[6].angular_velocity = finite(0.0);
    scenario.settings.contact_softness = Some(ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: finite(30.0),
        damping_ratio: finite(10.0),
        static_contact_hertz_multiplier: finite(1.0),
    });
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

/// Returns the permanent four-dynamic-body settling reproducer under the
/// selected hard/direct product policy. The body geometry is shared with the
/// S6 minimized soft-contact source, but no contact-softness policy is
/// authored into this S9 baseline lane.
pub fn p5_2_minimized_mixed_pile_hard_scenario() -> Result<ScenarioV1, ScenarioBuildError> {
    let mut scenario = p5_2_minimized_mixed_pile_energy_scenario()?;
    scenario.id = "p5.2-minimized-mixed-pile-hard".to_owned();
    scenario.settings.contact_softness = None;
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

/// Returns a reversible single-iteration comparator for diagnosing whether
/// repeating the selected soft-contact update amplifies its small-signal mode.
/// The accepted centered-stack source remains the two-iteration policy.
pub fn p5_2_settling_soft_i1_comparator_scenario(
    rectangle_count: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    let mut scenario =
        p5_2_settling_review_scenario(rectangle_count, P5_2SettlingReviewPolicy::SoftCandidate)?;
    scenario.id = format!("p5.2-settling-review-candidate-i1-{rectangle_count}");
    scenario.settings.iterations = 1;
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

/// Applies the same bounded diagnostic to the permanent four-body energy
/// reproducer without changing its authored bodies or physical coefficients.
pub fn p5_2_minimized_mixed_pile_soft_i1_scenario() -> Result<ScenarioV1, ScenarioBuildError> {
    let mut scenario = p5_2_minimized_mixed_pile_energy_scenario()?;
    scenario.id = "p5.2-minimized-mixed-pile-energy-i1".to_owned();
    scenario.settings.iterations = 1;
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

pub fn p5_2_coloring_churn_snapshots(
    requested_steps: u32,
) -> Result<Vec<P5_2ColoringChurnSnapshotV1>, ScenarioBuildError> {
    if requested_steps == 0 {
        return Err(ScenarioBuildError(
            "P5.2 coloring churn snapshots require at least one requested step".to_owned(),
        ));
    }
    let initial = p5_2_centered_rectangle_stack_scenario(8, requested_steps)?;
    let mut snapshots = Vec::with_capacity(5);
    let push = |snapshots: &mut Vec<P5_2ColoringChurnSnapshotV1>,
                label: &str,
                constructed: Vec<BodyId>,
                erased: Vec<BodyId>,
                mut scenario: ScenarioV1|
     -> Result<(), ScenarioBuildError> {
        scenario.id = format!("p5.2-coloring-churn-{label}");
        scenario.seed = 0x5035_4c00 + snapshots.len() as u64;
        scenario.bodies.sort_by_key(|body| body.id);
        scenario
            .validate()
            .map_err(|error| ScenarioBuildError(error.to_string()))?;
        snapshots.push(P5_2ColoringChurnSnapshotV1 {
            version: P5_2_COLORING_CHURN_VERSION,
            label: label.to_owned(),
            constructed,
            erased,
            scenario,
        });
        Ok(())
    };
    push(
        &mut snapshots,
        "initial-eight-stack",
        Vec::new(),
        Vec::new(),
        initial.clone(),
    )?;

    let mut constructed_circle = initial.clone();
    let mut circle = constructed_circle.bodies[1].clone();
    circle.id = BodyId {
        slot: 9,
        generation: 1,
    };
    circle.shape = ShapeV1::Circle {
        radius: finite(0.125),
    };
    circle.position = Vec2::new(0.475, 0.125);
    circle.inverse_inertia = finite(64.0);
    let circle_id = circle.id;
    constructed_circle.bodies.push(circle);
    push(
        &mut snapshots,
        "construct-circle",
        vec![circle_id],
        Vec::new(),
        constructed_circle.clone(),
    )?;

    let mut constructed_rectangle = constructed_circle.clone();
    let mut rectangle = constructed_rectangle.bodies[1].clone();
    rectangle.id = BodyId {
        slot: 10,
        generation: 1,
    };
    rectangle.shape = ShapeV1::Rectangle {
        half_extents: Vec2::new(0.15, 0.10),
    };
    rectangle.position = Vec2::new(-0.50, 0.10);
    rectangle.inverse_inertia = finite(92.307_69);
    let rectangle_id = rectangle.id;
    constructed_rectangle.bodies.push(rectangle);
    push(
        &mut snapshots,
        "construct-rectangle",
        vec![rectangle_id],
        Vec::new(),
        constructed_rectangle.clone(),
    )?;

    let erased_id = BodyId {
        slot: 10,
        generation: 1,
    };
    let mut erased = constructed_rectangle.clone();
    erased.bodies.retain(|body| body.id != erased_id);
    push(
        &mut snapshots,
        "erase-constructed-rectangle",
        Vec::new(),
        vec![erased_id],
        erased.clone(),
    )?;

    let mut reused = erased;
    let mut replacement = reused.bodies[1].clone();
    replacement.id = BodyId {
        slot: 10,
        generation: 2,
    };
    replacement.shape = ShapeV1::Circle {
        radius: finite(0.10),
    };
    replacement.position = Vec2::new(0.70, 0.10);
    replacement.inverse_inertia = finite(100.0);
    let replacement_id = replacement.id;
    reused.bodies.push(replacement);
    push(
        &mut snapshots,
        "reuse-erased-slot",
        vec![replacement_id],
        Vec::new(),
        reused,
    )?;
    Ok(snapshots)
}

pub fn p5_2_manifold_color_validation_scenario() -> Result<ScenarioV1, ScenarioBuildError> {
    let mut scenario =
        p5_2_centered_rectangle_stack_scenario(4, P5_2_MANIFOLD_COLOR_VALIDATION_STEPS)?;
    scenario.id = "p5.2-manifold-color-forward-i2-validation".to_owned();
    scenario.settings.iterations = 2;
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

pub fn p5_2_rectangle_pyramid_scenario(
    rows: u32,
    requested_steps: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !(2..=4).contains(&rows) {
        return Err(ScenarioBuildError(
            "small rectangle pyramid requires 2, 3, or 4 rows".to_owned(),
        ));
    }
    let mut scenario = p5_2_generated_floor_scenario(requested_steps)?;
    let dynamic_template = scenario.bodies[0].clone();
    let mut floor = scenario.bodies[1].clone();
    floor.id.slot = 0;
    let half_extents = Vec2::new(0.25, 0.1);
    let width = half_extents.x * 2.0;
    let height = half_extents.y * 2.0;
    let inverse_inertia = 12.0 / (width * width + height * height);
    let mut bodies = vec![floor];
    let mut slot = 1;
    for level in 0..rows {
        let count = rows - level;
        let start_x = -(count.saturating_sub(1) as f32) * half_extents.x;
        for column in 0..count {
            let mut body = dynamic_template.clone();
            body.id.slot = slot;
            body.shape = ShapeV1::Rectangle { half_extents };
            body.position = Vec2::new(
                start_x + column as f32 * width,
                half_extents.y + level as f32 * height,
            );
            body.angle = finite(0.0);
            body.linear_velocity = Vec2::ZERO;
            body.angular_velocity = finite(0.0);
            body.inverse_inertia = finite(inverse_inertia);
            body.sleeping = false;
            bodies.push(body);
            slot += 1;
        }
    }
    scenario.id = format!("p5.2-rectangle-pyramid-{rows}-rows");
    scenario.seed = 0;
    scenario.bodies = bodies;
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

pub fn p5_2_bucket_scenario(
    shape: P5_2BucketShape,
    body_count: u32,
    requested_steps: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if body_count != 100 {
        return Err(ScenarioBuildError(
            "P5.2 all-pairs bucket baseline requires exactly 100 dynamic bodies".to_owned(),
        ));
    }
    if requested_steps == 0 {
        return Err(ScenarioBuildError(
            "P5.2 bucket scenario requires at least one requested step".to_owned(),
        ));
    }
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.0),
    };
    let fixed_rectangle = |slot, position, half_extents| BodyStateV1 {
        id: BodyId {
            slot,
            generation: 1,
        },
        shape: ShapeV1::Rectangle { half_extents },
        position,
        angle: finite(0.0),
        linear_velocity: Vec2::ZERO,
        angular_velocity: finite(0.0),
        inverse_mass: finite(0.0),
        inverse_inertia: finite(0.0),
        material,
        sleeping: false,
    };
    let mut bodies = vec![
        fixed_rectangle(0, Vec2::new(0.0, -0.82), Vec2::new(1.06, 0.07)),
        fixed_rectangle(1, Vec2::new(-0.98, 0.075), Vec2::new(0.08, 0.825)),
        fixed_rectangle(2, Vec2::new(0.98, 0.075), Vec2::new(0.08, 0.825)),
    ];
    let mut random = StableRandom::new(shape.seed());
    for index in 0..body_count {
        let column = index % 10;
        let row = index / 10;
        let (body_shape, inverse_inertia, row_spacing) = match shape {
            P5_2BucketShape::Circle => (
                ShapeV1::Circle {
                    radius: finite(0.08),
                },
                312.5,
                0.175,
            ),
            P5_2BucketShape::Rectangle => (
                ShapeV1::Rectangle {
                    half_extents: Vec2::new(0.075, 0.055),
                },
                391.836_73,
                0.155,
            ),
        };
        bodies.push(BodyStateV1 {
            id: BodyId {
                slot: index + 3,
                generation: 1,
            },
            shape: body_shape,
            position: Vec2::new(
                -0.7875 + column as f32 * 0.175 + random.signed(0.003),
                -0.65 + row as f32 * row_spacing + random.signed(0.002),
            ),
            angle: finite(match shape {
                P5_2BucketShape::Circle => 0.0,
                P5_2BucketShape::Rectangle => random.signed(0.08),
            }),
            linear_velocity: Vec2::ZERO,
            angular_velocity: finite(0.0),
            inverse_mass: finite(1.0),
            inverse_inertia: finite(inverse_inertia),
            material,
            sleeping: false,
        });
    }
    let mut settings = default_settings();
    settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    settings.contact_policy = Some(p5_2_contact_policy_candidate());
    let scenario = ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: format!("p5.2-all-pairs-{}-bucket-100", shape.name()),
        seed: shape.seed(),
        requested_steps,
        settings,
        bodies,
        manifolds: Vec::new(),
        contact_cache: Vec::new(),
        actions: Vec::new(),
    };
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

pub fn p5_2_settling_fixture_scenario(
    kind: P5_2SettlingFixtureKind,
    seed: u64,
    requested_steps: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if requested_steps == 0 {
        return Err(ScenarioBuildError(
            "P5.2 settling fixtures require at least one requested step".to_owned(),
        ));
    }
    let base = p5_2_generated_floor_scenario(requested_steps)?;
    let template = base.bodies[0].clone();
    let mut floor = base.bodies[1].clone();
    floor.id = BodyId {
        slot: 0,
        generation: 1,
    };
    floor.position = Vec2::new(0.0, -0.1);
    floor.shape = ShapeV1::Rectangle {
        half_extents: Vec2::new(2.0, 0.1),
    };
    let material = template.material;
    let rectangle = |slot: u32,
                     position: Vec2,
                     angle: f32,
                     half_extents: Vec2,
                     linear_velocity: Vec2,
                     angular_velocity: f32| {
        let width = half_extents.x * 2.0;
        let height = half_extents.y * 2.0;
        BodyStateV1 {
            id: BodyId {
                slot,
                generation: 1,
            },
            shape: ShapeV1::Rectangle { half_extents },
            position,
            angle: finite(angle),
            linear_velocity,
            angular_velocity: finite(angular_velocity),
            inverse_mass: finite(1.0),
            inverse_inertia: finite(12.0 / (width * width + height * height)),
            material,
            sleeping: false,
        }
    };
    let circle = |slot: u32, position: Vec2, radius: f32| BodyStateV1 {
        id: BodyId {
            slot,
            generation: 1,
        },
        shape: ShapeV1::Circle {
            radius: finite(radius),
        },
        position,
        angle: finite(0.0),
        linear_velocity: Vec2::ZERO,
        angular_velocity: finite(0.0),
        inverse_mass: finite(1.0),
        inverse_inertia: finite(2.0 / (radius * radius)),
        material,
        sleeping: false,
    };
    let mut random = StableRandom::new(seed);
    let half_extents = Vec2::new(0.35, 0.125);
    let mut bodies = vec![floor];
    match kind {
        P5_2SettlingFixtureKind::NearFlatFloorDrop => {
            bodies.push(rectangle(
                1,
                Vec2::new(random.signed(0.12), 0.85 + random.signed(0.04)),
                random.signed(0.035),
                half_extents,
                Vec2::new(random.signed(0.05), 0.0),
                random.signed(0.15),
            ));
        }
        P5_2SettlingFixtureKind::BoxOnRestingBoxDrop => {
            bodies.push(rectangle(
                1,
                Vec2::new(0.0, half_extents.y),
                0.0,
                half_extents,
                Vec2::ZERO,
                0.0,
            ));
            bodies.push(rectangle(
                2,
                Vec2::new(random.signed(0.055), 0.95 + random.signed(0.05)),
                random.signed(0.03),
                half_extents,
                Vec2::new(random.signed(0.04), 0.0),
                random.signed(0.12),
            ));
        }
        P5_2SettlingFixtureKind::SequentialRectangles3
        | P5_2SettlingFixtureKind::SequentialRectangles4
        | P5_2SettlingFixtureKind::SequentialRectangles5 => {
            let count = kind
                .sequential_count()
                .expect("sequential settling fixture has a count");
            for level in 0..count {
                let y = if level == 0 {
                    half_extents.y
                } else {
                    0.55 + 0.42 * level as f32 + 0.16 * (level * level) as f32
                };
                bodies.push(rectangle(
                    level + 1,
                    Vec2::new(random.signed(0.035), y),
                    if level == 0 {
                        0.0
                    } else {
                        random.signed(0.025)
                    },
                    half_extents,
                    Vec2::ZERO,
                    if level == 0 { 0.0 } else { random.signed(0.08) },
                ));
            }
        }
        P5_2SettlingFixtureKind::SlideToRest => {
            bodies.push(rectangle(
                1,
                Vec2::new(-0.9, half_extents.y + 0.002),
                random.signed(0.01),
                half_extents,
                Vec2::new(2.0, 0.0),
                random.signed(0.05),
            ));
        }
        P5_2SettlingFixtureKind::SmallMixedPile => {
            let mut left = bodies[0].clone();
            left.id.slot = 1;
            left.position = Vec2::new(-1.05, 0.65);
            left.shape = ShapeV1::Rectangle {
                half_extents: Vec2::new(0.08, 0.75),
            };
            let mut right = left.clone();
            right.id.slot = 2;
            right.position.x = 1.05;
            bodies.extend([left, right]);
            for index in 0..8_u32 {
                let column = index % 3;
                let row = index / 3;
                let position = Vec2::new(
                    -0.45 + column as f32 * 0.45 + random.signed(0.04),
                    0.45 + row as f32 * 0.42 + random.signed(0.04),
                );
                if index.is_multiple_of(3) {
                    bodies.push(circle(index + 3, position, 0.15 + random.unit() * 0.03));
                } else {
                    bodies.push(rectangle(
                        index + 3,
                        position,
                        random.signed(0.12),
                        Vec2::new(0.18, 0.12),
                        Vec2::ZERO,
                        random.signed(0.08),
                    ));
                }
            }
        }
    }
    bodies.sort_by_key(|body| body.id);
    let mut scenario = base;
    scenario.id = format!("p5.2-settling-{}-seed-{seed}", kind.name());
    scenario.seed = seed;
    scenario.requested_steps = requested_steps;
    scenario.bodies = bodies;
    scenario.manifolds.clear();
    scenario.contact_cache.clear();
    scenario.actions.clear();
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

/// Shared soft-contact projection of one frozen S6.2 interaction fixture.
/// Identity and relaxation policies remain execution choices owned by the
/// validation runner.
pub fn p5_2_settling_soft_fixture_scenario(
    kind: P5_2SettlingFixtureKind,
    seed: u64,
) -> Result<ScenarioV1, ScenarioBuildError> {
    let mut scenario = p5_2_settling_fixture_scenario(kind, seed, P5_2_SETTLING_EVIDENCE_STEPS)?;
    scenario.id = format!("{}-soft-contact", scenario.id);
    scenario.settings.contact_softness = Some(ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: finite(30.0),
        damping_ratio: finite(10.0),
        static_contact_hertz_multiplier: finite(1.0),
    });
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
}

pub fn p5_2_random_settling_fixture_scenario(
    seed: u64,
    requested_steps: u32,
) -> Result<(P5_2SettlingFixtureKind, ScenarioV1), ScenarioBuildError> {
    let kind = P5_2SettlingFixtureKind::ALL[(seed as usize) % P5_2SettlingFixtureKind::ALL.len()];
    Ok((
        kind,
        p5_2_settling_fixture_scenario(kind, seed, requested_steps)?,
    ))
}

pub fn p5_2_generated_scenario(
    kind: P5_2GeneratedScenarioKind,
    requested_steps: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if requested_steps == 0 {
        return Err(ScenarioBuildError(
            "P5.2 generated scenario requires at least one step".to_owned(),
        ));
    }
    let mut scenario = match kind {
        P5_2GeneratedScenarioKind::RectangleFloor => {
            p5_2_generated_floor_scenario(requested_steps)?
        }
        P5_2GeneratedScenarioKind::CircleFloor => {
            analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, requested_steps)?
        }
        P5_2GeneratedScenarioKind::BilateralCircles => {
            p5_2_collision_scenario(P5_2CollisionScenarioKind::CircleCircle, requested_steps, 41)?
        }
        P5_2GeneratedScenarioKind::OffCenterRectangles => p5_2_collision_scenario(
            P5_2CollisionScenarioKind::RectangleRectangle,
            requested_steps,
            43,
        )?,
        P5_2GeneratedScenarioKind::SmallStack => {
            let base = p5_2_generated_floor_scenario(requested_steps)?;
            let mut floor = base.bodies[1].clone();
            floor.id.slot = 0;
            let mut bodies = vec![floor];
            for level in 0..4_u32 {
                let mut body = base.bodies[0].clone();
                body.id.slot = level + 1;
                body.shape = if level == 2 {
                    ShapeV1::Circle {
                        radius: finite(0.24),
                    }
                } else {
                    ShapeV1::Rectangle {
                        half_extents: Vec2::new(0.28, 0.20),
                    }
                };
                body.position = Vec2::new(
                    if level.is_multiple_of(2) {
                        -0.015
                    } else {
                        0.015
                    },
                    0.20 + level as f32 * 0.39,
                );
                body.angle = finite(if level == 1 { 0.03 } else { 0.0 });
                body.inverse_inertia = finite(4.0);
                bodies.push(body);
            }
            let mut scenario = base;
            scenario.bodies = bodies;
            scenario
        }
        P5_2GeneratedScenarioKind::FrictionSlide => {
            let mut scenario =
                analytic_fixture_scenario(AnalyticFixtureKind::RectangleFloor, requested_steps)?;
            scenario.bodies[0].position.y = 0.49;
            scenario.bodies[0].linear_velocity.x = 2.0;
            scenario
        }
        P5_2GeneratedScenarioKind::Restitution => {
            let mut scenario =
                analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, requested_steps)?;
            scenario.bodies[0].position.y = 0.49;
            scenario.bodies[0].linear_velocity.y = -3.0;
            scenario.bodies[0].material.restitution = finite(0.5);
            scenario.bodies[1].material.restitution = finite(0.5);
            scenario
        }
        P5_2GeneratedScenarioKind::PairChurn => {
            p5_2_collision_scenario(P5_2CollisionScenarioKind::CircleCircle, requested_steps, 47)?
        }
    };
    scenario.id = kind.name().to_owned();
    scenario.requested_steps = requested_steps;
    scenario.manifolds.clear();
    scenario.contact_cache.clear();
    scenario.actions.clear();
    scenario.settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    scenario.settings.contact_policy = Some(p5_2_contact_policy_candidate());
    match kind {
        P5_2GeneratedScenarioKind::BilateralCircles => {
            scenario.bodies[0].position.x = -0.34;
            scenario.bodies[1].position.x = 0.34;
            scenario.bodies[0].linear_velocity.x = 1.0;
            scenario.bodies[1].linear_velocity.x = -1.0;
            scenario.bodies[0].material.restitution = finite(0.0);
            scenario.bodies[1].material.restitution = finite(0.0);
        }
        P5_2GeneratedScenarioKind::OffCenterRectangles => {
            scenario.bodies[0].position = Vec2::new(-0.30, -0.10);
            scenario.bodies[1].position = Vec2::new(0.30, 0.14);
            scenario.bodies[0].linear_velocity = Vec2::new(1.0, 0.1);
            scenario.bodies[1].linear_velocity = Vec2::new(-1.0, -0.1);
            scenario.bodies[0].material.restitution = finite(0.0);
            scenario.bodies[1].material.restitution = finite(0.0);
        }
        P5_2GeneratedScenarioKind::PairChurn => {
            scenario.bodies[0].position.x = -0.42;
            scenario.bodies[1].position.x = 0.42;
            scenario.bodies[0].linear_velocity.x = 1.2;
            scenario.bodies[1].linear_velocity.x = -1.2;
            scenario.bodies[0].material.restitution = finite(0.8);
            scenario.bodies[1].material.restitution = finite(0.8);
        }
        _ => {}
    }
    Ok(scenario)
}

pub fn p5_2_generated_reset_grab_scenario() -> Result<BodyCommandScenarioV1, ScenarioBuildError> {
    let mut script = p5_1_reset_grab_scenario()?;
    script.id = "p5.2-generated-reset-grab".to_owned();
    script.scenario.id = "p5.2-generated-reset-grab-world".to_owned();
    script.scenario.manifolds.clear();
    script.scenario.contact_cache.clear();
    script.scenario.settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    script.scenario.settings.contact_policy = Some(p5_2_contact_policy_candidate());
    Ok(script)
}

pub fn p5_2_generated_performance_scenario(
    body_count: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !(2..=1024).contains(&body_count) || !body_count.is_multiple_of(2) {
        return Err(ScenarioBuildError(
            "P5.2 generated performance scenario requires an even 2-1024 body count".to_owned(),
        ));
    }
    let pair_count = body_count / 2;
    let columns = (pair_count as f32).sqrt().ceil() as u32;
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.0),
    };
    let mut bodies = Vec::with_capacity(body_count as usize);
    for pair in 0..pair_count {
        let center = Vec2::new((pair % columns) as f32 * 2.0, (pair / columns) as f32 * 2.0);
        for side in 0..2_u32 {
            let slot = pair * 2 + side;
            let shape = match (pair % 3, side) {
                (0, _) => ShapeV1::Circle {
                    radius: finite(0.25),
                },
                (1, 0) => ShapeV1::Rectangle {
                    half_extents: Vec2::new(0.25, 0.20),
                },
                (1, _) => ShapeV1::Circle {
                    radius: finite(0.25),
                },
                _ => ShapeV1::Rectangle {
                    half_extents: Vec2::new(0.25, 0.20),
                },
            };
            let direction = if side == 0 { -1.0 } else { 1.0 };
            bodies.push(BodyStateV1 {
                id: BodyId {
                    slot,
                    generation: 1,
                },
                shape,
                position: Vec2::new(center.x + direction * 0.245, center.y),
                angle: finite(if pair % 3 == 2 { direction * 0.04 } else { 0.0 }),
                linear_velocity: Vec2::new(-direction * 0.15, 0.0),
                angular_velocity: finite(0.0),
                inverse_mass: finite(1.0),
                inverse_inertia: finite(4.0),
                material,
                sleeping: false,
            });
        }
    }
    let mut settings = default_settings();
    settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    settings.gravity = Vec2::ZERO;
    settings.contact_policy = Some(p5_2_contact_policy_candidate());
    Ok(ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: format!("p5.2-generated-performance-{body_count}"),
        seed: 0x5035_3200 + u64::from(body_count),
        requested_steps: 1,
        settings,
        bodies,
        manifolds: Vec::new(),
        contact_cache: Vec::new(),
        actions: Vec::new(),
    })
}

pub(crate) fn p5_1_direct_render_scenario(
    requested_steps: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    let mut scenario = p5_1_rectangle_floor_scenario(requested_steps)?;
    scenario.id = "p5.1-direct-render".to_owned();
    scenario.manifolds.clear();
    Ok(scenario)
}

pub fn analytic_fixture_scenario(
    kind: AnalyticFixtureKind,
    requested_steps: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if requested_steps == 0 {
        return Err(ScenarioBuildError(
            "analytic fixture requires at least one requested step".to_owned(),
        ));
    }
    let dynamic_id = BodyId {
        slot: 0,
        generation: 1,
    };
    let fixed_id = BodyId {
        slot: 1,
        generation: 1,
    };
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.0),
    };
    let (id, dynamic_shape, dynamic_position, fixed_shape, fixed_position) = match kind {
        AnalyticFixtureKind::CircleFloor => (
            "analytic-circle-floor",
            ShapeV1::Circle {
                radius: finite(0.5),
            },
            Vec2::new(0.0, 1.5),
            ShapeV1::Rectangle {
                half_extents: Vec2::new(8.0, 0.25),
            },
            Vec2::new(0.0, -0.25),
        ),
        AnalyticFixtureKind::RectangleFloor => (
            "analytic-rectangle-floor",
            ShapeV1::Rectangle {
                half_extents: Vec2::new(0.5, 0.5),
            },
            Vec2::new(0.0, 1.5),
            ShapeV1::Rectangle {
                half_extents: Vec2::new(8.0, 0.25),
            },
            Vec2::new(0.0, -0.25),
        ),
        AnalyticFixtureKind::BoxOnBox => (
            "analytic-box-on-box",
            ShapeV1::Rectangle {
                half_extents: Vec2::new(0.5, 0.5),
            },
            Vec2::new(0.0, 0.99),
            ShapeV1::Rectangle {
                half_extents: Vec2::new(0.5, 0.5),
            },
            Vec2::ZERO,
        ),
    };
    Ok(ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: id.to_owned(),
        seed: 0,
        requested_steps,
        settings: default_settings(),
        bodies: vec![
            BodyStateV1 {
                id: dynamic_id,
                shape: dynamic_shape,
                position: dynamic_position,
                angle: finite(0.0),
                linear_velocity: Vec2::ZERO,
                angular_velocity: finite(0.0),
                inverse_mass: finite(1.0),
                inverse_inertia: finite(2.0),
                material,
                sleeping: false,
            },
            BodyStateV1 {
                id: fixed_id,
                shape: fixed_shape,
                position: fixed_position,
                angle: finite(0.0),
                linear_velocity: Vec2::ZERO,
                angular_velocity: finite(0.0),
                inverse_mass: finite(0.0),
                inverse_inertia: finite(0.0),
                material,
                sleeping: false,
            },
        ],
        manifolds: Vec::new(),
        contact_cache: Vec::new(),
        actions: Vec::new(),
    })
}

pub fn p5_2_collision_scenario(
    kind: P5_2CollisionScenarioKind,
    requested_steps: u32,
    seed: u64,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if requested_steps == 0 {
        return Err(ScenarioBuildError(
            "P5.2 collision scenario requires at least one requested step".to_owned(),
        ));
    }
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.1),
    };
    let body = |slot: u32,
                shape: ShapeV1,
                position: Vec2,
                angle: f32,
                inverse_mass: f32,
                inverse_inertia: f32| BodyStateV1 {
        id: BodyId {
            slot,
            generation: 1,
        },
        shape,
        position,
        angle: finite(angle),
        linear_velocity: Vec2::ZERO,
        angular_velocity: finite(0.0),
        inverse_mass: finite(inverse_mass),
        inverse_inertia: finite(inverse_inertia),
        material,
        sleeping: false,
    };
    let circle = |radius: f32| ShapeV1::Circle {
        radius: finite(radius),
    };
    let rectangle = |x: f32, y: f32| ShapeV1::Rectangle {
        half_extents: Vec2::new(x, y),
    };
    let bodies = match kind {
        P5_2CollisionScenarioKind::CircleCircle => vec![
            body(0, circle(0.35), Vec2::new(-0.25, 0.0), 0.0, 1.0, 4.0),
            body(1, circle(0.35), Vec2::new(0.25, 0.0), 0.0, 1.0, 4.0),
        ],
        P5_2CollisionScenarioKind::CircleRectangle => vec![
            body(0, circle(0.35), Vec2::new(0.45, 0.1), 0.0, 1.0, 4.0),
            body(1, rectangle(0.5, 0.3), Vec2::ZERO, 0.2, 1.0, 3.0),
        ],
        P5_2CollisionScenarioKind::RectangleRectangle => vec![
            body(
                0,
                rectangle(0.45, 0.3),
                Vec2::new(-0.2, 0.05),
                0.2,
                1.0,
                3.0,
            ),
            body(
                1,
                rectangle(0.45, 0.3),
                Vec2::new(0.35, 0.0),
                -0.15,
                1.0,
                3.0,
            ),
        ],
        P5_2CollisionScenarioKind::MixedSmallWorld => vec![
            body(0, circle(0.35), Vec2::new(-0.55, 0.30), 0.0, 1.0, 4.0),
            body(
                1,
                rectangle(0.45, 0.25),
                Vec2::new(0.0, 0.20),
                0.15,
                1.0,
                3.0,
            ),
            body(2, circle(0.30), Vec2::new(0.55, 0.30), 0.0, 1.0, 4.0),
            body(
                3,
                rectangle(2.0, 0.15),
                Vec2::new(0.0, -0.50),
                0.0,
                0.0,
                0.0,
            ),
            body(
                4,
                rectangle(0.25, 0.20),
                Vec2::new(1.50, 0.80),
                -0.25,
                1.0,
                5.0,
            ),
            body(5, circle(0.20), Vec2::new(-1.60, 0.90), 0.0, 0.0, 0.0),
        ],
    };
    let mut settings = default_settings();
    settings.gravity = Vec2::ZERO;
    Ok(ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: kind.name().to_owned(),
        seed,
        requested_steps,
        settings,
        bodies,
        manifolds: Vec::new(),
        contact_cache: Vec::new(),
        actions: Vec::new(),
    })
}

pub fn p5_2_candidate_scenario(
    pattern: P5_2CandidatePattern,
    body_count: u32,
    seed: u64,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !(2..=128).contains(&body_count) {
        return Err(ScenarioBuildError(
            "P5.2 candidate scenario requires between 2 and 128 bodies".to_owned(),
        ));
    }
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.1),
    };
    let mut random = StableRandom::new(seed);
    let mut bodies = Vec::with_capacity(body_count as usize);
    for slot in 0..body_count {
        let random_circle = slot % 3 == 0;
        let (shape, position, angle, fixed) = match pattern {
            P5_2CandidatePattern::Random => {
                let shape = if random_circle {
                    ShapeV1::Circle {
                        radius: finite(0.08 + random.unit() * 0.5),
                    }
                } else {
                    ShapeV1::Rectangle {
                        half_extents: Vec2::new(
                            0.06 + random.unit() * 0.55,
                            0.06 + random.unit() * 0.55,
                        ),
                    }
                };
                (
                    shape,
                    Vec2::new(random.signed(3.0), random.signed(3.0)),
                    random.signed(std::f32::consts::PI),
                    slot % 11 == 0,
                )
            }
            P5_2CandidatePattern::Touching => (
                ShapeV1::Rectangle {
                    half_extents: Vec2::new(0.5, 0.25),
                },
                Vec2::new(slot as f32, (slot % 2) as f32 * 0.5),
                0.0,
                slot % 7 == 0,
            ),
            P5_2CandidatePattern::RotatedThin => (
                ShapeV1::Rectangle {
                    half_extents: Vec2::new(0.8, 0.03),
                },
                Vec2::new(
                    (slot % 8) as f32 * 0.38 - 1.33,
                    (slot / 8) as f32 * 0.38 - 1.33,
                ),
                std::f32::consts::FRAC_PI_4 + random.signed(0.025),
                slot % 13 == 0,
            ),
            P5_2CandidatePattern::Coincident => {
                let shape = if random_circle {
                    ShapeV1::Circle {
                        radius: finite(0.2 + random.unit() * 0.15),
                    }
                } else {
                    ShapeV1::Rectangle {
                        half_extents: Vec2::new(0.25, 0.18),
                    }
                };
                (
                    shape,
                    Vec2::new(random.signed(0.02), random.signed(0.02)),
                    random.signed(0.05),
                    slot % 9 == 0,
                )
            }
            P5_2CandidatePattern::FixedOnly => (
                ShapeV1::Rectangle {
                    half_extents: Vec2::new(0.4, 0.4),
                },
                Vec2::new(random.signed(0.1), random.signed(0.1)),
                random.signed(0.1),
                true,
            ),
        };
        bodies.push(BodyStateV1 {
            id: BodyId {
                slot,
                generation: 1,
            },
            shape,
            position,
            angle: finite(angle),
            linear_velocity: Vec2::ZERO,
            angular_velocity: finite(0.0),
            inverse_mass: finite(if fixed { 0.0 } else { 1.0 }),
            inverse_inertia: finite(if fixed { 0.0 } else { 3.0 }),
            material,
            sleeping: false,
        });
    }
    let mut settings = default_settings();
    settings.iterations = P5_2_SELECTED_SOLVER_ITERATIONS;
    settings.gravity = Vec2::ZERO;
    Ok(ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: format!("p5.2-candidates-{}-{body_count}", pattern.name()),
        seed,
        requested_steps: 1,
        settings,
        bodies,
        manifolds: Vec::new(),
        contact_cache: Vec::new(),
        actions: Vec::new(),
    })
}

pub fn p5_2_narrowphase_pair_scenario(
    kind: P5_2CollisionScenarioKind,
    seed: u64,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if kind == P5_2CollisionScenarioKind::MixedSmallWorld {
        return Err(ScenarioBuildError(
            "randomized P5.2 narrowphase pairs require one analytic pair family".to_owned(),
        ));
    }
    let mut scenario = p5_2_collision_scenario(kind, 1, seed)?;
    let mut random = StableRandom::new(seed);
    let center = Vec2::new(random.signed(1.0), random.signed(1.0));
    match kind {
        P5_2CollisionScenarioKind::CircleCircle => {
            let radius_a = 0.1 + random.unit() * 0.4;
            let radius_b = 0.1 + random.unit() * 0.4;
            let direction_angle = random.signed(std::f32::consts::PI);
            let distance = (radius_a + radius_b) * (0.2 + random.unit() * 0.79);
            scenario.bodies[0].shape = ShapeV1::Circle {
                radius: finite(radius_a),
            };
            scenario.bodies[1].shape = ShapeV1::Circle {
                radius: finite(radius_b),
            };
            scenario.bodies[0].position = center;
            scenario.bodies[1].position = Vec2::new(
                center.x + direction_angle.cos() * distance,
                center.y + direction_angle.sin() * distance,
            );
        }
        P5_2CollisionScenarioKind::CircleRectangle => {
            let radius = 0.1 + random.unit() * 0.3;
            let half = Vec2::new(0.2 + random.unit() * 0.5, 0.2 + random.unit() * 0.5);
            let rectangle_angle = random.signed(std::f32::consts::PI);
            let local_circle = Vec2::new(
                half.x + radius * random.unit() * 0.8,
                random.signed(half.y * 0.8),
            );
            scenario.bodies[0].shape = ShapeV1::Circle {
                radius: finite(radius),
            };
            scenario.bodies[1].shape = ShapeV1::Rectangle { half_extents: half };
            scenario.bodies[1].position = center;
            scenario.bodies[1].angle = finite(rectangle_angle);
            scenario.bodies[0].position = Vec2::new(
                center.x + rectangle_angle.cos() * local_circle.x
                    - rectangle_angle.sin() * local_circle.y,
                center.y
                    + rectangle_angle.sin() * local_circle.x
                    + rectangle_angle.cos() * local_circle.y,
            );
        }
        P5_2CollisionScenarioKind::RectangleRectangle => {
            let half_a = Vec2::new(0.2 + random.unit() * 0.5, 0.2 + random.unit() * 0.5);
            let half_b = Vec2::new(0.2 + random.unit() * 0.5, 0.2 + random.unit() * 0.5);
            scenario.bodies[0].shape = ShapeV1::Rectangle {
                half_extents: half_a,
            };
            scenario.bodies[1].shape = ShapeV1::Rectangle {
                half_extents: half_b,
            };
            scenario.bodies[0].position = center;
            scenario.bodies[1].position = Vec2::new(
                center.x + random.signed(half_a.x.min(half_b.x)),
                center.y + random.signed(half_a.y.min(half_b.y)),
            );
            scenario.bodies[0].angle = finite(random.signed(std::f32::consts::PI));
            scenario.bodies[1].angle = finite(random.signed(std::f32::consts::PI));
        }
        P5_2CollisionScenarioKind::MixedSmallWorld => unreachable!(),
    }
    scenario.id = format!("{}-randomized-pair", kind.name());
    Ok(scenario)
}

pub fn p5_2_box2d_narrowphase_projection(
    source: &ScenarioV1,
) -> Result<ScenarioV1, ScenarioBuildError> {
    source
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    if source.bodies.len() < 2 {
        return Err(ScenarioBuildError(
            "P5.2 Box2D narrowphase projection requires at least two bodies".to_owned(),
        ));
    }
    if !source.manifolds.is_empty()
        || !source.contact_cache.is_empty()
        || !source.actions.is_empty()
    {
        return Err(ScenarioBuildError(
            "P5.2 Box2D narrowphase projection rejects authored manifolds, cache, and actions"
                .to_owned(),
        ));
    }
    let mut projection = source.clone();
    projection.id = format!("{}-box2d-narrowphase-v1", source.id);
    projection.requested_steps = 1;
    Ok(projection)
}

pub fn p5_2_box2d_world_projection(source: &ScenarioV1) -> Result<ScenarioV1, ScenarioBuildError> {
    source
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    if !source.manifolds.is_empty()
        || !source.contact_cache.is_empty()
        || !source.actions.is_empty()
    {
        return Err(ScenarioBuildError(
            "P5.2 Box2D world projection rejects authored manifolds, cache, and actions".to_owned(),
        ));
    }
    let mut projection = source.clone();
    projection.id = format!("{}-box2d-world-v1", source.id);
    projection
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(projection)
}

pub fn authored_one_contact_scenario(seed: u64) -> ScenarioV1 {
    let mut random = StableRandom::new(seed);
    let horizontal_offset = random.signed(0.18);
    let incoming_speed = 1.5 + random.unit() * 2.0;
    let dynamic_id = BodyId {
        slot: 0,
        generation: 1,
    };
    let fixed_id = BodyId {
        slot: 1,
        generation: 1,
    };
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.1),
    };
    ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: "authored-one-contact".to_owned(),
        seed,
        requested_steps: 1,
        settings: default_settings(),
        bodies: vec![
            BodyStateV1 {
                id: dynamic_id,
                shape: ShapeV1::Rectangle {
                    half_extents: Vec2::new(0.5, 0.25),
                },
                position: Vec2::new(horizontal_offset, 0.25),
                angle: finite(0.0),
                linear_velocity: Vec2::new(0.0, -incoming_speed),
                angular_velocity: finite(0.0),
                inverse_mass: finite(1.0),
                inverse_inertia: finite(3.2),
                material,
                sleeping: false,
            },
            BodyStateV1 {
                id: fixed_id,
                shape: ShapeV1::Rectangle {
                    half_extents: Vec2::new(8.0, 0.25),
                },
                position: Vec2::new(0.0, -0.25),
                angle: finite(0.0),
                linear_velocity: Vec2::ZERO,
                angular_velocity: finite(0.0),
                inverse_mass: finite(0.0),
                inverse_inertia: finite(0.0),
                material,
                sleeping: false,
            },
        ],
        manifolds: vec![AuthoredManifoldV1 {
            id: 0,
            body_a: dynamic_id,
            body_b: fixed_id,
            normal: Vec2::new(0.0, -1.0),
            material,
            points: vec![AuthoredContactPointV1 {
                local_anchor_a: Vec2::new(0.0, -0.25),
                local_anchor_b: Vec2::new(horizontal_offset, 0.25),
                world_point: Vec2::new(horizontal_offset, 0.0),
                separation: finite(0.0),
                feature_a: FeatureIdV1 {
                    feature_type: 1,
                    feature_index: 2,
                },
                feature_b: FeatureIdV1 {
                    feature_type: 1,
                    feature_index: 0,
                },
            }],
        }],
        contact_cache: Vec::new(),
        actions: Vec::new(),
    }
}

pub fn authored_chain_scenario(
    body_count: u32,
    seed: u64,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !(2..=100_000).contains(&body_count) {
        return Err(ScenarioBuildError(
            "authored chain body count must be between 2 and 100000".to_owned(),
        ));
    }
    let mut random = StableRandom::new(seed);
    let material = MaterialV1 {
        friction: finite(0.45),
        restitution: finite(0.0),
    };
    let mut bodies = Vec::with_capacity(body_count as usize);
    for slot in 0..body_count {
        bodies.push(BodyStateV1 {
            id: BodyId {
                slot,
                generation: 1,
            },
            shape: ShapeV1::Rectangle {
                half_extents: Vec2::new(0.45, 0.45),
            },
            position: Vec2::new(slot as f32 * 0.9, random.signed(0.001)),
            angle: finite(0.0),
            linear_velocity: Vec2::new(random.signed(0.05), random.signed(0.05)),
            angular_velocity: finite(random.signed(0.02)),
            inverse_mass: finite(0.5 + random.unit() * 1.5),
            inverse_inertia: finite(1.0 + random.unit() * 3.0),
            material,
            sleeping: false,
        });
    }
    let mut manifolds = Vec::with_capacity(body_count.saturating_sub(1) as usize);
    for slot in 0..body_count - 1 {
        manifolds.push(AuthoredManifoldV1 {
            id: slot,
            body_a: bodies[slot as usize].id,
            body_b: bodies[slot as usize + 1].id,
            normal: Vec2::new(1.0, 0.0),
            material,
            points: vec![AuthoredContactPointV1 {
                local_anchor_a: Vec2::new(0.45, 0.0),
                local_anchor_b: Vec2::new(-0.45, 0.0),
                world_point: Vec2::new(slot as f32 * 0.9 + 0.45, 0.0),
                separation: finite(0.0),
                feature_a: FeatureIdV1 {
                    feature_type: 1,
                    feature_index: 1,
                },
                feature_b: FeatureIdV1 {
                    feature_type: 1,
                    feature_index: 3,
                },
            }],
        });
    }
    Ok(ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: format!("authored-chain-{body_count}"),
        seed,
        requested_steps: 1,
        settings: default_settings(),
        bodies,
        manifolds,
        contact_cache: Vec::new(),
        actions: Vec::new(),
    })
}

pub fn p5_2_s13_authority_overflow_scenario() -> Result<ScenarioV1, ScenarioBuildError> {
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.0),
    };
    let mut bodies = Vec::with_capacity(22);
    for slot in 0..22 {
        bodies.push(s13_authored_body(
            slot,
            Vec2::new(slot as f32, 0.0),
            material,
        ));
    }
    let mut manifolds = Vec::with_capacity(21);
    for manifold_id in 0..21 {
        manifolds.push(AuthoredManifoldV1 {
            id: manifold_id,
            body_a: bodies[0].id,
            body_b: bodies[manifold_id as usize + 1].id,
            normal: Vec2::new(1.0, 0.0),
            material,
            points: vec![s13_authored_point(Vec2::new(0.0, 0.0))],
        });
    }
    s13_authored_scenario(
        "p5.2b-s13-authority-forced-overflow",
        0x5130,
        bodies,
        manifolds,
    )
}

pub fn p5_2_s13_authority_measurement_scenario(
    manifold_count: u32,
) -> Result<ScenarioV1, ScenarioBuildError> {
    if !matches!(manifold_count, 32 | 256 | 1_024) {
        return Err(ScenarioBuildError(
            "S13 authority measurement requires 32, 256, or 1024 manifolds".to_owned(),
        ));
    }
    let material = MaterialV1 {
        friction: finite(0.5),
        restitution: finite(0.0),
    };
    let body_count = manifold_count * 2;
    let mut bodies = Vec::with_capacity(body_count as usize);
    for slot in 0..body_count {
        let pair = slot / 2;
        let side = slot % 2;
        bodies.push(s13_authored_body(
            slot,
            Vec2::new(pair as f32 * 2.0 + side as f32, 0.0),
            material,
        ));
    }
    let mut manifolds = Vec::with_capacity(manifold_count as usize);
    for manifold_id in 0..manifold_count {
        manifolds.push(AuthoredManifoldV1 {
            id: manifold_id,
            body_a: bodies[(manifold_id * 2) as usize].id,
            body_b: bodies[(manifold_id * 2 + 1) as usize].id,
            normal: Vec2::new(1.0, 0.0),
            material,
            points: vec![s13_authored_point(Vec2::new(
                manifold_id as f32 * 2.0 + 0.5,
                0.0,
            ))],
        });
    }
    s13_authored_scenario(
        &format!("p5.2b-s13-authority-measurement-{manifold_count}"),
        0x5131 + u64::from(manifold_count),
        bodies,
        manifolds,
    )
}

fn s13_authored_body(slot: u32, position: Vec2, material: MaterialV1) -> BodyStateV1 {
    BodyStateV1 {
        id: BodyId {
            slot,
            generation: 1,
        },
        shape: ShapeV1::Rectangle {
            half_extents: Vec2::new(0.5, 0.5),
        },
        position,
        angle: finite(0.0),
        linear_velocity: Vec2::new(0.0, 0.0),
        angular_velocity: finite(0.0),
        inverse_mass: finite(1.0),
        inverse_inertia: finite(1.0),
        material,
        sleeping: false,
    }
}

fn s13_authored_point(world_point: Vec2) -> AuthoredContactPointV1 {
    AuthoredContactPointV1 {
        local_anchor_a: Vec2::new(0.5, 0.0),
        local_anchor_b: Vec2::new(-0.5, 0.0),
        world_point,
        separation: finite(0.0),
        feature_a: FeatureIdV1 {
            feature_type: 2,
            feature_index: 1,
        },
        feature_b: FeatureIdV1 {
            feature_type: 2,
            feature_index: 0,
        },
    }
}

fn s13_authored_scenario(
    id: &str,
    seed: u64,
    bodies: Vec<BodyStateV1>,
    manifolds: Vec<AuthoredManifoldV1>,
) -> Result<ScenarioV1, ScenarioBuildError> {
    let mut settings = default_settings();
    settings.iterations = 2;
    settings.contact_softness = Some(ContactSoftnessPolicyV1 {
        version: CONTACT_SOFTNESS_POLICY_VERSION,
        contact_hertz: finite(30.0),
        damping_ratio: finite(10.0),
        static_contact_hertz_multiplier: finite(1.0),
    });
    settings.contact_policy = Some(p5_2_contact_policy_candidate());
    let scenario = ScenarioV1 {
        schema_version: SCHEMA_VERSION,
        id: id.to_owned(),
        seed,
        requested_steps: 1,
        settings,
        bodies,
        manifolds,
        contact_cache: Vec::new(),
        actions: Vec::new(),
    };
    scenario
        .validate()
        .map_err(|error| ScenarioBuildError(error.to_string()))?;
    Ok(scenario)
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

pub fn p5_2_contact_policy_candidate() -> ContactPolicyV1 {
    ContactPolicyV1 {
        speculative_distance: finite(0.02),
        max_depenetration_velocity: finite(3.0),
    }
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("engine-owned scenario constants are finite")
}

struct StableRandom(u64);

impl StableRandom {
    fn new(seed: u64) -> Self {
        Self(if seed == 0 {
            0x9e37_79b9_7f4a_7c15
        } else {
            seed
        })
    }

    fn next(&mut self) -> u64 {
        let mut value = self.0;
        value ^= value >> 12;
        value ^= value << 25;
        value ^= value >> 27;
        self.0 = value;
        value.wrapping_mul(0x2545_f491_4f6c_dd1d)
    }

    fn unit(&mut self) -> f32 {
        let mantissa = (self.next() >> 40) as u32;
        mantissa as f32 / 0x00ff_ffff as f32
    }

    fn signed(&mut self, magnitude: f32) -> f32 {
        (self.unit() * 2.0 - 1.0) * magnitude
    }
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{NeutralActionV1, Validate, canonical_hash, canonical_json};

    use super::*;

    #[test]
    fn generators_replay_byte_for_byte() {
        assert_eq!(
            canonical_json(&authored_one_contact_scenario(42)).unwrap(),
            canonical_json(&authored_one_contact_scenario(42)).unwrap()
        );
        assert_eq!(
            canonical_json(&authored_chain_scenario(32, 99).unwrap()).unwrap(),
            canonical_json(&authored_chain_scenario(32, 99).unwrap()).unwrap()
        );
    }

    #[test]
    fn different_seeds_change_authored_state() {
        assert_ne!(
            canonical_json(&authored_one_contact_scenario(1)).unwrap(),
            canonical_json(&authored_one_contact_scenario(2)).unwrap()
        );
    }

    #[test]
    fn generated_contracts_are_valid_at_scale() {
        authored_one_contact_scenario(7).validate().unwrap();
        authored_chain_scenario(30_001, 7)
            .unwrap()
            .validate()
            .unwrap();
    }

    #[test]
    fn analytic_fixture_scenarios_are_shared_valid_and_replayable() {
        for kind in [
            AnalyticFixtureKind::CircleFloor,
            AnalyticFixtureKind::RectangleFloor,
            AnalyticFixtureKind::BoxOnBox,
        ] {
            let scenario = analytic_fixture_scenario(kind, 120).unwrap();
            scenario.validate().unwrap();
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(&analytic_fixture_scenario(kind, 120).unwrap()).unwrap()
            );
        }
        assert!(analytic_fixture_scenario(AnalyticFixtureKind::CircleFloor, 0).is_err());
    }

    #[test]
    fn p5_1_rectangle_floor_is_shared_valid_and_replayable() {
        let scenario = p5_1_rectangle_floor_scenario(120).unwrap();
        scenario.validate().unwrap();
        assert_eq!(scenario.bodies.len(), 2);
        assert_eq!(scenario.manifolds[0].points.len(), 2);
        assert_eq!(
            canonical_json(&scenario).unwrap(),
            canonical_json(&p5_1_rectangle_floor_scenario(120).unwrap()).unwrap()
        );
        assert!(p5_1_rectangle_floor_scenario(0).is_err());
    }

    #[test]
    fn p5_1_free_flight_is_shared_valid_and_bounded() {
        let scenario = p5_1_free_flight_scenario(100_000, 3, 17).unwrap();
        scenario.validate().unwrap();
        assert_eq!(scenario.bodies.len(), 100_000);
        assert_eq!(
            canonical_json(&p5_1_free_flight_scenario(32, 3, 17).unwrap()).unwrap(),
            canonical_json(&p5_1_free_flight_scenario(32, 3, 17).unwrap()).unwrap()
        );
        assert!(p5_1_free_flight_scenario(0, 3, 17).is_err());
        assert!(p5_1_free_flight_scenario(1, 0, 17).is_err());
    }

    #[test]
    fn p5_1_reset_grab_script_is_shared_valid_and_replayable() {
        let script = p5_1_reset_grab_scenario().unwrap();
        script.validate().unwrap();
        assert_eq!(script.scenario.requested_steps, 150);
        assert_eq!(script.scheduled_commands.len(), 6);
        assert_eq!(
            canonical_json(&script).unwrap(),
            canonical_json(&p5_1_reset_grab_scenario().unwrap()).unwrap()
        );
    }

    #[test]
    fn p5_1_direct_render_keeps_the_fixed_floor_as_a_body() {
        let scenario = p5_1_direct_render_scenario(3).unwrap();
        scenario.validate().unwrap();
        assert_eq!(scenario.bodies.len(), 2);
        assert_eq!(scenario.bodies[1].inverse_mass.get(), 0.0);
        assert!(scenario.manifolds.is_empty());
    }

    #[test]
    fn p5_2_collision_sources_and_neutral_projections_are_stable() {
        let expected = [
            (
                "f02a3a32e403b95991d1c366dd42e83121c8f1ca4cc4920a4d551ffde200cde2",
                "f767d3f644cebb77f962c3481eba3247f02005df8f33fe8a3c372c87bf2cac9c",
            ),
            (
                "5bb67226d9f4cf01137be258290c5f880f4884f04a6a3520cfb7e26fe10e6aac",
                "b557b253cfe0c5e1760c4b44800cc86f2c197565f8ae44811747e204331a7629",
            ),
            (
                "eaca28b3fc2b021561ef154041798f6e954dfd328cc4f71011d7aa3a792e0cb2",
                "f6a5e1e3aabe16125087122d467bea32e8b7c2e5636b617be6378a9622c50bfe",
            ),
            (
                "a3d5a6a239bc6b8331bc1178282a7c08e4f9b02e1d0e6e014566a30b9cfbcdfa",
                "1ccc20c8996680824efeb68794d780ee8b8894227b08d1444122c940f7b3c18d",
            ),
        ];
        for (kind, (source_hash, projection_hash)) in
            P5_2CollisionScenarioKind::ALL.into_iter().zip(expected)
        {
            let source = p5_2_collision_scenario(kind, 120, 17).unwrap();
            source.validate().unwrap();
            assert!(source.manifolds.is_empty());
            assert!(source.contact_cache.is_empty());
            assert!(source.actions.is_empty());
            let replay = p5_2_collision_scenario(kind, 120, 17).unwrap();
            assert_eq!(
                canonical_json(&source).unwrap(),
                canonical_json(&replay).unwrap()
            );

            let projection = p5_2_box2d_narrowphase_projection(&source).unwrap();
            projection.validate().unwrap();
            assert_eq!(projection.requested_steps, 1);
            assert_eq!(canonical_hash(&source).unwrap(), source_hash);
            assert_eq!(canonical_hash(&projection).unwrap(), projection_hash);
        }
    }

    #[test]
    fn p5_2_projection_rejects_unsupported_actions_instead_of_approximating() {
        let mut source =
            p5_2_collision_scenario(P5_2CollisionScenarioKind::CircleCircle, 2, 5).unwrap();
        source.actions.push(NeutralActionV1::SetGravity {
            step: 0,
            gravity: Vec2::new(0.0, -1.0),
        });
        let error = p5_2_box2d_narrowphase_projection(&source).unwrap_err();
        assert!(
            error
                .0
                .contains("rejects authored manifolds, cache, and actions")
        );
    }

    #[test]
    fn p5_2_box2d_world_projection_preserves_exact_step_stack_semantics() {
        let source = p5_2_centered_rectangle_stack_scenario(4, 600).unwrap();
        let projection = p5_2_box2d_world_projection(&source).unwrap();
        projection.validate().unwrap();
        assert_eq!(projection.requested_steps, 600);
        assert_eq!(projection.settings, source.settings);
        assert_eq!(projection.bodies, source.bodies);
        assert_eq!(projection.id, format!("{}-box2d-world-v1", source.id));
        assert_eq!(
            canonical_json(&projection).unwrap(),
            canonical_json(&p5_2_box2d_world_projection(&source).unwrap()).unwrap()
        );
    }

    #[test]
    fn p5_2_candidate_corpus_is_shared_valid_and_replayable() {
        for pattern in P5_2CandidatePattern::ALL {
            let scenario = p5_2_candidate_scenario(pattern, 32, 91).unwrap();
            scenario.validate().unwrap();
            assert_eq!(
                scenario.settings.iterations,
                P5_2_SELECTED_SOLVER_ITERATIONS
            );
            assert_eq!(scenario.bodies.len(), 32);
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(&p5_2_candidate_scenario(pattern, 32, 91).unwrap()).unwrap()
            );
        }
        assert!(p5_2_candidate_scenario(P5_2CandidatePattern::Random, 1, 0).is_err());
        assert!(p5_2_candidate_scenario(P5_2CandidatePattern::Random, 129, 0).is_err());
    }

    #[test]
    fn p5_2_randomized_narrowphase_pairs_are_valid_and_replayable() {
        for kind in P5_2CollisionScenarioKind::ALL[..3].iter().copied() {
            let scenario = p5_2_narrowphase_pair_scenario(kind, 77).unwrap();
            scenario.validate().unwrap();
            assert_eq!(scenario.bodies.len(), 2);
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(&p5_2_narrowphase_pair_scenario(kind, 77).unwrap()).unwrap()
            );
        }
        assert!(
            p5_2_narrowphase_pair_scenario(P5_2CollisionScenarioKind::MixedSmallWorld, 1).is_err()
        );
    }

    #[test]
    fn p5_2_generated_behavior_scenarios_are_neutral_and_replayable() {
        for kind in P5_2GeneratedScenarioKind::ALL {
            let scenario = p5_2_generated_scenario(kind, 120).unwrap();
            scenario.validate().unwrap();
            assert!(scenario.manifolds.is_empty());
            assert!(scenario.contact_cache.is_empty());
            assert!(scenario.actions.is_empty());
            assert!(scenario.bodies.len() >= 2);
            assert_eq!(scenario.id, kind.name());
            assert_eq!(
                scenario.settings.iterations,
                P5_2_SELECTED_SOLVER_ITERATIONS
            );
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(&p5_2_generated_scenario(kind, 120).unwrap()).unwrap()
            );
        }
        assert!(p5_2_generated_scenario(P5_2GeneratedScenarioKind::CircleFloor, 0).is_err());
        let script = p5_2_generated_reset_grab_scenario().unwrap();
        script.validate().unwrap();
        assert!(script.scenario.manifolds.is_empty());
        assert_eq!(
            script.scenario.settings.iterations,
            P5_2_SELECTED_SOLVER_ITERATIONS
        );
    }

    #[test]
    fn p5_2_centered_rectangle_stacks_are_valid_exact_and_replayable() {
        for count in [2, 4, 8, 16, 32, 64] {
            let scenario =
                p5_2_centered_rectangle_stack_scenario(count, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
                    .unwrap();
            scenario.validate().unwrap();
            assert_eq!(scenario.bodies.len(), count as usize + 1);
            assert_eq!(
                scenario.settings.iterations,
                P5_2_SELECTED_SOLVER_ITERATIONS
            );
            assert_eq!(scenario.bodies[0].inverse_mass.get(), 0.0);
            assert!(scenario.bodies.iter().all(|body| !body.sleeping));
            assert!(scenario.bodies[1..].iter().all(|body| {
                body.position.x == 0.0
                    && body.angle.get() == 0.0
                    && matches!(body.shape, ShapeV1::Rectangle { .. })
            }));
            assert!(scenario.manifolds.is_empty());
            assert!(scenario.contact_cache.is_empty());
            assert!(scenario.actions.is_empty());
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(
                    &p5_2_centered_rectangle_stack_scenario(
                        count,
                        P5_2_CENTERED_STACK_EVIDENCE_STEPS,
                    )
                    .unwrap(),
                )
                .unwrap()
            );
        }
        assert!(
            p5_2_centered_rectangle_stack_scenario(3, P5_2_CENTERED_STACK_EVIDENCE_STEPS).is_err()
        );
        assert!(p5_2_centered_rectangle_stack_scenario(4, 0).is_err());
        assert_eq!(
            canonical_json(&p5_2_stale_reseed_reproducer_scenario().unwrap()).unwrap(),
            canonical_json(
                &p5_2_centered_rectangle_stack_scenario(4, P5_2_SETTLING_EVIDENCE_STEPS).unwrap()
            )
            .unwrap()
        );
    }

    #[test]
    fn p5_2_settling_review_lanes_are_direct_exact_and_policy_separated() {
        for count in 8..=16 {
            let hard = p5_2_settling_review_scenario(count, P5_2SettlingReviewPolicy::HardControl)
                .unwrap();
            let identity =
                p5_2_settling_review_scenario(count, P5_2SettlingReviewPolicy::IdentityOnly)
                    .unwrap();
            let softness =
                p5_2_settling_review_scenario(count, P5_2SettlingReviewPolicy::SoftnessOnly)
                    .unwrap();
            let candidate =
                p5_2_settling_review_scenario(count, P5_2SettlingReviewPolicy::SoftCandidate)
                    .unwrap();
            hard.validate().unwrap();
            identity.validate().unwrap();
            softness.validate().unwrap();
            candidate.validate().unwrap();
            assert_eq!(hard.requested_steps, P5_2_CENTERED_STACK_EVIDENCE_STEPS);
            assert_eq!(candidate.requested_steps, hard.requested_steps);
            assert_eq!(identity.bodies, hard.bodies);
            assert_eq!(softness.bodies, hard.bodies);
            assert_eq!(candidate.bodies, hard.bodies);
            assert!(hard.settings.contact_softness.is_none());
            assert!(identity.settings.contact_softness.is_none());
            assert_eq!(
                softness.settings.contact_softness,
                candidate.settings.contact_softness
            );
            let soft_policy = candidate.settings.contact_softness.unwrap();
            assert_eq!(soft_policy.contact_hertz.get(), 30.0);
            assert_eq!(soft_policy.damping_ratio.get(), 10.0);
            assert_eq!(soft_policy.static_contact_hertz_multiplier.get(), 1.0);
            assert_eq!(
                P5_2SettlingReviewPolicy::HardControl.identity_policy(),
                ContactIdentityPolicyV1::DirectMaximumSeparation
            );
            assert_eq!(
                P5_2SettlingReviewPolicy::IdentityOnly.identity_policy(),
                ContactIdentityPolicyV1::PriorContactHysteresis
            );
            assert_eq!(
                P5_2SettlingReviewPolicy::SoftnessOnly.identity_policy(),
                ContactIdentityPolicyV1::DirectMaximumSeparation
            );
            assert_eq!(
                P5_2SettlingReviewPolicy::SoftCandidate.identity_policy(),
                ContactIdentityPolicyV1::PriorContactHysteresis
            );
            assert_ne!(
                canonical_json(&hard).unwrap(),
                canonical_json(&identity).unwrap()
            );
            assert_ne!(
                canonical_json(&hard).unwrap(),
                canonical_json(&softness).unwrap()
            );
            assert_ne!(
                canonical_json(&identity).unwrap(),
                canonical_json(&candidate).unwrap()
            );
            assert_ne!(
                canonical_json(&softness).unwrap(),
                canonical_json(&candidate).unwrap()
            );
            assert_ne!(
                canonical_json(&hard).unwrap(),
                canonical_json(&candidate).unwrap()
            );
            assert_eq!(
                canonical_json(&candidate).unwrap(),
                canonical_json(
                    &p5_2_settling_review_scenario(count, P5_2SettlingReviewPolicy::SoftCandidate,)
                        .unwrap(),
                )
                .unwrap()
            );
        }
        assert!(p5_2_settling_review_scenario(4, P5_2SettlingReviewPolicy::HardControl).is_err());
    }

    #[test]
    fn p5_2_soft_stack_matrix_is_shared_exact_and_frozen_to_three_depths() {
        for count in [4, 8, 16] {
            let source = p5_2_settling_soft_stack_matrix_scenario(count).unwrap();
            source.validate().unwrap();
            assert_eq!(source.bodies.len(), count as usize + 1);
            assert_eq!(source.requested_steps, P5_2_CENTERED_STACK_EVIDENCE_STEPS);
            let softness = source.settings.contact_softness.unwrap();
            assert_eq!(softness.contact_hertz.get(), 30.0);
            assert_eq!(softness.damping_ratio.get(), 10.0);
            assert_eq!(softness.static_contact_hertz_multiplier.get(), 1.0);
            assert_eq!(
                canonical_json(&source).unwrap(),
                canonical_json(&p5_2_settling_soft_stack_matrix_scenario(count).unwrap()).unwrap()
            );
        }
        assert!(p5_2_settling_soft_stack_matrix_scenario(2).is_err());
        assert!(p5_2_settling_soft_stack_matrix_scenario(32).is_err());
    }

    #[test]
    fn p5_2_s11_soft_depth_sweep_is_shared_exact_and_bounded() {
        for count in [2, 4, 8, 9, 10, 11, 12, 13, 14, 15, 16] {
            let source = p5_2_s11_soft_depth_sweep_scenario(count).unwrap();
            source.validate().unwrap();
            assert_eq!(source.bodies.len(), count as usize + 1);
            assert_eq!(source.requested_steps, P5_2_CENTERED_STACK_EVIDENCE_STEPS);
            assert_eq!(
                source
                    .settings
                    .contact_softness
                    .unwrap()
                    .contact_hertz
                    .get(),
                30.0
            );
            assert_eq!(
                canonical_json(&source).unwrap(),
                canonical_json(&p5_2_s11_soft_depth_sweep_scenario(count).unwrap()).unwrap()
            );
        }
        assert!(p5_2_s11_soft_depth_sweep_scenario(3).is_err());
        assert!(p5_2_s11_soft_depth_sweep_scenario(17).is_err());
        assert_eq!(
            canonical_json(&p5_2_s11_soft_composition_reproducer_scenario().unwrap()).unwrap(),
            canonical_json(&p5_2_s11_soft_depth_sweep_scenario(11).unwrap()).unwrap()
        );
    }

    #[test]
    fn p5_2_coloring_churn_snapshots_replay_construction_erase_and_generation_reuse() {
        let snapshots = p5_2_coloring_churn_snapshots(1).unwrap();
        assert_eq!(snapshots.len(), 5);
        assert!(snapshots.iter().all(|snapshot| {
            snapshot.version == P5_2_COLORING_CHURN_VERSION
                && snapshot.scenario.requested_steps == 1
                && snapshot.scenario.manifolds.is_empty()
                && snapshot.scenario.contact_cache.is_empty()
        }));
        assert_eq!(snapshots[1].constructed.len(), 1);
        assert_eq!(snapshots[2].constructed.len(), 1);
        assert_eq!(
            snapshots[3].erased,
            vec![BodyId {
                slot: 10,
                generation: 1
            }]
        );
        assert_eq!(
            snapshots[4].constructed,
            vec![BodyId {
                slot: 10,
                generation: 2
            }]
        );
        assert_eq!(
            serde_json::to_vec(&snapshots).unwrap(),
            serde_json::to_vec(&p5_2_coloring_churn_snapshots(1).unwrap()).unwrap()
        );
        assert!(p5_2_coloring_churn_snapshots(0).is_err());
    }

    #[test]
    fn p5_2_manifold_color_validation_is_the_bounded_forward_i2_source() {
        let scenario = p5_2_manifold_color_validation_scenario().unwrap();
        scenario.validate().unwrap();
        assert_eq!(
            scenario.requested_steps,
            P5_2_MANIFOLD_COLOR_VALIDATION_STEPS
        );
        assert_eq!(scenario.settings.iterations, 2);
        assert_eq!(scenario.bodies.len(), 5);
        assert!(scenario.manifolds.is_empty());
        assert!(scenario.contact_cache.is_empty());
        assert_eq!(
            serde_json::to_vec(&scenario).unwrap(),
            serde_json::to_vec(&p5_2_manifold_color_validation_scenario().unwrap()).unwrap()
        );
    }

    #[test]
    fn p5_2_small_rectangle_pyramids_are_shared_and_replayable() {
        for rows in 2..=4 {
            let scenario = p5_2_rectangle_pyramid_scenario(rows, 1_200).unwrap();
            scenario.validate().unwrap();
            assert_eq!(scenario.bodies.len(), 1 + (rows * (rows + 1) / 2) as usize);
            assert_eq!(scenario.bodies[0].inverse_mass.get(), 0.0);
            assert!(scenario.bodies[1..].iter().all(|body| {
                !body.sleeping
                    && body.inverse_mass.get() == 1.0
                    && matches!(body.shape, ShapeV1::Rectangle { .. })
            }));
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(&p5_2_rectangle_pyramid_scenario(rows, 1_200).unwrap()).unwrap()
            );
        }
        assert!(p5_2_rectangle_pyramid_scenario(1, 1_200).is_err());
        assert!(p5_2_rectangle_pyramid_scenario(5, 1_200).is_err());
        assert!(p5_2_rectangle_pyramid_scenario(3, 0).is_err());
    }

    #[test]
    fn p5_2_bucket_sources_use_ordinary_fixed_walls_and_replay_exactly() {
        for shape in P5_2BucketShape::ALL {
            let scenario = p5_2_bucket_scenario(shape, 100, P5_2_BUCKET_EVIDENCE_STEPS).unwrap();
            scenario.validate().unwrap();
            assert_eq!(scenario.bodies.len(), 103);
            assert_eq!(
                scenario.settings.iterations,
                P5_2_SELECTED_SOLVER_ITERATIONS
            );
            assert_eq!(scenario.seed, shape.seed());
            assert!(scenario.bodies[..3].iter().all(|body| {
                body.inverse_mass.get() == 0.0 && matches!(body.shape, ShapeV1::Rectangle { .. })
            }));
            assert!(
                scenario.bodies[3..]
                    .iter()
                    .all(|body| body.inverse_mass.get() == 1.0 && !body.sleeping)
            );
            assert!(scenario.manifolds.is_empty());
            assert!(scenario.contact_cache.is_empty());
            assert!(scenario.actions.is_empty());
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(
                    &p5_2_bucket_scenario(shape, 100, P5_2_BUCKET_EVIDENCE_STEPS).unwrap()
                )
                .unwrap()
            );
        }
        assert!(p5_2_bucket_scenario(P5_2BucketShape::Circle, 99, 1).is_err());
        assert!(p5_2_bucket_scenario(P5_2BucketShape::Circle, 100, 0).is_err());
    }

    #[test]
    fn p5_2_settling_fixtures_are_seeded_neutral_and_replayable() {
        for kind in P5_2SettlingFixtureKind::ALL {
            let scenario =
                p5_2_settling_fixture_scenario(kind, 0x6002, P5_2_SETTLING_EVIDENCE_STEPS).unwrap();
            scenario.validate().unwrap();
            assert!(scenario.manifolds.is_empty());
            assert!(scenario.contact_cache.is_empty());
            assert!(scenario.actions.is_empty());
            assert_eq!(scenario.bodies[0].inverse_mass.get(), 0.0);
            assert_eq!(scenario.requested_steps, P5_2_SETTLING_EVIDENCE_STEPS);
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(
                    &p5_2_settling_fixture_scenario(kind, 0x6002, P5_2_SETTLING_EVIDENCE_STEPS,)
                        .unwrap(),
                )
                .unwrap()
            );
            p5_2_box2d_world_projection(&scenario).unwrap();

            let soft = p5_2_settling_soft_fixture_scenario(kind, 0x6002).unwrap();
            soft.validate().unwrap();
            assert_eq!(soft.bodies, scenario.bodies);
            assert_eq!(soft.seed, scenario.seed);
            assert_eq!(soft.requested_steps, scenario.requested_steps);
            let softness = soft.settings.contact_softness.unwrap();
            assert_eq!(softness.contact_hertz.get(), 30.0);
            assert_eq!(softness.damping_ratio.get(), 10.0);
            assert_eq!(softness.static_contact_hertz_multiplier.get(), 1.0);
            assert_eq!(
                canonical_json(&soft).unwrap(),
                canonical_json(&p5_2_settling_soft_fixture_scenario(kind, 0x6002).unwrap())
                    .unwrap()
            );
        }
        let (kind, generated) = p5_2_random_settling_fixture_scenario(9, 120).unwrap();
        assert_eq!(kind, P5_2SettlingFixtureKind::SequentialRectangles3);
        assert_eq!(generated.seed, 9);
        assert!(
            p5_2_settling_fixture_scenario(P5_2SettlingFixtureKind::NearFlatFloorDrop, 1, 0)
                .is_err()
        );
    }

    #[test]
    fn p5_2_generated_performance_scenarios_are_sparse_and_replayable() {
        for body_count in [32, 256, 1024] {
            let scenario = p5_2_generated_performance_scenario(body_count).unwrap();
            scenario.validate().unwrap();
            assert_eq!(scenario.bodies.len(), body_count as usize);
            assert_eq!(
                scenario.settings.iterations,
                P5_2_SELECTED_SOLVER_ITERATIONS
            );
            assert!(scenario.manifolds.is_empty());
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(&p5_2_generated_performance_scenario(body_count).unwrap()).unwrap()
            );
        }
        assert!(p5_2_generated_performance_scenario(33).is_err());
    }
}
