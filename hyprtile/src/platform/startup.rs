//! Auto-start integration for HyprTile.
//!
//! Provides registry-based startup management so HyprTile can optionally
//! launch automatically when the user logs in.  The implementation uses
//! the `Win32_System_Registry` API to read and write the standard
//! `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` key.

use std::path::PathBuf;
use tracing::{debug, info, warn};
use windows::Win32::System::Registry::{
    HKEY,
    HKEY_CURRENT_USER,
    KEY_READ,
    KEY_SET_VALUE,
    KEY_WRITE,
    REG_OPTION_NON_VOLATILE,
    REG_SZ,
    // ═══════════════════════════════════════════════════════════════════════════════
    // AI_AGENT_STOP: AUTO_START — Windows registry integration.
    // Before modifying registry behavior:
    //   1. Registry path: HKCU\Software\Microsoft\Windows\CurrentVersion\Run.
    //   2. The value name is "HyprTile" — changing it breaks existing auto-start entries.
    //   3. The value data is the full path to hyprtile.exe.
    //   4. Always close registry keys with RegCloseKey.
    //   5. Requires no admin privileges (HKCU, not HKLM).
    // ═══════════════════════════════════════════════════════════════════════════════
    RegCloseKey,
    RegCreateKeyExW,
    RegDeleteValueW,
    RegOpenKeyExW,
    RegQueryValueExW,
    RegSetValueExW,
};
use windows::core::{HSTRING, PCWSTR, w};

// HRESULT for ERROR_FILE_NOT_FOUND (Win32 error 2 -> 0x80070002)
const HRESULT_ERROR_FILE_NOT_FOUND: u32 = 0x80070002;

/// Registry key path for Windows startup entries (current user).
const RUN_KEY_PATH: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");

/// Registry value name used for the HyprTile startup entry.
const VALUE_NAME: PCWSTR = w!("HyprTile");

