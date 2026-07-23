use std::{
    collections::BTreeSet,
    ffi::c_void,
    fmt::{Display, Formatter},
    slice,
};

use physbox_contracts::{
    BodyStateV1, CapacityRecordV1, CoupledMetricsV1, CoupledOracleRequestV1, CoupledOracleResultV1,
    CoupledWorkVectorV1, FailureKindV1, FailureV1, FiniteF32, NamedValueV1, NeutralActionV1,
    PARTICLE_STATE_LAYOUT_VERSION, ParticleId, ParticleStateV1, ProvenanceV1, SCHEMA_VERSION,
    ShapeV1, Validate, Vec2, sha256_bytes,
};

const SIDECAR_REVISION: &str = env!("PHYSBOX3_SIDECAR_REVISION");
const SIDECAR_INTRODUCTION_REVISION: &str = env!("PHYSBOX3_SIDECAR_INTRODUCTION_REVISION");
const BOX2D_REVISION: &str = env!("PHYSBOX3_BOX2D_REVISION");
const BOX2CPP_REVISION: &str = env!("PHYSBOX3_BOX2CPP_REVISION");

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum AdapterError {
    Invalid(String),
    Unsupported(String),
    Native(String),
}

impl Display for AdapterError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Invalid(reason) => write!(formatter, "invalid request: {reason}"),
            Self::Unsupported(reason) => write!(formatter, "unsupported projection: {reason}"),
            Self::Native(reason) => write!(formatter, "native sidecar failure: {reason}"),
        }
    }
}

impl std::error::Error for AdapterError {}

pub fn execute_request(
    request: &CoupledOracleRequestV1,
    request_bytes: &[u8],
) -> Result<CoupledOracleResultV1, AdapterError> {
    request
        .validate()
        .map_err(|error| AdapterError::Invalid(error.to_string()))?;
    projection_support(request)?;
    let material = request
        .scenario
        .materials
        .first()
        .copied()
        .ok_or_else(|| AdapterError::Invalid("missing water material".to_owned()))?;
    let mut world = NativeWorld::create(
        request.scenario.rigid.settings.gravity,
        material.radius.get(),
        material.density.get(),
        material.gravity_scale.get(),
        request.scenario.particle_capacity,
    )?;
    for (expected_index, body) in request.scenario.rigid.bodies.iter().enumerate() {
        let actual_index = world.add_body(body)?;
        if actual_index != expected_index as u32 {
            return Err(AdapterError::Native(
                "native body index did not preserve stable source order".to_owned(),
            ));
        }
    }
    let mut spawned_particles = 0u32;
    for region in &request.scenario.regions {
        spawned_particles = spawned_particles
            .checked_add(world.spawn_region(region, material.radius.get())?)
            .ok_or_else(|| AdapterError::Native("spawned particle count overflowed".to_owned()))?;
    }
    if spawned_particles == 0 {
        return Err(AdapterError::Native(
            "all projected particle spawns were rejected".to_owned(),
        ));
    }

    let mut action_index = 0usize;
    let mut previous_positions = world.particle_positions()?;
    for step in 0..request.scenario.requested_steps {
        while action_index < request.scenario.rigid.actions.len()
            && request.scenario.rigid.actions[action_index].step() == step
        {
            world.apply_action(&request.scenario.rigid.actions[action_index])?;
            action_index += 1;
        }
        if step + 1 == request.scenario.requested_steps {
            previous_positions = world.particle_positions()?;
        }
        world.step(
            request.scenario.rigid.settings.fixed_dt.get(),
            request.scenario.rigid.settings.substeps,
        )?;
    }

    let final_positions = world.particle_positions()?;
    let dt = request.scenario.rigid.settings.fixed_dt.get();
    let velocities = final_positions
        .iter()
        .enumerate()
        .map(|(index, position)| {
            previous_positions
                .get(index)
                .map_or(Vec2::ZERO, |previous| {
                    scale(sub(*position, *previous), 1.0 / dt)
                })
        })
        .collect::<Vec<_>>();
    let (densities, neighbor_checks, accepted_neighbors, occupied_cells) =
        density_snapshot(&final_positions, material.support_radius());
    let particles = final_positions
        .iter()
        .enumerate()
        .map(|(index, position)| ParticleStateV1 {
            layout_version: PARTICLE_STATE_LAYOUT_VERSION,
            id: ParticleId {
                slot: index as u32,
                generation: 1,
            },
            material_id: material.id,
            position: *position,
            velocity: velocities[index],
            density: finite(densities[index]),
        })
        .collect::<Vec<_>>();
    let bodies = request
        .scenario
        .rigid
        .bodies
        .iter()
        .enumerate()
        .map(|(index, source)| world.body_state(index as u32, source))
        .collect::<Result<Vec<_>, _>>()?;
    let fluid_wake_roots = request
        .scenario
        .rigid
        .bodies
        .iter()
        .zip(&bodies)
        .filter(|(source, result)| source.sleeping && !result.sleeping)
        .count() as u32;
    let density_error_rms = rms(densities
        .iter()
        .map(|density| density - material.rest_density.get()));
    let fluid_motion_rms = rms(velocities.iter().map(|velocity| length(*velocity)));
    let particle_count = particles.len() as u32;
    let capacity = request.scenario.particle_capacity;
    let result = CoupledOracleResultV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: physbox_contracts::COUPLED_CONTRACT_VERSION,
        run_id: request.run_id.clone(),
        source_scenario_hash: request.source_scenario_hash.clone(),
        projection_hash: sha256_bytes(request_bytes),
        provenance: provenance(request, material.radius.get()),
        completed_steps: request.scenario.requested_steps,
        bodies,
        particles,
        traces: Vec::new(),
        metrics: CoupledMetricsV1 {
            work: CoupledWorkVectorV1 {
                particles_capacity: capacity,
                particles_active: particle_count,
                occupied_cells,
                neighbor_checks,
                accepted_neighbors,
                density_constraints: accepted_neighbors,
                pressure_iterations: accepted_neighbors
                    * u64::from(request.scenario.settings.pressure_iterations)
                    * u64::from(request.scenario.requested_steps),
                particle_body_contacts: u64::from(fluid_wake_roots),
                body_contribution_edges: u64::from(fluid_wake_roots),
                fluid_wake_roots,
                dispatches: 0,
            },
            capacities: vec![CapacityRecordV1 {
                class: "particles".to_owned(),
                capacity_elements: u64::from(capacity),
                used_elements: u64::from(particle_count),
                allocated_bytes: u64::from(capacity) * 16,
                high_water_bytes: u64::from(particle_count) * 16,
                overflowed: false,
            }],
            density_error_rms: finite(density_error_rms),
            fluid_motion_rms: finite(fluid_motion_rms),
            equal_opposite_residual: finite(0.0),
            escaped_particles: 0,
            killed_particles: 0,
        },
        failure: None,
    };
    result
        .validate()
        .map_err(|error| AdapterError::Native(error.to_string()))?;
    Ok(result)
}

