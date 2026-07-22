//! Observation-only connected components for the generated contact graph.
//!
//! This module is deliberately outside the product graph. S26 uses the scalar
//! recurrence as an independent executable specification for an opt-in GPU
//! observer; no label produced here is a sleep or solver input.

const INVALID_LABEL: u32 = u32::MAX;

pub(crate) const CONTACT_COMPONENT_PHASE_BOUND_VERSION: &str =
    "physbox5-contact-component-phase-bound-v1";

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ScalarComponentObservation {
    pub labels: Vec<u32>,
    pub phases_executed: u32,
    pub last_changed_phase: u32,
    pub converged: bool,
}

/// Returns the fixed hook/shortcut phase count encoded by the GPU observer.
///
/// A hook only lowers a parent, and the following shortcut replaces every
/// parent by its grandparent. With roots hooked from the greater label to the
/// lesser label, every two phases at least double the already-reached prefix
/// of a worst-case ordered path. `2 * ceil(log2(N)) + 1` is therefore a
/// conservative finite bound for `N` reserved slots. The GPU independently
/// rejects any result that still has a non-root label or an edge crossing two
/// labels, so this bound can never silently admit a partial component.
pub(crate) fn component_phase_bound(body_capacity: u32) -> Result<u32, String> {
    if body_capacity == 0 {
        return Err("contact-component body capacity must be positive".to_owned());
    }
    let logarithm = u32::BITS - body_capacity.saturating_sub(1).leading_zeros();
    logarithm
        .checked_mul(2)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "contact-component phase bound overflowed".to_owned())
}

pub(crate) fn reference_component_labels(
    live_dynamic: &[bool],
    edges: &[(u32, u32)],
) -> Result<Vec<u32>, String> {
    validate_edges(live_dynamic, edges)?;
    let mut adjacency = vec![Vec::new(); live_dynamic.len()];
    for &(a, b) in edges {
        adjacency[a as usize].push(b);
        adjacency[b as usize].push(a);
    }
    let mut labels = vec![INVALID_LABEL; live_dynamic.len()];
    for slot in 0..live_dynamic.len() {
        if !live_dynamic[slot] || labels[slot] != INVALID_LABEL {
            continue;
        }
        let root = slot as u32;
        labels[slot] = root;
        let mut stack = vec![root];
        while let Some(body) = stack.pop() {
            for &neighbor in &adjacency[body as usize] {
                if labels[neighbor as usize] == INVALID_LABEL {
                    labels[neighbor as usize] = root;
                    stack.push(neighbor);
                }
            }
        }
    }
    Ok(labels)
}

pub(crate) fn observe_components_scalar(
    live_dynamic: &[bool],
    edges: &[(u32, u32)],
) -> Result<ScalarComponentObservation, String> {
    validate_edges(live_dynamic, edges)?;
    let capacity = u32::try_from(live_dynamic.len())
        .map_err(|_| "contact-component body capacity exceeds u32".to_owned())?;
    let phase_bound = component_phase_bound(capacity)?;
    let mut parents = live_dynamic
        .iter()
        .enumerate()
        .map(|(slot, &live)| if live { slot as u32 } else { INVALID_LABEL })
        .collect::<Vec<_>>();
    let mut last_changed_phase = 0;

    for phase in 1..=phase_bound {
        let before_hook = parents.clone();
        for &(a, b) in edges {
            let parent_a = before_hook[a as usize];
            let parent_b = before_hook[b as usize];
            let high = parent_a.max(parent_b) as usize;
            let low = parent_a.min(parent_b);
            parents[high] = parents[high].min(low);
        }

        let before_shortcut = parents.clone();
        for (slot, parent) in parents.iter_mut().enumerate() {
            if !live_dynamic[slot] {
                continue;
            }
            *parent = before_shortcut[before_shortcut[slot] as usize];
        }
        if parents != before_hook {
            last_changed_phase = phase;
        }
    }

    let converged = live_dynamic.iter().enumerate().all(|(slot, &live)| {
        !live || parents[slot] != INVALID_LABEL && parents[parents[slot] as usize] == parents[slot]
    }) && edges
        .iter()
        .all(|&(a, b)| parents[a as usize] == parents[b as usize]);

    Ok(ScalarComponentObservation {
        labels: parents,
        phases_executed: phase_bound,
        last_changed_phase,
        converged,
    })
}

