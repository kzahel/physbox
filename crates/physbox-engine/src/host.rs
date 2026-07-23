use bytemuck::{Pod, Zeroable};
use physbox_contracts::{
    BodyCommandBatchV1, BodyCommandV1, ConstructionIntentBatchV1, ConstructionIntentV1,
    ConstructionStateV1, GrabIntentBatchV1, GrabIntentV1, SCHEMA_VERSION, ScenarioV1,
    ScheduledBodyCommandV1, Vec2, canonical_hash,
};
use physbox_launch::{ValidationScenario, WorldCapacityPreset};

#[cfg(feature = "box2d-comparator")]
use crate::comparison::{ComparisonRenderPlan, ComparisonShadow};
use crate::{
    AdapterDiagnostics, BODY_COMMAND_CAPACITY, BoundedBodyCommands, Camera2d, EngineConfig,
    FrameInput, GpuSolverSchedulingPolicyV1, InputEvent, PointerButton, PointerPhase, StepRequest,
    body_world::{BodyCommandStepRange, BodyRenderPlan, BodyStepPlan, GpuBodyWorld},
    clock::{InteractiveClockInput, SimulationClock},
    construction::{
        BoundedConstructionIntents, CONSTRUCTION_INTENT_CAPACITY, empty_construction_intent_batch,
    },
    engine_selection::ComparisonEngine,
    gpu_solver::{GpuSolverCandidate, GpuSolverLab, GpuSolverMode},
    grab::{BoundedGrabIntents, GRAB_INTENT_CAPACITY, empty_grab_intent_batch},
    input::{SharedInputState, WorldPointerEvent},
    renderer::RenderFrameContext,
    report::{
        BodyCommandReport, CameraReport, ConstructionDiagnosticReport, FrameReport,
        SCENE_CONTRACT_VERSION, ScenarioResult, SceneContract, SleepCycleDiagnosticReport,
    },
    scenario::{
        P5_2_BUCKET_EVIDENCE_STEPS, P5_2_CENTERED_STACK_EVIDENCE_STEPS,
        P5_2_MANIFOLD_COLOR_VALIDATION_STEPS, P5_2BucketShape, P5_2GeneratedScenarioKind,
        P5_2SettlingReviewPolicy, authored_chain_scenario, p5_1_direct_render_scenario,
        p5_2_bucket_scenario, p5_2_centered_rectangle_stack_scenario,
        p5_2_generated_floor_scenario, p5_2_generated_reset_grab_scenario, p5_2_generated_scenario,
        p5_2_manifold_color_validation_scenario, p5_2_settling_review_scenario,
    },
    ui::{UiAction, UiFrameModel, WorkbenchTool, WorkbenchUi},
};

#[repr(C)]
#[derive(Clone, Copy, Pod, Zeroable)]
struct FrameParams {
    dt_frame: [f32; 4],
    camera: [f32; 4],
}

fn settling_review_lane(scenario: ValidationScenario) -> Option<(u32, P5_2SettlingReviewPolicy)> {
    match scenario {
        ValidationScenario::SettlingHard8 => Some((8, P5_2SettlingReviewPolicy::HardControl)),
        ValidationScenario::SettlingCandidate8 => {
            Some((8, P5_2SettlingReviewPolicy::SoftCandidate))
        }
        ValidationScenario::SettlingHard16 => Some((16, P5_2SettlingReviewPolicy::HardControl)),
        ValidationScenario::SettlingIdentity16 => {
            Some((16, P5_2SettlingReviewPolicy::IdentityOnly))
        }
        ValidationScenario::SettlingSoftness16 => {
            Some((16, P5_2SettlingReviewPolicy::SoftnessOnly))
        }
        ValidationScenario::SettlingCandidate16 => {
            Some((16, P5_2SettlingReviewPolicy::SoftCandidate))
        }
        _ => None,
    }
}

#[cfg(feature = "solver-lab")]
const fn solver_lab_stack_count(scenario: ValidationScenario) -> Option<u32> {
    match scenario {
        ValidationScenario::SolverLabStack2 => Some(2),
        ValidationScenario::SolverLabStack4 => Some(4),
        ValidationScenario::SolverLabStack8 => Some(8),
        ValidationScenario::SolverLabStack12 => Some(12),
        ValidationScenario::SolverLabStack14 => Some(14),
        ValidationScenario::SolverLabStack16 => Some(16),
        _ => None,
    }
}

fn configured_solver_scenario(config: &EngineConfig, mut scenario: ScenarioV1) -> ScenarioV1 {
    config
        .solver_profile
        .apply_to_scenario(&mut scenario)
        .expect("interactive solver profile accepts the shared scenario");
    scenario
}

