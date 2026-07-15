use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    CONSTRUCTION_INTENT_CONTRACT_VERSION, ConstructionIntentBatchV1, ConstructionIntentV1,
    ConstructionResultV1, ConstructionStateV1, SCHEMA_VERSION, Validate,
};

pub const CONSTRUCTION_INTENT_CAPACITY: u32 = 64;
pub const CONSTRUCTION_INTENT_BYTES: u32 = 32;
pub const CONSTRUCTION_STATE_BYTES: u32 = 80;
pub const CONSTRUCTION_CIRCLE_RADIUS: f32 = 0.10;
pub const CONSTRUCTION_RECTANGLE_HALF_EXTENTS: [f32; 2] = [0.12, 0.08];
pub const CONSTRUCTION_FRICTION: f32 = 0.5;
pub const CONSTRUCTION_RESTITUTION: f32 = 0.0;

pub(crate) const CONSTRUCTION_RESULT_NONE: u32 = 0;
pub(crate) const CONSTRUCTION_RESULT_PLACED: u32 = 1;
pub(crate) const CONSTRUCTION_RESULT_ERASED: u32 = 2;
pub(crate) const CONSTRUCTION_RESULT_CAPACITY_FULL: u32 = 3;
pub(crate) const CONSTRUCTION_RESULT_DEEP_OVERLAP: u32 = 4;
pub(crate) const CONSTRUCTION_RESULT_ERASE_MISS: u32 = 5;
pub(crate) const CONSTRUCTION_RESULT_GENERATION_EXHAUSTED: u32 = 6;

const INTENT_PLACE_CIRCLE: u32 = 1;
const INTENT_PLACE_RECTANGLE: u32 = 2;
const INTENT_ERASE_AT: u32 = 3;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct ConstructionIntentRaw {
    pub(crate) kind_sequence_padding: [u32; 4],
    pub(crate) target_padding: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
pub(crate) struct ConstructionStateRaw {
    pub(crate) header: [u32; 4],
    pub(crate) last_identity_kind: [u32; 4],
    pub(crate) success_counts: [u32; 4],
    pub(crate) outcome_counts: [u32; 4],
    pub(crate) extra_counts: [u32; 4],
}

const _: () = assert!(size_of::<ConstructionIntentRaw>() == CONSTRUCTION_INTENT_BYTES as usize);
const _: () = assert!(size_of::<ConstructionStateRaw>() == CONSTRUCTION_STATE_BYTES as usize);

#[derive(Clone, Debug)]
pub struct BoundedConstructionIntents {
    capacity: u32,
    attempted: u64,
    overflowed: bool,
    intents: Vec<ConstructionIntentV1>,
}

impl BoundedConstructionIntents {
    pub fn new(capacity: u32) -> Result<Self, String> {
        if capacity == 0 {
            return Err("construction intent capacity must be positive".to_owned());
        }
        Ok(Self {
            capacity,
            attempted: 0,
            overflowed: false,
            intents: Vec::with_capacity(capacity as usize),
        })
    }

    pub fn push(&mut self, intent: ConstructionIntentV1) -> Result<(), String> {
        intent.validate().map_err(|error| error.to_string())?;
        if self
            .intents
            .last()
            .is_some_and(|previous| previous.sequence() >= intent.sequence())
        {
            return Err("construction intent sequences must be strictly increasing".to_owned());
        }
        self.attempted = self
            .attempted
            .checked_add(1)
            .ok_or_else(|| "construction intent attempt count overflowed".to_owned())?;
        if self.intents.len() == self.capacity as usize {
            self.overflowed = true;
            return Err(format!(
                "construction intent capacity {} overflowed at attempt {}",
                self.capacity, self.attempted
            ));
        }
        self.intents.push(intent);
        Ok(())
    }

    pub fn artifact(&self) -> ConstructionIntentBatchV1 {
        ConstructionIntentBatchV1 {
            schema_version: SCHEMA_VERSION,
            contract_version: CONSTRUCTION_INTENT_CONTRACT_VERSION,
            capacity_intents: self.capacity,
            attempted_intents: self.attempted,
            overflowed: self.overflowed,
            intents: self.intents.clone(),
        }
    }

    pub fn clear(&mut self) {
        self.attempted = 0;
        self.overflowed = false;
        self.intents.clear();
    }

    pub fn len(&self) -> usize {
        self.intents.len()
    }

    pub fn is_empty(&self) -> bool {
        self.intents.is_empty()
    }

    pub fn overflowed(&self) -> bool {
        self.overflowed
    }
}

pub(crate) fn empty_construction_intent_batch() -> ConstructionIntentBatchV1 {
    ConstructionIntentBatchV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: CONSTRUCTION_INTENT_CONTRACT_VERSION,
        capacity_intents: CONSTRUCTION_INTENT_CAPACITY,
        attempted_intents: 0,
        overflowed: false,
        intents: Vec::new(),
    }
}

