//! Window filtering: decide which top-level HWNDs HyprTile should manage.
//!
//! The public entry point is [`should_manage`], which combines a battery of
//! individual checks.  Windows that fail any check are ignored by both the
//! window manager and the layout engine.

use crate::platform::window::WindowId;

/// Combined filter: returns `true` if the window should be managed.
///
/// A window must satisfy **all** of the following:
/// * valid HWND
/// * visible and on screen
/// * not a known system window (taskbar, desktop, …)
/// * not a tool window (`WS_EX_TOOLWINDOW`)
/// * not cloaked by DWM
/// * not a child / popup window
pub fn should_manage(hwnd: WindowId) -> bool {
    if !hwnd.is_valid() {
        return false;
    }
    if !is_visible_and_normal(hwnd) {
        return false;
    }
    if is_system_window(hwnd) {
        return false;
    }
    if is_tool_window(hwnd) {
        return false;
    }
    if is_cloaked(hwnd) {
        return false;
    }
    // UWP host windows (ApplicationFrameWindow) are tricky: we skip the
    // host itself and let the inner CoreWindow surface through separately.
    if is_uwp_host(hwnd) {
        return false;
    }
    passes_all_filters(hwnd)
}

/// Check whether the window belongs to a known system window class.
///
/// These are shells, desktops, and other infrastructure windows that
/// should never be tiled.
pub fn is_system_window(hwnd: WindowId) -> bool {
    let class = hwnd.get_class_name();
    let classes = system_window_classes();
    classes.contains(&class.as_str())
}

/// Check whether the window has the `WS_EX_TOOLWINDOW` extended style.
///
/// Tool windows do not appear on the taskbar and are typically
/// floating palettes or secondary utility windows.
pub fn is_tool_window(hwnd: WindowId) -> bool {
    hwnd.is_tool_window()
}

/// Check whether the window is cloaked by DWM.
///
/// Cloaked windows are rendered by the compositor but are not meant to
/// be visible to the user (e.g. UWP app suspend states, tabbed shells).
pub fn is_cloaked(hwnd: WindowId) -> bool {
    hwnd.is_cloaked()
}

/// Check whether the window is a UWP host (`ApplicationFrameWindow`).
///
/// These host the actual UWP content and should not be managed
/// directly; instead the inner `Windows.UI.Core.CoreWindow` is handled
/// by the platform layer.
pub fn is_uwp_host(hwnd: WindowId) -> bool {
    hwnd.get_class_name() == "ApplicationFrameWindow"
}

/// Check whether the window is an Electron / Chromium app.
///
/// Electron apps use the class name `Chrome_WidgetWin_1`.  Knowing this
/// is useful because Chromium windows sometimes mis-report their desired
/// state and need special-case handling in the layout engine.
pub fn is_electron(hwnd: WindowId) -> bool {
    hwnd.get_class_name() == "Chrome_WidgetWin_1"
}

/// Check whether the window is visible, not iconic, and not disabled.
///
/// This is a fast check that can be used as an early-out before the
/// heavier filter battery.
pub fn is_visible_and_normal(hwnd: WindowId) -> bool {
    if !hwnd.is_valid() {
        return false;
    }
    if !hwnd.is_visible() {
        return false;
    }
    if hwnd.is_iconic() {
        return false;
    }
    // is_zoomed (maximized) is still a normal visible state for our purposes.
    true
}

/// Combined filter that runs **all** checks and returns `true` only if
/// every individual check passes.
///
/// This is a stricter variant of [`should_manage`] that also asserts
/// the window is not a known system window and is not cloaked.  Most
/// callers should use [`should_manage`] directly.
pub fn passes_all_filters(hwnd: WindowId) -> bool {
    if !hwnd.is_valid() {
        return false;
    }
    if !is_visible_and_normal(hwnd) {
        return false;
    }
    if is_system_window(hwnd) {
        return false;
    }
    if is_tool_window(hwnd) {
        return false;
    }
    if is_cloaked(hwnd) {
        return false;
    }
    if is_uwp_host(hwnd) {
        return false;
    }
    true
}

/// Return the list of known system window class names that HyprTile
/// will never manage.
pub fn system_window_classes() -> Vec<&'static str> {
    vec![
        "Shell_TrayWnd",               // Primary taskbar
        "Shell_SecondaryTrayWnd",      // Secondary taskbar (multi-monitor)
        "Progman",                     // Program Manager desktop
        "WorkerW",                     // Desktop worker window
        "Windows.UI.Core.CoreWindow",  // UWP system windows (Start, Search)
        "StartMenuExperienceHost",     // Windows Start Menu
        "SearchHost",                  // Windows Search
        "Shell_DesktopWnd",            // Desktop window
        "SysListView32",               // Desktop icon list
        "WorkerW",                     // DWM worker window (duplicate safe)
        "NotifyIconOverflowWindow",    // System tray overflow
        "TopLevelWindowForOverflowXamlIsland", // Xaml island overflow
    ]
}

/// Return the list of known process names that HyprTile will exclude.
///
/// These are shell and system processes that occasionally create
/// top-level windows we never want to manage.
pub fn excluded_processes() -> Vec<&'static str> {
    vec![
        "explorer.exe",             // Windows Explorer / shell
        "SearchHost.exe",           // Windows Search
        "StartMenuExperienceHost.exe", // Start Menu
        "ShellExperienceHost.exe",  // Shell experience (notifications, etc.)
        "RuntimeBroker.exe",        // UWP broker
        "TextInputHost.exe",        // Touch keyboard / emoji panel
        "ApplicationFrameHost.exe", // UWP frame host
        "SecurityHealthSystray.exe", // Windows Security tray
        "CTFLoader.exe",            // CTF (Text Services Framework)
    ]
}
