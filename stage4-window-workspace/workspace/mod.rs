//! Workspace management: virtual desktops, monitor assignment, focus cycling.
//!
//! [`WorkspaceManager`] is the central authority for workspace state.
//! It tracks which windows live on which workspace, which workspace is
//! active on each monitor, and handles monitor disconnect by
//! redistributing windows to a fallback monitor.

pub mod model;

use model::*;
use crate::platform::monitor::Monitor;
use crate::platform::window::WindowId;
use anyhow::{anyhow, Result};
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Central authority for all workspace operations.
///
/// The manager owns the per-monitor workspace trees and the two
/// bidirectional lookup maps (`window -> workspace` and `window -> monitor`).
pub struct WorkspaceManager {
    /// Per-monitor workspace collections, keyed by monitor ID.
    monitors: HashMap<u32, MonitorWorkspace>,
    /// Which workspace each window lives on.
    window_to_workspace: HashMap<WindowId, u32>,
    /// Which monitor each window lives on.
    window_to_monitor: HashMap<WindowId, u32>,
}

impl WorkspaceManager {
    /// Create an empty manager.
    ///
    /// Call [`init_monitors`](Self::init_monitors) or
    /// [`add_monitor`](Self::add_monitor) before use.
    pub fn new() -> Self {
        Self {
            monitors: HashMap::new(),
            window_to_workspace: HashMap::new(),
            window_to_monitor: HashMap::new(),
        }
    }

    /// Initialize the manager from a list of enumerated monitors.
    ///
    /// Each monitor gets a default [`MonitorWorkspace`] with workspace 1
    /// as its active workspace.  Existing monitor entries are replaced.
    pub fn init_monitors(&mut self, monitors: &[Monitor]) {
        self.monitors.clear();
        for monitor in monitors {
            self.add_monitor(monitor);
        }
        info!(
            "WorkspaceManager initialized with {} monitor(s)",
            self.monitors.len()
        );
    }

    /// Add a single monitor.
    pub fn add_monitor(&mut self, monitor: &Monitor) {
        debug!("Adding monitor {} to workspace manager", monitor.id);
        self.monitors
            .insert(monitor.id, MonitorWorkspace::new(monitor.id));
    }

    /// Remove a monitor and all its associated workspace data.
    ///
    /// **Note:** Windows on the removed monitor are *not* automatically
    /// redistributed.  Call [`handle_monitor_disconnect`](Self::handle_monitor_disconnect)
    /// if you need to migrate windows before removal.
    pub fn remove_monitor(&mut self, monitor_id: u32) {
        debug!("Removing monitor {} from workspace manager", monitor_id);
        self.monitors.remove(&monitor_id);
    }

    /// Return an immutable reference to the active workspace on a monitor.
    pub fn get_active_workspace(&self, monitor_id: u32) -> Option<&Workspace> {
        self.monitors
            .get(&monitor_id)
            .map(|mw| mw.get_active_workspace())
    }

    /// Return a mutable reference to the active workspace on a monitor.
    pub fn get_active_workspace_mut(&mut self, monitor_id: u32) -> Option<&mut Workspace> {
        self.monitors
            .get_mut(&monitor_id)
            .map(|mw| mw.get_active_workspace_mut())
    }

    /// Switch the active workspace on a monitor.
    ///
    /// The target workspace is created automatically if it does not exist.
    pub fn switch_workspace(&mut self, monitor_id: u32, workspace_id: u32) -> Result<()> {
        let monitor = self
            .monitors
            .get_mut(&monitor_id)
            .ok_or_else(|| anyhow!("Monitor {} not found", monitor_id))?;

        let changed = monitor.switch_workspace(workspace_id);
        if changed {
            debug!(
                "Switched monitor {} to workspace {}",
                monitor_id, workspace_id
            );
        }
        Ok(())
    }

