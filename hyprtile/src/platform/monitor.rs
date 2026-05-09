use std::mem;
use std::sync::atomic::{AtomicU32, Ordering};
use tracing::{debug, info, warn};
use windows::Win32::Foundation::{BOOL, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Gdi::{
    // ═══════════════════════════════════════════════════════════════════════════════
    // AI_AGENT_STOP: MONITOR_LIFECYCLE — Monitor enumeration and DPI awareness.
    // Before modifying monitor handling:
    //   1. set_dpi_awareness() MUST be called before any window enumeration.
    //   2. Per-monitor DPI requires GetDpiForMonitor, not GetDpiForSystem.
    //   3. Handle monitor disconnect by redistributing windows to remaining monitors.
    //   4. work_area excludes the taskbar — use it, not rect, for tiling area.
    //   5. Monitor IDs are assigned sequentially and may change on re-enumeration.
    // ═══════════════════════════════════════════════════════════════════════════════
    EnumDisplayMonitors,
    GetMonitorInfoW,
    HDC,
    HMONITOR,
    MONITOR_DEFAULTTONEAREST,
    MONITORINFO,
    MONITORINFOEXW,
    MonitorFromWindow,
};
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, GetDpiForMonitor, MDT_EFFECTIVE_DPI,
    SetThreadDpiAwarenessContext,
};

use crate::util::rect::Rect;

/// Represents a display monitor with its properties.
#[derive(Debug, Clone)]
pub struct Monitor {
    /// HMONITOR handle
    pub handle: isize,
    /// Monitor ID (sequential index)
    pub id: u32,
    /// Full monitor rectangle
    pub rect: Rect,
    /// Work area rectangle (minus taskbar)
    pub work_area: Rect,
    /// DPI value (effective)
    pub dpi: u32,
    /// Whether this is the primary monitor
    pub is_primary: bool,
    /// Monitor display name
    pub name: String,
}

static MONITOR_COUNTER: AtomicU32 = AtomicU32::new(1);

impl Monitor {
    /// Create a Monitor from an HMONITOR handle.
    pub fn from_hmonitor(hmonitor: isize) -> Option<Self> {
        let mut info = MONITORINFOEXW {
            monitorInfo: MONITORINFO {
                cbSize: mem::size_of::<MONITORINFOEXW>() as u32,
                rcMonitor: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                rcWork: RECT {
                    left: 0,
                    top: 0,
                    right: 0,
                    bottom: 0,
                },
                dwFlags: 0,
            },
            szDevice: [0u16; 32],
        };

        unsafe {
            let result = GetMonitorInfoW(
                HMONITOR(hmonitor as *mut _),
                &mut info as *mut _ as *mut MONITORINFO,
            );
            if !result.as_bool() {
                return None;
            }
        }

        let rect = Rect::from_win32(&info.monitorInfo.rcMonitor);
        let work_area = Rect::from_win32(&info.monitorInfo.rcWork);
        let is_primary = info.monitorInfo.dwFlags == 1;
        let name = String::from_utf16_lossy(
            &info.szDevice[..info.szDevice.iter().position(|&c| c == 0).unwrap_or(32)],
        );

        let mut dpi_x: u32 = 96;
        let mut _dpi_y: u32 = 96;
        unsafe {
            let result = GetDpiForMonitor(
                HMONITOR(hmonitor as *mut _),
                MDT_EFFECTIVE_DPI,
                &mut dpi_x,
                &mut _dpi_y,
            );
            if result.is_err() {
                dpi_x = 96;
            }
        }

        Some(Monitor {
            handle: hmonitor,
            id: MONITOR_COUNTER.fetch_add(1, Ordering::SeqCst),
            rect,
            work_area,
            dpi: dpi_x,
            is_primary,
            name,
        })
    }

    /// Check if this monitor contains the given window.
    pub fn contains_window(&self, hwnd: isize) -> bool {
        let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut _);
        unsafe {
            let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
            if hmonitor.is_invalid() {
                return false;
            }
            hmonitor.0 as isize == self.handle
        }
    }

    /// Get the work area rectangle reduced by outer gaps.
    pub fn work_area_with_gaps(&self, outer_gaps: u32) -> Rect {
        self.work_area.adjust_for_gaps(0, outer_gaps as i32, false)
    }
}

