# Design: Optimal AI Development Setup for HyprWin/HyprTile
**Date:** 2026-05-08  
**Status:** Approved  
**Scope:** Repo cleanup, AI agent guidance, CI/CD, critical bug fixes

---

## 1. Problem Statement

HyprTile was bootstrapped with Kimi (an external AI) and has a complete but unverified Rust codebase. The repo currently has:
- Intermediate scaffolding (`stage1/`–`stage6/`) cluttering `main`
- No `CLAUDE.md` to guide future AI agents
- No CI/CD — bugs land silently
- No pinned Rust toolchain
- 12 confirmed bugs (7 critical/high severity) from `BUG_HUNT.md`
- Minor spec compliance gaps from `SPEC_AUDIT.md`

The goal is a single coordinated pass that produces a clean, green, AI-ready repo.

---

## 2. Repo Structure

### 2a. Branch cleanup
1. Create branch `kimi-stages` from current `main`
2. On `main`: delete `stage1-util-config/`, `stage2-platform/`, `stage3-layout/`, `stage4-window-workspace/`, `stage5-app-ipc/`, `stage6-docs-tests/`
3. Remove `hyprtile-v0.1.0.zip` (binary artifact)

### 2b. Final layout on `main`
```
HyprWin/
├── CLAUDE.md                          ← AI agent operating instructions
├── SPEC.md                            ← Project specification (existing)
├── .gitignore                         ← Updated (add *.zip, target/, *.exe)
├── rust-toolchain.toml                ← Pin stable toolchain
├── .editorconfig                      ← Consistent formatting
├── hyprtile/                          ← All source code
│   ├── Cargo.toml
│   ├── build.rs
│   ├── src/
│   ├── docs/
│   ├── tests/
│   └── resources/
└── .github/
    └── workflows/
        └── ci.yml                     ← Build/test/lint on every push
```

---

## 3. CLAUDE.md

Written as firm onboarding notes for an AI agent that has never seen this repo.

### Must include:
- **What HyprTile is:** A Hyprland-inspired tiling window manager for Windows 10/11. Single-process daemon. Uses Win32 hooks via `windows-rs`. Written in Rust 2024 edition.
- **What it is NOT:** Not cross-platform. Not a library. Not a fork of Komorebi or GlazeWM.
- **Build commands:**
  ```powershell
  cd hyprtile
  cargo build                          # debug
  cargo build --release                # release (lto + strip)
  cargo test                           # run all tests
  $env:RUST_LOG="hyprtile=debug"; cargo run   # run with logging
  cargo run -- --print-default-config  # dump default TOML
  cargo run -- --check-config          # validate config file
  ```
- **Module ownership map** (where to look for what):
  - `src/main.rs` — CLI entry point only, no business logic
  - `src/app.rs` — main coordinator; owns all subsystem handles
  - `src/config/` — TOML config loading, validation, hot-reload via `notify`
  - `src/platform/` — all Win32 API calls; nothing outside this module touches `windows-rs` directly
  - `src/layout/` — pure layout algorithms (no Win32); each algorithm is one file
  - `src/window/` — window state machine and rule matching
  - `src/workspace/` — per-monitor workspace management
  - `src/ipc/` — named pipe + TCP server; JSON protocol
  - `src/util/` — pure math (Rect, DPI, animation); no Win32
- **Critical invariants:**
  - Win32 message loops MUST run on their own dedicated threads — never move them to async tasks
  - `DeferWindowPos` batch MUST always call `commit()` — never early-return after `BeginDeferWindowPos`
  - Never call `unwrap()` on Win32 handle results — always use `if let Ok(h) = ...`
  - `platform/` is the only module that imports `windows::*` — keep this boundary clean
  - The IPC named pipe path is `\\.\pipe\hyprtile` — do not change without updating docs
- **Active bugs being fixed:** Point to `hyprtile/BUG_HUNT.md` — do not work around these, fix them
- **Off-limits:**
  - Do not add Linux/macOS abstractions
  - Do not replace `windows-rs` with raw `extern "system"` FFI
  - Do not touch the `kimi-stages` branch

---

## 4. Toolchain & Formatting Config

### `rust-toolchain.toml` (repo root)
```toml
[toolchain]
channel = "stable"
components = ["rustfmt", "clippy"]
```

### `.editorconfig` (repo root)
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

### `hyprtile/rustfmt.toml`
```toml
edition = "2024"
max_width = 100
use_small_heuristics = "Default"
```

