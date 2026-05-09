//! Window model: state machine and window data structures.
//!
//! Defines [`WindowState`] and [`Window`], the core types that represent
//! a managed top-level window inside HyprTile.  The state machine has five
//! states: `Tiling`, `Floating`, `Maximized`, `Fullscreen` and `Minimized`.

use crate::config::types::WindowRule;
use crate::platform::window::WindowId;
use crate::util::rect::Rect;

/// The five states a managed window can be in.
///
/// Transitions are driven by user keybinds, window rule matches, or
/// WinEventHook notifications.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowState {
    /// Window participates in the tiled layout.
    Tiling,
    /// Window is free-floating, positioned independently of the layout engine.
    Floating,
    /// Window is maximized (overrides tiling).
    Maximized,
    /// Window is in fullscreen mode (covers the entire monitor).
    Fullscreen,
    /// Window is minimized (iconic).  Layout engine ignores it.
    Minimized,
}

impl WindowState {
    /// Return `true` if the window should be positioned by the layout engine.
    pub fn is_tiling(self) -> bool {
        matches!(self, WindowState::Tiling)
    }

    /// Return `true` if the window is currently visible on screen.
    pub fn is_visible(self) -> bool {
        !matches!(self, WindowState::Minimized)
    }
}

/// Rich representation of a managed top-level window.
///
/// Equality and hashing are based **only** on [`WindowId`] so the same
/// underlying HWND is always treated as a single entity regardless of
/// mutable metadata changes (title, state, …).
#[derive(Debug, Clone)]
pub struct Window {
    /// Native window handle wrapped in a newtype.
    pub id: WindowId,
    /// Current state in the state machine.
    pub state: WindowState,
    /// Previous state before the most recent transition.
    ///
    /// Used to restore the correct state when exiting `Fullscreen` or
    /// `Minimized`.
    pub previous_state: Option<WindowState>,
    /// Win32 class name (e.g. "Chrome_WidgetWin_1").
    pub class_name: String,
    /// Window title text.
    pub title: String,
    /// Executable name of the owning process.
    pub process_name: String,
    /// Rectangle remembered when the window was last floating.
    ///
    /// If `None` the window has never been floated explicitly.
    pub floating_rect: Option<Rect>,
    /// Whether HyprTile is actively managing this window.
    pub is_managed: bool,
    /// Whether this is a UWP app hosted inside `ApplicationFrameWindow`.
    pub is_uwp: bool,
    /// Whether this is an Electron / Chromium-based app.
    pub is_electron: bool,
    /// Whether this window has the `WS_EX_TOOLWINDOW` style.
    pub is_tool: bool,
}

impl Window {
    /// Create a new window entry and immediately query its metadata from Win32.
    pub fn new(id: WindowId) -> Self {
        let class_name = id.get_class_name();
        let title = id.get_title();
        let process_name = id.get_process_name();
        let is_uwp = id.is_uwp_host();
        let is_electron = class_name == "Chrome_WidgetWin_1";
        let is_tool = id.is_tool_window();

        let mut win = Self {
            id,
            state: WindowState::Tiling,
            previous_state: None,
            class_name,
            title,
            process_name,
            floating_rect: id.get_rect(),
            is_managed: true,
            is_uwp,
            is_electron,
            is_tool,
        };

        // If the window is currently invisible or iconic, start in Minimized.
        if !id.is_visible() || id.is_iconic() {
            win.state = WindowState::Minimized;
        }

        // If the window is already maximized, reflect that.
        if id.is_zoomed() {
            win.state = WindowState::Maximized;
        }

        win
    }

    /// Re-query title, class name and process name from the Win32 API.
    ///
    /// Call this when a `WindowRenamed` event is received or periodically
    /// during a refresh sweep.
    pub fn refresh_info(&mut self) {
        self.title = self.id.get_title();
        self.class_name = self.id.get_class_name();
        self.process_name = self.id.get_process_name();
        self.is_uwp = self.id.is_uwp_host();
        self.is_electron = self.class_name == "Chrome_WidgetWin_1";
        self.is_tool = self.id.is_tool_window();
    }

