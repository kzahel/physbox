use std::mem::size_of;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{ScenarioV1, Validate};
use wgpu::util::DeviceExt;

use crate::GpuBodyStorage;

const WORKGROUP_SIZE: u32 = 64;
const COMMAND_CAPACITY: u32 = 64;

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct IntegrationParamsRaw {
    dt_gravity: [f32; 4],
    counts: [u32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct RenderParamsRaw {
    viewport: [f32; 4],
}

pub(crate) struct GpuBodyWorld {
    #[cfg_attr(target_arch = "wasm32", allow(dead_code))]
    storage: GpuBodyStorage,
    integration_bind_group: wgpu::BindGroup,
    integration_pipeline: wgpu::ComputePipeline,
    render_params: wgpu::Buffer,
    render_bind_group: wgpu::BindGroup,
    background_pipeline: wgpu::RenderPipeline,
    body_pipeline: wgpu::RenderPipeline,
    body_count: u32,
}

impl GpuBodyWorld {
    pub(crate) fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        scenario: &ScenarioV1,
    ) -> Result<Self, String> {
        scenario.validate().map_err(|error| error.to_string())?;
        if !scenario.manifolds.is_empty() {
            return Err("free-flight GPU body world does not consume manifolds".to_owned());
        }
        let body_count = u32::try_from(scenario.bodies.len())
            .map_err(|_| "GPU body count exceeds u32".to_owned())?;
        if body_count == 0 {
            return Err("GPU body world requires at least one body".to_owned());
        }
        let storage = GpuBodyStorage::new(device, body_count, COMMAND_CAPACITY, &scenario.bodies)?;
        let integration_params = IntegrationParamsRaw {
            dt_gravity: [
                scenario.settings.fixed_dt.get(),
                scenario.settings.gravity.x,
                scenario.settings.gravity.y,
                0.0,
            ],
            counts: [body_count, 0, 0, 0],
        };
        let integration_params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_body_integration_params"),
            contents: bytemuck::bytes_of(&integration_params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let integration_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                label: Some("physbox5_body_integration_layout"),
                entries: &[
                    storage_entry(0, false, wgpu::ShaderStages::COMPUTE),
                    storage_entry(1, false, wgpu::ShaderStages::COMPUTE),
                    storage_entry(2, true, wgpu::ShaderStages::COMPUTE),
                    storage_entry(3, true, wgpu::ShaderStages::COMPUTE),
                    uniform_entry(4, wgpu::ShaderStages::COMPUTE),
                ],
            });
        let integration_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_body_integration_bind_group"),
            layout: &integration_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: storage.pose_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage.motion_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: storage.properties_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: storage.metadata_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 4,
                    resource: integration_params.as_entire_binding(),
                },
            ],
        });
        let integration_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_body_integration_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_integrate.wgsl").into()),
        });
        let integration_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_body_integration_pipeline_layout"),
                bind_group_layouts: &[&integration_layout],
                push_constant_ranges: &[],
            });
        let integration_pipeline =
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some("physbox5_body_integration_pipeline"),
                layout: Some(&integration_pipeline_layout),
                module: &integration_shader,
                entry_point: Some("integrate_bodies"),
                compilation_options: Default::default(),
                cache: None,
            });

        let render_params = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_body_render_params"),
            size: size_of::<RenderParamsRaw>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let render_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_body_render_layout"),
            entries: &[
                storage_entry(0, true, wgpu::ShaderStages::VERTEX),
                storage_entry(1, true, wgpu::ShaderStages::VERTEX),
                storage_entry(2, true, wgpu::ShaderStages::VERTEX),
                uniform_entry(3, wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT),
            ],
        });
        let render_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_body_render_bind_group"),
            layout: &render_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: storage.pose_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: storage.properties_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: storage.metadata_buffer().as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: render_params.as_entire_binding(),
                },
            ],
        });
        let render_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_body_render_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("body_render.wgsl").into()),
        });
        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("physbox5_body_render_pipeline_layout"),
                bind_group_layouts: &[&render_layout],
                push_constant_ranges: &[],
            });
        let pipeline = |label, vertex, fragment, blend| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(label),
                layout: Some(&render_pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &render_shader,
                    entry_point: Some(vertex),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &render_shader,
                    entry_point: Some(fragment),
                    targets: &[Some(wgpu::ColorTargetState {
                        format: color_format,
                        blend,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: wgpu::PrimitiveState::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState::default(),
                multiview: None,
                cache: None,
            })
        };
        let background_pipeline = pipeline(
            "physbox5_body_background_pipeline",
            "background_vertex",
            "background_fragment",
            None,
        );
        let body_pipeline = pipeline(
            "physbox5_body_render_pipeline",
            "body_vertex",
            "body_fragment",
            Some(wgpu::BlendState::ALPHA_BLENDING),
        );

        Ok(Self {
            storage,
            integration_bind_group,
            integration_pipeline,
            render_params,
            render_bind_group,
            background_pipeline,
            body_pipeline,
            body_count,
        })
    }

    pub(crate) fn encode_steps(&self, encoder: &mut wgpu::CommandEncoder, steps: u32) {
        for _ in 0..steps {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("physbox5_body_integration_pass"),
                timestamp_writes: None,
            });
            pass.set_pipeline(&self.integration_pipeline);
            pass.set_bind_group(0, &self.integration_bind_group, &[]);
            pass.dispatch_workgroups(self.body_count.div_ceil(WORKGROUP_SIZE), 1, 1);
        }
    }

    pub(crate) fn encode_render(
        &self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: crate::renderer::RenderFrameTarget<'_>,
        frame_index: u64,
    ) {
        let params = RenderParamsRaw {
            viewport: [
                target.size[0] as f32,
                target.size[1] as f32,
                target.size[0] as f32 / target.size[1].max(1) as f32,
                frame_index as f32,
            ],
        };
        queue.write_buffer(&self.render_params, 0, bytemuck::bytes_of(&params));
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox5_body_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.008,
                        g: 0.015,
                        b: 0.035,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_bind_group(0, &self.render_bind_group, &[]);
        pass.set_pipeline(&self.background_pipeline);
        pass.draw(0..3, 0..1);
        pass.set_pipeline(&self.body_pipeline);
        pass.draw(0..6, 0..self.body_count);
    }

    pub(crate) const fn body_count(&self) -> u32 {
        self.body_count
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_capture_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(
            self.storage.pose_buffer(),
            0,
            destination,
            0,
            size_of::<[f32; 4]>() as u64,
        );
        encoder.copy_buffer_to_buffer(
            self.storage.motion_buffer(),
            0,
            destination,
            size_of::<[f32; 4]>() as u64,
            size_of::<[f32; 4]>() as u64,
        );
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_capture_identity(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        encoder.copy_buffer_to_buffer(
            self.storage.metadata_buffer(),
            0,
            destination,
            0,
            size_of::<[u32; 4]>() as u64,
        );
    }
}

fn storage_entry(
    binding: u32,
    read_only: bool,
    visibility: wgpu::ShaderStages,
) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Storage { read_only },
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}

fn uniform_entry(binding: u32, visibility: wgpu::ShaderStages) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility,
        ty: wgpu::BindingType::Buffer {
            ty: wgpu::BufferBindingType::Uniform,
            has_dynamic_offset: false,
            min_binding_size: None,
        },
        count: None,
    }
}
