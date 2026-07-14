use std::collections::VecDeque;

use serde::Serialize;

pub const FIXED_STEP_HZ: u32 = 60;
pub const FIXED_STEP_SECONDS: f64 = 1.0 / FIXED_STEP_HZ as f64;
pub const MAX_ACTIVE_WALL_SECONDS: f64 = 0.25;
const EFFECTIVE_SCALE_WINDOW_SECONDS: f64 = 1.0;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum InteractiveClockAction {
    #[default]
    Run,
    Pause,
    Resume,
    Rebase,
    SingleStep,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum PacingMode {
    InteractiveSlowClock,
    ExplicitStep,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PacingReport {
    pub mode: PacingMode,
    pub fixed_step_hz: u32,
    pub fixed_dt_seconds: f64,
    pub requested_time_scale: f64,
    pub completed_outer_steps_this_frame: u32,
    pub completed_outer_steps_total: u64,
    pub simulated_seconds_total: f64,
    pub admitted_active_wall_seconds_this_frame: f64,
    pub admitted_active_wall_seconds_total: f64,
    pub overload_slipped_wall_seconds_this_frame: f64,
    pub overload_slipped_wall_seconds_total: f64,
    pub lifecycle_discarded_wall_seconds_this_frame: f64,
    pub lifecycle_discarded_wall_seconds_total: f64,
    pub retained_fraction_seconds: f64,
    pub effective_simulation_scale: f64,
    pub presented_frame_hz: f64,
    pub zero_step_frames_total: u64,
    pub one_step_frames_total: u64,
    pub paused: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct InteractiveClockInput {
    pub elapsed_seconds: f64,
    pub requested_time_scale: f64,
    pub action: InteractiveClockAction,
}

pub(crate) struct SimulationClock {
    retained_fraction_seconds: f64,
    requested_time_scale: f64,
    completed_outer_steps_total: u64,
    simulated_seconds_total: f64,
    admitted_active_wall_seconds_total: f64,
    overload_slipped_wall_seconds_total: f64,
    lifecycle_discarded_wall_seconds_total: f64,
    zero_step_frames_total: u64,
    one_step_frames_total: u64,
    paused: bool,
    scale_window: VecDeque<(f64, f64)>,
    scale_window_wall_seconds: f64,
    scale_window_simulated_seconds: f64,
}

impl Default for SimulationClock {
    fn default() -> Self {
        Self {
            retained_fraction_seconds: 0.0,
            requested_time_scale: 1.0,
            completed_outer_steps_total: 0,
            simulated_seconds_total: 0.0,
            admitted_active_wall_seconds_total: 0.0,
            overload_slipped_wall_seconds_total: 0.0,
            lifecycle_discarded_wall_seconds_total: 0.0,
            zero_step_frames_total: 0,
            one_step_frames_total: 0,
            paused: false,
            scale_window: VecDeque::new(),
            scale_window_wall_seconds: 0.0,
            scale_window_simulated_seconds: 0.0,
        }
    }
}

impl SimulationClock {
    pub(crate) fn admit(&mut self, input: InteractiveClockInput) -> (u32, PacingReport) {
        let elapsed_seconds = finite_nonnegative(input.elapsed_seconds);
        let requested_time_scale = finite_nonnegative(input.requested_time_scale).clamp(0.0, 1.0);
        self.requested_time_scale = requested_time_scale;

        let discarded;
        let mut admitted = 0.0;
        let mut slipped = 0.0;
        let mut steps = 0;

        match input.action {
            InteractiveClockAction::Pause => {
                self.paused = true;
                self.retained_fraction_seconds = 0.0;
                discarded = elapsed_seconds;
            }
            InteractiveClockAction::Resume => {
                self.paused = false;
                self.retained_fraction_seconds = 0.0;
                discarded = elapsed_seconds;
            }
            InteractiveClockAction::Rebase => {
                self.retained_fraction_seconds = 0.0;
                discarded = elapsed_seconds;
            }
            InteractiveClockAction::SingleStep => {
                self.retained_fraction_seconds = 0.0;
                steps = 1;
                discarded = elapsed_seconds;
            }
            InteractiveClockAction::Run if self.paused => {
                discarded = elapsed_seconds;
            }
            InteractiveClockAction::Run => {
                admitted = elapsed_seconds.min(MAX_ACTIVE_WALL_SECONDS);
                discarded = elapsed_seconds - admitted;
                self.retained_fraction_seconds += admitted * requested_time_scale;

                let eligible_steps =
                    (self.retained_fraction_seconds / FIXED_STEP_SECONDS).floor() as u64;
                if eligible_steps > 0 {
                    steps = 1;
                    let slipped_simulated_seconds =
                        (eligible_steps - 1) as f64 * FIXED_STEP_SECONDS;
                    slipped = if requested_time_scale > 0.0 {
                        slipped_simulated_seconds / requested_time_scale
                    } else {
                        0.0
                    };
                    self.retained_fraction_seconds -= eligible_steps as f64 * FIXED_STEP_SECONDS;
                    self.retained_fraction_seconds = self
                        .retained_fraction_seconds
                        .clamp(0.0, FIXED_STEP_SECONDS.next_down());
                }
            }
        }

        self.completed_outer_steps_total += u64::from(steps);
        let simulated_this_frame = f64::from(steps) * FIXED_STEP_SECONDS;
        self.simulated_seconds_total += simulated_this_frame;
        self.admitted_active_wall_seconds_total += admitted;
        self.overload_slipped_wall_seconds_total += slipped;
        self.lifecycle_discarded_wall_seconds_total += discarded;
        match steps {
            0 => self.zero_step_frames_total += 1,
            1 => self.one_step_frames_total += 1,
            _ => unreachable!("interactive admission is structurally zero-or-one"),
        }
        self.push_scale_sample(admitted, simulated_this_frame);

        let effective_simulation_scale = if self.scale_window_wall_seconds > 0.0 {
            self.scale_window_simulated_seconds / self.scale_window_wall_seconds
        } else {
            0.0
        };
        let presented_frame_hz = if admitted > 0.0 { 1.0 / admitted } else { 0.0 };
        (
            steps,
            PacingReport {
                mode: PacingMode::InteractiveSlowClock,
                fixed_step_hz: FIXED_STEP_HZ,
                fixed_dt_seconds: FIXED_STEP_SECONDS,
                requested_time_scale,
                completed_outer_steps_this_frame: steps,
                completed_outer_steps_total: self.completed_outer_steps_total,
                simulated_seconds_total: self.simulated_seconds_total,
                admitted_active_wall_seconds_this_frame: admitted,
                admitted_active_wall_seconds_total: self.admitted_active_wall_seconds_total,
                overload_slipped_wall_seconds_this_frame: slipped,
                overload_slipped_wall_seconds_total: self.overload_slipped_wall_seconds_total,
                lifecycle_discarded_wall_seconds_this_frame: discarded,
                lifecycle_discarded_wall_seconds_total: self.lifecycle_discarded_wall_seconds_total,
                retained_fraction_seconds: self.retained_fraction_seconds,
                effective_simulation_scale,
                presented_frame_hz,
                zero_step_frames_total: self.zero_step_frames_total,
                one_step_frames_total: self.one_step_frames_total,
                paused: self.paused,
            },
        )
    }

    pub(crate) fn admit_exact(&mut self, steps: u32) -> PacingReport {
        self.completed_outer_steps_total += u64::from(steps);
        self.simulated_seconds_total += f64::from(steps) * FIXED_STEP_SECONDS;
        PacingReport {
            mode: PacingMode::ExplicitStep,
            fixed_step_hz: FIXED_STEP_HZ,
            fixed_dt_seconds: FIXED_STEP_SECONDS,
            requested_time_scale: self.requested_time_scale,
            completed_outer_steps_this_frame: steps,
            completed_outer_steps_total: self.completed_outer_steps_total,
            simulated_seconds_total: self.simulated_seconds_total,
            admitted_active_wall_seconds_this_frame: 0.0,
            admitted_active_wall_seconds_total: self.admitted_active_wall_seconds_total,
            overload_slipped_wall_seconds_this_frame: 0.0,
            overload_slipped_wall_seconds_total: self.overload_slipped_wall_seconds_total,
            lifecycle_discarded_wall_seconds_this_frame: 0.0,
            lifecycle_discarded_wall_seconds_total: self.lifecycle_discarded_wall_seconds_total,
            retained_fraction_seconds: self.retained_fraction_seconds,
            effective_simulation_scale: if self.scale_window_wall_seconds > 0.0 {
                self.scale_window_simulated_seconds / self.scale_window_wall_seconds
            } else {
                0.0
            },
            presented_frame_hz: 0.0,
            zero_step_frames_total: self.zero_step_frames_total,
            one_step_frames_total: self.one_step_frames_total,
            paused: self.paused,
        }
    }

    fn push_scale_sample(&mut self, wall_seconds: f64, simulated_seconds: f64) {
        self.scale_window
            .push_back((wall_seconds, simulated_seconds));
        self.scale_window_wall_seconds += wall_seconds;
        self.scale_window_simulated_seconds += simulated_seconds;
        while let Some((front_wall, _)) = self.scale_window.front().copied() {
            if self.scale_window_wall_seconds - front_wall < EFFECTIVE_SCALE_WINDOW_SECONDS {
                break;
            }
            let (removed_wall, removed_simulated) = self.scale_window.pop_front().unwrap();
            self.scale_window_wall_seconds -= removed_wall;
            self.scale_window_simulated_seconds -= removed_simulated;
        }
    }
}

fn finite_nonnegative(value: f64) -> f64 {
    if value.is_finite() && value > 0.0 {
        value
    } else {
        0.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run_sequence(hz: u32, frames: u32) -> PacingReport {
        let mut clock = SimulationClock::default();
        let mut report = None;
        for _ in 0..frames {
            let (steps, next) = clock.admit(InteractiveClockInput {
                elapsed_seconds: 1.0 / f64::from(hz),
                requested_time_scale: 1.0,
                action: InteractiveClockAction::Run,
            });
            assert!(steps <= 1);
            assert!(next.retained_fraction_seconds < FIXED_STEP_SECONDS);
            report = Some(next);
        }
        report.unwrap()
    }

    #[test]
    fn cadence_sequences_never_catch_up() {
        let expected = [(120, 120), (60, 120), (50, 100), (30, 60), (15, 30)];
        for (hz, steps) in expected {
            let report = run_sequence(hz, hz * 2);
            assert_eq!(report.completed_outer_steps_total, steps);
        }
    }

    #[test]
    fn long_stall_slips_whole_step_time_permanently() {
        let mut clock = SimulationClock::default();
        let (_, stalled) = clock.admit(InteractiveClockInput {
            elapsed_seconds: 0.1,
            requested_time_scale: 1.0,
            action: InteractiveClockAction::Run,
        });
        assert_eq!(stalled.completed_outer_steps_this_frame, 1);
        assert!((stalled.overload_slipped_wall_seconds_this_frame - 5.0 / 60.0).abs() < 1e-12);

        let (_, recovered) = clock.admit(InteractiveClockInput {
            elapsed_seconds: 1.0 / 120.0,
            requested_time_scale: 1.0,
            action: InteractiveClockAction::Run,
        });
        assert_eq!(recovered.completed_outer_steps_this_frame, 0);
        assert_eq!(recovered.completed_outer_steps_total, 1);
    }

    #[test]
    fn clamp_discard_is_separate_from_overload_slip() {
        let mut clock = SimulationClock::default();
        let (_, report) = clock.admit(InteractiveClockInput {
            elapsed_seconds: 1.0,
            requested_time_scale: 1.0,
            action: InteractiveClockAction::Run,
        });
        assert_eq!(report.admitted_active_wall_seconds_this_frame, 0.25);
        assert_eq!(report.lifecycle_discarded_wall_seconds_this_frame, 0.75);
        assert_eq!(report.completed_outer_steps_this_frame, 1);
        assert!(report.overload_slipped_wall_seconds_this_frame > 0.0);
    }

    #[test]
    fn requested_half_scale_is_a_ceiling() {
        let mut clock = SimulationClock::default();
        for _ in 0..120 {
            clock.admit(InteractiveClockInput {
                elapsed_seconds: 1.0 / 60.0,
                requested_time_scale: 0.5,
                action: InteractiveClockAction::Run,
            });
        }
        let (_, report) = clock.admit(InteractiveClockInput {
            elapsed_seconds: 0.0,
            requested_time_scale: 0.5,
            action: InteractiveClockAction::Run,
        });
        assert_eq!(report.completed_outer_steps_total, 60);
        assert!(report.effective_simulation_scale <= 0.5 + 1e-12);
    }

    #[test]
    fn pause_single_step_and_resume_create_no_debt() {
        let mut clock = SimulationClock::default();
        let (_, paused) = clock.admit(InteractiveClockInput {
            elapsed_seconds: 0.2,
            requested_time_scale: 1.0,
            action: InteractiveClockAction::Pause,
        });
        assert!(paused.paused);
        assert_eq!(paused.completed_outer_steps_total, 0);

        let (_, single) = clock.admit(InteractiveClockInput {
            elapsed_seconds: 2.0,
            requested_time_scale: 1.0,
            action: InteractiveClockAction::SingleStep,
        });
        assert_eq!(single.completed_outer_steps_total, 1);

        let (_, resumed) = clock.admit(InteractiveClockInput {
            elapsed_seconds: 0.5,
            requested_time_scale: 1.0,
            action: InteractiveClockAction::Resume,
        });
        assert!(!resumed.paused);
        assert_eq!(resumed.completed_outer_steps_total, 1);
        assert_eq!(resumed.retained_fraction_seconds, 0.0);
    }

    #[test]
    fn suspend_resume_rebases_without_a_step() {
        let mut clock = SimulationClock::default();
        clock.admit(InteractiveClockInput {
            elapsed_seconds: 1.0 / 120.0,
            requested_time_scale: 1.0,
            action: InteractiveClockAction::Run,
        });
        let (_, report) = clock.admit(InteractiveClockInput {
            elapsed_seconds: 60.0,
            requested_time_scale: 1.0,
            action: InteractiveClockAction::Rebase,
        });
        assert_eq!(report.completed_outer_steps_this_frame, 0);
        assert_eq!(report.retained_fraction_seconds, 0.0);
        assert_eq!(report.lifecycle_discarded_wall_seconds_this_frame, 60.0);
    }

    #[test]
    fn exact_mode_completes_the_authored_count_without_wall_time() {
        let mut clock = SimulationClock::default();
        let report = clock.admit_exact(600);
        assert_eq!(report.mode, PacingMode::ExplicitStep);
        assert_eq!(report.completed_outer_steps_this_frame, 600);
        assert_eq!(report.completed_outer_steps_total, 600);
        assert_eq!(report.admitted_active_wall_seconds_this_frame, 0.0);
        assert_eq!(report.overload_slipped_wall_seconds_this_frame, 0.0);
        assert!((report.simulated_seconds_total - 10.0).abs() < 1e-12);
    }
}
