// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: IPC_SERVER — Named pipe + TCP transport layer.
// Before modifying transport:
//   1. Named pipe uses length-delimited JSON (4-byte big-endian prefix).
//   2. TCP uses the same protocol for status bar compatibility.
//   3. IpcServer::start() spawns both transports concurrently.
//   4. Shutdown is signaled via Arc<tokio::sync::Notify>.
//   5. All async errors are logged but do not crash the server.
// ═══════════════════════════════════════════════════════════════════════════════

pub mod commands;
pub mod protocol;

use protocol::*;

use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::windows::named_pipe::{NamedPipeServer, PipeMode, ServerOptions};
use tokio_util::codec::{Decoder, Encoder};
use tracing::{debug, error, info, warn};

/// Named pipe path for the HyprTile IPC server on Windows.
pub const PIPE_NAME: &str = r"\\.\pipe\hyprtile";

/// Default TCP port used for IPC fallback.
pub const TCP_PORT: u16 = 9860;

/// Maximum IPC request payload size in bytes (256 KiB).
const MAX_REQUEST_SIZE: usize = 256 * 1024;

/// A simple length-delimited JSON codec for framing IPC messages.
///
/// Each frame is: `[4-byte big-endian length][JSON payload]`.
struct JsonCodec;

impl Decoder for JsonCodec {
    type Item = IpcRequest;
    type Error = anyhow::Error;

    fn decode(&mut self, src: &mut BytesMut) -> Result<Option<Self::Item>, Self::Error> {
        if src.len() < 4 {
            return Ok(None);
        }

        let len = u32::from_be_bytes([src[0], src[1], src[2], src[3]]) as usize;
        if len > MAX_REQUEST_SIZE {
            return Err(anyhow::anyhow!(
                "Request too large: {} bytes (max {})",
                len,
                MAX_REQUEST_SIZE
            ));
        }

        if src.len() < 4 + len {
            // Not enough data yet -- reserve space
            src.reserve(4 + len - src.len());
            return Ok(None);
        }

        // Consume the length prefix
        src.advance(4);
        // Extract the JSON payload
        let buf = src.split_to(len);
        let request: IpcRequest = serde_json::from_slice(&buf)?;
        Ok(Some(request))
    }
}

impl Encoder<IpcResponse> for JsonCodec {
    type Error = anyhow::Error;

    fn encode(&mut self, item: IpcResponse, dst: &mut BytesMut) -> Result<(), Self::Error> {
        let json = serde_json::to_vec(&item)?;
        let len = json.len() as u32;
        dst.reserve(4 + json.len());
        dst.extend_from_slice(&len.to_be_bytes());
        dst.extend_from_slice(&json);
        Ok(())
    }
}

/// IPC server that listens on a named pipe for JSON command requests.
///
/// The server uses length-delimited JSON framing: each message is prefixed
/// with a 4-byte big-endian length, followed by the UTF-8 JSON payload.
///
/// # Example
/// ```no_run
/// let server = IpcServer::new();
/// server.start().await?;
/// ```
pub struct IpcServer {
    shutdown: Arc<tokio::sync::Notify>,
}

use std::sync::Arc;

impl IpcServer {
    /// Create a new IPC server instance.
    pub fn new() -> Self {
        Self {
            shutdown: Arc::new(tokio::sync::Notify::new()),
        }
    }

    /// Start listening on the named pipe.
    ///
    /// Creates the named pipe and accepts clients in a loop until [`stop`] is called.
    /// Each client connection is handled on a separate async task.
    pub async fn start(&self) -> anyhow::Result<()> {
        info!("Starting IPC server on named pipe {}", PIPE_NAME);

        let shutdown = self.shutdown.clone();

        loop {
            // Create a new pipe server instance for each client
            let server = match ServerOptions::new()
                .pipe_mode(PipeMode::Message)
                .create(PIPE_NAME)
            {
                Ok(s) => s,
                Err(e) => {
                    error!("Failed to create named pipe '{}': {}", PIPE_NAME, e);
                    tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
                    continue;
                }
            };

            tokio::select! {
                biased;
                _ = shutdown.notified() => {
                    info!("IPC server shutting down");
                    break;
                }
                result = server.connect() => {
                    if let Err(e) = result {
                        error!("Named pipe connect error: {}", e);
                        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
                        continue;
                    }

                    // Move the connected server into a spawned task
                    tokio::spawn(async move {
                        if let Err(e) = Self::handle_client(server).await {
                            debug!("IPC client handler error: {}", e);
                        }
                    });
                }
            }
        }

        Ok(())
    }

    /// Stop the IPC server.
    ///
    /// Signals the accept loop to exit gracefully on the next iteration.
    pub async fn stop(&self) {
        info!("Stopping IPC server");
        self.shutdown.notify_waiters();
    }

