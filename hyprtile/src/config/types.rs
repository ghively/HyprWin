use serde::{Deserialize, Serialize};
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: CONFIG_TYPES — serde data model for TOML config.
// Before adding new config fields:
//   1. Add the field to the appropriate struct.
//   2. Use #[serde(default = "...")] for backward compatibility.
//   3. Add a default function returning the default value.
//   4. Add the field to default_config() in defaults.rs.
//   5. Document in docs/CONFIGURATION.md with example value.
// ═══════════════════════════════════════════════════════════════════════════════

/// The root configuration structure for HyprTile.
///
/// This struct is deserialized from the `hyprtile.toml` config file.
/// All fields have sensible defaults so that a minimal config (or no config)
/// will still produce a working setup.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    /// General behavior settings (modifier key, terminal, etc.)
    #[serde(default)]
    pub general: GeneralConfig,
    /// Keybind mappings in the form `"mod+RETURN" -> "exec_terminal"`.
    #[serde(default)]
    pub keybinds: HashMap<String, String>,
    /// Gap configuration between tiled windows.
    #[serde(default)]
    pub gaps: GapsConfig,
    /// Workspace count and per-monitor settings.
    #[serde(default)]
    pub workspaces: WorkspacesConfig,
    /// Rules that control how specific windows are handled.
    #[serde(default = "default_window_rules")]
    pub window_rules: Vec<WindowRule>,
    /// Per-monitor configuration overrides.
    #[serde(default)]
    pub monitors: Vec<MonitorConfig>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            keybinds: HashMap::new(),
            gaps: GapsConfig::default(),
            workspaces: WorkspacesConfig::default(),
            window_rules: default_window_rules(),
            monitors: Vec::new(),
        }
    }
}

/// General application behaviour settings.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralConfig {
    /// The primary modifier key used for all keybinds.
    #[serde(default = "default_mod_key")]
    pub mod_key: ModKey,
    /// Command to launch the default terminal emulator.
    #[serde(default = "default_terminal")]
    pub terminal: String,
    /// Whether resizing is allowed by dragging window borders.
    #[serde(default = "default_true")]
    pub resize_on_border: bool,
    /// Width in pixels of the resize border area.
    #[serde(default = "default_resize_border_width")]
    pub resize_border_width: u32,
    /// Whether HyprTile should start automatically on login.
    #[serde(default = "default_true")]
    pub auto_start: bool,
    /// Whether focus follows the mouse cursor.
    #[serde(default)]
    pub focus_follows_mouse: bool,
    /// Modifier key used for mouse-based actions (move, resize).
    #[serde(default)]
    pub mouse_modifier: ModKey,
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            mod_key: default_mod_key(),
            terminal: default_terminal(),
            resize_on_border: default_true(),
            resize_border_width: default_resize_border_width(),
            auto_start: default_true(),
            focus_follows_mouse: false,
            mouse_modifier: ModKey::Alt,
        }
    }
}

/// Modifier keys used for keybind combinations.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq, Hash)]
#[serde(rename_all = "UPPERCASE")]
#[derive(Default)]
pub enum ModKey {
    #[default]
    Alt,
    Win,
    Ctrl,
    Shift,
}

/// Gap configuration for tiled windows.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GapsConfig {
    /// Inner gap between tiled windows, in pixels.
    #[serde(default = "default_gap")]
    pub inner: u32,
    /// Outer gap between tiled windows and screen edges, in pixels.
    #[serde(default = "default_gap")]
    pub outer: u32,
    /// When true, gaps are disabled when only a single window is present.
    #[serde(default = "default_true")]
    pub smart: bool,
}

impl Default for GapsConfig {
    fn default() -> Self {
        Self {
            inner: default_gap(),
            outer: default_gap(),
            smart: default_true(),
        }
    }
}

/// Workspace configuration.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspacesConfig {
    /// Number of workspaces to create per monitor.
    #[serde(default = "default_workspace_count")]
    pub count: u32,
    /// Whether workspaces are independent per monitor.
    #[serde(default = "default_true")]
    pub per_monitor: bool,
}

impl Default for WorkspacesConfig {
    fn default() -> Self {
        Self {
            count: default_workspace_count(),
            per_monitor: default_true(),
        }
    }
}

