# Bug Hunt Report — HyprTile

> **Scope:** All source files under `src/`  
> **Date:** 2025-01-15  
> **Lines reviewed:** ~4,500 (27 `.rs` files)

---

## CRITICAL (would cause crashes/data loss)

### 1. [app.rs:390] — `ConfigManager::new()` is used but never implemented — Crash on startup
**Description:** `App::new()` calls `ConfigManager::new()?` (line 390), but `ConfigManager` only has `load()` and `load_from_path()`. There is no `new()` constructor. This code path is reached after `App::new()` has already successfully loaded the config, so it is technically dead code that stores the result in an unused field (`config_manager`). However, if a future refactor actually calls it, the compiler will error, or if it were to compile (e.g. via a trait), it would panic at runtime.
**Impact:** Binary will not compile if `ConfigManager::new()` is ever invoked in a way the type-checker validates. In the current code it is dead code that silently stores an invalid type.
**Fix:** Remove the dead `config_manager` field from `App` or implement `ConfigManager::new() -> Result<Self>` that wraps `ConfigManager::load()`.

### 2. [app.rs:462] — Orphaned nested thread spawn (hotkey message loop) — Thread leak
**Description:** Inside the `hotkey-loop` thread, a second thread `msg-loop` is spawned (line 462) without storing its `JoinHandle`. If the outer `hotkey-loop` is dropped, the inner `msg-loop` thread is orphaned and runs until the process exits. The inner thread owns the `action_tx` channel sender; if the outer thread panics or is force-killed, the receiver on the outer thread may hang because the sender is still alive in the orphan.
**Impact:** Every hotkey action from the orphaned thread tries to send into a channel whose receiver may be gone, causing error spam. Thread count grows unboundedly on repeated `App` restart.
**Fix:** Store the `JoinHandle` of the inner thread and join it before the outer thread exits. Or, eliminate the nesting and run the message loop directly in the outer thread.

### 3. [platform/window.rs:601] — `hprocess.unwrap()` after `is_err()` check — Panic on access-denied
**Description:** `get_window_process_name()` checks `hprocess.is_err()` and returns early in that branch, but on the success branch it unconditionally calls `hprocess.unwrap()`. If the `OpenProcess` call returns an invalid handle that is somehow not caught by `is_err()` (or if the code is later refactored), this will panic. More importantly, if `OpenProcess` fails silently, the code returns `String::new()` correctly—but the `unwrap()` is still a latent panic bomb.
**Impact:** If `OpenProcess` returns an `Ok` variant wrapping a null handle (theoretically possible with some Win32 APIs), the code will panic trying to use it.
**Fix:** Use `if let Ok(hprocess) = OpenProcess(...)` instead of `is_err()` + `unwrap()`.

### 4. [platform/dwm.rs:256] — `std::mem::transmute` of untyped function pointer — Memory safety / crash
**Description:** `GetProcAddress` returns an untyped `FARPROC`. The code transmutes it directly to `RtlGetVersionFn` without verifying the function signature or even checking that the DLL export is the expected one. If `ntdll.dll` is hooked, replaced, or the export is somehow different, calling the transmuted function is undefined behavior.
**Impact:** Potential stack corruption, arbitrary code execution, or immediate segfault on systems with atypical ntdll configurations (e.g. security software hooks).
**Fix:** Use `windows::Win32::System::SystemInformation::GetVersionExW` or the `windows-version` crate instead of manual `RtlGetVersion` via transmute. If transmute must be kept, add a size-of-type assertion and verify the function pointer is non-null before calling.

---

## HIGH (would cause incorrect behavior)

### 5. [workspace/model.rs:246,255] — `expect()` on active_workspace invariant — Panic after monitor re-init
**Description:** `get_active_workspace()` and `get_active_workspace_mut()` use `.expect("active_workspace always points to an existing workspace")`. However, `App::handle_monitor_changed()` (app.rs:720-740) calls `workspace_manager.init_monitors()` which clears and rebuilds all monitor workspace data. If a workspace switch happens concurrently (e.g. from a hotkey action queued before the monitor change event), `active_workspace` may point to a workspace ID that no longer exists in the rebuilt list.
**Impact:** Window manager thread panics, bringing down the entire application.
**Fix:** Replace `expect()` with a safe fallback: create a default workspace on demand or return `None` and let callers handle the missing workspace gracefully.

