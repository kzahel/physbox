use std::{mem::size_of, sync::Arc};

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    ANALYTIC_SHAPE_FLAG_CIRCLE, ANALYTIC_SHAPE_FLAG_RECTANGLE, ANALYTIC_SHAPE_LAYOUT_VERSION,
    AnalyticShapeEncodingV1, AnalyticShapeKindV1, BODY_LAYOUT_VERSION, BodyCommandBatchV1,
    BodyCommandV1, BodyId, BodyLaneAccessV1, BodyLayoutLaneV1, BodyLayoutV1, BodyStateV1,
    CapacityRecordV1, SCHEMA_VERSION, ShapeV1, Validate, Vec2,
};
use wgpu::util::DeviceExt;

use crate::awake_work::{GpuAwakeWork, SleepExecutionConfig};

pub const BODY_BYTES_PER_SLOT: u32 = 64;
pub const BODY_COMMAND_BYTES: u32 = 48;
pub const BODY_COMMAND_CAPACITY: u32 = 64;
pub(crate) const ACTIVE_BODY_STATE_WORDS: u32 = 20;
pub(crate) const ACTIVE_BODY_DISPATCH_64_OFFSET: u64 = 4 * size_of::<u32>() as u64;
pub(crate) const ACTIVE_BODY_DISPATCH_256_OFFSET: u64 = 7 * size_of::<u32>() as u64;
pub(crate) const ACTIVE_BODY_WINNER_DISPATCH_OFFSET: u64 = 10 * size_of::<u32>() as u64;
pub(crate) const ACTIVE_BODY_DRAW_OFFSET: u64 = 13 * size_of::<u32>() as u64;

const FLAG_LIVE: u32 = 1 << 0;
const FLAG_SLEEPING: u32 = 1 << 1;
const FLAG_FIXED: u32 = 1 << 2;

