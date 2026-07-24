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
use crate::interaction::{
    BODY_SELECTION_CAPACITY, CommandQueueError, CommandQueueErrorKind, CommandResultRequest,
    PointConstraintDef, PointConstraintId, PointQueryDef, QueuedBodySelection,
    QueuedPointConstraint, QueuedWorldCommand, WORLD_COMMAND_CAPACITY, WORLD_RESULT_BYTES,
    WorldCommandKind, WorldCommandResult, WorldCommandResults, WorldCommandStatus,
    cancelled_result, decode_results,
};
use crate::{
    BodyDef, BodyId, BodyType, ShapeDef, ShapeGeometry, ShapeId, SolverProfileName, StepDef,
    WorldDef,
};

const BODY_POSE_BYTES: u64 = size_of::<[f32; 4]>() as u64;

#[derive(Clone, Debug)]
struct BodyRecord {
    id: BodyId,
    body: BodyDef,
    shape: Option<ShapeDef>,
    live: bool,
}

#[derive(Clone, Copy, Debug)]
struct SelectionReservation {
    id: crate::BodySelectionId,
    consumed: bool,
}

#[derive(Clone, Copy, Debug, Default)]
enum ConstraintState {
    #[default]
    Free,
    Pending {
        id: PointConstraintId,
    },
    Live {
        id: PointConstraintId,
        body: Option<BodyId>,
    },
    DestroyQueued {
        id: PointConstraintId,
        body: Option<BodyId>,
    },
}

impl ConstraintState {
    const fn id(self) -> Option<PointConstraintId> {
        match self {
            Self::Free => None,
            Self::Pending { id } | Self::Live { id, .. } | Self::DestroyQueued { id, .. } => {
                Some(id)
            }
        }
    }

    const fn body(self) -> Option<BodyId> {
        match self {
            Self::Live { body, .. } | Self::DestroyQueued { body, .. } => body,
            Self::Free | Self::Pending { .. } => None,
        }
    }
}

struct InFlightCommands {
    token: u64,
    outer_step: u64,
    commands: Vec<QueuedWorldCommand>,
    result_requested: bool,
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
    queued_world_commands: Vec<QueuedWorldCommand>,
    selections: Vec<Option<SelectionReservation>>,
    selection_generations: Vec<u32>,
    constraint_state: ConstraintState,
    constraint_generation: u32,
    constraint_state_before_queue: Option<ConstraintState>,
    in_flight_commands: Option<InFlightCommands>,
    next_command_id: u32,
    next_result_token: u64,
    command_high_water: u32,
    result_high_water: u32,
    command_rejections: u64,
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
            queued_world_commands: Vec::with_capacity(WORLD_COMMAND_CAPACITY as usize),
            selections: vec![None; BODY_SELECTION_CAPACITY as usize],
            selection_generations: vec![0; BODY_SELECTION_CAPACITY as usize],
            constraint_state: ConstraintState::Free,
            constraint_generation: 0,
            constraint_state_before_queue: None,
            in_flight_commands: None,
            next_command_id: 1,
            next_result_token: 1,
            command_high_water: 0,
            result_high_water: 0,
            command_rejections: 0,
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

    /// Queues a deterministic dynamic-body point query for the next step.
    ///
    /// The returned selection is a bounded deferred handle. It may be passed
    /// immediately to a constraint-creation or selected-body-destruction
    /// command queued for the same step.
    pub fn queue_point_query(
        &mut self,
        query: PointQueryDef,
    ) -> Result<QueuedBodySelection, CommandQueueError> {
        query.validate().map_err(|error| self.reject(error))?;
        self.prepare_command_admission()?;
        let slot = self
            .selections
            .iter()
            .position(Option::is_none)
            .ok_or_else(|| {
                self.reject(CommandQueueError::new(
                    CommandQueueErrorKind::QueueFull,
                    "body-selection capacity is exhausted",
                ))
            })?;
        let generation = self.selection_generations[slot]
            .checked_add(1)
            .ok_or_else(|| {
                self.reject(CommandQueueError::new(
                    CommandQueueErrorKind::IdentifierExhausted,
                    "body-selection generation is exhausted",
                ))
            })?;
        let command = self.take_command_id()?;
        self.selection_generations[slot] = generation;
        let selection = crate::BodySelectionId::from_parts(slot as u32, generation);
        self.selections[slot] = Some(SelectionReservation {
            id: selection,
            consumed: false,
        });
        self.push_world_command(QueuedWorldCommand::PointQuery {
            command,
            selection,
            query,
        });
        Ok(QueuedBodySelection { command, selection })
    }

