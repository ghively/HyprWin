//! Core application coordinator for HyprTile.
//!
//! [`AppState`] holds the mutable runtime state shared across the application
//! and IPC handlers.  [`App`] wraps it with the Win32 event loop, hotkey
//! dispatch, and layout application.

use crate::config::ConfigManager;
use crate::config::types::Config;
use crate::layout::gaps::effective_gaps;
use crate::layout::{LayoutType, calculate_layout};
use crate::platform::dwm::{BorderColors, set_border_color};
use crate::platform::events::{EventHook, WindowEvent};
use crate::platform::input::HotkeyManager;
use crate::platform::monitor::{Monitor, enumerate_monitors, set_dpi_awareness};
use crate::platform::shutdown::{current_thread_id, post_quit_to_thread, run_message_pump};
use crate::platform::window::{
    WindowId,
    apply_tiled_positions,
    // â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
    // AI_AGENT_STOP: APP_COORDINATOR â€” This is the central event dispatch loop.
    // Before modifying event handling:
    //   1. All events flow through process_event() â€” add new cases there.
    //   2. handle_hotkey() dispatches all user actions â€” add new action strings.
    //   3. apply_layout() is called after every state change â€” keep it fast.
    //   4. Thread spawning/joining must stay balanced â€” never orphan threads.
    // â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
    close_window,
    enumerate_windows,
    focus_window,
};
use crate::platform::window::{should_manage_window, show_window};
use crate::util::dpi::{BASE_DPI, scale_rect_to_physical};
use crate::util::rect::Rect;
use crate::window::{WindowManager, filter, model::WindowState};
use crate::workspace::WorkspaceManager;
use crate::workspace::model::FocusDirection;

use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, RwLock};
use std::time::Duration;
use tokio::sync::Notify;
use tracing::{debug, error, info, warn};

// ---------------------------------------------------------------------------
// AppState -- shared mutable state accessible from IPC handlers
// ---------------------------------------------------------------------------

/// Runtime state of the HyprTile window manager.
///
/// Held by the main [`App`] and mutably borrowed by IPC command handlers.
/// All fields are public so that command handlers can inspect and modify
/// state directly.
pub struct AppState {
    /// Current configuration (may be hot-reloaded).
    pub config: Arc<RwLock<Config>>,
    /// Registry of all known windows and their states.
    pub window_manager: WindowManager,
    /// Workspace-to-monitor mapping and focus tracking.
    pub workspace_manager: WorkspaceManager,
    /// Detected monitors.
    pub monitors: Vec<Monitor>,
    /// Set to `false` to gracefully exit the event loop.
    pub running: bool,
    /// Shutdown signal for the TCP IPC server.
    pub tcp_shutdown: Arc<Notify>,
}

impl AppState {
    /// Create initial state from a loaded configuration.
    pub fn new(config: Config) -> Self {
        let window_manager = WindowManager::new(&config);
        let workspace_manager = WorkspaceManager::new();

        Self {
            config: Arc::new(RwLock::new(config)),
            window_manager,
            workspace_manager,
            monitors: Vec::new(),
            running: true,
            tcp_shutdown: Arc::new(Notify::new()),
        }
    }

    /// Replace the active configuration and propagate changes to subsystems.
    pub fn reload_config(&mut self, config: Config) {
        self.window_manager.reload_rules(&config);
        self.config = Arc::new(RwLock::new(config));
    }

    /// Return the monitor that currently contains the focused window,
    /// or the primary monitor as a fallback.
    pub fn get_focused_monitor(&self) -> Option<&Monitor> {
        let focused_window = self.window_manager.get_focused()?;

        // Fast path: look up which monitor the focused window is on
        if let Some((monitor_id, _ws_id)) =
            self.workspace_manager.get_window_location(focused_window)
        {
            return self.monitors.iter().find(|m| m.id == monitor_id);
        }

        // Fallback: check monitor rects
        let rect = focused_window.get_rect()?;
        let center = rect.center();
        self.monitors
            .iter()
            .find(|m| m.rect.contains(center))
            .or_else(|| self.monitors.first())
    }

