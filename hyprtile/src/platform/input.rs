use std::collections::HashMap;
use std::sync::mpsc::Sender;
use tracing::{debug, error, info, trace, warn};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

use crate::config::types::ModKey;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: HOTKEY_SYSTEM — Adding or changing hotkey behavior?
//   1. Key names map to VK codes via key_name_to_vk() — add new keys there.
//   2. parse_keybind() parses "mod+SHIFT+Q" syntax — supports 1+ modifiers.
//   3. Global static HOTKEY_ACTIONS maps ID → action for the message loop.
//   4. HotkeyManager::register() adds to both the local map AND global map.
//   5. run_message_loop() forwards actions via the hotkey_tx channel.
// ═══════════════════════════════════════════════════════════════════════════════

/// A hotkey binding: combination of modifiers + key that triggers an action.
/// Global map of registered hotkey IDs to their action strings.
///
/// This is accessed by the message loop thread to map WM_HOTKEY IDs back to
/// actionable commands without needing a reference to `HotkeyManager`.
use std::sync::Mutex;
static HOTKEY_ACTIONS: Mutex<std::collections::HashMap<u32, String>> =
    Mutex::new(std::collections::HashMap::new());

/// Register a hotkey action in the global map.
pub fn register_hotkey_action(id: u32, action: String) {
    if let Ok(mut map) = HOTKEY_ACTIONS.lock() {
        map.insert(id, action);
    }
}

/// Unregister a hotkey action from the global map.
pub fn unregister_hotkey_action(id: u32) {
    if let Ok(mut map) = HOTKEY_ACTIONS.lock() {
        map.remove(&id);
    }
}

/// Clear all hotkey actions from the global map.
pub fn clear_hotkey_actions() {
    if let Ok(mut map) = HOTKEY_ACTIONS.lock() {
        map.clear();
    }
}

/// Look up the action string for a given hotkey ID.
pub fn get_hotkey_action(id: u32) -> Option<String> {
    HOTKEY_ACTIONS.lock().ok()?.get(&id).cloned()
}

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Hotkey {
    /// Modifier keys (Alt, Win, Ctrl, Shift) that must be held.
    pub modifiers: Vec<ModKey>,
    /// The key name (e.g. "RETURN", "Q", "1", "LEFT").
    pub key: String,
    /// The action to execute when the hotkey is pressed.
    pub action: String,
}

/// Manages registered hotkeys using the Win32 `RegisterHotKey` API.
pub struct HotkeyManager {
    /// Map from hotkey ID to the Hotkey definition.
    hotkeys: HashMap<u32, Hotkey>,
    /// Next available hotkey ID.
    next_id: u32,
}

impl HotkeyManager {
    /// Create a new empty hotkey manager.
    pub fn new() -> Self {
        Self {
            hotkeys: HashMap::new(),
            next_id: 1,
        }
    }

    /// Register a hotkey with the OS.
    ///
    /// Returns the hotkey ID on success.
    pub fn register(&mut self, hotkey: Hotkey) -> anyhow::Result<u32> {
        let vk = key_name_to_vk(&hotkey.key);
        let vk = match vk {
            Some(v) => v,
            None => anyhow::bail!("Unknown key name: {}", hotkey.key),
        };

        let mut modifiers: HOT_KEY_MODIFIERS = HOT_KEY_MODIFIERS(0);
        for m in &hotkey.modifiers {
            modifiers.0 |= mod_key_to_bits(m);
        }

        let id = self.next_id;
        self.next_id += 1;

        unsafe {
            let result = RegisterHotKey(Some(HWND(0)), id as i32, modifiers, vk);
            if result.is_ok() {
                debug!(
                    "Registered hotkey id={} key={:?} mods={:?} action={}",
                    id, hotkey.key, hotkey.modifiers, hotkey.action
                );
                self.hotkeys.insert(id, hotkey);
                Ok(id)
            } else {
                let err = windows::core::Error::from_win32();
                anyhow::bail!("RegisterHotKey failed for key={}: {}", hotkey.key, err);
            }
        }
    }

    /// Unregister a hotkey by its ID.
    pub fn unregister(&mut self, id: u32) -> anyhow::Result<()> {
        unsafe {
            let result = UnregisterHotKey(Some(HWND(0)), id as i32);
            if result.is_ok() {
                self.hotkeys.remove(&id);
                trace!("Unregistered hotkey id={}", id);
                Ok(())
            } else {
                let err = windows::core::Error::from_win32();
                anyhow::bail!("UnregisterHotKey failed for id={}: {}", id, err);
            }
        }
    }

