// ============================================================================
// HyprTile Integration Tests
// ============================================================================
//
// Comprehensive test suite covering:
// - Rectangle math operations
// - Layout algorithm calculations
// - BSP tree operations
// - Window state machine transitions
// - Configuration parsing and validation
// - IPC protocol serialization
// - Workspace management
// - Animation easing and interpolation
// - Window rule matching
//
// Total: 55 tests

#![allow(dead_code)]

use std::collections::HashMap;

// Import the hyprtile library modules
use hyprtile::config::defaults::default_config;
use hyprtile::config::types::*;
use hyprtile::ipc::protocol::*;
use hyprtile::layout::bsp::{Node, SplitDirection, build_dwindle_tree};
use hyprtile::layout::dwindle::DwindleLayout;
use hyprtile::layout::gaps::{apply_gaps, effective_gaps};
use hyprtile::layout::grid::GridLayout;
use hyprtile::layout::master_stack::{MasterStackConfig, MasterStackLayout, Orientation};
use hyprtile::layout::monocle::MonocleLayout;
use hyprtile::layout::*;
use hyprtile::util::animation::*;
use hyprtile::util::rect::Rect;
use hyprtile::window::model::*;
use hyprtile::window::rules::{
    RuleEngine, class_matches, process_matches, title_matches, window_matches_rule,
};
use hyprtile::workspace::model::*;

// ============================================================================
// 1. Rect Math Tests (8 tests)
// ============================================================================

#[test]
fn test_rect_creation() {
    let r = Rect::new(10, 20, 800, 600);
    assert_eq!(r.x, 10);
    assert_eq!(r.y, 20);
    assert_eq!(r.width, 800);
    assert_eq!(r.height, 600);
}

#[test]
fn test_rect_contains_point() {
    let r = Rect::new(0, 0, 800, 600);

    // Point inside
    assert!(r.contains((400, 300)));
    assert!(r.contains((0, 0))); // Top-left corner (inclusive)
    assert!(r.contains((799, 599))); // Bottom-right edge (inside)

    // Point outside
    assert!(!r.contains((-1, 300))); // Left of rect
    assert!(!r.contains((800, 300))); // Right of rect
    assert!(!r.contains((400, -1))); // Above rect
    assert!(!r.contains((400, 600))); // Below rect
    assert!(!r.contains((800, 600))); // Diagonal outside
}

#[test]
fn test_rect_split_horizontal() {
    let r = Rect::new(0, 0, 1000, 600);
    let (left, right) = r.split_horizontal(0.5);

    assert_eq!(left.x, 0);
    assert_eq!(left.y, 0);
    assert_eq!(left.width, 500);
    assert_eq!(left.height, 600);

    assert_eq!(right.x, 500);
    assert_eq!(right.y, 0);
    assert_eq!(right.width, 500);
    assert_eq!(right.height, 600);
}

#[test]
fn test_rect_split_vertical() {
    let r = Rect::new(0, 0, 1000, 600);
    let (top, bottom) = r.split_vertical(0.5);

    assert_eq!(top.x, 0);
    assert_eq!(top.y, 0);
    assert_eq!(top.width, 1000);
    assert_eq!(top.height, 300);

    assert_eq!(bottom.x, 0);
    assert_eq!(bottom.y, 300);
    assert_eq!(bottom.width, 1000);
    assert_eq!(bottom.height, 300);
}

#[test]
fn test_rect_inset() {
    let r = Rect::new(0, 0, 1000, 600);
    let inset = r.inset(10);

    assert_eq!(inset.x, 10);
    assert_eq!(inset.y, 10);
    assert_eq!(inset.width, 980);
    assert_eq!(inset.height, 580);
}

#[test]
fn test_rect_intersects() {
    let r1 = Rect::new(0, 0, 100, 100);

    // Overlapping
    let r2 = Rect::new(50, 50, 100, 100);
    assert!(r1.intersects(&r2));
    assert!(r2.intersects(&r1));

    // Touching edge (not intersecting)
    let r3 = Rect::new(100, 0, 100, 100);
    assert!(!r1.intersects(&r3));

    // Separate
    let r4 = Rect::new(200, 200, 100, 100);
    assert!(!r1.intersects(&r4));

    // Contained within
    let r5 = Rect::new(25, 25, 50, 50);
    assert!(r1.intersects(&r5));
}

#[test]
fn test_rect_area() {
    let r1 = Rect::new(0, 0, 800, 600);
    assert_eq!(r1.area(), 480000);

    let r2 = Rect::new(0, 0, 0, 100);
    assert_eq!(r2.area(), 0);

    let r3 = Rect::new(0, 0, 1920, 1080);
    assert_eq!(r3.area(), 2073600);
}

#[test]
fn test_rect_center() {
    let r = Rect::new(0, 0, 800, 600);
    let (cx, cy) = r.center();
    assert_eq!(cx, 400);
    assert_eq!(cy, 300);

    let r2 = Rect::new(100, 50, 200, 100);
    let (cx2, cy2) = r2.center();
    assert_eq!(cx2, 200);
    assert_eq!(cy2, 100);
}

// ============================================================================
// 2. Layout Calculations (10 tests)
// ============================================================================

