use std::fmt;

pub const CAMERA_STATE_VERSION: u32 = 1;
pub const MIN_ORTHOGRAPHIC_HALF_HEIGHT: f32 = 0.08;
pub const MAX_ORTHOGRAPHIC_HALF_HEIGHT: f32 = 8.0;

/// Shared world-space camera state.
///
/// Selectively ported from PhysBox 4 revision
/// `4295957e15480ef701e6006c7cafb5f0728ef101`,
/// `crates/physbox-engine/src/camera.rs`. PhysBox 5 keeps only the focused 2D
/// transform contract and owns its input mapping separately.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Camera2d {
    center: [f32; 2],
    orthographic_half_height: f32,
    home_center: [f32; 2],
    home_orthographic_half_height: f32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum CameraError {
    NonFiniteCenter,
    InvalidOrthographicHalfHeight,
    InvalidTargetSize,
}

impl fmt::Display for CameraError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::NonFiniteCenter => "camera center must contain finite coordinates",
            Self::InvalidOrthographicHalfHeight => {
                "camera orthographic half-height must be finite and positive"
            }
            Self::InvalidTargetSize => "camera target width and height must be nonzero",
        })
    }
}

impl std::error::Error for CameraError {}

impl Camera2d {
    pub const fn fixed() -> Self {
        Self {
            center: [0.0, 0.0],
            orthographic_half_height: 1.0,
            home_center: [0.0, 0.0],
            home_orthographic_half_height: 1.0,
        }
    }

    pub fn try_new(center: [f32; 2], orthographic_half_height: f32) -> Result<Self, CameraError> {
        let camera = Self {
            center,
            orthographic_half_height,
            home_center: [0.0, 0.0],
            home_orthographic_half_height: 1.0,
        };
        camera.validate()?;
        Ok(camera)
    }

    pub fn try_new_at_home(
        center: [f32; 2],
        orthographic_half_height: f32,
    ) -> Result<Self, CameraError> {
        let camera = Self {
            center,
            orthographic_half_height,
            home_center: center,
            home_orthographic_half_height: orthographic_half_height,
        };
        camera.validate()?;
        Ok(camera)
    }

    pub const fn center(self) -> [f32; 2] {
        self.center
    }

    pub const fn orthographic_half_height(self) -> f32 {
        self.orthographic_half_height
    }

    pub fn recenter(&mut self) -> bool {
        if self.center == self.home_center
            && self.orthographic_half_height == self.home_orthographic_half_height
        {
            return false;
        }
        self.center = self.home_center;
        self.orthographic_half_height = self.home_orthographic_half_height;
        true
    }

    pub fn pan_world(&mut self, delta: [f32; 2]) {
        if delta.into_iter().all(f32::is_finite) {
            self.center[0] += delta[0];
            self.center[1] += delta[1];
        }
    }

    pub fn pan_screen(
        &mut self,
        delta: [f32; 2],
        target_size: [u32; 2],
    ) -> Result<(), CameraError> {
        let world_delta = self.resolve(target_size)?.screen_delta_to_world(delta);
        self.pan_world([-world_delta[0], -world_delta[1]]);
        Ok(())
    }

    pub fn zoom_at(
        &mut self,
        anchor: [f32; 2],
        factor: f32,
        target_size: [u32; 2],
    ) -> Result<(), CameraError> {
        if !anchor.into_iter().all(f32::is_finite) || !factor.is_finite() || factor <= 0.0 {
            return Err(CameraError::InvalidOrthographicHalfHeight);
        }
        let before = self.resolve(target_size)?.screen_to_world(anchor);
        self.orthographic_half_height = (self.orthographic_half_height / factor)
            .clamp(MIN_ORTHOGRAPHIC_HALF_HEIGHT, MAX_ORTHOGRAPHIC_HALF_HEIGHT);
        let after = self.resolve(target_size)?.screen_to_world(anchor);
        self.pan_world([before[0] - after[0], before[1] - after[1]]);
        Ok(())
    }

    pub fn resolve(self, target_size: [u32; 2]) -> Result<CameraFrame2d, CameraError> {
        self.validate()?;
        if target_size[0] == 0 || target_size[1] == 0 {
            return Err(CameraError::InvalidTargetSize);
        }
        let aspect = target_size[0] as f32 / target_size[1] as f32;
        let half_extents = [
            self.orthographic_half_height * aspect,
            self.orthographic_half_height,
        ];
        if !half_extents[0].is_finite() {
            return Err(CameraError::InvalidOrthographicHalfHeight);
        }
        Ok(CameraFrame2d {
            target_size,
            center: self.center,
            half_extents,
        })
    }