    /// Unregister all hotkeys.
    pub fn unregister_all(&mut self) {
        let ids: Vec<u32> = self.hotkeys.keys().copied().collect();
        for id in ids {
            unsafe {
                let _ = UnregisterHotKey(Some(HWND(0)), id as i32);
            }
        }
        self.hotkeys.clear();
        self.next_id = 1;
        info!("Unregistered all hotkeys");
    }

    /// Handle a WM_HOTKEY message.
    ///
    /// Returns the matching `Hotkey` if found.
    pub fn handle_message(&self, wparam: WPARAM, _lparam: LPARAM) -> Option<&Hotkey> {
        let id = wparam.0 as u32;
        let hk = self.hotkeys.get(&id);
        if hk.is_some() {
            if let Some(action) = hk.as_ref().map(|h| h.action.as_str()) {
                trace!("Hotkey triggered: id={} action={}", id, action);
            }
        }
        hk
    }

    /// Reload all hotkeys from a keybind map.
    ///
    /// Unregisters existing hotkeys and re-registers from the new configuration.
    pub fn reload_hotkeys(
        &mut self,
        keybinds: &HashMap<String, String>,
        mod_key: &ModKey,
    ) -> anyhow::Result<()> {
        self.unregister_all();

        let mut failed = Vec::new();

        for (keybind_str, action) in keybinds {
            match parse_keybind(keybind_str, mod_key) {
                Some(mut hotkey) => {
                    hotkey.action = action.clone();
                    if let Err(e) = self.register(hotkey) {
                        warn!("Failed to register hotkey '{}': {}", keybind_str, e);
                        failed.push(keybind_str.clone());
                    }
                }
                None => {
                    warn!("Failed to parse keybind: {}", keybind_str);
                    failed.push(keybind_str.clone());
                }
            }
        }

        if !failed.is_empty() {
            warn!("Failed to register {} hotkeys", failed.len());
        }

        info!(
            "Reloaded {} hotkeys ({} failed)",
            keybinds.len() - failed.len(),
            failed.len()
        );
        Ok(())
    }
}

impl Default for HotkeyManager {
    fn default() -> Self {
        Self::new()
    }
}

