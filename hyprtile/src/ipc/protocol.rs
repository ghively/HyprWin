use serde::{Deserialize, Serialize};
use serde_json::Value;

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: IPC_PROTOCOL — JSON command definitions.
// Before adding new IPC commands:
//   1. Add variant to IpcRequest enum with serde tag.
//   2. Add handler in commands.rs (takes &mut AppState).
//   3. Wire into handle_command() match in commands.rs.
//   4. Add request/response example to docs/IPC_PROTOCOL.md.
//   5. Add serialization test in tests/integration_tests.rs.
// ═══════════════════════════════════════════════════════════════════════════════

/// An incoming IPC request from a client (named pipe or TCP).
///
/// Uses internally tagged serialization with the `"command"` field
/// discriminating the variant. All field names use `snake_case`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "command", rename_all = "snake_case")]
pub enum IpcRequest {
    /// Query workspace information, optionally filtered by monitor.
    Workspaces { monitor: Option<u32> },
    /// Query information about the currently focused window.
    FocusedWindow,
    /// Query the current layout, optionally filtered by monitor.
    Layout { monitor: Option<u32> },
    /// Query the total number of managed windows.
    WindowCount,
    /// Move focus in the given direction: `"left"`, `"right"`, `"up"`, `"down"`.
    FocusDirection { direction: String },
    /// Move the focused window in the given direction.
    MoveDirection { direction: String },
    /// Toggle floating state of the focused window.
    ToggleFloat,
    /// Toggle fullscreen state of the focused window.
    ToggleFullscreen,
    /// Cycle to the next layout on the active workspace.
    CycleLayout,
    /// Switch to the workspace with the given id.
    SwitchWorkspace { id: u32 },
    /// Move the focused window to the workspace with the given id.
    MoveToWorkspace { id: u32 },
    /// Reload the configuration file.
    ReloadConfig,
    /// Request the daemon to shut down.
    Exit,
}

/// Response returned to the IPC client.
///
/// `success` indicates whether the command was executed successfully.
/// On success, `data` may contain JSON payload; on failure, `error` holds a message.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct IpcResponse {
    pub success: bool,
    pub data: Option<Value>,
    pub error: Option<String>,
}

impl IpcResponse {
    /// Create a successful response with optional JSON data.
    pub fn success(data: Option<Value>) -> Self {
        Self {
            success: true,
            data,
            error: None,
        }
    }

    /// Create an error response with the given message.
    pub fn error(msg: impl Into<String>) -> Self {
        Self {
            success: false,
            data: None,
            error: Some(msg.into()),
        }
    }
}

/// Snapshot information about a single workspace.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorkspaceInfo {
    pub id: u32,
    pub name: String,
    pub windows: usize,
    pub has_focus: bool,
}

/// Snapshot information about the currently focused window.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FocusedWindowInfo {
    /// The Win32 HWND cast to a u64 for JSON serialization.
    pub id: u64,
    pub title: String,
    pub class: String,
    /// Human-readable state: `"tiling"`, `"floating"`, `"maximized"`, `"fullscreen"`, `"minimized"`.
    pub state: String,
}

/// Snapshot information about available and current layout.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LayoutInfo {
    pub current: String,
    pub available: Vec<String>,
}
