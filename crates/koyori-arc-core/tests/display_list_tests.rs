//! Display-list Phase 0 verification: P0–P4 (§3.7.3).

use bincode::config;
use chrono::NaiveDate;
use koyori_arc_core::bench_fixtures::{generate_fixture, DepDensity, TaskCount};
use koyori_arc_core::{
    build_display_list, compute_row_window, BackendOutput, CanvasBackend, CommandBuffer,
    DOM_CAP, DrawOp, GanttDep, GanttGraph, GanttTask, HEADER_H, NativeBackend, NativeDrawOp,
    RenderBackend, ROW_H, ScrollViewport, SvgBackend,
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
    let list = build_display_list(graph, epoch(graph), today, None);
    match SvgBackend.render(&list) {
        BackendOutput::Svg(s) => s,
        _ => panic!("expected svg"),
    }
}

fn render_direct(graph: &GanttGraph, today: Option<NaiveDate>) -> String {
    koyori_arc_core::render(&graph.tasks, &graph.deps, today, None)
}

// --- P0: IR golden (deterministic snapshot) ---

#[test]
fn p0_ir_golden_two_tasks() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None, None);
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
    let list_a = build_display_list(&graph, ep, None, None);
    let list_b = build_display_list(&graph, ep, None, None);
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
    let list_a = build_display_list(&graph, ep, None, None);
    let list_b = build_display_list(&graph, ep, None, None);
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
    let svg = koyori_arc_core::render(&[], &[], None, None);
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
    let list = build_display_list(&graph, epoch(&graph), None, None);
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
    let list = build_display_list(&graph, epoch(&graph), None, None);
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
    let list = build_display_list(&graph, epoch(&graph), None, None);
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
    let list = build_display_list(&graph, epoch(&graph), None, None);
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

// --- Topological row assignment integration (build.rs) ---

fn reversed_two_task_graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![
            GanttTask {
                id: "task-2".to_string(),
                title: "Build".to_string(),
                progress_pct: 50,
                start: date(2026, 6, 4),
                end: Some(date(2026, 6, 8)),
            },
            GanttTask {
                id: "task-1".to_string(),
                title: "Design".to_string(),
                progress_pct: 100,
                start: date(2026, 6, 1),
                end: Some(date(2026, 6, 4)),
            },
        ],
        deps: vec![GanttDep {
            blocker_task_id: "task-1".to_string(),
            blocked_task_id: "task-2".to_string(),
        }],
    }
}

fn cycle_downstream_graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![
            GanttTask {
                id: "d".to_string(),
                title: "Downstream".to_string(),
                progress_pct: 0,
                start: date(2026, 6, 10),
                end: Some(date(2026, 6, 12)),
            },
            GanttTask {
                id: "a".to_string(),
                title: "Cycle A".to_string(),
                progress_pct: 0,
                start: date(2026, 6, 1),
                end: Some(date(2026, 6, 3)),
            },
            GanttTask {
                id: "b".to_string(),
                title: "Cycle B".to_string(),
                progress_pct: 0,
                start: date(2026, 6, 4),
                end: Some(date(2026, 6, 6)),
            },
            GanttTask {
                id: "c".to_string(),
                title: "Cycle C".to_string(),
                progress_pct: 0,
                start: date(2026, 6, 7),
                end: Some(date(2026, 6, 9)),
            },
        ],
        deps: vec![
            GanttDep {
                blocker_task_id: "a".to_string(),
                blocked_task_id: "b".to_string(),
            },
            GanttDep {
                blocker_task_id: "b".to_string(),
                blocked_task_id: "c".to_string(),
            },
            GanttDep {
                blocker_task_id: "c".to_string(),
                blocked_task_id: "a".to_string(),
            },
            GanttDep {
                blocker_task_id: "b".to_string(),
                blocked_task_id: "d".to_string(),
            },
        ],
    }
}

fn bar_row_from_bbox(bbox_y: f64) -> u32 {
    ((bbox_y - HEADER_H - 4.0) / ROW_H).round() as u32
}

