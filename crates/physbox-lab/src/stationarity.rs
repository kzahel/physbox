use std::collections::BTreeMap;

use physbox_contracts::{
    BodyId, BodyStateV1, CandidatePairV1, CollisionContactPointV1, CollisionManifoldV1,
    ContactCacheValueV1, GeometricFeatureKindV1, GeometricFeatureV1, OracleResultV1,
    PersistentContactKeyV1, PersistentContactV1, ScenarioV1, TraceRecordV1, TraceStageV1, Vec2,
};
use serde::{Deserialize, Serialize};

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

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct AuthorityTraceSnapshotV1 {
    pub completed_steps: u32,
    pub bodies: Vec<BodyStateV1>,
    pub manifolds: Vec<CollisionManifoldV1>,
    pub cache: Vec<PersistentContactV1>,
}

pub const STATIONARITY_CAPTURE_VERSION: u32 = 1;

#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StationarityManifoldClassV1 {
    FloorSupport,
    Interior,
    Top,
}

impl StationarityManifoldClassV1 {
    pub const ALL: [Self; 3] = [Self::FloorSupport, Self::Interior, Self::Top];

    pub const fn name(self) -> &'static str {
        match self {
            Self::FloorSupport => "floor-support",
            Self::Interior => "interior",
            Self::Top => "top",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StationarityActiveSetV1 {
    ScalarActive,
    ScalarInactive,
    Both,
    First,
    Second,
    None,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationarityPointSampleV1 {
    pub key: PersistentContactKeyV1,
    pub normal_impulse: f32,
    pub tangent_impulse: f32,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationarityManifoldSampleV1 {
    pub pair: CandidatePairV1,
    pub class: StationarityManifoldClassV1,
    pub supported_body: BodyId,
    pub supported_body_center_speed: f64,
    pub total_normal_impulse: f64,
    pub total_tangent_impulse: f64,
    pub active_set: StationarityActiveSetV1,
    pub points: Vec<StationarityPointSampleV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationaritySnapshotV1 {
    pub completed_steps: u32,
    pub manifolds: Vec<StationarityManifoldSampleV1>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationarityCaptureV1 {
    pub version: u32,
    pub engine: String,
    pub scene_label: String,
    pub source_scenario_hash: String,
    pub observation_version: u32,
    pub settled_start_step: u32,
    pub settled_end_step: u32,
    pub snapshots: Vec<StationaritySnapshotV1>,
}

pub struct StationaritySnapshotInputV1<'a> {
    pub completed_steps: u32,
    pub bodies: &'a [BodyStateV1],
    pub manifolds: &'a [CollisionManifoldV1],
    pub cache: &'a [PersistentContactV1],
}

pub fn stationarity_capture_v1<'a>(
    engine: &str,
    scene_label: &str,
    source: &ScenarioV1,
    source_scenario_hash: &str,
    inputs: impl IntoIterator<Item = StationaritySnapshotInputV1<'a>>,
) -> Result<StationarityCaptureV1, String> {
    let dynamic = source
        .bodies
        .iter()
        .filter(|body| body.inverse_mass.get() > 0.0)
        .collect::<Vec<_>>();
    let top_body = dynamic
        .iter()
        .max_by(|a, b| a.position.y.total_cmp(&b.position.y))
        .ok_or_else(|| "stationarity source has no dynamic body".to_owned())?
        .id;
    let mut snapshots = Vec::with_capacity(301);
    for input in inputs {
        if !(900..=1_200).contains(&input.completed_steps) {
            continue;
        }
        if input.bodies.len() != source.bodies.len() {
            return Err(format!(
                "stationarity body count differs at step {}",
                input.completed_steps
            ));
        }
        let body_map = input
            .bodies
            .iter()
            .map(|body| (body.id, body))
            .collect::<BTreeMap<_, _>>();
        let cache_map = input
            .cache
            .iter()
            .map(|entry| (entry.key, entry.value))
            .collect::<BTreeMap<_, _>>();
        let mut manifolds = Vec::with_capacity(input.manifolds.len());
        for manifold in input.manifolds {
            let body_a = body_map
                .get(&manifold.pair.body_a)
                .ok_or_else(|| "stationarity manifold body A is missing".to_owned())?;
            let body_b = body_map
                .get(&manifold.pair.body_b)
                .ok_or_else(|| "stationarity manifold body B is missing".to_owned())?;
            let (class, supported_body) =
                if body_a.inverse_mass.get() == 0.0 || body_b.inverse_mass.get() == 0.0 {
                    let supported = if body_a.inverse_mass.get() > 0.0 {
                        body_a.id
                    } else if body_b.inverse_mass.get() > 0.0 {
                        body_b.id
                    } else {
                        return Err("stationarity manifold joins two fixed bodies".to_owned());
                    };
                    (StationarityManifoldClassV1::FloorSupport, supported)
                } else {
                    if body_a.position.y.to_bits() == body_b.position.y.to_bits() {
                        return Err("stationarity support pair has equal body heights".to_owned());
                    }
                    let supported = if body_a.position.y > body_b.position.y {
                        body_a.id
                    } else {
                        body_b.id
                    };
                    (
                        if supported == top_body {
                            StationarityManifoldClassV1::Top
                        } else {
                            StationarityManifoldClassV1::Interior
                        },
                        supported,
                    )
                };
            let supported = body_map
                .get(&supported_body)
                .ok_or_else(|| "stationarity supported body is missing".to_owned())?;
            let mut points = manifold
                .points
                .iter()
                .map(|point| {
                    let key = PersistentContactKeyV1 {
                        pair: manifold.pair,
                        feature_a: point.feature_a,
                        feature_b: point.feature_b,
                    };
                    let value = cache_map.get(&key).ok_or_else(|| {
                        format!(
                            "stationarity cache omits current point at step {}",
                            input.completed_steps
                        )
                    })?;
                    Ok(StationarityPointSampleV1 {
                        key,
                        normal_impulse: value.normal_impulse.get(),
                        tangent_impulse: value.tangent_impulse.get(),
                    })
                })
                .collect::<Result<Vec<_>, String>>()?;
            points.sort_by_key(|point| point.key);
            let active_set = active_set(&points)?;
            manifolds.push(StationarityManifoldSampleV1 {
                pair: manifold.pair,
                class,
                supported_body,
                supported_body_center_speed: f64::from(supported.linear_velocity.x)
                    .hypot(f64::from(supported.linear_velocity.y)),
                total_normal_impulse: points
                    .iter()
                    .map(|point| f64::from(point.normal_impulse))
                    .sum(),
                total_tangent_impulse: points
                    .iter()
                    .map(|point| f64::from(point.tangent_impulse))
                    .sum(),
                active_set,
                points,
            });
        }
        manifolds.sort_by_key(|manifold| manifold.pair);
        snapshots.push(StationaritySnapshotV1 {
            completed_steps: input.completed_steps,
            manifolds,
        });
    }
    if snapshots.len() != 301
        || snapshots.first().map(|value| value.completed_steps) != Some(900)
        || snapshots.last().map(|value| value.completed_steps) != Some(1_200)
        || snapshots
            .windows(2)
            .any(|pair| pair[1].completed_steps != pair[0].completed_steps + 1)
    {
        return Err(
            "stationarity capture does not contain exact steps 900 through 1200".to_owned(),
        );
    }
    Ok(StationarityCaptureV1 {
        version: STATIONARITY_CAPTURE_VERSION,
        engine: engine.to_owned(),
        scene_label: scene_label.to_owned(),
        source_scenario_hash: source_scenario_hash.to_owned(),
        observation_version: 2,
        settled_start_step: 900,
        settled_end_step: 1_200,
        snapshots,
    })
}

fn active_set(points: &[StationarityPointSampleV1]) -> Result<StationarityActiveSetV1, String> {
    match points {
        [point] => Ok(if point.normal_impulse > 0.0 {
            StationarityActiveSetV1::ScalarActive
        } else {
            StationarityActiveSetV1::ScalarInactive
        }),
        [first, second] => Ok(
            match (first.normal_impulse > 0.0, second.normal_impulse > 0.0) {
                (true, true) => StationarityActiveSetV1::Both,
                (true, false) => StationarityActiveSetV1::First,
                (false, true) => StationarityActiveSetV1::Second,
                (false, false) => StationarityActiveSetV1::None,
            },
        ),
        _ => Err("stationarity active set requires one or two points".to_owned()),
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationarityMetricMaximaV1 {
    pub normal_point_amplitude_max: f64,
    pub normal_point_variance_max: f64,
    pub normal_manifold_amplitude_max: f64,
    pub normal_manifold_variance_max: f64,
    pub tangent_point_amplitude_max: f64,
    pub tangent_manifold_amplitude_max: f64,
    pub active_set_flip_count_max: u64,
    pub redistribution_transition_max: f64,
}

impl StationarityMetricMaximaV1 {
    fn zero() -> Self {
        Self {
            normal_point_amplitude_max: 0.0,
            normal_point_variance_max: 0.0,
            normal_manifold_amplitude_max: 0.0,
            normal_manifold_variance_max: 0.0,
            tangent_point_amplitude_max: 0.0,
            tangent_manifold_amplitude_max: 0.0,
            active_set_flip_count_max: 0,
            redistribution_transition_max: 0.0,
        }
    }

    pub fn named_values(&self) -> BTreeMap<&'static str, f64> {
        BTreeMap::from([
            (
                "active-set-flip-count-max",
                self.active_set_flip_count_max as f64,
            ),
            (
                "normal-manifold-amplitude-max",
                self.normal_manifold_amplitude_max,
            ),
            (
                "normal-manifold-variance-max",
                self.normal_manifold_variance_max,
            ),
            (
                "normal-point-amplitude-max",
                self.normal_point_amplitude_max,
            ),
            ("normal-point-variance-max", self.normal_point_variance_max),
            (
                "redistribution-transition-max",
                self.redistribution_transition_max,
            ),
            (
                "tangent-manifold-amplitude-max",
                self.tangent_manifold_amplitude_max,
            ),
            (
                "tangent-point-amplitude-max",
                self.tangent_point_amplitude_max,
            ),
        ])
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationarityClassAnalysisV1 {
    pub class: StationarityManifoldClassV1,
    pub point_tracks: u64,
    pub manifold_tracks: u64,
    pub point_identity_breaks: u64,
    pub manifold_identity_breaks: u64,
    pub comparable_active_set_transitions: u64,
    pub comparable_redistribution_transitions: u64,
    pub local_speed_max: f64,
    pub local_speed_normal_amplitude_correlation: Option<f64>,
    pub maxima: StationarityMetricMaximaV1,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationarityAnalysisV1 {
    pub version: u32,
    pub engines: Vec<String>,
    pub scenes: Vec<String>,
    pub classes: Vec<StationarityClassAnalysisV1>,
}

#[derive(Clone)]
struct PointTrack {
    class: StationarityManifoldClassV1,
    capture_index: usize,
    pair: CandidatePairV1,
    key: PersistentContactKeyV1,
    last_step: u32,
    normal: Vec<f64>,
    tangent: Vec<f64>,
}

#[derive(Clone)]
struct ManifoldTrack {
    class: StationarityManifoldClassV1,
    capture_index: usize,
    pair: CandidatePairV1,
    last_step: u32,
    normal: Vec<f64>,
    tangent: Vec<f64>,
    local_speed: Vec<f64>,
    point_keys: Vec<Vec<PersistentContactKeyV1>>,
    point_normals: Vec<Vec<f64>>,
    active_sets: Vec<StationarityActiveSetV1>,
}

pub fn analyze_stationarity_v1(
    captures: &[StationarityCaptureV1],
) -> Result<StationarityAnalysisV1, String> {
    if captures.is_empty() {
        return Err("stationarity analysis requires at least one capture".to_owned());
    }
    let mut point_tracks = Vec::<PointTrack>::new();
    let mut active_points =
        BTreeMap::<(usize, CandidatePairV1, PersistentContactKeyV1), usize>::new();
    let mut manifold_tracks = Vec::<ManifoldTrack>::new();
    let mut active_manifolds = BTreeMap::<(usize, CandidatePairV1), usize>::new();
    for (capture_index, capture) in captures.iter().enumerate() {
        validate_capture(capture)?;
        for snapshot in &capture.snapshots {
            for manifold in &snapshot.manifolds {
                let manifold_key = (capture_index, manifold.pair);
                let manifold_track_index = active_manifolds
                    .get(&manifold_key)
                    .copied()
                    .filter(|index| {
                        manifold_tracks[*index].last_step + 1 == snapshot.completed_steps
                    })
                    .unwrap_or_else(|| {
                        let index = manifold_tracks.len();
                        manifold_tracks.push(ManifoldTrack {
                            class: manifold.class,
                            capture_index,
                            pair: manifold.pair,
                            last_step: snapshot.completed_steps - 1,
                            normal: Vec::new(),
                            tangent: Vec::new(),
                            local_speed: Vec::new(),
                            point_keys: Vec::new(),
                            point_normals: Vec::new(),
                            active_sets: Vec::new(),
                        });
                        active_manifolds.insert(manifold_key, index);
                        index
                    });
                let track = &mut manifold_tracks[manifold_track_index];
                if track.class != manifold.class {
                    return Err("stationarity manifold class changed within a track".to_owned());
                }
                track.last_step = snapshot.completed_steps;
                track.normal.push(manifold.total_normal_impulse);
                track.tangent.push(manifold.total_tangent_impulse);
                track.local_speed.push(manifold.supported_body_center_speed);
                track
                    .point_keys
                    .push(manifold.points.iter().map(|point| point.key).collect());
                track.point_normals.push(
                    manifold
                        .points
                        .iter()
                        .map(|point| f64::from(point.normal_impulse))
                        .collect(),
                );
                track.active_sets.push(manifold.active_set);

                for point in &manifold.points {
                    let point_key = (capture_index, manifold.pair, point.key);
                    let point_track_index = active_points
                        .get(&point_key)
                        .copied()
                        .filter(|index| {
                            point_tracks[*index].last_step + 1 == snapshot.completed_steps
                        })
                        .unwrap_or_else(|| {
                            let index = point_tracks.len();
                            point_tracks.push(PointTrack {
                                class: manifold.class,
                                capture_index,
                                pair: manifold.pair,
                                key: point.key,
                                last_step: snapshot.completed_steps - 1,
                                normal: Vec::new(),
                                tangent: Vec::new(),
                            });
                            active_points.insert(point_key, index);
                            index
                        });
                    let track = &mut point_tracks[point_track_index];
                    if track.class != manifold.class {
                        return Err("stationarity point class changed within a track".to_owned());
                    }
                    track.last_step = snapshot.completed_steps;
                    track.normal.push(f64::from(point.normal_impulse));
                    track.tangent.push(f64::from(point.tangent_impulse));
                }
            }
        }
    }

    let mut classes = Vec::with_capacity(3);
    for class in StationarityManifoldClassV1::ALL {
        let eligible_points = point_tracks
            .iter()
            .filter(|track| track.class == class && track.normal.len() >= 2)
            .collect::<Vec<_>>();
        let eligible_manifolds = manifold_tracks
            .iter()
            .filter(|track| track.class == class && track.normal.len() >= 2)
            .collect::<Vec<_>>();
        if eligible_points.is_empty() || eligible_manifolds.is_empty() {
            return Err(format!(
                "stationarity class {} has no continuous tracks",
                class.name()
            ));
        }
        let mut maxima = StationarityMetricMaximaV1::zero();
        for track in &eligible_points {
            maxima.normal_point_amplitude_max = maxima
                .normal_point_amplitude_max
                .max(amplitude(&track.normal));
            maxima.normal_point_variance_max = maxima
                .normal_point_variance_max
                .max(population_variance(&track.normal));
            maxima.tangent_point_amplitude_max = maxima
                .tangent_point_amplitude_max
                .max(amplitude(&track.tangent));
        }
        let mut comparable_active_set_transitions = 0_u64;
        let mut comparable_redistribution_transitions = 0_u64;
        let mut correlation_pairs = Vec::with_capacity(eligible_manifolds.len());
        let mut local_speed_max = 0.0_f64;
        for track in &eligible_manifolds {
            let normal_amplitude = amplitude(&track.normal);
            maxima.normal_manifold_amplitude_max =
                maxima.normal_manifold_amplitude_max.max(normal_amplitude);
            maxima.normal_manifold_variance_max = maxima
                .normal_manifold_variance_max
                .max(population_variance(&track.normal));
            maxima.tangent_manifold_amplitude_max = maxima
                .tangent_manifold_amplitude_max
                .max(amplitude(&track.tangent));
            let mut active_flips = 0_u64;
            for index in 1..track.active_sets.len() {
                if track.point_keys[index] == track.point_keys[index - 1] {
                    comparable_active_set_transitions += 1;
                    comparable_redistribution_transitions += 1;
                    if track.active_sets[index] != track.active_sets[index - 1] {
                        active_flips += 1;
                    }
                    maxima.redistribution_transition_max =
                        maxima.redistribution_transition_max.max(redistribution(
                            &track.point_normals[index - 1],
                            &track.point_normals[index],
                        )?);
                }
            }
            maxima.active_set_flip_count_max = maxima.active_set_flip_count_max.max(active_flips);
            let speed = track.local_speed.iter().copied().fold(0.0_f64, f64::max);
            local_speed_max = local_speed_max.max(speed);
            correlation_pairs.push((speed, normal_amplitude));
        }
        classes.push(StationarityClassAnalysisV1 {
            class,
            point_tracks: eligible_points.len() as u64,
            manifold_tracks: eligible_manifolds.len() as u64,
            point_identity_breaks: identity_breaks_points(&eligible_points),
            manifold_identity_breaks: identity_breaks_manifolds(&eligible_manifolds),
            comparable_active_set_transitions,
            comparable_redistribution_transitions,
            local_speed_max,
            local_speed_normal_amplitude_correlation: correlation(&correlation_pairs),
            maxima,
        });
    }
    let mut engines = captures
        .iter()
        .map(|capture| capture.engine.clone())
        .collect::<Vec<_>>();
    engines.sort();
    engines.dedup();
    let mut scenes = captures
        .iter()
        .map(|capture| capture.scene_label.clone())
        .collect::<Vec<_>>();
    scenes.sort();
    scenes.dedup();
    Ok(StationarityAnalysisV1 {
        version: 1,
        engines,
        scenes,
        classes,
    })
}

fn validate_capture(capture: &StationarityCaptureV1) -> Result<(), String> {
    if capture.version != STATIONARITY_CAPTURE_VERSION
        || capture.observation_version != 2
        || capture.settled_start_step != 900
        || capture.settled_end_step != 1_200
        || capture.snapshots.len() != 301
        || capture.snapshots.first().map(|value| value.completed_steps) != Some(900)
        || capture.snapshots.last().map(|value| value.completed_steps) != Some(1_200)
        || capture
            .snapshots
            .windows(2)
            .any(|pair| pair[1].completed_steps != pair[0].completed_steps + 1)
    {
        return Err("invalid stationarity capture contract".to_owned());
    }
    for snapshot in &capture.snapshots {
        if snapshot
            .manifolds
            .windows(2)
            .any(|pair| pair[0].pair >= pair[1].pair)
        {
            return Err("stationarity manifolds are not sorted and unique".to_owned());
        }
        for manifold in &snapshot.manifolds {
            if manifold.points.is_empty()
                || manifold.points.len() > 2
                || manifold
                    .points
                    .windows(2)
                    .any(|pair| pair[0].key >= pair[1].key)
                || manifold.points.iter().any(|point| {
                    !point.normal_impulse.is_finite()
                        || point.normal_impulse < 0.0
                        || !point.tangent_impulse.is_finite()
                })
                || !manifold.supported_body_center_speed.is_finite()
                || manifold.supported_body_center_speed < 0.0
                || !manifold.total_normal_impulse.is_finite()
                || manifold.total_normal_impulse < 0.0
                || !manifold.total_tangent_impulse.is_finite()
            {
                return Err("stationarity manifold sample is invalid".to_owned());
            }
            if manifold
                .points
                .iter()
                .any(|point| point.key.pair != manifold.pair)
                || active_set(&manifold.points)? != manifold.active_set
                || manifold.total_normal_impulse.to_bits()
                    != manifold
                        .points
                        .iter()
                        .map(|point| f64::from(point.normal_impulse))
                        .sum::<f64>()
                        .to_bits()
                || manifold.total_tangent_impulse.to_bits()
                    != manifold
                        .points
                        .iter()
                        .map(|point| f64::from(point.tangent_impulse))
                        .sum::<f64>()
                        .to_bits()
            {
                return Err("stationarity derived manifold payload differs".to_owned());
            }
        }
    }
    Ok(())
}

fn amplitude(values: &[f64]) -> f64 {
    let minimum = values.iter().copied().fold(f64::INFINITY, f64::min);
    let maximum = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    maximum - minimum
}

fn population_variance(values: &[f64]) -> f64 {
    let mean = values.iter().sum::<f64>() / values.len() as f64;
    values
        .iter()
        .map(|value| {
            let delta = value - mean;
            delta * delta
        })
        .sum::<f64>()
        / values.len() as f64
}

fn redistribution(previous: &[f64], current: &[f64]) -> Result<f64, String> {
    if previous.len() != 2 || current.len() != 2 {
        return Ok(0.0);
    }
    let previous_total = previous.iter().sum::<f64>();
    let current_total = current.iter().sum::<f64>();
    if previous_total == 0.0 && current_total == 0.0 {
        return Ok(0.0);
    }
    if previous_total == 0.0 || current_total == 0.0 {
        return Ok(1.0);
    }
    let value = 0.5
        * ((previous[0] / previous_total - current[0] / current_total).abs()
            + (previous[1] / previous_total - current[1] / current_total).abs());
    if !value.is_finite() || !(0.0..=1.0 + f64::EPSILON).contains(&value) {
        return Err("stationarity redistribution is invalid".to_owned());
    }
    Ok(value.min(1.0))
}

fn identity_breaks_points(tracks: &[&PointTrack]) -> u64 {
    let mut counts = BTreeMap::<(usize, CandidatePairV1, PersistentContactKeyV1), u64>::new();
    for track in tracks {
        *counts
            .entry((track.capture_index, track.pair, track.key))
            .or_default() += 1;
    }
    counts.values().map(|count| count.saturating_sub(1)).sum()
}

fn identity_breaks_manifolds(tracks: &[&ManifoldTrack]) -> u64 {
    let mut counts = BTreeMap::<(usize, CandidatePairV1), u64>::new();
    for track in tracks {
        *counts.entry((track.capture_index, track.pair)).or_default() += 1;
    }
    counts.values().map(|count| count.saturating_sub(1)).sum()
}

fn correlation(values: &[(f64, f64)]) -> Option<f64> {
    if values.is_empty() {
        return None;
    }
    let mean_x = values.iter().map(|value| value.0).sum::<f64>() / values.len() as f64;
    let mean_y = values.iter().map(|value| value.1).sum::<f64>() / values.len() as f64;
    let variance_x = values
        .iter()
        .map(|value| (value.0 - mean_x).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    let variance_y = values
        .iter()
        .map(|value| (value.1 - mean_y).powi(2))
        .sum::<f64>()
        / values.len() as f64;
    if variance_x == 0.0 || variance_y == 0.0 {
        return None;
    }
    let covariance = values
        .iter()
        .map(|value| (value.0 - mean_x) * (value.1 - mean_y))
        .sum::<f64>()
        / values.len() as f64;
    Some(covariance / (variance_x.sqrt() * variance_y.sqrt()))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum StationarityRatioV1 {
    Finite { value: f64 },
    BothZero,
    AuthorityZeroCandidatePositive,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationarityMetricComparisonV1 {
    pub metric: String,
    pub candidate: f64,
    pub authority_band: f64,
    pub ratio: StationarityRatioV1,
    pub exceeds_authority: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum StationarityClassVerdictV1 {
    Stationary,
    Oscillating,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StationarityClassComparisonV1 {
    pub class: StationarityManifoldClassV1,
    pub verdict: StationarityClassVerdictV1,
    pub exceeding_metrics: Vec<String>,
    pub candidate_local_speed_correlation: Option<f64>,
    pub authority_local_speed_correlation: Option<f64>,
    pub metrics: Vec<StationarityMetricComparisonV1>,
}

pub fn compare_stationarity_v1(
    candidate: &StationarityAnalysisV1,
    authority: &StationarityAnalysisV1,
) -> Result<Vec<StationarityClassComparisonV1>, String> {
    let mut output = Vec::with_capacity(3);
    for class in StationarityManifoldClassV1::ALL {
        let candidate_class = candidate
            .classes
            .iter()
            .find(|value| value.class == class)
            .ok_or_else(|| format!("candidate omits class {}", class.name()))?;
        let authority_class = authority
            .classes
            .iter()
            .find(|value| value.class == class)
            .ok_or_else(|| format!("authority omits class {}", class.name()))?;
        let candidate_values = candidate_class.maxima.named_values();
        let authority_values = authority_class.maxima.named_values();
        if candidate_values.keys().ne(authority_values.keys()) {
            return Err("stationarity metric names differ between engines".to_owned());
        }
        let mut metrics = Vec::with_capacity(candidate_values.len());
        let mut exceeding_metrics = Vec::new();
        for (name, candidate_value) in candidate_values {
            let authority_band = authority_values[&name];
            let exceeds_authority = candidate_value > authority_band;
            if exceeds_authority {
                exceeding_metrics.push(name.to_owned());
            }
            let ratio = if authority_band > 0.0 {
                StationarityRatioV1::Finite {
                    value: candidate_value / authority_band,
                }
            } else if candidate_value == 0.0 {
                StationarityRatioV1::BothZero
            } else {
                StationarityRatioV1::AuthorityZeroCandidatePositive
            };
            metrics.push(StationarityMetricComparisonV1 {
                metric: name.to_owned(),
                candidate: candidate_value,
                authority_band,
                ratio,
                exceeds_authority,
            });
        }
        output.push(StationarityClassComparisonV1 {
            class,
            verdict: if exceeding_metrics.is_empty() {
                StationarityClassVerdictV1::Stationary
            } else {
                StationarityClassVerdictV1::Oscillating
            },
            exceeding_metrics,
            candidate_local_speed_correlation: candidate_class
                .local_speed_normal_amplitude_correlation,
            authority_local_speed_correlation: authority_class
                .local_speed_normal_amplitude_correlation,
            metrics,
        });
    }
    Ok(output)
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

    #[test]
    fn redistribution_and_zero_ratio_semantics_are_exact() {
        assert_eq!(redistribution(&[0.0, 0.0], &[0.0, 0.0]).unwrap(), 0.0);
        assert_eq!(redistribution(&[0.0, 0.0], &[1.0, 0.0]).unwrap(), 1.0);
        assert_eq!(redistribution(&[1.0, 0.0], &[0.0, 1.0]).unwrap(), 1.0);
        assert_eq!(population_variance(&[1.0, 3.0]), 1.0);
        assert_eq!(amplitude(&[1.0, 3.0]), 2.0);
        assert_eq!(correlation(&[(1.0, 2.0), (2.0, 4.0)]), Some(1.0));
        assert_eq!(correlation(&[(1.0, 2.0), (1.0, 3.0)]), None);
    }
}
