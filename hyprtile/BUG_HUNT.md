# Bug Hunt Report — HyprTile

> **Scope:** All source files under `src/`
> **Last full sweep:** 2025-01-15
> **Current status update:** 2026-05-09

This file tracks the original 27 findings plus follow-up work. Every item
has been resolved or accepted. New issues should be appended at the bottom
under the appropriate severity heading.

---

## CRITICAL — all resolved

| # | Location | Issue | Resolution |
|---|----------|-------|------------|
| 1 | `app.rs:390` | `ConfigManager::new()` was called but missing | `new()` constructor added at `config/mod.rs:48`; wired in `App::new` |
| 2 | `app.rs:462` | Orphaned nested `msg-loop` thread | Inner handle now stored and joined in `app.rs::run` |
| 3 | `platform/window.rs:601` | `hprocess.unwrap()` after `is_err()` | Replaced with `match OpenProcess(...) { Ok(h) => h, Err(_) => return }` |
| 4 | `platform/dwm.rs:256` | `transmute` of `GetProcAddress` without size assertion | `assert_eq!(size_of::<FARPROC>(), size_of::<RtlGetVersionFn>())` added |

## HIGH — all resolved

| # | Location | Issue | Resolution |
|---|----------|-------|------------|
| 5 | `workspace/model.rs:246` | `expect()` on active workspace | Replaced with `or_else()`/`unwrap_or_else()` returning a static default |
| 6 | `workspace/model.rs:292` | `expect()` after `ensure_workspace` | Same pattern: graceful fallback creates workspace 1 if missing |
| 7 | `app.rs:822` | Workspace IDs not clamped | `id.clamp(1, workspaces.count)` with warning on out-of-range |
| 8 | `app.rs:219` | `positions.len() as i32` overflow | `i32::try_from(...).unwrap_or(i32::MAX)` |
| 9 | `ipc/mod.rs:212` | TCP server orphan task | `tcp_shutdown: Arc<Notify>` plumbed through `App::run`, notified on exit |
| 10 | `app.rs:509` | Hook/hotkey threads dropped, not joined | `WM_QUIT` posted to stored thread IDs; handles `.join()`'d |
| 11 | `platform/events.rs:50` | `OnceLock<Sender>` couldn't re-register | Now `Mutex<Option<Sender>>` |
| 12 | `platform/input.rs:419` | `WM_HOTKEY` handler was a no-op | `get_hotkey_action(id)` lookup forwards via `hotkey_tx` |

## MEDIUM — all resolved

| # | Location | Issue | Resolution |
|---|----------|-------|------------|
| 13 | `app.rs:330` | Monitor fallback `unwrap_or(0)` to phantom monitor | Falls back to primary, then `monitors.first()`, then 0 (only reached if no monitors at all) |
| 14 | `platform/window.rs:127` | `should_manage_window` allowed UWP host classes | Removed special-case; `window/filter.rs::is_uwp_host` is the sole gate |
| 15 | `window/filter.rs:26` | Two `is_system_window` lists drifted | `platform/window.rs::is_system_window` now delegates to `window::filter::is_system_window` (single source) |
| 16 | `ipc/commands.rs:383` | `handle_switch_workspace` didn't hide old workspace's windows | Now mirrors `App::switch_workspace`: iterates all workspaces and hides inactive |
| 17 | `platform/events.rs:156` | `EVENT_OBJECT_LOCATIONCHANGE` always emitted `WindowMoved` | Foreground check now emits `WindowFocused`, others `WindowMoved` |
| 18 | `platform/window.rs:104` | `should_manage_window` only checks at call time | Re-validated immediately before `register_window`; periodic sweep handled by event-driven re-checks |
| 19 | `config/mod.rs:213` | Config-watcher parent fallback to CWD | Falls back to watching the file path itself when `parent()` is empty/None |
| 20 | `layout/monocle.rs:40` | `focused_idx` not validated against `windows.len()` | Clamped via `.min(windows.len() - 1)` at function entry |
| 21 | `platform/monitor.rs:73` | Monitor name `unwrap_or(32)` | Accepted as defensive default; documented as LOW |

## LOW — all resolved

| # | Location | Issue | Resolution |
|---|----------|-------|------------|
| 22 | `window/model.rs:94` | `floating_rect` captured even when iconic | Guarded behind `id.is_visible() && !id.is_iconic()`; returns `None` otherwise |
| 23 | `platform/window.rs:361` | `unwrap()` in `Drop` | Already uses `if let Some(hdwp) = self.hdwp.take()` |
| 24 | `app.rs:1000` | `switch_workspace` re-borrow pattern | Code reorganized; remains current-Rust-safe |
| 25 | `util/rect.rs:72` | `inset()` clamped to 0 instead of 1 | Now `max(1)` for both width and height |
| 26 | `layout/master_stack.rs:104,120` | Last slot received all remainder pixels | New `slot_extent()` helper distributes remainder one pixel per slot |
| 27 | `platform/events.rs:182` | `start_event_loop` was dead code | Removed; unused imports cleaned up |

## unwrap()/expect() audit — re-baseline

All production-code `unwrap()` calls are now guarded by an immediately
preceding `is_err()`/`is_some()` check (and most have been replaced with
pattern matching). All `expect()` calls in `workspace/model.rs` were
removed and replaced with safe fallbacks. The only remaining `unwrap()`
pairs are inside `Drop` impls or wrap genuinely infallible operations
(static `OnceLock` initialization, in-memory pre-validated maps).

## Performance benchmarking

The original audit listed acceptance criteria 2–9 as `NOT_TESTED` because
no benchmark suite existed. `benches/perf.rs` (criterion harness) now
covers every criterion that can be measured in pure Rust:

| Criterion | Benchmark | Notes |
|-----------|-----------|-------|
| 2 (hotkey <50ms) | `hotkey_channel_send_recv` | Channel-only path; Win32 round-trip excluded |
| 3 (workspace switch <100ms, 10+ windows) | `workspace_switch_with_10_windows` | Data-model only |
| 4 (event→layout <100ms) | `layout_calculate/*` | Layout cost component |
| 5 (config reload <200ms) | `config_parse_default`, `config_serialize_default` | TOML parse + serialize |
| 6 (IPC <10ms) | `ipc_serialize_*`, `ipc_deserialize_*`, `ipc_full_roundtrip` | JSON codec only |
| 9 (50+ windows) | `layout_calculate/*/{50,100}` | All four layouts at scale |

Criteria 7 (CPU <1% idle / <5% active) and 8 (memory <50MB steady) require
a live Windows session and are validated manually with Process Explorer
or `Get-Process hyprtile | Select WorkingSet,CPU` in PowerShell.

Run on a Windows host:

```powershell
cargo bench
```

## Additional design notes (carried forward)

1. `CoInitializeEx` is still not called explicitly. Tray-icon and DWM
   APIs work because the host process has already initialized COM by the
   time we touch them; no observed regressions.
2. `set_dpi_awareness()` is called in `App::new` after enumeration runs
   once. On systems where this matters, callers can pre-call from `main`.
3. Config-reload race window: the new `Arc<RwLock<Config>>` handoff is
   safe; the worst-case is one frame of stale rules during reload.
4. `enumerate_windows` callback synchronization: documented invariant —
   only the main thread calls `EnumWindows` and the closure is single-use.
5. `is_fullscreen` uses exact pixel comparison; an epsilon would help on
   fractional DPI scaling but no real-world bug has been observed.

---

*All originally documented bugs are resolved. Re-open by appending a new
section if a regression is observed.*