const COMMAND_RESET: u32 = 1;
const COMMAND_GRAB_BEGIN: u32 = 2;
const COMMAND_GRAB_TARGET: u32 = 3;
const COMMAND_GRAB_END: u32 = 4;

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BodyPoseRaw {
    position_angle: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BodyMotionRaw {
    linear_angular: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BodyPropertiesRaw {
    half_extents_inverse_mass_inertia: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BodyMetadataRaw {
    generation_flags_material: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct ActiveBodyStateRaw {
    values: [u32; ACTIVE_BODY_STATE_WORDS as usize],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Pod, Zeroable)]
struct BodyCommandRaw {
    kind_slot_generation_sequence: [u32; 4],
    data0: [f32; 4],
    data1: [f32; 4],
}

pub(crate) struct GpuBodyResetData {
    poses: Vec<BodyPoseRaw>,
    motions: Vec<BodyMotionRaw>,
    metadata: Vec<BodyMetadataRaw>,
    active_bodies: Vec<u32>,
    bodies: Vec<BodyStateV1>,
}

struct PackedBodies {
    slots: BodySlotRegistry,
    poses: Vec<BodyPoseRaw>,
    motions: Vec<BodyMotionRaw>,
    properties: Vec<BodyPropertiesRaw>,
    metadata: Vec<BodyMetadataRaw>,
    active_bodies: Vec<u32>,
}

const _: () = assert!(size_of::<BodyPoseRaw>() == 16);
const _: () = assert!(size_of::<BodyMotionRaw>() == 16);
const _: () = assert!(size_of::<BodyPropertiesRaw>() == 16);
const _: () = assert!(size_of::<BodyMetadataRaw>() == 16);
const _: () = assert!(size_of::<ActiveBodyStateRaw>() == 80);
const _: () = assert!(size_of::<BodyCommandRaw>() == BODY_COMMAND_BYTES as usize);

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BodySlotRegistry {
    generations: Vec<u32>,
    live: Vec<bool>,
}

impl BodySlotRegistry {
    pub fn with_capacity(capacity: u32) -> Result<Self, String> {
        if capacity == 0 {
            return Err("body slot capacity must be positive".to_owned());
        }
        let capacity = usize::try_from(capacity)
            .map_err(|_| "body slot capacity does not fit usize".to_owned())?;
        Ok(Self {
            generations: vec![1; capacity],
            live: vec![false; capacity],
        })
    }

    pub fn reserve(&mut self, id: BodyId) -> Result<(), String> {
        if id.generation == 0 {
            return Err("body generations must be nonzero".to_owned());
        }
        let index = self.index(id.slot)?;
        if self.live[index] {
            return Err(format!("body slot {} is already live", id.slot));
        }
        self.generations[index] = id.generation;
        self.live[index] = true;
        Ok(())
    }

    pub fn release(&mut self, id: BodyId) -> Result<(), String> {
        let index = self.index(id.slot)?;
        if !self.matches(id) {
            return Err(format!(
                "body handle {}:{} is stale or not live",
                id.slot, id.generation
            ));
        }
        self.generations[index] = self.generations[index]
            .checked_add(1)
            .ok_or_else(|| format!("body slot {} exhausted its generation", id.slot))?;
        self.live[index] = false;
        Ok(())
    }

    pub fn allocate_lowest(&mut self) -> Result<BodyId, String> {
        let index = self
            .live
            .iter()
            .position(|live| !live)
            .ok_or_else(|| "body slot capacity overflow".to_owned())?;
        self.live[index] = true;
        Ok(BodyId {
            slot: index as u32,
            generation: self.generations[index],
        })
    }

    pub fn matches(&self, id: BodyId) -> bool {
        let Ok(index) = usize::try_from(id.slot) else {
            return false;
        };
        self.live.get(index).copied().unwrap_or(false)
            && self.generations.get(index).copied() == Some(id.generation)
    }

    pub fn capacity(&self) -> u32 {
        self.live.len() as u32
    }

    fn index(&self, slot: u32) -> Result<usize, String> {
        let index = usize::try_from(slot).map_err(|_| "body slot does not fit usize".to_owned())?;
        if index >= self.live.len() {
            return Err(format!(
                "body slot {slot} exceeds capacity {}",
                self.live.len()
            ));
        }
        Ok(index)
    }
}

#[derive(Clone, Debug)]
pub struct BoundedBodyCommands {
    capacity: u32,
    attempted: u64,
    overflowed: bool,
    commands: Vec<BodyCommandV1>,
}

impl BoundedBodyCommands {
    pub fn new(capacity: u32) -> Result<Self, String> {
        if capacity == 0 {
            return Err("body command capacity must be positive".to_owned());
        }
        Ok(Self {
            capacity,
            attempted: 0,
            overflowed: false,
            commands: Vec::with_capacity(capacity as usize),
        })
    }

    pub fn push(&mut self, command: BodyCommandV1) -> Result<(), String> {
        command.validate().map_err(|error| error.to_string())?;
        if self
            .commands
            .last()
            .is_some_and(|previous| previous.sequence() >= command.sequence())
        {
            return Err("body command sequences must be strictly increasing".to_owned());
        }
        self.attempted = self
            .attempted
            .checked_add(1)
            .ok_or_else(|| "body command attempt count overflowed".to_owned())?;
        if self.commands.len() == self.capacity as usize {
            self.overflowed = true;
            return Err(format!(
                "body command capacity {} overflowed at attempt {}",
                self.capacity, self.attempted
            ));
        }
        self.commands.push(command);
        Ok(())
    }

    pub fn artifact(&self) -> BodyCommandBatchV1 {
        BodyCommandBatchV1 {
            schema_version: SCHEMA_VERSION,
            capacity_commands: self.capacity,
            attempted_commands: self.attempted,
            overflowed: self.overflowed,
            commands: self.commands.clone(),
        }
    }

    pub fn clear(&mut self) {
        self.attempted = 0;
        self.overflowed = false;
        self.commands.clear();
    }

    pub fn len(&self) -> usize {
        self.commands.len()
    }

    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }

    pub fn overflowed(&self) -> bool {
        self.overflowed
    }

    pub fn coalesce_grab_target(&mut self, body: BodyId, world_target: Vec2) -> bool {
        let Some(command) = self.commands.last_mut() else {
            return false;
        };
        match command {
            BodyCommandV1::GrabBegin {
                body: target_body,
                world_target: target,
                ..
            }
            | BodyCommandV1::GrabTarget {
                body: target_body,
                world_target: target,
                ..
            } if *target_body == body => {
                *target = world_target;
                true
            }
            _ => false,
        }
    }
}

pub struct GpuBodyStorage {
    pose: wgpu::Buffer,
    motion: wgpu::Buffer,
    properties: wgpu::Buffer,
    metadata: wgpu::Buffer,
    active_bodies: Arc<wgpu::Buffer>,
    awake_work: Arc<GpuAwakeWork>,
    commands: wgpu::Buffer,
    layout: BodyLayoutV1,
    command_capacity: u32,
    slots: BodySlotRegistry,
}

impl GpuBodyStorage {
    pub fn new(
        device: &wgpu::Device,
        capacity: u32,
        command_capacity: u32,
        bodies: &[BodyStateV1],
    ) -> Result<Self, String> {
        Self::new_with_sleeping(
            device,
            capacity,
            command_capacity,
            bodies,
            SleepExecutionConfig::disabled(60),
        )
    }

    pub(crate) fn new_with_sleeping(
        device: &wgpu::Device,
        capacity: u32,
        command_capacity: u32,
        bodies: &[BodyStateV1],
        sleep_config: SleepExecutionConfig,
    ) -> Result<Self, String> {
        if bodies.len() > capacity as usize {
            return Err(format!(
                "body capacity {capacity} cannot hold {} authored bodies",
                bodies.len()
            ));
        }
        if command_capacity == 0 {
            return Err("body command capacity must be positive".to_owned());
        }
        let packed = pack_bodies(capacity, bodies)?;

        check_buffer_limits::<BodyPoseRaw>(device, capacity, "body pose")?;
        check_buffer_limits::<BodyMotionRaw>(device, capacity, "body motion")?;
        check_buffer_limits::<BodyPropertiesRaw>(device, capacity, "body properties")?;
        check_buffer_limits::<BodyMetadataRaw>(device, capacity, "body metadata")?;
        check_buffer_limits::<u32>(
            device,
            capacity + ACTIVE_BODY_STATE_WORDS,
            "active body data",
        )?;
        check_buffer_limits::<BodyCommandRaw>(device, command_capacity, "body commands")?;

        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let pose = storage_init(device, "physbox5_body_pose", &packed.poses, usage);
        let motion = storage_init(device, "physbox5_body_motion", &packed.motions, usage);
        let properties = storage_init(
            device,
            "physbox5_body_properties",
            &packed.properties,
            usage,
        );
        let metadata = storage_init(device, "physbox5_body_metadata", &packed.metadata, usage);
        let active_bodies = Arc::new(storage_init(
            device,
            "physbox5_active_body_data",
            &packed.active_bodies,
            usage | wgpu::BufferUsages::INDIRECT,
        ));
        let awake_work = GpuAwakeWork::new(
            device,
            capacity,
            bodies,
            &metadata,
            &motion,
            &properties,
            &active_bodies,
            sleep_config,
        )?;
        let commands = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_commands"),
            size: u64::from(command_capacity) * u64::from(BODY_COMMAND_BYTES),
            usage,
            mapped_at_creation: false,
        });
        let layout = layout_artifact(capacity, bodies.len() as u64, bodies.len() as u32)?;
        layout.validate().map_err(|error| error.to_string())?;

        Ok(Self {
            pose,
            motion,
            properties,
            metadata,
            active_bodies,
            awake_work,
            commands,
            layout,
            command_capacity,
            slots: packed.slots,
        })
    }

    pub fn layout(&self) -> &BodyLayoutV1 {
        &self.layout
    }

    pub fn command_capacity(&self) -> u32 {
        self.command_capacity
    }

    pub fn slots(&self) -> &BodySlotRegistry {
        &self.slots
    }

    pub fn pose_buffer(&self) -> &wgpu::Buffer {
        &self.pose
    }

    pub fn motion_buffer(&self) -> &wgpu::Buffer {
        &self.motion
    }

    pub fn properties_buffer(&self) -> &wgpu::Buffer {
        &self.properties
    }

    pub fn metadata_buffer(&self) -> &wgpu::Buffer {
        &self.metadata
    }

    pub(crate) fn active_body_buffer(&self) -> &wgpu::Buffer {
        &self.active_bodies
    }

    pub(crate) fn active_body_handle(&self) -> Arc<wgpu::Buffer> {
        Arc::clone(&self.active_bodies)
    }

    pub(crate) fn awake_work_handle(&self) -> Arc<GpuAwakeWork> {
        Arc::clone(&self.awake_work)
    }

    pub(crate) fn awake_body_buffer(&self) -> &wgpu::Buffer {
        self.awake_work.awake_body_buffer()
    }

    pub(crate) fn wake_request_buffer(&self) -> &wgpu::Buffer {
        self.awake_work.wake_request_buffer()
    }

    pub(crate) fn encode_awake_compaction(&self, encoder: &mut wgpu::CommandEncoder) {
        self.awake_work.encode(encoder);
    }

    pub(crate) fn encode_sleep_admission(&self, encoder: &mut wgpu::CommandEncoder) {
        self.awake_work.encode_sleep_admission(encoder);
    }

    #[cfg(test)]
    pub(crate) fn sleep_state_buffer(&self) -> &wgpu::Buffer {
        self.awake_work.sleep_state_buffer()
    }

    pub fn sleep_state_capacity_record(&self) -> CapacityRecordV1 {
        self.awake_work.sleep_state_capacity()
    }

    pub fn awake_body_capacity_record(&self) -> CapacityRecordV1 {
        self.awake_work.awake_body_capacity()
    }

    pub fn sleep_wake_capacity_record(&self) -> CapacityRecordV1 {
        self.awake_work.wake_capacity()
    }

    pub fn command_buffer(&self) -> &wgpu::Buffer {
        &self.commands
    }

    pub fn upload_commands(
        &self,
        queue: &wgpu::Queue,
        batch: &BodyCommandBatchV1,
    ) -> Result<CapacityRecordV1, String> {
        batch.validate().map_err(|error| error.to_string())?;
        if batch.capacity_commands != self.command_capacity {
            return Err(format!(
                "command batch capacity {} does not match GPU capacity {}",
                batch.capacity_commands, self.command_capacity
            ));
        }
        for command in &batch.commands {
            if !self.slots.matches(command.body()) {
                let body = command.body();
                return Err(format!(
                    "body command targets stale handle {}:{}",
                    body.slot, body.generation
                ));
            }
        }
        let raw = batch.commands.iter().map(command_raw).collect::<Vec<_>>();
        if !raw.is_empty() {
            queue.write_buffer(&self.commands, 0, bytemuck::cast_slice(&raw));
        }
        Ok(CapacityRecordV1 {
            class: "body-commands".to_owned(),
            capacity_elements: u64::from(self.command_capacity),
            used_elements: raw.len() as u64,
            allocated_bytes: u64::from(self.command_capacity) * u64::from(BODY_COMMAND_BYTES),
            high_water_bytes: raw.len() as u64 * u64::from(BODY_COMMAND_BYTES),
            overflowed: batch.overflowed,
        })
    }

    pub fn body_capacity_record(&self) -> CapacityRecordV1 {
        CapacityRecordV1 {
            class: "bodies".to_owned(),
            capacity_elements: u64::from(self.layout.capacity_elements),
            used_elements: u64::from(self.layout.live_elements),
            allocated_bytes: self.layout.allocated_bytes,
            high_water_bytes: u64::from(self.layout.live_elements)
                * u64::from(self.layout.bytes_per_body),
            overflowed: self.layout.overflowed,
        }
    }

    pub(crate) fn upload_reset_data(&self, queue: &wgpu::Queue, data: &GpuBodyResetData) {
        queue.write_buffer(&self.pose, 0, bytemuck::cast_slice(&data.poses));
        queue.write_buffer(&self.motion, 0, bytemuck::cast_slice(&data.motions));
        queue.write_buffer(&self.metadata, 0, bytemuck::cast_slice(&data.metadata));
        queue.write_buffer(
            &self.active_bodies,
            0,
            bytemuck::cast_slice(&data.active_bodies),
        );
        self.awake_work.upload_reset(queue, &data.bodies);
    }
}

