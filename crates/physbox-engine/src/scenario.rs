use std::fmt::{Display, Formatter};

use physbox_contracts::{
    AuthoredContactPointV1, AuthoredManifoldV1, BodyId, BodyStateV1, FeatureIdV1, FiniteF32,
    MaterialV1, SCHEMA_VERSION, ScenarioV1, ShapeV1, SolverSettingsV1, Vec2,
};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ScenarioBuildError(pub String);

impl Display for ScenarioBuildError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ScenarioBuildError {}

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
}
