use serde::{Deserialize, Serialize};
use windows::Win32::Foundation::RECT;

/// A rectangle representing a window or monitor area in screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    /// Create a new rectangle with the given position and size.
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self {
        Self {
            x,
            y,
            width,
            height,
        }
    }

    /// Convert from a Win32 `RECT` to a `Rect`.
    ///
    /// The Win32 `RECT` uses left/top/right/bottom, so width is `right - left`
    /// and height is `bottom - top`.
    pub fn from_win32(rect: &RECT) -> Self {
        Self {
            x: rect.left,
            y: rect.top,
            width: rect.right - rect.left,
            height: rect.bottom - rect.top,
        }
    }

    /// Convert this `Rect` to a Win32 `RECT`.
    pub fn to_win32(&self) -> RECT {
        RECT {
            left: self.x,
            top: self.y,
            right: self.x + self.width,
            bottom: self.y + self.height,
        }
    }

    /// Check if the rectangle contains the given point.
    pub fn contains(&self, point: (i32, i32)) -> bool {
        let (px, py) = point;
        px >= self.x
            && px < self.x + self.width
            && py >= self.y
            && py < self.y + self.height
    }

    /// Check if this rectangle intersects with another.
    pub fn intersects(&self, other: &Rect) -> bool {
        self.x < other.x + other.width
            && self.x + self.width > other.x
            && self.y < other.y + other.height
            && self.y + self.height > other.y
    }

    /// Shrink the rectangle by the given amount on all sides.
    ///
    /// The inset amount is applied to each side, so the total reduction
    /// in width and height is `2 * amount`.
    pub fn inset(&self, amount: i32) -> Rect {
        Rect {
            x: self.x + amount,
            y: self.y + amount,
            width: (self.width - 2 * amount).max(0),
            height: (self.height - 2 * amount).max(0),
        }
    }

    /// Split the rectangle horizontally at the given ratio.
    ///
    /// Returns `(left, right)` where `left` takes up `ratio` of the width
    /// and `right` takes up the remaining `1.0 - ratio`.
    pub fn split_horizontal(&self, ratio: f64) -> (Rect, Rect) {
        let clamped_ratio = ratio.clamp(0.0, 1.0);
        let split_x = self.x + (self.width as f64 * clamped_ratio).round() as i32;

        let left = Rect {
            x: self.x,
            y: self.y,
            width: split_x - self.x,
            height: self.height,
        };

        let right = Rect {
            x: split_x,
            y: self.y,
            width: self.x + self.width - split_x,
            height: self.height,
        };

        (left, right)
    }

    /// Split the rectangle vertically at the given ratio.
    ///
    /// Returns `(top, bottom)` where `top` takes up `ratio` of the height
    /// and `bottom` takes up the remaining `1.0 - ratio`.
    pub fn split_vertical(&self, ratio: f64) -> (Rect, Rect) {
        let clamped_ratio = ratio.clamp(0.0, 1.0);
        let split_y = self.y + (self.height as f64 * clamped_ratio).round() as i32;

        let top = Rect {
            x: self.x,
            y: self.y,
            width: self.width,
            height: split_y - self.y,
        };

        let bottom = Rect {
            x: self.x,
            y: split_y,
            width: self.width,
            height: self.y + self.height - split_y,
        };

        (top, bottom)
    }

    /// Calculate the area of the rectangle.
    pub fn area(&self) -> i32 {
        self.width * self.height
    }

    /// Get the center point of the rectangle.
    pub fn center(&self) -> (i32, i32) {
        (self.x + self.width / 2, self.y + self.height / 2)
    }

    /// Check if the rectangle has zero or negative area.
    pub fn is_empty(&self) -> bool {
        self.width <= 0 || self.height <= 0
    }

    /// Adjust this rectangle for gaps.
    ///
    /// Applies outer gaps (shrinking the rect) and optionally inner gaps.
    /// When `is_single` is true and smart gaps are enabled, the rectangle
    /// is returned unchanged (outer gaps are applied but inner is not needed).
    pub fn adjust_for_gaps(&self, inner: i32, outer: i32, is_single: bool) -> Rect {
        if is_single {
            // For a single window, only apply outer gaps.
            self.inset(outer)
        } else {
            // For multiple windows, apply both outer and inner gaps.
            self.inset(outer + inner)
        }
    }
}

/// A point in 2D screen coordinates.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    /// Create a new point.
    pub fn new(x: i32, y: i32) -> Self {
        Self { x, y }
    }

    /// Calculate the Euclidean distance to another point.
    pub fn distance_to(&self, other: &Point) -> f64 {
        let dx = (self.x - other.x) as f64;
        let dy = (self.y - other.y) as f64;
        (dx * dx + dy * dy).sqrt()
    }
}

/// Create a `Rect` from a monitor work area `RECT`.
///
/// The work area excludes the taskbar and other reserved screen space.
pub fn rect_from_monitor_work_area(work_area: &RECT) -> Rect {
    Rect::from_win32(work_area)
}

