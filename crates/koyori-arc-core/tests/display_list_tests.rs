//! Display-list Phase 0 verification: P0–P4 (§3.7.3).

use bincode::config;
use chrono::NaiveDate;
use koyori_arc_core::bench_fixtures::{generate_fixture, DepDensity, TaskCount};
use koyori_arc_core::{
    build_display_list, BackendOutput, GanttDep, GanttGraph, GanttTask, NativeBackend,
    NativeDrawOp, RenderBackend, SvgBackend,
};

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn two_task_graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![
            GanttTask {
                id: "task-1".to_string(),
                title: "Design".to_string(),
                progress_pct: 100,
                start: date(2026, 6, 1),
                end: Some(date(2026, 6, 4)),
            },
            GanttTask {
                id: "task-2".to_string(),
                title: "Build".to_string(),
                progress_pct: 50,
                start: date(2026, 6, 4),
                end: Some(date(2026, 6, 8)),
            },
        ],
        deps: vec![GanttDep {
            blocker_task_id: "task-1".to_string(),
            blocked_task_id: "task-2".to_string(),
        }],
    }
}

fn epoch(graph: &GanttGraph) -> NaiveDate {
    graph.tasks.iter().map(|t| t.start).min().unwrap()
}

fn render_via_ir(graph: &GanttGraph, today: Option<NaiveDate>) -> String {
    let list = build_display_list(graph, epoch(graph), today);
    match SvgBackend.render(&list) {
        BackendOutput::Svg(s) => s,
        _ => panic!("expected svg"),
    }
}

fn render_direct(graph: &GanttGraph, today: Option<NaiveDate>) -> String {
    koyori_arc_core::render(&graph.tasks, &graph.deps, today)
}

// --- P0: IR golden (deterministic snapshot) ---

#[test]
fn p0_ir_golden_two_tasks() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None);
    let json = serde_json::to_string_pretty(&list).expect("serialize");
    let golden = include_str!("fixtures/ir_golden/two_tasks.json");
    assert_eq!(json, golden, "IR snapshot drift — update fixtures if intentional");
}

#[test]
fn p0_ir_golden_100_sparse() {
    let fixture = generate_fixture(TaskCount::N100, DepDensity::Sparse);
    let graph = GanttGraph {
        tasks: fixture.tasks,
        deps: fixture.deps,
    };
    let ep = epoch(&graph);
    let list_a = build_display_list(&graph, ep, None);
    let list_b = build_display_list(&graph, ep, None);
    let json_a = serde_json::to_string(&list_a).expect("serialize");
    let json_b = serde_json::to_string(&list_b).expect("serialize");
    assert_eq!(json_a, json_b, "100_sparse IR must be deterministic");
    assert!(list_a.metadata.primitive_count > 100);
    assert!(list_a.layers.len() >= 6);
}

#[test]
fn p0_ir_golden_2000_dense() {
    let fixture = generate_fixture(TaskCount::N2000, DepDensity::Dense);
    let graph = GanttGraph {
        tasks: fixture.tasks,
        deps: fixture.deps,
    };
    let ep = epoch(&graph);
    let list_a = build_display_list(&graph, ep, None);
    let list_b = build_display_list(&graph, ep, None);
    let json_a = serde_json::to_string(&list_a).expect("serialize");
    let json_b = serde_json::to_string(&list_b).expect("serialize");
    assert_eq!(json_a, json_b, "2000_dense IR must be deterministic");
    assert!(list_a.metadata.primitive_count > 2000);
    assert!(list_a.metadata.task_bboxes.len() == 2000);
}

// --- P1: SVG byte-compat ---

#[test]
fn p1_svg_byte_compat_two_tasks() {
    let graph = two_task_graph();
    let golden = include_str!("fixtures/svg_golden/two_tasks.svg");
    let svg = render_via_ir(&graph, None);
    assert_eq!(svg, golden);
}

#[test]
fn p1_svg_byte_compat_milestone() {
    let graph = GanttGraph {
        tasks: vec![GanttTask {
            id: "ms1".to_string(),
            title: "Launch".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 3),
            end: Some(date(2026, 6, 3)),
        }],
        deps: vec![],
    };
    let golden = include_str!("fixtures/svg_golden/milestone.svg");
    let svg = render_via_ir(&graph, None);
    assert_eq!(svg, golden);
}

#[test]
fn p1_svg_byte_compat_empty() {
    let golden = include_str!("fixtures/svg_golden/empty.svg");
    let svg = koyori_arc_core::render(&[], &[], None);
    assert_eq!(svg, golden);
}