#[test]
fn test_dwindle_single_window() {
    use hyprtile::platform::window::WindowId;

    let workspace = Rect::new(0, 0, 1920, 1080);
    let windows = vec![WindowId(1001)];

    let result = DwindleLayout::calculate(&windows, &workspace, 8, 8, true);

    // With smart gaps and single window, should get full workspace
    assert_eq!(result.len(), 1);
    assert_eq!(result[0].0, WindowId(1001));

    // Single window with smart gaps should get the full rect
    assert_eq!(result[0].1.x, 0);
    assert_eq!(result[0].1.y, 0);
    assert_eq!(result[0].1.width, 1920);
    assert_eq!(result[0].1.height, 1080);
}

#[test]
fn test_dwindle_two_windows() {
    use hyprtile::platform::window::WindowId;

    let workspace = Rect::new(0, 0, 1920, 1080);
    let windows = vec![WindowId(1001), WindowId(1002)];

    let result = DwindleLayout::calculate(&windows, &workspace, 0, 0, false);

    assert_eq!(result.len(), 2);

    // Two windows: split horizontally (left/right)
    // First window gets left half, second gets right half
    let ids: Vec<_> = result.iter().map(|(id, _)| *id).collect();
    assert!(ids.contains(&WindowId(1001)));
    assert!(ids.contains(&WindowId(1002)));
}

#[test]
fn test_dwindle_five_windows() {
    use hyprtile::platform::window::WindowId;

    let workspace = Rect::new(0, 0, 1920, 1080);
    let windows = vec![
        WindowId(1001),
        WindowId(1002),
        WindowId(1003),
        WindowId(1004),
        WindowId(1005),
    ];

    let result = DwindleLayout::calculate(&windows, &workspace, 8, 8, false);

    assert_eq!(result.len(), 5);

    // Each window should get a unique position
    let ids: Vec<_> = result.iter().map(|(id, _)| *id).collect();
    for w in &windows {
        assert!(ids.contains(w));
    }
}

#[test]
fn test_master_stack_default() {
    use hyprtile::platform::window::WindowId;

    let workspace = Rect::new(0, 0, 1920, 1080);
    let windows = vec![WindowId(1001), WindowId(1002), WindowId(1003)];
    let config = MasterStackConfig::default();

    let result = MasterStackLayout::calculate(&windows, &workspace, 0, 0, false, &config);

    assert_eq!(result.len(), 3);

    // Master gets left half (0.5 factor), stack gets right half
    let master_rect = result[0].1;
    assert_eq!(master_rect.x, 0);
    assert_eq!(master_rect.y, 0);
    // Master width should be approximately 50% of 1920
    assert_eq!(master_rect.width, 960);
    assert_eq!(master_rect.height, 1080);
}

#[test]
fn test_master_stack_two_masters() {
    use hyprtile::platform::window::WindowId;

    let workspace = Rect::new(0, 0, 1920, 1080);
    let windows = vec![
        WindowId(1001),
        WindowId(1002),
        WindowId(1003),
        WindowId(1004),
    ];

    let config = MasterStackConfig {
        master_count: 2,
        master_width_factor: 0.6,
        orientation: Orientation::Horizontal,
    };

    let result = MasterStackLayout::calculate(&windows, &workspace, 0, 0, false, &config);

    assert_eq!(result.len(), 4);

    // With 2 masters at 0.6 factor, master area is 1152px wide
    // Each master gets half the height
    let master1 = result[0].1;
    assert_eq!(master1.x, 0);
    assert_eq!(master1.width, 1152); // 1920 * 0.6
}

#[test]
fn test_monocle_layout() {
    use hyprtile::platform::window::WindowId;

    let workspace = Rect::new(0, 0, 1920, 1080);
    let windows = vec![WindowId(1001), WindowId(1002), WindowId(1003)];

    // Focus on first window (index 0)
    let result = MonocleLayout::calculate(&windows, &workspace, 0, 0, true, 0);

    assert_eq!(result.len(), 3);

    // All windows should get the full workspace rect (stacked)
    for (_, rect) in &result {
        assert_eq!(rect.x, 0);
        assert_eq!(rect.y, 0);
        assert_eq!(rect.width, 1920);
        assert_eq!(rect.height, 1080);
    }
}

#[test]
fn test_grid_four_windows() {
    use hyprtile::platform::window::WindowId;

    let workspace = Rect::new(0, 0, 1920, 1080);
    let windows = vec![
        WindowId(1001),
        WindowId(1002),
        WindowId(1003),
        WindowId(1004),
    ];

    let result = GridLayout::calculate(&windows, &workspace, 0, 0, false);

    assert_eq!(result.len(), 4);

    // 4 windows = 2x2 grid
    // Each window gets half width and half height
    let first_rect = result[0].1;
    assert_eq!(first_rect.width, 960);
    assert_eq!(first_rect.height, 540);
}

#[test]
fn test_grid_nine_windows() {
    use hyprtile::platform::window::WindowId;

    let workspace = Rect::new(0, 0, 1920, 1080);
    let windows = vec![
        WindowId(1001),
        WindowId(1002),
        WindowId(1003),
        WindowId(1004),
        WindowId(1005),
        WindowId(1006),
        WindowId(1007),
        WindowId(1008),
        WindowId(1009),
    ];

    let result = GridLayout::calculate(&windows, &workspace, 0, 0, false);

    assert_eq!(result.len(), 9);

    // 9 windows = 3x3 grid
    let first_rect = result[0].1;
    assert_eq!(first_rect.width, 640); // 1920 / 3
    assert_eq!(first_rect.height, 360); // 1080 / 3
}

