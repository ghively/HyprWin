// ═══════════════════════════════════════════════════════════════════════════════
// AI_AGENT_STOP: CONFIG_MANAGER — TOML loading, validation, hot-reload.
// Before modifying config behavior:
//   1. Config path: %APPDATA%\hyprtile\hyprtile.toml.
//   2. ensure_default_config() writes defaults if file is missing.
//   3. start_watching() uses notify crate — watch the directory, not the file.
//   4. validate() returns Vec<String> of issues — empty = valid config.
//   5. Hot-reload replaces the Arc<RwLock<Config>> — old readers stay valid.
// ═══════════════════════════════════════════════════════════════════════════════

pub mod defaults;
pub mod types;

use std::path::{Path, PathBuf};
use std::sync::{Arc, RwLock};
use std::time::Duration;

use anyhow::{Context, Result};
use notify::{Event, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{error, info, warn};

use types::*;

/// Manages the HyprTile configuration file: loading, hot-reloading, and
/// validation.
///
/// # Usage
///
/// ```ignore
/// let manager = ConfigManager::new()?;
/// let config = manager.get()?;
/// // use config...
/// manager.reload()?;           // manual reload
/// manager.start_watching()?;   // auto-reload on file change
/// ```
pub struct ConfigManager {
    config: Arc<RwLock<Config>>,
    config_path: PathBuf,
    _watcher: Option<RecommendedWatcher>,
}

impl ConfigManager {
    /// Create a new `ConfigManager`.
    ///
    /// 1. Ensures the config directory exists.
    /// 2. Writes a default config file if none exists.
    /// 3. Loads the configuration from disk.
    pub fn new() -> Result<Self> {
        let config_path = Self::get_config_path();

        // Ensure directory and default config exist.
        Self::ensure_config_dir()?;
        if !config_path.exists() {
            info!("No config file found, writing defaults");
            Self::ensure_default_config()?;
        }

        let config = Self::load_from_path(&config_path)?;
        info!("Configuration loaded from {}", config_path.display());

        Ok(Self {
            config: Arc::new(RwLock::new(config)),
            config_path,
            _watcher: None,
        })
    }

    /// Load configuration from the default config path.
    pub fn load() -> Result<Config> {
        let path = Self::get_config_path();
        Self::load_from_path(&path)
    }

    /// Load configuration from an explicit file path.
    ///
    /// # Errors
    ///
    /// Returns an error if the file cannot be read or parsed as TOML.
    pub fn load_from_path(path: &Path) -> Result<Config> {
        let contents = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&contents)
            .with_context(|| format!("failed to parse config file: {}", path.display()))?;

        let issues = Self::validate(&config);
        if !issues.is_empty() {
            warn!("Configuration validation issues found:");
            for issue in &issues {
                warn!("  - {}", issue);
            }
        }

        Ok(config)
    }

    /// Get the default configuration file path (`%APPDATA%\hyprtile\hyprtile.toml`).
    pub fn get_config_path() -> PathBuf {
        config_file_path()
    }

    /// Write a default configuration file if one does not already exist.
    ///
    /// Returns the path to the config file.
    pub fn ensure_default_config() -> Result<PathBuf> {
        let path = Self::get_config_path();

        if path.exists() {
            return Ok(path);
        }

        Self::ensure_config_dir()?;

        let default_config = defaults::default_config();
        let toml_string = toml::to_string_pretty(&default_config)
            .context("failed to serialize default config")?;

        std::fs::write(&path, toml_string)
            .with_context(|| format!("failed to write default config to {}", path.display()))?;

        info!("Default config written to {}", path.display());
        Ok(path)
    }

    /// Get a read-lock guard on the current configuration.
    ///
    /// # Errors
    ///
    /// Returns `PoisonError` if the lock is poisoned (a writer panicked
    /// while holding the lock).
    pub fn get(&self) -> std::sync::LockResult<std::sync::RwLockReadGuard<'_, Config>> {
        self.config.read()
    }

    /// Reload the configuration from disk.
    ///
    /// The in-memory config is replaced atomically.
    pub fn reload(&self) -> Result<()> {
        info!(
            "Reloading configuration from {}",
            self.config_path.display()
        );

        let new_config = Self::load_from_path(&self.config_path)?;

        let mut guard = self
            .config
            .write()
            .map_err(|e| anyhow::anyhow!("config lock poisoned: {}", e))?;
        *guard = new_config;

        info!("Configuration reloaded successfully");
        Ok(())
    }

    /// Start watching the config directory for changes.
    ///
    /// When the config file is modified, the configuration is automatically
    /// reloaded. The watcher runs in a background thread.
    ///
    /// # Errors
    ///
    /// Returns an error if the file watcher cannot be created.
    pub fn start_watching(&mut self) -> Result<()> {
        let config_path = self.config_path.clone();
        let config_arc = Arc::clone(&self.config);

        let watcher = notify::recommended_watcher(move |res: Result<Event, notify::Error>| {
            match res {
                Ok(event) => {
                    // Only react to modify / create / remove events.
                    let relevant = matches!(
                        event.kind,
                        notify::EventKind::Modify(_)
                            | notify::EventKind::Create(_)
                            | notify::EventKind::Remove(_)
                    );
                    if !relevant {
                        return;
                    }

                    // Check if the changed file is our config.
                    let is_our_file = event.paths.iter().any(|p| p == &config_path);
                    if !is_our_file {
                        return;
                    }

                    // Small debounce to avoid reloading on rapid successive events.
                    std::thread::sleep(Duration::from_millis(100));

                    info!("Config file changed on disk, auto-reloading");

                    let new_config = match ConfigManager::load_from_path(&config_path) {
                        Ok(cfg) => cfg,
                        Err(e) => {
                            error!("Failed to reload config: {}", e);
                            return;
                        }
                    };

                    match config_arc.write() {
                        Ok(mut guard) => {
                            *guard = new_config;
                            info!("Configuration auto-reloaded successfully");
                        }
                        Err(e) => {
                            error!("Config lock poisoned during auto-reload: {}", e);
                        }
                    }
                }
                Err(e) => {
                    error!("Config file watcher error: {}", e);
                }
            }
        })
        .context("failed to create config file watcher")?;

        let mut watcher: RecommendedWatcher = watcher;

        // Watch the parent directory so we catch renames/overwrites.
        let watch_dir = self.config_path.parent().unwrap_or_else(|| Path::new("."));
        watcher
            .watch(watch_dir, RecursiveMode::NonRecursive)
            .context("failed to start watching config directory")?;

        self._watcher = Some(watcher);
        info!("Started watching config directory: {}", watch_dir.display());

        Ok(())
    }

    /// Validate a configuration and return a list of human-readable issues.
    ///
    /// An empty vector means the configuration is valid.
    pub fn validate(config: &Config) -> Vec<String> {
        let mut issues = Vec::new();

        // Validate general settings
        if config.general.resize_border_width == 0 && config.general.resize_on_border {
            issues.push(
                "general.resize_border_width is 0 but resize_on_border is enabled".to_string(),
            );
        }

        if config.general.terminal.is_empty() {
            issues.push("general.terminal is empty".to_string());
        }

        // Validate gaps
        if config.gaps.inner > 100 {
            issues.push(format!(
                "gaps.inner is very large ({}), this may leave unusable screen space",
                config.gaps.inner
            ));
        }
        if config.gaps.outer > 100 {
            issues.push(format!(
                "gaps.outer is very large ({}), this may leave unusable screen space",
                config.gaps.outer
            ));
        }

        // Validate workspaces
        if config.workspaces.count == 0 {
            issues.push("workspaces.count must be at least 1".to_string());
        }
        if config.workspaces.count > 50 {
            issues.push(format!(
                "workspaces.count ({}) is unusually high",
                config.workspaces.count
            ));
        }

        // Validate monitor workspace assignments
        for (idx, monitor) in config.monitors.iter().enumerate() {
            for &ws_id in &monitor.workspaces {
                if ws_id == 0 || ws_id > config.workspaces.count {
                    issues.push(format!(
                        "monitors[{}].workspaces contains workspace {} which is outside the valid range 1..={}",
                        idx, ws_id, config.workspaces.count
                    ));
                }
            }

            let valid_layouts = ["dwindle", "master_stack", "monocle", "grid"];
            if !valid_layouts.contains(&monitor.default_layout.as_str()) {
                issues.push(format!(
                    "monitors[{}].default_layout '{}' is not a recognised layout (expected one of: {:?})",
                    idx, monitor.default_layout, valid_layouts
                ));
            }
        }

        // Validate window rules
        for (idx, rule) in config.window_rules.iter().enumerate() {
            let has_match = rule.match_class.is_some()
                || rule.match_title.is_some()
                || rule.match_process.is_some();
            if !has_match {
                issues.push(format!(
                    "window_rules[{}] has no match criteria (match_class, match_title, match_process)",
                    idx
                ));
            }

            if let Some(ref class) = rule.match_class
                && class.is_empty()
            {
                issues.push(format!(
                    "window_rules[{}].match_class is an empty string",
                    idx
                ));
            }
            if let Some(ref title) = rule.match_title
                && title.is_empty()
            {
                issues.push(format!(
                    "window_rules[{}].match_title is an empty string",
                    idx
                ));
            }
            if let Some(ref process) = rule.match_process
                && process.is_empty()
            {
                issues.push(format!(
                    "window_rules[{}].match_process is an empty string",
                    idx
                ));
            }
        }

        // Validate keybinds
        for (key, action) in &config.keybinds {
            if key.is_empty() {
                issues.push("keybinds contains an empty key".to_string());
            }
            if action.is_empty() {
                issues.push(format!("keybinds['{}'] has an empty action", key));
            }
        }

        issues
    }

    /// Ensure the config directory exists, creating it if necessary.
    fn ensure_config_dir() -> Result<()> {
        let dir = config_dir();
        if !dir.exists() {
            std::fs::create_dir_all(&dir)
                .with_context(|| format!("failed to create config directory: {}", dir.display()))?;
        }
        Ok(())
    }
}

