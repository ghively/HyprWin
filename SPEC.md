# SPEC.md — HyprTile Tiling Window Manager

## Project Metadata
- **Name**: hyprtile
- **Version**: 0.1.0
- **License**: BSD-3-Clause
- **Language**: Rust (edition 2024)
- **Targets**: Windows 10 (1903+), Windows 11
- **Architecture**: Single daemon executable with Win32 hooks

## Cargo.toml Dependencies

```toml
[package]
name = "hyprtile"
version = "0.1.0"
edition = "2024"
license = "BSD-3-Clause"
authors = ["HyprTile Contributors"]
description = "A Hyprland-inspired tiling window manager for Windows"
build = "build.rs"

[[bin]]
name = "hyprtile"
path = "src/main.rs"

[lib]
name = "hyprtile"
path = "src/lib.rs"

[dependencies]
# Win32 API
windows = { version = "0.59", features = [
    "Win32_Foundation",
    "Win32_Graphics_Dwm",
    "Win32_Graphics_Gdi",
    "Win32_System_LibraryLoader",
    "Win32_System_Performance",
    "Win32_System_Threading",
    "Win32_UI_HiDpi",
    "Win32_UI_Input_KeyboardAndMouse",
    "Win32_UI_Shell",
    "Win32_UI_WindowsAndMessaging",
    "Win32_System_WindowsProgramming",
    "Win32_Security",
    "Win32_System_Pipes",
    "Win32_Storage_FileSystem",
    "Win32_System_SystemServices",
] }

# Async runtime
 tokio = { version = "1.44", features = ["full"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"

# Config
toml = "0.8"

# File watching (hot reload)
notify = "7.0"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "fmt"] }

# CLI
clap = { version = "4.5", features = ["derive"] }

# Error handling
thiserror = "2.0"
anyhow = "1.0"

# Regex for window rules
regex = "1.11"

# Time
chrono = "0.4"

# IPC
tokio-util = { version = "0.7", features = ["codec"] }
bytes = "1.10"

# System tray
tray-icon = "0.19"

# Global hotkeys
global-hotkey = "0.6"

# Single instance
daisyuit = "0.1"

[dev-dependencies]
mockall = "0.13"
tempfile = "3.19"

[profile.release]
opt-level = 3
lto = true
strip = true
panic = "abort"

[profile.dev]
opt-level = 0
debug = true

[[test]]
name = "integration_tests"
path = "tests/integration_tests.rs"
```

## Project Structure

```
hyprtile/
├── Cargo.toml
├── build.rs
├── README.md
├── docs/
│   ├── CONFIGURATION.md
│   └── IPC_PROTOCOL.md
├── src/
│   ├── main.rs              # Entry point, CLI args
│   ├── lib.rs               # Library root
│   ├── app.rs               # Main application coordinator
│   ├── config/
│   │   ├── mod.rs           # Config loading, validation, hot-reload
│   │   ├── defaults.rs      # Default configuration values
│   │   └── types.rs         # Config data structures
│   ├── platform/
│   │   ├── mod.rs           # Platform abstraction
│   │   ├── window.rs        # Window operations
│   │   ├── monitor.rs       # Monitor enumeration and DPI
│   │   ├── events.rs        # WinEventHook
│   │   ├── dwm.rs           # DWM API wrappers
│   │   └── input.rs         # Raw input, hotkey
│   ├── layout/
│   │   ├── mod.rs           # Layout coordinator
│   │   ├── bsp.rs           # BSP tree
│   │   ├── dwindle.rs       # Dwindle layout
│   │   ├── master_stack.rs  # Master-stack layout
│   │   ├── monocle.rs       # Monocle layout
│   │   ├── grid.rs          # Grid layout
│   │   └── gaps.rs          # Gap calculation
│   ├── workspace/
│   │   ├── mod.rs           # Workspace manager
│   │   └── model.rs         # Workspace data model
│   ├── window/
│   │   ├── mod.rs           # Window state manager
│   │   ├── model.rs         # Window struct and state machine
│   │   ├── filter.rs        # Window filtering
│   │   └── rules.rs         # Window rules
│   ├── ipc/
│   │   ├── mod.rs           # IPC server
│   │   ├── protocol.rs      # JSON command definitions
│   │   └── commands.rs      # Command handlers
│   └── util/
│       ├── rect.rs          # Rectangle math
│       ├── dpi.rs           # DPI utilities
│       └── animation.rs     # Animation
├── tests/
│   ├── integration_tests.rs
│   └── fixtures/
└── resources/
    └── icon.ico
```

## Module Specifications

### 1. util::rect

**File**: `src/util/rect.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Rect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

impl Rect {
    pub fn new(x: i32, y: i32, width: i32, height: i32) -> Self;
    pub fn from_win32(rect: &windows::Win32::Foundation::RECT) -> Self;
    pub fn to_win32(&self) -> windows::Win32::Foundation::RECT;
    pub fn contains(&self, point: (i32, i32)) -> bool;
    pub fn intersects(&self, other: &Rect) -> bool;
    pub fn inset(&self, amount: i32) -> Rect;  // For gap reduction
    pub fn split_horizontal(&self, ratio: f64) -> (Rect, Rect);
    pub fn split_vertical(&self, ratio: f64) -> (Rect, Rect);
    pub fn area(&self) -> i32;
    pub fn center(&self) -> (i32, i32);
    pub fn is_empty(&self) -> bool;
    pub fn adjust_for_gaps(&self, inner: i32, outer: i32, is_single: bool) -> Rect;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Point {
    pub x: i32,
    pub y: i32,
}

impl Point {
    pub fn new(x: i32, y: i32) -> Self;
    pub fn distance_to(&self, other: &Point) -> f64;
}

pub fn rect_from_monitor_work_area(work_area: &windows::Win32::Foundation::RECT) -> Rect;
pub fn center_window_in_rect(window_size: (i32, i32), container: &Rect) -> Rect;
```

### 2. util::dpi

**File**: `src/util/dpi.rs`

```rust
/// Convert logical pixels to physical pixels for a given monitor DPI
pub fn logical_to_physical(logical: i32, dpi: u32) -> i32;

/// Convert physical pixels to logical pixels for a given monitor DPI
pub fn physical_to_logical(physical: i32, dpi: u32) -> i32;

/// Get the DPI for a specific monitor
pub fn get_monitor_dpi(hmonitor: isize) -> u32;

/// Get system DPI (fallback)
pub fn get_system_dpi() -> u32;

/// Scale a rectangle from logical to physical for a monitor
pub fn scale_rect_to_physical(rect: &super::rect::Rect, dpi: u32) -> super::rect::Rect;

/// Scale a rectangle from physical to logical for a monitor
pub fn scale_rect_to_logical(rect: &super::rect::Rect, dpi: u32) -> super::rect::Rect;
```

