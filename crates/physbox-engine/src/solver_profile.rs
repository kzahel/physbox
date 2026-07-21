use physbox_contracts::ScenarioV1;
#[cfg(feature = "solver-lab")]
use physbox_contracts::{
    CONTACT_SOFTNESS_POLICY_VERSION, ContactSoftnessPolicyV1, FiniteF32, Validate,
};
use physbox_launch::{SolverProfileName, SolverSelection, ValidationScenario};

#[cfg(feature = "solver-lab")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct BranchRobustInteractiveOptions {
    pub hertz: f32,
    pub damping: f32,
    pub static_scale: f32,
}

#[cfg(feature = "solver-lab")]
impl Default for BranchRobustInteractiveOptions {
    fn default() -> Self {
        Self {
            hertz: 30.0,
            damping: 10.0,
            static_scale: 2.0,
        }
    }
}

#[derive(Clone, Debug, Default, PartialEq)]
pub enum SolverProfileConfig {
    #[default]
    Selected,
    #[cfg(feature = "solver-lab")]
    BranchRobust(BranchRobustInteractiveOptions),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SolverParameter {
    Hertz,
    Damping,
    StaticScale,
}

impl SolverParameter {
    pub(crate) const ALL: [Self; 3] = [Self::Hertz, Self::Damping, Self::StaticScale];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::Hertz => "HZ",
            Self::Damping => "DAMPING",
            Self::StaticScale => "STATIC SCALE",
        }
    }
}

impl SolverProfileConfig {
    pub fn from_launch(selection: &SolverSelection) -> Result<Self, String> {
        match selection.profile {
            SolverProfileName::Selected => {
                if selection.options.is_some() {
                    return Err("selected solver does not accept profile options".to_owned());
                }
                Ok(Self::Selected)
            }
            #[cfg(feature = "solver-lab")]
            SolverProfileName::BranchRobust => {
                let mut options = BranchRobustInteractiveOptions::default();
                if let Some(encoded) = &selection.options {
                    let mut seen = std::collections::BTreeSet::new();
                    for field in encoded.split(',') {
                        let (name, value) = field.split_once('=').ok_or_else(|| {
                            format!("solver option {field} must use key=value syntax")
                        })?;
                        if !seen.insert(name) {
                            return Err(format!("duplicate solver option: {name}"));
                        }
                        let value = value
                            .parse::<f32>()
                            .map_err(|error| format!("invalid solver option {field}: {error}"))?;
                        if !value.is_finite() {
                            return Err(format!("solver option {name} must be finite"));
                        }
                        match name {
                            "hz" => options.hertz = value,
                            "damping" => options.damping = value,
                            "static-scale" => options.static_scale = value,
                            _ => return Err(format!("unknown branch-robust option: {name}")),
                        }
                    }
                }
                options.validate()?;
                Ok(Self::BranchRobust(options))
            }
        }
    }

