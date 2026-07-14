use bytemuck::{Pod, Zeroable};
use physbox_launch::ValidationScenario;
use wgpu::util::DeviceExt;

use crate::{
    EngineConfig, FrameInput,
    clock::{FIXED_STEP_HZ, FIXED_STEP_SECONDS, InteractiveClockInput, SimulationClock},
    renderer::RenderFrameContext,
    report::{FrameReport, SCENE_CONTRACT_VERSION, ScenarioResult, SceneContract},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct BootstrapState {
    position: [f32; 2],
    velocity: [f32; 2],
    angle: f32,
    angular_velocity: f32,
    padding: [f32; 2],
}

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameParams {
    dt: f32,
    width: f32,
    height: f32,
    frame_index: f32,
}

pub struct EngineHost {
    config: EngineConfig,
    #[cfg(not(target_arch = "wasm32"))]
    state_buffer: wgpu::Buffer,
    params_buffer: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    compute_pipeline: wgpu::ComputePipeline,
    render_pipeline: wgpu::RenderPipeline,
    clock: SimulationClock,
    frame_index: u64,
    fixed_steps_total: u64,
    pointer_events_total: u64,
}

impl EngineHost {
    pub(crate) fn new(
        device: &wgpu::Device,
        color_format: wgpu::TextureFormat,
        config: EngineConfig,
    ) -> Self {
        let initial_state = BootstrapState {
            position: [-0.56, 0.25],
            velocity: [0.42, -0.18],
            angle: 0.18,
            angular_velocity: 0.72,
            padding: [0.0; 2],
        };
        let state_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_bootstrap_state"),
            contents: bytemuck::bytes_of(&initial_state),
            usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_SRC,
        });
        let params_buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_frame_params"),
            size: std::mem::size_of::<FrameParams>() as u64,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_bootstrap_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Storage { read_only: false },
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::COMPUTE | wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_bootstrap_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: state_buffer.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: params_buffer.as_entire_binding(),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_bootstrap_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bootstrap.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_bootstrap_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let compute_pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("physbox5_bootstrap_compute_pipeline"),
            layout: Some(&pipeline_layout),
            module: &shader,
            entry_point: Some("step_main"),
            compilation_options: Default::default(),
            cache: None,
        });
        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("physbox5_bootstrap_render_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[],
                compilation_options: Default::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fragment_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: color_format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: Default::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            config,
            #[cfg(not(target_arch = "wasm32"))]
            state_buffer,
            params_buffer,
            bind_group,
            compute_pipeline,
            render_pipeline,
            clock: SimulationClock::default(),
            frame_index: 0,
            fixed_steps_total: 0,
            pointer_events_total: 0,
        }
    }

    pub fn render_frame(
        &mut self,
        input: FrameInput,
        context: RenderFrameContext<'_>,
    ) -> FrameReport {
        self.pointer_events_total += input.pointer_events.len() as u64;
        let (steps, pacing) = self.clock.admit(InteractiveClockInput {
            elapsed_seconds: input.elapsed_seconds,
            requested_time_scale: input.requested_time_scale,
            action: input.clock_action,
        });

        let params = FrameParams {
            dt: FIXED_STEP_SECONDS as f32,
            width: context.target.size[0] as f32,
            height: context.target.size[1] as f32,
            frame_index: self.frame_index as f32,
        };
        context
            .queue
            .write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        for _ in 0..steps {
            let mut pass = context
                .encoder
                .begin_compute_pass(&wgpu::ComputePassDescriptor {
                    label: Some("physbox5_bootstrap_step"),
                    timestamp_writes: None,
                });
            pass.set_pipeline(&self.compute_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }

        {
            let mut pass = context
                .encoder
                .begin_render_pass(&wgpu::RenderPassDescriptor {
                    label: Some("physbox5_bootstrap_render"),
                    color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                        view: context.target.color_view,
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
            pass.set_pipeline(&self.render_pipeline);
            pass.set_bind_group(0, &self.bind_group, &[]);
            pass.draw(0..3, 0..1);
        }

        self.frame_index += 1;
        self.fixed_steps_total += u64::from(steps);
        let contract = SceneContract {
            version: SCENE_CONTRACT_VERSION,
            engine: "physbox5",
            state_owner: "gpu",
            body_count: 1,
            fixed_step_hz: FIXED_STEP_HZ,
            scenario: self.config.scenario.name(),
        };
        let complete = self.frame_index >= 5 && self.fixed_steps_total >= 3;
        let scenario = match self.config.scenario {
            ValidationScenario::Bootstrap => ScenarioResult {
                name: "bootstrap",
                complete,
                passed: complete,
                reason: if complete {
                    "shared GPU host advanced and rendered"
                } else {
                    "waiting for shared GPU frames"
                },
            },
        };
        FrameReport {
            frame_index: self.frame_index,
            fixed_steps_total: self.fixed_steps_total,
            fixed_steps_this_frame: steps,
            pointer_events_total: self.pointer_events_total,
            target_size: context.target.size,
            pacing,
            scene_contract: contract,
            scenario,
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn state_buffer(&self) -> &wgpu::Buffer {
        &self.state_buffer
    }
}
