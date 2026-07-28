use std::fmt::{Display, Formatter};

use serde::{Deserialize, Serialize};

pub const PHYSICS_CONFIGURATION_VERSION: u32 = 3;
pub const MIN_FIXED_STEP_HZ: u32 = 15;
pub const MAX_FIXED_STEP_HZ: u32 = 240;
pub const MIN_SOLVER_SUBSTEPS: u32 = 1;
pub const MAX_SOLVER_SUBSTEPS: u32 = 8;
pub const MIN_BIASED_ITERATIONS: u32 = 1;
pub const MAX_BIASED_ITERATIONS: u32 = 4;
pub const MIN_CONTACT_HZ: f32 = 1.0;
pub const MAX_CONTACT_HZ: f32 = 120.0;
pub const MIN_CONTACT_DAMPING_RATIO: f32 = 0.5;
pub const MAX_CONTACT_DAMPING_RATIO: f32 = 20.0;
pub const MIN_STATIC_CONTACT_HZ_MULTIPLIER: f32 = 0.5;
pub const MAX_STATIC_CONTACT_HZ_MULTIPLIER: f32 = 16.0;
pub const MIN_SPECULATIVE_DISTANCE: f32 = 0.0;
pub const MAX_SPECULATIVE_DISTANCE: f32 = 1.0;
pub const MIN_MAX_DEPENETRATION_VELOCITY: f32 = 0.1;
pub const MAX_MAX_DEPENETRATION_VELOCITY: f32 = 100.0;
pub const MIN_SLEEP_SPEED_THRESHOLD: f32 = 0.001;
pub const MAX_SLEEP_SPEED_THRESHOLD: f32 = 1.0;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ConfigError(String);

impl ConfigError {
    pub(crate) fn new(message: impl Into<String>) -> Self {
        Self(message.into())
    }
}

impl Display for ConfigError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

impl std::error::Error for ConfigError {}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SolverProfileName {
    HardDirect,
    #[default]
    SoftCoupled,
}

impl SolverProfileName {
    pub const fn name(self) -> &'static str {
        match self {
            Self::HardDirect => "hard-direct",
            Self::SoftCoupled => "soft-coupled",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum SleepGrouping {
    #[serde(rename = "local-4")]
    Local4,
    #[serde(rename = "local-8")]
    Local8,
    #[default]
    #[serde(rename = "exact")]
    Exact,
}

impl SleepGrouping {
    pub const ALL: [Self; 3] = [Self::Local4, Self::Local8, Self::Exact];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Local4 => "local-4",
            Self::Local8 => "local-8",
            Self::Exact => "exact",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Local4 => "LOCAL 4",
            Self::Local8 => "LOCAL 8",
            Self::Exact => "EXACT",
        }
    }