/// A rule that matches windows based on class, title, or process name
/// and applies an action (float, tile) or assigns them to a workspace/monitor.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowRule {
    /// Regex pattern to match against the window class name.
    #[serde(default)]
    pub match_class: Option<String>,
    /// Regex pattern to match against the window title.
    #[serde(default)]
    pub match_title: Option<String>,
    /// Regex pattern to match against the process name.
    #[serde(default)]
    pub match_process: Option<String>,
    /// Action to apply (float or tile).
    #[serde(default)]
    pub action: Option<WindowAction>,
    /// Target workspace ID to assign the window to.
    #[serde(default)]
    pub workspace: Option<u32>,
    /// Target monitor ID to assign the window to.
    #[serde(default)]
    pub monitor: Option<u32>,
    /// Desired window size `[width, height]` in pixels.
    #[serde(default)]
    pub size: Option<[u32; 2]>,
    /// Desired window position (e.g. `"bottom_right"`, `"center"`).
    #[serde(default)]
    pub position: Option<String>,
}

/// Actions that can be applied to a window through a rule.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WindowAction {
    /// The window should float instead of being tiled.
    Float,
    /// The window should be tiled (the default).
    Tile,
}

/// Per-monitor configuration override.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorConfig {
    /// The monitor ID (0-based index).
    pub id: u32,
    /// List of workspace IDs assigned to this monitor.
    #[serde(default)]
    pub workspaces: Vec<u32>,
    /// Default layout name for workspaces on this monitor.
    #[serde(default = "default_layout")]
    pub default_layout: String,
}

// ---------------------------------------------------------------------------
// Serde default helper functions
// ---------------------------------------------------------------------------

fn default_mod_key() -> ModKey {
    ModKey::Alt
}

fn default_terminal() -> String {
    "wezterm.exe".to_string()
}

fn default_gap() -> u32 {
    8
}

fn default_workspace_count() -> u32 {
    10
}

fn default_true() -> bool {
    true
}

fn default_resize_border_width() -> u32 {
    8
}

fn default_layout() -> String {
    "dwindle".to_string()
}

fn default_window_rules() -> Vec<WindowRule> {
    vec![
        WindowRule {
            match_class: Some(".*-steam-.*".to_string()),
            match_title: None,
            match_process: None,
            action: Some(WindowAction::Float),
            workspace: None,
            monitor: None,
            size: None,
            position: None,
        },
        WindowRule {
            match_class: None,
            match_title: Some("Picture-in-Picture".to_string()),
            match_process: None,
            action: Some(WindowAction::Float),
            workspace: None,
            monitor: None,
            size: Some([400, 225]),
            position: Some("bottom_right".to_string()),
        },
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.general.mod_key, ModKey::Alt);
        assert_eq!(config.general.terminal, "wezterm.exe");
        assert_eq!(config.gaps.inner, 8);
        assert_eq!(config.gaps.outer, 8);
        assert_eq!(config.workspaces.count, 10);
        assert_eq!(config.window_rules.len(), 2);
    }

    #[test]
    fn test_mod_key_serialize() {
        let key = ModKey::Win;
        let json = serde_json::to_string(&key).unwrap();
        assert_eq!(json, "\"WIN\"");
    }

    #[test]
    fn test_window_action_serialize() {
        let action = WindowAction::Float;
        let json = serde_json::to_string(&action).unwrap();
        assert_eq!(json, "\"float\"");
    }

    #[test]
    fn test_window_rule_default() {
        let rule = WindowRule {
            match_class: None,
            match_title: None,
            match_process: None,
            action: None,
            workspace: None,
            monitor: None,
            size: None,
            position: None,
        };
        assert!(rule.match_class.is_none());
        assert!(rule.action.is_none());
    }

    #[test]
    fn test_monitor_config() {
        let monitor = MonitorConfig {
            id: 0,
            workspaces: vec![1, 2, 3],
            default_layout: "master_stack".to_string(),
        };
        assert_eq!(monitor.id, 0);
        assert_eq!(monitor.workspaces, vec![1, 2, 3]);
        assert_eq!(monitor.default_layout, "master_stack");
    }

    #[test]
    fn test_gaps_config_smart_default() {
        let gaps = GapsConfig::default();
        assert!(gaps.smart);
    }

    #[test]
    fn test_default_window_rules_count() {
        let rules = default_window_rules();
        assert_eq!(rules.len(), 2);
        assert!(rules[0].match_class.is_some());
        assert!(rules[1].match_title.is_some());
    }
}
