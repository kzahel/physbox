use std::{mem::size_of, sync::Arc};

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BodyStateV1, CapacityRecordV1, SLEEP_POLICY_VERSION, SLEEP_STATE_LAYOUT_VERSION,
    SLEEP_TRANSITION_CAUSE_INITIALIZATION,
};
use wgpu::util::DeviceExt;

use crate::body_state::ACTIVE_BODY_STATE_WORDS;

pub(crate) const SLEEP_STATE_WORDS: u32 = 8;
pub(crate) const SLEEP_STATE_BYTES: u32 = SLEEP_STATE_WORDS * size_of::<u32>() as u32;
pub(crate) const AWAKE_BODY_STATE_WORDS: u32 = ACTIVE_BODY_STATE_WORDS;

const SLEEP_FLAG_AWAKE: u32 = 1 << 0;
const SLEEP_FLAG_ELIGIBLE: u32 = 1 << 1;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SleepStateRaw {
    values: [u32; SLEEP_STATE_WORDS as usize],
}

const _: () = assert!(size_of::<SleepStateRaw>() == SLEEP_STATE_BYTES as usize);

pub(crate) struct GpuAwakeWork {
    sleep_state: wgpu::Buffer,
    awake_bodies: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    pipeline: wgpu::ComputePipeline,
    body_capacity: u32,
    initial_awake_count: u32,
}

impl GpuAwakeWork {
    pub(crate) fn new(
        device: &wgpu::Device,
        body_capacity: u32,
        bodies: &[BodyStateV1],
        metadata: &wgpu::Buffer,
        active_bodies: &wgpu::Buffer,
    ) -> Result<Arc<Self>, String> {
        let (sleep_state, awake_bodies) = initial_data(body_capacity, bodies)?;
        let initial_awake_count = awake_bodies[0];
        check_buffer(device, body_capacity, SLEEP_STATE_BYTES, "sleep state")?;
        check_buffer(
            device,
            body_capacity + AWAKE_BODY_STATE_WORDS,
            size_of::<u32>() as u32,
            "awake body data",
        )?;
        let usage = wgpu::BufferUsages::STORAGE
            | wgpu::BufferUsages::COPY_SRC
            | wgpu::BufferUsages::COPY_DST;
        let sleep_state = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_sleep_state_v1"),
            contents: bytemuck::cast_slice(&sleep_state),
            usage,
        });
        let awake_bodies = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_awake_body_data"),
            contents: bytemuck::cast_slice(&awake_bodies),
            usage: usage | wgpu::BufferUsages::INDIRECT,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_awake_compaction_layout"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                storage_entry(2, false),
                storage_entry(3, false),
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_awake_compaction_bind_group"),
            layout: &layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: metadata.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: active_bodies.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: sleep_state.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: awake_bodies.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_awake_compaction_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("awake_work.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_awake_compaction_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("physbox5_awake_compaction_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("compact_awake_bodies"),
            compilation_options: Default::default(),
            cache: None,
        });
        Ok(Arc::new(Self {
            sleep_state,
            awake_bodies,
            bind_group,
            pipeline,
            body_capacity,
            initial_awake_count,
        }))
    }

    pub(crate) fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_awake_compaction_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    pub(crate) fn upload_reset(&self, queue: &wgpu::Queue, bodies: &[BodyStateV1]) {
        let (sleep_state, awake_bodies) = initial_data(self.body_capacity, bodies)
            .expect("validated reset bodies fit the existing awake-work capacity");
        queue.write_buffer(&self.sleep_state, 0, bytemuck::cast_slice(&sleep_state));
        queue.write_buffer(&self.awake_bodies, 0, bytemuck::cast_slice(&awake_bodies));
    }

    #[cfg(test)]
    pub(crate) fn sleep_state_buffer(&self) -> &wgpu::Buffer {
        &self.sleep_state
    }

    #[cfg(test)]
    pub(crate) fn awake_body_buffer(&self) -> &wgpu::Buffer {
        &self.awake_bodies
    }

    pub(crate) fn sleep_state_capacity(&self) -> CapacityRecordV1 {
        CapacityRecordV1 {
            class: "sleep-state-v1".to_owned(),
            capacity_elements: u64::from(self.body_capacity),
            used_elements: u64::from(self.body_capacity),
            allocated_bytes: u64::from(self.body_capacity) * u64::from(SLEEP_STATE_BYTES),
            high_water_bytes: u64::from(self.body_capacity) * u64::from(SLEEP_STATE_BYTES),
            overflowed: false,
        }
    }

    pub(crate) fn awake_body_capacity(&self) -> CapacityRecordV1 {
        CapacityRecordV1 {
            class: "awake-body-slots".to_owned(),
            capacity_elements: u64::from(self.body_capacity),
            used_elements: u64::from(self.initial_awake_count),
            allocated_bytes: u64::from(self.body_capacity + AWAKE_BODY_STATE_WORDS)
                * size_of::<u32>() as u64,
            high_water_bytes: u64::from(self.initial_awake_count + AWAKE_BODY_STATE_WORDS)
                * size_of::<u32>() as u64,
            overflowed: false,
        }
    }
}

