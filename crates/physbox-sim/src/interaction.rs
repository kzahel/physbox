use std::fmt::{Display, Formatter};
use std::sync::{Arc, Mutex};

use bytemuck::{Pod, Zeroable};
use serde::Serialize;

use crate::BodyId;

pub const WORLD_COMMAND_CAPACITY: u32 = 64;
pub const WORLD_RESULT_CAPACITY: u32 = WORLD_COMMAND_CAPACITY;
pub const BODY_SELECTION_CAPACITY: u32 = WORLD_COMMAND_CAPACITY;
pub const POINT_CONSTRAINT_CAPACITY: u32 = 1;

pub(crate) const WORLD_COMMAND_BYTES: u64 = 64;
pub(crate) const WORLD_RESULT_BYTES: u64 = 32;
pub(crate) const WORLD_INTERACTION_COMMANDS_OFFSET: u64 =
    std::mem::offset_of!(WorldInteractionStorageRaw, commands) as u64;
pub(crate) const WORLD_INTERACTION_RESULTS_OFFSET: u64 =
    std::mem::offset_of!(WorldInteractionStorageRaw, results) as u64;

const STATUS_SUCCEEDED: u32 = 1;
const STATUS_INVALID: u32 = 2;
const STATUS_MISSED: u32 = 3;
const STATUS_CANCELLED: u32 = 4;
const STATUS_SUPERSEDED: u32 = 5;
const STATUS_FAILED: u32 = 6;

const KIND_POINT_QUERY: u32 = 1;
const KIND_CREATE_POINT_CONSTRAINT: u32 = 2;
const KIND_SET_POINT_CONSTRAINT_TARGET: u32 = 3;
const KIND_DESTROY_POINT_CONSTRAINT: u32 = 4;
const KIND_DESTROY_SELECTED_BODY: u32 = 5;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(transparent)]
pub struct CommandId(u32);

impl CommandId {
    pub const fn value(self) -> u32 {
        self.0
    }

