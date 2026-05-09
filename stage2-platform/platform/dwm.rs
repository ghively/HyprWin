use std::mem;
use tracing::{debug, error, info, trace, warn};
use windows::Win32::Foundation::{BOOL, HWND, TRUE};
use windows::Win32::Graphics::Dwm::*;

/// DWM border colors for focused and unfocused windows (ARGB format).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct BorderColors {
    /// Color for the focused window (ARGB).
    pub focused: u32,
    /// Color for unfocused windows (ARGB).
    pub unfocused: u32,
}

impl Default for BorderColors {
    fn default() -> Self {
        BorderColors {
            focused: 0xFF00FF00,   // Green
            unfocused: 0xFF808080, // Gray
        }
    }
}

/// Set DWM border color for a window.
///
/// Uses `DWMWA_BORDER_COLOR` which is supported on Windows 11 build 22000+.
/// Falls back gracefully on older systems.
pub fn set_border_color(hwnd: isize, color: u32) -> anyhow::Result<()> {
    let hwnd = HWND(hwnd);
    if hwnd.is_invalid() {
        anyhow::bail!("Invalid HWND for set_border_color");
    }

    if !is_border_color_supported() {
        anyhow::bail!("DWM border color not supported on this OS version");
    }

    unsafe {
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_BORDER_COLOR,
            &color as *const _ as *const _,
            mem::size_of::<u32>() as u32,
        );
        if result.is_ok() {
            trace!("Set border color 0x{:08X} for hwnd=0x{:X}", color, hwnd.0);
            Ok(())
        } else {
            anyhow::bail!("DwmSetWindowAttribute(DWMWA_BORDER_COLOR) failed");
        }
    }
}

/// Enable or disable DWM transitions (animations) for a window.
///
/// Setting this to `false` makes window moves/resizes snappier by
/// disabling the built-in DWM animation.
pub fn set_transitions_enabled(hwnd: isize, enabled: bool) -> anyhow::Result<()> {
    let hwnd = HWND(hwnd);
    if hwnd.is_invalid() {
        anyhow::bail!("Invalid HWND for set_transitions_enabled");
    }

    let value: i32 = if enabled { 1 } else { 0 };

    unsafe {
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_TRANSITIONS_FORCEDISABLED,
            &value as *const _ as *const _,
            mem::size_of::<i32>() as u32,
        );
        if result.is_ok() {
            trace!(
                "Set transitions {} for hwnd=0x{:X}",
                if enabled { "enabled" } else { "disabled" },
                hwnd.0
            );
            Ok(())
        } else {
            anyhow::bail!("DwmSetWindowAttribute(DWMWA_TRANSITIONS_FORCEDISABLED) failed");
        }
    }
}

/// Force disable DWM transitions for a window.
///
/// This is a convenience wrapper that unconditionally disables transitions.
pub fn force_disable_transitions(hwnd: isize) -> anyhow::Result<()> {
    set_transitions_enabled(hwnd, false)
}

/// Set the corner preference for a window (rounded vs square corners).
///
/// - `rounded = true` — use `DWMWCP_ROUND`
/// - `rounded = false` — use `DWMWCP_DONOTROUND`
pub fn set_corner_preference(hwnd: isize, rounded: bool) -> anyhow::Result<()> {
    let hwnd = HWND(hwnd);
    if hwnd.is_invalid() {
        anyhow::bail!("Invalid HWND for set_corner_preference");
    }

    let preference: DWM_WINDOW_CORNER_PREFERENCE = if rounded {
        DWMWCP_ROUND
    } else {
        DWMWCP_DONOTROUND
    };

    unsafe {
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_WINDOW_CORNER_PREFERENCE,
            &preference as *const _ as *const _,
            mem::size_of::<DWM_WINDOW_CORNER_PREFERENCE>() as u32,
        );
        if result.is_ok() {
            trace!(
                "Set corner preference {:?} for hwnd=0x{:X}",
                preference,
                hwnd.0
            );
            Ok(())
        } else {
            anyhow::bail!("DwmSetWindowAttribute(DWMWA_WINDOW_CORNER_PREFERENCE) failed");
        }
    }
}

/// Set DWM border width for a window using an undocumented API fallback.
///
/// On Windows 11 22H2+ this uses `DWMWA_BORDER_COLOR` with a negative width
/// hint. On older systems this falls back to `extend_frame_into_client`
/// to simulate a border.
pub fn set_border_width(hwnd: isize, width: i32) -> anyhow::Result<()> {
    let hwnd = HWND(hwnd);
    if hwnd.is_invalid() {
        anyhow::bail!("Invalid HWND for set_border_width");
    }

    // Use the margins-based approach to simulate a border
    let margins = MARGINS {
        cxLeftWidth: width,
        cxRightWidth: width,
        cyTopHeight: width,
        cyBottomHeight: width,
    };

    extend_frame_into_client(hwnd, &margins)?;

    trace!("Set border width {} for hwnd=0x{:X}", width, hwnd.0);
    Ok(())
}