/// Center a window of the given size within a container rectangle.
///
/// Returns a `Rect` representing the centered window's position and size.
pub fn center_window_in_rect(window_size: (i32, i32), container: &Rect) -> Rect {
    let (win_width, win_height) = window_size;
    let (center_x, center_y) = container.center();

    Rect {
        x: center_x - win_width / 2,
        y: center_y - win_height / 2,
        width: win_width,
        height: win_height,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rect_new() {
        let r = Rect::new(10, 20, 100, 200);
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 20);
        assert_eq!(r.width, 100);
        assert_eq!(r.height, 200);
    }

    #[test]
    fn test_rect_from_win32() {
        let rect = RECT {
            left: 10,
            top: 20,
            right: 110,
            bottom: 220,
        };
        let r = Rect::from_win32(&rect);
        assert_eq!(r.x, 10);
        assert_eq!(r.y, 20);
        assert_eq!(r.width, 100);
        assert_eq!(r.height, 200);
    }

    #[test]
    fn test_rect_to_win32() {
        let r = Rect::new(10, 20, 100, 200);
        let rect = r.to_win32();
        assert_eq!(rect.left, 10);
        assert_eq!(rect.top, 20);
        assert_eq!(rect.right, 110);
        assert_eq!(rect.bottom, 220);
    }

    #[test]
    fn test_contains() {
        let r = Rect::new(0, 0, 100, 100);
        assert!(r.contains((50, 50)));
        assert!(r.contains((0, 0)));
        assert!(!r.contains((100, 50)));
        assert!(!r.contains((50, 100)));
        assert!(!r.contains((-1, 50)));
    }

    #[test]
    fn test_intersects() {
        let a = Rect::new(0, 0, 100, 100);
        let b = Rect::new(50, 50, 100, 100);
        assert!(a.intersects(&b));

        let c = Rect::new(200, 200, 50, 50);
        assert!(!a.intersects(&c));
    }

    #[test]
    fn test_inset() {
        let r = Rect::new(0, 0, 100, 100);
        let inset = r.inset(10);
        assert_eq!(inset.x, 10);
        assert_eq!(inset.y, 10);
        assert_eq!(inset.width, 80);
        assert_eq!(inset.height, 80);
    }

    #[test]
    fn test_split_horizontal() {
        let r = Rect::new(0, 0, 100, 100);
        let (left, right) = r.split_horizontal(0.5);
        assert_eq!(left.x, 0);
        assert_eq!(left.width, 50);
        assert_eq!(right.x, 50);
        assert_eq!(right.width, 50);
    }

    #[test]
    fn test_split_vertical() {
        let r = Rect::new(0, 0, 100, 100);
        let (top, bottom) = r.split_vertical(0.5);
        assert_eq!(top.y, 0);
        assert_eq!(top.height, 50);
        assert_eq!(bottom.y, 50);
        assert_eq!(bottom.height, 50);
    }

    #[test]
    fn test_area() {
        let r = Rect::new(0, 0, 10, 20);
        assert_eq!(r.area(), 200);
    }

    #[test]
    fn test_center() {
        let r = Rect::new(0, 0, 100, 200);
        assert_eq!(r.center(), (50, 100));
    }

    #[test]
    fn test_is_empty() {
        assert!(!Rect::new(0, 0, 1, 1).is_empty());
        assert!(Rect::new(0, 0, 0, 10).is_empty());
        assert!(Rect::new(0, 0, 10, 0).is_empty());
        assert!(Rect::new(0, 0, -5, 10).is_empty());
    }

    #[test]
    fn test_adjust_for_gaps_single() {
        let r = Rect::new(0, 0, 100, 100);
        let adjusted = r.adjust_for_gaps(5, 10, true);
        assert_eq!(adjusted.x, 10);
        assert_eq!(adjusted.y, 10);
        assert_eq!(adjusted.width, 80);
        assert_eq!(adjusted.height, 80);
    }

    #[test]
    fn test_adjust_for_gaps_multi() {
        let r = Rect::new(0, 0, 100, 100);
        let adjusted = r.adjust_for_gaps(5, 10, false);
        assert_eq!(adjusted.x, 15);
        assert_eq!(adjusted.y, 15);
        assert_eq!(adjusted.width, 70);
        assert_eq!(adjusted.height, 70);
    }

    #[test]
    fn test_point_distance() {
        let a = Point::new(0, 0);
        let b = Point::new(3, 4);
        assert!((a.distance_to(&b) - 5.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_center_window_in_rect() {
        let container = Rect::new(0, 0, 200, 200);
        let win = center_window_in_rect((50, 50), &container);
        assert_eq!(win.x, 75);
        assert_eq!(win.y, 75);
        assert_eq!(win.width, 50);
        assert_eq!(win.height, 50);
    }
}