/// Convert a string key name to a Win32 virtual key code.
///
/// Supports common keys used in window manager keybinds:
/// "RETURN", "Q"-"Z", "0"-"9", arrow keys, function keys, etc.
pub fn key_name_to_vk(key: &str) -> Option<u32> {
    let upper = key.to_ascii_uppercase();

    match upper.as_str() {
        // Letters
        "A" => Some(VK_A.0 as u32),
        "B" => Some(VK_B.0 as u32),
        "C" => Some(VK_C.0 as u32),
        "D" => Some(VK_D.0 as u32),
        "E" => Some(VK_E.0 as u32),
        "F" => Some(VK_F.0 as u32),
        "G" => Some(VK_G.0 as u32),
        "H" => Some(VK_H.0 as u32),
        "I" => Some(VK_I.0 as u32),
        "J" => Some(VK_J.0 as u32),
        "K" => Some(VK_K.0 as u32),
        "L" => Some(VK_L.0 as u32),
        "M" => Some(VK_M.0 as u32),
        "N" => Some(VK_N.0 as u32),
        "O" => Some(VK_O.0 as u32),
        "P" => Some(VK_P.0 as u32),
        "Q" => Some(VK_Q.0 as u32),
        "R" => Some(VK_R.0 as u32),
        "S" => Some(VK_S.0 as u32),
        "T" => Some(VK_T.0 as u32),
        "U" => Some(VK_U.0 as u32),
        "V" => Some(VK_V.0 as u32),
        "W" => Some(VK_W.0 as u32),
        "X" => Some(VK_X.0 as u32),
        "Y" => Some(VK_Y.0 as u32),
        "Z" => Some(VK_Z.0 as u32),

        // Digits
        "0" | "1" | "2" | "3" | "4" | "5" | "6" | "7" | "8" | "9" => {
            Some(upper.as_bytes()[0] as u32)
        }

        // Special keys
        "RETURN" | "ENTER" => Some(VK_RETURN.0 as u32),
        "SPACE" => Some(VK_SPACE.0 as u32),
        "TAB" => Some(VK_TAB.0 as u32),
        "ESCAPE" | "ESC" => Some(VK_ESCAPE.0 as u32),
        "BACK" | "BACKSPACE" => Some(VK_BACK.0 as u32),
        "DELETE" | "DEL" => Some(VK_DELETE.0 as u32),
        "INSERT" | "INS" => Some(VK_INSERT.0 as u32),
        "HOME" => Some(VK_HOME.0 as u32),
        "END" => Some(VK_END.0 as u32),
        "PAGEUP" | "PGUP" => Some(VK_PRIOR.0 as u32),
        "PAGEDOWN" | "PGDN" => Some(VK_NEXT.0 as u32),

        // Arrow keys
        "LEFT" => Some(VK_LEFT.0 as u32),
        "RIGHT" => Some(VK_RIGHT.0 as u32),
        "UP" => Some(VK_UP.0 as u32),
        "DOWN" => Some(VK_DOWN.0 as u32),

        // Modifiers (standalone)
        "SHIFT" => Some(VK_SHIFT.0 as u32),
        "CONTROL" | "CTRL" => Some(VK_CONTROL.0 as u32),
        "MENU" | "ALT" => Some(VK_MENU.0 as u32),
        "LWIN" | "RWIN" | "WIN" => Some(VK_LWIN.0 as u32),

        // Function keys
        "F1" => Some(VK_F1.0 as u32),
        "F2" => Some(VK_F2.0 as u32),
        "F3" => Some(VK_F3.0 as u32),
        "F4" => Some(VK_F4.0 as u32),
        "F5" => Some(VK_F5.0 as u32),
        "F6" => Some(VK_F6.0 as u32),
        "F7" => Some(VK_F7.0 as u32),
        "F8" => Some(VK_F8.0 as u32),
        "F9" => Some(VK_F9.0 as u32),
        "F10" => Some(VK_F10.0 as u32),
        "F11" => Some(VK_F11.0 as u32),
        "F12" => Some(VK_F12.0 as u32),

        // Numpad
        "NUMPAD0" => Some(VK_NUMPAD0.0 as u32),
        "NUMPAD1" => Some(VK_NUMPAD1.0 as u32),
        "NUMPAD2" => Some(VK_NUMPAD2.0 as u32),
        "NUMPAD3" => Some(VK_NUMPAD3.0 as u32),
        "NUMPAD4" => Some(VK_NUMPAD4.0 as u32),
        "NUMPAD5" => Some(VK_NUMPAD5.0 as u32),
        "NUMPAD6" => Some(VK_NUMPAD6.0 as u32),
        "NUMPAD7" => Some(VK_NUMPAD7.0 as u32),
        "NUMPAD8" => Some(VK_NUMPAD8.0 as u32),
        "NUMPAD9" => Some(VK_NUMPAD9.0 as u32),

        _ => {
            // Try single character (like punctuation)
            if upper.len() == 1 {
                let ch = upper.as_bytes()[0];
                // Map common punctuation
                match ch as char {
                    ',' => Some(VK_OEM_COMMA.0 as u32),
                    '.' => Some(VK_OEM_PERIOD.0 as u32),
                    ';' => Some(VK_OEM_1.0 as u32),
                    '/' => Some(VK_OEM_2.0 as u32),
                    '`' => Some(VK_OEM_3.0 as u32),
                    '[' => Some(VK_OEM_4.0 as u32),
                    '\\' => Some(VK_OEM_5.0 as u32),
                    ']' => Some(VK_OEM_6.0 as u32),
                    '\'' => Some(VK_OEM_7.0 as u32),
                    '-' => Some(VK_OEM_MINUS.0 as u32),
                    '+' => Some(VK_OEM_PLUS.0 as u32),
                    _ => {
                        // For any other single ASCII character, use the char code directly
                        if ch.is_ascii_alphanumeric() {
                            Some(ch as u32)
                        } else {
                            None
                        }
                    }
                }
            } else {
                None
            }
        }
    }
}

