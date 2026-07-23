use serde::{Deserialize, Serialize};

use super::{
    LaunchError, LaunchOptions, LaunchRequest, PHYSICS_CONFIGURATION_VERSION,
    PhysicsConfigurationV3, SleepGrouping, SolverProfileName, WorldCapacityPreset,
    apply_legacy_solver_options, parse_web_query,
};

const PHYSICS_PREFERENCE_KEY: &str = "physbox5.physics.v4";
const PRIOR_PHYSICS_PREFERENCE_KEY: &str = "physbox5.physics.v3";
const PRIOR_V2_PHYSICS_PREFERENCE_KEY: &str = "physbox5.physics.v2";
const WORLD_PREFERENCE_KEY: &str = "physbox5.world.v1";
const LEGACY_SOLVER_PREFERENCE_KEY: &str = "physbox5.solver-lab.v1";

const WORLD_CAPACITY_QUERY_KEY: &str = "body-capacity";
const PHYSICS_QUERY_KEYS: [&str; 13] = [
    "solver",
    "sim-hz",
    "substeps",
    "iterations",
    "contact-hz",
    "contact-damping",
    "static-contact-scale",
    "speculative-distance",
    "max-depenetration-velocity",
    "sleeping",
    "sleep-threshold",
    "sleep-grouping",
    "solver-options",
];

pub trait WebPreferenceStore {
    fn read(&self, key: &str) -> Result<Option<String>, String>;
    fn write(&self, key: &str, value: &str) -> Result<(), String>;
    fn remove(&self, key: &str) -> Result<(), String>;
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LaunchPreferenceSelection {
    pub world_capacity: WorldCapacityPreset,
    pub physics: PhysicsConfigurationV3,
}

impl From<&LaunchOptions> for LaunchPreferenceSelection {
    fn from(options: &LaunchOptions) -> Self {
        Self {
            world_capacity: options.world_capacity,
            physics: options.physics,
        }
    }
}

#[derive(Debug)]
pub struct WebPreferenceController {
    query: Vec<(String, String)>,
    last_physics: Option<String>,
    last_world: Option<String>,
}

impl WebPreferenceController {
    pub fn restore<S: WebPreferenceStore>(
        query: &str,
        store: &S,
    ) -> Result<(Self, LaunchRequest), LaunchError> {
        let mut query = query_pairs(query);
        let has_explicit_physics = query
            .iter()
            .any(|(key, _)| PHYSICS_QUERY_KEYS.contains(&key.as_str()));
        if !has_explicit_physics && let Some(physics) = load_physics_preference(store) {
            append_physics_query(&mut query, physics);
        }
        let has_explicit_capacity = query.iter().any(|(key, _)| key == WORLD_CAPACITY_QUERY_KEY);
        if !has_explicit_capacity && let Some(capacity) = load_world_preference(store) {
            query.push((
                WORLD_CAPACITY_QUERY_KEY.to_owned(),
                capacity.slots().to_string(),
            ));
        }

        let request = parse_web_query(&encode_query(&query))?;
        Ok((
            Self {
                query,
                last_physics: None,
                last_world: None,
            },
            request,
        ))
    }

