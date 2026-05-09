//! Monocle layout implementation.
//!
//! In monocle mode every window receives the full workspace rectangle.
//! Only the focused window is actually visible; all others are stacked
//! behind it. This is useful when you want to focus on a single task
//! while keeping all other windows easily accessible via focus cycling.

use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use super::gaps::{apply_gaps, effective_gaps};
use tracing::trace;

/// The monocle tiling layout.
pub struct MonocleLayout;

impl MonocleLayout {
    /// Return the layout name used in configuration.
    pub fn name() -> &'static str {
        "monocle"
    }

    /// Calculate window positions for the monocle layout.
    ///
    /// Every window in `windows` receives the full workspace rectangle
    /// (minus gaps). The `focused_idx` parameter determines which window
    /// is conceptually "on top", but since all rects are identical this
    /// only affects the ordering of results.
    ///
    /// # Parameters
    /// - `windows`: slice of window identifiers to arrange.
    /// - `workspace_rect`: total area available for tiling.
    /// - `inner_gaps`: gap between adjacent windows (unused; monocle has
    ///   only one visible window at a time).
    /// - `outer_gaps`: gap between windows and workspace edges.
    /// - `smart_gaps`: when `true`, disable gaps for single windows.
    /// - `focused_idx`: index of the focused window within `windows`.
    ///
    /// # Returns
    /// A vector of `(WindowId, Rect)` pairs.
    pub fn calculate(
        windows: &[WindowId],
        workspace_rect: &Rect,
        inner_gaps: i32,
        outer_gaps: i32,
        smart_gaps: bool,
        focused_idx: usize,
    ) -> Vec<(WindowId, Rect)> {
        if windows.is_empty() {
            return Vec::new();
        }

        let (effective_inner, effective_outer) =
            effective_gaps(windows.len(), inner_gaps, outer_gaps, smart_gaps);

        // Apply outer gaps first.
        let outer_rect = apply_gaps(workspace_rect, effective_outer);
        // Apply inner gaps (normally zero for monocle since windows don't
        // share edges, but apply for consistency).
        let final_rect = apply_gaps(&outer_rect, effective_inner);

        // Ensure the rect has positive dimensions.
        let final_rect = Rect::new(
            final_rect.x,
            final_rect.y,
            final_rect.width.max(1),
            final_rect.height.max(1),
        );

        let mut results = Vec::with_capacity(windows.len());

        // Place the focused window last in the results so it visually
        // appears on top (last-painted window wins in most compositors).
        for (i, &window_id) in windows.iter().enumerate() {
            if i == focused_idx {
                continue;
            }
            results.push((window_id, final_rect));
        }

        // Add the focused window last so it renders on top.
        let focused_idx_clamped = focused_idx.min(windows.len() - 1);
        results.push((windows[focused_idx_clamped], final_rect));

        trace!(
            window_count = results.len(),
            focused_idx = focused_idx_clamped,
            "monocle layout calculated"
        );
        results
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wid(n: isize) -> WindowId {
        WindowId(n)
    }

    #[test]
    fn test_monocle_empty() {
        let workspace = Rect::new(0, 0, 1920, 1080);
        let result =
            MonocleLayout::calculate(&[], &workspace, 8, 8, true, 0);
        assert!(result.is_empty());
    }

    #[test]
    fn test_monocle_single_window() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1)];
        let result =
            MonocleLayout::calculate(&windows, &workspace, 0, 0, false, 0);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, wid(1));
        // Full workspace rect
        assert_eq!(result[0].1, workspace);
    }

    #[test]
    fn test_monocle_multiple_windows() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2), wid(3)];
        let result =
            MonocleLayout::calculate(&windows, &workspace, 0, 0, false, 1);
        assert_eq!(result.len(), 3);

        // All windows should have the same rectangle
        let first_rect = result[0].1;
        for (_, rect) in &result {
            assert_eq!(*rect, first_rect);
        }
    }

    #[test]
    fn test_monocle_focused_on_top() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2), wid(3)];
        let result =
            MonocleLayout::calculate(&windows, &workspace, 0, 0, false, 1);

        // The focused window (index 1 = wid(2)) should be last in results
        assert_eq!(result.last().unwrap().0, wid(2));
    }

    #[test]
    fn test_monocle_focused_idx_clamped() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2)];
        // focused_idx out of bounds should be clamped
        let result =
            MonocleLayout::calculate(&windows, &workspace, 0, 0, false, 99);
        assert_eq!(result.len(), 2);
        // Should use last valid index
        assert_eq!(result.last().unwrap().0, wid(2));
    }

    #[test]
    fn test_monocle_name() {
        assert_eq!(MonocleLayout::name(), "monocle");
    }

    #[test]
    fn test_monocle_with_gaps() {
        let workspace = Rect::new(0, 0, 100, 100);
        let windows = vec![wid(1), wid(2)];
        let result =
            MonocleLayout::calculate(&windows, &workspace, 4, 4, false, 0);
        assert_eq!(result.len(), 2);
        // With gaps, rect should be smaller than workspace
        assert!(result[0].1.width < workspace.width);
        assert!(result[0].1.height < workspace.height);
    }

    #[test]
    fn test_monocle_smart_gaps_single() {
        let workspace = Rect::new(0, 0, 100, 100);
        let windows = vec![wid(1)];
        let result =
            MonocleLayout::calculate(&windows, &workspace, 8, 8, true, 0);
        // With smart gaps and 1 window, should get full workspace
        assert_eq!(result[0].1, workspace);
    }
}
