# HyprTile IPC Protocol

This document describes the inter-process communication (IPC) protocol used by HyprTile for external control and status bar integration.

## Table of Contents

- [Transport](#transport)
  - [Named Pipe (Local)](#named-pipe-local)
  - [TCP Socket (Network)](#tcp-socket-network)
- [Message Format](#message-format)
  - [Request Format](#request-format)
  - [Response Format](#response-format)
- [Commands](#commands)
  - [workspaces](#workspaces)
  - [focused_window](#focused_window)
  - [layout](#layout)
  - [window_count](#window_count)
  - [focus_direction](#focus_direction)
  - [move_direction](#move_direction)
  - [toggle_float](#toggle_float)
  - [toggle_fullscreen](#toggle_fullscreen)
  - [cycle_layout](#cycle_layout)
  - [switch_workspace](#switch_workspace)
  - [move_to_workspace](#move_to_workspace)
  - [reload_config](#reload_config)
  - [exit](#exit)
- [Data Types](#data-types)
- [Example Clients](#example-clients)
  - [Rust Client](#rust-client)
  - [Python Client](#python-client)
  - [PowerShell Client](#powershell-client)
- [Status Bar Integration](#status-bar-integration)
  - [Yasb](#yasb-yet-another-status-bar)
  - [Rainmeter](#rainmeter)
  - [Custom Widget](#custom-widget)

---

## Transport

HyprTile exposes two IPC transport mechanisms:

### Named Pipe (Local)

The primary IPC transport on Windows. Best for local clients, status bars, and scripts running on the same machine.

| Property | Value |
|----------|-------|
| Pipe Name | `\\.\pipe\hyprtile` |
| Access | Duplex (read/write) |
| Max Instances | Unlimited |
| Security | Same-user access only |

**Connection example (PowerShell):**

```powershell
$pipe = New-Object System.IO.Pipes.NamedPipeClientStream(
    ".", "hyprtile", [System.IO.Pipes.PipeDirection]::InOut
)
$pipe.Connect(1000)  # 1 second timeout
$writer = New-Object System.IO.StreamWriter($pipe)
$reader = New-Object System.IO.StreamReader($pipe)
```

### TCP Socket (Network)

Useful for status bar integration, remote monitoring, or network-based tooling.

| Property | Value |
|----------|-------|
| Address | `127.0.0.1` (localhost only) |
| Port | `9860` |
| Protocol | TCP |

**Connection example (netcat):**

```bash
nc localhost 9860
```

> **Security Note:** The TCP server binds to localhost only (`127.0.0.1`). It does not accept remote connections. This is by design for security.

---

## Message Format

All messages use **JSON** encoding with a newline (`\n`) terminator.

### Request Format

Each request is a single JSON object with a `command` field:

```json
{
    "command": "<command_name>",
    ...command-specific fields
}
```

The `command` field is required. Additional fields depend on the specific command.

**Example request:**

```json
{"command": "workspaces", "monitor": 0}
```

### Response Format

Each response is a single JSON object:

```json
{
    "success": true,
    "data": { ...command-specific data... },
    "error": null
}
```

Or on failure:

```json
{
    "success": false,
    "data": null,
    "error": "Descriptive error message"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `success` | boolean | `true` if the command executed successfully |
| `data` | object or null | Command-specific response data (varies by command) |
| `error` | string or null | Error message if `success` is `false` |

---

## Commands

### `workspaces`

Retrieve workspace information for a monitor.

**Request:**

```json
{
    "command": "workspaces",
    "monitor": 0
}
```

- `monitor` (optional, integer): Monitor ID. If omitted, returns workspaces for the focused monitor.

**Response (success):**

```json
{
    "success": true,
    "data": {
        "workspaces": [
            {"id": 1, "name": "1", "windows": 3, "has_focus": true},
            {"id": 2, "name": "2", "windows": 1, "has_focus": false},
            {"id": 3, "name": "3", "windows": 0, "has_focus": false},
            {"id": 4, "name": "4", "windows": 0, "has_focus": false},
            {"id": 5, "name": "5", "windows": 2, "has_focus": false}
        ]
    },
    "error": null
}
```

**Response fields:**

| Field | Type | Description |
|-------|------|-------------|
| `workspaces` | array | List of workspace info objects |
| `workspaces[].id` | integer | Workspace ID (1-based) |
| `workspaces[].name` | string | Workspace display name |
| `workspaces[].windows` | integer | Number of windows in this workspace |
| `workspaces[].has_focus` | boolean | Whether this workspace is currently active |

---

### `focused_window`

Get information about the currently focused window.

**Request:**

```json
{"command": "focused_window"}
```

**Response (success):**

```json
{
    "success": true,
    "data": {
        "window": {
            "id": 123456,
            "title": "README.md - hyprtile",
            "class": "wezterm-gui",
            "state": "tiling"
        }
    },
    "error": null
}
```

**Response (no focused window):**

```json
{
    "success": true,
    "data": {"window": null},
    "error": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `window` | object or null | Focused window info, or null if no window is focused |
| `window.id` | integer | Native window handle value |
| `window.title` | string | Window title text |
| `window.class` | string | Window class name |
| `window.state` | string | Window state: `"tiling"`, `"floating"`, `"fullscreen"`, or `"minimized"` |

---

### `layout`

Get the current layout and available layouts.

**Request:**

```json
{
    "command": "layout",
    "monitor": 0
}
```

- `monitor` (optional, integer): Monitor ID. If omitted, returns layout for the focused monitor.

**Response (success):**

```json
{
    "success": true,
    "data": {
        "layout": {
            "current": "dwindle",
            "available": ["dwindle", "master_stack", "monocle", "grid"]
        }
    },
    "error": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `layout.current` | string | Name of the active layout |
| `layout.available` | array | List of all available layout names |

---

### `window_count`

Get the total number of managed windows.

**Request:**

```json
{"command": "window_count"}
```

**Response (success):**

```json
{
    "success": true,
    "data": {"count": 12},
    "error": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `count` | integer | Total number of windows managed by HyprTile |

---

### `focus_direction`

Move focus in a direction.

**Request:**

```json
{
    "command": "focus_direction",
    "direction": "left"
}
```

- `direction` (required, string): One of: `"left"`, `"right"`, `"up"`, `"down"`

**Response (success):**

```json
{
    "success": true,
    "data": null,
    "error": null
}
```

---

### `move_direction`

Move the focused window in a direction.

**Request:**

```json
{
    "command": "move_direction",
    "direction": "right"
}
```

- `direction` (required, string): One of: `"left"`, `"right"`, `"up"`, `"down"`

**Response (success):**

```json
{
    "success": true,
    "data": null,
    "error": null
}
```

---

### `toggle_float`

Toggle floating state for the focused window.

**Request:**

```json
{"command": "toggle_float"}
```

**Response (success):**

```json
{
    "success": true,
    "data": {
        "new_state": "floating"
    },
    "error": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `new_state` | string | The new window state after toggling |

---

### `toggle_fullscreen`

Toggle fullscreen state for the focused window.

**Request:**

```json
{"command": "toggle_fullscreen"}
```

**Response (success):**

```json
{
    "success": true,
    "data": {
        "new_state": "fullscreen"
    },
    "error": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `new_state` | string | The new window state after toggling |

---

### `cycle_layout`

Cycle to the next layout algorithm.

**Request:**

```json
{"command": "cycle_layout"}
```

**Response (success):**

```json
{
    "success": true,
    "data": {
        "new_layout": "master_stack"
    },
    "error": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `new_layout` | string | The name of the newly activated layout |

---

### `switch_workspace`

Switch to a workspace by ID.

**Request:**

```json
{
    "command": "switch_workspace",
    "id": 3
}
```

- `id` (required, integer): Workspace ID (1-10, or as configured)

**Response (success):**

```json
{
    "success": true,
    "data": {
        "workspace": 3,
        "previous": 1
    },
    "error": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `workspace` | integer | The workspace ID switched to |
| `previous` | integer | The workspace ID switched from |

**Response (error - invalid workspace):**

```json
{
    "success": false,
    "data": null,
    "error": "Workspace 15 does not exist"
}
```

---

### `move_to_workspace`

Move the focused window to a workspace.

**Request:**

```json
{
    "command": "move_to_workspace",
    "id": 2
}
```

- `id` (required, integer): Target workspace ID

**Response (success):**

```json
{
    "success": true,
    "data": {
        "window_id": 123456,
        "workspace": 2
    },
    "error": null
}
```

| Field | Type | Description |
|-------|------|-------------|
| `window_id` | integer | The window that was moved |
| `workspace` | integer | The workspace the window was moved to |

---

### `reload_config`

Reload the configuration file from disk.

**Request:**

```json
{"command": "reload_config"}
```

**Response (success):**

```json
{
    "success": true,
    "data": {"message": "Configuration reloaded successfully"},
    "error": null
}
```

**Response (error):**

```json
{
    "success": false,
    "data": null,
    "error": "Failed to parse config: TOML syntax error at line 42"
}
```

---

### `exit`

Exit HyprTile gracefully, restoring normal Windows behavior.

**Request:**

```json
{"command": "exit"}
```

**Response (success):**

```json
{
    "success": true,
    "data": {"message": "HyprTile is shutting down"},
    "error": null
}
```

> **Note:** After sending `exit`, the connection will be closed. No further commands can be sent until HyprTile is restarted.

---

## Data Types

### WorkspaceInfo

```json
{
    "id": 1,
    "name": "1",
    "windows": 3,
    "has_focus": true
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | integer | Workspace ID (1-based) |
| `name` | string | Display name |
| `windows` | integer | Number of windows in workspace |
| `has_focus` | boolean | Whether this workspace is active |

### FocusedWindowInfo

```json
{
    "id": 123456,
    "title": "Window Title",
    "class": "WindowClass",
    "state": "tiling"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `id` | integer | Window handle value |
| `title` | string | Window title |
| `class` | string | Window class name |
| `state` | string | Current window state |

### LayoutInfo

```json
{
    "current": "dwindle",
    "available": ["dwindle", "master_stack", "monocle", "grid"]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `current` | string | Active layout name |
| `available` | array | All available layout names |

---

## Example Clients

### Rust Client

```rust
use serde::{Deserialize, Serialize};
use tokio::net::windows::named_pipe::{NamedPipeClient, ClientOptions};
use tokio::io::{AsyncReadExt, AsyncWriteExt};

const PIPE_NAME: &str = r"\\.\pipe\hyprtile";

#[derive(Serialize)]
struct IpcRequest {
    command: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    monitor: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    id: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    direction: Option<String>,
}

#[derive(Deserialize, Debug)]
struct IpcResponse {
    success: bool,
    data: Option<serde_json::Value>,
    error: Option<String>,
}

async fn send_command(json: &str) -> Result<IpcResponse, Box<dyn std::error::Error>> {
    let mut client = ClientOptions::new().open(PIPE_NAME)?;

    // Send request with newline terminator
    client.write_all(json.as_bytes()).await?;
    client.write_all(b"\n").await?;
    client.flush().await?;

    // Read response
    let mut buf = vec![0u8; 4096];
    let n = client.read(&mut buf).await?;
    let response: IpcResponse = serde_json::from_slice(&buf[..n])?;

    Ok(response)
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Get workspace info
    let req = r#"{"command":"workspaces","monitor":0}"#;
    let resp = send_command(req).await?;
    println!("Workspaces: {:?}", resp.data);

    // Switch to workspace 3
    let req = r#"{"command":"switch_workspace","id":3}"#;
    let resp = send_command(req).await?;
    println!("Switch result: {:?}", resp);

    Ok(())
}
```

### Python Client

```python
import json
import struct
import sys

# For Windows named pipes
if sys.platform == "win32":
    import win32file
    import win32pipe

    PIPE_NAME = r"\\.\pipe\hyprtile"

    def send_command(request_dict: dict) -> dict:
        """Send an IPC command to HyprTile via named pipe."""
        handle = win32file.CreateFile(
            PIPE_NAME,
            win32file.GENERIC_READ | win32file.GENERIC_WRITE,
            0,
            None,
            win32file.OPEN_EXISTING,
            0,
            None,
        )

        request_json = json.dumps(request_dict) + "\n"
        request_bytes = request_json.encode("utf-8")

        win32file.WriteFile(handle, request_bytes)

        response = b""
        while True:
            hr, data = win32file.ReadFile(handle, 4096)
            response += data
            if b"\n" in response or len(data) == 0:
                break

        win32file.CloseHandle(handle)
        return json.loads(response.decode("utf-8"))

else:
    # TCP fallback for non-Windows platforms
    import socket

    TCP_HOST = "127.0.0.1"
    TCP_PORT = 9860

    def send_command(request_dict: dict) -> dict:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.connect((TCP_HOST, TCP_PORT))

        request_json = json.dumps(request_dict) + "\n"
        sock.sendall(request_json.encode("utf-8"))

        response = b""
        while True:
            chunk = sock.recv(4096)
            if not chunk:
                break
            response += chunk
            if b"\n" in response:
                break

        sock.close()
        return json.loads(response.decode("utf-8"))


def get_workspaces(monitor: int = 0) -> list:
    """Get workspace information."""
    resp = send_command({"command": "workspaces", "monitor": monitor})
    if resp["success"] and resp["data"]:
        return resp["data"].get("workspaces", [])
    return []


def switch_workspace(workspace_id: int) -> bool:
    """Switch to a workspace."""
    resp = send_command({"command": "switch_workspace", "id": workspace_id})
    return resp["success"]


def get_focused_window() -> dict | None:
    """Get the currently focused window."""
    resp = send_command({"command": "focused_window"})
    if resp["success"] and resp["data"]:
        return resp["data"].get("window")
    return None


def toggle_float() -> bool:
    """Toggle floating for the focused window."""
    resp = send_command({"command": "toggle_float"})
    return resp["success"]


# Example usage
if __name__ == "__main__":
    print("Workspaces:")
    for ws in get_workspaces():
        focus = " *" if ws["has_focus"] else ""
        print(f"  [{ws['id']}] {ws['name']}: {ws['windows']} windows{focus}")

    print(f"\nFocused window: {get_focused_window()}")
```

### PowerShell Client

```powershell
<#
.SYNOPSIS
    Send a command to a running HyprTile instance via named pipe.
#>
function Send-HyprTileCommand {
    param(
        [Parameter(Mandatory=$true)]
        [string]$Command,

        [hashtable]$Params = @{}
    )

    $request = @{ command = $Command } + $Params
    $json = ($request | ConvertTo-Json -Compress) + "`n"
    $bytes = [System.Text.Encoding]::UTF8.GetBytes($json)

    $pipe = New-Object System.IO.Pipes.NamedPipeClientStream(
        ".", "hyprtile", [System.IO.Pipes.PipeDirection]::InOut
    )
    $pipe.Connect(2000)

    $writer = New-Object System.IO.BinaryWriter($pipe)
    $writer.Write($bytes)
    $writer.Flush()

    $reader = New-Object System.IO.StreamReader($pipe)
    $responseJson = $reader.ReadLine()
    $response = $responseJson | ConvertFrom-Json

    $writer.Close()
    $reader.Close()
    $pipe.Close()

    return $response
}

# Usage examples
Send-HyprTileCommand -Command "workspaces" -Params @{monitor = 0}
Send-HyprTileCommand -Command "switch_workspace" -Params @{id = 3}
Send-HyprTileCommand -Command "toggle_float"
Send-HyprTileCommand -Command "cycle_layout"
```

---

## Status Bar Integration

### Yasb (Yet Another Status Bar)

Add a custom widget to your Yasb config:

```python
# In your yasbar config
from core.widgets import BaseWidget
import json
import win32file

class HyprTileWidget(BaseWidget):
    def __init__(self, pipe_name=r"\\.\pipe\hyprtile"):
        super().__init__()
        self.pipe_name = pipe_name
        self.update_interval = 1000  # ms

    def get_workspaces(self):
        try:
            handle = win32file.CreateFile(
                self.pipe_name,
                win32file.GENERIC_READ | win32file.GENERIC_WRITE,
                0, None, win32file.OPEN_EXISTING, 0, None
            )
            req = b'{"command":"workspaces"}\n'
            win32file.WriteFile(handle, req)
            _, resp = win32file.ReadFile(handle, 4096)
            win32file.CloseHandle(handle)
            data = json.loads(resp.decode())
            return data.get("data", {}).get("workspaces", [])
        except Exception:
            return []

    def update(self):
        workspaces = self.get_workspaces()
        output = ""
        for ws in workspaces:
            if ws["has_focus"]:
                output += f"[{ws['id']}] "
            elif ws["windows"] > 0:
                output += f" {ws['id']}  "
            else:
                output += f" .  "
        self.set_text(output.strip())
```

### Rainmeter

Create a Rainmeter skin that queries HyprTile via the TCP socket:

```ini
; HyprTileWorkspaces.ini
[Rainmeter]
Update=1000

[MeasureWorkspaces]
Measure=Plugin
Plugin=WebParser
URL=file://#CURRENTPATH#workspaces.json
RegExp=(?siU)"id":(\d+).*?"has_focus":(true|false)
FinishAction=[!UpdateMeterGroup Workspaces]

; Use a PowerShell script to fetch workspaces
[MeasureScript]
Measure=Script
ScriptFile=#@#Scripts\hyprtile.ps1
UpdateDivider=5

[MeterWorkspace1]
Meter=String
X=10
Y=5
FontFace=Segoe UI
FontSize=11
FontColor=#FFFFFFFF#
Text=1

; Add conditional styling for active workspace
[MeterWorkspaceStyleActive]
FontWeight=700
FontColor=#FF00FF00#
```

PowerShell helper script (`hyprtile.ps1`):

```powershell
$pipe = New-Object System.IO.Pipes.NamedPipeClientStream(".", "hyprtile", "InOut")
$pipe.Connect(500)
$writer = New-Object System.IO.StreamWriter($pipe)
$reader = New-Object System.IO.StreamReader($pipe)
$writer.WriteLine('{"command":"workspaces"}')
$writer.Flush()
$response = $reader.ReadLine()
$writer.Close(); $reader.Close(); $pipe.Close()
$response | Out-File -FilePath "$PSScriptRoot\workspaces.json" -Encoding UTF8
```

### Custom Widget

For custom status bars or widgets, the recommended approach is:

1. **Connect** to the named pipe `\\.\pipe\hyprtile`
2. **Send** a JSON request with a newline terminator
3. **Read** the JSON response (single line, newline-terminated)
4. **Parse** and display the data
5. **Disconnect** (or keep connection open for streaming)

**Polling interval recommendations:**

| Use Case | Interval | Rationale |
|----------|----------|-----------|
| Workspace indicator | 500-1000ms | Workspace changes are infrequent |
| Window title | 250-500ms | Title changes are semi-frequent |
| Layout indicator | 1000ms | Layout changes are infrequent |
| Window count | 1000ms | Changes are event-driven |

For lower latency, use a persistent connection and read responses as they arrive, rather than polling.

---

## Command Summary

| Command | Required Params | Optional Params | Returns |
|---------|----------------|-----------------|---------|
| `workspaces` | — | `monitor` | List of workspace info |
| `focused_window` | — | — | Focused window info |
| `layout` | — | `monitor` | Current and available layouts |
| `window_count` | — | — | Total managed window count |
| `focus_direction` | `direction` | — | Success confirmation |
| `move_direction` | `direction` | — | Success confirmation |
| `toggle_float` | — | — | New window state |
| `toggle_fullscreen` | — | — | New window state |
| `cycle_layout` | — | — | New layout name |
| `switch_workspace` | `id` | — | Workspace switch result |
| `move_to_workspace` | `id` | — | Move result |
| `reload_config` | — | — | Reload status |
| `exit` | — | — | Shutdown confirmation |
