use std::fmt::{Display, Formatter};
use std::mem::size_of;
use std::sync::{Arc, Mutex};

use physbox_contracts::{
    BodyCommandBatchV1, BodyId as ContractBodyId, BodyStateV1, ConstructionIntentBatchV1,
    ConstructionIntentV1, ConstructionResultV1, ContactPolicyV1, ContactSoftnessPolicyV1,
    FiniteF32, GRAB_INTENT_CONTRACT_VERSION, GrabIntentBatchV1, MaterialV1, SCHEMA_VERSION,
    ScenarioV1, ShapeV1, SolverSettingsV1, Vec2,
};

use crate::body_world::{BodyCommandStepRange, BodyStepPlan, GpuBodyWorld};
use crate::construction::CONSTRUCTION_INTENT_CAPACITY;
use crate::{
    BodyDef, BodyId, BodyType, RenderView, ShapeDef, ShapeGeometry, ShapeId, SolverProfileName,
    StepDef, WorldDef,
};

const BODY_POSE_BYTES: u64 = size_of::<[f32; 4]>() as u64;

#[derive(Clone, Debug)]
struct BodyRecord {
    id: BodyId,
    body: BodyDef,
    shape: Option<ShapeDef>,
    live: bool,
}

/// A caller-driven GPU-resident physics world.
pub struct World {
    definition: WorldDef,
    bodies: Vec<Option<BodyRecord>>,
    generations: Vec<u32>,
    inner: Option<GpuBodyWorld>,
    queued_construction: Vec<ConstructionIntentV1>,
    next_sequence: u32,
    outer_step: u64,
}

impl World {
    pub fn new(device: &wgpu::Device, definition: WorldDef) -> Result<Self, WorldError> {
        definition.validate().map_err(WorldError::definition)?;
        let capacity = definition.capacity.slots();
        let required_pose_bytes = u64::from(capacity) * BODY_POSE_BYTES;
        if required_pose_bytes > device.limits().max_buffer_size {
            return Err(WorldError::new(format!(
                "body capacity requires {required_pose_bytes} pose bytes, exceeding this device"
            )));
        }
        Ok(Self {
            definition,
            bodies: vec![None; capacity as usize],
            generations: vec![0; capacity as usize],
            inner: None,
            queued_construction: Vec::new(),
            next_sequence: 1,
            outer_step: 0,
        })
    }

    pub const fn definition(&self) -> WorldDef {
        self.definition
    }

    pub fn create_body(&mut self, body: BodyDef) -> Result<BodyId, WorldError> {
        body.validate().map_err(WorldError::definition)?;
        let slot = self
            .bodies
            .iter()
            .position(Option::is_none)
            .ok_or_else(|| WorldError::new("body capacity is full"))?;
        let generation = self.generations[slot]
            .checked_add(1)
            .ok_or_else(|| WorldError::new("body generation is exhausted"))?;
        self.generations[slot] = generation;
        let id = BodyId::from_parts(slot as u32, generation);
        self.bodies[slot] = Some(BodyRecord {
            id,
            body,
            shape: None,
            live: true,
        });
        Ok(id)
    }

    pub fn create_shape(&mut self, body: BodyId, shape: ShapeDef) -> Result<ShapeId, WorldError> {
        let initialized = self.inner.is_some();
        let body_definition = {
            let record = self.body_record(body)?;
            if record.shape.is_some() {
                return Err(WorldError::new(
                    "the current rigid world supports one shape per body",
                ));
            }
            record.body
        };
        shape
            .validate(body_definition.body_type)
            .map_err(WorldError::definition)?;
        if initialized {
            validate_runtime_construction(body_definition, shape)?;
            let sequence = self.take_sequence()?;
            let contract_body = contract_body_id(body);
            let world_target = vec2(body_definition.position);
            let intent = match shape.geometry {
                ShapeGeometry::Circle(_) => ConstructionIntentV1::PlaceCircleBody {
                    sequence,
                    body: Some(contract_body),
                    world_target,
                    result: ConstructionResultV1::Placed,
                },
                ShapeGeometry::Rectangle(_) => ConstructionIntentV1::PlaceRectangleBody {
                    sequence,
                    body: Some(contract_body),
                    world_target,
                    result: ConstructionResultV1::Placed,
                },
            };
            self.push_construction(intent)?;
        }
        self.body_record_mut(body)?.shape = Some(shape);
        Ok(ShapeId::from_body(body))
    }

    pub fn destroy_body(&mut self, body: BodyId) -> Result<(), WorldError> {
        self.body_record(body)?;
        if self.inner.is_some() {
            let sequence = self.take_sequence()?;
            self.push_construction(ConstructionIntentV1::EraseBody {
                sequence,
                body: Some(contract_body_id(body)),
                world_target: Vec2::ZERO,
            })?;
        }
        let record = self.body_record_mut(body)?;
        record.live = false;
        self.bodies[body.slot() as usize] = None;
        Ok(())
    }