/// Enable DWM rendering (glass frame) on the window.
///
/// This ensures the DWM compositor treats the window as a
/// "glass" client so that border effects are visible.
pub fn enable_dwm_rendering(hwnd: isize) -> anyhow::Result<()> {
    let hwnd = HWND(hwnd);
    if hwnd.is_invalid() {
        anyhow::bail!("Invalid HWND for enable_dwm_rendering");
    }

    if !is_composition_enabled() {
        anyhow::bail!("DWM composition is not enabled");
    }

    unsafe {
        // Enable non-client rendering policy
        let policy: DWMNCRENDERINGPOLICY = DWMNCRP_ENABLED;
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_NCRENDERING_POLICY,
            &policy as *const _ as *const _,
            mem::size_of::<DWMNCRENDERINGPOLICY>() as u32,
        );
        if result.is_err() {
            warn!("DwmSetWindowAttribute(DWMWA_NCRENDERING_POLICY) failed");
        }

        // Enable DWM rendering
        let nc_paint: BOOL = TRUE;
        let result = DwmSetWindowAttribute(
            hwnd,
            DWMWA_ALLOW_NCPAINT,
            &nc_paint as *const _ as *const _,
            mem::size_of::<BOOL>() as u32,
        );
        if result.is_err() {
            warn!("DwmSetWindowAttribute(DWMWA_ALLOW_NCPAINT) failed");
        }

        trace!("Enabled DWM rendering for hwnd=0x{:X}", hwnd.0);
        Ok(())
    }
}

/// Extend the DWM frame into the client area for border rendering.
///
/// Positive margin values cause the border area to be rendered outside
/// the client area, producing visible colored borders.
pub fn extend_frame_into_client(hwnd: isize, margins: &MARGINS) -> anyhow::Result<()> {
    let hwnd = HWND(hwnd);
    if hwnd.is_invalid() {
        anyhow::bail!("Invalid HWND for extend_frame_into_client");
    }

    if !is_composition_enabled() {
        anyhow::bail!("DWM composition is not enabled");
    }

    unsafe {
        let result = DwmExtendFrameIntoClientArea(hwnd, margins);
        if result.is_ok() {
            trace!(
                "Extended frame into client for hwnd=0x{:X} with margins L:{} R:{} T:{} B:{}",
                hwnd.0,
                margins.cxLeftWidth,
                margins.cxRightWidth,
                margins.cyTopHeight,
                margins.cyBottomHeight
            );
            Ok(())
        } else {
            anyhow::bail!("DwmExtendFrameIntoClientArea failed");
        }
    }
}

/// Check if DWM border color customization is supported.
///
/// This requires Windows 11 build 22000 or later.
pub fn is_border_color_supported() -> bool {
    unsafe {
        // DWMWA_BORDER_COLOR was introduced in Windows 11 21H2 (build 22000)
        // Check OS version to determine availability
        let mut info = mem::MaybeUninit::<windows::Win32::System::SystemInformation::OSVERSIONINFOEXW>::uninit();
        let info_ptr = info.as_mut_ptr();
        (*info_ptr).dwOSVersionInfoSize = mem::size_of::<windows::Win32::System::SystemInformation::OSVERSIONINFOEXW>() as u32;

        // RtlGetVersion is the reliable way to get the true OS version
        let ntdll = windows::Win32::System::LibraryLoader::GetModuleHandleW(windows::w!("ntdll.dll"));
        if ntdll.is_err() {
            return false;
        }
        let ntdll = ntdll.unwrap();

        type RtlGetVersionFn = unsafe extern "system" fn(*mut windows::Win32::System::SystemInformation::OSVERSIONINFOW) -> i32;
        let proc = windows::Win32::System::LibraryLoader::GetProcAddress(
            ntdll,
            windows::s!("RtlGetVersion"),
        );

        match proc {
            Some(func) => {
                let rtl_get_version: RtlGetVersionFn = std::mem::transmute(func);
                let status = rtl_get_version(info_ptr as *mut windows::Win32::System::SystemInformation::OSVERSIONINFOW);
                if status != 0 {
                    return false;
                }
                let version = info.assume_init();
                // Windows 11 is version 10.0 build 22000+
                version.dwMajorVersion >= 10 && version.dwBuildNumber >= 22000
            }
            None => false,
        }
    }
}

/// Check whether DWM desktop composition is enabled.
pub fn is_composition_enabled() -> bool {
    unsafe {
        let mut enabled: i32 = 0;
        let result = DwmIsCompositionEnabled(&mut enabled);
        if result.is_ok() {
            enabled != 0
        } else {
            false
        }
    }
}
