//! Bounded S12 scalar bridge for decomposing the pinned authority structure.
//!
//! This module owns an independent orchestration loop over PhysBox's scalar
//! equations and neutral contact projection. It does not link or copy Box2D.
//! The authority-side schedule is derived from clean Box2D revision
//! `3a4f0da8374af61293a03021c9a0b3ebcfe67948`, principally `solver.c`,
//! `constraint_graph.c`, `contact_solver.c`, and `physics_world.c`.

use physbox_contracts::{
    AuthoredManifoldV1, BodyStateV1, ContactCachePointV1, ContactSoftnessPolicyV1, FeatureIdV1,
    FiniteF32, ScenarioV1, ShapeV1, TraceRecordV1, TraceStageV1, Validate, Vec2,
};
use serde::{Deserialize, Serialize};

use crate::branch_robust::{
    BRANCH_ROBUST_CANDIDATE_ID, BRANCH_ROBUST_ERROR_BOUND_VERSION, BRANCH_ROBUST_FORMULA_VERSION,
};
use crate::branch_robust_scalar::{BranchRobustPerturbationV1, BranchRobustRunStatisticsV1};
use crate::precision_margin::{PrecisionPathStatisticsV1, PrecisionPerturbationV1};
use crate::scalar::{
    PreparedContact, ScalarError, ScalarObservedSolveResultV1, ScalarSolveResult,
    ScalarStepObservationV1, apply_restitution, collect_contact_cache, generate_portable_contacts,
    integrate_positions, integrate_positions_authority, integrate_velocities,
    maximum_depenetration_velocity, order_authority_contact_points, prepare_contact,
    refresh_authority_separations, refresh_branch_robust_authority_separations,
    refresh_precision_path_authority_separations, refresh_separations, solve_normal, solve_tangent,
    warm_start,
};