fn configured_host_scenario(config: &EngineConfig) -> ScenarioV1 {
    match config.scenario {
        ValidationScenario::BodyFreeFlight => {
            p5_1_direct_render_scenario(3).expect("engine-owned body free-flight scenario is valid")
        }
        ValidationScenario::Bootstrap | ValidationScenario::RigidFloor => {
            let expected_steps = if config.scenario == ValidationScenario::Bootstrap {
                3
            } else {
                120
            };
            configured_solver_scenario(
                config,
                p5_2_generated_floor_scenario(expected_steps)
                    .expect("engine-owned generated-floor scenario is valid"),
            )
        }
        ValidationScenario::ResetGrab => configured_solver_scenario(
            config,
            p5_2_generated_reset_grab_scenario()
                .expect("engine-owned generated reset/grab scenario is valid")
                .scenario,
        ),
        ValidationScenario::RigidStack => configured_solver_scenario(
            config,
            p5_2_generated_scenario(P5_2GeneratedScenarioKind::SmallStack, 180)
                .expect("engine-owned generated stack scenario is valid"),
        ),
        ValidationScenario::RectangleStack => configured_solver_scenario(
            config,
            p5_2_centered_rectangle_stack_scenario(4, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
                .expect("engine-owned centered rectangle stack scenario is valid"),
        ),
        ValidationScenario::CircleBucket100 | ValidationScenario::RectangleBucket100 => {
            let shape = if config.scenario == ValidationScenario::CircleBucket100 {
                P5_2BucketShape::Circle
            } else {
                P5_2BucketShape::Rectangle
            };
            configured_solver_scenario(
                config,
                p5_2_bucket_scenario(shape, 100, P5_2_BUCKET_EVIDENCE_STEPS)
                    .expect("engine-owned all-pairs bucket scenario is valid"),
            )
        }
        ValidationScenario::SolverManifoldI2 | ValidationScenario::SolverParallelColoring => {
            configured_solver_scenario(
                config,
                p5_2_manifold_color_validation_scenario()
                    .expect("engine-owned manifold-color validation scenario is valid"),
            )
        }
        ValidationScenario::SettlingHard8
        | ValidationScenario::SettlingCandidate8
        | ValidationScenario::SettlingHard16
        | ValidationScenario::SettlingIdentity16
        | ValidationScenario::SettlingSoftness16
        | ValidationScenario::SettlingCandidate16 => {
            let (rectangle_count, policy) = settling_review_lane(config.scenario)
                .expect("settling review launch scenario maps to a shared policy");
            configured_solver_scenario(
                config,
                p5_2_settling_review_scenario(rectangle_count, policy)
                    .expect("engine-owned settling review scenario is valid"),
            )
        }
        ValidationScenario::SolverJacobi | ValidationScenario::SolverColored => {
            let mut scenario = authored_chain_scenario(64, 91)
                .expect("engine-owned solver validation scenario is valid");
            scenario.settings.gravity = Vec2::ZERO;
            scenario
        }
        #[cfg(feature = "solver-lab")]
        ValidationScenario::SolverLabStack2
        | ValidationScenario::SolverLabStack4
        | ValidationScenario::SolverLabStack8
        | ValidationScenario::SolverLabStack12
        | ValidationScenario::SolverLabStack14
        | ValidationScenario::SolverLabStack16 => {
            let count = solver_lab_stack_count(config.scenario)
                .expect("solver-lab stack scenario maps to a rectangle count");
            configured_solver_scenario(
                config,
                p5_2_centered_rectangle_stack_scenario(count, P5_2_CENTERED_STACK_EVIDENCE_STEPS)
                    .expect("engine-owned solver-lab stack scenario is valid"),
            )
        }
    }
}

fn profiled_generated_world(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    scenario: &ScenarioV1,
    profile: &crate::SolverProfileConfig,
    world_capacity: WorldCapacityPreset,
) -> Result<GpuBodyWorld, String> {
    GpuBodyWorld::new_generated_with_solver_profile(
        device,
        color_format,
        scenario,
        world_capacity,
        profile,
    )
}

fn profiled_generated_world_with_scheduling(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    scenario: &ScenarioV1,
    profile: &crate::SolverProfileConfig,
    world_capacity: WorldCapacityPreset,
    scheduling: GpuSolverSchedulingPolicyV1,
) -> Result<GpuBodyWorld, String> {
    if !profile.is_selected() {
        return GpuBodyWorld::new_generated_with_solver_profile(
            device,
            color_format,
            scenario,
            world_capacity,
            profile,
        );
    }
    GpuBodyWorld::new_generated_with_scheduling(
        device,
        color_format,
        scenario,
        world_capacity,
        scheduling,
    )
}

fn profiled_generated_world_with_contact_policy(
    device: &wgpu::Device,
    color_format: wgpu::TextureFormat,
    scenario: &ScenarioV1,
    profile: &crate::SolverProfileConfig,
    world_capacity: WorldCapacityPreset,
    identity: physbox_contracts::ContactIdentityPolicyV1,
) -> Result<GpuBodyWorld, String> {
    if !profile.is_selected() {
        return GpuBodyWorld::new_generated_with_solver_profile(
            device,
            color_format,
            scenario,
            world_capacity,
            profile,
        );
    }
    GpuBodyWorld::new_generated_with_contact_policy(
        device,
        color_format,
        scenario,
        world_capacity,
        identity,
    )
}

struct PreparedBodyCommands {
    batch: BodyCommandBatchV1,
    ranges: Vec<BodyCommandStepRange>,
    grab_batch: GrabIntentBatchV1,
    grab_ranges: Vec<BodyCommandStepRange>,
    construction_batch: ConstructionIntentBatchV1,
    construction_ranges: Vec<BodyCommandStepRange>,
}

struct ConfigurationReplacement {
    config: EngineConfig,
    selected_tool: WorkbenchTool,
    scenario_activation: bool,
    scenario_restart: bool,
    preserve_camera: bool,
    solver_profiles: Vec<crate::SolverProfileConfig>,
}

struct BodyInteractionState {
    scheduled: Vec<ScheduledBodyCommandV1>,
    accept_live_input: bool,
    active_pointer: Option<ActiveToolPointer>,
    tool: WorkbenchTool,
    pending_body: BoundedBodyCommands,
    pending_grab: BoundedGrabIntents,
    pending_construction: BoundedConstructionIntents,
    next_sequence: u32,
    report: BodyCommandReport,
}

#[derive(Clone, Copy, Debug)]
struct ActiveToolPointer {
    id: u64,
    tool: WorkbenchTool,
    down_target: Vec2,
    current_target: Vec2,
    active_hold_seconds: f64,
    just_started: bool,
    repeated: bool,
}

const CONSTRUCTION_HOLD_DELAY_SECONDS: f64 = 0.25;
const SLEEP_WAKE_FLASH_MILLISECONDS: u32 = 160;
const CONSTRUCTION_REPEAT_INTERVAL_SECONDS: f64 = 0.10;

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
            tool: WorkbenchTool::Grab,
            pending_body: BoundedBodyCommands::new(BODY_COMMAND_CAPACITY)
                .expect("body command capacity is valid"),
            pending_grab: BoundedGrabIntents::new(GRAB_INTENT_CAPACITY)
                .expect("grab intent capacity is valid"),
            pending_construction: BoundedConstructionIntents::new(CONSTRUCTION_INTENT_CAPACITY)
                .expect("construction intent capacity is valid"),
            next_sequence,
            report: BodyCommandReport {
                grab_intent_contract_version: physbox_contracts::GRAB_INTENT_CONTRACT_VERSION,
                construction_intent_contract_version:
                    physbox_contracts::CONSTRUCTION_INTENT_CONTRACT_VERSION,
                capacity_commands: BODY_COMMAND_CAPACITY,
                capacity_grab_intents: GRAB_INTENT_CAPACITY,
                capacity_construction_intents: CONSTRUCTION_INTENT_CAPACITY,
                grab_state_bytes: crate::grab::GRAB_STATE_BYTES,
                ..Default::default()
            },
        }
    }

    fn select_tool(&mut self, tool: WorkbenchTool) {
        if self.tool == tool {
            return;
        }
        if self
            .active_pointer
            .is_some_and(|active| active.tool == WorkbenchTool::Grab)
        {
            let intent = GrabIntentV1::End {
                sequence: self.take_sequence(),
            };
            self.push_grab(intent);
        }
        self.active_pointer = None;
        self.report.grab_pointer_active = false;
        self.report.construction_pointer_active = false;
        self.tool = tool;
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
                    self.active_pointer = Some(ActiveToolPointer {
                        id: event.id,
                        tool: self.tool,
                        down_target: target,
                        current_target: target,
                        active_hold_seconds: 0.0,
                        just_started: true,
                        repeated: false,
                    });
                    if self.tool == WorkbenchTool::Grab {
                        let intent = GrabIntentV1::Begin {
                            sequence: self.take_sequence(),
                            world_target: target,
                        };
                        self.push_grab(intent);
                    } else {
                        self.report.construction_pointer_active = true;
                    }
                }
                (_, PointerPhase::Move)
                    if self
                        .active_pointer
                        .is_some_and(|active| active.id == event.id) =>
                {
                    let active = self.active_pointer.as_mut().expect("active pointer exists");
                    active.current_target = target;
                    if active.tool == WorkbenchTool::Grab {
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
                }
                (_, PointerPhase::Up | PointerPhase::Cancel)
                    if self
                        .active_pointer
                        .is_some_and(|active| active.id == event.id) =>
                {
                    let active = self.active_pointer.expect("active pointer exists");
                    if active.tool == WorkbenchTool::Grab {
                        let intent = GrabIntentV1::End {
                            sequence: self.take_sequence(),
                        };
                        self.push_grab(intent);
                    } else if event.phase == PointerPhase::Up && !active.repeated {
                        let sequence = self.take_sequence();
                        let intent = construction_intent(active.tool, sequence, active.down_target)
                            .expect("non-grab tools produce construction intents");
                        self.push_construction(intent);
                    }
                    self.active_pointer = None;
                    self.report.construction_pointer_active = false;
                }
                _ => {}
            }
        }
    }

    fn prepare(
        &mut self,
        steps: u32,
        completed_steps: u64,
        active_hold_seconds: f64,
    ) -> PreparedBodyCommands {
        self.report.grab_intent_dispatches_this_frame = 0;
        self.report.construction_intent_dispatches_this_frame = 0;
        self.report.grab_constraint_dispatches_this_frame = steps;
        let mut ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize];
        self.advance_construction_hold(active_hold_seconds, steps != 0);
        if steps == 0 {
            self.update_presentation_counts();
            return PreparedBodyCommands {
                batch: empty_command_batch(),
                ranges,
                grab_batch: empty_grab_intent_batch(),
                grab_ranges: vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize],
                construction_batch: empty_construction_intent_batch(),
                construction_ranges: vec![
                    BodyCommandStepRange { start: 0, count: 0 };
                    steps as usize
                ],
            };
        }
        if self.pending_body.overflowed()
            || self.pending_grab.overflowed()
            || self.pending_construction.overflowed()
        {
            self.report.overflowed = true;
            self.pending_body.clear();
            self.pending_grab.clear();
            self.pending_construction.clear();
            self.update_presentation_counts();
            return PreparedBodyCommands {
                batch: empty_command_batch(),
                ranges,
                grab_batch: empty_grab_intent_batch(),
                grab_ranges: vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize],
                construction_batch: empty_construction_intent_batch(),
                construction_ranges: vec![
                    BodyCommandStepRange { start: 0, count: 0 };
                    steps as usize
                ],
            };
        }
        let pending_body = self.pending_body.artifact().commands;
        let pending_grab = self.pending_grab.artifact().intents;
        let pending_construction = self.pending_construction.artifact().intents;
        let mut body_builder = BoundedBodyCommands::new(BODY_COMMAND_CAPACITY)
            .expect("body command capacity is valid");
        let mut grab_builder =
            BoundedGrabIntents::new(GRAB_INTENT_CAPACITY).expect("grab intent capacity is valid");
        let mut construction_builder =
            BoundedConstructionIntents::new(CONSTRUCTION_INTENT_CAPACITY)
                .expect("construction intent capacity is valid");
        let mut grab_ranges = vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize];
        let mut construction_ranges =
            vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize];
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
                for intent in &pending_construction {
                    if construction_builder.push(*intent).is_err() {
                        self.report.overflowed = true;
                    }
                }
                construction_ranges[0] = BodyCommandStepRange {
                    start: 0,
                    count: construction_builder.len() as u32,
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
        self.pending_construction.clear();
        if self.report.overflowed
            || body_builder.overflowed()
            || grab_builder.overflowed()
            || construction_builder.overflowed()
        {
            self.update_presentation_counts();
            return PreparedBodyCommands {
                batch: empty_command_batch(),
                ranges: vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize],
                grab_batch: empty_grab_intent_batch(),
                grab_ranges: vec![BodyCommandStepRange { start: 0, count: 0 }; steps as usize],
                construction_batch: empty_construction_intent_batch(),
                construction_ranges: vec![
                    BodyCommandStepRange { start: 0, count: 0 };
                    steps as usize
                ],
            };
        }
        let batch = body_builder.artifact();
        let grab_batch = grab_builder.artifact();
        let construction_batch = construction_builder.artifact();
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
                BodyCommandV1::GrabEnd { .. } | BodyCommandV1::GrabClear { .. } => {
                    self.report.grab_pointer_active = false;
                }
            }
        }
        self.report.executed_commands_total = self
            .report
            .executed_commands_total
            .saturating_add(grab_batch.intents.len() as u64);
        self.report.executed_commands_total = self
            .report
            .executed_commands_total
            .saturating_add(construction_batch.intents.len() as u64);
        self.report.executed_construction_intents_total = self
            .report
            .executed_construction_intents_total
            .saturating_add(construction_batch.intents.len() as u64);
        self.report.grab_intent_dispatches_this_frame =
            grab_ranges.iter().filter(|range| range.count != 0).count() as u32;
        self.report.construction_intent_dispatches_this_frame = construction_ranges
            .iter()
            .filter(|range| range.count != 0)
            .count() as u32;
        self.update_presentation_counts();
        PreparedBodyCommands {
            batch,
            ranges,
            grab_batch,
            grab_ranges,
            construction_batch,
            construction_ranges,
        }
    }

    fn update_presentation_counts(&mut self) {
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

    fn push_construction(&mut self, intent: ConstructionIntentV1) {
        self.report.queued_commands_total = self.report.queued_commands_total.saturating_add(1);
        self.report.queued_construction_intents_total = self
            .report
            .queued_construction_intents_total
            .saturating_add(1);
        if self.pending_construction.push(intent).is_err() {
            self.report.overflowed = true;
        }
    }

    fn advance_construction_hold(&mut self, elapsed_seconds: f64, can_emit: bool) {
        let Some(active) = &mut self.active_pointer else {
            return;
        };
        if !matches!(
            active.tool,
            WorkbenchTool::Circle | WorkbenchTool::Rectangle
        ) {
            return;
        }
        if active.just_started {
            active.just_started = false;
            return;
        }
        let delay_seconds = if active.repeated {
            CONSTRUCTION_REPEAT_INTERVAL_SECONDS
        } else {
            CONSTRUCTION_HOLD_DELAY_SECONDS
        };
        active.active_hold_seconds =
            (active.active_hold_seconds + elapsed_seconds.max(0.0)).min(delay_seconds);
        if !can_emit || active.active_hold_seconds + 1.0e-9 < delay_seconds {
            return;
        }

        let tool = active.tool;
        let target = active.current_target;
        active.active_hold_seconds = 0.0;
        active.repeated = true;
        let sequence = self.take_sequence();
        let intent = construction_intent(tool, sequence, target)
            .expect("repeat-enabled tools produce construction intents");
        self.push_construction(intent);
    }
}

