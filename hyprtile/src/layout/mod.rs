//! Layout coordinator for HyprTile.
//!
//! This module dispatches layout calculations to the appropriate algorithm
//! (dwindle, master-stack, monocle, grid) and provides the [`LayoutEngine`]
//! which tracks the currently active layout type.

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: FILE_GUIDANCE — mod.rs in layout/
// Before modifying this file:
//   1. Read the module's mod.rs to understand public API.
//   2. Check DEVELOPERS_GUIDE.md for architecture context.
//   3. Run tests after changes: cargo test
// ═══════════════════════════════════════════════════════════════════════════════

pub mod bsp;
pub mod dwindle;
pub mod gaps;
pub mod grid;
pub mod master_stack;
pub mod monocle;

use crate::config::types::GapsConfig;
use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use std::fmt;
use tracing::debug;

/// Available tiling layout algorithms.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutType {
    /// Fibonacci-spiral BSP layout (default).
    Dwindle,
    /// Master area + stack layout.
    MasterStack,
    /// Single fullscreen window at a time.
    Monocle,
    /// Uniform grid of rows and columns.
    Grid,
}

impl LayoutType {
    /// Return all supported layout types in their default order.
    pub fn all() -> Vec<LayoutType> {
        vec![
            LayoutType::Dwindle,
            LayoutType::MasterStack,
            LayoutType::Monocle,
            LayoutType::Grid,
        ]
    }

    /// Return the canonical name for this layout type.
    pub fn name(&self) -> &'static str {
        match self {
            LayoutType::Dwindle => "dwindle",
            LayoutType::MasterStack => "master_stack",
            LayoutType::Monocle => "monocle",
            LayoutType::Grid => "grid",
        }
    }

    /// Parse a layout type from its canonical name.
    ///
    /// Returns `None` if the name does not match any known layout.
    pub fn from_name(name: &str) -> Option<LayoutType> {
        match name {
            "dwindle" => Some(LayoutType::Dwindle),
            "master_stack" => Some(LayoutType::MasterStack),
            "monocle" => Some(LayoutType::Monocle),
            "grid" => Some(LayoutType::Grid),
            _ => None,
        }
    }

    /// Return the next layout type in the cycle.
    ///
    /// The cycle order is: Dwindle → MasterStack → Monocle → Grid → Dwindle.
    pub fn next(&self) -> LayoutType {
        match self {
            LayoutType::Dwindle => LayoutType::MasterStack,
            LayoutType::MasterStack => LayoutType::Monocle,
            LayoutType::Monocle => LayoutType::Grid,
            LayoutType::Grid => LayoutType::Dwindle,
        }
    }
}

impl fmt::Display for LayoutType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.name())
    }
}

/// The result of a layout calculation: a list of target rectangles
/// indexed by window identifier.
pub type LayoutResult = Vec<(WindowId, Rect)>;

/// Calculate a layout for the given parameters.
///
/// This is the main dispatcher that routes to the appropriate layout
/// algorithm based on `layout`.
///
/// # Parameters
/// - `layout`: the layout algorithm to use.
/// - `windows`: the ordered list of windows to arrange.
/// - `workspace_rect`: the total available area.
/// - `gaps`: gap configuration (inner, outer, smart).
/// - `focused_idx`: index of the focused window within `windows`; used
///   by the monocle layout to determine which window is on top.
/// - `master_width_factor`: adjustable ratio for master-stack layouts.
///
/// # Returns
/// A [`LayoutResult`] containing the target position for each window.
pub fn calculate_layout(
    layout: LayoutType,
    windows: &[WindowId],
    workspace_rect: &Rect,
    gaps: &GapsConfig,
    focused_idx: usize,
    master_width_factor: f64,
) -> LayoutResult {
    let inner = gaps.inner as i32;
    let outer = gaps.outer as i32;

    debug!(?layout, window_count = windows.len(), "calculating layout");

    match layout {
        LayoutType::Dwindle => {
            dwindle::DwindleLayout::calculate(windows, workspace_rect, inner, outer, gaps.smart)
        }
        LayoutType::MasterStack => {
            let config = master_stack::MasterStackConfig {
                master_width_factor: master_width_factor.clamp(0.1, 0.9),
                ..master_stack::MasterStackConfig::default()
            };
            master_stack::MasterStackLayout::calculate(
                windows,
                workspace_rect,
                inner,
                outer,
                gaps.smart,
                &config,
            )
        }
        LayoutType::Monocle => monocle::MonocleLayout::calculate(
            windows,
            workspace_rect,
            inner,
            outer,
            gaps.smart,
            focused_idx,
        ),
        LayoutType::Grid => {
            grid::GridLayout::calculate(windows, workspace_rect, inner, outer, gaps.smart)
        }
    }
}

