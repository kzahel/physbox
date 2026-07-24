use physbox_contracts::{ConstructionResultV1, ConstructionStateV1};
use physbox_launch::{
    FluidSolidCouplingQuality, RenderScalePreset, SolverProfileName, ValidationScenario,
    WorldCapacityPreset,
};
use physbox_ui::{
    CanvasFrame, CanvasRenderer, HitTarget, InstanceCapacities, InteractionState, Point,
    PointerButton as CanvasPointerButton, PointerEvent as CanvasPointerEvent,
    PointerId as CanvasPointerId, PointerPhase as CanvasPointerPhase, Primitive, Rect,
    RenderReport as CanvasRenderReport, RenderTarget as CanvasRenderTarget, Theme, WidgetCanvas,
    WidgetEventKind, WidgetId, WidgetRole, icons, text_width,
};

use crate::{
    InputEvent, PointerButton, PointerPhase, SolverProfileConfig,
    clock::PacingReport,
    engine_selection::ComparisonEngine,
    renderer::{DisplayMetrics, RenderFrameTarget},
    report::{FrameTimingReport, RenderScaleReport, UiReport},
};

pub(crate) const UI_CONTRACT_VERSION: u32 = 13;
const STATIC_INSTANCE_CAPACITY: usize = 384;
const DYNAMIC_INSTANCE_CAPACITY: usize = 512;
const COMPACT_DENSITY_START_POINTS: f32 = 520.0;
const COMPACT_DENSITY_END_POINTS: f32 = 600.0;
const COMPACT_TOOLBAR_SCALE: f32 = 36.0 / 52.0;
const COMPACT_MENU_SCALE: f32 = 0.78;
const COMPACT_DIAGNOSTICS_SCALE: f32 = 0.75;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum UiAction {
    ActivateScenario(ValidationScenario),
    ActivateSolverProfile(SolverProfileConfig),
    ActivateSolverProfileForScenario {
        profile: SolverProfileConfig,
        scenario: ValidationScenario,
    },
    ActivateWorldCapacity(WorldCapacityPreset),
    ActivateFluidSolidCouplingQuality(FluidSolidCouplingQuality),
    #[cfg(feature = "box2d-comparator")]
    SelectEngine(ComparisonEngine),
    RestartScenario,
    Recenter,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
enum MenuPage {
    #[default]
    Scenes,
    Solvers,
    World,
}

fn menu_pages(selected_engine: ComparisonEngine) -> &'static [MenuPage] {
    const PHYSBOX: &[MenuPage] = &[MenuPage::Scenes, MenuPage::Solvers, MenuPage::World];
    #[cfg(feature = "box2d-comparator")]
    const BOX2D: &[MenuPage] = &[MenuPage::Scenes, MenuPage::World];
    match selected_engine {
        ComparisonEngine::PhysBox => PHYSBOX,
        #[cfg(feature = "box2d-comparator")]
        ComparisonEngine::Box2d => BOX2D,
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub(crate) enum WorkbenchTool {
    #[default]
    Grab,
    Circle,
    Rectangle,
    Erase,
    Water,
    Surface,
}

impl WorkbenchTool {
    #[cfg(feature = "box2d-comparator")]
    const BASE: [Self; 4] = [Self::Grab, Self::Circle, Self::Rectangle, Self::Erase];
    const PHYSBOX: [Self; 5] = [
        Self::Grab,
        Self::Circle,
        Self::Rectangle,
        Self::Erase,
        Self::Surface,
    ];
    const ALL: [Self; 6] = [
        Self::Grab,
        Self::Circle,
        Self::Rectangle,
        Self::Erase,
        Self::Water,
        Self::Surface,
    ];

    pub(crate) const fn name(self) -> &'static str {
        match self {
            Self::Grab => "grab",
            Self::Circle => "circle",
            Self::Rectangle => "rectangle",
            Self::Erase => "erase",
            Self::Water => "water",
            Self::Surface => "surface",
        }
    }
}

fn available_tools(
    scenario: ValidationScenario,
    selected_engine: ComparisonEngine,
) -> &'static [WorkbenchTool] {
    match selected_engine {
        ComparisonEngine::PhysBox if scenario == ValidationScenario::LiquidSandbox => {
            &WorkbenchTool::ALL
        }
        ComparisonEngine::PhysBox => &WorkbenchTool::PHYSBOX,
        #[cfg(feature = "box2d-comparator")]
        ComparisonEngine::Box2d => &WorkbenchTool::BASE,
    }
}

pub(crate) struct UiRouting {
    pub events: Vec<InputEvent>,
    pub action: Option<UiAction>,
    pub selected_tool: WorkbenchTool,
    sleep_visuals_toggled: bool,
    render_scale_cycled: bool,
}

pub(crate) struct UiFrameModel<'a> {
    pub scenario: ValidationScenario,
    pub solver_profile: &'a SolverProfileConfig,
    pub pacing: &'a PacingReport,
    pub body_count: u32,
    pub world_capacity: WorldCapacityPreset,
    pub fluid_solid_coupling_quality: FluidSolidCouplingQuality,
    pub body_capacity: u32,
    pub contact_count: u32,
    pub capacity_bytes: u64,
    pub overflowed: bool,
    pub particle_count: u32,
    pub particle_capacity: u32,
    pub particle_dispatches_per_step: u32,
    pub sleeping_effective: bool,
    pub adapter_backend: &'a str,
    pub scenario_activations_total: u64,
    pub scenario_restarts_total: u64,
    pub construction_state: Option<&'a ConstructionStateV1>,
    pub construction_age_frames: Option<u64>,
    pub construction_diagnostic_source: &'static str,
    pub selected_engine: ComparisonEngine,
    pub comparator_available: bool,
    pub box2d_revision: Option<&'static str>,
    pub box2d_completed_steps: Option<u64>,
    pub box2d_live_bodies: Option<u32>,
    pub box2d_pending_actions: Option<u32>,
    pub box2d_resolved_actions: Option<u64>,
    pub box2d_identity_disagreements: Option<u64>,
    pub box2d_action_queue_overflowed: Option<bool>,
    pub box2d_aligned: Option<bool>,
    pub box2d_failure: Option<&'a str>,
    pub sleep_cycle: Option<&'a crate::SleepCycleMetricsV1>,
    #[cfg(feature = "box2d-comparator")]
    pub box2d_sleep_cycle: Option<&'a crate::SleepCycleMetricsV1>,
    pub sleep_cycle_age_frames: Option<u64>,
    pub frame_timing: &'a FrameTimingReport,
    pub render_scale: &'a RenderScaleReport,
}

/// Shared canvas UI and input capture.
///
/// The compact atlas, instanced glyph batch, static-write caching, and toolbar
/// sizing are selectively ported from PhysBox 4 revision
/// `4295957e15480ef701e6006c7cafb5f0728ef101`, principally `toolbar.rs`,
/// `options/font.rs`, `options/text.rs`, and `options/renderer.rs`. PhysBox 5
/// owns a smaller workbench-only model and one combined quad/glyph pipeline.
pub(crate) struct WorkbenchUi {
    renderer: CanvasRenderer,
    retained_key: Option<StaticKey>,
    retained_revision: u64,
    retained_batch: Batch,
    menu_open: bool,
    menu_page: MenuPage,
    selected_tool: WorkbenchTool,
    sleep_visuals_enabled: bool,
    render_scale: RenderScalePreset,
    solver_profiles: Vec<SolverProfileConfig>,
    draft_solver_profile: SolverProfileConfig,
    interactions: InteractionState,
}

impl WorkbenchUi {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        solver_profile: SolverProfileConfig,
        solver_profiles: Vec<SolverProfileConfig>,
        sleep_visuals_enabled: bool,
        render_scale: RenderScalePreset,
    ) -> Self {
        Self {
            renderer: CanvasRenderer::new(
                device,
                queue,
                color_format,
                InstanceCapacities::new(STATIC_INSTANCE_CAPACITY, DYNAMIC_INSTANCE_CAPACITY),
            ),
            retained_key: None,
            retained_revision: 0,
            retained_batch: Batch::new(STATIC_INSTANCE_CAPACITY),
            menu_open: false,
            menu_page: MenuPage::default(),
            selected_tool: WorkbenchTool::default(),
            sleep_visuals_enabled,
            render_scale,
            solver_profiles,
            draft_solver_profile: solver_profile,
            interactions: InteractionState::default(),
        }
    }

    pub(crate) fn route_events(
        &mut self,
        events: &[InputEvent],
        target_size: [u32; 2],
        display_metrics: DisplayMetrics,
        scenario: ValidationScenario,
        selected_engine: ComparisonEngine,
    ) -> UiRouting {
        let routing = route_events(
            UiRouteState {
                menu_open: &mut self.menu_open,
                menu_page: &mut self.menu_page,
                selected_tool: &mut self.selected_tool,
                solver_profiles: &mut self.solver_profiles,
                draft_solver_profile: &mut self.draft_solver_profile,
                interactions: &mut self.interactions,
                scenario,
                selected_engine,
            },
            events,
            target_size,
            display_metrics,
        );
        if routing.sleep_visuals_toggled {
            self.sleep_visuals_enabled = !self.sleep_visuals_enabled;
        }
        if routing.render_scale_cycled {
            self.render_scale = self.render_scale.next();
        }
        routing
    }

    pub(crate) fn select_tool(&mut self, tool: WorkbenchTool) {
        self.selected_tool = tool;
    }

    pub(crate) fn restrict_selected_tool(
        &mut self,
        scenario: ValidationScenario,
        selected_engine: ComparisonEngine,
    ) -> WorkbenchTool {
        if !available_tools(scenario, selected_engine).contains(&self.selected_tool) {
            self.selected_tool = WorkbenchTool::Grab;
        }
        self.selected_tool
    }

    pub(crate) const fn sleep_visuals_enabled(&self) -> bool {
        self.sleep_visuals_enabled
    }

    pub(crate) fn set_sleep_visuals_enabled(&mut self, enabled: bool) {
        self.sleep_visuals_enabled = enabled;
    }

    pub(crate) const fn render_scale(&self) -> RenderScalePreset {
        self.render_scale
    }

    pub(crate) fn set_render_scale(&mut self, render_scale: RenderScalePreset) {
        self.render_scale = render_scale;
    }

    pub(crate) fn set_solver_profiles(
        &mut self,
        solver_profiles: Vec<SolverProfileConfig>,
        active: SolverProfileConfig,
    ) {
        self.solver_profiles = solver_profiles;
        self.draft_solver_profile = active;
    }

    pub(crate) fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        display_metrics: DisplayMetrics,
        model: UiFrameModel<'_>,
    ) -> UiReport {
        let selected_engine = model.selected_engine;
        let layout = UiLayout::for_display(target.size, display_metrics);
        let key = StaticKey {
            target_size: target.size,
            toolbar_scale_bits: layout.toolbar_scale.to_bits(),
            menu_scale_bits: layout.menu_scale.to_bits(),
            scenario: model.scenario,
            menu_open: self.menu_open,
            menu_page: self.menu_page,
            selected_tool: self.selected_tool,
            solver_profile_key: self.draft_solver_profile.configuration_key(),
            active_solver_profile: model.solver_profile.profile_name(),
            solver_profile_count: self.solver_profiles.len(),
            world_capacity: model.world_capacity,
            fluid_solid_coupling_quality: model.fluid_solid_coupling_quality,
            sleeping_effective: model.sleeping_effective,
            sleep_visuals_enabled: self.sleep_visuals_enabled,
            render_scale: self.render_scale,
            selected_engine,
        };
        if self.retained_key.as_ref() != Some(&key) {
            self.retained_batch = static_batch(
                target.size,
                display_metrics,
                StaticBatchModel {
                    scenario: model.scenario,
                    menu_open: self.menu_open,
                    menu_page: self.menu_page,
                    selected_tool: self.selected_tool,
                    active_solver_profile: model.solver_profile,
                    solver_profiles: &self.solver_profiles,
                    draft_solver_profile: &self.draft_solver_profile,
                    world_capacity: model.world_capacity,
                    fluid_solid_coupling_quality: model.fluid_solid_coupling_quality,
                    sleeping_effective: model.sleeping_effective,
                    sleep_visuals_enabled: self.sleep_visuals_enabled,
                    render_scale: self.render_scale,
                    selected_engine,
                },
            );
            self.retained_revision = self.retained_revision.wrapping_add(1);
            self.retained_key = Some(key);
        }
        let live_batch = dynamic_batch(
            target.size,
            display_metrics,
            &model,
            !self.menu_open,
            self.selected_tool,
        );
        let canvas = self.renderer.encode(
            queue,
            encoder,
            CanvasFrame {
                target: CanvasRenderTarget {
                    color_view: target.color_view,
                    size: target.size,
                },
                retained_revision: self.retained_revision,
                retained: &self.retained_batch.primitives,
                live: &live_batch.primitives,
                source_overflowed: self.retained_batch.overflowed | live_batch.overflowed,
            },
        );
        ui_report(
            model,
            self.menu_open,
            self.selected_tool,
            self.sleep_visuals_enabled,
            layout,
            canvas,
        )
    }
}

