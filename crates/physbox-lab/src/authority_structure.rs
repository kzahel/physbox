//! Bounded S12 scalar bridge for decomposing the pinned authority structure.
//!
//! This module owns an independent orchestration loop over PhysBox's scalar
//! equations and neutral contact projection. It does not link or copy Box2D.
//! The authority-side schedule is derived from clean Box2D revision
//! `3a4f0da8374af61293a03021c9a0b3ebcfe67948`, principally `solver.c`,
//! `constraint_graph.c`, `contact_solver.c`, and `physics_world.c`.

use physbox_contracts::{
    BodyStateV1, ContactCachePointV1, ContactSoftnessPolicyV1, FiniteF32, ScenarioV1,
    TraceRecordV1, TraceStageV1, Validate,
};
use serde::{Deserialize, Serialize};

use crate::scalar::{
    PreparedContact, ScalarError, ScalarObservedSolveResultV1, ScalarSolveResult,
    ScalarStepObservationV1, apply_restitution, collect_contact_cache, generate_portable_contacts,
    integrate_positions, integrate_velocities, maximum_depenetration_velocity, prepare_contact,
    refresh_separations, solve_normal, solve_tangent, warm_start,
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
                    fixed_contacts = Some(prepare_contacts(
                        scenario,
                        &bodies,
                        &contact_cache,
                        &mut traces,
                    )?);
                }
            } else {
                refreshed_contacts =
                    prepare_contacts(scenario, &bodies, &outer_step_cache, &mut traces)?;
            }
            let contacts = if let Some(contacts) = fixed_contacts.as_mut() {
                contacts
            } else {
                &mut refreshed_contacts
            };
            let visit_order = contact_visit_order(contacts, &bodies, policy.accumulation);

            integrate_velocities(&mut bodies, scenario.settings.gravity, substep_dt)?;
            for &contact_index in &visit_order {
                warm_start(&contacts[contact_index], &mut bodies, &mut traces)?;
            }
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

            integrate_positions(&mut bodies, substep_dt, &mut traces)?;
            refresh_separations(contacts, &bodies);
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

fn prepare_contacts(
    scenario: &ScenarioV1,
    bodies: &[BodyStateV1],
    cache: &[ContactCachePointV1],
    traces: &mut Vec<TraceRecordV1>,
) -> Result<Vec<PreparedContact>, ScalarError> {
    let manifolds = generate_portable_contacts(scenario, bodies)?;
    manifolds
        .iter()
        .map(|manifold| prepare_contact(bodies, manifold, cache, traces))
        .collect()
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
}
