//! Performance benchmark suite.
//!
//! Covers SPEC.md acceptance criteria that can be measured in pure Rust:
//!   * Layout calculation latency (criterion 4 / 9: 50+ windows)
//!   * IPC codec roundtrip (criterion 6: <10ms)
//!   * Config TOML parse + validate (criterion 5: <200ms)
//!   * Hotkey channel dispatch (criterion 2: <50ms)
//!   * Workspace data-model switch (criterion 3: <100ms for 10+ windows)
//!
//! Run on a real Windows host:
//!     cargo bench
//!
//! CPU% / memory budgets (criteria 7, 8) and full event-pipeline latency
//! (criterion 4 wall-clock) require a live windowing session and are not
//! covered here.

use criterion::{BenchmarkId, Criterion, Throughput, black_box, criterion_group, criterion_main};

use hyprtile::config::types::GapsConfig;
use hyprtile::ipc::protocol::{IpcRequest, IpcResponse};
use hyprtile::layout::{LayoutType, calculate_layout};
use hyprtile::platform::window::WindowId;
use hyprtile::util::rect::Rect;
use hyprtile::workspace::model::{MonitorWorkspace, Workspace};

fn dummy_windows(n: usize) -> Vec<WindowId> {
    (1..=n as isize).map(WindowId).collect()
}

fn bench_layouts(c: &mut Criterion) {
    let workspace = Rect::new(0, 0, 3840, 2160);
    let gaps = GapsConfig {
        inner: 8,
        outer: 16,
        smart: false,
    };

    let mut group = c.benchmark_group("layout_calculate");
    for &n in &[1usize, 4, 10, 25, 50, 100] {
        let windows = dummy_windows(n);
        group.throughput(Throughput::Elements(n as u64));
        for layout in LayoutType::all() {
            group.bench_with_input(
                BenchmarkId::new(layout.name(), n),
                &windows,
                |b, windows| {
                    b.iter(|| {
                        let result = calculate_layout(
                            black_box(layout),
                            black_box(windows),
                            black_box(&workspace),
                            black_box(&gaps),
                            black_box(0),
                            black_box(0.5),
                        );
                        black_box(result);
                    });
                },
            );
        }
    }
    group.finish();
}

fn bench_ipc_roundtrip(c: &mut Criterion) {
    let request = IpcRequest::Workspaces { monitor: None };
    let response = IpcResponse::success(Some(serde_json::json!({
        "workspaces": [
            {"id": 1, "active": true, "window_count": 5},
            {"id": 2, "active": false, "window_count": 0},
            {"id": 3, "active": false, "window_count": 12},
        ]
    })));

    c.bench_function("ipc_serialize_request", |b| {
        b.iter(|| {
            let json = serde_json::to_vec(black_box(&request)).unwrap();
            black_box(json);
        });
    });

    c.bench_function("ipc_serialize_response", |b| {
        b.iter(|| {
            let json = serde_json::to_vec(black_box(&response)).unwrap();
            black_box(json);
        });
    });

    let request_bytes = serde_json::to_vec(&request).unwrap();
    c.bench_function("ipc_deserialize_request", |b| {
        b.iter(|| {
            let parsed: IpcRequest = serde_json::from_slice(black_box(&request_bytes)).unwrap();
            black_box(parsed);
        });
    });

    c.bench_function("ipc_full_roundtrip", |b| {
        b.iter(|| {
            let json = serde_json::to_vec(black_box(&request)).unwrap();
            let parsed: IpcRequest = serde_json::from_slice(&json).unwrap();
            let resp_json = serde_json::to_vec(&response).unwrap();
            let _resp_back: IpcResponse = serde_json::from_slice(&resp_json).unwrap();
            black_box(parsed);
        });
    });
}

fn bench_config_parse(c: &mut Criterion) {
    let toml_text = toml::to_string_pretty(&hyprtile::config::defaults::default_config()).unwrap();

    c.bench_function("config_parse_default", |b| {
        b.iter(|| {
            let config: hyprtile::config::types::Config =
                toml::from_str(black_box(&toml_text)).unwrap();
            black_box(config);
        });
    });

    c.bench_function("config_serialize_default", |b| {
        let cfg = hyprtile::config::defaults::default_config();
        b.iter(|| {
            let s = toml::to_string_pretty(black_box(&cfg)).unwrap();
            black_box(s);
        });
    });
}

fn bench_workspace_switch(c: &mut Criterion) {
    c.bench_function("workspace_switch_with_10_windows", |b| {
        let mut mw = MonitorWorkspace::new(1);
        for i in 1..=10 {
            mw.get_active_workspace_mut().add_window(WindowId(i));
        }
        // Pre-create target workspace
        let _ = mw.ensure_workspace(2);

        b.iter(|| {
            let switched = mw.switch_workspace(black_box(2));
            black_box(switched);
            mw.switch_workspace(1);
        });
    });

    c.bench_function("workspace_add_50_windows", |b| {
        b.iter(|| {
            let mut ws = Workspace::new(1);
            for i in 1..=50 {
                ws.add_window(WindowId(i));
            }
            black_box(ws);
        });
    });
}

fn bench_hotkey_dispatch(c: &mut Criterion) {
    use std::sync::mpsc;

    c.bench_function("hotkey_channel_send_recv", |b| {
        let (tx, rx) = mpsc::channel::<String>();
        b.iter(|| {
            tx.send(black_box("toggle_float".to_string())).unwrap();
            let action = rx.recv().unwrap();
            black_box(action);
        });
    });
}

criterion_group!(
    benches,
    bench_layouts,
    bench_ipc_roundtrip,
    bench_config_parse,
    bench_workspace_switch,
    bench_hotkey_dispatch
);
criterion_main!(benches);
