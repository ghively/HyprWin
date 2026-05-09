pub mod app;
pub mod config;
pub mod ipc;
pub mod layout;
pub mod platform;
pub mod util;
pub mod window;
pub mod workspace;

use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize the tracing subscriber for logging.
///
/// Uses the `RUST_LOG` environment variable for filtering, defaulting to `hyprtile=info`.
/// This should be called once at application startup.
pub fn setup_logging() {
    tracing_subscriber::registry()
        .with(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "hyprtile=info".into()),
        )
        .with(tracing_subscriber::fmt::layer().with_target(true))
        .init();
}

/// Application version from Cargo.toml.
pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// Application display name.
pub const APP_NAME: &str = "HyprTile";

/// Default named pipe path for IPC on Windows.
pub const DEFAULT_PIPE_NAME: &str = r"\\.\pipe\hyprtile";

/// Default TCP port for IPC fallback.
pub const DEFAULT_TCP_PORT: u16 = 9860;