    /// Apply tiling layout to the given monitor.
    ///
    /// 1. Retrieves the active workspace for the monitor.
    /// 2. Collects the tiling (non-floating, non-minimized) windows.
    /// 3. Calculates target positions using the active layout algorithm.
    /// 4. Batches the position changes via [`DeferredPositioner`].
    /// 5. Updates DWM border colors so the focused window is highlighted.
    pub fn apply_layout(&mut self, monitor_id: u32) {
        let workspace = match self.workspace_manager.get_active_workspace(monitor_id) {
            Some(ws) => ws,
            None => {
                debug!(
                    "No active workspace for monitor {}, skipping layout",
                    monitor_id
                );
                return;
            }
        };

        let workspace_id = workspace.id;
        let layout_type = workspace.layout_engine.current();

        // Collect tiling windows for this workspace
        let tiling_windows: Vec<WindowId> = workspace
            .windows
            .iter()
            .filter(|&&wid| {
                self.window_manager
                    .get_window(wid)
                    .map(|w| w.should_tile() && w.is_visible_and_managed())
                    .unwrap_or(false)
            })
            .copied()
            .collect();

        if tiling_windows.is_empty() {
            debug!(
                "No tiling windows on monitor {} workspace {}",
                monitor_id, workspace_id
            );
            return;
        }

        debug!(
            "Applying layout {:?} to monitor {} workspace {} ({} tiling windows)",
            layout_type,
            monitor_id,
            workspace_id,
            tiling_windows.len()
        );

        // Determine workspace rectangle (monitor work area with gaps)
        let monitor = match self.monitors.iter().find(|m| m.id == monitor_id) {
            Some(m) => m,
            None => {
                warn!("Monitor {} not found for layout", monitor_id);
                return;
            }
        };

        let gaps = self
            .config
            .read()
            .map(|c| c.gaps.clone())
            .unwrap_or_default();
        let (inner_gaps, outer_gaps) = effective_gaps(
            tiling_windows.len(),
            gaps.inner as i32,
            gaps.outer as i32,
            gaps.smart,
        );

        let workspace_rect = monitor.work_area_with_gaps(outer_gaps as u32);

        // Get focused window index for monocle layout
        let focused_idx = workspace
            .focused_window
            .and_then(|fw| tiling_windows.iter().position(|&w| w == fw))
            .unwrap_or(0);

        // Calculate layout
        let gaps_config = crate::config::types::GapsConfig {
            inner: inner_gaps as u32,
            outer: outer_gaps as u32,
            smart: gaps.smart,
        };
        let master_width_factor = workspace.master_width_factor;
        let positions = calculate_layout(
            layout_type,
            &tiling_windows,
            &workspace_rect,
            &gaps_config,
            focused_idx,
            master_width_factor,
        );

        // ------------------------------------------------------------------
        // DPI scaling: convert logical coordinates to physical pixels when
        // the monitor uses a non-standard DPI (e.g. 125%, 150%, 200%).
        // ------------------------------------------------------------------
        let monitor_dpi = monitor.dpi;
        let positions: Vec<(WindowId, Rect)> = if monitor_dpi != BASE_DPI {
            positions
                .into_iter()
                .map(|(wid, rect)| (wid, scale_rect_to_physical(&rect, monitor_dpi)))
                .collect()
        } else {
            positions
        };

        // ------------------------------------------------------------------
        // Animation wiring (initial implementation).
        // TODO: implement full timer-based animation loop. For now we set up
        // the interpolation path and fall through to immediate positioning
        // when animation is disabled or there are too many windows.
        // ------------------------------------------------------------------
        let animation_enabled = tiling_windows.len() <= 10;
        if animation_enabled {
            // Animation framework placeholder: interpolate_rect is available
            // for computing intermediate positions between the current window
            // rect and the target rect. A future update will run a timer loop
            // that calls interpolate_rect(from, to, eased_progress) for each
            // frame and applies the intermediate positions via the platform
            // layer until the animation completes.
            debug!(
                "Animation eligible for {} windows on monitor {} (interpolate_rect ready)",
                tiling_windows.len(),
                monitor_id
            );
        }

        // Apply positions via the platform-layer batch helper.
        for (window_id, rect) in &positions {
            debug!("  Positioning window {:?} at {:?}", window_id, rect);
        }
        if !apply_tiled_positions(&positions) {
            warn!(
                "Deferred window position commit failed on monitor {}",
                monitor_id
            );
        }

        // Apply DWM border colors
        self.apply_border_colors(monitor_id, &tiling_windows);
    }

    /// Apply layout to every monitor that has an active workspace.
    pub fn apply_all_layouts(&mut self) {
        let monitor_ids: Vec<u32> = self.monitors.iter().map(|m| m.id).collect();
        for monitor_id in monitor_ids {
            self.apply_layout(monitor_id);
        }
    }

    /// Apply DWM border colors so the focused window is highlighted and
    /// all others use the unfocused color.
    fn apply_border_colors(&self, _monitor_id: u32, windows: &[WindowId]) {
        let colors = BorderColors::default();
        let focused = self.window_manager.get_focused();

        for &wid in windows {
            let color = if focused == Some(wid) {
                colors.focused
            } else {
                colors.unfocused
            };
            if let Err(e) = set_border_color(wid.0, color) {
                debug!("Failed to set border color for {:?}: {}", wid, e);
            }
        }
    }