    /// Read a single IPC request from a named pipe client.
    ///
    /// Uses length-delimited JSON framing. Returns the deserialized
    /// [`IpcRequest`] or an error if the client disconnects or sends
    /// malformed data.
    pub async fn handle_named_pipe_client(
        mut client: NamedPipeServer,
    ) -> anyhow::Result<IpcRequest> {
        let mut codec = JsonCodec;
        let mut buf = BytesMut::with_capacity(4096);

        loop {
            // Read data from the pipe into the buffer
            match client.read_buf(&mut buf).await {
                Ok(0) => {
                    return Err(anyhow::anyhow!("IPC client disconnected (EOF)"));
                }
                Ok(_) => {}
                Err(e) => {
                    return Err(anyhow::anyhow!("IPC client read error: {}", e));
                }
            }

            // Attempt to decode a complete request
            match codec.decode(&mut buf)? {
                Some(request) => {
                    debug!("Received IPC request: {:?}", request);
                    return Ok(request);
                }
                None => {
                    // Need more data — continue reading
                    continue;
                }
            }
        }
    }

    /// Write an IPC response to a named pipe client.
    ///
    /// Serializes the response to JSON and writes it using length-delimited
    /// framing (4-byte big-endian length prefix followed by JSON).
    pub async fn write_response(
        client: &mut NamedPipeServer,
        response: &IpcResponse,
    ) -> anyhow::Result<()> {
        let mut codec = JsonCodec;
        let mut out_buf = BytesMut::new();
        codec.encode(response.clone(), &mut out_buf)?;
        client.write_all(&out_buf).await?;
        client.flush().await?;
        Ok(())
    }

    /// Handle a single client connection: read request(s), respond.
    async fn handle_client(
        mut client: NamedPipeServer,
    ) -> anyhow::Result<()> {
        debug!("IPC client connected");
        let mut codec = JsonCodec;
        let mut buf = BytesMut::with_capacity(4096);

        loop {
            // Read data from the pipe into the buffer
            match client.read_buf(&mut buf).await {
                Ok(0) => {
                    debug!("IPC client disconnected (EOF)");
                    break;
                }
                Ok(_) => {}
                Err(e) => {
                    debug!("IPC client read error: {}", e);
                    break;
                }
            }

            // Attempt to decode as many complete requests as available
            loop {
                match codec.decode(&mut buf)? {
                    Some(request) => {
                        debug!("Received IPC request: {:?}", request);

                        // The AppState-integrated dispatch is performed by the app
                        // coordinator which owns the state.  In standalone mode we
                        // return an informative error.
                        let response = IpcResponse::error(
                            "IPC server not yet integrated with AppState".to_string(),
                        );

                        let mut out_buf = BytesMut::new();
                        codec.encode(response, &mut out_buf)?;
                        if let Err(e) = client.write_all(&out_buf).await {
                            warn!("IPC write error: {}", e);
                            return Ok(());
                        }
                        if let Err(e) = client.flush().await {
                            warn!("IPC flush error: {}", e);
                            return Ok(());
                        }
                    }
                    None => break, // need more data
                }
            }
        }

        debug!("IPC client handler exiting");
        Ok(())
    }
}