impl GpuBodyResetData {
    pub(crate) fn new(capacity: u32, bodies: &[BodyStateV1]) -> Result<Self, String> {
        let packed = pack_bodies(capacity, bodies)?;
        Ok(Self {
            poses: packed.poses,
            motions: packed.motions,
            metadata: packed.metadata,
            active_bodies: packed.active_bodies,
            bodies: bodies.to_vec(),
        })
    }
}

pub fn analytic_shape_encoding(body: &BodyStateV1) -> Result<AnalyticShapeEncodingV1, String> {
    body.validate().map_err(|error| error.to_string())?;
    let (kind, property_payload, metadata_flag) = match body.shape {
        ShapeV1::Circle { radius } => (
            AnalyticShapeKindV1::Circle,
            Vec2::new(radius.get(), radius.get()),
            ANALYTIC_SHAPE_FLAG_CIRCLE,
        ),
        ShapeV1::Rectangle { half_extents } => (
            AnalyticShapeKindV1::Rectangle,
            half_extents,
            ANALYTIC_SHAPE_FLAG_RECTANGLE,
        ),
    };
    let encoding = AnalyticShapeEncodingV1 {
        schema_version: SCHEMA_VERSION,
        layout_version: ANALYTIC_SHAPE_LAYOUT_VERSION,
        body: body.id,
        kind,
        property_payload,
        metadata_flag,
        property_lane_stride_bytes: size_of::<BodyPropertiesRaw>() as u32,
        metadata_lane_stride_bytes: size_of::<BodyMetadataRaw>() as u32,
    };
    encoding.validate().map_err(|error| error.to_string())?;
    Ok(encoding)
}

