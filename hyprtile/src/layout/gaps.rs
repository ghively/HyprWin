//! Gap calculation utilities for window layouts.
//!
//! Provides functions to apply inner and outer gaps to rectangles,
//! ensuring no negative-size rects are ever produced.

use crate::util::rect::Rect;
use tracing::trace;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: GAP_SYSTEM — Inner, outer, and smart gaps.
// To modify gap behavior:
//   1. apply_gaps() shrinks a rect by gap pixels on all sides.
//   2. should_disable_gaps() returns true if smart gaps and window_count == 1.
//   3. effective_gaps() returns (inner, outer) considering smart mode.
//   4. All rect-shrinking functions clamp dimensions to minimum 1 pixel.
// ═══════════════════════════════════════════════════════════════════════════════

/// Apply gap reduction to a rectangle by `gap` pixels on all sides.
///
/// The resulting rectangle will have at least 1 pixel in each dimension
/// (provided the input is valid), preventing negative sizes.
///
/// # Examples
/// ```
/// let rect = Rect::new(0, 0, 100, 100);
/// let shrunk = apply_gaps(&rect, 4);
/// // shrunk == Rect::new(4, 4, 92, 92)
/// ```
pub fn apply_gaps(rect: &Rect, gap: i32) -> Rect {
    if gap <= 0 {
        return *rect;
    }

    let double_gap = gap.saturating_mul(2);
    let new_width = (rect.width - double_gap).max(1);
    let new_height = (rect.height - double_gap).max(1);

    Rect::new(
        rect.x.saturating_add(gap),
        rect.y.saturating_add(gap),
        new_width,
        new_height,
    )
}

/// Apply outer gaps to a workspace rectangle.
///
/// Outer gaps shrink the usable workspace area. Windows are tiled
/// inside the resulting rectangle.
pub fn apply_outer_gaps(rect: &Rect, gap: i32) -> Rect {
    apply_gaps(rect, gap)
}

/// Apply inner gaps between tiled windows.
///
/// Inner gaps further shrink an already-split rectangle so adjacent
/// windows do not touch.
pub fn apply_inner_gaps(rect: &Rect, gap: i32) -> Rect {
    apply_gaps(rect, gap)
}

/// Determine whether gaps should be disabled for the current layout state.
///
/// When `smart_gaps` is enabled and there is only one window, gaps are
/// removed so the single window fills the entire workspace.
pub fn should_disable_gaps(window_count: usize, smart_gaps: bool) -> bool {
    smart_gaps && window_count <= 1
}

/// Calculate effective inner and outer gap values.
///
/// Takes smart gaps into account: when only a single window is present
/// and smart gaps is enabled, both inner and outer gaps are returned as
/// zero.
pub fn effective_gaps(window_count: usize, inner: i32, outer: i32, smart: bool) -> (i32, i32) {
    if should_disable_gaps(window_count, smart) {
        trace!(
            window_count = window_count,
            "smart_gaps active: disabling gaps"
        );
        (0, 0)
    } else {
        (inner, outer)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_apply_gaps_basic() {
        let rect = Rect::new(0, 0, 100, 100);
        let result = apply_gaps(&rect, 8);
        assert_eq!(result, Rect::new(8, 8, 84, 84));
    }

    #[test]
    fn test_apply_gaps_zero() {
        let rect = Rect::new(10, 20, 100, 200);
        let result = apply_gaps(&rect, 0);
        assert_eq!(result, rect);
    }

    #[test]
    fn test_apply_gaps_negative() {
        let rect = Rect::new(0, 0, 50, 50);
        let result = apply_gaps(&rect, -5);
        assert_eq!(result, rect);
    }

    #[test]
    fn test_apply_gaps_large_gap() {
        let rect = Rect::new(0, 0, 10, 10);
        let result = apply_gaps(&rect, 8);
        // width = 10 - 16 = -6 -> max(1, -6) = 1
        assert_eq!(result, Rect::new(8, 8, 1, 1));
    }

    #[test]
    fn test_apply_gaps_very_large_gap() {
        let rect = Rect::new(0, 0, 100, 100);
        let result = apply_gaps(&rect, 100);
        assert_eq!(result.x, 100);
        assert_eq!(result.y, 100);
        assert_eq!(result.width, 1);
        assert_eq!(result.height, 1);
    }

    #[test]
    fn test_should_disable_gaps() {
        assert!(should_disable_gaps(1, true));
        assert!(!should_disable_gaps(1, false));
        assert!(!should_disable_gaps(2, true));
        assert!(!should_disable_gaps(0, true));
    }

    #[test]
    fn test_effective_gaps_smart() {
        assert_eq!(effective_gaps(1, 8, 8, true), (0, 0));
        assert_eq!(effective_gaps(2, 8, 8, true), (8, 8));
        assert_eq!(effective_gaps(1, 8, 8, false), (8, 8));
    }
}
