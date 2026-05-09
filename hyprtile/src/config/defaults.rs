use super::types::*;
use std::collections::HashMap;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: CONFIG_DEFAULTS — Default values for all config items.
// Before changing defaults:
//   1. Changing defaults affects all new users — consider backward compatibility.
//   2. Default keybinds must not conflict with common Windows shortcuts.
//   3. default_window_rules() should cover popular apps (Steam, browsers, etc.).
//   4. Keep defaults aligned with Hyprland conventions where possible.
// ═══════════════════════════════════════════════════════════════════════════════

/// Produce a fully-populated default configuration.
///
/// This is used when no config file exists or as the base set of values
/// before a user config is merged on top.
pub fn default_config() -> Config {
    Config {
        general: GeneralConfig {
            mod_key: ModKey::Alt,
            terminal: "wezterm.exe".to_string(),
            resize_on_border: true,
            resize_border_width: 8,
            auto_start: false,
            focus_follows_mouse: false,
            mouse_modifier: ModKey::Alt,
        },
        keybinds: default_keybinds(),
        gaps: GapsConfig {
            inner: 8,
            outer: 8,
            smart: true,
        },
        workspaces: WorkspacesConfig {
            count: 10,
            per_monitor: true,
        },
        window_rules: default_window_rules_vec(),
        monitors: vec![],
    }
}

/// Build the default keybind map.
///
/// The modifier token is written as `"mod"` — it is resolved to the
/// configured `mod_key` at registration time.
pub fn default_keybinds() -> HashMap<String, String> {
    let mut map = HashMap::new();

    // Application
    map.insert("mod+RETURN".to_string(), "exec_terminal".to_string());
    map.insert("mod+Q".to_string(), "close_window".to_string());

    // Focus
    map.insert("mod+LEFT".to_string(), "focus_left".to_string());
    map.insert("mod+RIGHT".to_string(), "focus_right".to_string());
    map.insert("mod+UP".to_string(), "focus_up".to_string());
    map.insert("mod+DOWN".to_string(), "focus_down".to_string());

    // Move
    map.insert("mod+SHIFT+LEFT".to_string(), "move_left".to_string());
    map.insert("mod+SHIFT+RIGHT".to_string(), "move_right".to_string());
    map.insert("mod+SHIFT+UP".to_string(), "move_up".to_string());
    map.insert("mod+SHIFT+DOWN".to_string(), "move_down".to_string());

    // Window state
    map.insert("mod+T".to_string(), "toggle_float".to_string());
    map.insert("mod+F".to_string(), "toggle_fullscreen".to_string());

    // Layout
    map.insert("mod+M".to_string(), "cycle_layout".to_string());

    // Config & lifecycle
    map.insert("mod+R".to_string(), "reload_config".to_string());
    map.insert("mod+SHIFT+E".to_string(), "exit".to_string());

    // Workspaces 1-9
    for i in 1..=9 {
        map.insert(
            format!("mod+{}", i),
            format!("workspace_{}", i),
        );
    }

    // Workspace 10 is bound to 0
    map.insert("mod+0".to_string(), "workspace_10".to_string());

    // Move to workspaces 1-9
    for i in 1..=9 {
        map.insert(
            format!("mod+SHIFT+{}", i),
            format!("move_to_workspace_{}", i),
        );
    }

    // Move to workspace 10
    map.insert(
        "mod+SHIFT+0".to_string(),
        "move_to_workspace_10".to_string(),
    );

    map
}

/// Default window rules that ship with HyprTile.
///
/// These rules make certain application windows float by default
/// (e.g. Steam dialogs, browser Picture-in-Picture windows).
pub fn default_window_rules_vec() -> Vec<WindowRule> {
    vec![
        // Steam windows (friends list, chat, settings, etc.)
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
        // Browser Picture-in-Picture
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
    fn test_default_keybinds_has_core_bindings() {
        let binds = default_keybinds();
        assert!(binds.contains_key("mod+RETURN"));
        assert!(binds.contains_key("mod+Q"));
        assert!(binds.contains_key("mod+T"));
        assert!(binds.contains_key("mod+F"));
        assert!(binds.contains_key("mod+M"));
        assert!(binds.contains_key("mod+R"));
        assert!(binds.contains_key("mod+SHIFT+E"));
    }

    #[test]
    fn test_default_keybinds_workspace_count() {
        let binds = default_keybinds();

        // 10 workspace binds + 10 move binds
        let workspace_binds: Vec<_> = binds
            .keys()
            .filter(|k| k.starts_with("mod+") && !k.contains("SHIFT"))
            .filter(|k| {
                k.strip_prefix("mod+")
                    .and_then(|n| n.parse::<u32>().ok())
                    .is_some()
            })
            .collect();
        assert_eq!(workspace_binds.len(), 10);
    }

    #[test]
    fn test_default_keybinds_move_workspace_count() {
        let binds = default_keybinds();

        let move_binds: Vec<_> = binds
            .keys()
            .filter(|k| k.starts_with("mod+SHIFT+"))
            .filter(|k| {
                k.strip_prefix("mod+SHIFT+")
                    .and_then(|n| n.parse::<u32>().ok())
                    .is_some()
            })
            .collect();
        assert_eq!(move_binds.len(), 10);
    }

    #[test]
    fn test_default_window_rules_vec_content() {
        let rules = default_window_rules_vec();
        assert_eq!(rules.len(), 2);
        assert!(rules[0].match_class.as_ref().unwrap().contains("steam"));
        assert_eq!(
            rules[1].match_title.as_ref().unwrap(),
            "Picture-in-Picture"
        );
        assert_eq!(rules[1].size, Some([400, 225]));
        assert_eq!(
            rules[1].position.as_ref().unwrap(),
            "bottom_right"
        );
    }

    #[test]
    fn test_default_config_structure() {
        let config = default_config();
        assert_eq!(config.general.mod_key, ModKey::Alt);
        assert_eq!(config.general.terminal, "wezterm.exe");
        assert_eq!(config.gaps.inner, 8);
        assert_eq!(config.gaps.outer, 8);
        assert_eq!(config.workspaces.count, 10);
        assert!(config.workspaces.per_monitor);
        assert_eq!(config.monitors.len(), 0);
        assert!(!config.general.auto_start);
    }
}
