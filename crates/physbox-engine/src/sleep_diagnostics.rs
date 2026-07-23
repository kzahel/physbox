use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use serde::Serialize;
use wgpu::util::DeviceExt;

use crate::{GpuBodyStorage, body_state::ACTIVE_BODY_DISPATCH_64_OFFSET};

pub const SLEEP_CYCLE_DIAGNOSTIC_VERSION: u32 = 1;
pub const SLEEP_CYCLE_DIAGNOSTIC_WORDS: u32 = 16;
pub const SLEEP_CYCLE_DIAGNOSTIC_BYTES: u32 =
    SLEEP_CYCLE_DIAGNOSTIC_WORDS * size_of::<u32>() as u32;
const POINT_DISPATCH_64_OFFSET: u64 = 4 * size_of::<u32>() as u64;
const INVALID: u32 = u32::MAX;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepCycleMetricsV1 {
    pub version: u32,
    pub observed_step: u32,
    pub awake_bodies: u32,
    pub sleeping_bodies: u32,
    pub slept_this_step: u32,
    pub woke_this_step: u32,
    pub mixed_awake_asleep_contact_cuts: Option<u32>,
    pub first_stable_sleep_step: Option<u32>,
    pub stable_candidate_step: Option<u32>,
    pub stable_confirmation_steps: u32,
    pub awake_body_time: u64,
    pub dynamic_bodies: u32,
    pub overflowed: bool,
}

impl SleepCycleMetricsV1 {
    pub(crate) fn initial(
        awake_bodies: u32,
        dynamic_bodies: u32,
        stable_confirmation_steps: u32,
        mixed_contact_cuts_available: bool,
    ) -> Self {
        Self {
            version: SLEEP_CYCLE_DIAGNOSTIC_VERSION,
            observed_step: 0,
            awake_bodies,
            sleeping_bodies: dynamic_bodies.saturating_sub(awake_bodies),
            slept_this_step: 0,
            woke_this_step: 0,
            mixed_awake_asleep_contact_cuts: mixed_contact_cuts_available.then_some(0),
            first_stable_sleep_step: None,
            stable_candidate_step: None,
            stable_confirmation_steps,
            awake_body_time: 0,
            dynamic_bodies,
            overflowed: false,
        }
    }
}