### 3. util::animation

**File**: `src/util/animation.rs`

```rust
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Easing {
    Linear,
    EaseOutCubic,
    EaseOutExpo,
}

impl Easing {
    pub fn apply(&self, t: f64) -> f64; // t in [0, 1], returns eased value
}

#[derive(Debug, Clone)]
pub struct Animation {
    pub duration_ms: u32,     // Total animation duration
    pub elapsed_ms: u32,      // Current elapsed time
    pub easing: Easing,
}

impl Animation {
    pub fn new(duration_ms: u32, easing: Easing) -> Self;
    pub fn tick(&mut self, delta_ms: u32) -> f64; // Returns progress [0, 1], 1.0 = done
    pub fn is_complete(&self) -> bool;
    pub fn reset(&mut self);
}

/// Interpolate between two rectangles
pub fn interpolate_rect(from: &Rect, to: &Rect, progress: f64) -> Rect;

/// Interpolate between two values
pub fn lerp(start: f64, end: f64, t: f64) -> f64;
```

### 4. config::types

**File**: `src/config/types.rs`

```rust
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub keybinds: HashMap<String, String>, // "mod+RETURN" -> "exec_terminal"
    #[serde(default)]
    pub gaps: GapsConfig,
    #[serde(default)]
    pub workspaces: WorkspacesConfig,
    #[serde(default = "default_window_rules")]
    pub window_rules: Vec<WindowRule>,
    #[serde(default)]
    pub monitors: Vec<MonitorConfig>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GeneralConfig {
    #[serde(default = "default_mod_key")]
    pub mod_key: ModKey,
    #[serde(default = "default_terminal")]
    pub terminal: String,
    #[serde(default = "default_true")]
    pub resize_on_border: bool,
    #[serde(default = "default_resize_border_width")]
    pub resize_border_width: u32,
    #[serde(default = "default_true")]
    pub auto_start: bool,
    #[serde(default)]
    pub focus_follows_mouse: bool,
    #[serde(default)]
    pub mouse_modifier: ModKey,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "UPPERCASE")]
pub enum ModKey {
    Alt,
    Win,
    Ctrl,
    Shift,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct GapsConfig {
    #[serde(default = "default_gap")]
    pub inner: u32,
    #[serde(default = "default_gap")]
    pub outer: u32,
    #[serde(default = "default_true")]
    pub smart: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WorkspacesConfig {
    #[serde(default = "default_workspace_count")]
    pub count: u32,
    #[serde(default = "default_true")]
    pub per_monitor: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct WindowRule {
    #[serde(default)]
    pub match_class: Option<String>,
    #[serde(default)]
    pub match_title: Option<String>,
    #[serde(default)]
    pub match_process: Option<String>,
    #[serde(default)]
    pub action: Option<WindowAction>,
    #[serde(default)]
    pub workspace: Option<u32>,
    #[serde(default)]
    pub monitor: Option<u32>,
    #[serde(default)]
    pub size: Option<[u32; 2]>,
    #[serde(default)]
    pub position: Option<String>, // "bottom_right", "center", etc.
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum WindowAction {
    Float,
    Tile,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct MonitorConfig {
    pub id: u32,
    #[serde(default)]
    pub workspaces: Vec<u32>,
    #[serde(default = "default_layout")]
    pub default_layout: String,
}

fn default_mod_key() -> ModKey { ModKey::Alt }
fn default_terminal() -> String { "wezterm.exe".to_string() }
fn default_gap() -> u32 { 8 }
fn default_workspace_count() -> u32 { 10 }
fn default_true() -> bool { true }
fn default_resize_border_width() -> u32 { 8 }
fn default_layout() -> String { "dwindle".to_string() }
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
```

### 5. config::defaults

**File**: `src/config/defaults.rs`

```rust
use super::types::*;
use std::collections::HashMap;

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

pub fn default_keybinds() -> HashMap<String, String> {
    let mut map = HashMap::new();
    map.insert("mod+RETURN".to_string(), "exec_terminal".to_string());
    map.insert("mod+Q".to_string(), "close_window".to_string());
    map.insert("mod+LEFT".to_string(), "focus_left".to_string());
    map.insert("mod+RIGHT".to_string(), "focus_right".to_string());
    map.insert("mod+UP".to_string(), "focus_up".to_string());
    map.insert("mod+DOWN".to_string(), "focus_down".to_string());
    map.insert("mod+SHIFT+LEFT".to_string(), "move_left".to_string());
    map.insert("mod+SHIFT+RIGHT".to_string(), "move_right".to_string());
    map.insert("mod+SHIFT+UP".to_string(), "move_up".to_string());
    map.insert("mod+SHIFT+DOWN".to_string(), "move_down".to_string());
    map.insert("mod+T".to_string(), "toggle_float".to_string());
    map.insert("mod+F".to_string(), "toggle_fullscreen".to_string());
    map.insert("mod+M".to_string(), "cycle_layout".to_string());
    map.insert("mod+R".to_string(), "reload_config".to_string());
    map.insert("mod+SHIFT+E".to_string(), "exit".to_string());
    // Workspaces 1-0
    for i in 1..=9 {
        map.insert(format!("mod+{}", i), format!("workspace_{}", i));
    }
    map.insert("mod+0".to_string(), "workspace_10".to_string());
    for i in 1..=9 {
        map.insert(format!("mod+SHIFT+{}", i), format!("move_to_workspace_{}", i));
    }
    map.insert("mod+SHIFT+0".to_string(), "move_to_workspace_10".to_string());
    map
}

fn default_window_rules_vec() -> Vec<WindowRule> {
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
```

### 6. config::mod

**File**: `src/config/mod.rs`

```rust
pub mod types;
pub mod defaults;

use types::*;
use std::path::{Path, PathBuf};
use tracing::{info, warn, error};
use notify::{Config as NotifyConfig, RecommendedWatcher, RecursiveMode, Watcher};
use std::sync::{Arc, RwLock, mpsc::channel};

pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    config_path: PathBuf,
    _watcher: Option<RecommendedWatcher>,
}

impl ConfigManager {
    pub fn new() -> anyhow::Result<Self>;
    pub fn load() -> anyhow::Result<Config>;
    pub fn load_from_path(path: &Path) -> anyhow::Result<Config>;
    pub fn get_config_path() -> PathBuf; // %APPDATA%\hyprtile\hyprtile.toml
    pub fn ensure_default_config() -> anyhow::Result<PathBuf>;
    pub fn get(&self) -> std::sync::LockResult<std::sync::RwLockReadGuard<Config>>;
    pub fn reload(&self) -> anyhow::Result<()>;
    pub fn start_watching(&mut self) -> anyhow::Result<()>;
    pub fn validate(config: &Config) -> Vec<String>; // Returns list of validation errors
}

pub fn config_dir() -> PathBuf;
pub fn config_file_path() -> PathBuf;
```