struct UiRouteState<'a> {
    menu_open: &'a mut bool,
    menu_page: &'a mut MenuPage,
    selected_tool: &'a mut WorkbenchTool,
    solver_profiles: &'a mut Vec<SolverProfileConfig>,
    draft_solver_profile: &'a mut SolverProfileConfig,
    interactions: &'a mut InteractionState,
    scenario: ValidationScenario,
    selected_engine: ComparisonEngine,
}

fn route_events(
    state: UiRouteState<'_>,
    events: &[InputEvent],
    target_size: [u32; 2],
    display_metrics: DisplayMetrics,
) -> UiRouting {
    let UiRouteState {
        menu_open,
        menu_page,
        selected_tool,
        solver_profiles,
        draft_solver_profile,
        interactions,
        scenario,
        selected_engine,
    } = state;
    let mut forwarded = Vec::with_capacity(events.len());
    let mut action = None;
    let mut sleep_visuals_toggled = false;
    let mut render_scale_cycled = false;
    let layout = UiLayout::for_display(target_size, display_metrics);
    for event in events.iter().copied() {
        let InputEvent::Pointer(pointer) = event else {
            forwarded.push(event);
            continue;
        };
        let pointer_id = CanvasPointerId(pointer.id);
        let eligible_button = matches!(
            pointer.button,
            PointerButton::Primary | PointerButton::Contact
        );
        let hits = layout.hit_targets(
            *menu_open,
            *menu_page,
            solver_profiles,
            scenario,
            selected_engine,
        );
        let targets = hits.iter().map(|(target, _)| *target).collect::<Vec<_>>();
        let canvas_event = CanvasPointerEvent {
            id: pointer_id,
            phase: match pointer.phase {
                PointerPhase::Down => CanvasPointerPhase::Down,
                PointerPhase::Move => CanvasPointerPhase::Move,
                PointerPhase::Up => CanvasPointerPhase::Up,
                PointerPhase::Cancel => CanvasPointerPhase::Cancel,
            },
            button: match pointer.button {
                PointerButton::Primary | PointerButton::Contact => CanvasPointerButton::Primary,
                PointerButton::Secondary => CanvasPointerButton::Secondary,
                PointerButton::Auxiliary => CanvasPointerButton::Middle,
            },
            position: Point::new(
                pointer.normalized_position[0] * target_size[0] as f32,
                pointer.normalized_position[1] * target_size[1] as f32,
            ),
        };
        let route_targets = if eligible_button || interactions.is_captured(pointer_id) {
            targets.as_slice()
        } else {
            &[]
        };
        let report = interactions.route(&[canvas_event], route_targets);
        for widget_event in report.widget_events {
            if let Some((_, hit)) = hits.iter().find(|(target, _)| target.id == widget_event.id) {
                if let UiHit::SolverSlider(parameter) = *hit {
                    if matches!(
                        widget_event.kind,
                        WidgetEventKind::Pressed
                            | WidgetEventKind::Dragged
                            | WidgetEventKind::Activated
                    ) && let Some(value) = widget_event.normalized_value
                    {
                        draft_solver_profile.set_parameter_normalized(parameter, value);
                    }
                    continue;
                }
                if widget_event.kind != WidgetEventKind::Pressed {
                    continue;
                }
                match *hit {
                    UiHit::Menu => *menu_open = !*menu_open,
                    UiHit::MenuPage(page) => *menu_page = page,
                    UiHit::Tool(tool) => *selected_tool = tool,
                    UiHit::Scenario(scenario) => {
                        *menu_open = false;
                        action = Some(UiAction::ActivateScenario(scenario));
                    }
                    UiHit::Restart => {
                        *menu_open = false;
                        action = Some(UiAction::RestartScenario);
                    }
                    UiHit::Recenter => {
                        *menu_open = false;
                        action = Some(UiAction::Recenter);
                    }
                    UiHit::SolverProfile(profile) => {
                        let sleep = draft_solver_profile.physics();
                        remember_solver_profile(solver_profiles, draft_solver_profile.clone());
                        *draft_solver_profile = solver_profiles
                            .iter()
                            .find(|candidate| candidate.profile_name() == profile)
                            .cloned()
                            .unwrap_or_else(|| SolverProfileConfig::select(profile));
                        draft_solver_profile.set_sleeping_enabled(sleep.sleeping_enabled);
                        draft_solver_profile.set_sleep_speed_threshold(sleep.sleep_speed_threshold);
                        draft_solver_profile.set_sleep_grouping(sleep.sleep_grouping);
                    }
                    UiHit::SolverSlider(_) => unreachable!("slider events are handled above"),
                    UiHit::RestoreSolverPreset => {
                        draft_solver_profile.restore_preset();
                    }
                    UiHit::ApplySolver => {
                        *menu_open = false;
                        action = Some(UiAction::ActivateSolverProfile(
                            draft_solver_profile.clone(),
                        ));
                    }
                    UiHit::SolverScene(scenario) => {
                        *menu_open = false;
                        action = Some(UiAction::ActivateSolverProfileForScenario {
                            profile: draft_solver_profile.clone(),
                            scenario,
                        });
                    }
                    UiHit::WorldCapacity(capacity) => {
                        *menu_open = false;
                        action = Some(UiAction::ActivateWorldCapacity(capacity));
                    }
                    UiHit::FluidSolidCouplingQuality(quality) => {
                        *menu_open = false;
                        action = Some(UiAction::ActivateFluidSolidCouplingQuality(quality));
                    }
                    UiHit::ToggleSleeping => {
                        let enabled = !draft_solver_profile.physics().sleeping_enabled;
                        draft_solver_profile.set_sleeping_enabled(enabled);
                        *menu_open = false;
                        action = Some(UiAction::ActivateSolverProfile(
                            draft_solver_profile.clone(),
                        ));
                    }
                    UiHit::CycleSleepThreshold => {
                        draft_solver_profile.cycle_sleep_speed_threshold();
                        *menu_open = false;
                        action = Some(UiAction::ActivateSolverProfile(
                            draft_solver_profile.clone(),
                        ));
                    }
                    UiHit::CycleSleepGrouping => {
                        draft_solver_profile.cycle_sleep_grouping();
                        *menu_open = false;
                        action = Some(UiAction::ActivateSolverProfile(
                            draft_solver_profile.clone(),
                        ));
                    }
                    UiHit::ToggleSleepVisuals => {
                        sleep_visuals_toggled = true;
                        *menu_open = false;
                    }
                    UiHit::CycleRenderScale => {
                        render_scale_cycled = true;
                        *menu_open = false;
                    }
                    #[cfg(feature = "box2d-comparator")]
                    UiHit::Engine(engine) => {
                        *menu_open = false;
                        action = Some(UiAction::SelectEngine(engine));
                    }
                    UiHit::Panel => {}
                }
            }
        }
        if !report.unhandled_events.is_empty() {
            forwarded.push(event);
        }
    }
    UiRouting {
        events: forwarded,
        action,
        selected_tool: *selected_tool,
        sleep_visuals_toggled,
        render_scale_cycled,
    }
}

pub(crate) fn remember_solver_profile(
    profiles: &mut Vec<SolverProfileConfig>,
    profile: SolverProfileConfig,
) {
    if let Some(existing) = profiles
        .iter_mut()
        .find(|candidate| candidate.profile_name() == profile.profile_name())
    {
        *existing = profile;
    } else {
        profiles.push(profile);
    }
}