### 6. [workspace/model.rs:292] — `expect()` after `ensure_workspace` — Panic if insertion race occurs
**Description:** `ensure_workspace()` pushes a new workspace and then immediately calls `.expect("workspace was just ensured to exist")` on a `find_mut` call. The `find_mut` iterates the `Vec`; if a bug elsewhere mutates the `Vec` between the push and the find (e.g. through aliasing or a re-entrant call), the workspace may not be found.
**Impact:** Panic in the workspace manager, crashing the WM.
**Fix:** Return the newly created workspace directly from the `push` result by capturing it before insertion, or use `Option` return type instead of `expect()`.

### 7. [app.rs:822-834] — Workspace hotkey parsing lacks bounds checking — Switch to invalid workspace
**Description:** Hotkey actions `workspace_N` and `move_to_workspace_N` parse the number with `rest.parse::<u32>()` and map `0 -> 10`. There is no validation that the workspace ID is within the configured `workspaces.count` range. A user can switch to workspace 999, which will be created on demand by `ensure_workspace`, but this violates config invariants and can lead to memory growth.
**Impact:** User can create arbitrarily many workspaces, growing memory unboundedly. Focus tracking may break when switching back to valid workspaces.
**Fix:** Clamp workspace IDs to `1..=workspaces.count` and log a warning for out-of-range requests.

### 8. [app.rs:219] — `positions.len() as i32` overflow in `DeferredPositioner::new`
**Description:** `DeferredPositioner::new()` takes an `i32` count. On a system with >2,147,483,647 tiling windows (theoretical) this would overflow. More realistically, `BeginDeferWindowPos` accepts `i32` and silently fails if the count is invalid. However, the actual issue is that if `positions.len()` exceeds `i32::MAX`, the cast wraps to a negative number, which `BeginDeferWindowPos` will reject or interpret incorrectly.
**Impact:** On extreme edge cases (not realistic on desktop), deferred positioning batch fails or corrupts.
**Fix:** Use `i32::try_from(positions.len())` and return an error if the count exceeds `i32::MAX`.

### 9. [ipc/mod.rs:212] — TCP server spawned without shutdown handle — Orphan task on exit
**Description:** `tokio::spawn(async move { crate::ipc::start_tcp_server(tcp_port).await })` (app.rs:480) launches the TCP server as a detached task. There is no `JoinHandle` stored, no `tokio::sync::Notify` or cancellation token, and no way to gracefully stop the server when the app exits.
**Impact:** The TCP server task keeps running after the main event loop exits, preventing clean process shutdown until the OS forcefully terminates the process. The named pipe server has the same issue in `IpcServer::start()` but at least it has a shutdown Notify; the TCP server does not.
**Fix:** Store the `JoinHandle`, or share an `Arc<tokio::sync::Notify>` cancellation token with the TCP task and notify it on exit.

### 10. [app.rs:509-511] — Event-loop threads dropped without joining — Zombie threads on exit
**Description:** On shutdown, the app does `drop(hook_handle); drop(hotkey_handle);`. `drop` on a `JoinHandle` does **not** join the thread; the thread keeps running in the background. The hook thread runs a `GetMessageW` loop that only exits when `PostQuitMessage` is sent—which never happens. The hotkey thread similarly loops on `GetMessageW`.
**Impact:** The process will not fully terminate because detached threads are still running message loops. The OS will eventually kill it, but leaked threads and unreleased Win32 hooks cause instability on restart.
**Fix:** Set a quit flag, send `PostThreadMessageW(WM_QUIT)` to each worker thread, and call `join()` on the handles before returning from `run()`.

### 11. [platform/events.rs:50] — `OnceLock<Sender<WindowEvent>>` is never cleared — Sender leak on re-registration
**Description:** `EVENT_SENDER` is a static `OnceLock`. It is set once when `EventHook::register()` is called. If the hook is ever unregistered and re-registered (e.g. on Explorer restart), the `set()` call will fail because `OnceLock` can only be set once. The error is silently ignored (`let _ = EVENT_SENDER.set(event_tx)`), so the old (possibly disconnected) sender is still used, and new events are lost.
**Impact:** After an Explorer restart, WinEventHook events are silently dropped because they are sent to the old, disconnected channel.
**Fix:** Use a `Mutex<Option<Sender<WindowEvent>>>` instead of `OnceLock`, or use `LazyLock` with an `AtomicBool` guard and replace the sender on re-registration.