    /// Queues creation of one retained point constraint from a deferred
    /// selection. The generational constraint handle is reserved
    /// synchronously even though the point query may later miss.
    pub fn queue_create_point_constraint(
        &mut self,
        selection: crate::BodySelectionId,
        definition: PointConstraintDef,
    ) -> Result<QueuedPointConstraint, CommandQueueError> {
        definition.validate().map_err(|error| self.reject(error))?;
        self.prepare_command_admission()?;
        self.consume_selection(selection)?;
        if !matches!(self.constraint_state, ConstraintState::Free) {
            self.restore_selection_consumption(selection);
            return Err(self.reject(CommandQueueError::new(
                CommandQueueErrorKind::InvalidConstraint,
                "the current world supports one live or pending point constraint",
            )));
        }
        let generation = match self.constraint_generation.checked_add(1) {
            Some(generation) => generation,
            None => {
                self.restore_selection_consumption(selection);
                return Err(self.reject(CommandQueueError::new(
                    CommandQueueErrorKind::IdentifierExhausted,
                    "point-constraint generation is exhausted",
                )));
            }
        };
        let command = match self.take_command_id() {
            Ok(command) => command,
            Err(error) => {
                self.restore_selection_consumption(selection);
                return Err(error);
            }
        };
        let constraint = PointConstraintId::from_parts(0, generation);
        self.constraint_generation = generation;
        self.constraint_state = ConstraintState::Pending { id: constraint };
        self.push_world_command(QueuedWorldCommand::CreatePointConstraint {
            command,
            selection,
            constraint,
            definition,
        });
        Ok(QueuedPointConstraint {
            command,
            constraint,
        })
    }

    /// Queues a new world-space target for a live or same-step-pending point
    /// constraint.
    pub fn queue_set_point_constraint_target(
        &mut self,
        constraint: PointConstraintId,
        target: [f32; 2],
    ) -> Result<crate::CommandId, CommandQueueError> {
        PointQueryDef::at(target)
            .validate()
            .map_err(|error| self.reject(error))?;
        self.prepare_command_admission()?;
        self.validate_constraint_for_update(constraint)?;
        let command = self.take_command_id()?;
        self.push_world_command(QueuedWorldCommand::SetPointConstraintTarget {
            command,
            constraint,
            target,
        });
        Ok(command)
    }

    /// Queues destruction of a live or same-step-pending point constraint.
    pub fn queue_destroy_point_constraint(
        &mut self,
        constraint: PointConstraintId,
    ) -> Result<crate::CommandId, CommandQueueError> {
        self.prepare_command_admission()?;
        self.validate_constraint_for_update(constraint)?;
        let body = self.constraint_state.body();
        let command = self.take_command_id()?;
        self.constraint_state = ConstraintState::DestroyQueued {
            id: constraint,
            body,
        };
        self.push_world_command(QueuedWorldCommand::DestroyPointConstraint {
            command,
            constraint,
        });
        Ok(command)
    }

    /// Queues destruction of the exact dynamic body produced by a deferred
    /// selection.
    pub fn queue_destroy_selected_body(
        &mut self,
        selection: crate::BodySelectionId,
    ) -> Result<crate::CommandId, CommandQueueError> {
        self.prepare_command_admission()?;
        self.consume_selection(selection)?;
        let command = match self.take_command_id() {
            Ok(command) => command,
            Err(error) => {
                self.restore_selection_consumption(selection);
                return Err(error);
            }
        };
        self.push_world_command(QueuedWorldCommand::DestroySelectedBody { command, selection });
        Ok(command)
    }