    pub const fn name(&self) -> &'static str {
        match self {
            Self::Selected => "selected",
            #[cfg(feature = "solver-lab")]
            Self::BranchRobust(_) => "branch-robust",
        }
    }

    pub const fn label(&self) -> &'static str {
        match self {
            Self::Selected => "PRODUCT HARD",
            #[cfg(feature = "solver-lab")]
            Self::BranchRobust(_) => "BRANCH ROBUST",
        }
    }

    pub const fn is_selected(&self) -> bool {
        matches!(self, Self::Selected)
    }

    pub fn options_string(&self) -> Option<String> {
        match self {
            Self::Selected => None,
            #[cfg(feature = "solver-lab")]
            Self::BranchRobust(options) => Some(format!(
                "hz={},damping={},static-scale={}",
                options.hertz, options.damping, options.static_scale,
            )),
        }
    }

    #[allow(unused_mut)]
    pub fn available() -> Vec<Self> {
        let mut profiles = vec![Self::Selected];
        #[cfg(feature = "solver-lab")]
        profiles.push(Self::BranchRobust(BranchRobustInteractiveOptions::default()));
        profiles
    }

    pub fn supports_scenario(&self, scenario: ValidationScenario) -> bool {
        #[cfg(not(feature = "solver-lab"))]
        let _ = scenario;
        match self {
            Self::Selected => true,
            #[cfg(feature = "solver-lab")]
            Self::BranchRobust(_) => matches!(
                scenario,
                ValidationScenario::Bootstrap
                    | ValidationScenario::RigidFloor
                    | ValidationScenario::RigidStack
                    | ValidationScenario::RectangleStack
                    | ValidationScenario::CircleBucket100
                    | ValidationScenario::RectangleBucket100
                    | ValidationScenario::ResetGrab
                    | ValidationScenario::SolverLabStack8
                    | ValidationScenario::SolverLabStack16
            ),
        }
    }

    pub(crate) fn select(name: SolverProfileName) -> Self {
        match name {
            SolverProfileName::Selected => Self::Selected,
            #[cfg(feature = "solver-lab")]
            SolverProfileName::BranchRobust => {
                Self::BranchRobust(BranchRobustInteractiveOptions::default())
            }
        }
    }

    pub(crate) fn profile_name(&self) -> SolverProfileName {
        match self {
            Self::Selected => SolverProfileName::Selected,
            #[cfg(feature = "solver-lab")]
            Self::BranchRobust(_) => SolverProfileName::BranchRobust,
        }
    }

    pub(crate) fn parameter_value(&self, parameter: SolverParameter) -> Option<f32> {
        #[cfg(not(feature = "solver-lab"))]
        let _ = parameter;
        match self {
            Self::Selected => None,
            #[cfg(feature = "solver-lab")]
            Self::BranchRobust(options) => Some(match parameter {
                SolverParameter::Hertz => options.hertz,
                SolverParameter::Damping => options.damping,
                SolverParameter::StaticScale => options.static_scale,
            }),
        }
    }

    pub(crate) fn adjust(&mut self, parameter: SolverParameter, direction: i32) {
        #[cfg(not(feature = "solver-lab"))]
        let _ = (parameter, direction);
        #[cfg(feature = "solver-lab")]
        if let Self::BranchRobust(options) = self {
            let (value, step, minimum, maximum) = match parameter {
                SolverParameter::Hertz => (&mut options.hertz, 1.0, 1.0, 120.0),
                SolverParameter::Damping => (&mut options.damping, 0.5, 0.5, 20.0),
                SolverParameter::StaticScale => (&mut options.static_scale, 0.5, 0.5, 16.0),
            };
            *value = (*value + direction.signum() as f32 * step).clamp(minimum, maximum);
        }
    }

    pub(crate) fn apply_to_scenario(&self, scenario: &mut ScenarioV1) -> Result<(), String> {
        #[cfg(not(feature = "solver-lab"))]
        let _ = scenario;
        match self {
            Self::Selected => Ok(()),
            #[cfg(feature = "solver-lab")]
            Self::BranchRobust(options) => {
                scenario.settings.contact_softness = Some(options.softness()?);
                scenario.validate().map_err(|error| error.to_string())
            }
        }
    }
}

#[cfg(feature = "solver-lab")]
impl BranchRobustInteractiveOptions {
    fn validate(self) -> Result<(), String> {
        for (label, value, minimum, maximum) in [
            ("hz", self.hertz, 1.0, 120.0),
            ("damping", self.damping, 0.5, 20.0),
            ("static-scale", self.static_scale, 0.5, 16.0),
        ] {
            if !(minimum..=maximum).contains(&value) {
                return Err(format!(
                    "branch-robust {label} must be between {minimum} and {maximum}"
                ));
            }
        }
        self.softness()?
            .validate()
            .map_err(|error| error.to_string())
    }

    fn softness(self) -> Result<ContactSoftnessPolicyV1, String> {
        Ok(ContactSoftnessPolicyV1 {
            version: CONTACT_SOFTNESS_POLICY_VERSION,
            contact_hertz: finite("hz", self.hertz)?,
            damping_ratio: finite("damping", self.damping)?,
            static_contact_hertz_multiplier: finite("static-scale", self.static_scale)?,
        })
    }
}

#[cfg(feature = "solver-lab")]
fn finite(label: &str, value: f32) -> Result<FiniteF32, String> {
    FiniteF32::new(value).map_err(|error| format!("invalid {label}: {error}"))
}

pub(crate) fn display_number_for_ui(value: f32) -> String {
    let encoded = format!("{value:.3}");
    encoded
        .trim_end_matches('0')
        .trim_end_matches('.')
        .to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selected_profile_is_canonical() {
        let profile = SolverProfileConfig::from_launch(&SolverSelection::default()).unwrap();
        assert_eq!(profile.name(), "selected");
        assert_eq!(profile.options_string(), None);
    }

    #[cfg(feature = "solver-lab")]
    #[test]
    fn branch_robust_options_parse_and_encode_canonically() {
        let profile = SolverProfileConfig::from_launch(&SolverSelection {
            profile: SolverProfileName::BranchRobust,
            options: Some("static-scale=8,hz=30,damping=10".to_owned()),
        })
        .unwrap();
        assert_eq!(profile.name(), "branch-robust");
        assert_eq!(
            profile.options_string().as_deref(),
            Some("hz=30,damping=10,static-scale=8")
        );

        let precise = SolverProfileConfig::from_launch(&SolverSelection {
            profile: SolverProfileName::BranchRobust,
            options: Some("hz=30.125,damping=10,static-scale=2.125".to_owned()),
        })
        .unwrap();
        assert_eq!(
            precise.options_string().as_deref(),
            Some("hz=30.125,damping=10,static-scale=2.125")
        );
        assert!(precise.supports_scenario(ValidationScenario::SolverLabStack16));
        assert!(!precise.supports_scenario(ValidationScenario::BodyFreeFlight));
        assert!(!precise.supports_scenario(ValidationScenario::SolverManifoldI2));
    }
}