### 7. platform::window

**File**: `src/platform/window.rs`

```rust
use windows::Win32::Foundation::{HWND, RECT, BOOL, LPARAM};
use windows::Win32::UI::WindowsAndMessaging::*;
use crate::util::rect::Rect;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub isize); // HWND wrapper

impl WindowId {
    pub fn as_raw(&self) -> HWND;
    pub fn from_raw(hwnd: HWND) -> Self;
    pub fn is_valid(&self) -> bool;
    pub fn is_visible(&self) -> bool;
    pub fn is_iconic(&self) -> bool;
    pub fn is_zoomed(&self) -> bool;
    pub fn get_rect(&self) -> Option<Rect>;
    pub fn get_title(&self) -> String;
    pub fn get_class_name(&self) -> String;
    pub fn get_process_name(&self) -> String;
    pub fn is_cloaked(&self) -> bool;
    pub fn is_uwp_host(&self) -> bool; // ApplicationFrameHost.exe
    pub fn is_tool_window(&self) -> bool;
    pub fn should_manage(&self) -> bool;
}

/// Check if window should be managed by the tiling WM
pub fn should_manage_window(hwnd: HWND) -> bool;

/// Check if window is a system window (taskbar, etc.)
pub fn is_system_window(hwnd: HWND) -> bool;

/// Check if window is cloaked by DWM
pub fn is_window_cloaked(hwnd: HWND) -> bool;

/// Get window extended style
pub fn get_window_ex_style(hwnd: HWND) -> WINDOW_EX_STYLE;

/// Get window style
pub fn get_window_style(hwnd: HWND) -> WINDOW_STYLE;

/// Enumerate all top-level windows
pub fn enumerate_windows() -> Vec<WindowId>;

/// Set window position (individual)
pub fn set_window_pos(hwnd: HWND, rect: &Rect, flags: SET_WINDOW_POS_FLAGS);

/// Begin deferred window positioning
pub struct DeferredPositioner {
    hdwp: Option<HDWP>,
}

impl DeferredPositioner {
    pub fn new(count: i32) -> Self;
    pub fn defer(&mut self, hwnd: HWND, rect: &Rect, flags: SET_WINDOW_POS_FLAGS) -> bool;
    pub fn commit(self) -> bool;
}

/// Get extended frame bounds for accurate sizing
pub fn get_extended_frame_bounds(hwnd: HWND) -> Option<Rect>;

/// Remove thick frame style from tiled window
pub fn remove_thick_frame(hwnd: HWND);

/// Restore thick frame style for floating window
pub fn restore_thick_frame(hwnd: HWND);

/// Close window
pub fn close_window(hwnd: HWND);

/// Bring window to foreground
pub fn focus_window(hwnd: HWND);

/// Set window style
pub fn set_window_style(hwnd: HWND, style: WINDOW_STYLE);

/// Set window extended style
pub fn set_window_ex_style(hwnd: HWND, style: WINDOW_EX_STYLE);

/// Check if window is fullscreen
pub fn is_fullscreen(hwnd: HWND, monitor_rect: &Rect) -> bool;

/// Show/hide window
pub fn show_window(hwnd: HWND, show: bool);
```

### 8. platform::monitor

**File**: `src/platform/monitor.rs`

```rust
use windows::Win32::Foundation::{RECT, LPARAM, BOOL};
use windows::Win32::Graphics::Gdi::*;
use crate::util::rect::Rect;

#[derive(Debug, Clone)]
pub struct Monitor {
    pub handle: isize, // HMONITOR
    pub id: u32,
    pub rect: Rect,         // Full monitor rect
    pub work_area: Rect,    // Work area (minus taskbar)
    pub dpi: u32,
    pub is_primary: bool,
    pub name: String,
}

impl Monitor {
    pub fn from_hmonitor(hmonitor: isize) -> Option<Self>;
    pub fn contains_window(&self, hwnd: isize) -> bool;
    pub fn work_area_with_gaps(&self, outer_gaps: u32) -> Rect;
}

/// Enumerate all connected monitors
pub fn enumerate_monitors() -> Vec<Monitor>;

/// Get the monitor containing a specific window
pub fn get_monitor_for_window(hwnd: isize) -> Option<Monitor>;

/// Get primary monitor
pub fn get_primary_monitor() -> Option<Monitor>;

/// Get monitor by ID
pub fn get_monitor_by_id(id: u32) -> Option<Monitor>;

/// Register for display change notifications
pub fn register_display_change_notification() -> anyhow::Result<()>;

/// Set per-monitor DPI awareness
pub fn set_dpi_awareness();
```

### 9. platform::events

**File**: `src/platform/events.rs`

```rust
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Accessibility::*;
use windows::Win32::UI::WindowsAndMessaging::*;
use crate::platform::window::WindowId;
use std::sync::mpsc::Sender;

#[derive(Debug, Clone, PartialEq)]
pub enum WindowEvent {
    WindowCreated(WindowId),
    WindowDestroyed(WindowId),
    WindowShown(WindowId),
    WindowHidden(WindowId),
    WindowMinimized(WindowId),
    WindowRestored(WindowId),
    WindowMoved(WindowId),         // User-initiated move
    WindowResized(WindowId),       // User-initiated resize
    WindowFocused(WindowId),
    WindowRenamed(WindowId),
    MonitorChanged,
    DpiChanged,
    ExplorerRestarted,
}

pub struct EventHook {
    hook: HWINEVENTHOOK,
}

impl EventHook {
    pub fn register(event_tx: Sender<WindowEvent>) -> anyhow::Result<Self>;
    pub fn unregister(&self);
}

/// WinEventHook callback (extern "system")
pub extern "system" fn event_hook_callback(
    hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    id_event_thread: u32,
    dwms_event_time: u32,
);

/// Process and classify raw Win32 events into WindowEvent types
pub fn classify_event(event: u32, hwnd: HWND) -> Option<WindowEvent>;

/// Start the event processing loop (runs in its own thread)
pub fn start_event_loop(event_tx: Sender<WindowEvent>) -> anyhow::Result<()>;

/// Debounce rapid events
pub struct EventDebouncer {
    threshold_ms: u64,
    max_events: usize,
}

impl EventDebouncer {
    pub fn new(threshold_ms: u64, max_events: usize) -> Self;
    pub fn should_debounce(&mut self, event_count: usize, elapsed_ms: u64) -> bool;
}
```