    pub const fn wake_propagation_rounds(self) -> Option<u32> {
        match self {
            Self::Local4 => Some(4),
            Self::Local8 => Some(8),
            Self::Exact => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PhysicsConfigurationV3 {
    pub version: u32,
    pub solver_profile: SolverProfileName,
    pub fixed_step_hz: u32,
    pub substeps: u32,
    pub biased_iterations: u32,
    pub contact_hertz: f32,
    pub contact_damping_ratio: f32,
    pub static_contact_hertz_multiplier: f32,
    pub speculative_distance: f32,
    pub max_depenetration_velocity: f32,
    pub sleeping_enabled: bool,
    pub sleep_speed_threshold: f32,
    pub sleep_grouping: SleepGrouping,
}

impl PhysicsConfigurationV3 {
    pub const HARD_DIRECT: Self = Self {
        version: PHYSICS_CONFIGURATION_VERSION,
        solver_profile: SolverProfileName::HardDirect,
        fixed_step_hz: 60,
        substeps: 4,
        biased_iterations: 2,
        contact_hertz: 30.0,
        contact_damping_ratio: 10.0,
        static_contact_hertz_multiplier: 2.0,
        speculative_distance: 0.02,
        max_depenetration_velocity: 3.0,
        sleeping_enabled: true,
        sleep_speed_threshold: 0.02,
        sleep_grouping: SleepGrouping::Exact,
    };

    pub const SOFT_COUPLED: Self = Self {
        solver_profile: SolverProfileName::SoftCoupled,
        biased_iterations: 1,
        ..Self::HARD_DIRECT
    };

    pub const fn for_profile(profile: SolverProfileName) -> Self {
        match profile {
            SolverProfileName::HardDirect => Self::HARD_DIRECT,
            SolverProfileName::SoftCoupled => Self::SOFT_COUPLED,
        }
    }

    pub fn fixed_dt_seconds(self) -> f32 {
        1.0 / self.fixed_step_hz as f32
    }

    pub fn validate(self) -> Result<(), ConfigError> {
        if self.version != PHYSICS_CONFIGURATION_VERSION {
            return Err(ConfigError::new(format!(
                "unsupported physics configuration version {}, expected {PHYSICS_CONFIGURATION_VERSION}",
                self.version
            )));
        }
        bounded_integer(
            "simulation hertz",
            self.fixed_step_hz,
            MIN_FIXED_STEP_HZ,
            MAX_FIXED_STEP_HZ,
        )?;
        bounded_integer(
            "solver substeps",
            self.substeps,
            MIN_SOLVER_SUBSTEPS,
            MAX_SOLVER_SUBSTEPS,
        )?;
        bounded_integer(
            "biased iterations",
            self.biased_iterations,
            MIN_BIASED_ITERATIONS,
            MAX_BIASED_ITERATIONS,
        )?;
        bounded_float(
            "contact hertz",
            self.contact_hertz,
            MIN_CONTACT_HZ,
            MAX_CONTACT_HZ,
        )?;
        bounded_float(
            "contact damping",
            self.contact_damping_ratio,
            MIN_CONTACT_DAMPING_RATIO,
            MAX_CONTACT_DAMPING_RATIO,
        )?;
        bounded_float(
            "static contact scale",
            self.static_contact_hertz_multiplier,
            MIN_STATIC_CONTACT_HZ_MULTIPLIER,
            MAX_STATIC_CONTACT_HZ_MULTIPLIER,
        )?;
        bounded_float(
            "speculative distance",
            self.speculative_distance,
            MIN_SPECULATIVE_DISTANCE,
            MAX_SPECULATIVE_DISTANCE,
        )?;
        bounded_float(
            "maximum depenetration velocity",
            self.max_depenetration_velocity,
            MIN_MAX_DEPENETRATION_VELOCITY,
            MAX_MAX_DEPENETRATION_VELOCITY,
        )?;
        bounded_float(
            "sleep speed threshold",
            self.sleep_speed_threshold,
            MIN_SLEEP_SPEED_THRESHOLD,
            MAX_SLEEP_SPEED_THRESHOLD,
        )
    }
}

impl Default for PhysicsConfigurationV3 {
    fn default() -> Self {
        Self::SOFT_COUPLED
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum WorldCapacity {
    #[default]
    Slots64,
    Slots256,
    Slots1000,
    Slots2000,
    Slots5000,
    Slots10000,
    Slots20000,
    Slots50000,
}

impl WorldCapacity {
    pub const ALL: [Self; 8] = [
        Self::Slots64,
        Self::Slots256,
        Self::Slots1000,
        Self::Slots2000,
        Self::Slots5000,
        Self::Slots10000,
        Self::Slots20000,
        Self::Slots50000,
    ];

    pub const fn slots(self) -> u32 {
        match self {
            Self::Slots64 => 64,
            Self::Slots256 => 256,
            Self::Slots1000 => 1_000,
            Self::Slots2000 => 2_000,
            Self::Slots5000 => 5_000,
            Self::Slots10000 => 10_000,
            Self::Slots20000 => 20_000,
            Self::Slots50000 => 50_000,
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Slots64 => "64 SLOTS",
            Self::Slots256 => "256 SLOTS",
            Self::Slots1000 => "1K SLOTS",
            Self::Slots2000 => "2K SLOTS",
            Self::Slots5000 => "5K SLOTS",
            Self::Slots10000 => "10K SLOTS",
            Self::Slots20000 => "20K SLOTS",
            Self::Slots50000 => "50K SLOTS",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FluidSolidCouplingQuality {
    #[default]
    Standard,
    Stabilized,
}

impl FluidSolidCouplingQuality {
    pub const ALL: [Self; 2] = [Self::Standard, Self::Stabilized];

    pub const fn name(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Stabilized => "stabilized",
        }
    }

    pub const fn label(self) -> &'static str {
        match self {
            Self::Standard => "STANDARD",
            Self::Stabilized => "STABILIZED",
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct QualityDef {
    pub physics: PhysicsConfigurationV3,
    pub fluid_solid_coupling: FluidSolidCouplingQuality,
}

impl QualityDef {
    pub fn validate(self) -> Result<(), ConfigError> {
        self.physics.validate()
    }
}

fn bounded_integer(label: &str, value: u32, minimum: u32, maximum: u32) -> Result<(), ConfigError> {
    if !(minimum..=maximum).contains(&value) {
        return Err(ConfigError::new(format!(
            "{label} must be between {minimum} and {maximum}"
        )));
    }
    Ok(())
}

fn bounded_float(label: &str, value: f32, minimum: f32, maximum: f32) -> Result<(), ConfigError> {
    if !value.is_finite() || !(minimum..=maximum).contains(&value) {
        return Err(ConfigError::new(format!(
            "{label} must be finite and between {minimum} and {maximum}"
        )));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn supported_defaults_are_valid_and_stable() {
        QualityDef::default().validate().unwrap();
        assert_eq!(
            PhysicsConfigurationV3::default(),
            PhysicsConfigurationV3::SOFT_COUPLED
        );
        assert_eq!(WorldCapacity::default().slots(), 64);
        assert_eq!(
            WorldCapacity::ALL.map(WorldCapacity::slots),
            [64, 256, 1_000, 2_000, 5_000, 10_000, 20_000, 50_000]
        );
        assert_eq!(SleepGrouping::default(), SleepGrouping::Exact);
    }

    #[test]
    fn invalid_supported_configuration_fails_before_gpu_allocation() {
        let mut quality = QualityDef::default();
        quality.physics.fixed_step_hz = 0;
        assert!(
            quality
                .validate()
                .unwrap_err()
                .to_string()
                .contains("hertz")
        );
    }
}
