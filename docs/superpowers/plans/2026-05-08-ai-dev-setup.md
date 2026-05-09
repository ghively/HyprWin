# HyprWin AI Development Setup Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Transform the HyprWin repo into a clean, green, AI-ready codebase with proper tooling, documentation, and two remaining live bugs fixed.

**Architecture:** Single coordinated pass — repo cleanup → config files → CLAUDE.md → CI → bug fixes → verify build. Each task commits independently so the branch stays bisectable.

**Tech Stack:** Rust 2024 edition, windows-rs 0.59, tokio, GitHub Actions (windows-latest runners)

---

## Pre-flight: What's Already Done

After re-reading the source, most BUG_HUNT.md items are **already fixed**:
- `events.rs`: uses `Mutex<Option<Sender>>` ✓
- `workspace/model.rs`: safe fallbacks replace all `expect()` ✓
- `input.rs`: WM_HOTKEY handler forwards actions ✓
- `workspace` bounds clamped in `handle_hotkey` ✓
- TCP server uses `Arc<Notify>` cancellation ✓
- `config/mod.rs`: `new`, `get`, `reload`, `start_watching`, `ensure_default_config` all exist ✓
- `util/rect.rs`: `Point::new`, `Point::distance_to`, `Rect::adjust_for_gaps` all exist ✓

**Two real bugs remain:**
1. Hook thread join hangs — `hook_handle.join()` called but no `WM_QUIT` posted to the hook thread
2. Config hot-reload watcher created but never started — `config_manager.start_watching()` never called

---

## Task 1: Archive Stages and Clean Main

**Files:**
- Delete: `stage1-util-config/`, `stage2-platform/`, `stage3-layout/`, `stage4-window-workspace/`, `stage5-app-ipc/`, `stage6-docs-tests/`
- Delete: `hyprtile-v0.1.0.zip`
- Modify: `.gitignore`

- [ ] **Step 1: Create the kimi-stages branch**

```powershell
cd C:\Git\WinHyprlnd
git checkout -b kimi-stages
git push origin kimi-stages
git checkout master
```

Expected: branch `kimi-stages` pushed to GitHub, back on `master`

- [ ] **Step 2: Remove stage directories and binary artifact from master**

```powershell
git rm -r stage1-util-config stage2-platform stage3-layout stage4-window-workspace stage5-app-ipc stage6-docs-tests
git rm hyprtile-v0.1.0.zip
```

Expected: 6 directories and 1 zip file staged for removal

- [ ] **Step 3: Update .gitignore**

Replace the contents of `.gitignore` at repo root with:

```gitignore
# Rust build artifacts
target/
**/*.rs.bk

# Editor files
.vscode/
.idea/
*.swp
*.swo
~*

# Binary artifacts — never commit these
*.exe
*.zip
*.msi

# Secrets
.env
.env.local

# Claude Code local settings
.claude/settings.local.json
```

- [ ] **Step 4: Commit**

```powershell
git add .gitignore
git commit -m "chore: archive stage dirs to kimi-stages, remove binary artifact"
```

Expected: commit with ~800 deletions, 1 modified file

---

## Task 2: Toolchain and Formatting Config

**Files:**
- Create: `rust-toolchain.toml` (repo root)
- Create: `.editorconfig` (repo root)
- Create: `hyprtile/rustfmt.toml`

- [ ] **Step 1: Create rust-toolchain.toml**

Create `C:\Git\WinHyprlnd\rust-toolchain.toml`:

```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

- [ ] **Step 2: Create .editorconfig**

Create `C:\Git\WinHyprlnd\.editorconfig`:

```ini
root = true

[*]
indent_style = space
indent_size = 4
end_of_line = lf
charset = utf-8
trim_trailing_whitespace = true
insert_final_newline = true

[*.toml]
indent_size = 4

[*.md]
trim_trailing_whitespace = false
```

- [ ] **Step 3: Create hyprtile/rustfmt.toml**

Create `C:\Git\WinHyprlnd\hyprtile\rustfmt.toml`:

```toml
edition = "2024"
max_width = 100
use_small_heuristics = "Default"
```

- [ ] **Step 4: Verify rustfmt runs**

```powershell
cd C:\Git\WinHyprlnd\hyprtile
cargo fmt --check
```

Expected: either passes clean, or shows diffs. If there are diffs, run `cargo fmt` to apply them.

- [ ] **Step 5: Commit**

```powershell
cd C:\Git\WinHyprlnd
git add rust-toolchain.toml .editorconfig hyprtile/rustfmt.toml
git add hyprtile/src  # if cargo fmt changed files
git commit -m "chore: add rust-toolchain.toml, .editorconfig, rustfmt.toml"
```

---

## Task 3: Write CLAUDE.md

**Files:**
- Create: `CLAUDE.md` (repo root)

- [ ] **Step 1: Create CLAUDE.md**

Create `C:\Git\WinHyprlnd\CLAUDE.md` with the following content:

```markdown
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
```

- [ ] **Step 2: Commit**

```powershell
cd C:\Git\WinHyprlnd
git add CLAUDE.md
git commit -m "docs: add CLAUDE.md AI agent guide"
```

---

## Task 4: GitHub Actions CI

**Files:**
- Create: `.github/workflows/ci.yml`

- [ ] **Step 1: Create .github/workflows directory**

```powershell
New-Item -ItemType Directory -Force -Path C:\Git\WinHyprlnd\.github\workflows
```

- [ ] **Step 2: Create ci.yml**

Create `C:\Git\WinHyprlnd\.github\workflows\ci.yml`:

```yaml
name: CI

