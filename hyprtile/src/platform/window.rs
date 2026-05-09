use std::mem;
use std::path::Path;
use tracing::{debug, trace, warn};
use windows::Win32::Foundation::{BOOL, HWND, LPARAM, RECT, TRUE};
use windows::Win32::Graphics::Dwm::{DwmGetWindowAttribute, DWMWA_CLOAKED, DWMWA_EXTENDED_FRAME_BOUNDS, DWM_CLOAKED_SHELL};
use windows::Win32::System::Threading::GetProcessImageFileNameW;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::util::rect::Rect;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: WIN32_WRAPPER — Every function here wraps a Win32 API call.
// Before adding new window operations:
//   1. Prefer SetWindowPos with SWP flags over direct style manipulation.
//   2. Always check IsWindow() before calling APIs on foreign HWNDs.
//   3. DeferredPositioner should be used for batch operations.
//   4. Test with elevated (admin) windows — UIPI may block operations.
// ═══════════════════════════════════════════════════════════════════════════════

/// Wrapper around a Win32 HWND representing a window handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct WindowId(pub isize);

impl WindowId {
    /// Convert to a raw Win32 HWND.
    pub fn as_raw(&self) -> HWND {
        HWND(self.0)
    }

    /// Create a WindowId from a raw Win32 HWND.
    pub fn from_raw(hwnd: HWND) -> Self {
        Self(hwnd.0)
    }

    /// Check if the window handle is valid and still exists.
    pub fn is_valid(&self) -> bool {
        let hwnd = HWND(self.0);
        !hwnd.is_invalid() && unsafe { IsWindow(hwnd).as_bool() }
    }

    /// Check if the window is visible.
    pub fn is_visible(&self) -> bool {
        let hwnd = HWND(self.0);
        unsafe { IsWindowVisible(hwnd).as_bool() }
    }

    /// Check if the window is minimized (iconic).
    pub fn is_iconic(&self) -> bool {
        let hwnd = HWND(self.0);
        unsafe { IsIconic(hwnd).as_bool() }
    }

    /// Check if the window is maximized (zoomed).
    pub fn is_zoomed(&self) -> bool {
        let hwnd = HWND(self.0);
        unsafe { IsZoomed(hwnd).as_bool() }
    }

    /// Get the window rectangle including borders.
    pub fn get_rect(&self) -> Option<Rect> {
        get_window_rect(HWND(self.0))
    }

    /// Get the window title text.
    pub fn get_title(&self) -> String {
        get_window_text(HWND(self.0))
    }

    /// Get the window class name.
    pub fn get_class_name(&self) -> String {
        get_class_name_for_window(HWND(self.0))
    }

    /// Get the process name (EXE filename) of the window.
    pub fn get_process_name(&self) -> String {
        get_window_process_name(HWND(self.0))
    }

    /// Check if the window is cloaked (hidden by DWM).
    pub fn is_cloaked(&self) -> bool {
        is_window_cloaked(HWND(self.0))
    }

    /// Check if the window is a UWP host (ApplicationFrameHost.exe).
    pub fn is_uwp_host(&self) -> bool {
        is_uwp_host_window(HWND(self.0))
    }

    /// Check if the window is a tool window (no taskbar button).
    pub fn is_tool_window(&self) -> bool {
        let ex_style = get_window_ex_style(HWND(self.0));
        ex_style.contains(WS_EX_TOOLWINDOW)
    }

    /// Check if this window should be managed by the tiling WM.
    pub fn should_manage(&self) -> bool {
        should_manage_window(HWND(self.0))
    }
}

/// Default flags for SetWindowPos tiling operations.
pub const SET_WINDOW_POS_FLAGS: SET_WINDOW_POS_FLAGS = SET_WINDOW_POS_FLAGS(
    SWP_NOACTIVATE.0
        | SWP_FRAMECHANGED.0
        | SWP_NOSENDCHANGING.0
        | SWP_ASYNCWINDOWPOS.0
        | SWP_NOZORDER.0
        | SWP_NOOWNERZORDER.0,
);