/// Parse a keybind string like "mod+SHIFT+Q" or "mod+RETURN" into a `Hotkey`.
///
/// The "mod" token is replaced with the configured modifier key (Alt, Win, Ctrl).
/// Other recognized modifiers: SHIFT, ALT, CTRL, WIN.
pub fn parse_keybind(keybind: &str, mod_key: &ModKey) -> Option<Hotkey> {
    let parts: Vec<&str> = keybind.split('+').map(|s| s.trim()).collect();
    if parts.is_empty() {
        return None;
    }

    let mut modifiers: Vec<ModKey> = Vec::new();
    let mut key: Option<String> = None;

    for part in &parts {
        let upper = part.to_ascii_uppercase();
        match upper.as_str() {
            "MOD" => {
                modifiers.push(mod_key.clone());
            }
            "SHIFT" => {
                modifiers.push(ModKey::Shift);
            }
            "ALT" => {
                modifiers.push(ModKey::Alt);
            }
            "CTRL" | "CONTROL" => {
                modifiers.push(ModKey::Ctrl);
            }
            "WIN" | "SUPER" | "META" => {
                modifiers.push(ModKey::Win);
            }
            _ => {
                // This must be the key
                if key.is_none() {
                    // Validate it's a known key
                    if key_name_to_vk(&upper).is_some() {
                        key = Some(upper);
                    } else {
                        return None;
                    }
                }
            }
        }
    }

    key.map(|k| Hotkey {
        modifiers,
        key: k,
        action: String::new(),
    })
}

/// Register all hotkeys from a keybind configuration map.
pub fn register_all_hotkeys(
    manager: &mut HotkeyManager,
    keybinds: &HashMap<String, String>,
    mod_key: &ModKey,
) -> anyhow::Result<()> {
    let mut failed = 0usize;

    for (keybind_str, action) in keybinds {
        match parse_keybind(keybind_str, mod_key) {
            Some(mut hotkey) => {
                hotkey.action = action.clone();
                if let Err(e) = manager.register(hotkey) {
                    warn!("Failed to register hotkey '{}': {}", keybind_str, e);
                    failed += 1;
                }
            }
            None => {
                warn!("Failed to parse keybind: {}", keybind_str);
                failed += 1;
            }
        }
    }

    let total = keybinds.len();
    let success = total - failed;
    info!(
        "Registered {}/{} hotkeys ({} failed)",
        success, total, failed
    );

    if failed == total && total > 0 {
        anyhow::bail!("All {} hotkey registrations failed", total);
    }

    Ok(())
}

/// Convert a `ModKey` to Win32 modifier bits for `RegisterHotKey`.
///
/// Returns `MOD_ALT`, `MOD_CONTROL`, `MOD_SHIFT`, or `MOD_WIN`.
pub fn mod_key_to_bits(mod_key: &ModKey) -> u32 {
    match mod_key {
        ModKey::Alt => MOD_ALT.0,
        ModKey::Win => MOD_WIN.0,
        ModKey::Ctrl => MOD_CONTROL.0,
        ModKey::Shift => MOD_SHIFT.0,
    }
}

/// Run the message loop for hotkey handling.
///
/// Creates a hidden message-only window and runs `GetMessageW` in a loop,
/// forwarding WM_HOTKEY messages as action strings through the channel.
pub fn run_message_loop(hotkey_tx: Sender<String>) -> anyhow::Result<()> {
    info!("Starting hotkey message loop");

    unsafe {
        // Create a message-only window to receive WM_HOTKEY
        let hwnd = CreateWindowExW(
            WS_EX_NOACTIVATE,
            windows::w!("Message"),
            None,
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            HWND_MESSAGE,
            None,
            None,
            None,
        )?;

        debug!("Hotkey message window created: {:?}", hwnd);

        let mut msg = MSG::default();
        loop {
            let result = GetMessageW(&mut msg, Some(HWND(0)), 0, 0);
            if result.0 == 0 {
                // WM_QUIT received
                break;
            }
            if result.0 < 0 {
                let err = windows::core::Error::from_win32();
                error!("GetMessageW failed: {}", err);
                break;
            }

            if msg.message == WM_HOTKEY {
                let id = msg.wParam.0 as u32;
                trace!("WM_HOTKEY received: id={}", id);
                // Look up the action in the global map and forward it
                if let Some(action) = get_hotkey_action(id) {
                    if let Err(e) = hotkey_tx.send(action) {
                        error!("Failed to send hotkey action: {}", e);
                    }
                } else {
                    warn!("Unknown hotkey ID: {}", id);
                }
            }

            TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        info!("Hotkey message loop exiting");
    }

    Ok(())
}
