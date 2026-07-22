use physbox_contracts::{
    CONTACT_SOFTNESS_POLICY_VERSION, ContactPolicyV1, ContactSoftnessPolicyV1, FiniteF32,
    ScenarioV1, Validate,
};
use physbox_launch::{
    MAX_BIASED_ITERATIONS, MAX_CONTACT_DAMPING_RATIO, MAX_CONTACT_HZ, MAX_FIXED_STEP_HZ,
    MAX_SOLVER_SUBSTEPS, MAX_STATIC_CONTACT_HZ_MULTIPLIER, MIN_BIASED_ITERATIONS,
    MIN_CONTACT_DAMPING_RATIO, MIN_CONTACT_HZ, MIN_FIXED_STEP_HZ, MIN_SOLVER_SUBSTEPS,
    MIN_STATIC_CONTACT_HZ_MULTIPLIER, PhysicsConfigurationV2, SolverProfileName,
    ValidationScenario,
};

#[derive(Clone, Debug, PartialEq)]
pub struct SolverProfileConfig {
    physics: PhysicsConfigurationV2,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum SolverParameter {
    SimHertz,
    Substeps,
    Iterations,
    ContactHertz,
    Damping,
    StaticScale,
}

impl SolverParameter {
    pub(crate) const ALL: [Self; 6] = [
        Self::SimHertz,
        Self::Substeps,
        Self::Iterations,
        Self::ContactHertz,
        Self::Damping,
        Self::StaticScale,
    ];

    pub(crate) const fn label(self) -> &'static str {
        match self {
            Self::SimHertz => "SIM HZ",
            Self::Substeps => "SUBSTEPS",
            Self::Iterations => "ITERATIONS",
            Self::ContactHertz => "CONTACT HZ",
            Self::Damping => "DAMPING",
            Self::StaticScale => "STATIC SCALE",
        }
    }
}

impl Default for SolverProfileConfig {
    fn default() -> Self {
        Self::from_physics(PhysicsConfigurationV2::default())
            .expect("launch-owned default physics configuration is valid")
    }
}

impl SolverProfileConfig {
    pub fn from_physics(physics: PhysicsConfigurationV2) -> Result<Self, String> {
        physics.validate().map_err(|error| error.to_string())?;
        let profile = Self { physics };
        profile.validate()?;
        Ok(profile)
    }

    pub fn hard_direct() -> Self {
        Self::from_physics(PhysicsConfigurationV2::HARD_DIRECT)
            .expect("hard-direct preset is valid")
    }

    pub fn soft_coupled() -> Self {
        Self::from_physics(PhysicsConfigurationV2::SOFT_COUPLED)
            .expect("soft-coupled preset is valid")
    }

    pub const fn physics(&self) -> PhysicsConfigurationV2 {
        self.physics
    }

    pub(crate) fn set_sleeping_enabled(&mut self, enabled: bool) {
        self.physics.sleeping_enabled = enabled;
    }