pub(crate) fn decode_sleep_cycle_metrics(bytes: &[u8]) -> Result<SleepCycleMetricsV1, String> {
    if bytes.len() != SLEEP_CYCLE_DIAGNOSTIC_BYTES as usize {
        return Err(format!(
            "sleep-cycle diagnostic requires {} bytes, received {}",
            SLEEP_CYCLE_DIAGNOSTIC_BYTES,
            bytes.len()
        ));
    }
    let words = bytemuck::cast_slice::<u8, u32>(bytes);
    if words[0] != SLEEP_CYCLE_DIAGNOSTIC_VERSION {
        return Err(format!(
            "unsupported sleep-cycle diagnostic version {}",
            words[0]
        ));
    }
    Ok(SleepCycleMetricsV1 {
        version: words[0],
        observed_step: words[1],
        awake_bodies: words[2],
        sleeping_bodies: words[3],
        slept_this_step: words[4],
        woke_this_step: words[5],
        mixed_awake_asleep_contact_cuts: Some(words[6]),
        first_stable_sleep_step: (words[7] != INVALID).then_some(words[7]),
        stable_candidate_step: (words[8] != INVALID).then_some(words[8]),
        stable_confirmation_steps: words[13],
        awake_body_time: u64::from(words[10]) | (u64::from(words[11]) << 32),
        dynamic_bodies: words[12],
        overflowed: words[14] != 0,
    })
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct ParamsRaw {
    counts: [u32; 4],
}

pub(crate) struct GpuSleepCycleDiagnostics {
    metrics: wgpu::Buffer,
    _params: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    prepare: wgpu::ComputePipeline,
    count_bodies: wgpu::ComputePipeline,
    count_cuts: wgpu::ComputePipeline,
    finalize: wgpu::ComputePipeline,
    live_bodies: std::sync::Arc<wgpu::Buffer>,
    point_state: std::sync::Arc<wgpu::Buffer>,
    stable_confirmation_steps: u32,
}

impl GpuSleepCycleDiagnostics {
    pub(crate) fn new(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        solver_points: &wgpu::Buffer,
        point_state: std::sync::Arc<wgpu::Buffer>,
        body_capacity: u32,
        point_capacity: u32,
        stable_confirmation_steps: u32,
    ) -> Self {
        let metrics = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_sleep_cycle_metrics"),
            contents: bytemuck::cast_slice(&[
                SLEEP_CYCLE_DIAGNOSTIC_VERSION,
                0,
                0,
                0,
                0,
                0,
                0,
                INVALID,
                INVALID,
                0,
                0,
                0,
                0,
                stable_confirmation_steps,
                0,
                0,
            ]),
            usage: wgpu::BufferUsages::STORAGE
                | wgpu::BufferUsages::COPY_SRC
                | wgpu::BufferUsages::COPY_DST,
        });
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_sleep_cycle_params"),
            contents: bytemuck::bytes_of(&ParamsRaw {
                counts: [body_capacity, point_capacity, stable_confirmation_steps, 0],
            }),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_sleep_cycle_layout"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                storage_entry(2, true),
                storage_entry(3, true),
                storage_entry(4, false),
                storage_entry(5, true),
                storage_entry(6, true),
                storage_entry(7, false),
                uniform_entry(8),
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_sleep_cycle_pipeline_layout"),
            bind_group_layouts: &[&layout],
            push_constant_ranges: &[],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_sleep_cycle_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sleep_diagnostics.wgsl").into()),
        });
        let pipeline = |entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry_point),
                layout: Some(&pipeline_layout),
                module: &shader,
                entry_point: Some(entry_point),
                compilation_options: wgpu::PipelineCompilationOptions::default(),
                cache: None,
            })
        };
        let live_bodies = storage.active_body_handle();
        let awake_work = storage.awake_work_handle();
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_sleep_cycle_group"),
            layout: &layout,
            entries: &[
                entry(0, storage.metadata_buffer()),
                entry(1, &live_bodies),
                entry(2, awake_work.sleep_state_buffer()),
                entry(3, awake_work.awake_body_buffer()),
                entry(4, awake_work.control_buffer()),
                entry(5, solver_points),
                entry(6, &point_state),
                entry(7, &metrics),
                entry(8, &params),
            ],
        });
        Self {
            metrics,
            _params: params,
            bind_group,
            prepare: pipeline("prepare_metrics"),
            count_bodies: pipeline("count_body_metrics"),
            count_cuts: pipeline("count_mixed_contact_cuts"),
            finalize: pipeline("finalize_metrics"),
            live_bodies,
            point_state,
            stable_confirmation_steps,
        }
    }

    pub(crate) fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        self.pass(
            encoder,
            "physbox5_sleep_metrics_prepare",
            &self.prepare,
            None,
        );
        self.pass(
            encoder,
            "physbox5_sleep_metrics_bodies",
            &self.count_bodies,
            Some((&self.live_bodies, ACTIVE_BODY_DISPATCH_64_OFFSET)),
        );
        self.pass(
            encoder,
            "physbox5_sleep_metrics_cuts",
            &self.count_cuts,
            Some((&self.point_state, POINT_DISPATCH_64_OFFSET)),
        );
        self.pass(
            encoder,
            "physbox5_sleep_metrics_finalize",
            &self.finalize,
            None,
        );
    }

    pub(crate) fn copy_to(&self, encoder: &mut wgpu::CommandEncoder, destination: &wgpu::Buffer) {
        encoder.copy_buffer_to_buffer(
            &self.metrics,
            0,
            destination,
            0,
            u64::from(SLEEP_CYCLE_DIAGNOSTIC_BYTES),
        );
    }

    pub(crate) fn reset(&self, queue: &wgpu::Queue) {
        queue.write_buffer(
            &self.metrics,
            0,
            bytemuck::cast_slice(&[
                SLEEP_CYCLE_DIAGNOSTIC_VERSION,
                0,
                0,
                0,
                0,
                0,
                0,
                INVALID,
                INVALID,
                0,
                0,
                0,
                0,
                self.stable_confirmation_steps,
                0,
                0,
            ]),
        );
    }

    pub(crate) fn allocated_bytes(&self) -> u64 {
        self.metrics.size() + self._params.size()
    }

    fn pass(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        label: &'static str,
        pipeline: &wgpu::ComputePipeline,
        indirect: Option<(&wgpu::Buffer, u64)>,
    ) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some(label),
            timestamp_writes: None,
        });
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        if let Some((buffer, offset)) = indirect {
            pass.dispatch_workgroups_indirect(buffer, offset);
        } else {
            pass.dispatch_workgroups(1, 1, 1);
        }
    }
}

fn entry<'a>(binding: u32, buffer: &'a wgpu::Buffer) -> wgpu::BindGroupEntry<'a> {
    wgpu::BindGroupEntry {
        binding,
        resource: buffer.as_entire_binding(),
    }
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
            min_binding_size: wgpu::BufferSize::new(size_of::<ParamsRaw>() as u64),
        },
        count: None,
    }
}