    pub fn destroy_shape(&mut self, shape: ShapeId) -> Result<(), WorldError> {
        self.destroy_body(shape.body())
    }

    pub fn step(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        step: StepDef,
    ) -> Result<StepReport, WorldError> {
        let expected = self.definition.quality.physics.fixed_dt_seconds();
        if !step.delta_seconds.is_finite()
            || step.delta_seconds <= 0.0
            || (step.delta_seconds - expected).abs() > 1.0e-6
        {
            return Err(WorldError::new(format!(
                "step delta must match configured fixed dt {expected}"
            )));
        }
        if self.inner.is_none() {
            let scenario = self.initial_scenario()?;
            self.inner = Some(
                GpuBodyWorld::new_generated_with_configuration(
                    device,
                    &scenario,
                    self.definition.capacity,
                    self.definition.quality.physics,
                )
                .map_err(WorldError::new)?,
            );
        }
        let queued_operations = self.queued_construction.len() as u32;
        let body_commands = empty_body_commands();
        let grab = empty_grab_intents();
        let construction = ConstructionIntentBatchV1 {
            schema_version: SCHEMA_VERSION,
            contract_version: physbox_contracts::CONSTRUCTION_INTENT_CONTRACT_VERSION,
            capacity_intents: CONSTRUCTION_INTENT_CAPACITY,
            attempted_intents: self.queued_construction.len() as u64,
            overflowed: false,
            intents: self.queued_construction.clone(),
        };
        let empty_range = [BodyCommandStepRange { start: 0, count: 0 }];
        let construction_range = [BodyCommandStepRange {
            start: 0,
            count: queued_operations,
        }];
        self.inner
            .as_ref()
            .expect("world was initialized above")
            .encode_steps_with_grab_intents(
                queue,
                encoder,
                1,
                BodyStepPlan {
                    batch: &body_commands,
                    ranges: &empty_range,
                    grab_batch: &grab,
                    grab_ranges: &empty_range,
                    construction_batch: &construction,
                    construction_ranges: &construction_range,
                    particle_commands: &[],
                    particle_ranges: &empty_range,
                    frame_timestamps: None,
                },
            )
            .map_err(WorldError::new)?;
        self.queued_construction.clear();
        self.outer_step = self.outer_step.saturating_add(1);
        Ok(StepReport {
            outer_step: self.outer_step,
            queued_operations,
            live_bodies: self.live_body_count(),
        })
    }

    pub fn render_view(&self) -> Result<RenderView<'_>, WorldError> {
        self.inner
            .as_ref()
            .map(GpuBodyWorld::render_view)
            .ok_or_else(|| WorldError::new("step the world once before requesting a render view"))
    }

    pub fn request_body_snapshot(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<SnapshotRequest, WorldError> {
        let inner = self
            .inner
            .as_ref()
            .ok_or_else(|| WorldError::new("step the world before requesting a snapshot"))?;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox_sim_body_snapshot"),
            size: u64::from(self.definition.capacity.slots()) * BODY_POSE_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        inner.encode_pose_snapshot(encoder, &readback);
        let completion = Arc::new(Mutex::new(None));
        let bodies = self
            .bodies
            .iter()
            .flatten()
            .filter(|body| body.live)
            .map(|body| body.id)
            .collect();
        Ok(SnapshotRequest {
            readback,
            completion,
            bodies,
            mapping_started: false,
        })
    }

    pub fn counters(&self) -> WorldCounters {
        WorldCounters {
            outer_steps: self.outer_step,
            live_bodies: self.live_body_count(),
            capacity_bodies: self.definition.capacity.slots(),
            pending_operations: self.queued_construction.len() as u32,
            gpu_initialized: self.inner.is_some(),
        }
    }

    fn initial_scenario(&self) -> Result<ScenarioV1, WorldError> {
        let bodies = self
            .bodies
            .iter()
            .flatten()
            .filter(|record| record.live)
            .map(body_state)
            .collect::<Result<Vec<_>, _>>()?;
        if bodies.is_empty() {
            return Err(WorldError::new(
                "create at least one body and shape before the first step",
            ));
        }
        Ok(ScenarioV1 {
            schema_version: SCHEMA_VERSION,
            id: "physbox-sim-world".to_owned(),
            seed: 0,
            requested_steps: 1,
            settings: solver_settings(self.definition),
            bodies,
            manifolds: Vec::new(),
            contact_cache: Vec::new(),
            actions: Vec::new(),
        })
    }

    fn body_record(&self, body: BodyId) -> Result<&BodyRecord, WorldError> {
        self.bodies
            .get(body.slot() as usize)
            .and_then(Option::as_ref)
            .filter(|record| record.id == body && record.live)
            .ok_or_else(|| WorldError::new("body handle is stale or unknown"))
    }

    fn body_record_mut(&mut self, body: BodyId) -> Result<&mut BodyRecord, WorldError> {
        self.bodies
            .get_mut(body.slot() as usize)
            .and_then(Option::as_mut)
            .filter(|record| record.id == body && record.live)
            .ok_or_else(|| WorldError::new("body handle is stale or unknown"))
    }

    fn take_sequence(&mut self) -> Result<u32, WorldError> {
        let sequence = self.next_sequence;
        self.next_sequence = self
            .next_sequence
            .checked_add(1)
            .ok_or_else(|| WorldError::new("operation sequence is exhausted"))?;
        Ok(sequence)
    }

    fn push_construction(&mut self, intent: ConstructionIntentV1) -> Result<(), WorldError> {
        if self.queued_construction.len() == CONSTRUCTION_INTENT_CAPACITY as usize {
            return Err(WorldError::new(
                "construction queue capacity is exhausted for this step",
            ));
        }
        self.queued_construction.push(intent);
        Ok(())
    }

    fn live_body_count(&self) -> u32 {
        self.bodies
            .iter()
            .flatten()
            .filter(|body| body.live)
            .count() as u32
    }
}