### 10. platform::dwm

**File**: `src/platform/dwm.rs`

```rust
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::*;

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderColors {
    pub focused: u32,    // ARGB
    pub unfocused: u32,  // ARGB
}

impl Default for BorderColors {
    fn default() -> Self {
        BorderColors {
            focused: 0xFF00FF00,   // Green
            unfocused: 0xFF808080, // Gray
        }
    }
}

/// Set DWM border color for a window
pub fn set_border_color(hwnd: isize, color: u32) -> anyhow::Result<()>;

/// Enable/disable DWM transitions
pub fn set_transitions_enabled(hwnd: isize, enabled: bool) -> anyhow::Result<()>;

/// Force disable transitions
pub fn force_disable_transitions(hwnd: isize) -> anyhow::Result<()>;

/// Set corner preference (rounded vs square)
pub fn set_corner_preference(hwnd: isize, rounded: bool) -> anyhow::Result<()>;

/// Set border width using undocumented API (fallback)
pub fn set_border_width(hwnd: isize, width: i32) -> anyhow::Result<()>;

/// Enable DWM rendering on the window
pub fn enable_dwm_rendering(hwnd: isize) -> anyhow::Result<()>;

/// Extend frame into client area for border rendering
pub fn extend_frame_into_client(hwnd: isize, margins: &MARGINS) -> anyhow::Result<()>;

/// Check if DWM border color is supported (Windows 11+)
pub fn is_border_color_supported() -> bool;

/// Get the current DWM composition state
pub fn is_composition_enabled() -> bool;
```

### 11. platform::input

**File**: `src/platform/input.rs`

```rust
use crate::config::types::ModKey;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hotkey {
    pub modifiers: Vec<ModKey>,
    pub key: String, // "RETURN", "Q", "1", "LEFT", etc.
    pub action: String,
}

pub struct HotkeyManager {
    hotkeys: HashMap<u32, Hotkey>, // id -> hotkey
    next_id: u32,
}

impl HotkeyManager {
    pub fn new() -> Self;
    pub fn register(&mut self, hotkey: Hotkey) -> anyhow::Result<u32>;
    pub fn unregister(&mut self, id: u32) -> anyhow::Result<()>;
    pub fn unregister_all(&mut self);
    pub fn handle_message(&self, wparam: usize, lparam: isize) -> Option<&Hotkey>;
    pub fn reload_hotkeys(&mut self, keybinds: &HashMap<String, String>) -> anyhow::Result<()>;
}

/// Convert string key name to virtual key code
pub fn key_name_to_vk(key: &str) -> Option<u32>;

/// Parse keybind string like "mod+SHIFT+Q" into modifiers and key
pub fn parse_keybind(keybind: &str, mod_key: &ModKey) -> Option<Hotkey>;

/// Register all hotkeys from config
pub fn register_all_hotkeys(
    manager: &mut HotkeyManager,
    keybinds: &HashMap<String, String>,
    mod_key: &ModKey,
) -> anyhow::Result<()>;

/// Convert ModKey to Win32 modifier bits
pub fn mod_key_to_bits(mod_key: &ModKey) -> u32;

/// Set up message loop for hotkey handling
pub fn run_message_loop(hotkey_tx: std::sync::mpsc::Sender<String>) -> anyhow::Result<()>;
```

### 12. platform::mod

**File**: `src/platform/mod.rs`

```rust
pub mod window;
pub mod monitor;
pub mod events;
pub mod dwm;
pub mod input;
```

### 13. layout::bsp

**File**: `src/layout/bsp.rs`

```rust
use crate::platform::window::WindowId;
use crate::util::rect::Rect;

#[derive(Debug, Clone)]
pub enum SplitDirection {
    Horizontal,
    Vertical,
}

#[derive(Debug, Clone)]
pub enum Node {
    Split {
        direction: SplitDirection,
        ratio: f64,       // 0.0 - 1.0 (left/top portion)
        left: Box<Node>,
        right: Box<Node>,
    },
    Window {
        window_id: WindowId,
    },
    Empty,
}

impl Node {
    pub fn new() -> Self;
    pub fn is_empty(&self) -> bool;
    pub fn window_count(&self) -> usize;
    pub fn contains_window(&self, window_id: WindowId) -> bool;
    pub fn insert_window(&mut self, window_id: WindowId, direction: SplitDirection);
    pub fn remove_window(&mut self, window_id: WindowId) -> bool;
    pub fn find_window_node(&mut self, window_id: WindowId) -> Option<&mut Node>;
    pub fn traverse<F>(&self, rect: &Rect, callback: &mut F)
    where
        F: FnMut(WindowId, Rect);
    pub fn rebalance_ratios(&mut self);
    pub fn get_split_at_point(&self, rect: &Rect, point: (i32, i32)) -> Option<(SplitDirection, f64)>;
    pub fn adjust_ratio(&mut self, point: (i32, i32), delta: f64) -> bool;
}

/// Build a BSP tree from a list of windows using the dwindle algorithm
pub fn build_dwindle_tree(windows: &[WindowId]) -> Node;

/// Build a BSP tree from a list of windows with custom split direction
pub fn build_tree_with_direction(windows: &[WindowId], direction: SplitDirection) -> Node;

/// Remove a window and rebalance the tree
pub fn remove_and_rebalance(root: &mut Node, window_id: WindowId) -> bool;
```

### 14. layout::dwindle

**File**: `src/layout/dwindle.rs`

```rust
use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use super::bsp::{Node, SplitDirection, build_dwindle_tree};
use super::gaps::apply_gaps;

pub struct DwindleLayout;

impl DwindleLayout {
    pub fn name() -> &'static str { "dwindle" }
    
    /// Calculate window positions for dwindle layout
    pub fn calculate(
        windows: &[WindowId],
        workspace_rect: &Rect,
        inner_gaps: i32,
        outer_gaps: i32,
        smart_gaps: bool,
    ) -> Vec<(WindowId, Rect)> {
        if windows.is_empty() {
            return vec![];
        }
        
        let effective_rect = if smart_gaps && windows.len() == 1 {
            workspace_rect.clone()
        } else {
            apply_gaps(workspace_rect, outer_gaps)
        };
        
        let tree = build_dwindle_tree(windows);
        let mut results = Vec::new();
        tree.traverse(&effective_rect, &mut |win_id, rect| {
            let gap_adjusted = if smart_gaps && windows.len() == 1 {
                rect
            } else {
                apply_gaps(&rect, inner_gaps)
            };
            results.push((win_id, gap_adjusted));
        });
        results
    }
}
```

