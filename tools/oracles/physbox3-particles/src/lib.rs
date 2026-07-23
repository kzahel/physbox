use std::{ffi::c_void, slice};

use physbox_contracts::{
    BodyStateV1, CoupledOracleRequestV1, CoupledOracleResultV1, NamedValueV1, NeutralActionV1,
    ParticleRegionV1, ProvenanceV1, ShapeV1, Validate, Vec2,
};
pub use physbox_particle_oracle_common::AdapterError;
use physbox_particle_oracle_common::ParticleOracleWorld;

const SIDECAR_REVISION: &str = env!("PHYSBOX3_SIDECAR_REVISION");
const SIDECAR_INTRODUCTION_REVISION: &str = env!("PHYSBOX3_SIDECAR_INTRODUCTION_REVISION");
const BOX2D_REVISION: &str = env!("PHYSBOX3_BOX2D_REVISION");
const BOX2CPP_REVISION: &str = env!("PHYSBOX3_BOX2CPP_REVISION");

pub fn execute_request(
    request: &CoupledOracleRequestV1,
    request_bytes: &[u8],
) -> Result<CoupledOracleResultV1, AdapterError> {
    physbox_particle_oracle_common::execute_request::<NativeWorld>(request, request_bytes)
}

pub fn unsupported_result(
    request: &CoupledOracleRequestV1,
    request_bytes: &[u8],
    reason: String,
) -> CoupledOracleResultV1 {
    physbox_particle_oracle_common::unsupported_result::<NativeWorld>(
        request,
        request_bytes,
        reason,
    )
}

