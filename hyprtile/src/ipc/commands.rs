use super::protocol::*;
use crate::app::AppState;
use crate::layout::LayoutType;
use crate::workspace::model::FocusDirection;
use serde_json::json;
use tracing::{debug, error, warn};

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: IPC_COMMAND_DISPATCH — All IPC commands handled here.
// Before adding new command handlers:
//   1. Each handler receives (&mut AppState) and returns IpcResponse.
//   2. Use IpcResponse::success(Some(json)) for data responses.
//   3. Use IpcResponse::error("msg") for failures — always include context.
//   4. Mirror app.rs behavior — IPC is just another UI for the same actions.
//   5. Keep handlers thin — delegate to AppState methods, don't duplicate logic.
// ═══════════════════════════════════════════════════════════════════════════════

/// Handle an incoming IPC request and produce a response.
///
/// Dispatches the request to the appropriate handler based on the variant.
/// Some commands only read state (`Workspaces`, `FocusedWindow`, etc.) while
/// others mutate it (`ToggleFloat`, `SwitchWorkspace`, etc.).
pub fn handle_command(request: IpcRequest, state: &mut AppState) -> IpcResponse {
    debug!("Handling IPC command: {:?}", request);

    match request {
        IpcRequest::Workspaces { monitor } => handle_workspaces(monitor, state),
        IpcRequest::FocusedWindow => handle_focused_window(state),
        IpcRequest::Layout { monitor } => handle_layout(monitor, state),
        IpcRequest::WindowCount => handle_window_count(state),
        IpcRequest::FocusDirection { direction } => handle_focus_direction(&direction, state),
        IpcRequest::MoveDirection { direction } => handle_move_direction(&direction, state),
        IpcRequest::ToggleFloat => handle_toggle_float(state),
        IpcRequest::ToggleFullscreen => handle_toggle_fullscreen(state),
        IpcRequest::CycleLayout => handle_cycle_layout(state),
        IpcRequest::SwitchWorkspace { id } => handle_switch_workspace(id, state),
        IpcRequest::MoveToWorkspace { id } => handle_move_to_workspace(id, state),
        IpcRequest::ReloadConfig => handle_reload_config(state),
        IpcRequest::Exit => handle_exit(state),
    }
}

// ---------------------------------------------------------------------------
// Query handlers (read-only)
// ---------------------------------------------------------------------------

/// Return workspace information for the requested monitor, or all monitors.
fn handle_workspaces(monitor: Option<u32>, state: &AppState) -> IpcResponse {
    let mut workspaces_info = Vec::new();

    match monitor {
        Some(monitor_id) => {
            if let Some(monitor_ws) = state.workspace_manager.monitors.get(&monitor_id) {
                for ws in &monitor_ws.workspaces {
                    let has_focus = ws.id == monitor_ws.active_workspace;
                    workspaces_info.push(WorkspaceInfo {
                        id: ws.id,
                        name: ws.name.clone(),
                        windows: ws.windows.len(),
                        has_focus,
                    });
                }
            } else {
                return IpcResponse::error(format!("Monitor {} not found", monitor_id));
            }
        }
        None => {
            // Return workspaces for all monitors
            for (monitor_id, monitor_ws) in &state.workspace_manager.monitors {
                for ws in &monitor_ws.workspaces {
                    let has_focus = ws.id == monitor_ws.active_workspace;
                    workspaces_info.push(WorkspaceInfo {
                        id: ws.id,
                        name: format!("{} (on monitor {})", ws.name, monitor_id),
                        windows: ws.windows.len(),
                        has_focus,
                    });
                }
            }
        }
    }

    IpcResponse::success(Some(json!(workspaces_info)))
}