### 15. layout::master_stack

**File**: `src/layout/master_stack.rs`

```rust
use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use super::gaps::apply_gaps;

#[derive(Debug, Clone)]
pub struct MasterStackConfig {
    pub master_count: usize,
    pub master_width_factor: f64, // 0.1 - 0.9
    pub orientation: Orientation,
}

#[derive(Debug, Clone)]
pub enum Orientation {
    Horizontal, // Master on left, stack on right
    Vertical,   // Master on top, stack on bottom
}

impl Default for MasterStackConfig {
    fn default() -> Self {
        MasterStackConfig {
            master_count: 1,
            master_width_factor: 0.5,
            orientation: Orientation::Horizontal,
        }
    }
}

pub struct MasterStackLayout;

impl MasterStackLayout {
    pub fn name() -> &'static str { "master_stack" }
    
    pub fn calculate(
        windows: &[WindowId],
        workspace_rect: &Rect,
        inner_gaps: i32,
        outer_gaps: i32,
        smart_gaps: bool,
        config: &MasterStackConfig,
    ) -> Vec<(WindowId, Rect)>;
}
```

### 16. layout::monocle

**File**: `src/layout/monocle.rs`

```rust
use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use super::gaps::apply_gaps;

pub struct MonocleLayout;

impl MonocleLayout {
    pub fn name() -> &'static str { "monocle" }
    
    /// All windows get full workspace rect (stacked, only focused visible)
    pub fn calculate(
        windows: &[WindowId],
        workspace_rect: &Rect,
        inner_gaps: i32,
        outer_gaps: i32,
        smart_gaps: bool,
        focused_idx: usize,
    ) -> Vec<(WindowId, Rect)>;
}
```

### 17. layout::grid

**File**: `src/layout/grid.rs`

```rust
use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use super::gaps::apply_gaps;

pub struct GridLayout;

impl GridLayout {
    pub fn name() -> &'static str { "grid" }
    
    /// Distribute windows in a grid (equal rows/columns)
    pub fn calculate(
        windows: &[WindowId],
        workspace_rect: &Rect,
        inner_gaps: i32,
        outer_gaps: i32,
        smart_gaps: bool,
    ) -> Vec<(WindowId, Rect)>;
}

/// Calculate optimal grid dimensions (rows, cols) for n windows
fn calculate_grid_dimensions(count: usize) -> (usize, usize);
```

### 18. layout::gaps

**File**: `src/layout/gaps.rs`

```rust
use crate::util::rect::Rect;

/// Apply gap reduction to a rectangle
/// Reduces the rect by `gap` pixels on all sides
pub fn apply_gaps(rect: &Rect, gap: i32) -> Rect;

/// Apply outer gaps to workspace rect
pub fn apply_outer_gaps(rect: &Rect, gap: i32) -> Rect;

/// Apply inner gaps between tiled windows
pub fn apply_inner_gaps(rect: &Rect, gap: i32) -> Rect;

/// Smart gaps: return true if gaps should be disabled
pub fn should_disable_gaps(window_count: usize, smart_gaps: bool) -> bool;

/// Calculate effective gaps considering smart gaps
pub fn effective_gaps(window_count: usize, inner: i32, outer: i32, smart: bool) -> (i32, i32);
```

### 19. layout::mod

**File**: `src/layout/mod.rs`

```rust
pub mod bsp;
pub mod dwindle;
pub mod master_stack;
pub mod monocle;
pub mod grid;
pub mod gaps;

use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use crate::config::types::GapsConfig;
use std::fmt;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum LayoutType {
    Dwindle,
    MasterStack,
    Monocle,
    Grid,
}

impl LayoutType {
    pub fn all() -> Vec<LayoutType> {
        vec![LayoutType::Dwindle, LayoutType::MasterStack, LayoutType::Monocle, LayoutType::Grid]
    }
    
    pub fn name(&self) -> &'static str {
        match self {
            LayoutType::Dwindle => "dwindle",
            LayoutType::MasterStack => "master_stack",
            LayoutType::Monocle => "monocle",
            LayoutType::Grid => "grid",
        }
    }
    
    pub fn from_name(name: &str) -> Option<LayoutType> {
        match name {
            "dwindle" => Some(LayoutType::Dwindle),
            "master_stack" => Some(LayoutType::MasterStack),
            "monocle" => Some(LayoutType::Monocle),
            "grid" => Some(LayoutType::Grid),
            _ => None,
        }
    }
    
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

/// Layout calculation result: list of (window_id, target_rect)
pub type LayoutResult = Vec<(WindowId, Rect)>;

/// Calculate layout for given parameters
pub fn calculate_layout(
    layout: LayoutType,
    windows: &[WindowId],
    workspace_rect: &Rect,
    gaps: &GapsConfig,
    focused_idx: usize, // for monocle
) -> LayoutResult;

/// Layout engine coordinator
pub struct LayoutEngine {
    current_layout: LayoutType,
}

impl LayoutEngine {
    pub fn new() -> Self;
    pub fn current(&self) -> LayoutType;
    pub fn cycle(&mut self) -> LayoutType;
    pub fn set_layout(&mut self, layout: LayoutType);
    pub fn calculate(&self, windows: &[WindowId], workspace_rect: &Rect, gaps: &GapsConfig) -> LayoutResult;
}
```

### 20. workspace::model

**File**: `src/workspace/model.rs`

```rust
use crate::platform::window::WindowId;
use crate::layout::{LayoutEngine, LayoutType};
use std::collections::HashSet;

#[derive(Debug, Clone)]
pub struct Workspace {
    pub id: u32,
    pub name: String,
    pub windows: Vec<WindowId>,
    pub layout_engine: LayoutEngine,
    pub focused_window: Option<WindowId>,
}

impl Workspace {
    pub fn new(id: u32) -> Self;
    pub fn is_empty(&self) -> bool;
    pub fn add_window(&mut self, window: WindowId) -> bool;
    pub fn remove_window(&mut self, window: WindowId) -> bool;
    pub fn contains(&self, window: WindowId) -> bool;
    pub fn focus_window(&mut self, window: WindowId) -> bool;
    pub fn get_focused_index(&self) -> usize;
    pub fn cycle_focus(&mut self, direction: FocusDirection);
    pub fn get_window_index(&self, window: WindowId) -> Option<usize>;
    pub fn move_focus(&mut self, from: WindowId, to: WindowId);
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusDirection {
    Left,
    Right,
    Up,
    Down,
    Next,
    Previous,
}

#[derive(Debug, Clone)]
pub struct MonitorWorkspace {
    pub monitor_id: u32,
    pub active_workspace: u32,
    pub workspaces: Vec<Workspace>,
}

impl MonitorWorkspace {
    pub fn new(monitor_id: u32) -> Self;
    pub fn get_active_workspace(&self) -> &Workspace;
    pub fn get_active_workspace_mut(&mut self) -> &mut Workspace;
    pub fn get_workspace(&self, id: u32) -> Option<&Workspace>;
    pub fn get_workspace_mut(&mut self, id: u32) -> Option<&mut Workspace>;
    pub fn switch_workspace(&mut self, id: u32) -> bool;
    pub fn ensure_workspace(&mut self, id: u32) -> &mut Workspace;
}
```