/// Check if a window should be managed by the tiling WM.
pub fn should_manage_window(hwnd: HWND) -> bool {
    if hwnd.is_invalid() || !unsafe { IsWindow(hwnd).as_bool() } {
        return false;
    }

    if !unsafe { IsWindowVisible(hwnd).as_bool() } {
        return false;
    }

    if is_window_cloaked(hwnd) {
        return false;
    }

    let style = get_window_style(hwnd);
    let ex_style = get_window_ex_style(hwnd);

    if !style.contains(WS_VISIBLE) {
        return false;
    }

    if style.contains(WS_CHILD) {
        return false;
    }

    if !style.contains(WS_CAPTION) && !style.contains(WS_THICKFRAME) {
        let class = get_class_name_for_window(hwnd);
        if class != "ApplicationFrameWindow" && class != "Windows.UI.Core.CoreWindow" {
            return false;
        }
    }

    if ex_style.contains(WS_EX_TOOLWINDOW) {
        return false;
    }

    if ex_style.contains(WS_EX_NOACTIVATE) {
        return false;
    }

    if is_system_window(hwnd) {
        return false;
    }

    let title = get_window_text(hwnd);
    if title.is_empty() {
        return false;
    }

    true
}

/// Check if a window is a known system window (taskbar, desktop, etc.).
pub fn is_system_window(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return false;
    }

    let class = get_class_name_for_window(hwnd);

    let system_classes: &[&str] = &[
        "Shell_TrayWnd",
        "Shell_SecondaryTrayWnd",
        "Progman",
        "WorkerW",
        "ImmersiveLauncher",
        "ImmersiveBackgroundWindow",
        "Windows.UI.Core.CoreWindow",
        "SearchBox",
        "XamlExplorerHostIslandWindow",
        "WindowsDashboard",
        "TopLevelWindowForOverflowXamlIsland",
        "ForegroundStaging",
        "MultitaskingViewFrame",
        "Microsoft-Windows-SnipperToolbar",
        "NotifyIconOverflowWindow",
        "Shell_DesktopWindow",
    ];

    system_classes.contains(&class.as_str())
}

/// Check if a window is cloaked by DWM.
pub fn is_window_cloaked(hwnd: HWND) -> bool {
    if hwnd.is_invalid() {
        return false;
    }

    let mut cloaked: u32 = 0;
    let size = mem::size_of::<u32>() as u32;
    unsafe {
        let result = DwmGetWindowAttribute(
            hwnd,
            DWMWA_CLOAKED,
            &mut cloaked as *mut _ as *mut _,
            size,
        );
        if result.is_err() {
            return false;
        }
    }

    cloaked & DWM_CLOAKED_SHELL != 0
}

/// Get the extended window style (GWL_EXSTYLE).
pub fn get_window_ex_style(hwnd: HWND) -> WINDOW_EX_STYLE {
    if hwnd.is_invalid() {
        return WINDOW_EX_STYLE(0);
    }
    unsafe { WINDOW_EX_STYLE(GetWindowLongW(hwnd, GWL_EXSTYLE) as u32) }
}

/// Get the window style (GWL_STYLE).
pub fn get_window_style(hwnd: HWND) -> WINDOW_STYLE {
    if hwnd.is_invalid() {
        return WINDOW_STYLE(0);
    }
    unsafe { WINDOW_STYLE(GetWindowLongW(hwnd, GWL_STYLE) as u32) }
}

/// Set the window style (GWL_STYLE).
pub fn set_window_style(hwnd: HWND, style: WINDOW_STYLE) {
    if hwnd.is_invalid() {
        return;
    }
    unsafe {
        SetWindowLongW(hwnd, GWL_STYLE, style.0 as i32);
    }
}

/// Set the extended window style (GWL_EXSTYLE).
pub fn set_window_ex_style(hwnd: HWND, style: WINDOW_EX_STYLE) {
    if hwnd.is_invalid() {
        return;
    }
    unsafe {
        SetWindowLongW(hwnd, GWL_EXSTYLE, style.0 as i32);
    }
}

/// Enumerate all top-level windows.
pub fn enumerate_windows() -> Vec<WindowId> {
    let mut windows: Vec<WindowId> = Vec::new();

    unsafe {
        let _ = EnumWindows(
            Some(enum_windows_callback),
            LPARAM(&mut windows as *mut Vec<WindowId> as isize),
        );
    }

    windows
}

extern "system" fn enum_windows_callback(hwnd: HWND, lparam: LPARAM) -> BOOL {
    let windows = unsafe { &mut *(lparam.0 as *mut Vec<WindowId>) };
    windows.push(WindowId::from_raw(hwnd));
    TRUE
}

/// Set the position and size of a window.
pub fn set_window_pos(hwnd: HWND, rect: &Rect, flags: SET_WINDOW_POS_FLAGS) {
    if hwnd.is_invalid() {
        return;
    }

    unsafe {
        let flags_to_use = SET_WINDOW_POS_FLAGS(
            flags.0
                | SWP_NOACTIVATE.0
                | SWP_FRAMECHANGED.0
                | SWP_NOSENDCHANGING.0
                | SWP_ASYNCWINDOWPOS.0
                | SWP_NOZORDER.0
                | SWP_NOOWNERZORDER.0,
        );
        let _ = SetWindowPos(
            hwnd,
            HWND(0),
            rect.x,
            rect.y,
            rect.width,
            rect.height,
            flags_to_use,
        );
    }
}