/// Return information about the currently focused window.
fn handle_focused_window(state: &AppState) -> IpcResponse {
    let focused_id = match state.window_manager.get_focused() {
        Some(id) => id,
        None => return IpcResponse::error("No window is focused".to_string()),
    };

    let window = match state.window_manager.get_window(focused_id) {
        Some(w) => w,
        None => return IpcResponse::error("Focused window not found in registry".to_string()),
    };

    let state_str = match window.state {
        crate::window::model::WindowState::Tiling => "tiling",
        crate::window::model::WindowState::Floating => "floating",
        crate::window::model::WindowState::Maximized => "maximized",
        crate::window::model::WindowState::Fullscreen => "fullscreen",
        crate::window::model::WindowState::Minimized => "minimized",
    };

    let info = FocusedWindowInfo {
        id: focused_id.0 as u64,
        title: window.title.clone(),
        class: window.class_name.clone(),
        state: state_str.to_string(),
    };

    IpcResponse::success(Some(json!(info)))
}

/// Return the current layout and available layouts.
fn handle_layout(monitor: Option<u32>, state: &AppState) -> IpcResponse {
    let monitor_id = monitor.unwrap_or_else(|| {
        state
            .get_focused_monitor()
            .map(|m| m.id)
            .unwrap_or(0)
    });

    let layout_name = state
        .workspace_manager
        .get_active_workspace(monitor_id)
        .map(|ws| ws.layout_engine.current().name().to_string())
        .unwrap_or_else(|| "dwindle".to_string());

    let available: Vec<String> = LayoutType::all()
        .iter()
        .map(|lt| lt.name().to_string())
        .collect();

    let info = LayoutInfo {
        current: layout_name,
        available,
    };

    IpcResponse::success(Some(json!(info)))
}

/// Return the total number of managed windows.
fn handle_window_count(state: &AppState) -> IpcResponse {
    let count = state.window_manager.count();
    IpcResponse::success(Some(json!(count)))
}

// ---------------------------------------------------------------------------
// Directional focus / move (mutating)
// ---------------------------------------------------------------------------

/// Parse a direction string into a [`FocusDirection`].
fn parse_direction(direction: &str) -> Option<FocusDirection> {
    match direction.to_lowercase().as_str() {
        "left" => Some(FocusDirection::Left),
        "right" => Some(FocusDirection::Right),
        "up" => Some(FocusDirection::Up),
        "down" => Some(FocusDirection::Down),
        _ => None,
    }
}

/// Handle focus-direction command.
fn handle_focus_direction(direction: &str, state: &mut AppState) -> IpcResponse {
    let dir = match parse_direction(direction) {
        Some(d) => d,
        None => return IpcResponse::error(format!("Invalid direction: {}", direction)),
    };

    if let Some(monitor) = state.get_focused_monitor() {
        state.workspace_manager.cycle_focus(monitor.id, dir);
        let focused = state.window_manager.get_focused();
        if let Some(focused_id) = focused {
            crate::platform::window::focus_window(focused_id.as_raw());
        }
        IpcResponse::success(None)
    } else {
        IpcResponse::error("No focused monitor".to_string())
    }
}