fn validate_edges(live_dynamic: &[bool], edges: &[(u32, u32)]) -> Result<(), String> {
    if live_dynamic.is_empty() {
        return Err("contact-component body capacity must be positive".to_owned());
    }
    for &(a, b) in edges {
        let Some(&a_live) = live_dynamic.get(a as usize) else {
            return Err(format!(
                "contact-component edge endpoint {a} exceeds capacity"
            ));
        };
        let Some(&b_live) = live_dynamic.get(b as usize) else {
            return Err(format!(
                "contact-component edge endpoint {b} exceeds capacity"
            ));
        };
        if a == b || !a_live || !b_live {
            return Err(format!(
                "contact-component edge ({a}, {b}) is not a live dynamic pair"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_matches_reference(live: &[bool], edges: &[(u32, u32)]) {
        let observed = observe_components_scalar(live, edges).unwrap();
        assert!(observed.converged, "did not converge for {edges:?}");
        assert_eq!(
            observed.labels,
            reference_component_labels(live, edges).unwrap()
        );
        assert_eq!(
            observed.phases_executed,
            component_phase_bound(live.len() as u32).unwrap()
        );
    }

    #[test]
    fn phase_bound_is_monotonic_and_capacity_derived() {
        assert_eq!(component_phase_bound(1), Ok(1));
        assert_eq!(component_phase_bound(2), Ok(3));
        assert_eq!(component_phase_bound(3), Ok(5));
        assert_eq!(component_phase_bound(4), Ok(5));
        assert_eq!(component_phase_bound(1_024), Ok(21));
        assert_eq!(component_phase_bound(100_000), Ok(35));
        assert!(component_phase_bound(0).is_err());
    }

    #[test]
    fn every_graph_through_six_vertices_matches_independent_traversal() {
        for vertex_count in 1..=6u32 {
            let candidates = (0..vertex_count)
                .flat_map(|a| ((a + 1)..vertex_count).map(move |b| (a, b)))
                .collect::<Vec<_>>();
            for mask in 0u64..(1u64 << candidates.len()) {
                let edges = candidates
                    .iter()
                    .enumerate()
                    .filter_map(|(index, edge)| ((mask >> index) & 1 == 1).then_some(*edge))
                    .collect::<Vec<_>>();
                assert_matches_reference(&vec![true; vertex_count as usize], &edges);
            }
        }
    }

    #[test]
    fn inactive_slots_and_disconnected_components_keep_exact_minimum_labels() {
        let live = [true, true, false, true, true, true, false, true];
        let edges = [(5, 3), (4, 5), (1, 0)];
        assert_matches_reference(&live, &edges);
        assert_eq!(
            observe_components_scalar(&live, &edges).unwrap().labels,
            vec![0, 0, INVALID_LABEL, 3, 3, 3, INVALID_LABEL, 7]
        );
    }

    #[test]
    fn adversarial_long_paths_stars_and_bridges_converge_inside_the_bound() {
        for vertex_count in [2u32, 3, 17, 64, 257, 1_024] {
            let live = vec![true; vertex_count as usize];
            let forward = (1..vertex_count)
                .map(|body| (body - 1, body))
                .collect::<Vec<_>>();
            let reverse = forward.iter().rev().copied().collect::<Vec<_>>();
            let star = (1..vertex_count)
                .rev()
                .map(|body| (body, 0))
                .collect::<Vec<_>>();
            assert_matches_reference(&live, &forward);
            assert_matches_reference(&live, &reverse);
            assert_matches_reference(&live, &star);
        }

        let live = vec![true; 256];
        let mut bridge = (1..128).map(|body| (body - 1, body)).collect::<Vec<_>>();
        bridge.extend((129..256).map(|body| (body - 1, body)));
        bridge.push((127, 255));
        assert_matches_reference(&live, &bridge);
    }

    #[test]
    fn malformed_edge_projection_fails_closed() {
        assert!(observe_components_scalar(&[true], &[(0, 0)]).is_err());
        assert!(observe_components_scalar(&[true, false], &[(0, 1)]).is_err());
        assert!(observe_components_scalar(&[true], &[(0, 1)]).is_err());
    }
}
