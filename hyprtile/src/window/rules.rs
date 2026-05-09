//! Window rules engine.
//!
//! Applies user-defined [`WindowRule`]s from the configuration file to
//! [`Window`] objects at registration time.  Rules can force a window
//! to float, pin it to a specific workspace or monitor, or set its
/// initial size.
use crate::config::types::{WindowAction, WindowRule};
use crate::window::model::Window;
use regex::Regex;
use tracing::{debug, warn};

// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•
// AI_AGENT_STOP: RULE_ENGINE â€” Regex-based window classification.
// Before adding new rule types:
//   1. WindowRule fields: match_class, match_title, match_process (all regex).
//   2. Rules are evaluated in order â€” first match wins.
//   3. Action can be: Float, Tile, or assign workspace/monitor/size/position.
//   4. Position string values: "center", "bottom_right", "top_left", etc.
//   5. Regex compilation happens once at RuleEngine construction.
// â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•â•

/// Owns the list of [`WindowRule`]s and exposes query helpers.
///
/// Created once from the parsed [`Config`](crate::config::types::Config)
/// and can be hot-reloaded when the configuration changes.
pub struct RuleEngine {
    rules: Vec<WindowRule>,
}

impl RuleEngine {
    /// Build a new engine from a (possibly empty) list of rules.
    pub fn new(rules: Vec<WindowRule>) -> Self {
        Self { rules }
    }

    /// Apply all matching rules to a freshly registered window.
    ///
    /// This mutates the window in-place:
    /// * `Float` action  -> [`WindowState::Floating`](super::model::WindowState::Floating)
    /// * `Tile` action   -> [`WindowState::Tiling`](super::model::WindowState::Tiling)
    /// * `workspace`     -> recorded externally by the [`WindowManager`](super::WindowManager)
    /// * `monitor`       -> recorded externally by the [`WindowManager`](super::WindowManager)
    /// * `size`          -> stored in `floating_rect`
    pub fn apply_rules(&self, window: &mut Window) {
        let matching = self.find_matching_rules(window);
        for rule in matching {
            debug!(
                "Applying rule to window {} (class='{}', title='{}'): {:?}",
                window.id.0, window.class_name, window.title, rule
            );

            if let Some(ref action) = rule.action {
                match action {
                    WindowAction::Float => {
                        window.state = super::model::WindowState::Floating;
                    }
                    WindowAction::Tile => {
                        window.state = super::model::WindowState::Tiling;
                    }
                }
            }

            if let Some(size) = rule.size {
                let current_rect = window
                    .floating_rect
                    .unwrap_or_else(|| Rect::new(100, 100, 800, 600));
                window.floating_rect = Some(Rect::new(
                    current_rect.x,
                    current_rect.y,
                    size[0] as i32,
                    size[1] as i32,
                ));
            }
        }
    }

    /// Return **all** rules that match the given window.
    pub fn find_matching_rules(&self, window: &Window) -> Vec<&WindowRule> {
        self.rules
            .iter()
            .filter(|rule| window_matches_rule(window, rule))
            .collect()
    }

    /// Return `true` if **any** matching rule requests floating.
    ///
    /// This is used by the layout engine to decide whether to include
    /// the window in tiling calculations.
    pub fn should_float(&self, window: &Window) -> bool {
        self.find_matching_rules(window)
            .iter()
            .any(|rule| matches!(rule.action, Some(WindowAction::Float)))
    }

    /// Return the workspace ID requested by the first matching rule
    /// that specifies one.
    pub fn target_workspace(&self, window: &Window) -> Option<u32> {
        self.find_matching_rules(window)
            .iter()
            .find_map(|rule| rule.workspace)
    }

    /// Return the monitor ID requested by the first matching rule
    /// that specifies one.
    pub fn target_monitor(&self, window: &Window) -> Option<u32> {
        self.find_matching_rules(window)
            .iter()
            .find_map(|rule| rule.monitor)
    }

    /// Replace the current rule set with a new one (hot-reload).
    pub fn reload_rules(&mut self, rules: Vec<WindowRule>) {
        debug!("Reloading window rules: {} rules", rules.len());
        self.rules = rules;
    }
}

/// Check whether a single window matches a single rule.
///
/// All present matchers on the rule (`match_class`, `match_title`,
/// `match_process`) must match.  If the rule has no matchers at all
/// it does **not** match (avoids accidentally globbing every window).
pub fn window_matches_rule(window: &Window, rule: &WindowRule) -> bool {
    let mut has_matcher = false;
    let mut all_match = true;

    if let Some(ref pattern) = rule.match_class {
        has_matcher = true;
        if !class_matches(&window.class_name, pattern) {
            all_match = false;
        }
    }

    if let Some(ref pattern) = rule.match_title {
        has_matcher = true;
        if !title_matches(&window.title, pattern) {
            all_match = false;
        }
    }

    if let Some(ref pattern) = rule.match_process {
        has_matcher = true;
        if !process_matches(&window.process_name, pattern) {
            all_match = false;
        }
    }

    has_matcher && all_match
}

/// Match a window class name against a pattern (regex or literal).
pub fn class_matches(class: &str, pattern: &str) -> bool {
    match Regex::new(pattern) {
        Ok(re) => re.is_match(class),
        Err(e) => {
            warn!(
                "Invalid class regex '{}': {}, falling back to literal",
                pattern, e
            );
            class == pattern
        }
    }
}

/// Match a window title against a pattern (regex or literal).
pub fn title_matches(title: &str, pattern: &str) -> bool {
    match Regex::new(pattern) {
        Ok(re) => re.is_match(title),
        Err(e) => {
            warn!(
                "Invalid title regex '{}': {}, falling back to literal",
                pattern, e
            );
            title == pattern
        }
    }
}

/// Match a process name against a pattern (regex or literal).
pub fn process_matches(process: &str, pattern: &str) -> bool {
    match Regex::new(pattern) {
        Ok(re) => re.is_match(process),
        Err(e) => {
            warn!(
                "Invalid process regex '{}': {}, falling back to literal",
                pattern, e
            );
            process == pattern
        }
    }
}

use crate::util::rect::Rect;