/// Handle move-direction command.
fn handle_move_direction(direction: &str, state: &mut AppState) -> IpcResponse {
    let dir = match parse_direction(direction) {
        Some(d) => d,
        None => return IpcResponse::error(format!("Invalid direction: {}", direction)),
    };

    let focused_id = match state.window_manager.get_focused() {
        Some(id) => id,
        None => return IpcResponse::error("No focused window".to_string()),
    };

    // Determine the target monitor/workspace based on direction
    let current_monitor_id = match state.workspace_manager.get_window_location(focused_id) {
        Some((mon_id, _ws_id)) => mon_id,
        None => {
            // Fallback: use focused monitor
            match state.get_focused_monitor() {
                Some(m) => m.id,
                None => return IpcResponse::error("No focused monitor".to_string()),
            }
        }
    };

    // Find the adjacent monitor in the given direction
    let current_monitor = match state.monitors.iter().find(|m| m.id == current_monitor_id) {
        Some(m) => m.clone(),
        None => return IpcResponse::error("Current monitor not found".to_string()),
    };

    let current_center = current_monitor.rect.center();

    let target_monitor = state
        .monitors
        .iter()
        .filter(|m| m.id != current_monitor_id)
        .min_by(|a, b| {
            let a_center = a.rect.center();
            let b_center = b.rect.center();

            let a_dist = directional_distance(current_center, a_center, &dir);
            let b_dist = directional_distance(current_center, b_center, &dir);
            a_dist.partial_cmp(&b_dist).unwrap_or(std::cmp::Ordering::Equal)
        })
        .cloned();

    if let Some(target) = target_monitor {
        // Move window to the target monitor's active workspace
        if let Ok(monitor_ws) = state
            .workspace_manager
            .monitors
            .get(&target.id)
            .ok_or_else(|| anyhow::anyhow!("Target monitor not found"))
        {
            let target_workspace = monitor_ws.active_workspace;
            if let Err(e) = state
                .workspace_manager
                .move_window_to_workspace(focused_id, target_workspace)
            {
                warn!("Failed to move window to workspace: {}", e);
                return IpcResponse::error(format!("Move failed: {}", e));
            }
            // Also update monitor tracking
            let _ = state
                .workspace_manager
                .move_window_to_monitor(focused_id, target.id);
        }

        // Re-apply layouts on both monitors
        state.apply_layout(current_monitor_id);
        state.apply_layout(target.id);

        IpcResponse::success(None)
    } else {
        // No adjacent monitor found; cycle within current workspace
        state.workspace_manager.cycle_focus(current_monitor_id, dir);
        IpcResponse::success(None)
    }
}

/// Compute a weighted directional distance between two points.
/// Prefers movement along the requested axis.
fn directional_distance(from: (i32, i32), to: (i32, i32), dir: &FocusDirection) -> f64 {
    let dx = (to.0 - from.0) as f64;
    let dy = (to.1 - from.1) as f64;

    match dir {
        FocusDirection::Left => {
            if dx >= 0.0 {
                f64::INFINITY
            } else {
                -dx * 2.0 + dy.abs()
            }
        }
        FocusDirection::Right => {
            if dx <= 0.0 {
                f64::INFINITY
            } else {
                dx * 2.0 + dy.abs()
            }
        }
        FocusDirection::Up => {
            if dy >= 0.0 {
                f64::INFINITY
            } else {
                dx.abs() + -dy * 2.0
            }
        }
        FocusDirection::Down => {
            if dy <= 0.0 {
                f64::INFINITY
            } else {
                dx.abs() + dy * 2.0
            }
        }
        FocusDirection::Next | FocusDirection::Previous => {
            (dx * dx + dy * dy).sqrt()
        }
    }
}

// ---------------------------------------------------------------------------
// Window state toggle handlers
// ---------------------------------------------------------------------------

/// Toggle floating state of the focused window.
fn handle_toggle_float(state: &mut AppState) -> IpcResponse {
    let focused_id = match state.window_manager.get_focused() {
        Some(id) => id,
        None => return IpcResponse::error("No focused window".to_string()),
    };

    let new_state = match state.window_manager.toggle_float(focused_id) {
        Some(s) => {
            let state_str = match s {
                crate::window::model::WindowState::Tiling => "tiling",
                crate::window::model::WindowState::Floating => "floating",
                _ => "other",
            };
            state_str.to_string()
        }
        None => return IpcResponse::error("Failed to toggle float".to_string()),
    };

    // Re-apply layout on the monitor containing this window
    if let Some((monitor_id, _)) = state.workspace_manager.get_window_location(focused_id) {
        state.apply_layout(monitor_id);
    } else {
        state.apply_all_layouts();
    }

    IpcResponse::success(Some(json!({ "state": new_state })))
}

/// Toggle fullscreen state of the focused window.
fn handle_toggle_fullscreen(state: &mut AppState) -> IpcResponse {
    let focused_id = match state.window_manager.get_focused() {
        Some(id) => id,
        None => return IpcResponse::error("No focused window".to_string()),
    };

    let new_state = match state.window_manager.toggle_fullscreen(focused_id) {
        Some(s) => {
            let state_str = match s {
                crate::window::model::WindowState::Fullscreen => "fullscreen",
                crate::window::model::WindowState::Tiling => "tiling",
                crate::window::model::WindowState::Floating => "floating",
                _ => "other",
            };
            state_str.to_string()
        }
        None => return IpcResponse::error("Failed to toggle fullscreen".to_string()),
    };

    if let Some((monitor_id, _)) = state.workspace_manager.get_window_location(focused_id) {
        state.apply_layout(monitor_id);
    } else {
        state.apply_all_layouts();
    }

    IpcResponse::success(Some(json!({ "state": new_state })))
}