    pub const fn name(&self) -> &'static str {
        self.physics.solver_profile.name()
    }

    pub const fn label(&self) -> &'static str {
        match self.physics.solver_profile {
            SolverProfileName::HardDirect => "HARD DIRECT",
            SolverProfileName::SoftCoupled => "SOFT COUPLED",
        }
    }

    pub const fn is_hard_direct(&self) -> bool {
        matches!(self.physics.solver_profile, SolverProfileName::HardDirect)
    }

    pub const fn is_selected(&self) -> bool {
        self.is_hard_direct()
    }

    pub const fn is_soft_coupled(&self) -> bool {
        matches!(self.physics.solver_profile, SolverProfileName::SoftCoupled)
    }

    pub fn is_preset(&self) -> bool {
        let mut expected = PhysicsConfigurationV2::for_profile(self.physics.solver_profile);
        expected.sleeping_enabled = self.physics.sleeping_enabled;
        self.physics == expected
    }

    pub(crate) fn restore_preset(&mut self) {
        let sleeping_enabled = self.physics.sleeping_enabled;
        self.physics = PhysicsConfigurationV2::for_profile(self.physics.solver_profile);
        self.physics.sleeping_enabled = sleeping_enabled;
    }

    pub(crate) fn effective_dynamic_contact_hz(&self) -> f32 {
        let substep_dt = self.physics.fixed_dt_seconds() / self.physics.substeps as f32;
        self.physics.contact_hertz.min(0.125 / substep_dt)
    }

    pub fn options_string(&self) -> Option<String> {
        self.is_soft_coupled().then(|| {
            format!(
                "hz={},damping={},static-scale={}",
                self.physics.contact_hertz,
                self.physics.contact_damping_ratio,
                self.physics.static_contact_hertz_multiplier,
            )
        })
    }

    pub(crate) fn configuration_key(&self) -> String {
        format!(
            "{}:{}:{}:{}:{}:{}:{}:{}:{}:{}",
            self.name(),
            self.physics.fixed_step_hz,
            self.physics.substeps,
            self.physics.biased_iterations,
            self.physics.contact_hertz,
            self.physics.contact_damping_ratio,
            self.physics.static_contact_hertz_multiplier,
            self.physics.speculative_distance,
            self.physics.max_depenetration_velocity,
            self.physics.sleeping_enabled,
        )
    }

    pub fn available() -> Vec<Self> {
        vec![Self::soft_coupled(), Self::hard_direct()]
    }

    pub fn supports_scenario(&self, scenario: ValidationScenario) -> bool {
        match self.physics.solver_profile {
            SolverProfileName::HardDirect => true,
            SolverProfileName::SoftCoupled => {
                let product = matches!(
                    scenario,
                    ValidationScenario::Bootstrap
                        | ValidationScenario::BodyFreeFlight
                        | ValidationScenario::RigidFloor
                        | ValidationScenario::RigidStack
                        | ValidationScenario::RectangleStack
                        | ValidationScenario::CircleBucket100
                        | ValidationScenario::RectangleBucket100
                        | ValidationScenario::ResetGrab
                );
                #[cfg(feature = "solver-lab")]
                let product = product
                    || matches!(
                        scenario,
                        ValidationScenario::SolverLabStack2
                            | ValidationScenario::SolverLabStack4
                            | ValidationScenario::SolverLabStack8
                            | ValidationScenario::SolverLabStack12
                            | ValidationScenario::SolverLabStack14
                            | ValidationScenario::SolverLabStack16
                    );
                product
            }
        }
    }

    pub(crate) fn select(name: SolverProfileName) -> Self {
        match name {
            SolverProfileName::HardDirect => Self::hard_direct(),
            SolverProfileName::SoftCoupled => Self::soft_coupled(),
        }
    }

    pub(crate) const fn profile_name(&self) -> SolverProfileName {
        self.physics.solver_profile
    }

    pub(crate) fn parameter_value(&self, parameter: SolverParameter) -> Option<f32> {
        self.is_soft_coupled().then_some(match parameter {
            SolverParameter::SimHertz => self.physics.fixed_step_hz as f32,
            SolverParameter::Substeps => self.physics.substeps as f32,
            SolverParameter::Iterations => self.physics.biased_iterations as f32,
            SolverParameter::ContactHertz => self.physics.contact_hertz,
            SolverParameter::Damping => self.physics.contact_damping_ratio,
            SolverParameter::StaticScale => self.physics.static_contact_hertz_multiplier,
        })
    }

    pub(crate) fn adjust(&mut self, parameter: SolverParameter, direction: i32) {
        if !self.is_soft_coupled() {
            return;
        }
        let direction = direction.signum();
        match parameter {
            SolverParameter::SimHertz => {
                self.physics.fixed_step_hz = adjust_u32(
                    self.physics.fixed_step_hz,
                    direction,
                    15,
                    MIN_FIXED_STEP_HZ,
                    MAX_FIXED_STEP_HZ,
                );
            }
            SolverParameter::Substeps => {
                self.physics.substeps = adjust_u32(
                    self.physics.substeps,
                    direction,
                    1,
                    MIN_SOLVER_SUBSTEPS,
                    MAX_SOLVER_SUBSTEPS,
                );
            }
            SolverParameter::Iterations => {
                self.physics.biased_iterations = adjust_u32(
                    self.physics.biased_iterations,
                    direction,
                    1,
                    MIN_BIASED_ITERATIONS,
                    MAX_BIASED_ITERATIONS,
                );
            }
            SolverParameter::ContactHertz => adjust_f32(
                &mut self.physics.contact_hertz,
                direction,
                1.0,
                MIN_CONTACT_HZ,
                MAX_CONTACT_HZ,
            ),
            SolverParameter::Damping => adjust_f32(
                &mut self.physics.contact_damping_ratio,
                direction,
                0.5,
                MIN_CONTACT_DAMPING_RATIO,
                MAX_CONTACT_DAMPING_RATIO,
            ),
            SolverParameter::StaticScale => adjust_f32(
                &mut self.physics.static_contact_hertz_multiplier,
                direction,
                0.5,
                MIN_STATIC_CONTACT_HZ_MULTIPLIER,
                MAX_STATIC_CONTACT_HZ_MULTIPLIER,
            ),
        }
    }

    /// Projects the launch-owned configuration into an engine-owned neutral
    /// scenario. Evidence tooling uses the same one-time projection as the
    /// shared host instead of rebuilding solver coefficients independently.
    pub fn apply_to_scenario(&self, scenario: &mut ScenarioV1) -> Result<(), String> {
        scenario.settings.fixed_dt = finite("fixed dt", self.physics.fixed_dt_seconds())?;
        scenario.settings.substeps = self.physics.substeps;
        scenario.settings.iterations = self.physics.biased_iterations;
        scenario.settings.contact_policy = Some(ContactPolicyV1 {
            speculative_distance: finite(
                "speculative distance",
                self.physics.speculative_distance,
            )?,
            max_depenetration_velocity: finite(
                "maximum depenetration velocity",
                self.physics.max_depenetration_velocity,
            )?,
        });
        scenario.settings.contact_softness = if self.is_soft_coupled() {
            Some(self.softness()?)
        } else {
            None
        };
        scenario.validate().map_err(|error| error.to_string())
    }

    fn validate(&self) -> Result<(), String> {
        if self.is_hard_direct()
            && (self.physics.fixed_step_hz != 60
                || self.physics.substeps != 4
                || self.physics.biased_iterations != 2)
        {
            return Err(
                "hard-direct comparison is fixed at 60 Hz, four substeps, and two iterations"
                    .to_owned(),
            );
        }
        if !(MIN_SOLVER_SUBSTEPS..=MAX_SOLVER_SUBSTEPS).contains(&self.physics.substeps)
            || !(MIN_BIASED_ITERATIONS..=MAX_BIASED_ITERATIONS)
                .contains(&self.physics.biased_iterations)
        {
            return Err("solver resolution lies outside product bounds".to_owned());
        }
        self.softness()?
            .validate()
            .map_err(|error| error.to_string())
    }

    fn softness(&self) -> Result<ContactSoftnessPolicyV1, String> {
        Ok(ContactSoftnessPolicyV1 {
            version: CONTACT_SOFTNESS_POLICY_VERSION,
            contact_hertz: finite("contact hertz", self.physics.contact_hertz)?,
            damping_ratio: finite("contact damping", self.physics.contact_damping_ratio)?,
            static_contact_hertz_multiplier: finite(
                "static contact hertz multiplier",
                self.physics.static_contact_hertz_multiplier,
            )?,
        })
    }
}