fn construction_intent(
    tool: WorkbenchTool,
    sequence: u32,
    world_target: Vec2,
) -> Option<ConstructionIntentV1> {
    match tool {
        WorkbenchTool::Circle => Some(ConstructionIntentV1::PlaceCircle {
            sequence,
            world_target,
        }),
        WorkbenchTool::Rectangle => Some(ConstructionIntentV1::PlaceRectangle {
            sequence,
            world_target,
        }),
        WorkbenchTool::Erase => Some(ConstructionIntentV1::EraseAt {
            sequence,
            world_target,
        }),
        WorkbenchTool::Grab => None,
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

#[cfg(feature = "box2d-comparator")]
fn target_visible_box2d_interactions(
    commands: &mut PreparedBodyCommands,
    shadow: &mut ComparisonShadow,
) -> (
    Vec<BodyCommandV1>,
    Vec<BodyCommandStepRange>,
    Vec<GrabIntentV1>,
    Vec<BodyCommandStepRange>,
) {
    for intent in &mut commands.construction_batch.intents {
        match shadow.resolve_box2d_construction(*intent) {
            Ok(targeted) => *intent = targeted,
            Err(error) => shadow.fail(error),
        }
    }

    let original_grab = commands.grab_batch.intents.clone();
    let original_grab_ranges = commands.grab_ranges.clone();
    let original_body = commands.batch.commands.clone();
    let original_body_ranges = commands.ranges.clone();
    let mut active = shadow.active_grab();
    let mut targeted = Vec::with_capacity(original_body.len() + original_grab.len());
    let mut ranges = Vec::with_capacity(original_body_ranges.len());
    for step in 0..original_body_ranges.len() {
        let start = targeted.len();
        let body_range = original_body_ranges[step];
        targeted.extend_from_slice(
            &original_body
                [body_range.start as usize..(body_range.start + body_range.count) as usize],
        );
        let grab_range = original_grab_ranges[step];
        for intent in &original_grab
            [grab_range.start as usize..(grab_range.start + grab_range.count) as usize]
        {
            let command = match *intent {
                GrabIntentV1::Begin {
                    sequence,
                    world_target,
                } => {
                    active = shadow.hit_test(world_target).unwrap_or(None);
                    if let Some(body) = active {
                        shadow.expect_box2d_grab_feedback(sequence, body);
                    }
                    Some(
                        active.map_or(BodyCommandV1::GrabClear { sequence }, |body| {
                            BodyCommandV1::GrabBegin {
                                sequence,
                                body,
                                world_target,
                            }
                        }),
                    )
                }
                GrabIntentV1::Target {
                    sequence,
                    world_target,
                } => active.map(|body| BodyCommandV1::GrabTarget {
                    sequence,
                    body,
                    world_target,
                }),
                GrabIntentV1::End { sequence } => {
                    let command = Some(
                        active.map_or(BodyCommandV1::GrabClear { sequence }, |body| {
                            BodyCommandV1::GrabEnd { sequence, body }
                        }),
                    );
                    active = None;
                    command
                }
            };
            if let Some(command) = command {
                targeted.push(command);
            }
        }
        targeted[start..].sort_by_key(BodyCommandV1::sequence);
        ranges.push(BodyCommandStepRange {
            start: start as u32,
            count: (targeted.len() - start) as u32,
        });
    }
    if targeted.len() <= BODY_COMMAND_CAPACITY as usize {
        commands.batch.commands = targeted;
        commands.batch.attempted_commands = commands.batch.commands.len() as u64;
        commands.ranges = ranges;
        commands.grab_batch = empty_grab_intent_batch();
        commands.grab_ranges =
            vec![BodyCommandStepRange { start: 0, count: 0 }; original_grab_ranges.len()];
    } else {
        shadow.fail(format!(
            "Box2D stable-ID targeting produced {} commands for GPU capacity {BODY_COMMAND_CAPACITY}",
            targeted.len()
        ));
        commands.batch = empty_command_batch();
        commands.ranges =
            vec![BodyCommandStepRange { start: 0, count: 0 }; original_body_ranges.len()];
        commands.grab_batch = empty_grab_intent_batch();
        commands.grab_ranges =
            vec![BodyCommandStepRange { start: 0, count: 0 }; original_grab_ranges.len()];
    }
    (
        original_body,
        original_body_ranges,
        original_grab,
        original_grab_ranges,
    )
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
    #[cfg(feature = "box2d-comparator")]
    comparison_shadow: ComparisonShadow,
    selected_engine: ComparisonEngine,
    clock: SimulationClock,
    camera: Camera2d,
    input: SharedInputState,
    frame_index: u64,
    fixed_steps_total: u64,
    pointer_events_total: u64,
    scenario_hash: String,
    physics_configuration_hash: String,
    body_interaction: Option<BodyInteractionState>,
    ui: WorkbenchUi,
    scenario_activations_total: u64,
    scenario_restarts_total: u64,
    solver_profiles: Vec<crate::SolverProfileConfig>,
    construction_diagnostic: Option<(ConstructionStateV1, u64)>,
    sleep_cycle_diagnostic: Option<(crate::SleepCycleMetricsV1, u64)>,
}

impl EngineHost {
    pub(crate) fn launch_preference_selection(&self) -> physbox_launch::LaunchPreferenceSelection {
        self.config.launch_preference_selection()
    }

    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        config: EngineConfig,
        adapter: AdapterDiagnostics,
    ) -> Self {
        assert!(
            config.solver_profile.supports_scenario(config.scenario),
            "solver profile {} does not support scenario {}",
            config.solver_profile.name(),
            config.scenario.name(),
        );
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
        let scenario = configured_host_scenario(&config);
        let solver_candidate = match config.scenario {
            ValidationScenario::Bootstrap
            | ValidationScenario::BodyFreeFlight
            | ValidationScenario::RigidFloor
            | ValidationScenario::RigidStack
            | ValidationScenario::RectangleStack
            | ValidationScenario::CircleBucket100
            | ValidationScenario::RectangleBucket100
            | ValidationScenario::ResetGrab
            | ValidationScenario::SolverManifoldI2
            | ValidationScenario::SolverParallelColoring
            | ValidationScenario::SettlingHard8
            | ValidationScenario::SettlingCandidate8
            | ValidationScenario::SettlingHard16
            | ValidationScenario::SettlingIdentity16
            | ValidationScenario::SettlingSoftness16
            | ValidationScenario::SettlingCandidate16 => None,
            #[cfg(feature = "solver-lab")]
            ValidationScenario::SolverLabStack2
            | ValidationScenario::SolverLabStack4
            | ValidationScenario::SolverLabStack8
            | ValidationScenario::SolverLabStack12
            | ValidationScenario::SolverLabStack14
            | ValidationScenario::SolverLabStack16 => None,
            ValidationScenario::SolverJacobi => Some(GpuSolverCandidate::JacobiGather),
            ValidationScenario::SolverColored => Some(GpuSolverCandidate::ColoredSoftTgs),
        };
        let solver_lab = solver_candidate.map(|candidate| {
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
            ValidationScenario::BodyFreeFlight => Some(
                GpuBodyWorld::new(device, color_format, &scenario)
                    .expect("engine-owned body free-flight GPU world is valid"),
            ),
            ValidationScenario::Bootstrap
            | ValidationScenario::RigidFloor
            | ValidationScenario::ResetGrab
            | ValidationScenario::RigidStack
            | ValidationScenario::RectangleStack
            | ValidationScenario::CircleBucket100
            | ValidationScenario::RectangleBucket100 => Some(
                profiled_generated_world(
                    device,
                    color_format,
                    &scenario,
                    &config.solver_profile,
                    config.world_capacity,
                )
                .expect("engine-owned generated GPU world is valid"),
            ),
            ValidationScenario::SolverManifoldI2 | ValidationScenario::SolverParallelColoring => {
                let scheduling_policy =
                    if config.scenario == ValidationScenario::SolverParallelColoring {
                        GpuSolverSchedulingPolicyV1::SELECTED
                    } else {
                        GpuSolverSchedulingPolicyV1::SERIAL_CONTROL
                    };
                let world = profiled_generated_world_with_scheduling(
                    device,
                    color_format,
                    &scenario,
                    &config.solver_profile,
                    config.world_capacity,
                    scheduling_policy,
                );
                Some(world.expect("engine-owned manifold-color validation world is valid"))
            }
            ValidationScenario::SettlingHard8
            | ValidationScenario::SettlingCandidate8
            | ValidationScenario::SettlingHard16
            | ValidationScenario::SettlingIdentity16
            | ValidationScenario::SettlingSoftness16
            | ValidationScenario::SettlingCandidate16 => {
                let (_, policy) = settling_review_lane(config.scenario)
                    .expect("settling review launch scenario maps to a shared policy");
                Some(
                    profiled_generated_world_with_contact_policy(
                        device,
                        color_format,
                        &scenario,
                        &config.solver_profile,
                        config.world_capacity,
                        policy.identity_policy(),
                    )
                    .expect("engine-owned settling review GPU world is valid"),
                )
            }
            #[cfg(feature = "solver-lab")]
            ValidationScenario::SolverLabStack2
            | ValidationScenario::SolverLabStack4
            | ValidationScenario::SolverLabStack8
            | ValidationScenario::SolverLabStack12
            | ValidationScenario::SolverLabStack14
            | ValidationScenario::SolverLabStack16 => Some(
                profiled_generated_world(
                    device,
                    color_format,
                    &scenario,
                    &config.solver_profile,
                    config.world_capacity,
                )
                .expect("engine-owned solver-lab GPU world is valid"),
            ),
            _ => None,
        };
        let scenario_hash = canonical_hash(&scenario).expect("engine-owned scenario hash is valid");
        #[cfg(feature = "box2d-comparator")]
        let comparison_shadow = {
            let capacity = body_world
                .as_ref()
                .map_or(config.world_capacity.slots(), GpuBodyWorld::body_capacity)
                .max(scenario.bodies.len() as u32);
            ComparisonShadow::new(
                device,
                color_format,
                &scenario,
                capacity,
                config.solver_profile.physics().sleeping_enabled,
                config.solver_profile.physics().sleep_speed_threshold,
            )
            .expect("pinned Box2D comparator accepts the configured shared scenario")
        };
        let body_interaction = match config.scenario {
            ValidationScenario::BodyFreeFlight
            | ValidationScenario::Bootstrap
            | ValidationScenario::RigidFloor
            | ValidationScenario::RigidStack
            | ValidationScenario::RectangleStack
            | ValidationScenario::CircleBucket100
            | ValidationScenario::RectangleBucket100
            | ValidationScenario::SolverManifoldI2
            | ValidationScenario::SolverParallelColoring
            | ValidationScenario::SettlingHard8
            | ValidationScenario::SettlingCandidate8
            | ValidationScenario::SettlingHard16
            | ValidationScenario::SettlingIdentity16
            | ValidationScenario::SettlingSoftness16
            | ValidationScenario::SettlingCandidate16 => {
                Some(BodyInteractionState::new(Vec::new(), true))
            }
            #[cfg(feature = "solver-lab")]
            ValidationScenario::SolverLabStack2
            | ValidationScenario::SolverLabStack4
            | ValidationScenario::SolverLabStack8
            | ValidationScenario::SolverLabStack12
            | ValidationScenario::SolverLabStack14
            | ValidationScenario::SolverLabStack16 => {
                Some(BodyInteractionState::new(Vec::new(), true))
            }
            ValidationScenario::ResetGrab => {
                let script = p5_2_generated_reset_grab_scenario()
                    .expect("engine-owned generated reset/grab scenario is valid");
                Some(BodyInteractionState::new(script.scheduled_commands, false))
            }
            ValidationScenario::SolverJacobi | ValidationScenario::SolverColored => None,
        };

        let physics_configuration_hash = config.physics_configuration_hash();
        let mut solver_profiles = crate::SolverProfileConfig::available();
        crate::ui::remember_solver_profile(&mut solver_profiles, config.solver_profile.clone());
        let ui = WorkbenchUi::new(
            device,
            queue,
            color_format,
            config.solver_profile.clone(),
            solver_profiles.clone(),
        );
        let clock = SimulationClock::new(config.solver_profile.physics().fixed_step_hz);
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
            #[cfg(feature = "box2d-comparator")]
            comparison_shadow,
            selected_engine: ComparisonEngine::default(),
            clock,
            camera: Camera2d::fixed(),
            input: SharedInputState::default(),
            frame_index: 0,
            fixed_steps_total: 0,
            pointer_events_total: 0,
            scenario_hash,
            physics_configuration_hash,
            body_interaction,
            ui,
            scenario_activations_total: 0,
            scenario_restarts_total: 0,
            solver_profiles,
            construction_diagnostic: None,
            sleep_cycle_diagnostic: None,
        }
    }

    fn replace_configuration(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        replacement: ConfigurationReplacement,
    ) {
        let ConfigurationReplacement {
            config,
            selected_tool,
            scenario_activation,
            scenario_restart,
            preserve_camera,
            solver_profiles,
        } = replacement;
        let camera = self.camera;
        let sleep_visuals_enabled = self.ui.sleep_visuals_enabled();
        let selected_engine = self.selected_engine;
        let mut replacement = Self::new(
            device,
            queue,
            self.color_format,
            config,
            self.adapter.clone(),
        );
        replacement.scenario_activations_total = self
            .scenario_activations_total
            .saturating_add(u64::from(scenario_activation));
        replacement.scenario_restarts_total = self
            .scenario_restarts_total
            .saturating_add(u64::from(scenario_restart));
        replacement.solver_profiles = solver_profiles.clone();
        replacement
            .ui
            .set_solver_profiles(solver_profiles, replacement.config.solver_profile.clone());
        replacement.ui.select_tool(selected_tool);
        replacement
            .ui
            .set_sleep_visuals_enabled(sleep_visuals_enabled);
        replacement.selected_engine = selected_engine;
        if preserve_camera {
            replacement.camera = camera;
        }
        *self = replacement;
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
        let mut routing = self.ui.route_events(
            &input.input_events,
            context.target.size,
            self.selected_engine,
        );
        if let Some(action) = routing.action {
            match action {
                UiAction::Recenter => {
                    routing.events.push(InputEvent::Recenter);
                }
                #[cfg(feature = "box2d-comparator")]
                UiAction::SelectEngine(engine) => {
                    self.selected_engine = engine;
                }
                UiAction::ActivateScenario(scenario) => {
                    if self.config.solver_profile.supports_scenario(scenario) {
                        self.replace_configuration(
                            context.device,
                            context.queue,
                            ConfigurationReplacement {
                                config: EngineConfig {
                                    scenario,
                                    world_capacity: self.config.world_capacity,
                                    solver_profile: self.config.solver_profile.clone(),
                                },
                                selected_tool: routing.selected_tool,
                                scenario_activation: true,
                                scenario_restart: false,
                                preserve_camera: false,
                                solver_profiles: self.solver_profiles.clone(),
                            },
                        );
                    }
                }
                UiAction::ActivateWorldCapacity(world_capacity) => {
                    if world_capacity != self.config.world_capacity {
                        self.replace_configuration(
                            context.device,
                            context.queue,
                            ConfigurationReplacement {
                                config: EngineConfig {
                                    scenario: self.config.scenario,
                                    world_capacity,
                                    solver_profile: self.config.solver_profile.clone(),
                                },
                                selected_tool: routing.selected_tool,
                                scenario_activation: false,
                                scenario_restart: true,
                                preserve_camera: true,
                                solver_profiles: self.solver_profiles.clone(),
                            },
                        );
                    }
                }
                UiAction::RestartScenario => {
                    self.replace_configuration(
                        context.device,
                        context.queue,
                        ConfigurationReplacement {
                            config: self.config.clone(),
                            selected_tool: routing.selected_tool,
                            scenario_activation: false,
                            scenario_restart: true,
                            preserve_camera: false,
                            solver_profiles: self.solver_profiles.clone(),
                        },
                    );
                }
                UiAction::ActivateSolverProfile(profile) => {
                    let scenario = if profile.supports_scenario(self.config.scenario) {
                        self.config.scenario
                    } else {
                        #[cfg(feature = "solver-lab")]
                        {
                            ValidationScenario::SolverLabStack16
                        }
                        #[cfg(not(feature = "solver-lab"))]
                        {
                            self.config.scenario
                        }
                    };
                    let mut solver_profiles = self.solver_profiles.clone();
                    crate::ui::remember_solver_profile(&mut solver_profiles, profile.clone());
                    self.replace_configuration(
                        context.device,
                        context.queue,
                        ConfigurationReplacement {
                            config: EngineConfig {
                                scenario,
                                world_capacity: self.config.world_capacity,
                                solver_profile: profile,
                            },
                            selected_tool: routing.selected_tool,
                            scenario_activation: scenario != self.config.scenario,
                            scenario_restart: false,
                            preserve_camera: true,
                            solver_profiles,
                        },
                    );
                }
                UiAction::ActivateSolverProfileForScenario { profile, scenario } => {
                    assert!(profile.supports_scenario(scenario));
                    let mut solver_profiles = self.solver_profiles.clone();
                    crate::ui::remember_solver_profile(&mut solver_profiles, profile.clone());
                    self.replace_configuration(
                        context.device,
                        context.queue,
                        ConfigurationReplacement {
                            config: EngineConfig {
                                scenario,
                                world_capacity: self.config.world_capacity,
                                solver_profile: profile,
                            },
                            selected_tool: routing.selected_tool,
                            scenario_activation: scenario != self.config.scenario,
                            scenario_restart: false,
                            preserve_camera: true,
                            solver_profiles,
                        },
                    );
                }
            }
        }
        self.pointer_events_total = self.pointer_events_total.saturating_add(raw_pointer_count);
        let (world_events, _) =
            self.input
                .apply_events(&mut self.camera, &routing.events, context.target.size);
        if let Some(interaction) = &mut self.body_interaction {
            interaction.select_tool(routing.selected_tool);
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
                self.clock.fixed_step_seconds() as f32,
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
            #[cfg(feature = "box2d-comparator")]
            self.comparison_shadow.advance(steps);
        } else if let Some(world) = &self.body_world {
            let active_hold_seconds = if pacing.mode == crate::PacingMode::ExplicitStep {
                f64::from(steps) * self.clock.fixed_step_seconds()
            } else {
                pacing.admitted_active_wall_seconds_this_frame
            };
            #[cfg_attr(not(feature = "box2d-comparator"), allow(unused_mut))]
            let mut commands = self
                .body_interaction
                .as_mut()
                .expect("GPU body worlds own body command state")
                .prepare(steps, self.fixed_steps_total, active_hold_seconds);
            #[cfg(feature = "box2d-comparator")]
            let (
                comparison_body_commands,
                comparison_body_ranges,
                comparison_grab_intents,
                comparison_grab_ranges,
            ) = if self.selected_engine == ComparisonEngine::Box2d {
                target_visible_box2d_interactions(&mut commands, &mut self.comparison_shadow)
            } else {
                (
                    commands.batch.commands.clone(),
                    commands.ranges.clone(),
                    commands.grab_batch.intents.clone(),
                    commands.grab_ranges.clone(),
                )
            };
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
                        construction_batch: &commands.construction_batch,
                        construction_ranges: &commands.construction_ranges,
                    },
                )
                .expect("engine-owned body command plan is valid");
            #[cfg(feature = "box2d-comparator")]
            {
                for step in 0..steps as usize {
                    let body = comparison_body_ranges[step];
                    let construction = commands.construction_ranges[step];
                    let body_commands = &comparison_body_commands
                        [body.start as usize..(body.start + body.count) as usize];
                    let grab_intents = &comparison_grab_intents[comparison_grab_ranges[step].start
                        as usize
                        ..(comparison_grab_ranges[step].start + comparison_grab_ranges[step].count)
                            as usize];
                    let construction_intents =
                        &commands.construction_batch.intents[construction.start as usize
                            ..(construction.start + construction.count) as usize];
                    if self.selected_engine == ComparisonEngine::Box2d {
                        self.comparison_shadow
                            .apply_interactions(body_commands, grab_intents, &[]);
                    } else {
                        self.comparison_shadow.enqueue_physbox_interactions(
                            body_commands,
                            grab_intents,
                            construction_intents,
                        );
                    }
                    self.comparison_shadow.advance_one();
                }
                self.comparison_shadow.refresh_snapshots();
            }
        } else {
            unreachable!("every shared scenario owns a GPU mechanics path");
        }

        #[cfg(feature = "box2d-comparator")]
        let render_box2d = self.selected_engine == ComparisonEngine::Box2d;
        #[cfg(not(feature = "box2d-comparator"))]
        let render_box2d = false;
        if render_box2d {
            #[cfg(feature = "box2d-comparator")]
            {
                let physics = self.config.solver_profile.physics();
                let quiet_outer_steps = physbox_contracts::SleepPolicyV1::experimental_s23()
                    .quiet_outer_steps(physics.fixed_step_hz)
                    .expect("validated physics hertz resolves the sleep quiet duration");
                let wake_flash_outer_steps = physics
                    .fixed_step_hz
                    .saturating_mul(SLEEP_WAKE_FLASH_MILLISECONDS)
                    .div_ceil(1_000)
                    .max(1);
                self.comparison_shadow.encode_render(
                    context.queue,
                    context.encoder,
                    context.target,
                    ComparisonRenderPlan {
                        frame_index: self.frame_index,
                        camera: camera_frame,
                        sleep_visuals_enabled: self.ui.sleep_visuals_enabled(),
                        current_outer_step: self.comparison_shadow.completed_steps() as u32,
                        quiet_outer_steps,
                        wake_flash_outer_steps,
                    },
                );
            }
        } else if let Some(world) = &self.body_world {
            let grab_pointer_active = self
                .body_interaction
                .as_ref()
                .is_some_and(|interaction| interaction.report.grab_pointer_active);
            let physics = self.config.solver_profile.physics();
            let quiet_outer_steps = physbox_contracts::SleepPolicyV1::experimental_s23()
                .quiet_outer_steps(physics.fixed_step_hz)
                .expect("validated physics hertz resolves the sleep quiet duration");
            let wake_flash_outer_steps = physics
                .fixed_step_hz
                .saturating_mul(SLEEP_WAKE_FLASH_MILLISECONDS)
                .div_ceil(1_000)
                .max(1);
            world.encode_render_with_timestamp(
                context.queue,
                context.encoder,
                context.target,
                BodyRenderPlan {
                    frame_index: self.frame_index,
                    camera: camera_frame,
                    grab_pointer_active,
                    sleep_visuals_enabled: self.ui.sleep_visuals_enabled(),
                    current_outer_step: self.fixed_steps_total.saturating_add(u64::from(steps))
                        as u32,
                    quiet_outer_steps,
                    wake_flash_outer_steps,
                    ending_timestamp: None,
                },
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
        let physics = self.config.solver_profile.physics();
        let internal_substep_hz = physics
            .fixed_step_hz
            .checked_mul(physics.substeps)
            .expect("validated product physics bounds cannot overflow substep hertz");
        let substep_dt = physics.fixed_dt_seconds() / physics.substeps as f32;
        let effective_dynamic_contact_hz = if self.config.solver_profile.is_soft_coupled() {
            physics.contact_hertz.min(0.125 / substep_dt)
        } else {
            0.0
        };
        let requested_static_contact_hz =
            physics.contact_hertz * physics.static_contact_hertz_multiplier;
        let effective_static_contact_hz = if self.config.solver_profile.is_soft_coupled() {
            effective_dynamic_contact_hz * physics.static_contact_hertz_multiplier
        } else {
            0.0
        };
        let sleep_policy = physbox_contracts::SleepPolicyV1::experimental_s23();
        let contract = SceneContract {
            version: SCENE_CONTRACT_VERSION,
            engine: "physbox5",
            state_owner: "gpu",
            body_count: self.body_world.as_ref().map_or_else(
                || if self.solver_lab.is_some() { 64 } else { 1 },
                GpuBodyWorld::initial_body_count,
            ),
            requested_body_capacity: self.config.world_capacity.slots(),
            body_capacity: self.body_world.as_ref().map_or_else(
                || self.config.world_capacity.slots(),
                GpuBodyWorld::body_capacity,
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
            generated_solver_execution: self
                .body_world
                .as_ref()
                .and_then(GpuBodyWorld::generated_solver_configuration)
                .map(|(execution, _)| execution),
            generated_solver_scheduling_policy: self
                .body_world
                .as_ref()
                .and_then(GpuBodyWorld::generated_solver_configuration)
                .map(|(_, scheduling)| scheduling),
            generated_contact_coefficient_policy: self
                .body_world
                .as_ref()
                .and_then(GpuBodyWorld::generated_contact_coefficient_policy),
            solver_profile: self.config.solver_profile.name(),
            solver_profile_options: self.config.solver_profile.options_string(),
            physics_configuration_version: physics.version,
            physics_configuration_hash: self.physics_configuration_hash.clone(),
            sleep_policy: "experimental-s23-v1",
            sleep_policy_version: physbox_contracts::SLEEP_POLICY_VERSION,
            sleep_state_layout_version: physbox_contracts::SLEEP_STATE_LAYOUT_VERSION,
            sleep_speed_threshold: physics.sleep_speed_threshold,
            sleep_grouping: physics.sleep_grouping.name(),
            #[cfg(feature = "box2d-comparator")]
            box2d_sleep_speed_threshold: Some(self.comparison_shadow.sleep_speed_threshold()),
            #[cfg(not(feature = "box2d-comparator"))]
            box2d_sleep_speed_threshold: None,
            sleep_quiet_outer_steps: sleep_policy
                .quiet_outer_steps(physics.fixed_step_hz)
                .expect("validated sleep policy and fixed-step hertz"),
            sleep_wake_propagation_rounds: physics
                .sleep_grouping
                .wake_propagation_rounds()
                .unwrap_or(0),
            sleep_impact_wake_speed: sleep_policy.impact_wake_speed.get(),
            requested_sleeping_enabled: physics.sleeping_enabled,
            effective_sleeping_enabled: self
                .body_world
                .as_ref()
                .is_some_and(GpuBodyWorld::sleeping_enabled),
            awake_compaction_enabled: true,
            fixed_step_hz: physics.fixed_step_hz,
            fixed_dt_seconds: physics.fixed_dt_seconds(),
            solver_substeps: physics.substeps,
            solver_biased_iterations: physics.biased_iterations,
            internal_substep_hz,
            requested_contact_hz: physics.contact_hertz,
            effective_dynamic_contact_hz,
            requested_static_contact_hz,
            effective_static_contact_hz,
            contact_damping_ratio: physics.contact_damping_ratio,
            speculative_distance: physics.speculative_distance,
            max_depenetration_velocity: physics.max_depenetration_velocity,
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
                        "selected GPU manifold-block colored floor solve completed"
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
            ValidationScenario::CircleBucket100 | ValidationScenario::RectangleBucket100 => {
                let complete = self.fixed_steps_total >= u64::from(P5_2_BUCKET_EVIDENCE_STEPS);
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete,
                    passed: complete,
                    reason: if complete {
                        "100-body GPU bucket completed the selected broadphase path"
                    } else {
                        "waiting for 600 GPU broadphase bucket steps"
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
            ValidationScenario::SolverManifoldI2 | ValidationScenario::SolverParallelColoring => {
                let complete =
                    self.fixed_steps_total >= u64::from(P5_2_MANIFOLD_COLOR_VALIDATION_STEPS);
                let expected_scheduling =
                    if self.config.scenario == ValidationScenario::SolverParallelColoring {
                        GpuSolverSchedulingPolicyV1::SELECTED
                    } else {
                        GpuSolverSchedulingPolicyV1::SERIAL_CONTROL
                    };
                let passed = complete
                    && !contract.capacity_overflowed
                    && contract.generated_solver_scheduling_policy == Some(expected_scheduling);
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete,
                    passed,
                    reason: if passed {
                        if self.config.scenario == ValidationScenario::SolverParallelColoring {
                            "shared generated selected parallel scheduler completed"
                        } else {
                            "shared generated serial scheduler control completed"
                        }
                    } else if complete {
                        "manifold-color execution reported the wrong policy or capacity overflow"
                    } else {
                        "waiting for eight shared manifold-color candidate steps"
                    },
                }
            }
            ValidationScenario::SettlingHard8
            | ValidationScenario::SettlingCandidate8
            | ValidationScenario::SettlingHard16
            | ValidationScenario::SettlingIdentity16
            | ValidationScenario::SettlingSoftness16
            | ValidationScenario::SettlingCandidate16 => {
                let complete =
                    self.fixed_steps_total >= u64::from(P5_2_CENTERED_STACK_EVIDENCE_STEPS);
                let passed = complete && !contract.capacity_overflowed;
                let (_, policy) = settling_review_lane(self.config.scenario)
                    .expect("settling review launch scenario maps to a shared policy");
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete,
                    passed,
                    reason: if passed {
                        match policy {
                            P5_2SettlingReviewPolicy::HardControl => {
                                "hard settling control completed its exact step contract"
                            }
                            P5_2SettlingReviewPolicy::IdentityOnly => {
                                "identity-only settling control completed its exact step contract"
                            }
                            P5_2SettlingReviewPolicy::SoftnessOnly => {
                                "softness-only settling control completed its exact step contract"
                            }
                            P5_2SettlingReviewPolicy::SoftCandidate => {
                                "soft settling candidate completed its exact step contract"
                            }
                        }
                    } else if complete {
                        "settling review lane reported capacity overflow"
                    } else {
                        "waiting for 1200 settling review steps"
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
            #[cfg(feature = "solver-lab")]
            ValidationScenario::SolverLabStack2
            | ValidationScenario::SolverLabStack4
            | ValidationScenario::SolverLabStack8
            | ValidationScenario::SolverLabStack12
            | ValidationScenario::SolverLabStack14
            | ValidationScenario::SolverLabStack16 => {
                let complete =
                    self.fixed_steps_total >= u64::from(P5_2_CENTERED_STACK_EVIDENCE_STEPS);
                ScenarioResult {
                    name: self.config.scenario.name(),
                    complete,
                    passed: complete && !contract.capacity_overflowed,
                    reason: if complete && !contract.capacity_overflowed {
                        "interactive solver-lab stack completed its exact step contract"
                    } else if complete {
                        "interactive solver-lab stack reported capacity overflow"
                    } else {
                        "waiting for 1200 interactive solver-lab steps"
                    },
                }
            }
        };
        let construction_diagnostic = self.construction_diagnostic_report();
        let sleep_cycle_diagnostic = self.sleep_cycle_diagnostic_report();
        #[cfg(feature = "box2d-comparator")]
        let (
            comparator_available,
            box2d_revision,
            box2d_completed_steps,
            box2d_live_bodies,
            box2d_pending_actions,
            box2d_resolved_actions,
            box2d_identity_disagreements,
            box2d_action_queue_overflowed,
            box2d_aligned,
            box2d_failure,
            visible_body_count,
        ) = (
            true,
            Some(ComparisonShadow::revision()),
            Some(self.comparison_shadow.completed_steps()),
            Some(self.comparison_shadow.live_bodies()),
            Some(self.comparison_shadow.pending_action_count()),
            Some(self.comparison_shadow.resolved_action_count()),
            Some(self.comparison_shadow.identity_disagreement_count()),
            Some(self.comparison_shadow.queue_overflowed()),
            Some(self.comparison_shadow.completed_steps() == self.fixed_steps_total),
            self.comparison_shadow.failure(),
            if self.selected_engine == ComparisonEngine::Box2d {
                self.comparison_shadow.live_bodies()
            } else {
                contract.body_count
            },
        );
        #[cfg(not(feature = "box2d-comparator"))]
        let (
            comparator_available,
            box2d_revision,
            box2d_completed_steps,
            box2d_live_bodies,
            box2d_pending_actions,
            box2d_resolved_actions,
            box2d_identity_disagreements,
            box2d_action_queue_overflowed,
            box2d_aligned,
            box2d_failure,
            visible_body_count,
        ) = (
            false,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            None,
            contract.body_count,
        );
        let ui = self.ui.encode(
            context.queue,
            context.encoder,
            context.target,
            UiFrameModel {
                scenario: self.config.scenario,
                solver_profile: &self.config.solver_profile,
                pacing: &pacing,
                body_count: visible_body_count,
                world_capacity: self.config.world_capacity,
                body_capacity: contract.body_capacity,
                contact_count: contract.contact_point_count,
                capacity_bytes: contract.solver_capacity_bytes,
                overflowed: contract.capacity_overflowed,
                sleeping_effective: contract.effective_sleeping_enabled,
                adapter_backend: &self.adapter.backend,
                scenario_activations_total: self.scenario_activations_total,
                scenario_restarts_total: self.scenario_restarts_total,
                construction_state: construction_diagnostic.state.as_ref(),
                construction_age_frames: construction_diagnostic.age_frames,
                construction_diagnostic_source: construction_diagnostic.source,
                selected_engine: self.selected_engine,
                comparator_available,
                box2d_revision,
                box2d_completed_steps,
                box2d_live_bodies,
                box2d_pending_actions,
                box2d_resolved_actions,
                box2d_identity_disagreements,
                box2d_action_queue_overflowed,
                box2d_aligned,
                box2d_failure,
                sleep_cycle: sleep_cycle_diagnostic.state.as_ref(),
                #[cfg(feature = "box2d-comparator")]
                box2d_sleep_cycle: sleep_cycle_diagnostic.comparison_state.as_ref(),
                sleep_cycle_age_frames: sleep_cycle_diagnostic.age_frames,
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
            construction_diagnostic,
            sleep_cycle_diagnostic,
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

    #[cfg(any(not(target_arch = "wasm32"), feature = "box2d-comparator"))]
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

    pub fn copy_construction_state(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<(), String> {
        let world = self.body_world.as_ref().ok_or_else(|| {
            "construction state is available only for the GPU body world".to_owned()
        })?;
        world.copy_construction_state(encoder, destination);
        Ok(())
    }

    pub(crate) fn copy_sleep_cycle_metrics(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<(), String> {
        self.body_world
            .as_ref()
            .ok_or_else(|| "sleep-cycle diagnostics require the GPU body world".to_owned())?
            .copy_sleep_cycle_metrics(encoder, destination)
    }

    pub(crate) fn construction_diagnostic_epoch(&self) -> (String, u64, u64) {
        (
            self.scenario_hash.clone(),
            self.scenario_activations_total,
            self.scenario_restarts_total,
        )
    }

    pub(crate) fn publish_construction_diagnostic(
        &mut self,
        state: ConstructionStateV1,
        observed_frame_index: u64,
    ) {
        #[cfg(feature = "box2d-comparator")]
        self.comparison_shadow.publish_construction_feedback(&state);
        self.construction_diagnostic = Some((state, observed_frame_index));
    }

    pub(crate) fn publish_sleep_cycle_diagnostic(
        &mut self,
        state: crate::SleepCycleMetricsV1,
        observed_frame_index: u64,
    ) {
        self.sleep_cycle_diagnostic = Some((state, observed_frame_index));
    }

    #[cfg(feature = "box2d-comparator")]
    pub(crate) fn publish_grab_diagnostic(&mut self, state: crate::GrabValidationState) {
        self.comparison_shadow.publish_grab_feedback(&state);
    }

    #[cfg(feature = "box2d-comparator")]
    pub(crate) fn fail_comparison(&mut self, message: String) {
        self.comparison_shadow.fail(message);
    }

    fn construction_diagnostic_report(&self) -> ConstructionDiagnosticReport {
        let (state, observed_frame_index) = self
            .construction_diagnostic
            .as_ref()
            .map_or((None, None), |(state, frame)| {
                (Some(state.clone()), Some(*frame))
            });
        ConstructionDiagnosticReport {
            source: if self.body_world.is_some() {
                "async-gpu"
            } else {
                "unavailable"
            },
            age_frames: observed_frame_index.map(|frame| self.frame_index.saturating_sub(frame)),
            state,
            observed_frame_index,
            blocking_readback_for_advancement: false,
        }
    }

    fn sleep_cycle_diagnostic_report(&self) -> SleepCycleDiagnosticReport {
        let (state, observed_frame_index) = self
            .sleep_cycle_diagnostic
            .as_ref()
            .map_or((None, None), |(state, frame)| {
                (Some(state.clone()), Some(*frame))
            });
        SleepCycleDiagnosticReport {
            source: if self
                .body_world
                .as_ref()
                .is_some_and(GpuBodyWorld::sleeping_enabled)
            {
                "asynchronous-gpu"
            } else {
                "unavailable"
            },
            age_frames: observed_frame_index.map(|frame| self.frame_index.saturating_sub(frame)),
            state,
            #[cfg(feature = "box2d-comparator")]
            comparison_state: Some(self.comparison_shadow.sleep_cycle_metrics().clone()),
            #[cfg(not(feature = "box2d-comparator"))]
            comparison_state: None,
            observed_frame_index,
            blocking_readback_for_advancement: false,
        }
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

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_construction_graph_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<(), String> {
        self.body_world
            .as_ref()
            .ok_or_else(|| "construction graph evidence requires a GPU body world".to_owned())?
            .copy_construction_graph_evidence(encoder, destination)
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn construction_body_capacity(&self) -> Result<u32, String> {
        self.body_world
            .as_ref()
            .map(crate::body_world::GpuBodyWorld::body_capacity)
            .ok_or_else(|| "construction body evidence requires a GPU body world".to_owned())
    }

    #[cfg(not(target_arch = "wasm32"))]
    pub(crate) fn copy_construction_body_evidence(
        &self,
        encoder: &mut wgpu::CommandEncoder,
        destination: &wgpu::Buffer,
    ) -> Result<(), String> {
        self.body_world
            .as_ref()
            .ok_or_else(|| "construction body evidence requires a GPU body world".to_owned())?
            .copy_construction_body_evidence(encoder, destination);
        Ok(())
    }
}

#[cfg(test)]
mod body_interaction_tests {
    use super::*;

    fn pointer(id: u64, phase: PointerPhase, world_target: Vec2) -> WorldPointerEvent {
        WorldPointerEvent {
            id,
            phase,
            button: PointerButton::Primary,
            world_target,
        }
    }

    fn intent_target(intent: &ConstructionIntentV1) -> Vec2 {
        match intent {
            ConstructionIntentV1::PlaceCircle { world_target, .. }
            | ConstructionIntentV1::PlaceRectangle { world_target, .. }
            | ConstructionIntentV1::PlaceCircleBody { world_target, .. }
            | ConstructionIntentV1::PlaceRectangleBody { world_target, .. }
            | ConstructionIntentV1::EraseAt { world_target, .. }
            | ConstructionIntentV1::EraseBody { world_target, .. } => *world_target,
        }
    }

    #[test]
    fn held_placement_repeats_at_the_moving_pointer_and_stops_on_release() {
        let mut interaction = BodyInteractionState::new(Vec::new(), true);
        interaction.select_tool(WorkbenchTool::Circle);
        let down_target = Vec2::new(-0.4, 0.6);
        let moved_target = Vec2::new(0.3, 0.8);
        interaction.capture_pointer_events(&[pointer(1, PointerPhase::Down, down_target)]);
        interaction.prepare(0, 0, 0.0);

        let before_delay = interaction.prepare(14, 0, 14.0 / 60.0);
        assert!(before_delay.construction_batch.intents.is_empty());
        interaction.capture_pointer_events(&[pointer(1, PointerPhase::Move, moved_target)]);

        let first_repeat = interaction.prepare(1, 14, 1.0 / 60.0);
        assert_eq!(first_repeat.construction_batch.intents.len(), 1);
        assert_eq!(
            intent_target(&first_repeat.construction_batch.intents[0]),
            moved_target
        );

        let before_interval = interaction.prepare(5, 15, 5.0 / 60.0);
        assert!(before_interval.construction_batch.intents.is_empty());
        let second_repeat = interaction.prepare(1, 20, 1.0 / 60.0);
        assert_eq!(second_repeat.construction_batch.intents.len(), 1);
        assert_eq!(
            intent_target(&second_repeat.construction_batch.intents[0]),
            moved_target
        );

        interaction.capture_pointer_events(&[pointer(1, PointerPhase::Up, moved_target)]);
        let after_release = interaction.prepare(60, 21, 1.0);
        assert!(after_release.construction_batch.intents.is_empty());
        assert!(!interaction.report.construction_pointer_active);
        assert_eq!(interaction.report.queued_construction_intents_total, 2);
    }

    #[test]
    fn quick_tap_places_once_and_cancel_places_nothing() {
        let mut interaction = BodyInteractionState::new(Vec::new(), true);
        interaction.select_tool(WorkbenchTool::Rectangle);
        let tap_target = Vec2::new(0.2, 0.7);
        interaction.capture_pointer_events(&[
            pointer(1, PointerPhase::Down, tap_target),
            pointer(1, PointerPhase::Up, tap_target),
        ]);
        let tap = interaction.prepare(1, 0, 1.0 / 60.0);
        assert_eq!(tap.construction_batch.intents.len(), 1);
        assert_eq!(
            intent_target(&tap.construction_batch.intents[0]),
            tap_target
        );

        interaction.capture_pointer_events(&[pointer(2, PointerPhase::Down, tap_target)]);
        interaction.capture_pointer_events(&[pointer(2, PointerPhase::Cancel, tap_target)]);
        let cancelled = interaction.prepare(120, 1, 2.0);
        assert!(cancelled.construction_batch.intents.is_empty());
        assert_eq!(interaction.report.queued_construction_intents_total, 1);
    }

    #[test]
    fn hold_time_survives_zero_step_frames_without_queuing_a_backlog() {
        let mut interaction = BodyInteractionState::new(Vec::new(), true);
        interaction.select_tool(WorkbenchTool::Circle);
        let target = Vec2::new(0.1, 0.6);
        interaction.capture_pointer_events(&[pointer(1, PointerPhase::Down, target)]);
        interaction.prepare(0, 0, 0.0);

        assert!(
            interaction
                .prepare(0, 0, 0.24)
                .construction_batch
                .intents
                .is_empty()
        );
        assert!(
            interaction
                .prepare(0, 0, 0.02)
                .construction_batch
                .intents
                .is_empty()
        );
        assert_eq!(interaction.report.queued_construction_intents_total, 0);

        let admitted = interaction.prepare(1, 0, 0.0);
        assert_eq!(admitted.construction_batch.intents.len(), 1);
        assert_eq!(interaction.report.queued_construction_intents_total, 1);
    }

    #[test]
    fn erase_remains_release_committed_without_hold_repeat() {
        let mut interaction = BodyInteractionState::new(Vec::new(), true);
        interaction.select_tool(WorkbenchTool::Erase);
        let target = Vec2::new(0.0, 0.5);
        interaction.capture_pointer_events(&[pointer(1, PointerPhase::Down, target)]);
        let held = interaction.prepare(120, 0, 2.0);
        assert!(held.construction_batch.intents.is_empty());

        interaction.capture_pointer_events(&[pointer(1, PointerPhase::Up, target)]);
        let released = interaction.prepare(1, 120, 1.0 / 60.0);
        assert_eq!(released.construction_batch.intents.len(), 1);
        assert!(matches!(
            released.construction_batch.intents[0],
            ConstructionIntentV1::EraseAt { .. }
        ));
    }
}