    /// Transition to a new state, saving the previous one.
    ///
    /// The previous state is stored so that callers can later restore it
    /// (e.g. exiting fullscreen returns to tiling or floating).
    pub fn set_state(&mut self, new_state: WindowState) {
        if self.state == new_state {
            return;
        }
        self.previous_state = Some(self.state);
        self.state = new_state;
    }

    /// Toggle between `Tiling` and `Floating`.
    ///
    /// If the window is in any other state it transitions to `Tiling`.
    /// Returns the new state.
    pub fn toggle_float(&mut self) -> WindowState {
        let new_state = match self.state {
            WindowState::Floating => WindowState::Tiling,
            _ => {
                // Remember the current rect before going floating.
                if let Some(rect) = self.id.get_rect() {
                    self.floating_rect = Some(rect);
                }
                WindowState::Floating
            }
        };
        self.set_state(new_state);
        self.state
    }

    /// Toggle fullscreen mode.
    ///
    /// Entering fullscreen stores the previous state; exiting restores it.
    /// If the window was `Minimized` it goes to `Tiling` on exit.
    /// Returns the new state.
    pub fn toggle_fullscreen(&mut self) -> WindowState {
        match self.state {
            WindowState::Fullscreen => {
                // Restore previous state, defaulting to Tiling.
                let prev = self.previous_state.unwrap_or(WindowState::Tiling);
                self.previous_state = Some(self.state);
                self.state = prev;
            }
            _ => {
                self.set_state(WindowState::Fullscreen);
            }
        };
        self.state
    }

    /// Transition to `Minimized`.
    pub fn minimize(&mut self) {
        self.set_state(WindowState::Minimized);
    }

    /// Restore from `Minimized`.
    ///
    /// If there is a recorded previous state it is restored, otherwise
    /// defaults to `Tiling`.
    pub fn restore(&mut self) {
        let target = self.previous_state.unwrap_or(WindowState::Tiling);
        self.set_state(target);
    }

    /// Return `true` if the window should participate in layout calculations.
    ///
    /// Only `Tiling` windows are positioned by the layout engine.
    pub fn should_tile(&self) -> bool {
        self.is_managed && self.state.is_tiling()
    }

    /// Return `true` if the window is visible and actively managed.
    pub fn is_visible_and_managed(&self) -> bool {
        self.is_managed && self.state.is_visible()
    }

    /// Check whether this window matches a single [`WindowRule`].
    ///
    /// A window matches when **all** present rule fields match:
    /// * `match_class`   – regex against [`Window::class_name`]
    /// * `match_title`   – regex against [`Window::title`]
    /// * `match_process` – regex against [`Window::process_name`]
    ///
    /// An empty rule with no matchers returns `false`.
    pub fn matches_rule(&self, rule: &WindowRule) -> bool {
        use regex::Regex;

        let mut matched = false;

        if let Some(ref pattern) = rule.match_class {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(&self.class_name) {
                    matched = true;
                } else {
                    return false;
                }
            } else {
                // Invalid regex: fall back to literal comparison.
                if self.class_name == *pattern {
                    matched = true;
                } else {
                    return false;
                }
            }
        }

        if let Some(ref pattern) = rule.match_title {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(&self.title) {
                    matched = true;
                } else {
                    return false;
                }
            } else {
                if self.title == *pattern {
                    matched = true;
                } else {
                    return false;
                }
            }
        }

        if let Some(ref pattern) = rule.match_process {
            if let Ok(re) = Regex::new(pattern) {
                if re.is_match(&self.process_name) {
                    matched = true;
                } else {
                    return false;
                }
            } else {
                if self.process_name == *pattern {
                    matched = true;
                } else {
                    return false;
                }
            }
        }

        matched
    }
}

impl PartialEq for Window {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl Eq for Window {}

impl std::hash::Hash for Window {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}
