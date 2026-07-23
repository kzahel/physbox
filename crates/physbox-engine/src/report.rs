use serde::Serialize;

use physbox_contracts::ConstructionStateV1;

use crate::clock::PacingReport;
use crate::{
    GpuColorMaintenanceV1, GpuColorOrderV4, GpuColorScheduleV4, GpuColorUnitV4,
    GpuColoredDispatchV1, GpuContactCoefficientPolicyV1, GpuNormalSolverV4,
    GpuSolverAccumulationV4, GpuSolverExecutionV4, GpuSolverSchedulingPolicyV1,
    SleepCycleMetricsV1,
};

pub const SCENE_CONTRACT_VERSION: u32 = 13;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneContract {
    pub version: u32,
    pub engine: &'static str,
    pub state_owner: &'static str,
    pub body_count: u32,
    pub requested_body_capacity: u32,
    pub body_capacity: u32,
    pub contact_point_count: u32,
    pub solver_dispatches_per_step: u32,
    pub solver_capacity_bytes: u64,
    pub particle_count: u32,
    pub particle_capacity: u32,
    pub particle_dispatches_per_step: u32,
    pub particle_capacity_bytes: u64,
    pub particle_readback_for_frame_advancement: bool,
    pub particle_visible_in_box2d: bool,
    pub particle_box2d_interaction: bool,
    pub particle_rigid_coupling: bool,
    pub particle_coupled_schedule: Option<&'static str>,
    pub particle_body_generation_checked: bool,
    pub capacity_overflowed: bool,
    pub generated_solver_execution: Option<GpuSolverExecutionV4>,
    pub generated_solver_scheduling_policy: Option<GpuSolverSchedulingPolicyV1>,
    pub generated_contact_coefficient_policy: Option<GpuContactCoefficientPolicyV1>,
    pub solver_profile: &'static str,
    pub solver_profile_options: Option<String>,
    pub physics_configuration_version: u32,
    pub physics_configuration_hash: String,
    pub sleep_policy: &'static str,
    pub sleep_policy_version: u32,
    pub sleep_state_layout_version: u32,
    pub sleep_speed_threshold: f32,
    pub sleep_grouping: &'static str,
    pub box2d_sleep_speed_threshold: Option<f32>,
    pub sleep_quiet_outer_steps: u32,
    pub sleep_wake_propagation_rounds: u32,
    pub sleep_impact_wake_speed: f32,
    pub requested_sleeping_enabled: bool,
    pub effective_sleeping_enabled: bool,
    pub awake_compaction_enabled: bool,
    pub fixed_step_hz: u32,
    pub fixed_dt_seconds: f32,
    pub solver_substeps: u32,
    pub solver_biased_iterations: u32,
    pub internal_substep_hz: u32,
    pub requested_contact_hz: f32,
    pub effective_dynamic_contact_hz: f32,
    pub requested_static_contact_hz: f32,
    pub effective_static_contact_hz: f32,
    pub contact_damping_ratio: f32,
    pub speculative_distance: f32,
    pub max_depenetration_velocity: f32,
    pub scenario: &'static str,
    pub scenario_hash: String,
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyCommandReport {
    pub grab_intent_contract_version: u32,
    pub construction_intent_contract_version: u32,
    pub capacity_commands: u32,
    pub capacity_grab_intents: u32,
    pub capacity_construction_intents: u32,
    pub capacity_particle_commands: u32,
    pub grab_state_bytes: u32,
    pub queued_commands_total: u64,
    pub executed_commands_total: u64,
    pub reset_commands_total: u64,
    pub coalesced_targets_total: u64,
    pub queued_construction_intents_total: u64,
    pub executed_construction_intents_total: u64,
    pub queued_particle_commands_total: u64,
    pub executed_particle_commands_total: u64,
    pub overflowed: bool,
    pub grab_pointer_active: bool,
    pub construction_pointer_active: bool,
    pub gpu_selection_readback: bool,
    pub grab_intent_dispatches_this_frame: u32,
    pub grab_constraint_dispatches_this_frame: u32,
    pub construction_intent_dispatches_this_frame: u32,
    pub particle_command_dispatches_this_frame: u32,
    pub grab_tether_prepare_dispatches_this_frame: u32,
    pub grab_tether_draw_calls_this_frame: u32,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CameraReport {
    pub version: u32,
    pub center: [f32; 2],
    pub orthographic_half_height: f32,
    pub pan_changes_total: u64,
    pub zoom_changes_total: u64,
    pub recenter_changes_total: u64,
    pub touch_navigation_active: bool,
    pub secondary_pan_active: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UiReport {
    pub version: u32,
    pub menu_open: bool,
    pub current_tool: &'static str,
    pub current_scene: &'static str,
    pub selected_engine: &'static str,
    pub comparator_available: bool,
    pub box2d_revision: Option<&'static str>,
    pub box2d_completed_steps: Option<u64>,
    pub box2d_live_bodies: Option<u32>,
    pub box2d_pending_actions: Option<u32>,
    pub box2d_resolved_actions: Option<u64>,
    pub box2d_identity_disagreements: Option<u64>,
    pub box2d_action_queue_overflowed: Option<bool>,
    pub box2d_aligned: Option<bool>,
    pub box2d_failure: Option<String>,
    pub sleep_visuals_enabled: bool,
    pub static_instance_capacity: u32,
    pub dynamic_instance_capacity: u32,
    pub instance_stride_bytes: u32,
    pub glyph_atlas_bytes: u64,
    pub allocated_bytes: u64,
    pub static_instances: u32,
    pub dynamic_instances: u32,
    pub static_glyphs: u32,
    pub dynamic_glyphs: u32,
    pub static_writes_total: u64,
    pub dynamic_writes_total: u64,
    pub parameter_writes_total: u64,
    pub static_upload_bytes_total: u64,
    pub dynamic_upload_bytes_total: u64,
    pub parameter_upload_bytes_total: u64,
    pub render_passes_this_frame: u32,
    pub draw_calls_this_frame: u32,
    pub diagnostic_source: &'static str,
    pub diagnostic_age_frames: Option<u64>,
    pub scenario_activations_total: u64,
    pub scenario_restarts_total: u64,
    pub overflowed: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioResult {
    pub name: &'static str,
    pub complete: bool,
    pub passed: bool,
    pub reason: &'static str,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConstructionDiagnosticReport {
    pub source: &'static str,
    pub state: Option<ConstructionStateV1>,
    pub observed_frame_index: Option<u64>,
    pub age_frames: Option<u64>,
    pub blocking_readback_for_advancement: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SleepCycleDiagnosticReport {
    pub source: &'static str,
    pub state: Option<SleepCycleMetricsV1>,
    pub comparison_state: Option<SleepCycleMetricsV1>,
    pub observed_frame_index: Option<u64>,
    pub age_frames: Option<u64>,
    pub blocking_readback_for_advancement: bool,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FrameReport {
    pub frame_index: u64,
    pub fixed_steps_total: u64,
    pub fixed_steps_this_frame: u32,
    pub pointer_events_total: u64,
    pub target_size: [u32; 2],
    pub pacing: PacingReport,
    pub camera: CameraReport,
    pub ui: UiReport,
    pub body_commands: BodyCommandReport,
    pub construction_diagnostic: ConstructionDiagnosticReport,
    pub sleep_cycle_diagnostic: SleepCycleDiagnosticReport,
    pub scene_contract: SceneContract,
    pub scenario: ScenarioResult,
}

impl FrameReport {
    pub fn marker_json(&self) -> String {
        serde_json::to_string(self).expect("frame reports are serializable")
    }

    pub fn compact_marker_json(&self) -> String {
        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct CompactPacing<'a> {
            mode: &'a crate::clock::PacingMode,
            completed_outer_steps_total: u64,
            simulated_seconds_total: f64,
            overload_slipped_wall_seconds_total: f64,
            effective_simulation_scale: f64,
        }

        #[derive(Serialize)]
        struct CompactBodyCommands {
            #[serde(rename = "e")]
            executed_commands_total: u64,
            #[serde(rename = "c")]
            executed_construction_intents_total: u64,
            #[serde(rename = "r")]
            reset_commands_total: u64,
            #[serde(rename = "t")]
            coalesced_targets_total: u64,
            #[serde(rename = "o")]
            overflowed: bool,
            #[serde(rename = "g")]
            grab_pointer_active: bool,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct CompactScenario {
            name: &'static str,
            complete: bool,
            passed: bool,
        }

        #[derive(Serialize)]
        struct CompactSceneContract<'a> {
            #[serde(rename = "v")]
            version: u32,
            #[serde(rename = "b")]
            body_count: u32,
            #[serde(rename = "a")]
            requested_body_capacity: u32,
            #[serde(rename = "z")]
            body_capacity: u32,
            #[serde(rename = "c")]
            contact_point_count: u32,
            #[serde(rename = "d")]
            solver_dispatches_per_step: u32,
            #[serde(rename = "m")]
            solver_capacity_bytes: u64,
            #[serde(rename = "o")]
            capacity_overflowed: bool,
            #[serde(rename = "x", skip_serializing_if = "Option::is_none")]
            generated_solver_execution: Option<[u32; 8]>,
            #[serde(rename = "g", skip_serializing_if = "Option::is_none")]
            generated_solver_scheduling_policy: Option<[u32; 3]>,
            #[serde(rename = "q", skip_serializing_if = "Option::is_none")]
            generated_contact_coefficient_policy: Option<u32>,
            #[serde(rename = "p")]
            solver_profile: &'static str,
            #[serde(rename = "r", skip_serializing_if = "Option::is_none")]
            solver_profile_options: Option<&'a str>,
            #[serde(rename = "k")]
            physics_configuration_hash: &'a str,
            #[serde(rename = "w")]
            requested_sleeping_enabled: bool,
            #[serde(rename = "y")]
            effective_sleeping_enabled: bool,
            #[serde(rename = "t")]
            sleep_speed_threshold: f32,
            #[serde(rename = "j")]
            sleep_grouping: u32,
            #[serde(rename = "n")]
            scenario: &'static str,
            #[serde(rename = "h")]
            scenario_hash: &'a str,
        }

        #[derive(Serialize)]
        struct CompactConstructionDiagnostic<'a> {
            #[serde(rename = "a")]
            attempted_intents: u64,
            #[serde(rename = "l")]
            live_bodies: u32,
            #[serde(rename = "r")]
            last_result: &'a physbox_contracts::ConstructionResultV1,
            #[serde(rename = "x")]
            age_frames: Option<u64>,
            #[serde(rename = "b")]
            blocking: bool,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct CompactFrameReport<'a> {
            pointer_events_total: u64,
            pacing: CompactPacing<'a>,
            camera: [f32; 3],
            #[serde(rename = "u")]
            ui: u32,
            body_commands: CompactBodyCommands,
            #[serde(rename = "d", skip_serializing_if = "Option::is_none")]
            construction_diagnostic: Option<CompactConstructionDiagnostic<'a>>,
            #[serde(rename = "b", skip_serializing_if = "Option::is_none")]
            box2d_live_bodies: Option<u32>,
            scene_contract: CompactSceneContract<'a>,
            scenario: CompactScenario,
        }

        serde_json::to_string(&CompactFrameReport {
            pointer_events_total: self.pointer_events_total,
            pacing: CompactPacing {
                mode: &self.pacing.mode,
                completed_outer_steps_total: self.pacing.completed_outer_steps_total,
                simulated_seconds_total: self.pacing.simulated_seconds_total,
                overload_slipped_wall_seconds_total: self
                    .pacing
                    .overload_slipped_wall_seconds_total,
                effective_simulation_scale: self.pacing.effective_simulation_scale,
            },
            camera: [
                self.camera.center[0],
                self.camera.center[1],
                self.camera.orthographic_half_height,
            ],
            ui: self.ui.draw_calls_this_frame
                | ((self.ui.box2d_identity_disagreements.unwrap_or_default() != 0) as u32) << 23
                | ((self.ui.box2d_pending_actions.unwrap_or_default() != 0) as u32) << 24
                | (self.ui.box2d_action_queue_overflowed.unwrap_or_default() as u32) << 25
                | ((self.ui.box2d_failure.is_some()) as u32) << 26
                | (self.ui.box2d_aligned.unwrap_or_default() as u32) << 27
                | (self.ui.comparator_available as u32) << 28
                | ((self.ui.selected_engine == "box2d") as u32) << 29
                | ((self.ui.sleep_visuals_enabled as u32) << 30)
                | ((self.ui.overflowed as u32) << 31),
            body_commands: CompactBodyCommands {
                executed_commands_total: self.body_commands.executed_commands_total,
                executed_construction_intents_total: self
                    .body_commands
                    .executed_construction_intents_total,
                reset_commands_total: self.body_commands.reset_commands_total,
                coalesced_targets_total: self.body_commands.coalesced_targets_total,
                overflowed: self.body_commands.overflowed,
                grab_pointer_active: self.body_commands.grab_pointer_active,
            },
            construction_diagnostic: self.construction_diagnostic.state.as_ref().map(|state| {
                CompactConstructionDiagnostic {
                    attempted_intents: state.attempted_intents,
                    live_bodies: state.live_bodies,
                    last_result: &state.last_result,
                    age_frames: self.construction_diagnostic.age_frames,
                    blocking: self
                        .construction_diagnostic
                        .blocking_readback_for_advancement,
                }
            }),
            box2d_live_bodies: self.ui.box2d_live_bodies,
            scene_contract: CompactSceneContract {
                version: self.scene_contract.version,
                body_count: self.scene_contract.body_count,
                requested_body_capacity: self.scene_contract.requested_body_capacity,
                body_capacity: self.scene_contract.body_capacity,
                contact_point_count: self.scene_contract.contact_point_count,
                solver_dispatches_per_step: self.scene_contract.solver_dispatches_per_step,
                solver_capacity_bytes: self.scene_contract.solver_capacity_bytes,
                capacity_overflowed: self.scene_contract.capacity_overflowed,
                generated_solver_execution: self
                    .scene_contract
                    .generated_solver_execution
                    .map(compact_execution),
                generated_solver_scheduling_policy: self
                    .scene_contract
                    .generated_solver_scheduling_policy
                    .map(compact_scheduling),
                generated_contact_coefficient_policy: self
                    .scene_contract
                    .generated_contact_coefficient_policy
                    .map(compact_contact_coefficients),
                solver_profile: self.scene_contract.solver_profile,
                solver_profile_options: self.scene_contract.solver_profile_options.as_deref(),
                physics_configuration_hash: &self.scene_contract.physics_configuration_hash,
                requested_sleeping_enabled: self.scene_contract.requested_sleeping_enabled,
                effective_sleeping_enabled: self.scene_contract.effective_sleeping_enabled,
                sleep_speed_threshold: self.scene_contract.sleep_speed_threshold,
                sleep_grouping: match self.scene_contract.sleep_grouping {
                    "local-4" => 4,
                    "local-8" => 8,
                    "exact" => 0,
                    _ => u32::MAX,
                },
                scenario: self.scene_contract.scenario,
                scenario_hash: &self.scene_contract.scenario_hash,
            },
            scenario: CompactScenario {
                name: self.scenario.name,
                complete: self.scenario.complete,
                passed: self.scenario.passed,
            },
        })
        .expect("compact frame reports are serializable")
    }
}

const fn compact_execution(execution: GpuSolverExecutionV4) -> [u32; 8] {
    [
        execution.version,
        match execution.accumulation {
            GpuSolverAccumulationV4::JacobiGather => 0,
            GpuSolverAccumulationV4::ColoredInPlace => 1,
        },
        match execution.normal_solver {
            GpuNormalSolverV4::PointJacobi => 0,
            GpuNormalSolverV4::ScalarMassSplit => 1,
            GpuNormalSolverV4::ManifoldBlock => 2,
            GpuNormalSolverV4::BlockMassSplit => 3,
            GpuNormalSolverV4::ManifoldPointSequential => 4,
        },
        match execution.color_schedule {
            GpuColorScheduleV4::None => 0,
            GpuColorScheduleV4::FixedFullGrid => 1,
            GpuColorScheduleV4::CompactIndirect => 2,
        },
        match execution.color_unit {
            GpuColorUnitV4::Point => 0,
            GpuColorUnitV4::Manifold => 1,
        },
        match execution.color_order {
            GpuColorOrderV4::Forward => 0,
            GpuColorOrderV4::Alternating => 1,
            GpuColorOrderV4::AuthorityOverflowFirst => 2,
        },
        execution.warm_start as u32,
        execution.relaxation as u32,
    ]
}

const fn compact_scheduling(policy: GpuSolverSchedulingPolicyV1) -> [u32; 3] {
    [
        policy.version,
        match policy.color_maintenance {
            GpuColorMaintenanceV1::FullRebuild => 0,
            GpuColorMaintenanceV1::StablePairIncremental => 1,
            GpuColorMaintenanceV1::FullRebuildParallelCompaction => 2,
            GpuColorMaintenanceV1::FullRebuildParallelSpeculative => 3,
            GpuColorMaintenanceV1::FullRebuildParallelSpeculativeStaticLate => 4,
            GpuColorMaintenanceV1::AuthoritySourceOrder => 5,
        },
        match policy.colored_dispatch {
            GpuColoredDispatchV1::PerColorIndirect => 0,
            GpuColoredDispatchV1::WorkgroupPerIsland => 1,
            GpuColoredDispatchV1::PerColorIndirectFusedNormalTangent => 2,
            GpuColoredDispatchV1::WorkgroupPerIslandFusedNormalTangent => 3,
        },
    ]
}

const fn compact_contact_coefficients(policy: GpuContactCoefficientPolicyV1) -> u32 {
    match policy {
        GpuContactCoefficientPolicyV1::SettingsDerived => 0,
        GpuContactCoefficientPolicyV1::StaticStiffHard => 1,
        GpuContactCoefficientPolicyV1::AuthorityCappedSoft => 2,
        GpuContactCoefficientPolicyV1::AuthorityTunableSoft => 3,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PacingMode, PacingReport};

    #[test]
    fn compact_marker_fits_android_log_line() {
        let report = FrameReport {
            frame_index: u64::MAX,
            fixed_steps_total: u64::MAX,
            fixed_steps_this_frame: 1,
            pointer_events_total: u64::MAX,
            target_size: [u32::MAX, u32::MAX],
            pacing: PacingReport {
                mode: PacingMode::InteractiveSlowClock,
                fixed_step_hz: 60,
                fixed_dt_seconds: 1.0 / 60.0,
                requested_time_scale: 1.0,
                completed_outer_steps_this_frame: 1,
                completed_outer_steps_total: u64::MAX,
                simulated_seconds_total: f64::MAX,
                admitted_active_wall_seconds_this_frame: f64::MAX,
                admitted_active_wall_seconds_total: f64::MAX,
                overload_slipped_wall_seconds_this_frame: f64::MAX,
                overload_slipped_wall_seconds_total: f64::MAX,
                lifecycle_discarded_wall_seconds_this_frame: f64::MAX,
                lifecycle_discarded_wall_seconds_total: f64::MAX,
                retained_fraction_seconds: 1.0 / 120.0,
                effective_simulation_scale: 1.0,
                presented_frame_hz: 60.0,
                zero_step_frames_total: u64::MAX,
                one_step_frames_total: u64::MAX,
                paused: false,
            },
            camera: CameraReport {
                version: crate::CAMERA_STATE_VERSION,
                center: [f32::MAX, f32::MAX],
                orthographic_half_height: f32::MAX,
                pan_changes_total: u64::MAX,
                zoom_changes_total: u64::MAX,
                recenter_changes_total: u64::MAX,
                touch_navigation_active: true,
                secondary_pan_active: true,
            },
            ui: UiReport {
                version: crate::ui::UI_CONTRACT_VERSION,
                menu_open: true,
                current_tool: "rectangle",
                current_scene: "bootstrap",
                selected_engine: "physbox",
                comparator_available: true,
                box2d_revision: Some("3a4f0da8374af61293a03021c9a0b3ebcfe67948"),
                box2d_completed_steps: Some(u64::MAX),
                box2d_live_bodies: Some(u32::MAX),
                box2d_pending_actions: Some(u32::MAX),
                box2d_resolved_actions: Some(u64::MAX),
                box2d_identity_disagreements: Some(u64::MAX),
                box2d_action_queue_overflowed: Some(true),
                box2d_aligned: Some(true),
                box2d_failure: None,
                sleep_visuals_enabled: true,
                static_instance_capacity: 256,
                dynamic_instance_capacity: 192,
                instance_stride_bytes: 64,
                glyph_atlas_bytes: 3_072,
                allocated_bytes: 31_760,
                static_instances: 256,
                dynamic_instances: 192,
                static_glyphs: 250,
                dynamic_glyphs: 190,
                static_writes_total: u64::MAX,
                dynamic_writes_total: u64::MAX,
                parameter_writes_total: u64::MAX,
                static_upload_bytes_total: u64::MAX,
                dynamic_upload_bytes_total: u64::MAX,
                parameter_upload_bytes_total: u64::MAX,
                render_passes_this_frame: 1,
                draw_calls_this_frame: 2,
                diagnostic_source: "declared",
                diagnostic_age_frames: None,
                scenario_activations_total: u64::MAX,
                scenario_restarts_total: u64::MAX,
                overflowed: false,
            },
            body_commands: BodyCommandReport {
                grab_intent_contract_version: 1,
                construction_intent_contract_version: 1,
                capacity_commands: 64,
                capacity_grab_intents: 64,
                capacity_construction_intents: 64,
                capacity_particle_commands: 64,
                grab_state_bytes: 64,
                queued_commands_total: u64::MAX,
                executed_commands_total: u64::MAX,
                reset_commands_total: u64::MAX,
                coalesced_targets_total: u64::MAX,
                queued_construction_intents_total: u64::MAX,
                executed_construction_intents_total: u64::MAX,
                queued_particle_commands_total: u64::MAX,
                executed_particle_commands_total: u64::MAX,
                overflowed: false,
                grab_pointer_active: false,
                construction_pointer_active: false,
                gpu_selection_readback: false,
                grab_intent_dispatches_this_frame: 1,
                grab_constraint_dispatches_this_frame: 1,
                construction_intent_dispatches_this_frame: 1,
                particle_command_dispatches_this_frame: 1,
                grab_tether_prepare_dispatches_this_frame: 1,
                grab_tether_draw_calls_this_frame: 1,
            },
            construction_diagnostic: ConstructionDiagnosticReport {
                source: "async-gpu",
                state: None,
                observed_frame_index: None,
                age_frames: None,
                blocking_readback_for_advancement: false,
            },
            sleep_cycle_diagnostic: SleepCycleDiagnosticReport {
                source: "asynchronous-gpu",
                state: Some(SleepCycleMetricsV1::initial(1, 1, 30, true)),
                comparison_state: None,
                observed_frame_index: Some(0),
                age_frames: Some(0),
                blocking_readback_for_advancement: false,
            },
            scene_contract: SceneContract {
                version: SCENE_CONTRACT_VERSION,
                engine: "physbox5",
                state_owner: "gpu",
                body_count: u32::MAX,
                requested_body_capacity: 10_000,
                body_capacity: 10_000,
                contact_point_count: 2,
                solver_dispatches_per_step: 68,
                solver_capacity_bytes: 512,
                particle_count: 4_096,
                particle_capacity: 32_768,
                particle_dispatches_per_step: 92,
                particle_capacity_bytes: 2_097_152,
                particle_readback_for_frame_advancement: false,
                particle_visible_in_box2d: false,
                particle_box2d_interaction: false,
                particle_rigid_coupling: true,
                particle_coupled_schedule: Some("rigid-then-coupled-batch"),
                particle_body_generation_checked: true,
                capacity_overflowed: false,
                generated_solver_execution: Some(GpuSolverExecutionV4::SELECTED),
                generated_solver_scheduling_policy: Some(GpuSolverSchedulingPolicyV1::SELECTED),
                generated_contact_coefficient_policy: Some(GpuContactCoefficientPolicyV1::SELECTED),
                solver_profile: "selected",
                solver_profile_options: None,
                physics_configuration_version: physbox_launch::PHYSICS_CONFIGURATION_VERSION,
                physics_configuration_hash: "e".repeat(64),
                sleep_policy: "experimental-s23-v1",
                sleep_policy_version: physbox_contracts::SLEEP_POLICY_VERSION,
                sleep_state_layout_version: physbox_contracts::SLEEP_STATE_LAYOUT_VERSION,
                sleep_speed_threshold: 0.02,
                sleep_grouping: "local-4",
                box2d_sleep_speed_threshold: Some(0.02),
                sleep_quiet_outer_steps: 30,
                sleep_wake_propagation_rounds: 4,
                sleep_impact_wake_speed: 0.05,
                requested_sleeping_enabled: false,
                effective_sleeping_enabled: false,
                awake_compaction_enabled: true,
                fixed_step_hz: 60,
                fixed_dt_seconds: 1.0 / 60.0,
                solver_substeps: 4,
                solver_biased_iterations: 2,
                internal_substep_hz: 240,
                requested_contact_hz: 30.0,
                effective_dynamic_contact_hz: 30.0,
                requested_static_contact_hz: 60.0,
                effective_static_contact_hz: 60.0,
                contact_damping_ratio: 10.0,
                speculative_distance: 0.02,
                max_depenetration_velocity: 3.0,
                scenario: "bootstrap",
                scenario_hash: "f".repeat(64),
            },
            scenario: ScenarioResult {
                name: "bootstrap",
                complete: true,
                passed: true,
                reason: "shared GPU host advanced and rendered",
            },
        };
        let marker = report.compact_marker_json();
        let compact: serde_json::Value = serde_json::from_str(&marker).unwrap();
        assert_eq!(
            compact["u"],
            (1_u32 << 30)
                | (1_u32 << 28)
                | (1_u32 << 27)
                | (1_u32 << 25)
                | (1_u32 << 24)
                | (1_u32 << 23)
                | 2
        );
        assert_eq!(compact["b"], u32::MAX);
        assert_eq!(compact["sceneContract"]["z"], 10_000);
        assert_eq!(compact["sceneContract"]["t"], 0.02);
        assert_eq!(compact["sceneContract"]["j"], 4);
        assert!(
            marker.len() < 900,
            "compact marker is {} bytes",
            marker.len()
        );
        assert!(marker.contains("\"c\":"));
        assert!(marker.contains("\"g\":false"));
    }
}
