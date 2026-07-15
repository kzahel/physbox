use serde::Serialize;

use crate::clock::PacingReport;

pub const SCENE_CONTRACT_VERSION: u32 = 3;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneContract {
    pub version: u32,
    pub engine: &'static str,
    pub state_owner: &'static str,
    pub body_count: u32,
    pub contact_point_count: u32,
    pub solver_dispatches_per_step: u32,
    pub solver_capacity_bytes: u64,
    pub capacity_overflowed: bool,
    pub fixed_step_hz: u32,
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
    pub grab_state_bytes: u32,
    pub queued_commands_total: u64,
    pub executed_commands_total: u64,
    pub reset_commands_total: u64,
    pub coalesced_targets_total: u64,
    pub queued_construction_intents_total: u64,
    pub executed_construction_intents_total: u64,
    pub overflowed: bool,
    pub grab_pointer_active: bool,
    pub construction_pointer_active: bool,
    pub gpu_selection_readback: bool,
    pub grab_intent_dispatches_this_frame: u32,
    pub grab_constraint_dispatches_this_frame: u32,
    pub construction_intent_dispatches_this_frame: u32,
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
        #[serde(rename_all = "camelCase")]
        struct CompactFrameReport<'a> {
            pointer_events_total: u64,
            pacing: CompactPacing<'a>,
            camera: [f32; 3],
            #[serde(rename = "u")]
            ui: u32,
            body_commands: CompactBodyCommands,
            scene_contract: &'a SceneContract,
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
            ui: self.ui.draw_calls_this_frame | ((self.ui.overflowed as u32) << 31),
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
            scene_contract: &self.scene_contract,
            scenario: CompactScenario {
                name: self.scenario.name,
                complete: self.scenario.complete,
                passed: self.scenario.passed,
            },
        })
        .expect("compact frame reports are serializable")
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
                grab_state_bytes: 64,
                queued_commands_total: u64::MAX,
                executed_commands_total: u64::MAX,
                reset_commands_total: u64::MAX,
                coalesced_targets_total: u64::MAX,
                queued_construction_intents_total: u64::MAX,
                executed_construction_intents_total: u64::MAX,
                overflowed: false,
                grab_pointer_active: false,
                construction_pointer_active: false,
                gpu_selection_readback: false,
                grab_intent_dispatches_this_frame: 1,
                grab_constraint_dispatches_this_frame: 1,
                construction_intent_dispatches_this_frame: 1,
                grab_tether_prepare_dispatches_this_frame: 1,
                grab_tether_draw_calls_this_frame: 1,
            },
            scene_contract: SceneContract {
                version: SCENE_CONTRACT_VERSION,
                engine: "physbox5",
                state_owner: "gpu",
                body_count: u32::MAX,
                contact_point_count: 2,
                solver_dispatches_per_step: 68,
                solver_capacity_bytes: 512,
                capacity_overflowed: false,
                fixed_step_hz: 60,
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
        assert!(
            marker.len() < 900,
            "compact marker is {} bytes",
            marker.len()
        );
        assert!(marker.contains("\"c\":"));
        assert!(marker.contains("\"g\":false"));
    }
}