### 21. workspace::mod

**File**: `src/workspace/mod.rs`

```rust
pub mod model;

use model::*;
use crate::platform::window::WindowId;
use crate::platform::monitor::Monitor;
use crate::util::rect::Rect;
use std::collections::HashMap;

pub struct WorkspaceManager {
    monitors: HashMap<u32, MonitorWorkspace>,
    window_to_workspace: HashMap<WindowId, u32>, // track which workspace each window is on
    window_to_monitor: HashMap<WindowId, u32>,   // track which monitor each window is on
}

impl WorkspaceManager {
    pub fn new() -> Self;
    pub fn init_monitors(&mut self, monitors: &[Monitor]);
    pub fn add_monitor(&mut self, monitor: &Monitor);
    pub fn remove_monitor(&mut self, monitor_id: u32);
    pub fn get_active_workspace(&self, monitor_id: u32) -> Option<&Workspace>;
    pub fn get_active_workspace_mut(&mut self, monitor_id: u32) -> Option<&mut Workspace>;
    pub fn switch_workspace(&mut self, monitor_id: u32, workspace_id: u32) -> anyhow::Result<()>;
    pub fn move_window_to_workspace(&mut self, window: WindowId, workspace_id: u32) -> anyhow::Result<()>;
    pub fn move_window_to_monitor(&mut self, window: WindowId, monitor_id: u32) -> anyhow::Result<()>;
    pub fn add_window(&mut self, window: WindowId, monitor_id: u32) -> anyhow::Result<()>;
    pub fn remove_window(&mut self, window: WindowId) -> Option<(u32, u32)>; // Returns (monitor_id, workspace_id)
    pub fn get_window_location(&self, window: WindowId) -> Option<(u32, u32)>; // (monitor_id, workspace_id)
    pub fn handle_monitor_disconnect(&mut self, monitor_id: u32, fallback_monitor: u32);
    pub fn get_all_windows(&self) -> Vec<WindowId>;
    pub fn get_workspace_for_window(&self, window: WindowId) -> Option<u32>;
    pub fn cycle_focus(&mut self, monitor_id: u32, direction: FocusDirection);
}
```

### 22. window::model

**File**: `src/window/model.rs`

```rust
use crate::platform::window::WindowId;
use crate::util::rect::Rect;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum WindowState {
    Tiling,
    Floating,
    Maximized,
    Fullscreen,
    Minimized,
}

#[derive(Debug, Clone)]
pub struct Window {
    pub id: WindowId,
    pub state: WindowState,
    pub previous_state: Option<WindowState>,
    pub class_name: String,
    pub title: String,
    pub process_name: String,
    pub floating_rect: Option<Rect>, // Remember position when floating
    pub is_managed: bool,
    pub is_uwp: bool,
    pub is_electron: bool,
    pub is_tool: bool,
}

impl Window {
    pub fn new(id: WindowId) -> Self;
    pub fn refresh_info(&mut self);
    pub fn set_state(&mut self, new_state: WindowState);
    pub fn toggle_float(&mut self) -> WindowState;
    pub fn toggle_fullscreen(&mut self) -> WindowState;
    pub fn minimize(&mut self);
    pub fn restore(&mut self);
    pub fn should_tile(&self) -> bool;
    pub fn is_visible_and_managed(&self) -> bool;
    pub fn matches_rule(&self, rule: &crate::config::types::WindowRule) -> bool;
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
```

### 23. window::filter

**File**: `src/window/filter.rs`

```rust
use crate::platform::window::WindowId;

/// Filter out windows that should never be managed
pub fn should_manage(hwnd: WindowId) -> bool;

/// Check if window is a system/process window
pub fn is_system_window(hwnd: WindowId) -> bool;

/// Check if window is a tool window (no taskbar button)
pub fn is_tool_window(hwnd: WindowId) -> bool;

/// Check if window is cloaked by DWM
pub fn is_cloaked(hwnd: WindowId) -> bool;

/// Check if window is a UWP host window
pub fn is_uwp_host(hwnd: WindowId) -> bool;

/// Check if window is an Electron app
pub fn is_electron(hwnd: WindowId) -> bool;

/// Check if window is visible and on screen
pub fn is_visible_and_normal(hwnd: WindowId) -> bool;

/// Combined filter: all checks
pub fn passes_all_filters(hwnd: WindowId) -> bool;

/// List of known system window classes to exclude
pub fn system_window_classes() -> Vec<&'static str>;

/// List of known process names to exclude
pub fn excluded_processes() -> Vec<&'static str>;
```

### 24. window::rules

**File**: `src/window/rules.rs`

```rust
use crate::config::types::{WindowRule, WindowAction};
use crate::window::model::Window;
use regex::Regex;

pub struct RuleEngine {
    rules: Vec<WindowRule>,
}

impl RuleEngine {
    pub fn new(rules: Vec<WindowRule>) -> Self;
    pub fn apply_rules(&self, window: &mut Window);
    pub fn find_matching_rules(&self, window: &Window) -> Vec<&WindowRule>;
    pub fn should_float(&self, window: &Window) -> bool;
    pub fn target_workspace(&self, window: &Window) -> Option<u32>;
    pub fn target_monitor(&self, window: &Window) -> Option<u32>;
}

/// Check if a window matches a rule
pub fn window_matches_rule(window: &Window, rule: &WindowRule) -> bool;

/// Check class match with regex
pub fn class_matches(class: &str, pattern: &str) -> bool;

/// Check title match with regex
pub fn title_matches(title: &str, pattern: &str) -> bool;

/// Check process match (exact or regex)
pub fn process_matches(process: &str, pattern: &str) -> bool;
```

### 25. window::mod

**File**: `src/window/mod.rs`

