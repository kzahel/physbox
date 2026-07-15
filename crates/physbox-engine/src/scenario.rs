use std::fmt::{Display, Formatter};

use physbox_contracts::{
    AuthoredContactPointV1, AuthoredManifoldV1, BodyCommandScenarioV1, BodyCommandV1, BodyId,
    BodyStateV1, FeatureIdV1, FiniteF32, MaterialV1, SCHEMA_VERSION, ScenarioV1,
    ScheduledBodyCommandV1, ShapeV1, SolverSettingsV1, Validate, Vec2,
};

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

fn default_settings() -> SolverSettingsV1 {
    SolverSettingsV1 {
        fixed_dt: finite(1.0 / 60.0),
        gravity: Vec2::new(0.0, -10.0),
        substeps: 4,
        iterations: 1,
        bias_rate: finite(0.2),
        mass_coefficient: finite(1.0),
        impulse_coefficient: finite(0.0),
        restitution_threshold: finite(1.0),
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
    fn p5_2_candidate_corpus_is_shared_valid_and_replayable() {
        for pattern in P5_2CandidatePattern::ALL {
            let scenario = p5_2_candidate_scenario(pattern, 32, 91).unwrap();
            scenario.validate().unwrap();
            assert_eq!(scenario.bodies.len(), 32);
            assert_eq!(
                canonical_json(&scenario).unwrap(),
                canonical_json(&p5_2_candidate_scenario(pattern, 32, 91).unwrap()).unwrap()
            );
        }
        assert!(p5_2_candidate_scenario(P5_2CandidatePattern::Random, 1, 0).is_err());
        assert!(p5_2_candidate_scenario(P5_2CandidatePattern::Random, 129, 0).is_err());
    }
}
