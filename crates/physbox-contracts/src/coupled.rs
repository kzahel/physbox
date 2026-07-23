use serde::{Deserialize, Serialize};

use crate::{
    BodyId, BodyStateV1, CapacityRecordV1, ContractError, FailureV1, FiniteF32, ProvenanceV1,
    ScenarioV1, ShapeV1, Validate, Vec2, canonical_hash, finite_f32, is_sha256,
    validate_sorted_unique, version,
};

pub const COUPLED_CONTRACT_VERSION: u32 = 1;
pub const PARTICLE_STATE_LAYOUT_VERSION: u32 = 1;
pub const PARTICLE_MATERIAL_VERSION: u32 = 1;
pub const COUPLED_SCALAR_SPEC_VERSION: u32 = 1;
pub const COUPLED_TRACE_VERSION: u32 = 1;
pub const SLEEP_TRANSITION_CAUSE_FLUID: u32 = 1 << 10;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleId {
    pub slot: u32,
    pub generation: u32,
}

impl Validate for ParticleId {
    fn validate(&self) -> Result<(), ContractError> {
        if self.generation == 0 {
            return Err(ContractError(
                "particle identity requires a nonzero generation".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ParticleMaterialKindV1 {
    Water,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleMaterialV1 {
    pub version: u32,
    pub id: u32,
    pub kind: ParticleMaterialKindV1,
    pub radius: FiniteF32,
    pub density: FiniteF32,
    pub gravity_scale: FiniteF32,
    pub support_radius_multiplier: FiniteF32,
    pub rest_density: FiniteF32,
    pub pressure_strength: FiniteF32,
    pub near_pressure_strength: FiniteF32,
    pub damping_strength: FiniteF32,
    pub max_pressure_displacement_multiplier: FiniteF32,
}

impl ParticleMaterialV1 {
    pub fn water_v1(id: u32) -> Self {
        Self {
            version: PARTICLE_MATERIAL_VERSION,
            id,
            kind: ParticleMaterialKindV1::Water,
            radius: finite(0.1),
            density: finite(1.0),
            gravity_scale: finite(1.0),
            support_radius_multiplier: finite(3.0),
            rest_density: finite(0.45),
            pressure_strength: finite(140.0),
            near_pressure_strength: finite(260.0),
            damping_strength: finite(0.18),
            max_pressure_displacement_multiplier: finite(0.75),
        }
    }

    pub fn particle_mass(self) -> f32 {
        self.density.get() * std::f32::consts::PI * self.radius.get() * self.radius.get()
    }

    pub fn support_radius(self) -> f32 {
        self.radius.get() * self.support_radius_multiplier.get()
    }
}

impl Validate for ParticleMaterialV1 {
    fn validate(&self) -> Result<(), ContractError> {
        for value in [
            self.radius,
            self.density,
            self.gravity_scale,
            self.support_radius_multiplier,
            self.rest_density,
            self.pressure_strength,
            self.near_pressure_strength,
            self.damping_strength,
            self.max_pressure_displacement_multiplier,
        ] {
            value.validate()?;
        }
        if self.version != PARTICLE_MATERIAL_VERSION
            || self.radius.get() <= 0.0
            || self.density.get() <= 0.0
            || self.gravity_scale.get() < 0.0
            || self.support_radius_multiplier.get() <= 2.0
            || self.rest_density.get() < 0.0
            || self.pressure_strength.get() < 0.0
            || self.near_pressure_strength.get() < 0.0
            || !(0.0..=1.0).contains(&self.damping_strength.get())
            || self.max_pressure_displacement_multiplier.get() <= 0.0
        {
            return Err(ContractError(
                "invalid version-1 particle material parameters".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleRegionV1 {
    pub id: u32,
    pub material_id: u32,
    pub lower: Vec2,
    pub upper: Vec2,
    pub spacing: FiniteF32,
    pub initial_velocity: Vec2,
}

impl Validate for ParticleRegionV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.lower.validate()?;
        self.upper.validate()?;
        self.spacing.validate()?;
        self.initial_velocity.validate()?;
        if self.lower.x >= self.upper.x || self.lower.y >= self.upper.y || self.spacing.get() <= 0.0
        {
            return Err(ContractError(
                "particle region requires ordered bounds and positive spacing".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoupledScheduleV1 {
    FluidAfterEachRigidSubstep,
    RigidThenCoupledBatch,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoupledSettingsV1 {
    pub version: u32,
    pub schedule: CoupledScheduleV1,
    pub pressure_iterations: u32,
    pub coupling_iterations: u32,
}

impl CoupledSettingsV1 {
    pub const fn initial() -> Self {
        Self {
            version: COUPLED_CONTRACT_VERSION,
            schedule: CoupledScheduleV1::FluidAfterEachRigidSubstep,
            pressure_iterations: 2,
            coupling_iterations: 1,
        }
    }
}

impl Validate for CoupledSettingsV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.version != COUPLED_CONTRACT_VERSION
            || !(1..=8).contains(&self.pressure_iterations)
            || !(1..=8).contains(&self.coupling_iterations)
        {
            return Err(ContractError(
                "coupled settings require version 1 and one through eight iterations".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum CoupledActionV1 {
    EmitCircle {
        step: u32,
        center: Vec2,
        radius: FiniteF32,
        initial_velocity: Vec2,
        material_id: u32,
    },
    EraseCircle {
        step: u32,
        center: Vec2,
        radius: FiniteF32,
    },
}

impl CoupledActionV1 {
    pub const fn step(self) -> u32 {
        match self {
            Self::EmitCircle { step, .. } | Self::EraseCircle { step, .. } => step,
        }
    }
}

impl Validate for CoupledActionV1 {
    fn validate(&self) -> Result<(), ContractError> {
        match self {
            Self::EmitCircle {
                center,
                radius,
                initial_velocity,
                ..
            } => {
                center.validate()?;
                radius.validate()?;
                initial_velocity.validate()?;
                if radius.get() <= 0.0 {
                    return Err(ContractError(
                        "particle emission radius must be positive".to_owned(),
                    ));
                }
            }
            Self::EraseCircle { center, radius, .. } => {
                center.validate()?;
                radius.validate()?;
                if radius.get() <= 0.0 {
                    return Err(ContractError(
                        "particle erase radius must be positive".to_owned(),
                    ));
                }
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoupledScenarioV1 {
    pub schema_version: u32,
    pub contract_version: u32,
    pub id: String,
    pub seed: u64,
    pub requested_steps: u32,
    pub particle_capacity: u32,
    pub rigid: ScenarioV1,
    pub settings: CoupledSettingsV1,
    pub materials: Vec<ParticleMaterialV1>,
    pub regions: Vec<ParticleRegionV1>,
    pub actions: Vec<CoupledActionV1>,
}

impl Validate for CoupledScenarioV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.contract_version != COUPLED_CONTRACT_VERSION
            || self.id.is_empty()
            || self.requested_steps == 0
            || self.particle_capacity == 0
            || self.rigid.requested_steps != self.requested_steps
        {
            return Err(ContractError(
                "invalid coupled scenario identity, capacity, or rigid step count".to_owned(),
            ));
        }
        self.rigid.validate()?;
        self.settings.validate()?;
        validate_sorted_unique(
            self.materials.iter().map(|value| value.id),
            "particle materials",
        )?;
        for material in &self.materials {
            material.validate()?;
        }
        validate_sorted_unique(
            self.regions.iter().map(|value| value.id),
            "particle regions",
        )?;
        for region in &self.regions {
            region.validate()?;
            if self
                .materials
                .binary_search_by_key(&region.material_id, |value| value.id)
                .is_err()
            {
                return Err(ContractError(format!(
                    "particle region {} references missing material {}",
                    region.id, region.material_id
                )));
            }
        }
        let mut previous_step = 0;
        for (index, action) in self.actions.iter().enumerate() {
            action.validate()?;
            let step = action.step();
            if step >= self.requested_steps || (index > 0 && step < previous_step) {
                return Err(ContractError(
                    "coupled actions must be step-sorted inside the requested window".to_owned(),
                ));
            }
            if let CoupledActionV1::EmitCircle { material_id, .. } = action
                && self
                    .materials
                    .binary_search_by_key(material_id, |value| value.id)
                    .is_err()
            {
                return Err(ContractError(format!(
                    "particle action references missing material {material_id}"
                )));
            }
            previous_step = step;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoupledOracleRequestV1 {
    pub schema_version: u32,
    pub run_id: String,
    pub source_scenario_hash: String,
    pub scenario: CoupledScenarioV1,
}

impl Validate for CoupledOracleRequestV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.run_id.is_empty() || !is_sha256(&self.source_scenario_hash) {
            return Err(ContractError(
                "coupled oracle request requires run id and source scenario SHA-256".to_owned(),
            ));
        }
        self.scenario.validate()?;
        if canonical_hash(&self.scenario)? != self.source_scenario_hash {
            return Err(ContractError(
                "coupled source scenario hash mismatch".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleStateV1 {
    pub layout_version: u32,
    pub id: ParticleId,
    pub material_id: u32,
    pub position: Vec2,
    pub velocity: Vec2,
    pub density: FiniteF32,
}

impl Validate for ParticleStateV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.layout_version != PARTICLE_STATE_LAYOUT_VERSION {
            return Err(ContractError(format!(
                "unsupported particle-state layout version {}",
                self.layout_version
            )));
        }
        self.id.validate()?;
        self.position.validate()?;
        self.velocity.validate()?;
        self.density.validate()?;
        if self.density.get() < 0.0 {
            return Err(ContractError(
                "particle density must be nonnegative".to_owned(),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CoupledTraceStageV1 {
    Integrate,
    Neighborhood,
    Density,
    Pressure,
    ParticleBodyContact,
    BodyGather,
    Wake,
    VelocityReconstruction,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoupledTraceRecordV1 {
    pub version: u32,
    pub step: u32,
    pub iteration: u32,
    pub stage: CoupledTraceStageV1,
    pub particle: Option<ParticleId>,
    pub body: Option<BodyId>,
    pub values: Vec<crate::TraceValueV1>,
}

impl Validate for CoupledTraceRecordV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.version != COUPLED_TRACE_VERSION {
            return Err(ContractError(
                "unsupported coupled trace version".to_owned(),
            ));
        }
        if let Some(particle) = self.particle {
            particle.validate()?;
        }
        if let Some(body) = self.body
            && body.generation == 0
        {
            return Err(ContractError(
                "coupled trace body requires a nonzero generation".to_owned(),
            ));
        }
        validate_sorted_unique(
            self.values.iter().map(|value| &value.name),
            "coupled trace values",
        )?;
        for value in &self.values {
            value.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoupledWorkVectorV1 {
    pub particles_capacity: u32,
    pub particles_active: u32,
    pub occupied_cells: u32,
    pub neighbor_checks: u64,
    pub accepted_neighbors: u64,
    pub density_constraints: u64,
    pub pressure_iterations: u64,
    pub particle_body_contacts: u64,
    pub body_contribution_edges: u64,
    pub fluid_wake_roots: u32,
    pub dispatches: u32,
}

impl Validate for CoupledWorkVectorV1 {
    fn validate(&self) -> Result<(), ContractError> {
        if self.particles_active > self.particles_capacity
            || self.accepted_neighbors > self.neighbor_checks
            || self.body_contribution_edges < self.particle_body_contacts
            || u64::from(self.fluid_wake_roots) > self.particle_body_contacts
        {
            return Err(ContractError("invalid coupled work vector".to_owned()));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoupledMetricsV1 {
    pub work: CoupledWorkVectorV1,
    pub capacities: Vec<CapacityRecordV1>,
    pub density_error_rms: FiniteF32,
    pub fluid_motion_rms: FiniteF32,
    pub equal_opposite_residual: FiniteF32,
    pub escaped_particles: u32,
    pub killed_particles: u32,
}

impl Validate for CoupledMetricsV1 {
    fn validate(&self) -> Result<(), ContractError> {
        self.work.validate()?;
        validate_sorted_unique(
            self.capacities.iter().map(|value| &value.class),
            "coupled capacities",
        )?;
        for capacity in &self.capacities {
            capacity.validate()?;
            if capacity.overflowed {
                return Err(ContractError(
                    "accepted coupled metrics cannot contain capacity overflow".to_owned(),
                ));
            }
        }
        for value in [
            self.density_error_rms,
            self.fluid_motion_rms,
            self.equal_opposite_residual,
        ] {
            value.validate()?;
            if value.get() < 0.0 {
                return Err(ContractError(
                    "coupled metric magnitudes must be nonnegative".to_owned(),
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CoupledOracleResultV1 {
    pub schema_version: u32,
    pub contract_version: u32,
    pub run_id: String,
    pub source_scenario_hash: String,
    pub projection_hash: String,
    pub provenance: ProvenanceV1,
    pub completed_steps: u32,
    pub bodies: Vec<BodyStateV1>,
    pub particles: Vec<ParticleStateV1>,
    pub traces: Vec<CoupledTraceRecordV1>,
    pub metrics: CoupledMetricsV1,
    pub failure: Option<FailureV1>,
}

impl Validate for CoupledOracleResultV1 {
    fn validate(&self) -> Result<(), ContractError> {
        version(self.schema_version)?;
        if self.contract_version != COUPLED_CONTRACT_VERSION
            || self.run_id.is_empty()
            || !is_sha256(&self.source_scenario_hash)
            || !is_sha256(&self.projection_hash)
            || self.completed_steps == 0
        {
            return Err(ContractError(
                "invalid coupled result identity or completed-step count".to_owned(),
            ));
        }
        self.provenance.validate()?;
        validate_sorted_unique(
            self.bodies.iter().map(|body| body.id),
            "coupled result bodies",
        )?;
        for body in &self.bodies {
            body.validate()?;
        }
        validate_sorted_unique(
            self.particles.iter().map(|particle| particle.id),
            "coupled result particles",
        )?;
        for particle in &self.particles {
            particle.validate()?;
        }
        for trace in &self.traces {
            trace.validate()?;
        }
        self.metrics.validate()?;
        if let Some(failure) = &self.failure {
            failure.validate()?;
        }
        Ok(())
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleIntegrationInputV1 {
    pub position: Vec2,
    pub velocity: Vec2,
    pub gravity: Vec2,
    pub gravity_scale: f32,
    pub dt: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleIntegrationOutputV1 {
    pub position: Vec2,
    pub velocity: Vec2,
}

pub fn integrate_particle_v1(
    input: ParticleIntegrationInputV1,
) -> Result<ParticleIntegrationOutputV1, ContractError> {
    input.position.validate()?;
    input.velocity.validate()?;
    input.gravity.validate()?;
    finite_f32("particle gravity scale", input.gravity_scale)?;
    finite_f32("particle dt", input.dt)?;
    if input.gravity_scale < 0.0 || input.dt <= 0.0 {
        return Err(ContractError(
            "particle integration requires nonnegative gravity scale and positive dt".to_owned(),
        ));
    }
    let velocity = add(
        input.velocity,
        scale(input.gravity, input.gravity_scale * input.dt),
    );
    let position = add(input.position, scale(velocity, input.dt));
    Ok(ParticleIntegrationOutputV1 { position, velocity })
}

pub fn particle_kernel_weight_v1(distance: f32, support_radius: f32) -> Result<f32, ContractError> {
    finite_f32("particle distance", distance)?;
    finite_f32("particle support radius", support_radius)?;
    if distance < 0.0 || support_radius <= 0.0 {
        return Err(ContractError(
            "particle kernel requires nonnegative distance and positive support radius".to_owned(),
        ));
    }
    Ok((1.0 - distance / support_radius).clamp(0.0, 1.0))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticlePairProjectionInputV1 {
    pub position_a: Vec2,
    pub position_b: Vec2,
    pub density_a: f32,
    pub near_density_a: f32,
    pub density_b: f32,
    pub near_density_b: f32,
    pub material: ParticleMaterialV1,
    pub dt: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticlePairProjectionOutputV1 {
    pub position_a: Vec2,
    pub position_b: Vec2,
    pub displacement: Vec2,
    pub weight: f32,
}

pub fn project_particle_pair_v1(
    input: ParticlePairProjectionInputV1,
) -> Result<ParticlePairProjectionOutputV1, ContractError> {
    input.material.validate()?;
    input.position_a.validate()?;
    input.position_b.validate()?;
    for (name, value) in [
        ("density a", input.density_a),
        ("near density a", input.near_density_a),
        ("density b", input.density_b),
        ("near density b", input.near_density_b),
        ("pair projection dt", input.dt),
    ] {
        finite_f32(name, value)?;
    }
    if input.density_a < 0.0
        || input.near_density_a < 0.0
        || input.density_b < 0.0
        || input.near_density_b < 0.0
        || input.dt <= 0.0
    {
        return Err(ContractError(
            "particle pair projection requires nonnegative densities and positive dt".to_owned(),
        ));
    }
    let delta = sub(input.position_b, input.position_a);
    let distance = length(delta);
    let weight = particle_kernel_weight_v1(distance, input.material.support_radius())?;
    if weight == 0.0 {
        return Ok(ParticlePairProjectionOutputV1 {
            position_a: input.position_a,
            position_b: input.position_b,
            displacement: Vec2::ZERO,
            weight,
        });
    }
    let normal = normalize_or(delta, Vec2::new(1.0, 0.0));
    let pressure_a = input.material.pressure_strength.get()
        * (input.density_a - input.material.rest_density.get()).max(0.0);
    let pressure_b = input.material.pressure_strength.get()
        * (input.density_b - input.material.rest_density.get()).max(0.0);
    let near_pressure_a = input.material.near_pressure_strength.get() * input.near_density_a;
    let near_pressure_b = input.material.near_pressure_strength.get() * input.near_density_b;
    let magnitude = (input.dt
        * input.dt
        * ((pressure_a + pressure_b) * weight
            + (near_pressure_a + near_pressure_b) * weight * weight))
        .min(
            input.material.radius.get() * input.material.max_pressure_displacement_multiplier.get(),
        )
        .max(0.0);
    let displacement = scale(normal, magnitude * 0.5);
    Ok(ParticlePairProjectionOutputV1 {
        position_a: sub(input.position_a, displacement),
        position_b: add(input.position_b, displacement),
        displacement,
        weight,
    })
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleBoundaryQueryV1 {
    pub particle_position: Vec2,
    pub particle_radius: f32,
    pub body_position: Vec2,
    pub body_angle: f32,
    pub shape: ShapeV1,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleBoundaryContactV1 {
    pub point: Vec2,
    pub normal: Vec2,
    pub signed_distance: f32,
    pub correction: f32,
}

pub fn particle_boundary_contact_v1(
    query: ParticleBoundaryQueryV1,
) -> Result<Option<ParticleBoundaryContactV1>, ContractError> {
    query.particle_position.validate()?;
    query.body_position.validate()?;
    query.shape.validate()?;
    finite_f32("particle radius", query.particle_radius)?;
    finite_f32("body angle", query.body_angle)?;
    if query.particle_radius <= 0.0 {
        return Err(ContractError(
            "particle boundary query requires positive particle radius".to_owned(),
        ));
    }
    let local = rotate_inverse(
        sub(query.particle_position, query.body_position),
        query.body_angle,
    );
    let (local_point, local_normal, signed_distance) = match query.shape {
        ShapeV1::Circle { radius } => {
            let distance = length(local);
            let normal = normalize_or(local, Vec2::new(0.0, 1.0));
            (scale(normal, radius.get()), normal, distance - radius.get())
        }
        ShapeV1::Rectangle { half_extents } => rectangle_distance(local, half_extents),
    };
    if signed_distance > query.particle_radius {
        return Ok(None);
    }
    let normal = rotate(local_normal, query.body_angle);
    Ok(Some(ParticleBoundaryContactV1 {
        point: add(query.body_position, rotate(local_point, query.body_angle)),
        normal,
        signed_distance,
        correction: query.particle_radius - signed_distance,
    }))
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleBodyContactInputV1 {
    pub particle_velocity: Vec2,
    pub particle_inverse_mass: f32,
    pub body_linear_velocity: Vec2,
    pub body_angular_velocity: f32,
    pub body_inverse_mass: f32,
    pub body_inverse_inertia: f32,
    pub body_center: Vec2,
    pub contact_point: Vec2,
    pub normal: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParticleBodyContactResponseV1 {
    pub normal_impulse: f32,
    pub particle_delta_velocity: Vec2,
    pub body_delta_velocity: Vec2,
    pub body_delta_angular_velocity: f32,
    pub linear_impulse_residual: Vec2,
}

pub fn solve_particle_body_contact_v1(
    input: ParticleBodyContactInputV1,
) -> Result<ParticleBodyContactResponseV1, ContractError> {
    input.particle_velocity.validate()?;
    input.body_linear_velocity.validate()?;
    input.body_center.validate()?;
    input.contact_point.validate()?;
    input.normal.validate()?;
    for (name, value) in [
        ("particle inverse mass", input.particle_inverse_mass),
        ("body angular velocity", input.body_angular_velocity),
        ("body inverse mass", input.body_inverse_mass),
        ("body inverse inertia", input.body_inverse_inertia),
    ] {
        finite_f32(name, value)?;
    }
    if input.particle_inverse_mass <= 0.0
        || input.body_inverse_mass < 0.0
        || input.body_inverse_inertia < 0.0
    {
        return Err(ContractError(
            "particle/body contact requires positive particle inverse mass and nonnegative body inverse properties"
                .to_owned(),
        ));
    }
    let normal_length = length(input.normal);
    if (normal_length - 1.0).abs() > 1.0e-4 {
        return Err(ContractError(
            "particle/body contact normal must be unit length".to_owned(),
        ));
    }
    let lever = sub(input.contact_point, input.body_center);
    let body_point_velocity = add(
        input.body_linear_velocity,
        Vec2::new(
            -input.body_angular_velocity * lever.y,
            input.body_angular_velocity * lever.x,
        ),
    );
    let normal_speed = dot(
        sub(input.particle_velocity, body_point_velocity),
        input.normal,
    );
    let rotational = cross(lever, input.normal);
    let inverse_effective_mass = input.particle_inverse_mass
        + input.body_inverse_mass
        + input.body_inverse_inertia * rotational * rotational;
    let normal_impulse = if normal_speed < 0.0 && inverse_effective_mass > 0.0 {
        -normal_speed / inverse_effective_mass
    } else {
        0.0
    };
    let particle_impulse = scale(input.normal, normal_impulse);
    let body_impulse = scale(particle_impulse, -1.0);
    let particle_delta_velocity = scale(particle_impulse, input.particle_inverse_mass);
    let body_delta_velocity = scale(body_impulse, input.body_inverse_mass);
    let body_delta_angular_velocity = input.body_inverse_inertia * cross(lever, body_impulse);
    Ok(ParticleBodyContactResponseV1 {
        normal_impulse,
        particle_delta_velocity,
        body_delta_velocity,
        body_delta_angular_velocity,
        linear_impulse_residual: add(particle_impulse, body_impulse),
    })
}

fn rectangle_distance(local: Vec2, half: Vec2) -> (Vec2, Vec2, f32) {
    let clamped = Vec2::new(
        local.x.clamp(-half.x, half.x),
        local.y.clamp(-half.y, half.y),
    );
    let outside = sub(local, clamped);
    let outside_distance = length(outside);
    if outside_distance > 1.0e-6 {
        return (
            clamped,
            scale(outside, 1.0 / outside_distance),
            outside_distance,
        );
    }
    let x_depth = half.x - local.x.abs();
    let y_depth = half.y - local.y.abs();
    if x_depth < y_depth {
        let sign = if local.x < 0.0 { -1.0 } else { 1.0 };
        (
            Vec2::new(sign * half.x, local.y),
            Vec2::new(sign, 0.0),
            -x_depth,
        )
    } else {
        let sign = if local.y < 0.0 { -1.0 } else { 1.0 };
        (
            Vec2::new(local.x, sign * half.y),
            Vec2::new(0.0, sign),
            -y_depth,
        )
    }
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x + b.x, a.y + b.y)
}

fn sub(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x - b.x, a.y - b.y)
}

fn scale(value: Vec2, factor: f32) -> Vec2 {
    Vec2::new(value.x * factor, value.y * factor)
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn length(value: Vec2) -> f32 {
    dot(value, value).sqrt()
}

fn normalize_or(value: Vec2, fallback: Vec2) -> Vec2 {
    let magnitude = length(value);
    if magnitude > 1.0e-6 {
        scale(value, 1.0 / magnitude)
    } else {
        fallback
    }
}

fn rotate(value: Vec2, angle: f32) -> Vec2 {
    let (sin, cos) = angle.sin_cos();
    Vec2::new(cos * value.x - sin * value.y, sin * value.x + cos * value.y)
}

fn rotate_inverse(value: Vec2, angle: f32) -> Vec2 {
    rotate(value, -angle)
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("built-in coupled constant is finite")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{MaterialV1, SCHEMA_VERSION, SolverSettingsV1};

    #[test]
    fn water_material_matches_frozen_sidecar_seed() {
        let material = ParticleMaterialV1::water_v1(0);
        material.validate().unwrap();
        assert_eq!(material.radius.get().to_bits(), 0.1f32.to_bits());
        assert_eq!(material.support_radius().to_bits(), 0.3f32.to_bits());
        assert!((material.particle_mass() - std::f32::consts::PI * 0.01).abs() < 1.0e-7);
    }

    #[test]
    fn integration_uses_semi_implicit_velocity() {
        let output = integrate_particle_v1(ParticleIntegrationInputV1 {
            position: Vec2::new(1.0, 2.0),
            velocity: Vec2::new(3.0, 4.0),
            gravity: Vec2::new(0.0, -10.0),
            gravity_scale: 0.5,
            dt: 0.1,
        })
        .unwrap();
        assert_eq!(output.velocity, Vec2::new(3.0, 3.5));
        assert_eq!(output.position, Vec2::new(1.3, 2.35));
    }

    #[test]
    fn pair_projection_is_symmetric_and_bounded() {
        let input = ParticlePairProjectionInputV1 {
            position_a: Vec2::new(-0.05, 0.0),
            position_b: Vec2::new(0.05, 0.0),
            density_a: 1.0,
            near_density_a: 0.5,
            density_b: 1.0,
            near_density_b: 0.5,
            material: ParticleMaterialV1::water_v1(0),
            dt: 1.0 / 60.0,
        };
        let output = project_particle_pair_v1(input).unwrap();
        let center_before = scale(add(input.position_a, input.position_b), 0.5);
        let center_after = scale(add(output.position_a, output.position_b), 0.5);
        assert!((center_before.x - center_after.x).abs() < 1.0e-7);
        assert!((center_before.y - center_after.y).abs() < 1.0e-7);
        assert!(length(output.displacement) <= 0.075 + 1.0e-7);
    }

    #[test]
    fn rotated_rectangle_boundary_returns_world_normal() {
        let contact = particle_boundary_contact_v1(ParticleBoundaryQueryV1 {
            particle_position: Vec2::new(0.0, 0.9),
            particle_radius: 0.1,
            body_position: Vec2::ZERO,
            body_angle: std::f32::consts::FRAC_PI_2,
            shape: ShapeV1::Rectangle {
                half_extents: Vec2::new(1.0, 0.5),
            },
        })
        .unwrap()
        .unwrap();
        assert!(contact.normal.y > 0.999);
        assert!(contact.normal.x.abs() < 1.0e-5);
    }

    #[test]
    fn contact_response_is_equal_opposite_and_includes_torque() {
        let response = solve_particle_body_contact_v1(ParticleBodyContactInputV1 {
            particle_velocity: Vec2::new(-2.0, 0.0),
            particle_inverse_mass: 2.0,
            body_linear_velocity: Vec2::ZERO,
            body_angular_velocity: 0.0,
            body_inverse_mass: 0.5,
            body_inverse_inertia: 1.0,
            body_center: Vec2::ZERO,
            contact_point: Vec2::new(0.0, 1.0),
            normal: Vec2::new(1.0, 0.0),
        })
        .unwrap();
        assert!(response.normal_impulse > 0.0);
        assert_eq!(response.linear_impulse_residual, Vec2::ZERO);
        assert!(response.body_delta_angular_velocity > 0.0);
    }

    #[test]
    fn coupled_request_hash_is_self_authenticating() {
        let rigid = ScenarioV1 {
            schema_version: SCHEMA_VERSION,
            id: "coupled-contract-rigid".to_owned(),
            seed: 31,
            requested_steps: 60,
            settings: SolverSettingsV1 {
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
            bodies: vec![BodyStateV1 {
                id: BodyId {
                    slot: 0,
                    generation: 1,
                },
                shape: ShapeV1::Rectangle {
                    half_extents: Vec2::new(5.0, 0.5),
                },
                position: Vec2::new(0.0, -0.5),
                angle: finite(0.0),
                linear_velocity: Vec2::ZERO,
                angular_velocity: finite(0.0),
                inverse_mass: finite(0.0),
                inverse_inertia: finite(0.0),
                material: MaterialV1 {
                    friction: finite(0.2),
                    restitution: finite(0.0),
                },
                sleeping: false,
            }],
            manifolds: Vec::new(),
            contact_cache: Vec::new(),
            actions: Vec::new(),
        };
        let scenario = CoupledScenarioV1 {
            schema_version: SCHEMA_VERSION,
            contract_version: COUPLED_CONTRACT_VERSION,
            id: "coupled-contract".to_owned(),
            seed: 31,
            requested_steps: 60,
            particle_capacity: 4_096,
            rigid,
            settings: CoupledSettingsV1::initial(),
            materials: vec![ParticleMaterialV1::water_v1(0)],
            regions: vec![ParticleRegionV1 {
                id: 0,
                material_id: 0,
                lower: Vec2::new(-1.0, 0.0),
                upper: Vec2::new(1.0, 2.0),
                spacing: finite(0.2),
                initial_velocity: Vec2::ZERO,
            }],
            actions: Vec::new(),
        };
        let hash = canonical_hash(&scenario).unwrap();
        CoupledOracleRequestV1 {
            schema_version: SCHEMA_VERSION,
            run_id: "coupled-request".to_owned(),
            source_scenario_hash: hash,
            scenario,
        }
        .validate()
        .unwrap();
    }
}