#[cfg_attr(not(feature = "solver-lab"), allow(dead_code))]
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum UiHit {
    Menu,
    MenuPage(MenuPage),
    Tool(WorkbenchTool),
    Scenario(ValidationScenario),
    Restart,
    Recenter,
    SolverProfile(SolverProfileName),
    SolverSlider(crate::solver_profile::SolverParameter),
    SolverScene(ValidationScenario),
    WorldCapacity(WorldCapacityPreset),
    FluidSolidCouplingQuality(FluidSolidCouplingQuality),
    ToggleSleeping,
    CycleSleepThreshold,
    CycleSleepGrouping,
    ToggleSleepVisuals,
    CycleRenderScale,
    #[cfg(feature = "box2d-comparator")]
    Engine(ComparisonEngine),
    RestoreSolverPreset,
    ApplySolver,
    Panel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiLayout {
    target_size: [u32; 2],
    display_metrics: DisplayMetrics,
    toolbar_scale: f32,
    menu_scale: f32,
    diagnostics_scale: f32,
}

impl UiLayout {
    pub(crate) fn for_target(target_size: [u32; 2]) -> Self {
        Self::for_display(target_size, DisplayMetrics::ONE)
    }

    pub(crate) fn for_display(target_size: [u32; 2], display_metrics: DisplayMetrics) -> Self {
        let minimum_dimension = target_size[0].min(target_size[1]).max(1) as f32;
        let base_scale = (minimum_dimension / 600.0).clamp(1.0, 2.0);
        let pixels_per_point = display_metrics.pixels_per_point();
        let logical_short_edge = minimum_dimension / pixels_per_point;
        let compact_progress = ((logical_short_edge - COMPACT_DENSITY_START_POINTS)
            / (COMPACT_DENSITY_END_POINTS - COMPACT_DENSITY_START_POINTS))
            .clamp(0.0, 1.0);
        let resolve_scale = |compact_scale: f32| {
            let point_scale = 1.0 + (compact_scale - 1.0) * compact_progress;
            base_scale.min(pixels_per_point * point_scale)
        };
        Self {
            target_size,
            display_metrics,
            toolbar_scale: resolve_scale(COMPACT_TOOLBAR_SCALE),
            menu_scale: resolve_scale(COMPACT_MENU_SCALE),
            diagnostics_scale: resolve_scale(COMPACT_DIAGNOSTICS_SCALE),
        }
    }

    fn logical_target_size(self) -> [f32; 2] {
        let pixels_per_point = self.display_metrics.pixels_per_point();
        [
            self.target_size[0] as f32 / pixels_per_point,
            self.target_size[1] as f32 / pixels_per_point,
        ]
    }

    fn hit_targets(
        self,
        menu_open: bool,
        menu_page: MenuPage,
        solver_profiles: &[SolverProfileConfig],
        scenario: ValidationScenario,
        selected_engine: ComparisonEngine,
    ) -> Vec<(HitTarget, UiHit)> {
        let mut hits = Vec::with_capacity(48);
        {
            let mut push = |bounds: [f32; 4], hit| {
                hits.push((
                    HitTarget {
                        id: WidgetId::new(hits.len() as u64 + 1),
                        bounds: Rect::from_array(bounds),
                        role: WidgetRole::Button,
                    },
                    hit,
                ));
            };
            push(self.menu_button(), UiHit::Menu);
            for (index, tool) in available_tools(scenario, selected_engine)
                .iter()
                .enumerate()
            {
                push(self.tool_button_at(index), UiHit::Tool(*tool));
            }
            if !menu_open {
                return hits;
            }
            push(
                self.menu_panel(menu_page, solver_profiles.len()),
                UiHit::Panel,
            );
            for page in menu_pages(selected_engine) {
                push(self.menu_tab(*page), UiHit::MenuPage(*page));
            }
            match menu_page {
                MenuPage::Scenes => {
                    for (index, scenario) in ValidationScenario::MENU.iter().copied().enumerate() {
                        push(self.scenario_button(index), UiHit::Scenario(scenario));
                    }
                    push(self.restart_button(), UiHit::Restart);
                    push(self.recenter_button(), UiHit::Recenter);
                }
                MenuPage::Solvers => {
                    if selected_engine != ComparisonEngine::PhysBox {
                        return hits;
                    }
                    let profile_count = solver_profiles.len();
                    for (index, profile) in solver_profiles.iter().enumerate() {
                        push(
                            self.solver_profile_button(index, profile_count),
                            UiHit::SolverProfile(profile.profile_name()),
                        );
                    }
                    for (index, parameter) in crate::solver_profile::SolverParameter::ALL
                        .iter()
                        .copied()
                        .enumerate()
                    {
                        push(
                            self.solver_parameter_slider(index, profile_count),
                            UiHit::SolverSlider(parameter),
                        );
                    }
                    #[cfg(feature = "solver-lab")]
                    {
                        push(
                            self.solver_scene_button(0, profile_count),
                            UiHit::SolverScene(ValidationScenario::SolverLabStack8),
                        );
                        push(
                            self.solver_scene_button(1, profile_count),
                            UiHit::SolverScene(ValidationScenario::SolverLabStack16),
                        );
                    }
                    push(
                        self.restore_solver_button(profile_count),
                        UiHit::RestoreSolverPreset,
                    );
                    push(self.apply_solver_button(profile_count), UiHit::ApplySolver);
                }
                MenuPage::World => {
                    #[cfg(feature = "box2d-comparator")]
                    for (index, engine) in [ComparisonEngine::PhysBox, ComparisonEngine::Box2d]
                        .into_iter()
                        .enumerate()
                    {
                        push(self.world_engine_button(index), UiHit::Engine(engine));
                    }
                    for (index, capacity) in WorldCapacityPreset::ALL.into_iter().enumerate() {
                        push(
                            self.world_capacity_button(index),
                            UiHit::WorldCapacity(capacity),
                        );
                    }
                    if scenario == ValidationScenario::LiquidSandbox
                        && selected_engine == ComparisonEngine::PhysBox
                    {
                        for (index, quality) in
                            FluidSolidCouplingQuality::ALL.into_iter().enumerate()
                        {
                            push(
                                self.world_coupling_quality_button(index),
                                UiHit::FluidSolidCouplingQuality(quality),
                            );
                        }
                    }
                    push(self.world_sleeping_button(), UiHit::ToggleSleeping);
                    push(
                        self.world_sleep_threshold_button(),
                        UiHit::CycleSleepThreshold,
                    );
                    if selected_engine == ComparisonEngine::PhysBox {
                        push(
                            self.world_sleep_grouping_button(),
                            UiHit::CycleSleepGrouping,
                        );
                    }
                    push(self.world_sleep_visuals_button(), UiHit::ToggleSleepVisuals);
                    push(self.world_render_scale_button(), UiHit::CycleRenderScale);
                }
            }
        }
        for (target, hit) in &mut hits {
            if let UiHit::SolverSlider(parameter) = hit {
                target.role = WidgetRole::HorizontalSlider {
                    steps: parameter.slider_steps(),
                };
            }
        }
        hits
    }

    #[cfg(all(test, feature = "box2d-comparator"))]
    fn hit_test(
        self,
        normalized: [f32; 2],
        menu_open: bool,
        menu_page: MenuPage,
        solver_profiles: &[SolverProfileConfig],
        scenario: ValidationScenario,
        selected_engine: ComparisonEngine,
    ) -> Option<UiHit> {
        let point = Point::new(
            normalized[0] * self.target_size[0] as f32,
            normalized[1] * self.target_size[1] as f32,
        );
        self.hit_targets(
            menu_open,
            menu_page,
            solver_profiles,
            scenario,
            selected_engine,
        )
        .into_iter()
        .rev()
        .find_map(|(target, hit)| target.bounds.contains(point).then_some(hit))
    }

    fn menu_button(self) -> [f32; 4] {
        [
            12.0 * self.toolbar_scale,
            24.0 * self.toolbar_scale,
            52.0 * self.toolbar_scale,
            52.0 * self.toolbar_scale,
        ]
    }

    fn tool_button(self, tool: WorkbenchTool) -> [f32; 4] {
        let index = match tool {
            WorkbenchTool::Grab => 0,
            WorkbenchTool::Circle => 1,
            WorkbenchTool::Rectangle => 2,
            WorkbenchTool::Erase => 3,
            WorkbenchTool::Water => 4,
            WorkbenchTool::Surface => 5,
        };
        self.tool_button_at(index)
    }

    fn tool_button_at(self, index: usize) -> [f32; 4] {
        let logical_width = self.target_size[0] as f32 / self.toolbar_scale;
        let (stride, width) = if logical_width < 460.0 {
            let stride = ((logical_width - 78.0) / WorkbenchTool::ALL.len() as f32).max(1.0);
            (stride, (stride - 6.0).max(1.0))
        } else {
            (58.0, 52.0)
        };
        [
            (70.0 + index as f32 * stride) * self.toolbar_scale,
            24.0 * self.toolbar_scale,
            width * self.toolbar_scale,
            52.0 * self.toolbar_scale,
        ]
    }

    fn menu_panel(self, page: MenuPage, solver_profile_count: usize) -> [f32; 4] {
        let bottom = match page {
            MenuPage::Scenes => self.restart_y_logical() + 50.0,
            MenuPage::Solvers => {
                let apply = self.apply_solver_button(solver_profile_count);
                (apply[1] + apply[3]) / self.menu_scale
                    + if self.compact_landscape() { 4.0 } else { 8.0 }
            }
            MenuPage::World => {
                let render_scale = self.world_render_scale_button();
                (render_scale[1] + render_scale[3]) / self.menu_scale + 62.0
            }
        };
        let wide = self.compact_landscape()
            || (page == MenuPage::Scenes && self.scenario_columns() > 1)
            || (page == MenuPage::Solvers && self.solver_profile_columns(solver_profile_count) > 1);
        let logical = if self.phone_portrait() {
            [
                12.0,
                84.0,
                (self.target_size[0] as f32 / self.menu_scale - 24.0).max(336.0),
                bottom - 84.0,
            ]
        } else if wide {
            [12.0, 84.0, 620.0, bottom - 84.0]
        } else {
            [12.0, 84.0, 360.0, bottom - 84.0]
        };
        logical.map(|value| value * self.menu_scale)
    }

    fn menu_tab(self, page: MenuPage) -> [f32; 4] {
        let (x, width) = match page {
            MenuPage::Scenes => (24.0, 104.0),
            MenuPage::Solvers => (136.0, 104.0),
            MenuPage::World => (248.0, 104.0),
        };
        [
            x * self.menu_scale,
            90.0 * self.menu_scale,
            width * self.menu_scale,
            28.0 * self.menu_scale,
        ]
    }

    fn scenario_button(self, index: usize) -> [f32; 4] {
        let columns = self.scenario_columns();
        if columns > 1 {
            let column = index % columns;
            let row = index / columns;
            let (stride, width) = if columns == 3 {
                (198.0, 188.0)
            } else {
                (298.0, 286.0)
            };
            return [
                (24.0 + column as f32 * stride) * self.menu_scale,
                (126.0 + row as f32 * 38.0) * self.menu_scale,
                width * self.menu_scale,
                34.0 * self.menu_scale,
            ];
        }
        [
            24.0 * self.menu_scale,
            (126.0 + index as f32 * 38.0) * self.menu_scale,
            336.0 * self.menu_scale,
            34.0 * self.menu_scale,
        ]
    }

    fn world_capacity_button(self, index: usize) -> [f32; 4] {
        [
            24.0 * self.menu_scale,
            (154.0 + index as f32 * 38.0) * self.menu_scale,
            336.0 * self.menu_scale,
            34.0 * self.menu_scale,
        ]
    }

    fn world_coupling_quality_button(self, index: usize) -> [f32; 4] {
        if self.compact_landscape() {
            return [
                (372.0 + index as f32 * 126.0) * self.menu_scale,
                154.0 * self.menu_scale,
                120.0 * self.menu_scale,
                34.0 * self.menu_scale,
            ];
        }
        let last = self.world_capacity_button(WorldCapacityPreset::ALL.len() - 1);
        [
            (24.0 + index as f32 * 172.0) * self.menu_scale,
            last[1] + last[3] + 12.0 * self.menu_scale,
            164.0 * self.menu_scale,
            34.0 * self.menu_scale,
        ]
    }

    #[cfg(feature = "box2d-comparator")]
    fn world_engine_button(self, index: usize) -> [f32; 4] {
        [
            (112.0 + index as f32 * 122.0) * self.menu_scale,
            122.0 * self.menu_scale,
            114.0 * self.menu_scale,
            26.0 * self.menu_scale,
        ]
    }

    fn world_sleeping_button(self) -> [f32; 4] {
        let coupling = self.world_coupling_quality_button(0);
        if self.compact_landscape() {
            return [
                coupling[0],
                coupling[1] + coupling[3] + 12.0 * self.menu_scale,
                248.0 * self.menu_scale,
                34.0 * self.menu_scale,
            ];
        }
        [
            24.0 * self.menu_scale,
            coupling[1] + coupling[3] + 12.0 * self.menu_scale,
            336.0 * self.menu_scale,
            34.0 * self.menu_scale,
        ]
    }

    fn world_sleep_threshold_button(self) -> [f32; 4] {
        let sleeping = self.world_sleeping_button();
        [
            sleeping[0],
            sleeping[1] + sleeping[3] + 12.0 * self.menu_scale,
            sleeping[2],
            sleeping[3],
        ]
    }

    fn world_sleep_grouping_button(self) -> [f32; 4] {
        let threshold = self.world_sleep_threshold_button();
        [
            threshold[0],
            threshold[1] + threshold[3] + 12.0 * self.menu_scale,
            threshold[2],
            threshold[3],
        ]
    }

    fn world_sleep_visuals_button(self) -> [f32; 4] {
        let grouping = self.world_sleep_grouping_button();
        [
            grouping[0],
            grouping[1] + grouping[3] + 12.0 * self.menu_scale,
            grouping[2],
            grouping[3],
        ]
    }

    fn world_render_scale_button(self) -> [f32; 4] {
        if self.compact_landscape() {
            let last = self.world_capacity_button(WorldCapacityPreset::ALL.len() - 1);
            return [
                last[0],
                last[1] + last[3] + 12.0 * self.menu_scale,
                last[2],
                last[3],
            ];
        }
        let visuals = self.world_sleep_visuals_button();
        [
            visuals[0],
            visuals[1] + visuals[3] + 50.0 * self.menu_scale,
            visuals[2],
            visuals[3],
        ]
    }

    fn restart_button(self) -> [f32; 4] {
        if self.compact_landscape() {
            return [
                24.0 * self.menu_scale,
                self.restart_y_logical() * self.menu_scale,
                226.0 * self.menu_scale,
                38.0 * self.menu_scale,
            ];
        }
        [
            24.0 * self.menu_scale,
            self.restart_y_logical() * self.menu_scale,
            162.0 * self.menu_scale,
            38.0 * self.menu_scale,
        ]
    }

    fn recenter_button(self) -> [f32; 4] {
        if self.compact_landscape() {
            return [
                262.0 * self.menu_scale,
                self.restart_y_logical() * self.menu_scale,
                226.0 * self.menu_scale,
                38.0 * self.menu_scale,
            ];
        }
        [
            198.0 * self.menu_scale,
            self.restart_y_logical() * self.menu_scale,
            162.0 * self.menu_scale,
            38.0 * self.menu_scale,
        ]
    }

    fn solver_profile_button(self, index: usize, profile_count: usize) -> [f32; 4] {
        let columns = self.solver_profile_columns(profile_count);
        let column = index % columns;
        let row = index / columns;
        let (start_y, stride, height) = self.solver_profile_metrics();
        [
            (24.0 + column as f32 * 298.0) * self.menu_scale,
            (start_y + row as f32 * stride) * self.menu_scale,
            if columns == 1 { 336.0 } else { 286.0 } * self.menu_scale,
            height * self.menu_scale,
        ]
    }

    fn solver_parameter_slider(self, index: usize, profile_count: usize) -> [f32; 4] {
        let (base, stride, height) = self.solver_parameter_metrics(profile_count);
        let (column, row) = self.solver_parameter_grid_position(index);
        let phone_portrait = self.phone_portrait();
        let logical_width = self.target_size[0] as f32 / self.menu_scale;
        let x = if self.compact_landscape() {
            160.0 + column as f32 * 298.0
        } else if phone_portrait {
            (logical_width * 0.45).clamp(166.0, 235.0)
        } else {
            166.0
        };
        let width = if self.compact_landscape() {
            128.0
        } else if phone_portrait {
            (logical_width - x - 24.0).max(1.0)
        } else {
            194.0
        };
        [
            x * self.menu_scale,
            (base + row as f32 * stride) * self.menu_scale,
            width * self.menu_scale,
            height * self.menu_scale,
        ]
    }

    fn solver_parameter_text_origins(
        self,
        index: usize,
        profile_count: usize,
    ) -> ([f32; 2], [f32; 2]) {
        let (base, stride, _) = self.solver_parameter_metrics(profile_count);
        let (column, row) = self.solver_parameter_grid_position(index);
        let column_offset = if self.compact_landscape() {
            column as f32 * 298.0
        } else {
            0.0
        };
        let phone_portrait = self.phone_portrait();
        let y = (base + if phone_portrait { 11.0 } else { 8.0 } + row as f32 * stride)
            * self.menu_scale;
        (
            [(28.0 + column_offset) * self.menu_scale, y],
            [
                (if self.compact_landscape() {
                    135.0 + column_offset
                } else if phone_portrait {
                    150.0
                } else {
                    170.0
                }) * self.menu_scale,
                y,
            ],
        )
    }

    fn solver_parameter_grid_position(self, index: usize) -> (usize, usize) {
        let columns = if self.compact_landscape() { 2 } else { 1 };
        (index % columns, index / columns)
    }

    #[cfg_attr(not(feature = "solver-lab"), allow(dead_code))]
    fn solver_scene_button(self, index: usize, profile_count: usize) -> [f32; 4] {
        let (base, stride, height) = self.solver_parameter_metrics(profile_count);
        let parameter_columns = if self.compact_landscape() { 2 } else { 1 };
        let parameter_rows = crate::solver_profile::SolverParameter::ALL
            .len()
            .div_ceil(parameter_columns);
        let y = base
            + parameter_rows as f32 * stride
            + if self.compact_landscape() { 2.0 } else { 10.0 };
        [
            (24.0 + index as f32 * 174.0) * self.menu_scale,
            y * self.menu_scale,
            162.0 * self.menu_scale,
            height * self.menu_scale,
        ]
    }

    fn apply_solver_button(self, profile_count: usize) -> [f32; 4] {
        let scene = self.solver_scene_button(0, profile_count);
        let compact = self.compact_landscape();
        [
            198.0 * self.menu_scale,
            scene[1] + scene[3] + if compact { 4.0 } else { 14.0 } * self.menu_scale,
            162.0 * self.menu_scale,
            if compact { 32.0 } else { 38.0 } * self.menu_scale,
        ]
    }

    fn restore_solver_button(self, profile_count: usize) -> [f32; 4] {
        let apply = self.apply_solver_button(profile_count);
        [
            24.0 * self.menu_scale,
            apply[1],
            162.0 * self.menu_scale,
            apply[3],
        ]
    }

    fn solver_profile_columns(self, profile_count: usize) -> usize {
        if self.compact_landscape() && profile_count > 4 {
            3
        } else {
            usize::from(
                self.compact_landscape()
                    || (profile_count > 4 && self.target_size[0] as f32 / self.menu_scale >= 720.0),
            ) + 1
        }
    }

    fn solver_profile_metrics(self) -> (f32, f32, f32) {
        if self.compact_landscape() {
            (144.0, 30.0, 28.0)
        } else {
            (154.0, 38.0, 34.0)
        }
    }

    fn solver_parameter_metrics(self, profile_count: usize) -> (f32, f32, f32) {
        let columns = self.solver_profile_columns(profile_count);
        let rows = profile_count.div_ceil(columns);
        let (start_y, profile_stride, profile_height) = self.solver_profile_metrics();
        let compact = self.compact_landscape();
        let dense = rows > 2;
        (
            start_y
                + rows.saturating_sub(1) as f32 * profile_stride
                + profile_height
                + if compact { 4.0 } else { 10.0 },
            if compact {
                30.0
            } else if self.phone_portrait() {
                58.0
            } else if dense {
                32.0
            } else {
                38.0
            },
            if compact {
                28.0
            } else if self.phone_portrait() {
                52.0
            } else if dense {
                30.0
            } else {
                34.0
            },
        )
    }

    fn compact_landscape(self) -> bool {
        let logical = self.logical_target_size();
        logical[1] < 520.0 && logical[0] > logical[1]
    }

    fn phone_portrait(self) -> bool {
        let logical = self.logical_target_size();
        logical[0] < 520.0 && logical[1] >= logical[0]
    }

    fn scenario_columns(self) -> usize {
        if self.compact_landscape() {
            3
        } else if self.target_size[0] as f32 / self.menu_scale >= 632.0 {
            2
        } else {
            1
        }
    }

    fn restart_y_logical(self) -> f32 {
        let rows = ValidationScenario::MENU
            .len()
            .div_ceil(self.scenario_columns());
        let gap = if self.compact_landscape() { 20.0 } else { 10.0 };
        126.0 + (rows.saturating_sub(1)) as f32 * 38.0 + 34.0 + gap
    }

    pub(crate) fn center_normalized(self, hit: &str, index: usize) -> [f32; 2] {
        let bounds = match hit {
            "menu" => self.menu_button(),
            "tool" => self.tool_button(WorkbenchTool::ALL[index]),
            "scenario" => self.scenario_button(index),
            "restart" => self.restart_button(),
            "recenter" => self.recenter_button(),
            "solver-profile" => {
                self.solver_profile_button(index, SolverProfileConfig::available().len())
            }
            "solver-slider" => {
                self.solver_parameter_slider(index, SolverProfileConfig::available().len())
            }
            "apply-solver" => self.apply_solver_button(SolverProfileConfig::available().len()),
            "restore-solver" => self.restore_solver_button(SolverProfileConfig::available().len()),
            "world-tab" => self.menu_tab(MenuPage::World),
            "world-capacity" => self.world_capacity_button(index),
            "world-coupling-quality" => self.world_coupling_quality_button(index),
            "world-sleeping" => self.world_sleeping_button(),
            "world-sleep-threshold" => self.world_sleep_threshold_button(),
            "world-sleep-grouping" => self.world_sleep_grouping_button(),
            "world-sleep-visuals" => self.world_sleep_visuals_button(),
            "world-render-scale" => self.world_render_scale_button(),
            #[cfg(feature = "box2d-comparator")]
            "world-engine" => self.world_engine_button(index),
            _ => panic!("unknown UI test target"),
        };
        [
            (bounds[0] + bounds[2] * 0.5) / self.target_size[0] as f32,
            (bounds[1] + bounds[3] * 0.5) / self.target_size[1] as f32,
        ]
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticKey {
    target_size: [u32; 2],
    toolbar_scale_bits: u32,
    menu_scale_bits: u32,
    scenario: ValidationScenario,
    menu_open: bool,
    menu_page: MenuPage,
    selected_tool: WorkbenchTool,
    solver_profile_key: String,
    active_solver_profile: SolverProfileName,
    solver_profile_count: usize,
    world_capacity: WorldCapacityPreset,
    fluid_solid_coupling_quality: FluidSolidCouplingQuality,
    sleeping_effective: bool,
    sleep_visuals_enabled: bool,
    render_scale: RenderScalePreset,
    selected_engine: ComparisonEngine,
}

struct Batch {
    primitives: Vec<Primitive>,
    overflowed: bool,
}

impl Batch {
    fn new(capacity: usize) -> Self {
        Self {
            primitives: Vec::with_capacity(capacity.min(96)),
            overflowed: false,
        }
    }

    fn push_quad(&mut self, rect: [f32; 4], color: [f32; 4]) {
        self.primitives.push(Primitive::RoundedRect {
            bounds: Rect::from_array(rect),
            fill: color,
            radius: 10.0,
            border_width: 1.0,
            border: Theme::DARK.border,
        });
    }

    fn push_text(&mut self, text: &str, origin: [f32; 2], pixel: f32, color: [f32; 4]) {
        self.primitives.push(Primitive::Text {
            text: text.to_owned(),
            origin: Point::new(origin[0], origin[1]),
            pixel_size: pixel,
            color,
        });
    }
}

fn ui_report(
    model: UiFrameModel<'_>,
    menu_open: bool,
    selected_tool: WorkbenchTool,
    sleep_visuals_enabled: bool,
    layout: UiLayout,
    canvas: CanvasRenderReport,
) -> UiReport {
    UiReport {
        version: UI_CONTRACT_VERSION,
        pixels_per_point: layout.display_metrics.pixels_per_point(),
        toolbar_scale: layout.toolbar_scale,
        menu_scale: layout.menu_scale,
        diagnostics_scale: layout.diagnostics_scale,
        menu_open,
        current_tool: selected_tool.name(),
        current_scene: model.scenario.name(),
        selected_engine: model.selected_engine.name(),
        comparator_available: model.comparator_available,
        box2d_revision: model.box2d_revision,
        box2d_completed_steps: model.box2d_completed_steps,
        box2d_live_bodies: model.box2d_live_bodies,
        box2d_pending_actions: model.box2d_pending_actions,
        box2d_resolved_actions: model.box2d_resolved_actions,
        box2d_identity_disagreements: model.box2d_identity_disagreements,
        box2d_action_queue_overflowed: model.box2d_action_queue_overflowed,
        box2d_aligned: model.box2d_aligned,
        box2d_failure: model.box2d_failure.map(str::to_owned),
        sleep_visuals_enabled,
        static_instance_capacity: canvas.retained_instance_capacity,
        dynamic_instance_capacity: canvas.live_instance_capacity,
        instance_stride_bytes: canvas.instance_stride_bytes,
        glyph_atlas_bytes: canvas.atlas_bytes,
        allocated_bytes: canvas.allocated_bytes,
        static_instances: canvas.retained_instances,
        dynamic_instances: canvas.live_instances,
        static_glyphs: canvas.retained_glyphs,
        dynamic_glyphs: canvas.live_glyphs,
        static_writes_total: canvas.retained_writes_total,
        dynamic_writes_total: canvas.live_writes_total,
        parameter_writes_total: canvas.parameter_writes_total,
        static_upload_bytes_total: canvas.retained_upload_bytes_total,
        dynamic_upload_bytes_total: canvas.live_upload_bytes_total,
        parameter_upload_bytes_total: canvas.parameter_upload_bytes_total,
        render_passes_this_frame: canvas.render_passes_this_frame,
        draw_calls_this_frame: canvas.draw_calls_this_frame,
        diagnostic_source: model.construction_diagnostic_source,
        diagnostic_age_frames: model.construction_age_frames,
        scenario_activations_total: model.scenario_activations_total,
        scenario_restarts_total: model.scenario_restarts_total,
        overflowed: canvas.overflowed,
    }
}

struct StaticBatchModel<'a> {
    scenario: ValidationScenario,
    menu_open: bool,
    menu_page: MenuPage,
    selected_tool: WorkbenchTool,
    active_solver_profile: &'a SolverProfileConfig,
    solver_profiles: &'a [SolverProfileConfig],
    draft_solver_profile: &'a SolverProfileConfig,
    world_capacity: WorldCapacityPreset,
    fluid_solid_coupling_quality: FluidSolidCouplingQuality,
    sleeping_effective: bool,
    sleep_visuals_enabled: bool,
    render_scale: RenderScalePreset,
    selected_engine: ComparisonEngine,
}

fn static_batch(
    target_size: [u32; 2],
    display_metrics: DisplayMetrics,
    model: StaticBatchModel<'_>,
) -> Batch {
    let StaticBatchModel {
        scenario,
        menu_open,
        menu_page,
        selected_tool,
        active_solver_profile,
        solver_profiles,
        draft_solver_profile,
        world_capacity,
        fluid_solid_coupling_quality,
        sleeping_effective,
        sleep_visuals_enabled,
        render_scale,
        selected_engine,
    } = model;
    let layout = UiLayout::for_display(target_size, display_metrics);
    let scale = layout.menu_scale;
    let theme = Theme::DARK;
    let mut canvas = WidgetCanvas::new(theme, STATIC_INSTANCE_CAPACITY, 64);
    let mut next_widget_id = 1_u64;
    macro_rules! widget_id {
        () => {{
            let id = WidgetId::new(next_widget_id);
            next_widget_id += 1;
            id
        }};
    }

    canvas.icon_button(
        widget_id!(),
        Rect::from_array(layout.menu_button()),
        icons::MORE_HORIZONTAL,
        menu_open,
    );
    for (index, tool) in available_tools(scenario, selected_engine)
        .iter()
        .enumerate()
    {
        canvas.icon_button(
            widget_id!(),
            Rect::from_array(layout.tool_button_at(index)),
            tool_icon(*tool),
            *tool == selected_tool,
        );
    }

    if menu_open {
        canvas.panel(Rect::from_array(
            layout.menu_panel(menu_page, solver_profiles.len()),
        ));
        for page in menu_pages(selected_engine) {
            let label = match page {
                MenuPage::Scenes => "SCENES",
                MenuPage::Solvers => "SOLVERS",
                MenuPage::World => "WORLD",
            };
            canvas.tab(
                widget_id!(),
                Rect::from_array(layout.menu_tab(*page)),
                label,
                *page == menu_page,
                1.65 * scale,
            );
        }
        match menu_page {
            MenuPage::Scenes => {
                for (index, candidate) in ValidationScenario::MENU.iter().copied().enumerate() {
                    canvas.button(
                        widget_id!(),
                        Rect::from_array(layout.scenario_button(index)),
                        candidate.name(),
                        candidate == scenario,
                        1.65 * scale,
                    );
                }
                canvas.button(
                    widget_id!(),
                    Rect::from_array(layout.restart_button()),
                    "RESTART",
                    false,
                    1.65 * scale,
                );
                canvas.button(
                    widget_id!(),
                    Rect::from_array(layout.recenter_button()),
                    "RECENTER",
                    false,
                    1.65 * scale,
                );
            }
            MenuPage::Solvers => {
                let profile_count = solver_profiles.len();
                canvas.label(
                    &format!(
                        "ACTIVE {} {}",
                        active_solver_profile.label(),
                        if draft_solver_profile.is_preset() {
                            "PRESET"
                        } else {
                            "CUSTOM"
                        }
                    ),
                    Point::new(28.0 * scale, 128.0 * scale),
                    1.45 * scale,
                    theme.accent,
                );
                for (index, profile) in solver_profiles.iter().enumerate() {
                    canvas.button(
                        widget_id!(),
                        Rect::from_array(layout.solver_profile_button(index, profile_count)),
                        profile.label(),
                        profile.profile_name() == draft_solver_profile.profile_name(),
                        1.6 * scale,
                    );
                }
                for (index, parameter) in crate::solver_profile::SolverParameter::ALL
                    .iter()
                    .copied()
                    .enumerate()
                {
                    let Some(value) = draft_solver_profile.parameter_value(parameter) else {
                        continue;
                    };
                    let (label_origin, value_origin) =
                        layout.solver_parameter_text_origins(index, profile_count);
                    canvas.label(
                        parameter.label(),
                        Point::new(label_origin[0], label_origin[1]),
                        1.45 * scale,
                        theme.text,
                    );
                    let display =
                        if parameter == crate::solver_profile::SolverParameter::ContactHertz {
                            let effective = draft_solver_profile.effective_dynamic_contact_hz();
                            if (effective - value).abs() > 0.001 {
                                format!(
                                    "{}>{}",
                                    crate::solver_profile::display_number_for_ui(value),
                                    crate::solver_profile::display_number_for_ui(effective)
                                )
                            } else {
                                crate::solver_profile::display_number_for_ui(value)
                            }
                        } else {
                            crate::solver_profile::display_number_for_ui(value)
                        };
                    let value_pixel_size = if layout.phone_portrait() {
                        2.1 * scale
                    } else {
                        1.45 * scale
                    };
                    let value_origin = if layout.phone_portrait() {
                        let slider = layout.solver_parameter_slider(index, profile_count);
                        let label_end = label_origin[0]
                            + text_width(parameter.label(), 1.45 * scale)
                            + 12.0 * scale;
                        let right_aligned =
                            slider[0] - text_width(&display, value_pixel_size) - 12.0 * scale;
                        Point::new(label_end.max(right_aligned), value_origin[1])
                    } else {
                        Point::new(value_origin[0], value_origin[1])
                    };
                    canvas.label(&display, value_origin, value_pixel_size, theme.accent);
                    canvas.discrete_slider(
                        widget_id!(),
                        Rect::from_array(layout.solver_parameter_slider(index, profile_count)),
                        draft_solver_profile
                            .parameter_normalized(parameter)
                            .unwrap_or_default(),
                        parameter.slider_steps(),
                    );
                }
                #[cfg(feature = "solver-lab")]
                for (index, label) in ["STACK 8", "STACK 16"].iter().enumerate() {
                    canvas.button(
                        widget_id!(),
                        Rect::from_array(layout.solver_scene_button(index, profile_count)),
                        label,
                        false,
                        1.55 * scale,
                    );
                }
                canvas.button(
                    widget_id!(),
                    Rect::from_array(layout.restore_solver_button(profile_count)),
                    "RESET PRESET",
                    false,
                    1.5 * scale,
                );
                canvas.button(
                    widget_id!(),
                    Rect::from_array(layout.apply_solver_button(profile_count)),
                    "APPLY",
                    true,
                    1.65 * scale,
                );
            }
            MenuPage::World => {
                canvas.label(
                    #[cfg(feature = "box2d-comparator")]
                    "ENGINE",
                    #[cfg(not(feature = "box2d-comparator"))]
                    &format!("ACTIVE {}", world_capacity.label()),
                    Point::new(28.0 * scale, 128.0 * scale),
                    1.45 * scale,
                    theme.accent,
                );
                #[cfg(feature = "box2d-comparator")]
                for (index, engine) in [ComparisonEngine::PhysBox, ComparisonEngine::Box2d]
                    .into_iter()
                    .enumerate()
                {
                    canvas.button(
                        widget_id!(),
                        Rect::from_array(layout.world_engine_button(index)),
                        engine.label(),
                        engine == selected_engine,
                        1.4 * scale,
                    );
                }
                for (index, capacity) in WorldCapacityPreset::ALL.into_iter().enumerate() {
                    canvas.button(
                        widget_id!(),
                        Rect::from_array(layout.world_capacity_button(index)),
                        capacity.label(),
                        capacity == world_capacity,
                        1.6 * scale,
                    );
                }
                if scenario == ValidationScenario::LiquidSandbox
                    && selected_engine == ComparisonEngine::PhysBox
                {
                    for (index, quality) in FluidSolidCouplingQuality::ALL.into_iter().enumerate() {
                        canvas.button(
                            widget_id!(),
                            Rect::from_array(layout.world_coupling_quality_button(index)),
                            quality.label(),
                            quality == fluid_solid_coupling_quality,
                            1.25 * scale,
                        );
                    }
                }
                let sleeping_requested = draft_solver_profile.physics().sleeping_enabled;
                canvas.toggle(
                    widget_id!(),
                    Rect::from_array(layout.world_sleeping_button()),
                    "SLEEPING",
                    sleeping_requested,
                    1.4 * scale,
                );
                canvas.button(
                    widget_id!(),
                    Rect::from_array(layout.world_sleep_threshold_button()),
                    &format!(
                        "THRESHOLD {:.3} M/S",
                        draft_solver_profile.physics().sleep_speed_threshold
                    ),
                    false,
                    1.35 * scale,
                );
                if selected_engine == ComparisonEngine::PhysBox {
                    canvas.button(
                        widget_id!(),
                        Rect::from_array(layout.world_sleep_grouping_button()),
                        &format!(
                            "GROUPING {}",
                            draft_solver_profile.physics().sleep_grouping.label()
                        ),
                        false,
                        1.35 * scale,
                    );
                }
                canvas.toggle(
                    widget_id!(),
                    Rect::from_array(layout.world_sleep_visuals_button()),
                    "SLEEP VISUALS",
                    sleep_visuals_enabled,
                    1.35 * scale,
                );
                let visuals = layout.world_sleep_visuals_button();
                canvas.label(
                    match (sleeping_requested, sleeping_effective) {
                        (true, true) => "REQUESTED ON  EFFECTIVE ON",
                        (true, false) => "REQUESTED ON  EFFECTIVE OFF",
                        (false, _) => "REQUESTED OFF EFFECTIVE OFF",
                    },
                    Point::new(28.0 * scale, visuals[1] + visuals[3] + 12.0 * scale),
                    1.15 * scale,
                    theme.text_muted,
                );
                canvas.label(
                    "TEAL ACTIVE  AMBER QUIET  BLUE Z  PINK !",
                    Point::new(28.0 * scale, visuals[1] + visuals[3] + 31.0 * scale),
                    1.0 * scale,
                    theme.text_muted,
                );
                canvas.button(
                    widget_id!(),
                    Rect::from_array(layout.world_render_scale_button()),
                    &format!("RENDER SCALE {}", render_scale.label()),
                    false,
                    1.35 * scale,
                );
            }
        }
    }
    let scene = canvas.finish();
    let _ = next_widget_id;
    Batch {
        primitives: scene.primitives,
        overflowed: scene.overflowed,
    }
}

const fn tool_icon(tool: WorkbenchTool) -> physbox_ui::IconId {
    match tool {
        WorkbenchTool::Grab => icons::HAND,
        WorkbenchTool::Circle => icons::CIRCLE_OUTLINE,
        WorkbenchTool::Rectangle => icons::SQUARE_OUTLINE,
        WorkbenchTool::Erase => icons::CROSS,
        WorkbenchTool::Water => icons::DROPLETS,
        WorkbenchTool::Surface => icons::DIAGONAL_LINE,
    }
}

fn dynamic_batch(
    target_size: [u32; 2],
    display_metrics: DisplayMetrics,
    model: &UiFrameModel<'_>,
    diagnostics_visible: bool,
    selected_tool: WorkbenchTool,
) -> Batch {
    let layout = UiLayout::for_display(target_size, display_metrics);
    let scale = layout.diagnostics_scale;
    let mut batch = Batch::new(DYNAMIC_INSTANCE_CAPACITY);
    if !diagnostics_visible {
        return batch;
    }
    let compact_diagnostics = layout.logical_target_size()[1] < 520.0;
    let width = 310.0 * scale;
    let height = if model.scenario == ValidationScenario::LiquidSandbox
        && model.selected_engine == ComparisonEngine::PhysBox
    {
        (if compact_diagnostics { 416.0 } else { 530.0 }) * scale
    } else if model.selected_engine != ComparisonEngine::PhysBox {
        400.0 * scale
    } else {
        (if compact_diagnostics { 368.0 } else { 456.0 }) * scale
    };
    let logical_width = target_size[0] as f32 / scale;
    let origin = [
        target_size[0] as f32 - width - 12.0 * scale,
        if logical_width < 720.0 {
            target_size[1] as f32 - height - 12.0 * scale
        } else {
            24.0 * scale
        },
    ];
    batch.push_quad(
        [origin[0], origin[1], width, height],
        [0.015, 0.03, 0.065, 0.80],
    );
    let construction_failures = model.construction_state.map_or(0, |state| {
        state
            .capacity_failures
            .saturating_add(state.deep_overlap_failures)
            .saturating_add(state.erase_miss_failures)
            .saturating_add(state.generation_failures)
    });
    let construction_result =
        model
            .construction_state
            .map_or("WAIT", |state| match state.last_result {
                ConstructionResultV1::None => "READY",
                ConstructionResultV1::Placed => "PLACED",
                ConstructionResultV1::Erased => "ERASED",
                ConstructionResultV1::CapacityFull => "FULL",
                ConstructionResultV1::DeepOverlap => "OVERLAP",
                ConstructionResultV1::EraseMiss => "MISS",
                ConstructionResultV1::GenerationExhausted => "GENFAIL",
            });
    let live_bodies = if model.selected_engine == ComparisonEngine::PhysBox {
        model
            .construction_state
            .map_or(model.body_count, |state| state.live_bodies)
    } else {
        model.body_count
    };
    let physics = model.solver_profile.physics();
    let contact = if model.solver_profile.is_soft_coupled() {
        let effective = model.solver_profile.effective_dynamic_contact_hz();
        if (physics.contact_hertz - effective).abs() > 0.0005 {
            format!(
                "CONTACT {}>{}HZ",
                crate::solver_profile::display_number_for_ui(physics.contact_hertz),
                crate::solver_profile::display_number_for_ui(effective)
            )
        } else {
            format!(
                "CONTACT {}HZ",
                crate::solver_profile::display_number_for_ui(effective)
            )
        }
    } else {
        "CONTACT HARD".to_owned()
    };
    let simulation_status = if model.pacing.effective_simulation_scale >= 0.95 {
        "OK"
    } else {
        "SLOW"
    };
    let mut lines = vec![
        format!("ENGINE {}", model.selected_engine.label()),
        format!("SCENE {}", model.scenario.name()),
        format!(
            "SCALE {}% {}X{}",
            model.render_scale.requested_percent,
            model.render_scale.world_render_size[0],
            model.render_scale.world_render_size[1],
        ),
    ];
    match model.selected_engine {
        #[cfg(feature = "box2d-comparator")]
        ComparisonEngine::Box2d => {
            let revision = model.box2d_revision.unwrap_or("UNAVAILABLE");
            let split = revision.len().min(20);
            lines.extend([
                "BOX2D RIGID ONLY".to_owned(),
                "FLUID HIDDEN".to_owned(),
                format!("REV {}", &revision[..split]),
                format!("    {}", &revision[split..]),
                format!(
                    "CFG {}HZ DT{:.2}MS X{}",
                    physics.fixed_step_hz,
                    physics.fixed_dt_seconds() * 1_000.0,
                    physics.substeps
                ),
                format!(
                    "FPS {:.0}",
                    model.pacing.presented_frame_hz.clamp(0.0, 999.0)
                ),
                format!(
                    "SIM {:.0}% {} SLIP {:.0}MS",
                    (model.pacing.effective_simulation_scale * 100.0).clamp(0.0, 999.0),
                    simulation_status,
                    (model.pacing.overload_slipped_wall_seconds_total * 1000.0)
                        .clamp(0.0, 999_999.0)
                ),
                format!("BODY {}/{}", live_bodies, model.body_capacity),
                format!(
                    "STEP {} ALIGN {}",
                    model
                        .box2d_completed_steps
                        .map_or_else(|| "N/A".to_owned(), |steps| steps.to_string()),
                    match model.box2d_aligned {
                        Some(true) => "YES",
                        Some(false) => "NO",
                        None => "N/A",
                    }
                ),
                format!(
                    "ACTIONS P{} R{} D{}",
                    model.box2d_pending_actions.unwrap_or(0),
                    model.box2d_resolved_actions.unwrap_or(0),
                    model.box2d_identity_disagreements.unwrap_or(0),
                ),
                format!("SLEEP BOX2D T{:.3} PUBLIC", physics.sleep_speed_threshold),
                model.box2d_sleep_cycle.map_or_else(
                    || "CYCLE WAIT".to_owned(),
                    |cycle| {
                        format!(
                            "AWAKE {} SLEEP {} D{} W{}",
                            cycle.awake_bodies,
                            cycle.sleeping_bodies,
                            cycle.slept_this_step,
                            cycle.woke_this_step
                        )
                    },
                ),
                model.box2d_sleep_cycle.map_or_else(
                    || "AWT N/A STABLE N/A".to_owned(),
                    |cycle| {
                        format!(
                            "AWT {} STABLE {}",
                            cycle.awake_body_time,
                            cycle
                                .first_stable_sleep_step
                                .map_or_else(|| "N/A".to_owned(), |step| step.to_string())
                        )
                    },
                ),
                format!("RENDER {}", model.adapter_backend),
                model.box2d_failure.map_or_else(
                    || "SHADOW OK".to_owned(),
                    |failure| format!("SHADOW FAIL {failure}"),
                ),
            ]);
        }
        ComparisonEngine::PhysBox => {
            lines.extend([
                format!("SOLVER {}", model.solver_profile.label()),
                format!(
                    "CFG {}HZ DT{:.2}MS",
                    physics.fixed_step_hz,
                    physics.fixed_dt_seconds() * 1_000.0
                ),
                contact,
                format!(
                    "FPS {:.0}",
                    model.pacing.presented_frame_hz.clamp(0.0, 999.0)
                ),
                format!(
                    "SIM {:.0}% {} SLIP {:.0}MS",
                    (model.pacing.effective_simulation_scale * 100.0).clamp(0.0, 999.0),
                    simulation_status,
                    (model.pacing.overload_slipped_wall_seconds_total * 1000.0)
                        .clamp(0.0, 999_999.0)
                ),
                format!(
                    "INNER {}HZ X{} I{}",
                    physics.fixed_step_hz * physics.substeps,
                    physics.substeps,
                    physics.biased_iterations
                ),
                format!(
                    "BODY {}/{} POINTCAP {}",
                    live_bodies, model.body_capacity, model.contact_count
                ),
                format!(
                    "CAP {}KB OVERFLOW {}",
                    model.capacity_bytes.div_ceil(1024),
                    if model.overflowed { "YES" } else { "NO" }
                ),
                format!("GPU {}", model.adapter_backend),
                format!(
                    "BUILD {} FAIL {} AGE {}",
                    construction_result,
                    construction_failures,
                    model
                        .construction_age_frames
                        .map_or_else(|| "N/A".to_owned(), |age| age.to_string())
                ),
                format!(
                    "SLEEP {} T{:.3}",
                    physics.sleep_grouping.label(),
                    physics.sleep_speed_threshold
                ),
                model.sleep_cycle.map_or_else(
                    || "CYCLE WAIT".to_owned(),
                    |cycle| {
                        format!(
                            "AWAKE {} SLEEP {} D{} W{}",
                            cycle.awake_bodies,
                            cycle.sleeping_bodies,
                            cycle.slept_this_step,
                            cycle.woke_this_step
                        )
                    },
                ),
                model.sleep_cycle.map_or_else(
                    || "CUT N/A AWT N/A".to_owned(),
                    |cycle| {
                        format!(
                            "CUT {} AWT {}",
                            cycle
                                .mixed_awake_asleep_contact_cuts
                                .map_or_else(|| "N/A".to_owned(), |cuts| cuts.to_string()),
                            cycle.awake_body_time
                        )
                    },
                ),
                model.sleep_cycle.map_or_else(
                    || "STABLE N/A AGE N/A".to_owned(),
                    |cycle| {
                        format!(
                            "STABLE {} AGE {}",
                            cycle
                                .first_stable_sleep_step
                                .map_or_else(|| "N/A".to_owned(), |step| step.to_string()),
                            model
                                .sleep_cycle_age_frames
                                .map_or_else(|| "N/A".to_owned(), |age| age.to_string())
                        )
                    },
                ),
            ]);
            lines.extend(frame_timing_lines(model.frame_timing));
            if model.scenario == ValidationScenario::LiquidSandbox {
                lines.extend([
                    format!(
                        "WATER START {} CAP {}",
                        model.particle_count, model.particle_capacity
                    ),
                    format!(
                        "WATER WORK {} TOOL {}",
                        model.particle_dispatches_per_step,
                        selected_tool.name()
                    ),
                    format!(
                        "FLUID OVF DECL {} RB0",
                        if model.overflowed { "YES" } else { "NO" }
                    ),
                ]);
            }
        }
    }
    let color = [0.72, 0.84, 0.96, 1.0];
    let line_step = if compact_diagnostics { 16.0 } else { 20.0 };
    let text_size = if compact_diagnostics { 1.4 } else { 1.7 };
    for (index, line) in lines.iter().enumerate() {
        batch.push_text(
            line,
            [
                origin[0] + 10.0 * scale,
                origin[1] + (10.0 + index as f32 * line_step) * scale,
            ],
            text_size * scale,
            color,
        );
    }
    batch
}

fn frame_timing_lines(timing: &FrameTimingReport) -> [String; 5] {
    let value = |measurement: Option<f64>| {
        measurement.map_or_else(
            || "N/A".to_owned(),
            |milliseconds| format!("{milliseconds:.2}"),
        )
    };
    let status = if timing.error.is_some() {
        "ERROR"
    } else if !timing.timestamp_queries_supported {
        "UNAVAIL"
    } else if timing.total_gpu_ms.is_none() {
        "WAIT"
    } else {
        "READY"
    };
    [
        format!(
            "TIME {status} A{} RB{} ASYNC",
            timing
                .age_frames
                .map_or_else(|| "N/A".to_owned(), |age| age.to_string()),
            timing.pending_samples,
        ),
        format!(
            "PHYS RIG {} LIQ {}MS",
            value(timing.rigid_compute_ms),
            value(timing.liquid_compute_ms)
        ),
        format!(
            "DRAW B{} P{}MS",
            value(timing.body_render_ms),
            value(timing.particle_render_ms),
        ),
        format!(
            "POST C{} UI{}MS",
            value(timing.composite_render_ms),
            value(timing.ui_render_ms)
        ),
        format!(
            "TOTAL GPU{} CPU{}MS",
            value(timing.total_gpu_ms),
            value(timing.cpu_encode_ms),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PointerEvent, WheelEvent};

    fn pointer(id: u64, phase: PointerPhase, button: PointerButton, at: [f32; 2]) -> InputEvent {
        InputEvent::Pointer(PointerEvent {
            id,
            phase,
            button,
            normalized_position: at,
        })
    }

    #[test]
    fn debug_timing_lines_expose_gpu_stage_split_and_async_age() {
        let mut timing = FrameTimingReport::unavailable();
        assert_eq!(
            frame_timing_lines(&timing)[0],
            "TIME UNAVAIL AN/A RB0 ASYNC"
        );
        timing.timestamp_queries_supported = true;
        timing.rigid_compute_ms = Some(1.25);
        timing.liquid_compute_ms = Some(2.5);
        timing.body_render_ms = Some(0.5);
        timing.particle_render_ms = Some(0.75);
        timing.composite_render_ms = Some(0.125);
        timing.ui_render_ms = Some(0.25);
        timing.total_gpu_ms = Some(5.25);
        timing.cpu_encode_ms = Some(1.0);
        timing.age_frames = Some(2);
        timing.pending_samples = 1;
        let lines = frame_timing_lines(&timing);
        assert_eq!(lines[0], "TIME READY A2 RB1 ASYNC");
        assert_eq!(lines[1], "PHYS RIG 1.25 LIQ 2.50MS");
        assert_eq!(lines[2], "DRAW B0.50 P0.75MS");
        assert_eq!(lines[3], "POST C0.12 UI0.25MS");
        assert_eq!(lines[4], "TOTAL GPU5.25 CPU1.00MS");
    }

    #[test]
    fn layout_keeps_touch_targets_on_screen_at_phone_and_desktop_aspects() {
        for target in [[960, 600], [600, 960], [430, 932], [2400, 1080], [932, 430]] {
            let layout = UiLayout::for_target(target);
            for bounds in [
                layout.menu_button(),
                layout.menu_panel(MenuPage::Scenes, 0),
                layout.menu_panel(MenuPage::World, 0),
                layout.restart_button(),
                layout.recenter_button(),
            ]
            .into_iter()
            .chain(
                WorkbenchTool::ALL
                    .into_iter()
                    .map(|tool| layout.tool_button(tool)),
            )
            .chain(
                ValidationScenario::MENU
                    .iter()
                    .enumerate()
                    .map(|(index, _)| layout.scenario_button(index)),
            ) {
                assert!(bounds[0] >= 0.0 && bounds[1] >= 0.0);
                assert!(bounds[0] + bounds[2] <= target[0] as f32);
                assert!(bounds[1] + bounds[3] <= target[1] as f32);
            }
            for bounds in [
                layout.menu_tab(MenuPage::Scenes),
                layout.menu_tab(MenuPage::Solvers),
                layout.menu_tab(MenuPage::World),
                layout.world_capacity_button(0),
                layout.world_capacity_button(1),
                layout.world_capacity_button(2),
                layout.world_capacity_button(3),
                layout.world_coupling_quality_button(0),
                layout.world_coupling_quality_button(1),
                layout.world_sleeping_button(),
                layout.world_sleep_threshold_button(),
                layout.world_sleep_grouping_button(),
                layout.world_sleep_visuals_button(),
                layout.world_render_scale_button(),
            ] {
                assert!(bounds[0] >= 0.0 && bounds[1] >= 0.0);
                assert!(bounds[0] + bounds[2] <= target[0] as f32);
                assert!(bounds[1] + bounds[3] <= target[1] as f32);
            }
        }
    }

    #[test]
    fn display_density_preserves_phone_size_and_compacts_equivalent_desktops() {
        let phone = UiLayout::for_display(
            [860, 1_864],
            DisplayMetrics::new(2.0).expect("phone display metric is valid"),
        );
        let legacy_phone_scale = (860.0_f32 / 600.0).clamp(1.0, 2.0);
        assert_eq!(phone.toolbar_scale, legacy_phone_scale);
        assert_eq!(phone.menu_scale, legacy_phone_scale);
        assert_eq!(phone.diagnostics_scale, legacy_phone_scale);

        let desktop_1x = UiLayout::for_display([960, 600], DisplayMetrics::ONE);
        let desktop_2x = UiLayout::for_display(
            [1_920, 1_200],
            DisplayMetrics::new(2.0).expect("desktop display metric is valid"),
        );
        assert!((desktop_1x.menu_button()[2] - 36.0).abs() < 0.001);
        assert!((desktop_1x.menu_scale - COMPACT_MENU_SCALE).abs() < f32::EPSILON);
        assert!((desktop_1x.diagnostics_scale - COMPACT_DIAGNOSTICS_SCALE).abs() < f32::EPSILON);
        assert!((desktop_2x.toolbar_scale / 2.0 - desktop_1x.toolbar_scale).abs() < f32::EPSILON);
        assert!((desktop_2x.menu_scale / 2.0 - desktop_1x.menu_scale).abs() < f32::EPSILON);
        assert!(
            (desktop_2x.diagnostics_scale / 2.0 - desktop_1x.diagnostics_scale).abs()
                < f32::EPSILON
        );
        for (physical_1x, physical_2x) in desktop_1x
            .menu_panel(MenuPage::World, 0)
            .into_iter()
            .zip(desktop_2x.menu_panel(MenuPage::World, 0))
        {
            assert!((physical_2x / 2.0 - physical_1x).abs() < 0.001);
        }
    }

    #[test]
    fn world_menu_emits_one_explicit_capacity_restart_action() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut interactions = InteractionState::default();
        let mut menu_open = true;
        let mut menu_page = MenuPage::World;
        let mut selected_tool = WorkbenchTool::Circle;
        let mut profiles = SolverProfileConfig::available();
        let mut draft = SolverProfileConfig::default();
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                1,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-capacity", 3),
            )],
            target,
            DisplayMetrics::ONE,
        );
        assert_eq!(
            routed.action,
            Some(UiAction::ActivateWorldCapacity(
                WorldCapacityPreset::Slots10000
            ))
        );
        assert_eq!(routed.selected_tool, WorkbenchTool::Circle);
        assert!(!menu_open);
    }

    #[test]
    fn liquid_world_menu_emits_one_explicit_coupling_quality_restart_action() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut interactions = InteractionState::default();
        let mut menu_open = true;
        let mut menu_page = MenuPage::World;
        let mut selected_tool = WorkbenchTool::Circle;
        let mut profiles = SolverProfileConfig::available();
        let mut draft = SolverProfileConfig::default();
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::LiquidSandbox,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                1,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-coupling-quality", 1),
            )],
            target,
            DisplayMetrics::ONE,
        );
        assert_eq!(
            routed.action,
            Some(UiAction::ActivateFluidSolidCouplingQuality(
                FluidSolidCouplingQuality::Stabilized
            ))
        );
        assert_eq!(routed.selected_tool, WorkbenchTool::Circle);
        assert!(!menu_open);

        menu_open = true;
        let hidden = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                2,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-coupling-quality", 1),
            )],
            target,
            DisplayMetrics::ONE,
        );
        assert!(hidden.action.is_none());
    }

    #[test]
    fn world_menu_can_disable_the_default_sleeping_request_immediately() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut interactions = InteractionState::default();
        let mut menu_open = true;
        let mut menu_page = MenuPage::World;
        let mut selected_tool = WorkbenchTool::Rectangle;
        let mut profiles = SolverProfileConfig::available();
        let mut draft = SolverProfileConfig::default();
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                1,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-sleeping", 0),
            )],
            target,
            DisplayMetrics::ONE,
        );
        let Some(UiAction::ActivateSolverProfile(profile)) = routed.action else {
            panic!("sleeping toggle must apply one complete physics configuration");
        };
        assert!(!profile.physics().sleeping_enabled);
        assert_eq!(profile.profile_name(), SolverProfileName::SoftCoupled);
        assert_eq!(routed.selected_tool, WorkbenchTool::Rectangle);
        assert!(!menu_open);
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn world_menu_applies_shared_threshold_and_physbox_only_grouping() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut interactions = InteractionState::default();
        let mut menu_open = true;
        let mut menu_page = MenuPage::World;
        let mut selected_tool = WorkbenchTool::Grab;
        let mut profiles = SolverProfileConfig::available();
        let mut draft = SolverProfileConfig::default();

        let threshold = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::Box2d,
            },
            &[pointer(
                1,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-sleep-threshold", 0),
            )],
            target,
            DisplayMetrics::ONE,
        );
        let Some(UiAction::ActivateSolverProfile(profile)) = threshold.action else {
            panic!("shared threshold must apply one complete configuration");
        };
        assert_eq!(profile.physics().sleep_speed_threshold, 0.05);
        assert_eq!(
            profile.physics().sleep_grouping,
            physbox_launch::SleepGrouping::Exact
        );

        menu_open = true;
        let grouping = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                2,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-sleep-grouping", 0),
            )],
            target,
            DisplayMetrics::ONE,
        );
        let Some(UiAction::ActivateSolverProfile(profile)) = grouping.action else {
            panic!("PhysBox grouping must apply one complete configuration");
        };
        assert_eq!(
            profile.physics().sleep_grouping,
            physbox_launch::SleepGrouping::Local4
        );
        assert_eq!(profile.physics().sleep_speed_threshold, 0.05);

        menu_open = true;
        let hidden = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::Box2d,
            },
            &[pointer(
                3,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-sleep-grouping", 0),
            )],
            target,
            DisplayMetrics::ONE,
        );
        assert!(hidden.action.is_none());
        assert_eq!(
            draft.physics().sleep_grouping,
            physbox_launch::SleepGrouping::Local4
        );
    }

    #[test]
    fn world_menu_toggles_sleep_visuals_without_a_physics_action() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut interactions = InteractionState::default();
        let mut menu_open = true;
        let mut menu_page = MenuPage::World;
        let mut selected_tool = WorkbenchTool::Grab;
        let mut profiles = SolverProfileConfig::available();
        let mut draft = SolverProfileConfig::default();
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                1,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-sleep-visuals", 0),
            )],
            target,
            DisplayMetrics::ONE,
        );
        assert!(routed.sleep_visuals_toggled);
        assert!(routed.action.is_none());
        assert!(draft.physics().sleeping_enabled);
        assert!(!menu_open);
    }

    #[test]
    fn world_menu_cycles_render_scale_without_a_physics_action() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut interactions = InteractionState::default();
        let mut menu_open = true;
        let mut menu_page = MenuPage::World;
        let mut selected_tool = WorkbenchTool::Grab;
        let mut profiles = SolverProfileConfig::available();
        let mut draft = SolverProfileConfig::default();
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                1,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-render-scale", 0),
            )],
            target,
            DisplayMetrics::ONE,
        );
        assert!(routed.render_scale_cycled);
        assert!(routed.action.is_none());
        assert_eq!(draft.physics(), SolverProfileConfig::default().physics());
        assert!(!menu_open);
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn world_menu_switches_engine_and_hides_physbox_solver_page() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut interactions = InteractionState::default();
        let mut menu_open = true;
        let mut menu_page = MenuPage::World;
        let mut selected_tool = WorkbenchTool::Grab;
        let mut profiles = SolverProfileConfig::available();
        let mut draft = SolverProfileConfig::default();
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                1,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("world-engine", 1),
            )],
            target,
            DisplayMetrics::ONE,
        );
        assert_eq!(
            routed.action,
            Some(UiAction::SelectEngine(ComparisonEngine::Box2d))
        );
        assert!(!menu_open);
        assert_eq!(
            menu_pages(ComparisonEngine::Box2d),
            &[MenuPage::Scenes, MenuPage::World]
        );
        assert!(
            layout
                .hit_test(
                    layout.center_normalized("world-tab", 0),
                    true,
                    MenuPage::World,
                    &profiles,
                    ValidationScenario::Bootstrap,
                    ComparisonEngine::Box2d,
                )
                .is_some()
        );
        let solver_center = {
            let bounds = layout.menu_tab(MenuPage::Solvers);
            [
                (bounds[0] + bounds[2] * 0.5) / target[0] as f32,
                (bounds[1] + bounds[3] * 0.5) / target[1] as f32,
            ]
        };
        assert_eq!(
            layout.hit_test(
                solver_center,
                true,
                MenuPage::World,
                &profiles,
                ValidationScenario::Bootstrap,
                ComparisonEngine::Box2d,
            ),
            Some(UiHit::Panel)
        );
    }

    #[test]
    fn retained_canvas_descriptions_are_bounded_and_include_text() {
        let active = SolverProfileConfig::default();
        let profiles = SolverProfileConfig::available();
        let static_closed = static_batch(
            [960, 600],
            DisplayMetrics::ONE,
            StaticBatchModel {
                scenario: ValidationScenario::Bootstrap,
                menu_open: false,
                menu_page: MenuPage::Scenes,
                selected_tool: WorkbenchTool::Grab,
                active_solver_profile: &active,
                solver_profiles: &profiles,
                draft_solver_profile: &active,
                world_capacity: WorldCapacityPreset::Slots64,
                fluid_solid_coupling_quality: FluidSolidCouplingQuality::Standard,
                sleeping_effective: false,
                sleep_visuals_enabled: false,
                render_scale: RenderScalePreset::Percent100,
                selected_engine: ComparisonEngine::PhysBox,
            },
        );
        let static_open = static_batch(
            [960, 600],
            DisplayMetrics::ONE,
            StaticBatchModel {
                scenario: ValidationScenario::Bootstrap,
                menu_open: true,
                menu_page: MenuPage::Scenes,
                selected_tool: WorkbenchTool::Rectangle,
                active_solver_profile: &active,
                solver_profiles: &profiles,
                draft_solver_profile: &active,
                world_capacity: WorldCapacityPreset::Slots64,
                fluid_solid_coupling_quality: FluidSolidCouplingQuality::Standard,
                sleeping_effective: false,
                sleep_visuals_enabled: false,
                render_scale: RenderScalePreset::Percent100,
                selected_engine: ComparisonEngine::PhysBox,
            },
        );
        let static_world = static_batch(
            [960, 600],
            DisplayMetrics::ONE,
            StaticBatchModel {
                scenario: ValidationScenario::Bootstrap,
                menu_open: true,
                menu_page: MenuPage::World,
                selected_tool: WorkbenchTool::Rectangle,
                active_solver_profile: &active,
                solver_profiles: &profiles,
                draft_solver_profile: &active,
                world_capacity: WorldCapacityPreset::Slots10000,
                fluid_solid_coupling_quality: FluidSolidCouplingQuality::Standard,
                sleeping_effective: false,
                sleep_visuals_enabled: true,
                render_scale: RenderScalePreset::Percent50,
                selected_engine: ComparisonEngine::PhysBox,
            },
        );
        assert!(!static_closed.overflowed && !static_open.overflowed && !static_world.overflowed);
        assert!(static_open.primitives.len() > static_closed.primitives.len());
        assert!(
            static_open
                .primitives
                .iter()
                .any(|primitive| matches!(primitive, Primitive::Text { .. }))
        );
        assert!(static_open.primitives.len() <= STATIC_INSTANCE_CAPACITY);
        assert!(static_world.primitives.len() <= STATIC_INSTANCE_CAPACITY);
    }

    #[cfg(feature = "solver-lab")]
    #[test]
    fn solver_menu_batch_is_bounded() {
        let active = SolverProfileConfig::soft_coupled();
        let profiles = SolverProfileConfig::available();
        let batch = static_batch(
            [960, 600],
            DisplayMetrics::ONE,
            StaticBatchModel {
                scenario: ValidationScenario::SolverLabStack16,
                menu_open: true,
                menu_page: MenuPage::Solvers,
                selected_tool: WorkbenchTool::Grab,
                active_solver_profile: &active,
                solver_profiles: &profiles,
                draft_solver_profile: &active,
                world_capacity: WorldCapacityPreset::Slots64,
                sleeping_effective: false,
                sleep_visuals_enabled: false,
                render_scale: RenderScalePreset::Percent100,
                selected_engine: ComparisonEngine::PhysBox,
            },
        );
        assert!(!batch.overflowed);
        assert!(batch.primitives.len() <= STATIC_INSTANCE_CAPACITY);
    }

    #[cfg(feature = "solver-lab")]
    #[test]
    fn solver_menu_touch_targets_fit_phone_desktop_and_landscape() {
        let profile_count = 2;
        for target in [[960, 600], [600, 960], [430, 932], [2400, 1080], [932, 430]] {
            let layout = UiLayout::for_target(target);
            let bounds = [
                layout.menu_panel(MenuPage::Solvers, profile_count),
                layout.menu_tab(MenuPage::Scenes),
                layout.menu_tab(MenuPage::Solvers),
                layout.solver_profile_button(0, profile_count),
                layout.solver_profile_button(1, profile_count),
                layout.solver_parameter_slider(0, profile_count),
                layout.solver_parameter_slider(2, profile_count),
                layout.solver_scene_button(0, profile_count),
                layout.solver_scene_button(1, profile_count),
                layout.apply_solver_button(profile_count),
            ];
            for bounds in bounds {
                assert!(bounds[0] >= 0.0 && bounds[1] >= 0.0);
                assert!(bounds[0] + bounds[2] <= target[0] as f32);
                assert!(bounds[1] + bounds[3] <= target[1] as f32);
            }
        }
    }

    #[cfg(feature = "solver-lab")]
    #[test]
    fn phone_portrait_solver_sliders_are_wide_with_prominent_values() {
        let target = [1080, 2400];
        let display_metrics = DisplayMetrics::new(2.5).unwrap();
        let profiles = SolverProfileConfig::available();
        let active = SolverProfileConfig::soft_coupled();
        let layout = UiLayout::for_display(target, display_metrics);
        let slider = layout.solver_parameter_slider(0, profiles.len());
        let (_, value_origin) = layout.solver_parameter_text_origins(0, profiles.len());
        let panel = layout.menu_panel(MenuPage::Solvers, profiles.len());

        assert!(layout.phone_portrait());
        assert!(slider[2] / display_metrics.pixels_per_point() >= 240.0);
        assert!(slider[3] / display_metrics.pixels_per_point() >= 36.0);
        assert!(value_origin[0] / layout.menu_scale >= 150.0);
        assert!(value_origin[0] < slider[0]);
        assert!(panel[0] + panel[2] <= target[0] as f32);

        let batch = static_batch(
            target,
            display_metrics,
            StaticBatchModel {
                scenario: ValidationScenario::Bootstrap,
                menu_open: true,
                menu_page: MenuPage::Solvers,
                selected_tool: WorkbenchTool::Grab,
                active_solver_profile: &active,
                solver_profiles: &profiles,
                draft_solver_profile: &active,
                world_capacity: WorldCapacityPreset::Slots64,
                sleeping_effective: true,
                sleep_visuals_enabled: false,
                render_scale: RenderScalePreset::Percent100,
                selected_engine: ComparisonEngine::PhysBox,
            },
        );
        let value_size = batch
            .primitives
            .iter()
            .find_map(|primitive| match primitive {
                Primitive::Text {
                    text, pixel_size, ..
                } if text == "60" => Some(*pixel_size),
                _ => None,
            });
        assert_eq!(value_size, Some(2.1 * layout.menu_scale));

        let text_primitive = |expected: &str| {
            batch
                .primitives
                .iter()
                .find_map(|primitive| match primitive {
                    Primitive::Text {
                        text,
                        origin,
                        pixel_size,
                        ..
                    } if text == expected => Some((*origin, *pixel_size)),
                    _ => None,
                })
        };
        let (label_origin, label_size) = text_primitive("STATIC SCALE").unwrap();
        let (static_value_origin, static_value_size) = text_primitive("2").unwrap();
        let static_slider = layout.solver_parameter_slider(5, profiles.len());
        assert!(label_origin.x + text_width("STATIC SCALE", label_size) < static_value_origin.x);
        assert!(static_value_origin.x + text_width("2", static_value_size) < static_slider[0]);
    }

    #[cfg(feature = "solver-lab")]
    #[test]
    fn solver_menu_layout_scales_beyond_a_pair() {
        let profile_count = 6;
        for target in [[960, 600], [600, 960], [430, 932], [2400, 1080], [932, 430]] {
            let layout = UiLayout::for_target(target);
            for index in 0..profile_count {
                let bounds = layout.solver_profile_button(index, profile_count);
                assert!(bounds[0] >= 0.0 && bounds[1] >= 0.0);
                assert!(bounds[0] + bounds[2] <= target[0] as f32);
                assert!(bounds[1] + bounds[3] <= target[1] as f32);
            }
            let panel = layout.menu_panel(MenuPage::Solvers, profile_count);
            assert!(panel[0] + panel[2] <= target[0] as f32);
            assert!(panel[1] + panel[3] <= target[1] as f32);
        }
    }

    #[cfg(feature = "solver-lab")]
    #[test]
    fn solver_menu_edits_a_draft_and_applies_it_as_one_action() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut interactions = InteractionState::default();
        let mut menu_open = true;
        let mut menu_page = MenuPage::Solvers;
        let mut selected_tool = WorkbenchTool::Grab;
        let mut profiles = SolverProfileConfig::available();
        let mut draft = SolverProfileConfig::default();
        draft.set_sleeping_enabled(true);

        let profile_position = layout.center_normalized("solver-profile", 0);
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[
                pointer(
                    1,
                    PointerPhase::Down,
                    PointerButton::Primary,
                    profile_position,
                ),
                pointer(
                    1,
                    PointerPhase::Up,
                    PointerButton::Primary,
                    profile_position,
                ),
            ],
            target,
            DisplayMetrics::ONE,
        );
        assert!(routed.action.is_none());

        let desired = [75.0, 5.0, 2.0, 31.0, 10.5, 2.5];
        for (index, (parameter, desired)) in crate::solver_profile::SolverParameter::ALL
            .into_iter()
            .zip(desired)
            .enumerate()
        {
            let bounds = layout.solver_parameter_slider(index, profiles.len());
            let (minimum, maximum, _) = parameter.slider_spec();
            let normalized = (desired - minimum) / (maximum - minimum);
            let position = [
                (bounds[0] + bounds[2] * normalized) / target[0] as f32,
                (bounds[1] + bounds[3] * 0.5) / target[1] as f32,
            ];
            let start = [
                bounds[0] / target[0] as f32,
                (bounds[1] + bounds[3] * 0.5) / target[1] as f32,
            ];
            let id = 10 + index as u64;
            let routed = route_events(
                UiRouteState {
                    menu_open: &mut menu_open,
                    menu_page: &mut menu_page,
                    selected_tool: &mut selected_tool,
                    solver_profiles: &mut profiles,
                    draft_solver_profile: &mut draft,
                    interactions: &mut interactions,
                    scenario: ValidationScenario::Bootstrap,
                    selected_engine: ComparisonEngine::PhysBox,
                },
                &[
                    pointer(id, PointerPhase::Down, PointerButton::Primary, start),
                    pointer(id, PointerPhase::Move, PointerButton::Primary, position),
                    pointer(id, PointerPhase::Up, PointerButton::Primary, position),
                ],
                target,
                DisplayMetrics::ONE,
            );
            assert!(routed.action.is_none());
        }
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                8,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("apply-solver", 0),
            )],
            target,
            DisplayMetrics::ONE,
        );
        let UiAction::ActivateSolverProfile(profile) = routed.action.unwrap() else {
            panic!("solver menu must emit one profile activation");
        };
        assert_eq!(
            profile.options_string().as_deref(),
            Some("hz=31,damping=10.5,static-scale=2.5")
        );
        let physics = profile.physics();
        assert_eq!(physics.fixed_step_hz, 75);
        assert_eq!(physics.substeps, 5);
        assert_eq!(physics.biased_iterations, 2);
        assert!(physics.sleeping_enabled);
        assert!(!menu_open);

        menu_open = true;
        let restored = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut draft,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[pointer(
                9,
                PointerPhase::Down,
                PointerButton::Primary,
                layout.center_normalized("restore-solver", 0),
            )],
            target,
            DisplayMetrics::ONE,
        );
        assert!(restored.action.is_none());
        assert!(draft.is_preset());
        let mut expected = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
        expected.sleeping_enabled = true;
        assert_eq!(draft.physics(), expected);
    }

    #[test]
    fn ui_capture_precedes_world_input_and_secondary_stays_camera_owned() {
        let target = [960, 600];
        let menu = UiLayout::for_target(target).center_normalized("menu", 0);
        let mut interactions = InteractionState::default();
        let mut menu_open = false;
        let mut menu_page = MenuPage::Scenes;
        let mut selected_tool = WorkbenchTool::Grab;
        let mut profiles = SolverProfileConfig::available();
        let mut solver_profile = SolverProfileConfig::default();
        let events = [
            pointer(1, PointerPhase::Down, PointerButton::Primary, menu),
            pointer(1, PointerPhase::Up, PointerButton::Primary, menu),
            pointer(2, PointerPhase::Down, PointerButton::Secondary, menu),
            InputEvent::Wheel(WheelEvent {
                normalized_position: menu,
                zoom_steps: 1.0,
            }),
        ];
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut solver_profile,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &events,
            target,
            DisplayMetrics::ONE,
        );
        assert!(menu_open);
        assert_eq!(routed.events.len(), 2);
        assert!(matches!(routed.events[0], InputEvent::Pointer(_)));
        assert!(matches!(routed.events[1], InputEvent::Wheel(_)));
    }

    #[test]
    fn tool_buttons_are_shared_captured_and_select_exactly_one_tool() {
        let target = [430, 932];
        let layout = UiLayout::for_target(target);
        let rectangle = layout.center_normalized("tool", 2);
        let mut interactions = InteractionState::default();
        let mut menu_open = false;
        let mut menu_page = MenuPage::Scenes;
        let mut selected_tool = WorkbenchTool::Grab;
        let mut profiles = SolverProfileConfig::available();
        let mut solver_profile = SolverProfileConfig::default();
        let routed = route_events(
            UiRouteState {
                menu_open: &mut menu_open,
                menu_page: &mut menu_page,
                selected_tool: &mut selected_tool,
                solver_profiles: &mut profiles,
                draft_solver_profile: &mut solver_profile,
                interactions: &mut interactions,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[
                pointer(7, PointerPhase::Down, PointerButton::Contact, rectangle),
                pointer(7, PointerPhase::Up, PointerButton::Contact, rectangle),
            ],
            target,
            DisplayMetrics::ONE,
        );
        assert_eq!(selected_tool, WorkbenchTool::Rectangle);
        assert_eq!(routed.selected_tool, WorkbenchTool::Rectangle);
        assert!(routed.events.is_empty());
        assert_eq!(interactions.captured_pointers(), 0);
    }

    #[test]
    fn surface_is_shared_across_physbox_scenes_while_water_requires_particles() {
        assert_eq!(
            available_tools(ValidationScenario::LiquidSandbox, ComparisonEngine::PhysBox),
            &WorkbenchTool::ALL
        );
        assert_eq!(
            available_tools(ValidationScenario::Bootstrap, ComparisonEngine::PhysBox),
            &WorkbenchTool::PHYSBOX
        );
        #[cfg(feature = "box2d-comparator")]
        assert_eq!(
            available_tools(ValidationScenario::LiquidSandbox, ComparisonEngine::Box2d),
            &WorkbenchTool::BASE
        );
    }
}
