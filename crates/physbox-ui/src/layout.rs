use crate::{Insets, Rect, Size};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Axis {
    Horizontal,
    Vertical,
}

/// Cross-host UI scale derived only from the render-target extent.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResponsiveScale(pub f32);

impl ResponsiveScale {
    pub fn for_target(target: [u32; 2], reference_short_edge: f32) -> Self {
        let short = target[0].max(1).min(target[1].max(1)) as f32;
        Self((short / reference_short_edge.max(1.0)).clamp(0.65, 2.0))
    }

    pub fn pixels(self, logical: f32) -> f32 {
        logical * self.0
    }
}

/// A deterministic one-dimensional layout cursor.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct LinearLayout {
    bounds: Rect,
    axis: Axis,
    gap: f32,
    cursor: f32,
}

impl LinearLayout {
    pub fn new(bounds: Rect, axis: Axis, gap: f32, padding: Insets) -> Self {
        let bounds = bounds.inset(padding);
        let cursor = match axis {
            Axis::Horizontal => bounds.x,
            Axis::Vertical => bounds.y,
        };
        Self {
            bounds,
            axis,
            gap,
            cursor,
        }
    }

    pub fn next(&mut self, size: Size) -> Option<Rect> {
        let rect = match self.axis {
            Axis::Horizontal => Rect::new(self.cursor, self.bounds.y, size.width, size.height),
            Axis::Vertical => Rect::new(self.bounds.x, self.cursor, size.width, size.height),
        };
        let fits = rect.right() <= self.bounds.right() && rect.bottom() <= self.bounds.bottom();
        if !fits {
            return None;
        }
        self.cursor = match self.axis {
            Axis::Horizontal => rect.right() + self.gap,
            Axis::Vertical => rect.bottom() + self.gap,
        };
        Some(rect)
    }
}

/// A wrapping left-to-right layout for touch-sized controls.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct FlowLayout {
    bounds: Rect,
    item: Size,
    gap: Size,
    cursor_x: f32,
    cursor_y: f32,
}

/// Multiple children occupying the same bounded region.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OverlayLayout {
    bounds: Rect,
}

impl OverlayLayout {
    pub const fn new(bounds: Rect) -> Self {
        Self { bounds }
    }

    pub const fn layer(self) -> Rect {
        self.bounds
    }
}

/// A vertically scrolling layout whose geometry is clipped to one viewport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ScrollColumn {
    viewport: Rect,
    content_y: f32,
    gap: f32,
    cursor: f32,
}

impl ScrollColumn {
    pub fn new(viewport: Rect, content_offset: f32, gap: f32, padding: Insets) -> Self {
        let viewport = viewport.inset(padding);
        Self {
            viewport,
            content_y: viewport.y - content_offset.max(0.0),
            gap,
            cursor: 0.0,
        }
    }

    /// Returns the full item bounds and the visible clipped portion.
    pub fn next(&mut self, height: f32) -> (Rect, Option<Rect>) {
        let bounds = Rect::new(
            self.viewport.x,
            self.content_y + self.cursor,
            self.viewport.width,
            height.max(0.0),
        );
        self.cursor += height.max(0.0) + self.gap;
        (bounds, bounds.intersect(self.viewport))
    }

    pub const fn viewport(self) -> Rect {
        self.viewport
    }
}

impl FlowLayout {
    pub fn new(bounds: Rect, item: Size, gap: Size, padding: Insets) -> Self {
        let bounds = bounds.inset(padding);
        Self {
            bounds,
            item,
            gap,
            cursor_x: bounds.x,
            cursor_y: bounds.y,
        }
    }

    pub fn next(&mut self) -> Option<Rect> {
        if self.cursor_x + self.item.width > self.bounds.right() {
            self.cursor_x = self.bounds.x;
            self.cursor_y += self.item.height + self.gap.height;
        }
        let rect = Rect::new(
            self.cursor_x,
            self.cursor_y,
            self.item.width,
            self.item.height,
        );
        if rect.bottom() > self.bounds.bottom() {
            return None;
        }
        self.cursor_x = rect.right() + self.gap.width;
        Some(rect)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_layout_fails_closed_at_bounds() {
        let mut layout = LinearLayout::new(
            Rect::new(0.0, 0.0, 100.0, 30.0),
            Axis::Horizontal,
            5.0,
            Insets::all(5.0),
        );
        assert_eq!(
            layout.next(Size::new(30.0, 20.0)),
            Some(Rect::new(5.0, 5.0, 30.0, 20.0))
        );
        assert_eq!(
            layout.next(Size::new(50.0, 20.0)),
            Some(Rect::new(40.0, 5.0, 50.0, 20.0))
        );
        assert_eq!(layout.next(Size::new(1.0, 20.0)), None);
    }

    #[test]
    fn flow_wraps_and_stops_when_height_is_exhausted() {
        let mut layout = FlowLayout::new(
            Rect::new(0.0, 0.0, 75.0, 45.0),
            Size::new(30.0, 20.0),
            Size::new(5.0, 5.0),
            Insets::default(),
        );
        assert_eq!(layout.next(), Some(Rect::new(0.0, 0.0, 30.0, 20.0)));
        assert_eq!(layout.next(), Some(Rect::new(35.0, 0.0, 30.0, 20.0)));
        assert_eq!(layout.next(), Some(Rect::new(0.0, 25.0, 30.0, 20.0)));
        assert_eq!(layout.next(), Some(Rect::new(35.0, 25.0, 30.0, 20.0)));
        assert_eq!(layout.next(), None);
    }

    #[test]
    fn scroll_column_reports_full_and_clipped_geometry() {
        let mut layout = ScrollColumn::new(
            Rect::new(0.0, 10.0, 100.0, 40.0),
            15.0,
            5.0,
            Insets::default(),
        );
        let (first, visible) = layout.next(20.0);
        assert_eq!(first, Rect::new(0.0, -5.0, 100.0, 20.0));
        assert_eq!(visible, Some(Rect::new(0.0, 10.0, 100.0, 5.0)));
        let (second, visible) = layout.next(20.0);
        assert_eq!(second, Rect::new(0.0, 20.0, 100.0, 20.0));
        assert_eq!(visible, Some(second));
    }
}