/// Enumerate all connected monitors.
pub fn enumerate_monitors() -> Vec<Monitor> {
    let mut monitors: Vec<Monitor> = Vec::new();

    unsafe {
        let _ = EnumDisplayMonitors(
            None,
            None,
            Some(enum_monitors_callback),
            LPARAM(&mut monitors as *mut Vec<Monitor> as isize),
        );
    }

    monitors.sort_by_key(|m| if m.is_primary { 0 } else { m.id });
    monitors
}

extern "system" fn enum_monitors_callback(
    _hmonitor: HMONITOR,
    _hdc: HDC,
    _clip_rect: *mut RECT,
    lparam: LPARAM,
) -> BOOL {
    let monitors = unsafe { &mut *(lparam.0 as *mut Vec<Monitor>) };

    if let Some(monitor) = Monitor::from_hmonitor(_hmonitor.0 as isize) {
        monitors.push(monitor);
    }

    TRUE
}

/// Get the monitor that contains the specified window.
pub fn get_monitor_for_window(hwnd: isize) -> Option<Monitor> {
    let hwnd = windows::Win32::Foundation::HWND(hwnd as *mut _);
    unsafe {
        let hmonitor = MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST);
        if hmonitor.is_invalid() {
            return None;
        }
        Monitor::from_hmonitor(hmonitor.0 as isize)
    }
}

/// Get the primary monitor.
pub fn get_primary_monitor() -> Option<Monitor> {
    let monitors = enumerate_monitors();
    monitors.into_iter().find(|m| m.is_primary)
}

/// Get a monitor by its ID.
pub fn get_monitor_by_id(id: u32) -> Option<Monitor> {
    let monitors = enumerate_monitors();
    monitors.into_iter().find(|m| m.id == id)
}

/// Set per-monitor DPI awareness for the current thread.
pub fn set_dpi_awareness() {
    unsafe {
        let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        if previous.is_invalid() {
            warn!("Failed to set per-monitor DPI awareness context v2");
        } else {
            info!("Set DPI awareness to PerMonitorAwareV2");
        }
    }
}

/// Register for display change notifications.
pub fn register_display_change_notification() -> anyhow::Result<()> {
    use std::ptr::null_mut;
    use windows::Win32::Devices::Display::GUID_DEVINTERFACE_MONITOR;
    use windows::Win32::UI::WindowsAndMessaging::{
        CreateWindowExW, DBT_DEVTYP_DEVICEINTERFACE, DEVICE_NOTIFY_WINDOW_HANDLE, HWND_MESSAGE,
        RegisterDeviceNotificationW,
    };

    info!("Registering for display change notifications");

    unsafe {
        // Create a message-only window to receive notifications
        let hwnd = CreateWindowExW(
            windows::Win32::UI::WindowsAndMessaging::WS_EX_NOACTIVATE,
            windows::core::w!("Message"),
            None,
            windows::Win32::UI::WindowsAndMessaging::WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            Some(HWND_MESSAGE),
            None,
            None,
            Some(null_mut()),
        );

        let hwnd = match hwnd {
            Ok(h) => h,
            Err(e) => anyhow::bail!(
                "Failed to create message-only window for display notifications: {}",
                e
            ),
        };

        // Register for device notifications
        let notification_filter = DEV_BROADCAST_DEVICEINTERFACE_W {
            dbcc_size: mem::size_of::<DEV_BROADCAST_DEVICEINTERFACE_W>() as u32,
            dbcc_devicetype: DBT_DEVTYP_DEVICEINTERFACE.0,
            dbcc_reserved: 0,
            dbcc_classguid: GUID_DEVINTERFACE_MONITOR,
            dbcc_name: [0u16; 1],
        };

        let h_notify = RegisterDeviceNotificationW(
            hwnd.into(),
            &notification_filter as *const _ as *const _,
            DEVICE_NOTIFY_WINDOW_HANDLE,
        );

        if h_notify.is_err() {
            warn!("Failed to register device notification for display changes");
        } else {
            debug!("Successfully registered display change notification");
        }
    }

    Ok(())
}

// ---------------------------------------------------------------------------
// Internal types for display change notification
// ---------------------------------------------------------------------------

#[repr(C)]
struct DEV_BROADCAST_DEVICEINTERFACE_W {
    dbcc_size: u32,
    dbcc_devicetype: u32,
    dbcc_reserved: u32,
    dbcc_classguid: windows::core::GUID,
    dbcc_name: [u16; 1],
}