fn pack_bodies(capacity: u32, bodies: &[BodyStateV1]) -> Result<PackedBodies, String> {
    if bodies.len() > capacity as usize {
        return Err(format!(
            "body capacity {capacity} cannot hold {} authored bodies",
            bodies.len()
        ));
    }
    let mut slots = BodySlotRegistry::with_capacity(capacity)?;
    let mut poses = vec![BodyPoseRaw::zeroed(); capacity as usize];
    let mut motions = vec![BodyMotionRaw::zeroed(); capacity as usize];
    let mut properties = vec![BodyPropertiesRaw::zeroed(); capacity as usize];
    let mut metadata = vec![
        BodyMetadataRaw {
            generation_flags_material: [1, 0, 0, 0],
        };
        capacity as usize
    ];
    let mut active_slots = Vec::with_capacity(capacity as usize);
    let mut fixed_count = 0;
    for body in bodies {
        body.validate().map_err(|error| error.to_string())?;
        slots.reserve(body.id)?;
        let index = body.id.slot as usize;
        let shape = analytic_shape_encoding(body)?;
        poses[index] = BodyPoseRaw {
            position_angle: [body.position.x, body.position.y, body.angle.get(), 0.0],
        };
        motions[index] = BodyMotionRaw {
            linear_angular: [
                body.linear_velocity.x,
                body.linear_velocity.y,
                body.angular_velocity.get(),
                0.0,
            ],
        };
        properties[index] = BodyPropertiesRaw {
            half_extents_inverse_mass_inertia: [
                shape.property_payload.x,
                shape.property_payload.y,
                body.inverse_mass.get(),
                body.inverse_inertia.get(),
            ],
        };
        let mut flags = FLAG_LIVE | shape.metadata_flag;
        if body.sleeping {
            flags |= FLAG_SLEEPING;
        }
        if body.inverse_mass.get() == 0.0 {
            flags |= FLAG_FIXED;
            fixed_count += 1;
        }
        metadata[index] = BodyMetadataRaw {
            generation_flags_material: [
                body.id.generation,
                flags,
                body.material.friction.get().to_bits(),
                body.material.restitution.get().to_bits(),
            ],
        };
        active_slots.push(body.id.slot);
    }
    active_slots.sort_unstable();
    let live_count = active_slots.len() as u32;
    active_slots.resize(capacity as usize, 0);
    let active_state = active_body_state(live_count, fixed_count);
    let mut active_bodies =
        Vec::with_capacity(ACTIVE_BODY_STATE_WORDS as usize + active_slots.len());
    active_bodies.extend_from_slice(&active_state.values);
    active_bodies.extend_from_slice(&active_slots);
    Ok(PackedBodies {
        slots,
        poses,
        motions,
        properties,
        metadata,
        active_bodies,
    })
}

