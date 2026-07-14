use physbox_launch::{LaunchOptions, ValidationScenario};

#[derive(Clone, Debug)]
pub struct EngineConfig {
    pub scenario: ValidationScenario,
}

impl From<&LaunchOptions> for EngineConfig {
    fn from(options: &LaunchOptions) -> Self {
        Self {
            scenario: options.scenario,
        }
    }
}
