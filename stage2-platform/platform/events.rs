use std::sync::mpsc::Sender;
use std::sync::OnceLock;
use std::thread;
use std::time::Instant;
use tracing::{debug, error, info, trace, warn};
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Accessibility::{
    SetWinEventHook, UnhookWinEvent, HWINEVENTHOOK,
    EVENT_OBJECT_CREATE, EVENT_OBJECT_DESTROY, EVENT_OBJECT_HIDE, EVENT_OBJECT_LOCATIONCHANGE,
    EVENT_OBJECT_NAMECHANGE, EVENT_OBJECT_SHOW, EVENT_SYSTEM_MINIMIZEEND,
    EVENT_SYSTEM_MINIMIZESTART, EVENT_SYSTEM_MOVESIZEEND, EVENT_SYSTEM_MOVESIZESTART,
};
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::platform::window::WindowId;

/// Represents a significant change in window state that the WM needs to handle.
#[derive(Debug, Clone, PartialEq)]
pub enum WindowEvent {
    /// A new window was created.
    WindowCreated(WindowId),
    /// A window was destroyed.
    WindowDestroyed(WindowId),
    /// A window became visible.
    WindowShown(WindowId),
    /// A window was hidden.
    WindowHidden(WindowId),
    /// A window was minimized.
    WindowMinimized(WindowId),
    /// A window was restored from minimized.
    WindowRestored(WindowId),
    /// A window is being moved by the user.
    WindowMoved(WindowId),
    /// A window is being resized by the user.
    WindowResized(WindowId),
    /// A window received focus.
    WindowFocused(WindowId),
    /// A window title changed.
    WindowRenamed(WindowId),
    /// Monitor configuration changed.
    MonitorChanged,
    /// DPI scaling changed.
    DpiChanged,
    /// Windows Explorer restarted (taskbar recreated).
    ExplorerRestarted,
}

static EVENT_SENDER: OnceLock<Sender<WindowEvent>> = OnceLock::new();

/// Handle to a registered WinEvent hook. Unhooks automatically on drop.
pub struct EventHook {
    hook: HWINEVENTHOOK,
}

impl EventHook {
    /// Register a SetWinEventHook for all window events we care about.
    pub fn register(event_tx: Sender<WindowEvent>) -> anyhow::Result<Self> {
        let _ = EVENT_SENDER.set(event_tx);

        unsafe {
            let hook = SetWinEventHook(
                EVENT_OBJECT_CREATE,
                EVENT_OBJECT_NAMECHANGE,
                None,
                Some(event_hook_callback),
                0,
                0,
                WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
            );

            if hook.is_invalid() {
                anyhow::bail!("SetWinEventHook failed");
            }

            info!("WinEvent hook registered successfully");
            Ok(Self { hook })
        }
    }

    /// Unregister the WinEvent hook.
    pub fn unregister(&self) {
        unsafe {
            let result = UnhookWinEvent(self.hook);
            if result.as_bool() {
                debug!("WinEvent hook unregistered");
            } else {
                warn!("Failed to unregister WinEvent hook");
            }
        }
    }
}

impl Drop for EventHook {
    fn drop(&mut self) {
        self.unregister();
    }
}

/// WinEventHook callback — receives raw Win32 accessibility events and classifies them.
///
/// This is called from the OS on a background thread. We filter out irrelevant
/// events (carets, cursors, sound objects) and forward window-level events
/// to the application channel.
pub extern "system" fn event_hook_callback(
    _hook: HWINEVENTHOOK,
    event: u32,
    hwnd: HWND,
    id_object: i32,
    id_child: i32,
    _id_event_thread: u32,
    _dwms_event_time: u32,
) {
    // We only care about window-level objects, not child objects
    if id_object != OBJID_WINDOW.0 as i32 && id_object != 0 {
        return;
    }
    if id_child != 0 {
        return;
    }
    if hwnd.is_invalid() {
        return;
    }

    // Ignore null HWND events
    if hwnd.0 == 0 {
        return;
    }

    if let Some(window_event) = classify_event(event, hwnd) {
        trace!("Classified event: {:?} for hwnd=0x{:X}", window_event, hwnd.0);

        if let Some(tx) = EVENT_SENDER.get() {
            if let Err(e) = tx.send(window_event) {
                warn!("Failed to send event to channel: {}", e);
            }
        }
    }
}