/// Layout engine that tracks and cycles the active layout type.
///
/// The engine defaults to [`LayoutType::Dwindle`] and provides
/// convenience methods to query and change the current layout.
#[derive(Debug, Clone)]
pub struct LayoutEngine {
    current_layout: LayoutType,
}

impl LayoutEngine {
    /// Create a new layout engine with the default layout (Dwindle).
    pub fn new() -> Self {
        LayoutEngine {
            current_layout: LayoutType::Dwindle,
        }
    }

    /// Return the currently active layout type.
    pub fn current(&self) -> LayoutType {
        self.current_layout
    }

    /// Cycle to the next layout and return the new layout type.
    ///
    /// The cycle order is: Dwindle → MasterStack → Monocle → Grid → Dwindle.
    pub fn cycle(&mut self) -> LayoutType {
        self.current_layout = self.current_layout.next();
        debug!(layout = ?self.current_layout, "cycled layout");
        self.current_layout
    }

    /// Set the active layout to a specific type.
    pub fn set_layout(&mut self, layout: LayoutType) {
        debug!(?layout, "set layout");
        self.current_layout = layout;
    }

    /// Calculate the layout using the currently active layout type.
    ///
    /// # Parameters
    /// - `windows`: the ordered list of windows to arrange.
    /// - `workspace_rect`: the total available area.
    /// - `gaps`: gap configuration.
    /// - `master_width_factor`: adjustable ratio for master-stack layouts.
    ///
    /// # Returns
    /// A [`LayoutResult`] from the active layout algorithm.
    pub fn calculate(
        &self,
        windows: &[WindowId],
        workspace_rect: &Rect,
        gaps: &GapsConfig,
        master_width_factor: f64,
    ) -> LayoutResult {
        // For the layout engine, we assume the first window is focused
        // when needed (e.g., by monocle). Callers that need precise
        // focus control can use `calculate_layout` directly.
        calculate_layout(
            self.current_layout,
            windows,
            workspace_rect,
            gaps,
            0, // Default focused index: first window
            master_width_factor,
        )
    }
}

impl Default for LayoutEngine {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wid(n: isize) -> WindowId {
        WindowId(n)
    }

    // Helper: create a GapsConfig for testing
    fn test_gaps() -> GapsConfig {
        GapsConfig {
            inner: 0,
            outer: 0,
            smart: false,
        }
    }

    #[test]
    fn test_layout_type_all() {
        let all = LayoutType::all();
        assert_eq!(all.len(), 4);
        assert_eq!(all[0], LayoutType::Dwindle);
        assert_eq!(all[1], LayoutType::MasterStack);
        assert_eq!(all[2], LayoutType::Monocle);
        assert_eq!(all[3], LayoutType::Grid);
    }

    #[test]
    fn test_layout_type_name() {
        assert_eq!(LayoutType::Dwindle.name(), "dwindle");
        assert_eq!(LayoutType::MasterStack.name(), "master_stack");
        assert_eq!(LayoutType::Monocle.name(), "monocle");
        assert_eq!(LayoutType::Grid.name(), "grid");
    }

    #[test]
    fn test_layout_type_from_name() {
        assert_eq!(LayoutType::from_name("dwindle"), Some(LayoutType::Dwindle));
        assert_eq!(
            LayoutType::from_name("master_stack"),
            Some(LayoutType::MasterStack)
        );
        assert_eq!(LayoutType::from_name("monocle"), Some(LayoutType::Monocle));
        assert_eq!(LayoutType::from_name("grid"), Some(LayoutType::Grid));
        assert_eq!(LayoutType::from_name("unknown"), None);
    }

