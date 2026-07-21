use physbox_launch::{LaunchOptions, ValidationScenario};

use crate::SolverProfileConfig;

#[derive(Clone, Debug, Default)]
pub struct EngineConfig {
    pub scenario: ValidationScenario,
    pub solver_profile: SolverProfileConfig,
}

impl TryFrom<&LaunchOptions> for EngineConfig {
    type Error = String;

    fn try_from(options: &LaunchOptions) -> Result<Self, Self::Error> {
        let solver_profile = SolverProfileConfig::from_launch(&options.solver)?;
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

#[cfg(all(test, feature = "solver-lab"))]
mod tests {
    use super::*;

    #[test]
    fn lab_profile_is_projected_once_and_rejects_non_generated_scenes() {
        let mut options = LaunchOptions {
            scenario: ValidationScenario::SolverLabStack16,
            solver: physbox_launch::SolverSelection {
                profile: physbox_launch::SolverProfileName::BranchRobust,
                options: Some("hz=30,damping=10,static-scale=8".to_owned()),
            },
            ..Default::default()
        };
        let config = EngineConfig::try_from(&options).unwrap();
        assert_eq!(config.solver_profile.name(), "branch-robust");

        options.scenario = ValidationScenario::BodyFreeFlight;
        let error = EngineConfig::try_from(&options).unwrap_err();
        assert!(error.contains("does not support"));
    }
}
