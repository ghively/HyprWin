//! Workspace model: data structures for virtual desktops and per-monitor
//! workspace collections.
//!
//! A [`Workspace`] is a named virtual desktop that owns a list of
//! [`WindowId`]s and a [`LayoutEngine`].  A [`MonitorWorkspace`] bundles
//! multiple workspaces together for a single physical monitor.

use crate::layout::LayoutEngine;
use crate::platform::window::WindowId;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: WORKSPACE_MODEL — Per-monitor workspace container.
// Before modifying workspace behavior:
//   1. Workspaces are created on demand (lazy initialization).
//   2. Each workspace owns a LayoutEngine (independent layout choice).
//   3. focus_window() and cycle_focus() manage focus within a workspace.
//   4. MonitorWorkspace.ensure_workspace() creates if missing.
//   5. Never use expect() — always have a fallback (see get_active_workspace).
// ═══════════════════════════════════════════════════════════════════════════════

/// Directional focus movement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusDirection {
    /// Move focus to the window conceptually to the left.
    Left,
    /// Move focus to the window conceptually to the right.
    Right,
    /// Move focus to the window conceptually above.
    Up,
    /// Move focus to the window conceptually below.
    Down,
    /// Move focus to the next window in the list (wrapping).
    Next,
    /// Move focus to the previous window in the list (wrapping).
    Previous,
}

/// A virtual desktop (workspace) that groups windows.
///
/// Each workspace maintains its own window list, focused-window pointer,
/// and layout engine so that switching workspaces is cheap and preserves
/// layout state.
#[derive(Debug, Clone)]
pub struct Workspace {
    /// Numeric workspace ID (1-based in user-facing UI).
    pub id: u32,
    /// Human-readable name, e.g. "1", "2", "mail", "dev".
    pub name: String,
    /// Windows currently assigned to this workspace, in focus order.
    pub windows: Vec<WindowId>,
    /// Layout engine with the user's chosen layout for this workspace.
    pub layout_engine: LayoutEngine,
    /// Which window inside this workspace has keyboard focus.
    pub focused_window: Option<WindowId>,
    /// Adjustable split ratio for master-stack layouts (0.1 - 0.9).
    pub master_width_factor: f64,
    /// Adjustable split ratio for dwindle/BSP layouts (0.1 - 0.9).
    pub dwindle_ratio: f64,
}

impl Workspace {
    /// Create an empty workspace with the given ID.
    ///
    /// The name defaults to the stringified ID.
    pub fn new(id: u32) -> Self {
        Self {
            id,
            name: id.to_string(),
            windows: Vec::new(),
            layout_engine: LayoutEngine::new(),
            focused_window: None,
            master_width_factor: 0.5,
            dwindle_ratio: 0.5,
        }
    }

    /// Return `true` if the workspace has no windows.
    pub fn is_empty(&self) -> bool {
        self.windows.is_empty()
    }

    /// Add a window to this workspace if it is not already present.
    ///
    /// Returns `true` if the window was newly added.
    pub fn add_window(&mut self, window: WindowId) -> bool {
        if self.windows.contains(&window) {
            false
        } else {
            self.windows.push(window);
            // If this is the first window, give it focus automatically.
            if self.focused_window.is_none() {
                self.focused_window = Some(window);
            }
            true
        }
    }

    /// Remove a window from this workspace.
    ///
    /// If the removed window had focus, focus moves to the next window
    /// in the list (wrapping if necessary).  Returns `true` if the window
    /// was present and removed.
    pub fn remove_window(&mut self, window: WindowId) -> bool {
        let idx = match self.windows.iter().position(|&w| w == window) {
            Some(i) => i,
            None => return false,
        };

        self.windows.remove(idx);

        if self.focused_window == Some(window) {
            // Move focus to the window that now occupies the same slot,
            // or the last window if we removed the last one.
            self.focused_window = if self.windows.is_empty() {
                None
            } else {
                let new_idx = idx.min(self.windows.len().saturating_sub(1));
                Some(self.windows[new_idx])
            };
        }

        true
    }

    /// Return `true` if the window is in this workspace.
    pub fn contains(&self, window: WindowId) -> bool {
        self.windows.contains(&window)
    }

    /// Set focus to a specific window.
    ///
    /// Returns `true` only if the window is present in this workspace.
    pub fn focus_window(&mut self, window: WindowId) -> bool {
        if self.windows.contains(&window) {
            self.focused_window = Some(window);
            true
        } else {
            false
        }
    }

    /// Return the index of the currently focused window, or `0` if none.
    pub fn get_focused_index(&self) -> usize {
        match self.focused_window {
            Some(w) => self.windows.iter().position(|&x| x == w).unwrap_or(0),
            None => 0,
        }
    }

    /// Cycle focus in the given direction.
    ///
    /// * `Left` / `Up` / `Previous` move to the previous window.
    /// * `Right` / `Down` / `Next` move to the next window.
    ///
    /// Wrapping is applied at both ends of the window list.
    pub fn cycle_focus(&mut self, direction: FocusDirection) {
        if self.windows.len() <= 1 {
            return;
        }

        let current_idx = self.get_focused_index();
        let new_idx = match direction {
            FocusDirection::Left
            | FocusDirection::Up
            | FocusDirection::Previous => {
                if current_idx == 0 {
                    self.windows.len() - 1
                } else {
                    current_idx - 1
                }
            }
            FocusDirection::Right
            | FocusDirection::Down
            | FocusDirection::Next => {
                if current_idx + 1 >= self.windows.len() {
                    0
                } else {
                    current_idx + 1
                }
            }
        };

        self.focused_window = Some(self.windows[new_idx]);
    }