pub struct SnapshotRequest {
    readback: wgpu::Buffer,
    completion: Arc<Mutex<Option<Result<(), String>>>>,
    bodies: Vec<BodyId>,
    mapping_started: bool,
}

impl SnapshotRequest {
    /// Starts asynchronous mapping after the caller has submitted the encoder
    /// containing the snapshot copy.
    pub fn begin_map(&mut self) -> Result<(), WorldError> {
        if self.mapping_started {
            return Err(WorldError::new("snapshot mapping was already started"));
        }
        self.mapping_started = true;
        let callback_completion = Arc::clone(&self.completion);
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                *callback_completion
                    .lock()
                    .expect("snapshot completion lock is not poisoned") =
                    Some(result.map_err(|error| error.to_string()));
            });
        Ok(())
    }

    /// Returns `None` until the caller's submitted copy has completed and the
    /// device has been polled.
    pub fn try_take(&mut self) -> Option<Result<BodySnapshot, WorldError>> {
        if !self.mapping_started {
            return None;
        }
        let result = self
            .completion
            .lock()
            .expect("snapshot completion lock is not poisoned")
            .take()?;
        if let Err(error) = result {
            return Some(Err(WorldError::new(format!(
                "snapshot mapping failed: {error}"
            ))));
        }
        let bytes = self.readback.slice(..).get_mapped_range();
        let mut values = Vec::with_capacity(self.bodies.len());
        for body in &self.bodies {
            let offset = body.slot() as usize * BODY_POSE_BYTES as usize;
            let pose = bytemuck::pod_read_unaligned::<[f32; 4]>(
                &bytes[offset..offset + BODY_POSE_BYTES as usize],
            );
            values.push(BodySnapshotEntry {
                body: *body,
                position: [pose[0], pose[1]],
                angle: pose[2],
            });
        }
        drop(bytes);
        self.readback.unmap();
        Some(Ok(BodySnapshot { bodies: values }))
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct BodySnapshot {
    pub bodies: Vec<BodySnapshotEntry>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BodySnapshotEntry {
    pub body: BodyId,
    pub position: [f32; 2],
    pub angle: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct StepReport {
    pub outer_step: u64,
    pub queued_operations: u32,
    pub live_bodies: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldCounters {
    pub outer_steps: u64,
    pub live_bodies: u32,
    pub capacity_bodies: u32,
    pub pending_operations: u32,
    pub gpu_initialized: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldError(String);

impl WorldError {
    fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }

    fn definition(error: impl Display) -> Self {
        Self(error.to_string())
    }
}

impl Display for WorldError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for WorldError {}

fn body_state(record: &BodyRecord) -> Result<BodyStateV1, WorldError> {
    let shape = record
        .shape
        .ok_or_else(|| WorldError::new("every body requires a shape before the first step"))?;
    let (shape_contract, area, inverse_inertia_factor) = match shape.geometry {
        ShapeGeometry::Circle(circle) => (
            ShapeV1::Circle {
                radius: finite(circle.radius)?,
            },
            std::f32::consts::PI * circle.radius * circle.radius,
            2.0 / (circle.radius * circle.radius),
        ),
        ShapeGeometry::Rectangle(rectangle) => (
            ShapeV1::Rectangle {
                half_extents: vec2(rectangle.half_extents),
            },
            4.0 * rectangle.half_extents[0] * rectangle.half_extents[1],
            3.0 / (rectangle.half_extents[0] * rectangle.half_extents[0]
                + rectangle.half_extents[1] * rectangle.half_extents[1]),
        ),
    };
    let inverse_mass = if record.body.body_type == BodyType::Static {
        0.0
    } else {
        1.0 / (shape.material.density * area)
    };
    let inverse_inertia = if record.body.body_type == BodyType::Static {
        0.0
    } else {
        inverse_inertia_factor * inverse_mass
    };
    Ok(BodyStateV1 {
        id: contract_body_id(record.id),
        shape: shape_contract,
        position: vec2(record.body.position),
        angle: finite(record.body.angle)?,
        linear_velocity: vec2(record.body.linear_velocity),
        angular_velocity: finite(record.body.angular_velocity)?,
        inverse_mass: finite(inverse_mass)?,
        inverse_inertia: finite(inverse_inertia)?,
        material: MaterialV1 {
            friction: finite(shape.material.friction)?,
            restitution: finite(shape.material.restitution)?,
        },
        sleeping: false,
    })
}

fn solver_settings(definition: WorldDef) -> SolverSettingsV1 {
    let physics = definition.quality.physics;
    SolverSettingsV1 {
        fixed_dt: finite(physics.fixed_dt_seconds()).expect("validated fixed dt is finite"),
        gravity: vec2(definition.gravity),
        substeps: physics.substeps,
        iterations: physics.biased_iterations,
        bias_rate: finite(0.2).expect("constant is finite"),
        mass_coefficient: finite(1.0).expect("constant is finite"),
        impulse_coefficient: finite(0.0).expect("constant is finite"),
        contact_softness: (physics.solver_profile == SolverProfileName::SoftCoupled).then(|| {
            ContactSoftnessPolicyV1 {
                version: physbox_contracts::CONTACT_SOFTNESS_POLICY_VERSION,
                contact_hertz: finite(physics.contact_hertz).expect("validated value is finite"),
                damping_ratio: finite(physics.contact_damping_ratio)
                    .expect("validated value is finite"),
                static_contact_hertz_multiplier: finite(physics.static_contact_hertz_multiplier)
                    .expect("validated value is finite"),
            }
        }),
        restitution_threshold: finite(1.0).expect("constant is finite"),
        contact_policy: Some(ContactPolicyV1 {
            speculative_distance: finite(physics.speculative_distance)
                .expect("validated value is finite"),
            max_depenetration_velocity: finite(physics.max_depenetration_velocity)
                .expect("validated value is finite"),
        }),
    }
}

fn validate_runtime_construction(body: BodyDef, shape: ShapeDef) -> Result<(), WorldError> {
    if body.body_type != BodyType::Dynamic
        || body.angle != 0.0
        || body.linear_velocity != [0.0; 2]
        || body.angular_velocity != 0.0
        || shape.material
            != (crate::MaterialDef {
                density: crate::construction::CONSTRUCTION_DENSITY,
                friction: crate::construction::CONSTRUCTION_FRICTION,
                restitution: crate::construction::CONSTRUCTION_RESTITUTION,
            })
    {
        return Err(WorldError::new(
            "runtime creation currently requires the standard dynamic construction material and zero initial rotation/velocity",
        ));
    }
    let geometry_supported = match shape.geometry {
        ShapeGeometry::Circle(circle) => {
            circle.radius == crate::construction::CONSTRUCTION_CIRCLE_RADIUS
        }
        ShapeGeometry::Rectangle(rectangle) => {
            rectangle.half_extents == crate::construction::CONSTRUCTION_RECTANGLE_HALF_EXTENTS
        }
    };
    if geometry_supported {
        Ok(())
    } else {
        Err(WorldError::new(
            "runtime creation currently supports the standard construction circle and rectangle sizes",
        ))
    }
}

fn empty_body_commands() -> BodyCommandBatchV1 {
    BodyCommandBatchV1 {
        schema_version: SCHEMA_VERSION,
        capacity_commands: crate::body_state::BODY_COMMAND_CAPACITY,
        attempted_commands: 0,
        overflowed: false,
        commands: Vec::new(),
    }
}

fn empty_grab_intents() -> GrabIntentBatchV1 {
    GrabIntentBatchV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: GRAB_INTENT_CONTRACT_VERSION,
        capacity_intents: crate::grab::GRAB_INTENT_CAPACITY,
        attempted_intents: 0,
        overflowed: false,
        intents: Vec::new(),
    }
}

fn contract_body_id(body: BodyId) -> ContractBodyId {
    ContractBodyId {
        slot: body.slot(),
        generation: body.generation(),
    }
}

fn vec2(value: [f32; 2]) -> Vec2 {
    Vec2::new(value[0], value[1])
}

fn finite(value: f32) -> Result<FiniteF32, WorldError> {
    FiniteF32::new(value).map_err(|error| WorldError::new(error.to_string()))
}
