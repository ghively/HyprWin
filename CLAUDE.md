# HyprWin / HyprTile — AI Agent Guide

## What This Is

HyprTile is a Hyprland-inspired tiling window manager for Windows 10/11.
- Single-process daemon written in Rust (2024 edition)
- Integrates with Windows via Win32 hooks (`windows-rs` crate)
- All source code lives in `hyprtile/`

**Not** cross-platform. **Not** a library. **Not** a fork of Komorebi or GlazeWM.

## Build Commands

All commands run from `hyprtile/`:

```powershell
cargo build                                        # debug build
cargo build --release                              # release (lto + strip)
cargo test                                         # run all tests
$env:RUST_LOG="hyprtile=debug"; cargo run          # run with logging
cargo run -- --print-default-config                # dump default TOML to stdout
cargo run -- --check-config                        # validate config file and exit
cargo fmt --check                                  # check formatting
cargo clippy -- -D warnings                        # lint
```

## Module Map

| Module | File(s) | Owns |
|--------|---------|------|
| Entry point | `src/main.rs` | CLI arg parsing only — no business logic |
| App coordinator | `src/app.rs` | Event loop, thread lifecycle, hotkey dispatch |
| Config | `src/config/` | TOML load/validate/hot-reload via `notify` crate |
| Platform | `src/platform/` | **All Win32 API calls live here and only here** |
| Layout | `src/layout/` | Pure layout algorithms — no Win32 imports |
| Window | `src/window/` | Window state machine and rule matching |
| Workspace | `src/workspace/` | Per-monitor workspace management |
| IPC | `src/ipc/` | Named pipe + TCP server, JSON protocol |
| Util | `src/util/` | Pure math (Rect, DPI, animation) — no Win32 |

## Critical Invariants

1. **Win32 message loops stay on their own threads.** Never move `GetMessageW` loops into async tasks or the main thread.
2. **`DeferWindowPos` batch must always commit.** Never early-return after `BeginDeferWindowPos` without calling `EndDeferWindowPos`.
3. **Never `unwrap()` on Win32 handle results.** Always use `if let Ok(h) = OpenProcess(...)`.
4. **`platform/` is the only module that imports `windows::*`.** Keep this boundary clean — no Win32 imports in `layout/`, `util/`, etc.
5. **IPC pipe path is `\\.\pipe\hyprtile`.** Don't change without updating `docs/IPC_PROTOCOL.md`.
6. **Config path is `%APPDATA%\hyprtile\hyprtile.toml`.** Don't change without updating `docs/CONFIGURATION.md`.

## Key Extension Points

- **Add a layout algorithm:** Create `src/layout/my_layout.rs`, implement the layout function, add a variant to `LayoutType` in `src/layout/mod.rs`.
- **Add an IPC command:** Add a variant to `IpcCommand` in `src/ipc/protocol.rs`, handle it in `src/ipc/commands.rs`.
- **Add a hotkey action:** Add a match arm in `handle_hotkey()` in `src/app.rs`.
- **Add a window rule action:** Add a variant to `WindowAction` in `src/config/types.rs`.

## Testing

```powershell
cargo test                           # all tests
cargo test test_rect_                # rect math tests
cargo test test_layout_              # layout tests
cargo test test_bsp_                 # BSP tree tests
cargo test --test integration_tests  # full integration suite
```

Tests in `src/util/rect.rs`, `src/layout/`, and `tests/integration_tests.rs`.
Win32-dependent code is not unit-tested (requires live Windows session).

## Project Documents

- `SPEC.md` — Full feature specification
- `hyprtile/docs/CONFIGURATION.md` — Config reference
- `hyprtile/docs/IPC_PROTOCOL.md` — IPC command reference
- `hyprtile/docs/DEVELOPERS_GUIDE.md` — Architecture deep-dive
- `hyprtile/BUG_HUNT.md` — Known bugs (most fixed; see plan for remaining)
- `hyprtile/SPEC_AUDIT.md` — Spec compliance audit (most gaps resolved)

## Off-Limits

- Do not add Linux/macOS platform abstractions
- Do not replace `windows-rs` bindings with raw `extern "system"` FFI blocks
- Do not push binary artifacts (`*.exe`, `*.zip`) to any branch
- The `kimi-stages` branch contains Kimi's incremental scaffolding — do not merge it into `master`
