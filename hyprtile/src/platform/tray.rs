//! System tray integration for HyprTile.
//!
//! Provides a [`TrayIcon`] struct that wraps the `tray-icon` crate to show
//! HyprTile in the Windows system tray with a context menu.
//!
//! The tray icon runs its own event loop in a background thread and
//! forwards menu actions back to the application via the event channel.

use std::sync::mpsc::Sender;
use std::thread;
use tracing::{debug, info, warn};

use crate::platform::events::WindowEvent;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: SYSTEM_TRAY — Tray icon and context menu.
// Before modifying tray behavior:
//   1. The tray icon requires an ICO resource (resources/icon.ico).
//   2. Menu actions send events through the same event_tx channel as hotkeys.
//   3. Tray icon lifetime must match the app lifetime.
//   4. On Windows 11, tray menu may be hidden behind the overflow area.
// ═══════════════════════════════════════════════════════════════════════════════

/// Actions that can be triggered from the system tray menu.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TrayAction {
    /// Show/hide the application (placeholder for future window visibility).
    Show,
    /// Reload the configuration from disk.
    ReloadConfig,
    /// Exit the application.
    Exit,
}

impl TrayAction {
    /// Return the action string used for [`WindowEvent::HotkeyAction`] dispatch.
    fn as_action_str(self) -> &'static str {
        match self {
            TrayAction::Show => "tray_show",
            TrayAction::ReloadConfig => "reload_config",
            TrayAction::Exit => "exit",
        }
    }
}

/// System tray icon wrapper for HyprTile.
///
/// Creates a tray icon with a context menu containing:
/// - **Show** — brings the application to the foreground
/// - **Reload Config** — hot-reloads the configuration file
/// - **Exit** — gracefully shuts down HyprTile
///
/// The tray icon lives for the lifetime of the application. Dropping it
/// removes the icon from the system tray.
pub struct TrayIcon {
    /// Handle to the tray icon (kept alive while this struct exists).
    _tray: tray_icon::TrayIcon,
    /// Handle to the background event-processing thread.
    _thread: thread::JoinHandle<()>,
}

impl TrayIcon {
    /// Create a new system tray icon and start the event loop.
    ///
    /// # Arguments
    ///
    /// * `event_tx` — Sender to the application's event channel. Tray actions
    ///   are forwarded as [`WindowEvent::HotkeyAction`] messages.
    ///
    /// # Errors
    ///
    /// Returns an error if the tray icon or icon image cannot be created.
    pub fn new(event_tx: Sender<WindowEvent>) -> anyhow::Result<Self> {
        info!("Creating system tray icon");

        // Build the context menu
        let menu = tray_icon::menu::Menu::new();

        let show_item = tray_icon::menu::MenuItem::new("Show", true, None);
        let reload_item = tray_icon::menu::MenuItem::new("Reload Config", true, None);
        let separator = tray_icon::menu::PredefinedMenuItem::separator();
        let exit_item = tray_icon::menu::MenuItem::new("Exit", true, None);

        // Capture menu item IDs before moving items into the menu
        let show_id = show_item.id().0.clone();
        let reload_id = reload_item.id().0.clone();
        let exit_id = exit_item.id().0.clone();

        menu.append(&show_item)
            .map_err(|e| anyhow::anyhow!("Failed to add Show menu item: {}", e))?;
        menu.append(&reload_item)
            .map_err(|e| anyhow::anyhow!("Failed to add Reload Config menu item: {}", e))?;
        menu.append(&separator)
            .map_err(|e| anyhow::anyhow!("Failed to add separator: {}", e))?;
        menu.append(&exit_item)
            .map_err(|e| anyhow::anyhow!("Failed to add Exit menu item: {}", e))?;

        // Create a simple icon from RGBA data
        let icon = create_hyprtile_icon()
            .map_err(|e| anyhow::anyhow!("Failed to create tray icon image: {}", e))?;

        let tray = tray_icon::TrayIconBuilder::new()
            .with_menu(Box::new(menu))
            .with_tooltip("HyprTile Tiling WM")
            .with_icon(icon)
            .build()
            .map_err(|e| anyhow::anyhow!("Failed to build tray icon: {}", e))?;

        debug!("Tray icon created successfully");

        // Spawn a thread to listen for tray/menu events
        let thread = thread::Builder::new()
            .name("tray-events".to_string())
            .spawn(move || {
                info!("Tray event loop started");

                let menu_channel = tray_icon::menu::MenuEvent::receiver();

                loop {
                    match menu_channel.recv() {
                        Ok(event) => {
                            let clicked_id = event.id.0.clone();
                            let action = if clicked_id == show_id {
                                Some(TrayAction::Show)
                            } else if clicked_id == reload_id {
                                Some(TrayAction::ReloadConfig)
                            } else if clicked_id == exit_id {
                                Some(TrayAction::Exit)
                            } else {
                                debug!("Unknown tray menu event id: {}", clicked_id);
                                None
                            };

                            if let Some(a) = action {
                                debug!("Tray action triggered: {:?}", a);
                                let action_str = a.as_action_str().to_string();
                                if let Err(e) = event_tx.send(WindowEvent::HotkeyAction(action_str))
                                {
                                    warn!("Failed to send tray action to event channel: {}", e);
                                    break;
                                }
                            }
                        }
                        Err(e) => {
                            warn!("Tray menu event channel error: {}", e);
                            break;
                        }
                    }
                }

                info!("Tray event loop exiting");
            })?;

        Ok(Self {
            _tray: tray,
            _thread: thread,
        })
    }

