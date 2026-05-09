//! Window management: registry, state machine, rule application.
//!
//! [`WindowManager`] is the central authority for every top-level window
//! that HyprTile decides to manage.  It owns the `HashMap<WindowId, Window>`
//! registry, applies window rules at registration time, and exposes
//! filtered views of the window list to the layout engine.

// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
// AI_AGENT_STOP: WINDOW_MANAGER â€” HWND registry and state coordinator.
// Before modifying window management:
//   1. register_window() applies filter + rules before inserting.
//   2. WindowManager is the single owner of Window metadata.
//   3. All layout decisions use get_tiling_windows() from here.
//   4. Focus tracking lives here â€” workspace model delegates to it.
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

pub mod filter;
pub mod model;
pub mod rules;

use crate::config::types::Config;
use crate::platform::events::WindowEvent;
use crate::platform::window::WindowId;
use model::{Window, WindowState};
use rules::RuleEngine;
use std::collections::HashMap;
use tracing::{debug, info, warn};

/// Central registry of all managed windows.
///
/// Created once at startup and lives for the lifetime of the application.
/// All mutations go through the methods on this struct so that invariants
/// (e.g. "a floating window is never in the tiling layout") are maintained.
pub struct WindowManager {
    /// Every window we know about, keyed by its native handle.
    windows: HashMap<WindowId, Window>,
    /// Rule engine loaded from the current configuration.
    rule_engine: RuleEngine,
    /// Which window currently has keyboard focus (may be `None`).
    focused: Option<WindowId>,
}

impl WindowManager {
    /// Create a new manager and load rules from `config`.
    pub fn new(config: &Config) -> Self {
        let rule_engine = RuleEngine::new(config.window_rules.clone());
        info!(
            "WindowManager created with {} window rules",
            config.window_rules.len()
        );
        Self {
            windows: HashMap::new(),
            rule_engine,
            focused: None,
        }
    }

    /// Register a new top-level window.
    ///
    /// 1. Create a [`Window`] and query its metadata.
    /// 2. Apply window rules (may change state to `Floating`).
    /// 3. Insert into the registry.
    ///
    /// Returns a reference to the newly inserted window, or `None` if
    /// the window should not be managed (fails the filter checks).
    pub fn register_window(&mut self, hwnd: WindowId) -> Option<&Window> {
        if !filter::should_manage(hwnd) {
            debug!("Window {} failed management filter, skipping", hwnd.0);
            return None;
        }

        if self.windows.contains_key(&hwnd) {
            debug!("Window {} already registered", hwnd.0);
            return self.windows.get(&hwnd);
        }

        let mut window = Window::new(hwnd);
        self.rule_engine.apply_rules(&mut window);

        debug!(
            "Registered window {} (class='{}', title='{}', state={:?}, managed={})",
            hwnd.0, window.class_name, window.title, window.state, window.is_managed
        );

        self.windows.insert(hwnd, window);
        self.windows.get(&hwnd)
    }

    /// Remove a window from the registry.
    ///
    /// If the removed window was focused, focus is cleared.
    /// Returns the removed `Window` if it existed.
    pub fn unregister_window(&mut self, hwnd: WindowId) -> Option<Window> {
        let removed = self.windows.remove(&hwnd);
        if removed.is_some() {
            debug!("Unregistered window {}", hwnd.0);
            if self.focused == Some(hwnd) {
                self.focused = None;
            }
        }
        removed
    }

    /// Immutable access to a registered window.
    pub fn get_window(&self, hwnd: WindowId) -> Option<&Window> {
        self.windows.get(&hwnd)
    }

    /// Mutable access to a registered window.
    pub fn get_window_mut(&mut self, hwnd: WindowId) -> Option<&mut Window> {
        self.windows.get_mut(&hwnd)
    }

    /// Return the ID of the currently focused window.
    pub fn get_focused(&self) -> Option<WindowId> {
        self.focused
    }

    /// Set the focused window.
    ///
    /// No-op if the window is not in the registry.
    pub fn set_focused(&mut self, hwnd: WindowId) {
        if self.windows.contains_key(&hwnd) {
            self.focused = Some(hwnd);
            debug!("Focus set to window {}", hwnd.0);
        } else {
            warn!("Tried to focus unregistered window {}", hwnd.0);
        }
    }

    /// Return the IDs of all managed, visible windows.
    ///
    /// This is the broadest useful set: every window that is both
    /// marked `is_managed` and not `Minimized`.
    pub fn get_all_managed(&self) -> Vec<WindowId> {
        self.windows
            .values()
            .filter(|w| w.is_visible_and_managed())
            .map(|w| w.id)
            .collect()
    }

    /// Return the IDs of all windows currently in `Tiling` state.
    ///
    /// These are the windows that the layout engine should position.
    pub fn get_tiling_windows(&self) -> Vec<WindowId> {
        self.windows
            .values()
            .filter(|w| w.state == WindowState::Tiling && w.is_managed)
            .map(|w| w.id)
            .collect()
    }

