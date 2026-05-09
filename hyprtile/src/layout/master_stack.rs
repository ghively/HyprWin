//! Master-stack layout implementation.
//!
//! The master area (left in horizontal mode, top in vertical mode)
//! holds the "master" windows. Remaining windows are stacked in the
//! secondary area. This is a classic tiling layout seen in many
//! window managers such as dwm and XMonad.

use super::gaps::apply_gaps;
use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use tracing::trace;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: MASTER_STACK_LAYOUT — Master area + stack area layout.
// To modify master behavior:
//   1. MasterStackConfig controls master_count, master_width_factor, orientation.
//   2. master_width_factor is in (0.1, 0.9) — clamped in the caller.
//   3. Remainder pixels are distributed one-per-slot via slot_extent() so
//      no single window absorbs the entire division remainder.
//   4. Orientation::Vertical puts master on top, stack on bottom.
// ═══════════════════════════════════════════════════════════════════════════════

/// Orientation of the master area relative to the stack.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Orientation {
    /// Master area is on the left, stack is on the right.
    Horizontal,
    /// Master area is on the top, stack is on the bottom.
    Vertical,
}

/// Configuration for the master-stack layout.
#[derive(Debug, Clone)]
pub struct MasterStackConfig {
    /// Number of windows in the master area.
    pub master_count: usize,
    /// Width (or height) of the master area as a fraction [0.1 - 0.9].
    pub master_width_factor: f64,
    /// Orientation of the split between master and stack.
    pub orientation: Orientation,
}

impl Default for MasterStackConfig {
    fn default() -> Self {
        MasterStackConfig {
            master_count: 1,
            master_width_factor: 0.5,
            orientation: Orientation::Horizontal,
        }
    }
}

/// The master-stack tiling layout.
pub struct MasterStackLayout;

impl MasterStackLayout {
    /// Return the layout name used in configuration.
    pub fn name() -> &'static str {
        "master_stack"
    }

    /// Calculate window positions for the master-stack layout.
    ///
    /// # Parameters
    /// - `windows`: slice of window identifiers to arrange.
    /// - `workspace_rect`: total area available for tiling.
    /// - `inner_gaps`: gap between adjacent windows.
    /// - `outer_gaps`: gap between windows and workspace edges.
    /// - `smart_gaps`: when `true`, disable gaps for single windows.
    /// - `config`: layout-specific configuration.
    ///
    /// # Returns
    /// A vector of `(WindowId, Rect)` pairs.
    pub fn calculate(
        windows: &[WindowId],
        workspace_rect: &Rect,
        inner_gaps: i32,
        outer_gaps: i32,
        _smart_gaps: bool,
        config: &MasterStackConfig,
    ) -> Vec<(WindowId, Rect)> {
        if windows.is_empty() {
            return Vec::new();
        }

        // Apply outer gaps to the workspace.
        let rect = apply_gaps(workspace_rect, outer_gaps);
        let gap = inner_gaps.max(0);

        let master_count = config.master_count.min(windows.len());
        let stack_count = windows.len() - master_count;

        let mut results = Vec::with_capacity(windows.len());

        match config.orientation {
            Orientation::Horizontal => {
                // Split workspace into master (left) and stack (right).
                // When there's no stack, master fills the entire workspace.
                let master_width = if stack_count == 0 {
                    rect.width
                } else {
                    ((rect.width as f64 * config.master_width_factor).round() as i32)
                        .max(1)
                        .min(rect.width.saturating_sub(1))
                };
                let stack_x = rect.x + master_width;
                let stack_width = rect.width - master_width;

                let master_rect = Rect::new(rect.x, rect.y, master_width, rect.height);
                let stack_rect = Rect::new(stack_x, rect.y, stack_width, rect.height);

                // Apply inner gaps to both regions.
                let master_region = apply_gaps(&master_rect, gap);
                let stack_region = apply_gaps(&stack_rect, gap);

                // Master windows: split vertically (top to bottom).
                if master_count > 0 {
                    for (i, &win) in windows.iter().enumerate().take(master_count) {
                        let (off, len) = slot_extent(master_region.height, master_count, i);
                        let win_rect = Rect::new(
                            master_region.x,
                            master_region.y + off,
                            master_region.width,
                            len,
                        );
                        results.push((win, apply_gaps(&win_rect, gap)));
                    }
                }

                // Stack windows: split vertically (top to bottom).
                if stack_count > 0 {
                    for i in 0..stack_count {
                        let si = i + master_count;
                        let (off, len) = slot_extent(stack_region.height, stack_count, i);
                        let win_rect = Rect::new(
                            stack_region.x,
                            stack_region.y + off,
                            stack_region.width,
                            len,
                        );
                        results.push((windows[si], apply_gaps(&win_rect, gap)));
                    }
                }
            }
            Orientation::Vertical => {
                // Split workspace into master (top) and stack (bottom).
                // When there's no stack, master fills the entire workspace.
                let master_height = if stack_count == 0 {
                    rect.height
                } else {
                    ((rect.height as f64 * config.master_width_factor).round() as i32)
                        .max(1)
                        .min(rect.height.saturating_sub(1))
                };
                let stack_y = rect.y + master_height;
                let stack_height = rect.height - master_height;

                let master_rect = Rect::new(rect.x, rect.y, rect.width, master_height);
                let stack_rect = Rect::new(rect.x, stack_y, rect.width, stack_height);

                // Apply inner gaps to both regions.
                let master_region = apply_gaps(&master_rect, gap);
                let stack_region = apply_gaps(&stack_rect, gap);

                // Master windows: split horizontally (left to right).
                if master_count > 0 {
                    for (i, &win) in windows.iter().enumerate().take(master_count) {
                        let (off, len) = slot_extent(master_region.width, master_count, i);
                        let win_rect = Rect::new(
                            master_region.x + off,
                            master_region.y,
                            len,
                            master_region.height,
                        );
                        results.push((win, apply_gaps(&win_rect, gap)));
                    }
                }

                // Stack windows: split horizontally (left to right).
                if stack_count > 0 {
                    for i in 0..stack_count {
                        let si = i + master_count;
                        let (off, len) = slot_extent(stack_region.width, stack_count, i);
                        let win_rect = Rect::new(
                            stack_region.x + off,
                            stack_region.y,
                            len,
                            stack_region.height,
                        );
                        results.push((windows[si], apply_gaps(&win_rect, gap)));
                    }
                }
            }
        }

        trace!(
            window_count = results.len(),
            master_count, stack_count, "master_stack layout calculated"
        );
        results
    }
}