    /// Internal method used by IPC handlers to reload configuration.
    pub fn reload_config_internal(&mut self) -> anyhow::Result<()> {
        let config_manager = ConfigManager::load()?;
        let config = config_manager;
        info!("Configuration reloaded");
        self.reload_config(config);
        self.apply_all_layouts();
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// App -- event loop and lifecycle
// ---------------------------------------------------------------------------

/// Main application coordinator.
///
/// Owns the [`AppState`] and the cross-thread event channel.  Sets up the
/// Win32 hook, hotkey message loop, and drives the main event loop.
#[allow(dead_code)]
pub struct App {
    state: AppState,
    event_tx: Sender<WindowEvent>,
    event_rx: Receiver<WindowEvent>,
    config_manager: ConfigManager,
    /// Optional config path override from CLI.
    config_path: Option<std::path::PathBuf>,
    /// System tray icon (optional â€” creation may fail on headless systems).
    tray: Option<crate::platform::tray::TrayIcon>,
    /// Registered hotkey manager (kept alive for the lifetime of the app).
    hotkey_manager: Option<HotkeyManager>,
}

impl App {
    /// Initialise a new HyprTile application.
    ///
    /// 1. Loads (or creates) the configuration.
    /// 2. Sets per-monitor DPI awareness.
    /// 3. Enumerates connected monitors and initialises workspaces.
    /// 4. Enumerates existing top-level windows and registers the manageable ones.
    /// 5. Applies an initial layout to all monitors.
    pub fn new(config_path: Option<std::path::PathBuf>) -> anyhow::Result<Self> {
        info!("{} {} starting", crate::APP_NAME, crate::VERSION);

        // 1. Configuration
        let config_manager = match &config_path {
            Some(path) => ConfigManager::load_from_path(path)?,
            None => ConfigManager::load()?,
        };
        let config = config_manager.clone();

        // 2. DPI awareness
        set_dpi_awareness();
        info!("DPI awareness set");

        // 3. Initialise AppState
        let mut state = AppState::new(config);

        // 4. Enumerate monitors and create workspaces
        state.monitors = enumerate_monitors();
        info!("Detected {} monitor(s)", state.monitors.len());
        for monitor in &state.monitors {
            debug!(
                "  Monitor {}: {}x{} at ({},{})",
                monitor.id, monitor.rect.width, monitor.rect.height, monitor.rect.x, monitor.rect.y
            );
        }
        state.workspace_manager.init_monitors(&state.monitors);

        // 5. Enumerate existing windows
        let existing_windows = enumerate_windows();
        info!("Found {} top-level windows", existing_windows.len());

        for window_id in existing_windows {
            if should_manage_window(window_id.as_raw()) {
                debug!("Registering existing window {:?}", window_id);
                if let Some(window) = state.window_manager.register_window(window_id) {
                    // Determine which monitor this window belongs to
                    let monitor_id = state
                        .monitors
                        .iter()
                        .find(|m| m.contains_window(window_id.0))
                        .map(|m| m.id)
                        .unwrap_or_else(|| {
                            // Fallback to primary, then to the first known monitor.
                            // Never default to a hardcoded id (monitor counters start at 1).
                            state
                                .monitors
                                .iter()
                                .find(|m| m.is_primary)
                                .or_else(|| state.monitors.first())
                                .map(|m| m.id)
                                .unwrap_or(0)
                        });

                    if let Err(e) = state.workspace_manager.add_window(window_id, monitor_id) {
                        warn!(
                            "Failed to add window {:?} to workspace on monitor {}: {}",
                            window_id, monitor_id, e
                        );
                    }

                    // Apply window rules
                    if window.should_tile() {
                        debug!("Window {:?} should tile", window_id);
                    } else {
                        debug!("Window {:?} is floating per rules", window_id);
                    }
                }
            }
        }

        // 6. Apply initial layout
        state.apply_all_layouts();
        info!("Initial layout applied");

        // Event channel
        let (event_tx, event_rx) = channel();

        // Create system tray icon (non-fatal if it fails)
        let tray = match crate::platform::tray::TrayIcon::new(event_tx.clone()) {
            Ok(t) => {
                info!("System tray icon created");
                Some(t)
            }
            Err(e) => {
                warn!("Failed to create system tray icon: {}", e);
                None
            }
        };

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
    }

    /// Run the main event loop.
    ///
    /// 1. Starts the WinEventHook callback in a background thread.
    /// 2. Starts the hotkey message loop in another background thread.
    /// 3. Blocks on the internal channel, dispatching each [`WindowEvent`] to
    ///    the appropriate handler.
    /// 4. On exit, unregisters hooks and hotkeys.
    pub fn run(&mut self) -> anyhow::Result<()> {
        info!("Starting main event loop");

        // Clone the sender for the event hook thread
        let event_tx_for_hook = self.event_tx.clone();

        // Shared Win32 thread ID for the hook thread so we can post WM_QUIT on shutdown
        let hook_thread_id = Arc::new(std::sync::atomic::AtomicU32::new(0));
        let hook_thread_id_inner = Arc::clone(&hook_thread_id);

        // 1. Start WinEventHook thread
        let hook_handle = std::thread::Builder::new()
            .name("event-hook".to_string())
            .spawn(move || {
                // Store this thread's Win32 ID so the main thread can post WM_QUIT
                hook_thread_id_inner
                    .store(current_thread_id(), std::sync::atomic::Ordering::SeqCst);
                match EventHook::register(event_tx_for_hook) {
                    Ok(hook) => {
                        info!("WinEventHook registered");
                        // The hook callback runs on the same thread; keep it alive
                        // by running a Win32 message pump until WM_QUIT.
                        run_message_pump();
                        hook.unregister();
                    }
                    Err(e) => {
                        error!("Failed to register WinEventHook: {}", e);
                    }
                }
            })?;

        // 2. Start hotkey message loop thread (no nesting -- msg loop runs directly here)
        let event_tx_for_hotkey = self.event_tx.clone();
        let hotkey_shutdown = Arc::new(std::sync::atomic::AtomicBool::new(false));
        let hotkey_shutdown_clone = hotkey_shutdown.clone();
        let hotkey_handle = std::thread::Builder::new()
            .name("hotkey-loop".to_string())
            .spawn(move || {
                use crate::platform::input::run_message_loop;

                // Run the message loop directly in this thread -- no nested spawn
                // Actions are forwarded as WindowEvent::HotkeyAction through the channel
                let (action_tx, action_rx) = std::sync::mpsc::channel::<String>();

                // Spawn the actual Win32 message loop in a sub-thread so we can
                // monitor the shutdown flag and the action channel concurrently
                let msg_loop_handle = match std::thread::Builder::new()
                    .name("msg-loop".to_string())
                    .spawn(move || {
                        if let Err(e) = run_message_loop(action_tx) {
                            error!("Hotkey message loop error: {}", e);
                        }
                    }) {
                    Ok(h) => h,
                    Err(e) => {
                        error!("Failed to spawn message loop thread: {}", e);
                        return;
                    }
                };

                // Forward action strings as custom events through the main event channel
                loop {
                    match action_rx.recv_timeout(Duration::from_millis(100)) {
                        Ok(action) => {
                            debug!("Hotkey action received: {}", action);
                            if let Err(e) =
                                event_tx_for_hotkey.send(WindowEvent::HotkeyAction(action))
                            {
                                error!("Failed to send hotkey action to event channel: {}", e);
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                            if hotkey_shutdown_clone.load(std::sync::atomic::Ordering::SeqCst) {
                                break;
                            }
                        }
                        Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                            break;
                        }
                    }
                }

                // Join the inner message loop thread before exiting
                let _ = msg_loop_handle.join();
            })?;

        // 3. Start TCP IPC server for status bar integration
        let tcp_port = crate::DEFAULT_TCP_PORT;
        let tcp_shutdown = self.state.tcp_shutdown.clone();
        let tcp_shutdown_for_server = tcp_shutdown.clone();
        let _tcp_handle = tokio::spawn(async move {
            tokio::select! {
                biased;
                _ = tcp_shutdown.notified() => {
                    info!("TCP IPC server shutting down");
                }
                result = crate::ipc::start_tcp_server(tcp_port, tcp_shutdown_for_server) => {
                    if let Err(e) = result {
                        warn!("TCP IPC server error: {}", e);
                    }
                }
            }
        });

        // 4. Main event loop -- process events with a timeout so we can check `running`
        while self.state.running {
            match self.event_rx.recv_timeout(Duration::from_millis(100)) {
                Ok(event) => {
                    debug!("Received event: {:?}", event);
                    self.process_event(event);
                }
                Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {
                    // Normal -- check running flag and continue
                    continue;
                }
                Err(std::sync::mpsc::RecvTimeoutError::Disconnected) => {
                    warn!("Event channel disconnected");
                    break;
                }
            }
        }

        info!("Main event loop exiting");

        // 5. Graceful shutdown: signal threads to stop and join them
        hotkey_shutdown.store(true, std::sync::atomic::Ordering::SeqCst);
        self.state.tcp_shutdown.notify_waiters();

        // Signal the hook thread to exit by posting WM_QUIT to its Win32 message queue.
        post_quit_to_thread(hook_thread_id.load(std::sync::atomic::Ordering::SeqCst));

        // Join worker threads with a timeout to avoid hanging forever
        let _join_timeout = Duration::from_secs(2);
        let hook_join = hook_handle.join();
        if hook_join.is_err() {
            error!("Event hook thread panicked or failed to join");
        }
        let hotkey_join = hotkey_handle.join();
        if hotkey_join.is_err() {
            error!("Hotkey loop thread panicked or failed to join");
        }

        info!("Shutdown complete");
        Ok(())
    }

    // -----------------------------------------------------------------------
    // Event dispatch
    // -----------------------------------------------------------------------

    /// Route a [`WindowEvent`] to the appropriate specialised handler.
    fn process_event(&mut self, event: WindowEvent) {
        match event {
            WindowEvent::WindowCreated(hwnd) => self.handle_window_created(hwnd),
            WindowEvent::WindowDestroyed(hwnd) => self.handle_window_destroyed(hwnd),
            WindowEvent::WindowFocused(hwnd) => self.handle_window_focused(hwnd),
            WindowEvent::WindowMinimized(hwnd) => self.handle_window_minimized(hwnd),
            WindowEvent::WindowRestored(hwnd) => self.handle_window_restored(hwnd),
            WindowEvent::WindowMoved(hwnd) => self.handle_window_moved(hwnd),
            WindowEvent::WindowResized(hwnd) => self.handle_window_resized(hwnd),
            WindowEvent::MonitorChanged => self.handle_monitor_changed(),
            WindowEvent::DpiChanged => {
                // Re-enumerate monitors and re-apply layouts
                self.handle_monitor_changed();
            }
            WindowEvent::ExplorerRestarted => {
                info!("Explorer restarted, re-enumerating windows");
                self.reenumerate_windows();
            }
            WindowEvent::HotkeyAction(ref action) => {
                let action = action.clone();
                if let Err(e) = self.handle_hotkey(&action) {
                    warn!("Failed to handle hotkey action '{}': {}", action, e);
                }
            }
            other => {
                debug!("Unhandled event: {:?}", other);
            }
        }
    }

    // -----------------------------------------------------------------------
    // Per-event handlers
    // -----------------------------------------------------------------------

    /// A new window was created.  Register it, apply window rules, add it to
    /// the appropriate workspace, and re-layout.
    fn handle_window_created(&mut self, hwnd: WindowId) {
        debug!("Window created: {:?}", hwnd);

        // Early filter: use the window filter module for consistent management checks
        if !filter::should_manage(hwnd) {
            debug!("Window {:?} failed filter::should_manage, skipping", hwnd);
            return;
        }

        if !should_manage_window(hwnd.as_raw()) {
            return;
        }

        if let Some(window) = self.state.window_manager.register_window(hwnd) {
            // Determine target monitor
            let monitor_id = self
                .state
                .monitors
                .iter()
                .find(|m| m.contains_window(hwnd.0))
                .map(|m| m.id)
                .or_else(|| {
                    self.state
                        .monitors
                        .iter()
                        .find(|m| m.is_primary)
                        .map(|m| m.id)
                })
                .unwrap_or(0);

            if let Err(e) = self.state.workspace_manager.add_window(hwnd, monitor_id) {
                warn!("Failed to add window {:?} to workspace: {}", hwnd, e);
                return;
            }

            debug!(
                "Window {:?} (class='{}', title='{}') registered on monitor {}",
                hwnd, window.class_name, window.title, monitor_id
            );

            self.state.apply_layout(monitor_id);
        }
    }

    /// A window was destroyed.  Remove it from the workspace and window
    /// manager, then re-layout the monitor it was on.
    fn handle_window_destroyed(&mut self, hwnd: WindowId) {
        debug!("Window destroyed: {:?}", hwnd);

        let location = self.state.workspace_manager.remove_window(hwnd);
        self.state.window_manager.unregister_window(hwnd);

        if let Some((monitor_id, _workspace_id)) = location {
            self.state.apply_layout(monitor_id);
        }
    }

    /// A window gained focus.  Track it in the window manager and workspace.
    fn handle_window_focused(&mut self, hwnd: WindowId) {
        debug!("Window focused: {:?}", hwnd);

        self.state.window_manager.set_focused(hwnd);

        // Update focused window in the workspace too
        if let Some((_monitor_id, workspace_id)) =
            self.state.workspace_manager.get_window_location(hwnd)
        {
            // Collect monitor IDs that own this workspace first to avoid borrow issues
            let monitor_ids: Vec<u32> = self
                .state
                .workspace_manager
                .monitors
                .iter()
                .filter(|(_, mon_ws)| mon_ws.workspaces.iter().any(|ws| ws.id == workspace_id))
                .map(|(&mon_id, _)| mon_id)
                .collect();
            for mon_id in monitor_ids {
                if let Some(mw) = self.state.workspace_manager.monitors.get_mut(&mon_id)
                    && let Some(ws) = mw.get_workspace_mut(workspace_id)
                {
                    let _ = ws.focus_window(hwnd);
                }
                // Re-apply border colors on this monitor
                self.state.apply_layout(mon_id);
            }
        }
    }

    /// A window was minimised.  Remove it from the layout and mark it minimised.
    fn handle_window_minimized(&mut self, hwnd: WindowId) {
        debug!("Window minimized: {:?}", hwnd);

        if let Some(window) = self.state.window_manager.get_window_mut(hwnd) {
            window.minimize();
        }

        if let Some((monitor_id, _)) = self.state.workspace_manager.get_window_location(hwnd) {
            self.state.apply_layout(monitor_id);
        }
    }

    /// A window was restored from minimised state.  Return it to the layout.
    fn handle_window_restored(&mut self, hwnd: WindowId) {
        debug!("Window restored: {:?}", hwnd);

        if let Some(window) = self.state.window_manager.get_window_mut(hwnd) {
            window.restore();
        }

        if let Some((monitor_id, _)) = self.state.workspace_manager.get_window_location(hwnd) {
            self.state.apply_layout(monitor_id);
        }
    }

    /// A window was moved by the user.  If it is currently tiled, float it so
    /// the user's manual positioning is respected.
    fn handle_window_moved(&mut self, hwnd: WindowId) {
        debug!("Window moved: {:?}", hwnd);

        if let Some(window) = self.state.window_manager.get_window(hwnd)
            && window.state == WindowState::Tiling
        {
            // User moved a tiled window -- float it
            info!("User moved tiled window {:?} -- converting to float", hwnd);
            let _ = window;
            self.state.window_manager.toggle_float(hwnd);

            if let Some((monitor_id, _)) = self.state.workspace_manager.get_window_location(hwnd) {
                self.state.apply_layout(monitor_id);
            }
        }
    }

    /// A window was resized by the user.  Similar to move, respect user action.
    fn handle_window_resized(&mut self, hwnd: WindowId) {
        debug!("Window resized: {:?}", hwnd);

        if let Some(window) = self.state.window_manager.get_window(hwnd)
            && window.state == WindowState::Tiling
        {
            // User resized a tiled window -- float it
            info!(
                "User resized tiled window {:?} -- converting to float",
                hwnd
            );
            let _ = window;
            self.state.window_manager.toggle_float(hwnd);

            if let Some((monitor_id, _)) = self.state.workspace_manager.get_window_location(hwnd) {
                self.state.apply_layout(monitor_id);
            }
        }
    }

    /// Monitor configuration changed (added, removed, or resolution/DPI change).
    /// Re-enumerate monitors and redistribute windows.
    fn handle_monitor_changed(&mut self) {
        info!("Monitor configuration changed");

        let old_monitors: Vec<Monitor> = self.state.monitors.clone();
        self.state.monitors = enumerate_monitors();

        info!(
            "Monitor count changed from {} to {}",
            old_monitors.len(),
            self.state.monitors.len()
        );

        // In a full implementation this would:
        // - Detect which monitors were added/removed by comparing old/new lists
        // - Migrate windows from removed monitors to remaining ones
        // - Initialise workspaces only for newly connected monitors
        // For now, re-init all monitors and re-apply layouts as a safe fallback

        self.state
            .workspace_manager
            .init_monitors(&self.state.monitors);
        self.state.apply_all_layouts();
    }

    // -----------------------------------------------------------------------
    // Hotkey action handlers
    // -----------------------------------------------------------------------

    /// Dispatch a hotkey action string to the appropriate handler.
    pub fn handle_hotkey(&mut self, action: &str) -> anyhow::Result<()> {
        debug!("Handling hotkey action: {}", action);

        match action {
            "exec_terminal" => self.exec_terminal(),
            "close_window" => self.close_focused_window(),
            "focus_left" => {
                self.focus_direction(FocusDirection::Left);
                Ok(())
            }
            "focus_right" => {
                self.focus_direction(FocusDirection::Right);
                Ok(())
            }
            "focus_up" => {
                self.focus_direction(FocusDirection::Up);
                Ok(())
            }
            "focus_down" => {
                self.focus_direction(FocusDirection::Down);
                Ok(())
            }
            "move_left" => {
                self.move_direction(FocusDirection::Left);
                Ok(())
            }
            "move_right" => {
                self.move_direction(FocusDirection::Right);
                Ok(())
            }
            "move_up" => {
                self.move_direction(FocusDirection::Up);
                Ok(())
            }
            "move_down" => {
                self.move_direction(FocusDirection::Down);
                Ok(())
            }
            "toggle_float" => {
                self.toggle_float();
                Ok(())
            }
            "toggle_fullscreen" => {
                self.toggle_fullscreen();
                Ok(())
            }
            "cycle_layout" => {
                self.cycle_layout();
                Ok(())
            }
            "resize_increase" => {
                self.resize_split(0.05);
                Ok(())
            }
            "resize_decrease" => {
                self.resize_split(-0.05);
                Ok(())
            }
            "reload_config" => {
                self.reload_config();
                Ok(())
            }
            "tray_show" => {
                info!("Show action triggered from system tray");
                // Currently a no-op: HyprTile is a background service.
                // Future: could show a status window.
                Ok(())
            }
            "exit" => {
                self.exit();
                Ok(())
            }
            other => {
                // Check for workspace_N and move_to_workspace_N patterns.
                // Key "0" maps to workspace 10 for ergonomic 1-0 key row access.
                if let Some(rest) = other.strip_prefix("workspace_")
                    && let Ok(id) = rest.parse::<u32>()
                {
                    let id = if id == 0 { 10 } else { id };
                    let max_ws = self
                        .state
                        .config
                        .read()
                        .map(|c| c.workspaces.count)
                        .unwrap_or(10);
                    let clamped = id.clamp(1, max_ws);
                    if clamped != id {
                        warn!("Workspace ID {} out of range, clamped to {}", id, clamped);
                    }
                    self.switch_workspace(clamped);
                    return Ok(());
                }
                if let Some(rest) = other.strip_prefix("move_to_workspace_")
                    && let Ok(id) = rest.parse::<u32>()
                {
                    let id = if id == 0 { 10 } else { id };
                    let max_ws = self
                        .state
                        .config
                        .read()
                        .map(|c| c.workspaces.count)
                        .unwrap_or(10);
                    let clamped = id.clamp(1, max_ws);
                    if clamped != id {
                        warn!("Workspace ID {} out of range, clamped to {}", id, clamped);
                    }
                    self.move_to_workspace(clamped);
                    return Ok(());
                }
                warn!("Unknown hotkey action: {}", other);
                Err(anyhow::anyhow!("Unknown action: {}", other))
            }
        }
    }

    /// Launch the configured terminal emulator.
    fn exec_terminal(&self) -> anyhow::Result<()> {
        let config = self
            .state
            .config
            .read()
            .map_err(|e| anyhow::anyhow!("Config lock poisoned: {}", e))?;
        let terminal = config.general.terminal.clone();
        drop(config);

        info!("Executing terminal: {}", terminal);

        std::process::Command::new(&terminal)
            .spawn()
            .map_err(|e| anyhow::anyhow!("Failed to launch terminal '{}': {}", terminal, e))?;

        Ok(())
    }

    /// Close the currently focused window.
    fn close_focused_window(&self) -> anyhow::Result<()> {
        if let Some(focused) = self.state.window_manager.get_focused() {
            info!("Closing focused window {:?}", focused);
            close_window(focused.as_raw());
            Ok(())
        } else {
            warn!("No focused window to close");
            Err(anyhow::anyhow!("No focused window"))
        }
    }

    /// Move focus in the given direction.
    ///
    /// Computes the centre point of every tiling window on the current monitor,
    /// then finds the closest window in the requested direction using a
    /// weighted distance that strongly prefers alignment on the target axis.
    fn focus_direction(&mut self, direction: FocusDirection) {
        let monitor_id = match self.state.get_focused_monitor() {
            Some(m) => m.id,
            None => {
                warn!("No focused monitor for directional focus");
                return;
            }
        };

        // Use the workspace manager's cycle_focus for basic cycling
        self.state
            .workspace_manager
            .cycle_focus(monitor_id, direction);

        // Focus the window that is now focused in the workspace
        if let Some(ws) = self
            .state
            .workspace_manager
            .get_active_workspace(monitor_id)
            && let Some(focused) = ws.focused_window
        {
            focus_window(focused.as_raw());
        }

        // Re-apply layout to update border colors
        self.state.apply_layout(monitor_id);
    }

    /// Move the focused window in the given direction.
    ///
    /// If the window is at the edge of the workspace, it may be moved to an
    /// adjacent monitor's workspace when available.
    fn move_direction(&mut self, direction: FocusDirection) {
        let focused_id = match self.state.window_manager.get_focused() {
            Some(id) => id,
            None => {
                warn!("No focused window to move");
                return;
            }
        };

        let current_monitor_id = match self.state.workspace_manager.get_window_location(focused_id)
        {
            Some((mon_id, _)) => mon_id,
            None => {
                warn!(
                    "Focused window {:?} not tracked in any workspace",
                    focused_id
                );
                return;
            }
        };

        // Find an adjacent monitor in the given direction
        let current_monitor = match self
            .state
            .monitors
            .iter()
            .find(|m| m.id == current_monitor_id)
        {
            Some(m) => m.clone(),
            None => return,
        };

        let current_center = current_monitor.rect.center();

        let target_monitor = self
            .state
            .monitors
            .iter()
            .filter(|m| m.id != current_monitor_id)
            .min_by(|a, b| {
                let a_dist =
                    weighted_directional_distance(current_center, a.rect.center(), &direction);
                let b_dist =
                    weighted_directional_distance(current_center, b.rect.center(), &direction);
                a_dist
                    .partial_cmp(&b_dist)
                    .unwrap_or(std::cmp::Ordering::Equal)
            })
            .cloned();

        if let Some(target) = target_monitor {
            // Read the target workspace before any mutable borrows
            let target_workspace = self
                .state
                .workspace_manager
                .monitors
                .get(&target.id)
                .map(|mw| mw.active_workspace);

            if let Some(tw) = target_workspace {
                if let Err(e) = self
                    .state
                    .workspace_manager
                    .move_window_to_workspace(focused_id, tw)
                {
                    warn!("Failed to move window to workspace: {}", e);
                    return;
                }
                let _ = self
                    .state
                    .workspace_manager
                    .move_window_to_monitor(focused_id, target.id);
            }

            self.state.apply_layout(current_monitor_id);
            self.state.apply_layout(target.id);
        } else {
            // No adjacent monitor -- cycle within the current workspace
            self.state
                .workspace_manager
                .cycle_focus(current_monitor_id, direction);
            if let Some(ws) = self
                .state
                .workspace_manager
                .get_active_workspace(current_monitor_id)
                && let Some(focused) = ws.focused_window
            {
                focus_window(focused.as_raw());
            }
            self.state.apply_layout(current_monitor_id);
        }
    }

    /// Switch the focused monitor to the given workspace.
    fn switch_workspace(&mut self, id: u32) {
        let monitor_id = self.state.get_focused_monitor().map(|m| m.id).unwrap_or(0);

        if let Err(e) = self
            .state
            .workspace_manager
            .switch_workspace(monitor_id, id)
        {
            warn!("Failed to switch to workspace {}: {}", id, e);
            return;
        }

        // Show/hide windows based on workspace visibility
        if let Some(mon_ws) = self.state.workspace_manager.monitors.get(&monitor_id) {
            // Hide windows on inactive workspaces
            for ws in &mon_ws.workspaces {
                let visible = ws.id == id;
                for &win_id in &ws.windows {
                    if let Some(window) = self.state.window_manager.get_window(win_id)
                        && window.is_visible_and_managed()
                    {
                        show_window(win_id.as_raw(), visible);
                    }
                }
            }
        }

        info!("Switched monitor {} to workspace {}", monitor_id, id);
        self.state.apply_layout(monitor_id);
    }

    /// Move the focused window to the given workspace.
    fn move_to_workspace(&mut self, id: u32) {
        let focused_id = match self.state.window_manager.get_focused() {
            Some(id) => id,
            None => {
                warn!("No focused window to move");
                return;
            }
        };

        let source_monitor = self
            .state
            .workspace_manager
            .get_window_location(focused_id)
            .map(|(mon_id, _)| mon_id);

        if let Err(e) = self
            .state
            .workspace_manager
            .move_window_to_workspace(focused_id, id)
        {
            warn!("Failed to move window to workspace {}: {}", id, e);
            return;
        }

        if let Some(mon_id) = source_monitor {
            self.state.apply_layout(mon_id);
        }

        // Find which monitor now hosts workspace `id`
        let target_monitor_ids: Vec<u32> = self
            .state
            .workspace_manager
            .monitors
            .iter()
            .filter(|(_, monitor_ws)| monitor_ws.workspaces.iter().any(|ws| ws.id == id))
            .map(|(&monitor_id, _)| monitor_id)
            .collect();
        for monitor_id in target_monitor_ids {
            self.state.apply_layout(monitor_id);
        }

        info!("Moved window {:?} to workspace {}", focused_id, id);
    }

    /// Cycle the layout on the active workspace of the focused monitor.
    fn cycle_layout(&mut self) {
        let monitor_id = self.state.get_focused_monitor().map(|m| m.id).unwrap_or(0);

        if let Some(ws) = self
            .state
            .workspace_manager
            .get_active_workspace_mut(monitor_id)
        {
            let new_layout = ws.layout_engine.cycle();
            info!(
                "Cycled layout on monitor {} to {:?}",
                monitor_id, new_layout
            );
            let _ = ws;
            self.state.apply_layout(monitor_id);
        }
    }

    /// Adjust the split ratio of the active workspace's layout.
    ///
    /// For master-stack layouts this changes the master area width factor.
    /// For dwindle layouts this changes the default split ratio used when
    /// building the BSP tree. The delta is clamped to [0.1, 0.9] to prevent
    /// windows from collapsing to zero size.
    fn resize_split(&mut self, delta: f64) {
        let monitor_id = self.state.get_focused_monitor().map(|m| m.id).unwrap_or(0);

        let Some(ws) = self
            .state
            .workspace_manager
            .get_active_workspace_mut(monitor_id)
        else {
            warn!("No active workspace on monitor {} for resize", monitor_id);
            return;
        };

        let layout_type = ws.layout_engine.current();
        match layout_type {
            LayoutType::MasterStack => {
                let new_ratio = (ws.master_width_factor + delta).clamp(0.1, 0.9);
                if (new_ratio - ws.master_width_factor).abs() > f64::EPSILON {
                    ws.master_width_factor = new_ratio;
                    info!(
                        "Resized master-stack split on monitor {} to {:.2}",
                        monitor_id, new_ratio
                    );
                    let _ = ws;
                    self.state.apply_layout(monitor_id);
                }
            }
            LayoutType::Dwindle => {
                let new_ratio = (ws.dwindle_ratio + delta).clamp(0.1, 0.9);
                if (new_ratio - ws.dwindle_ratio).abs() > f64::EPSILON {
                    ws.dwindle_ratio = new_ratio;
                    info!(
                        "Resized dwindle split on monitor {} to {:.2}",
                        monitor_id, new_ratio
                    );
                    // TODO: persist the BSP tree with adjusted ratios so that
                    // resize_increase/resize_decrease can target specific splits
                    // based on the focused window. For now we adjust the default
                    // ratio used when new splits are created.
                    let _ = ws;
                    self.state.apply_layout(monitor_id);
                }
            }
            LayoutType::Monocle | LayoutType::Grid => {
                debug!("Resize not applicable for {:?} layout", layout_type);
            }
        }
    }

    /// Toggle floating state on the focused window.
    fn toggle_float(&mut self) {
        if let Some(focused) = self.state.window_manager.get_focused() {
            match self.state.window_manager.toggle_float(focused) {
                Some(new_state) => {
                    info!("Window {:?} is now {:?}", focused, new_state);
                }
                None => warn!("Failed to toggle float for {:?}", focused),
            }

            if let Some((monitor_id, _)) = self.state.workspace_manager.get_window_location(focused)
            {
                self.state.apply_layout(monitor_id);
            } else {
                self.state.apply_all_layouts();
            }
        }
    }

    /// Toggle fullscreen state on the focused window.
    fn toggle_fullscreen(&mut self) {
        if let Some(focused) = self.state.window_manager.get_focused() {
            match self.state.window_manager.toggle_fullscreen(focused) {
                Some(new_state) => {
                    info!("Window {:?} is now {:?}", focused, new_state);
                }
                None => warn!("Failed to toggle fullscreen for {:?}", focused),
            }

            if let Some((monitor_id, _)) = self.state.workspace_manager.get_window_location(focused)
            {
                self.state.apply_layout(monitor_id);
            } else {
                self.state.apply_all_layouts();
            }
        }
    }

    /// Reload the configuration from disk.
    fn reload_config(&mut self) {
        info!("Reloading configuration");

        let config_result = match &self.config_path {
            Some(path) => ConfigManager::load_from_path(path),
            None => ConfigManager::load(),
        };

        match config_result {
            Ok(config) => {
                self.state.reload_config(config);
                info!("Configuration reloaded successfully");
                self.state.apply_all_layouts();
            }
            Err(e) => {
                error!("Failed to reload configuration: {}", e);
            }
        }
    }

    /// Set the running flag to false so the event loop exits.
    fn exit(&mut self) {
        info!("Exit requested");
        self.state.running = false;
    }

    // -----------------------------------------------------------------------
    // Helpers
    // -----------------------------------------------------------------------

    /// Re-enumerate all top-level windows.  Used after Explorer restart.
    fn reenumerate_windows(&mut self) {
        let existing = enumerate_windows();
        for hwnd in existing {
            if should_manage_window(hwnd.as_raw())
                && self.state.window_manager.get_window(hwnd).is_none()
            {
                debug!("Registering newly discovered window {:?}", hwnd);
                if let Some(_window) = self.state.window_manager.register_window(hwnd) {
                    let monitor_id = self
                        .state
                        .monitors
                        .iter()
                        .find(|m| m.contains_window(hwnd.0))
                        .map(|m| m.id)
                        .unwrap_or(0);
                    let _ = self.state.workspace_manager.add_window(hwnd, monitor_id);
                }
            }
        }
        self.state.apply_all_layouts();
    }
}

// ---------------------------------------------------------------------------
// Distance helpers
// ---------------------------------------------------------------------------

/// Compute a weighted directional distance between two centre points.
///
/// Movement opposite to the requested direction returns `INFINITY` so that
/// only windows *in* the direction are considered.  Alignment on the target
/// axis is strongly preferred (2x weight) over orthogonal distance.
fn weighted_directional_distance(from: (i32, i32), to: (i32, i32), dir: &FocusDirection) -> f64 {
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
            // For Next/Previous, use simple distance (no directional bias)
            (dx * dx + dy * dy).sqrt()
        }
    }
}