fn active_body_state(live_count: u32, fixed_count: u32) -> ActiveBodyStateRaw {
    let mut values = [0; ACTIVE_BODY_STATE_WORDS as usize];
    values[0] = live_count;
    values[1] = live_count - fixed_count;
    values[2] = fixed_count;
    values[4..7].copy_from_slice(&[live_count.div_ceil(64), 1, 1]);
    values[7..10].copy_from_slice(&[live_count.div_ceil(256), 1, 1]);
    values[10..13].copy_from_slice(&[live_count.saturating_mul(32).div_ceil(64), 1, 1]);
    values[13..17].copy_from_slice(&[6, live_count, 0, 0]);
    ActiveBodyStateRaw { values }
}

fn layout_artifact(capacity: u32, attempted: u64, live: u32) -> Result<BodyLayoutV1, String> {
    let lane = |name: &str, access| BodyLayoutLaneV1 {
        name: name.to_owned(),
        element_stride_bytes: 16,
        capacity_elements: capacity,
        allocated_bytes: u64::from(capacity) * 16,
        access,
    };
    let lanes = vec![
        lane("metadata", BodyLaneAccessV1::ReadWrite),
        lane("motion", BodyLaneAccessV1::ReadWrite),
        lane("pose", BodyLaneAccessV1::ReadWrite),
        lane("properties", BodyLaneAccessV1::ReadWrite),
    ];
    let layout = BodyLayoutV1 {
        schema_version: SCHEMA_VERSION,
        layout_version: BODY_LAYOUT_VERSION,
        capacity_elements: capacity,
        live_elements: live,
        attempted_elements: attempted,
        bytes_per_body: BODY_BYTES_PER_SLOT,
        allocated_bytes: u64::from(capacity) * u64::from(BODY_BYTES_PER_SLOT),
        overflowed: attempted > u64::from(capacity),
        lanes,
    };
    layout.validate().map_err(|error| error.to_string())?;
    Ok(layout)
}