/// Batched window positioning using BeginDeferWindowPos / DeferWindowPos / EndDeferWindowPos.
pub struct DeferredPositioner {
    hdwp: Option<HDWP>,
}

impl DeferredPositioner {
    /// Begin a deferred window positioning batch for `count` windows.
    pub fn new(count: i32) -> Self {
        unsafe {
            let hdwp = BeginDeferWindowPos(count);
            if hdwp.is_invalid() {
                warn!("BeginDeferWindowPos failed");
                Self { hdwp: None }
            } else {
                Self { hdwp: Some(hdwp) }
            }
        }
    }

    /// Queue a window position change.
    /// Returns true on success, false on failure (invalidates the batch).
    pub fn defer(&mut self, hwnd: HWND, rect: &Rect, flags: SET_WINDOW_POS_FLAGS) -> bool {
        let Some(hdwp) = self.hdwp else {
            return false;
        };

        if hwnd.is_invalid() {
            return false;
        }

        let flags_to_use = SET_WINDOW_POS_FLAGS(
            flags.0
                | SWP_NOACTIVATE.0
                | SWP_FRAMECHANGED.0
                | SWP_NOSENDCHANGING.0
                | SWP_ASYNCWINDOWPOS.0
                | SWP_NOZORDER.0
                | SWP_NOOWNERZORDER.0,
        );

        unsafe {
            let new_hdwp = DeferWindowPos(hdwp, hwnd, HWND(0), rect.x, rect.y, rect.width, rect.height, flags_to_use);
            if new_hdwp.is_invalid() {
                warn!("DeferWindowPos failed for hwnd={:?}", hwnd);
                self.hdwp = None;
                false
            } else {
                self.hdwp = Some(new_hdwp);
                true
            }
        }
    }

    /// Commit all deferred position changes.
    /// Returns true on success.
    pub fn commit(self) -> bool {
        match self.hdwp {
            Some(hdwp) => {
                let result = unsafe { EndDeferWindowPos(hdwp) };
                result.as_bool()
            }
            None => false,
        }
    }
}

impl Drop for DeferredPositioner {
    fn drop(&mut self) {
        if let Some(hdwp) = self.hdwp.take() {
            trace!("DeferredPositioner dropped without commit — committing now");
            unsafe {
                let _ = EndDeferWindowPos(hdwp);
            }
        }
    }
}

/// Get the extended frame bounds for a window (DWM extended bounds, more accurate than GetWindowRect).
pub fn get_extended_frame_bounds(hwnd: HWND) -> Option<Rect> {
    if hwnd.is_invalid() {
        return None;
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };

    unsafe {
        let result = DwmGetWindowAttribute(
            hwnd,
            DWMWA_EXTENDED_FRAME_BOUNDS,
            &mut rect as *mut _ as *mut _,
            mem::size_of::<RECT>() as u32,
        );
        if result.is_ok() {
            Some(Rect::from_win32(&rect))
        } else {
            None
        }
    }
}

/// Remove the thick frame style from a window (for tiling).
pub fn remove_thick_frame(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }

    let style = get_window_style(hwnd);
    let new_style = WINDOW_STYLE(style.0 & !(WS_THICKFRAME.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_MAXIMIZEBOX.0 | WS_MINIMIZEBOX.0));
    set_window_style(hwnd, new_style);

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND(0),
            0,
            0,
            0,
            0,
            SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOOWNERZORDER
                | SWP_NOACTIVATE
                | SWP_FRAMECHANGED
                | SWP_ASYNCWINDOWPOS
                | SWP_NOSENDCHANGING,
        );
    }
}

/// Restore the thick frame style for a floating window.
pub fn restore_thick_frame(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }

    let style = get_window_style(hwnd);
    let new_style = WINDOW_STYLE(style.0 | WS_THICKFRAME.0 | WS_CAPTION.0 | WS_SYSMENU.0 | WS_MAXIMIZEBOX.0 | WS_MINIMIZEBOX.0);
    set_window_style(hwnd, new_style);

    unsafe {
        let _ = SetWindowPos(
            hwnd,
            HWND(0),
            0,
            0,
            0,
            0,
            SWP_NOMOVE
                | SWP_NOSIZE
                | SWP_NOZORDER
                | SWP_NOOWNERZORDER
                | SWP_NOACTIVATE
                | SWP_FRAMECHANGED
                | SWP_ASYNCWINDOWPOS
                | SWP_NOSENDCHANGING,
        );
    }
}

