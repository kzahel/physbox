use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BodyId, FiniteF32, GRAB_INTENT_CONTRACT_VERSION, GrabIntentBatchV1, GrabIntentV1, GrabTuningV1,
    SCHEMA_VERSION, Validate, Vec2,
};
use serde::Serialize;

pub const GRAB_INTENT_CAPACITY: u32 = 64;
pub const GRAB_INTENT_BYTES: u32 = 32;
pub const GRAB_STATE_BYTES: u32 = 64;
pub const GRAB_PARAMETER_BYTES: u32 = 48;
pub const GRAB_SPRING_ACCELERATION: f32 = 45.0;
pub const GRAB_DAMPING_RATE: f32 = 10.0;
pub const GRAB_MAX_ACCELERATION: f32 = 80.0;
pub const GRAB_MAX_LINEAR_SPEED: f32 = 20.0;
pub const GRAB_MAX_ANGULAR_SPEED: f32 = 30.0;

pub fn provisional_grab_tuning() -> GrabTuningV1 {
    let finite = |value| FiniteF32::new(value).expect("grab tuning constants are finite");
    GrabTuningV1 {
        spring_acceleration: finite(GRAB_SPRING_ACCELERATION),
        damping_rate: finite(GRAB_DAMPING_RATE),
        max_acceleration: finite(GRAB_MAX_ACCELERATION),
        max_linear_speed: finite(GRAB_MAX_LINEAR_SPEED),
        max_angular_speed: finite(GRAB_MAX_ANGULAR_SPEED),
    }
}