    fn validate(self) -> Result<(), CameraError> {
        if !self.center.into_iter().all(f32::is_finite) {
            return Err(CameraError::NonFiniteCenter);
        }
        if !self.home_center.into_iter().all(f32::is_finite) {
            return Err(CameraError::NonFiniteCenter);
        }
        if !self.orthographic_half_height.is_finite() || self.orthographic_half_height <= 0.0 {
            return Err(CameraError::InvalidOrthographicHalfHeight);
        }
        if !self.home_orthographic_half_height.is_finite()
            || self.home_orthographic_half_height <= 0.0
        {
            return Err(CameraError::InvalidOrthographicHalfHeight);
        }
        Ok(())
    }
}

impl Default for Camera2d {
    fn default() -> Self {
        Self::fixed()
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraFrame2d {
    target_size: [u32; 2],
    center: [f32; 2],
    half_extents: [f32; 2],
}

impl CameraFrame2d {
    pub const fn target_size(self) -> [u32; 2] {
        self.target_size
    }

    pub const fn center(self) -> [f32; 2] {
        self.center
    }

    pub const fn half_extents(self) -> [f32; 2] {
        self.half_extents
    }

    pub fn screen_to_world(self, normalized: [f32; 2]) -> [f32; 2] {
        [
            self.center[0] + (normalized[0] * 2.0 - 1.0) * self.half_extents[0],
            self.center[1] + (1.0 - normalized[1] * 2.0) * self.half_extents[1],
        ]
    }

    pub fn world_to_screen(self, world: [f32; 2]) -> [f32; 2] {
        [
            ((world[0] - self.center[0]) / self.half_extents[0] + 1.0) * 0.5,
            (1.0 - (world[1] - self.center[1]) / self.half_extents[1]) * 0.5,
        ]
    }

    pub fn screen_delta_to_world(self, delta: [f32; 2]) -> [f32; 2] {
        [
            delta[0] * 2.0 * self.half_extents[0],
            delta[1] * -2.0 * self.half_extents[1],
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 1.0e-5;

    fn assert_point_close(actual: [f32; 2], expected: [f32; 2]) {
        assert!((actual[0] - expected[0]).abs() <= EPSILON, "x: {actual:?}");
        assert!((actual[1] - expected[1]).abs() <= EPSILON, "y: {actual:?}");
    }

    #[test]
    fn screen_world_transforms_round_trip_at_every_aspect() {
        let camera = Camera2d::try_new([0.25, -0.4], 1.7).unwrap();
        for target in [[600, 600], [960, 600], [600, 960]] {
            let frame = camera.resolve(target).unwrap();
            assert_eq!(frame.target_size(), target);
            for point in [[0.25, -0.4], [-0.7, 0.2], [1.9, -1.1]] {
                assert_point_close(frame.screen_to_world(frame.world_to_screen(point)), point);
            }
        }
    }

    #[test]
    fn anchored_zoom_preserves_world_point_and_clamps() {
        let target = [960, 600];
        let anchor = [0.72, 0.31];
        let mut camera = Camera2d::fixed();
        let before = camera.resolve(target).unwrap().screen_to_world(anchor);
        camera.zoom_at(anchor, 1.05, target).unwrap();
        assert_point_close(
            camera.resolve(target).unwrap().screen_to_world(anchor),
            before,
        );
        camera.zoom_at(anchor, f32::MAX, target).unwrap();
        assert_eq!(
            camera.orthographic_half_height(),
            MIN_ORTHOGRAPHIC_HALF_HEIGHT
        );
        camera.zoom_at(anchor, f32::MIN_POSITIVE, target).unwrap();
        assert_eq!(
            camera.orthographic_half_height(),
            MAX_ORTHOGRAPHIC_HALF_HEIGHT
        );
    }

    #[test]
    fn screen_pan_drags_world_content_and_recenter_is_explicit() {
        let mut camera = Camera2d::fixed();
        camera.pan_screen([0.1, -0.2], [1000, 500]).unwrap();
        assert_point_close(camera.center(), [-0.4, -0.4]);
        assert!(camera.recenter());
        assert!(!camera.recenter());
        assert_eq!(camera, Camera2d::fixed());
    }

    #[test]
    fn scene_home_is_the_recenter_target() {
        let mut camera = Camera2d::try_new_at_home([0.0, 2.5], 3.5).unwrap();
        camera.pan_world([1.0, -0.5]);
        camera.zoom_at([0.5, 0.5], 2.0, [960, 600]).unwrap();
        assert!(camera.recenter());
        assert_eq!(camera.center(), [0.0, 2.5]);
        assert_eq!(camera.orthographic_half_height(), 3.5);
        assert!(!camera.recenter());
    }
}
