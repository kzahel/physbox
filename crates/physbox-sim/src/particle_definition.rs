use serde::Serialize;

use crate::{
    DefinitionError,
    particle_world::{
        MAX_PRODUCT_PARTICLE_CAPACITY, PARTICLE_COMMAND_CAPACITY, circle_particle_count,
    },
};

pub const PREVIEW_WATER_RADIUS: f32 = 0.05;
pub const MAX_PARTICLE_CAPACITY: u32 = MAX_PRODUCT_PARTICLE_CAPACITY;
pub const PARTICLE_EMISSION_COMMAND_CAPACITY: u32 = PARTICLE_COMMAND_CAPACITY;
pub use crate::particle_world::MAX_PARTICLES_PER_EMISSION;
const PREVIEW_WATER_LOWER_BOUND: [f32; 2] = [-16.0, -10.0];
const PREVIEW_WATER_UPPER_BOUND: [f32; 2] = [16.0, 24.0];

/// One bounded circle of semantic preview water to emit on the next step.
///
/// The default spacing is the current preview-water particle diameter.
/// Emission changes particle lifecycle only; it does not expose or change
/// solver/material parameters.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleEmissionDef {
    pub center: [f32; 2],
    pub radius: f32,
    pub spacing: f32,
    pub initial_velocity: [f32; 2],
}

impl ParticleEmissionDef {
    pub const fn water_circle(center: [f32; 2], radius: f32) -> Self {
        Self {
            center,
            radius,
            spacing: PREVIEW_WATER_RADIUS * 2.0,
            initial_velocity: [0.0, 0.0],
        }
    }

    pub const fn with_spacing(mut self, spacing: f32) -> Self {
        self.spacing = spacing;
        self
    }

    pub const fn with_initial_velocity(mut self, initial_velocity: [f32; 2]) -> Self {
        self.initial_velocity = initial_velocity;
        self
    }

    pub fn requested_particle_count(self) -> Result<u32, DefinitionError> {
        self.validate()?;
        circle_particle_count(self.radius, self.spacing).map_err(DefinitionError::new)
    }

    pub fn validate(self) -> Result<(), DefinitionError> {
        if !self
            .center
            .into_iter()
            .chain(self.initial_velocity)
            .chain([self.radius, self.spacing])
            .all(f32::is_finite)
        {
            return Err(DefinitionError::new(
                "particle emission center, radius, spacing, and velocity must be finite",
            ));
        }
        if self.radius <= 0.0 {
            return Err(DefinitionError::new(
                "preview-water emission radius must be positive",
            ));
        }
        if self.spacing < PREVIEW_WATER_RADIUS * 2.0 {
            return Err(DefinitionError::new(format!(
                "preview-water emission spacing must be at least {}",
                PREVIEW_WATER_RADIUS * 2.0
            )));
        }
        let lower = [self.center[0] - self.radius, self.center[1] - self.radius];
        let upper = [self.center[0] + self.radius, self.center[1] + self.radius];
        if !lower.into_iter().chain(upper).all(f32::is_finite)
            || lower[0] < PREVIEW_WATER_LOWER_BOUND[0]
            || lower[1] < PREVIEW_WATER_LOWER_BOUND[1]
            || upper[0] > PREVIEW_WATER_UPPER_BOUND[0]
            || upper[1] > PREVIEW_WATER_UPPER_BOUND[1]
        {
            return Err(DefinitionError::new(
                "particle emission is outside the supported preview-water world bounds",
            ));
        }
        circle_particle_count(self.radius, self.spacing).map_err(DefinitionError::new)?;
        Ok(())
    }
}

/// One rectangular block of initially live preview-water particles.
#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleRegionDef {
    pub lower: [f32; 2],
    pub upper: [f32; 2],
    pub spacing: f32,
    pub initial_velocity: [f32; 2],
}

impl ParticleRegionDef {
    pub const fn rectangle(lower: [f32; 2], upper: [f32; 2], spacing: f32) -> Self {
        Self {
            lower,
            upper,
            spacing,
            initial_velocity: [0.0, 0.0],
        }
    }

    pub const fn with_initial_velocity(mut self, initial_velocity: [f32; 2]) -> Self {
        self.initial_velocity = initial_velocity;
        self
    }

    pub fn validate(self) -> Result<(), DefinitionError> {
        if !self
            .lower
            .into_iter()
            .chain(self.upper)
            .chain(self.initial_velocity)
            .chain([self.spacing])
            .all(f32::is_finite)
        {
            return Err(DefinitionError::new(
                "particle region bounds, spacing, and velocity must be finite",
            ));
        }
        if self.lower[0] >= self.upper[0] || self.lower[1] >= self.upper[1] {
            return Err(DefinitionError::new(
                "particle region requires ordered lower and upper bounds",
            ));
        }
        if self.lower[0] < PREVIEW_WATER_LOWER_BOUND[0]
            || self.lower[1] < PREVIEW_WATER_LOWER_BOUND[1]
            || self.upper[0] > PREVIEW_WATER_UPPER_BOUND[0]
            || self.upper[1] > PREVIEW_WATER_UPPER_BOUND[1]
        {
            return Err(DefinitionError::new(
                "particle region is outside the supported preview-water world bounds",
            ));
        }
        if self.spacing < PREVIEW_WATER_RADIUS * 2.0 {
            return Err(DefinitionError::new(format!(
                "preview-water spacing must be at least {}",
                PREVIEW_WATER_RADIUS * 2.0
            )));
        }
        Ok(())
    }