#[test]
fn test_gaps_application() {
    let r = Rect::new(0, 0, 1000, 600);

    // Apply outer gaps of 10px
    let with_gaps = apply_gaps(&r, 10);
    assert_eq!(with_gaps.x, 10);
    assert_eq!(with_gaps.y, 10);
    assert_eq!(with_gaps.width, 980); // 1000 - 20
    assert_eq!(with_gaps.height, 580); // 600 - 20
}

#[test]
fn test_smart_gaps_single() {
    // Smart gaps with 1 window: gaps should be 0
    let (inner, outer) = effective_gaps(1, 8, 8, true);
    assert_eq!(inner, 0);
    assert_eq!(outer, 0);

    // Smart gaps with 2+ windows: gaps should be preserved
    let (inner, outer) = effective_gaps(3, 8, 8, true);
    assert_eq!(inner, 8);
    assert_eq!(outer, 8);

    // Smart gaps disabled: gaps always apply
    let (inner, outer) = effective_gaps(1, 8, 8, false);
    assert_eq!(inner, 8);
    assert_eq!(outer, 8);
}

// ============================================================================
// 3. BSP Tree Tests (6 tests)
// ============================================================================

#[test]
fn test_bsp_insert_empty() {
    let mut node = Node::new();
    assert!(node.is_empty());

    use hyprtile::platform::window::WindowId;
    node.insert_window(WindowId(1001), SplitDirection::Horizontal);
    assert!(!node.is_empty());
}

#[test]
fn test_bsp_insert_multiple() {
    use hyprtile::platform::window::WindowId;

    let mut node = Node::new();
    node.insert_window(WindowId(1001), SplitDirection::Horizontal);
    node.insert_window(WindowId(1002), SplitDirection::Vertical);
    node.insert_window(WindowId(1003), SplitDirection::Horizontal);

    assert_eq!(node.window_count(), 3);
    assert!(node.contains_window(WindowId(1001)));
    assert!(node.contains_window(WindowId(1002)));
    assert!(node.contains_window(WindowId(1003)));
}

#[test]
fn test_bsp_remove() {
    use hyprtile::platform::window::WindowId;

    let mut node = Node::new();
    node.insert_window(WindowId(1001), SplitDirection::Horizontal);
    node.insert_window(WindowId(1002), SplitDirection::Vertical);
    node.insert_window(WindowId(1003), SplitDirection::Horizontal);

    assert_eq!(node.window_count(), 3);

    // Remove middle window
    let removed = node.remove_window(WindowId(1002));
    assert!(removed);
    assert_eq!(node.window_count(), 2);
    assert!(!node.contains_window(WindowId(1002)));
    assert!(node.contains_window(WindowId(1001)));
    assert!(node.contains_window(WindowId(1003)));
}

#[test]
fn test_bsp_traverse() {
    use hyprtile::platform::window::WindowId;

    let mut node = Node::new();
    node.insert_window(WindowId(1001), SplitDirection::Horizontal);
    node.insert_window(WindowId(1002), SplitDirection::Vertical);

    let workspace = Rect::new(0, 0, 1000, 600);
    let mut visited = Vec::new();
    node.traverse(&workspace, &mut |wid, rect| {
        visited.push((wid, rect));
    });

    assert_eq!(visited.len(), 2);
    assert!(visited.iter().any(|(wid, _)| *wid == WindowId(1001)));
    assert!(visited.iter().any(|(wid, _)| *wid == WindowId(1002)));
}

#[test]
fn test_bsp_rebalance() {
    use hyprtile::platform::window::WindowId;

    let mut node = Node::new();
    node.insert_window(WindowId(1001), SplitDirection::Horizontal);
    node.insert_window(WindowId(1002), SplitDirection::Vertical);
    node.insert_window(WindowId(1003), SplitDirection::Horizontal);

    // After rebalancing, all leaf splits should have equal ratios
    node.rebalance_ratios();

    // Tree should still contain all windows
    assert_eq!(node.window_count(), 3);
    assert!(node.contains_window(WindowId(1001)));
    assert!(node.contains_window(WindowId(1002)));
    assert!(node.contains_window(WindowId(1003)));
}

#[test]
fn test_bsp_dwindle_build() {
    use hyprtile::platform::window::WindowId;

    let windows = vec![
        WindowId(1001),
        WindowId(1002),
        WindowId(1003),
        WindowId(1004),
    ];

    let tree = build_dwindle_tree(&windows);

    assert_eq!(tree.window_count(), 4);
    for w in &windows {
        assert!(tree.contains_window(*w));
    }
}

// ============================================================================
// 4. Window State Machine Tests (6 tests)
// ============================================================================

#[test]
fn test_state_tiling_default() {
    use hyprtile::platform::window::WindowId;

    let window = Window::new(WindowId(1001));

    assert_eq!(window.state, WindowState::Tiling);
    assert!(window.previous_state.is_none());
    assert!(window.should_tile());
}