```rust
pub mod model;
pub mod filter;
pub mod rules;

use model::*;
use rules::RuleEngine;
use crate::platform::window::WindowId;
use crate::config::types::Config;
use std::collections::HashMap;
use tracing::{info, debug, warn};

pub struct WindowManager {
    windows: HashMap<WindowId, Window>,
    rule_engine: RuleEngine,
    focused: Option<WindowId>,
}

impl WindowManager {
    pub fn new(config: &Config) -> Self;
    pub fn register_window(&mut self, hwnd: WindowId) -> Option<&Window>;
    pub fn unregister_window(&mut self, hwnd: WindowId) -> Option<Window>;
    pub fn get_window(&self, hwnd: WindowId) -> Option<&Window>;
    pub fn get_window_mut(&mut self, hwnd: WindowId) -> Option<&mut Window>;
    pub fn get_focused(&self) -> Option<WindowId>;
    pub fn set_focused(&mut self, hwnd: WindowId);
    pub fn get_all_managed(&self) -> Vec<WindowId>;
    pub fn get_tiling_windows(&self) -> Vec<WindowId>;
    pub fn get_floating_windows(&self) -> Vec<WindowId>;
    pub fn get_visible_windows(&self) -> Vec<WindowId>;
    pub fn refresh_window_info(&mut self, hwnd: WindowId);
    pub fn handle_state_change(&mut self, hwnd: WindowId, event: &crate::platform::events::WindowEvent);
    pub fn toggle_float(&mut self, hwnd: WindowId) -> Option<WindowState>;
    pub fn toggle_fullscreen(&mut self, hwnd: WindowId) -> Option<WindowState>;
    pub fn close_focused(&self);
    pub fn count(&self) -> usize;
    pub fn reload_rules(&mut self, config: &Config);
}
```

### 26. ipc::protocol

**File**: `src/ipc/protocol.rs`

```rust
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum IpcRequest {
    Workspaces { monitor: Option<u32> },
    FocusedWindow,
    Layout { monitor: Option<u32> },
    WindowCount,
    FocusDirection { direction: String },
    MoveDirection { direction: String },
    ToggleFloat,
    ToggleFullscreen,
    CycleLayout,
    SwitchWorkspace { id: u32 },
    MoveToWorkspace { id: u32 },
    ReloadConfig,
    Exit,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub success: bool,
    pub data: Option<serde_json::Value>,
    pub error: Option<String>,
}

impl IpcResponse {
    pub fn success(data: Option<serde_json::Value>) -> Self;
    pub fn error(msg: String) -> Self;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub id: u32,
    pub name: String,
    pub windows: usize,
    pub has_focus: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusedWindowInfo {
    pub id: u64,
    pub title: String,
    pub class: String,
    pub state: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutInfo {
    pub current: String,
    pub available: Vec<String>,
}
```

### 27. ipc::commands

**File**: `src/ipc/commands.rs`

```rust
use super::protocol::*;
use crate::app::AppState;

pub fn handle_command(request: IpcRequest, state: &mut AppState) -> IpcResponse;

fn handle_workspaces(args: &WorkspacesArgs, state: &AppState) -> IpcResponse;
fn handle_focused_window(state: &AppState) -> IpcResponse;
fn handle_layout(args: &LayoutArgs, state: &AppState) -> IpcResponse;
fn handle_window_count(state: &AppState) -> IpcResponse;
fn handle_focus_direction(direction: &str, state: &mut AppState) -> IpcResponse;
fn handle_move_direction(direction: &str, state: &mut AppState) -> IpcResponse;
fn handle_toggle_float(state: &mut AppState) -> IpcResponse;
fn handle_toggle_fullscreen(state: &mut AppState) -> IpcResponse;
fn handle_cycle_layout(state: &mut AppState) -> IpcResponse;
fn handle_switch_workspace(id: u32, state: &mut AppState) -> IpcResponse;
fn handle_move_to_workspace(id: u32, state: &mut AppState) -> IpcResponse;
fn handle_reload_config(state: &mut AppState) -> IpcResponse;
fn handle_exit(state: &mut AppState) -> IpcResponse;

// Helper structs for command args
#[derive(Debug, Clone)]
pub struct WorkspacesArgs { pub monitor: Option<u32> }

#[derive(Debug, Clone)]
pub struct LayoutArgs { pub monitor: Option<u32> }
```

### 28. ipc::mod

**File**: `src/ipc/mod.rs`

```rust
pub mod protocol;
pub mod commands;

use protocol::*;
use tokio::net::windows::named_pipe::*;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tracing::{info, error};
use std::sync::Arc;
use tokio::sync::Mutex;

pub const PIPE_NAME: &str = r"\\.\pipe\hyprtile";
pub const TCP_PORT: u16 = 9860;

pub struct IpcServer {
    shutdown: Arc<tokio::sync::Notify>,
}

impl IpcServer {
    pub fn new() -> Self;
    pub async fn start(&self) -> anyhow::Result<()>;
    pub async fn stop(&self);
    pub async fn handle_named_pipe_client(client: NamedPipeServer) -> anyhow::Result<IpcRequest>;
    pub async fn write_response(client: &mut NamedPipeServer, response: &IpcResponse) -> anyhow::Result<()>;
}

/// Start TCP socket server for status bar integration
pub async fn start_tcp_server(port: u16) -> anyhow::Result<()>;

/// Send command to running hyprtile instance
pub async fn send_command(command: &str) -> anyhow::Result<IpcResponse>;

/// Parse JSON request from buffer
pub fn parse_request(buf: &[u8]) -> anyhow::Result<IpcRequest>;

/// Serialize response to JSON bytes
pub fn serialize_response(response: &IpcResponse) -> Vec<u8>;
```

### 29. app.rs

**File**: `src/app.rs`