    /// Move a window to a different workspace.
    ///
    /// The window is removed from its current workspace (if any) and
    /// inserted into the target workspace on the **same monitor**.
    pub fn move_window_to_workspace(
        &mut self,
        window: WindowId,
        workspace_id: u32,
    ) -> Result<()> {
        // Find which monitor currently hosts this window.
        let monitor_id = self
            .window_to_monitor
            .get(&window)
            .copied()
            .ok_or_else(|| anyhow!("Window {} is not assigned to any monitor", window.as_raw().0))?;

        // Remove from current workspace.
        if let Some(current_ws_id) = self.window_to_workspace.remove(&window) {
            if let Some(mw) = self.monitors.get_mut(&monitor_id) {
                if let Some(ws) = mw.get_workspace_mut(current_ws_id) {
                    ws.remove_window(window);
                }
            }
        }

        // Ensure target workspace exists on that monitor.
        let monitor = self
            .monitors
            .get_mut(&monitor_id)
            .ok_or_else(|| anyhow!("Monitor {} not found", monitor_id))?;
        monitor.ensure_workspace(workspace_id);

        // Add to new workspace.
        if let Some(ws) = monitor.get_workspace_mut(workspace_id) {
            ws.add_window(window);
        }
        self.window_to_workspace.insert(window, workspace_id);

        debug!(
            "Moved window {} to workspace {} on monitor {}",
            window.as_raw().0, workspace_id, monitor_id
        );
        Ok(())
    }

    /// Move a window to a different monitor.
    ///
    /// The window is removed from its current workspace and added to
    /// the active workspace of the target monitor.
    pub fn move_window_to_monitor(&mut self, window: WindowId, monitor_id: u32) -> Result<()> {
        // Verify target monitor exists.
        if !self.monitors.contains_key(&monitor_id) {
            return Err(anyhow!("Monitor {} not found", monitor_id));
        }

        // Remove from current location.
        if let Some((old_monitor, old_workspace)) = self.remove_window_internal(window) {
            debug!(
                "Removed window {} from monitor {} workspace {} for monitor move",
                window.as_raw().0, old_monitor, old_workspace
            );
        }

        // Add to active workspace of target monitor.
        let target_monitor = self
            .monitors
            .get_mut(&monitor_id)
            .ok_or_else(|| anyhow!("Monitor {} disappeared during move", monitor_id))?;
        let active_ws_id = target_monitor.active_workspace;
        target_monitor
            .get_active_workspace_mut()
            .add_window(window);

        self.window_to_monitor.insert(window, monitor_id);
        self.window_to_workspace.insert(window, active_ws_id);

        debug!(
            "Moved window {} to monitor {} workspace {}",
            window.as_raw().0, monitor_id, active_ws_id
        );
        Ok(())
    }

    /// Add a window to the active workspace of the given monitor.
    ///
    /// This is the normal path for newly created windows.
    pub fn add_window(&mut self, window: WindowId, monitor_id: u32) -> Result<()> {
        let monitor = self
            .monitors
            .get_mut(&monitor_id)
            .ok_or_else(|| anyhow!("Monitor {} not found", monitor_id))?;

        let active_ws_id = monitor.active_workspace;
        monitor
            .get_active_workspace_mut()
            .add_window(window);

        self.window_to_monitor.insert(window, monitor_id);
        self.window_to_workspace.insert(window, active_ws_id);

        debug!(
            "Added window {} to monitor {} workspace {}",
            window.as_raw().0, monitor_id, active_ws_id
        );
        Ok(())
    }

    /// Remove a window from whatever workspace and monitor it is on.
    ///
    /// Returns `(monitor_id, workspace_id)` if the window was tracked,
    /// or `None` if it was unknown.
    pub fn remove_window(&mut self, window: WindowId) -> Option<(u32, u32)> {
        self.remove_window_internal(window)
    }

