use physbox_launch::{LaunchOptions, ValidationScenario};
use sha2::{Digest, Sha256};

use crate::SolverProfileConfig;

#[derive(Clone, Debug, Default)]
pub struct EngineConfig {
    pub scenario: ValidationScenario,
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
            solver_profile,
        })
    }
}

impl EngineConfig {
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
            physics: physbox_launch::PhysicsConfigurationV1::SOFT_COUPLED,
            ..Default::default()
        };
        let config = EngineConfig::try_from(&options).unwrap();
        assert_eq!(config.solver_profile.name(), "soft-coupled");

        options.scenario = ValidationScenario::SolverManifoldI2;
        let error = EngineConfig::try_from(&options).unwrap_err();
        assert!(error.contains("does not support"));
    }

    #[test]
    fn configuration_hash_covers_every_applied_physics_field() {
        let mut options = LaunchOptions {
            physics: physbox_launch::PhysicsConfigurationV1::SOFT_COUPLED,
            ..Default::default()
        };
        let original = EngineConfig::try_from(&options).unwrap();
        let original_hash = original.physics_configuration_hash();
        assert_eq!(original_hash.len(), 64);
        assert_eq!(original_hash, original.physics_configuration_hash());

        options.physics.contact_damping_ratio = 9.5;
        let changed = EngineConfig::try_from(&options).unwrap();
        assert_ne!(original_hash, changed.physics_configuration_hash());
    }
}
