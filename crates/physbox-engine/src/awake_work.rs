use std::{mem::size_of, sync::Arc};

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BodyStateV1, CapacityRecordV1, SLEEP_POLICY_VERSION, SLEEP_STATE_LAYOUT_VERSION,
    SLEEP_TRANSITION_CAUSE_CONFIGURATION, SLEEP_TRANSITION_CAUSE_INITIALIZATION, SleepPolicyV1,
};
use wgpu::util::DeviceExt;

use crate::body_state::{ACTIVE_BODY_DISPATCH_64_OFFSET, ACTIVE_BODY_STATE_WORDS};

pub(crate) const SLEEP_STATE_WORDS: u32 = 8;
pub(crate) const SLEEP_STATE_BYTES: u32 = SLEEP_STATE_WORDS * size_of::<u32>() as u32;
pub(crate) const AWAKE_BODY_STATE_WORDS: u32 = ACTIVE_BODY_STATE_WORDS;

const SLEEP_FLAG_AWAKE: u32 = 1 << 0;
const SLEEP_FLAG_ELIGIBLE: u32 = 1 << 1;

#[derive(Clone, Copy, Debug)]
pub(crate) struct SleepExecutionConfig {
    pub(crate) enabled: bool,
    pub(crate) fixed_step_hz: u32,
    pub(crate) wake_propagation_rounds: u32,
}

impl SleepExecutionConfig {
    pub(crate) fn disabled(fixed_step_hz: u32) -> Self {
        Self {
            enabled: false,
            fixed_step_hz,
            wake_propagation_rounds: SleepPolicyV1::experimental_s23().wake_propagation_rounds,
        }
    }

