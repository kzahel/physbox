use std::collections::BTreeSet;

use bytemuck::{Pod, Zeroable};
use physbox_contracts::{ConstructionResultV1, ConstructionStateV1};
use physbox_launch::{SolverProfileName, ValidationScenario, WorldCapacityPreset};
use wgpu::util::DeviceExt;

use crate::{
    InputEvent, PointerButton, PointerPhase, SolverProfileConfig,
    clock::PacingReport,
    engine_selection::ComparisonEngine,
    renderer::RenderFrameTarget,
    report::UiReport,
    ui_font::{
        GLYPH_CELL_WIDTH, GLYPH_HEIGHT, GLYPH_WIDTH, glyph_atlas_bytes, glyph_atlas_height,
        glyph_atlas_r8, glyph_atlas_width, glyph_uv,
    },
};

pub(crate) const UI_CONTRACT_VERSION: u32 = 11;
const STATIC_INSTANCE_CAPACITY: usize = 384;
const DYNAMIC_INSTANCE_CAPACITY: usize = 320;

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum UiAction {
    ActivateScenario(ValidationScenario),
    ActivateSolverProfile(SolverProfileConfig),
    ActivateSolverProfileForScenario {
        profile: SolverProfileConfig,
        scenario: ValidationScenario,
    },
    ActivateWorldCapacity(WorldCapacityPreset),
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

    const fn label(self) -> &'static str {
        match self {
            Self::Grab => "GRAB",
            Self::Circle => "CIRC",
            Self::Rectangle => "RECT",
            Self::Erase => "ERASE",
            Self::Water => "WATER",
            Self::Surface => "SURF",
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
}

pub(crate) struct UiFrameModel<'a> {
    pub scenario: ValidationScenario,
    pub solver_profile: &'a SolverProfileConfig,
    pub pacing: &'a PacingReport,
    pub body_count: u32,
    pub world_capacity: WorldCapacityPreset,
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
}

struct UiViewState<'a> {
    menu_open: bool,
    menu_page: MenuPage,
    selected_tool: WorkbenchTool,
    sleep_visuals_enabled: bool,
    solver_profiles: &'a [SolverProfileConfig],
    draft_solver_profile: &'a SolverProfileConfig,
    selected_engine: ComparisonEngine,
}

/// Shared canvas UI and input capture.
///
/// The compact atlas, instanced glyph batch, static-write caching, and toolbar
/// sizing are selectively ported from PhysBox 4 revision
/// `4295957e15480ef701e6006c7cafb5f0728ef101`, principally `toolbar.rs`,
/// `options/font.rs`, `options/text.rs`, and `options/renderer.rs`. PhysBox 5
/// owns a smaller workbench-only model and one combined quad/glyph pipeline.
pub(crate) struct WorkbenchUi {
    renderer: UiRenderer,
    menu_open: bool,
    menu_page: MenuPage,
    selected_tool: WorkbenchTool,
    sleep_visuals_enabled: bool,
    solver_profiles: Vec<SolverProfileConfig>,
    draft_solver_profile: SolverProfileConfig,
    captured_pointers: BTreeSet<u64>,
}

impl WorkbenchUi {
    pub(crate) fn new(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        color_format: wgpu::TextureFormat,
        solver_profile: SolverProfileConfig,
        solver_profiles: Vec<SolverProfileConfig>,
    ) -> Self {
        Self {
            renderer: UiRenderer::new(device, queue, color_format),
            menu_open: false,
            menu_page: MenuPage::default(),
            selected_tool: WorkbenchTool::default(),
            sleep_visuals_enabled: false,
            solver_profiles,
            draft_solver_profile: solver_profile,
            captured_pointers: BTreeSet::new(),
        }
    }

    pub(crate) fn route_events(
        &mut self,
        events: &[InputEvent],
        target_size: [u32; 2],
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
                captured_pointers: &mut self.captured_pointers,
                scenario,
                selected_engine,
            },
            events,
            target_size,
        );
        if routing.sleep_visuals_toggled {
            self.sleep_visuals_enabled = !self.sleep_visuals_enabled;
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
        model: UiFrameModel<'_>,
    ) -> UiReport {
        let selected_engine = model.selected_engine;
        self.renderer.encode(
            queue,
            encoder,
            target,
            model,
            UiViewState {
                menu_open: self.menu_open,
                menu_page: self.menu_page,
                selected_tool: self.selected_tool,
                sleep_visuals_enabled: self.sleep_visuals_enabled,
                solver_profiles: &self.solver_profiles,
                draft_solver_profile: &self.draft_solver_profile,
                selected_engine,
            },
        )
    }
}

struct UiRouteState<'a> {
    menu_open: &'a mut bool,
    menu_page: &'a mut MenuPage,
    selected_tool: &'a mut WorkbenchTool,
    solver_profiles: &'a mut Vec<SolverProfileConfig>,
    draft_solver_profile: &'a mut SolverProfileConfig,
    captured_pointers: &'a mut BTreeSet<u64>,
    scenario: ValidationScenario,
    selected_engine: ComparisonEngine,
}

