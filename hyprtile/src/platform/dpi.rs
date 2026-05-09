//! Win32 DPI queries.
//!
//! Pure DPI scaling math (logical↔physical conversion, rect scaling) lives in
//! [`crate::util::dpi`]. This module is the only place that talks to the OS.

use windows::Win32::UI::HiDpi::{GetDpiForMonitor, GetDpiForSystem, MONITOR_DPI_TYPE};

use crate::util::dpi::BASE_DPI;

/// Get the DPI for a specific monitor.
///
/// Uses the `GetDpiForMonitor` Win32 API with the effective DPI type.
/// Falls back to [`BASE_DPI`] (96) if the API call fails.
pub fn get_monitor_dpi(hmonitor: isize) -> u32 {
    let mut dpi_x: u32 = 0;
    let mut dpi_y: u32 = 0;
    let result = unsafe {
        GetDpiForMonitor(
            windows::Win32::Graphics::Gdi::HMONITOR(hmonitor as *mut _),
            MONITOR_DPI_TYPE(0), // MDT_EFFECTIVE_DPI
            &mut dpi_x,
            &mut dpi_y,
        )
    };
    if result.is_ok() && dpi_x > 0 {
        dpi_x
    } else {
        BASE_DPI
    }
}

/// Get the system DPI (fallback when per-monitor DPI is unavailable).
pub fn get_system_dpi() -> u32 {
    let dpi = unsafe { GetDpiForSystem() };
    if dpi > 0 { dpi } else { BASE_DPI }
}
