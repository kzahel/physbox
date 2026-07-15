use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BodyCommandBatchV1, BodyCommandV1, GrabIntentBatchV1, GrabIntentV1, SCHEMA_VERSION,
    ScheduledBodyCommandV1, Vec2, canonical_hash,
};
use physbox_launch::ValidationScenario;

use crate::{
    AdapterDiagnostics, BODY_COMMAND_CAPACITY, BoundedBodyCommands, Camera2d, EngineConfig,
    FrameInput, InputEvent, PointerButton, PointerPhase, StepRequest,
    body_world::{BodyCommandStepRange, BodyStepPlan, GpuBodyWorld},
    clock::{FIXED_STEP_HZ, FIXED_STEP_SECONDS, InteractiveClockInput, SimulationClock},
    gpu_solver::{GpuSolverCandidate, GpuSolverLab, GpuSolverMode},
    grab::{BoundedGrabIntents, GRAB_INTENT_CAPACITY, empty_grab_intent_batch},
    input::{SharedInputState, WorldPointerEvent},
    renderer::RenderFrameContext,
    report::{
        BodyCommandReport, CameraReport, FrameReport, SCENE_CONTRACT_VERSION, ScenarioResult,
        SceneContract,
    },
    scenario::{
        P5_2_CENTERED_STACK_EVIDENCE_STEPS, P5_2GeneratedScenarioKind, authored_chain_scenario,
        p5_1_direct_render_scenario, p5_2_centered_rectangle_stack_scenario,
        p5_2_generated_floor_scenario, p5_2_generated_reset_grab_scenario, p5_2_generated_scenario,
    },
    ui::{UiAction, UiFrameModel, WorkbenchUi},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameParams {
    dt_frame: [f32; 4],
    camera: [f32; 4],
}

struct PreparedBodyCommands {
    batch: BodyCommandBatchV1,
    ranges: Vec<BodyCommandStepRange>,
    grab_batch: GrabIntentBatchV1,
    grab_ranges: Vec<BodyCommandStepRange>,
}

struct BodyInteractionState {
    scheduled: Vec<ScheduledBodyCommandV1>,
    accept_live_input: bool,
    active_pointer: Option<u64>,
    pending_body: BoundedBodyCommands,
    pending_grab: BoundedGrabIntents,
    next_sequence: u32,
    report: BodyCommandReport,
}

impl BodyInteractionState {
    fn new(scheduled: Vec<ScheduledBodyCommandV1>, accept_live_input: bool) -> Self {
        let next_sequence = scheduled
            .iter()
            .map(|scheduled| scheduled.command.sequence())
            .max()
            .unwrap_or(0)
            + 1;
        Self {
            scheduled,
            accept_live_input,
            active_pointer: None,
            pending_body: BoundedBodyCommands::new(BODY_COMMAND_CAPACITY)
                .expect("body command capacity is valid"),
            pending_grab: BoundedGrabIntents::new(GRAB_INTENT_CAPACITY)
                .expect("grab intent capacity is valid"),
            next_sequence,
            report: BodyCommandReport {
                grab_intent_contract_version: physbox_contracts::GRAB_INTENT_CONTRACT_VERSION,
                capacity_commands: BODY_COMMAND_CAPACITY,
                capacity_grab_intents: GRAB_INTENT_CAPACITY,
                grab_state_bytes: crate::grab::GRAB_STATE_BYTES,
                ..Default::default()
            },
        }
    }

    fn capture_pointer_events(&mut self, events: &[WorldPointerEvent]) {
        if !self.accept_live_input {
            return;
        }
        for event in events {
            let target = event.world_target;
            match (event.button, event.phase) {
                (PointerButton::Primary | PointerButton::Contact, PointerPhase::Down)
                    if self.active_pointer.is_none() =>
                {
                    self.active_pointer = Some(event.id);
                    let intent = GrabIntentV1::Begin {
                        sequence: self.take_sequence(),
                        world_target: target,
                    };
                    self.push_grab(intent);
                }
                (_, PointerPhase::Move) if self.active_pointer == Some(event.id) => {
                    if self.pending_grab.coalesce_target(target) {
                        self.report.coalesced_targets_total += 1;
                    } else {
                        let intent = GrabIntentV1::Target {
                            sequence: self.take_sequence(),
                            world_target: target,
                        };
                        self.push_grab(intent);
                    }
                }
                (_, PointerPhase::Up | PointerPhase::Cancel)
                    if self.active_pointer == Some(event.id) =>
                {
                    let intent = GrabIntentV1::End {
                        sequence: self.take_sequence(),
                    };
                    self.push_grab(intent);
                    self.active_pointer = None;
                }
                _ => {}
            }
        }
    }

    fn prepare(&mut self, steps: u32, completed_steps: u64) -> PreparedBodyCommands {
        self.report.grab_intent_dispatches_this_frame = 0;
        self.report.grab_constraint_dispatches_this_frame = steps;
        let mut ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize];
        if steps == 0 {
            self.update_grab_presentation_counts();
            return PreparedBodyCommands {
                batch: empty_command_batch(),
                ranges,
                grab_batch: empty_grab_intent_batch(),
                grab_ranges: vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize],
            };
        }
        if self.pending_body.overflowed() || self.pending_grab.overflowed() {
            self.report.overflowed = true;
            self.pending_body.clear();
            self.pending_grab.clear();
            self.update_grab_presentation_counts();
            return PreparedBodyCommands {
                batch: empty_command_batch(),
                ranges,
                grab_batch: empty_grab_intent_batch(),
                grab_ranges: vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize],
            };
        }
        let pending_body = self.pending_body.artifact().commands;
        let pending_grab = self.pending_grab.artifact().intents;
        let mut body_builder = BoundedBodyCommands::new(BODY_COMMAND_CAPACITY)
            .expect("body command capacity is valid");
        let mut grab_builder =
            BoundedGrabIntents::new(GRAB_INTENT_CAPACITY).expect("grab intent capacity is valid");
        let mut grab_ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize];
        for step_offset in 0..steps {
            let start = body_builder.len() as u32;
            if step_offset == 0 {
                for command in &pending_body {
                    if body_builder.push(command.clone()).is_err() {
                        self.report.overflowed = true;
                    }
                }
                for intent in &pending_grab {
                    if grab_builder.push(*intent).is_err() {
                        self.report.overflowed = true;
                    }
                }
                grab_ranges[0] = BodyCommandStepRange {
                    start: 0,
                    count: grab_builder.artifact().intents.len() as u32,
                };
            }
            let absolute_step = completed_steps + u64::from(step_offset);
            for scheduled in self
                .scheduled
                .iter()
                .filter(|scheduled| u64::from(scheduled.step) == absolute_step)
            {
                self.report.queued_commands_total += 1;
                if body_builder.push(scheduled.command.clone()).is_err() {
                    self.report.overflowed = true;
                }
            }
            ranges[step_offset as usize] = BodyCommandStepRange {
                start,
                count: body_builder.len() as u32 - start,
            };
        }
        self.pending_body.clear();
        self.pending_grab.clear();
        if self.report.overflowed || body_builder.overflowed() || grab_builder.overflowed() {
            self.update_grab_presentation_counts();
            return PreparedBodyCommands {
                batch: empty_command_batch(),
                ranges: vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize],
                grab_batch: empty_grab_intent_batch(),
                grab_ranges: vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize],
            };
        }
        let batch = body_builder.artifact();
        let grab_batch = grab_builder.artifact();
        for command in &batch.commands {
            self.report.executed_commands_total += 1;
            match command {
                BodyCommandV1::Reset { .. } => {
                    self.report.reset_commands_total += 1;
                    self.report.grab_pointer_active = false;
                }
                BodyCommandV1::GrabBegin { .. } | BodyCommandV1::GrabTarget { .. } => {
                    self.report.grab_pointer_active = true;
                }
                BodyCommandV1::GrabEnd { .. } => self.report.grab_pointer_active = false,
            }
        }
        self.report.executed_commands_total = self
            .report
            .executed_commands_total
            .saturating_add(grab_batch.intents.len() as u64);
        self.report.grab_intent_dispatches_this_frame =
            grab_ranges.iter().filter(|range| range.count != 0).count() as u32;
        self.update_grab_presentation_counts();
        PreparedBodyCommands {
            batch,
            ranges,
            grab_batch,
            grab_ranges,
        }
    }

    fn update_grab_presentation_counts(&mut self) {
        let active = u32::from(self.report.grab_pointer_active);
        self.report.grab_tether_prepare_dispatches_this_frame = active;
        self.report.grab_tether_draw_calls_this_frame = active;
    }

    fn take_sequence(&mut self) -> u32 {
        let sequence = self.next_sequence;
        self.next_sequence = self.next_sequence.checked_add(1).unwrap_or_else(|| {
            self.report.overflowed = true;
            self.next_sequence
        });
        sequence
    }

    fn push_grab(&mut self, intent: GrabIntentV1) {
        self.report.queued_commands_total += 1;
        self.report.grab_pointer_active = !matches!(intent, GrabIntentV1::End { .. });
        if self.pending_grab.push(intent).is_err() {
            self.report.overflowed = true;
        }
    }
}