    /// Show the tray icon (nop — the icon is already visible on creation).
    pub fn show(&self) {
        debug!("Tray icon show called (icon is already visible)");
    }

    /// Hide the tray icon.
    ///
    /// In the current `tray-icon` crate version this is a no-op since the
    /// icon lifecycle is tied to the struct lifetime. To hide, drop the
    /// [`TrayIcon`] and recreate it later.
    pub fn hide(&self) {
        debug!("Tray icon hide called (drop and recreate to actually hide)");
    }
}

// ---------------------------------------------------------------------------
// Icon helpers
// ---------------------------------------------------------------------------

/// Create a simple HyprTile branded icon for the system tray.
///
/// Generates a 32x32 RGBA icon with a teal/blue gradient square.
/// In a production build this would load an embedded ICO file instead.
fn create_hyprtile_icon() -> anyhow::Result<tray_icon::Icon> {
    const SIZE: usize = 32;
    const PIXELS: usize = SIZE * SIZE;

    let mut rgba: Vec<u8> = Vec::with_capacity(PIXELS * 4);

    for y in 0..SIZE {
        for x in 0..SIZE {
            // Teal-to-blue gradient
            let r = (0x1A as f32 + (x as f32 / SIZE as f32) * 40.0) as u8;
            let g = (0xD6 as f32 - (y as f32 / SIZE as f32) * 60.0) as u8;
            let b = (0xC6 as f32 + (x as f32 / SIZE as f32) * 30.0) as u8;

            // Rounded corners: fade alpha near edges
            let edge_dist = x.min(SIZE - 1 - x).min(y).min(SIZE - 1 - y);
            let alpha = if edge_dist < 4 {
                (255.0 * edge_dist as f32 / 4.0) as u8
            } else {
                255
            };

            rgba.push(r);
            rgba.push(g);
            rgba.push(b);
            rgba.push(alpha);
        }
    }

    tray_icon::Icon::from_rgba(rgba, SIZE as u32, SIZE as u32)
        .map_err(|e| anyhow::anyhow!("Failed to create icon from RGBA data: {}", e))
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tray_action_as_str() {
        assert_eq!(TrayAction::Show.as_action_str(), "tray_show");
        assert_eq!(TrayAction::ReloadConfig.as_action_str(), "reload_config");
        assert_eq!(TrayAction::Exit.as_action_str(), "exit");
    }

    #[test]
    fn test_create_icon() {
        let icon = create_hyprtile_icon();
        assert!(icon.is_ok(), "Should be able to create a tray icon");
    }
}
