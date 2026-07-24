use physbox_launch::{
    FluidSolidCouplingQuality, LaunchOptions, LaunchPreferenceSelection, RenderScalePreset,
    ValidationScenario, WorldCapacityPreset,
};
use sha2::{Digest, Sha256};

use crate::SolverProfileConfig;

#[derive(Clone, Debug, Default)]
pub struct EngineConfig {
    pub scenario: ValidationScenario,
    pub world_capacity: WorldCapacityPreset,
    pub fluid_solid_coupling_quality: FluidSolidCouplingQuality,
    pub solver_profile: SolverProfileConfig,
    pub sleep_visuals_enabled: bool,
    pub render_scale: RenderScalePreset,
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
            fluid_solid_coupling_quality: options.fluid_solid_coupling_quality,
            solver_profile,
            sleep_visuals_enabled: options.sleep_visuals_enabled,
            render_scale: options.render_scale,
        })
    }
}

impl EngineConfig {
    pub fn launch_preference_selection(&self) -> LaunchPreferenceSelection {
        LaunchPreferenceSelection {
            world_capacity: self.world_capacity,
            fluid_solid_coupling_quality: self.fluid_solid_coupling_quality,
            physics: self.solver_profile.physics(),
            sleep_visuals_enabled: self.sleep_visuals_enabled,
            render_scale: self.render_scale,
        }
    }

    pub fn physics_configuration_hash(&self) -> String {
        let encoded = serde_json::to_vec(&self.solver_profile.physics())
            .expect("validated physics configurations are serializable");
        format!("{:x}", Sha256::digest(encoded))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    #[cfg(feature = "solver-lab")]
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
    #[cfg(feature = "solver-lab")]
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
        options.physics.sleeping_enabled = false;
        let sleeping_disabled = EngineConfig::try_from(&options).unwrap();
        assert_ne!(
            original_hash,
            sleeping_disabled.physics_configuration_hash()
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
        options.physics.sleep_grouping = physbox_launch::SleepGrouping::Local4;
        assert_ne!(
            original_hash,
            EngineConfig::try_from(&options)
                .unwrap()
                .physics_configuration_hash()
        );
    }

    #[test]
    fn presentation_launch_preference_reaches_the_shared_engine() {
        let options = LaunchOptions {
            sleep_visuals_enabled: true,
            render_scale: RenderScalePreset::Percent50,
            ..Default::default()
        };
        let config = EngineConfig::try_from(&options).unwrap();
        assert!(config.sleep_visuals_enabled);
        assert!(config.launch_preference_selection().sleep_visuals_enabled);
        assert_eq!(config.render_scale, RenderScalePreset::Percent50);
        assert_eq!(
            config.launch_preference_selection().render_scale,
            RenderScalePreset::Percent50
        );
        assert_eq!(
            config.physics_configuration_hash(),
            EngineConfig::default().physics_configuration_hash()
        );
    }

    #[test]
    fn coupling_quality_is_shared_restart_state_without_changing_rigid_physics_identity() {
        let options = LaunchOptions {
            fluid_solid_coupling_quality: FluidSolidCouplingQuality::Stabilized,
            ..Default::default()
        };
        let config = EngineConfig::try_from(&options).unwrap();
        assert_eq!(
            config.fluid_solid_coupling_quality,
            FluidSolidCouplingQuality::Stabilized
        );
        assert_eq!(
            config
                .launch_preference_selection()
                .fluid_solid_coupling_quality,
            FluidSolidCouplingQuality::Stabilized
        );
        assert_eq!(
            config.physics_configuration_hash(),
            EngineConfig::default().physics_configuration_hash()
        );
    }
}
