use std::fmt::{Display, Formatter};

use physbox_contracts::{
    AuthoredContactPointV1, AuthoredManifoldV1, BodyCommandScenarioV1, BodyCommandV1, BodyId,
    BodyStateV1, FeatureIdV1, FiniteF32, MaterialV1, SCHEMA_VERSION, ScenarioV1,
    ScheduledBodyCommandV1, ShapeV1, SolverSettingsV1, Vec2,
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
    use physbox_contracts::{Validate, canonical_json};

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
}
