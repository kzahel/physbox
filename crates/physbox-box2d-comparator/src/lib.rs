use physbox_contracts::{BodyId, BodyStateV1, ScenarioV1, ShapeV1, Validate, Vec2};

pub const BOX2D_REVISION: &str = env!("PHYSBOX5_BOX2D_REVISION");
pub const SHAPE_CIRCLE: u32 = 1;
pub const SHAPE_RECTANGLE: u32 = 2;
pub const BODY_LIVE: u32 = 1 << 0;
pub const BODY_FIXED: u32 = 1 << 1;
pub const BODY_AWAKE: u32 = 1 << 2;

#[cfg(target_arch = "wasm32")]
pub fn install_web_runtime(runtime: wasm_bindgen::JsValue) -> Result<(), String> {
    platform::install(runtime)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldSettings {
    pub gravity: Vec2,
    pub restitution_threshold: f32,
    pub capacity: u32,
    pub sleeping_enabled: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct GrabTuning {
    pub spring_acceleration: f32,
    pub damping_rate: f32,
    pub max_acceleration: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodySnapshot {
    pub id: BodyId,
    pub shape: ShapeV1,
    pub position: Vec2,
    pub angle: f32,
    pub linear_velocity: Vec2,
    pub angular_velocity: f32,
    pub fixed: bool,
    pub awake: bool,
}

impl BodySnapshot {
    pub fn contains(self, point: Vec2) -> bool {
        let delta = Vec2::new(point.x - self.position.x, point.y - self.position.y);
        let (sine, cosine) = self.angle.sin_cos();
        let local = Vec2::new(
            cosine * delta.x + sine * delta.y,
            -sine * delta.x + cosine * delta.y,
        );
        match self.shape {
            ShapeV1::Circle { radius } => {
                local.x * local.x + local.y * local.y <= radius.get() * radius.get()
            }
            ShapeV1::Rectangle { half_extents } => {
                local.x.abs() <= half_extents.x && local.y.abs() <= half_extents.y
            }
        }
    }
}

pub struct Box2dWorld {
    platform: platform::World,
    capacity: u32,
}

impl Box2dWorld {
    pub fn new(
        scenario: &ScenarioV1,
        capacity: u32,
        sleeping_enabled: bool,
    ) -> Result<Self, String> {
        scenario.validate().map_err(|error| error.to_string())?;
        if capacity == 0 || scenario.bodies.len() > capacity as usize {
            return Err(format!(
                "Box2D comparator capacity {capacity} cannot hold {} bodies",
                scenario.bodies.len()
            ));
        }
        let settings = WorldSettings {
            gravity: scenario.settings.gravity,
            restitution_threshold: scenario.settings.restitution_threshold.get(),
            capacity,
            sleeping_enabled,
        };
        let mut platform = platform::World::new(settings)?;
        for body in &scenario.bodies {
            platform.add_body(body)?;
        }
        Ok(Self { platform, capacity })
    }

    pub fn add_body(&mut self, body: &BodyStateV1) -> Result<(), String> {
        body.validate().map_err(|error| error.to_string())?;
        if body.id.slot >= self.capacity {
            return Err(format!(
                "body slot {} exceeds Box2D comparator capacity {}",
                body.id.slot, self.capacity
            ));
        }
        self.platform.add_body(body)
    }

    pub fn remove_body(&mut self, body: BodyId) -> Result<(), String> {
        self.platform.remove_body(body)
    }

    pub fn apply_impulse(
        &mut self,
        body: BodyId,
        impulse: Vec2,
        world_point: Vec2,
    ) -> Result<(), String> {
        self.platform.apply_impulse(body, impulse, world_point)
    }

    pub fn begin_grab(
        &mut self,
        body: BodyId,
        world_anchor: Vec2,
        world_target: Vec2,
        tuning: GrabTuning,
    ) -> Result<(), String> {
        self.platform
            .begin_grab(body, world_anchor, world_target, tuning)
    }

    pub fn update_grab(&mut self, world_target: Vec2) -> Result<(), String> {
        self.platform.update_grab(world_target)
    }

    pub fn end_grab(&mut self) {
        self.platform.end_grab();
    }

    pub fn set_gravity(&mut self, gravity: Vec2) {
        self.platform.set_gravity(gravity);
    }

    pub fn step(&mut self, dt: f32, substeps: u32) -> Result<(), String> {
        if !dt.is_finite() || dt <= 0.0 || substeps == 0 || substeps > i32::MAX as u32 {
            return Err(
                "Box2D comparator step requires finite positive dt and substeps".to_owned(),
            );
        }
        self.platform.step(dt, substeps)
    }

    pub fn snapshots(&self) -> Result<Vec<BodySnapshot>, String> {
        self.platform.snapshots(self.capacity)
    }

    pub fn hit_test(&self, point: Vec2, dynamic_only: bool) -> Result<Option<BodyId>, String> {
        let mut best: Option<(f32, BodyId)> = None;
        for body in self.snapshots()? {
            if (dynamic_only && body.fixed) || !body.contains(point) {
                continue;
            }
            let dx = point.x - body.position.x;
            let dy = point.y - body.position.y;
            let distance = dx * dx + dy * dy;
            let replace = best.is_none_or(|(best_distance, best_id)| {
                distance < best_distance
                    || (distance == best_distance
                        && (body.id.slot, body.id.generation) < (best_id.slot, best_id.generation))
            });
            if replace {
                best = Some((distance, body.id));
            }
        }
        Ok(best.map(|(_, id)| id))
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }
}

#[cfg(not(target_arch = "wasm32"))]
mod platform;

#[cfg(target_arch = "wasm32")]
#[path = "web.rs"]
mod platform;

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use super::*;
    use physbox_contracts::Validate;
    use std::sync::{Mutex, OnceLock};

    fn box2d_test_guard() -> std::sync::MutexGuard<'static, ()> {
        static LOCK: OnceLock<Mutex<()>> = OnceLock::new();
        LOCK.get_or_init(|| Mutex::new(())).lock().unwrap()
    }

    fn scenario() -> ScenarioV1 {
        let finite = |value| physbox_contracts::FiniteF32::new(value).unwrap();
        let material = physbox_contracts::MaterialV1 {
            friction: finite(0.5),
            restitution: finite(0.0),
        };
        physbox_contracts::ScenarioV1 {
            schema_version: physbox_contracts::SCHEMA_VERSION,
            id: "comparator-runtime".to_owned(),
            seed: 0,
            requested_steps: 180,
            settings: physbox_contracts::SolverSettingsV1 {
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
            },
            bodies: vec![
                physbox_contracts::BodyStateV1 {
                    id: BodyId {
                        slot: 0,
                        generation: 1,
                    },
                    shape: ShapeV1::Rectangle {
                        half_extents: Vec2::new(2.0, 0.1),
                    },
                    position: Vec2::new(0.0, -0.1),
                    angle: physbox_contracts::FiniteF32::new(0.0).unwrap(),
                    linear_velocity: Vec2::ZERO,
                    angular_velocity: physbox_contracts::FiniteF32::new(0.0).unwrap(),
                    inverse_mass: physbox_contracts::FiniteF32::new(0.0).unwrap(),
                    inverse_inertia: physbox_contracts::FiniteF32::new(0.0).unwrap(),
                    material,
                    sleeping: false,
                },
                physbox_contracts::BodyStateV1 {
                    id: BodyId {
                        slot: 3,
                        generation: 4,
                    },
                    shape: ShapeV1::Circle {
                        radius: physbox_contracts::FiniteF32::new(0.1).unwrap(),
                    },
                    position: Vec2::new(0.0, 1.0),
                    angle: physbox_contracts::FiniteF32::new(0.0).unwrap(),
                    linear_velocity: Vec2::ZERO,
                    angular_velocity: physbox_contracts::FiniteF32::new(0.0).unwrap(),
                    inverse_mass: physbox_contracts::FiniteF32::new(1.0).unwrap(),
                    inverse_inertia: physbox_contracts::FiniteF32::new(200.0).unwrap(),
                    material,
                    sleeping: false,
                },
            ],
            manifolds: Vec::new(),
            contact_cache: Vec::new(),
            actions: Vec::new(),
        }
    }

    #[test]
    fn pinned_world_steps_and_exposes_authoritative_sleep() {
        let _guard = box2d_test_guard();
        let scenario = scenario();
        scenario.validate().unwrap();
        let mut world = Box2dWorld::new(&scenario, 8, true).unwrap();
        for _ in 0..180 {
            world.step(1.0 / 60.0, 4).unwrap();
        }
        let bodies = world.snapshots().unwrap();
        assert_eq!(bodies.len(), 2);
        let circle = bodies.iter().find(|body| body.id.slot == 3).unwrap();
        assert!((circle.position.y - 0.1).abs() < 0.002);
        assert!(!circle.awake);
        assert_eq!(
            world.hit_test(Vec2::new(0.0, 0.1), true).unwrap(),
            Some(circle.id)
        );
        assert_eq!(BOX2D_REVISION.len(), 40);
    }

    #[test]
    fn stable_identity_survives_remove_and_recreate() {
        let _guard = box2d_test_guard();
        let scenario = scenario();
        let mut world = Box2dWorld::new(&scenario, 8, true).unwrap();
        let old = BodyId {
            slot: 3,
            generation: 4,
        };
        world.remove_body(old).unwrap();
        let mut replacement = scenario.bodies[1].clone();
        replacement.id.generation = 5;
        replacement.position = Vec2::new(0.5, 0.5);
        world.add_body(&replacement).unwrap();
        let bodies = world.snapshots().unwrap();
        assert!(bodies.iter().any(|body| body.id == replacement.id));
        assert!(!bodies.iter().any(|body| body.id == old));
    }
}