    #[test]
    fn test_layout_type_next() {
        assert_eq!(LayoutType::Dwindle.next(), LayoutType::MasterStack);
        assert_eq!(LayoutType::MasterStack.next(), LayoutType::Monocle);
        assert_eq!(LayoutType::Monocle.next(), LayoutType::Grid);
        assert_eq!(LayoutType::Grid.next(), LayoutType::Dwindle);
    }

    #[test]
    fn test_layout_type_display() {
        assert_eq!(format!("{}", LayoutType::Dwindle), "dwindle");
        assert_eq!(format!("{}", LayoutType::Grid), "grid");
    }

    #[test]
    fn test_calculate_layout_dwindle() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2), wid(3)];
        let gaps = test_gaps();
        let result = calculate_layout(LayoutType::Dwindle, &windows, &workspace, &gaps, 0, 0.5);
        assert_eq!(result.len(), 3);
        for (_, rect) in &result {
            assert!(rect.width > 0);
            assert!(rect.height > 0);
        }
    }

    #[test]
    fn test_calculate_layout_master_stack() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2)];
        let gaps = test_gaps();
        let result = calculate_layout(LayoutType::MasterStack, &windows, &workspace, &gaps, 0, 0.5);
        assert_eq!(result.len(), 2);
        for (_, rect) in &result {
            assert!(rect.width > 0);
            assert!(rect.height > 0);
        }
    }

    #[test]
    fn test_calculate_layout_monocle() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2), wid(3)];
        let gaps = test_gaps();
        let result = calculate_layout(LayoutType::Monocle, &windows, &workspace, &gaps, 1, 0.5);
        assert_eq!(result.len(), 3);
        // All windows in monocle should have the same rect
        let first_rect = result[0].1;
        for (_, rect) in &result {
            assert_eq!(*rect, first_rect);
        }
    }

    #[test]
    fn test_calculate_layout_grid() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows: Vec<WindowId> = (1..=4).map(|n| wid(n)).collect();
        let gaps = test_gaps();
        let result = calculate_layout(LayoutType::Grid, &windows, &workspace, &gaps, 0, 0.5);
        assert_eq!(result.len(), 4);
        for (_, rect) in &result {
            assert!(rect.width > 0);
            assert!(rect.height > 0);
        }
    }

    #[test]
    fn test_calculate_layout_empty() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let gaps = test_gaps();
        for layout in LayoutType::all() {
            let result = calculate_layout(layout, &[], &workspace, &gaps, 0, 0.5);
            assert!(
                result.is_empty(),
                "layout {:?} should return empty for no windows",
                layout
            );
        }
    }

    #[test]
    fn test_layout_engine_new() {
        let engine = LayoutEngine::new();
        assert_eq!(engine.current(), LayoutType::Dwindle);
    }

    #[test]
    fn test_layout_engine_default() {
        let engine: LayoutEngine = Default::default();
        assert_eq!(engine.current(), LayoutType::Dwindle);
    }

    #[test]
    fn test_layout_engine_cycle() {
        let mut engine = LayoutEngine::new();
        assert_eq!(engine.current(), LayoutType::Dwindle);

        assert_eq!(engine.cycle(), LayoutType::MasterStack);
        assert_eq!(engine.cycle(), LayoutType::Monocle);
        assert_eq!(engine.cycle(), LayoutType::Grid);
        assert_eq!(engine.cycle(), LayoutType::Dwindle);
    }

    #[test]
    fn test_layout_engine_set_layout() {
        let mut engine = LayoutEngine::new();
        engine.set_layout(LayoutType::Grid);
        assert_eq!(engine.current(), LayoutType::Grid);
    }

    #[test]
    fn test_layout_engine_calculate() {
        let engine = LayoutEngine::new();
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2)];
        let gaps = test_gaps();
        let result = engine.calculate(&windows, &workspace, &gaps, 0.5);
        assert_eq!(result.len(), 2);
        for (_, rect) in &result {
            assert!(rect.width > 0);
            assert!(rect.height > 0);
        }
    }

    #[test]
    fn test_layout_engine_calculate_empty() {
        let engine = LayoutEngine::new();
        let workspace = Rect::new(0, 0, 1000, 600);
        let gaps = test_gaps();
        let result = engine.calculate(&[], &workspace, &gaps, 0.5);
        assert!(result.is_empty());
    }
}