// ---------------------------------------------------------------------------
// Layout and workspace handlers
// ---------------------------------------------------------------------------

/// Cycle to the next layout on the focused monitor's active workspace.
fn handle_cycle_layout(state: &mut AppState) -> IpcResponse {
    let monitor_id = state
        .get_focused_monitor()
        .map(|m| m.id)
        .unwrap_or(0);

    if let Some(ws) = state.workspace_manager.get_active_workspace_mut(monitor_id) {
        let new_layout = ws.layout_engine.cycle();
        let layout_name = new_layout.name().to_string();
        debug!(
            "Cycled layout on monitor {} workspace {} to {}",
            monitor_id, ws.id, layout_name
        );
        drop(ws);
        state.apply_layout(monitor_id);
        return IpcResponse::success(Some(json!({ "layout": layout_name })));
    }

    IpcResponse::error("No active workspace to cycle layout".to_string())
}

/// Switch the focused monitor to the given workspace.
fn handle_switch_workspace(id: u32, state: &mut AppState) -> IpcResponse {
    let monitor_id = state
        .get_focused_monitor()
        .map(|m| m.id)
        .unwrap_or(0);

    if let Err(e) = state.workspace_manager.switch_workspace(monitor_id, id) {
        return IpcResponse::error(format!("Failed to switch workspace: {}", e));
    }

    // Show windows on the new workspace, hide on the old
    if let Some(monitor_ws) = state.workspace_manager.monitors.get(&monitor_id) {
        if let Some(ws) = monitor_ws.get_workspace(id) {
            for &win_id in &ws.windows {
                crate::platform::window::show_window(win_id.as_raw(), true);
            }
        }
    }

    state.apply_layout(monitor_id);
    IpcResponse::success(Some(json!({ "workspace": id })))
}

/// Move the focused window to the given workspace.
fn handle_move_to_workspace(id: u32, state: &mut AppState) -> IpcResponse {
    let focused_id = match state.window_manager.get_focused() {
        Some(id) => id,
        None => return IpcResponse::error("No focused window".to_string()),
    };

    // Determine the source monitor for later re-layout
    let source_monitor = state
        .workspace_manager
        .get_window_location(focused_id)
        .map(|(mon_id, _)| mon_id);

    if let Err(e) = state
        .workspace_manager
        .move_window_to_workspace(focused_id, id)
    {
        return IpcResponse::error(format!("Failed to move window: {}", e));
    }

    // Re-apply layouts on affected monitors
    if let Some(mon_id) = source_monitor {
        state.apply_layout(mon_id);
    }

    // Find which monitor now has workspace `id` and re-layout it
    for (&monitor_id, monitor_ws) in &state.workspace_manager.monitors {
        if monitor_ws.workspaces.iter().any(|ws| ws.id == id) {
            state.apply_layout(monitor_id);
            break;
        }
    }

    IpcResponse::success(Some(json!({ "workspace": id })))
}

// ---------------------------------------------------------------------------
// Config and lifecycle handlers
// ---------------------------------------------------------------------------

/// Reload the configuration file.
fn handle_reload_config(state: &mut AppState) -> IpcResponse {
    match state.reload_config_internal() {
        Ok(_) => IpcResponse::success(Some(json!({ "status": "reloaded" }))),
        Err(e) => {
            warn!("Config reload failed: {}", e);
            IpcResponse::error(format!("Config reload failed: {}", e))
        }
    }
}

/// Request the daemon to shut down.
fn handle_exit(state: &mut AppState) -> IpcResponse {
    state.running = false;
    IpcResponse::success(Some(json!({ "status": "exiting" })))
}