/// Compute the (offset, length) of slot `index` when splitting `total`
/// pixels into `count` slots. Distributes the remainder one pixel at a
/// time across the first `total % count` slots, so layouts add up exactly
/// to `total` without giving the last slot an oversized share.
#[inline]
fn slot_extent(total: i32, count: usize, index: usize) -> (i32, i32) {
    debug_assert!(index < count);
    let count_i = count as i32;
    let base = total / count_i;
    let remainder = total - base * count_i;
    let extra_before = (index as i32).min(remainder);
    let extra_here = if (index as i32) < remainder { 1 } else { 0 };
    let offset = base * index as i32 + extra_before;
    let length = base + extra_here;
    (offset, length)
}

#[cfg(test)]
mod slot_tests {
    use super::slot_extent;

    #[test]
    fn slot_extent_partitions_exactly() {
        for total in [10, 11, 100, 101, 1023, 1080] {
            for count in 1usize..=8 {
                let mut sum = 0;
                let mut last_end = 0;
                for i in 0..count {
                    let (off, len) = slot_extent(total, count, i);
                    assert_eq!(off, last_end, "offsets must be contiguous");
                    assert!(len >= 1 || total == 0, "slot length must be positive");
                    sum += len;
                    last_end = off + len;
                }
                assert_eq!(sum, total, "slots must sum to total");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wid(n: isize) -> WindowId {
        WindowId(n)
    }

    #[test]
    fn test_master_stack_empty() {
        let workspace = Rect::new(0, 0, 1920, 1080);
        let config = MasterStackConfig::default();
        let result = MasterStackLayout::calculate(&[], &workspace, 8, 8, false, &config);
        assert!(result.is_empty());
    }

    #[test]
    fn test_master_stack_single_window() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1)];
        let config = MasterStackConfig::default();
        let result = MasterStackLayout::calculate(&windows, &workspace, 0, 0, false, &config);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, wid(1));
        // With 0 gaps, single master gets the full workspace
        assert_eq!(result[0].1, workspace);
    }

    #[test]
    fn test_master_stack_two_windows() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2)];
        let config = MasterStackConfig::default();
        let result = MasterStackLayout::calculate(&windows, &workspace, 0, 0, false, &config);
        assert_eq!(result.len(), 2);

        // Window 1 should be in the left half (master area, 50%)
        // Window 2 should be in the right half (stack area)
        assert_eq!(result[0].0, wid(1));
        assert_eq!(result[1].0, wid(2));

        // Both should have positive dimensions
        assert!(result[0].1.width > 0);
        assert!(result[0].1.height > 0);
        assert!(result[1].1.width > 0);
        assert!(result[1].1.height > 0);
    }

    #[test]
    fn test_master_stack_multiple_masters() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows: Vec<WindowId> = (1..=5).map(wid).collect();
        let config = MasterStackConfig {
            master_count: 2,
            master_width_factor: 0.5,
            orientation: Orientation::Horizontal,
        };
        let result = MasterStackLayout::calculate(&windows, &workspace, 0, 0, false, &config);
        assert_eq!(result.len(), 5);

        // First two are masters, rest are stack
        assert_eq!(result[0].0, wid(1));
        assert_eq!(result[1].0, wid(2));
        assert_eq!(result[2].0, wid(3));
    }

    #[test]
    fn test_master_stack_vertical_orientation() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2)];
        let config = MasterStackConfig {
            master_count: 1,
            master_width_factor: 0.5,
            orientation: Orientation::Vertical,
        };
        let result = MasterStackLayout::calculate(&windows, &workspace, 0, 0, false, &config);
        assert_eq!(result.len(), 2);

        // In vertical mode, master is on top, stack on bottom
        assert_eq!(result[0].0, wid(1));
        assert_eq!(result[1].0, wid(2));
    }

    #[test]
    fn test_master_stack_name() {
        assert_eq!(MasterStackLayout::name(), "master_stack");
    }

    #[test]
    fn test_master_stack_config_default() {
        let config = MasterStackConfig::default();
        assert_eq!(config.master_count, 1);
        assert!((config.master_width_factor - 0.5).abs() < f64::EPSILON);
        assert_eq!(config.orientation, Orientation::Horizontal);
    }

    #[test]
    fn test_master_stack_with_gaps() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2)];
        let config = MasterStackConfig::default();
        let result = MasterStackLayout::calculate(&windows, &workspace, 8, 8, false, &config);
        assert_eq!(result.len(), 2);

        // Both should have positive dimensions even with gaps
        assert!(result[0].1.width > 0);
        assert!(result[0].1.height > 0);
        assert!(result[1].1.width > 0);
        assert!(result[1].1.height > 0);
    }
}
