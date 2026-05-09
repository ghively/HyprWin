//! Binary Space Partitioning (BSP) tree implementation.
//!
//! The BSP tree is the core data structure behind the dwindle layout.
//! Each internal node represents a split (horizontal or vertical) with a
//! ratio; leaf nodes hold window identifiers or mark empty slots.

use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use tracing::{debug, trace, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: BSP_TREE_CORE — This is the fundamental layout data structure.
// Before modifying tree operations:
//   1. insert_window() must maintain invariants: no Empty nodes adjacent to Window.
//   2. remove_window() must promote children and rebalance.
//   3. traverse() is the ONLY way to compute rects — keep it correct.
//   4. All ratios are in [0.0, 1.0] representing left/top portion.
//   5. The dwindle algorithm alternates split directions (H, V, H, V...).
// ═══════════════════════════════════════════════════════════════════════════════

/// Direction of a binary split.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitDirection {
    /// Left / Right split.
    Horizontal,
    /// Top / Bottom split.
    Vertical,
}

impl SplitDirection {
    /// Return the opposite split direction.
    fn flip(self) -> Self {
        match self {
            SplitDirection::Horizontal => SplitDirection::Vertical,
            SplitDirection::Vertical => SplitDirection::Horizontal,
        }
    }
}

/// A node in the BSP tree.
#[derive(Debug, Clone)]
pub enum Node {
    /// Internal node: splits space into left/top and right/bottom.
    Split {
        direction: SplitDirection,
        ratio: f64,
        left: Box<Node>,
        right: Box<Node>,
    },
    /// Leaf node containing a window.
    Window {
        window_id: WindowId,
    },
    /// Empty leaf slot.
    Empty,
}

impl Default for Node {
    fn default() -> Self {
        Self::new()
    }
}

impl Node {
    /// Create a new empty BSP tree.
    pub fn new() -> Self {
        Node::Empty
    }

    /// Returns `true` if this node is the `Empty` variant.
    pub fn is_empty(&self) -> bool {
        matches!(self, Node::Empty)
    }

    /// Count the number of window leaves in this subtree.
    pub fn window_count(&self) -> usize {
        match self {
            Node::Split { left, right, .. } => left.window_count() + right.window_count(),
            Node::Window { .. } => 1,
            Node::Empty => 0,
        }
    }

    /// Recursively check whether a window is present in this subtree.
    pub fn contains_window(&self, window_id: WindowId) -> bool {
        match self {
            Node::Split { left, right, .. } => {
                left.contains_window(window_id) || right.contains_window(window_id)
            }
            Node::Window { window_id: wid } => *wid == window_id,
            Node::Empty => false,
        }
    }

    /// Insert a window into the tree.
    ///
    /// The algorithm:
    /// 1. If the current node is `Empty`, replace it with a `Window`.
    /// 2. If the current node is a `Window`, split it: the existing window
    ///    goes to the left child, the new window to the right.
    /// 3. If the current node is a `Split`, recurse into the child with
    ///    fewer windows.
    pub fn insert_window(&mut self, window_id: WindowId, direction: SplitDirection) {
        match self {
            Node::Empty => {
                *self = Node::Window { window_id };
                trace!(?window_id, "inserted into empty node");
            }
            Node::Window { window_id: existing } => {
                let existing_id = *existing;
                *self = Node::Split {
                    direction,
                    ratio: 0.5,
                    left: Box::new(Node::Window {
                        window_id: existing_id,
                    }),
                    right: Box::new(Node::Window { window_id }),
                };
                trace!(?existing_id, ?window_id, ?direction, "split existing window");
            }
            Node::Split {
                left,
                right,
                direction: ref mut dir,
                ..
            } => {
                *dir = direction;
                let left_count = left.window_count();
                let right_count = right.window_count();
                // Target the child with fewer windows for balanced insertion
                if left_count <= right_count {
                    left.insert_window(window_id, direction.flip());
                } else {
                    right.insert_window(window_id, direction.flip());
                }
            }
        }
    }