    pub fn synchronize<S: WebPreferenceStore>(
        &mut self,
        selection: LaunchPreferenceSelection,
        store: &S,
    ) -> Option<String> {
        let physics = PhysicsPreferenceV4::from(selection.physics);
        let encoded_physics =
            serde_json::to_string(&physics).expect("validated physics preference is serializable");
        if self.last_physics.as_deref() != Some(encoded_physics.as_str()) {
            let _ = store.write(PHYSICS_PREFERENCE_KEY, &encoded_physics);
            let _ = store.remove(PRIOR_PHYSICS_PREFERENCE_KEY);
            let _ = store.remove(PRIOR_V2_PHYSICS_PREFERENCE_KEY);
            let _ = store.remove(LEGACY_SOLVER_PREFERENCE_KEY);
            self.last_physics = Some(encoded_physics);
        }

        let world = WorldPreferenceV1::from(selection.world_capacity);
        let encoded_world =
            serde_json::to_string(&world).expect("validated world preference is serializable");
        if self.last_world.as_deref() != Some(encoded_world.as_str()) {
            let _ = store.write(WORLD_PREFERENCE_KEY, &encoded_world);
            self.last_world = Some(encoded_world);
        }

        let previous = encode_query(&self.query);
        self.query.retain(|(key, _)| {
            key != WORLD_CAPACITY_QUERY_KEY && !PHYSICS_QUERY_KEYS.contains(&key.as_str())
        });
        if selection.world_capacity != WorldCapacityPreset::default() {
            self.query.push((
                WORLD_CAPACITY_QUERY_KEY.to_owned(),
                selection.world_capacity.slots().to_string(),
            ));
        }
        if selection.physics != PhysicsConfigurationV3::default() {
            append_physics_query(&mut self.query, selection.physics);
        }
        let next = encode_query(&self.query);
        (next != previous).then_some(next)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct PhysicsPreferenceV4 {
    version: u32,
    solver: SolverProfileName,
    sim_hz: u32,
    substeps: u32,
    iterations: u32,
    contact_hz: f32,
    contact_damping: f32,
    static_contact_scale: f32,
    speculative_distance: f32,
    max_depenetration_velocity: f32,
    sleeping: bool,
    sleep_threshold: f32,
    sleep_grouping: SleepGrouping,
}

impl From<PhysicsConfigurationV3> for PhysicsPreferenceV4 {
    fn from(physics: PhysicsConfigurationV3) -> Self {
        Self {
            version: 4,
            solver: physics.solver_profile,
            sim_hz: physics.fixed_step_hz,
            substeps: physics.substeps,
            iterations: physics.biased_iterations,
            contact_hz: physics.contact_hertz,
            contact_damping: physics.contact_damping_ratio,
            static_contact_scale: physics.static_contact_hertz_multiplier,
            speculative_distance: physics.speculative_distance,
            max_depenetration_velocity: physics.max_depenetration_velocity,
            sleeping: physics.sleeping_enabled,
            sleep_threshold: physics.sleep_speed_threshold,
            sleep_grouping: physics.sleep_grouping,
        }
    }
}

impl PhysicsPreferenceV4 {
    fn into_physics(self) -> Option<PhysicsConfigurationV3> {
        if self.version != 4 {
            return None;
        }
        let physics = PhysicsConfigurationV3 {
            version: PHYSICS_CONFIGURATION_VERSION,
            solver_profile: self.solver,
            fixed_step_hz: self.sim_hz,
            substeps: self.substeps,
            biased_iterations: self.iterations,
            contact_hertz: self.contact_hz,
            contact_damping_ratio: self.contact_damping,
            static_contact_hertz_multiplier: self.static_contact_scale,
            speculative_distance: self.speculative_distance,
            max_depenetration_velocity: self.max_depenetration_velocity,
            sleeping_enabled: self.sleeping,
            sleep_speed_threshold: self.sleep_threshold,
            sleep_grouping: self.sleep_grouping,
        };
        if physics.validate().is_err() {
            return None;
        }
        if physics.solver_profile == SolverProfileName::HardDirect
            && (physics.fixed_step_hz != PhysicsConfigurationV3::HARD_DIRECT.fixed_step_hz
                || physics.substeps != PhysicsConfigurationV3::HARD_DIRECT.substeps
                || physics.biased_iterations
                    != PhysicsConfigurationV3::HARD_DIRECT.biased_iterations)
        {
            return None;
        }
        Some(physics)
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhysicsPreferenceV3 {
    version: u32,
    solver: SolverProfileName,
    sim_hz: u32,
    substeps: u32,
    iterations: u32,
    contact_hz: f32,
    contact_damping: f32,
    static_contact_scale: f32,
    speculative_distance: f32,
    max_depenetration_velocity: f32,
    sleeping: bool,
}

impl PhysicsPreferenceV3 {
    fn migrate(self) -> Option<PhysicsConfigurationV3> {
        if self.version != 3 {
            return None;
        }
        PhysicsPreferenceV4 {
            version: 4,
            solver: self.solver,
            sim_hz: self.sim_hz,
            substeps: self.substeps,
            iterations: self.iterations,
            contact_hz: self.contact_hz,
            contact_damping: self.contact_damping,
            static_contact_scale: self.static_contact_scale,
            speculative_distance: self.speculative_distance,
            max_depenetration_velocity: self.max_depenetration_velocity,
            sleeping: self.sleeping,
            sleep_threshold: PhysicsConfigurationV3::SOFT_COUPLED.sleep_speed_threshold,
            sleep_grouping: SleepGrouping::Local4,
        }
        .into_physics()
    }
}

#[derive(Clone, Copy, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PhysicsPreferenceV2 {
    version: u32,
    solver: SolverProfileName,
    sim_hz: u32,
    substeps: u32,
    iterations: u32,
    contact_hz: f32,
    contact_damping: f32,
    static_contact_scale: f32,
    speculative_distance: f32,
    max_depenetration_velocity: f32,
}

impl PhysicsPreferenceV2 {
    fn migrate(self) -> Option<PhysicsConfigurationV3> {
        if self.version != 2 {
            return None;
        }
        PhysicsPreferenceV4 {
            version: 4,
            solver: self.solver,
            sim_hz: self.sim_hz,
            substeps: self.substeps,
            iterations: self.iterations,
            contact_hz: self.contact_hz,
            contact_damping: self.contact_damping,
            static_contact_scale: self.static_contact_scale,
            speculative_distance: self.speculative_distance,
            max_depenetration_velocity: self.max_depenetration_velocity,
            sleeping: false,
            sleep_threshold: PhysicsConfigurationV3::SOFT_COUPLED.sleep_speed_threshold,
            sleep_grouping: SleepGrouping::Local4,
        }
        .into_physics()
    }
}

#[derive(Debug, Deserialize)]
struct LegacySolverPreferenceV1 {
    version: u32,
    solver: String,
    options: Option<String>,
}

impl LegacySolverPreferenceV1 {
    fn migrate(self) -> Option<PhysicsConfigurationV3> {
        if self.version != 1 {
            return None;
        }
        let profile = match self.solver.as_str() {
            "selected" => SolverProfileName::HardDirect,
            "branch-robust" => SolverProfileName::SoftCoupled,
            _ => return None,
        };
        let mut physics = PhysicsConfigurationV3::for_profile(profile);
        if let Some(options) = self.options {
            apply_legacy_solver_options(&mut physics, &mut Default::default(), &options).ok()?;
        }
        PhysicsPreferenceV4::from(physics).into_physics()
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
struct WorldPreferenceV1 {
    version: u32,
    body_capacity: u32,
}

impl From<WorldCapacityPreset> for WorldPreferenceV1 {
    fn from(capacity: WorldCapacityPreset) -> Self {
        Self {
            version: 1,
            body_capacity: capacity.slots(),
        }
    }
}

impl WorldPreferenceV1 {
    fn into_capacity(self) -> Option<WorldCapacityPreset> {
        if self.version != 1 {
            return None;
        }
        WorldCapacityPreset::ALL
            .into_iter()
            .find(|capacity| capacity.slots() == self.body_capacity)
    }
}

fn load_physics_preference<S: WebPreferenceStore>(store: &S) -> Option<PhysicsConfigurationV3> {
    read_json::<PhysicsPreferenceV4, _>(store, PHYSICS_PREFERENCE_KEY)
        .and_then(PhysicsPreferenceV4::into_physics)
        .or_else(|| {
            read_json::<PhysicsPreferenceV3, _>(store, PRIOR_PHYSICS_PREFERENCE_KEY)
                .and_then(PhysicsPreferenceV3::migrate)
        })
        .or_else(|| {
            read_json::<PhysicsPreferenceV2, _>(store, PRIOR_V2_PHYSICS_PREFERENCE_KEY)
                .and_then(PhysicsPreferenceV2::migrate)
        })
        .or_else(|| {
            read_json::<LegacySolverPreferenceV1, _>(store, LEGACY_SOLVER_PREFERENCE_KEY)
                .and_then(LegacySolverPreferenceV1::migrate)
        })
}

fn load_world_preference<S: WebPreferenceStore>(store: &S) -> Option<WorldCapacityPreset> {
    read_json::<WorldPreferenceV1, _>(store, WORLD_PREFERENCE_KEY)
        .and_then(WorldPreferenceV1::into_capacity)
}

fn read_json<T: for<'de> Deserialize<'de>, S: WebPreferenceStore>(
    store: &S,
    key: &str,
) -> Option<T> {
    store
        .read(key)
        .ok()
        .flatten()
        .and_then(|value| serde_json::from_str(&value).ok())
}

fn append_physics_query(query: &mut Vec<(String, String)>, physics: PhysicsConfigurationV3) {
    query.extend([
        (
            "solver".to_owned(),
            physics.solver_profile.name().to_owned(),
        ),
        ("sim-hz".to_owned(), physics.fixed_step_hz.to_string()),
        ("substeps".to_owned(), physics.substeps.to_string()),
        (
            "iterations".to_owned(),
            physics.biased_iterations.to_string(),
        ),
        ("contact-hz".to_owned(), physics.contact_hertz.to_string()),
        (
            "contact-damping".to_owned(),
            physics.contact_damping_ratio.to_string(),
        ),
        (
            "static-contact-scale".to_owned(),
            physics.static_contact_hertz_multiplier.to_string(),
        ),
        (
            "speculative-distance".to_owned(),
            physics.speculative_distance.to_string(),
        ),
        (
            "max-depenetration-velocity".to_owned(),
            physics.max_depenetration_velocity.to_string(),
        ),
        (
            "sleeping".to_owned(),
            if physics.sleeping_enabled {
                "on"
            } else {
                "off"
            }
            .to_owned(),
        ),
        (
            "sleep-threshold".to_owned(),
            physics.sleep_speed_threshold.to_string(),
        ),
        (
            "sleep-grouping".to_owned(),
            physics.sleep_grouping.name().to_owned(),
        ),
    ]);
}

fn query_pairs(query: &str) -> Vec<(String, String)> {
    form_urlencoded::parse(query.trim_start_matches('?').as_bytes())
        .map(|(key, value)| (key.into_owned(), value.into_owned()))
        .collect()
}

fn encode_query(query: &[(String, String)]) -> String {
    if query.is_empty() {
        return String::new();
    }
    let mut encoded = form_urlencoded::Serializer::new(String::new());
    encoded.extend_pairs(
        query
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str())),
    );
    format!("?{}", encoded.finish())
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::collections::BTreeMap;

    use super::*;

    #[derive(Default)]
    struct MemoryStore {
        values: RefCell<BTreeMap<String, String>>,
    }

    impl MemoryStore {
        fn with(self, key: &str, value: &str) -> Self {
            self.values
                .borrow_mut()
                .insert(key.to_owned(), value.to_owned());
            self
        }

        fn get(&self, key: &str) -> Option<String> {
            self.values.borrow().get(key).cloned()
        }
    }

    impl WebPreferenceStore for MemoryStore {
        fn read(&self, key: &str) -> Result<Option<String>, String> {
            Ok(self.get(key))
        }

        fn write(&self, key: &str, value: &str) -> Result<(), String> {
            self.values
                .borrow_mut()
                .insert(key.to_owned(), value.to_owned());
            Ok(())
        }

        fn remove(&self, key: &str) -> Result<(), String> {
            self.values.borrow_mut().remove(key);
            Ok(())
        }
    }

    fn custom_preference(version: u32) -> String {
        format!(
            r#"{{"version":{version},"solver":"soft-coupled","simHz":120,"substeps":8,"iterations":2,"contactHz":45.5,"contactDamping":8.0,"staticContactScale":3.0,"speculativeDistance":0.03,"maxDepenetrationVelocity":4.0{}}}"#,
            if version == 4 {
                r#","sleeping":true,"sleepThreshold":0.05,"sleepGrouping":"exact""#
            } else if version == 3 {
                r#","sleeping":true"#
            } else {
                ""
            }
        )
    }

    #[test]
    fn current_preferences_restore_before_shared_web_parsing() {
        let store = MemoryStore::default()
            .with(PHYSICS_PREFERENCE_KEY, &custom_preference(4))
            .with(WORLD_PREFERENCE_KEY, r#"{"version":1,"bodyCapacity":1000}"#);
        let (_, request) =
            WebPreferenceController::restore("?scenario=rigid-floor", &store).unwrap();
        assert_eq!(request.options.scenario.name(), "rigid-floor");
        assert_eq!(
            request.options.world_capacity,
            WorldCapacityPreset::Slots1000
        );
        assert_eq!(request.options.physics.fixed_step_hz, 120);
        assert_eq!(request.options.physics.substeps, 8);
        assert!(request.options.physics.sleeping_enabled);
        assert_eq!(request.options.physics.sleep_speed_threshold, 0.05);
        assert_eq!(request.options.physics.sleep_grouping, SleepGrouping::Exact);
    }

    #[test]
    fn explicit_query_has_precedence_over_saved_preferences() {
        let store = MemoryStore::default()
            .with(PHYSICS_PREFERENCE_KEY, &custom_preference(4))
            .with(WORLD_PREFERENCE_KEY, r#"{"version":1,"bodyCapacity":1000}"#);
        let (_, request) =
            WebPreferenceController::restore("?solver=hard-direct&body-capacity=256", &store)
                .unwrap();
        assert_eq!(request.options.physics, PhysicsConfigurationV3::HARD_DIRECT);
        assert_eq!(
            request.options.world_capacity,
            WorldCapacityPreset::Slots256
        );
    }

    #[test]
    fn prior_and_legacy_physics_records_migrate() {
        let prior =
            MemoryStore::default().with(PRIOR_PHYSICS_PREFERENCE_KEY, &custom_preference(3));
        let (_, request) = WebPreferenceController::restore("", &prior).unwrap();
        assert_eq!(request.options.physics.fixed_step_hz, 120);
        assert!(request.options.physics.sleeping_enabled);
        assert_eq!(request.options.physics.sleep_speed_threshold, 0.02);
        assert_eq!(
            request.options.physics.sleep_grouping,
            SleepGrouping::Local4
        );

        let legacy = MemoryStore::default().with(
            LEGACY_SOLVER_PREFERENCE_KEY,
            r#"{"version":1,"solver":"branch-robust","options":"hz=30,damping=10,static-scale=8"}"#,
        );
        let (_, request) = WebPreferenceController::restore("", &legacy).unwrap();
        assert_eq!(
            request.options.physics.solver_profile,
            SolverProfileName::SoftCoupled
        );
        assert_eq!(request.options.physics.static_contact_hertz_multiplier, 8.0);
    }

    #[test]
    fn malformed_and_invalid_records_are_ignored() {
        let store = MemoryStore::default()
            .with(PHYSICS_PREFERENCE_KEY, "{")
            .with(
                PRIOR_PHYSICS_PREFERENCE_KEY,
                r#"{"version":2,"solver":"hard-direct","simHz":120,"substeps":4,"iterations":2,"contactHz":30.0,"contactDamping":10.0,"staticContactScale":2.0,"speculativeDistance":0.02,"maxDepenetrationVelocity":3.0}"#,
            )
            .with(
                WORLD_PREFERENCE_KEY,
                r#"{"version":1,"bodyCapacity":4096}"#,
            );
        let (_, request) = WebPreferenceController::restore("", &store).unwrap();
        assert_eq!(request.options, LaunchOptions::default());
    }

    #[test]
    fn synchronization_persists_effective_values_and_canonicalizes_query() {
        let store = MemoryStore::default().with(
            LEGACY_SOLVER_PREFERENCE_KEY,
            r#"{"version":1,"solver":"selected","options":null}"#,
        );
        let (mut controller, _) =
            WebPreferenceController::restore("?scenario=rigid-stack", &store).unwrap();
        let mut physics = PhysicsConfigurationV3::SOFT_COUPLED;
        physics.static_contact_hertz_multiplier = 2.5;
        let search = controller
            .synchronize(
                LaunchPreferenceSelection {
                    world_capacity: WorldCapacityPreset::Slots256,
                    physics,
                },
                &store,
            )
            .unwrap();
        assert!(search.starts_with("?scenario=rigid-stack&body-capacity=256&"));
        assert!(search.contains("solver=soft-coupled"));
        assert!(search.contains("static-contact-scale=2.5"));
        assert!(store.get(PRIOR_PHYSICS_PREFERENCE_KEY).is_none());
        assert!(store.get(LEGACY_SOLVER_PREFERENCE_KEY).is_none());

        let saved: PhysicsPreferenceV4 =
            serde_json::from_str(&store.get(PHYSICS_PREFERENCE_KEY).unwrap()).unwrap();
        assert_eq!(saved.static_contact_scale, 2.5);
        let world: WorldPreferenceV1 =
            serde_json::from_str(&store.get(WORLD_PREFERENCE_KEY).unwrap()).unwrap();
        assert_eq!(world.body_capacity, 256);
    }

    #[test]
    fn default_effective_values_elide_owned_query_fields() {
        let store = MemoryStore::default();
        let (mut controller, _) = WebPreferenceController::restore(
            "?scenario=rigid-floor&solver=soft-coupled&body-capacity=64",
            &store,
        )
        .unwrap();
        let search = controller
            .synchronize(
                LaunchPreferenceSelection {
                    world_capacity: WorldCapacityPreset::default(),
                    physics: PhysicsConfigurationV3::default(),
                },
                &store,
            )
            .unwrap();
        assert_eq!(search, "?scenario=rigid-floor");
        assert!(
            controller
                .synchronize(
                    LaunchPreferenceSelection {
                        world_capacity: WorldCapacityPreset::default(),
                        physics: PhysicsConfigurationV3::default(),
                    },
                    &store,
                )
                .is_none()
        );
    }
}