on:
  push:
    branches: [master]
  pull_request:
    branches: [master]

env:
  CARGO_TERM_COLOR: always

jobs:
  fmt:
    name: Format check
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Check formatting
        run: cargo fmt --manifest-path hyprtile/Cargo.toml --check

  clippy:
    name: Clippy
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run clippy
        run: cargo clippy --manifest-path hyprtile/Cargo.toml -- -D warnings

  build:
    name: Build (release)
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Build release binary
        run: cargo build --manifest-path hyprtile/Cargo.toml --release

  test:
    name: Test
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - name: Run tests
        run: cargo test --manifest-path hyprtile/Cargo.toml
```

- [ ] **Step 3: Commit**

```powershell
cd C:\Git\WinHyprlnd
git add .github/workflows/ci.yml
git commit -m "ci: add GitHub Actions workflow (fmt, clippy, build, test)"
```

---

## Task 5: Fix Hook Thread Graceful Shutdown

**Problem:** `run()` calls `hook_handle.join()` but never sends `WM_QUIT` to the hook thread. The hook thread is blocked in `GetMessageW` and will never wake up, so the join hangs indefinitely on exit.

**Fix:** Share the hook thread's Win32 thread ID via `Arc<AtomicU32>`. After signalling shutdown, post `WM_QUIT` to that thread ID before joining.

**Files:**
- Modify: `hyprtile/src/app.rs` (the `run()` method)

- [ ] **Step 1: Read the current run() method shutdown block**

Open `hyprtile/src/app.rs` and locate the shutdown block (around line 559). It currently looks like:

```rust
// 5. Graceful shutdown: signal threads to stop and join them
hotkey_shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
self.state.tcp_shutdown.notify_waiters();

// Join worker threads with a timeout to avoid hanging forever
let join_timeout = Duration::from_secs(2);
let hook_join = hook_handle.join();
if hook_join.is_err() {
    error!("Event hook thread panicked or failed to join");
}
let hotkey_join = hotkey_handle.join();
if hotkey_join.is_err() {
    error!("Hotkey loop thread panicked or failed to join");
}
```

- [ ] **Step 2: Add AtomicU32 for hook thread ID before thread spawn**

In `run()`, before the `hook_handle` spawn (around line 426), add:

```rust
use std::sync::atomic::{AtomicU32, Ordering};
let hook_thread_id = Arc::new(AtomicU32::new(0));
let hook_thread_id_inner = Arc::clone(&hook_thread_id);
```

- [ ] **Step 3: Store thread ID inside hook thread spawn**

Inside the hook thread closure, immediately after the closure opens, add:

```rust
// Store our Win32 thread ID so the main thread can post WM_QUIT to us
unsafe {
    hook_thread_id_inner.store(
        windows::Win32::UI::WindowsAndMessaging::GetCurrentThreadId(),
        Ordering::SeqCst,
    );
}
```

The full hook thread spawn becomes:

```rust
let hook_handle = std::thread::Builder::new()
    .name("event-hook".to_string())
    .spawn(move || {
        unsafe {
            hook_thread_id_inner.store(
                windows::Win32::UI::WindowsAndMessaging::GetCurrentThreadId(),
                Ordering::SeqCst,
            );
        }
        match EventHook::register(event_tx_for_hook) {
            Ok(hook) => {
                info!("WinEventHook registered");
                loop {
                    unsafe {
                        let mut msg = std::mem::zeroed();
                        if windows::Win32::UI::WindowsAndMessaging::GetMessageW(
                            &mut msg,
                            windows::Win32::Foundation::HWND(std::ptr::null_mut()),
                            0,
                            0,
                        )
                        .0 > 0
                        {
                            windows::Win32::UI::WindowsAndMessaging::TranslateMessage(&msg);
                            windows::Win32::UI::WindowsAndMessaging::DispatchMessageW(&msg);
                        } else {
                            break;
                        }
                    }
                }
                hook.unregister();
            }
            Err(e) => {
                error!("Failed to register WinEventHook: {}", e);
            }
        }
    })?;
```

- [ ] **Step 4: Post WM_QUIT to hook thread in shutdown block**

Replace the shutdown block with:

```rust
// 5. Graceful shutdown: signal threads to stop and join them
hotkey_shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
self.state.tcp_shutdown.notify_waiters();

// Signal the hook thread to exit its GetMessageW loop
let hook_tid = hook_thread_id.load(std::sync::atomic::Ordering::SeqCst);
if hook_tid != 0 {
    unsafe {
        let _ = PostThreadMessageW(hook_tid, WM_QUIT, WPARAM(0), LPARAM(0));
    }
}