#[test]
fn test_state_toggle_float() {
    use hyprtile::platform::window::WindowId;

    let mut window = Window::new(WindowId(1001));
    assert_eq!(window.state, WindowState::Tiling);

    // Toggle from tiling to floating
    let new_state = window.toggle_float();
    assert_eq!(new_state, WindowState::Floating);
    assert_eq!(window.state, WindowState::Floating);
    assert_eq!(window.previous_state, Some(WindowState::Tiling));

    // Toggle back to tiling
    let new_state = window.toggle_float();
    assert_eq!(new_state, WindowState::Tiling);
    assert_eq!(window.state, WindowState::Tiling);
}

#[test]
fn test_state_toggle_fullscreen() {
    use hyprtile::platform::window::WindowId;

    let mut window = Window::new(WindowId(1001));
    assert_eq!(window.state, WindowState::Tiling);

    // Toggle to fullscreen
    let new_state = window.toggle_fullscreen();
    assert_eq!(new_state, WindowState::Fullscreen);
    assert_eq!(window.state, WindowState::Fullscreen);
    assert_eq!(window.previous_state, Some(WindowState::Tiling));

    // Toggle back
    let new_state = window.toggle_fullscreen();
    assert_eq!(new_state, WindowState::Tiling);
    assert_eq!(window.state, WindowState::Tiling);
}

#[test]
fn test_state_minimize_restore() {
    use hyprtile::platform::window::WindowId;

    let mut window = Window::new(WindowId(1001));

    // Minimize from tiling
    window.minimize();
    assert_eq!(window.state, WindowState::Minimized);
    assert_eq!(window.previous_state, Some(WindowState::Tiling));

    // Restore returns to tiling
    window.restore();
    assert_eq!(window.state, WindowState::Tiling);
}

#[test]
fn test_state_transition_chain() {
    use hyprtile::platform::window::WindowId;

    let mut window = Window::new(WindowId(1001));

    // TILING -> FLOATING
    window.toggle_float();
    assert_eq!(window.state, WindowState::Floating);

    // FLOATING -> FULLSCREEN
    window.toggle_fullscreen();
    assert_eq!(window.state, WindowState::Fullscreen);
    assert_eq!(window.previous_state, Some(WindowState::Floating));

    // FULLSCREEN -> FLOATING (restore)
    window.toggle_fullscreen();
    assert_eq!(window.state, WindowState::Floating);

    // FLOATING -> MINIMIZED
    window.minimize();
    assert_eq!(window.state, WindowState::Minimized);

    // MINIMIZED -> FLOATING (restore)
    window.restore();
    assert_eq!(window.state, WindowState::Floating);

    // FLOATING -> TILING
    window.toggle_float();
    assert_eq!(window.state, WindowState::Tiling);
}

#[test]
fn test_should_tile() {
    use hyprtile::platform::window::WindowId;

    let mut window = Window::new(WindowId(1001));
    assert!(window.should_tile());

    // Floating windows should not tile
    window.set_state(WindowState::Floating);
    assert!(!window.should_tile());

    // Fullscreen windows should not tile
    window.set_state(WindowState::Fullscreen);
    assert!(!window.should_tile());

    // Minimized windows should not tile
    window.set_state(WindowState::Minimized);
    assert!(!window.should_tile());

    // Maximized windows should not tile
    window.set_state(WindowState::Maximized);
    assert!(!window.should_tile());
}

// ============================================================================
// 5. Config Parsing Tests (4 tests)
// ============================================================================

#[test]
fn test_parse_default_config() {
    let config = default_config();

    // Verify general settings
    assert_eq!(config.general.terminal, "wezterm.exe");
    assert_eq!(config.general.resize_on_border, true);
    assert_eq!(config.general.auto_start, false);
    assert_eq!(config.general.focus_follows_mouse, false);
    assert_eq!(config.general.resize_border_width, 8);

    // Verify gaps
    assert_eq!(config.gaps.inner, 8);
    assert_eq!(config.gaps.outer, 8);
    assert_eq!(config.gaps.smart, true);

    // Verify workspaces
    assert_eq!(config.workspaces.count, 10);
    assert_eq!(config.workspaces.per_monitor, true);

    // Verify default keybinds exist
    assert!(config.keybinds.contains_key("mod+RETURN"));
    assert!(config.keybinds.contains_key("mod+Q"));
    assert!(config.keybinds.contains_key("mod+T"));
    assert!(config.keybinds.contains_key("mod+F"));
    assert!(config.keybinds.contains_key("mod+M"));
    assert!(config.keybinds.contains_key("mod+SHIFT+E"));
    assert_eq!(config.keybinds["mod+RETURN"], "exec_terminal");
    assert_eq!(config.keybinds["mod+Q"], "close_window");
    assert_eq!(config.keybinds["mod+T"], "toggle_float");

    // Verify window rules exist (default)
    assert!(!config.window_rules.is_empty());
}

#[test]
fn test_parse_custom_config() {
    let toml_str = r#"
[general]
mod_key = "WIN"
terminal = "alacritty.exe"

[gaps]
inner = 15
outer = 12
smart = false

[workspaces]
count = 5

[[window_rules]]
match_class = "myapp"
action = "float"
"#;

    let config: Config = toml::from_str(toml_str).unwrap();

    assert_eq!(config.general.mod_key, ModKey::Win);
    assert_eq!(config.general.terminal, "alacritty.exe");
    assert_eq!(config.gaps.inner, 15);
    assert_eq!(config.gaps.outer, 12);
    assert_eq!(config.gaps.smart, false);
    assert_eq!(config.workspaces.count, 5);
    assert_eq!(config.window_rules.len(), 1);
    assert_eq!(
        config.window_rules[0].match_class,
        Some("myapp".to_string())
    );
}