    pub(crate) fn enabled(fixed_step_hz: u32) -> Self {
        Self {
            enabled: true,
            ..Self::disabled(fixed_step_hz)
        }
    }
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SleepStateRaw {
    values: [u32; SLEEP_STATE_WORDS as usize],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct SleepParamsRaw {
    counts: [u32; 4],
    thresholds: [f32; 4],
}

const _: () = assert!(size_of::<SleepStateRaw>() == SLEEP_STATE_BYTES as usize);
const _: () = assert!(size_of::<SleepParamsRaw>() == 32);

pub(crate) struct GpuAwakeWork {
    sleep_state: wgpu::Buffer,
    awake_bodies: Arc<wgpu::Buffer>,
    wake_requests: wgpu::Buffer,
    wake_frontiers: [wgpu::Buffer; 2],
    control: wgpu::Buffer,
    params: wgpu::Buffer,
    begin_bind_group: wgpu::BindGroup,
    finalize_bind_groups: [wgpu::BindGroup; 2],
    compact_bind_group: wgpu::BindGroup,
    begin_pipeline: wgpu::ComputePipeline,
    finalize_pipeline: wgpu::ComputePipeline,
    compact_pipeline: wgpu::ComputePipeline,
    live_bodies: Arc<wgpu::Buffer>,
    body_capacity: u32,
    initial_awake_count: u32,
    config: SleepExecutionConfig,
}

impl GpuAwakeWork {
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn new(
        device: &wgpu::Device,
        body_capacity: u32,
        bodies: &[BodyStateV1],
        metadata: &wgpu::Buffer,
        motion: &wgpu::Buffer,
        properties: &wgpu::Buffer,
        active_bodies: &Arc<wgpu::Buffer>,
        config: SleepExecutionConfig,
    ) -> Result<Arc<Self>, String> {
        if config.fixed_step_hz == 0 {
            return Err("sleep execution requires positive fixed-step hertz".to_owned());
        }
        let policy = SleepPolicyV1::experimental_s23();
        let quiet_steps = policy
            .quiet_outer_steps(config.fixed_step_hz)
            .map_err(|error| error.to_string())?;
        let (sleep_state, awake_bodies) = initial_data(body_capacity, bodies, config.enabled)?;
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
        let awake_bodies = Arc::new(
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("physbox5_awake_body_data"),
                contents: bytemuck::cast_slice(&awake_bodies),
                usage: usage | wgpu::BufferUsages::INDIRECT,
            }),
        );
        let atomic_buffer = |label| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: u64::from(body_capacity) * size_of::<u32>() as u64,
                usage,
                mapped_at_creation: false,
            })
        };
        let wake_requests = atomic_buffer("physbox5_sleep_wake_requests");
        let wake_frontiers = [
            atomic_buffer("physbox5_sleep_wake_frontier_a"),
            atomic_buffer("physbox5_sleep_wake_frontier_b"),
        ];
        let control = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_sleep_control"),
            contents: bytemuck::cast_slice(&[0_u32; 4]),
            usage,
        });
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_sleep_params"),
            contents: bytemuck::bytes_of(&SleepParamsRaw {
                counts: [
                    body_capacity,
                    u32::from(config.enabled),
                    quiet_steps,
                    config.wake_propagation_rounds,
                ],
                thresholds: [
                    policy.speed_threshold.get(),
                    policy.impact_wake_speed.get(),
                    0.0,
                    0.0,
                ],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let begin_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_sleep_begin_layout"),
            entries: &[
                storage_entry(0, false),
                storage_entry(1, true),
                storage_entry(2, false),
                storage_entry(4, false),
                storage_entry(5, true),
                storage_entry(6, false),
                storage_entry(7, false),
                storage_entry(9, false),
                uniform_entry(10),
            ],
        });
        let begin_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_sleep_begin_bind_group"),
            layout: &begin_layout,
            entries: &[
                entry(0, metadata),
                entry(1, active_bodies),
                entry(2, &sleep_state),
                entry(4, motion),
                entry(5, properties),
                entry(6, &wake_requests),
                entry(7, &wake_frontiers[0]),
                entry(9, &control),
                entry(10, &params),
            ],
        });
        let finalize_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_sleep_finalize_layout"),
            entries: &[
                storage_entry(0, false),
                storage_entry(1, true),
                storage_entry(2, false),
                storage_entry(7, false),
                storage_entry(9, false),
                uniform_entry(10),
            ],
        });
        let finalize_bind_group = |label, final_frontier: usize| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &finalize_layout,
                entries: &[
                    entry(0, metadata),
                    entry(1, active_bodies),
                    entry(2, &sleep_state),
                    entry(7, &wake_frontiers[final_frontier]),
                    entry(9, &control),
                    entry(10, &params),
                ],
            })
        };
        let finalize_bind_groups = [
            finalize_bind_group("physbox5_sleep_finalize_a_bind_group", 0),
            finalize_bind_group("physbox5_sleep_finalize_b_bind_group", 1),
        ];
        let compact_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_awake_compaction_layout"),
            entries: &[
                storage_entry(0, false),
                storage_entry(1, true),
                storage_entry(2, false),
                storage_entry(3, false),
                storage_entry(9, false),
            ],
        });
        let compact_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_awake_compaction_bind_group"),
            layout: &compact_layout,
            entries: &[
                entry(0, metadata),
                entry(1, active_bodies),
                entry(2, &sleep_state),
                entry(3, &awake_bodies),
                entry(9, &control),
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_awake_work_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("awake_work.wgsl").into()),
        });
        let pipeline = |label, entry_point, bind_group_layout: &wgpu::BindGroupLayout| {
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some(label),
                bind_group_layouts: &[bind_group_layout],
                push_constant_ranges: &[],
            });
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(label),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let begin_pipeline = pipeline(
            "physbox5_sleep_begin_pipeline",
            "begin_sleep_step",
            &begin_layout,
        );
        let finalize_pipeline = pipeline(
            "physbox5_sleep_finalize_pipeline",
            "finalize_wakes",
            &finalize_layout,
        );
        let compact_pipeline = pipeline(
            "physbox5_awake_compaction_pipeline",
            "compact_awake_bodies",
            &compact_layout,
        );
        Ok(Arc::new(Self {
            sleep_state,
            awake_bodies,
            wake_requests,
            wake_frontiers,
            control,
            params,
            begin_bind_group,
            finalize_bind_groups,
            compact_bind_group,
            begin_pipeline,
            finalize_pipeline,
            compact_pipeline,
            live_bodies: Arc::clone(active_bodies),
            body_capacity,
            initial_awake_count,
            config,
        }))
    }

    pub(crate) fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        self.encode_begin_step(encoder);
        self.encode_finalize_wakes(encoder, 0);
        self.encode_compaction(encoder);
    }

    pub(crate) fn encode_begin_step(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_sleep_begin_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.begin_pipeline);
        pass.set_bind_group(0, &self.begin_bind_group, &[]);
        pass.dispatch_workgroups_indirect(&self.live_bodies, ACTIVE_BODY_DISPATCH_64_OFFSET);
    }

    pub(crate) fn encode_finalize_wakes(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        frontier: usize,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_sleep_finalize_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.finalize_pipeline);
        pass.set_bind_group(0, &self.finalize_bind_groups[frontier], &[]);
        pass.dispatch_workgroups_indirect(&self.live_bodies, ACTIVE_BODY_DISPATCH_64_OFFSET);
    }

    pub(crate) fn encode_compaction(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_awake_compaction_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.compact_pipeline);
        pass.set_bind_group(0, &self.compact_bind_group, &[]);
        pass.dispatch_workgroups(1, 1, 1);
    }

    pub(crate) fn upload_reset(&self, queue: &wgpu::Queue, bodies: &[BodyStateV1]) {
        let (sleep_state, awake_bodies) =
            initial_data(self.body_capacity, bodies, self.config.enabled)
                .expect("validated reset bodies fit the existing awake-work capacity");
        queue.write_buffer(&self.sleep_state, 0, bytemuck::cast_slice(&sleep_state));
        queue.write_buffer(&self.awake_bodies, 0, bytemuck::cast_slice(&awake_bodies));
        let cleared_wake_words = vec![0_u32; self.body_capacity as usize];
        queue.write_buffer(
            &self.wake_requests,
            0,
            bytemuck::cast_slice(&cleared_wake_words),
        );
        for frontier in &self.wake_frontiers {
            queue.write_buffer(frontier, 0, bytemuck::cast_slice(&cleared_wake_words));
        }
        queue.write_buffer(&self.control, 0, bytemuck::cast_slice(&[0_u32; 4]));
    }

    pub(crate) fn sleep_state_buffer(&self) -> &wgpu::Buffer {
        &self.sleep_state
    }

    #[cfg(test)]
    pub(crate) fn awake_body_buffer(&self) -> &wgpu::Buffer {
        &self.awake_bodies
    }

    pub(crate) fn wake_request_buffer(&self) -> &wgpu::Buffer {
        &self.wake_requests
    }

    pub(crate) fn wake_frontier_buffer(&self, index: usize) -> &wgpu::Buffer {
        &self.wake_frontiers[index]
    }

    pub(crate) fn params_buffer(&self) -> &wgpu::Buffer {
        &self.params
    }

    pub(crate) const fn wake_propagation_rounds(&self) -> u32 {
        self.config.wake_propagation_rounds
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

    pub(crate) fn wake_capacity(&self) -> CapacityRecordV1 {
        let fixed_words =
            (size_of::<SleepParamsRaw>() + 4 * size_of::<u32>()) as u64 / size_of::<u32>() as u64;
        let capacity_elements = u64::from(self.body_capacity) * 3 + fixed_words;
        CapacityRecordV1 {
            class: "sleep-wake-scratch".to_owned(),
            capacity_elements,
            used_elements: capacity_elements,
            allocated_bytes: capacity_elements * size_of::<u32>() as u64,
            high_water_bytes: capacity_elements * size_of::<u32>() as u64,
            overflowed: false,
        }
    }
}

