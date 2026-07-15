use std::{ffi::c_void, ptr, time::Instant};

use physbox_contracts::{
    NamedValueV1, ScenarioV1, ShapeV1, Validate, canonical_hash, sha256_bytes,
};
use serde::{Deserialize, Serialize};

#[allow(
    dead_code,
    non_camel_case_types,
    non_snake_case,
    non_upper_case_globals,
    unsafe_op_in_unsafe_fn
)]
mod ffi {
    include!(concat!(env!("OUT_DIR"), "/box2d_benchmark_bindings.rs"));
}

pub const BOX2D_REVISION: &str = env!("PHYSBOX5_BOX2D_REVISION");
pub const SUCCESS_SENTINEL: &str = "PHYSBOX5_BOX2D_BENCHMARK_SUCCESS";

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Box2dBenchmarkResult {
    pub schema_version: u32,
    pub run_id: String,
    pub source_scenario_hash: String,
    pub projection_hash: String,
    pub scope: String,
    pub source_revision: String,
    pub source_dirty: bool,
    pub host: String,
    pub toolchain: String,
    pub worker_count: u32,
    pub warmup_steps: u32,
    pub samples_seconds: Vec<f64>,
    pub contact_count_min: u32,
    pub contact_count_max: u32,
    pub task_count_max: u32,
    pub box2d_bytes_max: u64,
    pub final_body_hash: String,
    pub build_options: Vec<NamedValueV1>,
    pub runtime_settings: Vec<NamedValueV1>,
}

pub fn benchmark_scenario(
    scenario: &ScenarioV1,
    run_id: &str,
    projection_hash: String,
    worker_count: u32,
    warmup_steps: u32,
    samples: u32,
) -> Result<Box2dBenchmarkResult, String> {
    scenario.validate().map_err(|error| error.to_string())?;
    if run_id.is_empty()
        || projection_hash.len() != 64
        || !(1..=32).contains(&worker_count)
        || warmup_steps == 0
        || samples == 0
    {
        return Err("Box2D benchmark requires 1-32 workers and non-zero warmup/samples".to_owned());
    }
    if !scenario.actions.is_empty() {
        return Err("optimized Box2D benchmark does not yet support neutral actions".to_owned());
    }
    if scenario
        .bodies
        .iter()
        .any(|body| body.inverse_mass.get() > 0.0 && body.inverse_inertia.get() == 0.0)
    {
        return Err("optimized Box2D benchmark rejects locked dynamic rotation".to_owned());
    }

    for _ in 0..warmup_steps {
        let world = WorldInstance::new(scenario, worker_count)?;
        world.step(scenario);
    }

    let mut samples_seconds = Vec::with_capacity(samples as usize);
    let mut contact_count_min = u32::MAX;
    let mut contact_count_max = 0_u32;
    let mut task_count_max = 0_u32;
    let mut box2d_bytes_max = 0_u64;
    let mut final_body_hash = String::new();
    for sample in 0..samples {
        let world = WorldInstance::new(scenario, worker_count)?;
        let started = Instant::now();
        world.step(scenario);
        samples_seconds.push(started.elapsed().as_secs_f64());
        let counters = world.counters();
        contact_count_min = contact_count_min.min(counters.contactCount.max(0) as u32);
        contact_count_max = contact_count_max.max(counters.contactCount.max(0) as u32);
        task_count_max = task_count_max.max(counters.taskCount.max(0) as u32);
        box2d_bytes_max = box2d_bytes_max.max(counters.byteCount.max(0) as u64);
        if sample + 1 == samples {
            final_body_hash = world.body_hash(scenario)?;
        }
    }
    if !scenario.manifolds.is_empty() && contact_count_max == 0 {
        return Err(
            "Box2D projection produced zero contacts for an authored-contact scene".to_owned(),
        );
    }
    Ok(Box2dBenchmarkResult {
        schema_version: physbox_contracts::SCHEMA_VERSION,
        run_id: run_id.to_owned(),
        source_scenario_hash: canonical_hash(scenario).map_err(|error| error.to_string())?,
        projection_hash,
        scope: "end-to-end-physics-world-step".to_owned(),
        source_revision: BOX2D_REVISION.to_owned(),
        source_dirty: false,
        host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
        toolchain: format!(
            "{}; {}; {}",
            env!("PHYSBOX5_RUSTC_VERSION"),
            env!("PHYSBOX5_CMAKE_VERSION"),
            env!("PHYSBOX5_CXX_VERSION")
        ),
        worker_count,
        warmup_steps,
        samples_seconds,
        contact_count_min,
        contact_count_max,
        task_count_max,
        box2d_bytes_max,
        final_body_hash,
        build_options: vec![
            named("box2d_disable_simd", "off"),
            named("box2d_validate", "off"),
            named("build_shared_libs", "off"),
            named("build_type", "release"),
            named("task_scheduler", "physbox-fixed-worker-pool"),
        ],
        runtime_settings: vec![
            named(
                "candidate_max_depenetration_velocity",
                &scenario.settings.contact_policy.map_or_else(
                    || "disabled".to_owned(),
                    |policy| policy.max_depenetration_velocity.get().to_string(),
                ),
            ),
            named(
                "candidate_speculative_distance",
                &scenario.settings.contact_policy.map_or_else(
                    || "disabled".to_owned(),
                    |policy| policy.speculative_distance.get().to_string(),
                ),
            ),
            named("contact_speed", "3 (Box2D default)"),
            named("fixed_dt", &scenario.settings.fixed_dt.get().to_string()),
            named("substeps", &scenario.settings.substeps.to_string()),
            named("worker_count", &worker_count.to_string()),
        ],
    })
}

