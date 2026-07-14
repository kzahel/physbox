use serde::Serialize;

use crate::clock::PacingReport;

pub const SCENE_CONTRACT_VERSION: u32 = 1;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SceneContract {
    pub version: u32,
    pub engine: &'static str,
    pub state_owner: &'static str,
    pub body_count: u32,
    pub fixed_step_hz: u32,
    pub scenario: &'static str,
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
            lifecycle_discarded_wall_seconds_total: f64,
            effective_simulation_scale: f64,
        }

        #[derive(Serialize)]
        #[serde(rename_all = "camelCase")]
        struct CompactFrameReport<'a> {
            frame_index: u64,
            fixed_steps_total: u64,
            fixed_steps_this_frame: u32,
            pointer_events_total: u64,
            target_size: [u32; 2],
            pacing: CompactPacing<'a>,
            scene_contract: &'a SceneContract,
            scenario: &'a ScenarioResult,
        }

        serde_json::to_string(&CompactFrameReport {
            frame_index: self.frame_index,
            fixed_steps_total: self.fixed_steps_total,
            fixed_steps_this_frame: self.fixed_steps_this_frame,
            pointer_events_total: self.pointer_events_total,
            target_size: self.target_size,
            pacing: CompactPacing {
                mode: &self.pacing.mode,
                completed_outer_steps_total: self.pacing.completed_outer_steps_total,
                simulated_seconds_total: self.pacing.simulated_seconds_total,
                overload_slipped_wall_seconds_total: self
                    .pacing
                    .overload_slipped_wall_seconds_total,
                lifecycle_discarded_wall_seconds_total: self
                    .pacing
                    .lifecycle_discarded_wall_seconds_total,
                effective_simulation_scale: self.pacing.effective_simulation_scale,
            },
            scene_contract: &self.scene_contract,
            scenario: &self.scenario,
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
            scene_contract: SceneContract {
                version: 1,
                engine: "physbox5",
                state_owner: "gpu",
                body_count: u32::MAX,
                fixed_step_hz: 60,
                scenario: "bootstrap",
            },
            scenario: ScenarioResult {
                name: "bootstrap",
                complete: true,
                passed: true,
                reason: "shared GPU host advanced and rendered",
            },
        };
        assert!(report.compact_marker_json().len() < 900);
    }
}
