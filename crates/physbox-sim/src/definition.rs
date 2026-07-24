use std::fmt::{Display, Formatter};

use serde::Serialize;

use crate::{ConfigError, QualityDef, WorldCapacity};

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyId {
    slot: u32,
    generation: u32,
}

impl BodyId {
    pub const fn slot(self) -> u32 {
        self.slot
    }

    pub const fn generation(self) -> u32 {
        self.generation
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeId {
    body: BodyId,
}

impl ShapeId {
    pub const fn body(self) -> BodyId {
        self.body
    }
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum BodyType {
    Static,
    #[default]
    Dynamic,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BodyDef {
    pub body_type: BodyType,
    pub position: [f32; 2],
    pub angle: f32,
    pub linear_velocity: [f32; 2],
    pub angular_velocity: f32,
}

impl BodyDef {
    pub const fn dynamic() -> Self {
        Self {
            body_type: BodyType::Dynamic,
            position: [0.0; 2],
            angle: 0.0,
            linear_velocity: [0.0; 2],
            angular_velocity: 0.0,
        }
    }

    pub const fn fixed() -> Self {
        Self {
            body_type: BodyType::Static,
            ..Self::dynamic()
        }
    }

    pub const fn with_position(mut self, position: [f32; 2]) -> Self {
        self.position = position;
        self
    }

    pub const fn with_angle(mut self, angle: f32) -> Self {
        self.angle = angle;
        self
    }

    pub const fn with_linear_velocity(mut self, velocity: [f32; 2]) -> Self {
        self.linear_velocity = velocity;
        self
    }

    pub const fn with_angular_velocity(mut self, velocity: f32) -> Self {
        self.angular_velocity = velocity;
        self
    }

    pub fn validate(self) -> Result<(), DefinitionError> {
        finite_pair("body position", self.position)?;
        finite("body angle", self.angle)?;
        finite_pair("body linear velocity", self.linear_velocity)?;
        finite("body angular velocity", self.angular_velocity)
    }
}

impl Default for BodyDef {
    fn default() -> Self {
        Self::dynamic()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MaterialDef {
    pub density: f32,
    pub friction: f32,
    pub restitution: f32,
}

impl MaterialDef {
    pub const fn with_density(mut self, density: f32) -> Self {
        self.density = density;
        self
    }

    pub const fn with_friction(mut self, friction: f32) -> Self {
        self.friction = friction;
        self
    }

    pub const fn with_restitution(mut self, restitution: f32) -> Self {
        self.restitution = restitution;
        self
    }

    fn validate(self, body_type: BodyType) -> Result<(), DefinitionError> {
        finite("shape density", self.density)?;
        finite("shape friction", self.friction)?;
        finite("shape restitution", self.restitution)?;
        if self.density < 0.0
            || (body_type == BodyType::Dynamic && self.density <= 0.0)
            || self.friction < 0.0
            || !(0.0..=1.0).contains(&self.restitution)
        {
            return Err(DefinitionError(
                "dynamic density must be positive, friction nonnegative, and restitution within 0..=1"
                    .to_owned(),
            ));
        }
        Ok(())
    }
}

impl Default for MaterialDef {
    fn default() -> Self {
        Self {
            density: 1.0,
            friction: 0.5,
            restitution: 0.0,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Circle {
    pub radius: f32,
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Rectangle {
    pub half_extents: [f32; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum ShapeGeometry {
    Circle(Circle),
    Rectangle(Rectangle),
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShapeDef {
    pub geometry: ShapeGeometry,
    pub material: MaterialDef,
}

impl ShapeDef {
    pub const fn circle(radius: f32) -> Self {
        Self {
            geometry: ShapeGeometry::Circle(Circle { radius }),
            material: MaterialDef {
                density: 1.0,
                friction: 0.5,
                restitution: 0.0,
            },
        }
    }

    pub const fn rectangle(half_extents: [f32; 2]) -> Self {
        Self {
            geometry: ShapeGeometry::Rectangle(Rectangle { half_extents }),
            material: MaterialDef {
                density: 1.0,
                friction: 0.5,
                restitution: 0.0,
            },
        }
    }

    pub const fn with_density(mut self, density: f32) -> Self {
        self.material.density = density;
        self
    }

    pub const fn with_friction(mut self, friction: f32) -> Self {
        self.material.friction = friction;
        self
    }

    pub const fn with_restitution(mut self, restitution: f32) -> Self {
        self.material.restitution = restitution;
        self
    }

    pub fn validate(self, body_type: BodyType) -> Result<(), DefinitionError> {
        self.material.validate(body_type)?;
        match self.geometry {
            ShapeGeometry::Circle(Circle { radius }) if radius.is_finite() && radius > 0.0 => {
                Ok(())
            }
            ShapeGeometry::Rectangle(Rectangle { half_extents })
                if half_extents[0].is_finite()
                    && half_extents[1].is_finite()
                    && half_extents[0] > 0.0
                    && half_extents[1] > 0.0 =>
            {
                Ok(())
            }
            ShapeGeometry::Circle(_) => Err(DefinitionError(
                "circle radius must be positive and finite".to_owned(),
            )),
            ShapeGeometry::Rectangle(_) => Err(DefinitionError(
                "rectangle half extents must be positive and finite".to_owned(),
            )),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StepDef {
    pub delta_seconds: f32,
}

impl StepDef {
    pub const fn fixed(delta_seconds: f32) -> Self {
        Self { delta_seconds }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorldDef {
    pub gravity: [f32; 2],
    pub capacity: WorldCapacity,
    pub quality: QualityDef,
}

impl WorldDef {
    pub fn validate(self) -> Result<(), DefinitionError> {
        finite_pair("world gravity", self.gravity)?;
        self.quality
            .validate()
            .map_err(DefinitionError::from_config)
    }
}

impl Default for WorldDef {
    fn default() -> Self {
        Self {
            gravity: [0.0, -10.0],
            capacity: WorldCapacity::default(),
            quality: QualityDef::default(),
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DefinitionError(String);

impl DefinitionError {
    fn from_config(error: ConfigError) -> Self {
        Self(error.to_string())
    }
}

impl Display for DefinitionError {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.0)
    }
}

fn finite(label: &str, value: f32) -> Result<(), DefinitionError> {
    if value.is_finite() {
        Ok(())
    } else {
        Err(DefinitionError(format!("{label} must be finite")))
    }
}

fn finite_pair(label: &str, value: [f32; 2]) -> Result<(), DefinitionError> {
    finite(label, value[0])?;
    finite(label, value[1])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn public_definitions_validate_before_gpu_allocation() {
        WorldDef::default().validate().unwrap();
        BodyDef::dynamic().validate().unwrap();
        ShapeDef::circle(0.25).validate(BodyType::Dynamic).unwrap();
        ShapeDef::rectangle([0.3, 0.2])
            .validate(BodyType::Static)
            .unwrap();
    }

    #[test]
    fn invalid_geometry_and_dynamic_density_are_rejected() {
        assert!(ShapeDef::circle(0.0).validate(BodyType::Dynamic).is_err());
        assert!(
            ShapeDef::rectangle([1.0, f32::NAN])
                .validate(BodyType::Static)
                .is_err()
        );
        assert!(
            ShapeDef::circle(1.0)
                .with_density(0.0)
                .validate(BodyType::Dynamic)
                .is_err()
        );
    }

    #[test]
    fn handles_are_generational_and_shapes_retain_body_identity() {
        let body = BodyId {
            slot: 3,
            generation: 7,
        };
        let shape = ShapeId { body };
        assert_eq!(body.slot(), 3);
        assert_eq!(body.generation(), 7);
        assert_eq!(shape.body(), body);
    }
}