fn finite(label: &str, value: f32) -> Result<FiniteF32, String> {
    FiniteF32::new(value).map_err(|error| format!("invalid {label}: {error}"))
}

fn adjust_u32(value: u32, direction: i32, step: u32, minimum: u32, maximum: u32) -> u32 {
    if direction < 0 {
        value.saturating_sub(step).max(minimum)
    } else {
        value.saturating_add(step).min(maximum)
    }
}

fn adjust_f32(value: &mut f32, direction: i32, step: f32, minimum: f32, maximum: f32) {
    *value = (*value + direction as f32 * step).clamp(minimum, maximum);
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
    fn soft_coupled_profile_is_the_canonical_fresh_default() {
        let profile = SolverProfileConfig::default();
        assert_eq!(profile.name(), "soft-coupled");
        assert_eq!(profile.label(), "SOFT COUPLED");
        assert_eq!(
            profile.options_string().as_deref(),
            Some("hz=30,damping=10,static-scale=2")
        );
    }

    #[test]
    fn soft_configuration_is_concrete_and_encodes_legacy_contact_options() {
        let mut physics = PhysicsConfigurationV2::SOFT_COUPLED;
        physics.static_contact_hertz_multiplier = 8.0;
        let profile = SolverProfileConfig::from_physics(physics).unwrap();
        assert_eq!(profile.name(), "soft-coupled");
        assert_eq!(profile.label(), "SOFT COUPLED");
        assert_eq!(
            profile.options_string().as_deref(),
            Some("hz=30,damping=10,static-scale=8")
        );
        assert!(profile.supports_scenario(ValidationScenario::RectangleStack));
        #[cfg(feature = "solver-lab")]
        assert!(profile.supports_scenario(ValidationScenario::SolverLabStack16));
        assert!(profile.supports_scenario(ValidationScenario::BodyFreeFlight));
        assert!(!profile.supports_scenario(ValidationScenario::SolverManifoldI2));
    }

    #[test]
    fn hard_comparison_rejects_resolution_changes_before_gpu_allocation() {
        let mut physics = PhysicsConfigurationV2::HARD_DIRECT;
        physics.substeps = 8;
        let error = SolverProfileConfig::from_physics(physics).unwrap_err();
        assert!(error.contains("hard-direct comparison is fixed"));
    }

    #[test]
    fn complete_soft_configuration_projects_into_the_neutral_scenario_once() {
        let mut physics = PhysicsConfigurationV2::SOFT_COUPLED;
        physics.fixed_step_hz = 120;
        physics.substeps = 8;
        physics.biased_iterations = 2;
        physics.contact_hertz = 45.0;
        physics.contact_damping_ratio = 8.0;
        physics.static_contact_hertz_multiplier = 3.0;
        physics.speculative_distance = 0.03;
        physics.max_depenetration_velocity = 4.0;
        let profile = SolverProfileConfig::from_physics(physics).unwrap();
        let mut scenario = crate::p5_2_generated_floor_scenario(1).unwrap();
        profile.apply_to_scenario(&mut scenario).unwrap();

        assert_eq!(scenario.settings.fixed_dt.get(), 1.0 / 120.0);
        assert_eq!(scenario.settings.substeps, 8);
        assert_eq!(scenario.settings.iterations, 2);
        let softness = scenario.settings.contact_softness.unwrap();
        assert_eq!(softness.contact_hertz.get(), 45.0);
        assert_eq!(softness.damping_ratio.get(), 8.0);
        assert_eq!(softness.static_contact_hertz_multiplier.get(), 3.0);
        let recovery = scenario.settings.contact_policy.unwrap();
        assert_eq!(recovery.speculative_distance.get(), 0.03);
        assert_eq!(recovery.max_depenetration_velocity.get(), 4.0);
    }
}