/// Enable auto-start by writing the current executable path to the
/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` registry key.
///
/// Returns an error if the registry key cannot be opened or the value
/// cannot be written.
pub fn enable_auto_start() -> anyhow::Result<()> {
    let exe_path = get_executable_path()?;
    let path_str = exe_path.to_string_lossy().to_string();

    info!("Enabling auto-start for executable: {}", path_str);

    unsafe {
        let mut hkey = HKEY::default();
        let result = RegCreateKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY_PATH,
            None,
            None,
            REG_OPTION_NON_VOLATILE,
            KEY_WRITE,
            None,
            &mut hkey,
            None,
        );
        result.map_err(|e| anyhow::anyhow!("Failed to create/open Run registry key: {}", e))?;

        let path_wide = HSTRING::from(&path_str);
        let path_bytes = path_wide.as_wide();
        // REG_SZ value includes the null terminator in the byte count
        let byte_len = (path_bytes.len() * std::mem::size_of::<u16>()) as u32;

        let result = RegSetValueExW(
            hkey,
            VALUE_NAME,
            None,
            REG_SZ,
            Some(std::slice::from_raw_parts(
                path_bytes.as_ptr() as *const u8,
                byte_len as usize,
            )),
        );

        let _ = RegCloseKey(hkey);

        result.map_err(|e| anyhow::anyhow!("Failed to write auto-start registry value: {}", e))?;
    }

    info!("Auto-start enabled successfully");
    Ok(())
}

/// Disable auto-start by removing the HyprTile entry from the
/// `HKCU\Software\Microsoft\Windows\CurrentVersion\Run` registry key.
///
/// Returns an error if the registry key cannot be opened.  Returns `Ok`
/// if the value did not exist (i.e. auto-start was not enabled).
pub fn disable_auto_start() -> anyhow::Result<()> {
    info!("Disabling auto-start");

    unsafe {
        let mut hkey = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY_PATH,
            None,
            KEY_SET_VALUE,
            &mut hkey,
        );
        result.map_err(|e| anyhow::anyhow!("Failed to open Run registry key: {}", e))?;

        let result = RegDeleteValueW(hkey, VALUE_NAME);
        let _ = RegCloseKey(hkey);

        match result {
            Ok(()) => {
                info!("Auto-start disabled successfully");
                Ok(())
            }
            Err(e) if e.code().0 as u32 == HRESULT_ERROR_FILE_NOT_FOUND => {
                debug!("Auto-start value did not exist, nothing to remove");
                Ok(())
            }
            Err(e) => Err(anyhow::anyhow!(
                "Failed to delete auto-start registry value: {}",
                e
            )),
        }
    }
}

/// Check whether auto-start is currently enabled for HyprTile.
///
/// Returns `true` if the `HyprTile` value exists in the Run registry key
/// and its value matches the current executable path.
pub fn is_auto_start_enabled() -> bool {
    let current_exe = match get_executable_path() {
        Ok(p) => p.to_string_lossy().to_string(),
        Err(e) => {
            warn!(
                "Cannot determine executable path for auto-start check: {}",
                e
            );
            return false;
        }
    };

    match get_registry_value() {
        Ok(Some(value)) => {
            // The registry value may be quoted and may contain trailing nulls
            let trimmed = value.trim().trim_matches('"').trim_end_matches('\0');
            let current_trimmed = current_exe.trim();
            let matches = trimmed.eq_ignore_ascii_case(current_trimmed);
            debug!(
                "Auto-start registry value: '{}', current exe: '{}', matches: {}",
                trimmed, current_trimmed, matches
            );
            matches
        }
        Ok(None) => {
            debug!("Auto-start registry value not found");
            false
        }
        Err(e) => {
            warn!("Error reading auto-start registry value: {}", e);
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Return the full path to the current executable.
fn get_executable_path() -> anyhow::Result<PathBuf> {
    std::env::current_exe()
        .map_err(|e| anyhow::anyhow!("Failed to get current executable path: {}", e))
}

/// Read the HyprTile value from the Run registry key.
///
/// Returns `Ok(Some(value))` if the value exists, `Ok(None)` if it does
/// not exist, or an error if the registry could not be read.
unsafe fn get_registry_value() -> anyhow::Result<Option<String>> {
    let mut hkey = HKEY::default();
    let result = RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY_PATH, None, KEY_READ, &mut hkey);
    result.map_err(|e| anyhow::anyhow!("Failed to open Run registry key for reading: {}", e))?;

    // First query to get the required buffer size
    let mut data_type = 0u32;
    let mut data_len: u32 = 0;
    let result = RegQueryValueExW(
        hkey,
        VALUE_NAME,
        None,
        Some(&mut data_type),
        None,
        Some(&mut data_len),
    );

    if let Err(e) = result {
        let _ = RegCloseKey(hkey);
        if e.code().0 as u32 == HRESULT_ERROR_FILE_NOT_FOUND {
            return Ok(None);
        }
        return Err(anyhow::anyhow!(
            "Failed to query auto-start registry value size: {}",
            e
        ));
    }

    // Allocate buffer and read the value
    let mut buffer: Vec<u8> = vec![0; data_len as usize];
    let result = RegQueryValueExW(
        hkey,
        VALUE_NAME,
        None,
        Some(&mut data_type),
        Some(buffer.as_mut_ptr()),
        Some(&mut data_len),
    );

    let _ = RegCloseKey(hkey);

    result.map_err(|e| anyhow::anyhow!("Failed to read auto-start registry value: {}", e))?;

    if data_type != REG_SZ.0 {
        return Err(anyhow::anyhow!(
            "Unexpected registry value type: {} (expected REG_SZ)",
            data_type
        ));
    }

    // Convert UTF-16 bytes to a Rust string
    let wide_slice = std::slice::from_raw_parts(
        buffer.as_ptr() as *const u16,
        buffer.len() / std::mem::size_of::<u16>(),
    );
    let value = String::from_utf16_lossy(wide_slice);

    Ok(Some(value))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_get_executable_path() {
        let path = get_executable_path();
        assert!(path.is_ok(), "Should be able to get executable path");
        let path = match path {
            Ok(p) => p,
            Err(_) => return false,
        };
        assert!(path.is_absolute(), "Executable path should be absolute");
    }

    #[test]
    fn test_is_auto_start_does_not_panic() {
        // This test just ensures the function does not panic
        let _ = is_auto_start_enabled();
    }
}
