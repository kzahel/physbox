use physbox_launch::{
    LaunchOptions, LaunchPreferenceSelection, ValidationScenario, WorldCapacityPreset,
};
use sha2::{Digest, Sha256};

use crate::SolverProfileConfig;

#[derive(Clone, Debug, Default)]
pub struct EngineConfig {
    pub scenario: ValidationScenario,
    pub world_capacity: WorldCapacityPreset,
    pub solver_profile: SolverProfileConfig,
}

impl TryFrom<&LaunchOptions> for EngineConfig {
    type Error = String;

    fn try_from(options: &LaunchOptions) -> Result<Self, Self::Error> {
        let solver_profile = SolverProfileConfig::from_physics(options.physics)?;
        if !solver_profile.supports_scenario(options.scenario) {
            return Err(format!(
                "solver profile {} does not support scenario {}",
                solver_profile.name(),
                options.scenario.name()
            ));
        }
        Ok(Self {
            scenario: options.scenario,
            world_capacity: options.world_capacity,
            solver_profile,
        })
    }
}

impl EngineConfig {
    pub fn launch_preference_selection(&self) -> LaunchPreferenceSelection {
        LaunchPreferenceSelection {
            world_capacity: self.world_capacity,
            physics: self.solver_profile.physics(),
        }
    }

    pub fn physics_configuration_hash(&self) -> String {
        let encoded = serde_json::to_vec(&self.solver_profile.physics())
            .expect("validated physics configurations are serializable");
        format!("{:x}", Sha256::digest(encoded))
    }
}

#[cfg(all(test, feature = "solver-lab"))]
mod tests {
    use super::*;

    #[test]
    fn lab_profile_is_projected_once_and_rejects_non_generated_scenes() {
        let mut options = LaunchOptions {
            scenario: ValidationScenario::SolverLabStack16,
            physics: physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED,
            ..Default::default()
        };
        let config = EngineConfig::try_from(&options).unwrap();
        assert_eq!(config.solver_profile.name(), "soft-coupled");
        assert_eq!(config.world_capacity, WorldCapacityPreset::Slots64);

        options.scenario = ValidationScenario::SolverManifoldI2;
        let error = EngineConfig::try_from(&options).unwrap_err();
        assert!(error.contains("does not support"));
    }

    #[test]
    fn configuration_hash_covers_every_applied_physics_field() {
        let mut options = LaunchOptions {
            physics: physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED,
            ..Default::default()
        };
        let original = EngineConfig::try_from(&options).unwrap();
        let original_hash = original.physics_configuration_hash();
        assert_eq!(original_hash.len(), 64);
        assert_eq!(original_hash, original.physics_configuration_hash());

        options.physics.contact_damping_ratio = 9.5;
        let changed = EngineConfig::try_from(&options).unwrap();
        assert_ne!(original_hash, changed.physics_configuration_hash());

        options.physics = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
        options.physics.sleeping_enabled = true;
        let sleeping_requested = EngineConfig::try_from(&options).unwrap();
        assert_ne!(
            original_hash,
            sleeping_requested.physics_configuration_hash()
        );

        options.physics = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
        options.physics.sleep_speed_threshold = 0.05;
        assert_ne!(
            original_hash,
            EngineConfig::try_from(&options)
                .unwrap()
                .physics_configuration_hash()
        );

        options.physics = physbox_launch::PhysicsConfigurationV3::SOFT_COUPLED;
        options.physics.sleep_grouping = physbox_launch::SleepGrouping::Exact;
        assert_ne!(
            original_hash,
            EngineConfig::try_from(&options)
                .unwrap()
                .physics_configuration_hash()
        );
    }
}
