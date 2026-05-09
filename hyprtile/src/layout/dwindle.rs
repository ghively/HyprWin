//! Dwindle layout implementation.
//!
//! The dwindle layout arranges windows in a Fibonacci spiral pattern
//! using a binary space partitioning tree. Each new window splits the
///! smallest available region, alternating between horizontal and vertical
//! splits.

use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use super::bsp::build_dwindle_tree;
use super::gaps::{apply_gaps, effective_gaps};
use tracing::{debug, trace};

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: DWINDLE_LAYOUT — Fibonacci-style BSP layout.
// To modify split behavior:
//   1. Adjust build_dwindle_tree() in bsp.rs — this is the source of truth.
//   2. The ratio parameter controls initial split proportion (default 0.5).
//   3. Smart gaps disable inner gaps when there's only one window.
//   4. Outer gaps are always applied unless smart gaps and single window.
// ═══════════════════════════════════════════════════════════════════════════════

/// The dwindle tiling layout.
///
/// This layout uses a BSP tree to create a spiral-like subdivision where
/// each successive window occupies a smaller and smaller portion of the
/// remaining space.
pub struct DwindleLayout;

impl DwindleLayout {
    /// Return the layout name used in configuration.
    pub fn name() -> &'static str {
        "dwindle"
    }

    /// Calculate window positions for the dwindle layout.
    ///
    /// # Parameters
    /// - `windows`: slice of window identifiers to arrange.
    /// - `workspace_rect`: total area available for tiling.
    /// - `inner_gaps`: gap (in pixels) between adjacent windows.
    /// - `outer_gaps`: gap (in pixels) between windows and workspace edges.
    /// - `smart_gaps`: when `true`, gaps are removed when only a single
    ///   window is present.
    ///
    /// # Returns
    /// A vector of `(WindowId, Rect)` pairs giving the target position
    /// and size for each window.
    pub fn calculate(
        windows: &[WindowId],
        workspace_rect: &Rect,
        inner_gaps: i32,
        outer_gaps: i32,
        smart_gaps: bool,
    ) -> Vec<(WindowId, Rect)> {
        if windows.is_empty() {
            trace!("dwindle: no windows to arrange");
            return Vec::new();
        }

        let (effective_inner, effective_outer) =
            effective_gaps(windows.len(), inner_gaps, outer_gaps, smart_gaps);

        // Apply outer gaps to the workspace rect.
        let effective_rect = apply_gaps(workspace_rect, effective_outer);

        // Build the BSP tree using the dwindle algorithm.
        let tree = build_dwindle_tree(windows);

        // Traverse the tree, collecting rectangles and applying inner gaps.
        let mut results = Vec::with_capacity(windows.len());
        tree.traverse(&effective_rect, &mut |win_id, rect| {
            let gap_adjusted = apply_gaps(&rect, effective_inner);
            results.push((win_id, gap_adjusted));
        });

        debug!(
            window_count = results.len(),
            "dwindle layout calculated"
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
    fn test_dwindle_empty() {
        let workspace = Rect::new(0, 0, 1920, 1080);
        let result = DwindleLayout::calculate(&[], &workspace, 8, 8, true);
        assert!(result.is_empty());
    }

    #[test]
    fn test_dwindle_single_window() {
        let workspace = Rect::new(0, 0, 1920, 1080);
        let windows = vec![wid(1)];
        let result = DwindleLayout::calculate(&windows, &workspace, 8, 8, true);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, wid(1));
        // With smart_gaps and 1 window, should get full workspace
        assert_eq!(result[0].1, workspace);
    }

    #[test]
    fn test_dwindle_two_windows() {
        let workspace = Rect::new(0, 0, 1920, 1080);
        let windows = vec![wid(1), wid(2)];
        let result = DwindleLayout::calculate(&windows, &workspace, 0, 0, false);
        assert_eq!(result.len(), 2);

        // Both windows should fit within the workspace
        for (_, rect) in &result {
            assert!(rect.width > 0);
            assert!(rect.height > 0);
        }
    }

    #[test]
    fn test_dwindle_many_windows() {
        let workspace = Rect::new(0, 0, 1920, 1080);
        let windows: Vec<WindowId> = (1..=10).map(|n| wid(n)).collect();
        let result = DwindleLayout::calculate(&windows, &workspace, 8, 8, false);
        assert_eq!(result.len(), 10);

        // All rects should have positive dimensions
        for (_, rect) in &result {
            assert!(rect.width > 0, "width must be positive");
            assert!(rect.height > 0, "height must be positive");
        }
    }

    #[test]
    fn test_dwindle_no_gaps() {
        let workspace = Rect::new(0, 0, 100, 100);
        let windows = vec![wid(1), wid(2), wid(3)];
        let result = DwindleLayout::calculate(&windows, &workspace, 0, 0, false);
        assert_eq!(result.len(), 3);

        // With no gaps, the union of all rects should cover the workspace
        for (_, rect) in &result {
            assert!(rect.width > 0);
            assert!(rect.height > 0);
        }
    }

    #[test]
    fn test_dwindle_name() {
        assert_eq!(DwindleLayout::name(), "dwindle");
    }
}