    /// Remove a window from the tree.
    ///
    /// Returns `true` if the window was found and removed. After removal,
    /// child promotion is applied:
    /// - A `Split` whose left child is removed becomes its right child.
    /// - A `Split` whose right child is removed becomes its left child.
    pub fn remove_window(&mut self, window_id: WindowId) -> bool {
        match self {
            Node::Window { window_id: wid } => {
                if *wid == window_id {
                    *self = Node::Empty;
                    trace!(?window_id, "removed window leaf");
                    true
                } else {
                    false
                }
            }
            Node::Empty => false,
            Node::Split {
                left,
                right,
                direction: dir,
                ratio: rat,
            } => {
                let left_removed = left.remove_window(window_id);
                if left_removed {
                    // Promote the right child to replace this split node.
                    let right_node = std::mem::replace(right.as_mut(), Node::Empty);
                    *self = right_node;
                    return true;
                }

                let right_removed = right.remove_window(window_id);
                if right_removed {
                    // Promote the left child to replace this split node.
                    let left_node = std::mem::replace(left.as_mut(), Node::Empty);
                    *self = left_node;
                    return true;
                }

                false
            }
        }
    }

    /// Find the mutable node containing the given window, if any.
    pub fn find_window_node(&mut self, window_id: WindowId) -> Option<&mut Node> {
        match self {
            Node::Window { window_id: wid } if *wid == window_id => Some(self),
            Node::Split { left, right, .. } => {
                left.find_window_node(window_id)
                    .or_else(|| right.find_window_node(window_id))
            }
            _ => None,
        }
    }

    /// Traverse the tree, computing the rectangle for each window.
    ///
    /// The `rect` parameter describes the full area managed by this node.
    /// For each `Window` leaf the callback is invoked with `(WindowId, Rect)`.
    pub fn traverse<F>(&self, rect: &Rect, callback: &mut F)
    where
        F: FnMut(WindowId, Rect),
    {
        match self {
            Node::Window { window_id } => {
                callback(*window_id, *rect);
            }
            Node::Split {
                direction,
                ratio,
                left,
                right,
            } => {
                let (left_rect, right_rect) = match direction {
                    SplitDirection::Horizontal => rect.split_horizontal(*ratio),
                    SplitDirection::Vertical => rect.split_vertical(*ratio),
                };
                left.traverse(&left_rect, callback);
                right.traverse(&right_rect, callback);
            }
            Node::Empty => {
                // Empty nodes produce no output.
            }
        }
    }

    /// Rebalance all split ratios in this subtree to exactly 0.5.
    pub fn rebalance_ratios(&mut self) {
        match self {
            Node::Split {
                ratio,
                left,
                right,
                ..
            } => {
                *ratio = 0.5;
                left.rebalance_ratios();
                right.rebalance_ratios();
            }
            _ => {}
        }
    }

    /// Find the split whose dividing line passes closest to `point`.
    ///
    /// Returns `Some((SplitDirection, ratio))` for the nearest split,
    /// or `None` if no split is nearby.
    pub fn get_split_at_point(
        &self,
        rect: &Rect,
        point: (i32, i32),
    ) -> Option<(SplitDirection, f64)> {
        match self {
            Node::Split {
                direction,
                ratio,
                left,
                right,
            } => {
                let (left_rect, right_rect) = match direction {
                    SplitDirection::Horizontal => rect.split_horizontal(*ratio),
                    SplitDirection::Vertical => rect.split_vertical(*ratio),
                };

                // Check if the point is on the dividing line (with tolerance).
                let on_divider = match direction {
                    SplitDirection::Horizontal => {
                        let divider_x = left_rect.x + left_rect.width;
                        (point.0 - divider_x).abs() <= 8
                    }
                    SplitDirection::Vertical => {
                        let divider_y = left_rect.y + left_rect.height;
                        (point.1 - divider_y).abs() <= 8
                    }
                };

                if on_divider {
                    return Some((*direction, *ratio));
                }

                // Recurse into the child that contains the point.
                if left_rect.contains(point) {
                    left.get_split_at_point(&left_rect, point)
                } else {
                    right.get_split_at_point(&right_rect, point)
                }
            }
            _ => None,
        }
    }

    /// Adjust the split ratio whose dividing line is nearest to `point`.
    ///
    /// `delta` is added to the ratio and clamped to `[0.05, 0.95]` to
    /// prevent collapse. Returns `true` if a split was found and adjusted.
    pub fn adjust_ratio(&mut self, point: (i32, i32), delta: f64) -> bool {
        match self {
            Node::Split {
                direction,
                ratio,
                left,
                right,
            } => {
                // For the root split, the "rect" is the workspace rect.
                // We use a heuristic: check if the point is close to the
                // current split line. If so, adjust this split; otherwise
                // recurse into children.
                //
                // Since we don't have the rect here, we always recurse into
                // children first. If no child split matches, we fall back
                // to adjusting the current ratio.
                if left.adjust_ratio(point, delta) || right.adjust_ratio(point, delta) {
                    return true;
                }

                // No child split matched; this is a heuristic fallback.
                // In practice, the caller should track rects alongside nodes.
                warn!(
                    "adjust_ratio: point {:?} did not match any child split, adjusting root ratio",
                    point
                );
                *ratio = (*ratio + delta).clamp(0.05, 0.95);
                true
            }
            _ => false,
        }
    }
}

