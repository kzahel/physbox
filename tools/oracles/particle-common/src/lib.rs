use std::{
    collections::BTreeSet,
    fmt::{Display, Formatter},
};

use physbox_contracts::{
    BodyStateV1, CapacityRecordV1, CoupledMetricsV1, CoupledOracleRequestV1, CoupledOracleResultV1,
    CoupledWorkVectorV1, FailureKindV1, FailureV1, FiniteF32, NeutralActionV1,
    PARTICLE_STATE_LAYOUT_VERSION, ParticleId, ParticleRegionV1, ParticleStateV1, ProvenanceV1,
    SCHEMA_VERSION, Validate, Vec2, sha256_bytes,
};

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
            Self::Native(reason) => write!(formatter, "native oracle failure: {reason}"),
        }
    }
}

impl std::error::Error for AdapterError {}

pub trait ParticleOracleWorld: Sized {
    const DISPLAY_NAME: &'static str;

    fn create(
        gravity: Vec2,
        particle_radius: f32,
        particle_density: f32,
        gravity_scale: f32,
        capacity: u32,
    ) -> Result<Self, AdapterError>;

    fn add_body(&mut self, body: &BodyStateV1) -> Result<u32, AdapterError>;

    fn spawn_region(
        &mut self,
        region: &ParticleRegionV1,
        particle_radius: f32,
    ) -> Result<u32, AdapterError>;

    fn apply_action(&mut self, action: &NeutralActionV1) -> Result<(), AdapterError>;

    fn step(&mut self, dt: f32, substeps: u32) -> Result<(), AdapterError>;

    fn particle_positions(&self) -> Result<Vec<Vec2>, AdapterError>;

    fn body_state(&self, index: u32, source: &BodyStateV1) -> Result<BodyStateV1, AdapterError>;

    fn particle_body_contacts(&self) -> u64 {
        0
    }

    fn provenance(request: &CoupledOracleRequestV1, particle_radius: f32) -> ProvenanceV1;
}

pub fn execute_request<W: ParticleOracleWorld>(
    request: &CoupledOracleRequestV1,
    request_bytes: &[u8],
) -> Result<CoupledOracleResultV1, AdapterError> {
    request
        .validate()
        .map_err(|error| AdapterError::Invalid(error.to_string()))?;
    projection_support::<W>(request)?;
    let material = request
        .scenario
        .materials
        .first()
        .copied()
        .ok_or_else(|| AdapterError::Invalid("missing water material".to_owned()))?;
    let mut world = W::create(
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
    let particle_body_contacts = world
        .particle_body_contacts()
        .max(u64::from(fluid_wake_roots));
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
        provenance: W::provenance(request, material.radius.get()),
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
                particle_body_contacts,
                body_contribution_edges: particle_body_contacts,
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

pub fn unsupported_result<W: ParticleOracleWorld>(
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
        provenance: W::provenance(request, material_radius),
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

fn projection_support<W: ParticleOracleWorld>(
    request: &CoupledOracleRequestV1,
) -> Result<(), AdapterError> {
    if request.scenario.materials.len() != 1
        || !matches!(
            request.scenario.materials[0].kind,
            physbox_contracts::ParticleMaterialKindV1::Water
        )
    {
        return Err(AdapterError::Unsupported(format!(
            "{} supports exactly one water material",
            W::DISPLAY_NAME
        )));
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
        return Err(AdapterError::Unsupported(format!(
            "native {} projection does not approximate the PhysBox grab constraint",
            W::DISPLAY_NAME
        )));
    }
    Ok(())
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
