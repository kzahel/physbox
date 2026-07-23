use crate::{
    Color, HitTarget, IconId, Point, Primitive, Rect, Theme, WidgetId, WidgetRole, text_width,
};

/// Neutral widget output. Application meaning remains associated with the
/// caller-selected [`WidgetId`] values.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct WidgetScene {
    pub primitives: Vec<Primitive>,
    pub hit_targets: Vec<HitTarget>,
    pub overflowed: bool,
}

/// Bounded immediate widget painter shared by every host.
///
/// Widgets add their visual primitives and their exact hit target together, so
/// drawing and interaction cannot drift onto separate geometry.
pub struct WidgetCanvas {
    theme: Theme,
    primitives: Vec<Primitive>,
    hit_targets: Vec<HitTarget>,
    primitive_capacity: usize,
    hit_capacity: usize,
    overflowed: bool,
}

impl WidgetCanvas {
    pub fn new(theme: Theme, primitive_capacity: usize, hit_capacity: usize) -> Self {
        Self {
            theme,
            primitives: Vec::with_capacity(primitive_capacity.min(128)),
            hit_targets: Vec::with_capacity(hit_capacity.min(64)),
            primitive_capacity,
            hit_capacity,
            overflowed: false,
        }
    }

    pub const fn theme(&self) -> Theme {
        self.theme
    }

    pub fn panel(&mut self, bounds: Rect) {
        self.rounded_rect(
            bounds,
            self.theme.panel,
            self.theme.corner_radius,
            self.theme.border,
        );
    }

    pub fn card(&mut self, bounds: Rect) {
        self.rounded_rect(
            bounds,
            self.theme.card,
            self.theme.control_radius,
            self.theme.border,
        );
    }

    pub fn label(&mut self, text: &str, origin: Point, pixel_size: f32, color: Color) {
        self.push_primitive(Primitive::Text {
            text: text.to_owned(),
            origin,
            pixel_size,
            color,
        });
    }

    pub fn button(
        &mut self,
        id: WidgetId,
        bounds: Rect,
        label: &str,
        selected: bool,
        pixel_size: f32,
    ) {
        self.control(id, bounds, WidgetRole::Button, selected);
        let width = text_width(label, pixel_size);
        let origin = Point::new(
            bounds.x + ((bounds.width - width) * 0.5).max(5.0),
            bounds.y + (bounds.height - 7.0 * pixel_size) * 0.5,
        );
        self.label(
            label,
            origin,
            pixel_size,
            if selected {
                self.theme.accent
            } else {
                self.theme.text
            },
        );
    }

    pub fn icon_button(&mut self, id: WidgetId, bounds: Rect, icon: IconId, selected: bool) {
        self.control(id, bounds, WidgetRole::Button, selected);
        let inset = bounds.height.min(bounds.width) * 0.24;
        self.push_primitive(Primitive::Icon {
            icon,
            bounds: bounds.inset(crate::Insets::all(inset)),
            color: if selected {
                self.theme.accent
            } else {
                self.theme.text
            },
        });
    }

    pub fn tab(
        &mut self,
        id: WidgetId,
        bounds: Rect,
        label: &str,
        selected: bool,
        pixel_size: f32,
    ) {
        self.button(id, bounds, label, selected, pixel_size);
    }

    pub fn segmented_control(
        &mut self,
        segments: &[(WidgetId, Rect, &str, bool)],
        pixel_size: f32,
    ) {
        for &(id, bounds, label, selected) in segments {
            self.button(id, bounds, label, selected, pixel_size);
        }
    }

    pub fn toggle(
        &mut self,
        id: WidgetId,
        bounds: Rect,
        label: &str,
        enabled: bool,
        pixel_size: f32,
    ) {
        self.control(id, bounds, WidgetRole::Button, enabled);
        let switch_width = bounds.height * 1.45;
        let switch_bounds = Rect::new(
            bounds.right() - switch_width - 7.0,
            bounds.y + bounds.height * 0.23,
            switch_width,
            bounds.height * 0.54,
        );
        self.rounded_rect(
            switch_bounds,
            if enabled {
                self.theme.control_selected
            } else {
                self.theme.scrim
            },
            switch_bounds.height * 0.5,
            if enabled {
                self.theme.border_selected
            } else {
                self.theme.border
            },
        );
        let thumb = switch_bounds.height * 0.72;
        let thumb_x = if enabled {
            switch_bounds.right() - thumb - (switch_bounds.height - thumb) * 0.5
        } else {
            switch_bounds.x + (switch_bounds.height - thumb) * 0.5
        };
        self.rounded_rect(
            Rect::new(
                thumb_x,
                switch_bounds.y + (switch_bounds.height - thumb) * 0.5,
                thumb,
                thumb,
            ),
            self.theme.text,
            thumb * 0.5,
            self.theme.text,
        );
        self.label(
            label,
            Point::new(
                bounds.x + 9.0,
                bounds.y + (bounds.height - 7.0 * pixel_size) * 0.5,
            ),
            pixel_size,
            if enabled {
                self.theme.accent
            } else {
                self.theme.text
            },
        );
    }

    pub fn discrete_slider(
        &mut self,
        id: WidgetId,
        bounds: Rect,
        normalized_value: f32,
        steps: u32,
    ) {
        self.slider(id, bounds, normalized_value, steps);
    }