fn initial_data(
    body_capacity: u32,
    bodies: &[BodyStateV1],
    sleeping_enabled: bool,
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
        let awake = dynamic && (!sleeping_enabled || !body.sleeping);
        sleep_state[index].values[0] = body.id.generation;
        sleep_state[index].values[1] = if dynamic {
            SLEEP_FLAG_ELIGIBLE | if awake { SLEEP_FLAG_AWAKE } else { 0 }
        } else {
            0
        };
        sleep_state[index].values[4] =
            SLEEP_TRANSITION_CAUSE_INITIALIZATION | SLEEP_TRANSITION_CAUSE_CONFIGURATION;
        if awake {
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

fn uniform_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::COMPUTE,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn entry<'a>(binding: u32, buffer: &'a wgpu::Buffer) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
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
        let (states, awake) = initial_data(8, &scenario.bodies, false).unwrap();
        assert_eq!(awake[0], 1);
        assert_eq!(awake[AWAKE_BODY_STATE_WORDS as usize], 7);
        assert_eq!(states[7].values[1], SLEEP_FLAG_AWAKE | SLEEP_FLAG_ELIGIBLE);
        assert_eq!(states[2].values[1], 0);
        assert_eq!(states[7].values[5], SLEEP_POLICY_VERSION);
        assert_eq!(states[7].values[6], SLEEP_STATE_LAYOUT_VERSION);
        assert_eq!(size_of::<SleepStateRaw>(), 32);
    }

    #[test]
    fn authored_sleepers_are_excluded_only_when_sleeping_is_enabled() {
        let mut scenario = crate::p5_1_rectangle_floor_scenario(1).unwrap();
        scenario.bodies[0].sleeping = true;
        let (_, disabled) = initial_data(2, &scenario.bodies, false).unwrap();
        let (enabled_states, enabled) = initial_data(2, &scenario.bodies, true).unwrap();
        assert_eq!(disabled[0], 1);
        assert_eq!(enabled[0], 0);
        assert_eq!(enabled_states[0].values[1], SLEEP_FLAG_ELIGIBLE);
    }
}