fn named(name: &str, value: &str) -> NamedValueV1 {
    NamedValueV1 {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn finite(value: f32) -> physbox_contracts::FiniteF32 {
    physbox_contracts::FiniteF32::new(value).expect("native particle oracle values are finite")
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
struct NativeBodyState {
    position_x: f32,
    position_y: f32,
    angle: f32,
    linear_velocity_x: f32,
    linear_velocity_y: f32,
    angular_velocity: f32,
    awake: u32,
}

unsafe extern "C" {
    fn pb3_oracle_create(
        gravity_x: f32,
        gravity_y: f32,
        particle_radius: f32,
        particle_density: f32,
        particle_gravity_scale: f32,
        particle_capacity: u32,
    ) -> *mut c_void;
    fn pb3_oracle_destroy(handle: *mut c_void);
    fn pb3_oracle_add_circle(
        handle: *mut c_void,
        dynamic_body: u32,
        position_x: f32,
        position_y: f32,
        angle: f32,
        linear_velocity_x: f32,
        linear_velocity_y: f32,
        angular_velocity: f32,
        radius: f32,
        inverse_mass: f32,
        inverse_inertia: f32,
        friction: f32,
        restitution: f32,
        sleeping: u32,
    ) -> i32;
    fn pb3_oracle_add_rectangle(
        handle: *mut c_void,
        dynamic_body: u32,
        position_x: f32,
        position_y: f32,
        angle: f32,
        linear_velocity_x: f32,
        linear_velocity_y: f32,
        angular_velocity: f32,
        half_extent_x: f32,
        half_extent_y: f32,
        inverse_mass: f32,
        inverse_inertia: f32,
        friction: f32,
        restitution: f32,
        sleeping: u32,
    ) -> i32;
    fn pb3_oracle_spawn_rectangle(
        handle: *mut c_void,
        lower_x: f32,
        lower_y: f32,
        upper_x: f32,
        upper_y: f32,
        particle_radius: f32,
        spacing: f32,
        velocity_x: f32,
        velocity_y: f32,
    ) -> u32;
    fn pb3_oracle_set_gravity(handle: *mut c_void, gravity_x: f32, gravity_y: f32);
    fn pb3_oracle_apply_impulse(
        handle: *mut c_void,
        body_index: u32,
        impulse_x: f32,
        impulse_y: f32,
        point_x: f32,
        point_y: f32,
    ) -> u32;
    fn pb3_oracle_step(handle: *mut c_void, time_step: f32, substeps: u32) -> u32;
    fn pb3_oracle_particle_count(handle: *const c_void) -> u32;
    fn pb3_oracle_particle_positions(handle: *const c_void) -> *const f32;
    fn pb3_oracle_body_state(handle: *const c_void, index: u32, state: *mut NativeBodyState)
    -> u32;
}

struct NativeWorld {
    handle: *mut c_void,
    pending_bodies: Vec<BodyStateV1>,
    bodies_realized: bool,
}

impl NativeWorld {
    fn realize_bodies(&mut self) -> Result<(), AdapterError> {
        if self.bodies_realized {
            return Ok(());
        }
        for (expected_index, body) in self.pending_bodies.iter().enumerate() {
            let dynamic_body = u32::from(body.inverse_mass.get() > 0.0);
            let sleeping = u32::from(body.sleeping);
            let index = unsafe {
                match body.shape {
                    ShapeV1::Circle { radius } => pb3_oracle_add_circle(
                        self.handle,
                        dynamic_body,
                        body.position.x,
                        body.position.y,
                        body.angle.get(),
                        body.linear_velocity.x,
                        body.linear_velocity.y,
                        body.angular_velocity.get(),
                        radius.get(),
                        body.inverse_mass.get(),
                        body.inverse_inertia.get(),
                        body.material.friction.get(),
                        body.material.restitution.get(),
                        sleeping,
                    ),
                    ShapeV1::Rectangle { half_extents } => pb3_oracle_add_rectangle(
                        self.handle,
                        dynamic_body,
                        body.position.x,
                        body.position.y,
                        body.angle.get(),
                        body.linear_velocity.x,
                        body.linear_velocity.y,
                        body.angular_velocity.get(),
                        half_extents.x,
                        half_extents.y,
                        body.inverse_mass.get(),
                        body.inverse_inertia.get(),
                        body.material.friction.get(),
                        body.material.restitution.get(),
                        sleeping,
                    ),
                }
            };
            let actual_index = u32::try_from(index)
                .map_err(|_| AdapterError::Native("could not create body".to_owned()))?;
            if actual_index != expected_index as u32 {
                return Err(AdapterError::Native(
                    "native body realization did not preserve stable source order".to_owned(),
                ));
            }
        }
        self.bodies_realized = true;
        Ok(())
    }
}

impl ParticleOracleWorld for NativeWorld {
    const DISPLAY_NAME: &'static str = "PhysBox 3 sidecar";

    fn create(
        gravity: Vec2,
        particle_radius: f32,
        particle_density: f32,
        gravity_scale: f32,
        capacity: u32,
    ) -> Result<Self, AdapterError> {
        let handle = unsafe {
            pb3_oracle_create(
                gravity.x,
                gravity.y,
                particle_radius,
                particle_density,
                gravity_scale,
                capacity,
            )
        };
        if handle.is_null() {
            return Err(AdapterError::Native(
                "could not create native particle world".to_owned(),
            ));
        }
        Ok(Self {
            handle,
            pending_bodies: Vec::new(),
            bodies_realized: false,
        })
    }

    fn add_body(&mut self, body: &BodyStateV1) -> Result<u32, AdapterError> {
        if self.bodies_realized {
            return Err(AdapterError::Native(
                "cannot add a body after native realization".to_owned(),
            ));
        }
        let index = u32::try_from(self.pending_bodies.len())
            .map_err(|_| AdapterError::Native("body count exceeds u32".to_owned()))?;
        self.pending_bodies.push(body.clone());
        Ok(index)
    }

    fn spawn_region(
        &mut self,
        region: &ParticleRegionV1,
        particle_radius: f32,
    ) -> Result<u32, AdapterError> {
        let created = unsafe {
            pb3_oracle_spawn_rectangle(
                self.handle,
                region.lower.x,
                region.lower.y,
                region.upper.x,
                region.upper.y,
                particle_radius,
                region.spacing.get(),
                region.initial_velocity.x,
                region.initial_velocity.y,
            )
        };
        if created == 0 {
            return Err(AdapterError::Native(
                "PhysBox 3 rejected an exact authored particle lattice".to_owned(),
            ));
        }
        Ok(created)
    }

    fn apply_action(&mut self, action: &NeutralActionV1) -> Result<(), AdapterError> {
        self.realize_bodies()?;
        match action {
            NeutralActionV1::ApplyImpulse {
                body,
                impulse,
                world_point,
                ..
            } => {
                let applied = unsafe {
                    pb3_oracle_apply_impulse(
                        self.handle,
                        body.slot,
                        impulse.x,
                        impulse.y,
                        world_point.x,
                        world_point.y,
                    )
                };
                if applied == 0 {
                    return Err(AdapterError::Native(format!(
                        "could not apply impulse to body slot {}",
                        body.slot
                    )));
                }
            }
            NeutralActionV1::SetGravity { gravity, .. } => unsafe {
                pb3_oracle_set_gravity(self.handle, gravity.x, gravity.y);
            },
            NeutralActionV1::GrabBegin { .. }
            | NeutralActionV1::GrabTarget { .. }
            | NeutralActionV1::GrabEnd { .. } => {
                return Err(AdapterError::Unsupported(
                    "grab action reached native execution after projection validation".to_owned(),
                ));
            }
        }
        Ok(())
    }

    fn step(&mut self, dt: f32, substeps: u32) -> Result<(), AdapterError> {
        self.realize_bodies()?;
        if unsafe { pb3_oracle_step(self.handle, dt, substeps) } == 0 {
            return Err(AdapterError::Native(
                "native particle step failed".to_owned(),
            ));
        }
        Ok(())
    }

    fn particle_positions(&self) -> Result<Vec<Vec2>, AdapterError> {
        let count = unsafe { pb3_oracle_particle_count(self.handle) } as usize;
        if count == 0 {
            return Ok(Vec::new());
        }
        let pointer = unsafe { pb3_oracle_particle_positions(self.handle) };
        if pointer.is_null() {
            return Err(AdapterError::Native(
                "native particle position pointer is null".to_owned(),
            ));
        }
        let values = unsafe { slice::from_raw_parts(pointer, count * 2) };
        let mut positions = Vec::with_capacity(count);
        for pair in values.chunks_exact(2) {
            let position = Vec2::new(pair[0], pair[1]);
            position
                .validate()
                .map_err(|error| AdapterError::Native(error.to_string()))?;
            positions.push(position);
        }
        Ok(positions)
    }

    fn body_state(&self, index: u32, source: &BodyStateV1) -> Result<BodyStateV1, AdapterError> {
        let mut state = NativeBodyState::default();
        if unsafe { pb3_oracle_body_state(self.handle, index, &mut state) } == 0 {
            return Err(AdapterError::Native(format!(
                "could not read native body index {index}"
            )));
        }
        let result = BodyStateV1 {
            id: source.id,
            shape: source.shape,
            position: Vec2::new(state.position_x, state.position_y),
            angle: finite(state.angle),
            linear_velocity: Vec2::new(state.linear_velocity_x, state.linear_velocity_y),
            angular_velocity: finite(state.angular_velocity),
            inverse_mass: source.inverse_mass,
            inverse_inertia: source.inverse_inertia,
            material: source.material,
            sleeping: state.awake == 0,
        };
        result
            .validate()
            .map_err(|error| AdapterError::Native(error.to_string()))?;
        Ok(result)
    }

    fn provenance(request: &CoupledOracleRequestV1, particle_radius: f32) -> ProvenanceV1 {
        ProvenanceV1 {
            run_id: request.run_id.clone(),
            source_revision: SIDECAR_REVISION.to_owned(),
            source_dirty: false,
            source_patch_digest: None,
            host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            adapter: "physbox3-particle-sidecar-native".to_owned(),
            backend: "box2d-v3-reduced-particles".to_owned(),
            driver: "native-c++".to_owned(),
            toolchain: env!("CXX_VERSION").to_owned(),
            build_options: vec![
                named("box2cpp-revision", BOX2CPP_REVISION),
                named("box2d-revision", BOX2D_REVISION),
                named(
                    "sidecar-introduction-revision",
                    SIDECAR_INTRODUCTION_REVISION,
                ),
                named("sidecar-revision", SIDECAR_REVISION),
            ],
            runtime_settings: vec![
                named("coupling-order", "particle-then-box2d-per-rigid-substep"),
                named(
                    "initialization-order",
                    "authored particle lattice before native bodies",
                ),
                named("particle-radius", &format!("{particle_radius:.9}")),
                named(
                    "region-projection",
                    "exact authored lattice via bounded one-point sidecar circle calls",
                ),
                named(
                    "rigid-substeps",
                    &request.scenario.rigid.settings.substeps.to_string(),
                ),
            ],
        }
    }
}

impl Drop for NativeWorld {
    fn drop(&mut self) {
        unsafe {
            pb3_oracle_destroy(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physbox_contracts::{
        CoupledOracleRequestV1, FailureKindV1, FiniteF32, ParticleRegionV1, SCHEMA_VERSION,
        canonical_hash, canonical_json, sha256_bytes,
    };
    use physbox_engine::{CoupledScenarioKind, canonical_coupled_scenario};

    #[test]
    fn contained_water_executes_with_fresh_identity() {
        let scenario = canonical_coupled_scenario(CoupledScenarioKind::ContainedWater).unwrap();
        let request = CoupledOracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id: "physbox3-sidecar-contained-test".to_owned(),
            source_scenario_hash: canonical_hash(&scenario).unwrap(),
            scenario,
        };
        let bytes = canonical_json(&request).unwrap();
        let result = execute_request(&request, &bytes).unwrap();
        assert_eq!(result.completed_steps, request.scenario.requested_steps);
        assert_eq!(result.particles.len(), 456);
        assert_eq!(result.projection_hash, sha256_bytes(&bytes));
        assert!(result.failure.is_none());
        result.validate().unwrap();
    }

    #[test]
    fn rectangle_projection_preserves_the_authored_lattice() {
        let mut world = NativeWorld::create(Vec2::ZERO, 0.1, 1.0, 1.0, 16).unwrap();
        let region = ParticleRegionV1 {
            id: 0,
            material_id: 0,
            lower: Vec2::ZERO,
            upper: Vec2::new(0.6, 0.6),
            spacing: FiniteF32::new(0.2).unwrap(),
            initial_velocity: Vec2::ZERO,
        };
        assert_eq!(world.spawn_region(&region, 0.1).unwrap(), 9);
        let positions = world.particle_positions().unwrap();
        let expected = [
            Vec2::new(0.1, 0.1),
            Vec2::new(0.3, 0.1),
            Vec2::new(0.5, 0.1),
            Vec2::new(0.1, 0.3),
            Vec2::new(0.3, 0.3),
            Vec2::new(0.5, 0.3),
            Vec2::new(0.1, 0.5),
            Vec2::new(0.3, 0.5),
            Vec2::new(0.5, 0.5),
        ];
        assert_eq!(positions.len(), expected.len());
        for (actual, expected) in positions.iter().zip(expected) {
            assert!((actual.x - expected.x).abs() <= 1.0e-6);
            assert!((actual.y - expected.y).abs() <= 1.0e-6);
        }
    }

    #[test]
    fn grabbed_boundary_is_rejected_without_approximation() {
        let scenario = canonical_coupled_scenario(CoupledScenarioKind::GrabbedBoundary).unwrap();
        let request = CoupledOracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id: "physbox3-sidecar-unsupported-grab".to_owned(),
            source_scenario_hash: canonical_hash(&scenario).unwrap(),
            scenario,
        };
        let bytes = canonical_json(&request).unwrap();
        let reason = match execute_request(&request, &bytes) {
            Err(AdapterError::Unsupported(reason)) => reason,
            other => panic!("expected unsupported projection, got {other:?}"),
        };
        let result = unsupported_result(&request, &bytes, reason);
        result.validate().unwrap();
        assert!(matches!(
            result.failure.as_ref().map(|failure| &failure.kind),
            Some(FailureKindV1::UnsupportedFeature)
        ));
    }
}
