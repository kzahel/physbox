use physbox_engine::{GPU_SCHEDULER_WORK_VECTOR_VERSION, GpuSchedulerWorkVectorV1};
use serde::Serialize;

pub const SCHEDULER_BENCHMARK_CONTRACT_VERSION: u32 = 1;
pub const SCHEDULER_SPARSE_BODY_COUNTS: [u32; 3] = [32, 256, 1_024];
pub const SCHEDULER_AUTHORED_CONTACT_COUNTS: [u32; 2] = [1_000, 10_000];

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SchedulerBenchmarkScopeV1 {
    GeneratedCompleteStep,
    AuthoredSolverStep,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerTimingSeriesV1 {
    pub ordinary_gpu_seconds: Option<Vec<f64>>,
    pub host_completion_seconds: Vec<f64>,
    pub cpu_encode_submit_seconds: Vec<f64>,
    pub color_construction_seconds: Option<Vec<f64>>,
    pub color_compaction_seconds: Option<Vec<f64>>,
    pub active_color_solve_seconds: Option<Vec<f64>>,
    pub empty_color_solve_seconds: Option<Vec<f64>>,
    pub other_solver_seconds: Option<Vec<f64>>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerBenchmarkRowV1 {
    pub contract_version: u32,
    pub case_id: String,
    pub scope: SchedulerBenchmarkScopeV1,
    pub source_scenario_hash: String,
    pub body_count: u32,
    pub authored_contact_count: Option<u32>,
    pub warmup_steps: u32,
    pub samples: u32,
    pub work: GpuSchedulerWorkVectorV1,
    pub timing: SchedulerTimingSeriesV1,
    pub dispatches_per_step: u32,
    pub allocated_bytes: u64,
    pub replay_exact: bool,
    pub capacity_accepted: bool,
}

impl SchedulerBenchmarkRowV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.contract_version != SCHEDULER_BENCHMARK_CONTRACT_VERSION {
            return Err("scheduler benchmark contract version differs".to_owned());
        }
        if self.case_id.is_empty()
            || self.source_scenario_hash.len() != 64
            || self.body_count == 0
            || self.warmup_steps == 0
            || self.samples == 0
            || self.dispatches_per_step == 0
            || self.allocated_bytes == 0
        {
            return Err("scheduler benchmark row has incomplete identity or work".to_owned());
        }
        if self.work.version != GPU_SCHEDULER_WORK_VECTOR_VERSION
            || self.work.body_slots != self.body_count
            || self.work.contact_point_slots < self.work.live_contact_points
            || self.work.maximum_colors == 0
            || self.work.active_colors == 0
            || self.work.active_colors > self.work.maximum_colors
            || self.work.color_occupancy.len() != self.work.active_colors as usize
            || self.work.color_units == 0
            || self.work.compacted_color_units != self.work.color_units
            || self.work.assignment_digest.len() != 64
            || self.work.compacted_order_digest.len() != 64
        {
            return Err("scheduler benchmark row has an invalid work vector".to_owned());
        }
        match (self.scope, self.authored_contact_count) {
            (SchedulerBenchmarkScopeV1::GeneratedCompleteStep, None) => {}
            (SchedulerBenchmarkScopeV1::AuthoredSolverStep, Some(contacts))
                if contacts == self.work.live_contact_points => {}
            _ => return Err("scheduler benchmark source scope is inconsistent".to_owned()),
        }
        validate_samples(
            &self.timing.host_completion_seconds,
            self.samples,
            "host completion",
        )?;
        validate_samples(
            &self.timing.cpu_encode_submit_seconds,
            self.samples,
            "CPU encode/submit",
        )?;
        for (name, values) in [
            ("ordinary GPU", self.timing.ordinary_gpu_seconds.as_deref()),
            (
                "color construction",
                self.timing.color_construction_seconds.as_deref(),
            ),
            (
                "color compaction",
                self.timing.color_compaction_seconds.as_deref(),
            ),
            (
                "active color solve",
                self.timing.active_color_solve_seconds.as_deref(),
            ),
            (
                "empty color solve",
                self.timing.empty_color_solve_seconds.as_deref(),
            ),
            ("other solver", self.timing.other_solver_seconds.as_deref()),
        ] {
            if let Some(values) = values {
                validate_samples(values, self.samples, name)?;
            }
        }
        let diagnostic_presence = [
            self.timing.color_construction_seconds.is_some(),
            self.timing.color_compaction_seconds.is_some(),
            self.timing.active_color_solve_seconds.is_some(),
            self.timing.empty_color_solve_seconds.is_some(),
            self.timing.other_solver_seconds.is_some(),
        ];
        if diagnostic_presence.iter().any(|present| *present)
            && !diagnostic_presence.iter().all(|present| *present)
        {
            return Err("scheduler diagnostic timing series are incomplete".to_owned());
        }
        if !self.replay_exact || !self.capacity_accepted {
            return Err("scheduler benchmark row failed replay or capacity".to_owned());
        }
        Ok(())
    }
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SchedulerMarginalSlopeV1 {
    pub contract_version: u32,
    pub scope: SchedulerBenchmarkScopeV1,
    pub from_case_id: String,
    pub to_case_id: String,
    pub added_body_slots: u32,
    pub added_contact_point_slots: u32,
    pub ordinary_gpu_seconds_per_added_body: Option<f64>,
    pub ordinary_gpu_seconds_per_added_contact_slot: Option<f64>,
    pub host_seconds_per_added_body: f64,
    pub color_construction_seconds_per_added_contact_slot: Option<f64>,
    pub color_compaction_seconds_per_added_contact_slot: Option<f64>,
}

pub fn scheduler_marginal_slope(
    from: &SchedulerBenchmarkRowV1,
    to: &SchedulerBenchmarkRowV1,
) -> Result<SchedulerMarginalSlopeV1, String> {
    from.validate()?;
    to.validate()?;
    if from.scope != to.scope {
        return Err("scheduler slope rows have different scopes".to_owned());
    }
    let added_body_slots = to
        .body_count
        .checked_sub(from.body_count)
        .filter(|delta| *delta != 0)
        .ok_or_else(|| "scheduler slope body counts do not increase".to_owned())?;
    let added_contact_point_slots = to
        .work
        .contact_point_slots
        .checked_sub(from.work.contact_point_slots)
        .filter(|delta| *delta != 0)
        .ok_or_else(|| "scheduler slope contact slots do not increase".to_owned())?;
    let per_body = |from: &[f64], to: &[f64]| (p50(to) - p50(from)) / f64::from(added_body_slots);
    let per_contact =
        |from: &[f64], to: &[f64]| (p50(to) - p50(from)) / f64::from(added_contact_point_slots);
    Ok(SchedulerMarginalSlopeV1 {
        contract_version: SCHEDULER_BENCHMARK_CONTRACT_VERSION,
        scope: from.scope,
        from_case_id: from.case_id.clone(),
        to_case_id: to.case_id.clone(),
        added_body_slots,
        added_contact_point_slots,
        ordinary_gpu_seconds_per_added_body: paired_optional(
            &from.timing.ordinary_gpu_seconds,
            &to.timing.ordinary_gpu_seconds,
            per_body,
        )?,
        ordinary_gpu_seconds_per_added_contact_slot: paired_optional(
            &from.timing.ordinary_gpu_seconds,
            &to.timing.ordinary_gpu_seconds,
            per_contact,
        )?,
        host_seconds_per_added_body: per_body(
            &from.timing.host_completion_seconds,
            &to.timing.host_completion_seconds,
        ),
        color_construction_seconds_per_added_contact_slot: paired_optional(
            &from.timing.color_construction_seconds,
            &to.timing.color_construction_seconds,
            per_contact,
        )?,
        color_compaction_seconds_per_added_contact_slot: paired_optional(
            &from.timing.color_compaction_seconds,
            &to.timing.color_compaction_seconds,
            per_contact,
        )?,
    })
}

fn paired_optional(
    from: &Option<Vec<f64>>,
    to: &Option<Vec<f64>>,
    calculate: impl FnOnce(&[f64], &[f64]) -> f64,
) -> Result<Option<f64>, String> {
    match (from, to) {
        (Some(from), Some(to)) => Ok(Some(calculate(from, to))),
        (None, None) => Ok(None),
        _ => Err("scheduler slope timing availability differs".to_owned()),
    }
}

fn validate_samples(values: &[f64], expected: u32, name: &str) -> Result<(), String> {
    if values.len() != expected as usize
        || values
            .iter()
            .any(|value| !value.is_finite() || *value < 0.0)
    {
        return Err(format!("scheduler {name} samples are invalid"));
    }
    Ok(())
}

fn p50(values: &[f64]) -> f64 {
    let mut sorted = values.to_vec();
    sorted.sort_by(f64::total_cmp);
    sorted[(sorted.len() - 1) / 2]
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(case: &str, bodies: u32, points: u32, ordinary: f64) -> SchedulerBenchmarkRowV1 {
        SchedulerBenchmarkRowV1 {
            contract_version: SCHEDULER_BENCHMARK_CONTRACT_VERSION,
            case_id: case.to_owned(),
            scope: SchedulerBenchmarkScopeV1::GeneratedCompleteStep,
            source_scenario_hash: "a".repeat(64),
            body_count: bodies,
            authored_contact_count: None,
            warmup_steps: 1,
            samples: 3,
            work: GpuSchedulerWorkVectorV1 {
                version: GPU_SCHEDULER_WORK_VECTOR_VERSION,
                body_slots: bodies,
                contact_point_slots: points,
                live_contact_points: points,
                color_units: points,
                maximum_colors: 32,
                active_colors: 1,
                color_occupancy: vec![points],
                body_mask_slots_cleared: bodies,
                color_count_slots_cleared: 32,
                assignment_point_slots_visited: points,
                prefix_color_slots_visited: 32,
                compaction_point_slots_visited: points,
                compacted_color_units: points,
                encoded_color_passes: 32,
                active_color_passes: 1,
                empty_indirect_launches: 31,
                scheduled_color_unit_invocations: 64,
                assignment_digest: "b".repeat(64),
                compacted_order_digest: "c".repeat(64),
            },
            timing: SchedulerTimingSeriesV1 {
                ordinary_gpu_seconds: Some(vec![ordinary * 1.1, ordinary, ordinary * 0.9]),
                host_completion_seconds: vec![ordinary * 1.2; 3],
                cpu_encode_submit_seconds: vec![0.001; 3],
                color_construction_seconds: Some(vec![ordinary * 0.4; 3]),
                color_compaction_seconds: Some(vec![ordinary * 0.3; 3]),
                active_color_solve_seconds: Some(vec![ordinary * 0.1; 3]),
                empty_color_solve_seconds: Some(vec![ordinary * 0.05; 3]),
                other_solver_seconds: Some(vec![ordinary * 0.15; 3]),
            },
            dispatches_per_step: 10,
            allocated_bytes: 1_024,
            replay_exact: true,
            capacity_accepted: true,
        }
    }

    #[test]
    fn slope_uses_p50_and_declares_both_denominators() {
        let from = row("generated-256", 256, 512, 0.008);
        let to = row("generated-1024", 1_024, 2_048, 0.017);
        let slope = scheduler_marginal_slope(&from, &to).unwrap();
        assert_eq!(slope.added_body_slots, 768);
        assert_eq!(slope.added_contact_point_slots, 1_536);
        assert!(slope.ordinary_gpu_seconds_per_added_body.unwrap() > 0.0);
        assert!(
            slope
                .color_compaction_seconds_per_added_contact_slot
                .unwrap()
                > 0.0
        );
    }

    #[test]
    fn partial_diagnostic_series_is_rejected() {
        let mut invalid = row("generated-32", 32, 64, 0.006);
        invalid.timing.color_compaction_seconds = None;
        assert!(invalid.validate().is_err());
    }
}