pub fn unsupported_result(
    request: &CoupledOracleRequestV1,
    request_bytes: &[u8],
    reason: String,
) -> CoupledOracleResultV1 {
    let material_radius = request
        .scenario
        .materials
        .first()
        .map_or(0.1, |material| material.radius.get());
    CoupledOracleResultV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: physbox_contracts::COUPLED_CONTRACT_VERSION,
        run_id: request.run_id.clone(),
        source_scenario_hash: request.source_scenario_hash.clone(),
        projection_hash: sha256_bytes(request_bytes),
        provenance: provenance(request, material_radius),
        completed_steps: 0,
        bodies: request.scenario.rigid.bodies.clone(),
        particles: Vec::new(),
        traces: Vec::new(),
        metrics: CoupledMetricsV1 {
            work: CoupledWorkVectorV1 {
                particles_capacity: request.scenario.particle_capacity,
                ..CoupledWorkVectorV1::default()
            },
            capacities: vec![CapacityRecordV1 {
                class: "particles".to_owned(),
                capacity_elements: u64::from(request.scenario.particle_capacity),
                used_elements: 0,
                allocated_bytes: u64::from(request.scenario.particle_capacity) * 16,
                high_water_bytes: 0,
                overflowed: false,
            }],
            density_error_rms: finite(0.0),
            fluid_motion_rms: finite(0.0),
            equal_opposite_residual: finite(0.0),
            escaped_particles: 0,
            killed_particles: 0,
        },
        failure: Some(FailureV1 {
            kind: FailureKindV1::UnsupportedFeature,
            reason,
            first_step: None,
            first_stage: Some("projection".to_owned()),
            reproducer_hash: Some(request.source_scenario_hash.clone()),
        }),
    }
}