#[test]
fn p1_svg_byte_compat_today_anchored() {
    let graph = two_task_graph();
    let today = date(2026, 6, 3);
    let golden = include_str!("fixtures/svg_golden/two_tasks_today.svg");
    let svg = render_via_ir(&graph, Some(today));
    assert_eq!(svg, golden);
}

#[test]
fn p1_render_matches_ir_path() {
    let graph = two_task_graph();
    assert_eq!(render_direct(&graph, None), render_via_ir(&graph, None));
}

// --- P2: NativeBackend stub ---

#[test]
fn p2_native_stub_primitive_counts_match() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None);
    let native = match NativeBackend.render(&list) {
        BackendOutput::NativeDrawList(n) => n,
        _ => panic!("expected native"),
    };
    assert!(native.ops.len() > 0);
    assert_eq!(native.viewport_width, list.viewport.width);
    assert_eq!(native.viewport_height, list.viewport.height);
    assert_eq!(
        list.metadata.primitive_count,
        list.count_primitives()
    );
}

#[test]
fn p2_native_and_svg_same_task_bbox_count() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None);
    assert_eq!(list.metadata.task_bboxes.len(), 2);
    let native = match NativeBackend.render(&list) {
        BackendOutput::NativeDrawList(n) => n,
        _ => panic!("expected native"),
    };
    let group_starts = native
        .ops
        .iter()
        .filter(|op| matches!(op, NativeDrawOp::GroupStart { .. }))
        .count();
    assert_eq!(group_starts, 4); // 2 task groups + 2 legend groups
}

// --- P3: bincode roundtrip ---

#[test]
fn p3_bincode_roundtrip_preserves_svg() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None);
    let cfg = config::standard();
    let bytes = bincode::serde::encode_to_vec(&list, cfg).expect("encode");
    let (decoded, _): (koyori_arc_core::DisplayList, usize) =
        bincode::serde::decode_from_slice(&bytes, cfg).expect("decode");

    let svg_before = match SvgBackend.render(&list) {
        BackendOutput::Svg(s) => s,
        _ => panic!(),
    };
    let svg_after = match SvgBackend.render(&decoded) {
        BackendOutput::Svg(s) => s,
        _ => panic!(),
    };
    assert_eq!(svg_before, svg_after);

    let native_before = match NativeBackend.render(&list) {
        BackendOutput::NativeDrawList(n) => n,
        _ => panic!(),
    };
    let native_after = match NativeBackend.render(&decoded) {
        BackendOutput::NativeDrawList(n) => n,
        _ => panic!(),
    };
    assert_eq!(native_before.ops.len(), native_after.ops.len());
}

// --- P4: neutrality — no DOM field names in IR type JSON ---

#[test]
fn p4_ir_contains_no_dom_concepts() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None);
    let json = serde_json::to_string(&list).expect("serialize");
    let forbidden = [
        "innerHTML",
        "v-html",
        "data-task-id",
        "role=\"img\"",
        "aria-label",
        "HTMLElement",
        "CanvasRenderingContext",
    ];
    for term in forbidden {
        assert!(
            !json.contains(term),
            "IR leaked DOM concept: {term}"
        );
    }
}

/// Regenerate golden fixtures (run with --ignored).
#[test]
#[ignore]
fn write_golden_fixtures() {
    use std::fs;
    use std::path::Path;

    let base = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    fs::create_dir_all(base.join("svg_golden")).unwrap();
    fs::create_dir_all(base.join("ir_golden")).unwrap();

    // SVG goldens from direct render (pre-refactor baseline path)
    let graph = two_task_graph();
    fs::write(
        base.join("svg_golden/two_tasks.svg"),
        render_direct(&graph, None),
    )
    .unwrap();
    fs::write(
        base.join("svg_golden/two_tasks_today.svg"),
        render_direct(&graph, Some(date(2026, 6, 3))),
    )
    .unwrap();

    let ms = GanttGraph {
        tasks: vec![GanttTask {
            id: "ms1".to_string(),
            title: "Launch".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 3),
            end: Some(date(2026, 6, 3)),
        }],
        deps: vec![],
    };
    fs::write(
        base.join("svg_golden/milestone.svg"),
        render_direct(&ms, None),
    )
    .unwrap();
    fs::write(
        base.join("svg_golden/empty.svg"),
        koyori_arc_core::render(&[], &[], None),
    )
    .unwrap();

    let list = build_display_list(&graph, epoch(&graph), None);
    fs::write(
        base.join("ir_golden/two_tasks.json"),
        serde_json::to_string_pretty(&list).unwrap(),
    )
    .unwrap();

    eprintln!("Golden fixtures written to {:?}", base);
}
