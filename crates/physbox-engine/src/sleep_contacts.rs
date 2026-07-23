use std::{mem::size_of, sync::Arc};

use crate::{GpuBodyStorage, awake_work::GpuAwakeWork, body_state::ACTIVE_BODY_DISPATCH_64_OFFSET};

pub(crate) struct GpuSleepContacts {
    awake_work: Arc<GpuAwakeWork>,
    point_state: Arc<wgpu::Buffer>,
    live_bodies: Arc<wgpu::Buffer>,
    root_bind_group: wgpu::BindGroup,
    propagation_bind_groups: [wgpu::BindGroup; 2],
    copy_bind_groups: [wgpu::BindGroup; 2],
    erased_pipeline: wgpu::ComputePipeline,
    impact_pipeline: wgpu::ComputePipeline,
    copy_pipeline: wgpu::ComputePipeline,
    propagation_pipeline: wgpu::ComputePipeline,
}

impl GpuSleepContacts {
    pub(crate) fn new(
        device: &wgpu::Device,
        storage: &GpuBodyStorage,
        points: &wgpu::Buffer,
        point_state: Arc<wgpu::Buffer>,
    ) -> Self {
        let awake_work = storage.awake_work_handle();
        let live_bodies = storage.active_body_handle();
        let contact_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_sleep_contacts_layout"),
            entries: &[
                storage_entry(0, true),
                storage_entry(1, true),
                storage_entry(2, true),
                storage_entry(3, true),
                storage_entry(4, true),
                storage_entry(5, true),
                storage_entry(7, false),
                storage_entry(8, false),
                uniform_entry(9),
            ],
        });
        let bind_group = |label, source: usize| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &contact_layout,
                entries: &[
                    entry(0, points),
                    entry(1, &point_state),
                    entry(2, storage.metadata_buffer()),
                    entry(3, storage.motion_buffer()),
                    entry(4, storage.pose_buffer()),
                    entry(5, awake_work.sleep_state_buffer()),
                    entry(7, awake_work.wake_frontier_buffer(source)),
                    entry(8, awake_work.wake_frontier_buffer(1 - source)),
                    entry(9, awake_work.params_buffer()),
                ],
            })
        };
        let root_bind_group = bind_group("physbox5_sleep_contact_roots_bind_group", 0);
        let propagation_bind_groups = [
            bind_group("physbox5_sleep_contact_a_to_b_bind_group", 0),
            bind_group("physbox5_sleep_contact_b_to_a_bind_group", 1),
        ];
        let copy_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_sleep_frontier_copy_layout"),
            entries: &[
                storage_entry(6, true),
                storage_entry(7, false),
                storage_entry(8, false),
            ],
        });
        let copy_bind_group = |label, source: usize| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some(label),
                layout: &copy_layout,
                entries: &[
                    entry(6, storage.active_body_buffer()),
                    entry(7, awake_work.wake_frontier_buffer(source)),
                    entry(8, awake_work.wake_frontier_buffer(1 - source)),
                ],
            })
        };
        let copy_bind_groups = [
            copy_bind_group("physbox5_sleep_frontier_a_to_b_copy_bind_group", 0),
            copy_bind_group("physbox5_sleep_frontier_b_to_a_copy_bind_group", 1),
        ];
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_sleep_contacts_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("sleep_contacts.wgsl").into()),
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
        Self {
            awake_work,
            point_state,
            live_bodies,
            root_bind_group,
            propagation_bind_groups,
            copy_bind_groups,
            erased_pipeline: pipeline(
                "physbox5_sleep_erased_neighbor_pipeline",
                "seed_erased_neighbors",
                &contact_layout,
            ),
            impact_pipeline: pipeline(
                "physbox5_sleep_impact_pipeline",
                "seed_contact_impacts",
                &contact_layout,
            ),
            copy_pipeline: pipeline(
                "physbox5_sleep_frontier_copy_pipeline",
                "copy_frontier",
                &copy_layout,
            ),
            propagation_pipeline: pipeline(
                "physbox5_sleep_propagation_pipeline",
                "propagate_frontier",
                &contact_layout,
            ),
        }
    }

    pub(crate) fn encode_erased_neighbor_roots(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("physbox5_sleep_erased_neighbor_pass"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.erased_pipeline);
        pass.set_bind_group(0, &self.root_bind_group, &[]);
        pass.dispatch_workgroups_indirect(&self.point_state, 4 * size_of::<u32>() as u64);
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn encode_contact_wakes(&self, encoder: &mut wgpu::CommandEncoder) -> usize {
        self.encode_contact_wake_roots(encoder);
        self.encode_frontier_propagation(encoder)
    }

    pub(crate) fn encode_contact_wake_roots(&self, encoder: &mut wgpu::CommandEncoder) {
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_sleep_contact_impact_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.impact_pipeline);
            pass.set_bind_group(0, &self.root_bind_group, &[]);
            pass.dispatch_workgroups_indirect(&self.point_state, 4 * size_of::<u32>() as u64);
        }
    }

    pub(crate) fn encode_frontier_propagation(&self, encoder: &mut wgpu::CommandEncoder) -> usize {
        let mut source = 0;
        for _ in 0..self.awake_work.wake_propagation_rounds() {
            let group = &self.propagation_bind_groups[source];
            let copy_group = &self.copy_bind_groups[source];
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_sleep_frontier_copy_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.copy_pipeline);
                pass.set_bind_group(0, copy_group, &[]);
                pass.dispatch_workgroups_indirect(
                    &self.live_bodies,
                    ACTIVE_BODY_DISPATCH_64_OFFSET,
                );
            }
            {
                let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_sleep_propagation_pass"),
                    timestamp_writes: None,
                });
                pass.set_pipeline(&self.propagation_pipeline);
                pass.set_bind_group(0, group, &[]);
                pass.dispatch_workgroups_indirect(&self.point_state, 4 * size_of::<u32>() as u64);
            }
            source = 1 - source;
        }
        source
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