```rust
use crate::config::{ConfigManager, types::Config};
use crate::platform::{events::*, monitor::*, window::*};
use crate::layout::*;
use crate::window::{WindowManager, model::*};
use crate::workspace::{WorkspaceManager, model::*};
use crate::ipc::*;
use std::sync::{Arc, RwLock, mpsc::{channel, Sender, Receiver}};
use tracing::{info, warn, error};

pub struct AppState {
    pub config: Arc<RwLock<Config>>,
    pub window_manager: WindowManager,
    pub workspace_manager: WorkspaceManager,
    pub monitors: Vec<Monitor>,
    pub running: bool,
}

impl AppState {
    pub fn new(config: Config) -> Self;
    pub fn reload_config(&mut self, config: Config);
    pub fn get_focused_monitor(&self) -> Option<&Monitor>;
}

pub struct App {
    state: AppState,
    event_tx: Sender<WindowEvent>,
    event_rx: Receiver<WindowEvent>,
    config_manager: ConfigManager,
}

impl App {
    pub fn new() -> anyhow::Result<Self>;
    pub fn run(&mut self) -> anyhow::Result<()>;
    fn process_event(&mut self, event: WindowEvent);
    fn handle_window_created(&mut self, hwnd: WindowId);
    fn handle_window_destroyed(&mut self, hwnd: WindowId);
    fn handle_window_focused(&mut self, hwnd: WindowId);
    fn handle_window_minimized(&mut self, hwnd: WindowId);
    fn handle_window_restored(&mut self, hwnd: WindowId);
    fn handle_window_moved(&mut self, hwnd: WindowId);
    fn handle_monitor_changed(&mut self);
    fn apply_layout(&mut self, monitor_id: u32);
    fn apply_all_layouts(&mut self);
    fn handle_hotkey(&mut self, action: &str) -> anyhow::Result<()>;
    fn focus_direction(&mut self, direction: FocusDirection);
    fn move_direction(&mut self, direction: FocusDirection);
    fn switch_workspace(&mut self, id: u32);
    fn move_to_workspace(&mut self, id: u32);
    fn cycle_layout(&mut self);
    fn toggle_float(&mut self);
    fn toggle_fullscreen(&mut self);
    fn reload_config(&mut self);
    fn exit(&mut self);
}
```

### 30. lib.rs

**File**: `src/lib.rs`

```rust
pub mod app;
pub mod config;
pub mod ipc;
pub mod layout;
pub mod platform;
pub mod util;
pub mod window;
pub mod workspace;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

pub fn setup_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hyprtile=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
pub const APP_NAME: &str = "HyprTile";
pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\hyprtile";
pub const DEFAULT_TCP_PORT: u16 = 9860;
```

### 31. main.rs

**File**: `src/main.rs`

```rust
use clap::Parser;
use hyprtile::{app::App, setup_logging};
use tracing::{info, error};

#[derive(Parser, Debug)]
#[command(name = "hyprtile")]
#[command(about = "A Hyprland-inspired tiling window manager for Windows")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Run in foreground (not as daemon)
    #[arg(short, long)]
    foreground: bool,
    
    /// Config file path
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,
    
    /// Send IPC command to running instance
    #[arg(short, long)]
    command: Option<String>,
    
    /// Check configuration and exit
    #[arg(long)]
    check_config: bool,
    
    /// Print default configuration
    #[arg(long)]
    print_default_config: bool,
    
    /// Run with verbose logging
    #[arg(short, long)]
    verbose: bool,
}

fn main() {
    setup_logging();
    
    let cli = Cli::parse();
    
    if cli.print_default_config {
        let config = hyprtile::config::defaults::default_config();
        let toml = toml::to_string_pretty(&config).unwrap();
        println!("{}", toml);
        return;
    }
    
    info!("HyprTile {} starting", env!("CARGO_PKG_VERSION"));
    
    match App::new() {
        Ok(mut app) => {
            if let Err(e) = app.run() {
                error!("Application error: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("Failed to initialize: {}", e);
            std::process::exit(1);
        }
    }
}
```

### 32. build.rs

**File**: `build.rs`

```rust
use std::env;
use std::path::PathBuf;

fn main() {
    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());
    
    // Embed version info
    println!("cargo:rustc-env=VERSION={}", env!("CARGO_PKG_VERSION"));
    
    // Resource compilation would go here for icon embedding
    // On Windows, embed the icon resource
    #[cfg(windows)]
    {
        let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
        let resource_file = manifest_dir.join("resources").join("hyprtile.rc");
        if resource_file.exists() {
            println!("cargo:rerun-if-changed={}", resource_file.display());
            // Use windres or rc.exe to compile .rc file
        }
    }
    
    println!("cargo:rerun-if-changed=build.rs");
}
```

### 33. tests/integration_tests.rs

**File**: `tests/integration_tests.rs`

```rust
use hyprtile::*;

#[test]
fn test_rect_math() {
    // Test rectangle operations
}

#[test]
fn test_layout_calculations() {
    // Test each layout produces valid rects
}

#[test]
fn test_window_state_machine() {
    // Test state transitions
}

#[test]
fn test_config_parsing() {
    // Test TOML config parsing
}

#[test]
fn test_ipc_protocol() {
    // Test IPC serialization
}

#[test]
fn test_workspace_management() {
    // Test workspace CRUD
}

#[test]
fn test_window_filtering() {
    // Test window filter logic
}

#[test]
fn test_gap_calculations() {
    // Test gap math
}

#[test]
fn test_animation_easing() {
    // Test easing functions
}
```

## Data Flow

```
WinEventHook (window events)
    |
    v
Event Processor (dedup, debounce, classify)
    |
    v
Window State Manager (HWND registry, state machine)
    |
    v
Layout Engine (BSP tree -> Rect calculations)
    |
    v
DeferWindowPos batch execution
    |
    v
DWM API (borders, transitions, effects)
```

## Window State Machine

States: TILING, FLOATING, MAXIMIZED, FULLSCREEN, MINIMIZED

Transitions:
- TILING <-> FLOATING: user toggle ($mod+T), window rule match
- Any state -> MINIMIZED: user minimizes (stop managing position)
- MINIMIZED -> TILING/FLOATING: user restores (resume management)
- Any state -> FULLSCREEN: detect fullscreen mode, release from tiling
- FULLSCREEN -> previous: detect exit fullscreen, restore

## Keybind -> Action Mapping

| Keybind | Action | Handler |
|---------|--------|---------|
| $mod+RETURN | exec_terminal | Launch configured terminal |
| $mod+Q | close_window | Close focused window |
| $mod+Arrow | focus_* | Change focus directionally |
| $mod+SHIFT+Arrow | move_* | Move window directionally |
| $mod+T | toggle_float | Toggle tiling/floating |
| $mod+F | toggle_fullscreen | Toggle fullscreen |
| $mod+M | cycle_layout | Cycle through layouts |
| $mod+1-0 | workspace_* | Switch to workspace |
| $mod+SHIFT+1-0 | move_to_workspace_* | Move window to workspace |
| $mod+R | reload_config | Reload configuration |
| $mod+SHIFT+E | exit | Exit HyprTile |

## Acceptance Criteria

- All 4 layout algorithms produce correct arrangements
- All core keybindings work within 50ms
- Workspace switching <100ms for 10+ windows
- Window events processed and laid out within 100ms
- Config hot-reload <200ms
- IPC response <10ms
- CPU <1% idle, <5% active
- Memory <50MB steady state
- Handles 50+ windows without degradation
- Unit test coverage >60%