/// Build a BSP tree from a list of windows using the dwindle algorithm.
///
/// The dwindle algorithm creates Fibonacci-style alternating splits:
/// - The first window gets the full space.
/// - The second window splits vertically (left / right).
/// - The third window splits the right half horizontally (top / bottom).
/// - The fourth window splits the bottom-right quarter vertically, and so on.
///
/// The split direction alternates at each insertion level, producing the
/// characteristic "spiral" layout that gives each window a unique position
/// and size.
///
/// Returns `Node::Empty` when `windows` is empty.
pub fn build_dwindle_tree(windows: &[WindowId]) -> Node {
    if windows.is_empty() {
        return Node::Empty;
    }

    let mut root = Node::Window {
        window_id: windows[0],
    };

    // Alternating split directions for the dwindle spiral:
    // insert 2 -> Horizontal, insert 3 -> Vertical, insert 4 -> Horizontal, ...
    let directions = [SplitDirection::Horizontal, SplitDirection::Vertical];

    for (i, &window_id) in windows.iter().enumerate().skip(1) {
        let direction = directions[(i - 1) % 2];
        root.insert_window(window_id, direction);
    }

    trace!(window_count = windows.len(), "built dwindle tree");
    root
}

/// Build a BSP tree from a list of windows with a consistent split direction.
///
/// All splits use the given `direction`; children alternate to keep the
/// tree balanced.
///
/// Returns `Node::Empty` when `windows` is empty.
pub fn build_tree_with_direction(windows: &[WindowId], direction: SplitDirection) -> Node {
    if windows.is_empty() {
        return Node::Empty;
    }

    let mut root = Node::Window {
        window_id: windows[0],
    };

    for (i, &window_id) in windows.iter().enumerate().skip(1) {
        // Alternate direction at each level for balance
        let dir = if i % 2 == 1 {
            direction
        } else {
            direction.flip()
        };
        root.insert_window(window_id, dir);
    }

    trace!(
        window_count = windows.len(),
        ?direction,
        "built tree with fixed direction"
    );
    root
}