pub const AUTHORITY_STRUCTURE_POLICY_VERSION: u32 = 1;
pub const AUTHORITY_STRUCTURE_AXIS_COUNT: usize = 5;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityStructureSideV1 {
    Physbox,
    Authority,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityStructureAxisV1 {
    Preparation,
    Accumulation,
    Coefficients,
    BiasRelaxation,
    Restitution,
}

impl AuthorityStructureAxisV1 {
    pub const ALL: [Self; AUTHORITY_STRUCTURE_AXIS_COUNT] = [
        Self::Preparation,
        Self::Accumulation,
        Self::Coefficients,
        Self::BiasRelaxation,
        Self::Restitution,
    ];

    pub const fn number(self) -> u8 {
        match self {
            Self::Preparation => 1,
            Self::Accumulation => 2,
            Self::Coefficients => 3,
            Self::BiasRelaxation => 4,
            Self::Restitution => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityStructurePolicyV1 {
    pub version: u32,
    pub preparation: AuthorityStructureSideV1,
    pub accumulation: AuthorityStructureSideV1,
    pub coefficients: AuthorityStructureSideV1,
    pub bias_relaxation: AuthorityStructureSideV1,
    pub restitution: AuthorityStructureSideV1,
}

impl AuthorityStructurePolicyV1 {
    pub const fn physbox() -> Self {
        Self::uniform(AuthorityStructureSideV1::Physbox)
    }

    pub const fn authority() -> Self {
        Self::uniform(AuthorityStructureSideV1::Authority)
    }

    pub const fn p_plus(axis: AuthorityStructureAxisV1) -> Self {
        Self::physbox().with_axis(axis, AuthorityStructureSideV1::Authority)
    }

    pub const fn a_minus(axis: AuthorityStructureAxisV1) -> Self {
        Self::authority().with_axis(axis, AuthorityStructureSideV1::Physbox)
    }

    pub const fn interaction(
        first: AuthorityStructureAxisV1,
        second: AuthorityStructureAxisV1,
    ) -> Self {
        Self::p_plus(first).with_axis(second, AuthorityStructureSideV1::Authority)
    }

    pub fn validate(self) -> Result<(), ScalarError> {
        if self.version != AUTHORITY_STRUCTURE_POLICY_VERSION {
            return Err(ScalarError(format!(
                "authority structure policy version {} is unsupported",
                self.version
            )));
        }
        Ok(())
    }

    pub const fn side(self, axis: AuthorityStructureAxisV1) -> AuthorityStructureSideV1 {
        match axis {
            AuthorityStructureAxisV1::Preparation => self.preparation,
            AuthorityStructureAxisV1::Accumulation => self.accumulation,
            AuthorityStructureAxisV1::Coefficients => self.coefficients,
            AuthorityStructureAxisV1::BiasRelaxation => self.bias_relaxation,
            AuthorityStructureAxisV1::Restitution => self.restitution,
        }
    }

    const fn uniform(side: AuthorityStructureSideV1) -> Self {
        Self {
            version: AUTHORITY_STRUCTURE_POLICY_VERSION,
            preparation: side,
            accumulation: side,
            coefficients: side,
            bias_relaxation: side,
            restitution: side,
        }
    }

    const fn with_axis(
        mut self,
        axis: AuthorityStructureAxisV1,
        side: AuthorityStructureSideV1,
    ) -> Self {
        match axis {
            AuthorityStructureAxisV1::Preparation => self.preparation = side,
            AuthorityStructureAxisV1::Accumulation => self.accumulation = side,
            AuthorityStructureAxisV1::Coefficients => self.coefficients = side,
            AuthorityStructureAxisV1::BiasRelaxation => self.bias_relaxation = side,
            AuthorityStructureAxisV1::Restitution => self.restitution = side,
        }
        self
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityStructureConfigurationV1 {
    pub id: String,
    pub policy: AuthorityStructurePolicyV1,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityInteractionSlotV1 {
    pub id: String,
    pub policy: Option<AuthorityStructurePolicyV1>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum AuthorityStructurePassKindV1 {
    IntegrateVelocity,
    Prepare,
    WarmStart,
    BiasedSolve,
    IntegratePosition,
    Refresh,
    Relaxation,
    Restitution,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorityStructurePassStateV1 {
    pub substep: u32,
    pub kind: AuthorityStructurePassKindV1,
    pub bodies: Vec<BodyStateV1>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct BranchRobustAuthoritySolveResultV1 {
    pub candidate_id: &'static str,
    pub formula_version: u32,
    pub error_bound_version: u32,
    pub perturbation: BranchRobustPerturbationV1,
    pub statistics: BranchRobustRunStatisticsV1,
    pub solve: ScalarObservedSolveResultV1,
}

#[derive(Clone, Debug, PartialEq)]
pub struct PrecisionPathAuthoritySolveResultV1 {
    pub base_candidate_id: &'static str,
    pub perturbation: PrecisionPerturbationV1,
    pub amplitude: f64,
    pub statistics: PrecisionPathStatisticsV1,
    pub solve: ScalarObservedSolveResultV1,
}

pub fn mandatory_authority_structure_configurations_v1() -> Vec<AuthorityStructureConfigurationV1> {
    let mut configurations = vec![
        AuthorityStructureConfigurationV1 {
            id: "P".to_owned(),
            policy: AuthorityStructurePolicyV1::physbox(),
        },
        AuthorityStructureConfigurationV1 {
            id: "A".to_owned(),
            policy: AuthorityStructurePolicyV1::authority(),
        },
    ];
    for axis in AuthorityStructureAxisV1::ALL {
        configurations.push(AuthorityStructureConfigurationV1 {
            id: format!("P+X{}", axis.number()),
            policy: AuthorityStructurePolicyV1::p_plus(axis),
        });
        configurations.push(AuthorityStructureConfigurationV1 {
            id: format!("A-X{}", axis.number()),
            policy: AuthorityStructurePolicyV1::a_minus(axis),
        });
    }
    configurations
}

pub fn conditional_authority_interaction_slots_v1() -> Vec<AuthorityInteractionSlotV1> {
    (1..=3)
        .map(|number| AuthorityInteractionSlotV1 {
            id: format!("I{number}"),
            policy: None,
        })
        .collect()
}

/// Runs one S12 policy over the portable scalar projection.
///
/// The all-PhysBox policy intentionally repeats the existing loop's operation
/// order exactly. Authority-side preparation is once per outer step;
/// authority-side accumulation uses the pinned constraint-graph coloring;
/// authority-side restitution is deferred until the outer step ends.
pub fn solve_authority_structure_steps_observed(
    scenario: &ScenarioV1,
    policy: AuthorityStructurePolicyV1,
    retain_traces: bool,
) -> Result<ScalarObservedSolveResultV1, ScalarError> {
    solve_authority_structure_steps_observed_internal(
        scenario,
        policy,
        retain_traces,
        AuthoritySolveDiagnostics::default(),
    )
}

pub fn solve_branch_robust_authority_structure_steps_observed(
    scenario: &ScenarioV1,
    policy: AuthorityStructurePolicyV1,
    perturbation: BranchRobustPerturbationV1,
    retain_traces: bool,
) -> Result<BranchRobustAuthoritySolveResultV1, ScalarError> {
    if policy.preparation != AuthorityStructureSideV1::Authority {
        return Err(ScalarError(
            "branch-robust separation requires authority-side preparation".to_owned(),
        ));
    }
    let mut statistics = BranchRobustRunStatisticsV1::default();
    let solve = solve_authority_structure_steps_observed_internal(
        scenario,
        policy,
        retain_traces,
        AuthoritySolveDiagnostics {
            branch_robust_perturbation: Some(perturbation),
            branch_robust_statistics: Some(&mut statistics),
            ..AuthoritySolveDiagnostics::default()
        },
    )?;
    Ok(BranchRobustAuthoritySolveResultV1 {
        candidate_id: BRANCH_ROBUST_CANDIDATE_ID,
        formula_version: BRANCH_ROBUST_FORMULA_VERSION,
        error_bound_version: BRANCH_ROBUST_ERROR_BOUND_VERSION,
        perturbation,
        statistics,
        solve,
    })
}

pub fn solve_precision_path_authority_structure_steps_observed(
    scenario: &ScenarioV1,
    policy: AuthorityStructurePolicyV1,
    perturbation: PrecisionPerturbationV1,
    amplitude: f64,
    retain_traces: bool,
) -> Result<PrecisionPathAuthoritySolveResultV1, ScalarError> {
    if policy.preparation != AuthorityStructureSideV1::Authority {
        return Err(ScalarError(
            "S15 precision path requires authority-side preparation".to_owned(),
        ));
    }
    if !amplitude.is_finite() || amplitude < 0.0 {
        return Err(ScalarError(
            "S15 precision amplitude must be finite and non-negative".to_owned(),
        ));
    }
    let mut statistics = PrecisionPathStatisticsV1::default();
    let solve = solve_authority_structure_steps_observed_internal(
        scenario,
        policy,
        retain_traces,
        AuthoritySolveDiagnostics {
            precision_perturbation: Some(perturbation),
            precision_amplitude: amplitude,
            precision_statistics: Some(&mut statistics),
            ..AuthoritySolveDiagnostics::default()
        },
    )?;
    Ok(PrecisionPathAuthoritySolveResultV1 {
        base_candidate_id: crate::precision_margin::PRECISION_BASE_CANDIDATE_ID,
        perturbation,
        amplitude,
        statistics,
        solve,
    })
}

pub fn solve_authority_structure_one_step_pass_states(
    scenario: &ScenarioV1,
    policy: AuthorityStructurePolicyV1,
) -> Result<
    (
        ScalarObservedSolveResultV1,
        Vec<AuthorityStructurePassStateV1>,
    ),
    ScalarError,
> {
    if scenario.requested_steps != 1 {
        return Err(ScalarError(
            "authority pass-state diagnostic requires exactly one outer step".to_owned(),
        ));
    }
    let mut pass_states = Vec::new();
    let result = solve_authority_structure_steps_observed_internal(
        scenario,
        policy,
        true,
        AuthoritySolveDiagnostics {
            pass_states: Some(&mut pass_states),
            ..AuthoritySolveDiagnostics::default()
        },
    )?;
    Ok((result, pass_states))
}

#[derive(Default)]
struct AuthoritySolveDiagnostics<'a> {
    pass_states: Option<&'a mut Vec<AuthorityStructurePassStateV1>>,
    branch_robust_perturbation: Option<BranchRobustPerturbationV1>,
    branch_robust_statistics: Option<&'a mut BranchRobustRunStatisticsV1>,
    precision_perturbation: Option<PrecisionPerturbationV1>,
    precision_amplitude: f64,
    precision_statistics: Option<&'a mut PrecisionPathStatisticsV1>,
}

fn solve_authority_structure_steps_observed_internal(
    scenario: &ScenarioV1,
    policy: AuthorityStructurePolicyV1,
    retain_traces: bool,
    mut diagnostics: AuthoritySolveDiagnostics<'_>,
) -> Result<ScalarObservedSolveResultV1, ScalarError> {
    policy.validate()?;
    scenario
        .validate()
        .map_err(|error| ScalarError(error.to_string()))?;
    if !scenario.manifolds.is_empty() {
        return Err(ScalarError(
            "authority structure bridge requires generated contacts".to_owned(),
        ));
    }
    if !scenario.actions.is_empty() {
        return Err(ScalarError(
            "authority structure bridge does not support actions".to_owned(),
        ));
    }

    let contact_softness = selected_contact_softness(scenario, policy)?;
    let biased_iterations = if policy.bias_relaxation == AuthorityStructureSideV1::Authority {
        1
    } else {
        scenario.settings.iterations
    };
    let substep_dt = scenario.settings.fixed_dt.get() / scenario.settings.substeps as f32;
    let mut bodies = scenario.bodies.clone();
    let mut contact_cache = scenario.contact_cache.clone();
    let mut traces = Vec::new();
    let mut observations = Vec::with_capacity(scenario.requested_steps as usize);

    for step in 0..scenario.requested_steps {
        let outer_step_cache = contact_cache.clone();
        let mut fixed_contacts: Option<Vec<PreparedContact>> = None;
        let mut refreshed_contacts = Vec::new();
        for substep in 0..scenario.settings.substeps {
            let trace_start = traces.len();
            if policy.preparation == AuthorityStructureSideV1::Authority {
                if fixed_contacts.is_none() {
                    let mut contacts =
                        prepare_contacts(scenario, &bodies, &contact_cache, &mut traces, true)?;
                    if policy.accumulation == AuthorityStructureSideV1::Authority {
                        order_authority_contact_points(&mut contacts);
                    }
                    fixed_contacts = Some(contacts);
                }
            } else {
                refreshed_contacts =
                    prepare_contacts(scenario, &bodies, &outer_step_cache, &mut traces, false)?;
                if policy.accumulation == AuthorityStructureSideV1::Authority {
                    order_authority_contact_points(&mut refreshed_contacts);
                }
            }
            let contacts = if let Some(contacts) = fixed_contacts.as_mut() {
                contacts
            } else {
                &mut refreshed_contacts
            };
            let visit_order = contact_visit_order(contacts, &bodies, policy.accumulation);

            integrate_velocities(&mut bodies, scenario.settings.gravity, substep_dt)?;
            record_pass_state(
                &mut diagnostics.pass_states,
                substep,
                AuthorityStructurePassKindV1::IntegrateVelocity,
                &bodies,
            );
            for &contact_index in &visit_order {
                warm_start(&contacts[contact_index], &mut bodies, &mut traces)?;
            }
            record_pass_state(
                &mut diagnostics.pass_states,
                substep,
                AuthorityStructurePassKindV1::WarmStart,
                &bodies,
            );
            for iteration in 0..biased_iterations {
                for &contact_index in &visit_order {
                    solve_normal(
                        &mut contacts[contact_index],
                        &mut bodies,
                        substep_dt,
                        scenario.settings.bias_rate.get(),
                        maximum_depenetration_velocity(scenario),
                        scenario.settings.mass_coefficient.get(),
                        scenario.settings.impulse_coefficient.get(),
                        contact_softness,
                        true,
                        iteration,
                        TraceStageV1::NormalSolve,
                        &mut traces,
                    )?;
                    solve_tangent(
                        &mut contacts[contact_index],
                        &mut bodies,
                        iteration,
                        TraceStageV1::TangentSolve,
                        &mut traces,
                    )?;
                }
            }
            record_pass_state(
                &mut diagnostics.pass_states,
                substep,
                AuthorityStructurePassKindV1::BiasedSolve,
                &bodies,
            );

            if policy.preparation == AuthorityStructureSideV1::Authority {
                integrate_positions_authority(&mut bodies, substep_dt, &mut traces)?;
            } else {
                integrate_positions(&mut bodies, substep_dt, &mut traces)?;
            }
            record_pass_state(
                &mut diagnostics.pass_states,
                substep,
                AuthorityStructurePassKindV1::IntegratePosition,
                &bodies,
            );
            if policy.preparation == AuthorityStructureSideV1::Authority {
                if let (Some(perturbation), Some(statistics)) = (
                    diagnostics.precision_perturbation,
                    diagnostics.precision_statistics.as_deref_mut(),
                ) {
                    refresh_precision_path_authority_separations(
                        contacts,
                        &bodies,
                        step,
                        substep,
                        perturbation,
                        diagnostics.precision_amplitude,
                        statistics,
                    );
                } else if let (Some(perturbation), Some(statistics)) = (
                    diagnostics.branch_robust_perturbation,
                    diagnostics.branch_robust_statistics.as_deref_mut(),
                ) {
                    refresh_branch_robust_authority_separations(
                        contacts,
                        &bodies,
                        step,
                        substep,
                        perturbation,
                        statistics,
                    );
                } else {
                    refresh_authority_separations(contacts, &bodies);
                }
            } else {
                refresh_separations(contacts, &bodies);
            }
            record_pass_state(
                &mut diagnostics.pass_states,
                substep,
                AuthorityStructurePassKindV1::Refresh,
                &bodies,
            );
            for &contact_index in &visit_order {
                solve_normal(
                    &mut contacts[contact_index],
                    &mut bodies,
                    substep_dt,
                    scenario.settings.bias_rate.get(),
                    maximum_depenetration_velocity(scenario),
                    scenario.settings.mass_coefficient.get(),
                    scenario.settings.impulse_coefficient.get(),
                    contact_softness,
                    false,
                    0,
                    TraceStageV1::Relaxation,
                    &mut traces,
                )?;
                solve_tangent(
                    &mut contacts[contact_index],
                    &mut bodies,
                    0,
                    TraceStageV1::Relaxation,
                    &mut traces,
                )?;
                if policy.restitution == AuthorityStructureSideV1::Physbox {
                    apply_restitution(
                        &mut contacts[contact_index],
                        &mut bodies,
                        scenario.settings.restitution_threshold.get(),
                        &mut traces,
                    )?;
                }
            }
            record_pass_state(
                &mut diagnostics.pass_states,
                substep,
                AuthorityStructurePassKindV1::Relaxation,
                &bodies,
            );
            if policy.restitution == AuthorityStructureSideV1::Physbox {
                record_pass_state(
                    &mut diagnostics.pass_states,
                    substep,
                    AuthorityStructurePassKindV1::Restitution,
                    &bodies,
                );
            }
            contact_cache = collect_contact_cache(contacts, &bodies)?;
            assign_trace_location(&mut traces[trace_start..], step, substep);
        }

        if policy.restitution == AuthorityStructureSideV1::Authority {
            let trace_start = traces.len();
            let contacts = if let Some(contacts) = fixed_contacts.as_mut() {
                contacts
            } else {
                &mut refreshed_contacts
            };
            let visit_order = contact_visit_order(contacts, &bodies, policy.accumulation);
            for contact_index in visit_order {
                apply_restitution(
                    &mut contacts[contact_index],
                    &mut bodies,
                    scenario.settings.restitution_threshold.get(),
                    &mut traces,
                )?;
            }
            contact_cache = collect_contact_cache(contacts, &bodies)?;
            assign_trace_location(
                &mut traces[trace_start..],
                step,
                scenario.settings.substeps - 1,
            );
        }

        observations.push(observe_step(scenario, step + 1, &bodies, &contact_cache)?);
        if !retain_traces {
            traces.clear();
        }
    }

    Ok(ScalarObservedSolveResultV1 {
        result: ScalarSolveResult {
            bodies,
            contact_cache,
            traces,
        },
        observations,
    })
}

fn record_pass_state(
    output: &mut Option<&mut Vec<AuthorityStructurePassStateV1>>,
    substep: u32,
    kind: AuthorityStructurePassKindV1,
    bodies: &[BodyStateV1],
) {
    if let Some(output) = output.as_deref_mut() {
        output.push(AuthorityStructurePassStateV1 {
            substep,
            kind,
            bodies: bodies.to_vec(),
        });
    }
}

fn prepare_contacts(
    scenario: &ScenarioV1,
    bodies: &[BodyStateV1],
    cache: &[ContactCachePointV1],
    traces: &mut Vec<TraceRecordV1>,
    authority_identity: bool,
) -> Result<Vec<PreparedContact>, ScalarError> {
    let mut manifolds = generate_portable_contacts(scenario, bodies)?;
    if authority_identity {
        canonicalize_authority_features(&mut manifolds, bodies)?;
    }
    manifolds
        .iter()
        .map(|manifold| prepare_contact(bodies, manifold, cache, traces))
        .collect()
}

fn canonicalize_authority_features(
    manifolds: &mut [AuthoredManifoldV1],
    bodies: &[BodyStateV1],
) -> Result<(), ScalarError> {
    for manifold in manifolds {
        let body_a = bodies
            .iter()
            .find(|body| body.id == manifold.body_a)
            .ok_or_else(|| ScalarError("authority manifold body A is missing".to_owned()))?;
        let body_b = bodies
            .iter()
            .find(|body| body.id == manifold.body_b)
            .ok_or_else(|| ScalarError("authority manifold body B is missing".to_owned()))?;
        let mut authority_order = (0..manifold.points.len()).collect::<Vec<_>>();
        let tangent = Vec2::new(manifold.normal.y, -manifold.normal.x);
        authority_order.sort_by(|&left, &right| {
            let left_coordinate = dot(manifold.points[left].world_point, tangent);
            let right_coordinate = dot(manifold.points[right].world_point, tangent);
            right_coordinate.total_cmp(&left_coordinate)
        });
        let reference_a = manifold.points.len() == 2
            && manifold
                .points
                .iter()
                .all(|point| point.feature_a.feature_type == 1);
        let reference_b = manifold.points.len() == 2
            && manifold
                .points
                .iter()
                .all(|point| point.feature_b.feature_type == 1);
        for (rank, &point_index) in authority_order.iter().enumerate() {
            let point = &mut manifold.points[point_index];
            if reference_a {
                let reference_face = point.feature_a.feature_index;
                point.feature_a = authority_vertex(if rank == 0 {
                    reference_face
                } else {
                    (reference_face + 1) % 4
                });
                point.feature_b = authority_vertex(point.feature_b.feature_index);
            } else if reference_b {
                let reference_face = point.feature_b.feature_index;
                point.feature_a = authority_vertex(point.feature_a.feature_index);
                point.feature_b = authority_vertex(if rank == 0 {
                    (reference_face + 1) % 4
                } else {
                    reference_face
                });
            } else {
                point.feature_a = authority_feature(body_a.shape, point.local_anchor_a);
                point.feature_b = authority_feature(body_b.shape, point.local_anchor_b);
            }
            point.local_anchor_a = inverse_rotate(
                Vec2::new(
                    point.world_point.x - body_a.position.x,
                    point.world_point.y - body_a.position.y,
                ),
                body_a.angle.get(),
            );
            point.local_anchor_b = inverse_rotate(
                Vec2::new(
                    point.world_point.x - body_b.position.x,
                    point.world_point.y - body_b.position.y,
                ),
                body_b.angle.get(),
            );
        }
    }
    Ok(())
}

fn authority_vertex(feature_index: u32) -> FeatureIdV1 {
    FeatureIdV1 {
        feature_type: 3,
        feature_index,
    }
}

fn authority_feature(shape: ShapeV1, local_anchor: Vec2) -> FeatureIdV1 {
    let feature_index = match shape {
        ShapeV1::Circle { .. } => 0,
        ShapeV1::Rectangle { half_extents } => [
            Vec2::new(-half_extents.x, -half_extents.y),
            Vec2::new(half_extents.x, -half_extents.y),
            Vec2::new(half_extents.x, half_extents.y),
            Vec2::new(-half_extents.x, half_extents.y),
        ]
        .into_iter()
        .enumerate()
        .min_by(|(_, left), (_, right)| {
            squared_distance(*left, local_anchor).total_cmp(&squared_distance(*right, local_anchor))
        })
        .map_or(0, |(index, _)| index as u32),
    };
    authority_vertex(feature_index)
}

fn dot(left: Vec2, right: Vec2) -> f32 {
    left.x * right.x + left.y * right.y
}

fn squared_distance(left: Vec2, right: Vec2) -> f32 {
    let x = left.x - right.x;
    let y = left.y - right.y;
    x * x + y * y
}

fn inverse_rotate(vector: Vec2, angle: f32) -> Vec2 {
    let (sine, cosine) = (-angle).sin_cos();
    Vec2::new(
        cosine * vector.x - sine * vector.y,
        sine * vector.x + cosine * vector.y,
    )
}

fn selected_contact_softness(
    scenario: &ScenarioV1,
    policy: AuthorityStructurePolicyV1,
) -> Result<Option<ContactSoftnessPolicyV1>, ScalarError> {
    let Some(mut softness) = scenario.settings.contact_softness else {
        return Ok(None);
    };
    if policy.coefficients == AuthorityStructureSideV1::Authority {
        softness.static_contact_hertz_multiplier =
            FiniteF32::new(2.0).map_err(|error| ScalarError(error.to_string()))?;
    }
    Ok(Some(softness))
}

fn contact_visit_order(
    contacts: &[PreparedContact],
    bodies: &[BodyStateV1],
    side: AuthorityStructureSideV1,
) -> Vec<usize> {
    if side == AuthorityStructureSideV1::Physbox {
        return (0..contacts.len()).collect();
    }

    // Box2D has 20 dynamic colors, three non-overflow static-capable colors,
    // and one overflow color. Dynamic pairs take the lowest free dynamic
    // color. Static pairs take the highest free non-overflow color. The solver
    // visits overflow first and then active colors in ascending order.
    const DYNAMIC_COLOR_COUNT: usize = 20;
    const NON_OVERFLOW_COLOR_COUNT: usize = 23;
    let mut colors = vec![Vec::new(); NON_OVERFLOW_COLOR_COUNT];
    let mut occupied = vec![vec![false; bodies.len()]; NON_OVERFLOW_COLOR_COUNT];
    let mut overflow = Vec::new();

    for (contact_index, contact) in contacts.iter().enumerate() {
        let dynamic_a =
            (bodies[contact.body_a].inverse_mass.get() != 0.0).then_some(contact.body_a);
        let dynamic_b =
            (bodies[contact.body_b].inverse_mass.get() != 0.0).then_some(contact.body_b);
        let dynamic_bodies = [dynamic_a, dynamic_b];
        let selected = if dynamic_a.is_some() && dynamic_b.is_some() {
            (0..DYNAMIC_COLOR_COUNT).find(|&color| {
                dynamic_bodies
                    .iter()
                    .flatten()
                    .all(|&body| !occupied[color][body])
            })
        } else {
            (1..NON_OVERFLOW_COLOR_COUNT).rev().find(|&color| {
                dynamic_bodies
                    .iter()
                    .flatten()
                    .all(|&body| !occupied[color][body])
            })
        };
        if let Some(color) = selected {
            for &body in dynamic_bodies.iter().flatten() {
                occupied[color][body] = true;
            }
            colors[color].push(contact_index);
        } else {
            overflow.push(contact_index);
        }
    }

    overflow
        .into_iter()
        .chain(colors.into_iter().flatten())
        .collect()
}

fn observe_step(
    scenario: &ScenarioV1,
    completed_steps: u32,
    bodies: &[BodyStateV1],
    contact_cache: &[ContactCachePointV1],
) -> Result<ScalarStepObservationV1, ScalarError> {
    let final_manifolds = generate_portable_contacts(scenario, bodies)?;
    let mut maximum_penetration = 0.0_f32;
    let mut maximum_separation = 0.0_f32;
    for point in final_manifolds
        .iter()
        .flat_map(|manifold| manifold.points.iter())
    {
        maximum_penetration = maximum_penetration.max((-point.separation.get()).max(0.0));
        maximum_separation = maximum_separation.max(point.separation.get().max(0.0));
    }
    Ok(ScalarStepObservationV1 {
        completed_steps,
        bodies: bodies.to_vec(),
        contact_cache: contact_cache.to_vec(),
        active_normal_points: contact_cache
            .iter()
            .filter(|point| point.normal_impulse.get() > 0.0)
            .count() as u32,
        total_normal_impulse: contact_cache
            .iter()
            .map(|point| f64::from(point.normal_impulse.get()))
            .sum(),
        maximum_penetration,
        maximum_separation,
    })
}

fn assign_trace_location(traces: &mut [TraceRecordV1], step: u32, substep: u32) {
    for trace in traces {
        trace.step = step;
        trace.substep = substep;
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use physbox_engine::p5_2_centered_rectangle_stack_scenario;

    use super::*;
    use crate::scalar::{
        ScalarSubstepImpulsePolicyV1,
        solve_portable_narrowphase_steps_observed_with_substep_impulses,
    };

    #[test]
    fn frozen_matrix_has_two_endpoints_ten_ablations_and_three_slots() {
        let configurations = mandatory_authority_structure_configurations_v1();
        assert_eq!(configurations.len(), 12);
        assert_eq!(configurations[0].id, "P");
        assert_eq!(configurations[1].id, "A");
        assert_eq!(
            configurations
                .iter()
                .map(|configuration| configuration.id.as_str())
                .collect::<BTreeSet<_>>()
                .len(),
            12
        );
        for axis in AuthorityStructureAxisV1::ALL {
            let p_plus = AuthorityStructurePolicyV1::p_plus(axis);
            let a_minus = AuthorityStructurePolicyV1::a_minus(axis);
            assert_eq!(p_plus.side(axis), AuthorityStructureSideV1::Authority);
            assert_eq!(a_minus.side(axis), AuthorityStructureSideV1::Physbox);
            for other in AuthorityStructureAxisV1::ALL {
                if other != axis {
                    assert_eq!(p_plus.side(other), AuthorityStructureSideV1::Physbox);
                    assert_eq!(a_minus.side(other), AuthorityStructureSideV1::Authority);
                }
            }
        }
        assert_eq!(
            conditional_authority_interaction_slots_v1()
                .iter()
                .map(|slot| slot.id.as_str())
                .collect::<Vec<_>>(),
            ["I1", "I2", "I3"]
        );
    }

    #[test]
    fn unknown_policy_version_fails_closed() {
        let mut policy = AuthorityStructurePolicyV1::physbox();
        policy.version += 1;
        assert!(policy.validate().is_err());
    }

    #[test]
    fn physbox_bridge_is_exact_on_a_portable_stack() {
        let mut scenario = p5_2_centered_rectangle_stack_scenario(4, 8).unwrap();
        scenario.settings.contact_softness = Some(ContactSoftnessPolicyV1 {
            version: 1,
            contact_hertz: FiniteF32::new(30.0).unwrap(),
            damping_ratio: FiniteF32::new(10.0).unwrap(),
            static_contact_hertz_multiplier: FiniteF32::new(1.0).unwrap(),
        });
        let expected = solve_portable_narrowphase_steps_observed_with_substep_impulses(
            &scenario,
            ScalarSubstepImpulsePolicyV1::OuterStepStaleReseed,
            true,
        )
        .unwrap();
        let actual = solve_authority_structure_steps_observed(
            &scenario,
            AuthorityStructurePolicyV1::physbox(),
            true,
        )
        .unwrap();
        assert_eq!(actual, expected);
    }

    #[test]
    fn authority_bridge_replays_exactly() {
        let mut scenario = p5_2_centered_rectangle_stack_scenario(4, 8).unwrap();
        scenario.settings.contact_softness = Some(ContactSoftnessPolicyV1 {
            version: 1,
            contact_hertz: FiniteF32::new(30.0).unwrap(),
            damping_ratio: FiniteF32::new(10.0).unwrap(),
            static_contact_hertz_multiplier: FiniteF32::new(1.0).unwrap(),
        });
        let first = solve_authority_structure_steps_observed(
            &scenario,
            AuthorityStructurePolicyV1::authority(),
            true,
        )
        .unwrap();
        let second = solve_authority_structure_steps_observed(
            &scenario,
            AuthorityStructurePolicyV1::authority(),
            true,
        )
        .unwrap();
        assert_eq!(first, second);
    }

    #[test]
    fn branch_robust_lanes_replay_and_remain_finite_locally() {
        let mut scenario = p5_2_centered_rectangle_stack_scenario(2, 2).unwrap();
        scenario.settings.contact_softness = Some(ContactSoftnessPolicyV1 {
            version: 1,
            contact_hertz: FiniteF32::new(30.0).unwrap(),
            damping_ratio: FiniteF32::new(10.0).unwrap(),
            static_contact_hertz_multiplier: FiniteF32::new(1.0).unwrap(),
        });
        let policy = AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution);
        for lane in BranchRobustPerturbationV1::ALL {
            let first = solve_branch_robust_authority_structure_steps_observed(
                &scenario, policy, lane, true,
            )
            .unwrap();
            let second = solve_branch_robust_authority_structure_steps_observed(
                &scenario, policy, lane, true,
            )
            .unwrap();
            assert_eq!(first, second);
            assert!(first.statistics.evaluations > 0);
            assert_eq!(first.statistics.bound_violations, 0);
            assert_eq!(first.statistics.non_finite_results, 0);
            assert_eq!(first.statistics.normal_range_violations, 0);
            assert!(
                first
                    .solve
                    .result
                    .bodies
                    .iter()
                    .all(|body| body.validate().is_ok())
            );
        }
    }

    #[test]
    fn branch_robust_rejects_non_authority_preparation() {
        let scenario = p5_2_centered_rectangle_stack_scenario(2, 1).unwrap();
        assert!(
            solve_branch_robust_authority_structure_steps_observed(
                &scenario,
                AuthorityStructurePolicyV1::physbox(),
                BranchRobustPerturbationV1::Nearest,
                false,
            )
            .is_err()
        );
    }
}