    fn particle_count(self, remaining_capacity: u32) -> Result<u32, DefinitionError> {
        self.validate()?;
        let radius = PREVIEW_WATER_RADIUS;
        let mut count = 0u32;
        let mut y = self.lower[1] + radius;
        while y <= self.upper[1] - radius + 1.0e-6 {
            let mut x = self.lower[0] + radius;
            while x <= self.upper[0] - radius + 1.0e-6 {
                count = count
                    .checked_add(1)
                    .ok_or_else(|| DefinitionError::new("authored particle count exceeds u32"))?;
                if count > remaining_capacity {
                    return Err(DefinitionError::new(
                        "authored particle regions overflow particle capacity",
                    ));
                }
                let next_x = x + self.spacing;
                if next_x <= x {
                    return Err(DefinitionError::new(
                        "particle spacing cannot advance across the authored x bounds",
                    ));
                }
                x = next_x;
            }
            let next_y = y + self.spacing;
            if next_y <= y {
                return Err(DefinitionError::new(
                    "particle spacing cannot advance across the authored y bounds",
                ));
            }
            y = next_y;
        }
        Ok(count)
    }
}

/// Preset for the particle integration and water material of one world.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum WaterProfile {
    #[default]
    Standard,
    /// One fluid substep, a fine neighbor grid and reduced water gravity for dense displays.
    HighCount,
}

/// Pre-step authoring for the current single-material preview-water system.
///
/// This definition owns the water profile and optional initial placement.
/// An empty region list creates an initially empty system for runtime emission. It may be installed on a `World` exactly once before the first
/// step.
#[derive(Clone, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticleSystemDef {
    capacity: u32,
    regions: Vec<ParticleRegionDef>,
    profile: WaterProfile,
}

impl ParticleSystemDef {
    pub const fn water(capacity: u32) -> Self {
        Self {
            capacity,
            regions: Vec::new(),
            profile: WaterProfile::Standard,
        }
    }

    pub const fn with_profile(mut self, profile: WaterProfile) -> Self {
        self.profile = profile;
        self
    }

    pub const fn profile(&self) -> WaterProfile {
        self.profile
    }

    pub fn with_region(mut self, region: ParticleRegionDef) -> Self {
        self.regions.push(region);
        self
    }

    pub const fn capacity(&self) -> u32 {
        self.capacity
    }

    pub fn regions(&self) -> &[ParticleRegionDef] {
        &self.regions
    }

    pub const fn particle_radius(&self) -> f32 {
        PREVIEW_WATER_RADIUS
    }

    pub fn initial_particle_count(&self) -> Result<u32, DefinitionError> {
        self.validated_count()
    }

    pub fn validate(&self) -> Result<(), DefinitionError> {
        self.validated_count().map(|_| ())
    }

    fn validated_count(&self) -> Result<u32, DefinitionError> {
        if self.capacity == 0 || self.capacity > MAX_PARTICLE_CAPACITY {
            return Err(DefinitionError::new(format!(
                "particle capacity must be within 1..={MAX_PARTICLE_CAPACITY}"
            )));
        }
        let mut total = 0u32;
        for region in &self.regions {
            let remaining = self.capacity - total;
            total += region.particle_count(remaining)?;
        }
        if !self.regions.is_empty() && total == 0 {
            return Err(DefinitionError::new(
                "authored particle regions produced no particles",
            ));
        }
        Ok(total)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preview_water_counts_the_exact_authored_lattice() {
        let definition = ParticleSystemDef::water(32).with_region(
            ParticleRegionDef::rectangle([0.0, 0.0], [0.4, 0.3], 0.1)
                .with_initial_velocity([0.2, -0.1]),
        );
        assert_eq!(definition.initial_particle_count().unwrap(), 12);
        assert_eq!(definition.particle_radius(), 0.05);
    }

    #[test]
    fn preview_water_accepts_empty_and_rejects_invalid_or_over_capacity_authoring() {
        let empty = ParticleSystemDef::water(32);
        empty.validate().unwrap();
        assert_eq!(empty.initial_particle_count().unwrap(), 0);
        assert!(
            ParticleSystemDef::water(0)
                .with_region(ParticleRegionDef::rectangle([0.0, 0.0], [0.2, 0.2], 0.1))
                .validate()
                .is_err()
        );
        assert!(
            ParticleSystemDef::water(1)
                .with_region(ParticleRegionDef::rectangle([0.0, 0.0], [0.3, 0.3], 0.1))
                .validate()
                .is_err()
        );
        assert!(
            ParticleSystemDef::water(32)
                .with_region(ParticleRegionDef::rectangle([0.0, 0.0], [0.3, 0.3], 0.05))
                .validate()
                .is_err()
        );
    }

    #[test]
    fn preview_water_emission_is_bounded_and_uses_semantic_defaults() {
        let emission =
            ParticleEmissionDef::water_circle([0.0, 1.0], 0.3).with_initial_velocity([0.2, -0.1]);
        assert_eq!(emission.spacing, PREVIEW_WATER_RADIUS * 2.0);
        assert_eq!(emission.requested_particle_count().unwrap(), 29);
        assert!(
            ParticleEmissionDef::water_circle([16.0, 0.0], 0.3)
                .validate()
                .is_err()
        );
        assert!(
            ParticleEmissionDef::water_circle([0.0, 0.0], 100.0)
                .validate()
                .is_err()
        );
        assert!(
            emission
                .with_spacing(PREVIEW_WATER_RADIUS)
                .validate()
                .is_err()
        );
    }
}