fn initial_data(
    body_capacity: u32,
    bodies: &[BodyStateV1],
) -> Result<(Vec<SleepStateRaw>, Vec<u32>), String> {
    if bodies.len() > body_capacity as usize {
        return Err(format!(
            "awake-work capacity {body_capacity} cannot hold {} bodies",
            bodies.len()
        ));
    }
    let mut sleep_state = vec![
        SleepStateRaw {
            values: [
                1,
                0,
                0,
                0,
                SLEEP_TRANSITION_CAUSE_INITIALIZATION,
                SLEEP_POLICY_VERSION,
                SLEEP_STATE_LAYOUT_VERSION,
                0,
            ],
        };
        body_capacity as usize
    ];
    let mut awake_slots = Vec::with_capacity(body_capacity as usize);
    for body in bodies {
        let index = usize::try_from(body.id.slot)
            .map_err(|_| "sleep-state body slot does not fit usize".to_owned())?;
        if index >= sleep_state.len() {
            return Err(format!(
                "sleep-state body slot {} exceeds capacity {body_capacity}",
                body.id.slot
            ));
        }
        let dynamic = body.inverse_mass.get() > 0.0;
        sleep_state[index].values[0] = body.id.generation;
        sleep_state[index].values[1] = if dynamic {
            SLEEP_FLAG_AWAKE | SLEEP_FLAG_ELIGIBLE
        } else {
            0
        };
        if dynamic {
            awake_slots.push(body.id.slot);
        }
    }
    awake_slots.sort_unstable();
    let awake_count = awake_slots.len() as u32;
    awake_slots.resize(body_capacity as usize, 0);
    let mut awake_bodies = vec![0; (body_capacity + AWAKE_BODY_STATE_WORDS) as usize];
    awake_bodies[0] = awake_count;
    awake_bodies[1] = awake_count;
    awake_bodies[4..7].copy_from_slice(&[awake_count.div_ceil(64), 1, 1]);
    awake_bodies[7..10].copy_from_slice(&[awake_count.div_ceil(256), 1, 1]);
    awake_bodies[10..13].copy_from_slice(&[awake_count.saturating_mul(32).div_ceil(64), 1, 1]);
    awake_bodies[13..17].copy_from_slice(&[6, awake_count, 0, 0]);
    awake_bodies[AWAKE_BODY_STATE_WORDS as usize..].copy_from_slice(&awake_slots);
    Ok((sleep_state, awake_bodies))
}

fn check_buffer(
    device: &wgpu::Device,
    elements: u32,
    stride: u32,
    label: &str,
) -> Result<(), String> {
    let bytes = u64::from(elements)
        .checked_mul(u64::from(stride))
        .ok_or_else(|| format!("{label} allocation overflowed u64"))?;
    let limits = device.limits();
    if bytes > limits.max_buffer_size || bytes > u64::from(limits.max_storage_buffer_binding_size) {
        return Err(format!(
            "{label} requires {bytes} bytes, exceeding portable device buffer limits"
        ));
    }
    Ok(())
}

fn storage_entry(binding: u32, read_only: bool) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn initial_awake_data_is_sorted_dynamic_only_and_versioned() {
        let mut scenario = crate::p5_1_rectangle_floor_scenario(1).unwrap();
        scenario.bodies[0].id.slot = 7;
        scenario.bodies[1].id.slot = 2;
        scenario.bodies.sort_by_key(|body| body.id);
        let (states, awake) = initial_data(8, &scenario.bodies).unwrap();
        assert_eq!(awake[0], 1);
        assert_eq!(awake[AWAKE_BODY_STATE_WORDS as usize], 7);
        assert_eq!(states[7].values[1], SLEEP_FLAG_AWAKE | SLEEP_FLAG_ELIGIBLE);
        assert_eq!(states[2].values[1], 0);
        assert_eq!(states[7].values[5], SLEEP_POLICY_VERSION);
        assert_eq!(states[7].values[6], SLEEP_STATE_LAYOUT_VERSION);
        assert_eq!(size_of::<SleepStateRaw>(), 32);
    }
}