    /// Cancels commands that have not yet been passed to `step`.
    ///
    /// Submitted GPU commands are no longer queued and must be reconciled
    /// through their asynchronous result request. Dropping the world releases
    /// all remaining reservations without manufacturing post-drop events.
    pub fn cancel_queued_commands(&mut self) -> WorldCommandResults {
        let commands = std::mem::take(&mut self.queued_world_commands);
        let results = commands
            .iter()
            .copied()
            .map(cancelled_result)
            .collect::<Vec<_>>();
        for selection in &mut self.selections {
            *selection = None;
        }
        if let Some(previous) = self.constraint_state_before_queue.take() {
            self.constraint_state = previous;
        }
        self.result_high_water = self
            .result_high_water
            .max(u32::try_from(results.len()).unwrap_or(u32::MAX));
        WorldCommandResults {
            outer_step: self.outer_step,
            results,
        }
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
        let queued_world_commands = self.queued_world_commands.len() as u32;
        let queued_construction = self.queued_construction.len() as u32;
        let queued_operations = queued_construction + queued_world_commands;
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
            count: queued_construction,
        }];
        let raw_world_commands = self
            .queued_world_commands
            .iter()
            .copied()
            .enumerate()
            .map(|(index, command)| command.raw(index as u32))
            .collect::<Vec<_>>();
        let next_result_token = (!self.queued_world_commands.is_empty())
            .then(|| {
                self.next_result_token
                    .checked_add(1)
                    .ok_or_else(|| WorldError::new("command-result token is exhausted"))
            })
            .transpose()?;
        self.inner
            .as_ref()
            .expect("world was initialized above")
            .encode_world_step_with_interactions(
                queue,
                encoder,
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
                &raw_world_commands,
            )
            .map_err(WorldError::new)?;
        self.queued_construction.clear();
        self.outer_step = self.outer_step.saturating_add(1);
        if let Some(next_result_token) = next_result_token {
            let token = self.next_result_token;
            self.next_result_token = next_result_token;
            self.in_flight_commands = Some(InFlightCommands {
                token,
                outer_step: self.outer_step,
                commands: std::mem::take(&mut self.queued_world_commands),
                result_requested: false,
            });
            self.constraint_state_before_queue = None;
        }
        Ok(StepReport {
            outer_step: self.outer_step,
            queued_operations,
            live_bodies: self.live_body_count(),
        })
    }

    /// Records a copy of the complete bounded terminal-result batch for the
    /// last command-bearing step. The caller owns submission and polling.
    pub fn request_command_results(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
    ) -> Result<CommandResultRequest, WorldError> {
        let in_flight = self
            .in_flight_commands
            .as_mut()
            .ok_or_else(|| WorldError::new("no world command results are pending"))?;
        if in_flight.result_requested {
            return Err(WorldError::new(
                "world command results already have an outstanding request",
            ));
        }
        let command_count = in_flight.commands.len() as u32;
        let readback = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox_sim_world_command_results"),
            size: u64::from(command_count) * WORLD_RESULT_BYTES,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        self.inner
            .as_ref()
            .ok_or_else(|| WorldError::new("step the world before requesting command results"))?
            .copy_world_command_results(encoder, &readback, command_count);
        in_flight.result_requested = true;
        Ok(CommandResultRequest {
            readback,
            completion: Arc::new(Mutex::new(None)),
            token: in_flight.token,
            command_count,
            mapping_started: false,
        })
    }

    /// Releases an outstanding result-copy request so the same non-lossy GPU
    /// batch can be requested again.
    ///
    /// This does not discard or reconcile results. It is useful when a caller
    /// abandons an encoder before submission or chooses to replace a pending
    /// map operation.
    pub fn abandon_command_result_request(
        &mut self,
        request: CommandResultRequest,
    ) -> Result<(), WorldError> {
        let in_flight = self
            .in_flight_commands
            .as_mut()
            .ok_or_else(|| WorldError::new("no world command results are pending"))?;
        if in_flight.token != request.token || !in_flight.result_requested {
            return Err(WorldError::new(
                "command-result request does not match the outstanding batch",
            ));
        }
        request.readback.unmap();
        in_flight.result_requested = false;
        Ok(())
    }

    /// Returns `None` until an explicitly submitted result copy has completed.
    /// Successful completion reconciles GPU-selected body retirement and every
    /// logical selection/constraint reservation before yielding application
    /// results.
    pub fn try_take_command_results(
        &mut self,
        request: &mut CommandResultRequest,
    ) -> Option<Result<WorldCommandResults, WorldError>> {
        if !request.mapping_started {
            return None;
        }
        let in_flight = match self.in_flight_commands.as_ref() {
            Some(in_flight) if in_flight.token == request.token => in_flight,
            Some(_) => {
                return Some(Err(WorldError::new(
                    "command-result request belongs to a different pending batch",
                )));
            }
            None => {
                return Some(Err(WorldError::new(
                    "command-result batch was already reconciled",
                )));
            }
        };
        if request.command_count != in_flight.commands.len() as u32 {
            return Some(Err(WorldError::new(
                "command-result request count does not match its pending batch",
            )));
        }
        let mapping = request
            .completion
            .lock()
            .expect("command-result completion lock is not poisoned")
            .take()?;
        if let Err(error) = mapping {
            request.readback.unmap();
            if let Some(in_flight) = self.in_flight_commands.as_mut() {
                in_flight.result_requested = false;
            }
            return Some(Err(WorldError::new(format!(
                "command-result mapping failed: {error}"
            ))));
        }
        let mapped = request.readback.slice(..).get_mapped_range();
        let decoded = decode_results(&mapped, &in_flight.commands).map_err(WorldError::new);
        drop(mapped);
        request.readback.unmap();
        let results = match decoded {
            Ok(results) => results,
            Err(error) => {
                if let Some(in_flight) = self.in_flight_commands.as_mut() {
                    in_flight.result_requested = false;
                }
                return Some(Err(error));
            }
        };
        let in_flight = self
            .in_flight_commands
            .take()
            .expect("validated pending command batch exists");
        self.reconcile_command_results(&results);
        self.result_high_water = self
            .result_high_water
            .max(u32::try_from(results.len()).unwrap_or(u32::MAX));
        Some(Ok(WorldCommandResults {
            outer_step: in_flight.outer_step,
            results,
        }))
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

    /// Creates a reusable same-device observation of public body physics
    /// state. Step the world once before creating it.
    pub fn create_gpu_body_observation(
        &self,
        device: &wgpu::Device,
    ) -> Result<crate::GpuBodyObservation, WorldError> {
        self.inner
            .as_ref()
            .ok_or_else(|| WorldError::new("step the world before creating a GPU observation"))?
            .create_gpu_body_observation(device)
            .map_err(|error| WorldError::new(error.to_string()))
    }

    pub fn counters(&self) -> WorldCounters {
        let queued_destructions = self
            .queued_world_commands
            .iter()
            .filter(|command| matches!(command, QueuedWorldCommand::DestroySelectedBody { .. }))
            .count();
        let in_flight_destructions = self.in_flight_commands.as_ref().map_or(0, |batch| {
            batch
                .commands
                .iter()
                .filter(|command| matches!(command, QueuedWorldCommand::DestroySelectedBody { .. }))
                .count()
        });
        WorldCounters {
            outer_steps: self.outer_step,
            live_bodies: self.live_body_count(),
            capacity_bodies: self.definition.capacity.slots(),
            pending_operations: (self.queued_construction.len() + self.queued_world_commands.len())
                as u32,
            gpu_initialized: self.inner.is_some(),
            command_capacity: WORLD_COMMAND_CAPACITY,
            result_capacity: crate::WORLD_RESULT_CAPACITY,
            queued_commands: self.queued_world_commands.len() as u32,
            in_flight_commands: self
                .in_flight_commands
                .as_ref()
                .map_or(0, |batch| batch.commands.len() as u32),
            pending_selections: self.selections.iter().flatten().count() as u32,
            pending_constraints: u32::from(
                matches!(self.constraint_state, ConstraintState::Pending { .. })
                    || matches!(
                        self.constraint_state,
                        ConstraintState::DestroyQueued { body: None, .. }
                    ),
            ),
            live_constraints: u32::from(
                matches!(self.constraint_state, ConstraintState::Live { .. })
                    || matches!(
                        self.constraint_state,
                        ConstraintState::DestroyQueued { body: Some(_), .. }
                    ),
            ),
            pending_destructions: (queued_destructions + in_flight_destructions) as u32,
            command_high_water: self.command_high_water,
            result_high_water: self.result_high_water,
            command_rejections: self.command_rejections,
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

    fn prepare_command_admission(&mut self) -> Result<(), CommandQueueError> {
        if self.in_flight_commands.is_some() {
            return Err(self.reject(CommandQueueError::new(
                CommandQueueErrorKind::ResultsPending,
                "reconcile the previous asynchronous command-result batch before queueing more commands",
            )));
        }
        if self.queued_world_commands.len() == WORLD_COMMAND_CAPACITY as usize {
            return Err(self.reject(CommandQueueError::new(
                CommandQueueErrorKind::QueueFull,
                "world command capacity is exhausted for this step",
            )));
        }
        if self.constraint_state_before_queue.is_none() {
            self.constraint_state_before_queue = Some(self.constraint_state);
        }
        Ok(())
    }

    fn take_command_id(&mut self) -> Result<crate::CommandId, CommandQueueError> {
        let value = self.next_command_id;
        let next = value.checked_add(1).ok_or_else(|| {
            self.reject(CommandQueueError::new(
                CommandQueueErrorKind::IdentifierExhausted,
                "world command identifier is exhausted",
            ))
        })?;
        self.next_command_id = next;
        Ok(crate::CommandId::from_value(value))
    }

    fn push_world_command(&mut self, command: QueuedWorldCommand) {
        self.queued_world_commands.push(command);
        self.command_high_water = self
            .command_high_water
            .max(self.queued_world_commands.len() as u32);
    }

    fn consume_selection(
        &mut self,
        selection: crate::BodySelectionId,
    ) -> Result<(), CommandQueueError> {
        let Some(reservation) = self
            .selections
            .get_mut(selection.slot() as usize)
            .and_then(Option::as_mut)
        else {
            return Err(self.reject(CommandQueueError::new(
                CommandQueueErrorKind::InvalidSelection,
                "body selection is stale or unknown",
            )));
        };
        if reservation.id != selection || reservation.consumed {
            return Err(self.reject(CommandQueueError::new(
                CommandQueueErrorKind::InvalidSelection,
                "body selection is stale, unknown, or already consumed",
            )));
        }
        reservation.consumed = true;
        Ok(())
    }

    fn restore_selection_consumption(&mut self, selection: crate::BodySelectionId) {
        if let Some(reservation) = self
            .selections
            .get_mut(selection.slot() as usize)
            .and_then(Option::as_mut)
            .filter(|reservation| reservation.id == selection)
        {
            reservation.consumed = false;
        }
    }

    fn validate_constraint_for_update(
        &mut self,
        constraint: PointConstraintId,
    ) -> Result<(), CommandQueueError> {
        match self.constraint_state {
            ConstraintState::Pending { id } | ConstraintState::Live { id, .. }
                if id == constraint =>
            {
                Ok(())
            }
            ConstraintState::DestroyQueued { id, .. } if id == constraint => {
                Err(self.reject(CommandQueueError::new(
                    CommandQueueErrorKind::InvalidConstraint,
                    "point constraint already has a queued destruction",
                )))
            }
            ConstraintState::Free
            | ConstraintState::Pending { .. }
            | ConstraintState::Live { .. }
            | ConstraintState::DestroyQueued { .. } => Err(self.reject(CommandQueueError::new(
                CommandQueueErrorKind::InvalidConstraint,
                "point constraint is stale or unknown",
            ))),
        }
    }

    fn reject(&mut self, error: CommandQueueError) -> CommandQueueError {
        self.command_rejections = self.command_rejections.saturating_add(1);
        error
    }

    fn reconcile_command_results(&mut self, results: &[WorldCommandResult]) {
        for result in results {
            match result.kind {
                WorldCommandKind::PointQuery => {}
                WorldCommandKind::CreatePointConstraint => {
                    let Some(constraint) = result.constraint else {
                        continue;
                    };
                    if result.status == WorldCommandStatus::Succeeded {
                        self.constraint_state = ConstraintState::Live {
                            id: constraint,
                            body: result.body,
                        };
                    } else if self.constraint_state.id() == Some(constraint) {
                        self.constraint_state = ConstraintState::Free;
                    }
                }
                WorldCommandKind::SetPointConstraintTarget => {
                    if matches!(
                        result.status,
                        WorldCommandStatus::Invalid
                            | WorldCommandStatus::Cancelled
                            | WorldCommandStatus::Failed
                    ) && self.constraint_state.id() == result.constraint
                    {
                        self.constraint_state = ConstraintState::Free;
                    }
                }
                WorldCommandKind::DestroyPointConstraint => {
                    if self.constraint_state.id() == result.constraint {
                        self.constraint_state = ConstraintState::Free;
                    }
                }
                WorldCommandKind::DestroySelectedBody => {
                    if result.status != WorldCommandStatus::Succeeded {
                        continue;
                    }
                    let Some(body) = result.body else {
                        continue;
                    };
                    if self
                        .bodies
                        .get(body.slot() as usize)
                        .and_then(Option::as_ref)
                        .is_some_and(|record| record.id == body)
                    {
                        self.bodies[body.slot() as usize] = None;
                    }
                    if self.constraint_state.body() == Some(body) {
                        self.constraint_state = ConstraintState::Free;
                    }
                }
            }
        }
        for selection in &mut self.selections {
            *selection = None;
        }
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
    pub command_capacity: u32,
    pub result_capacity: u32,
    pub queued_commands: u32,
    pub in_flight_commands: u32,
    pub pending_selections: u32,
    pub pending_constraints: u32,
    pub live_constraints: u32,
    pub pending_destructions: u32,
    pub command_high_water: u32,
    pub result_high_water: u32,
    pub command_rejections: u64,
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
    let (inverse_mass, inverse_inertia) = record.body.mass_properties.map_or_else(
        || {
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
            (inverse_mass, inverse_inertia)
        },
        |mass| (mass.inverse_mass, mass.inverse_inertia),
    );
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
        sleeping: record.body.sleeping,
    })
}

#[cfg(feature = "engine-internal")]
pub fn engine_unconfigured_world(
    device: &wgpu::Device,
    world: World,
) -> Result<GpuBodyWorld, String> {
    let scenario = world
        .initial_scenario()
        .map_err(|error| error.to_string())?;
    GpuBodyWorld::new(device, &scenario)
}

#[cfg(feature = "engine-internal")]
pub fn engine_generated_world(device: &wgpu::Device, world: World) -> Result<GpuBodyWorld, String> {
    let definition = world.definition;
    let scenario = world
        .initial_scenario()
        .map_err(|error| error.to_string())?;
    GpuBodyWorld::new_generated_with_configuration(
        device,
        &scenario,
        definition.capacity,
        definition.quality.physics,
    )
}

#[cfg(feature = "engine-internal")]
pub fn engine_generated_world_with_scheduling(
    device: &wgpu::Device,
    world: World,
    scheduling: crate::GpuSolverSchedulingPolicyV1,
) -> Result<GpuBodyWorld, String> {
    let definition = world.definition;
    let scenario = world
        .initial_scenario()
        .map_err(|error| error.to_string())?;
    if definition.quality.physics.solver_profile != SolverProfileName::HardDirect {
        return GpuBodyWorld::new_generated_with_configuration(
            device,
            &scenario,
            definition.capacity,
            definition.quality.physics,
        );
    }
    GpuBodyWorld::new_generated_with_scheduling(device, &scenario, definition.capacity, scheduling)
}

#[cfg(feature = "engine-internal")]
pub fn engine_generated_world_with_contact_policy(
    device: &wgpu::Device,
    world: World,
    identity: physbox_contracts::ContactIdentityPolicyV1,
) -> Result<GpuBodyWorld, String> {
    let definition = world.definition;
    let scenario = world
        .initial_scenario()
        .map_err(|error| error.to_string())?;
    if definition.quality.physics.solver_profile != SolverProfileName::HardDirect {
        return GpuBodyWorld::new_generated_with_configuration(
            device,
            &scenario,
            definition.capacity,
            definition.quality.physics,
        );
    }
    GpuBodyWorld::new_generated_with_contact_policy(
        device,
        &scenario,
        definition.capacity,
        identity,
    )
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
        || body.mass_properties.is_some()
        || body.sleeping
        || shape.material
            != (crate::MaterialDef {
                density: crate::construction::CONSTRUCTION_DENSITY,
                friction: crate::construction::CONSTRUCTION_FRICTION,
                restitution: crate::construction::CONSTRUCTION_RESTITUTION,
            })
    {
        return Err(WorldError::new(
            "runtime creation currently requires derived mass, awake state, the standard dynamic construction material, and zero initial rotation/velocity",
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::MassPropertiesDef;

    #[test]
    fn explicit_initial_mass_and_sleep_state_project_exactly() {
        let record = BodyRecord {
            id: BodyId::from_parts(3, 7),
            body: BodyDef::dynamic()
                .with_position([1.0, 2.0])
                .with_mass_properties(MassPropertiesDef::from_inverse(0.75, 4.25))
                .with_sleeping(true),
            shape: Some(ShapeDef::rectangle([0.3, 0.2]).with_friction(0.4)),
            live: true,
        };
        let state = body_state(&record).unwrap();
        assert_eq!(
            state.id,
            ContractBodyId {
                slot: 3,
                generation: 7
            }
        );
        assert_eq!(state.inverse_mass.get(), 0.75);
        assert_eq!(state.inverse_inertia.get(), 4.25);
        assert!(state.sleeping);
    }

    #[test]
    fn runtime_construction_rejects_initial_state_overrides() {
        let body =
            BodyDef::dynamic().with_mass_properties(MassPropertiesDef::from_inverse(1.0, 1.0));
        let shape = ShapeDef::circle(crate::construction::CONSTRUCTION_CIRCLE_RADIUS)
            .with_density(crate::construction::CONSTRUCTION_DENSITY)
            .with_friction(crate::construction::CONSTRUCTION_FRICTION)
            .with_restitution(crate::construction::CONSTRUCTION_RESTITUTION);
        assert!(validate_runtime_construction(body, shape).is_err());
    }

    #[test]
    fn bounded_command_admission_and_cancellation_release_every_reservation() {
        let instance = crate::gpu_context::create_instance();
        let adapter =
            pollster::block_on(crate::gpu_context::request_adapter(&instance, None)).unwrap();
        let (device, _) = pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut world = World::new(&device, WorldDef::default()).unwrap();

        for _ in 0..WORLD_COMMAND_CAPACITY {
            world
                .queue_point_query(PointQueryDef::at([0.0, 0.0]))
                .unwrap();
        }
        let error = world
            .queue_point_query(PointQueryDef::at([0.0, 0.0]))
            .unwrap_err();
        assert_eq!(error.kind(), CommandQueueErrorKind::QueueFull);
        assert_eq!(world.counters().queued_commands, WORLD_COMMAND_CAPACITY);
        assert_eq!(world.counters().pending_selections, BODY_SELECTION_CAPACITY);
        assert_eq!(world.counters().command_rejections, 1);

        let cancelled = world.cancel_queued_commands();
        assert_eq!(cancelled.results.len(), WORLD_COMMAND_CAPACITY as usize);
        assert!(
            cancelled
                .results
                .iter()
                .all(|result| result.status == WorldCommandStatus::Cancelled)
        );
        assert_eq!(world.counters().queued_commands, 0);
        assert_eq!(world.counters().pending_selections, 0);
        assert_eq!(world.counters().result_high_water, WORLD_COMMAND_CAPACITY);
    }

    #[test]
    fn pending_constraint_commands_are_ordered_and_selection_is_single_use() {
        let instance = crate::gpu_context::create_instance();
        let adapter =
            pollster::block_on(crate::gpu_context::request_adapter(&instance, None)).unwrap();
        let (device, _) = pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut world = World::new(&device, WorldDef::default()).unwrap();

        let selection = world
            .queue_point_query(PointQueryDef::at([0.0, 0.0]))
            .unwrap();
        let constraint = world
            .queue_create_point_constraint(
                selection.selection,
                PointConstraintDef::at_target([0.0, 0.0]),
            )
            .unwrap();
        world
            .queue_set_point_constraint_target(constraint.constraint, [1.0, 0.0])
            .unwrap();
        world
            .queue_destroy_point_constraint(constraint.constraint)
            .unwrap();

        assert_eq!(
            world
                .queue_destroy_selected_body(selection.selection)
                .unwrap_err()
                .kind(),
            CommandQueueErrorKind::InvalidSelection
        );
        assert_eq!(
            world
                .queue_set_point_constraint_target(constraint.constraint, [2.0, 0.0])
                .unwrap_err()
                .kind(),
            CommandQueueErrorKind::InvalidConstraint
        );
        assert_eq!(world.counters().pending_constraints, 1);
        assert_eq!(world.counters().live_constraints, 0);

        let cancelled = world.cancel_queued_commands();
        assert_eq!(cancelled.results.len(), 4);
        assert_eq!(world.counters().pending_constraints, 0);
        assert_eq!(world.counters().live_constraints, 0);
    }
}
