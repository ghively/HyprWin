# HyprTile — A Hyprland-Inspired Tiling Window Manager for Windows

<p align="center">
  <img src="https://raw.githubusercontent.com/hyprtile/hyprtile/main/resources/icon.png" width="128" height="128" alt="HyprTile Logo">
</p>

<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-BSD--3--Clause-blue.svg" alt="License: BSD-3-Clause"></a>
  <a href="https://www.rust-lang.org"><img src="https://img.shields.io/badge/built%20with-Rust-orange.svg" alt="Built with Rust"></a>
  <a href="#installation"><img src="https://img.shields.io/badge/platform-Windows%2010%2F11-0078D6.svg" alt="Platform: Windows 10/11"></a>
  <a href="https://github.com/hyprtile/hyprtile/releases"><img src="https://img.shields.io/github/v/release/hyprtile/hyprtile.svg" alt="Latest Release"></a>
</p>

<p align="center">
  <b>Automatic tiling, dynamic layouts, and keyboard-driven window management for Windows.</b>
</p>

---

HyprTile brings the power and flexibility of modern Linux tiling window managers to Windows. Inspired by [Hyprland](https://hyprland.org/), it automatically arranges your windows into efficient layouts, eliminates manual resizing, and puts keyboard control at the center of your workflow. Whether you're a developer juggling terminals and browsers, a power user managing dozens of windows, or someone looking to reclaim screen real estate, HyprTile adapts to how you work.

Built in Rust for performance and reliability, HyprTile integrates deeply with the Windows Desktop Window Manager (DWM) to provide colored window borders, smooth transitions, and native-feeling behavior. With per-monitor workspaces, four distinct layout algorithms, a comprehensive IPC system for status bar integration, and configuration hot-reload, HyprTile offers a complete tiling experience without the overhead of a Linux VM or WSL.

## Key Features

### Four Tiling Layouts

| Layout | Description | Best For |
|--------|-------------|----------|
| **Dwindle** | Recursive binary space partitioning (BSP) that alternates split direction at each level | General purpose, most flexible |
| **Master Stack** | One or more master windows on the left/top with a stack of remaining windows | Reading + reference workflows |
| **Monocle** | All windows maximized and stacked (only focused window visible) | Focus work, presentations |
| **Grid** | Equal-sized grid of rows and columns | Monitoring many windows at once |

### Multi-Monitor Support
- Independent workspaces per monitor or shared across all monitors
- Automatic monitor detection with DPI awareness
- Hot-plug handling for monitor connect/disconnect
- Per-monitor layout persistence

### DWM-Powered Visuals
- Colored window borders (focused/unfocused) via native DWM API
- Configurable border width and colors
- Smooth animated transitions between layout changes
- Corner preference support (rounded vs. square)

### Configuration
- **Hot-reload**: Edit `hyprtile.toml` and changes apply instantly
- TOML-based configuration with sensible defaults
- Window rules with regex matching (class, title, process name)
- Per-monitor configuration and workspace assignment
- Customizable gaps (inner, outer, smart gaps)

### IPC & Extensibility
- **Named Pipe**: `\\.\pipe\hyprtile` for local clients
- **TCP Socket**: `localhost:9860` for status bar integration
- 13 IPC commands for full remote control
- JSON request/response protocol
- Example clients in Rust and Python provided

## Installation

### Prerequisites

- **Windows 10** (version 1903 or later) or **Windows 11**
- **Visual C++ Redistributable 2019+** (usually already installed)
- (Optional) [WezTerm](https://wezfurlong.org/wezterm/) or another terminal emulator

### Option 1: Install from crates.io (Recommended)

```powershell
cargo install hyprtile
```

Ensure `%USERPROFILE%\.cargo\bin` is in your PATH, then run:

```powershell
hyprtile.exe
```

### Option 2: Build from Source

```powershell
# Clone the repository
git clone https://github.com/hyprtile/hyprtile.git
cd hyprtile

# Build in release mode
cargo build --release

# The binary will be at:
# .\target\release\hyprtile.exe

# Run directly
.\target\release\hyprtile.exe
```

### Option 3: Pre-built Binary

Download the latest release from the [Releases](https://github.com/hyprtile/hyprtile/releases) page and place `hyprtile.exe` in a directory of your choice.

### Auto-Start (Optional)

To start HyprTile automatically on login, create a shortcut in your Startup folder:

```powershell
$startup = "$env:APPDATA\Microsoft\Windows\Start Menu\Programs\Startup"
$hyprtile = (Get-Command hyprtile.exe).Source
Copy-Item $hyprtile $startup
```

Or add `auto_start = true` to the `[general]` section of your config.

## Configuration

### Quick Start

On first launch, HyprTile creates a default configuration file at:

```
%APPDATA%\hyprtile\hyprtile.toml
```

Edit this file to customize keybinds, gaps, window rules, and more. Changes are applied automatically without restarting.

### Minimal Config Example

```toml
[general]
mod_key = "ALT"          # ALT, WIN, CTRL, or SHIFT
terminal = "wezterm.exe" # Your preferred terminal

[gaps]
inner = 8
outer = 8

[keybinds]
"mod+RETURN" = "exec_terminal"
"mod+Q"      = "close_window"
"mod+T"      = "toggle_float"
"mod+F"      = "toggle_fullscreen"
"mod+M"      = "cycle_layout"
"mod+R"      = "reload_config"

[[window_rules]]
match_class = ".*-steam-.*"
action = "float"
```

For a complete configuration reference, see [docs/CONFIGURATION.md](docs/CONFIGURATION.md).

## Keybinds Reference

### Default Keybindings

The `$mod` key is configurable (default: `ALT`).

| Keybind | Action | Description |
|---------|--------|-------------|
| `$mod + Enter` | `exec_terminal` | Launch configured terminal |
| `$mod + Q` | `close_window` | Close the focused window |
| `$mod + Left` | `focus_left` | Move focus left |
| `$mod + Right` | `focus_right` | Move focus right |
| `$mod + Up` | `focus_up` | Move focus up |
| `$mod + Down` | `focus_down` | Move focus down |
| `$mod + Shift + Left` | `move_left` | Move window left |
| `$mod + Shift + Right` | `move_right` | Move window right |
| `$mod + Shift + Up` | `move_up` | Move window up |
| `$mod + Shift + Down` | `move_down` | Move window down |
| `$mod + T` | `toggle_float` | Toggle tiling/floating for focused window |
| `$mod + F` | `toggle_fullscreen` | Toggle fullscreen for focused window |
| `$mod + M` | `cycle_layout` | Cycle to next layout (Dwindle -> Master Stack -> Monocle -> Grid) |
| `$mod + R` | `reload_config` | Reload configuration from disk |
| `$mod + Shift + E` | `exit` | Exit HyprTile |
| `$mod + 1-9` | `workspace_N` | Switch to workspace N (1-9) |
| `$mod + 0` | `workspace_10` | Switch to workspace 10 |
| `$mod + Shift + 1-9` | `move_to_workspace_N` | Move focused window to workspace N |
| `$mod + Shift + 0` | `move_to_workspace_10` | Move focused window to workspace 10 |

### Mouse Actions

| Action | Description |
|--------|-------------|
| `$mod + Click + Drag` | Move floating window |
| `$mod + Right-Click + Drag` | Resize floating window |
| Drag window to screen edge | Move window to adjacent monitor |

## Architecture

### System Overview

```
+-----------------------------------------------------------------------+
|                           HyprTile System                             |
+-----------------------------------------------------------------------+
|                                                                       |
|  +------------------+     +------------------+     +----------------+ |
|  |   WinEventHook   |     |   HotkeyManager  |     |   IPC Server   | |
|  |                  |     |                  |     |                | |
|  |  - Window create |     |  - Global hooks  |     |  - Named pipe  | |
|  |  - Window destroy|     |  - Key parsing   |     |  - TCP socket  | |
|  |  - Focus changes |     |  - Action dispatch|    |  - JSON proto  | |
|  |  - Min/Max/Restore|    |                  |     |                | |
|  +--------+---------+     +--------+---------+     +--------+-------+ |
|           |                        |                        |         |
|           v                        v                        v         |
|  +--------+---------+     +--------+---------+     +--------+-------+ |
|  |  Event Processor |     |   Action Router  |     | Command Handler| |
|  |                  |     |                  |     |                | |
|  |  - Deduplicate   |     |  - Parse keybind |     |  - 13 commands | |
|  |  - Debounce      |     |  - Dispatch      |     |  - Query/ctrl  | |
|  |  - Classify      |     |  - Execute       |     |  - Status bar  | |
|  +--------+---------+     +--------+---------+     +--------+-------+ |
|           |                        |                        |         |
|           v                        v                        v         |
|  +--------+---------+     +--------+---------+     +--------+-------+ |
|  | Window Manager   |     | Config Manager   |     |  Layout Engine | |
|  |                  |     |                  |     |                | |
|  |  - HWND registry |     |  - TOML parse    |     |  - Dwindle     | |
|  |  - State machine |     |  - Validation    |     |  - MasterStack | |
|  |  - Rule engine   |     |  - Hot reload    |     |  - Monocle     | |
|  |  - Focus tracking|     |  - File watch    |     |  - Grid        | |
|  +--------+---------+     +------------------+     +--------+-------+ |
|           |                                                 |         |
|           v                                                 v         |
|  +--------+---------+                             +--------+-------+ |
|  | Workspace Manager|                             |   BSP Tree      | |
|  |                  |                             |                 | |
|  |  - Per-monitor   |                             |  - Binary split  | |
|  |  - Window lists  |                             |  - Traversal    | |
|  |  - Focus cycles  |                             |  - Rebalance    | |
|  +--------+---------+                             +--------+-------+ |
|           |                                                 |         |
|           +------------------+------------------------------+         |
|                              v                                        |
|                   +----------+----------+                             |
|                   | DeferredPositioner  |                             |
|                   |                     |                             |
|                   |  - HDWP batch API   |                             |
|                   |  - Atomic updates   |                             |
|                   +----------+----------+                             |
|                              v                                        |
|                   +----------+----------+                             |
|                   |     DWM Renderer    |                             |
|                   |                     |                             |
|                   |  - Border colors    |                             |
|                   |  - Transitions      |                             |
|                   |  - Corner prefs     |                             |
|                   +---------------------+                             |
|                                                                       |
+-----------------------------------------------------------------------+
```

### Data Flow

```
  WinEventHook (raw Win32 window events)
       |
       v
  +---------------------------+
  |  Event Processor          |
  |  (dedup, debounce,        |
  |   classify events)        |
  +---------------------------+
       |
       v
  +---------------------------+
  |  Window State Manager     |
  |  (HWND registry, rules,   |
  |   state machine)          |
  +---------------------------+
       |
       v
  +---------------------------+
  |  Layout Engine            |
  |  (BSP tree -> Rect calc)  |
  +---------------------------+
       |
       v
  +---------------------------+
  |  DeferWindowPos           |
  |  (batch window moves)     |
  +---------------------------+
       |
       v
  +---------------------------+
  |  DWM API                  |
  |  (borders, transitions)   |
  +---------------------------+
```

### Window State Machine

```
                         +----------+
                    +--->|  TILING  |<----------+
                    |    +----+-----+           |
                    |         |                 |
                    |    $mod+T                 |
                    |   toggle_float            |
                    |         |                 |
                    |         v                 |
             $mod+F |    +----+-----+           | restore
         fullscreen |    | FLOATING |           |
                    |    +----+-----+           |
                    |         |                 |
                    |         | minimize        |
                    |         v                 |
                    |    +----+-----+           |
                    +----+MINIMIZED|------------+
                    |    +----------+           |
                    |                           |
                    |    +----+-----+           |
                    +----|FULLSCREEN|-----------+
                         +----------+
                              |
                              | exit fullscreen
                              v
                         (returns to
                       previous state)
```

**States:**
- **TILING** — Window is managed by the layout engine, position/size controlled automatically
- **FLOATING** — Window is free-floating, user controls position and size
- **FULLSCREEN** — Window occupies the full monitor, temporarily removed from tiling
- **MINIMIZED** — Window is minimized, removed from layout calculations

**Transitions:**
- `TILING <-> FLOATING`: User toggle (`$mod+T`) or window rule match
- `Any -> MINIMIZED`: User minimizes (window stops being managed)
- `MINIMIZED -> TILING/FLOATING`: User restores (previous state resumed)
- `Any -> FULLSCREEN`: Detect fullscreen mode (`$mod+F`)
- `FULLSCREEN -> previous`: Exit fullscreen, restore to prior state

## Development

### Project Structure

```
hyprtile/
├── Cargo.toml              # Package manifest
├── build.rs                # Build script (resource embedding)
├── README.md               # This file
├── LICENSE                 # BSD-3-Clause license
├── docs/
│   ├── CONFIGURATION.md    # Full configuration reference
│   └── IPC_PROTOCOL.md     # IPC protocol documentation
├── src/
│   ├── main.rs             # CLI entry point (clap args)
│   ├── lib.rs              # Library root, logging setup
│   ├── app.rs              # Main application coordinator
│   ├── config/
│   │   ├── mod.rs          # Config loading, validation, hot-reload
│   │   ├── defaults.rs     # Default configuration values
│   │   └── types.rs        # Config data structures
│   ├── platform/
│   │   ├── mod.rs          # Platform abstraction layer
│   │   ├── window.rs       # Window operations (HWND wrappers)
│   │   ├── monitor.rs      # Monitor enumeration and DPI
│   │   ├── events.rs       # WinEventHook callbacks
│   │   ├── dwm.rs          # DWM API wrappers (borders, effects)
│   │   └── input.rs        # Raw input, global hotkeys
│   ├── layout/
│   │   ├── mod.rs          # Layout coordinator and types
│   │   ├── bsp.rs          # BSP tree data structure
│   │   ├── dwindle.rs      # Dwindle layout algorithm
│   │   ├── master_stack.rs # Master-stack layout algorithm
│   │   ├── monocle.rs      # Monocle layout algorithm
│   │   ├── grid.rs         # Grid layout algorithm
│   │   └── gaps.rs         # Gap calculation utilities
│   ├── workspace/
│   │   ├── mod.rs          # Workspace manager
│   │   └── model.rs        # Workspace data model
│   ├── window/
│   │   ├── mod.rs          # Window state manager
│   │   ├── model.rs        # Window struct and state machine
│   │   ├── filter.rs       # Window filtering logic
│   │   └── rules.rs        # Window rule matching engine
│   ├── ipc/
│   │   ├── mod.rs          # IPC server (named pipe + TCP)
│   │   ├── protocol.rs     # JSON command definitions
│   │   └── commands.rs     # Command handlers
│   └── util/
│       ├── rect.rs         # Rectangle math operations
│       ├── dpi.rs          # DPI scaling utilities
│       └── animation.rs    # Animation easing and interpolation
├── tests/
│   ├── integration_tests.rs# Comprehensive test suite (50+ tests)
│   └── fixtures/           # Test fixtures (sample configs)
└── resources/
    ├── hyprtile.rc         # Windows resource file
    └── icon.ico            # Application icon
```

### Building

```bash
# Debug build
cargo build

# Release build (optimized)
cargo build --release

# Run tests
cargo test

# Run with verbose logging
$env:RUST_LOG="hyprtile=debug"; cargo run

# Print default config
cargo run -- --print-default-config
```

### Running Tests

```bash
# Run all tests
cargo test --test integration_tests

# Run specific test group
cargo test test_rect_
cargo test test_layout_
cargo test test_bsp_
```

## IPC Protocol

For full IPC documentation including all 13 commands with request/response examples, client code in Rust and Python, and status bar integration guides, see [docs/IPC_PROTOCOL.md](docs/IPC_PROTOCOL.md).

### Quick Example

Send a command to a running HyprTile instance:

```powershell
# Using named pipe (PowerShell)
$pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", "hyprtile", "Out")
$pipe.Connect(1000)
$writer = New-Object System.IO.StreamWriter($pipe)
$writer.WriteLine('{\"command\":\"workspaces\"}')
$writer.Flush()
$pipe.Dispose()
```

```bash
# Using TCP (netcat / ncat)
echo '{"command":"workspaces"}' | nc localhost 9860
```

## License

HyprTile is licensed under the **BSD 3-Clause License**. See [LICENSE](LICENSE) for the full text.

```
BSD 3-Clause License

Copyright (c) 2024, HyprTile Contributors

Redistribution and use in source and binary forms, with or without
modification, are permitted provided that the following conditions are met:

1. Redistributions of source code must retain the above copyright notice, this
   list of conditions and the following disclaimer.

2. Redistributions in binary form must reproduce the above copyright notice,
   this list of conditions and the following disclaimer in the documentation
   and/or other materials provided with the distribution.

3. Neither the name of the copyright holder nor the names of its
   contributors may be used to endorse or promote products derived from
   this software without specific prior written permission.
```

## Acknowledgments

HyprTile stands on the shoulders of excellent open-source projects:

- **[Hyprland](https://hyprland.org/)** — The Wayland compositor that inspired HyprTile's design philosophy, visual style, and configuration approach. Thank you to Vaxry and the Hyprland community for pushing the boundaries of what a tiling WM can be.

- **[Komorebi](https://github.com/LGUG2Z/komorebi)** — An excellent tiling window manager for Windows that proved Rust and Win32 can deliver a first-class tiling experience. Komorebi's architecture informed several design decisions in HyprTile.

- **[GlazeWM](https://github.com/glzr-io/glazewm)** — A pioneering Windows tiling WM written in C#. GlazeWM demonstrated the viability of tiling window management on Windows and established patterns for Win32 integration.

- **[windows-rs](https://github.com/microsoft/windows-rs)** — Microsoft's official Rust bindings for the Windows API. Without this project, building a native Windows application in Rust would be significantly more challenging.

- **[tokio](https://tokio.rs/)** — The asynchronous runtime that powers HyprTile's IPC server, file watcher, and event processing with excellent performance.

---

<p align="center">
  Made with Rust on Windows
</p>
