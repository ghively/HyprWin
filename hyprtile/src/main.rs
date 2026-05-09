use clap::Parser;
use hyprtile::{app::App, setup_logging};
use tracing::{error, info};

// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: ENTRY_POINT — Binary CLI and startup sequence.
// Before modifying startup:
//   1. setup_logging() must be called before any other operation.
//   2. set_dpi_awareness() should ideally be called here before App::new().
//   3. --print-default-config outputs TOML and exits — useful for new users.
//   4. --command sends IPC to running instance — don't spawn a second app.
//   5. All errors in main() should return non-zero exit codes.
// ═══════════════════════════════════════════════════════════════════════════════

/// Command-line interface for HyprTile.
///
/// Provides options for foreground mode, custom config, IPC commands,
/// config validation, and verbose logging.
#[derive(Parser, Debug)]
#[command(name = "hyprtile")]
#[command(about = "A Hyprland-inspired tiling window manager for Windows")]
#[command(version = env!("CARGO_PKG_VERSION"))]
struct Cli {
    /// Run in foreground (not as daemon)
    #[arg(short, long)]
    foreground: bool,

    /// Config file path
    #[arg(short, long)]
    config: Option<std::path::PathBuf>,

    /// Send IPC command to running instance
    #[arg(short, long)]
    command: Option<String>,

    /// Check configuration and exit
    #[arg(long)]
    check_config: bool,

    /// Print default configuration
    #[arg(long)]
    print_default_config: bool,

    /// Run with verbose logging
    #[arg(short, long)]
    verbose: bool,
}

/// Entry point for the HyprTile tiling window manager.
///
/// Parses CLI arguments, initializes logging, handles one-off commands
/// (print-default-config, check-config, send-command), then creates and runs
/// the main [`App`] coordinator.
fn main() {
    setup_logging();

    let cli = Cli::parse();

    // Handle one-off commands that don't require app initialization
    if cli.print_default_config {
        let config = hyprtile::config::defaults::default_config();
        match toml::to_string_pretty(&config) {
            Ok(toml_str) => println!("{}", toml_str),
            Err(e) => {
                error!("Failed to serialize default config: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Send IPC command to running instance and exit
    if let Some(cmd) = cli.command {
        let rt = match tokio::runtime::Runtime::new() {
            Ok(rt) => rt,
            Err(e) => {
                error!("Failed to create async runtime: {}", e);
                std::process::exit(1);
            }
        };
        rt.block_on(async {
            let request = match hyprtile::ipc::parse_request(cmd.as_bytes()) {
                Ok(req) => req,
                Err(e) => {
                    // Try as a simple command string
                    let json = format!("{{\"command\":\"{}\"}}", cmd);
                    match hyprtile::ipc::parse_request(json.as_bytes()) {
                        Ok(req) => req,
                        Err(_) => {
                            error!("Failed to parse IPC command '{}': {}", cmd, e);
                            std::process::exit(1);
                        }
                    }
                }
            };

            let json = match serde_json::to_vec(&request) {
                Ok(j) => j,
                Err(e) => {
                    error!("Failed to serialize IPC request: {}", e);
                    std::process::exit(1);
                }
            };

            // Write the request to the named pipe
            let pipe_path = hyprtile::DEFAULT_PIPE_NAME;
            match hyprtile::ipc::send_command(pipe_path, &json).await {
                Ok(response) => {
                    let json = hyprtile::ipc::serialize_response(&response);
                    match String::from_utf8(json) {
                        Ok(s) => println!("{}", s),
                        Err(_) => println!("{:?}", response),
                    }
                }
                Err(e) => {
                    error!("Failed to send IPC command: {}", e);
                    std::process::exit(1);
                }
            }
        });
        return;
    }

    info!("HyprTile {} starting", env!("CARGO_PKG_VERSION"));

    // Handle check-config: load and validate configuration
    if cli.check_config {
        let config_result = match &cli.config {
            Some(path) => hyprtile::config::ConfigManager::load_from_path(path),
            None => hyprtile::config::ConfigManager::load(),
        };
        match config_result {
            Ok(config) => {
                let errors = hyprtile::config::ConfigManager::validate(&config);
                if errors.is_empty() {
                    info!("Configuration is valid");
                } else {
                    for err in &errors {
                        error!("Config validation error: {}", err);
                    }
                    std::process::exit(1);
                }
            }
            Err(e) => {
                error!("Configuration error: {}", e);
                std::process::exit(1);
            }
        }
        return;
    }

    // Create and run the main application
    match App::new(cli.config) {
        Ok(mut app) => {
            if let Err(e) = app.run() {
                error!("Application error: {}", e);
                std::process::exit(1);
            }
        }
        Err(e) => {
            error!("Failed to initialize: {}", e);
            std::process::exit(1);
        }
    }
}