---

## 5. GitHub Actions CI/CD

File: `.github/workflows/ci.yml`

```yaml
name: CI
on:
  push:
    branches: [main]
  pull_request:
    branches: [main]

jobs:
  fmt:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo fmt --manifest-path hyprtile/Cargo.toml --check

  clippy:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo clippy --manifest-path hyprtile/Cargo.toml -- -D warnings

  build:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo build --manifest-path hyprtile/Cargo.toml --release

  test:
    runs-on: windows-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo test --manifest-path hyprtile/Cargo.toml
```

All jobs use `windows-latest` — the Win32 API calls mean this code will not compile on Linux/macOS. No matrix builds needed at this stage.

---

## 6. Bug Fixes

Fixes applied in priority order. All in `hyprtile/src/`.

### Critical

| Bug | File | Fix |
|-----|------|-----|
| #12 Hotkeys never fire | `platform/input.rs:419` | Wire WM_HOTKEY handler: look up hotkey ID in registered map, send action string through `hotkey_tx` |
| #1 `ConfigManager::new()` missing | `app.rs:390` | Implement `ConfigManager::new() -> Result<Self>` as a thin wrapper around `load()` |
| #10 Threads not joined on exit | `app.rs:509` | Send `WM_QUIT` via `PostThreadMessageW` to each worker, then `join()` before returning |
| #11 `OnceLock` not resettable | `platform/events.rs:50` | Replace `OnceLock<Sender<WindowEvent>>` with `Mutex<Option<Sender<WindowEvent>>>` |
| #3 `hprocess.unwrap()` | `platform/window.rs:601` | Rewrite as `if let Ok(hprocess) = OpenProcess(...) { ... }` |
| #4 `transmute` of fn pointer | `platform/dwm.rs:256` | Replace manual `RtlGetVersion` transmute with `GetVersionExW` from `windows-rs` |
| #2 Orphaned nested thread | `app.rs:462` | Store inner `JoinHandle`, join before outer thread exits |

### High

| Bug | File | Fix |
|-----|------|-----|
| #5 `expect()` in workspace after monitor reinit | `workspace/model.rs:246` | Return `Option`, let callers handle missing workspace |
| #6 `expect()` after `ensure_workspace` | `workspace/model.rs:292` | Return the new workspace directly from push, drop `expect()` |
| #7 Workspace ID not bounds-checked | `app.rs:822` | Clamp to `1..=workspaces.count`, warn on out-of-range |
| #9 TCP server no shutdown handle | `app.rs:480` | Share `Arc<Notify>` cancellation token with TCP task |

### Spec Gaps (minor)

| Item | File | Fix |
|------|------|-----|
| `Point::new` missing | `src/util/rect.rs` | Add `fn new(x: i32, y: i32) -> Self` |
| `Point::distance_to` missing | `src/util/rect.rs` | Add `fn distance_to(&self, other: &Point) -> f64` |
| `Rect::adjust_for_gaps` missing | `src/util/rect.rs` | Add `fn adjust_for_gaps(&self, gaps: i32) -> Self` clamped to min 1px |
| `ConfigManager::get` missing | `src/config/mod.rs` | Add `fn get(&self) -> &Config` returning current config |
| `ConfigManager::reload` missing | `src/config/mod.rs` | Add `fn reload(&mut self) -> Result<()>` wrapping `load_from_path` |
| `ConfigManager::start_watching` missing | `src/config/mod.rs` | Add `fn start_watching(&self, tx: Sender<()>) -> Result<()>` using `notify` |
| `ConfigManager::ensure_default_config` missing | `src/config/mod.rs` | Add `fn ensure_default_config() -> Result<()>` writing defaults if file absent |
| `WindowId::should_manage` missing | `src/platform/window.rs` | Add method wrapping free function `should_manage_window` |

---

## 7. Success Criteria

- [ ] `main` branch contains no stage directories and no binary artifacts
- [ ] `rust-toolchain.toml` and `.editorconfig` present at repo root
- [ ] `CLAUDE.md` present and covers all sections above
- [ ] `cargo fmt --check` passes
- [ ] `cargo clippy -- -D warnings` passes
- [ ] `cargo build --release` succeeds
- [ ] `cargo test` passes
- [ ] GitHub Actions CI is green on first push
- [ ] All 7 critical/high bugs from BUG_HUNT.md are fixed
- [ ] All spec gaps from SPEC_AUDIT.md minor section are resolved