### 12. [platform/input.rs:390-435] — Hotkey message loop never forwards actions — Dead code / broken hotkeys
**Description:** `run_message_loop()` creates a message window and runs `GetMessageW`, but inside the WM_HOTKEY handler (line 419-425) the code is a no-op: it extracts the hotkey ID but never actually maps it to an action or sends anything through the `hotkey_tx` channel. The comment says "For now we send the raw ID and let the caller handle mapping" but nothing is sent.
**Impact:** No hotkey actions ever reach the application. The entire hotkey subsystem is non-functional.
**Fix:** Look up the hotkey in a `HotkeyManager` (passed into the function or stored in thread-local state) and send the action string through `hotkey_tx`.

---

## MEDIUM (edge cases, inconsistencies)

### 13. [app.rs:330-348] — Monitor fallback uses `unwrap_or(0)` for unknown windows — Windows assigned to non-existent monitor
**Description:** When enumerating existing windows, if `find(|m| m.contains_window(...))` fails, the code falls back to the primary monitor, then to monitor `0` with `.unwrap_or(0)`. If no monitor with ID `0` exists (e.g. because `MONITOR_COUNTER` starts at 1), the window is assigned to a non-existent monitor.
**Impact:** The window is tracked in `workspace_manager` under a phantom monitor ID. Layout calls for that monitor silently do nothing. The window is essentially unmanaged.
**Fix:** Use the first monitor in the list as the ultimate fallback, not hardcoded `0`.

### 14. [platform/window.rs:127-132] — UWP host check inconsistent with window filter
**Description:** `should_manage_window()` allows `ApplicationFrameWindow` and `Windows.UI.Core.CoreWindow` (line 129) even if they lack `WS_CAPTION | WS_THICKFRAME`. However, `window/filter.rs:36` says UWP host windows should be skipped and returns `false` for `is_uwp_host()`. This means the platform layer and the filter layer disagree on whether these windows should be managed.
**Impact:** Some UWP windows may be managed by one path and rejected by another, causing double-processing or missing windows in the layout.
**Fix:** Consolidate the UWP handling: remove the special-case from `should_manage_window()` and let the filter module be the single source of truth.

### 15. [window/filter.rs:26-38] — `passes_all_filters` duplicates `should_manage` but `is_system_window` uses different class list
**Description:** `should_manage()` calls `is_system_window()` which checks against `system_window_classes()` from `filter.rs`. But `platform/window.rs::is_system_window()` (line 162-181) uses a different, larger list. A window classified as system by the platform layer may pass the filter layer.
**Impact:** System windows (e.g. `MultitaskingViewFrame`, `ForegroundStaging`) may be registered as managed windows.
**Fix:** Make `platform/window.rs::is_system_window()` delegate to `window::filter::is_system_window()` so the list lives in exactly one place.

### 16. [ipc/commands.rs:383-405] — `handle_switch_workspace` shows new workspace but does not hide old workspace windows
**Description:** The IPC handler for `SwitchWorkspace` shows windows on the target workspace (line 398-400) but does not hide windows on the previously active workspace. The equivalent code in `app.rs:1003-1016` does both hide and show.
**Impact:** When switching workspaces via IPC, windows from the old workspace remain visible, breaking the virtual-desktop illusion.
**Fix:** Mirror the logic from `app.rs::switch_workspace()`: iterate all workspaces on the monitor and call `show_window(win_id, ws.id == id)`.

### 17. [platform/events.rs:156-163] — `EVENT_OBJECT_LOCATIONCHANGE` always emits `WindowMoved` regardless of foreground
**Description:** The `EVENT_OBJECT_LOCATIONCHANGE` handler has a `GetForegroundWindow() == hwnd` check that is dead code—both branches return `WindowMoved`. The comment suggests it could be a focus change, but the code does not distinguish.
**Impact:** Focus changes that come through as location change are misclassified as moves, causing tiled windows to be incorrectly floated (app.rs:673-693).
**Fix:** If `GetForegroundWindow() == hwnd`, emit `WindowFocused`; otherwise emit `WindowMoved`.