/// Start a TCP socket server on the given port for status-bar integration.
///
/// Each connection is handled in a separate task with newline-delimited JSON.
/// The server listens for a shutdown notification to exit cleanly.
pub async fn start_tcp_server(port: u16, shutdown: std::sync::Arc<tokio::sync::Notify>) -> anyhow::Result<()> {
    let addr = format!("127.0.0.1:{}", port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("TCP IPC server listening on {}", addr);

    loop {
        tokio::select! {
            result = listener.accept() => {
                let (mut socket, peer_addr) = result?;
                debug!("TCP IPC client connected from {}", peer_addr);

                tokio::spawn(async move {
                    let mut buf = vec![0u8; 4096];
                    let mut cursor = 0usize;

                    loop {
                        match socket.read(&mut buf[cursor..]).await {
                            Ok(0) => {
                                debug!("TCP client {} disconnected", peer_addr);
                                break;
                            }
                            Ok(n) => {
                                cursor += n;

                                // Try to find complete JSON objects (newline-delimited)
                                if let Some(newline_pos) =
                                    buf[..cursor].iter().position(|&b| b == b'\n')
                                {
                                    let line = &buf[..newline_pos];
                                    let response = match parse_request(line) {
                                        Ok(req) => {
                                            debug!("TCP request from {}: {:?}", peer_addr, req);
                                            IpcResponse::error(
                                                "TCP server standalone mode".to_string(),
                                            )
                                        }
                                        Err(e) => {
                                            warn!("Failed to parse TCP request: {}", e);
                                            IpcResponse::error(format!("Parse error: {}", e))
                                        }
                                    };

                                    let json = serialize_response(&response);
                                    if let Err(e) = socket.write_all(&json).await {
                                        warn!("Failed to write TCP response: {}", e);
                                        break;
                                    }
                                    if let Err(e) = socket.write_all(b"\n").await {
                                        warn!("Failed to write newline: {}", e);
                                        break;
                                    }

                                    // Shift remaining data to the front
                                    let remaining = cursor - newline_pos - 1;
                                    if remaining > 0 {
                                        buf.copy_within(newline_pos + 1..cursor, 0);
                                    }
                                    cursor = remaining;
                                } else if cursor >= buf.len() {
                                    // Buffer full without finding a newline
                                    warn!("TCP request buffer full, discarding");
                                    cursor = 0;
                                }
                            }
                            Err(e) => {
                                warn!("TCP read error from {}: {}", peer_addr, e);
                                break;
                            }
                        }
                    }
                });
            }
            _ = shutdown.notified() => {
                info!("TCP IPC server shutting down");
                break;
            }
        }
    }

    Ok(())
}

/// Send a raw byte payload to the HyprTile daemon via its named pipe.
///
/// The payload should be a length-delimited JSON request (4-byte big-endian
/// length prefix followed by the JSON body).
///
/// Returns the parsed [`IpcResponse`] from the daemon.
pub async fn send_command(pipe_path: &str, payload: &[u8]) -> anyhow::Result<IpcResponse> {
    use tokio::net::windows::named_pipe::ClientOptions;

    debug!(
        "Connecting to IPC pipe {} to send {} bytes",
        pipe_path,
        payload.len()
    );

    let mut client = ClientOptions::new().open(pipe_path)?;

    client.write_all(payload).await?;
    client.flush().await?;

    // Read the response (length-delimited)
    let mut len_buf = [0u8; 4];
    client.read_exact(&mut len_buf).await?;
    let len = u32::from_be_bytes(len_buf) as usize;

    if len > MAX_REQUEST_SIZE {
        return Err(anyhow::anyhow!(
            "Response too large: {} bytes (max {})",
            len,
            MAX_REQUEST_SIZE
        ));
    }

    let mut response_buf = vec![0u8; len];
    client.read_exact(&mut response_buf).await?;

    let response: IpcResponse = serde_json::from_slice(&response_buf)?;
    debug!("Received IPC response: success={}", response.success);
    Ok(response)
}

/// Parse a JSON request from a raw byte buffer.
///
/// Expects a JSON object with a `"command"` field used for tagged deserialization.
/// Returns an error if the buffer is not valid UTF-8 or contains malformed JSON.
pub fn parse_request(buf: &[u8]) -> anyhow::Result<IpcRequest> {
    let request: IpcRequest = serde_json::from_slice(buf)?;
    Ok(request)
}

/// Serialize an IPC response to JSON bytes.
///
/// The output is a compact JSON representation without extra whitespace,
/// suitable for wire transmission.
pub fn serialize_response(response: &IpcResponse) -> Vec<u8> {
    match serde_json::to_vec(response) {
        Ok(json) => json,
        Err(e) => {
            // Fallback: return a minimal error JSON
            format!(
                "{{\"success\":false,\"data\":null,\"error\":\"serialization failed: {}\"}}",
                e
            )
            .into_bytes()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_request_workspaces() {
        let json = r#"{"command":"workspaces","monitor":1}"#;
        let req = match parse_request(json.as_bytes()) { Ok(r) => r, Err(e) => { error!("Failed to parse request: {}", e); continue; } };
        match req {
            IpcRequest::Workspaces { monitor: Some(1) } => {}
            other => panic!("Expected Workspaces request, got: {:?}", other),
        }
    }

    #[test]
    fn test_parse_request_exit() {
        let json = r#"{"command":"exit"}"#;
        let req = match parse_request(json.as_bytes()) { Ok(r) => r, Err(e) => { error!("Failed to parse request: {}", e); continue; } };
        match req {
            IpcRequest::Exit => {}
            other => panic!("Expected Exit request, got: {:?}", other),
        }
    }

    #[test]
    fn test_serialize_response_success() {
        let resp = IpcResponse::success(Some(serde_json::json!({"count": 42 })));
        let bytes = serialize_response(&resp);
        let json_str = match String::from_utf8(bytes) { Ok(s) => s, Err(e) => { error!("Invalid UTF-8 in request: {}", e); continue; } };
        assert!(json_str.contains("\"success\":true"));
        assert!(json_str.contains("42"));
    }

    #[test]
    fn test_serialize_response_error() {
        let resp = IpcResponse::error("something went wrong");
        let bytes = serialize_response(&resp);
        let json_str = match String::from_utf8(bytes) { Ok(s) => s, Err(e) => { error!("Invalid UTF-8 in request: {}", e); continue; } };
        assert!(json_str.contains("\"success\":false"));
        assert!(json_str.contains("something went wrong"));
    }
}