    /// Return the `(monitor_id, workspace_id)` location of a window.
    pub fn get_window_location(&self, window: WindowId) -> Option<(u32, u32)> {
        let monitor = *self.window_to_monitor.get(&window)?;
        let workspace = *self.window_to_workspace.get(&window)?;
        Some((monitor, workspace))
    }

    /// Handle a monitor disconnect by migrating all of its windows to
    /// a fallback monitor.
    ///
    /// Every window on every workspace of the disconnected monitor is
    /// moved to the active workspace of `fallback_monitor`.
    pub fn handle_monitor_disconnect(&mut self, disconnected_id: u32, fallback_id: u32) {
        info!(
            "Handling monitor disconnect: {} -> fallback {}",
            disconnected_id, fallback_id
        );

        let Some(mut mw) = self.monitors.remove(&disconnected_id) else {
            warn!(
                "Tried to handle disconnect for unknown monitor {}",
                disconnected_id
            );
            return;
        };

        // Collect all windows from all workspaces of the disconnected monitor.
        let windows_to_migrate: Vec<(WindowId, u32)> = mw
            .workspaces
            .iter()
            .flat_map(|ws| ws.windows.iter().map(|&w| (w, ws.id)))
            .collect();

        // Drop the old monitor data now that we have the list.
        drop(mw);

        // Ensure fallback monitor exists.
        if !self.monitors.contains_key(&fallback_id) {
            warn!(
                "Fallback monitor {} not found, creating default",
                fallback_id
            );
            self.monitors
                .insert(fallback_id, MonitorWorkspace::new(fallback_id));
        }

        let fallback_ws_id = self.monitors[&fallback_id].active_workspace;

        for (window, _old_ws) in windows_to_migrate {
            // Remove old tracking entries.
            self.window_to_monitor.remove(&window);
            self.window_to_workspace.remove(&window);

            // Add to fallback monitor's active workspace.
            if let Some(fm) = self.monitors.get_mut(&fallback_id) {
                fm.get_active_workspace_mut().add_window(window);
            }
            self.window_to_monitor.insert(window, fallback_id);
            self.window_to_workspace.insert(window, fallback_ws_id);
        }

        info!(
            "Migrated windows from monitor {} to monitor {} workspace {}",
            disconnected_id, fallback_id, fallback_ws_id
        );
    }

    /// Return the IDs of all windows across all workspaces on all monitors.
    pub fn get_all_windows(&self) -> Vec<WindowId> {
        self.monitors
            .values()
            .flat_map(|mw| mw.workspaces.iter().flat_map(|ws| ws.windows.iter().copied()))
            .collect()
    }

    /// Return the workspace ID that a window lives on, if tracked.
    pub fn get_workspace_for_window(&self, window: WindowId) -> Option<u32> {
        self.window_to_workspace.get(&window).copied()
    }

    /// Cycle focus on the active workspace of a monitor.
    ///
    /// No-op if the monitor does not exist or the workspace is empty.
    pub fn cycle_focus(&mut self, monitor_id: u32, direction: FocusDirection) {
        let Some(mw) = self.monitors.get_mut(&monitor_id) else {
            warn!(
                "cycle_focus called for unknown monitor {}",
                monitor_id
            );
            return;
        };
        mw.get_active_workspace_mut().cycle_focus(direction);
    }

    // ------------------------------------------------------------------
    // Internal helpers
    // ------------------------------------------------------------------

    /// Remove a window from its workspace and tracking maps.
    ///
    /// Returns `(monitor_id, workspace_id)` if the window was tracked.
    fn remove_window_internal(&mut self, window: WindowId) -> Option<(u32, u32)> {
        let monitor_id = self.window_to_monitor.remove(&window)?;
        let workspace_id = self.window_to_workspace.remove(&window)?;

        if let Some(mw) = self.monitors.get_mut(&monitor_id) {
            if let Some(ws) = mw.get_workspace_mut(workspace_id) {
                ws.remove_window(window);
            }
        }

        Some((monitor_id, workspace_id))
    }
}