    pub(crate) const fn from_value(value: u32) -> Self {
        Self(value)
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodySelectionId {
    slot: u32,
    generation: u32,
}

impl BodySelectionId {
    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub(crate) const fn from_parts(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PointConstraintId {
    slot: u32,
    generation: u32,
}

impl PointConstraintId {
    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }

    pub(crate) const fn from_parts(slot: u32, generation: u32) -> Self {
        Self { slot, generation }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointQueryDef {
    pub point: [f32; 2],
}

impl PointQueryDef {
    pub const fn at(point: [f32; 2]) -> Self {
        Self { point }
    }

    pub(crate) fn validate(self) -> Result<(), CommandQueueError> {
        finite_pair("point query", self.point)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointConstraintTuning {
    pub frequency_hz: f32,
    pub damping_ratio: f32,
    pub max_acceleration: f32,
    pub max_linear_speed: f32,
    pub max_angular_speed: f32,
}

impl PointConstraintTuning {
    pub(crate) fn validate(self) -> Result<(), CommandQueueError> {
        positive_finite("point-constraint frequency", self.frequency_hz)?;
        nonnegative_finite("point-constraint damping ratio", self.damping_ratio)?;
        positive_finite(
            "point-constraint maximum acceleration",
            self.max_acceleration,
        )?;
        positive_finite(
            "point-constraint maximum linear speed",
            self.max_linear_speed,
        )?;
        positive_finite(
            "point-constraint maximum angular speed",
            self.max_angular_speed,
        )?;
        if self
            .acceleration_coefficients()
            .into_iter()
            .all(f32::is_finite)
        {
            Ok(())
        } else {
            Err(CommandQueueError::new(
                CommandQueueErrorKind::InvalidDefinition,
                "point-constraint frequency and damping produce non-finite coefficients",
            ))
        }
    }

    pub(crate) fn acceleration_coefficients(self) -> [f32; 2] {
        let omega = std::f32::consts::TAU * self.frequency_hz;
        [omega * omega, 2.0 * self.damping_ratio * omega]
    }
}

impl Default for PointConstraintTuning {
    fn default() -> Self {
        let spring_acceleration = crate::grab::GRAB_SPRING_ACCELERATION;
        let omega = spring_acceleration.sqrt();
        Self {
            frequency_hz: omega / std::f32::consts::TAU,
            damping_ratio: crate::grab::GRAB_DAMPING_RATE / (2.0 * omega),
            max_acceleration: crate::grab::GRAB_MAX_ACCELERATION,
            max_linear_speed: crate::grab::GRAB_MAX_LINEAR_SPEED,
            max_angular_speed: crate::grab::GRAB_MAX_ANGULAR_SPEED,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PointConstraintDef {
    pub target: [f32; 2],
    pub tuning: PointConstraintTuning,
}

impl PointConstraintDef {
    pub fn at_target(target: [f32; 2]) -> Self {
        Self {
            target,
            tuning: PointConstraintTuning::default(),
        }
    }

    pub const fn with_tuning(mut self, tuning: PointConstraintTuning) -> Self {
        self.tuning = tuning;
        self
    }

    pub(crate) fn validate(self) -> Result<(), CommandQueueError> {
        finite_pair("point-constraint target", self.target)?;
        self.tuning.validate()
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuedBodySelection {
    pub command: CommandId,
    pub selection: BodySelectionId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct QueuedPointConstraint {
    pub command: CommandId,
    pub constraint: PointConstraintId,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldCommandKind {
    PointQuery,
    CreatePointConstraint,
    SetPointConstraintTarget,
    DestroyPointConstraint,
    DestroySelectedBody,
}

impl WorldCommandKind {
    const fn raw(self) -> u32 {
        match self {
            Self::PointQuery => KIND_POINT_QUERY,
            Self::CreatePointConstraint => KIND_CREATE_POINT_CONSTRAINT,
            Self::SetPointConstraintTarget => KIND_SET_POINT_CONSTRAINT_TARGET,
            Self::DestroyPointConstraint => KIND_DESTROY_POINT_CONSTRAINT,
            Self::DestroySelectedBody => KIND_DESTROY_SELECTED_BODY,
        }
    }

    fn from_raw(raw: u32) -> Result<Self, String> {
        match raw {
            KIND_POINT_QUERY => Ok(Self::PointQuery),
            KIND_CREATE_POINT_CONSTRAINT => Ok(Self::CreatePointConstraint),
            KIND_SET_POINT_CONSTRAINT_TARGET => Ok(Self::SetPointConstraintTarget),
            KIND_DESTROY_POINT_CONSTRAINT => Ok(Self::DestroyPointConstraint),
            KIND_DESTROY_SELECTED_BODY => Ok(Self::DestroySelectedBody),
            _ => Err(format!("unknown world command kind {raw}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WorldCommandStatus {
    Succeeded,
    Invalid,
    Missed,
    Cancelled,
    Superseded,
    Failed,
}

impl WorldCommandStatus {
    fn from_raw(raw: u32) -> Result<Self, String> {
        match raw {
            STATUS_SUCCEEDED => Ok(Self::Succeeded),
            STATUS_INVALID => Ok(Self::Invalid),
            STATUS_MISSED => Ok(Self::Missed),
            STATUS_CANCELLED => Ok(Self::Cancelled),
            STATUS_SUPERSEDED => Ok(Self::Superseded),
            STATUS_FAILED => Ok(Self::Failed),
            _ => Err(format!("unknown world command status {raw}")),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct WorldCommandResult {
    pub command: CommandId,
    pub kind: WorldCommandKind,
    pub status: WorldCommandStatus,
    pub body: Option<BodyId>,
    pub selection: Option<BodySelectionId>,
    pub constraint: Option<PointConstraintId>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct WorldCommandResults {
    pub outer_step: u64,
    pub results: Vec<WorldCommandResult>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CommandQueueErrorKind {
    QueueFull,
    ResultsPending,
    InvalidSelection,
    InvalidConstraint,
    InvalidDefinition,
    IdentifierExhausted,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandQueueError {
    kind: CommandQueueErrorKind,
    message: String,
}

impl CommandQueueError {
    pub const fn kind(&self) -> CommandQueueErrorKind {
        self.kind
    }

    pub(crate) fn new(kind: CommandQueueErrorKind, message: impl Into<String>) -> Self {
        Self {
            kind,
            message: message.into(),
        }
    }
}

impl Display for CommandQueueError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for CommandQueueError {}

pub struct CommandResultRequest {
    pub(crate) readback: wgpu::Buffer,
    pub(crate) completion: Arc<Mutex<Option<Result<(), String>>>>,
    pub(crate) token: u64,
    pub(crate) command_count: u32,
    pub(crate) mapping_started: bool,
}

impl CommandResultRequest {
    /// Starts mapping after the caller submits the encoder containing the
    /// result copy. This method never polls or waits for the device.
    pub fn begin_map(&mut self) -> Result<(), CommandResultRequestError> {
        if self.mapping_started {
            return Err(CommandResultRequestError(
                "command-result mapping was already started".to_owned(),
            ));
        }
        self.mapping_started = true;
        let callback_completion = Arc::clone(&self.completion);
        self.readback
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                *callback_completion
                    .lock()
                    .expect("command-result completion lock is not poisoned") =
                    Some(result.map_err(|error| error.to_string()));
            });
        Ok(())
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandResultRequestError(String);

impl Display for CommandResultRequestError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for CommandResultRequestError {}

#[derive(Clone, Copy, Debug)]
pub(crate) enum QueuedWorldCommand {
    PointQuery {
        command: CommandId,
        selection: BodySelectionId,
        query: PointQueryDef,
    },
    CreatePointConstraint {
        command: CommandId,
        selection: BodySelectionId,
        constraint: PointConstraintId,
        definition: PointConstraintDef,
    },
    SetPointConstraintTarget {
        command: CommandId,
        constraint: PointConstraintId,
        target: [f32; 2],
    },
    DestroyPointConstraint {
        command: CommandId,
        constraint: PointConstraintId,
    },
    DestroySelectedBody {
        command: CommandId,
        selection: BodySelectionId,
    },
}

impl QueuedWorldCommand {
    pub(crate) const fn command(self) -> CommandId {
        match self {
            Self::PointQuery { command, .. }
            | Self::CreatePointConstraint { command, .. }
            | Self::SetPointConstraintTarget { command, .. }
            | Self::DestroyPointConstraint { command, .. }
            | Self::DestroySelectedBody { command, .. } => command,
        }
    }

    pub(crate) const fn kind(self) -> WorldCommandKind {
        match self {
            Self::PointQuery { .. } => WorldCommandKind::PointQuery,
            Self::CreatePointConstraint { .. } => WorldCommandKind::CreatePointConstraint,
            Self::SetPointConstraintTarget { .. } => WorldCommandKind::SetPointConstraintTarget,
            Self::DestroyPointConstraint { .. } => WorldCommandKind::DestroyPointConstraint,
            Self::DestroySelectedBody { .. } => WorldCommandKind::DestroySelectedBody,
        }
    }

    pub(crate) fn raw(self, result_index: u32) -> WorldCommandRaw {
        let mut raw = WorldCommandRaw {
            header: [self.kind().raw(), self.command().value(), result_index, 0],
            handles: [u32::MAX, 0, u32::MAX, 0],
            data0: [0.0; 4],
            data1: [0.0; 4],
        };
        match self {
            Self::PointQuery {
                selection, query, ..
            } => {
                raw.handles[0] = selection.slot;
                raw.handles[1] = selection.generation;
                raw.data0[..2].copy_from_slice(&query.point);
            }
            Self::CreatePointConstraint {
                selection,
                constraint,
                definition,
                ..
            } => {
                raw.handles = [
                    selection.slot,
                    selection.generation,
                    constraint.slot,
                    constraint.generation,
                ];
                let coefficients = definition.tuning.acceleration_coefficients();
                raw.data0 = [
                    definition.target[0],
                    definition.target[1],
                    coefficients[0],
                    coefficients[1],
                ];
                raw.data1 = [
                    definition.tuning.max_acceleration,
                    definition.tuning.max_linear_speed,
                    definition.tuning.max_angular_speed,
                    0.0,
                ];
            }
            Self::SetPointConstraintTarget {
                constraint, target, ..
            } => {
                raw.handles[2] = constraint.slot;
                raw.handles[3] = constraint.generation;
                raw.data0[..2].copy_from_slice(&target);
            }
            Self::DestroyPointConstraint { constraint, .. } => {
                raw.handles[2] = constraint.slot;
                raw.handles[3] = constraint.generation;
            }
            Self::DestroySelectedBody { selection, .. } => {
                raw.handles[0] = selection.slot;
                raw.handles[1] = selection.generation;
            }
        }
        raw
    }
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct WorldCommandRaw {
    pub header: [u32; 4],
    pub handles: [u32; 4],
    pub data0: [f32; 4],
    pub data1: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BodySelectionRaw {
    identity_status: [u32; 4],
    point_padding: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct PointConstraintRaw {
    identity_status: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct WorldResultRaw {
    pub command_status_kind_body: [u32; 4],
    pub identity_handles: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
pub(crate) struct WorldInteractionStorageRaw {
    pub header: [u32; 4],
    pub commands: [WorldCommandRaw; WORLD_COMMAND_CAPACITY as usize],
    selections: [BodySelectionRaw; BODY_SELECTION_CAPACITY as usize],
    constraint: PointConstraintRaw,
    pub results: [WorldResultRaw; WORLD_RESULT_CAPACITY as usize],
}

const _: () = assert!(std::mem::size_of::<WorldCommandRaw>() == WORLD_COMMAND_BYTES as usize);
const _: () = assert!(std::mem::size_of::<WorldResultRaw>() == WORLD_RESULT_BYTES as usize);

pub(crate) fn decode_results(
    bytes: &[u8],
    commands: &[QueuedWorldCommand],
) -> Result<Vec<WorldCommandResult>, String> {
    let expected = commands.len() * WORLD_RESULT_BYTES as usize;
    if bytes.len() != expected {
        return Err(format!(
            "world command results require {expected} bytes, received {}",
            bytes.len()
        ));
    }
    let mut results = Vec::with_capacity(commands.len());
    for (index, (chunk, queued)) in bytes
        .chunks_exact(WORLD_RESULT_BYTES as usize)
        .zip(commands.iter().copied())
        .enumerate()
    {
        let raw = bytemuck::pod_read_unaligned::<WorldResultRaw>(chunk);
        let command = CommandId::from_value(raw.command_status_kind_body[0]);
        let status = WorldCommandStatus::from_raw(raw.command_status_kind_body[1])?;
        let kind = WorldCommandKind::from_raw(raw.command_status_kind_body[2])?;
        if command != queued.command() || kind != queued.kind() {
            return Err(format!(
                "world command result {index} does not match its accepted command"
            ));
        }
        let body = (raw.command_status_kind_body[3] != u32::MAX)
            .then(|| BodyId::from_parts(raw.command_status_kind_body[3], raw.identity_handles[0]));
        let selection = match queued {
            QueuedWorldCommand::PointQuery { selection, .. }
            | QueuedWorldCommand::CreatePointConstraint { selection, .. }
            | QueuedWorldCommand::DestroySelectedBody { selection, .. } => Some(selection),
            QueuedWorldCommand::SetPointConstraintTarget { .. }
            | QueuedWorldCommand::DestroyPointConstraint { .. } => None,
        };
        let constraint = match queued {
            QueuedWorldCommand::CreatePointConstraint { constraint, .. }
            | QueuedWorldCommand::SetPointConstraintTarget { constraint, .. }
            | QueuedWorldCommand::DestroyPointConstraint { constraint, .. } => Some(constraint),
            QueuedWorldCommand::PointQuery { .. }
            | QueuedWorldCommand::DestroySelectedBody { .. } => None,
        };
        results.push(WorldCommandResult {
            command,
            kind,
            status,
            body,
            selection,
            constraint,
        });
    }
    Ok(results)
}

pub(crate) fn cancelled_result(command: QueuedWorldCommand) -> WorldCommandResult {
    let selection = match command {
        QueuedWorldCommand::PointQuery { selection, .. }
        | QueuedWorldCommand::CreatePointConstraint { selection, .. }
        | QueuedWorldCommand::DestroySelectedBody { selection, .. } => Some(selection),
        QueuedWorldCommand::SetPointConstraintTarget { .. }
        | QueuedWorldCommand::DestroyPointConstraint { .. } => None,
    };
    let constraint = match command {
        QueuedWorldCommand::CreatePointConstraint { constraint, .. }
        | QueuedWorldCommand::SetPointConstraintTarget { constraint, .. }
        | QueuedWorldCommand::DestroyPointConstraint { constraint, .. } => Some(constraint),
        QueuedWorldCommand::PointQuery { .. } | QueuedWorldCommand::DestroySelectedBody { .. } => {
            None
        }
    };
    WorldCommandResult {
        command: command.command(),
        kind: command.kind(),
        status: WorldCommandStatus::Cancelled,
        body: None,
        selection,
        constraint,
    }
}

fn finite_pair(label: &str, value: [f32; 2]) -> Result<(), CommandQueueError> {
    if value.into_iter().all(f32::is_finite) {
        Ok(())
    } else {
        Err(CommandQueueError::new(
            CommandQueueErrorKind::InvalidDefinition,
            format!("{label} coordinates must be finite"),
        ))
    }
}

fn positive_finite(label: &str, value: f32) -> Result<(), CommandQueueError> {
    if value.is_finite() && value > 0.0 {
        Ok(())
    } else {
        Err(CommandQueueError::new(
            CommandQueueErrorKind::InvalidDefinition,
            format!("{label} must be finite and positive"),
        ))
    }
}

fn nonnegative_finite(label: &str, value: f32) -> Result<(), CommandQueueError> {
    if value.is_finite() && value >= 0.0 {
        Ok(())
    } else {
        Err(CommandQueueError::new(
            CommandQueueErrorKind::InvalidDefinition,
            format!("{label} must be finite and nonnegative"),
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_tuning_preserves_private_grab_coefficients() {
        let tuning = PointConstraintTuning::default();
        let [spring, damping] = tuning.acceleration_coefficients();
        assert!((spring - crate::grab::GRAB_SPRING_ACCELERATION).abs() < 1.0e-5);
        assert!((damping - crate::grab::GRAB_DAMPING_RATE).abs() < 1.0e-5);
    }

    #[test]
    fn command_and_result_layouts_are_shader_stable() {
        assert_eq!(std::mem::size_of::<WorldCommandRaw>(), 64);
        assert_eq!(std::mem::size_of::<BodySelectionRaw>(), 32);
        assert_eq!(std::mem::size_of::<PointConstraintRaw>(), 16);
        assert_eq!(std::mem::size_of::<WorldResultRaw>(), 32);
        assert_eq!(
            WORLD_INTERACTION_RESULTS_OFFSET,
            16 + 64 * 64 + 64 * 32 + 16
        );
    }
}