fn command_raw(command: &BodyCommandV1) -> BodyCommandRaw {
    let body = command.body();
    let mut raw = BodyCommandRaw {
        kind_slot_generation_sequence: [0, body.slot, body.generation, command.sequence()],
        data0: [0.0; 4],
        data1: [0.0; 4],
    };
    match command {
        BodyCommandV1::Reset {
            position,
            angle,
            linear_velocity,
            angular_velocity,
            ..
        } => {
            raw.kind_slot_generation_sequence[0] = COMMAND_RESET;
            raw.data0 = [position.x, position.y, angle.get(), 0.0];
            raw.data1 = [
                linear_velocity.x,
                linear_velocity.y,
                angular_velocity.get(),
                0.0,
            ];
        }
        BodyCommandV1::GrabBegin { world_target, .. } => {
            raw.kind_slot_generation_sequence[0] = COMMAND_GRAB_BEGIN;
            raw.data0 = [world_target.x, world_target.y, 0.0, 0.0];
        }
        BodyCommandV1::GrabTarget { world_target, .. } => {
            raw.kind_slot_generation_sequence[0] = COMMAND_GRAB_TARGET;
            raw.data0 = [world_target.x, world_target.y, 0.0, 0.0];
        }
        BodyCommandV1::GrabEnd { .. } => {
            raw.kind_slot_generation_sequence[0] = COMMAND_GRAB_END;
        }
    }
    raw
}