    /// Return the IDs of all windows currently in `Floating` state.
    pub fn get_floating_windows(&self) -> Vec<WindowId> {
        self.windows
            .values()
            .filter(|w| w.state == WindowState::Floating && w.is_managed)
            .map(|w| w.id)
            .collect()
    }

    /// Return the IDs of all visible managed windows (both tiling and floating).
    pub fn get_visible_windows(&self) -> Vec<WindowId> {
        self.windows
            .values()
            .filter(|w| w.is_visible_and_managed())
            .map(|w| w.id)
            .collect()
    }

    /// Re-query title, class name and process name for a window.
    ///
    /// Call this after a rename event or during periodic refresh.
    pub fn refresh_window_info(&mut self, hwnd: WindowId) {
        if let Some(window) = self.windows.get_mut(&hwnd) {
            window.refresh_info();
            debug!(
                "Refreshed window {}: class='{}' title='{}' process='{}'",
                hwnd.0, window.class_name, window.title, window.process_name
            );
        }
    }

    /// Update internal state in response to a platform [`WindowEvent`].
    ///
    /// This is the bridge between raw WinEventHook notifications and the
    /// HyprTile state machine.
    pub fn handle_state_change(&mut self, hwnd: WindowId, event: &WindowEvent) {
        match event {
            WindowEvent::WindowCreated(_) => {
                self.register_window(hwnd);
            }
            WindowEvent::WindowDestroyed(_) => {
                self.unregister_window(hwnd);
            }
            WindowEvent::WindowShown(_) => {
                if let Some(window) = self.windows.get_mut(&hwnd) {
                    if window.state == WindowState::Minimized {
                        window.restore();
                    }
                } else {
                    self.register_window(hwnd);
                }
            }
            WindowEvent::WindowHidden(_) => {
                // Window moved off-screen or cloaked; treat like minimize
                // for layout purposes.
                if let Some(window) = self.windows.get_mut(&hwnd) {
                    window.minimize();
                }
            }
            WindowEvent::WindowMinimized(_) => {
                if let Some(window) = self.windows.get_mut(&hwnd) {
                    window.minimize();
                }
            }
            WindowEvent::WindowRestored(_) => {
                if let Some(window) = self.windows.get_mut(&hwnd) {
                    window.restore();
                }
            }
            WindowEvent::WindowFocused(_) => {
                self.set_focused(hwnd);
            }
            WindowEvent::WindowRenamed(_) => {
                self.refresh_window_info(hwnd);
            }
            _ => {
                // WindowMoved, WindowResized, MonitorChanged, DpiChanged,
                // ExplorerRestarted: no direct state machine transition.
            }
        }
    }

    /// Toggle a window between `Tiling` and `Floating`.
    ///
    /// Returns the new state, or `None` if the window is not registered.
    pub fn toggle_float(&mut self, hwnd: WindowId) -> Option<WindowState> {
        let window = self.windows.get_mut(&hwnd)?;
        let new_state = window.toggle_float();
        info!("Toggled float for window {} -> {:?}", hwnd.0, new_state);
        Some(new_state)
    }

    /// Toggle fullscreen mode for a window.
    ///
    /// Returns the new state, or `None` if the window is not registered.
    pub fn toggle_fullscreen(&mut self, hwnd: WindowId) -> Option<WindowState> {
        let window = self.windows.get_mut(&hwnd)?;
        let new_state = window.toggle_fullscreen();
        info!(
            "Toggled fullscreen for window {} -> {:?}",
            hwnd.0, new_state
        );
        Some(new_state)
    }

    /// Close the currently focused window.
    ///
    /// No-op if no window has focus.
    pub fn close_focused(&self) {
        if let Some(hwnd) = self.focused {
            debug!("Closing focused window {}", hwnd.0);
            crate::platform::window::close_window(hwnd.as_raw());
        } else {
            warn!("close_focused called but no window has focus");
        }
    }

    /// Return the number of registered windows.
    pub fn count(&self) -> usize {
        self.windows.len()
    }

    /// Replace the rule engine with a fresh one built from `config`.
    ///
    /// Call this after configuration hot-reload.
    pub fn reload_rules(&mut self, config: &Config) {
        info!(
            "Reloading window rules from config: {} rules",
            config.window_rules.len()
        );
        self.rule_engine.reload_rules(config.window_rules.clone());

        // Re-apply rules to all existing windows so that changes take
        // effect immediately.
        for window in self.windows.values_mut() {
            let old_state = window.state;
            self.rule_engine.apply_rules(window);
            if window.state != old_state {
                debug!(
                    "Window {} state changed from {:?} to {:?} after rule reload",
                    window.id.0, old_state, window.state
                );
            }
        }
    }
}