#[test]
fn topological_rows_bar_matches_dependency_endpoints_when_input_reversed() {
    let graph = reversed_two_task_graph();
    let ep = epoch(&graph);
    let list = build_display_list(&graph, ep, None, None);

    let row_by_task: std::collections::HashMap<&str, u32> = list
        .metadata
        .task_bboxes
        .iter()
        .map(|tb| (tb.task_id.as_str(), tb.row))
        .collect();

    assert!(
        row_by_task["task-1"] < row_by_task["task-2"],
        "blocker must be above blocked even when blocked is listed first"
    );

    for tb in &list.metadata.task_bboxes {
        assert_eq!(
            tb.row,
            bar_row_from_bbox(tb.bbox.y),
            "bar y must match assigned topo row for {}",
            tb.task_id
        );
    }

    let blocker_row = row_by_task["task-1"];
    let blocked_row = row_by_task["task-2"];
    assert!(blocker_row < blocked_row);
}

#[test]
fn topological_rows_cycle_downstream_blocker_above_blocked() {
    let graph = cycle_downstream_graph();
    let ep = epoch(&graph);
    let list = build_display_list(&graph, ep, None, None);

    let row_by_task: std::collections::HashMap<&str, u32> = list
        .metadata
        .task_bboxes
        .iter()
        .map(|tb| (tb.task_id.as_str(), tb.row))
        .collect();

    assert!(
        row_by_task["b"] < row_by_task["d"],
        "cycle member B must stay above downstream D"
    );
    assert!(row_by_task["a"] < row_by_task["d"]);
    assert!(row_by_task["c"] < row_by_task["d"]);

    for tb in &list.metadata.task_bboxes {
        assert_eq!(tb.row, bar_row_from_bbox(tb.bbox.y));
    }
}

// --- Phase 2: CanvasBackend + CommandBuffer ---

#[test]
fn p2_canvas_command_buffer_golden_two_tasks() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None, None);
    let buffer = match CanvasBackend.render(&list) {
        BackendOutput::CanvasCommands(b) => b,
        _ => panic!("expected canvas commands"),
    };
    let json = serde_json::to_string_pretty(&buffer).expect("serialize");
    let golden = include_str!("fixtures/canvas_golden/two_tasks.json");
    assert_eq!(
        json, golden,
        "CommandBuffer snapshot drift — update fixtures if intentional"
    );
}

#[test]
fn p2_canvas_matches_native_op_count_and_viewport() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None, None);
    let canvas = match CanvasBackend.render(&list) {
        BackendOutput::CanvasCommands(b) => b,
        _ => panic!("expected canvas"),
    };
    let native = match NativeBackend.render(&list) {
        BackendOutput::NativeDrawList(n) => n,
        _ => panic!("expected native"),
    };
    assert_eq!(canvas.ops.len(), native.ops.len());
    assert_eq!(canvas.viewport_width, native.viewport_width);
    assert_eq!(canvas.viewport_height, native.viewport_height);
}

#[test]
fn p2_canvas_bincode_roundtrip_preserves_ops() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None, None);
    let before = match CanvasBackend.render(&list) {
        BackendOutput::CanvasCommands(b) => b,
        _ => panic!("expected canvas"),
    };
    let cfg = config::standard();
    let bytes = bincode::serde::encode_to_vec(&before, cfg).expect("encode");
    let (after, _): (CommandBuffer, usize) =
        bincode::serde::decode_from_slice(&bytes, cfg).expect("decode");
    assert_eq!(before, after);
    assert!(!after.ops.is_empty());
    assert!(after
        .ops
        .iter()
        .any(|op| matches!(op, DrawOp::FillRect { .. })));
}

#[test]
fn p2_canvas_serde_json_roundtrip_preserves_ops() {
    let graph = two_task_graph();
    let list = build_display_list(&graph, epoch(&graph), None, None);
    let before = match CanvasBackend.render(&list) {
        BackendOutput::CanvasCommands(b) => b,
        _ => panic!("expected canvas"),
    };
    let json = serde_json::to_string(&before).expect("serialize");
    let after: CommandBuffer = serde_json::from_str(&json).expect("deserialize");
    assert_eq!(before, after);
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
    fs::create_dir_all(base.join("canvas_golden")).unwrap();

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
        koyori_arc_core::render(&[], &[], None, None),
    )
    .unwrap();

    let list = build_display_list(&graph, epoch(&graph), None, None);
    fs::write(
        base.join("ir_golden/two_tasks.json"),
        serde_json::to_string_pretty(&list).unwrap(),
    )
    .unwrap();

    let canvas = match CanvasBackend.render(&list) {
        BackendOutput::CanvasCommands(b) => b,
        _ => panic!("expected canvas"),
    };
    fs::write(
        base.join("canvas_golden/two_tasks.json"),
        serde_json::to_string_pretty(&canvas).unwrap(),
    )
    .unwrap();

    eprintln!("Golden fixtures written to {:?}", base);
}