fn empty_command_batch() -> BodyCommandBatchV1 {
    BodyCommandBatchV1 {
        schema_version: SCHEMA_VERSION,
        capacity_commands: BODY_COMMAND_CAPACITY,
        attempted_commands: 0,
        overflowed: false,
        commands: Vec::new(),
    }
}

pub struct EngineHost {
    config: EngineConfig,
    color_format: wgpu::TextureFormat,
    adapter: AdapterDiagnostics,
    params_buffer: wgpu::Buffer,
    render_pipeline: wgpu::RenderPipeline,
    solver_lab: Option<GpuSolverLab>,
    solver_bind_group: Option<wgpu::BindGroup>,
    solver_executed: bool,
    body_world: Option<GpuBodyWorld>,
    clock: SimulationClock,
    camera: Camera2d,
    input: SharedInputState,
    frame_index: u64,
    fixed_steps_total: u64,
    pointer_events_total: u64,
    scenario_hash: String,
    body_interaction: Option<BodyInteractionState>,
    ui: WorkbenchUi,
    scenario_activations_total: u64,
    scenario_restarts_total: u64,
}

impl EngineHost {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        config: EngineConfig,
        adapter: AdapterDiagnostics,
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
            | ValidationScenario::RigidFloor
            | ValidationScenario::RigidStack
            | ValidationScenario::RectangleStack
            | ValidationScenario::ResetGrab => None,
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
                let scenario = p5_2_generated_floor_scenario(expected_steps)
                    .expect("engine-owned generated-floor scenario is valid");
                Some(
                    GpuBodyWorld::new_generated(device, color_format, &scenario)
                        .expect("engine-owned generated-contact GPU world is valid"),
                )
            }
            ValidationScenario::ResetGrab => {
                let script = p5_2_generated_reset_grab_scenario()
                    .expect("engine-owned generated reset/grab scenario is valid");
                Some(
                    GpuBodyWorld::new_generated(device, color_format, &script.scenario)
                        .expect("engine-owned generated reset/grab GPU world is valid"),
                )
            }
            ValidationScenario::RigidStack => {
                let scenario = p5_2_generated_scenario(P5_2GeneratedScenarioKind::SmallStack, 180)
                    .expect("engine-owned generated stack scenario is valid");
                Some(
                    GpuBodyWorld::new_generated(device, color_format, &scenario)
                        .expect("engine-owned generated stack GPU world is valid"),
                )
            }
            ValidationScenario::RectangleStack => {
                let scenario =
                    p5_2_centered_rectangle_stack_scenario(4, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
                        .expect("engine-owned centered rectangle stack scenario is valid");
                Some(
                    GpuBodyWorld::new_generated(device, color_format, &scenario)
                        .expect("engine-owned centered rectangle stack GPU world is valid"),
                )
            }
            _ => None,
        };
        let scenario_hash = match config.scenario {
            ValidationScenario::BodyFreeFlight => canonical_hash(
                &p5_1_direct_render_scenario(3)
                    .expect("engine-owned body free-flight scenario is valid"),
            ),
            ValidationScenario::Bootstrap => canonical_hash(
                &p5_2_generated_floor_scenario(3)
                    .expect("engine-owned bootstrap generated scenario is valid"),
            ),
            ValidationScenario::RigidFloor => canonical_hash(
                &p5_2_generated_floor_scenario(120)
                    .expect("engine-owned generated-floor scenario is valid"),
            ),
            ValidationScenario::RigidStack => canonical_hash(
                &p5_2_generated_scenario(P5_2GeneratedScenarioKind::SmallStack, 180)
                    .expect("engine-owned generated stack scenario is valid"),
            ),
            ValidationScenario::RectangleStack => canonical_hash(
                &p5_2_centered_rectangle_stack_scenario(4, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
                    .expect("engine-owned centered rectangle stack scenario is valid"),
            ),
            ValidationScenario::ResetGrab => canonical_hash(
                &p5_2_generated_reset_grab_scenario()
                    .expect("engine-owned generated reset/grab scenario is valid"),
            ),
            ValidationScenario::SolverJacobi | ValidationScenario::SolverColored => {
                let mut scenario = authored_chain_scenario(64, 91)
                    .expect("engine-owned solver validation scenario is valid");
                scenario.settings.gravity = Vec2::ZERO;
                canonical_hash(&scenario)
            }
        }
        .expect("engine-owned scenario hash is valid");
        let body_interaction = match config.scenario {
            ValidationScenario::BodyFreeFlight
            | ValidationScenario::Bootstrap
            | ValidationScenario::RigidFloor
            | ValidationScenario::RigidStack
            | ValidationScenario::RectangleStack => {
                Some(BodyInteractionState::new(Vec::new(), true))
            }
            ValidationScenario::ResetGrab => {
                let script = p5_2_generated_reset_grab_scenario()
                    .expect("engine-owned generated reset/grab scenario is valid");
                Some(BodyInteractionState::new(script.scheduled_commands, false))
            }
            ValidationScenario::SolverJacobi | ValidationScenario::SolverColored => None,
        };

        let ui = WorkbenchUi::new(device, queue, color_format);
        Self {
            config,
            color_format,
            adapter,
            params_buffer,
            render_pipeline,
            solver_lab,
            solver_bind_group,
            solver_executed: false,
            body_world,
            clock: SimulationClock::default(),
            camera: Camera2d::fixed(),
            input: SharedInputState::default(),
            frame_index: 0,
            fixed_steps_total: 0,
            pointer_events_total: 0,
            scenario_hash,
            body_interaction,
            ui,
            scenario_activations_total: 0,
            scenario_restarts_total: 0,
        }
    }

    pub fn render_frame(
        &mut self,
        input: FrameInput,
        context: RenderFrameContext<'_>,
    ) -> FrameReport {
        let raw_pointer_count = input
            .input_events
            .iter()
            .filter(|event| matches!(event, InputEvent::Pointer(_)))
            .count() as u64;
        let mut routing = self
            .ui
            .route_events(&input.input_events, context.target.size);
        if let Some(action) = routing.action {
            match action {
                UiAction::Recenter => {
                    routing.events.push(InputEvent::Recenter);
                }
                UiAction::ActivateScenario(_) | UiAction::RestartScenario => {
                    let activations = self.scenario_activations_total.saturating_add(matches!(
                        action,
                        UiAction::ActivateScenario(_)
                    )
                        as u64);
                    let restarts = self.scenario_restarts_total.saturating_add(matches!(
                        action,
                        UiAction::RestartScenario
                    )
                        as u64);
                    let scenario = match action {
                        UiAction::ActivateScenario(scenario) => scenario,
                        UiAction::RestartScenario => self.config.scenario,
                        UiAction::Recenter => unreachable!(),
                    };
                    let mut replacement = Self::new(
                        context.device,
                        context.queue,
                        self.color_format,
                        EngineConfig { scenario },
                        self.adapter.clone(),
                    );
                    replacement.scenario_activations_total = activations;
                    replacement.scenario_restarts_total = restarts;
                    *self = replacement;
                }
            }
        }
        self.pointer_events_total = self.pointer_events_total.saturating_add(raw_pointer_count);
        let (world_events, _) =
            self.input
                .apply_events(&mut self.camera, &routing.events, context.target.size);
        if let Some(interaction) = &mut self.body_interaction {
            interaction.capture_pointer_events(&world_events);
        }
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

        let camera_frame = self
            .camera
            .resolve(context.target.size)
            .expect("nonzero frame target resolves the shared camera");
        let params = FrameParams {
            dt_frame: [
                FIXED_STEP_SECONDS as f32,
                self.frame_index as f32,
                context.target.size[0] as f32,
                context.target.size[1] as f32,
            ],
            camera: [
                camera_frame.center()[0],
                camera_frame.center()[1],
                camera_frame.half_extents()[0],
                camera_frame.half_extents()[1],
            ],
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
            let commands = self
                .body_interaction
                .as_mut()
                .expect("GPU body worlds own body command state")
                .prepare(steps, self.fixed_steps_total);
            world
                .encode_steps_with_grab_intents(
                    context.queue,
                    context.encoder,
                    steps,
                    BodyStepPlan {
                        batch: &commands.batch,
                        ranges: &commands.ranges,
                        grab_batch: &commands.grab_batch,
                        grab_ranges: &commands.grab_ranges,
                    },
                )
                .expect("engine-owned body command plan is valid");
        } else {
            unreachable!("every shared scenario owns a GPU mechanics path");
        }

        if let Some(world) = &self.body_world {
            let grab_pointer_active = self
                .body_interaction
                .as_ref()
                .is_some_and(|interaction| interaction.report.grab_pointer_active);
            world.encode_render(
                context.queue,
                context.encoder,
                context.target,
                self.frame_index,
                camera_frame,
                grab_pointer_active,
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
                .is_some_and(GpuBodyWorld::solver_capacity_overflowed)
                || self
                    .body_interaction
                    .as_ref()
                    .is_some_and(|interaction| interaction.report.overflowed),
            fixed_step_hz: FIXED_STEP_HZ,
            scenario: self.config.scenario.name(),
            scenario_hash: self.scenario_hash.clone(),
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
            ValidationScenario::RigidStack => {
                let complete = self.fixed_steps_total >= 180;
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete,
                    passed: complete,
                    reason: if complete {
                        "generated mixed-shape GPU stack completed"
                    } else {
                        "waiting for 180 generated stack steps"
                    },
                }
            }
            ValidationScenario::RectangleStack => {
                let complete = self.fixed_steps_total >= P5_2_CENTERED_STACK_EVIDENCE_STEPS as u64;
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete,
                    passed: complete,
                    reason: if complete {
                        "centered four-rectangle GPU stack completed its exact step contract"
                    } else {
                        "waiting for 1200 centered rectangle stack steps"
                    },
                }
            }
            ValidationScenario::ResetGrab => {
                let complete = self.fixed_steps_total >= 150;
                let commands = &self
                    .body_interaction
                    .as_ref()
                    .expect("reset/grab scenario owns body commands")
                    .report;
                let passed = complete
                    && commands.executed_commands_total == 6
                    && commands.reset_commands_total == 2
                    && !commands.grab_pointer_active
                    && !commands.overflowed;
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete,
                    passed,
                    reason: if passed {
                        "shared reset/grab/release command sequence completed"
                    } else if complete {
                        "reset/grab command counters did not match the shared scenario"
                    } else {
                        "waiting for 150 shared reset/grab steps"
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
        let ui = self.ui.encode(
            context.queue,
            context.encoder,
            context.target,
            UiFrameModel {
                scenario: self.config.scenario,
                pacing: &pacing,
                body_count: contract.body_count,
                contact_count: contract.contact_point_count,
                capacity_bytes: contract.solver_capacity_bytes,
                overflowed: contract.capacity_overflowed,
                adapter_backend: &self.adapter.backend,
                scenario_activations_total: self.scenario_activations_total,
                scenario_restarts_total: self.scenario_restarts_total,
            },
        );
        FrameReport {
            frame_index: self.frame_index,
            fixed_steps_total: self.fixed_steps_total,
            fixed_steps_this_frame: steps,
            pointer_events_total: self.pointer_events_total,
            target_size: context.target.size,
            pacing,
            camera: CameraReport {
                version: crate::CAMERA_STATE_VERSION,
                center: self.camera.center(),
                orthographic_half_height: self.camera.orthographic_half_height(),
                pan_changes_total: self.input.pan_changes_total(),
                zoom_changes_total: self.input.zoom_changes_total(),
                recenter_changes_total: self.input.recenter_changes_total(),
                touch_navigation_active: self.input.touch_camera_active(),
                secondary_pan_active: self.input.secondary_pan_active(),
            },
            ui,
            body_commands: self
                .body_interaction
                .as_ref()
                .map_or_else(BodyCommandReport::default, |interaction| {
                    interaction.report.clone()
                }),
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
    pub(crate) fn copy_grab_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<(), String> {
        let world = self
            .body_world
            .as_ref()
            .ok_or_else(|| "grab state is available only for the GPU body world".to_owned())?;
        world.copy_grab_state(encoder, destination);
        Ok(())
    }

    #[cfg(test)]
    pub(crate) fn overwrite_body_generation_for_test(
        &self,
        queue: &wgpu::Queue,
        slot: u32,
        generation: u32,
    ) {
        self.body_world
            .as_ref()
            .expect("test scenario owns a body world")
            .overwrite_body_generation_for_test(queue, slot, generation);
    }

    #[cfg(test)]
    pub(crate) fn overwrite_body_pose_for_test(
        &self,
        queue: &wgpu::Queue,
        slot: u32,
        pose: [f32; 4],
    ) {
        self.body_world
            .as_ref()
            .expect("test scenario owns a body world")
            .overwrite_body_pose_for_test(queue, slot, pose);
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