#[test]
fn test_config_validation() {
    let config = default_config();
    let errors = hyprtile::config::ConfigManager::validate(&config);

    // Default config should have no validation errors
    assert!(errors.is_empty());
}

#[test]
fn test_config_serialization_roundtrip() {
    let original = default_config();

    // Serialize to TOML
    let toml_string = toml::to_string_pretty(&original).unwrap();
    assert!(!toml_string.is_empty());
    assert!(toml_string.contains("terminal"));
    assert!(toml_string.contains("gaps"));

    // Deserialize back
    let parsed: Config = toml::from_str(&toml_string).unwrap();

    // Verify key fields round-trip correctly
    assert_eq!(parsed.general.terminal, original.general.terminal);
    assert_eq!(parsed.gaps.inner, original.gaps.inner);
    assert_eq!(parsed.gaps.outer, original.gaps.outer);
    assert_eq!(parsed.gaps.smart, original.gaps.smart);
    assert_eq!(parsed.workspaces.count, original.workspaces.count);
    assert_eq!(
        parsed.workspaces.per_monitor,
        original.workspaces.per_monitor
    );
}

// ============================================================================
// 6. IPC Protocol Tests (5 tests)
// ============================================================================

#[test]
fn test_ipc_request_serialize() {
    let req = IpcRequest::Workspaces { monitor: Some(0) };
    let json = serde_json::to_string(&req).unwrap();
    assert!(json.contains("workspaces"));
    assert!(json.contains("0"));

    let req2 = IpcRequest::FocusedWindow;
    let json2 = serde_json::to_string(&req2).unwrap();
    assert!(json2.contains("focused_window"));

    let req3 = IpcRequest::SwitchWorkspace { id: 3 };
    let json3 = serde_json::to_string(&req3).unwrap();
    assert!(json3.contains("switch_workspace"));
    assert!(json3.contains("3"));
}

#[test]
fn test_ipc_response_serialize() {
    // Success response
    let resp = IpcResponse::success(Some(serde_json::json!({"count": 5 })));
    let json = serde_json::to_string(&resp).unwrap();
    assert!(json.contains("true"));
    assert!(json.contains("count"));
    assert!(json.contains("5"));

    // Error response
    let resp_err = IpcResponse::error("Invalid workspace".to_string());
    let json_err = serde_json::to_string(&resp_err).unwrap();
    assert!(json_err.contains("false"));
    assert!(json_err.contains("Invalid workspace"));
}

#[test]
fn test_ipc_workspaces_command() {
    let req = IpcRequest::Workspaces { monitor: Some(0) };

    match &req {
        IpcRequest::Workspaces { monitor } => {
            assert_eq!(*monitor, Some(0));
        }
        _ => panic!("Expected Workspaces command"),
    }

    // Serialize and deserialize
    let json = serde_json::to_string(&req).unwrap();
    let deserialized: IpcRequest = serde_json::from_str(&json).unwrap();
    match deserialized {
        IpcRequest::Workspaces { monitor } => {
            assert_eq!(monitor, Some(0));
        }
        _ => panic!("Round-trip failed"),
    }
}