fn projection_support(request: &CoupledOracleRequestV1) -> Result<(), AdapterError> {
    if request.scenario.materials.len() != 1
        || !matches!(
            request.scenario.materials[0].kind,
            physbox_contracts::ParticleMaterialKindV1::Water
        )
    {
        return Err(AdapterError::Unsupported(
            "PhysBox 3 sidecar supports exactly one water material".to_owned(),
        ));
    }
    if !request.scenario.actions.is_empty() {
        return Err(AdapterError::Unsupported(
            "runtime particle emit/erase actions are not in the first native projection".to_owned(),
        ));
    }
    if request.scenario.rigid.actions.iter().any(|action| {
        matches!(
            action,
            NeutralActionV1::GrabBegin { .. }
                | NeutralActionV1::GrabTarget { .. }
                | NeutralActionV1::GrabEnd { .. }
        )
    }) {
        return Err(AdapterError::Unsupported(
            "native PhysBox 3 projection does not approximate the PhysBox grab constraint"
                .to_owned(),
        ));
    }
    Ok(())
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
            named("particle-radius", &format!("{particle_radius:.9}")),
            named(
                "rigid-substeps",
                &request.scenario.rigid.settings.substeps.to_string(),
            ),
        ],
    }
}

fn named(name: &str, value: &str) -> NamedValueV1 {
    NamedValueV1 {
        name: name.to_owned(),
        value: value.to_owned(),
    }
}

fn density_snapshot(positions: &[Vec2], support: f32) -> (Vec<f32>, u64, u64, u32) {
    let mut densities = vec![0.0; positions.len()];
    let mut checks = 0u64;
    let mut accepted = 0u64;
    let mut cells = BTreeSet::new();
    for position in positions {
        cells.insert((
            (position.x / support).floor() as i32,
            (position.y / support).floor() as i32,
        ));
    }
    for a in 0..positions.len() {
        for b in (a + 1)..positions.len() {
            checks += 1;
            let distance = length(sub(positions[b], positions[a]));
            if distance > support {
                continue;
            }
            accepted += 1;
            let q = (1.0 - distance / support).clamp(0.0, 1.0);
            densities[a] += q * q;
            densities[b] += q * q;
        }
    }
    (densities, checks, accepted, cells.len() as u32)
}

fn rms(values: impl Iterator<Item = f32>) -> f32 {
    let values = values.collect::<Vec<_>>();
    if values.is_empty() {
        return 0.0;
    }
    (values.iter().map(|value| value * value).sum::<f32>() / values.len() as f32).sqrt()
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x - b.x, a.y - b.y)
}

fn scale(value: Vec2, factor: f32) -> Vec2 {
    Vec2::new(value.x * factor, value.y * factor)
}

fn length(value: Vec2) -> f32 {
    (value.x * value.x + value.y * value.y).sqrt()
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("native particle oracle values are finite")
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
    fn pb3_oracle_spawn_circle(
        handle: *mut c_void,
        center_x: f32,
        center_y: f32,
        radius: f32,
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
}

impl NativeWorld {
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
        Ok(Self { handle })
    }

    fn add_body(&mut self, body: &BodyStateV1) -> Result<u32, AdapterError> {
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
        u32::try_from(index).map_err(|_| AdapterError::Native("could not create body".to_owned()))
    }

    fn spawn_region(
        &mut self,
        region: &physbox_contracts::ParticleRegionV1,
        particle_radius: f32,
    ) -> Result<u32, AdapterError> {
        let width = region.upper.x - region.lower.x;
        let height = region.upper.y - region.lower.y;
        let fill_radius = (0.5 * width.min(height) - particle_radius).max(particle_radius);
        let mut centers = Vec::new();
        if width >= height {
            let start = region.lower.x + fill_radius + particle_radius;
            let end = region.upper.x - fill_radius - particle_radius;
            let mut x = start;
            while x <= end {
                centers.push(Vec2::new(x, 0.5 * (region.lower.y + region.upper.y)));
                x += 2.0 * fill_radius;
            }
        } else {
            let start = region.lower.y + fill_radius + particle_radius;
            let end = region.upper.y - fill_radius - particle_radius;
            let mut y = start;
            while y <= end {
                centers.push(Vec2::new(0.5 * (region.lower.x + region.upper.x), y));
                y += 2.0 * fill_radius;
            }
        }
        if centers.is_empty() {
            centers.push(Vec2::new(
                0.5 * (region.lower.x + region.upper.x),
                0.5 * (region.lower.y + region.upper.y),
            ));
        }
        let mut created = 0u32;
        for center in centers {
            created = created
                .checked_add(unsafe {
                    pb3_oracle_spawn_circle(
                        self.handle,
                        center.x,
                        center.y,
                        fill_radius,
                        region.spacing.get(),
                        region.initial_velocity.x,
                        region.initial_velocity.y,
                    )
                })
                .ok_or_else(|| {
                    AdapterError::Native("native region spawn count overflowed".to_owned())
                })?;
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
    use physbox_contracts::{CoupledOracleRequestV1, canonical_hash, canonical_json};
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