### 18. [platform/window.rs:104-149] — `should_manage_window` checks `IsWindowVisible` at call time only
**Description:** The function queries visibility, cloaking, and style at a single point in time. If a window passes the filter and is then immediately cloaked or hidden before registration completes, it may be added to the workspace and later layout attempts will operate on an invisible window.
**Impact:** Cloaked or hidden windows may briefly appear in the tiling layout, causing flicker or incorrect layouts.
**Fix:** Re-check `should_manage_window()` immediately before calling `register_window()` in the event handlers, and add a periodic re-validation sweep.

### 19. [config/mod.rs:213] — `config_path.parent().unwrap_or_else(|| Path::new("."))` may watch wrong directory
**Description:** If the config file is at the root of a drive (e.g. `C:\hyprtile.toml`), `parent()` returns `None` and the watcher falls back to `.` (current working directory), which may not contain the config.
**Impact:** Config hot-reload does not trigger if the file is at an unusual path.
**Fix:** Use the config file path itself with a file-level watcher, or resolve `.` to the actual current exe directory.

### 20. [layout/monocle.rs:40-47] — `focused_idx` is not validated against windows length before use
**Description:** The `calculate` function accepts `focused_idx: usize` but only validates `windows.is_empty()`. If `focused_idx >= windows.len()` and the caller passes it, the loop skips the `if i == focused_idx` branch for all windows, then at line 81 clamps the index. However, the non-focused windows are still all added; the focused one is added last with the clamped index. This is mostly correct, but if `focused_idx` is wildly out of bounds, the "focused on top" semantics are lost.
**Impact:** Monocle layout may place the wrong window on top.
**Fix:** Clamp `focused_idx` to `windows.len().saturating_sub(1)` at the start of the function, before the loop.

### 21. [platform/monitor.rs:73] — `unwrap_or(32)` on device name null position
**Description:** `String::from_utf16_lossy` slices the device name up to the first null character using `position(|&c| c == 0).unwrap_or(32)`. If the 32-element array contains no null terminator (extremely unlikely but possible with corrupt monitor data), the entire 32 characters are used, potentially producing garbage.
**Impact:** Monitor name contains garbage bytes, confusing logging and display.
**Fix:** This is defensively acceptable; downgrade to LOW.

---

## LOW (code quality, minor)

### 22. [window/model.rs:94] — `floating_rect` captured from current window rect even when minimized
**Description:** `Window::new()` sets `floating_rect: id.get_rect()`. If the window is minimized, `get_rect()` may return incorrect/zero coordinates. When the window is later floated, it restores to these bad coordinates.
**Impact:** Minimized windows that are later floated may appear at `(0,0)` with zero size.
**Fix:** Only capture `floating_rect` if the window is currently visible and not iconic.

### 23. [platform/window.rs:361] — `unwrap()` in `Drop` for `DeferredPositioner`
**Description:** The `Drop` impl does `let hdwp = self.hdwp.take().unwrap()` after checking `is_some()`. This is safe but unnecessary; `if let Some(hdwp) = self.hdwp.take()` is clearer.
**Impact:** None—logically unreachable panic path. Code smell only.
**Fix:** Replace with `if let Some(hdwp) = self.hdwp.take()`.

### 24. [app.rs:1000-1016] — `switch_workspace` re-queries `mon_ws` after mutable borrow
**Description:** The code does `self.state.workspace_manager.switch_workspace()` (mutable borrow), then re-borrows `mon_ws` immutably. This is fine in current Rust, but the pattern of re-querying the same data after a state mutation is fragile.
**Impact:** None currently. Future refactors may introduce bugs.
**Fix:** Store the old active workspace ID before the switch, then use it in the visibility loop without re-querying.

### 25. [util/rect.rs:72] — `inset()` uses `max(0)` instead of `max(1)` for dimensions
**Description:** `inset()` reduces width/height by `2 * amount` and clamps to `0`. If a 1-pixel window is inset by 1 pixel, the result has zero area. The `apply_gaps()` function in `layout/gaps.rs` clamps to `1`, so behavior is inconsistent.
**Impact:** Tiny windows or very large gap configs can produce zero-area rects, which `SetWindowPos` may reject.
**Fix:** Clamp to `max(1)` consistently across all rect-shrinking functions.