pub(crate) fn construction_intent_raw(intent: ConstructionIntentV1) -> ConstructionIntentRaw {
    let kind = match intent {
        ConstructionIntentV1::PlaceCircle { .. } => INTENT_PLACE_CIRCLE,
        ConstructionIntentV1::PlaceRectangle { .. } => INTENT_PLACE_RECTANGLE,
        ConstructionIntentV1::EraseAt { .. } => INTENT_ERASE_AT,
    };
    let target = intent.world_target();
    ConstructionIntentRaw {
        kind_sequence_padding: [kind, intent.sequence(), 0, 0],
        target_padding: [target.x, target.y, 0.0, 0.0],
    }
}

pub(crate) fn initial_construction_state(capacity: u32, live: u32) -> ConstructionStateRaw {
    ConstructionStateRaw {
        header: [capacity, live, 0, CONSTRUCTION_RESULT_NONE],
        last_identity_kind: [u32::MAX, 0, 0, 0],
        success_counts: [0; 4],
        outcome_counts: [0; 4],
        extra_counts: [0; 4],
    }
}

fn construction_state(raw: ConstructionStateRaw) -> Result<ConstructionStateV1, String> {
    let last_result = match raw.header[3] {
        CONSTRUCTION_RESULT_NONE => ConstructionResultV1::None,
        CONSTRUCTION_RESULT_PLACED => ConstructionResultV1::Placed,
        CONSTRUCTION_RESULT_ERASED => ConstructionResultV1::Erased,
        CONSTRUCTION_RESULT_CAPACITY_FULL => ConstructionResultV1::CapacityFull,
        CONSTRUCTION_RESULT_DEEP_OVERLAP => ConstructionResultV1::DeepOverlap,
        CONSTRUCTION_RESULT_ERASE_MISS => ConstructionResultV1::EraseMiss,
        CONSTRUCTION_RESULT_GENERATION_EXHAUSTED => ConstructionResultV1::GenerationExhausted,
        code => {
            return Err(format!(
                "GPU construction state emitted unknown result {code}"
            ));
        }
    };
    let last_body = matches!(
        last_result,
        ConstructionResultV1::Placed | ConstructionResultV1::Erased
    )
    .then_some(physbox_contracts::BodyId {
        slot: raw.last_identity_kind[0],
        generation: raw.last_identity_kind[1],
    });
    let state = ConstructionStateV1 {
        schema_version: SCHEMA_VERSION,
        contract_version: CONSTRUCTION_INTENT_CONTRACT_VERSION,
        capacity_bodies: raw.header[0],
        live_bodies: raw.header[1],
        attempted_intents: u64::from(raw.success_counts[0]),
        succeeded_intents: u64::from(raw.success_counts[1]),
        placed_circles: u64::from(raw.success_counts[2]),
        placed_rectangles: u64::from(raw.success_counts[3]),
        erased_bodies: u64::from(raw.outcome_counts[0]),
        capacity_failures: u64::from(raw.outcome_counts[1]),
        deep_overlap_failures: u64::from(raw.outcome_counts[2]),
        erase_miss_failures: u64::from(raw.outcome_counts[3]),
        generation_failures: u64::from(raw.extra_counts[0]),
        grab_invalidations: u64::from(raw.extra_counts[1]),
        last_sequence: raw.header[2],
        last_result,
        last_body,
    };
    state
        .validate()
        .map_err(|error| format!("{error}; raw={raw:?}"))?;
    Ok(state)
}

pub fn construction_state_from_bytes(bytes: &[u8]) -> Result<ConstructionStateV1, String> {
    if bytes.len() != CONSTRUCTION_STATE_BYTES as usize {
        return Err(format!(
            "construction state requires {CONSTRUCTION_STATE_BYTES} bytes, received {}",
            bytes.len()
        ));
    }
    construction_state(bytemuck::pod_read_unaligned(bytes))
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{ConstructionIntentV1, Vec2};

    use super::*;

    #[test]
    fn bounded_intents_reject_reordering_and_overflow() {
        let mut intents = BoundedConstructionIntents::new(2).unwrap();
        intents
            .push(ConstructionIntentV1::PlaceCircle {
                sequence: 1,
                world_target: Vec2::new(0.0, 1.0),
            })
            .unwrap();
        assert!(
            intents
                .push(ConstructionIntentV1::EraseAt {
                    sequence: 1,
                    world_target: Vec2::ZERO,
                })
                .is_err()
        );
        intents
            .push(ConstructionIntentV1::PlaceRectangle {
                sequence: 2,
                world_target: Vec2::new(0.5, 1.0),
            })
            .unwrap();
        assert!(
            intents
                .push(ConstructionIntentV1::EraseAt {
                    sequence: 3,
                    world_target: Vec2::ZERO,
                })
                .is_err()
        );
        let artifact = intents.artifact();
        assert!(artifact.overflowed);
        artifact.validate().unwrap();
    }

    #[test]
    fn raw_state_decodes_to_the_versioned_contract() {
        let raw = ConstructionStateRaw {
            header: [64, 3, 9, CONSTRUCTION_RESULT_ERASED],
            last_identity_kind: [2, 4, 3, 0],
            success_counts: [3, 2, 1, 0],
            outcome_counts: [1, 0, 1, 0],
            extra_counts: [0, 1, 0, 0],
        };
        let state = construction_state(raw).unwrap();
        assert_eq!(state.live_bodies, 3);
        assert_eq!(state.succeeded_intents, 2);
        assert_eq!(state.last_body.unwrap().generation, 4);
    }
}
