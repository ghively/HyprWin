# HyprTile Configuration Guide

This document provides a complete reference for configuring HyprTile via the `hyprtile.toml` configuration file.

## Table of Contents

- [Configuration File Location](#configuration-file-location)
- [Configuration Sections](#configuration-sections)
  - [`[general]`](#general)
  - [`[keybinds]`](#keybinds)
  - [`[gaps]`](#gaps)
  - [`[workspaces]`](#workspaces)
  - [`[[window_rules]]`](#window_rules)
  - [`[[monitors]]`](#monitors)
- [Full Annotated Example](#full-annotated-example)
- [Common Window Rules](#common-window-rules)

---

## Configuration File Location

HyprTile reads its configuration from:

```
%APPDATA%\hyprtile\hyprtile.toml
```

Which resolves to a path like:

```
C:\Users\<Username>\AppData\Roaming\hyprtile\hyprtile.toml
```

If the file does not exist on first launch, HyprTile creates it with sensible defaults. You can override the config location via the `--config` CLI flag:

```powershell
hyprtile.exe --config "C:\path\to\custom_config.toml"
```

Or print the default configuration with:

```powershell
hyprtile.exe --print-default-config
```

---

## Configuration Sections

### `[general]`

General behavior and appearance settings.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `mod_key` | string | `"ALT"` | Primary modifier key. One of: `"ALT"`, `"WIN"`, `"CTRL"`, `"SHIFT"` |
| `terminal` | string | `"wezterm.exe"` | Terminal emulator launched by `exec_terminal` action |
| `resize_on_border` | boolean | `true` | Enable mouse resize by dragging window borders |
| `resize_border_width` | integer | `8` | Width of the resize border hit area in pixels |
| `auto_start` | boolean | `false` | Start HyprTile automatically on user login |
| `focus_follows_mouse` | boolean | `false` | Automatically focus window under cursor |
| `mouse_modifier` | string | `"ALT"` | Modifier key for mouse actions (move/resize floating windows) |

**Example:**

```toml
[general]
mod_key = "WIN"
terminal = "alacritty.exe"
resize_on_border = true
resize_border_width = 8
auto_start = true
focus_follows_mouse = false
mouse_modifier = "ALT"
```

---

### `[keybinds]`

The `keybinds` table maps key combinations to actions. The `"mod"` string in a keybind is automatically substituted with the value of `general.mod_key`.

#### Action Names

The following action names can be assigned to keybinds:

| Action Name | Description |
|-------------|-------------|
| `exec_terminal` | Launch the configured terminal emulator |
| `close_window` | Close the currently focused window |
| `focus_left` | Move focus to the window on the left |
| `focus_right` | Move focus to the window on the right |
| `focus_up` | Move focus to the window above |
| `focus_down` | Move focus to the window below |
| `move_left` | Move the focused window to the left position |
| `move_right` | Move the focused window to the right position |
| `move_up` | Move the focused window to the above position |
| `move_down` | Move the focused window to the below position |
| `toggle_float` | Toggle the focused window between tiling and floating |
| `toggle_fullscreen` | Toggle fullscreen mode for the focused window |
| `cycle_layout` | Cycle to the next layout algorithm |
| `reload_config` | Reload the configuration file from disk |
| `exit` | Exit HyprTile and return to normal Windows behavior |
| `workspace_N` | Switch to workspace N (1-10) |
| `move_to_workspace_N` | Move focused window to workspace N (1-10) |

#### Key Names

The following key names can be used in keybind definitions:

- **Letters**: `"A"` through `"Z"`
- **Numbers**: `"0"` through `"9"`
- **Arrow keys**: `"LEFT"`, `"RIGHT"`, `"UP"`, `"DOWN"`
- **Special**: `"RETURN"`, `"SPACE"`, `"TAB"`, `"ESCAPE"`, `"BACKSPACE"`
- **Function**: `"F1"` through `"F12"`
- **Modifiers in keybind**: `"SHIFT"` (combined with `mod`)

**Example:**

```toml
[keybinds]
# Application launch
"mod+RETURN" = "exec_terminal"

# Window management
"mod+Q" = "close_window"
"mod+T" = "toggle_float"
"mod+F" = "toggle_fullscreen"

# Focus movement
"mod+LEFT"  = "focus_left"
"mod+RIGHT" = "focus_right"
"mod+UP"    = "focus_up"
"mod+DOWN"  = "focus_down"

# Window movement
"mod+SHIFT+LEFT"  = "move_left"
"mod+SHIFT+RIGHT" = "move_right"
"mod+SHIFT+UP"    = "move_up"
"mod+SHIFT+DOWN"  = "move_down"

# Layout and config
"mod+M" = "cycle_layout"
"mod+R" = "reload_config"
"mod+SHIFT+E" = "exit"

# Workspaces
"mod+1" = "workspace_1"
"mod+2" = "workspace_2"
"mod+3" = "workspace_3"
"mod+4" = "workspace_4"
"mod+5" = "workspace_5"
"mod+6" = "workspace_6"
"mod+7" = "workspace_7"
"mod+8" = "workspace_8"
"mod+9" = "workspace_9"
"mod+0" = "workspace_10"

# Move to workspace
"mod+SHIFT+1" = "move_to_workspace_1"
"mod+SHIFT+2" = "move_to_workspace_2"
"mod+SHIFT+3" = "move_to_workspace_3"
```

---

### `[gaps]`

Configure spacing between tiled windows and around the screen edge.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `inner` | integer | `8` | Gap size between adjacent tiled windows (pixels) |
| `outer` | integer | `8` | Gap size between tiled windows and screen edge (pixels) |
| `smart` | boolean | `true` | When `true`, gaps are removed when only one window is visible |

**How gaps work:**

- **Inner gaps** create space between adjacent windows in a layout
- **Outer gaps** create a margin around the entire workspace area
- **Smart gaps** (`smart = true`) automatically disable both inner and outer gaps when only a single window is visible on a workspace, giving that window the full screen area

**Example:**

```toml
[gaps]
inner = 10   # 10px gap between windows
outer = 10   # 10px margin around screen edge
smart = true # No gaps for single-window workspaces
```

---

### `[workspaces]`

Configure workspace behavior.

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `count` | integer | `10` | Total number of workspaces (numbered 1 through N) |
| `per_monitor` | boolean | `true` | When `true`, each monitor has its own independent set of workspaces |

**Example:**

```toml
[workspaces]
count = 10
per_monitor = true
```

With `per_monitor = true` and 2 monitors, each monitor has 10 independent workspaces. With `per_monitor = false`, all monitors share the same 10 workspaces.

---

### `[[window_rules]]`

Window rules allow you to automatically apply actions to windows based on matching criteria. Rules are evaluated in order when a new window is created.

#### Match Fields

| Field | Type | Description |
|-------|------|-------------|
| `match_class` | string (regex) | Match against the window's class name |
| `match_title` | string (regex) | Match against the window's title bar text |
| `match_process` | string (regex) | Match against the process executable name |

All match fields are optional. If multiple match fields are specified on a single rule, they are combined with **AND** logic (all must match). The match values are treated as regular expressions.

#### Action Fields

| Field | Type | Description |
|-------|------|-------------|
| `action` | string | `"float"` to float the window, `"tile"` to force tiling |
| `workspace` | integer | Automatically move the window to this workspace |
| `monitor` | integer | Automatically move the window to this monitor |
| `size` | array | `[width, height]` in pixels for floating windows |
| `position` | string | Position for floating windows: `"center"`, `"bottom_right"`, `"top_right"`, `"bottom_left"`, `"top_left"` |

**Example:**

```toml
[[window_rules]]
match_class = ".*steam.*"
action = "float"

[[window_rules]]
match_title = "Picture-in-Picture"
action = "float"
size = [400, 225]
position = "bottom_right"

[[window_rules]]
match_process = "firefox.exe"
action = "tile"
workspace = 2

[[window_rules]]
match_class = "Chrome_WidgetWin_1"
match_title = ".*YouTube.*"
action = "float"
```

---

### `[[monitors]]`

Optional per-monitor configuration. If not specified, all monitors use defaults.

| Field | Type | Description |
|-------|------|-------------|
| `id` | integer | Monitor ID (0-based, in order of enumeration) |
| `workspaces` | array | Workspace IDs assigned to this monitor (e.g., `[1, 2, 3, 4, 5]`) |
| `default_layout` | string | Default layout for this monitor: `"dwindle"`, `"master_stack"`, `"monocle"`, `"grid"` |

**Example:**

```toml
# Primary monitor gets workspaces 1-5 with dwindle layout
[[monitors]]
id = 0
workspaces = [1, 2, 3, 4, 5]
default_layout = "dwindle"

# Secondary monitor gets workspaces 6-10 with master_stack layout
[[monitors]]
id = 1
workspaces = [6, 7, 8, 9, 10]
default_layout = "master_stack"
```

---

## Full Annotated Example

Below is a complete, heavily annotated `hyprtile.toml` that demonstrates all available options:

```toml
# =============================================================================
# HyprTile Configuration File
# Location: %APPDATA%\hyprtile\hyprtile.toml
# Reload: Press $mod+R or edit this file (auto-reload when hot-reload is enabled)
# =============================================================================

# ---------------------------------------------------------------------------
# [general] - Core behavior settings
# ---------------------------------------------------------------------------
[general]
# Modifier key used for all keybinds. "mod" in keybinds is replaced with this.
# Options: "ALT", "WIN" (Windows key), "CTRL", "SHIFT"
mod_key = "ALT"

# Terminal emulator launched by the "exec_terminal" action.
# Must be in PATH or provide full path.
terminal = "wezterm.exe"

# Enable mouse resize by dragging window borders and edges.
# When enabled, a border zone around each window acts as a resize handle.
resize_on_border = true

# Width (in pixels) of the invisible resize border hit area.
# Larger values make resizing easier but may interfere with other apps.
resize_border_width = 8

# Start HyprTile automatically when the user logs in.
# Creates a startup registry entry on first run.
auto_start = false

# Automatically focus the window under the mouse cursor.
# When enabled, moving the mouse between windows changes focus.
focus_follows_mouse = false

# Modifier key for mouse-based actions on floating windows.
# Hold this key and drag to move, or right-drag to resize.
mouse_modifier = "ALT"

# ---------------------------------------------------------------------------
# [gaps] - Window spacing configuration
# ---------------------------------------------------------------------------
[gaps]
# Gap between adjacent tiled windows (pixels).
inner = 8

# Gap between tiled windows and the edge of the screen (pixels).
outer = 8

# Smart gaps: automatically remove gaps when only one window is visible.
# This maximizes screen usage for single-window workspaces.
smart = true

# ---------------------------------------------------------------------------
# [workspaces] - Workspace behavior
# ---------------------------------------------------------------------------
[workspaces]
# Total number of workspaces. Workspaces are numbered 1 through this value.
count = 10

# Whether each monitor has its own independent set of workspaces.
# true: Each monitor has `count` workspaces (monitor 0: 1-10, monitor 1: 1-10)
# false: All monitors share workspaces 1-10
per_monitor = true

# ---------------------------------------------------------------------------
# [keybinds] - Keyboard shortcuts
# ---------------------------------------------------------------------------
# The string "mod" is automatically replaced with general.mod_key.
# Key names: RETURN, SPACE, TAB, ESCAPE, BACKSPACE, F1-F12,
#            LEFT, RIGHT, UP, DOWN, A-Z, 0-9
[keybinds]
# Application launch
"mod+RETURN" = "exec_terminal"

# Window management
"mod+Q" = "close_window"
"mod+T" = "toggle_float"
"mod+F" = "toggle_fullscreen"

# Focus navigation
"mod+LEFT"  = "focus_left"
"mod+RIGHT" = "focus_right"
"mod+UP"    = "focus_up"
"mod+DOWN"  = "focus_down"

# Window movement (within layout)
"mod+SHIFT+LEFT"  = "move_left"
"mod+SHIFT+RIGHT" = "move_right"
"mod+SHIFT+UP"    = "move_up"
"mod+SHIFT+DOWN"  = "move_down"

# Layout and system
"mod+M"       = "cycle_layout"
"mod+R"       = "reload_config"
"mod+SHIFT+E" = "exit"

# Workspace switching
"mod+1" = "workspace_1"
"mod+2" = "workspace_2"
"mod+3" = "workspace_3"
"mod+4" = "workspace_4"
"mod+5" = "workspace_5"
"mod+6" = "workspace_6"
"mod+7" = "workspace_7"
"mod+8" = "workspace_8"
"mod+9" = "workspace_9"
"mod+0" = "workspace_10"

# Move window to workspace
"mod+SHIFT+1" = "move_to_workspace_1"
"mod+SHIFT+2" = "move_to_workspace_2"
"mod+SHIFT+3" = "move_to_workspace_3"
"mod+SHIFT+4" = "move_to_workspace_4"
"mod+SHIFT+5" = "move_to_workspace_5"
"mod+SHIFT+6" = "move_to_workspace_6"
"mod+SHIFT+7" = "move_to_workspace_7"
"mod+SHIFT+8" = "move_to_workspace_8"
"mod+SHIFT+9" = "move_to_workspace_9"
"mod+SHIFT+0" = "move_to_workspace_10"

# ---------------------------------------------------------------------------
# [[window_rules]] - Automatic window behavior
# ---------------------------------------------------------------------------
# Rules are evaluated in order when a window is created.
# Each rule can match by class, title, or process (regex).
# Multiple match fields on one rule use AND logic.

# Steam windows should float (dialogs, notifications, etc.)
[[window_rules]]
match_class = ".*steam.*"
action = "float"

# Browser Picture-in-Picture windows: float in bottom-right corner
[[window_rules]]
match_title = "Picture-in-Picture"
action = "float"
size = [400, 225]
position = "bottom_right"

# File dialogs should float
[[window_rules]]
match_class = "#32770"
action = "float"

# Terminal windows go to workspace 1
[[window_rules]]
match_process = "wezterm.exe"
action = "tile"
workspace = 1

# Browser windows go to workspace 2
[[window_rules]]
match_process = "firefox.exe"
action = "tile"
workspace = 2

# Code editors go to workspace 3
[[window_rules]]
match_process = "Code.exe"
action = "tile"
workspace = 3

# ---------------------------------------------------------------------------
# [[monitors]] - Per-monitor configuration (optional)
# ---------------------------------------------------------------------------
# If omitted, all monitors use default settings.

# Primary monitor (left)
[[monitors]]
id = 0
workspaces = [1, 2, 3, 4, 5]
default_layout = "dwindle"

# Secondary monitor (right, portrait)
[[monitors]]
id = 1
workspaces = [6, 7, 8, 9, 10]
default_layout = "master_stack"
```

---

## Common Window Rules

This section provides copy-paste ready window rules for popular applications and common scenarios.

### Application-Specific Rules

```toml
# --- Steam ---
# Steam client and overlay windows should float
[[window_rules]]
match_class = ".*steam.*"
action = "float"

# --- Browsers ---
# Firefox Picture-in-Picture
[[window_rules]]
match_title = "Picture-in-Picture"
action = "float"
size = [400, 225]
position = "bottom_right"

# Chrome/Chromium Picture-in-Picture
[[window_rules]]
match_title = "Picture in picture"
action = "float"
size = [400, 225]
position = "bottom_right"

# --- Communication Apps ---
# Discord popup notifications
[[window_rules]]
match_class = "Chrome_WidgetWin_1"
match_title = "Discord"
action = "tile"

# Slack calls and huddles
[[window_rules]]
match_title = "Slack | .*huddle.*"
action = "float"

# Zoom meeting windows
[[window_rules]]
match_process = "Zoom.exe"
match_title = "Zoom Meeting"
action = "tile"

# --- Development Tools ---
# Visual Studio Code
[[window_rules]]
match_process = "Code.exe"
action = "tile"
workspace = 3

# JetBrains IDE tool windows (popups)
[[window_rules]]
match_title = ".* - Popup"
action = "float"

# Windows Terminal
[[window_rules]]
match_process = "WindowsTerminal.exe"
action = "tile"
workspace = 1

# --- System Dialogs ---
# All standard Windows dialogs (Open, Save, etc.)
[[window_rules]]
match_class = "#32770"
action = "float"

# Task Manager
[[window_rules]]
match_process = "Taskmgr.exe"
action = "float"

# Windows Settings
[[window_rules]]
match_title = "Settings"
match_process = "ApplicationFrameHost.exe"
action = "float"

# --- Media Players ---
# VLC media player
[[window_rules]]
match_process = "vlc.exe"
action = "tile"
workspace = 5

# Spotify
[[window_rules]]
match_process = "Spotify.exe"
action = "tile"
workspace = 5

# --- Games ---
# Most games handle their own window management
[[window_rules]]
match_process = ".*\.exe"
match_title = ".*\(fullscreen\).*"
action = "float"
```

### General-Purpose Rules

```toml
# Float all dialogs by their class name pattern
[[window_rules]]
match_class = ".*dialog.*"
action = "float"

# Float splash screens
[[window_rules]]
match_title = ".*[Ss]plash.*"
action = "float"

# Float small utility windows (roughly under 300x200)
# Note: Size-based rules require the window to exist first;
# these are best handled by class or title patterns.
```

---

## Configuration Validation

HyprTile validates your configuration on load and reports issues:

```
[WARN] Config validation: Unknown layout "dwindle2" in monitors[1].default_layout
[WARN] Config validation: Keybind "mod+INVALID_KEY" uses unrecognized key
[INFO] Config loaded successfully from %APPDATA%\hyprtile\hyprtile.toml
```

Common validation warnings:

| Warning | Cause | Fix |
|---------|-------|-----|
| `Unknown layout "X"` | Invalid layout name in `default_layout` | Use `"dwindle"`, `"master_stack"`, `"monocle"`, or `"grid"` |
| `Unrecognized key "X"` | Invalid key name in keybind | Check key names against the supported list |
| `Invalid regex in window_rules[N]` | Regex syntax error in match field | Verify regex syntax |
| `Workspace N exceeds count` | Workspace ID larger than `workspaces.count` | Increase `count` or reduce workspace ID |

---

## Tips and Best Practices

1. **Start with defaults**: Run HyprTile without a config file first to generate sensible defaults, then customize incrementally.

2. **Use `$mod+R` liberally**: The config hot-reload lets you experiment without restarting.

3. **Regex testing**: Test your window rule regex patterns in an online regex tester before adding them to your config.

4. **Rule ordering matters**: More specific rules should come before general ones. Rules are evaluated top-to-bottom.

5. **Backup before major changes**: Save a working copy of your config before making large changes.

6. **Use workspaces strategically**: Assign application types to consistent workspaces (e.g., terminals on 1, browsers on 2, code on 3).

7. **Monitor your performance**: If you notice sluggishness with many rules, consider consolidating or simplifying regex patterns.
