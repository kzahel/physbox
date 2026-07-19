use std::collections::BTreeMap;

use physbox_contracts::{BodyStateV1, ScenarioV1, Vec2};
use physbox_engine::{
    GpuColoringEvidenceV4, GpuFixedFloorContactEvidence, GpuGeneratedContactPassMotionWindowV1,
    GpuGeneratedContactPassTraceV3, GpuSolverPassDescriptorV1, GpuSolverPassKindV1,
};
use serde::Serialize;

use crate::scalar::{Box2dPointNormalInput, box2d_point_normal_update};

pub const SETTLING_PASS_ENERGY_REPORT_VERSION: u32 = 1;
pub const SETTLING_PASS_ENERGY_WINDOW_REPORT_VERSION: u32 = 1;
pub const RELAXATION_DERIVATION_REPORT_VERSION: u32 = 1;
pub const STATIC_SUPPORT_ATTRIBUTION_REPORT_VERSION: u32 = 1;
pub const IMPULSE_OPERATOR_REPORT_VERSION: u32 = 1;
pub const SOFT_IMPULSE_OPERATOR_REPORT_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct KineticEnergyAccountingV1 {
    pub ordered_f32: f32,
    pub compensated_f64: f64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ScalarPassCoverageV1 {
    Mirrored,
    PreparedContactBoundary,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalarStateDifferenceV1 {
    pub compared_f32_values: u32,
    pub exact_f32_values: u32,
    pub max_absolute_difference: f32,
    pub max_ulp_difference: u32,
    pub max_ulp_field: Option<&'static str>,
    pub max_ulp_index: Option<u32>,
}

impl ScalarStateDifferenceV1 {
    pub const fn is_exact(self) -> bool {
        self.compared_f32_values == self.exact_f32_values
    }
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingPassEnergyRowV1 {
    pub pass_index: u32,
    pub descriptor: GpuSolverPassDescriptorV1,
    pub coverage: ScalarPassCoverageV1,
    pub gpu_before: KineticEnergyAccountingV1,
    pub gpu_after: KineticEnergyAccountingV1,
    pub gpu_delta_ordered_f32: f32,
    pub gpu_delta_compensated_f64: f64,
    pub scalar_after: KineticEnergyAccountingV1,
    pub scalar_delta_ordered_f32: f32,
    pub scalar_delta_compensated_f64: f64,
    pub state_difference: ScalarStateDifferenceV1,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingPassEnergyReportV1 {
    pub version: u32,
    pub source_scenario_hash: String,
    pub target_step: u32,
    pub adapter: physbox_engine::AdapterDiagnostics,
    pub matched_state_precision: &'static str,
    pub energy_accounting: [&'static str; 2],
    pub rows: Vec<SettlingPassEnergyRowV1>,
    pub first_non_exact_pass: Option<u32>,
    pub maximum_ulp_difference: u32,
    pub maximum_ulp_pass: Option<u32>,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingPassEnergyWindowBoundaryV1 {
    pub completed_step: u32,
    pub pass_index: u32,
    pub descriptor: GpuSolverPassDescriptorV1,
    pub energy_before: KineticEnergyAccountingV1,
    pub energy_after: KineticEnergyAccountingV1,
    pub delta_compensated_f64: f64,
    pub material_positive_threshold: f64,
    pub material_positive: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingPassEnergyWindowStepV1 {
    pub completed_step: u32,
    pub energy_before: KineticEnergyAccountingV1,
    pub energy_after: KineticEnergyAccountingV1,
    pub endpoint_delta_compensated_f64: f64,
    pub pass_delta_sum_compensated_f64: f64,
    pub reconciliation_error_compensated_f64: f64,
    pub reconciliation_tolerance: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingPassEnergyAnchorV1 {
    pub completed_step: u32,
    pub net_delta_compensated_f64: f64,
    pub material_positive_threshold: f64,
    pub materially_positive: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingPassEnergyWindowFamilyV1 {
    pub kind: GpuSolverPassKindV1,
    pub boundary_count: u32,
    pub materially_positive_boundaries: u32,
    pub materially_positive_steps: u32,
    pub net_delta_compensated_f64: f64,
    pub gross_positive_delta_compensated_f64: f64,
    pub material_positive_delta_compensated_f64: f64,
    pub maximum_positive_delta_compensated_f64: f64,
    pub anchors: [SettlingPassEnergyAnchorV1; 3],
    pub window_material_positive_threshold: f64,
    pub consistent_material_injector: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingPassEnergyWindowReportV1 {
    pub version: u32,
    pub source_scenario_hash: String,
    pub first_step: u32,
    pub last_step: u32,
    pub anchor_steps: [u32; 3],
    pub matched_state_precision: &'static str,
    pub energy_accounting: [&'static str; 2],
    pub material_absolute_epsilon: f64,
    pub material_relative_epsilon: f64,
    pub reconciliation_absolute_epsilon: f64,
    pub reconciliation_relative_epsilon: f64,
    pub rows: Vec<SettlingPassEnergyWindowBoundaryV1>,
    pub steps: Vec<SettlingPassEnergyWindowStepV1>,
    pub families: Vec<SettlingPassEnergyWindowFamilyV1>,
    pub window_energy_before: KineticEnergyAccountingV1,
    pub window_energy_after: KineticEnergyAccountingV1,
    pub window_endpoint_delta_compensated_f64: f64,
    pub window_pass_delta_sum_compensated_f64: f64,
    pub window_reconciliation_error_compensated_f64: f64,
    pub window_reconciliation_tolerance: f64,
    pub maximum_step_reconciliation_error_compensated_f64: f64,
    pub cross_step_motion_exact: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum ImpulseOperatorClassV1 {
    Fixed,
    Contraction,
    Oscillation,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpulseVectorMapV1 {
    pub normal_l1_change: f64,
    pub tangent_l1_change: f64,
    pub combined_l2_change: f64,
    pub endpoint_l2_before: f64,
    pub endpoint_l2_after: f64,
    pub normal_total_before: f64,
    pub normal_total_after: f64,
    pub active_set_flips: u32,
    pub redistribution_transition_max: f64,
    pub identity_slot_changes: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalarColorImpulseMapV1 {
    pub color: u32,
    pub manifold_count: u32,
    pub impulse: ImpulseVectorMapV1,
    pub kinetic_energy_delta_compensated_f64: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpulseOperatorPassRowV1 {
    pub pass_index: u32,
    pub descriptor: GpuSolverPassDescriptorV1,
    pub classification: ImpulseOperatorClassV1,
    pub impulse: ImpulseVectorMapV1,
    pub kinetic_energy_delta_compensated_f64: f64,
    pub reset_mode: u32,
    pub exact_first_prepared_reseed: bool,
    pub discarded_prior_impulse_l1: f64,
    pub scalar_colors: Vec<ScalarColorImpulseMapV1>,
    pub scalar_replay_state_difference: Option<ScalarStateDifferenceV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpulseOperatorMechanismV1 {
    pub inter_substep_refreshes: u32,
    pub exact_first_prepared_reseeds: u32,
    pub discarded_prior_impulse_l1: f64,
    pub warm_start_passes: u32,
    pub warm_start_energy_changes: u32,
    pub warm_start_followed_by_nonfixed_biased_normal: u32,
    pub opposing_warm_bias_energy_pairs: u32,
    pub intra_block_active_set_flips: u32,
    pub intra_manifold_redistribution_max: f64,
    pub opposing_color_energy_transitions: u32,
    pub cross_color_normal_load_cancellation: f64,
    pub named_mechanism: &'static str,
    pub correction_must_change: &'static str,
    pub correction_must_preserve: &'static str,
    pub stale_reseed_signature: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImpulseOperatorReportV1 {
    pub version: u32,
    pub source_scenario_hash: String,
    pub target_step: u32,
    pub matched_state_precision: &'static str,
    pub classification_rule: &'static str,
    pub vector_identity_rule: &'static str,
    pub rows: Vec<ImpulseOperatorPassRowV1>,
    pub mechanism: ImpulseOperatorMechanismV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftCrossStepImpulseHandoffV1 {
    pub previous_completed_step: u32,
    pub current_completed_step: u32,
    pub retained_points: u32,
    pub added_points: u32,
    pub lost_points: u32,
    pub exact_impulse_points: u32,
    pub impulse: ImpulseVectorMapV1,
    pub exact_slot_and_impulse_handoff: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftImpulseOperatorFamilyV1 {
    pub kind: GpuSolverPassKindV1,
    pub pass_count: u32,
    pub fixed_maps: u32,
    pub contraction_maps: u32,
    pub oscillation_maps: u32,
    pub kinetic_energy_delta_compensated_f64: f64,
    pub materially_positive: bool,
    pub impulse_normal_l1_change: f64,
    pub impulse_tangent_l1_change: f64,
    pub endpoint_l2_before_total: f64,
    pub endpoint_l2_after_total: f64,
    pub active_set_flips: u32,
    pub redistribution_transition_max: f64,
    pub static_net_delta_compensated_f64: f64,
    pub dynamic_net_delta_compensated_f64: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftNormalCompositionV1 {
    pub outer_step_energy_before_compensated_f64: f64,
    pub outer_step_energy_after_compensated_f64: f64,
    pub outer_step_delta_compensated_f64: f64,
    pub material_positive_threshold: f64,
    pub integrate_velocity_delta_compensated_f64: f64,
    pub warm_start_delta_compensated_f64: f64,
    pub gravity_warm_support_deficit_compensated_f64: f64,
    pub normal_biased_delta_compensated_f64: f64,
    pub normal_relaxation_delta_compensated_f64: f64,
    pub biased_relaxation_net_compensated_f64: f64,
    pub tangent_net_compensated_f64: f64,
    pub inter_substep_refreshes: u32,
    pub exact_first_prepared_reseeds: u32,
    pub discarded_prior_impulse_l1: f64,
    pub normal_biased_materially_positive: bool,
    pub normal_relaxation_materially_positive: bool,
    pub gravity_warm_deficit_materially_positive: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SoftImpulseOperatorReportV1 {
    pub version: u32,
    pub source_scenario_hash: String,
    pub previous_completed_step: u32,
    pub target_step: u32,
    pub matched_state_precision: &'static str,
    pub classification_rule: &'static str,
    pub vector_identity_rule: &'static str,
    pub rows: Vec<ImpulseOperatorPassRowV1>,
    pub families: Vec<SoftImpulseOperatorFamilyV1>,
    pub cross_step_handoff: SoftCrossStepImpulseHandoffV1,
    pub composition: SoftNormalCompositionV1,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StaticSupportEnergyAttributionRowV1 {
    pub pass_index: u32,
    pub descriptor: GpuSolverPassDescriptorV1,
    pub static_manifolds: u32,
    pub dynamic_manifolds: u32,
    pub static_injecting_manifolds: u32,
    pub dynamic_injecting_manifolds: u32,
    pub static_net_delta_compensated_f64: f64,
    pub dynamic_net_delta_compensated_f64: f64,
    pub static_positive_delta_compensated_f64: f64,
    pub dynamic_positive_delta_compensated_f64: f64,
    pub accounting_error_compensated_f64: f64,
    pub replay_state_difference: ScalarStateDifferenceV1,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SettlingStaticSupportAttributionReportV1 {
    pub version: u32,
    pub source_scenario_hash: String,
    pub target_step: u32,
    pub rows: Vec<StaticSupportEnergyAttributionRowV1>,
    pub maximum_accounting_error_compensated_f64: f64,
    pub maximum_replay_ulp_difference: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalarRelaxationSummaryV1 {
    pub energy_before: KineticEnergyAccountingV1,
    pub energy_after: KineticEnergyAccountingV1,
    pub energy_delta_compensated_f64: f64,
    pub maximum_manifold_energy_delta: f64,
    pub energy_injecting_manifolds: u32,
    pub positive_separation_points: u32,
    pub supported_points_before: u32,
    pub supported_points_after: u32,
    pub normal_impulse_before: f64,
    pub normal_impulse_after: f64,
    pub maximum_penetration: f32,
    pub maximum_separation: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalarRelaxationDerivationRowV1 {
    pub pass_index: u32,
    pub descriptor: GpuSolverPassDescriptorV1,
    pub authority_point_sequential: ScalarRelaxationSummaryV1,
    pub coupled_energy_projection: ScalarRelaxationSummaryV1,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScalarRelaxationDerivationReportV1 {
    pub version: u32,
    pub source_scenario_hash: String,
    pub target_step: u32,
    pub authority_revision: &'static str,
    pub authority_contract: &'static str,
    pub candidate_contract: &'static str,
    pub rows: Vec<ScalarRelaxationDerivationRowV1>,
}

impl ScalarRelaxationDerivationReportV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != RELAXATION_DERIVATION_REPORT_VERSION
            || self.source_scenario_hash.len() != 64
            || self.target_step == 0
            || self.authority_revision != "3a4f0da8374af61293a03021c9a0b3ebcfe67948"
            || self.rows.is_empty()
        {
            return Err("scalar relaxation-derivation identity is invalid".to_owned());
        }
        for row in &self.rows {
            if row.descriptor.kind != GpuSolverPassKindV1::NormalRelaxation
                || !row
                    .authority_point_sequential
                    .energy_delta_compensated_f64
                    .is_finite()
                || !row
                    .coupled_energy_projection
                    .energy_delta_compensated_f64
                    .is_finite()
                || row.coupled_energy_projection.energy_injecting_manifolds != 0
                || row.coupled_energy_projection.maximum_manifold_energy_delta > 1.0e-12
                || row.coupled_energy_projection.maximum_penetration
                    != row.authority_point_sequential.maximum_penetration
                || row.coupled_energy_projection.maximum_separation
                    != row.authority_point_sequential.maximum_separation
            {
                return Err("scalar relaxation-derivation row fails its analytic gate".to_owned());
            }
        }
        Ok(())
    }
}

impl SettlingPassEnergyReportV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != SETTLING_PASS_ENERGY_REPORT_VERSION
            || self.source_scenario_hash.len() != 64
            || self.target_step == 0
            || self.matched_state_precision != "f32"
            || self.rows.is_empty()
        {
            return Err("settling pass-energy report identity is invalid".to_owned());
        }
        for (index, row) in self.rows.iter().enumerate() {
            if row.pass_index != index as u32
                || row.state_difference.compared_f32_values == 0
                || !row.gpu_before.ordered_f32.is_finite()
                || !row.gpu_before.compensated_f64.is_finite()
                || !row.gpu_after.ordered_f32.is_finite()
                || !row.gpu_after.compensated_f64.is_finite()
                || !row.scalar_after.ordered_f32.is_finite()
                || !row.scalar_after.compensated_f64.is_finite()
            {
                return Err("settling pass-energy row is invalid".to_owned());
            }
        }
        let first_non_exact = self
            .rows
            .iter()
            .find(|row| !row.state_difference.is_exact())
            .map(|row| row.pass_index);
        let maximum = self
            .rows
            .iter()
            .map(|row| row.state_difference.max_ulp_difference)
            .max()
            .unwrap_or(0);
        let maximum_pass = self
            .rows
            .iter()
            .max_by_key(|row| row.state_difference.max_ulp_difference)
            .filter(|row| row.state_difference.max_ulp_difference != 0)
            .map(|row| row.pass_index);
        if self.first_non_exact_pass != first_non_exact
            || self.maximum_ulp_difference != maximum
            || self.maximum_ulp_pass != maximum_pass
        {
            return Err("settling pass-energy report summary is inconsistent".to_owned());
        }
        Ok(())
    }
}

impl SettlingStaticSupportAttributionReportV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != STATIC_SUPPORT_ATTRIBUTION_REPORT_VERSION
            || self.source_scenario_hash.len() != 64
            || self.target_step == 0
            || self.rows.is_empty()
            || !self.maximum_accounting_error_compensated_f64.is_finite()
        {
            return Err("static-support attribution identity is invalid".to_owned());
        }
        for row in &self.rows {
            if !matches!(
                row.descriptor.kind,
                GpuSolverPassKindV1::WarmStart
                    | GpuSolverPassKindV1::NormalBiased
                    | GpuSolverPassKindV1::NormalRelaxation
            ) || row.static_manifolds + row.dynamic_manifolds == 0
                || !row.static_net_delta_compensated_f64.is_finite()
                || !row.dynamic_net_delta_compensated_f64.is_finite()
                || !row.static_positive_delta_compensated_f64.is_finite()
                || !row.dynamic_positive_delta_compensated_f64.is_finite()
                || !row.accounting_error_compensated_f64.is_finite()
            {
                return Err("static-support attribution row is invalid".to_owned());
            }
        }
        let maximum_error = self
            .rows
            .iter()
            .map(|row| row.accounting_error_compensated_f64.abs())
            .fold(0.0_f64, f64::max);
        let maximum_ulp = self
            .rows
            .iter()
            .map(|row| row.replay_state_difference.max_ulp_difference)
            .max()
            .unwrap_or(0);
        if self.maximum_accounting_error_compensated_f64 != maximum_error
            || self.maximum_replay_ulp_difference != maximum_ulp
        {
            return Err("static-support attribution summary is inconsistent".to_owned());
        }
        Ok(())
    }
}

pub fn analyze_settling_pass_energy(
    scenario: &ScenarioV1,
    trace: &GpuGeneratedContactPassTraceV3,
) -> Result<SettlingPassEnergyReportV1, String> {
    trace.validate()?;
    if scenario.bodies.len() != trace.states[0].bodies.len()
        || trace.descriptors.len() + 1 != trace.states.len()
    {
        return Err("settling pass-energy scenario and trace disagree".to_owned());
    }
    let source_hash =
        physbox_contracts::canonical_hash(scenario).map_err(|error| error.to_string())?;
    if source_hash != trace.source_scenario_hash {
        return Err("settling pass-energy source hash does not match the trace".to_owned());
    }

    let mut rows = Vec::with_capacity(trace.descriptors.len());
    for (pass_index, ((before, after), descriptor)) in trace
        .states
        .windows(2)
        .map(|states| (&states[0], &states[1]))
        .zip(&trace.descriptors)
        .enumerate()
    {
        let gpu_before = kinetic_energy(&before.bodies);
        let gpu_after = kinetic_energy(&after.bodies);
        let (predicted_bodies, predicted_contacts, coverage) = match descriptor.kind {
            GpuSolverPassKindV1::PrepareContacts | GpuSolverPassKindV1::RefreshContacts => (
                before.bodies.clone(),
                after.contacts.clone(),
                ScalarPassCoverageV1::PreparedContactBoundary,
            ),
            _ => {
                let mut bodies = before.bodies.clone();
                let mut contacts = before.contacts.clone();
                replay_pass(&mut bodies, &mut contacts, descriptor, &trace.coloring)?;
                (bodies, contacts, ScalarPassCoverageV1::Mirrored)
            }
        };
        let scalar_after = kinetic_energy(&predicted_bodies);
        let state_difference = compare_state(
            &predicted_bodies,
            &after.bodies,
            &predicted_contacts,
            &after.contacts,
            coverage == ScalarPassCoverageV1::Mirrored,
        )?;
        rows.push(SettlingPassEnergyRowV1 {
            pass_index: pass_index as u32,
            descriptor: *descriptor,
            coverage,
            gpu_before,
            gpu_after,
            gpu_delta_ordered_f32: gpu_after.ordered_f32 - gpu_before.ordered_f32,
            gpu_delta_compensated_f64: gpu_after.compensated_f64 - gpu_before.compensated_f64,
            scalar_after,
            scalar_delta_ordered_f32: scalar_after.ordered_f32 - gpu_before.ordered_f32,
            scalar_delta_compensated_f64: scalar_after.compensated_f64 - gpu_before.compensated_f64,
            state_difference,
        });
    }
    let first_non_exact_pass = rows
        .iter()
        .find(|row| !row.state_difference.is_exact())
        .map(|row| row.pass_index);
    let maximum_ulp_difference = rows
        .iter()
        .map(|row| row.state_difference.max_ulp_difference)
        .max()
        .unwrap_or(0);
    let maximum_ulp_pass = rows
        .iter()
        .max_by_key(|row| row.state_difference.max_ulp_difference)
        .filter(|row| row.state_difference.max_ulp_difference != 0)
        .map(|row| row.pass_index);
    let report = SettlingPassEnergyReportV1 {
        version: SETTLING_PASS_ENERGY_REPORT_VERSION,
        source_scenario_hash: source_hash,
        target_step: trace.target_step,
        adapter: trace.adapter.clone(),
        matched_state_precision: "f32",
        energy_accounting: ["strict-ordered-f32", "compensated-f64-over-f32-state"],
        rows,
        first_non_exact_pass,
        maximum_ulp_difference,
        maximum_ulp_pass,
    };
    report.validate()?;
    Ok(report)
}

const PASS_ENERGY_WINDOW_ANCHORS: [u32; 3] = [900, 1_000, 1_100];
const PASS_ENERGY_WINDOW_MATERIAL_ABSOLUTE_EPSILON: f64 = 1.0e-12;
const PASS_ENERGY_WINDOW_MATERIAL_RELATIVE_EPSILON: f64 = 1.0e-5;
const PASS_ENERGY_WINDOW_RECONCILIATION_ABSOLUTE_EPSILON: f64 = 1.0e-10;
const PASS_ENERGY_WINDOW_RECONCILIATION_RELATIVE_EPSILON: f64 = 1.0e-8;

const PASS_ENERGY_FAMILIES: [GpuSolverPassKindV1; 10] = [
    GpuSolverPassKindV1::IntegrateVelocity,
    GpuSolverPassKindV1::PrepareContacts,
    GpuSolverPassKindV1::RefreshContacts,
    GpuSolverPassKindV1::WarmStart,
    GpuSolverPassKindV1::NormalBiased,
    GpuSolverPassKindV1::TangentBiased,
    GpuSolverPassKindV1::IntegratePosition,
    GpuSolverPassKindV1::NormalRelaxation,
    GpuSolverPassKindV1::TangentRelaxation,
    GpuSolverPassKindV1::Restitution,
];

impl SettlingPassEnergyWindowReportV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != SETTLING_PASS_ENERGY_WINDOW_REPORT_VERSION
            || self.source_scenario_hash.len() != 64
            || self.first_step == 0
            || self.first_step > self.last_step
            || self.anchor_steps != PASS_ENERGY_WINDOW_ANCHORS
            || self.matched_state_precision != "f32"
            || self.material_absolute_epsilon != PASS_ENERGY_WINDOW_MATERIAL_ABSOLUTE_EPSILON
            || self.material_relative_epsilon != PASS_ENERGY_WINDOW_MATERIAL_RELATIVE_EPSILON
            || self.reconciliation_absolute_epsilon
                != PASS_ENERGY_WINDOW_RECONCILIATION_ABSOLUTE_EPSILON
            || self.reconciliation_relative_epsilon
                != PASS_ENERGY_WINDOW_RECONCILIATION_RELATIVE_EPSILON
            || self.rows.is_empty()
            || self.steps.len() != (self.last_step - self.first_step + 1) as usize
            || self.families.len() != PASS_ENERGY_FAMILIES.len()
            || !self.cross_step_motion_exact
        {
            return Err("settling pass-energy window identity is invalid".to_owned());
        }
        if self.steps.iter().any(|step| {
            step.reconciliation_error_compensated_f64.abs() > step.reconciliation_tolerance
        }) || self.window_reconciliation_error_compensated_f64.abs()
            > self.window_reconciliation_tolerance
        {
            return Err("settling pass-energy window does not reconcile".to_owned());
        }
        for (index, step) in self.steps.iter().enumerate() {
            if step.completed_step != self.first_step + index as u32 {
                return Err("settling pass-energy window steps are not contiguous".to_owned());
            }
        }
        for (kind, family) in PASS_ENERGY_FAMILIES.iter().zip(&self.families) {
            if family.kind != *kind
                || family.boundary_count == 0
                || family.anchors.map(|anchor| anchor.completed_step) != PASS_ENERGY_WINDOW_ANCHORS
            {
                return Err("settling pass-energy window family summary is invalid".to_owned());
            }
        }
        Ok(())
    }
}

pub fn analyze_settling_pass_energy_window(
    scenario: &ScenarioV1,
    window: &GpuGeneratedContactPassMotionWindowV1,
) -> Result<SettlingPassEnergyWindowReportV1, String> {
    window.validate()?;
    let source_hash =
        physbox_contracts::canonical_hash(scenario).map_err(|error| error.to_string())?;
    if source_hash != window.source_scenario_hash
        || window.first_step > PASS_ENERGY_WINDOW_ANCHORS[0]
        || window.last_step < PASS_ENERGY_WINDOW_ANCHORS[2]
    {
        return Err("settling pass-energy window source or anchor range changed".to_owned());
    }
    let body_count = scenario.bodies.len();
    if window
        .steps
        .iter()
        .flat_map(|step| &step.states)
        .any(|motions| motions.len() != body_count)
    {
        return Err("settling pass-energy motion/body counts differ".to_owned());
    }

    let mut rows = Vec::with_capacity(window.steps.len() * window.descriptors.len());
    let mut steps = Vec::with_capacity(window.steps.len());
    let mut cross_step_motion_exact = true;
    for (step_index, step) in window.steps.iter().enumerate() {
        if step_index > 0 {
            cross_step_motion_exact &=
                window.steps[step_index - 1].states.last() == step.states.first();
        }
        let energy_before = kinetic_energy_from_motions(&scenario.bodies, &step.states[0])?;
        let energy_after = kinetic_energy_from_motions(
            &scenario.bodies,
            step.states
                .last()
                .ok_or_else(|| "pass-energy window step has no final state".to_owned())?,
        )?;
        let endpoint_delta = energy_after.compensated_f64 - energy_before.compensated_f64;
        let mut pass_delta_sum = 0.0_f64;
        for (pass_index, ((before, after), descriptor)) in step
            .states
            .windows(2)
            .map(|states| (&states[0], &states[1]))
            .zip(&window.descriptors)
            .enumerate()
        {
            let before = kinetic_energy_from_motions(&scenario.bodies, before)?;
            let after = kinetic_energy_from_motions(&scenario.bodies, after)?;
            let delta = after.compensated_f64 - before.compensated_f64;
            let threshold = material_energy_threshold(before, after);
            pass_delta_sum += delta;
            rows.push(SettlingPassEnergyWindowBoundaryV1 {
                completed_step: step.completed_step,
                pass_index: pass_index as u32,
                descriptor: *descriptor,
                energy_before: before,
                energy_after: after,
                delta_compensated_f64: delta,
                material_positive_threshold: threshold,
                material_positive: delta > threshold,
            });
        }
        let reconciliation_error = pass_delta_sum - endpoint_delta;
        let reconciliation_tolerance = reconciliation_tolerance(endpoint_delta);
        steps.push(SettlingPassEnergyWindowStepV1 {
            completed_step: step.completed_step,
            energy_before,
            energy_after,
            endpoint_delta_compensated_f64: endpoint_delta,
            pass_delta_sum_compensated_f64: pass_delta_sum,
            reconciliation_error_compensated_f64: reconciliation_error,
            reconciliation_tolerance,
        });
    }
    let window_energy_before = steps
        .first()
        .ok_or_else(|| "pass-energy window has no first step".to_owned())?
        .energy_before;
    let window_energy_after = steps
        .last()
        .ok_or_else(|| "pass-energy window has no final step".to_owned())?
        .energy_after;
    let window_endpoint_delta =
        window_energy_after.compensated_f64 - window_energy_before.compensated_f64;
    let window_pass_delta_sum = rows
        .iter()
        .map(|row| row.delta_compensated_f64)
        .sum::<f64>();
    let window_reconciliation_error = window_pass_delta_sum - window_endpoint_delta;
    let window_reconciliation_tolerance = reconciliation_tolerance(window_endpoint_delta);
    let maximum_step_reconciliation_error = steps
        .iter()
        .map(|step| step.reconciliation_error_compensated_f64.abs())
        .fold(0.0_f64, f64::max);
    let window_material_threshold =
        material_energy_threshold(window_energy_before, window_energy_after);

    let mut families = Vec::with_capacity(PASS_ENERGY_FAMILIES.len());
    for kind in PASS_ENERGY_FAMILIES {
        let family_rows = rows
            .iter()
            .filter(|row| row.descriptor.kind == kind)
            .collect::<Vec<_>>();
        let mut materially_positive_steps = 0;
        for step in &steps {
            let net = family_rows
                .iter()
                .filter(|row| row.completed_step == step.completed_step)
                .map(|row| row.delta_compensated_f64)
                .sum::<f64>();
            if net > material_energy_threshold(step.energy_before, step.energy_after) {
                materially_positive_steps += 1;
            }
        }
        let anchors = PASS_ENERGY_WINDOW_ANCHORS.map(|completed_step| {
            let step = steps
                .iter()
                .find(|step| step.completed_step == completed_step)
                .expect("validated pass-energy window includes every anchor");
            let net = family_rows
                .iter()
                .filter(|row| row.completed_step == completed_step)
                .map(|row| row.delta_compensated_f64)
                .sum::<f64>();
            let threshold = material_energy_threshold(step.energy_before, step.energy_after);
            SettlingPassEnergyAnchorV1 {
                completed_step,
                net_delta_compensated_f64: net,
                material_positive_threshold: threshold,
                materially_positive: net > threshold,
            }
        });
        let net_delta = family_rows
            .iter()
            .map(|row| row.delta_compensated_f64)
            .sum::<f64>();
        families.push(SettlingPassEnergyWindowFamilyV1 {
            kind,
            boundary_count: family_rows.len() as u32,
            materially_positive_boundaries: family_rows
                .iter()
                .filter(|row| row.material_positive)
                .count() as u32,
            materially_positive_steps,
            net_delta_compensated_f64: net_delta,
            gross_positive_delta_compensated_f64: family_rows
                .iter()
                .map(|row| row.delta_compensated_f64.max(0.0))
                .sum(),
            material_positive_delta_compensated_f64: family_rows
                .iter()
                .filter(|row| row.material_positive)
                .map(|row| row.delta_compensated_f64)
                .sum(),
            maximum_positive_delta_compensated_f64: family_rows
                .iter()
                .map(|row| row.delta_compensated_f64.max(0.0))
                .fold(0.0_f64, f64::max),
            anchors,
            window_material_positive_threshold: window_material_threshold,
            consistent_material_injector: net_delta > window_material_threshold
                && anchors.iter().all(|anchor| anchor.materially_positive),
        });
    }
    let report = SettlingPassEnergyWindowReportV1 {
        version: SETTLING_PASS_ENERGY_WINDOW_REPORT_VERSION,
        source_scenario_hash: source_hash,
        first_step: window.first_step,
        last_step: window.last_step,
        anchor_steps: PASS_ENERGY_WINDOW_ANCHORS,
        matched_state_precision: "f32",
        energy_accounting: ["strict-ordered-f32", "compensated-f64-over-f32-state"],
        material_absolute_epsilon: PASS_ENERGY_WINDOW_MATERIAL_ABSOLUTE_EPSILON,
        material_relative_epsilon: PASS_ENERGY_WINDOW_MATERIAL_RELATIVE_EPSILON,
        reconciliation_absolute_epsilon: PASS_ENERGY_WINDOW_RECONCILIATION_ABSOLUTE_EPSILON,
        reconciliation_relative_epsilon: PASS_ENERGY_WINDOW_RECONCILIATION_RELATIVE_EPSILON,
        rows,
        steps,
        families,
        window_energy_before,
        window_energy_after,
        window_endpoint_delta_compensated_f64: window_endpoint_delta,
        window_pass_delta_sum_compensated_f64: window_pass_delta_sum,
        window_reconciliation_error_compensated_f64: window_reconciliation_error,
        window_reconciliation_tolerance,
        maximum_step_reconciliation_error_compensated_f64: maximum_step_reconciliation_error,
        cross_step_motion_exact,
    };
    report.validate()?;
    Ok(report)
}

fn kinetic_energy_from_motions(
    bodies: &[BodyStateV1],
    motions: &[[f32; 4]],
) -> Result<KineticEnergyAccountingV1, String> {
    if bodies.len() != motions.len() {
        return Err("kinetic-energy motion/body counts differ".to_owned());
    }
    let mut ordered = 0.0_f32;
    let mut compensated = 0.0_f64;
    let mut correction = 0.0_f64;
    for (body, motion) in bodies.iter().zip(motions) {
        let inverse_mass = body.inverse_mass.get();
        if inverse_mass <= 0.0 {
            continue;
        }
        let mass = inverse_mass.recip();
        let inverse_inertia = body.inverse_inertia.get();
        let inertia = if inverse_inertia > 0.0 {
            inverse_inertia.recip()
        } else {
            0.0
        };
        let linear_squared = motion[0] * motion[0] + motion[1] * motion[1];
        let angular_squared = motion[2] * motion[2];
        let term = 0.5_f32 * mass * linear_squared + 0.5_f32 * inertia * angular_squared;
        ordered += term;

        let vx = f64::from(motion[0]);
        let vy = f64::from(motion[1]);
        let angular = f64::from(motion[2]);
        let wider_term = 0.5 * f64::from(mass) * (vx * vx + vy * vy)
            + 0.5 * f64::from(inertia) * angular * angular;
        let adjusted = wider_term - correction;
        let next = compensated + adjusted;
        correction = (next - compensated) - adjusted;
        compensated = next;
    }
    Ok(KineticEnergyAccountingV1 {
        ordered_f32: ordered,
        compensated_f64: compensated,
    })
}

fn material_energy_threshold(
    before: KineticEnergyAccountingV1,
    after: KineticEnergyAccountingV1,
) -> f64 {
    PASS_ENERGY_WINDOW_MATERIAL_ABSOLUTE_EPSILON.max(
        PASS_ENERGY_WINDOW_MATERIAL_RELATIVE_EPSILON
            * before
                .compensated_f64
                .abs()
                .max(after.compensated_f64.abs()),
    )
}

fn reconciliation_tolerance(reference_delta: f64) -> f64 {
    PASS_ENERGY_WINDOW_RECONCILIATION_ABSOLUTE_EPSILON
        + PASS_ENERGY_WINDOW_RECONCILIATION_RELATIVE_EPSILON * reference_delta.abs()
}

impl ImpulseOperatorReportV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != IMPULSE_OPERATOR_REPORT_VERSION
            || self.source_scenario_hash.len() != 64
            || self.target_step == 0
            || self.matched_state_precision != "f32"
            || self.rows.is_empty()
            || self
                .rows
                .iter()
                .enumerate()
                .any(|(index, row)| row.pass_index != index as u32)
            || self.mechanism.inter_substep_refreshes != 3
            || self.mechanism.warm_start_passes != 4
        {
            return Err(
                "impulse-operator report identity or selected schedule is invalid".to_owned(),
            );
        }
        if self.mechanism.stale_reseed_signature
            != (self.mechanism.exact_first_prepared_reseeds == 3
                && self.mechanism.warm_start_followed_by_nonfixed_biased_normal == 4)
        {
            return Err("impulse-operator mechanism summary is inconsistent".to_owned());
        }
        for row in &self.rows {
            let finite = [
                row.impulse.normal_l1_change,
                row.impulse.tangent_l1_change,
                row.impulse.combined_l2_change,
                row.impulse.endpoint_l2_before,
                row.impulse.endpoint_l2_after,
                row.impulse.normal_total_before,
                row.impulse.normal_total_after,
                row.impulse.redistribution_transition_max,
                row.kinetic_energy_delta_compensated_f64,
                row.discarded_prior_impulse_l1,
            ]
            .into_iter()
            .all(f64::is_finite);
            if !finite {
                return Err("impulse-operator row contains a non-finite metric".to_owned());
            }
            let exact_fixed = row.impulse.combined_l2_change == 0.0;
            if (row.classification == ImpulseOperatorClassV1::Fixed) != exact_fixed {
                return Err("impulse-operator fixed classification is inconsistent".to_owned());
            }
        }
        Ok(())
    }
}

/// Replays the selected settled one-step trace through the existing matched
/// `f32` scalar recurrence and exposes the stored impulse map at every GPU
/// pass and scalar color boundary.
pub fn analyze_impulse_operator(
    scenario: &ScenarioV1,
    trace: &GpuGeneratedContactPassTraceV3,
) -> Result<ImpulseOperatorReportV1, String> {
    trace.validate()?;
    let source_hash =
        physbox_contracts::canonical_hash(scenario).map_err(|error| error.to_string())?;
    if source_hash != trace.source_scenario_hash
        || scenario.bodies.len() != trace.states[0].bodies.len()
    {
        return Err("impulse-operator source does not match the pass trace".to_owned());
    }
    let final_contacts = &trace
        .states
        .last()
        .ok_or_else(|| "impulse-operator trace has no final state".to_owned())?
        .contacts;
    let first_prepared = trace
        .descriptors
        .iter()
        .position(|descriptor| descriptor.kind == GpuSolverPassKindV1::PrepareContacts)
        .and_then(|index| trace.states.get(index + 1))
        .ok_or_else(|| "impulse-operator trace omits the first prepared state".to_owned())?;
    let mut rows = Vec::with_capacity(trace.descriptors.len());
    for (pass_index, descriptor) in trace.descriptors.iter().enumerate() {
        let before = &trace.states[pass_index];
        let after = &trace.states[pass_index + 1];
        let impulse = impulse_vector_map(&before.contacts, &after.contacts, final_contacts)?;
        let classification = if impulse.combined_l2_change == 0.0 {
            ImpulseOperatorClassV1::Fixed
        } else if impulse.endpoint_l2_after < impulse.endpoint_l2_before {
            ImpulseOperatorClassV1::Contraction
        } else {
            ImpulseOperatorClassV1::Oscillation
        };
        let reset_mode = if matches!(
            descriptor.kind,
            GpuSolverPassKindV1::PrepareContacts | GpuSolverPassKindV1::RefreshContacts
        ) {
            descriptor.parameters.uints0[3]
        } else {
            0
        };
        let exact_first_prepared_reseed = descriptor.kind == GpuSolverPassKindV1::RefreshContacts
            && reset_mode == 1
            && impulse_vectors_exact(&after.contacts, &first_prepared.contacts)?
            && !impulse_vectors_exact(&before.contacts, &after.contacts)?;
        let discarded_prior_impulse_l1 = if exact_first_prepared_reseed {
            impulse.normal_l1_change + impulse.tangent_l1_change
        } else {
            0.0
        };
        let (scalar_colors, scalar_replay_state_difference) =
            scalar_color_impulse_maps(before, after, descriptor, &trace.coloring, final_contacts)?;
        rows.push(ImpulseOperatorPassRowV1 {
            pass_index: pass_index as u32,
            descriptor: *descriptor,
            classification,
            impulse,
            kinetic_energy_delta_compensated_f64: kinetic_energy(&after.bodies).compensated_f64
                - kinetic_energy(&before.bodies).compensated_f64,
            reset_mode,
            exact_first_prepared_reseed,
            discarded_prior_impulse_l1,
            scalar_colors,
            scalar_replay_state_difference,
        });
    }

    let inter_substep_refreshes = rows
        .iter()
        .filter(|row| {
            row.descriptor.kind == GpuSolverPassKindV1::RefreshContacts && row.reset_mode == 1
        })
        .count() as u32;
    let exact_first_prepared_reseeds = rows
        .iter()
        .filter(|row| row.exact_first_prepared_reseed)
        .count() as u32;
    let discarded_prior_impulse_l1 = rows.iter().map(|row| row.discarded_prior_impulse_l1).sum();
    let warm_start_passes = rows
        .iter()
        .filter(|row| row.descriptor.kind == GpuSolverPassKindV1::WarmStart)
        .count() as u32;
    let warm_start_energy_changes = rows
        .iter()
        .filter(|row| {
            row.descriptor.kind == GpuSolverPassKindV1::WarmStart
                && row.kinetic_energy_delta_compensated_f64 != 0.0
        })
        .count() as u32;
    let mut warm_start_followed_by_nonfixed_biased_normal = 0_u32;
    let mut opposing_warm_bias_energy_pairs = 0_u32;
    for (index, row) in rows.iter().enumerate() {
        if row.descriptor.kind != GpuSolverPassKindV1::WarmStart {
            continue;
        }
        let Some(normal) = rows[index + 1..].iter().find(|candidate| {
            candidate.descriptor.substep == row.descriptor.substep
                && candidate.descriptor.kind == GpuSolverPassKindV1::NormalBiased
        }) else {
            continue;
        };
        if normal.classification != ImpulseOperatorClassV1::Fixed {
            warm_start_followed_by_nonfixed_biased_normal += 1;
        }
        if row.kinetic_energy_delta_compensated_f64 * normal.kinetic_energy_delta_compensated_f64
            < 0.0
        {
            opposing_warm_bias_energy_pairs += 1;
        }
    }
    let intra_block_active_set_flips = rows.iter().map(|row| row.impulse.active_set_flips).sum();
    let intra_manifold_redistribution_max = rows
        .iter()
        .map(|row| row.impulse.redistribution_transition_max)
        .fold(0.0_f64, f64::max);
    let mut opposing_color_energy_transitions = 0_u32;
    let mut cross_color_normal_load_cancellation = 0.0_f64;
    for row in &rows {
        let active = row
            .scalar_colors
            .iter()
            .filter(|color| color.manifold_count != 0)
            .collect::<Vec<_>>();
        opposing_color_energy_transitions += active
            .windows(2)
            .filter(|pair| {
                pair[0].kinetic_energy_delta_compensated_f64
                    * pair[1].kinetic_energy_delta_compensated_f64
                    < 0.0
            })
            .count() as u32;
        let absolute: f64 = active
            .iter()
            .map(|color| {
                (color.impulse.normal_total_after - color.impulse.normal_total_before).abs()
            })
            .sum();
        let net: f64 = active
            .iter()
            .map(|color| color.impulse.normal_total_after - color.impulse.normal_total_before)
            .sum();
        cross_color_normal_load_cancellation += (absolute - net.abs()).max(0.0);
    }
    let stale_reseed_signature =
        exact_first_prepared_reseeds == 3 && warm_start_followed_by_nonfixed_biased_normal == 4;
    let mechanism = ImpulseOperatorMechanismV1 {
        inter_substep_refreshes,
        exact_first_prepared_reseeds,
        discarded_prior_impulse_l1,
        warm_start_passes,
        warm_start_energy_changes,
        warm_start_followed_by_nonfixed_biased_normal,
        opposing_warm_bias_energy_pairs,
        intra_block_active_set_flips,
        intra_manifold_redistribution_max,
        opposing_color_energy_transitions,
        cross_color_normal_load_cancellation,
        named_mechanism: "inter-substep-stale-persistence-reseed",
        correction_must_change: "retain the current within-step accumulated impulses when refreshed geometry and effective masses are prepared at substep boundaries",
        correction_must_preserve: "refreshed anchors and effective masses, unilateral active sets, penetration correction, deterministic color support transfer, and outer-step persistence",
        stale_reseed_signature,
    };
    let report = ImpulseOperatorReportV1 {
        version: IMPULSE_OPERATOR_REPORT_VERSION,
        source_scenario_hash: source_hash,
        target_step: trace.target_step,
        matched_state_precision: "f32",
        classification_rule: "fixed iff bit-exact impulse vector; otherwise contraction iff Euclidean distance to final persisted vector strictly decreases; every other changed map is oscillation",
        vector_identity_rule: "prepared-point capacity slot order; invalid slots are exact zero; normal then tangent component",
        rows,
        mechanism,
    };
    report.validate()?;
    Ok(report)
}

impl SoftImpulseOperatorReportV1 {
    pub fn validate(&self) -> Result<(), String> {
        if self.version != SOFT_IMPULSE_OPERATOR_REPORT_VERSION
            || self.source_scenario_hash.len() != 64
            || self.previous_completed_step == 0
            || self.target_step != self.previous_completed_step + 1
            || self.matched_state_precision != "f32"
            || self.rows.is_empty()
            || self.families.len() != PASS_ENERGY_FAMILIES.len()
            || self.cross_step_handoff.previous_completed_step != self.previous_completed_step
            || self.cross_step_handoff.current_completed_step != self.target_step
        {
            return Err("soft impulse-operator report identity is invalid".to_owned());
        }
        for (kind, family) in PASS_ENERGY_FAMILIES.iter().zip(&self.families) {
            if family.kind != *kind
                || family.pass_count == 0
                || family.fixed_maps + family.contraction_maps + family.oscillation_maps
                    != family.pass_count
            {
                return Err("soft impulse-operator family summary is invalid".to_owned());
            }
        }
        let finite = [
            self.composition.outer_step_energy_before_compensated_f64,
            self.composition.outer_step_energy_after_compensated_f64,
            self.composition.outer_step_delta_compensated_f64,
            self.composition.material_positive_threshold,
            self.composition.integrate_velocity_delta_compensated_f64,
            self.composition.warm_start_delta_compensated_f64,
            self.composition
                .gravity_warm_support_deficit_compensated_f64,
            self.composition.normal_biased_delta_compensated_f64,
            self.composition.normal_relaxation_delta_compensated_f64,
            self.composition.biased_relaxation_net_compensated_f64,
            self.composition.tangent_net_compensated_f64,
            self.composition.discarded_prior_impulse_l1,
        ]
        .into_iter()
        .all(f64::is_finite);
        if !finite {
            return Err("soft impulse-operator composition is non-finite".to_owned());
        }
        Ok(())
    }
}

/// Applies the frozen S10 impulse-map classification to one exact soft step,
/// and adds the outer-step persistence handoff plus scene-local energy
/// composition required by S11.2. The hard S10 mechanism label is
/// deliberately not carried into this report.
pub fn analyze_soft_impulse_operator(
    scenario: &ScenarioV1,
    previous: &GpuGeneratedContactPassTraceV3,
    current: &GpuGeneratedContactPassTraceV3,
) -> Result<SoftImpulseOperatorReportV1, String> {
    previous.validate()?;
    current.validate()?;
    let source_hash =
        physbox_contracts::canonical_hash(scenario).map_err(|error| error.to_string())?;
    if scenario.settings.contact_softness.is_none()
        || source_hash != previous.source_scenario_hash
        || source_hash != current.source_scenario_hash
        || current.target_step != previous.target_step + 1
        || previous.descriptors != current.descriptors
    {
        return Err("soft impulse-operator sources or adjacent prefixes differ".to_owned());
    }
    let operator = analyze_impulse_operator(scenario, current)?;
    let support = analyze_static_support_energy_attribution(current)?;
    let energy_before = kinetic_energy(&current.states[0].bodies);
    let energy_after = kinetic_energy(
        &current
            .states
            .last()
            .ok_or_else(|| "soft impulse-operator trace has no final state".to_owned())?
            .bodies,
    );
    let material_threshold = material_energy_threshold(energy_before, energy_after);

    let mut families = Vec::with_capacity(PASS_ENERGY_FAMILIES.len());
    for kind in PASS_ENERGY_FAMILIES {
        let rows = operator
            .rows
            .iter()
            .filter(|row| row.descriptor.kind == kind)
            .collect::<Vec<_>>();
        let energy_delta = rows
            .iter()
            .map(|row| row.kinetic_energy_delta_compensated_f64)
            .sum::<f64>();
        let support_rows = support
            .rows
            .iter()
            .filter(|row| row.descriptor.kind == kind)
            .collect::<Vec<_>>();
        families.push(SoftImpulseOperatorFamilyV1 {
            kind,
            pass_count: rows.len() as u32,
            fixed_maps: rows
                .iter()
                .filter(|row| row.classification == ImpulseOperatorClassV1::Fixed)
                .count() as u32,
            contraction_maps: rows
                .iter()
                .filter(|row| row.classification == ImpulseOperatorClassV1::Contraction)
                .count() as u32,
            oscillation_maps: rows
                .iter()
                .filter(|row| row.classification == ImpulseOperatorClassV1::Oscillation)
                .count() as u32,
            kinetic_energy_delta_compensated_f64: energy_delta,
            materially_positive: energy_delta > material_threshold,
            impulse_normal_l1_change: rows.iter().map(|row| row.impulse.normal_l1_change).sum(),
            impulse_tangent_l1_change: rows.iter().map(|row| row.impulse.tangent_l1_change).sum(),
            endpoint_l2_before_total: rows.iter().map(|row| row.impulse.endpoint_l2_before).sum(),
            endpoint_l2_after_total: rows.iter().map(|row| row.impulse.endpoint_l2_after).sum(),
            active_set_flips: rows.iter().map(|row| row.impulse.active_set_flips).sum(),
            redistribution_transition_max: rows
                .iter()
                .map(|row| row.impulse.redistribution_transition_max)
                .fold(0.0_f64, f64::max),
            static_net_delta_compensated_f64: support_rows
                .iter()
                .map(|row| row.static_net_delta_compensated_f64)
                .sum(),
            dynamic_net_delta_compensated_f64: support_rows
                .iter()
                .map(|row| row.dynamic_net_delta_compensated_f64)
                .sum(),
        });
    }
    let family_energy = |kind| {
        families
            .iter()
            .find(|family| family.kind == kind)
            .map_or(0.0, |family| family.kinetic_energy_delta_compensated_f64)
    };
    let integrate_velocity = family_energy(GpuSolverPassKindV1::IntegrateVelocity);
    let warm_start = family_energy(GpuSolverPassKindV1::WarmStart);
    let normal_biased = family_energy(GpuSolverPassKindV1::NormalBiased);
    let normal_relaxation = family_energy(GpuSolverPassKindV1::NormalRelaxation);
    let tangent_net = family_energy(GpuSolverPassKindV1::TangentBiased)
        + family_energy(GpuSolverPassKindV1::TangentRelaxation);
    let cross_step_handoff = analyze_soft_cross_step_handoff(previous, current)?;
    let composition = SoftNormalCompositionV1 {
        outer_step_energy_before_compensated_f64: energy_before.compensated_f64,
        outer_step_energy_after_compensated_f64: energy_after.compensated_f64,
        outer_step_delta_compensated_f64: energy_after.compensated_f64
            - energy_before.compensated_f64,
        material_positive_threshold: material_threshold,
        integrate_velocity_delta_compensated_f64: integrate_velocity,
        warm_start_delta_compensated_f64: warm_start,
        gravity_warm_support_deficit_compensated_f64: integrate_velocity + warm_start,
        normal_biased_delta_compensated_f64: normal_biased,
        normal_relaxation_delta_compensated_f64: normal_relaxation,
        biased_relaxation_net_compensated_f64: normal_biased + normal_relaxation,
        tangent_net_compensated_f64: tangent_net,
        inter_substep_refreshes: operator.mechanism.inter_substep_refreshes,
        exact_first_prepared_reseeds: operator.mechanism.exact_first_prepared_reseeds,
        discarded_prior_impulse_l1: operator.mechanism.discarded_prior_impulse_l1,
        normal_biased_materially_positive: normal_biased > material_threshold,
        normal_relaxation_materially_positive: normal_relaxation > material_threshold,
        gravity_warm_deficit_materially_positive: integrate_velocity + warm_start
            > material_threshold,
    };
    let report = SoftImpulseOperatorReportV1 {
        version: SOFT_IMPULSE_OPERATOR_REPORT_VERSION,
        source_scenario_hash: source_hash,
        previous_completed_step: previous.target_step,
        target_step: current.target_step,
        matched_state_precision: "f32",
        classification_rule: operator.classification_rule,
        vector_identity_rule: operator.vector_identity_rule,
        rows: operator.rows,
        families,
        cross_step_handoff,
        composition,
    };
    report.validate()?;
    Ok(report)
}

fn analyze_soft_cross_step_handoff(
    previous: &GpuGeneratedContactPassTraceV3,
    current: &GpuGeneratedContactPassTraceV3,
) -> Result<SoftCrossStepImpulseHandoffV1, String> {
    let before = &previous
        .states
        .last()
        .ok_or_else(|| "soft handoff previous trace has no final state".to_owned())?
        .contacts;
    let after = &current
        .states
        .first()
        .ok_or_else(|| "soft handoff current trace has no initial state".to_owned())?
        .contacts;
    let impulse = impulse_vector_map(before, after, after)?;
    let key = |point: &GpuFixedFloorContactEvidence| {
        (
            point.body_a,
            point.body_b,
            point.manifold_index,
            point.point_in_manifold,
        )
    };
    let previous_points = before
        .iter()
        .filter(|point| point.valid)
        .map(|point| (key(point), point))
        .collect::<BTreeMap<_, _>>();
    let current_points = after
        .iter()
        .filter(|point| point.valid)
        .map(|point| (key(point), point))
        .collect::<BTreeMap<_, _>>();
    let retained_points = previous_points
        .keys()
        .filter(|key| current_points.contains_key(key))
        .count() as u32;
    let exact_impulse_points = previous_points
        .iter()
        .filter(|(key, previous)| {
            current_points.get(key).is_some_and(|current| {
                previous.normal_impulse.to_bits() == current.normal_impulse.to_bits()
                    && previous.tangent_impulse.to_bits() == current.tangent_impulse.to_bits()
            })
        })
        .count() as u32;
    Ok(SoftCrossStepImpulseHandoffV1 {
        previous_completed_step: previous.target_step,
        current_completed_step: current.target_step,
        retained_points,
        added_points: current_points.len() as u32 - retained_points,
        lost_points: previous_points.len() as u32 - retained_points,
        exact_impulse_points,
        impulse,
        exact_slot_and_impulse_handoff: exact_impulse_points == retained_points
            && previous_points.len() == current_points.len()
            && retained_points as usize == previous_points.len(),
    })
}

fn scalar_color_impulse_maps(
    before: &physbox_engine::GpuGeneratedContactPassStateV3,
    after: &physbox_engine::GpuGeneratedContactPassStateV3,
    descriptor: &GpuSolverPassDescriptorV1,
    coloring: &GpuColoringEvidenceV4,
    final_contacts: &[GpuFixedFloorContactEvidence],
) -> Result<
    (
        Vec<ScalarColorImpulseMapV1>,
        Option<ScalarStateDifferenceV1>,
    ),
    String,
> {
    if !matches!(
        descriptor.kind,
        GpuSolverPassKindV1::WarmStart
            | GpuSolverPassKindV1::NormalBiased
            | GpuSolverPassKindV1::TangentBiased
            | GpuSolverPassKindV1::NormalRelaxation
            | GpuSolverPassKindV1::TangentRelaxation
            | GpuSolverPassKindV1::Restitution
    ) {
        return Ok((Vec::new(), None));
    }
    if descriptor.first_color != Some(0)
        || descriptor.color_passes != coloring.max_colors
        || coloring.compacted_point_indices.len()
            != coloring
                .color_occupancy
                .iter()
                .map(|&value| value as usize)
                .sum::<usize>()
    {
        return Err(
            "impulse-operator scalar mirror requires the complete compact color schedule"
                .to_owned(),
        );
    }
    let mut bodies = before.bodies.clone();
    let mut contacts = before.contacts.clone();
    let mut rows = Vec::with_capacity(coloring.color_occupancy.len());
    let mut offset = 0_usize;
    for (color, &occupancy) in coloring.color_occupancy.iter().enumerate() {
        let end = offset + occupancy as usize;
        let prior_contacts = contacts.clone();
        let energy_before = kinetic_energy(&bodies).compensated_f64;
        for &point_index in &coloring.compacted_point_indices[offset..end] {
            solve_manifold(&mut bodies, &mut contacts, point_index as usize, descriptor)?;
        }
        rows.push(ScalarColorImpulseMapV1 {
            color: color as u32,
            manifold_count: occupancy,
            impulse: impulse_vector_map(&prior_contacts, &contacts, final_contacts)?,
            kinetic_energy_delta_compensated_f64: kinetic_energy(&bodies).compensated_f64
                - energy_before,
        });
        offset = end;
    }
    let difference = compare_state(&bodies, &after.bodies, &contacts, &after.contacts, true)?;
    Ok((rows, Some(difference)))
}

fn impulse_vectors_exact(
    left: &[GpuFixedFloorContactEvidence],
    right: &[GpuFixedFloorContactEvidence],
) -> Result<bool, String> {
    if left.len() != right.len() {
        return Err("impulse vectors have different capacities".to_owned());
    }
    Ok(left.iter().zip(right).all(|(left, right)| {
        impulse_value(left, true).to_bits() == impulse_value(right, true).to_bits()
            && impulse_value(left, false).to_bits() == impulse_value(right, false).to_bits()
    }))
}

fn impulse_vector_map(
    before: &[GpuFixedFloorContactEvidence],
    after: &[GpuFixedFloorContactEvidence],
    endpoint: &[GpuFixedFloorContactEvidence],
) -> Result<ImpulseVectorMapV1, String> {
    if before.len() != after.len() || before.len() != endpoint.len() {
        return Err("impulse-vector map capacities differ".to_owned());
    }
    let mut normal_l1_change = 0.0_f64;
    let mut tangent_l1_change = 0.0_f64;
    let mut combined_squared_change = 0.0_f64;
    let mut endpoint_squared_before = 0.0_f64;
    let mut endpoint_squared_after = 0.0_f64;
    let mut normal_total_before = 0.0_f64;
    let mut normal_total_after = 0.0_f64;
    let mut identity_slot_changes = 0_u32;
    for ((before, after), endpoint) in before.iter().zip(after).zip(endpoint) {
        let normal_before = impulse_value(before, true);
        let normal_after = impulse_value(after, true);
        let normal_endpoint = impulse_value(endpoint, true);
        let tangent_before = impulse_value(before, false);
        let tangent_after = impulse_value(after, false);
        let tangent_endpoint = impulse_value(endpoint, false);
        let normal_delta = f64::from(normal_after) - f64::from(normal_before);
        let tangent_delta = f64::from(tangent_after) - f64::from(tangent_before);
        normal_l1_change += normal_delta.abs();
        tangent_l1_change += tangent_delta.abs();
        combined_squared_change += normal_delta * normal_delta + tangent_delta * tangent_delta;
        endpoint_squared_before += (f64::from(normal_before) - f64::from(normal_endpoint)).powi(2)
            + (f64::from(tangent_before) - f64::from(tangent_endpoint)).powi(2);
        endpoint_squared_after += (f64::from(normal_after) - f64::from(normal_endpoint)).powi(2)
            + (f64::from(tangent_after) - f64::from(tangent_endpoint)).powi(2);
        normal_total_before += f64::from(normal_before);
        normal_total_after += f64::from(normal_after);
        let identity = |point: &GpuFixedFloorContactEvidence| {
            point.valid.then_some((
                point.body_a,
                point.body_b,
                point.manifold_index,
                point.point_in_manifold,
            ))
        };
        if identity(before) != identity(after) {
            identity_slot_changes += 1;
        }
    }
    let (active_set_flips, redistribution_transition_max) =
        manifold_transition_metrics(before, after)?;
    Ok(ImpulseVectorMapV1 {
        normal_l1_change,
        tangent_l1_change,
        combined_l2_change: combined_squared_change.sqrt(),
        endpoint_l2_before: endpoint_squared_before.sqrt(),
        endpoint_l2_after: endpoint_squared_after.sqrt(),
        normal_total_before,
        normal_total_after,
        active_set_flips,
        redistribution_transition_max,
        identity_slot_changes,
    })
}

fn impulse_value(point: &GpuFixedFloorContactEvidence, normal: bool) -> f32 {
    if !point.valid {
        0.0
    } else if normal {
        point.normal_impulse
    } else {
        point.tangent_impulse
    }
}

fn manifold_transition_metrics(
    before: &[GpuFixedFloorContactEvidence],
    after: &[GpuFixedFloorContactEvidence],
) -> Result<(u32, f64), String> {
    let mut active_set_flips = 0_u32;
    let mut redistribution_transition_max = 0.0_f64;
    for (index, first) in before.iter().enumerate() {
        if !first.valid || first.point_in_manifold != 0 {
            continue;
        }
        let Some(next_first) = after.get(index) else {
            return Err("impulse manifold transition is truncated".to_owned());
        };
        if !next_first.valid
            || (first.body_a, first.body_b, first.manifold_index)
                != (
                    next_first.body_a,
                    next_first.body_b,
                    next_first.manifold_index,
                )
        {
            continue;
        }
        let second = before.get(index + 1).filter(|point| {
            point.valid
                && point.point_in_manifold == 1
                && (point.body_a, point.body_b, point.manifold_index)
                    == (first.body_a, first.body_b, first.manifold_index)
        });
        let next_second = after.get(index + 1).filter(|point| {
            point.valid
                && point.point_in_manifold == 1
                && (point.body_a, point.body_b, point.manifold_index)
                    == (first.body_a, first.body_b, first.manifold_index)
        });
        if second.is_some() != next_second.is_some() {
            continue;
        }
        let old = [
            first.normal_impulse,
            second.map_or(0.0, |point| point.normal_impulse),
        ];
        let new = [
            next_first.normal_impulse,
            next_second.map_or(0.0, |point| point.normal_impulse),
        ];
        if [old[0] > 0.0, old[1] > 0.0] != [new[0] > 0.0, new[1] > 0.0] {
            active_set_flips += 1;
        }
        if second.is_some() {
            let old_total = f64::from(old[0]) + f64::from(old[1]);
            let new_total = f64::from(new[0]) + f64::from(new[1]);
            let transition = if old_total == 0.0 && new_total == 0.0 {
                0.0
            } else if old_total == 0.0 || new_total == 0.0 {
                1.0
            } else {
                0.5 * ((f64::from(old[0]) / old_total - f64::from(new[0]) / new_total).abs()
                    + (f64::from(old[1]) / old_total - f64::from(new[1]) / new_total).abs())
            };
            redistribution_transition_max = redistribution_transition_max.max(transition);
        }
    }
    Ok((active_set_flips, redistribution_transition_max))
}

/// Attribute each colored warm-start and normal pass to manifolds touching an
/// immovable body versus dynamic-only manifolds while preserving the exact
/// compacted color and manifold order used by the matched scalar replay.
pub fn analyze_static_support_energy_attribution(
    trace: &GpuGeneratedContactPassTraceV3,
) -> Result<SettlingStaticSupportAttributionReportV1, String> {
    trace.validate()?;
    let mut rows = Vec::new();
    for (pass_index, descriptor) in trace.descriptors.iter().enumerate() {
        if !matches!(
            descriptor.kind,
            GpuSolverPassKindV1::WarmStart
                | GpuSolverPassKindV1::NormalBiased
                | GpuSolverPassKindV1::NormalRelaxation
        ) {
            continue;
        }
        let before = trace
            .states
            .get(pass_index)
            .ok_or_else(|| "normal attribution is missing a pre-pass state".to_owned())?;
        let after = trace
            .states
            .get(pass_index + 1)
            .ok_or_else(|| "normal attribution is missing a post-pass state".to_owned())?;
        let mut bodies = before.bodies.clone();
        let mut contacts = before.contacts.clone();
        let mut static_manifolds = 0_u32;
        let mut dynamic_manifolds = 0_u32;
        let mut static_injecting_manifolds = 0_u32;
        let mut dynamic_injecting_manifolds = 0_u32;
        let mut static_net = 0.0_f64;
        let mut dynamic_net = 0.0_f64;
        let mut static_positive = 0.0_f64;
        let mut dynamic_positive = 0.0_f64;
        visit_colored_manifolds(descriptor, &trace.coloring, |point_index| {
            let point = contacts
                .get(point_index)
                .ok_or_else(|| "normal attribution points outside contact capacity".to_owned())?;
            let body_a = bodies
                .get(point.body_a as usize)
                .ok_or_else(|| "normal attribution body A is invalid".to_owned())?;
            let body_b = bodies
                .get(point.body_b as usize)
                .ok_or_else(|| "normal attribution body B is invalid".to_owned())?;
            let static_support =
                body_a.inverse_mass.get() == 0.0 || body_b.inverse_mass.get() == 0.0;
            let energy_before = kinetic_energy(&bodies).compensated_f64;
            solve_manifold(&mut bodies, &mut contacts, point_index, descriptor)?;
            let delta = kinetic_energy(&bodies).compensated_f64 - energy_before;
            if static_support {
                static_manifolds += 1;
                static_net += delta;
                if delta > 0.0 {
                    static_injecting_manifolds += 1;
                    static_positive += delta;
                }
            } else {
                dynamic_manifolds += 1;
                dynamic_net += delta;
                if delta > 0.0 {
                    dynamic_injecting_manifolds += 1;
                    dynamic_positive += delta;
                }
            }
            Ok(())
        })?;
        let replay_state_difference =
            compare_state(&bodies, &after.bodies, &contacts, &after.contacts, true)?;
        let gpu_delta = kinetic_energy(&after.bodies).compensated_f64
            - kinetic_energy(&before.bodies).compensated_f64;
        rows.push(StaticSupportEnergyAttributionRowV1 {
            pass_index: pass_index as u32,
            descriptor: *descriptor,
            static_manifolds,
            dynamic_manifolds,
            static_injecting_manifolds,
            dynamic_injecting_manifolds,
            static_net_delta_compensated_f64: static_net,
            dynamic_net_delta_compensated_f64: dynamic_net,
            static_positive_delta_compensated_f64: static_positive,
            dynamic_positive_delta_compensated_f64: dynamic_positive,
            accounting_error_compensated_f64: static_net + dynamic_net - gpu_delta,
            replay_state_difference,
        });
    }
    let maximum_accounting_error_compensated_f64 = rows
        .iter()
        .map(|row| row.accounting_error_compensated_f64.abs())
        .fold(0.0_f64, f64::max);
    let maximum_replay_ulp_difference = rows
        .iter()
        .map(|row| row.replay_state_difference.max_ulp_difference)
        .max()
        .unwrap_or(0);
    let report = SettlingStaticSupportAttributionReportV1 {
        version: STATIC_SUPPORT_ATTRIBUTION_REPORT_VERSION,
        source_scenario_hash: trace.source_scenario_hash.clone(),
        target_step: trace.target_step,
        rows,
        maximum_accounting_error_compensated_f64,
        maximum_replay_ulp_difference,
    };
    report.validate()?;
    Ok(report)
}

/// Compare the pinned point-sequential relaxation recurrence with the single
/// S6.5d coupled candidate at every captured colored relaxation boundary.
///
/// Both lanes start from the exact captured `f32` pre-pass state and traverse
/// the same compacted color order. The candidate is the unilateral block
/// minimizer of `v^T delta + 1/2 delta^T K delta`: it deliberately has no
/// speculative or penetration target during relaxation. Biased passes retain
/// responsibility for penetration resolution and support construction.
pub fn analyze_scalar_relaxation_derivation(
    trace: &GpuGeneratedContactPassTraceV3,
) -> Result<ScalarRelaxationDerivationReportV1, String> {
    trace.validate()?;
    let mut rows = Vec::new();
    for (pass_index, descriptor) in trace.descriptors.iter().enumerate() {
        if descriptor.kind != GpuSolverPassKindV1::NormalRelaxation {
            continue;
        }
        let before = trace
            .states
            .get(pass_index)
            .ok_or_else(|| "relaxation derivation is missing a pre-pass state".to_owned())?;
        let authority_point_sequential = replay_relaxation_policy(
            &before.bodies,
            &before.contacts,
            descriptor,
            &trace.coloring,
            ScalarRelaxationPolicy::Box2dPointSequential,
        )?;
        let coupled_energy_projection = replay_relaxation_policy(
            &before.bodies,
            &before.contacts,
            descriptor,
            &trace.coloring,
            ScalarRelaxationPolicy::CoupledEnergyProjection,
        )?;
        rows.push(ScalarRelaxationDerivationRowV1 {
            pass_index: pass_index as u32,
            descriptor: *descriptor,
            authority_point_sequential,
            coupled_energy_projection,
        });
    }
    let report = ScalarRelaxationDerivationReportV1 {
        version: RELAXATION_DERIVATION_REPORT_VERSION,
        source_scenario_hash: trace.source_scenario_hash.clone(),
        target_step: trace.target_step,
        authority_revision: "3a4f0da8374af61293a03021c9a0b3ebcfe67948",
        authority_contract: "pinned-box2d-v3-point-sequential-use-bias-false-with-speculation",
        candidate_contract: "colored-coupled-unilateral-zero-target-kinetic-projection",
        rows,
    };
    report.validate()?;
    Ok(report)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum ScalarRelaxationPolicy {
    Box2dPointSequential,
    CoupledEnergyProjection,
}

fn replay_relaxation_policy(
    source_bodies: &[BodyStateV1],
    source_contacts: &[GpuFixedFloorContactEvidence],
    descriptor: &GpuSolverPassDescriptorV1,
    coloring: &GpuColoringEvidenceV4,
    policy: ScalarRelaxationPolicy,
) -> Result<ScalarRelaxationSummaryV1, String> {
    if descriptor.kind != GpuSolverPassKindV1::NormalRelaxation {
        return Err("scalar relaxation policy received a non-relaxation pass".to_owned());
    }
    let mut bodies = source_bodies.to_vec();
    let mut contacts = source_contacts.to_vec();
    let energy_before = kinetic_energy(&bodies);
    let positive_separation_points = contacts
        .iter()
        .filter(|point| point.valid && point.separation > 0.0)
        .count() as u32;
    let supported_points_before = contacts
        .iter()
        .filter(|point| point.valid && point.normal_impulse > 0.0)
        .count() as u32;
    let normal_impulse_before = contacts
        .iter()
        .filter(|point| point.valid)
        .map(|point| f64::from(point.normal_impulse))
        .sum();
    let (maximum_penetration, maximum_separation) = separation_extrema(&contacts);

    if descriptor.first_color != Some(0)
        || descriptor.color_passes != coloring.max_colors
        || coloring.compacted_point_indices.len()
            != coloring
                .color_occupancy
                .iter()
                .map(|&value| value as usize)
                .sum::<usize>()
    {
        return Err(
            "relaxation derivation requires the complete compact color schedule".to_owned(),
        );
    }
    let mut maximum_manifold_energy_delta = f64::NEG_INFINITY;
    let mut energy_injecting_manifolds = 0_u32;
    let mut offset = 0_usize;
    for &occupancy in &coloring.color_occupancy {
        let end = offset + occupancy as usize;
        for &point_index in &coloring.compacted_point_indices[offset..end] {
            let before = kinetic_energy(&bodies).compensated_f64;
            solve_relaxation_manifold(
                &mut bodies,
                &mut contacts,
                point_index as usize,
                descriptor,
                policy,
            )?;
            let delta = kinetic_energy(&bodies).compensated_f64 - before;
            maximum_manifold_energy_delta = maximum_manifold_energy_delta.max(delta);
            if delta > 1.0e-12 {
                energy_injecting_manifolds += 1;
            }
        }
        offset = end;
    }
    if maximum_manifold_energy_delta == f64::NEG_INFINITY {
        maximum_manifold_energy_delta = 0.0;
    }
    let energy_after = kinetic_energy(&bodies);
    let supported_points_after = contacts
        .iter()
        .filter(|point| point.valid && point.normal_impulse > 0.0)
        .count() as u32;
    let normal_impulse_after = contacts
        .iter()
        .filter(|point| point.valid)
        .map(|point| f64::from(point.normal_impulse))
        .sum();
    Ok(ScalarRelaxationSummaryV1 {
        energy_before,
        energy_after,
        energy_delta_compensated_f64: energy_after.compensated_f64 - energy_before.compensated_f64,
        maximum_manifold_energy_delta,
        energy_injecting_manifolds,
        positive_separation_points,
        supported_points_before,
        supported_points_after,
        normal_impulse_before,
        normal_impulse_after,
        maximum_penetration,
        maximum_separation,
    })
}

fn separation_extrema(contacts: &[GpuFixedFloorContactEvidence]) -> (f32, f32) {
    let mut penetration = 0.0_f32;
    let mut separation = 0.0_f32;
    for point in contacts.iter().filter(|point| point.valid) {
        penetration = penetration.max((-point.separation).max(0.0));
        separation = separation.max(point.separation.max(0.0));
    }
    (penetration, separation)
}

pub fn kinetic_energy(bodies: &[BodyStateV1]) -> KineticEnergyAccountingV1 {
    let mut ordered = 0.0_f32;
    let mut compensated = 0.0_f64;
    let mut correction = 0.0_f64;
    for body in bodies {
        let inverse_mass = body.inverse_mass.get();
        if inverse_mass <= 0.0 {
            continue;
        }
        let mass = inverse_mass.recip();
        let inverse_inertia = body.inverse_inertia.get();
        let inertia = if inverse_inertia > 0.0 {
            inverse_inertia.recip()
        } else {
            0.0
        };
        let linear_squared = body.linear_velocity.x * body.linear_velocity.x
            + body.linear_velocity.y * body.linear_velocity.y;
        let angular_squared = body.angular_velocity.get() * body.angular_velocity.get();
        let term = 0.5_f32 * mass * linear_squared + 0.5_f32 * inertia * angular_squared;
        ordered += term;

        let vx = f64::from(body.linear_velocity.x);
        let vy = f64::from(body.linear_velocity.y);
        let angular = f64::from(body.angular_velocity.get());
        let wider_term = 0.5 * f64::from(mass) * (vx * vx + vy * vy)
            + 0.5 * f64::from(inertia) * angular * angular;
        let adjusted = wider_term - correction;
        let next = compensated + adjusted;
        correction = (next - compensated) - adjusted;
        compensated = next;
    }
    KineticEnergyAccountingV1 {
        ordered_f32: ordered,
        compensated_f64: compensated,
    }
}

fn replay_pass(
    bodies: &mut [BodyStateV1],
    contacts: &mut [GpuFixedFloorContactEvidence],
    descriptor: &GpuSolverPassDescriptorV1,
    coloring: &GpuColoringEvidenceV4,
) -> Result<(), String> {
    match descriptor.kind {
        GpuSolverPassKindV1::IntegrateVelocity => {
            let dt = descriptor.parameters.floats0[0];
            let gravity_x = descriptor.parameters.floats1[1];
            let gravity_y = descriptor.parameters.floats1[2];
            for body in bodies {
                if body.inverse_mass.get() > 0.0 {
                    body.linear_velocity.x += gravity_x * dt;
                    body.linear_velocity.y += gravity_y * dt;
                }
            }
        }
        GpuSolverPassKindV1::IntegratePosition => {
            let dt = descriptor.parameters.floats0[0];
            for body in bodies {
                if body.inverse_mass.get() > 0.0 {
                    body.position.x += body.linear_velocity.x * dt;
                    body.position.y += body.linear_velocity.y * dt;
                    let angle = body.angle.get() + body.angular_velocity.get() * dt;
                    body.angle = physbox_contracts::FiniteF32::new(angle)
                        .map_err(|error| error.to_string())?;
                }
            }
        }
        GpuSolverPassKindV1::WarmStart
        | GpuSolverPassKindV1::NormalBiased
        | GpuSolverPassKindV1::TangentBiased
        | GpuSolverPassKindV1::NormalRelaxation
        | GpuSolverPassKindV1::TangentRelaxation
        | GpuSolverPassKindV1::Restitution => {
            replay_colored_manifolds(bodies, contacts, descriptor, coloring)?;
        }
        GpuSolverPassKindV1::Gather => {
            return Err("selected pass-energy recurrence does not admit gather".to_owned());
        }
        GpuSolverPassKindV1::PrepareContacts | GpuSolverPassKindV1::RefreshContacts => {
            return Err(
                "prepared-contact boundaries must be synchronized by the caller".to_owned(),
            );
        }
    }
    Ok(())
}

fn replay_colored_manifolds(
    bodies: &mut [BodyStateV1],
    contacts: &mut [GpuFixedFloorContactEvidence],
    descriptor: &GpuSolverPassDescriptorV1,
    coloring: &GpuColoringEvidenceV4,
) -> Result<(), String> {
    visit_colored_manifolds(descriptor, coloring, |point_index| {
        solve_manifold(bodies, contacts, point_index, descriptor)
    })
}

fn visit_colored_manifolds(
    descriptor: &GpuSolverPassDescriptorV1,
    coloring: &GpuColoringEvidenceV4,
    mut visit: impl FnMut(usize) -> Result<(), String>,
) -> Result<(), String> {
    if descriptor.first_color != Some(0)
        || descriptor.color_passes != coloring.max_colors
        || coloring.compacted_point_indices.len()
            != coloring
                .color_occupancy
                .iter()
                .map(|&value| value as usize)
                .sum::<usize>()
    {
        return Err("scalar recurrence requires the complete compact color schedule".to_owned());
    }
    let mut offset = 0_usize;
    for &occupancy in &coloring.color_occupancy {
        let end = offset + occupancy as usize;
        for &point_index in &coloring.compacted_point_indices[offset..end] {
            visit(point_index as usize)?;
        }
        offset = end;
    }
    Ok(())
}

fn solve_relaxation_manifold(
    bodies: &mut [BodyStateV1],
    contacts: &mut [GpuFixedFloorContactEvidence],
    point_index: usize,
    descriptor: &GpuSolverPassDescriptorV1,
    policy: ScalarRelaxationPolicy,
) -> Result<(), String> {
    let Some(point0) = contacts.get(point_index) else {
        return Err("relaxation derivation points outside contact capacity".to_owned());
    };
    if !point0.valid || point0.point_in_manifold != 0 {
        return Err("relaxation derivation color unit is not a manifold representative".to_owned());
    }
    let has_second = contacts.get(point_index + 1).is_some_and(|point1| {
        point1.valid
            && point1.body_a == point0.body_a
            && point1.body_b == point0.body_b
            && point1.manifold_index == point0.manifold_index
            && point1.point_in_manifold == 1
    });
    let body_a = point0.body_a as usize;
    let body_b = point0.body_b as usize;
    if body_a == body_b || body_a >= bodies.len() || body_b >= bodies.len() {
        return Err("relaxation derivation contact bodies are invalid".to_owned());
    }
    let point0 = contacts[point_index].clone();
    let point1 = has_second.then(|| contacts[point_index + 1].clone());
    let mut motion_a = motion(&bodies[body_a]);
    let mut motion_b = motion(&bodies[body_b]);

    match policy {
        ScalarRelaxationPolicy::Box2dPointSequential => {
            for (offset, point) in std::iter::once(&point0).chain(point1.as_ref()).enumerate() {
                let normal_velocity = relative_velocity(point, point.normal, motion_a, motion_b);
                let update = box2d_point_normal_update(Box2dPointNormalInput {
                    separation: point.separation,
                    substep_dt: descriptor.parameters.floats0[0],
                    normal_velocity,
                    normal_mass: point.normal_effective_mass,
                    old_impulse: point.normal_impulse,
                    penetration_velocity_bias: 0.0,
                    penetration_mass_scale: 1.0,
                    penetration_impulse_scale: 0.0,
                    use_bias: false,
                })
                .map_err(|error| error.to_string())?;
                let contact = &mut contacts[point_index + offset];
                contact.normal_impulse = update.impulse;
                contact.total_normal_impulse += update.applied_impulse.max(0.0);
                let impulse = scale(point.normal, update.applied_impulse);
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse, -1.0),
                    point.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            }
        }
        ScalarRelaxationPolicy::CoupledEnergyProjection => {
            let matrix =
                energy_block_matrix(&bodies[body_a], &bodies[body_b], &point0, point1.as_ref());
            let old = [
                point0.normal_impulse,
                point1.as_ref().map_or(0.0, |point| point.normal_impulse),
            ];
            let base = [
                relative_velocity(&point0, point0.normal, motion_a, motion_b),
                point1.as_ref().map_or(0.0, |point| {
                    relative_velocity(point, point.normal, motion_a, motion_b)
                }),
            ];
            let solution = solve_normal_block(matrix, base, old, [0.0, 0.0], has_second);
            for (offset, point) in std::iter::once(&point0).chain(point1.as_ref()).enumerate() {
                let applied = solution[offset] - old[offset];
                let contact = &mut contacts[point_index + offset];
                contact.normal_impulse = solution[offset];
                contact.total_normal_impulse += applied.max(0.0);
                let impulse = scale(point.normal, applied);
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse, -1.0),
                    point.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            }
        }
    }
    set_motion(&mut bodies[body_a], motion_a)?;
    set_motion(&mut bodies[body_b], motion_b)?;
    Ok(())
}

fn energy_block_matrix(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    point0: &GpuFixedFloorContactEvidence,
    point1: Option<&GpuFixedFloorContactEvidence>,
) -> [f32; 4] {
    let k11 = energy_block_matrix_entry(body_a, body_b, point0, point0);
    point1.map_or([k11, 0.0, 0.0, 0.0], |point1| {
        [
            k11,
            energy_block_matrix_entry(body_a, body_b, point0, point1),
            energy_block_matrix_entry(body_a, body_b, point1, point0),
            energy_block_matrix_entry(body_a, body_b, point1, point1),
        ]
    })
}

fn energy_block_matrix_entry(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    left: &GpuFixedFloorContactEvidence,
    right: &GpuFixedFloorContactEvidence,
) -> f32 {
    (body_a.inverse_mass.get() + body_b.inverse_mass.get()) * dot(left.normal, right.normal)
        + body_a.inverse_inertia.get()
            * cross(left.anchor_a, left.normal)
            * cross(right.anchor_a, right.normal)
        + body_b.inverse_inertia.get()
            * cross(left.anchor_b, left.normal)
            * cross(right.anchor_b, right.normal)
}

fn solve_manifold(
    bodies: &mut [BodyStateV1],
    contacts: &mut [GpuFixedFloorContactEvidence],
    point_index: usize,
    descriptor: &GpuSolverPassDescriptorV1,
) -> Result<(), String> {
    let Some(point0) = contacts.get(point_index) else {
        return Err("scalar recurrence color points outside contact capacity".to_owned());
    };
    if !point0.valid || point0.point_in_manifold != 0 {
        return Err(
            "scalar recurrence color unit is not a valid manifold representative".to_owned(),
        );
    }
    let has_second = contacts.get(point_index + 1).is_some_and(|point1| {
        point1.valid
            && point1.body_a == point0.body_a
            && point1.body_b == point0.body_b
            && point1.manifold_index == point0.manifold_index
            && point1.point_in_manifold == 1
    });
    let body_a = point0.body_a as usize;
    let body_b = point0.body_b as usize;
    if body_a == body_b || body_a >= bodies.len() || body_b >= bodies.len() {
        return Err("scalar recurrence contact bodies are invalid".to_owned());
    }
    let mut motion_a = motion(&bodies[body_a]);
    let mut motion_b = motion(&bodies[body_b]);

    match descriptor.kind {
        GpuSolverPassKindV1::WarmStart => {
            let point = contacts[point_index].clone();
            let impulse = add(
                scale(point.normal, point.normal_impulse),
                scale(point.tangent, point.tangent_impulse),
            );
            motion_a = apply_impulse(
                &bodies[body_a],
                motion_a,
                scale(impulse, -1.0),
                point.anchor_a,
            );
            motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            if has_second {
                let point = contacts[point_index + 1].clone();
                let impulse = add(
                    scale(point.normal, point.normal_impulse),
                    scale(point.tangent, point.tangent_impulse),
                );
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse, -1.0),
                    point.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            }
        }
        GpuSolverPassKindV1::NormalBiased | GpuSolverPassKindV1::NormalRelaxation => {
            let point0 = contacts[point_index].clone();
            let point1 = has_second.then(|| contacts[point_index + 1].clone());
            if descriptor.kind == GpuSolverPassKindV1::NormalRelaxation
                && descriptor.parameters.floats2[3] == 2.0
            {
                let equation0 = normal_equation(
                    &bodies[body_a],
                    &bodies[body_b],
                    &point0,
                    motion_a,
                    motion_b,
                    descriptor,
                );
                let k11 = block_matrix_entry(&bodies[body_a], &bodies[body_b], &point0, &point0);
                let mass0 = if k11 > 0.0 { k11.recip() } else { 0.0 };
                let old0 = point0.normal_impulse;
                let next0 = (old0 - mass0 * equation0[0] - equation0[1] * old0).max(0.0);
                let applied0 = next0 - old0;
                contacts[point_index].normal_impulse = next0;
                contacts[point_index].total_normal_impulse += applied0.max(0.0);
                let impulse0 = scale(point0.normal, applied0);
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse0, -1.0),
                    point0.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse0, point0.anchor_b);
                if let Some(point1) = point1 {
                    let equation1 = normal_equation(
                        &bodies[body_a],
                        &bodies[body_b],
                        &point1,
                        motion_a,
                        motion_b,
                        descriptor,
                    );
                    let k22 =
                        block_matrix_entry(&bodies[body_a], &bodies[body_b], &point1, &point1);
                    let mass1 = if k22 > 0.0 { k22.recip() } else { 0.0 };
                    let old1 = point1.normal_impulse;
                    let next1 = (old1 - mass1 * equation1[0] - equation1[1] * old1).max(0.0);
                    let applied1 = next1 - old1;
                    contacts[point_index + 1].normal_impulse = next1;
                    contacts[point_index + 1].total_normal_impulse += applied1.max(0.0);
                    let impulse1 = scale(point1.normal, applied1);
                    motion_a = apply_impulse(
                        &bodies[body_a],
                        motion_a,
                        scale(impulse1, -1.0),
                        point1.anchor_a,
                    );
                    motion_b = apply_impulse(&bodies[body_b], motion_b, impulse1, point1.anchor_b);
                }
            } else {
                let matrix =
                    block_matrix(&bodies[body_a], &bodies[body_b], &point0, point1.as_ref());
                let old = [
                    point0.normal_impulse,
                    point1.as_ref().map_or(0.0, |point| point.normal_impulse),
                ];
                let equation0 = normal_equation(
                    &bodies[body_a],
                    &bodies[body_b],
                    &point0,
                    motion_a,
                    motion_b,
                    descriptor,
                );
                let equation1 = point1.as_ref().map_or([0.0, 0.0], |point| {
                    normal_equation(
                        &bodies[body_a],
                        &bodies[body_b],
                        point,
                        motion_a,
                        motion_b,
                        descriptor,
                    )
                });
                let solution = solve_normal_block(
                    matrix,
                    [equation0[0], equation1[0]],
                    old,
                    [equation0[1], equation1[1]],
                    has_second,
                );
                let applied0 = solution[0] - old[0];
                contacts[point_index].normal_impulse = solution[0];
                contacts[point_index].total_normal_impulse += applied0.max(0.0);
                let impulse0 = scale(point0.normal, applied0);
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse0, -1.0),
                    point0.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse0, point0.anchor_b);
                if let Some(point1) = point1 {
                    let applied1 = solution[1] - old[1];
                    contacts[point_index + 1].normal_impulse = solution[1];
                    contacts[point_index + 1].total_normal_impulse += applied1.max(0.0);
                    let impulse1 = scale(point1.normal, applied1);
                    motion_a = apply_impulse(
                        &bodies[body_a],
                        motion_a,
                        scale(impulse1, -1.0),
                        point1.anchor_a,
                    );
                    motion_b = apply_impulse(&bodies[body_b], motion_b, impulse1, point1.anchor_b);
                }
            }
        }
        GpuSolverPassKindV1::TangentBiased | GpuSolverPassKindV1::TangentRelaxation => {
            let point = contacts[point_index].clone();
            let tangent_velocity = relative_velocity(&point, point.tangent, motion_a, motion_b);
            let old = point.tangent_impulse;
            let limit = point.friction * point.normal_impulse;
            let next = (old - point.tangent_effective_mass * tangent_velocity).clamp(-limit, limit);
            contacts[point_index].tangent_impulse = next;
            let impulse = scale(point.tangent, next - old);
            motion_a = apply_impulse(
                &bodies[body_a],
                motion_a,
                scale(impulse, -1.0),
                point.anchor_a,
            );
            motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            if has_second {
                let point = contacts[point_index + 1].clone();
                let tangent_velocity = relative_velocity(&point, point.tangent, motion_a, motion_b);
                let old = point.tangent_impulse;
                let limit = point.friction * point.normal_impulse;
                let next =
                    (old - point.tangent_effective_mass * tangent_velocity).clamp(-limit, limit);
                contacts[point_index + 1].tangent_impulse = next;
                let impulse = scale(point.tangent, next - old);
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse, -1.0),
                    point.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            }
        }
        GpuSolverPassKindV1::Restitution => {
            let point = contacts[point_index].clone();
            let normal_velocity = relative_velocity(&point, point.normal, motion_a, motion_b);
            let old = point.normal_impulse;
            let eligible = point.restitution > 0.0
                && point.initial_normal_velocity < -descriptor.parameters.floats1[0]
                && point.total_normal_impulse > 0.0;
            let next = if eligible {
                (old - point.normal_effective_mass
                    * (normal_velocity + point.restitution * point.initial_normal_velocity))
                    .max(0.0)
            } else {
                old
            };
            contacts[point_index].normal_impulse = next;
            let impulse = scale(point.normal, next - old);
            motion_a = apply_impulse(
                &bodies[body_a],
                motion_a,
                scale(impulse, -1.0),
                point.anchor_a,
            );
            motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            if has_second {
                let point = contacts[point_index + 1].clone();
                let normal_velocity = relative_velocity(&point, point.normal, motion_a, motion_b);
                let old = point.normal_impulse;
                let eligible = point.restitution > 0.0
                    && point.initial_normal_velocity < -descriptor.parameters.floats1[0]
                    && point.total_normal_impulse > 0.0;
                let next = if eligible {
                    (old - point.normal_effective_mass
                        * (normal_velocity + point.restitution * point.initial_normal_velocity))
                        .max(0.0)
                } else {
                    old
                };
                contacts[point_index + 1].normal_impulse = next;
                let impulse = scale(point.normal, next - old);
                motion_a = apply_impulse(
                    &bodies[body_a],
                    motion_a,
                    scale(impulse, -1.0),
                    point.anchor_a,
                );
                motion_b = apply_impulse(&bodies[body_b], motion_b, impulse, point.anchor_b);
            }
        }
        _ => return Err("scalar manifold recurrence received a non-contact pass".to_owned()),
    }

    set_motion(&mut bodies[body_a], motion_a)?;
    set_motion(&mut bodies[body_b], motion_b)?;
    Ok(())
}

fn normal_equation(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    point: &GpuFixedFloorContactEvidence,
    motion_a: [f32; 3],
    motion_b: [f32; 3],
    descriptor: &GpuSolverPassDescriptorV1,
) -> [f32; 2] {
    let normal_velocity = relative_velocity(point, point.normal, motion_a, motion_b);
    if descriptor.parameters.floats2[3] == 3.0 {
        return [normal_velocity, 0.0];
    }
    let mut velocity_bias = 0.0_f32;
    let mut mass_scale = 1.0_f32;
    let mut impulse_scale = 0.0_f32;
    let coefficients = if descriptor.parameters.floats3[3] != 0.0
        && (body_a.inverse_mass.get() == 0.0 || body_b.inverse_mass.get() == 0.0)
    {
        &descriptor.parameters.floats3[..3]
    } else {
        &descriptor.parameters.floats0[1..4]
    };
    if point.separation > 0.0 {
        velocity_bias = point.separation / descriptor.parameters.floats0[0];
    } else if descriptor.parameters.uints1[3] != 0 || descriptor.parameters.floats2[3] == 1.0 {
        if descriptor.parameters.uints1[3] != 0 {
            velocity_bias = coefficients[0] * point.separation / descriptor.parameters.floats0[0];
            if descriptor.parameters.floats2[0] > 0.0 {
                velocity_bias = velocity_bias.max(-descriptor.parameters.floats2[0]);
            }
        }
        mass_scale = coefficients[1];
        impulse_scale = coefficients[2];
    }
    [mass_scale * normal_velocity + velocity_bias, impulse_scale]
}

fn block_matrix(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    point0: &GpuFixedFloorContactEvidence,
    point1: Option<&GpuFixedFloorContactEvidence>,
) -> [f32; 4] {
    let k11 = block_matrix_entry(body_a, body_b, point0, point0);
    point1.map_or([k11, 0.0, 0.0, 0.0], |point1| {
        [
            k11,
            block_matrix_entry(body_a, body_b, point0, point1),
            block_matrix_entry(body_a, body_b, point1, point0),
            block_matrix_entry(body_a, body_b, point1, point1),
        ]
    })
}

fn block_matrix_entry(
    body_a: &BodyStateV1,
    body_b: &BodyStateV1,
    left: &GpuFixedFloorContactEvidence,
    right: &GpuFixedFloorContactEvidence,
) -> f32 {
    body_a.inverse_mass.get()
        + body_b.inverse_mass.get()
        + body_a.inverse_inertia.get()
            * cross(left.anchor_a, left.normal)
            * cross(right.anchor_a, right.normal)
        + body_b.inverse_inertia.get()
            * cross(left.anchor_b, left.normal)
            * cross(right.anchor_b, right.normal)
}

fn solve_normal_block(
    matrix: [f32; 4],
    base: [f32; 2],
    old: [f32; 2],
    impulse_scale: [f32; 2],
    has_second: bool,
) -> [f32; 2] {
    if !has_second {
        let mass = if matrix[0] > 0.0 {
            matrix[0].recip()
        } else {
            0.0
        };
        return [
            (old[0] - mass * base[0] - impulse_scale[0] * old[0]).max(0.0),
            0.0,
        ];
    }
    let scaled_old = [impulse_scale[0] * old[0], impulse_scale[1] * old[1]];
    let rhs = [
        base[0] + matrix[0] * scaled_old[0] + matrix[1] * scaled_old[1],
        base[1] + matrix[2] * scaled_old[0] + matrix[3] * scaled_old[1],
    ];
    let determinant = matrix[0] * matrix[3] - matrix[1] * matrix[2];
    if determinant > 0.0 {
        let delta = [
            (-matrix[3] * rhs[0] + matrix[1] * rhs[1]) / determinant,
            (matrix[2] * rhs[0] - matrix[0] * rhs[1]) / determinant,
        ];
        let candidate = [old[0] + delta[0], old[1] + delta[1]];
        if candidate[0] >= 0.0 && candidate[1] >= 0.0 {
            return candidate;
        }
    }
    if matrix[0] > 0.0 {
        let delta1 = -old[1];
        let delta0 = -(rhs[0] + matrix[1] * delta1) / matrix[0];
        let candidate0 = old[0] + delta0;
        let inactive_velocity = rhs[1] + matrix[2] * delta0 + matrix[3] * delta1;
        if candidate0 >= 0.0 && inactive_velocity >= 0.0 {
            return [candidate0, 0.0];
        }
    }
    if matrix[3] > 0.0 {
        let delta0 = -old[0];
        let delta1 = -(rhs[1] + matrix[2] * delta0) / matrix[3];
        let candidate1 = old[1] + delta1;
        let inactive_velocity = rhs[0] + matrix[0] * delta0 + matrix[1] * delta1;
        if candidate1 >= 0.0 && inactive_velocity >= 0.0 {
            return [0.0, candidate1];
        }
    }
    let delta = [-old[0], -old[1]];
    let velocity = [
        rhs[0] + matrix[0] * delta[0] + matrix[1] * delta[1],
        rhs[1] + matrix[2] * delta[0] + matrix[3] * delta[1],
    ];
    if velocity[0] >= 0.0 && velocity[1] >= 0.0 {
        [0.0, 0.0]
    } else {
        old
    }
}

fn motion(body: &BodyStateV1) -> [f32; 3] {
    [
        body.linear_velocity.x,
        body.linear_velocity.y,
        body.angular_velocity.get(),
    ]
}

fn set_motion(body: &mut BodyStateV1, motion: [f32; 3]) -> Result<(), String> {
    body.linear_velocity = Vec2::new(motion[0], motion[1]);
    body.angular_velocity =
        physbox_contracts::FiniteF32::new(motion[2]).map_err(|error| error.to_string())?;
    Ok(())
}

fn point_velocity(motion: [f32; 3], anchor: Vec2) -> Vec2 {
    Vec2::new(
        motion[0] - motion[2] * anchor.y,
        motion[1] + motion[2] * anchor.x,
    )
}

fn relative_velocity(
    point: &GpuFixedFloorContactEvidence,
    axis: Vec2,
    motion_a: [f32; 3],
    motion_b: [f32; 3],
) -> f32 {
    let velocity_a = point_velocity(motion_a, point.anchor_a);
    let velocity_b = point_velocity(motion_b, point.anchor_b);
    dot(
        Vec2::new(velocity_b.x - velocity_a.x, velocity_b.y - velocity_a.y),
        axis,
    )
}

fn apply_impulse(
    body: &BodyStateV1,
    mut motion: [f32; 3],
    impulse: Vec2,
    anchor: Vec2,
) -> [f32; 3] {
    motion[0] += impulse.x * body.inverse_mass.get();
    motion[1] += impulse.y * body.inverse_mass.get();
    motion[2] += body.inverse_inertia.get() * cross(anchor, impulse);
    motion
}

fn add(a: Vec2, b: Vec2) -> Vec2 {
    Vec2::new(a.x + b.x, a.y + b.y)
}

fn scale(value: Vec2, scalar: f32) -> Vec2 {
    Vec2::new(value.x * scalar, value.y * scalar)
}

fn dot(a: Vec2, b: Vec2) -> f32 {
    a.x * b.x + a.y * b.y
}

fn cross(a: Vec2, b: Vec2) -> f32 {
    a.x * b.y - a.y * b.x
}

fn compare_state(
    predicted_bodies: &[BodyStateV1],
    gpu_bodies: &[BodyStateV1],
    predicted_contacts: &[GpuFixedFloorContactEvidence],
    gpu_contacts: &[GpuFixedFloorContactEvidence],
    compare_contacts: bool,
) -> Result<ScalarStateDifferenceV1, String> {
    if predicted_bodies.len() != gpu_bodies.len()
        || (compare_contacts && predicted_contacts.len() != gpu_contacts.len())
    {
        return Err("scalar/GPU state shapes disagree".to_owned());
    }
    let mut difference = ScalarStateDifferenceV1 {
        compared_f32_values: 0,
        exact_f32_values: 0,
        max_absolute_difference: 0.0,
        max_ulp_difference: 0,
        max_ulp_field: None,
        max_ulp_index: None,
    };
    for (index, (predicted, gpu)) in predicted_bodies.iter().zip(gpu_bodies).enumerate() {
        for (field, left, right) in [
            ("body.position.x", predicted.position.x, gpu.position.x),
            ("body.position.y", predicted.position.y, gpu.position.y),
            ("body.angle", predicted.angle.get(), gpu.angle.get()),
            (
                "body.linearVelocity.x",
                predicted.linear_velocity.x,
                gpu.linear_velocity.x,
            ),
            (
                "body.linearVelocity.y",
                predicted.linear_velocity.y,
                gpu.linear_velocity.y,
            ),
            (
                "body.angularVelocity",
                predicted.angular_velocity.get(),
                gpu.angular_velocity.get(),
            ),
        ] {
            include_difference(&mut difference, field, index as u32, left, right);
        }
    }
    if compare_contacts {
        for (index, (predicted, gpu)) in predicted_contacts.iter().zip(gpu_contacts).enumerate() {
            for (field, left, right) in [
                ("contact.normal.x", predicted.normal.x, gpu.normal.x),
                ("contact.normal.y", predicted.normal.y, gpu.normal.y),
                ("contact.tangent.x", predicted.tangent.x, gpu.tangent.x),
                ("contact.tangent.y", predicted.tangent.y, gpu.tangent.y),
                ("contact.anchorA.x", predicted.anchor_a.x, gpu.anchor_a.x),
                ("contact.anchorA.y", predicted.anchor_a.y, gpu.anchor_a.y),
                ("contact.anchorB.x", predicted.anchor_b.x, gpu.anchor_b.x),
                ("contact.anchorB.y", predicted.anchor_b.y, gpu.anchor_b.y),
                (
                    "contact.initialNormalVelocity",
                    predicted.initial_normal_velocity,
                    gpu.initial_normal_velocity,
                ),
                (
                    "contact.normalEffectiveMass",
                    predicted.normal_effective_mass,
                    gpu.normal_effective_mass,
                ),
                (
                    "contact.tangentEffectiveMass",
                    predicted.tangent_effective_mass,
                    gpu.tangent_effective_mass,
                ),
                ("contact.separation", predicted.separation, gpu.separation),
                ("contact.friction", predicted.friction, gpu.friction),
                (
                    "contact.restitution",
                    predicted.restitution,
                    gpu.restitution,
                ),
                (
                    "contact.normalImpulse",
                    predicted.normal_impulse,
                    gpu.normal_impulse,
                ),
                (
                    "contact.tangentImpulse",
                    predicted.tangent_impulse,
                    gpu.tangent_impulse,
                ),
                (
                    "contact.totalNormalImpulse",
                    predicted.total_normal_impulse,
                    gpu.total_normal_impulse,
                ),
            ] {
                include_difference(&mut difference, field, index as u32, left, right);
            }
        }
    }
    Ok(difference)
}

fn include_difference(
    difference: &mut ScalarStateDifferenceV1,
    field: &'static str,
    index: u32,
    left: f32,
    right: f32,
) {
    difference.compared_f32_values += 1;
    if left.to_bits() == right.to_bits() {
        difference.exact_f32_values += 1;
        return;
    }
    difference.max_absolute_difference =
        difference.max_absolute_difference.max((left - right).abs());
    let ulp = ulp_difference(left, right);
    if ulp > difference.max_ulp_difference {
        difference.max_ulp_difference = ulp;
        difference.max_ulp_field = Some(field);
        difference.max_ulp_index = Some(index);
    }
}

fn ulp_difference(left: f32, right: f32) -> u32 {
    if left == right && left.to_bits() != right.to_bits() {
        return 1;
    }
    fn ordered(value: f32) -> i32 {
        let bits = value.to_bits() as i32;
        if bits < 0 { i32::MIN - bits } else { bits }
    }
    ordered(left).abs_diff(ordered(right))
}

#[cfg(test)]
mod tests {
    use physbox_contracts::{BodyId, FiniteF32, MaterialV1, ShapeV1};

    use super::*;

    fn finite(value: f32) -> FiniteF32 {
        FiniteF32::new(value).unwrap()
    }

    #[test]
    fn kinetic_energy_reports_strict_and_compensated_accumulations() {
        let bodies = [BodyStateV1 {
            id: BodyId {
                slot: 0,
                generation: 1,
            },
            shape: ShapeV1::Circle {
                radius: finite(1.0),
            },
            position: Vec2::ZERO,
            angle: finite(0.0),
            linear_velocity: Vec2::new(3.0, 4.0),
            angular_velocity: finite(2.0),
            inverse_mass: finite(0.5),
            inverse_inertia: finite(0.25),
            material: MaterialV1 {
                friction: finite(0.5),
                restitution: finite(0.0),
            },
            sleeping: false,
        }];
        let energy = kinetic_energy(&bodies);
        assert_eq!(energy.ordered_f32, 33.0);
        assert_eq!(energy.compensated_f64, 33.0);
    }

    #[test]
    fn ulp_distance_handles_sign_and_adjacent_values() {
        assert_eq!(ulp_difference(-0.0, 0.0), 1);
        assert_eq!(
            ulp_difference(1.0, f32::from_bits(1.0_f32.to_bits() + 1)),
            1
        );
        assert_eq!(ulp_difference(-1.0, -1.0), 0);
    }

    #[test]
    fn coupled_zero_target_block_is_a_dissipative_unilateral_projection() {
        for matrix in [[2.0, 0.5, 0.5, 3.0], [6.0, -1.0, -1.0, 4.0]] {
            for base in [[-1.0, 0.25], [0.5, 1.0], [-0.1, -0.2]] {
                for old in [[0.0, 0.0], [0.5, 0.25], [0.01, 1.0]] {
                    let solution = solve_normal_block(matrix, base, old, [0.0, 0.0], true);
                    let delta = [solution[0] - old[0], solution[1] - old[1]];
                    let energy_delta = f64::from(base[0] * delta[0] + base[1] * delta[1])
                        + 0.5
                            * f64::from(
                                matrix[0] * delta[0] * delta[0]
                                    + (matrix[1] + matrix[2]) * delta[0] * delta[1]
                                    + matrix[3] * delta[1] * delta[1],
                            );
                    assert!(solution[0] >= 0.0 && solution[1] >= 0.0);
                    assert!(energy_delta <= 1.0e-6, "energy delta {energy_delta}");
                }
            }
        }
    }
}