// --- Phase 1: row virtualization ---

fn test_viewport() -> ScrollViewport {
    ScrollViewport {
        scroll_y: 0.0,
        client_height: 800.0,
    }
}

fn graph_from_fixture(count: TaskCount, density: DepDensity) -> GanttGraph {
    let fixture = generate_fixture(count, density);
    GanttGraph {
        tasks: fixture.tasks,
        deps: fixture.deps,
    }
}

fn count_svg_open_tags(svg: &str) -> usize {
    svg.split('<')
        .skip(1)
        .filter(|s| !s.starts_with('/') && !s.starts_with('!'))
        .count()
}

fn extract_task_groups(svg: &str) -> Vec<String> {
    let mut groups = Vec::new();
    let mut rest = svg;
    while let Some(idx) = rest.find(r#"data-task-id=""#) {
        let slice = &rest[idx..];
        if let Some(end) = slice.find("</g>") {
            groups.push(slice[..end + 4].to_string());
            rest = &slice[end + 4..];
        } else {
            break;
        }
    }
    groups.sort();
    groups
}

#[test]
fn p1_viewport_none_matches_full_render() {
    let graph = two_task_graph();
    let ep = epoch(&graph);
    let full = build_display_list(&graph, ep, None, None);
    let via_none = build_display_list(&graph, ep, None, None);
    assert_eq!(
        serde_json::to_string(&full).unwrap(),
        serde_json::to_string(&via_none).unwrap()
    );
}

#[test]
fn p1_viewport_row_fragments_match_full_svg() {
    let graph = two_task_graph();
    let ep = epoch(&graph);
    let full_svg = render_via_ir(&graph, None);
    let vp = ScrollViewport {
        scroll_y: 0.0,
        client_height: 200.0,
    };
    let list = build_display_list(&graph, ep, None, Some(vp));
    let vp_svg = match SvgBackend.render(&list) {
        BackendOutput::Svg(s) => s,
        _ => panic!(),
    };
    let full_groups = extract_task_groups(&full_svg);
    let vp_groups = extract_task_groups(&vp_svg);
    assert_eq!(full_groups, vp_groups);
}

#[test]
fn p1_dom_cap_invariant_independent_of_n() {
    let vp = test_viewport();
    let mut counts = Vec::new();
    for count in [TaskCount::N100, TaskCount::N2000, TaskCount::N5000] {
        let graph = graph_from_fixture(count, DepDensity::Dense);
        let ep = epoch(&graph);
        let list = build_display_list(&graph, ep, None, Some(vp));
        let full = build_display_list(&graph, ep, None, None);
        let svg = match SvgBackend.render(&list) {
            BackendOutput::Svg(s) => s,
            _ => panic!(),
        };
        let elems = count_svg_open_tags(&svg);
        counts.push(elems);
        assert!(
            list.metadata.primitive_count <= DOM_CAP * 4,
            "primitive_count {} exceeds DOM_CAP margin for {:?}",
            list.metadata.primitive_count,
            count
        );
        assert!(
            elems <= DOM_CAP as usize * 4,
            "svg elems {elems} exceeds DOM_CAP margin for {:?}",
            count
        );
        if count.get() >= 2000 {
            assert!(
                list.metadata.primitive_count < full.metadata.primitive_count / 5,
                "virtualized primitives should be ≪ full for {:?}",
                count
            );
        }
    }
    let max = *counts.iter().max().unwrap();
    let min = *counts.iter().min().unwrap();
    // Grid chrome scales with timeline span; deps incident to visible rows may vary.
    // Invariant: sub-linear growth in N (not O(N)).
    assert!(
        max <= min * 4,
        "DOM elem count should be sub-linear in N: min={min} max={max}"
    );
}

#[test]
fn p1_virtualized_primitive_count_much_smaller_than_full() {
    let graph = graph_from_fixture(TaskCount::N5000, DepDensity::Dense);
    let ep = epoch(&graph);
    let full = build_display_list(&graph, ep, None, None);
    let virt = build_display_list(&graph, ep, None, Some(test_viewport()));
    assert!(virt.metadata.primitive_count < full.metadata.primitive_count / 10);
}

#[test]
fn p1_compute_row_window_buffer() {
    let window = compute_row_window(
        Some(ScrollViewport {
            scroll_y: HEADER_H + ROW_H * 10.0,
            client_height: ROW_H * 5.0,
        }),
        100,
    )
    .unwrap();
    assert_eq!(window, (8, 17));
}