/// Get the HyprTile configuration directory (`%APPDATA%\hyprtile`).
pub fn config_dir() -> PathBuf {
    dirs::config_dir()
        .map(|d| d.join("hyprtile"))
        .unwrap_or_else(|| PathBuf::from(".").join("hyprtile"))
}

/// Get the full path to the HyprTile configuration file.
pub fn config_file_path() -> PathBuf {
    config_dir().join("hyprtile.toml")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_validate_empty_terminal() {
        let mut config = defaults::default_config();
        config.general.terminal = String::new();
        let issues = ConfigManager::validate(&config);
        assert!(issues.iter().any(|i| i.contains("terminal is empty")));
    }

    #[test]
    fn test_validate_zero_workspace_count() {
        let mut config = defaults::default_config();
        config.workspaces.count = 0;
        let issues = ConfigManager::validate(&config);
        assert!(
            issues
                .iter()
                .any(|i| i.contains("workspaces.count must be at least"))
        );
    }

    #[test]
    fn test_validate_invalid_monitor_layout() {
        let mut config = defaults::default_config();
        config.monitors.push(MonitorConfig {
            id: 0,
            workspaces: vec![1, 2],
            default_layout: "invalid_layout".to_string(),
        });
        let issues = ConfigManager::validate(&config);
        assert!(issues.iter().any(|i| i.contains("not a recognised layout")));
    }

    #[test]
    fn test_validate_window_rule_no_match() {
        let mut config = defaults::default_config();
        config.window_rules.push(WindowRule {
            match_class: None,
            match_title: None,
            match_process: None,
            action: Some(WindowAction::Float),
            workspace: None,
            monitor: None,
            size: None,
            position: None,
        });
        let issues = ConfigManager::validate(&config);
        assert!(issues.iter().any(|i| i.contains("no match criteria")));
    }

    #[test]
    fn test_validate_good_config() {
        let config = defaults::default_config();
        let issues = ConfigManager::validate(&config);
        // A good default config should have no issues
        assert!(issues.is_empty(), "Expected no issues, got: {:?}", issues);
    }

    #[test]
    fn test_config_paths() {
        let dir = config_dir();
        assert!(dir.to_string_lossy().contains("hyprtile"));

        let file = config_file_path();
        assert_eq!(file.file_name().unwrap(), "hyprtile.toml");
    }

    #[test]
    fn test_default_config_serializes_to_toml() {
        let config = defaults::default_config();
        let toml_str = toml::to_string_pretty(&config).unwrap();
        assert!(toml_str.contains("mod_key"));
        assert!(toml_str.contains("terminal"));
        assert!(toml_str.contains("gaps"));
    }
}
