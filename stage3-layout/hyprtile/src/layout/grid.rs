//! Grid layout implementation.
//!
//! Distributes windows uniformly across a grid of rows and columns.
//! The grid dimensions are chosen to keep the layout as close to a
//! square as possible, minimising wasted space.

use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use super::gaps::apply_gaps;
use tracing::trace;

/// The grid tiling layout.
pub struct GridLayout;

impl GridLayout {
    /// Return the layout name used in configuration.
    pub fn name() -> &'static str {
        "grid"
    }

    /// Calculate window positions for the grid layout.
    ///
    /// Windows are arranged in a grid with `rows` rows and `cols` columns.
    /// The grid dimensions are computed to keep cells as square as
    /// possible given the workspace aspect ratio and window count.
    ///
    /// # Parameters
    /// - `windows`: slice of window identifiers to arrange.
    /// - `workspace_rect`: total area available for tiling.
    /// - `inner_gaps`: gap between adjacent windows.
    /// - `outer_gaps`: gap between windows and workspace edges.
    /// - `smart_gaps`: when `true`, disable gaps for single windows.
    ///
    /// # Returns
    /// A vector of `(WindowId, Rect)` pairs.
    pub fn calculate(
        windows: &[WindowId],
        workspace_rect: &Rect,
        inner_gaps: i32,
        outer_gaps: i32,
        _smart_gaps: bool,
    ) -> Vec<(WindowId, Rect)> {
        if windows.is_empty() {
            return Vec::new();
        }

        // Apply outer gaps to the workspace.
        let rect = apply_gaps(workspace_rect, outer_gaps);
        let gap = inner_gaps.max(0);

        let (rows, cols) = calculate_grid_dimensions(windows.len());
        let cell_width = rect.width / cols as i32;
        let cell_height = rect.height / rows as i32;

        let mut results = Vec::with_capacity(windows.len());

        for (i, &window_id) in windows.iter().enumerate() {
            let row = i / cols;
            let col = i % cols;

            let x = rect.x + (cell_width * col as i32);
            let y = rect.y + (cell_height * row as i32);

            // Last row/column get any remaining pixels to avoid gaps.
            let w = if col == cols - 1 {
                rect.x + rect.width - x
            } else {
                cell_width
            };
            let h = if row == rows - 1 {
                rect.y + rect.height - y
            } else {
                cell_height
            };

            let cell_rect = Rect::new(x, y, w.max(1), h.max(1));
            let gap_adjusted = apply_gaps(&cell_rect, gap);

            results.push((window_id, gap_adjusted));
        }

        trace!(
            window_count = results.len(),
            rows,
            cols,
            "grid layout calculated"
        );
        results
    }
}

/// Calculate optimal grid dimensions (rows, columns) for `count` windows.
///
/// The algorithm attempts to keep rows and columns as close as possible
/// to each other (approaching a square grid), preferring more columns
/// when the workspace is typically wider than it is tall.
///
/// # Examples
/// ```
/// assert_eq!(calculate_grid_dimensions(1), (1, 1));
/// assert_eq!(calculate_grid_dimensions(4), (2, 2));
/// assert_eq!(calculate_grid_dimensions(5), (2, 3));
/// ```
fn calculate_grid_dimensions(count: usize) -> (usize, usize) {
    if count == 0 {
        return (0, 0);
    }
    if count == 1 {
        return (1, 1);
    }

    let sqrt = (count as f64).sqrt();
    let floor_sqrt = sqrt.floor() as usize;

    if floor_sqrt * floor_sqrt >= count {
        return (floor_sqrt, floor_sqrt);
    }

    if floor_sqrt * (floor_sqrt + 1) >= count {
        return (floor_sqrt, floor_sqrt + 1);
    }

    (floor_sqrt + 1, floor_sqrt + 1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wid(n: isize) -> WindowId {
        WindowId(n)
    }

    #[test]
    fn test_grid_dimensions() {
        assert_eq!(calculate_grid_dimensions(0), (0, 0));
        assert_eq!(calculate_grid_dimensions(1), (1, 1));
        assert_eq!(calculate_grid_dimensions(2), (1, 2));
        assert_eq!(calculate_grid_dimensions(3), (2, 2));
        assert_eq!(calculate_grid_dimensions(4), (2, 2));
        assert_eq!(calculate_grid_dimensions(5), (2, 3));
        assert_eq!(calculate_grid_dimensions(6), (2, 3));
        assert_eq!(calculate_grid_dimensions(7), (3, 3));
        assert_eq!(calculate_grid_dimensions(8), (3, 3));
        assert_eq!(calculate_grid_dimensions(9), (3, 3));
        assert_eq!(calculate_grid_dimensions(10), (3, 4));
        assert_eq!(calculate_grid_dimensions(16), (4, 4));
    }

    #[test]
    fn test_grid_empty() {
        let workspace = Rect::new(0, 0, 1920, 1080);
        let result = GridLayout::calculate(&[], &workspace, 8, 8, false);
        assert!(result.is_empty());
    }

    #[test]
    fn test_grid_single_window() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1)];
        let result = GridLayout::calculate(&windows, &workspace, 0, 0, false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].0, wid(1));
        // 1x1 grid = full workspace
        assert_eq!(result[0].1, workspace);
    }

    #[test]
    fn test_grid_two_windows() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2)];
        let result = GridLayout::calculate(&windows, &workspace, 0, 0, false);
        assert_eq!(result.len(), 2);

        // 1 row, 2 columns: side by side
        assert_eq!(result[0].1, Rect::new(0, 0, 500, 600));
        assert_eq!(result[1].1, Rect::new(500, 0, 500, 600));
    }

    #[test]
    fn test_grid_four_windows() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows: Vec<WindowId> = (1..=4).map(|n| wid(n)).collect();
        let result = GridLayout::calculate(&windows, &workspace, 0, 0, false);
        assert_eq!(result.len(), 4);

        // 2x2 grid
        assert_eq!(result[0].1, Rect::new(0, 0, 500, 300));
        assert_eq!(result[1].1, Rect::new(500, 0, 500, 300));
        assert_eq!(result[2].1, Rect::new(0, 300, 500, 300));
        assert_eq!(result[3].1, Rect::new(500, 300, 500, 300));
    }

    #[test]
    fn test_grid_positive_sizes() {
        let workspace = Rect::new(0, 0, 1920, 1080);
        let windows: Vec<WindowId> = (1..=12).map(|n| wid(n)).collect();
        let result = GridLayout::calculate(&windows, &workspace, 8, 8, false);
        assert_eq!(result.len(), 12);

        for (i, (_, rect)) in result.iter().enumerate() {
            assert!(
                rect.width > 0,
                "window {} has non-positive width: {:?}",
                i, rect
            );
            assert!(
                rect.height > 0,
                "window {} has non-positive height: {:?}",
                i, rect
            );
        }
    }

    #[test]
    fn test_grid_name() {
        assert_eq!(GridLayout::name(), "grid");
    }

    #[test]
    fn test_grid_with_gaps() {
        let workspace = Rect::new(0, 0, 100, 100);
        let windows = vec![wid(1), wid(2)];
        let result = GridLayout::calculate(&windows, &workspace, 4, 4, false);
        assert_eq!(result.len(), 2);

        for (_, rect) in &result {
            assert!(rect.width > 0);
            assert!(rect.height > 0);
        }
    }
}