let hook_join = hook_handle.join();
if hook_join.is_err() {
    error!("Event hook thread panicked or failed to join");
}
let hotkey_join = hotkey_handle.join();
if hotkey_join.is_err() {
    error!("Hotkey loop thread panicked or failed to join");
}
```

- [ ] **Step 5: Verify it compiles**

```powershell
cd C:\Git\WinHyprlnd\hyprtile
cargo build 2>&1
```

Expected: compiles with 0 errors. Warnings are acceptable.

- [ ] **Step 6: Commit**

```powershell
cd C:\Git\WinHyprlnd
git add hyprtile/src/app.rs
git commit -m "fix: post WM_QUIT to hook thread before joining to prevent hang on exit"
```

---

## Task 6: Wire Up Config Hot-Reload Watcher

**Problem:** `App` holds a `ConfigManager` (which can own a `RecommendedWatcher`) but `start_watching()` is never called. Automatic hot-reload from file changes never activates — only the IPC `reload_config` command works.

**Files:**
- Modify: `hyprtile/src/app.rs` — call `start_watching()` in `App::new()`

- [ ] **Step 1: Find where config_manager is created in App::new()**

In `App::new()`, locate around line 404:

```rust
Ok(App {
    state,
    event_tx,
    event_rx,
    config_manager: ConfigManager::new()?,  // placeholder
    config_path,
    tray,
    hotkey_manager: None,
})
```

- [ ] **Step 2: Replace the placeholder with a properly initialised ConfigManager**

Replace the `App::new()` return with:

```rust
let mut config_manager = ConfigManager::new()?;
if let Err(e) = config_manager.start_watching() {
    warn!("Config file watcher could not be started: {}", e);
}

Ok(App {
    state,
    event_tx,
    event_rx,
    config_manager,
    config_path,
    tray,
    hotkey_manager: None,
})
```

- [ ] **Step 3: Verify it compiles**

```powershell
cd C:\Git\WinHyprlnd\hyprtile
cargo build 2>&1
```

Expected: 0 errors.

- [ ] **Step 4: Commit**

```powershell
cd C:\Git\WinHyprlnd
git add hyprtile/src/app.rs
git commit -m "fix: start config file watcher in App::new so hot-reload works automatically"
```

---

## Task 7: Full Build and Test Verification

**Files:** None (verification only)

- [ ] **Step 1: Clean build in release mode**

```powershell
cd C:\Git\WinHyprlnd\hyprtile
cargo build --release 2>&1
```

Expected: `Finished release [optimized] target(s)` with no errors. Note any warnings.

- [ ] **Step 2: Run the full test suite**

```powershell
cargo test 2>&1
```

Expected: all tests pass. If any fail, fix them before proceeding.

- [ ] **Step 3: Run clippy**

```powershell
cargo clippy -- -D warnings 2>&1
```

Expected: no errors. If clippy reports errors, fix each one. Common fixes:
- Unused variable: prefix with `_`
- Needless `return`: remove the keyword
- `unwrap()` on `Option` in non-test code: use `?` or `if let`

- [ ] **Step 4: Check formatting**

```powershell
cargo fmt --check 2>&1
```

If it reports diffs, apply them:
```powershell
cargo fmt
```

- [ ] **Step 5: Commit any fixes from steps 2-4**

```powershell
cd C:\Git\WinHyprlnd
git add hyprtile/src
git commit -m "fix: resolve clippy warnings and fmt issues found during verification"
```

Only create this commit if there were actual changes.

---

## Task 8: Push and Verify CI

**Files:** None

- [ ] **Step 1: Push master to GitHub**

```powershell
cd C:\Git\WinHyprlnd
git push origin master
```

- [ ] **Step 2: Open GitHub Actions in browser**

Navigate to: `https://github.com/ghively/HyprWin/actions`

Expected: CI run triggered. All 4 jobs (fmt, clippy, build, test) should show green within ~5-10 minutes.

- [ ] **Step 3: Fix any CI failures**

If a job fails:
- Click the failed job to see the log
- Fix the issue locally
- Commit and push — a new CI run will trigger automatically

- [ ] **Step 4: Confirm all checks green**

All 4 jobs pass: fmt ✓ clippy ✓ build ✓ test ✓

---

## Success Criteria Checklist

- [ ] `master` has no stage directories and no binary artifacts
- [ ] `kimi-stages` branch exists on GitHub with the archived scaffolding
- [ ] `rust-toolchain.toml`, `.editorconfig`, `hyprtile/rustfmt.toml` present
- [ ] `CLAUDE.md` present with module map, invariants, build commands
- [ ] `.github/workflows/ci.yml` present
- [ ] Hook thread receives `WM_QUIT` on exit — join no longer hangs
- [ ] Config hot-reload watcher is started automatically on launch
- [ ] `cargo build --release` succeeds
- [ ] `cargo test` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo fmt --check` passes
- [ ] GitHub Actions CI is green on first push
