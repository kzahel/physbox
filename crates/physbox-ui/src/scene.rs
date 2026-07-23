use crate::{Color, HitTarget, Point, Rect};

/// Opaque icon identity interpreted by the canvas renderer.
#[derive(Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct IconId(pub u16);

/// Neutral UI drawing commands. Widgets are composed from these primitives.
#[derive(Clone, Debug, PartialEq)]
pub enum Primitive {
    RoundedRect {
        bounds: Rect,
        fill: Color,
        radius: f32,
        border_width: f32,
        border: Color,
    },
    Text {
        text: String,
        origin: Point,
        pixel_size: f32,
        color: Color,
    },
    Icon {
        icon: IconId,
        bounds: Rect,
        color: Color,
    },
}

/// Bounded primitive capacities for retained and live canvas content.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct BatchCapacities {
    pub retained_primitives: usize,
    pub live_primitives: usize,
    pub hit_targets: usize,
}

impl BatchCapacities {
    pub const fn new(
        retained_primitives: usize,
        live_primitives: usize,
        hit_targets: usize,
    ) -> Self {
        Self {
            retained_primitives,
            live_primitives,
            hit_targets,
        }
    }
}

/// One semantic and visual canvas description.
#[derive(Clone, Debug, PartialEq)]
pub struct CanvasScene {
    /// Caller-owned revision used to avoid redundant retained uploads.
    pub retained_revision: u64,
    pub retained: Vec<Primitive>,
    pub live: Vec<Primitive>,
    pub hit_targets: Vec<HitTarget>,
    pub overflowed: bool,
}

/// Fail-visible builder for a [`CanvasScene`].
#[derive(Debug)]
pub struct CanvasSceneBuilder {
    scene: CanvasScene,
    capacities: BatchCapacities,
}

impl CanvasSceneBuilder {
    pub fn new(retained_revision: u64, capacities: BatchCapacities) -> Self {
        Self {
            scene: CanvasScene {
                retained_revision,
                retained: Vec::with_capacity(capacities.retained_primitives),
                live: Vec::with_capacity(capacities.live_primitives),
                hit_targets: Vec::with_capacity(capacities.hit_targets),
                overflowed: false,
            },
            capacities,
        }
    }

    pub fn push_retained(&mut self, primitive: Primitive) {
        push_bounded(
            &mut self.scene.retained,
            primitive,
            self.capacities.retained_primitives,
            &mut self.scene.overflowed,
        );
    }

    pub fn push_live(&mut self, primitive: Primitive) {
        push_bounded(
            &mut self.scene.live,
            primitive,
            self.capacities.live_primitives,
            &mut self.scene.overflowed,
        );
    }

    pub fn push_hit_target(&mut self, target: HitTarget) {
        push_bounded(
            &mut self.scene.hit_targets,
            target,
            self.capacities.hit_targets,
            &mut self.scene.overflowed,
        );
    }

    pub fn finish(self) -> CanvasScene {
        self.scene
    }
}

fn push_bounded<T>(values: &mut Vec<T>, value: T, capacity: usize, overflowed: &mut bool) {
    if values.len() == capacity {
        *overflowed = true;
    } else {
        values.push(value);
    }
}

/// Renderer-owned bounded work report, independent of application diagnostics.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RenderReport {
    pub retained_instance_capacity: u32,
    pub live_instance_capacity: u32,
    pub instance_stride_bytes: u32,
    pub atlas_bytes: u64,
    pub allocated_bytes: u64,
    pub retained_instances: u32,
    pub live_instances: u32,
    pub retained_glyphs: u32,
    pub live_glyphs: u32,
    pub retained_writes_total: u64,
    pub live_writes_total: u64,
    pub parameter_writes_total: u64,
    pub retained_upload_bytes_total: u64,
    pub live_upload_bytes_total: u64,
    pub parameter_upload_bytes_total: u64,
    pub render_passes_this_frame: u32,
    pub draw_calls_this_frame: u32,
    pub overflowed: bool,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scene_builder_reports_each_bounded_overflow() {
        let mut builder = CanvasSceneBuilder::new(7, BatchCapacities::new(1, 0, 0));
        let primitive = Primitive::RoundedRect {
            bounds: Rect::new(0.0, 0.0, 1.0, 1.0),
            fill: [0.0; 4],
            radius: 0.0,
            border_width: 0.0,
            border: [0.0; 4],
        };
        builder.push_retained(primitive.clone());
        builder.push_retained(primitive);
        builder.push_live(Primitive::Text {
            text: "bounded".to_owned(),
            origin: Point::default(),
            pixel_size: 1.0,
            color: [1.0; 4],
        });
        let scene = builder.finish();
        assert_eq!(scene.retained_revision, 7);
        assert_eq!(scene.retained.len(), 1);
        assert!(scene.live.is_empty());
        assert!(scene.overflowed);
    }
}