    /// Return the index of a window in this workspace, if present.
    pub fn get_window_index(&self, window: WindowId) -> Option<usize> {
        self.windows.iter().position(|&w| w == window)
    }

    /// Move a window from one position to another within this workspace.
    ///
    /// No-op if either `from` or `to` is out of bounds or the same.
    pub fn move_focus(&mut self, from: WindowId, to: WindowId) {
        if from == to {
            return;
        }
        let from_idx = match self.get_window_index(from) {
            Some(i) => i,
            None => return,
        };
        let to_idx = match self.get_window_index(to) {
            Some(i) => i,
            None => return,
        };

        let window = self.windows.remove(from_idx);
        // Adjust target index if removal shifted it.
        let insert_idx = if from_idx < to_idx {
            to_idx
        } else {
            to_idx
        };
        self.windows.insert(insert_idx, window);

        // Update focused window pointer if it was the moved window.
        if self.focused_window == Some(from) {
            self.focused_window = Some(to);
        }
    }
}

/// Per-monitor workspace collection.
///
/// Each physical monitor has one active workspace at a time, but may
/// contain any number of named workspaces that can be switched to on
/// demand.
#[derive(Debug, Clone)]
pub struct MonitorWorkspace {
    /// Monitor this collection belongs to.
    pub monitor_id: u32,
    /// ID of the workspace currently visible on this monitor.
    pub active_workspace: u32,
    /// All workspaces allocated for this monitor.
    pub workspaces: Vec<Workspace>,
}

impl MonitorWorkspace {
    /// Create a new monitor workspace collection.
    ///
    /// Workspace 1 is created automatically and set as active.
    pub fn new(monitor_id: u32) -> Self {
        let workspace = Workspace::new(1);
        Self {
            monitor_id,
            active_workspace: 1,
            workspaces: vec![workspace],
        }
    }

    /// Return a reference to the currently active workspace.
    pub fn get_active_workspace(&self) -> &Workspace {
        self.workspaces
            .iter()
            .find(|ws| ws.id == self.active_workspace)
            .or_else(|| self.workspaces.first())
            .unwrap_or_else(|| {
                // This path is theoretically unreachable because MonitorWorkspace::new()
                // always creates at least one workspace. Returning a static default
                // prevents panics in the face of memory corruption or re-entrant bugs.
                static DEFAULT: std::sync::OnceLock<Workspace> = std::sync::OnceLock::new();
                DEFAULT.get_or_init(|| Workspace::new(1))
            })
    }

    /// Return a mutable reference to the currently active workspace.
    pub fn get_active_workspace_mut(&mut self) -> &mut Workspace {
        let active_id = self.active_workspace;
        if let Some(idx) = self.workspaces.iter().position(|ws| ws.id == active_id) {
            return &mut self.workspaces[idx];
        }
        // Fallback: ensure workspace 1 exists and make it active
        if !self.workspaces.iter().any(|ws| ws.id == 1) {
            self.workspaces.push(Workspace::new(1));
        }
        self.active_workspace = 1;
        if let Some(idx) = self.workspaces.iter().position(|ws| ws.id == 1) {
            return &mut self.workspaces[idx];
        }
        // Ultimate fallback (theoretically unreachable)
        self.workspaces.push(Workspace::new(1));
        let last = self.workspaces.len() - 1;
        &mut self.workspaces[last]
    }

    /// Look up a workspace by ID (immutable).
    pub fn get_workspace(&self, id: u32) -> Option<&Workspace> {
        self.workspaces.iter().find(|ws| ws.id == id)
    }

    /// Look up a workspace by ID (mutable).
    pub fn get_workspace_mut(&mut self, id: u32) -> Option<&mut Workspace> {
        self.workspaces.iter_mut().find(|ws| ws.id == id)
    }

    /// Switch the active workspace on this monitor.
    ///
    /// If `id` does not yet exist it is created automatically.
    /// Returns `true` if the switch actually changed the active workspace.
    pub fn switch_workspace(&mut self, id: u32) -> bool {
        if self.active_workspace == id {
            return false;
        }
        self.ensure_workspace(id);
        self.active_workspace = id;
        true
    }

    /// Ensure that a workspace with the given ID exists.
    ///
    /// If it does not exist, it is created and appended to the list.
    /// Returns a mutable reference to the workspace.
    pub fn ensure_workspace(&mut self, id: u32) -> &mut Workspace {
        if !self.workspaces.iter().any(|ws| ws.id == id) {
            self.workspaces.push(Workspace::new(id));
            let last = self.workspaces.len() - 1;
            return &mut self.workspaces[last];
        }
        if let Some(idx) = self.workspaces.iter().position(|ws| ws.id == id) {
            return &mut self.workspaces[idx];
        }
        // Ultimate fallback (theoretically unreachable)
        self.workspaces.push(Workspace::new(id));
        let last = self.workspaces.len() - 1;
        &mut self.workspaces[last]
    }
}
