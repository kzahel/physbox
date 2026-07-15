use bytemuck::{Pod, Zeroable};
use physbox_launch::ValidationScenario;

use crate::{
    EngineConfig, FrameInput, StepRequest,
    body_world::GpuBodyWorld,
    clock::{FIXED_STEP_HZ, FIXED_STEP_SECONDS, InteractiveClockInput, SimulationClock},
    gpu_solver::{GpuSolverCandidate, GpuSolverLab, GpuSolverMode},
    renderer::RenderFrameContext,
    report::{FrameReport, SCENE_CONTRACT_VERSION, ScenarioResult, SceneContract},
    scenario::{
        authored_chain_scenario, p5_1_direct_render_scenario, p5_1_rectangle_floor_scenario,
    },
};

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
    params_buffer: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
    solver_lab: Option<GpuSolverLab>,
    solver_bind_group: Option<wgpu::BindGroup>,
    solver_executed: bool,
    body_world: Option<GpuBodyWorld>,
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
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_bootstrap_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("bootstrap.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_bootstrap_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
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
        let solver_candidate = match config.scenario {
            ValidationScenario::Bootstrap
            | ValidationScenario::BodyFreeFlight
            | ValidationScenario::RigidFloor => None,
            ValidationScenario::SolverJacobi => Some(GpuSolverCandidate::JacobiGather),
            ValidationScenario::SolverColored => Some(GpuSolverCandidate::ColoredSoftTgs),
        };
        let solver_lab = solver_candidate.map(|candidate| {
            let mut scenario = authored_chain_scenario(64, 91)
                .expect("engine-owned solver validation scenario is valid");
            scenario.settings.gravity = physbox_contracts::Vec2::ZERO;
            GpuSolverLab::new(device, &scenario, candidate, GpuSolverMode::SoftStep)
                .expect("engine-owned solver validation lab is valid")
        });
        let solver_bind_group = solver_lab.as_ref().map(|lab| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("physbox5_solver_render_bind_group"),
                layout: &bind_group_layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: lab.body_buffer().as_entire_binding(),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: params_buffer.as_entire_binding(),
                    },
                ],
            })
        });
        let body_world = match config.scenario {
            ValidationScenario::BodyFreeFlight => {
                let scenario = p5_1_direct_render_scenario(3)
                    .expect("engine-owned body free-flight scenario is valid");
                Some(
                    GpuBodyWorld::new(device, color_format, &scenario)
                        .expect("engine-owned body free-flight GPU world is valid"),
                )
            }
            ValidationScenario::Bootstrap | ValidationScenario::RigidFloor => {
                let expected_steps = if config.scenario == ValidationScenario::Bootstrap {
                    3
                } else {
                    120
                };
                let scenario = p5_1_rectangle_floor_scenario(expected_steps)
                    .expect("engine-owned fixed-floor scenario is valid");
                Some(
                    GpuBodyWorld::new(device, color_format, &scenario)
                        .expect("engine-owned fixed-floor GPU world is valid"),
                )
            }
            _ => None,
        };

        Self {
            config,
            params_buffer,
            render_pipeline,
            solver_lab,
            solver_bind_group,
            solver_executed: false,
            body_world,
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
        let (steps, pacing) = match input.step_request {
            StepRequest::Interactive {
                elapsed_seconds,
                requested_time_scale,
                action,
            } => self.clock.admit(InteractiveClockInput {
                elapsed_seconds,
                requested_time_scale,
                action,
            }),
            StepRequest::Exact { steps } => (steps, self.clock.admit_exact(steps)),
        };

        let params = FrameParams {
            dt: FIXED_STEP_SECONDS as f32,
            width: context.target.size[0] as f32,
            height: context.target.size[1] as f32,
            frame_index: self.frame_index as f32,
        };
        context
            .queue
            .write_buffer(&self.params_buffer, 0, bytemuck::bytes_of(&params));

        if let Some(lab) = &self.solver_lab {
            for _ in 0..steps {
                lab.encode(context.encoder);
                self.solver_executed = true;
            }
        } else if let Some(world) = &self.body_world {
            world.encode_steps(context.encoder, steps);
        } else {
            unreachable!("every shared scenario owns a GPU mechanics path");
        }

        if let Some(world) = &self.body_world {
            world.encode_render(
                context.queue,
                context.encoder,
                context.target,
                self.frame_index,
            );
        } else {
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
            pass.set_bind_group(
                0,
                self.solver_bind_group
                    .as_ref()
                    .expect("solver laboratory owns its render binding"),
                &[],
            );
            pass.draw(0..3, 0..1);
        }

        self.frame_index += 1;
        self.fixed_steps_total += u64::from(steps);
        let contract = SceneContract {
            version: SCENE_CONTRACT_VERSION,
            engine: "physbox5",
            state_owner: "gpu",
            body_count: self.body_world.as_ref().map_or_else(
                || if self.solver_lab.is_some() { 64 } else { 1 },
                GpuBodyWorld::body_count,
            ),
            contact_point_count: self
                .body_world
                .as_ref()
                .map_or(0, GpuBodyWorld::contact_point_count),
            solver_dispatches_per_step: self
                .body_world
                .as_ref()
                .map_or(0, GpuBodyWorld::solver_dispatches_per_step),
            solver_capacity_bytes: self
                .body_world
                .as_ref()
                .map_or(0, GpuBodyWorld::solver_capacity_bytes),
            capacity_overflowed: self
                .body_world
                .as_ref()
                .is_some_and(GpuBodyWorld::solver_capacity_overflowed),
            fixed_step_hz: FIXED_STEP_HZ,
            scenario: self.config.scenario.name(),
        };
        let scenario = match self.config.scenario {
            ValidationScenario::Bootstrap => {
                let complete = self.fixed_steps_total >= 3;
                ScenarioResult {
                    name: "bootstrap",
                    complete,
                    passed: complete,
                    reason: if complete {
                        "shared GPU host advanced and rendered"
                    } else {
                        "waiting for shared GPU frames"
                    },
                }
            }
            ValidationScenario::BodyFreeFlight => {
                let complete = self.fixed_steps_total >= 3;
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete,
                    passed: complete,
                    reason: if complete {
                        "GPU SoA body integrated and rendered directly"
                    } else {
                        "waiting for three shared GPU body steps"
                    },
                }
            }
            ValidationScenario::RigidFloor => {
                let complete = self.fixed_steps_total >= 120;
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete,
                    passed: complete,
                    reason: if complete {
                        "selected GPU Jacobi-gather floor solve completed"
                    } else {
                        "waiting for 120 shared GPU fixed-floor steps"
                    },
                }
            }
            ValidationScenario::SolverJacobi | ValidationScenario::SolverColored => {
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete: self.solver_executed,
                    passed: self.solver_executed,
                    reason: if self.solver_executed {
                        "shared GPU authored-contact candidate dispatched"
                    } else {
                        "waiting for one shared GPU solver step"
                    },
                }
            }
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
    pub(crate) fn copy_capture_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) {
        if let Some(world) = &self.body_world {
            world.copy_capture_state(encoder, destination);
        } else {
            let source = self
                .solver_lab
                .as_ref()
                .expect("non-product captures belong to the solver laboratory")
                .body_buffer();
            encoder.copy_buffer_to_buffer(
                source,
                0,
                destination,
                0,
                (8 * std::mem::size_of::<f32>()) as u64,
            );
        }
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_capture_identity(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<(), String> {
        let world = self.body_world.as_ref().ok_or_else(|| {
            "capture identity is available only for the GPU body world".to_owned()
        })?;
        world.copy_capture_identity(encoder, destination);
        Ok(())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn floor_pass_evidence_layout(
        &self,
    ) -> Result<crate::body_floor_solver::FixedFloorEvidenceLayout, String> {
        self.body_world
            .as_ref()
            .ok_or_else(|| "pass evidence requires the GPU body world".to_owned())?
            .floor_pass_evidence_layout()
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_floor_pass_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<crate::body_floor_solver::FixedFloorEvidenceLayout, String> {
        self.body_world
            .as_ref()
            .ok_or_else(|| "pass evidence requires the GPU body world".to_owned())?
            .copy_floor_pass_evidence(encoder, destination)
    }
}