pub(crate) const NO_GRAB_SLOT: u32 = u32::MAX;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GrabIntentRaw {
    pub(crate) kind_sequence_padding: [u32; 4],
    pub(crate) target_padding: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GrabStateRaw {
    pub(crate) identity_status: [u32; 4],
    pub(crate) local_anchor_target: [f32; 4],
    pub(crate) counters: [u32; 4],
    pub(crate) activity: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct GrabParamsRaw {
    pub(crate) dt_tuning: [f32; 4],
    pub(crate) limits_padding: [f32; 4],
    pub(crate) counts: [u32; 4],
}

const _: () = assert!(std::mem::size_of::<GrabIntentRaw>() == GRAB_INTENT_BYTES as usize);
const _: () = assert!(std::mem::size_of::<GrabStateRaw>() == GRAB_STATE_BYTES as usize);
const _: () = assert!(std::mem::size_of::<GrabParamsRaw>() == GRAB_PARAMETER_BYTES as usize);

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct GrabValidationState {
    pub contract_version: u32,
    pub active: bool,
    pub selected_body: Option<BodyId>,
    pub last_sequence: u32,
    pub local_anchor: [f32; 2],
    pub world_target: [f32; 2],
    pub begin_attempts: u32,
    pub successful_selections: u32,
    pub no_hit_selections: u32,
    pub fixed_only_rejections: u32,
    pub target_updates: u32,
    pub end_requests: u32,
    pub stale_generation_clears: u32,
    pub scanned_bodies_high_water: u32,
}

pub(crate) fn decode_grab_state(bytes: &[u8]) -> Result<GrabValidationState, String> {
    if bytes.len() != GRAB_STATE_BYTES as usize {
        return Err(format!(
            "grab state requires {GRAB_STATE_BYTES} bytes, received {}",
            bytes.len()
        ));
    }
    let raw = bytemuck::pod_read_unaligned::<GrabStateRaw>(bytes);
    let active = raw.identity_status[0] != NO_GRAB_SLOT;
    let selected_body = active.then_some(BodyId {
        slot: raw.identity_status[0],
        generation: raw.identity_status[1],
    });
    Ok(GrabValidationState {
        contract_version: GRAB_INTENT_CONTRACT_VERSION,
        active,
        selected_body,
        last_sequence: raw.identity_status[3],
        local_anchor: [raw.local_anchor_target[0], raw.local_anchor_target[1]],
        world_target: [raw.local_anchor_target[2], raw.local_anchor_target[3]],
        begin_attempts: raw.counters[0],
        successful_selections: raw.counters[1],
        no_hit_selections: raw.counters[2],
        fixed_only_rejections: raw.counters[3],
        target_updates: raw.activity[0],
        end_requests: raw.activity[1],
        stale_generation_clears: raw.activity[2],
        scanned_bodies_high_water: raw.activity[3],
    })
}

#[derive(Clone, Debug)]
pub(crate) struct BoundedGrabIntents {
    capacity: u32,
    attempted: u64,
    overflowed: bool,
    intents: Vec<GrabIntentV1>,
}

impl BoundedGrabIntents {
    pub(crate) fn new(capacity: u32) -> Result<Self, String> {
        if capacity == 0 {
            return Err("grab intent capacity must be positive".to_owned());
        }
        Ok(Self {
            capacity,
            attempted: 0,
            overflowed: false,
            intents: Vec::with_capacity(capacity as usize),
        })
    }

    pub(crate) fn push(&mut self, intent: GrabIntentV1) -> Result<(), String> {
        intent.validate().map_err(|error| error.to_string())?;
        if self
            .intents
            .last()
            .is_some_and(|previous| previous.sequence() >= intent.sequence())
        {
            return Err("grab intent sequences must be strictly increasing".to_owned());
        }
        self.attempted = self
            .attempted
            .checked_add(1)
            .ok_or_else(|| "grab intent attempt count overflowed".to_owned())?;
        if self.intents.len() == self.capacity as usize {
            self.overflowed = true;
            return Err(format!(
                "grab intent capacity {} overflowed at attempt {}",
                self.capacity, self.attempted
            ));
        }
        self.intents.push(intent);
        Ok(())
    }

    pub(crate) fn coalesce_target(&mut self, world_target: Vec2) -> bool {
        let Some(last) = self.intents.last_mut() else {
            return false;
        };
        match last {
            GrabIntentV1::Target {
                world_target: target,
                ..
            } => {
                *target = world_target;
                true
            }
            GrabIntentV1::Begin { .. } | GrabIntentV1::End { .. } => false,
        }
    }

    pub(crate) fn artifact(&self) -> GrabIntentBatchV1 {
        GrabIntentBatchV1 {
            schema_version: SCHEMA_VERSION,
            contract_version: GRAB_INTENT_CONTRACT_VERSION,
            capacity_intents: self.capacity,
            attempted_intents: self.attempted,
            overflowed: self.overflowed,
            intents: self.intents.clone(),
        }
    }

    pub(crate) fn clear(&mut self) {
        self.attempted = 0;
        self.overflowed = false;
        self.intents.clear();
    }

    pub(crate) fn overflowed(&self) -> bool {
        self.overflowed
    }
}

pub(crate) fn empty_grab_intent_batch() -> GrabIntentBatchV1 {
    GrabIntentBatchV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: GRAB_INTENT_CONTRACT_VERSION,
        capacity_intents: GRAB_INTENT_CAPACITY,
        attempted_intents: 0,
        overflowed: false,
        intents: Vec::new(),
    }
}

pub(crate) fn grab_intent_raw(intent: GrabIntentV1) -> GrabIntentRaw {
    let mut raw = GrabIntentRaw {
        kind_sequence_padding: [0, intent.sequence(), 0, 0],
        target_padding: [0.0; 4],
    };
    match intent {
        GrabIntentV1::Begin { world_target, .. } => {
            raw.kind_sequence_padding[0] = 1;
            raw.target_padding = [world_target.x, world_target.y, 0.0, 0.0];
        }
        GrabIntentV1::Target { world_target, .. } => {
            raw.kind_sequence_padding[0] = 2;
            raw.target_padding = [world_target.x, world_target.y, 0.0, 0.0];
        }
        GrabIntentV1::End { .. } => raw.kind_sequence_padding[0] = 3,
    }
    raw
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pending_target_updates_coalesce_without_changing_the_begin_point() {
        let mut queue = BoundedGrabIntents::new(2).unwrap();
        queue
            .push(GrabIntentV1::Begin {
                sequence: 7,
                world_target: Vec2::ZERO,
            })
            .unwrap();
        assert!(!queue.coalesce_target(Vec2::new(0.25, 0.5)));
        queue
            .push(GrabIntentV1::Target {
                sequence: 8,
                world_target: Vec2::new(0.25, 0.5),
            })
            .unwrap();
        assert!(queue.coalesce_target(Vec2::new(0.5, 0.75)));
        let artifact = queue.artifact();
        assert_eq!(artifact.intents.len(), 2);
        assert_eq!(artifact.intents[0].sequence(), 7);
        assert!(matches!(
            artifact.intents[0],
            GrabIntentV1::Begin { world_target, .. } if world_target == Vec2::ZERO
        ));
        assert!(matches!(
            artifact.intents[1],
            GrabIntentV1::Target { world_target, .. }
                if world_target == Vec2::new(0.5, 0.75)
        ));
        artifact.validate().unwrap();
    }

    #[test]
    fn intent_overflow_is_atomic_and_diagnosed_by_the_batch() {
        let mut queue = BoundedGrabIntents::new(1).unwrap();
        queue.push(GrabIntentV1::End { sequence: 1 }).unwrap();
        assert!(queue.push(GrabIntentV1::End { sequence: 2 }).is_err());
        assert!(queue.overflowed());
        let artifact = queue.artifact();
        assert_eq!(artifact.attempted_intents, 2);
        assert!(artifact.overflowed);
        artifact.validate().unwrap();
    }
}
