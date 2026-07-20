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
use crate::event_structure::{
    EventFamilyV1, EventGraphClassV1, EventLaneV1, EventNormalOrientationV1, EventPairV1,
    EventPointIdentityV1, EventStreamV1, NormalDecisionEventV1, NormalPassV1, NormalRouteChoiceV1,
    PreparedContactEventV1, PreparedPointEventV1, reference_owner_and_face,
};
use crate::precision_margin::{PrecisionPathStatisticsV1, PrecisionPerturbationV1};
use crate::scalar::{
    PreparedContact, ScalarError, ScalarObservedSolveResultV1, ScalarSolveResult,
    ScalarStepObservationV1, apply_restitution, collect_contact_cache, generate_portable_contacts,
    integrate_positions, integrate_positions_authority, integrate_velocities,
    maximum_depenetration_velocity, order_authority_contact_points, prepare_contact,
    refresh_authority_separations, refresh_branch_robust_authority_separations,
    refresh_precision_path_authority_separations, refresh_separations, solve_normal,
    solve_normal_observed, solve_normal_routed, solve_tangent, warm_start,
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

#[derive(Clone, Debug, PartialEq)]
pub struct AuthorityEventSolveResultV1 {
    pub solve: ScalarObservedSolveResultV1,
    pub precision_statistics: Option<PrecisionPathStatisticsV1>,
    pub events: EventStreamV1,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RoutedAuthorityEventSolveResultV1 {
    pub solve: ScalarObservedSolveResultV1,
    pub events: EventStreamV1,
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

pub fn solve_authority_event_stream_v1(
    scenario: &ScenarioV1,
    lane: EventLaneV1,
) -> Result<AuthorityEventSolveResultV1, ScalarError> {
    if lane == EventLaneV1::FullSolveF64 {
        return Err(ScalarError(
            "full-solve-f64 events are owned by precision_margin".to_owned(),
        ));
    }
    let policy = AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution);
    let mut events = EventStreamV1::new(lane);
    let mut statistics = PrecisionPathStatisticsV1::default();
    let precision =
        (lane == EventLaneV1::CancellationPathF64).then_some(PrecisionPerturbationV1::Nearest);
    let solve = solve_authority_structure_steps_observed_internal(
        scenario,
        policy,
        false,
        AuthoritySolveDiagnostics {
            precision_perturbation: precision,
            precision_statistics: precision.map(|_| &mut statistics),
            event_stream: Some(&mut events),
            ..AuthoritySolveDiagnostics::default()
        },
    )?;
    Ok(AuthorityEventSolveResultV1 {
        solve,
        precision_statistics: precision.map(|_| statistics),
        events,
    })
}

pub fn solve_routed_authority_event_stream_v1(
    scenario: &ScenarioV1,
    owner: &EventStreamV1,
    families: &[EventFamilyV1],
) -> Result<RoutedAuthorityEventSolveResultV1, ScalarError> {
    if owner.lane != EventLaneV1::FullSolveF64 {
        return Err(ScalarError(
            "routed exact-s12-f32 requires the immutable full-solve-f64 owner stream".to_owned(),
        ));
    }
    if families.is_empty()
        || families
            .iter()
            .any(|family| !EventFamilyV1::ALL.contains(family))
    {
        return Err(ScalarError(
            "routed exact-s12-f32 requires one or more declared event families".to_owned(),
        ));
    }
    let policy = AuthorityStructurePolicyV1::a_minus(AuthorityStructureAxisV1::Restitution);
    let mut events = EventStreamV1::new(EventLaneV1::ExactS12F32);
    let mut route = EventRouteState::new(owner, families);
    let solve = solve_authority_structure_steps_observed_internal(
        scenario,
        policy,
        false,
        AuthoritySolveDiagnostics {
            event_stream: Some(&mut events),
            event_route: Some(&mut route),
            ..AuthoritySolveDiagnostics::default()
        },
    )?;
    route.finish()?;
    Ok(RoutedAuthorityEventSolveResultV1 { solve, events })
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

struct EventRouteState<'a> {
    owner: &'a EventStreamV1,
    families: Vec<EventFamilyV1>,
    normal_cursor: usize,
}

impl<'a> EventRouteState<'a> {
    fn new(owner: &'a EventStreamV1, families: &[EventFamilyV1]) -> Self {
        let mut families = families.to_vec();
        families.sort();
        families.dedup();
        Self {
            owner,
            families,
            normal_cursor: 0,
        }
    }

    fn routes(&self, family: EventFamilyV1) -> bool {
        self.families.binary_search(&family).is_ok()
    }

    fn routes_normal_decisions(&self) -> bool {
        self.routes(EventFamilyV1::SeparationPositiveAndSpeculativeBranch)
            || self.routes(EventFamilyV1::UnilateralActiveSetProjection)
    }

    fn finish(&self) -> Result<(), ScalarError> {
        if self.routes_normal_decisions() && self.normal_cursor != self.owner.normal_decisions.len()
        {
            return Err(owner_choice_unavailable(
                self.normal_family(),
                0,
                "receiving lane ended before consuming the immutable owner normal stream",
            ));
        }
        Ok(())
    }

    fn normal_family(&self) -> EventFamilyV1 {
        if self.routes(EventFamilyV1::SeparationPositiveAndSpeculativeBranch) {
            EventFamilyV1::SeparationPositiveAndSpeculativeBranch
        } else {
            EventFamilyV1::UnilateralActiveSetProjection
        }
    }
}

fn owner_choice_unavailable(
    family: EventFamilyV1,
    completed_step: u32,
    reason: &str,
) -> ScalarError {
    ScalarError(format!(
        "owner-choice-unavailable: family={family:?} completed_step={completed_step} reason={reason}"
    ))
}

#[derive(Default)]
struct AuthoritySolveDiagnostics<'a, 'owner> {
    pass_states: Option<&'a mut Vec<AuthorityStructurePassStateV1>>,
    branch_robust_perturbation: Option<BranchRobustPerturbationV1>,
    branch_robust_statistics: Option<&'a mut BranchRobustRunStatisticsV1>,
    precision_perturbation: Option<PrecisionPerturbationV1>,
    precision_amplitude: f64,
    precision_statistics: Option<&'a mut PrecisionPathStatisticsV1>,
    event_stream: Option<&'a mut EventStreamV1>,
    event_route: Option<&'a mut EventRouteState<'owner>>,
}

fn solve_authority_structure_steps_observed_internal(
    scenario: &ScenarioV1,
    policy: AuthorityStructurePolicyV1,
    retain_traces: bool,
    mut diagnostics: AuthoritySolveDiagnostics<'_, '_>,
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
            if substep == 0
                && let Some(route) = diagnostics.event_route.as_deref_mut()
            {
                route_prepared_contacts(route, step + 1, contacts, &bodies)?;
            }
            let mut visit_schedule = contact_visit_schedule(contacts, &bodies, policy.accumulation);
            if let Some(route) = diagnostics.event_route.as_deref_mut() {
                route_visit_schedule(route, step + 1, contacts, &bodies, &mut visit_schedule)?;
            }
            let visit_order = visit_schedule
                .iter()
                .map(|entry| entry.contact_index)
                .collect::<Vec<_>>();
            if substep == 0
                && let Some(events) = diagnostics.event_stream.as_deref_mut()
            {
                record_prepared_events(events, step + 1, contacts, &bodies, &visit_schedule);
            }

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
                for (visit_rank, &contact_index) in visit_order.iter().enumerate() {
                    if let Some(route) = diagnostics.event_route.as_deref_mut() {
                        let pair = contact_pair(&contacts[contact_index], &bodies);
                        let choices = routed_normal_choices(
                            route,
                            step + 1,
                            substep,
                            NormalPassV1::Biased,
                            visit_rank as u32,
                            pair,
                            &contacts[contact_index],
                        )?;
                        let mut decisions = Vec::new();
                        solve_normal_routed(
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
                            &mut decisions,
                            &choices,
                        )?;
                        if let Some(events) = diagnostics.event_stream.as_deref_mut() {
                            record_normal_events(
                                events,
                                step + 1,
                                substep,
                                NormalPassV1::Biased,
                                visit_rank as u32,
                                pair,
                                decisions,
                            );
                        }
                    } else if let Some(events) = diagnostics.event_stream.as_deref_mut() {
                        let pair = contact_pair(&contacts[contact_index], &bodies);
                        let mut decisions = Vec::new();
                        solve_normal_observed(
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
                            &mut decisions,
                        )?;
                        record_normal_events(
                            events,
                            step + 1,
                            substep,
                            NormalPassV1::Biased,
                            visit_rank as u32,
                            pair,
                            decisions,
                        );
                    } else {
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
                    }
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
            for (visit_rank, &contact_index) in visit_order.iter().enumerate() {
                if let Some(route) = diagnostics.event_route.as_deref_mut() {
                    let pair = contact_pair(&contacts[contact_index], &bodies);
                    let choices = routed_normal_choices(
                        route,
                        step + 1,
                        substep,
                        NormalPassV1::Relaxation,
                        visit_rank as u32,
                        pair,
                        &contacts[contact_index],
                    )?;
                    let mut decisions = Vec::new();
                    solve_normal_routed(
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
                        &mut decisions,
                        &choices,
                    )?;
                    if let Some(events) = diagnostics.event_stream.as_deref_mut() {
                        record_normal_events(
                            events,
                            step + 1,
                            substep,
                            NormalPassV1::Relaxation,
                            visit_rank as u32,
                            pair,
                            decisions,
                        );
                    }
                } else if let Some(events) = diagnostics.event_stream.as_deref_mut() {
                    let pair = contact_pair(&contacts[contact_index], &bodies);
                    let mut decisions = Vec::new();
                    solve_normal_observed(
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
                        &mut decisions,
                    )?;
                    record_normal_events(
                        events,
                        step + 1,
                        substep,
                        NormalPassV1::Relaxation,
                        visit_rank as u32,
                        pair,
                        decisions,
                    );
                } else {
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
                }
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

fn contact_pair(contact: &PreparedContact, bodies: &[BodyStateV1]) -> EventPairV1 {
    EventPairV1 {
        body_a: bodies[contact.body_a].id,
        body_b: bodies[contact.body_b].id,
    }
}

fn record_prepared_events(
    events: &mut EventStreamV1,
    completed_step: u32,
    contacts: &[PreparedContact],
    bodies: &[BodyStateV1],
    schedule: &[ContactVisitScheduleEntry],
) {
    for (preparation_ordinal, contact) in contacts.iter().enumerate() {
        let (visit_rank, graph_class) = schedule
            .iter()
            .enumerate()
            .find(|(_, entry)| entry.contact_index == preparation_ordinal)
            .map_or(
                (u32::MAX, EventGraphClassV1::Unavailable),
                |(rank, entry)| (rank as u32, entry.graph_class),
            );
        events.prepared_contacts.push(PreparedContactEventV1 {
            completed_step,
            preparation_ordinal: preparation_ordinal as u32,
            pair: contact_pair(contact, bodies),
            admitted: true,
            point_count: contact.points.len() as u32,
            reference_owner: contact.event_reference_owner,
            reference_face: contact.event_reference_face,
            normal_orientation: EventNormalOrientationV1::BodyAToBodyB,
            graph_class,
            visit_rank,
            points: contact
                .points
                .iter()
                .enumerate()
                .map(|(point_ordinal, point)| {
                    let identity = EventPointIdentityV1 {
                        feature_a: point.feature_a,
                        feature_b: point.feature_b,
                    };
                    PreparedPointEventV1 {
                        point_ordinal: point_ordinal as u32,
                        identity,
                        cache_hit: point.cache_hit,
                        cache_owner: point.cache_hit.then_some(identity),
                        normal_impulse: f64::from(point.normal_impulse),
                        tangent_impulse: f64::from(point.tangent_impulse),
                    }
                })
                .collect(),
        });
    }
}

fn record_normal_events(
    events: &mut EventStreamV1,
    completed_step: u32,
    substep: u32,
    pass: NormalPassV1,
    visit_rank: u32,
    pair: EventPairV1,
    decisions: Vec<crate::event_structure::NormalDecisionRawV1>,
) {
    events
        .normal_decisions
        .extend(decisions.into_iter().map(|decision| NormalDecisionEventV1 {
            completed_step,
            substep,
            pass,
            visit_rank,
            pair,
            point_ordinal: decision.point_ordinal,
            identity: decision.identity,
            separation_positive: decision.separation_positive,
            candidate_selected: decision.candidate_selected,
            separation: decision.separation,
            old_impulse: decision.old_impulse,
            unclamped_impulse: decision.unclamped_impulse,
            new_impulse: decision.new_impulse,
            applied_impulse: decision.applied_impulse,
        }));
}

fn route_prepared_contacts(
    route: &EventRouteState<'_>,
    completed_step: u32,
    contacts: &mut Vec<PreparedContact>,
    bodies: &[BodyStateV1],
) -> Result<(), ScalarError> {
    let owner = route
        .owner
        .prepared_contacts
        .iter()
        .filter(|event| event.completed_step == completed_step)
        .collect::<Vec<_>>();
    if owner.is_empty() && !contacts.is_empty() {
        return Err(owner_choice_unavailable(
            EventFamilyV1::ContactAdmissibilityAndPointCount,
            completed_step,
            "owner has no prepared-contact events for a non-empty local step",
        ));
    }

    if route.routes(EventFamilyV1::ContactAdmissibilityAndPointCount) {
        for event in &owner {
            if !event.admitted {
                continue;
            }
            if !contacts
                .iter()
                .any(|contact| contact_pair(contact, bodies) == event.pair)
            {
                return Err(owner_choice_unavailable(
                    EventFamilyV1::ContactAdmissibilityAndPointCount,
                    completed_step,
                    "owner-admitted pair is not locally generated",
                ));
            }
        }
        contacts.retain(|contact| {
            let pair = contact_pair(contact, bodies);
            owner
                .iter()
                .any(|event| event.admitted && event.pair == pair)
        });
        for contact in contacts.iter_mut() {
            let pair = contact_pair(contact, bodies);
            let event = owner
                .iter()
                .find(|event| event.pair == pair && event.admitted)
                .expect("retained routed contact has an owner event");
            let point_count = event.point_count as usize;
            if point_count > contact.points.len() {
                return Err(owner_choice_unavailable(
                    EventFamilyV1::ContactAdmissibilityAndPointCount,
                    completed_step,
                    "owner point count exceeds locally generated points",
                ));
            }
            contact.points.truncate(point_count);
        }
    }

    for contact in contacts.iter_mut() {
        let pair = contact_pair(contact, bodies);
        let event = owner.iter().find(|event| event.pair == pair);
        if route.routes(EventFamilyV1::ReferenceOwnerAndAuthorityFeatureIdentity) {
            let Some(event) = event else {
                return Err(owner_choice_unavailable(
                    EventFamilyV1::ReferenceOwnerAndAuthorityFeatureIdentity,
                    completed_step,
                    "local contact has no owner identity event",
                ));
            };
            if event.normal_orientation != EventNormalOrientationV1::BodyAToBodyB {
                return Err(owner_choice_unavailable(
                    EventFamilyV1::ReferenceOwnerAndAuthorityFeatureIdentity,
                    completed_step,
                    "owner normal orientation has no local body-A-to-body-B choice",
                ));
            }
            let local_points = contact.points.clone();
            let mut selected = Vec::with_capacity(event.points.len());
            for owner_point in &event.points {
                let Some(point) = local_points.iter().find(|point| {
                    point.feature_a == owner_point.identity.feature_a
                        && point.feature_b == owner_point.identity.feature_b
                }) else {
                    return Err(owner_choice_unavailable(
                        EventFamilyV1::ReferenceOwnerAndAuthorityFeatureIdentity,
                        completed_step,
                        "owner feature pair is not a locally generated point choice",
                    ));
                };
                selected.push(point.clone());
            }
            contact.points = selected;
            contact.event_reference_owner = event.reference_owner;
            contact.event_reference_face = event.reference_face;
        }

        if route.routes(EventFamilyV1::WarmCacheMatchAndImpulseOwnership) {
            let Some(event) = event else {
                return Err(owner_choice_unavailable(
                    EventFamilyV1::WarmCacheMatchAndImpulseOwnership,
                    completed_step,
                    "local contact has no owner cache event",
                ));
            };
            if event.points.len() != contact.points.len() {
                return Err(owner_choice_unavailable(
                    EventFamilyV1::WarmCacheMatchAndImpulseOwnership,
                    completed_step,
                    "owner cache point count has no local correspondence",
                ));
            }
            for (point, owner_point) in contact.points.iter_mut().zip(&event.points) {
                let identity = EventPointIdentityV1 {
                    feature_a: point.feature_a,
                    feature_b: point.feature_b,
                };
                if identity != owner_point.identity {
                    return Err(owner_choice_unavailable(
                        EventFamilyV1::WarmCacheMatchAndImpulseOwnership,
                        completed_step,
                        "owner cache key does not match the local point key",
                    ));
                }
                if owner_point.cache_hit {
                    if !point.cache_hit || owner_point.cache_owner != Some(identity) {
                        return Err(owner_choice_unavailable(
                            EventFamilyV1::WarmCacheMatchAndImpulseOwnership,
                            completed_step,
                            "owner cache hit has no locally matched impulse record",
                        ));
                    }
                } else {
                    point.cache_hit = false;
                    point.normal_impulse = 0.0;
                    point.tangent_impulse = 0.0;
                }
            }
        }
    }
    Ok(())
}

fn route_visit_schedule(
    route: &EventRouteState<'_>,
    completed_step: u32,
    contacts: &[PreparedContact],
    bodies: &[BodyStateV1],
    schedule: &mut [ContactVisitScheduleEntry],
) -> Result<(), ScalarError> {
    if !route.routes(EventFamilyV1::SupportGraphColorAndOverflowOrder) {
        return Ok(());
    }
    let owner = route
        .owner
        .prepared_contacts
        .iter()
        .filter(|event| event.completed_step == completed_step)
        .collect::<Vec<_>>();
    if owner.len() != contacts.len() {
        return Err(owner_choice_unavailable(
            EventFamilyV1::SupportGraphColorAndOverflowOrder,
            completed_step,
            "owner and local contact counts differ",
        ));
    }
    let mut ranked = Vec::with_capacity(schedule.len());
    for entry in schedule.iter().copied() {
        let pair = contact_pair(&contacts[entry.contact_index], bodies);
        let Some(event) = owner.iter().find(|event| event.pair == pair) else {
            return Err(owner_choice_unavailable(
                EventFamilyV1::SupportGraphColorAndOverflowOrder,
                completed_step,
                "owner visit pair is not locally available",
            ));
        };
        if event.graph_class != entry.graph_class {
            return Err(owner_choice_unavailable(
                EventFamilyV1::SupportGraphColorAndOverflowOrder,
                completed_step,
                "owner graph class is not the locally produced class",
            ));
        }
        ranked.push((event.visit_rank, entry));
    }
    ranked.sort_by_key(|(rank, _)| *rank);
    if ranked.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(owner_choice_unavailable(
            EventFamilyV1::SupportGraphColorAndOverflowOrder,
            completed_step,
            "owner visit ranks are not unique",
        ));
    }
    for (target, (_, entry)) in schedule.iter_mut().zip(ranked) {
        *target = entry;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
fn routed_normal_choices(
    route: &mut EventRouteState<'_>,
    completed_step: u32,
    substep: u32,
    pass: NormalPassV1,
    visit_rank: u32,
    pair: EventPairV1,
    contact: &PreparedContact,
) -> Result<Vec<NormalRouteChoiceV1>, ScalarError> {
    if !route.routes_normal_decisions() {
        return Ok(Vec::new());
    }
    let mut choices = Vec::with_capacity(contact.points.len());
    for (point_ordinal, point) in contact.points.iter().enumerate() {
        let Some(owner) = route.owner.normal_decisions.get(route.normal_cursor) else {
            return Err(owner_choice_unavailable(
                route.normal_family(),
                completed_step,
                "owner normal stream ended before the local stream",
            ));
        };
        let identity = EventPointIdentityV1 {
            feature_a: point.feature_a,
            feature_b: point.feature_b,
        };
        if owner.completed_step != completed_step
            || owner.substep != substep
            || owner.pass != pass
            || owner.visit_rank != visit_rank
            || owner.pair != pair
            || owner.point_ordinal != point_ordinal as u32
            || owner.identity != identity
        {
            return Err(owner_choice_unavailable(
                route.normal_family(),
                completed_step,
                "owner normal event key has no exact local event choice",
            ));
        }
        choices.push(NormalRouteChoiceV1 {
            separation_positive: route
                .routes(EventFamilyV1::SeparationPositiveAndSpeculativeBranch)
                .then_some(owner.separation_positive),
            candidate_selected: route
                .routes(EventFamilyV1::UnilateralActiveSetProjection)
                .then_some(owner.candidate_selected),
        });
        route.normal_cursor += 1;
    }
    Ok(choices)
}

fn prepare_contacts(
    scenario: &ScenarioV1,
    bodies: &[BodyStateV1],
    cache: &[ContactCachePointV1],
    traces: &mut Vec<TraceRecordV1>,
    authority_identity: bool,
) -> Result<Vec<PreparedContact>, ScalarError> {
    let mut manifolds = generate_portable_contacts(scenario, bodies)?;
    let reference_metadata = manifolds
        .iter()
        .map(|manifold| {
            reference_owner_and_face(
                &manifold
                    .points
                    .iter()
                    .map(|point| EventPointIdentityV1 {
                        feature_a: point.feature_a,
                        feature_b: point.feature_b,
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect::<Vec<_>>();
    if authority_identity {
        canonicalize_authority_features(&mut manifolds, bodies)?;
    }
    manifolds
        .iter()
        .zip(reference_metadata)
        .map(|(manifold, (reference_owner, reference_face))| {
            let mut contact = prepare_contact(bodies, manifold, cache, traces)?;
            contact.event_reference_owner = reference_owner;
            contact.event_reference_face = reference_face;
            Ok(contact)
        })
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
    contact_visit_schedule(contacts, bodies, side)
        .into_iter()
        .map(|entry| entry.contact_index)
        .collect()
}

#[derive(Clone, Copy)]
struct ContactVisitScheduleEntry {
    contact_index: usize,
    graph_class: EventGraphClassV1,
}

fn contact_visit_schedule(
    contacts: &[PreparedContact],
    bodies: &[BodyStateV1],
    side: AuthorityStructureSideV1,
) -> Vec<ContactVisitScheduleEntry> {
    if side == AuthorityStructureSideV1::Physbox {
        return (0..contacts.len())
            .map(|contact_index| ContactVisitScheduleEntry {
                contact_index,
                graph_class: EventGraphClassV1::Unavailable,
            })
            .collect();
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
        .map(|contact_index| ContactVisitScheduleEntry {
            contact_index,
            graph_class: EventGraphClassV1::Overflow,
        })
        .chain(
            colors
                .into_iter()
                .enumerate()
                .flat_map(|(color, contacts)| {
                    contacts
                        .into_iter()
                        .map(move |contact_index| ContactVisitScheduleEntry {
                            contact_index,
                            graph_class: EventGraphClassV1::Color(color as u8),
                        })
                }),
        )
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