### 26. [layout/master_stack.rs:104,120] — Integer division for slot heights discards remainder
**Description:** `master_region.height / master_count as i32` uses truncating division. Remainder pixels are only given to the last window. For many windows with small heights, this can lead to 1-pixel gaps or overlaps.
**Impact:** Layout precision is off by a few pixels with many windows. Purely visual.
**Fix:** Distribute remainder pixels across all windows (round-robin) instead of giving them all to the last slot.

### 27. [platform/events.rs:113-228] — `start_event_loop` is dead code
**Description:** The entire `start_event_loop()` function (lines 180-235) is defined but never called anywhere in the project. It creates a message window and runs a loop that is redundant with the hook thread's loop in `app.rs`.
**Impact:** Dead code increases binary size and maintenance burden.
**Fix:** Remove the function, or integrate it into `App::run()` and call it instead of the inline loop.

---

## unwrap() / expect() Audit Summary

### Production code unwrap() calls (6 total)
| File | Line | Context | Risk |
|------|------|---------|------|
| `platform/window.rs` | 361 | `self.hdwp.take().unwrap()` in `Drop` | LOW (guarded by `is_some`) |
| `platform/window.rs` | 601 | `hprocess.unwrap()` after `is_err` | **HIGH** (panic on edge case) |
| `platform/dwm.rs` | 246 | `ntdll.unwrap()` after `is_err` | **MEDIUM** (guard present but pattern fragile) |
| `platform/input.rs` | 46 | `vk.unwrap()` after `is_none` | **MEDIUM** (guard present but pattern fragile) |
| `platform/monitor.rs` | 217 | `hwnd.unwrap()` after `is_err` | **MEDIUM** (guard present but pattern fragile) |
| `platform/input.rs` | 107 | `hk.as_ref().unwrap().action` | LOW (guard present on previous line) |

### Production code expect() calls (4 total)
| File | Line | Context | Risk |
|------|------|---------|------|
| `workspace/model.rs` | 246 | `expect("active_workspace always points...")` | **HIGH** (can be violated on monitor change) |
| `workspace/model.rs` | 255 | `expect("active_workspace always points...")` | **HIGH** (same as above) |
| `workspace/model.rs` | 292 | `expect("workspace was just ensured...")` | **HIGH** (theoretically impossible but panic is wrong tool) |
| `app.rs` | 467 | `.expect("Failed to spawn message loop thread")` | **HIGH** (thread spawn failure should be graceful) |

### Test-only unwrap() calls (9 total)
All in `#[cfg(test)]` blocks — harmless in production.

### Recommended fix for all unwrap/expect
Replace every `x.is_err()` / `x.unwrap()` pair with `if let Ok(v) = x { ... } else { return/continue; }`. Replace every `.expect()` with a graceful fallback (return `None`, create default, or log and continue).

---

## Additional Observations (not bugs, but design concerns)

1. **No `CoInitialize` call:** The tray-icon crate and DWM APIs may use COM under the hood. `CoInitializeEx` is not called explicitly. On modern Windows this often works because other libraries have already initialized COM, but it is a latent issue on clean threads.

2. **DPI awareness set after window creation:** `set_dpi_awareness()` is called in `App::new()` after some window enumeration has already happened. Existing windows may have been queried at the wrong DPI. Move `set_dpi_awareness()` to the very top of `main()`.

3. **Config reload races:** `reload_config()` replaces `self.config` with a new `Arc`, but layout calculations may hold a read lock on the old `Arc` while the new one is being swapped. This is safe (old data remains valid), but window rules are re-applied while layouts may be in progress, causing transient inconsistent states.

4. **`enumerate_windows` callback lacks synchronization:** `enum_windows_callback` (platform/window.rs:257-261) casts an `LPARAM` to `&mut Vec<WindowId>` without synchronization. While `EnumWindows` is documented to call the callback sequentially on the same thread, if any other code path calls `EnumWindows` with the same callback from a different thread, the mutable reference would be aliased. This is currently safe because only one call site exists.

5. **`is_fullscreen` uses exact pixel equality:** `is_fullscreen()` compares exact coordinates (`window_rect.x == monitor_rect.x`). If a window is 1 pixel off due to DPI scaling or border widths, it is not recognized as fullscreen. Use an epsilon tolerance.

---

*End of report.*
