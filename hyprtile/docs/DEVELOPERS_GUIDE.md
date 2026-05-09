# HyprTile Developer Guide

> A comprehensive guide for developers who want to extend, modify, or contribute to HyprTile.

---

## Table of Contents

1. [Architecture Overview](#1-architecture-overview)
2. [Module Reference](#2-module-reference)
3. [How to Add a New Layout Algorithm](#3-how-to-add-a-new-layout-algorithm)
4. [How to Add a New IPC Command](#4-how-to-add-a-new-ipc-command)
5. [How to Add a New Hotkey Action](#5-how-to-add-a-new-hotkey-action)
6. [Window Lifecycle Deep Dive](#6-window-lifecycle-deep-dive)
7. [Testing Guide](#7-testing-guide)
8. [Debugging Guide](#8-debugging-guide)
9. [Windows API Patterns Used](#9-windows-api-patterns-used)
10. [Contributing Guidelines](#10-contributing-guidelines)
11. [Build System](#11-build-system)
12. [Common Development Tasks](#12-common-development-tasks)

---

## 1. Architecture Overview

### Module Graph

```
                    +-------------------+
                    |   main.rs / CLI   |
                    +--------+----------+
                             |
                             v
                    +--------+----------+
                    |   app::App        |<-------------+ IPC clients
                    |   (coordinator) |<-------------+ Hotkey thread
                    +--------+----------+
                             |
        +--------------------+--------------------+
        |                    |                    |
        v                    v                    v
+-------+--------+  +--------+----------+  +------+-------+
| window::Window |  | workspace::Workspace |  | layout::Engine |
|    Manager     |  |      Manager         |  +------+-------+
+-------+--------+  +--------+----------+         |
        |                    |                      |
        v                    v                      v
+-------+--------+  +-------+--------+   +--------+----------+
| platform::window |  | platform::monitor |   | layout::algorithms |
|    (Win32 HWND)  |  |   (HMONITOR)      |   | (BSP/dwindle/...) |
+------------------+  +-----------------+   +------------------+
        ^
        |
+-------+--------+
| platform::events |
| (WinEventHook)   |
+------------------+
        |
        v
+-------+--------+
|   Win32 OS     |
+----------------+
```

### Event Flow

```
Win32 Event (SetWinEventHook)
        |
        v
+-------+--------+
| platform::events |  <-- Runs on "event-hook" thread
| classify_event()   |
+-------+--------+
        |
        v
  mpsc::Sender<WindowEvent>
        |
        v
+-------+--------+
| app::App::run()  |  <-- Main thread (blocking recv)
| process_event()  |
+-------+--------+
        |
        +--------> WindowCreated  --> register_window() + apply_layout()
        +--------> WindowDestroyed --> unregister_window() + apply_layout()
        +--------> WindowFocused  --> set_focused() + apply_border_colors()
        +--------> WindowMoved    --> toggle_float() + apply_layout()
        +--------> MonitorChanged --> reenumerate + apply_all_layouts()
        +--------> HotkeyAction   --> handle_hotkey()
        |
        v
+-------+--------+
|  apply_layout()  |
+-------+--------+
        |
        v
  layout::calculate_layout()  --> algorithm-specific rect calculation
        |
        v
  DeferredPositioner::defer()  --> batch via BeginDeferWindowPos
        |
        v
  platform::dwm::set_border_color()  --> DWMWA_BORDER_COLOR
```

### Window Lifecycle

```
+-----------+     +--------------+     +-------------+     +-----------+
|  WinEvent   | --> |   filter::   | --> |  rule_engine | --> | workspace |
|  CREATED   |     | should_manage|     | apply_rules()|     | add_window|
+-----------+     +--------------+     +-------------+     +-----------+
                                              |                  |
                                              v                  v
                                       +-------------+     +-----------+
                                       | WindowState |     | layout::  |
                                       |  Tiling/    | --> | calculate |
                                       |  Floating   |     +-----------+
                                       +-------------+
```

### Thread Model

| Thread | Name | Responsibility |
|--------|------|---------------|
| **Main thread** | `main` | Owns `AppState`, processes events from `mpsc` channel, applies layouts |
| **Event hook thread** | `event-hook` | Runs `SetWinEventHook` callback, translates Win32 events into `WindowEvent` variants, sends them via `mpsc` |
| **Hotkey thread** | `hotkey-loop` | Creates message-only window, receives `WM_HOTKEY`, forwards action strings via `mpsc` |
| **IPC server task** | `tokio::spawn` | Async named-pipe (`tokio::net::windows::named_pipe`) and TCP listener; decodes length-delimited JSON requests |
| **Config watcher** | `notify` background thread | Watches `%APPDATA%\hyprtile\` for file changes, auto-reloads config |

> **Important:** All mutable state lives on the main thread. The other threads are "event producers" only. IPC command handlers mutably borrow `AppState` on the main thread via the same channel mechanism.

---

## 2. Module Reference

### `util`

**Files:** `src/util/rect.rs`, `src/util/dpi.rs`, `src/util/animation.rs`

**Purpose:** Pure, platform-independent utilities. No Win32 dependencies except for `RECT` conversion in `rect.rs`.

| Type | Description |
|------|-------------|
| `Rect` | Axis-aligned rectangle with `x`, `y`, `width`, `height`. Core geometry type. |
| `Point` | 2D point with Euclidean distance helper. |
| `Easing` | Animation curve: `Linear`, `EaseOutCubic`, `EaseOutExpo`. |
| `Animation` | Progress tracker: `tick(delta_ms) -> f64` in `[0, 1]`. |

**Key API:**
- `Rect::split_horizontal(ratio: f64) -> (Rect, Rect)` — used by all layout algorithms
- `Rect::inset(amount: i32) -> Rect` — gap reduction
- `scale_rect_to_physical(rect, dpi)` — converts logical to physical pixels for HiDPI monitors

**Integration:** `layout` uses `Rect` for all geometry. `platform::monitor` uses DPI helpers. `animation` is wired into `app::apply_layout()` (currently a placeholder for full timer-based animation).

---

### `config`

**Files:** `src/config/mod.rs`, `src/config/types.rs`, `src/config/defaults.rs`

**Purpose:** TOML config parsing, validation, hot-reload via `notify`.

| Type | Description |
|------|-------------|
| `Config` | Root struct: `general`, `keybinds`, `gaps`, `workspaces`, `window_rules`, `monitors` |
| `ConfigManager` | Loads config, holds `Arc<RwLock<Config>>`, starts file watcher |
| `WindowRule` | Regex-based rule: `match_class`, `match_title`, `match_process` → action/workspace/monitor |
| `GapsConfig` | `inner`, `outer`, `smart` |

**Key API:**
- `ConfigManager::load() -> Result<Config>` — from `%APPDATA%\hyprtile\hyprtile.toml`
- `ConfigManager::validate(&Config) -> Vec<String>` — returns human-readable issues
- `ConfigManager::start_watching() -> Result<()>` — auto-reload on file change (100ms debounce)

**Integration:** `App::new()` creates the manager. `AppState` holds `Arc<RwLock<Config>>` so IPC handlers can read it. `WindowManager` owns a `RuleEngine` initialized from `config.window_rules`.

---

### `platform`

**Files:** `src/platform/window.rs`, `src/platform/monitor.rs`, `src/platform/events.rs`, `src/platform/dwm.rs`, `src/platform/input.rs`, `src/platform/startup.rs`, `src/platform/tray.rs`

**Purpose:** The only modules that touch Win32 APIs. All unsafe code lives here.

#### `platform::window`

| Type | Description |
|------|-------------|
| `WindowId(pub isize)` | Newtype around `HWND`. `isize` so it serializes to JSON as a number. |
| `DeferredPositioner` | Batches `SetWindowPos` via `BeginDeferWindowPos` / `DeferWindowPos` / `EndDeferWindowPos`. |

**Key API:**
- `WindowId::is_valid()`, `is_visible()`, `is_cloaked()`, `get_rect()`, `get_title()`, `get_class_name()`, `get_process_name()`
- `set_window_pos(hwnd, rect, flags)` — single window position
- `DeferredPositioner::defer(hwnd, rect, flags) -> bool` — batch add
- `DeferredPositioner::commit() -> bool` — apply all at once

#### `platform::monitor`

| Type | Description |
|------|-------------|
| `Monitor` | `handle` (HMONITOR), `id`, `rect`, `work_area`, `dpi`, `is_primary`, `name` |

**Key API:**
- `enumerate_monitors() -> Vec<Monitor>` — callback-based via `EnumDisplayMonitors`
- `set_dpi_awareness()` — sets per-monitor DPI awareness at startup

#### `platform::events`

| Type | Description |
|------|-------------|
| `WindowEvent` | 15 variants covering create/destroy/show/hide/move/resize/focus/rename/minimize/restore/monitor/dpi/explorer/hotkey |
| `EventHook` | RAII wrapper around `HWINEVENTHOOK`. Unregisters on `Drop`. |
| `EventDebouncer` | Time/count-based debouncer for rapid event sequences. |

**Key API:**
- `EventHook::register(tx) -> Result<EventHook>` — calls `SetWinEventHook` with `event_hook_callback`
- `classify_event(raw_event, hwnd) -> Option<WindowEvent>` — maps Win32 constants to our enum

#### `platform::dwm`

| Type | Description |
|------|-------------|
| `BorderColors` | `focused: u32` (ARGB), `unfocused: u32` (ARGB) |

**Key API:**
- `set_border_color(hwnd, color)` — `DWMWA_BORDER_COLOR` (Windows 11+)
- `force_disable_transitions(hwnd)` — `DWMWA_TRANSITIONS_FORCEDISABLED`
- `set_corner_preference(hwnd, rounded)` — `DWMWA_WINDOW_CORNER_PREFERENCE`
- `extend_frame_into_client(hwnd, margins)` — simulate borders on older Windows

#### `platform::input`

| Type | Description |
|------|-------------|
| `Hotkey` | `{ modifiers: Vec<ModKey>, key: String, action: String }` |
| `HotkeyManager` | `HashMap<u32, Hotkey>` keyed by registration ID |

**Key API:**
- `parse_keybind("mod+SHIFT+Q", &ModKey::Alt) -> Option<Hotkey>`
- `register_all_hotkeys(&mut manager, &keybinds, &mod_key)`
- `run_message_loop(tx) -> Result<()>` — blocking `GetMessageW` loop for `WM_HOTKEY`

---

### `layout`

**Files:** `src/layout/mod.rs`, `src/layout/bsp.rs`, `src/layout/dwindle.rs`, `src/layout/master_stack.rs`, `src/layout/monocle.rs`, `src/layout/grid.rs`, `src/layout/gaps.rs`

**Purpose:** Calculate target rectangles for tiled windows.

| Type | Description |
|------|-------------|
| `LayoutType` | Enum: `Dwindle`, `MasterStack`, `Monocle`, `Grid` |
| `LayoutEngine` | Tracks current `LayoutType`, provides `cycle()` and `set_layout()` |
| `LayoutResult` | Type alias: `Vec<(WindowId, Rect)>` |
| `Node` | BSP tree node: `Split { direction, ratio, left, right }` or `Window { window_id }` |

**Key API:**
- `calculate_layout(layout, windows, workspace_rect, gaps, focused_idx, master_width_factor) -> LayoutResult`
- `LayoutEngine::calculate(...)` — uses the engine's current layout type

**Integration:** Called from `AppState::apply_layout()` after filtering for tiling windows. Results are DPI-scaled and passed to `DeferredPositioner`.

---

### `window`

**Files:** `src/window/mod.rs`, `src/window/model.rs`, `src/window/filter.rs`, `src/window/rules.rs`

**Purpose:** Registry of all managed windows, state machine, rule application, filtering.

| Type | Description |
|------|-------------|
| `WindowManager` | `HashMap<WindowId, Window>` registry + `RuleEngine` + focused tracking |
| `Window` | Rich metadata: `id`, `state`, `class_name`, `title`, `process_name`, `floating_rect` |
| `WindowState` | `Tiling`, `Floating`, `Maximized`, `Fullscreen`, `Minimized` |
| `RuleEngine` | Holds `Vec<WindowRule>`, applies them at registration time |

**Key API:**
- `WindowManager::register_window(hwnd) -> Option<&Window>` — create, filter, apply rules, insert
- `WindowManager::get_tiling_windows() -> Vec<WindowId>` — what the layout engine needs
- `Window::toggle_float() -> WindowState` — toggles between Tiling/Floating, saves rect
- `Window::toggle_fullscreen() -> WindowState` — saves/restores previous state
- `filter::should_manage(hwnd) -> bool` — the 6-check filter (valid, visible, not system, not tool, not cloaked, not UWP host)

**Integration:** `App::handle_window_created()` calls `filter::should_manage()`, then `WindowManager::register_window()`, then `workspace_manager.add_window()`, then `apply_layout()`.

---

### `workspace`

**Files:** `src/workspace/mod.rs`, `src/workspace/model.rs`

**Purpose:** Virtual desktop management, per-monitor workspace collections, focus cycling.

| Type | Description |
|------|-------------|
| `WorkspaceManager` | `HashMap<u32, MonitorWorkspace>` + bidirectional lookups (`window_to_workspace`, `window_to_monitor`) |
| `Workspace` | `id`, `name`, `windows: Vec<WindowId>`, `layout_engine: LayoutEngine`, `focused_window` |
| `MonitorWorkspace` | `monitor_id`, `active_workspace: u32`, `workspaces: Vec<Workspace>` |
| `FocusDirection` | `Left`, `Right`, `Up`, `Down`, `Next`, `Previous` |

**Key API:**
- `WorkspaceManager::add_window(window, monitor_id)` — adds to active workspace of monitor
- `WorkspaceManager::move_window_to_workspace(window, workspace_id)` — removes from old, adds to new
- `WorkspaceManager::switch_workspace(monitor_id, workspace_id)` — changes active workspace, shows/hides windows
- `Workspace::cycle_focus(direction)` — wraps at list boundaries

**Integration:** `App::apply_layout()` fetches the active workspace per monitor, filters its windows through `WindowManager`, then calls `LayoutEngine::calculate()`.

---

### `ipc`

**Files:** `src/ipc/mod.rs`, `src/ipc/protocol.rs`, `src/ipc/commands.rs`

**Purpose:** Named pipe and TCP servers for external control (status bars, CLI, scripts).

| Type | Description |
|------|-------------|
| `IpcServer` | Named pipe server using `tokio::net::windows::named_pipe` |
| `IpcRequest` | Tagged enum: `Workspaces`, `FocusedWindow`, `Layout`, `ToggleFloat`, `SwitchWorkspace`, ... |
| `IpcResponse` | `{ success: bool, data: Option<Value>, error: Option<String> }` |
| `JsonCodec` | Length-delimited framing: `[4-byte BE length][JSON payload]` |

**Key API:**
- `ipc::send_command(pipe_path, payload) -> Result<IpcResponse>` — client side
- `handle_command(request, &mut AppState) -> IpcResponse` — server-side dispatch

**Integration:** `App::run()` spawns the TCP server via `tokio::spawn`. The named pipe server is started similarly. `main.rs` uses `send_command()` for the `--command` CLI option.

---

### `app`

**File:** `src/app.rs`

**Purpose:** The central coordinator. Owns `AppState`, wires together all subsystems, runs the main event loop.

| Type | Description |
|------|-------------|
| `AppState` | Public fields: `config`, `window_manager`, `workspace_manager`, `monitors`, `running` |
| `App` | Owns `AppState`, `event_rx`, `config_manager`, optional `tray` |

**Key API:**
- `App::new(config_path) -> Result<App>` — load config, enumerate monitors, enumerate existing windows, apply initial layout
- `App::run() -> Result<()>` — spawn threads, block on `event_rx.recv_timeout(100ms)`, dispatch events
- `AppState::apply_layout(monitor_id)` — the full layout pipeline (filter → layout → DPI scale → defer → DWM borders)
- `AppState::apply_all_layouts()` — calls `apply_layout()` for every monitor

---

## 3. How to Add a New Layout Algorithm

Let's add a **"Centered Master"** layout: one large centered window, all others stacked in a strip on the right.

### Step 1: Create the layout file

Create `src/layout/centered_master.rs`:

```rust
use crate::platform::window::WindowId;
use crate::util::rect::Rect;
use super::gaps::{apply_gaps, effective_gaps};

pub struct CenteredMasterLayout;

impl CenteredMasterLayout {
    pub fn name() -> &'static str {
        "centered_master"
    }

    pub fn calculate(
        windows: &[WindowId],
        workspace_rect: &Rect,
        inner_gaps: i32,
        outer_gaps: i32,
        smart_gaps: bool,
    ) -> Vec<(WindowId, Rect)> {
        if windows.is_empty() {
            return Vec::new();
        }

        let (effective_inner, effective_outer) =
            effective_gaps(windows.len(), inner_gaps, outer_gaps, smart_gaps);

        let effective_rect = apply_gaps(workspace_rect, effective_outer);

        if windows.len() == 1 {
            return vec![(
                windows[0],
                apply_gaps(&effective_rect, effective_inner),
            )];
        }

        // Split: 60% for master (centered), 40% for stack on the right
        let (master_area, stack_area) =
            effective_rect.split_horizontal(0.6);

        let mut results = Vec::with_capacity(windows.len());

        // Master window (first) gets centered area
        results.push((
            windows[0],
            apply_gaps(&master_area, effective_inner),
        ));

        // Stack windows share the right area, split vertically
        let stack_count = windows.len() - 1;
        let stack_height = stack_area.height / stack_count as i32;

        for (i, &win) in windows[1..].iter().enumerate() {
            let y = stack_area.y + (i as i32 * stack_height);
            let h = if i == stack_count - 1 {
                // Last window gets remaining height
                stack_area.y + stack_area.height - y
            } else {
                stack_height
            };

            let rect = Rect::new(stack_area.x, y, stack_area.width, h);
            results.push((win, apply_gaps(&rect, effective_inner)));
        }

        results
    }
}
```

### Step 2: Export from `layout/mod.rs`

Add `pub mod centered_master;` near the top of `src/layout/mod.rs`.

### Step 3: Add to `LayoutType` enum

In `src/layout/mod.rs`:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutType {
    Dwindle,
    MasterStack,
    Monocle,
    Grid,
    CenteredMaster,   // <-- NEW
}
```

Update `name()`, `from_name()`, `next()`, and `all()`:

```rust
impl LayoutType {
    pub fn all() -> Vec<LayoutType> {
        vec![
            LayoutType::Dwindle,
            LayoutType::MasterStack,
            LayoutType::Monocle,
            LayoutType::Grid,
            LayoutType::CenteredMaster,  // <-- NEW
        ]
    }

    pub fn name(&self) -> &'static str {
        match self {
            LayoutType::Dwindle => "dwindle",
            LayoutType::MasterStack => "master_stack",
            LayoutType::Monocle => "monocle",
            LayoutType::Grid => "grid",
            LayoutType::CenteredMaster => "centered_master",  // <-- NEW
        }
    }

    pub fn from_name(name: &str) -> Option<LayoutType> {
        match name {
            "dwindle" => Some(LayoutType::Dwindle),
            "master_stack" => Some(LayoutType::MasterStack),
            "monocle" => Some(LayoutType::Monocle),
            "grid" => Some(LayoutType::Grid),
            "centered_master" => Some(LayoutType::CenteredMaster),  // <-- NEW
            _ => None,
        }
    }

    pub fn next(&self) -> LayoutType {
        match self {
            LayoutType::Dwindle => LayoutType::MasterStack,
            LayoutType::MasterStack => LayoutType::Monocle,
            LayoutType::Monocle => LayoutType::Grid,
            LayoutType::Grid => LayoutType::CenteredMaster,  // <-- NEW
            LayoutType::CenteredMaster => LayoutType::Dwindle,  // <-- NEW
        }
    }
}
```

### Step 4: Wire into `calculate_layout()` dispatcher

In `src/layout/mod.rs`, add the match arm:

```rust
pub fn calculate_layout(
    layout: LayoutType,
    windows: &[WindowId],
    workspace_rect: &Rect,
    gaps: &GapsConfig,
    focused_idx: usize,
    master_width_factor: f64,
) -> LayoutResult {
    // ... existing code ...
    match layout {
        // ... existing variants ...
        LayoutType::CenteredMaster => {
            centered_master::CenteredMasterLayout::calculate(
                windows, workspace_rect, inner, outer, gaps.smart
            )
        }
    }
}
```

### Step 5: Add tests

Add tests at the bottom of `src/layout/centered_master.rs`:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn wid(n: isize) -> WindowId {
        WindowId(n)
    }

    #[test]
    fn test_centered_master_empty() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let result = CenteredMasterLayout::calculate(&[], &workspace, 0, 0, false);
        assert!(result.is_empty());
    }

    #[test]
    fn test_centered_master_single() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1)];
        let result = CenteredMasterLayout::calculate(&windows, &workspace, 0, 0, false);
        assert_eq!(result.len(), 1);
        assert_eq!(result[0].1.width, 1000);
    }

    #[test]
    fn test_centered_master_multiple() {
        let workspace = Rect::new(0, 0, 1000, 600);
        let windows = vec![wid(1), wid(2), wid(3)];
        let result = CenteredMasterLayout::calculate(&windows, &workspace, 0, 0, false);
        assert_eq!(result.len(), 3);
        // Master gets ~60% width
        assert_eq!(result[0].1.width, 600);
        // Stack gets ~40% width
        assert_eq!(result[1].1.width, 400);
    }
}
```

Also add a `calculate_layout` test in `src/layout/mod.rs` under `#[cfg(test)]`:

```rust
#[test]
fn test_calculate_layout_centered_master() {
    let workspace = Rect::new(0, 0, 1000, 600);
    let windows = vec![wid(1), wid(2)];
    let gaps = test_gaps();
    let result = calculate_layout(LayoutType::CenteredMaster, &windows, &workspace, &gaps, 0, 0.5);
    assert_eq!(result.len(), 2);
    for (_, rect) in &result {
        assert!(rect.width > 0);
        assert!(rect.height > 0);
    }
}
```

### Step 6: Document in `CONFIGURATION.md`

Add a row to the layout table in `docs/CONFIGURATION.md`:

```markdown
| `centered_master` | One large centered window, rest stacked on the right | 60/40 split |
```

---

## 4. How to Add a New IPC Command

Let's add a **`KillWindow`** command that kills the focused window by process instead of just closing it.

### Step 1: Add variant to `IpcRequest`

In `src/ipc/protocol.rs`:

```rust
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum IpcRequest {
    // ... existing variants ...
    KillWindow,   // <-- NEW
}
```

### Step 2: Add handler in `commands.rs`

In `src/ipc/commands.rs`:

```rust
pub fn handle_command(request: IpcRequest, state: &mut AppState) -> IpcResponse {
    match request {
        // ... existing arms ...
        IpcRequest::KillWindow => handle_kill_window(state),
    }
}

// Add the handler function:
fn handle_kill_window(state: &mut AppState) -> IpcResponse {
    let focused_id = match state.window_manager.get_focused() {
        Some(id) => id,
        None => return IpcResponse::error("No focused window".to_string()),
    };

    let window = match state.window_manager.get_window(focused_id) {
        Some(w) => w,
        None => return IpcResponse::error("Focused window not in registry".to_string()),
    };

    let process_name = window.process_name.clone();
    info!("Killing process: {}", process_name);

    // Use taskkill to terminate the process
    match std::process::Command::new("taskkill")
        .args(["/F", "/IM", &process_name])
        .spawn()
    {
        Ok(_) => IpcResponse::success(Some(serde_json::json!({ "process": process_name }))),
        Err(e) => IpcResponse::error(format!("Failed to kill process: {}", e)),
    }
}
```

### Step 3: Already wired via `match` in `handle_command()`

No additional wiring needed — the `match` in `handle_command` dispatches it.

### Step 4: Add serialization test

In `tests/integration_tests.rs` (or `src/ipc/mod.rs` under `#[cfg(test)]`):

```rust
#[test]
fn test_ipc_kill_window_command() {
    let req = IpcRequest::KillWindow;
    let json = serde_json::to_string(&req).unwrap();
    assert_eq!(json, r#"{"command":"kill_window"}"#);

    let deserialized: IpcRequest = serde_json::from_str(&json).unwrap();
    match deserialized {
        IpcRequest::KillWindow => {},
        _ => panic!("Expected KillWindow command"),
    }
}
```

### Step 5: Document in `IPC_PROTOCOL.md`

In `docs/IPC_PROTOCOL.md`, add:

```markdown
### `kill_window`

Kills the process of the currently focused window using `taskkill /F /IM`.

**Request:**
```json
{ "command": "kill_window" }
```

**Response:**
```json
{ "success": true, "data": { "process": "notepad.exe" } }
```
```

---

## 5. How to Add a New Hotkey Action

Let's add **`resize_increase`** and **`resize_decrease`** actions (they already exist in the code — this shows how you would add a new one like `reload_rules_only`).

### Step 1: Add keybind to default config

In `src/config/defaults.rs`:

```rust
pub fn default_keybinds() -> HashMap<String, String> {
    let mut map = HashMap::new();
    // ... existing bindings ...
    map.insert("mod+SHIFT+R".to_string(), "reload_rules_only".to_string());
    map
}
```

### Step 2: Add handler in `App::handle_hotkey()`

In `src/app.rs`, in the `handle_hotkey()` method:

```rust
pub fn handle_hotkey(&mut self, action: &str) -> anyhow::Result<()> {
    match action {
        // ... existing arms ...
        "reload_rules_only" => {
            self.reload_rules_only();
            Ok(())
        }
        // ...
    }
}
```

Add the helper method:

```rust
fn reload_rules_only(&mut self) {
    let config = match self.state.config.read() {
        Ok(c) => c.clone(),
        Err(_) => {
            warn!("Config lock poisoned, cannot reload rules");
            return;
        }
    };
    self.state.window_manager.reload_rules(&config);
    info!("Window rules reloaded without full config reload");
}
```

### Step 3: Document in `CONFIGURATION.md`

Add to the keybind table:

```markdown
| `mod+SHIFT+R` | `reload_rules_only` | Reload only window rules (faster than full config reload) |
```

---

## 6. Window Lifecycle Deep Dive

### State Machine

```
                  +----------+
                  |  TILING  |<---------------------------+
                  +----+-----+                            |
                       |                                 |
            $mod+T / rule(float)                        |
                       |                                 |
                       v                                 |
                  +----------+      $mod+T               |
         +------->| FLOATING |---------------------------+
         |        +----+-----+
         |             |
         |    $mod+F / user fullscreen
         |             |
         |             v
         |        +----------+      $mod+F / exit fullscreen
         |        | FULLSCREEN |--------------------------+
         |        +----+-----+                             |
         |             |                                   |
         |     user minimize                               |
         |             |                                   |
         |             v                                   |
         |        +----------+      user restore             |
         |        | MINIMIZED|------------------------------+
         |        +----------+                            |
         |                                                |
         |     user maximize                              |
         |             |                                  |
         |             v                                  |
         |        +----------+      user un-maximize       |
         +-------| MAXIMIZED |----------------------------->+
                  +----------+
```

### Discovery Pipeline

When a `WindowCreated` event fires, this is the exact sequence:

```
1. WinEventHook callback receives EVENT_OBJECT_CREATE
        |
        v
2. classify_event() converts to WindowEvent::WindowCreated(hwnd)
        |
        v
3. Sent via mpsc to main thread
        |
        v
4. App::process_event() routes to handle_window_created(hwnd)
        |
        v
5. filter::should_manage(hwnd) -- 6 checks:
   a. hwnd.is_valid()            -- HWND != NULL and IsWindow()
   b. is_visible_and_normal()   -- WS_VISIBLE && !IsIconic()
   c. is_system_window()        -- class_name not in system list
   d. is_tool_window()           -- !WS_EX_TOOLWINDOW
   e. is_cloaked()               -- DWMWA_CLOAKED != 1
   f. is_uwp_host()              -- class != "ApplicationFrameWindow"
        |
        v
6. should_manage_window(hwnd)   -- platform::window duplicate check
        |
        v
7. WindowManager::register_window(hwnd)
   a. filter::should_manage() again (defense in depth)
   b. Window::new(hwnd)          -- queries class, title, process
   c. RuleEngine::apply_rules()  -- regex matching on class/title/process
   d. Insert into HashMap<WindowId, Window>
        |
        v
8. Determine target monitor
   a. Check which monitor's rect contains the window
   b. Fallback to primary monitor
        |
        v
9. WorkspaceManager::add_window(hwnd, monitor_id)
   a. Get active workspace on that monitor
   b. Push WindowId to workspace.windows Vec
   c. Set focused_window if it was None
   d. Update window_to_workspace and window_to_monitor maps
        |
        v
10. AppState::apply_layout(monitor_id)
    a. Get active workspace
    b. Filter windows: state == Tiling && is_visible_and_managed()
    c. calculate_layout() with current LayoutType
    d. DPI scale if monitor.dpi != 96
    e. DeferredPositioner batch
    f. DWM border colors
```

### Rule Matching Details

The `RuleEngine` uses **AND logic** across fields and **OR logic** across rules:

```rust
// A window matches a rule only if ALL present fields match:
// match_class   AND match_title AND match_process
//
// If a field is None, it is skipped (acts as "always true" for that field).
//
// The rule must have at least one matcher (empty rules match nothing).
```

Regex patterns are compiled with `Regex::new()`. If compilation fails, a literal string comparison is used as fallback.

Example rule evaluation:

```rust
let rule = WindowRule {
    match_class: Some(".*-steam-.*".to_string()),
    match_title: None,
    match_process: None,
    action: Some(WindowAction::Float),
    // ...
};
// Matches any window whose class_name contains "-steam-"
// (e.g. "Chrome_WidgetWin_1" for Steam's browser, NOT "Steam" itself)
```

---

## 7. Testing Guide

### Running Tests

```bash
# Run all tests (unit + integration)
cargo test

# Run with logging visible
RUST_LOG=hyprtile=debug cargo test -- --nocapture

# Run only integration tests
cargo test --test integration_tests

# Run a specific test
cargo test test_dwindle_single_window

# Run tests in a specific module
cargo test layout::
```

### Adding a New Integration Test

Open `tests/integration_tests.rs`. Add a new test function following the existing patterns:

```rust
// ============================================================================
// 11. My New Feature Tests
// ============================================================================

#[test]
fn test_my_new_feature() {
    use hyprtile::platform::window::WindowId;

    // Create test data — use fake WindowIds (any isize works in tests)
    let workspace = Rect::new(0, 0, 1920, 1080);
    let windows = vec![WindowId(1001), WindowId(1002)];

    // Call the code under test
    let result = DwindleLayout::calculate(&windows, &workspace, 8, 8, false);

    // Assert expectations
    assert_eq!(result.len(), 2);
    for (_, rect) in &result {
        assert!(rect.width > 0, "width must be positive");
        assert!(rect.height > 0, "height must be positive");
    }
}
```

### Mock Patterns

Since `WindowId` wraps an `isize` (the raw HWND), you can use any integer in tests:

```rust
fn fake_window_id(n: isize) -> WindowId {
    WindowId(n)
}
```

For testing code that requires `Window` structs, construct them directly:

```rust
use hyprtile::window::model::{Window, WindowState};
use hyprtile::platform::window::WindowId;

let mut window = Window::new(WindowId(42));
window.class_name = "MyApp".to_string();
window.title = "Hello".to_string();
window.process_name = "myapp.exe".to_string();
window.state = WindowState::Tiling;
```

> **Note:** Tests that call real Win32 APIs (like `Window::new()`) will fail on non-Windows platforms because `get_class_name()` and friends call `GetClassNameW`. The existing integration tests avoid calling `Window::new()` where possible, or use `WindowId` directly with layout algorithms (which are pure functions).

### Test Fixtures

The `tests/fixtures/test_config.toml` file provides a sample configuration for parser tests:

```rust
#[test]
fn test_fixture_config() {
    let path = std::path::PathBuf::from("tests/fixtures/test_config.toml");
    let config = hyprtile::config::ConfigManager::load_from_path(&path).unwrap();
    assert_eq!(config.workspaces.count, 10);
}
```

---

## 8. Debugging Guide

### Logging Levels

```bash
# Default: info level for hyprtile, warn for everything else
RUST_LOG=hyprtile=info cargo run

# Debug mode: verbose event tracing
RUST_LOG=hyprtile=debug cargo run

# Trace mode: every WinEvent and IPC message
RUST_LOG=hyprtile=trace cargo run

# Also show windows-rs crate warnings
RUST_LOG=hyprtile=debug,windows=warn cargo run

# Log to a file instead of stderr
RUST_LOG=hyprtile=debug cargo run 2> hyprtile.log
```

### Tracing Event Flow

Enable `hyprtile=trace` to see the full event pipeline:

```
TRACE classify_event: Classified event: WindowCreated(WindowId(0x123456)) for hwnd=0x123456
DEBUG Received event: WindowCreated(WindowId(0x123456))
DEBUG Window created: WindowId(0x123456)
DEBUG Window 123456 failed management filter, skipping
```

Or for a successfully managed window:

```
DEBUG Window created: WindowId(0x789ABC)
DEBUG Window 789ABC (class='Chrome_WidgetWin_1', title='GitHub') registered on monitor 0
DEBUG Applying layout Dwindle to monitor 0 workspace 1 (3 tiling windows)
DEBUG   Positioning window WindowId(0x789ABC) at Rect { x: 0, y: 0, width: 960, height: 1080 }
```

### Common Issues and Solutions

| Symptom | Cause | Solution |
|---------|-------|----------|
| Windows not tiling | Filter rejecting them | Check `trace` logs for `failed filter`. Verify class isn't in `system_window_classes()` |
| Explorer windows tiled | `explorer.exe` not excluded | Check `excluded_processes()` or add a window rule |
| DWM borders not showing | On Windows 10 or older Win11 | `DWMWA_BORDER_COLOR` requires build 22000+. Check `is_border_color_supported()` |
| High CPU usage | Rapid event storm | Enable `EventDebouncer` in `process_event()` |
| Layout not updating | `apply_layout()` skipped | Check if workspace has 0 tiling windows (debug log will say) |
| Hotkeys not working | Another app registered them | Run as Administrator; check `RegisterHotKey` error in logs |
| Config not reloading | Watcher not started | Call `config_manager.start_watching()` in `App::new()` |

### Runtime Inspection via IPC

```bash
# Check current layout
hyprtile --command '{"command":"layout"}'

# Check workspaces
hyprtile --command '{"command":"workspaces"}'

# Check focused window
hyprtile --command '{"command":"focused_window"}'

# Count managed windows
hyprtile --command '{"command":"window_count"}'
```

These are read-only commands and safe to run at any time.

---

## 9. Windows API Patterns Used

### windows-rs vs Raw Win32

HyprTile uses the **`windows`** crate (version 0.59) which provides:
- **Type-safe wrappers** for `HWND`, `HMONITOR`, `RECT`, etc.
- **Enumerated constants** as Rust types (e.g., `DWMWA_BORDER_COLOR` instead of raw `35`)
- **`Result<T>` returns** from most functions instead of checking `GetLastError()` manually

Example comparison:

```rust
// Raw Win32 (C-style)
HWND hwnd = GetForegroundWindow();
if (hwnd == NULL) { /* error */ }

// windows-rs
let hwnd = unsafe { GetForegroundWindow() };
if hwnd.is_invalid() { /* error */ }
```

### Safe Wrappers for Unsafe Operations

Every `unsafe` block is confined to `platform/` modules. The pattern is:

```rust
// In platform::window
pub fn get_title(&self) -> String {
    unsafe {
        let mut buf = [0u16; 512];
        let len = GetWindowTextW(self.as_raw(), &mut buf);
        String::from_utf16_lossy(&buf[..len as usize])
    }
}
```

The `unsafe` is **not** exposed in public APIs. Callers of `WindowId::get_title()` see a safe function.

### Error Handling Patterns

Two patterns coexist:

1. **`anyhow::Result<()>`** for operations that "just need to report failure" (most platform calls)
2. **`thiserror` derived errors** for specific error types (not heavily used in current codebase; `anyhow` is preferred for simplicity)

Example:

```rust
pub fn set_border_color(hwnd: isize, color: u32) -> anyhow::Result<()> {
    let hwnd = HWND(hwnd);
    if hwnd.is_invalid() {
        anyhow::bail!("Invalid HWND");
    }
    unsafe {
        let result = DwmSetWindowAttribute(hwnd, DWMWA_BORDER_COLOR, ...);
        if result.is_ok() {
            Ok(())
        } else {
            anyhow::bail!("DwmSetWindowAttribute failed");
        }
    }
}
```

### DPI Awareness Implementation

HyprTile sets **per-monitor DPI awareness v2** at startup:

```rust
// In app::App::new()
set_dpi_awareness(); // calls SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2)
```

All layout calculations happen in **logical pixels** (96 DPI baseline). Before calling `DeferredPositioner::defer()`, rectangles are scaled:

```rust
// In AppState::apply_layout()
let monitor_dpi = monitor.dpi;
let positions: Vec<(WindowId, Rect)> = if monitor_dpi != BASE_DPI {
    positions.into_iter()
        .map(|(wid, rect)| (wid, scale_rect_to_physical(&rect, monitor_dpi)))
        .collect()
} else {
    positions
};
```

This ensures tiled windows are the correct size on 125%, 150%, and 200% DPI displays.

---

## 10. Contributing Guidelines

### Code Style

- **Follow existing patterns.** If you add a new module, mirror the structure of an existing one (e.g., copy `src/layout/dwindle.rs` as a template).
- **Use `tracing`** for all logging: `info!`, `warn!`, `error!`, `debug!`, `trace!`. Never use `println!` in library code.
- **Document public items** with `///` doc comments.
- **Keep `unsafe` in `platform/` only.** Business logic in `app/`, `layout/`, `window/` should be 100% safe Rust.
- **Use `anyhow::Result<()>`** for error propagation unless you need a specific error type.

### PR Checklist

Before submitting a pull request:

- [ ] `cargo test` passes (all unit + integration tests)
- [ ] `cargo clippy -- -D warnings` passes with no warnings
- [ ] `cargo fmt` has been run
- [ ] New code has doc comments
- [ ] New public APIs have examples in doc comments
- [ ] If adding a layout: tests added in both the algorithm file and `layout/mod.rs`
- [ ] If adding an IPC command: serialization test added
- [ ] If adding a hotkey action: default config updated, `handle_hotkey()` arm added

### Documentation Requirements

- **Algorithm changes:** Update `docs/CONFIGURATION.md` with the new layout name and description.
- **IPC changes:** Update `docs/IPC_PROTOCOL.md` with request/response examples.
- **Config changes:** Update `docs/CONFIGURATION.md` with new TOML keys and defaults.

### Test Requirements

- Every new layout algorithm must have at least: empty-input test, single-window test, multi-window test.
- Every new IPC command must have a round-trip serialization test.
- Every new `WindowEvent` variant must have a `classify_event()` test.
- Aim for >60% line coverage (reported by `cargo tarpaulin` if installed).

---

## 11. Build System

### How to Build

```bash
# Debug build (fast compile, no optimizations)
cargo build

# Release build (optimized, LTO, stripped)
cargo build --release

# The release binary will be at:
# target/release/hyprtile.exe
```

### Profile Settings

From `Cargo.toml`:

```toml
[profile.release]
opt-level = 3      # Maximum optimization
lto = true         # Link-time optimization (slower build, smaller + faster binary)
strip = true       # Remove debug symbols
panic = "abort"    # Don't unwind on panic (smaller binary)

[profile.dev]
opt-level = 0      # No optimization (fastest compile)
debug = true       # Include debug info
```

### Feature Flags

Currently HyprTile has no Cargo features. All functionality is compiled unconditionally. If you add optional features (e.g., `tracing` integration, animation support), follow this pattern:

```toml
[features]
default = []
animation = ["dep:tokio"]  # example
```

### Resource Compilation

`build.rs` handles Windows resource compilation for icon embedding:

```rust
#[cfg(windows)]
{
    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    let resource_file = manifest_dir.join("resources").join("hyprtile.rc");
    if resource_file.exists() {
        println!("cargo:rerun-if-changed={}", resource_file.display());
        // Use windres or rc.exe to compile .rc file
    }
}
```

To embed an icon:
1. Place `icon.ico` in `resources/`
2. Create `resources/hyprtile.rc`:
   ```
   IDI_ICON1 ICON "icon.ico"
   ```
3. Install `windres` (via MSYS2) or use `rc.exe` from Windows SDK
4. Uncomment the resource compilation logic in `build.rs`

---

## 12. Common Development Tasks

### "I want to change the default gap size"

Edit `src/config/types.rs` and `src/config/defaults.rs`:

```rust
// In types.rs
fn default_gap() -> u32 {
    12  // was 8
}

// In defaults.rs
GapsConfig {
    inner: 12,
    outer: 12,
    smart: true,
}
```

Run `cargo test` to verify defaults tests still pass, then update `docs/CONFIGURATION.md`.

### "I want to add a new default keybind"

Edit `src/config/defaults.rs`:

```rust
pub fn default_keybinds() -> HashMap<String, String> {
    let mut map = HashMap::new();
    // ... existing bindings ...
    map.insert("mod+S".to_string(), "toggle_sticky".to_string());
    map
}
```

Add the handler in `src/app.rs` `handle_hotkey()`, then add a test in `src/config/defaults.rs` under `#[cfg(test)]`.

### "I want to change the focus border color"

Edit `src/platform/dwm.rs`:

```rust
impl Default for BorderColors {
    fn default() -> Self {
        BorderColors {
            focused: 0xFF00BFFF,   // Deep Sky Blue (was green)
            unfocused: 0xFF404040, // Dark gray (was gray)
        }
    }
}
```

The format is **ARGB** (Alpha-Red-Green-Blue). `0xFF` = fully opaque.

### "I want to add support for a new window type"

Example: You want HyprTile to manage **XAML Islands** windows that are currently filtered out.

1. Check the window class name by adding temporary logging:

```rust
// Temporary: in filter.rs
pub fn should_manage(hwnd: WindowId) -> bool {
    let class = hwnd.get_class_name();
    tracing::info!("Checking window class: {}", class);
    // ... rest of checks
}
```

2. Run HyprTile, open the app, and read the log to find the class name (e.g., `Windows_XAMLHost`).

3. If it's a system window that should be excluded, add to `system_window_classes()` in `filter.rs`.

4. If it's a window that should be managed but isn't, check why it's being rejected:
   - Is it cloaked? → Check `is_cloaked()` logic
   - Is it a tool window? → May need special handling
   - Is it a child window? → May need to check parent relationship

5. For complex cases, add a window rule in the default config instead:

```rust
// In config/types.rs default_window_rules()
WindowRule {
    match_class: Some("Windows_XAMLHost".to_string()),
    match_title: None,
    match_process: None,
    action: Some(WindowAction::Tile),  // Force tiling
    workspace: None,
    monitor: None,
    size: None,
    position: None,
},
```

---

## Quick Reference: File Map

| Concern | File(s) |
|---------|---------|
| Entry point / CLI | `src/main.rs` |
| Event loop coordinator | `src/app.rs` |
| Window registry + state machine | `src/window/mod.rs`, `src/window/model.rs` |
| Window filtering | `src/window/filter.rs` |
| Window rules (regex) | `src/window/rules.rs` |
| Workspace + virtual desktops | `src/workspace/mod.rs`, `src/workspace/model.rs` |
| Layout dispatcher | `src/layout/mod.rs` |
| BSP tree | `src/layout/bsp.rs` |
| Layout algorithms | `src/layout/dwindle.rs`, `master_stack.rs`, `monocle.rs`, `grid.rs` |
| Gap calculation | `src/layout/gaps.rs` |
| Geometry primitives | `src/util/rect.rs` |
| DPI scaling | `src/util/dpi.rs` |
| Animation | `src/util/animation.rs` |
| Config loading / validation | `src/config/mod.rs` |
| Config data structures | `src/config/types.rs` |
| Default config values | `src/config/defaults.rs` |
| IPC protocol | `src/ipc/protocol.rs` |
| IPC server | `src/ipc/mod.rs` |
| IPC command handlers | `src/ipc/commands.rs` |
| Win32 window ops | `src/platform/window.rs` |
| Win32 monitor ops | `src/platform/monitor.rs` |
| WinEventHook | `src/platform/events.rs` |
| DWM API | `src/platform/dwm.rs` |
| Hotkey registration | `src/platform/input.rs` |
| Integration tests | `tests/integration_tests.rs` |
| Config fixture | `tests/fixtures/test_config.toml` |

---

*This guide is a living document. If you find something unclear or outdated, please open an issue or PR.*