struct WorldInstance {
    world: WorldGuard,
    bodies: Vec<ffi::b2BodyId>,
    scheduler: SchedulerGuard,
}

impl WorldInstance {
    fn new(scenario: &ScenarioV1, worker_count: u32) -> Result<Self, String> {
        // SAFETY: the scheduler and world are uniquely owned by this instance. Every Box2D
        // definition originates from its public initializer and all pointers remain live.
        unsafe {
            let scheduler = SchedulerGuard::new(worker_count)?;
            let mut world_def = ffi::b2DefaultWorldDef();
            world_def.gravity = raw_vec(scenario.settings.gravity);
            world_def.restitutionThreshold = scenario.settings.restitution_threshold.get();
            world_def.workerCount = worker_count as i32;
            if worker_count > 1 {
                world_def.enqueueTask = Some(ffi::physbox_enqueue_task);
                world_def.finishTask = Some(ffi::physbox_finish_task);
                world_def.userTaskContext = scheduler.pointer.cast::<c_void>();
            } else {
                world_def.enqueueTask = None;
                world_def.finishTask = None;
                world_def.userTaskContext = ptr::null_mut();
            }
            world_def.enableSleep = true;
            world_def.enableContinuous = true;
            world_def.enableContactSoftening = false;
            let world = WorldGuard(ffi::b2CreateWorld(&raw const world_def));
            let mut bodies = Vec::with_capacity(scenario.bodies.len());
            for body in &scenario.bodies {
                let mut body_def = ffi::b2DefaultBodyDef();
                body_def.type_ = if body.inverse_mass.get() == 0.0 {
                    ffi::b2BodyType_b2_staticBody
                } else {
                    ffi::b2BodyType_b2_dynamicBody
                };
                body_def.position = raw_vec(body.position);
                let (sine, cosine) = body.angle.get().sin_cos();
                body_def.rotation = ffi::b2Rot { c: cosine, s: sine };
                body_def.linearVelocity = raw_vec(body.linear_velocity);
                body_def.angularVelocity = body.angular_velocity.get();
                body_def.isAwake = !body.sleeping;
                body_def.enableSleep = true;
                let raw_body = ffi::b2CreateBody(world.0, &raw const body_def);
                let mut shape_def = ffi::b2DefaultShapeDef();
                shape_def.material.friction = body.material.friction.get();
                shape_def.material.restitution = body.material.restitution.get();
                shape_def.density = 1.0;
                match body.shape {
                    ShapeV1::Circle { radius } => {
                        let circle = ffi::b2Circle {
                            center: ffi::b2Vec2 { x: 0.0, y: 0.0 },
                            radius: radius.get(),
                        };
                        ffi::b2CreateCircleShape(raw_body, &raw const shape_def, &raw const circle);
                    }
                    ShapeV1::Rectangle { half_extents } => {
                        let polygon = ffi::b2MakeBox(half_extents.x, half_extents.y);
                        ffi::b2CreatePolygonShape(
                            raw_body,
                            &raw const shape_def,
                            &raw const polygon,
                        );
                    }
                }
                if body.inverse_mass.get() > 0.0 {
                    ffi::b2Body_SetMassData(
                        raw_body,
                        ffi::b2MassData {
                            mass: 1.0 / body.inverse_mass.get(),
                            center: ffi::b2Vec2 { x: 0.0, y: 0.0 },
                            rotationalInertia: 1.0 / body.inverse_inertia.get(),
                        },
                    );
                }
                bodies.push(raw_body);
            }
            Ok(Self {
                world,
                bodies,
                scheduler,
            })
        }
    }