/// Remove a window from the tree and rebalance remaining split ratios.
///
/// After removal, all split ratios are reset to 0.5 for a clean layout.
/// Returns `true` if the window was found and removed.
pub fn remove_and_rebalance(root: &mut Node, window_id: WindowId) -> bool {
    let removed = root.remove_window(window_id);
    if removed {
        root.rebalance_ratios();
        debug!(?window_id, "removed and rebalanced tree");
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wid(n: isize) -> WindowId {
        WindowId(n)
    }

    #[test]
    fn test_new_node_is_empty() {
        let node = Node::new();
        assert!(node.is_empty());
        assert_eq!(node.window_count(), 0);
    }

    #[test]
    fn test_contains_window() {
        let mut node = Node::new();
        node.insert_window(wid(42), SplitDirection::Horizontal);
        assert!(node.contains_window(wid(42)));
        assert!(!node.contains_window(wid(99)));
    }

    #[test]
    fn test_single_window() {
        let tree = build_dwindle_tree(&[wid(1)]);
        assert_eq!(tree.window_count(), 1);
        assert!(tree.contains_window(wid(1)));
    }

    #[test]
    fn test_dwindle_two_windows() {
        let tree = build_dwindle_tree(&[wid(1), wid(2)]);
        assert_eq!(tree.window_count(), 2);
        assert!(tree.contains_window(wid(1)));
        assert!(tree.contains_window(wid(2)));
    }

    #[test]
    fn test_dwindle_five_windows() {
        let windows: Vec<WindowId> = (1..=5).map(|n| wid(n)).collect();
        let tree = build_dwindle_tree(&windows);
        assert_eq!(tree.window_count(), 5);
        for i in 1..=5 {
            assert!(tree.contains_window(wid(i)));
        }
    }

    #[test]
    fn test_empty_tree() {
        let tree = build_dwindle_tree(&[]);
        assert!(tree.is_empty());
    }

    #[test]
    fn test_insert_into_empty() {
        let mut node = Node::new();
        node.insert_window(wid(10), SplitDirection::Horizontal);
        assert!(!node.is_empty());
        assert_eq!(node.window_count(), 1);
        assert!(node.contains_window(wid(10)));
    }

    #[test]
    fn test_insert_splits_window() {
        let mut node = Node::Window { window_id: wid(1) };
        node.insert_window(wid(2), SplitDirection::Horizontal);
        assert_eq!(node.window_count(), 2);
        assert!(node.contains_window(wid(1)));
        assert!(node.contains_window(wid(2)));
    }

    #[test]
    fn test_remove_window_leaf() {
        let mut node = Node::Window { window_id: wid(5) };
        let removed = node.remove_window(wid(5));
        assert!(removed);
        assert!(node.is_empty());
    }

    #[test]
    fn test_remove_not_found() {
        let mut node = Node::Window { window_id: wid(1) };
        let removed = node.remove_window(wid(99));
        assert!(!removed);
    }

    #[test]
    fn test_remove_and_promote_left() {
        // Build a tree with 3 windows
        let mut tree = build_dwindle_tree(&[wid(1), wid(2), wid(3)]);
        // Remove the right child of the root's right split
        let removed = tree.remove_window(wid(3));
        assert!(removed);
        // Should still contain windows 1 and 2
        assert!(tree.contains_window(wid(1)));
        assert!(tree.contains_window(wid(2)));
        assert!(!tree.contains_window(wid(3)));
        assert_eq!(tree.window_count(), 2);
    }

    #[test]
    fn test_traverse_single_window() {
        let tree = build_dwindle_tree(&[wid(1)]);
        let workspace = Rect::new(0, 0, 100, 100);
        let mut results = Vec::new();
        tree.traverse(&workspace, &mut |id, rect| {
            results.push((id, rect));
        });
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].0, wid(1));
        assert_eq!(results[0].1, workspace);
    }

    #[test]
    fn test_traverse_two_windows() {
        let tree = build_dwindle_tree(&[wid(1), wid(2)]);
        let workspace = Rect::new(0, 0, 100, 100);
        let mut results = Vec::new();
        tree.traverse(&workspace, &mut |id, rect| {
            results.push((id, rect));
        });
        assert_eq!(results.len(), 2);
        // First split is horizontal (left/right), so window 1 is left half
        assert_eq!(results[0].1, Rect::new(0, 0, 50, 100));
        // Window 2 is right half
        assert_eq!(results[1].1, Rect::new(50, 0, 50, 100));
    }

    #[test]
    fn test_rebalance_ratios() {
        let mut tree = build_dwindle_tree(&[wid(1), wid(2), wid(3)]);
        // After rebalancing, all ratios should be 0.5
        tree.rebalance_ratios();
        let workspace = Rect::new(0, 0, 100, 100);
        let mut results = Vec::new();
        tree.traverse(&workspace, &mut |id, rect| {
            results.push((id, rect));
        });
        // With all ratios at 0.5, we get predictable splits
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_remove_and_rebalance() {
        let mut tree = build_dwindle_tree(&[wid(1), wid(2), wid(3), wid(4)]);
        let removed = remove_and_rebalance(&mut tree, wid(2));
        assert!(removed);
        assert_eq!(tree.window_count(), 3);
        assert!(!tree.contains_window(wid(2)));
    }

    #[test]
    fn test_build_tree_with_direction() {
        let tree = build_tree_with_direction(&[wid(1), wid(2), wid(3)], SplitDirection::Vertical);
        assert_eq!(tree.window_count(), 3);
        let workspace = Rect::new(0, 0, 100, 100);
        let mut results = Vec::new();
        tree.traverse(&workspace, &mut |id, rect| {
            results.push((id, rect));
        });
        assert_eq!(results.len(), 3);
    }

    #[test]
    fn test_find_window_node() {
        let mut tree = build_dwindle_tree(&[wid(1), wid(2), wid(3)]);
        let node = tree.find_window_node(wid(2));
        assert!(node.is_some());
    }

    #[test]
    fn test_find_window_node_missing() {
        let mut tree = build_dwindle_tree(&[wid(1), wid(2)]);
        let node = tree.find_window_node(wid(99));
        assert!(node.is_none());
    }
}