    pub fn continuous_slider(&mut self, id: WidgetId, bounds: Rect, normalized_value: f32) {
        self.slider(id, bounds, normalized_value, 0);
    }

    pub fn separator(&mut self, bounds: Rect) {
        self.rounded_rect(bounds, self.theme.border, 0.0, self.theme.border);
    }

    /// Non-interactive rounded decoration using caller-selected neutral colors.
    pub fn decoration(
        &mut self,
        bounds: Rect,
        fill: Color,
        radius: f32,
        border_width: f32,
        border: Color,
    ) {
        self.push_primitive(Primitive::RoundedRect {
            bounds,
            fill,
            radius,
            border_width,
            border,
        });
    }

    pub fn finish(self) -> WidgetScene {
        WidgetScene {
            primitives: self.primitives,
            hit_targets: self.hit_targets,
            overflowed: self.overflowed,
        }
    }

    fn slider(&mut self, id: WidgetId, bounds: Rect, normalized_value: f32, steps: u32) {
        self.push_target(HitTarget {
            id,
            bounds,
            role: WidgetRole::HorizontalSlider { steps },
        });
        let track_height = 4.0_f32.min(bounds.height);
        let track = Rect::new(
            bounds.x + 8.0,
            bounds.y + (bounds.height - track_height) * 0.5,
            (bounds.width - 16.0).max(1.0),
            track_height,
        );
        let value = normalized_value.clamp(0.0, 1.0);
        self.rounded_rect(
            track,
            self.theme.scrim,
            track_height * 0.5,
            self.theme.border,
        );
        self.rounded_rect(
            Rect::new(
                track.x,
                track.y,
                (track.width * value).max(1.0),
                track.height,
            ),
            self.theme.accent,
            track_height * 0.5,
            self.theme.accent,
        );
        let thumb = bounds.height.min(18.0);
        self.rounded_rect(
            Rect::new(
                track.x + track.width * value - thumb * 0.5,
                bounds.y + (bounds.height - thumb) * 0.5,
                thumb,
                thumb,
            ),
            self.theme.text,
            thumb * 0.5,
            self.theme.border_selected,
        );
    }

    fn control(&mut self, id: WidgetId, bounds: Rect, role: WidgetRole, selected: bool) {
        self.push_target(HitTarget { id, bounds, role });
        self.rounded_rect(
            bounds,
            if selected {
                self.theme.control_selected
            } else {
                self.theme.control
            },
            self.theme.control_radius,
            if selected {
                self.theme.border_selected
            } else {
                self.theme.border
            },
        );
    }

    fn rounded_rect(&mut self, bounds: Rect, fill: Color, radius: f32, border: Color) {
        self.push_primitive(Primitive::RoundedRect {
            bounds,
            fill,
            radius,
            border_width: self.theme.border_width,
            border,
        });
    }

    fn push_primitive(&mut self, primitive: Primitive) {
        if self.primitives.len() == self.primitive_capacity {
            self.overflowed = true;
        } else {
            self.primitives.push(primitive);
        }
    }

    fn push_target(&mut self, target: HitTarget) {
        if self.hit_targets.len() == self.hit_capacity {
            self.overflowed = true;
        } else {
            self.hit_targets.push(target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{PointerEvent, PointerId, PointerPhase};

    #[test]
    fn controls_share_exact_draw_and_hit_geometry() {
        let mut canvas = WidgetCanvas::new(Theme::DARK, 32, 8);
        let bounds = Rect::new(10.0, 20.0, 120.0, 36.0);
        canvas.button(WidgetId::new(7), bounds, "BUTTON", false, 1.5);
        let scene = canvas.finish();
        assert_eq!(scene.hit_targets[0].bounds, bounds);
        assert!(matches!(
            scene.primitives[0],
            Primitive::RoundedRect {
                bounds: drawn,
                ..
            } if drawn == bounds
        ));
    }

    #[test]
    fn slider_uses_declared_snapping_and_bounded_output() {
        let mut canvas = WidgetCanvas::new(Theme::DARK, 4, 1);
        let bounds = Rect::new(10.0, 10.0, 100.0, 30.0);
        canvas.discrete_slider(WidgetId::new(4), bounds, 0.5, 4);
        let scene = canvas.finish();
        assert_eq!(
            scene.hit_targets[0].role,
            WidgetRole::HorizontalSlider { steps: 4 }
        );
        assert_eq!(scene.primitives.len(), 3);
        assert!(!scene.overflowed);

        let mut interactions = crate::InteractionState::default();
        let report = interactions.route(
            &[PointerEvent {
                id: PointerId(1),
                phase: PointerPhase::Down,
                button: crate::PointerButton::Primary,
                position: Point::new(82.0, 20.0),
            }],
            &scene.hit_targets,
        );
        assert_eq!(report.widget_events[0].normalized_value, Some(0.75));
    }

    #[test]
    fn widget_capacity_overflow_is_visible() {
        let mut canvas = WidgetCanvas::new(Theme::DARK, 0, 0);
        canvas.button(
            WidgetId::new(1),
            Rect::new(0.0, 0.0, 20.0, 20.0),
            "X",
            false,
            1.0,
        );
        let scene = canvas.finish();
        assert!(scene.primitives.is_empty());
        assert!(scene.hit_targets.is_empty());
        assert!(scene.overflowed);
    }
}