    fn step(&self, scenario: &ScenarioV1) {
        // SAFETY: the world and scheduler remain live and this call is exclusive.
        unsafe {
            self.scheduler.begin_step();
            ffi::b2World_Step(
                self.world.0,
                scenario.settings.fixed_dt.get(),
                scenario.settings.substeps as i32,
            );
        }
    }

    fn counters(&self) -> ffi::b2Counters {
        // SAFETY: the live world is not being stepped while counters are read.
        unsafe { ffi::b2World_GetCounters(self.world.0) }
    }

    fn body_hash(&self, scenario: &ScenarioV1) -> Result<String, String> {
        let mut bodies = scenario.bodies.clone();
        // SAFETY: every raw body belongs to the live world and getters are read-only.
        unsafe {
            for (body, raw) in bodies.iter_mut().zip(&self.bodies) {
                let position = ffi::b2Body_GetPosition(*raw);
                let rotation = ffi::b2Body_GetRotation(*raw);
                let velocity = ffi::b2Body_GetLinearVelocity(*raw);
                let angular_velocity = ffi::b2Body_GetAngularVelocity(*raw);
                body.position = physbox_contracts::Vec2::new(position.x, position.y);
                body.angle = physbox_contracts::FiniteF32::new(rotation.s.atan2(rotation.c))
                    .map_err(|error| error.to_string())?;
                body.linear_velocity = physbox_contracts::Vec2::new(velocity.x, velocity.y);
                body.angular_velocity = physbox_contracts::FiniteF32::new(angular_velocity)
                    .map_err(|error| error.to_string())?;
                body.validate().map_err(|error| error.to_string())?;
            }
        }
        let bytes = serde_json::to_vec(&bodies).map_err(|error| error.to_string())?;
        Ok(sha256_bytes(&bytes))
    }
}

struct WorldGuard(ffi::b2WorldId);

impl Drop for WorldGuard {
    fn drop(&mut self) {
        // SAFETY: this guard uniquely destroys its live world once.
        unsafe { ffi::b2DestroyWorld(self.0) }
    }
}

struct SchedulerGuard {
    pointer: *mut ffi::physbox_scheduler,
}

impl SchedulerGuard {
    unsafe fn new(worker_count: u32) -> Result<Self, String> {
        let pointer = if worker_count > 1 {
            // SAFETY: creation returns an owned opaque scheduler pointer.
            unsafe { ffi::physbox_scheduler_create(worker_count as i32) }
        } else {
            ptr::null_mut()
        };
        if worker_count > 1 && pointer.is_null() {
            return Err("failed to create Box2D benchmark scheduler".to_owned());
        }
        Ok(Self { pointer })
    }

    unsafe fn begin_step(&self) {
        if !self.pointer.is_null() {
            // SAFETY: the pointer is live and no prior step tasks remain.
            unsafe { ffi::physbox_scheduler_begin_step(self.pointer) };
        }
    }
}

impl Drop for SchedulerGuard {
    fn drop(&mut self) {
        if !self.pointer.is_null() {
            // SAFETY: the world was destroyed first and owns no remaining tasks.
            unsafe { ffi::physbox_scheduler_destroy(self.pointer) };
        }
    }
}

fn raw_vec(value: physbox_contracts::Vec2) -> ffi::b2Vec2 {
    ffi::b2Vec2 {
        x: value.x,
        y: value.y,
    }
}

fn named(name: &str, value: &str) -> NamedValueV1 {
    NamedValueV1 {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use physbox_engine::authored_chain_scenario;

    use super::*;

    #[test]
    fn optimized_single_and_multi_worker_lanes_share_the_source_projection() {
        let scenario = authored_chain_scenario(32, 7).unwrap();
        let hash = canonical_hash(&scenario).unwrap();
        let single = benchmark_scenario(&scenario, "single", hash.clone(), 1, 1, 2).unwrap();
        let multi = benchmark_scenario(&scenario, "multi", hash, 4, 1, 2).unwrap();
        assert_eq!(single.source_scenario_hash, multi.source_scenario_hash);
        assert_eq!(single.source_revision, BOX2D_REVISION);
        assert_eq!(multi.worker_count, 4);
        assert!(single.contact_count_max > 0);
        assert!(multi.contact_count_max > 0);
        assert!(single.samples_seconds.iter().all(|sample| *sample > 0.0));
        assert!(multi.samples_seconds.iter().all(|sample| *sample > 0.0));
    }
}