fn storage_init<T: Pod>(
    device: &wgpu::Device,
    label: &'static str,
    values: &[T],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some(label),
        contents: bytemuck::cast_slice(values),
        usage,
    })
}

fn check_buffer_limits<T>(device: &wgpu::Device, capacity: u32, label: &str) -> Result<(), String> {
    let bytes = u64::from(capacity)
        .checked_mul(size_of::<T>() as u64)
        .ok_or_else(|| format!("{label} allocation overflowed u64"))?;
    let limits = device.limits();
    if bytes > limits.max_buffer_size || bytes > u64::from(limits.max_storage_buffer_binding_size) {
        return Err(format!(
            "{label} requires {bytes} bytes, exceeding portable device buffer limits"
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{FiniteF32, ShapeV1, Vec2};

    use super::*;

    fn grab(sequence: u32, body: BodyId) -> BodyCommandV1 {
        BodyCommandV1::GrabTarget {
            sequence,
            body,
            world_target: Vec2::new(0.25, 0.5),
        }
    }

    #[test]
    fn circle_and_rectangle_share_the_accepted_analytic_payload_lanes() {
        let mut scenario = crate::p5_1_rectangle_floor_scenario(1).unwrap();
        let rectangle = analytic_shape_encoding(&scenario.bodies[0]).unwrap();
        assert_eq!(rectangle.kind, AnalyticShapeKindV1::Rectangle);
        assert_eq!(rectangle.metadata_flag, ANALYTIC_SHAPE_FLAG_RECTANGLE);
        assert_eq!(rectangle.property_lane_stride_bytes, 16);
        assert_eq!(rectangle.metadata_lane_stride_bytes, 16);

        scenario.bodies[0].shape = ShapeV1::Circle {
            radius: FiniteF32::new(0.35).unwrap(),
        };
        let circle = analytic_shape_encoding(&scenario.bodies[0]).unwrap();
        assert_eq!(circle.kind, AnalyticShapeKindV1::Circle);
        assert_eq!(circle.property_payload, Vec2::new(0.35, 0.35));
        assert_eq!(circle.metadata_flag, ANALYTIC_SHAPE_FLAG_CIRCLE);

        let packed = pack_bodies(2, &scenario.bodies).unwrap();
        assert_eq!(
            packed.properties[0].half_extents_inverse_mass_inertia[0],
            0.35
        );
        assert_eq!(
            packed.properties[0].half_extents_inverse_mass_inertia[1],
            0.35
        );
        assert_eq!(
            packed.metadata[0].generation_flags_material[1] & ANALYTIC_SHAPE_FLAG_CIRCLE,
            ANALYTIC_SHAPE_FLAG_CIRCLE
        );
        assert_eq!(BODY_BYTES_PER_SLOT, 64);
    }

    #[test]
    fn layout_strides_and_totals_are_exact() {
        let layout = layout_artifact(100_000, 100_000, 100_000).unwrap();
        assert_eq!(layout.bytes_per_body, 64);
        assert_eq!(layout.allocated_bytes, 6_400_000);
        assert_eq!(layout.lanes.len(), 4);
        layout.validate().unwrap();
    }

    #[test]
    fn releasing_a_slot_invalidates_its_old_generation() {
        let mut slots = BodySlotRegistry::with_capacity(2).unwrap();
        let first = slots.allocate_lowest().unwrap();
        assert!(slots.matches(first));
        slots.release(first).unwrap();
        assert!(!slots.matches(first));
        let replacement = slots.allocate_lowest().unwrap();
        assert_eq!(replacement.slot, first.slot);
        assert_eq!(replacement.generation, first.generation + 1);
        assert!(!slots.matches(first));
        assert!(slots.matches(replacement));
    }

    #[test]
    fn command_order_and_overflow_are_diagnosed() {
        let body = BodyId {
            slot: 0,
            generation: 1,
        };
        let mut queue = BoundedBodyCommands::new(2).unwrap();
        queue.push(grab(1, body)).unwrap();
        assert!(queue.push(grab(1, body)).is_err());
        queue.push(grab(2, body)).unwrap();
        let error = queue.push(grab(3, body)).unwrap_err();
        assert!(error.contains("capacity 2 overflowed"));
        let artifact = queue.artifact();
        assert!(artifact.overflowed);
        assert_eq!(artifact.attempted_commands, 3);
        artifact.validate().unwrap();
    }

    #[test]
    fn pending_grab_targets_coalesce_without_reordering_the_command() {
        let body = BodyId {
            slot: 0,
            generation: 1,
        };
        let mut queue = BoundedBodyCommands::new(4).unwrap();
        queue
            .push(BodyCommandV1::GrabBegin {
                sequence: 7,
                body,
                world_target: Vec2::new(0.0, 0.0),
            })
            .unwrap();
        assert!(queue.coalesce_grab_target(body, Vec2::new(0.5, 0.25)));
        let artifact = queue.artifact();
        assert_eq!(artifact.commands.len(), 1);
        assert_eq!(artifact.commands[0].sequence(), 7);
        assert!(matches!(
            artifact.commands[0],
            BodyCommandV1::GrabBegin { world_target, .. }
                if world_target == Vec2::new(0.5, 0.25)
        ));
    }

    #[test]
    fn gpu_allocation_uploads_only_live_generation_commands() {
        let instance = crate::create_instance();
        let adapter = pollster::block_on(crate::gpu_context::request_adapter(&instance, None))
            .expect("test GPU adapter");
        let (device, queue) =
            pollster::block_on(crate::gpu_context::request_device(&adapter)).unwrap();
        let mut scenario = crate::p5_1_rectangle_floor_scenario(1).unwrap();
        let capacity_error = match GpuBodyStorage::new(&device, 1, 4, &scenario.bodies) {
            Ok(_) => panic!("undersized body storage unexpectedly succeeded"),
            Err(error) => error,
        };
        assert!(capacity_error.contains("cannot hold 2 authored bodies"));
        scenario.bodies[0].id.generation = 7;
        let storage = GpuBodyStorage::new(&device, 4, 4, &scenario.bodies).unwrap();
        assert_eq!(storage.layout().allocated_bytes, 256);
        assert_eq!(storage.body_capacity_record().high_water_bytes, 128);

        let mut accepted = BoundedBodyCommands::new(4).unwrap();
        accepted
            .push(grab(
                1,
                BodyId {
                    slot: 0,
                    generation: 7,
                },
            ))
            .unwrap();
        let capacity = storage
            .upload_commands(&queue, &accepted.artifact())
            .unwrap();
        assert_eq!(capacity.used_elements, 1);
        assert!(!capacity.overflowed);

        let mut stale = BoundedBodyCommands::new(4).unwrap();
        stale
            .push(grab(
                2,
                BodyId {
                    slot: 0,
                    generation: 6,
                },
            ))
            .unwrap();
        assert!(
            storage
                .upload_commands(&queue, &stale.artifact())
                .unwrap_err()
                .contains("stale handle 0:6")
        );
    }
}
