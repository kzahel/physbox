use physbox_contracts::{
    CollisionManifoldV1, ContactCacheValueV1, FiniteF32, PersistentContactV1, Validate,
};

#[derive(Clone, Debug, PartialEq)]
pub struct ScalarPersistenceStep {
    pub cache: Vec<PersistentContactV1>,
    pub matched: u64,
    pub cold: u64,
    pub expired: u64,
}

pub fn transfer_scalar_persistence(
    manifolds: &[CollisionManifoldV1],
    previous: &[PersistentContactV1],
    completed_step: u32,
) -> Result<ScalarPersistenceStep, String> {
    for manifold in manifolds {
        manifold.validate().map_err(|error| error.to_string())?;
    }
    for entry in previous {
        entry.validate().map_err(|error| error.to_string())?;
    }
    for pair in previous.windows(2) {
        if pair[0].key >= pair[1].key {
            return Err("previous scalar persistence cache must be sorted and unique".to_owned());
        }
    }
    let mut current = manifolds
        .iter()
        .flat_map(|manifold| {
            manifold.points.iter().map(|point| PersistentContactV1 {
                key: point.key(manifold.pair),
                value: ContactCacheValueV1 {
                    normal_impulse: finite(0.0),
                    tangent_impulse: finite(0.0),
                    last_seen_step: completed_step,
                },
            })
        })
        .collect::<Vec<_>>();
    current.sort_by_key(|entry| entry.key);
    for pair in current.windows(2) {
        if pair[0].key == pair[1].key {
            return Err("current scalar persistence keys are not unique".to_owned());
        }
    }

    let mut previous_index = 0_usize;
    let mut matched = 0_u64;
    let mut cold = 0_u64;
    for entry in &mut current {
        while previous_index < previous.len() && previous[previous_index].key < entry.key {
            previous_index += 1;
        }
        if previous_index < previous.len() && previous[previous_index].key == entry.key {
            entry.value.normal_impulse = previous[previous_index].value.normal_impulse;
            entry.value.tangent_impulse = previous[previous_index].value.tangent_impulse;
            matched += 1;
        } else {
            cold += 1;
        }
    }
    Ok(ScalarPersistenceStep {
        cache: current,
        matched,
        cold,
        expired: previous.len() as u64 - matched,
    })
}

fn finite(value: f32) -> FiniteF32 {
    FiniteF32::new(value).expect("finite persistence constant")
}

#[cfg(test)]
mod tests {
    use physbox_engine::{P5_2CollisionScenarioKind, p5_2_collision_scenario};

    use crate::{
        analytic_narrowphase::generate_scalar_narrowphase,
        collision_candidates::enumerate_scalar_candidates,
    };

    use super::*;

    #[test]
    fn scalar_transfer_is_sorted_and_expires_missing_points() {
        let scenario =
            p5_2_collision_scenario(P5_2CollisionScenarioKind::RectangleRectangle, 1, 17).unwrap();
        let candidates = enumerate_scalar_candidates(&scenario).unwrap();
        let manifolds = generate_scalar_narrowphase(&scenario, &candidates.candidates).unwrap();
        let cold = transfer_scalar_persistence(&manifolds, &[], 1).unwrap();
        assert_eq!(cold.cold, cold.cache.len() as u64);
        let warm = transfer_scalar_persistence(&manifolds, &cold.cache, 2).unwrap();
        assert_eq!(warm.matched, warm.cache.len() as u64);
        let expired = transfer_scalar_persistence(&[], &warm.cache, 3).unwrap();
        assert_eq!(expired.expired, warm.cache.len() as u64);
        assert!(expired.cache.is_empty());
    }
}
