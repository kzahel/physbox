use std::collections::BTreeMap;

use physbox_contracts::{
    BodyId, BodyStateV1, CandidatePairV1, CollisionContactPointV1, CollisionManifoldV1,
    ContactCacheValueV1, GeometricFeatureKindV1, GeometricFeatureV1, OracleResultV1,
    PersistentContactKeyV1, PersistentContactV1, ScenarioV1, TraceRecordV1, TraceStageV1, Vec2,
};
use serde::Serialize;

pub const STATIONARITY_METRIC_CONTRACT_VERSION: u32 = 1;

#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StationarityMetricContractV1 {
    pub version: u32,
    pub settled_start_step: u32,
    pub settled_end_step: u32,
    pub samples: u32,
    pub track_split_rule: &'static str,
    pub amplitude_rule: &'static str,
    pub variance_rule: &'static str,
    pub active_set_rule: &'static str,
    pub redistribution_rule: &'static str,
    pub manifold_classes: [&'static str; 3],
    pub authority_band_rule: &'static str,
    pub zero_ratio_rule: &'static str,
    pub class_verdict_rule: &'static str,
    pub correlation_rule: &'static str,
    pub majority_removal_rule: &'static str,
    pub hypothesis_rule: &'static str,
}

pub const fn stationarity_metric_contract_v1() -> StationarityMetricContractV1 {
    StationarityMetricContractV1 {
        version: STATIONARITY_METRIC_CONTRACT_VERSION,
        settled_start_step: 900,
        settled_end_step: 1_200,
        samples: 301,
        track_split_rule: "exact engine point identity; absence splits; continuous tracks require at least two samples",
        amplitude_rule: "max-minus-min over exact captured f32 inputs",
        variance_rule: "population variance accumulated in f64 over exact captured f32 inputs",
        active_set_rule: "ordered exact normal-impulse-greater-than-zero bits; compare only identical ordered point sets",
        redistribution_rule: "half-L1 change of two-point impulse shares; both-zero=0; exactly-one-zero=1",
        manifold_classes: ["floor-support", "interior", "top"],
        authority_band_rule: "per-class maximum matched warm-on authority observable across centered 8/16 stacks",
        zero_ratio_rule: "positive authority gives quotient; both exact zero is both-zero; authority zero with positive candidate is authority-zero-candidate-positive",
        class_verdict_rule: "stationary iff every PhysBox class maximum is at or below its authority band; any strict exceedance is oscillating",
        correlation_rule: "population Pearson across matched manifold tracks; null on exact zero population variance",
        majority_removal_rule: "16-stack structure preserved and both settled-max-speed and last-quartile-energy removals strictly greater than 0.5",
        hypothesis_rule: "H1 iff majority removal and a floor/interior normal, active-set, or redistribution metric exceeds authority; otherwise H2",
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct AuthorityTraceSnapshotV1 {
    pub completed_steps: u32,
    pub bodies: Vec<BodyStateV1>,
    pub manifolds: Vec<CollisionManifoldV1>,
    pub cache: Vec<PersistentContactV1>,
}

pub fn decode_box2d_step_trace(
    source: &ScenarioV1,
    result: &OracleResultV1,
) -> Result<Vec<AuthorityTraceSnapshotV1>, String> {
    if result.failure.is_some() || result.traces.is_empty() {
        return Err("Box2D stationarity trace is missing or failed".to_owned());
    }
    let mut body_records = BTreeMap::<u32, Vec<&TraceRecordV1>>::new();
    let mut contact_records = BTreeMap::<u32, Vec<&TraceRecordV1>>::new();
    for record in &result.traces {
        match record.stage {
            TraceStageV1::Integrate => body_records.entry(record.step).or_default().push(record),
            TraceStageV1::Prepare => contact_records.entry(record.step).or_default().push(record),
            _ => return Err("Box2D stationarity trace contains an unsupported stage".to_owned()),
        }
    }
    let source_by_id = source
        .bodies
        .iter()
        .map(|body| (body.id, body))
        .collect::<BTreeMap<_, _>>();
    let mut snapshots = Vec::with_capacity(source.requested_steps as usize);
    for completed_steps in 1..=source.requested_steps {
        let records = body_records
            .remove(&completed_steps)
            .ok_or_else(|| format!("Box2D trace omits body step {completed_steps}"))?;
        if records.len() != source.bodies.len() {
            return Err(format!(
                "Box2D trace body count differs at step {completed_steps}"
            ));
        }
        let mut bodies = Vec::with_capacity(records.len());
        for record in records {
            let id = BodyId {
                slot: record.manifold_id,
                generation: record.point_index,
            };
            let mut body = source_by_id
                .get(&id)
                .ok_or_else(|| format!("Box2D trace names unknown body {id:?}"))?
                .to_owned()
                .clone();
            body.position = Vec2::new(value(record, "position_x")?, value(record, "position_y")?);
            body.angle = finite(value(record, "angle")?)?;
            body.linear_velocity = Vec2::new(
                value(record, "linear_velocity_x")?,
                value(record, "linear_velocity_y")?,
            );
            body.angular_velocity = finite(value(record, "angular_velocity")?)?;
            body.sleeping = false;
            bodies.push(body);
        }
        bodies.sort_by_key(|body| body.id);
        if bodies
            .iter()
            .map(|body| body.id)
            .ne(source.bodies.iter().map(|body| body.id))
        {
            return Err(format!(
                "Box2D trace body identity differs at step {completed_steps}"
            ));
        }
        let body_by_id = bodies
            .iter()
            .map(|body| (body.id, body))
            .collect::<BTreeMap<_, _>>();
        let mut grouped = BTreeMap::<CandidatePairV1, Vec<&TraceRecordV1>>::new();
        for record in contact_records.remove(&completed_steps).unwrap_or_default() {
            let pair = CandidatePairV1 {
                body_a: BodyId {
                    slot: exact_u32(record, "body_a_slot")?,
                    generation: exact_u32(record, "body_a_generation")?,
                },
                body_b: BodyId {
                    slot: exact_u32(record, "body_b_slot")?,
                    generation: exact_u32(record, "body_b_generation")?,
                },
            };
            if pair.body_a >= pair.body_b
                || !source_by_id.contains_key(&pair.body_a)
                || !source_by_id.contains_key(&pair.body_b)
            {
                return Err("Box2D trace contains an invalid source pair".to_owned());
            }
            grouped.entry(pair).or_default().push(record);
        }
        let mut manifolds = Vec::with_capacity(grouped.len());
        let mut cache = Vec::new();
        for (pair, mut records) in grouped {
            records.sort_by_key(|record| record.point_index);
            if records.len() > 2 {
                return Err("Box2D trace manifold exceeds two points".to_owned());
            }
            let normal = Vec2::new(
                value(records[0], "normal_x")?,
                value(records[0], "normal_y")?,
            );
            let body_a = body_by_id
                .get(&pair.body_a)
                .ok_or_else(|| "Box2D trace body A is missing".to_owned())?;
            let body_b = body_by_id
                .get(&pair.body_b)
                .ok_or_else(|| "Box2D trace body B is missing".to_owned())?;
            let mut points = Vec::with_capacity(records.len());
            for record in records {
                if value(record, "normal_x")?.to_bits() != normal.x.to_bits()
                    || value(record, "normal_y")?.to_bits() != normal.y.to_bits()
                {
                    return Err("Box2D trace manifold points disagree on normal".to_owned());
                }
                let point_id = exact_u32(record, "point_id")?;
                if point_id > u16::MAX.into() {
                    return Err("Box2D public point id exceeds u16".to_owned());
                }
                let feature_a = GeometricFeatureV1 {
                    kind: GeometricFeatureKindV1::Vertex,
                    index: point_id >> 8,
                };
                let feature_b = GeometricFeatureV1 {
                    kind: GeometricFeatureKindV1::Vertex,
                    index: point_id & 0xff,
                };
                if feature_a.index >= 4 || feature_b.index >= 4 {
                    return Err(
                        "Box2D rectangle point id is outside the source feature range".to_owned(),
                    );
                }
                let point = CollisionContactPointV1 {
                    local_anchor_a: inverse_rotate(
                        Vec2::new(value(record, "anchor_a_x")?, value(record, "anchor_a_y")?),
                        body_a.angle.get(),
                    ),
                    local_anchor_b: inverse_rotate(
                        Vec2::new(value(record, "anchor_b_x")?, value(record, "anchor_b_y")?),
                        body_b.angle.get(),
                    ),
                    world_point: Vec2::new(value(record, "point_x")?, value(record, "point_y")?),
                    separation: finite(value(record, "separation")?)?,
                    feature_a,
                    feature_b,
                };
                let key = PersistentContactKeyV1 {
                    pair,
                    feature_a,
                    feature_b,
                };
                cache.push(PersistentContactV1 {
                    key,
                    value: ContactCacheValueV1 {
                        normal_impulse: finite(value(record, "normal_impulse")?)?,
                        tangent_impulse: finite(value(record, "tangent_impulse")?)?,
                        last_seen_step: completed_steps,
                    },
                });
                points.push(point);
            }
            manifolds.push(CollisionManifoldV1 {
                pair,
                normal,
                material: mix_material(body_a.material, body_b.material)?,
                points,
            });
        }
        cache.sort_by_key(|entry| entry.key);
        snapshots.push(AuthorityTraceSnapshotV1 {
            completed_steps,
            bodies,
            manifolds,
            cache,
        });
    }
    if !body_records.is_empty() || !contact_records.is_empty() {
        return Err("Box2D trace contains steps outside the requested range".to_owned());
    }
    Ok(snapshots)
}

fn value(record: &TraceRecordV1, name: &str) -> Result<f32, String> {
    record
        .values
        .binary_search_by_key(&name, |value| value.name.as_str())
        .ok()
        .map(|index| record.values[index].value.get())
        .ok_or_else(|| format!("Box2D trace value {name} is missing"))
}

fn exact_u32(record: &TraceRecordV1, name: &str) -> Result<u32, String> {
    let value = value(record, name)?;
    if value < 0.0 || value > u32::MAX as f32 || value.fract() != 0.0 {
        return Err(format!("Box2D trace value {name} is not an exact u32"));
    }
    let output = value as u32;
    if output as f32 != value {
        return Err(format!("Box2D trace value {name} lost integer precision"));
    }
    Ok(output)
}

fn inverse_rotate(vector: Vec2, angle: f32) -> Vec2 {
    let (sine, cosine) = (-angle).sin_cos();
    Vec2::new(
        cosine * vector.x - sine * vector.y,
        sine * vector.x + cosine * vector.y,
    )
}

fn mix_material(
    a: physbox_contracts::MaterialV1,
    b: physbox_contracts::MaterialV1,
) -> Result<physbox_contracts::MaterialV1, String> {
    Ok(physbox_contracts::MaterialV1 {
        friction: finite((a.friction.get() * b.friction.get()).sqrt())?,
        restitution: finite(a.restitution.get().max(b.restitution.get()))?,
    })
}

fn finite(value: f32) -> Result<physbox_contracts::FiniteF32, String> {
    physbox_contracts::FiniteF32::new(value).map_err(|error| error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn metric_contract_is_frozen() {
        let contract = stationarity_metric_contract_v1();
        assert_eq!(contract.samples, 301);
        assert_eq!(
            contract.manifold_classes,
            ["floor-support", "interior", "top"]
        );
        assert!(contract.majority_removal_rule.contains("0.5"));
    }
}