fn route_events(
    state: UiRouteState<'_>,
    events: &[InputEvent],
    target_size: [u32; 2],
) -> UiRouting {
    let UiRouteState {
        menu_open,
        menu_page,
        selected_tool,
        solver_profiles,
        draft_solver_profile,
        captured_pointers,
        scenario,
        selected_engine,
    } = state;
    let mut forwarded = Vec::with_capacity(events.len());
    let mut action = None;
    let mut sleep_visuals_toggled = false;
    for event in events.iter().copied() {
        let InputEvent::Pointer(pointer) = event else {
            forwarded.push(event);
            continue;
        };
        if captured_pointers.contains(&pointer.id) {
            if matches!(pointer.phase, PointerPhase::Up | PointerPhase::Cancel) {
                captured_pointers.remove(&pointer.id);
            }
            continue;
        }
        let activates_ui = matches!(
            pointer.button,
            PointerButton::Primary | PointerButton::Contact
        ) && pointer.phase == PointerPhase::Down;
        if activates_ui {
            let layout = UiLayout::for_target(target_size);
            if let Some(hit) = layout.hit_test(
                pointer.normalized_position,
                *menu_open,
                *menu_page,
                solver_profiles,
                scenario,
                selected_engine,
            ) {
                captured_pointers.insert(pointer.id);
                match hit {
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
                    UiHit::SolverParameter(parameter, direction) => {
                        draft_solver_profile.adjust(parameter, direction);
                    }
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
                    #[cfg(feature = "box2d-comparator")]
                    UiHit::Engine(engine) => {
                        *menu_open = false;
                        action = Some(UiAction::SelectEngine(engine));
                    }
                    UiHit::Panel => {}
                }
                continue;
            }
        }
        forwarded.push(event);
    }
    UiRouting {
        events: forwarded,
        action,
        selected_tool: *selected_tool,
        sleep_visuals_toggled,
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
    SolverParameter(crate::solver_profile::SolverParameter, i32),
    SolverScene(ValidationScenario),
    WorldCapacity(WorldCapacityPreset),
    ToggleSleeping,
    CycleSleepThreshold,
    CycleSleepGrouping,
    ToggleSleepVisuals,
    #[cfg(feature = "box2d-comparator")]
    Engine(ComparisonEngine),
    RestoreSolverPreset,
    ApplySolver,
    Panel,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct UiLayout {
    target_size: [u32; 2],
    scale: f32,
}

impl UiLayout {
    pub(crate) fn for_target(target_size: [u32; 2]) -> Self {
        let minimum_dimension = target_size[0].min(target_size[1]).max(1) as f32;
        Self {
            target_size,
            scale: (minimum_dimension / 600.0).clamp(1.0, 2.0),
        }
    }

    fn hit_test(
        self,
        normalized: [f32; 2],
        menu_open: bool,
        menu_page: MenuPage,
        solver_profiles: &[SolverProfileConfig],
        scenario: ValidationScenario,
        selected_engine: ComparisonEngine,
    ) -> Option<UiHit> {
        let pixel = [
            normalized[0] * self.target_size[0] as f32,
            normalized[1] * self.target_size[1] as f32,
        ];
        if contains(self.menu_button(), pixel) {
            return Some(UiHit::Menu);
        }
        for tool in available_tools(scenario, selected_engine) {
            if contains(self.tool_button(*tool), pixel) {
                return Some(UiHit::Tool(*tool));
            }
        }
        if !menu_open {
            return None;
        }
        for page in menu_pages(selected_engine) {
            if contains(self.menu_tab(*page), pixel) {
                return Some(UiHit::MenuPage(*page));
            }
        }
        match menu_page {
            MenuPage::Scenes => {
                for (index, scenario) in ValidationScenario::MENU.iter().copied().enumerate() {
                    if contains(self.scenario_button(index), pixel) {
                        return Some(UiHit::Scenario(scenario));
                    }
                }
                if contains(self.restart_button(), pixel) {
                    return Some(UiHit::Restart);
                }
                if contains(self.recenter_button(), pixel) {
                    return Some(UiHit::Recenter);
                }
            }
            MenuPage::Solvers => {
                if selected_engine != ComparisonEngine::PhysBox {
                    return contains(self.menu_panel(menu_page, solver_profiles.len()), pixel)
                        .then_some(UiHit::Panel);
                }
                let profile_count = solver_profiles.len();
                for (index, profile) in solver_profiles.iter().enumerate() {
                    if contains(self.solver_profile_button(index, profile_count), pixel) {
                        return Some(UiHit::SolverProfile(profile.profile_name()));
                    }
                }
                for (index, parameter) in crate::solver_profile::SolverParameter::ALL
                    .iter()
                    .copied()
                    .enumerate()
                {
                    if contains(self.solver_parameter_minus(index, profile_count), pixel) {
                        return Some(UiHit::SolverParameter(parameter, -1));
                    }
                    if contains(self.solver_parameter_plus(index, profile_count), pixel) {
                        return Some(UiHit::SolverParameter(parameter, 1));
                    }
                }
                #[cfg(feature = "solver-lab")]
                {
                    if contains(self.solver_scene_button(0, profile_count), pixel) {
                        return Some(UiHit::SolverScene(ValidationScenario::SolverLabStack8));
                    }
                    if contains(self.solver_scene_button(1, profile_count), pixel) {
                        return Some(UiHit::SolverScene(ValidationScenario::SolverLabStack16));
                    }
                }
                if contains(self.restore_solver_button(profile_count), pixel) {
                    return Some(UiHit::RestoreSolverPreset);
                }
                if contains(self.apply_solver_button(profile_count), pixel) {
                    return Some(UiHit::ApplySolver);
                }
            }
            MenuPage::World => {
                #[cfg(feature = "box2d-comparator")]
                for (index, engine) in [ComparisonEngine::PhysBox, ComparisonEngine::Box2d]
                    .into_iter()
                    .enumerate()
                {
                    if contains(self.world_engine_button(index), pixel) {
                        return Some(UiHit::Engine(engine));
                    }
                }
                for (index, capacity) in WorldCapacityPreset::ALL.into_iter().enumerate() {
                    if contains(self.world_capacity_button(index), pixel) {
                        return Some(UiHit::WorldCapacity(capacity));
                    }
                }
                if contains(self.world_sleeping_button(), pixel) {
                    return Some(UiHit::ToggleSleeping);
                }
                if contains(self.world_sleep_threshold_button(), pixel) {
                    return Some(UiHit::CycleSleepThreshold);
                }
                if selected_engine == ComparisonEngine::PhysBox
                    && contains(self.world_sleep_grouping_button(), pixel)
                {
                    return Some(UiHit::CycleSleepGrouping);
                }
                if contains(self.world_sleep_visuals_button(), pixel) {
                    return Some(UiHit::ToggleSleepVisuals);
                }
            }
        }
        contains(self.menu_panel(menu_page, solver_profiles.len()), pixel).then_some(UiHit::Panel)
    }

    fn menu_button(self) -> [f32; 4] {
        [
            12.0 * self.scale,
            24.0 * self.scale,
            100.0 * self.scale,
            52.0 * self.scale,
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
        let logical_width = self.target_size[0] as f32 / self.scale;
        let (stride, width) = if logical_width < 580.0 {
            let stride = ((logical_width - 130.0) / WorkbenchTool::ALL.len() as f32).max(1.0);
            (stride, (stride - 6.0).max(1.0))
        } else {
            (76.0, 70.0)
        };
        [
            (118.0 + index as f32 * stride) * self.scale,
            24.0 * self.scale,
            width * self.scale,
            52.0 * self.scale,
        ]
    }

    fn menu_panel(self, page: MenuPage, solver_profile_count: usize) -> [f32; 4] {
        let bottom = match page {
            MenuPage::Scenes => self.restart_y_logical() + 50.0,
            MenuPage::Solvers => {
                let apply = self.apply_solver_button(solver_profile_count);
                (apply[1] + apply[3]) / self.scale
                    + if self.compact_landscape() { 4.0 } else { 8.0 }
            }
            MenuPage::World => {
                let visuals = self.world_sleep_visuals_button();
                (visuals[1] + visuals[3]) / self.scale + 62.0
            }
        };
        let wide = self.compact_landscape()
            || (page == MenuPage::Scenes && self.scenario_columns() > 1)
            || (page == MenuPage::Solvers && self.solver_profile_columns(solver_profile_count) > 1);
        let logical = if wide {
            [12.0, 84.0, 620.0, bottom - 84.0]
        } else {
            [12.0, 84.0, 360.0, bottom - 84.0]
        };
        logical.map(|value| value * self.scale)
    }

    fn menu_tab(self, page: MenuPage) -> [f32; 4] {
        let (x, width) = match page {
            MenuPage::Scenes => (24.0, 104.0),
            MenuPage::Solvers => (136.0, 104.0),
            MenuPage::World => (248.0, 104.0),
        };
        [
            x * self.scale,
            90.0 * self.scale,
            width * self.scale,
            28.0 * self.scale,
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
                (24.0 + column as f32 * stride) * self.scale,
                (126.0 + row as f32 * 38.0) * self.scale,
                width * self.scale,
                34.0 * self.scale,
            ];
        }
        [
            24.0 * self.scale,
            (126.0 + index as f32 * 38.0) * self.scale,
            336.0 * self.scale,
            34.0 * self.scale,
        ]
    }

    fn world_capacity_button(self, index: usize) -> [f32; 4] {
        [
            24.0 * self.scale,
            (154.0 + index as f32 * 38.0) * self.scale,
            336.0 * self.scale,
            34.0 * self.scale,
        ]
    }

    #[cfg(feature = "box2d-comparator")]
    fn world_engine_button(self, index: usize) -> [f32; 4] {
        [
            (112.0 + index as f32 * 122.0) * self.scale,
            122.0 * self.scale,
            114.0 * self.scale,
            26.0 * self.scale,
        ]
    }

    fn world_sleeping_button(self) -> [f32; 4] {
        if self.compact_landscape() {
            return [
                372.0 * self.scale,
                154.0 * self.scale,
                248.0 * self.scale,
                34.0 * self.scale,
            ];
        }
        let last = self.world_capacity_button(WorldCapacityPreset::ALL.len() - 1);
        [
            24.0 * self.scale,
            last[1] + last[3] + 12.0 * self.scale,
            336.0 * self.scale,
            34.0 * self.scale,
        ]
    }

    fn world_sleep_threshold_button(self) -> [f32; 4] {
        let sleeping = self.world_sleeping_button();
        [
            sleeping[0],
            sleeping[1] + sleeping[3] + 12.0 * self.scale,
            sleeping[2],
            sleeping[3],
        ]
    }

    fn world_sleep_grouping_button(self) -> [f32; 4] {
        let threshold = self.world_sleep_threshold_button();
        [
            threshold[0],
            threshold[1] + threshold[3] + 12.0 * self.scale,
            threshold[2],
            threshold[3],
        ]
    }

    fn world_sleep_visuals_button(self) -> [f32; 4] {
        let grouping = self.world_sleep_grouping_button();
        [
            grouping[0],
            grouping[1] + grouping[3] + 12.0 * self.scale,
            grouping[2],
            grouping[3],
        ]
    }

    fn restart_button(self) -> [f32; 4] {
        if self.compact_landscape() {
            return [
                24.0 * self.scale,
                self.restart_y_logical() * self.scale,
                226.0 * self.scale,
                38.0 * self.scale,
            ];
        }
        [
            24.0 * self.scale,
            self.restart_y_logical() * self.scale,
            162.0 * self.scale,
            38.0 * self.scale,
        ]
    }

    fn recenter_button(self) -> [f32; 4] {
        if self.compact_landscape() {
            return [
                262.0 * self.scale,
                self.restart_y_logical() * self.scale,
                226.0 * self.scale,
                38.0 * self.scale,
            ];
        }
        [
            198.0 * self.scale,
            self.restart_y_logical() * self.scale,
            162.0 * self.scale,
            38.0 * self.scale,
        ]
    }

    fn solver_profile_button(self, index: usize, profile_count: usize) -> [f32; 4] {
        let columns = self.solver_profile_columns(profile_count);
        let column = index % columns;
        let row = index / columns;
        let (start_y, stride, height) = self.solver_profile_metrics();
        [
            (24.0 + column as f32 * 298.0) * self.scale,
            (start_y + row as f32 * stride) * self.scale,
            if columns == 1 { 336.0 } else { 286.0 } * self.scale,
            height * self.scale,
        ]
    }

    fn solver_parameter_minus(self, index: usize, profile_count: usize) -> [f32; 4] {
        let (base, stride, height) = self.solver_parameter_metrics(profile_count);
        let (column, row) = self.solver_parameter_grid_position(index);
        [
            (if self.compact_landscape() {
                190.0 + column as f32 * 298.0
            } else {
                238.0
            }) * self.scale,
            (base + row as f32 * stride) * self.scale,
            52.0 * self.scale,
            height * self.scale,
        ]
    }

    fn solver_parameter_plus(self, index: usize, profile_count: usize) -> [f32; 4] {
        let (base, stride, height) = self.solver_parameter_metrics(profile_count);
        let (column, row) = self.solver_parameter_grid_position(index);
        [
            (if self.compact_landscape() {
                244.0 + column as f32 * 298.0
            } else {
                308.0
            }) * self.scale,
            (base + row as f32 * stride) * self.scale,
            52.0 * self.scale,
            height * self.scale,
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
        let y = (base + 8.0 + row as f32 * stride) * self.scale;
        (
            [(28.0 + column_offset) * self.scale, y],
            [
                (if self.compact_landscape() {
                    135.0 + column_offset
                } else {
                    170.0
                }) * self.scale,
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
            (24.0 + index as f32 * 174.0) * self.scale,
            y * self.scale,
            162.0 * self.scale,
            height * self.scale,
        ]
    }

    fn apply_solver_button(self, profile_count: usize) -> [f32; 4] {
        let scene = self.solver_scene_button(0, profile_count);
        let compact = self.compact_landscape();
        [
            198.0 * self.scale,
            scene[1] + scene[3] + if compact { 4.0 } else { 14.0 } * self.scale,
            162.0 * self.scale,
            if compact { 32.0 } else { 38.0 } * self.scale,
        ]
    }

    fn restore_solver_button(self, profile_count: usize) -> [f32; 4] {
        let apply = self.apply_solver_button(profile_count);
        [24.0 * self.scale, apply[1], 162.0 * self.scale, apply[3]]
    }

    fn solver_profile_columns(self, profile_count: usize) -> usize {
        if self.compact_landscape() && profile_count > 4 {
            3
        } else {
            usize::from(
                self.compact_landscape() || (profile_count > 4 && self.target_size[0] >= 720),
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
            } else if dense {
                32.0
            } else {
                38.0
            },
            if compact {
                28.0
            } else if dense {
                30.0
            } else {
                34.0
            },
        )
    }

    fn compact_landscape(self) -> bool {
        self.target_size[1] < 520 && self.target_size[0] > self.target_size[1]
    }

    fn scenario_columns(self) -> usize {
        if self.compact_landscape() {
            3
        } else if self.target_size[0] as f32 / self.scale >= 632.0 {
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
            "solver-minus" => {
                self.solver_parameter_minus(index, SolverProfileConfig::available().len())
            }
            "solver-plus" => {
                self.solver_parameter_plus(index, SolverProfileConfig::available().len())
            }
            "apply-solver" => self.apply_solver_button(SolverProfileConfig::available().len()),
            "restore-solver" => self.restore_solver_button(SolverProfileConfig::available().len()),
            "world-tab" => self.menu_tab(MenuPage::World),
            "world-capacity" => self.world_capacity_button(index),
            "world-sleeping" => self.world_sleeping_button(),
            "world-sleep-threshold" => self.world_sleep_threshold_button(),
            "world-sleep-grouping" => self.world_sleep_grouping_button(),
            "world-sleep-visuals" => self.world_sleep_visuals_button(),
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

fn contains(bounds: [f32; 4], pixel: [f32; 2]) -> bool {
    pixel[0] >= bounds[0]
        && pixel[1] >= bounds[1]
        && pixel[0] <= bounds[0] + bounds[2]
        && pixel[1] <= bounds[1] + bounds[3]
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
struct UiParams {
    viewport_padding: [f32; 4],
}

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
struct UiInstance {
    rect: [f32; 4],
    uv: [f32; 4],
    color: [f32; 4],
    mode: [f32; 4],
}

fn instance_layout() -> wgpu::VertexBufferLayout<'static> {
    const ATTRIBUTES: [wgpu::VertexAttribute; 4] = [
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 0,
            shader_location: 0,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 16,
            shader_location: 1,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 32,
            shader_location: 2,
        },
        wgpu::VertexAttribute {
            format: wgpu::VertexFormat::Float32x4,
            offset: 48,
            shader_location: 3,
        },
    ];
    wgpu::VertexBufferLayout {
        array_stride: std::mem::size_of::<UiInstance>() as u64,
        step_mode: wgpu::VertexStepMode::Instance,
        attributes: &ATTRIBUTES,
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
struct StaticKey {
    target_size: [u32; 2],
    scenario: ValidationScenario,
    menu_open: bool,
    menu_page: MenuPage,
    selected_tool: WorkbenchTool,
    solver_profile_key: String,
    active_solver_profile: SolverProfileName,
    solver_profile_count: usize,
    world_capacity: WorldCapacityPreset,
    sleeping_effective: bool,
    sleep_visuals_enabled: bool,
    selected_engine: ComparisonEngine,
}

struct UiRenderer {
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    params: wgpu::Buffer,
    static_instances: wgpu::Buffer,
    dynamic_instances: wgpu::Buffer,
    last_target_size: Option<[u32; 2]>,
    static_key: Option<StaticKey>,
    static_count: u32,
    static_glyph_count: u32,
    dynamic_count: u32,
    dynamic_glyph_count: u32,
    static_writes_total: u64,
    dynamic_writes_total: u64,
    parameter_writes_total: u64,
    static_upload_bytes_total: u64,
    dynamic_upload_bytes_total: u64,
    parameter_upload_bytes_total: u64,
    overflowed: bool,
}

impl UiRenderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, color_format: wgpu::TextureFormat) -> Self {
        let params = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("physbox5_ui_params"),
            contents: bytemuck::bytes_of(&UiParams::zeroed()),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });
        let atlas = glyph_atlas_r8();
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("physbox5_ui_glyph_atlas"),
            size: wgpu::Extent3d {
                width: glyph_atlas_width(),
                height: glyph_atlas_height(),
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        queue.write_texture(
            wgpu::TexelCopyTextureInfo {
                texture: &texture,
                mip_level: 0,
                origin: Default::default(),
                aspect: wgpu::TextureAspect::All,
            },
            &atlas,
            wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(glyph_atlas_width()),
                rows_per_image: Some(glyph_atlas_height()),
            },
            wgpu::Extent3d {
                width: glyph_atlas_width(),
                height: glyph_atlas_height(),
                depth_or_array_layers: 1,
            },
        );
        let texture_view = texture.create_view(&Default::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("physbox5_ui_glyph_sampler"),
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });
        let bind_group_layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("physbox5_ui_bind_group_layout"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                        view_dimension: wgpu::TextureViewDimension::D2,
                        multisampled: false,
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 2,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("physbox5_ui_bind_group"),
            layout: &bind_group_layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: params.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(&texture_view),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("physbox5_ui_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("ui.wgsl").into()),
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("physbox5_ui_pipeline_layout"),
            bind_group_layouts: &[&bind_group_layout],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("physbox5_ui_pipeline"),
            layout: Some(&pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vertex_main"),
                buffers: &[instance_layout()],
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
        let static_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_ui_static_instances"),
            size: (STATIC_INSTANCE_CAPACITY * std::mem::size_of::<UiInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let dynamic_instances = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("physbox5_ui_dynamic_instances"),
            size: (DYNAMIC_INSTANCE_CAPACITY * std::mem::size_of::<UiInstance>()) as u64,
            usage: wgpu::BufferUsages::VERTEX | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        Self {
            pipeline,
            bind_group,
            params,
            static_instances,
            dynamic_instances,
            last_target_size: None,
            static_key: None,
            static_count: 0,
            static_glyph_count: 0,
            dynamic_count: 0,
            dynamic_glyph_count: 0,
            static_writes_total: 0,
            dynamic_writes_total: 0,
            parameter_writes_total: 0,
            static_upload_bytes_total: 0,
            dynamic_upload_bytes_total: 0,
            parameter_upload_bytes_total: 0,
            overflowed: false,
        }
    }

    fn encode(
        &mut self,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        target: RenderFrameTarget<'_>,
        model: UiFrameModel<'_>,
        view: UiViewState<'_>,
    ) -> UiReport {
        let UiViewState {
            menu_open,
            menu_page,
            selected_tool,
            sleep_visuals_enabled,
            solver_profiles,
            draft_solver_profile,
            selected_engine,
        } = view;
        if self.last_target_size != Some(target.size) {
            let params = UiParams {
                viewport_padding: [target.size[0] as f32, target.size[1] as f32, 0.0, 0.0],
            };
            let bytes = bytemuck::bytes_of(&params);
            queue.write_buffer(&self.params, 0, bytes);
            self.parameter_writes_total = self.parameter_writes_total.saturating_add(1);
            self.parameter_upload_bytes_total = self
                .parameter_upload_bytes_total
                .saturating_add(bytes.len() as u64);
            self.last_target_size = Some(target.size);
        }
        let key = StaticKey {
            target_size: target.size,
            scenario: model.scenario,
            menu_open,
            menu_page,
            selected_tool,
            solver_profile_key: draft_solver_profile.configuration_key(),
            active_solver_profile: model.solver_profile.profile_name(),
            solver_profile_count: solver_profiles.len(),
            world_capacity: model.world_capacity,
            sleeping_effective: model.sleeping_effective,
            sleep_visuals_enabled,
            selected_engine,
        };
        if self.static_key.as_ref() != Some(&key) {
            let batch = static_batch(
                target.size,
                StaticBatchModel {
                    scenario: model.scenario,
                    menu_open,
                    menu_page,
                    selected_tool,
                    active_solver_profile: model.solver_profile,
                    solver_profiles,
                    draft_solver_profile,
                    world_capacity: model.world_capacity,
                    sleeping_effective: model.sleeping_effective,
                    sleep_visuals_enabled,
                    selected_engine,
                },
            );
            if !batch.instances.is_empty() {
                let bytes = bytemuck::cast_slice(&batch.instances);
                queue.write_buffer(&self.static_instances, 0, bytes);
                self.static_upload_bytes_total = self
                    .static_upload_bytes_total
                    .saturating_add(bytes.len() as u64);
            }
            self.static_count = batch.instances.len() as u32;
            self.static_glyph_count = batch.glyph_count;
            self.static_writes_total = self.static_writes_total.saturating_add(1);
            self.overflowed |= batch.overflowed;
            self.static_key = Some(key);
        }
        let dynamic = dynamic_batch(target.size, &model, !menu_open, selected_tool);
        if !dynamic.instances.is_empty() {
            let bytes = bytemuck::cast_slice(&dynamic.instances);
            queue.write_buffer(&self.dynamic_instances, 0, bytes);
            self.dynamic_upload_bytes_total = self
                .dynamic_upload_bytes_total
                .saturating_add(bytes.len() as u64);
        }
        self.dynamic_count = dynamic.instances.len() as u32;
        self.dynamic_glyph_count = dynamic.glyph_count;
        self.dynamic_writes_total = self.dynamic_writes_total.saturating_add(1);
        self.overflowed |= dynamic.overflowed;

        let mut draw_calls = 0;
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("physbox5_ui_render_pass"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: target.color_view,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        if self.static_count > 0 {
            pass.set_vertex_buffer(0, self.static_instances.slice(..));
            pass.draw(0..6, 0..self.static_count);
            draw_calls += 1;
        }
        if self.dynamic_count > 0 {
            pass.set_vertex_buffer(0, self.dynamic_instances.slice(..));
            pass.draw(0..6, 0..self.dynamic_count);
            draw_calls += 1;
        }
        drop(pass);

        UiReport {
            version: UI_CONTRACT_VERSION,
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
            static_instance_capacity: STATIC_INSTANCE_CAPACITY as u32,
            dynamic_instance_capacity: DYNAMIC_INSTANCE_CAPACITY as u32,
            instance_stride_bytes: std::mem::size_of::<UiInstance>() as u32,
            glyph_atlas_bytes: glyph_atlas_bytes(),
            allocated_bytes: glyph_atlas_bytes()
                + std::mem::size_of::<UiParams>() as u64
                + ((STATIC_INSTANCE_CAPACITY + DYNAMIC_INSTANCE_CAPACITY)
                    * std::mem::size_of::<UiInstance>()) as u64,
            static_instances: self.static_count,
            dynamic_instances: self.dynamic_count,
            static_glyphs: self.static_glyph_count,
            dynamic_glyphs: self.dynamic_glyph_count,
            static_writes_total: self.static_writes_total,
            dynamic_writes_total: self.dynamic_writes_total,
            parameter_writes_total: self.parameter_writes_total,
            static_upload_bytes_total: self.static_upload_bytes_total,
            dynamic_upload_bytes_total: self.dynamic_upload_bytes_total,
            parameter_upload_bytes_total: self.parameter_upload_bytes_total,
            render_passes_this_frame: 1,
            draw_calls_this_frame: draw_calls,
            diagnostic_source: model.construction_diagnostic_source,
            diagnostic_age_frames: model.construction_age_frames,
            scenario_activations_total: model.scenario_activations_total,
            scenario_restarts_total: model.scenario_restarts_total,
            overflowed: self.overflowed,
        }
    }
}

struct Batch {
    instances: Vec<UiInstance>,
    capacity: usize,
    glyph_count: u32,
    overflowed: bool,
}

impl Batch {
    fn new(capacity: usize) -> Self {
        Self {
            instances: Vec::with_capacity(capacity),
            capacity,
            glyph_count: 0,
            overflowed: false,
        }
    }

    fn push_quad(&mut self, rect: [f32; 4], color: [f32; 4]) {
        self.push(UiInstance {
            rect,
            uv: [0.0; 4],
            color,
            mode: [0.0; 4],
        });
    }

    fn push_text(&mut self, text: &str, origin: [f32; 2], pixel: f32, color: [f32; 4]) {
        let mut x = origin[0];
        for byte in text.bytes() {
            if byte != b' ' {
                let before = self.instances.len();
                self.push(UiInstance {
                    rect: [
                        x,
                        origin[1],
                        GLYPH_WIDTH as f32 * pixel,
                        GLYPH_HEIGHT as f32 * pixel,
                    ],
                    uv: glyph_uv(byte),
                    color,
                    mode: [1.0, 0.0, 0.0, 0.0],
                });
                if self.instances.len() != before {
                    self.glyph_count = self.glyph_count.saturating_add(1);
                }
            }
            x += GLYPH_CELL_WIDTH as f32 * pixel;
        }
    }

    fn push(&mut self, instance: UiInstance) {
        if self.instances.len() == self.capacity {
            self.overflowed = true;
            return;
        }
        self.instances.push(instance);
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
    sleeping_effective: bool,
    sleep_visuals_enabled: bool,
    selected_engine: ComparisonEngine,
}

fn static_batch(target_size: [u32; 2], model: StaticBatchModel<'_>) -> Batch {
    let StaticBatchModel {
        scenario,
        menu_open,
        menu_page,
        selected_tool,
        active_solver_profile,
        solver_profiles,
        draft_solver_profile,
        world_capacity,
        sleeping_effective,
        sleep_visuals_enabled,
        selected_engine,
    } = model;
    let layout = UiLayout::for_target(target_size);
    let scale = layout.scale;
    let mut batch = Batch::new(STATIC_INSTANCE_CAPACITY);
    let panel = [0.025, 0.045, 0.085, 0.92];
    let button = [0.08, 0.14, 0.22, 0.94];
    let selected = [0.08, 0.38, 0.52, 0.96];
    let text = [0.82, 0.92, 1.0, 1.0];
    let accent = [0.32, 0.86, 1.0, 1.0];
    batch.push_quad(
        layout.menu_button(),
        if menu_open { selected } else { button },
    );
    batch.push_text("MENU", [27.0 * scale, 41.0 * scale], 2.4 * scale, text);
    for tool in available_tools(scenario, selected_engine) {
        let bounds = layout.tool_button(*tool);
        batch.push_quad(
            bounds,
            if *tool == selected_tool {
                selected
            } else {
                button
            },
        );
        let label_scale = (1.8 * scale)
            .min((bounds[2] - 8.0 * scale) / (tool.label().len() as f32 * GLYPH_CELL_WIDTH as f32));
        batch.push_text(
            tool.label(),
            [bounds[0] + 4.0 * scale, bounds[1] + 17.0 * scale],
            label_scale,
            text,
        );
    }
    if menu_open {
        batch.push_quad(layout.menu_panel(menu_page, solver_profiles.len()), panel);
        for page in menu_pages(selected_engine) {
            let bounds = layout.menu_tab(*page);
            batch.push_quad(bounds, if *page == menu_page { selected } else { button });
            batch.push_text(
                match page {
                    MenuPage::Scenes => "SCENES",
                    MenuPage::Solvers => "SOLVERS",
                    MenuPage::World => "WORLD",
                },
                [bounds[0] + 9.0 * scale, bounds[1] + 6.0 * scale],
                1.8 * scale,
                if *page == menu_page { accent } else { text },
            );
        }
        match menu_page {
            MenuPage::Scenes => {
                for (index, candidate) in ValidationScenario::MENU.iter().copied().enumerate() {
                    let bounds = layout.scenario_button(index);
                    batch.push_quad(
                        bounds,
                        if candidate == scenario {
                            selected
                        } else {
                            button
                        },
                    );
                    batch.push_text(
                        candidate.name(),
                        [bounds[0] + 10.0 * scale, bounds[1] + 9.0 * scale],
                        1.8 * scale,
                        text,
                    );
                }
                let restart = layout.restart_button();
                let recenter = layout.recenter_button();
                batch.push_quad(restart, button);
                batch.push_quad(recenter, button);
                batch.push_text(
                    "RESTART",
                    [restart[0] + 11.0 * scale, restart[1] + 10.0 * scale],
                    1.8 * scale,
                    text,
                );
                batch.push_text(
                    "RECENTER",
                    [recenter[0] + 10.0 * scale, recenter[1] + 10.0 * scale],
                    1.8 * scale,
                    text,
                );
            }
            MenuPage::Solvers => {
                let profile_count = solver_profiles.len();
                batch.push_text(
                    &format!(
                        "ACTIVE {} {}",
                        active_solver_profile.label(),
                        if draft_solver_profile.is_preset() {
                            "PRESET"
                        } else {
                            "CUSTOM"
                        }
                    ),
                    [28.0 * scale, 128.0 * scale],
                    1.5 * scale,
                    accent,
                );
                for (index, profile) in solver_profiles.iter().enumerate() {
                    let bounds = layout.solver_profile_button(index, profile_count);
                    batch.push_quad(
                        bounds,
                        if profile.profile_name() == draft_solver_profile.profile_name() {
                            selected
                        } else {
                            button
                        },
                    );
                    batch.push_text(
                        profile.label(),
                        [bounds[0] + 10.0 * scale, bounds[1] + 9.0 * scale],
                        1.8 * scale,
                        text,
                    );
                }
                for (index, parameter) in crate::solver_profile::SolverParameter::ALL
                    .iter()
                    .copied()
                    .enumerate()
                {
                    let (label_origin, value_origin) =
                        layout.solver_parameter_text_origins(index, profile_count);
                    batch.push_text(parameter.label(), label_origin, 1.7 * scale, text);
                    if let Some(value) = draft_solver_profile.parameter_value(parameter) {
                        let value =
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
                        batch.push_text(&value, value_origin, 1.7 * scale, accent);
                        for (bounds, label) in [
                            (layout.solver_parameter_minus(index, profile_count), "-"),
                            (layout.solver_parameter_plus(index, profile_count), "+"),
                        ] {
                            batch.push_quad(bounds, button);
                            batch.push_text(
                                label,
                                [bounds[0] + 19.0 * scale, bounds[1] + 8.0 * scale],
                                2.0 * scale,
                                text,
                            );
                        }
                    }
                }
                #[cfg(feature = "solver-lab")]
                for (index, label) in ["STACK 8", "STACK 16"].iter().enumerate() {
                    let bounds = layout.solver_scene_button(index, profile_count);
                    batch.push_quad(bounds, button);
                    batch.push_text(
                        label,
                        [bounds[0] + 10.0 * scale, bounds[1] + 9.0 * scale],
                        1.7 * scale,
                        text,
                    );
                }
                let apply = layout.apply_solver_button(profile_count);
                let restore = layout.restore_solver_button(profile_count);
                batch.push_quad(restore, button);
                batch.push_text(
                    "RESET PRESET",
                    [restore[0] + 10.0 * scale, restore[1] + 10.0 * scale],
                    1.6 * scale,
                    text,
                );
                batch.push_quad(apply, button);
                batch.push_text(
                    "APPLY",
                    [apply[0] + 10.0 * scale, apply[1] + 10.0 * scale],
                    1.8 * scale,
                    text,
                );
            }
            MenuPage::World => {
                batch.push_text(
                    #[cfg(feature = "box2d-comparator")]
                    "ENGINE",
                    #[cfg(not(feature = "box2d-comparator"))]
                    &format!("ACTIVE {}", world_capacity.label()),
                    [28.0 * scale, 128.0 * scale],
                    1.5 * scale,
                    accent,
                );
                #[cfg(feature = "box2d-comparator")]
                for (index, engine) in [ComparisonEngine::PhysBox, ComparisonEngine::Box2d]
                    .into_iter()
                    .enumerate()
                {
                    let bounds = layout.world_engine_button(index);
                    batch.push_quad(
                        bounds,
                        if engine == selected_engine {
                            selected
                        } else {
                            button
                        },
                    );
                    batch.push_text(
                        engine.label(),
                        [bounds[0] + 8.0 * scale, bounds[1] + 6.0 * scale],
                        1.45 * scale,
                        if engine == selected_engine {
                            accent
                        } else {
                            text
                        },
                    );
                }
                for (index, capacity) in WorldCapacityPreset::ALL.into_iter().enumerate() {
                    let bounds = layout.world_capacity_button(index);
                    batch.push_quad(
                        bounds,
                        if capacity == world_capacity {
                            selected
                        } else {
                            button
                        },
                    );
                    batch.push_text(
                        capacity.label(),
                        [bounds[0] + 10.0 * scale, bounds[1] + 9.0 * scale],
                        1.8 * scale,
                        text,
                    );
                }
                let sleeping = layout.world_sleeping_button();
                let sleeping_requested = draft_solver_profile.physics().sleeping_enabled;
                batch.push_quad(sleeping, if sleeping_requested { selected } else { button });
                batch.push_text(
                    match (sleeping_requested, sleeping_effective) {
                        (true, true) => "SLEEP REQUEST ON  EFFECTIVE ON",
                        (true, false) => "SLEEP REQUEST ON  EFFECTIVE OFF",
                        (false, _) => "SLEEP REQUEST OFF EFFECTIVE OFF",
                    },
                    [sleeping[0] + 9.0 * scale, sleeping[1] + 10.0 * scale],
                    1.35 * scale,
                    if sleeping_requested { accent } else { text },
                );
                let sleep_threshold = layout.world_sleep_threshold_button();
                batch.push_quad(sleep_threshold, button);
                batch.push_text(
                    &format!(
                        "SLEEP THRESHOLD {:.3} M/S",
                        draft_solver_profile.physics().sleep_speed_threshold
                    ),
                    [
                        sleep_threshold[0] + 9.0 * scale,
                        sleep_threshold[1] + 10.0 * scale,
                    ],
                    1.35 * scale,
                    text,
                );
                if selected_engine == ComparisonEngine::PhysBox {
                    let sleep_grouping = layout.world_sleep_grouping_button();
                    batch.push_quad(sleep_grouping, button);
                    batch.push_text(
                        &format!(
                            "SLEEP GROUPING {}",
                            draft_solver_profile.physics().sleep_grouping.label()
                        ),
                        [
                            sleep_grouping[0] + 9.0 * scale,
                            sleep_grouping[1] + 10.0 * scale,
                        ],
                        1.35 * scale,
                        text,
                    );
                }
                let visuals = layout.world_sleep_visuals_button();
                batch.push_text(
                    "HIGH TIERS MAY RUN SLOW",
                    [28.0 * scale, visuals[1] + visuals[3] + 34.0 * scale],
                    1.35 * scale,
                    text,
                );
                batch.push_quad(
                    visuals,
                    if sleep_visuals_enabled {
                        selected
                    } else {
                        button
                    },
                );
                batch.push_text(
                    if sleep_visuals_enabled {
                        "SLEEP VISUALS ON"
                    } else {
                        "SLEEP VISUALS OFF"
                    },
                    [visuals[0] + 9.0 * scale, visuals[1] + 10.0 * scale],
                    1.55 * scale,
                    if sleep_visuals_enabled { accent } else { text },
                );
                batch.push_text(
                    "TEAL ACTIVE  AMBER QUIET  BLUE Z  PINK !",
                    [28.0 * scale, visuals[1] + visuals[3] + 12.0 * scale],
                    1.05 * scale,
                    text,
                );
            }
        }
    }
    batch
}

fn dynamic_batch(
    target_size: [u32; 2],
    model: &UiFrameModel<'_>,
    diagnostics_visible: bool,
    selected_tool: WorkbenchTool,
) -> Batch {
    let layout = UiLayout::for_target(target_size);
    let scale = layout.scale;
    let mut batch = Batch::new(DYNAMIC_INSTANCE_CAPACITY);
    if !diagnostics_visible {
        return batch;
    }
    let width = 310.0 * scale;
    let height = if model.scenario == ValidationScenario::LiquidSandbox
        && model.selected_engine == ComparisonEngine::PhysBox
    {
        410.0 * scale
    } else if model.selected_engine != ComparisonEngine::PhysBox {
        380.0 * scale
    } else {
        336.0 * scale
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
    for (index, line) in lines.iter().enumerate() {
        batch.push_text(
            line,
            [
                origin[0] + 10.0 * scale,
                origin[1] + (10.0 + index as f32 * 20.0) * scale,
            ],
            1.7 * scale,
            color,
        );
    }
    batch
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
                layout.world_sleeping_button(),
                layout.world_sleep_threshold_button(),
                layout.world_sleep_grouping_button(),
                layout.world_sleep_visuals_button(),
            ] {
                assert!(bounds[0] >= 0.0 && bounds[1] >= 0.0);
                assert!(bounds[0] + bounds[2] <= target[0] as f32);
                assert!(bounds[1] + bounds[3] <= target[1] as f32);
            }
        }
    }

    #[test]
    fn world_menu_emits_one_explicit_capacity_restart_action() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut captured = BTreeSet::new();
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
                captured_pointers: &mut captured,
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
    fn world_menu_can_disable_the_default_sleeping_request_immediately() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut captured = BTreeSet::new();
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
                captured_pointers: &mut captured,
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
        let mut captured = BTreeSet::new();
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
                captured_pointers: &mut captured,
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
                captured_pointers: &mut captured,
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
                captured_pointers: &mut captured,
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
        let mut captured = BTreeSet::new();
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
                captured_pointers: &mut captured,
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
        );
        assert!(routed.sleep_visuals_toggled);
        assert!(routed.action.is_none());
        assert!(draft.physics().sleeping_enabled);
        assert!(!menu_open);
    }

    #[cfg(feature = "box2d-comparator")]
    #[test]
    fn world_menu_switches_engine_and_hides_physbox_solver_page() {
        let target = [960, 600];
        let layout = UiLayout::for_target(target);
        let mut captured = BTreeSet::new();
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
                captured_pointers: &mut captured,
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
    fn batches_are_bounded_and_use_one_instance_per_visible_glyph() {
        let active = SolverProfileConfig::default();
        let profiles = SolverProfileConfig::available();
        let static_closed = static_batch(
            [960, 600],
            StaticBatchModel {
                scenario: ValidationScenario::Bootstrap,
                menu_open: false,
                menu_page: MenuPage::Scenes,
                selected_tool: WorkbenchTool::Grab,
                active_solver_profile: &active,
                solver_profiles: &profiles,
                draft_solver_profile: &active,
                world_capacity: WorldCapacityPreset::Slots64,
                sleeping_effective: false,
                sleep_visuals_enabled: false,
                selected_engine: ComparisonEngine::PhysBox,
            },
        );
        let static_open = static_batch(
            [960, 600],
            StaticBatchModel {
                scenario: ValidationScenario::Bootstrap,
                menu_open: true,
                menu_page: MenuPage::Scenes,
                selected_tool: WorkbenchTool::Rectangle,
                active_solver_profile: &active,
                solver_profiles: &profiles,
                draft_solver_profile: &active,
                world_capacity: WorldCapacityPreset::Slots64,
                sleeping_effective: false,
                sleep_visuals_enabled: false,
                selected_engine: ComparisonEngine::PhysBox,
            },
        );
        let static_world = static_batch(
            [960, 600],
            StaticBatchModel {
                scenario: ValidationScenario::Bootstrap,
                menu_open: true,
                menu_page: MenuPage::World,
                selected_tool: WorkbenchTool::Rectangle,
                active_solver_profile: &active,
                solver_profiles: &profiles,
                draft_solver_profile: &active,
                world_capacity: WorldCapacityPreset::Slots10000,
                sleeping_effective: false,
                sleep_visuals_enabled: true,
                selected_engine: ComparisonEngine::PhysBox,
            },
        );
        assert!(!static_closed.overflowed && !static_open.overflowed && !static_world.overflowed);
        assert!(static_open.instances.len() > static_closed.instances.len());
        assert!(static_open.glyph_count > 0);
        assert!(static_open.instances.len() <= STATIC_INSTANCE_CAPACITY);
        assert!(static_world.instances.len() <= STATIC_INSTANCE_CAPACITY);
    }

    #[cfg(feature = "solver-lab")]
    #[test]
    fn solver_menu_batch_is_bounded() {
        let active = SolverProfileConfig::soft_coupled();
        let profiles = SolverProfileConfig::available();
        let batch = static_batch(
            [960, 600],
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
                selected_engine: ComparisonEngine::PhysBox,
            },
        );
        assert!(!batch.overflowed);
        assert!(batch.instances.len() <= STATIC_INSTANCE_CAPACITY);
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
                layout.solver_parameter_minus(0, profile_count),
                layout.solver_parameter_plus(2, profile_count),
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
        let mut captured = BTreeSet::new();
        let mut menu_open = true;
        let mut menu_page = MenuPage::Solvers;
        let mut selected_tool = WorkbenchTool::Grab;
        let mut profiles = SolverProfileConfig::available();
        let mut draft = SolverProfileConfig::default();
        draft.set_sleeping_enabled(true);

        for (id, position) in [
            (1, layout.center_normalized("solver-profile", 0)),
            (2, layout.center_normalized("solver-plus", 0)),
            (3, layout.center_normalized("solver-plus", 1)),
            (4, layout.center_normalized("solver-plus", 2)),
            (5, layout.center_normalized("solver-plus", 3)),
            (6, layout.center_normalized("solver-plus", 4)),
            (7, layout.center_normalized("solver-plus", 5)),
        ] {
            let routed = route_events(
                UiRouteState {
                    menu_open: &mut menu_open,
                    menu_page: &mut menu_page,
                    selected_tool: &mut selected_tool,
                    solver_profiles: &mut profiles,
                    draft_solver_profile: &mut draft,
                    captured_pointers: &mut captured,
                    scenario: ValidationScenario::Bootstrap,
                    selected_engine: ComparisonEngine::PhysBox,
                },
                &[pointer(
                    id,
                    PointerPhase::Down,
                    PointerButton::Primary,
                    position,
                )],
                target,
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
                captured_pointers: &mut captured,
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
                captured_pointers: &mut captured,
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
        let mut captured = BTreeSet::new();
        captured.insert(99);
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
                captured_pointers: &mut captured,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &events,
            target,
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
        let mut captured = BTreeSet::new();
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
                captured_pointers: &mut captured,
                scenario: ValidationScenario::Bootstrap,
                selected_engine: ComparisonEngine::PhysBox,
            },
            &[
                pointer(7, PointerPhase::Down, PointerButton::Contact, rectangle),
                pointer(7, PointerPhase::Up, PointerButton::Contact, rectangle),
            ],
            target,
        );
        assert_eq!(selected_tool, WorkbenchTool::Rectangle);
        assert_eq!(routed.selected_tool, WorkbenchTool::Rectangle);
        assert!(routed.events.is_empty());
        assert!(captured.is_empty());
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