/// Close a window by sending WM_CLOSE.
pub fn close_window(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }
    debug!("Closing window {:?}", hwnd);
    unsafe {
        let _ = PostMessageW(hwnd, WM_CLOSE, WPARAM(0), LPARAM(0));
    }
}

/// Bring a window to the foreground and set focus.
pub fn focus_window(hwnd: HWND) {
    if hwnd.is_invalid() {
        return;
    }
    unsafe {
        if IsIconic(hwnd).as_bool() {
            let _ = ShowWindowAsync(hwnd, SW_RESTORE);
        }
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(hwnd);
        let _ = SetActiveWindow(hwnd);
    }
}

/// Check if a window is in fullscreen mode.
pub fn is_fullscreen(hwnd: HWND, monitor_rect: &Rect) -> bool {
    if hwnd.is_invalid() {
        return false;
    }

    let Some(window_rect) = get_window_rect(hwnd) else {
        return false;
    };

    let style = get_window_style(hwnd);
    let ex_style = get_window_ex_style(hwnd);

    // Window covers the full monitor area
    let covers_monitor = window_rect.x == monitor_rect.x
        && window_rect.y == monitor_rect.y
        && window_rect.width == monitor_rect.width
        && window_rect.height == monitor_rect.height;

    // No window decorations in fullscreen
    let no_decorations = !style.contains(WS_CAPTION) && !style.contains(WS_THICKFRAME);

    covers_monitor && (no_decorations || ex_style.contains(WS_EX_TOPMOST))
}

/// Show or hide a window.
pub fn show_window(hwnd: HWND, show: bool) {
    if hwnd.is_invalid() {
        return;
    }
    unsafe {
        if show {
            let _ = ShowWindowAsync(hwnd, SW_SHOWNA);
        } else {
            let _ = ShowWindowAsync(hwnd, SW_HIDE);
        }
    }
}

// ---------------------------------------------------------------------------
// Internal helpers
// ---------------------------------------------------------------------------

fn get_window_rect(hwnd: HWND) -> Option<Rect> {
    if hwnd.is_invalid() {
        return None;
    }

    let mut rect = RECT {
        left: 0,
        top: 0,
        right: 0,
        bottom: 0,
    };
    let result = unsafe { GetWindowRect(hwnd, &mut rect) };
    if result.is_ok() {
        Some(Rect::from_win32(&rect))
    } else {
        None
    }
}

fn get_window_text(hwnd: HWND) -> String {
    if hwnd.is_invalid() {
        return String::new();
    }

    unsafe {
        let len = GetWindowTextLengthW(hwnd);
        if len == 0 {
            return String::new();
        }
        let mut buffer = vec![0u16; (len + 1) as usize];
        let result = GetWindowTextW(hwnd, &mut buffer);
        if result > 0 {
            String::from_utf16_lossy(&buffer[..result as usize])
        } else {
            String::new()
        }
    }
}

fn get_class_name_for_window(hwnd: HWND) -> String {
    if hwnd.is_invalid() {
        return String::new();
    }

    let mut buffer = vec![0u16; 256];
    let result = unsafe { GetClassNameW(hwnd, &mut buffer) };
    if result > 0 {
        String::from_utf16_lossy(&buffer[..result as usize])
    } else {
        String::new()
    }
}

fn get_window_process_name(hwnd: HWND) -> String {
    if hwnd.is_invalid() {
        return String::new();
    }

    use windows::Win32::Foundation::CloseHandle;
    use windows::Win32::System::Threading::{GetWindowThreadProcessId, OpenProcess, PROCESS_QUERY_INFORMATION};

    let mut pid = 0u32;
    unsafe {
        GetWindowThreadProcessId(hwnd, Some(&mut pid));
    }

    if pid == 0 {
        return String::new();
    }

    unsafe {
        let hprocess = match OpenProcess(PROCESS_QUERY_INFORMATION, false, pid) {
            Ok(h) => h,
            Err(_) => return String::new(),
        };
        let mut buffer = vec![0u16; 512];
        let result = GetProcessImageFileNameW(hprocess, &mut buffer);
        let _ = CloseHandle(hprocess);

        if result > 0 {
            let path_str = String::from_utf16_lossy(&buffer[..result as usize]);
            Path::new(&path_str)
                .file_stem()
                .and_then(|s| s.to_str())
                .unwrap_or("")
                .to_string()
        } else {
            String::new()
        }
    }
}

fn is_uwp_host_window(hwnd: HWND) -> bool {
    let process = get_window_process_name(hwnd);
    process.eq_ignore_ascii_case("ApplicationFrameHost")
}