/// Convert a raw Win32 event constant into our typed `WindowEvent` enum.
pub fn classify_event(event: u32, hwnd: HWND) -> Option<WindowEvent> {
    let window_id = WindowId::from_raw(hwnd);

    match event {
        EVENT_OBJECT_CREATE => Some(WindowEvent::WindowCreated(window_id)),
        EVENT_OBJECT_DESTROY => Some(WindowEvent::WindowDestroyed(window_id)),
        EVENT_OBJECT_SHOW => Some(WindowEvent::WindowShown(window_id)),
        EVENT_OBJECT_HIDE => Some(WindowEvent::WindowHidden(window_id)),
        EVENT_SYSTEM_MINIMIZESTART => Some(WindowEvent::WindowMinimized(window_id)),
        EVENT_SYSTEM_MINIMIZEEND => Some(WindowEvent::WindowRestored(window_id)),
        EVENT_SYSTEM_MOVESIZESTART => Some(WindowEvent::WindowMoved(window_id)),
        EVENT_SYSTEM_MOVESIZEEND => Some(WindowEvent::WindowResized(window_id)),
        EVENT_OBJECT_LOCATIONCHANGE => {
            unsafe {
                if GetForegroundWindow() == hwnd {
                    // Could be a focus change or just a move
                    Some(WindowEvent::WindowMoved(window_id))
                } else {
                    Some(WindowEvent::WindowMoved(window_id))
                }
            }
        }
        EVENT_OBJECT_NAMECHANGE => Some(WindowEvent::WindowRenamed(window_id)),
        // Handle focus events that may come through as locationchange
        _ => {
            trace!("Unhandled WinEvent 0x{:X} for hwnd=0x{:X}", event, hwnd.0);
            None
        }
    }
}

/// Start the event processing loop in its own thread.
///
/// This creates a hidden message window and runs a standard `GetMessage` loop.
/// The OS delivers WinEventHook callbacks on this thread, and we also receive
/// `WM_DISPLAYCHANGE` / `WM_DPICHANGED` messages here.
pub fn start_event_loop(event_tx: Sender<WindowEvent>) -> anyhow::Result<()> {
    let tx = event_tx.clone();

    thread::Builder::new()
        .name("hyprtile-events".to_string())
        .spawn(move || {
            info!("Event processing loop started");

            unsafe {
                // Create a message-only window
                let hwnd = match CreateWindowExW(
                    WS_EX_NOACTIVATE,
                    windows::w!("Message"),
                    None,
                    WS_OVERLAPPED,
                    0, 0, 0, 0,
                    HWND_MESSAGE,
                    None,
                    None,
                    None,
                ) {
                    Ok(h) => h,
                    Err(e) => {
                        error!("Failed to create event loop message window: {}", e);
                        return;
                    }
                };

                // Store hwnd for later use
                debug!("Event loop message window created: {:?}", hwnd);

                // Message loop
                let mut msg = MSG::default();
                while GetMessageW(&mut msg, Some(HWND(0)), 0, 0).as_bool() {
                    TranslateMessage(&msg);
                    DispatchMessageW(&msg);

                    // Handle display change
                    if msg.message == WM_DISPLAYCHANGE {
                        info!("WM_DISPLAYCHANGE received — monitor configuration changed");
                        let _ = tx.send(WindowEvent::MonitorChanged);
                    }

                    // Handle DPI change
                    if msg.message == WM_DPICHANGED {
                        info!("WM_DPICHANGED received — DPI changed");
                        let _ = tx.send(WindowEvent::DpiChanged);
                    }
                }

                info!("Event processing loop exiting");
            }
        })?;

    Ok(())
}

/// Debounce rapid sequences of events (e.g. rapid focus changes during
/// workspace switches) to avoid unnecessary layout calculations.
pub struct EventDebouncer {
    threshold_ms: u64,
    max_events: usize,
    first_event_time: Option<Instant>,
    event_count: usize,
}

impl EventDebouncer {
    /// Create a new debouncer with the given time threshold and maximum event count.
    pub fn new(threshold_ms: u64, max_events: usize) -> Self {
        Self {
            threshold_ms,
            max_events,
            first_event_time: None,
            event_count: 0,
        }
    }

    /// Determine whether the current event should be debounced.
    ///
    /// Returns `true` if the event should be suppressed, `false` otherwise.
    /// Call this with the current event count and elapsed time since the first event.
    pub fn should_debounce(&mut self, event_count: usize, elapsed_ms: u64) -> bool {
        match self.first_event_time {
            None => {
                self.first_event_time = Some(Instant::now());
                self.event_count = event_count;
                false
            }
            Some(start) => {
                let elapsed = start.elapsed().as_millis() as u64;

                if elapsed > self.threshold_ms {
                    // Reset — outside the debounce window
                    self.first_event_time = Some(Instant::now());
                    self.event_count = event_count;
                    false
                } else if event_count >= self.max_events {
                    // Inside window but event count exceeded — debounce
                    debug!(
                        "Debouncing {} events within {}ms",
                        event_count, elapsed
                    );
                    true
                } else {
                    // Inside window, count below threshold — allow
                    false
                }
            }
        }
    }

    /// Reset the debouncer state.
    pub fn reset(&mut self) {
        self.first_event_time = None;
        self.event_count = 0;
    }
}
ount = 0;
    }
}
