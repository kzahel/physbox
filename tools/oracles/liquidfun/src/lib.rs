use std::{ffi::c_void, slice};

use physbox_contracts::{
    BodyStateV1, CoupledOracleRequestV1, CoupledOracleResultV1, NamedValueV1, NeutralActionV1,
    ParticleRegionV1, ProvenanceV1, ShapeV1, Validate, Vec2,
};
pub use physbox_particle_oracle_common::AdapterError;
use physbox_particle_oracle_common::ParticleOracleWorld;

const LIQUIDFUN_REVISION: &str = env!("LIQUIDFUN_REVISION");

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
    physbox_contracts::FiniteF32::new(value).expect("LiquidFun oracle values are finite")
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
    fn liquidfun_oracle_create(
        gravity_x: f32,
        gravity_y: f32,
        particle_radius: f32,
        particle_density: f32,
        particle_gravity_scale: f32,
        particle_capacity: u32,
    ) -> *mut c_void;
    fn liquidfun_oracle_destroy(handle: *mut c_void);
    fn liquidfun_oracle_add_circle(
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
    fn liquidfun_oracle_add_rectangle(
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
    fn liquidfun_oracle_spawn_rectangle(
        handle: *mut c_void,
        lower_x: f32,
        lower_y: f32,
        upper_x: f32,
        upper_y: f32,
        spacing: f32,
        velocity_x: f32,
        velocity_y: f32,
    ) -> u32;
    fn liquidfun_oracle_set_gravity(handle: *mut c_void, gravity_x: f32, gravity_y: f32);
    fn liquidfun_oracle_apply_impulse(
        handle: *mut c_void,
        body_index: u32,
        impulse_x: f32,
        impulse_y: f32,
        point_x: f32,
        point_y: f32,
    ) -> u32;
    fn liquidfun_oracle_step(handle: *mut c_void, time_step: f32, substeps: u32) -> u32;
    fn liquidfun_oracle_particle_count(handle: *const c_void) -> u32;
    fn liquidfun_oracle_particle_positions(handle: *const c_void) -> *const f32;
    fn liquidfun_oracle_body_contact_count(handle: *const c_void) -> u32;
    fn liquidfun_oracle_body_state(
        handle: *const c_void,
        index: u32,
        state: *mut NativeBodyState,
    ) -> u32;
}

struct NativeWorld {
    handle: *mut c_void,
}

impl ParticleOracleWorld for NativeWorld {
    const DISPLAY_NAME: &'static str = "original LiquidFun";

    fn create(
        gravity: Vec2,
        particle_radius: f32,
        particle_density: f32,
        gravity_scale: f32,
        capacity: u32,
    ) -> Result<Self, AdapterError> {
        let handle = unsafe {
            liquidfun_oracle_create(
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
                "could not create original LiquidFun world".to_owned(),
            ));
        }
        Ok(Self { handle })
    }

    fn add_body(&mut self, body: &BodyStateV1) -> Result<u32, AdapterError> {
        let dynamic_body = u32::from(body.inverse_mass.get() > 0.0);
        let sleeping = u32::from(body.sleeping);
        let index = unsafe {
            match body.shape {
                ShapeV1::Circle { radius } => liquidfun_oracle_add_circle(
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
                ShapeV1::Rectangle { half_extents } => liquidfun_oracle_add_rectangle(
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
        u32::try_from(index).map_err(|_| AdapterError::Native("could not create body".to_owned()))
    }

    fn spawn_region(
        &mut self,
        region: &ParticleRegionV1,
        _particle_radius: f32,
    ) -> Result<u32, AdapterError> {
        let created = unsafe {
            liquidfun_oracle_spawn_rectangle(
                self.handle,
                region.lower.x,
                region.lower.y,
                region.upper.x,
                region.upper.y,
                region.spacing.get(),
                region.initial_velocity.x,
                region.initial_velocity.y,
            )
        };
        if created == 0 {
            return Err(AdapterError::Native(
                "LiquidFun rejected a projected particle region".to_owned(),
            ));
        }
        Ok(created)
    }

    fn apply_action(&mut self, action: &NeutralActionV1) -> Result<(), AdapterError> {
        match action {
            NeutralActionV1::ApplyImpulse {
                body,
                impulse,
                world_point,
                ..
            } => {
                let applied = unsafe {
                    liquidfun_oracle_apply_impulse(
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
                liquidfun_oracle_set_gravity(self.handle, gravity.x, gravity.y);
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
        if unsafe { liquidfun_oracle_step(self.handle, dt, substeps) } == 0 {
            return Err(AdapterError::Native(
                "original LiquidFun step failed".to_owned(),
            ));
        }
        Ok(())
    }

    fn particle_positions(&self) -> Result<Vec<Vec2>, AdapterError> {
        let count = unsafe { liquidfun_oracle_particle_count(self.handle) } as usize;
        if count == 0 {
            return Ok(Vec::new());
        }
        let pointer = unsafe { liquidfun_oracle_particle_positions(self.handle) };
        if pointer.is_null() {
            return Err(AdapterError::Native(
                "LiquidFun particle position pointer is null".to_owned(),
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
        if unsafe { liquidfun_oracle_body_state(self.handle, index, &mut state) } == 0 {
            return Err(AdapterError::Native(format!(
                "could not read LiquidFun body index {index}"
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

    fn particle_body_contacts(&self) -> u64 {
        u64::from(unsafe { liquidfun_oracle_body_contact_count(self.handle) })
    }

    fn provenance(request: &CoupledOracleRequestV1, particle_radius: f32) -> ProvenanceV1 {
        ProvenanceV1 {
            run_id: request.run_id.clone(),
            source_revision: LIQUIDFUN_REVISION.to_owned(),
            source_dirty: false,
            source_patch_digest: None,
            host: format!("{}-{}", std::env::consts::OS, std::env::consts::ARCH),
            adapter: "liquidfun-native".to_owned(),
            backend: "original-liquidfun-box2d-2.3".to_owned(),
            driver: "native-c++".to_owned(),
            toolchain: env!("CXX_VERSION").to_owned(),
            build_options: vec![
                named("box2d-version", "2.3.0"),
                named("liquidfun-external-language-api", "1"),
                named("liquidfun-revision", LIQUIDFUN_REVISION),
            ],
            runtime_settings: vec![
                named("coupling-order", "liquidfun-world-coupled-step"),
                named("particle-iterations-per-substep", "1"),
                named("particle-radius", &format!("{particle_radius:.9}")),
                named("position-iterations", "3"),
                named(
                    "rigid-substeps",
                    &request.scenario.rigid.settings.substeps.to_string(),
                ),
                named("velocity-iterations", "8"),
            ],
        }
    }
}

impl Drop for NativeWorld {
    fn drop(&mut self) {
        unsafe {
            liquidfun_oracle_destroy(self.handle);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use physbox_contracts::{
        CoupledOracleRequestV1, FailureKindV1, SCHEMA_VERSION, canonical_hash, canonical_json,
        sha256_bytes,
    };
    use physbox_engine::{CoupledScenarioKind, canonical_coupled_scenario};

    #[test]
    fn contained_water_executes_against_original_liquidfun() {
        let scenario = canonical_coupled_scenario(CoupledScenarioKind::ContainedWater).unwrap();
        let request = CoupledOracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id: "liquidfun-contained-test".to_owned(),
            source_scenario_hash: canonical_hash(&scenario).unwrap(),
            scenario,
        };
        let bytes = canonical_json(&request).unwrap();
        let result = execute_request(&request, &bytes).unwrap();
        assert_eq!(result.completed_steps, request.scenario.requested_steps);
        assert!(!result.particles.is_empty());
        assert_eq!(result.projection_hash, sha256_bytes(&bytes));
        assert!(result.failure.is_none());
        result.validate().unwrap();
    }

    #[test]
    fn grabbed_boundary_is_rejected_without_approximation() {
        let scenario = canonical_coupled_scenario(CoupledScenarioKind::GrabbedBoundary).unwrap();
        let request = CoupledOracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id: "liquidfun-unsupported-grab".to_owned(),
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
