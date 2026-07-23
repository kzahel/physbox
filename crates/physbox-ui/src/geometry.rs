/// A point in UI pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Point {
    pub x: f32,
    pub y: f32,
}

impl Point {
    pub const fn new(x: f32, y: f32) -> Self {
        Self { x, y }
    }
}

/// A size in UI pixels.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Size {
    pub width: f32,
    pub height: f32,
}

impl Size {
    pub const fn new(width: f32, height: f32) -> Self {
        Self { width, height }
    }
}

/// Insets applied to the four sides of a rectangle.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Insets {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Insets {
    pub const fn all(value: f32) -> Self {
        Self {
            left: value,
            top: value,
            right: value,
            bottom: value,
        }
    }

    pub const fn symmetric(horizontal: f32, vertical: f32) -> Self {
        Self {
            left: horizontal,
            top: vertical,
            right: horizontal,
            bottom: vertical,
        }
    }
}

/// An axis-aligned rectangle stored as origin plus size.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Rect {
    pub x: f32,
    pub y: f32,
    pub width: f32,
    pub height: f32,
}

impl Rect {
    pub const ZERO: Self = Self::new(0.0, 0.0, 0.0, 0.0);

    pub const fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    pub const fn from_array(value: [f32; 4]) -> Self {
        Self::new(value[0], value[1], value[2], value[3])
    }

    pub const fn to_array(self) -> [f32; 4] {
        [self.x, self.y, self.width, self.height]
    }

    pub fn right(self) -> f32 {
        self.x + self.width.max(0.0)
    }

    pub fn bottom(self) -> f32 {
        self.y + self.height.max(0.0)
    }

    pub fn center(self) -> Point {
        Point::new(self.x + self.width * 0.5, self.y + self.height * 0.5)
    }

    pub fn contains(self, point: Point) -> bool {
        point.x >= self.x
            && point.y >= self.y
            && point.x <= self.right()
            && point.y <= self.bottom()
    }

    pub fn inset(self, insets: Insets) -> Self {
        let width = (self.width - insets.left - insets.right).max(0.0);
        let height = (self.height - insets.top - insets.bottom).max(0.0);
        Self::new(self.x + insets.left, self.y + insets.top, width, height)
    }

    pub fn intersect(self, other: Self) -> Option<Self> {
        let left = self.x.max(other.x);
        let top = self.y.max(other.y);
        let right = self.right().min(other.right());
        let bottom = self.bottom().min(other.bottom());
        (right >= left && bottom >= top).then(|| Self::new(left, top, right - left, bottom - top))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inclusive_contains_and_inset_are_stable() {
        let rect = Rect::new(10.0, 20.0, 30.0, 40.0);
        assert!(rect.contains(Point::new(10.0, 20.0)));
        assert!(rect.contains(Point::new(40.0, 60.0)));
        assert!(!rect.contains(Point::new(40.1, 60.0)));
        assert_eq!(
            rect.inset(Insets::symmetric(3.0, 5.0)),
            Rect::new(13.0, 25.0, 24.0, 30.0)
        );
    }

    #[test]
    fn intersection_fails_closed_for_separate_rectangles() {
        assert_eq!(
            Rect::new(0.0, 0.0, 10.0, 10.0).intersect(Rect::new(20.0, 0.0, 4.0, 4.0)),
            None
        );
        assert_eq!(
            Rect::new(0.0, 0.0, 10.0, 10.0).intersect(Rect::new(8.0, 7.0, 4.0, 6.0)),
            Some(Rect::new(8.0, 7.0, 2.0, 3.0))
        );
    }
}