#[test]
fn test_ipc_focused_window_command() {
    let req = IpcRequest::FocusedWindow;

    match &req {
        IpcRequest::FocusedWindow => {
            // Correct variant
        }
        _ => panic!("Expected FocusedWindow command"),
    }

    let json = serde_json::to_string(&req).unwrap();
    assert_eq!(json, r#"{"command":"focused_window"}"#);
}

#[test]
fn test_ipc_switch_workspace_command() {
    let req = IpcRequest::SwitchWorkspace { id: 5 };

    match &req {
        IpcRequest::SwitchWorkspace { id } => {
            assert_eq!(*id, 5);
        }
        _ => panic!("Expected SwitchWorkspace command"),
    }

    let json = serde_json::to_string(&req).unwrap();
    let deserialized: IpcRequest = serde_json::from_str(&json).unwrap();
    match deserialized {
        IpcRequest::SwitchWorkspace { id } => {
            assert_eq!(id, 5);
        }
        _ => panic!("Round-trip failed"),
    }
}

// ============================================================================
// 7. Workspace Tests (6 tests)
// ============================================================================

#[test]
fn test_workspace_new() {
    let ws = Workspace::new(1);

    assert_eq!(ws.id, 1);
    assert!(ws.is_empty());
    assert_eq!(ws.windows.len(), 0);
    assert!(ws.focused_window.is_none());
}

#[test]
fn test_workspace_add_window() {
    use hyprtile::platform::window::WindowId;

    let mut ws = Workspace::new(1);
    let w1 = WindowId(1001);
    let w2 = WindowId(1002);

    // Add first window
    let added1 = ws.add_window(w1);
    assert!(added1);
    assert_eq!(ws.windows.len(), 1);
    assert!(ws.contains(w1));

    // Add second window
    let added2 = ws.add_window(w2);
    assert!(added2);
    assert_eq!(ws.windows.len(), 2);
    assert!(ws.contains(w2));

    // Adding duplicate should return false
    let added_dup = ws.add_window(w1);
    assert!(!added_dup);
    assert_eq!(ws.windows.len(), 2);
}

#[test]
fn test_workspace_remove_window() {
    use hyprtile::platform::window::WindowId;

    let mut ws = Workspace::new(1);
    let w1 = WindowId(1001);
    let w2 = WindowId(1002);
    let w3 = WindowId(1003);

    ws.add_window(w1);
    ws.add_window(w2);
    ws.add_window(w3);
    assert_eq!(ws.windows.len(), 3);

    // Remove middle window
    let removed = ws.remove_window(w2);
    assert!(removed);
    assert_eq!(ws.windows.len(), 2);
    assert!(!ws.contains(w2));
    assert!(ws.contains(w1));
    assert!(ws.contains(w3));

    // Remove nonexistent window
    let removed_fake = ws.remove_window(WindowId(9999));
    assert!(!removed_fake);
}

#[test]
fn test_workspace_switch() {
    let mut mw = MonitorWorkspace::new(0);

    // Initially should have workspace 1 active
    assert_eq!(mw.active_workspace, 1);

    // Switch to workspace 3
    let switched = mw.switch_workspace(3);
    assert!(switched);
    assert_eq!(mw.active_workspace, 3);

    // Get active workspace
    let ws = mw.get_active_workspace();
    assert_eq!(ws.id, 3);

    // Switch to non-existent workspace should create it
    let switched = mw.switch_workspace(7);
    assert!(switched);
    assert_eq!(mw.active_workspace, 7);
}

#[test]
fn test_workspace_focus() {
    use hyprtile::platform::window::WindowId;

    let mut ws = Workspace::new(1);
    let w1 = WindowId(1001);
    let w2 = WindowId(1002);

    ws.add_window(w1);
    ws.add_window(w2);

    // Focus first window
    let focused = ws.focus_window(w1);
    assert!(focused);
    assert_eq!(ws.focused_window, Some(w1));

    // Focus second window
    let focused = ws.focus_window(w2);
    assert!(focused);
    assert_eq!(ws.focused_window, Some(w2));

    // Try to focus nonexistent window
    let focused = ws.focus_window(WindowId(9999));
    assert!(!focused);
}

#[test]
fn test_workspace_cycle_focus() {
    use hyprtile::platform::window::WindowId;

    let mut ws = Workspace::new(1);
    let w1 = WindowId(1001);
    let w2 = WindowId(1002);
    let w3 = WindowId(1003);

    ws.add_window(w1);
    ws.add_window(w2);
    ws.add_window(w3);
    ws.focus_window(w1);

    assert_eq!(ws.focused_window, Some(w1));

    // Cycle forward
    ws.cycle_focus(FocusDirection::Next);
    assert_eq!(ws.focused_window, Some(w2));

    ws.cycle_focus(FocusDirection::Next);
    assert_eq!(ws.focused_window, Some(w3));

    // Cycle backward
    ws.cycle_focus(FocusDirection::Previous);
    assert_eq!(ws.focused_window, Some(w2));

    ws.cycle_focus(FocusDirection::Previous);
    assert_eq!(ws.focused_window, Some(w1));
}

// ============================================================================
// 8. Animation Tests (6 tests)
// ============================================================================

#[test]
fn test_easing_linear() {
    // Linear easing: f(t) = t
    let ease = Easing::Linear;

    assert!((ease.apply(0.0) - 0.0).abs() < 0.001);
    assert!((ease.apply(0.25) - 0.25).abs() < 0.001);
    assert!((ease.apply(0.5) - 0.5).abs() < 0.001);
    assert!((ease.apply(0.75) - 0.75).abs() < 0.001);
    assert!((ease.apply(1.0) - 1.0).abs() < 0.001);
}

#[test]
fn test_easing_ease_out_cubic() {
    let ease = Easing::EaseOutCubic;

    // At t=0, result should be 0
    assert!((ease.apply(0.0) - 0.0).abs() < 0.001);

    // At t=1, result should be 1
    assert!((ease.apply(1.0) - 1.0).abs() < 0.001);

    // At t=0.5, cubic ease-out should be > 0.5 (fast start, slow end)
    let val = ease.apply(0.5);
    assert!(val > 0.5);
    assert!(val < 1.0);
}

#[test]
fn test_easing_ease_out_expo() {
    let ease = Easing::EaseOutExpo;

    // At t=0, result should be 0
    assert!((ease.apply(0.0) - 0.0).abs() < 0.001);

    // At t=1, result should be 1
    assert!((ease.apply(1.0) - 1.0).abs() < 0.001);

    // At t=0.5, expo ease-out should be significantly > 0.5
    let val = ease.apply(0.5);
    assert!(val > 0.5);
}

#[test]
fn test_animation_progress() {
    let mut anim = Animation::new(1000, Easing::Linear);

    // At start, progress should be 0
    let p0 = anim.tick(0);
    assert_eq!(p0, 0.0);

    // After half duration
    let p1 = anim.tick(500);
    assert!(p1 > 0.0);
    assert!(p1 <= 0.5);

    // Complete the animation
    let p2 = anim.tick(500);
    assert_eq!(p2, 1.0);
    assert!(anim.is_complete());
}

#[test]
fn test_animation_complete() {
    let mut anim = Animation::new(100, Easing::Linear);

    assert!(!anim.is_complete());

    // Tick past the duration
    anim.tick(150);
    assert!(anim.is_complete());

    // Reset and check again
    anim.reset();
    assert!(!anim.is_complete());
    assert_eq!(anim.elapsed_ms, 0);
}

#[test]
fn test_interpolate_rect() {
    let from = Rect::new(0, 0, 800, 600);
    let to = Rect::new(100, 50, 400, 300);

    // At progress 0, should equal 'from'
    let r0 = interpolate_rect(&from, &to, 0.0);
    assert_eq!(r0.x, from.x);
    assert_eq!(r0.y, from.y);

    // At progress 1, should equal 'to'
    let r1 = interpolate_rect(&from, &to, 1.0);
    assert_eq!(r1.x, to.x);
    assert_eq!(r1.y, to.y);
    assert_eq!(r1.width, to.width);
    assert_eq!(r1.height, to.height);

    // At progress 0.5, should be halfway
    let r_half = interpolate_rect(&from, &to, 0.5);
    assert_eq!(r_half.x, 50); // (0 + 100) / 2
    assert_eq!(r_half.y, 25); // (0 + 50) / 2
    assert_eq!(r_half.width, 600); // (800 + 400) / 2
    assert_eq!(r_half.height, 450); // (600 + 300) / 2
}

// ============================================================================
// 9. Window Rules Tests (4 tests)
// ============================================================================

#[test]
fn test_rule_class_match() {
    use hyprtile::platform::window::WindowId;

    let mut window = Window::new(WindowId(1001));
    window.class_name = "Steam".to_string();

    let rule = WindowRule {
        match_class: Some(".*[Ss]team.*".to_string()),
        match_title: None,
        match_process: None,
        action: Some(WindowAction::Float),
        workspace: None,
        monitor: None,
        size: None,
        position: None,
    };

    assert!(window.matches_rule(&rule));
    assert!(class_matches("Steam", ".*[Ss]team.*"));
}

#[test]
fn test_rule_title_match() {
    use hyprtile::platform::window::WindowId;

    let mut window = Window::new(WindowId(1001));
    window.title = "Picture-in-Picture - YouTube".to_string();

    let rule = WindowRule {
        match_class: None,
        match_title: Some("Picture-in-Picture.*".to_string()),
        match_process: None,
        action: Some(WindowAction::Float),
        workspace: None,
        monitor: None,
        size: Some([400, 225]),
        position: Some("bottom_right".to_string()),
    };

    assert!(window.matches_rule(&rule));
    assert!(title_matches(
        "Picture-in-Picture - YouTube",
        "Picture-in-Picture.*"
    ));
}

#[test]
fn test_rule_process_match() {
    use hyprtile::platform::window::WindowId;

    let mut window = Window::new(WindowId(1001));
    window.process_name = "firefox.exe".to_string();

    let rule = WindowRule {
        match_class: None,
        match_title: None,
        match_process: Some("firefox\\.exe".to_string()),
        action: Some(WindowAction::Tile),
        workspace: Some(2),
        monitor: None,
        size: None,
        position: None,
    };

    assert!(window.matches_rule(&rule));
    assert!(process_matches("firefox.exe", "firefox\\.exe"));
}

#[test]
fn test_rule_no_match() {
    use hyprtile::platform::window::WindowId;

    let mut window = Window::new(WindowId(1001));
    window.class_name = "Notepad".to_string();
    window.title = "Untitled".to_string();
    window.process_name = "notepad.exe".to_string();

    let rule = WindowRule {
        match_class: Some(".*[Ss]team.*".to_string()),
        match_title: None,
        match_process: None,
        action: Some(WindowAction::Float),
        workspace: None,
        monitor: None,
        size: None,
        position: None,
    };

    // Window is Notepad, rule matches Steam - should not match
    assert!(!window.matches_rule(&rule));
    assert!(!class_matches("Notepad", ".*[Ss]team.*"));
}

// ============================================================================
// 10. Additional Edge Case Tests (5 bonus tests)
// ============================================================================

#[test]
fn test_rect_empty() {
    let r = Rect::new(0, 0, 0, 100);
    assert!(r.is_empty());

    let r2 = Rect::new(0, 0, 100, 0);
    assert!(r2.is_empty());

    let r3 = Rect::new(0, 0, 100, 100);
    assert!(!r3.is_empty());
}

#[test]
fn test_rect_negative_inset() {
    // Negative inset should expand the rect
    let r = Rect::new(10, 10, 100, 100);
    let expanded = r.inset(-5);
    assert_eq!(expanded.x, 5);
    assert_eq!(expanded.y, 5);
    assert_eq!(expanded.width, 110);
    assert_eq!(expanded.height, 110);
}

#[test]
fn test_bsp_window_count_empty() {
    let node = Node::new();
    assert_eq!(node.window_count(), 0);
}

#[test]
fn test_workspace_focus_index() {
    use hyprtile::platform::window::WindowId;

    let mut ws = Workspace::new(1);
    let w1 = WindowId(1001);
    let w2 = WindowId(1002);

    ws.add_window(w1);
    ws.add_window(w2);
    ws.focus_window(w1);

    assert_eq!(ws.get_focused_index(), 0);

    ws.focus_window(w2);
    assert_eq!(ws.get_focused_index(), 1);
}

#[test]
fn test_layout_type_all() {
    let all = LayoutType::all();
    assert_eq!(all.len(), 4);
    assert!(all.contains(&LayoutType::Dwindle));
    assert!(all.contains(&LayoutType::MasterStack));
    assert!(all.contains(&LayoutType::Monocle));
    assert!(all.contains(&LayoutType::Grid));
}

#[test]
fn test_layout_type_from_name() {
    assert_eq!(LayoutType::from_name("dwindle"), Some(LayoutType::Dwindle));
    assert_eq!(
        LayoutType::from_name("master_stack"),
        Some(LayoutType::MasterStack)
    );
    assert_eq!(LayoutType::from_name("monocle"), Some(LayoutType::Monocle));
    assert_eq!(LayoutType::from_name("grid"), Some(LayoutType::Grid));
    assert_eq!(LayoutType::from_name("nonexistent"), None);
}

#[test]
fn test_layout_type_next() {
    assert_eq!(LayoutType::Dwindle.next(), LayoutType::MasterStack);
    assert_eq!(LayoutType::MasterStack.next(), LayoutType::Monocle);
    assert_eq!(LayoutType::Monocle.next(), LayoutType::Grid);
    assert_eq!(LayoutType::Grid.next(), LayoutType::Dwindle);
}

#[test]
fn test_easing_clamped() {
    // Easing should handle edge values
    let ease = Easing::EaseOutCubic;
    assert!(ease.apply(0.0) >= 0.0);
    assert!(ease.apply(1.0) <= 1.0 || (ease.apply(1.0) - 1.0).abs() < 0.001);
}

#[test]
fn test_config_mod_key_variants() {
    let config_alt: Config = toml::from_str(
        r#"[general]
mod_key = "ALT"
"#,
    )
    .unwrap();
    assert_eq!(config_alt.general.mod_key, ModKey::Alt);

    let config_win: Config = toml::from_str(
        r#"[general]
mod_key = "WIN"
"#,
    )
    .unwrap();
    assert_eq!(config_win.general.mod_key, ModKey::Win);

    let config_ctrl: Config = toml::from_str(
        r#"[general]
mod_key = "CTRL"
"#,
    )
    .unwrap();
    assert_eq!(config_ctrl.general.mod_key, ModKey::Ctrl);
}

#[test]
fn test_workspace_get_index() {
    use hyprtile::platform::window::WindowId;

    let mut ws = Workspace::new(1);
    let w1 = WindowId(1001);
    let w2 = WindowId(1002);

    ws.add_window(w1);
    ws.add_window(w2);

    assert_eq!(ws.get_window_index(w1), Some(0));
    assert_eq!(ws.get_window_index(w2), Some(1));
    assert_eq!(ws.get_window_index(WindowId(9999)), None);
}

#[test]
fn test_dwindle_layout_name() {
    assert_eq!(DwindleLayout::name(), "dwindle");
}

#[test]
fn test_master_stack_layout_name() {
    assert_eq!(MasterStackLayout::name(), "master_stack");
}

#[test]
fn test_monocle_layout_name() {
    assert_eq!(MonocleLayout::name(), "monocle");
}

#[test]
fn test_grid_layout_name() {
    assert_eq!(GridLayout::name(), "grid");
}

#[test]
fn test_lerp_function() {
    // lerp(0, 100, 0.5) = 50
    assert!((lerp(0.0, 100.0, 0.5) - 50.0).abs() < 0.001);
    assert!((lerp(0.0, 100.0, 0.0) - 0.0).abs() < 0.001);
    assert!((lerp(0.0, 100.0, 1.0) - 100.0).abs() < 0.001);
    assert!((lerp(10.0, 20.0, 0.5) - 15.0).abs() < 0.001);
}

#[test]
fn test_gaps_effective_gaps_multi_window() {
    let (inner, outer) = effective_gaps(5, 10, 10, true);
    assert_eq!(inner, 10);
    assert_eq!(outer, 10);
}

#[test]
fn test_window_rule_engine() {
    use hyprtile::platform::window::WindowId;

    let rules = vec![WindowRule {
        match_class: Some("Steam".to_string()),
        match_title: None,
        match_process: None,
        action: Some(WindowAction::Float),
        workspace: None,
        monitor: None,
        size: None,
        position: None,
    }];

    let engine = RuleEngine::new(rules);

    let mut steam_window = Window::new(WindowId(1001));
    steam_window.class_name = "Steam".to_string();

    assert!(engine.should_float(&steam_window));
}

#[test]
fn test_workspace_contains() {
    use hyprtile::platform::window::WindowId;

    let mut ws = Workspace::new(1);
    let w1 = WindowId(1001);

    assert!(!ws.contains(w1));
    ws.add_window(w1);
    assert!(ws.contains(w1));
}

#[test]
fn test_monitor_workspace_get_workspace() {
    let mut mw = MonitorWorkspace::new(0);

    // Ensure workspace 5 exists
    mw.ensure_workspace(5);

    let ws = mw.get_workspace(5);
    assert!(ws.is_some());
    assert_eq!(ws.unwrap().id, 5);

    let ws_missing = mw.get_workspace(99);
    assert!(ws_missing.is_none());
}
