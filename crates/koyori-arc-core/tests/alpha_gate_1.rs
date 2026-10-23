//! 0.1.0-alpha.1 の関門 1: 図が黙って嘘を描く形を塞いだ試験。
//!
//! 日時の字、end が start より前のタスク、進捗の線の順、重なった id。
//! 凡例と題のはみ出しは、字の幅の見積もりに触れるので `display_list/build.rs` の
//! 単体試験にある。

use chrono::NaiveDate;
use koyori_arc_core::{
    build_display_list, render_canvas_commands, render_svg, render_svg_error, GanttDep, GanttGraph,
    GanttTask, LayerKind, Primitive, CODE_PARSE_ERROR, HEADER_H, ROW_H,
};

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn task(id: &str, title: &str, pct: i16, start: NaiveDate, end: Option<NaiveDate>) -> GanttTask {
    GanttTask {
        id: id.to_string(),
        title: title.to_string(),
        progress_pct: pct,
        start,
        end,
    }
}

fn dep(blocker: &str, blocked: &str) -> GanttDep {
    GanttDep {
        blocker_task_id: blocker.to_string(),
        blocked_task_id: blocked.to_string(),
    }
}

fn epoch(graph: &GanttGraph) -> NaiveDate {
    graph.tasks.iter().map(|t| t.start).min().unwrap()
}

fn progress_points(graph: &GanttGraph, today: Option<NaiveDate>) -> Vec<(f64, f64)> {
    let list = build_display_list(graph, epoch(graph), today, None);
    for layer in &list.layers {
        if layer.kind == LayerKind::ProgressLine {
            if let Some(Primitive::Polyline(p)) = layer.primitives.first() {
                return p.points.clone();
            }
        }
    }
    Vec::new()
}

// --- 一: 日時の字 ---

const DATE_TASKS: &str = r#"[
  {"id":"a","title":"A","progress_pct":50,"start":"2026-06-01","end":"2026-06-05"},
  {"id":"b","title":"B","progress_pct":20,"start":"2026-06-03"}
]"#;

const DATETIME_TASKS: &str = r#"[
  {"id":"a","title":"A","progress_pct":50,"start":"2026-06-01T09:00:00Z","end":"2026-06-05T23:30:00+09:00"},
  {"id":"b","title":"B","progress_pct":20,"start":"2026-06-03T00:00:00.000Z","end":null}
]"#;

#[test]
fn datetime_strings_draw_the_same_chart_as_dates() {
    let from_dates = render_svg(DATE_TASKS, "[]", Some("2026-06-03".to_string()), None, None);
    let from_datetimes = render_svg(
        DATETIME_TASKS,
        "[]",
        Some("2026-06-03T09:00:00+09:00".to_string()),
        None,
        None,
    );
    assert_eq!(from_dates, from_datetimes);
    assert_ne!(from_dates, koyori_arc_core::empty_svg());
    assert_eq!(
        render_svg_error(
            DATETIME_TASKS,
            "[]",
            Some("2026-06-03T09:00:00+09:00".to_string()),
            None
        ),
        None
    );
}

#[test]
fn canvas_entry_accepts_the_same_datetime_strings() {
    let json = render_canvas_commands(
        DATETIME_TASKS,
        "[]",
        Some("2026-06-03T00:00:00.000Z".to_string()),
        None,
        None,
    );
    let v: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert!(v.get("error").is_none(), "{json}");
}

#[test]
fn the_calendar_day_as_written_is_used_not_a_converted_one() {
    // UTC に直すと 6 月 2 日の 15:30 だが、字は 6 月 3 日の 00:30（+09:00）。
    let tasks = r#"[{"id":"a","title":"A","progress_pct":0,"start":"2026-06-03T00:30:00+09:00","end":"2026-06-06"}]"#;
    let by_date =
        r#"[{"id":"a","title":"A","progress_pct":0,"start":"2026-06-03","end":"2026-06-06"}]"#;
    assert_eq!(
        render_svg(tasks, "[]", None, None, None),
        render_svg(by_date, "[]", None, None, None)
    );
}

#[test]
fn an_unreadable_today_is_refused_not_dropped() {
    for bad in [
        "garbage",
        "20260608",
        "2026-13-45",
        "2026-06-08T",
        "06/08/2026",
    ] {
        let reported = render_svg_error(DATE_TASKS, "[]", Some(bad.to_string()), None)
            .unwrap_or_else(|| panic!("{bad:?} must be reported"));
        let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
        assert_eq!(v["code"], CODE_PARSE_ERROR, "{bad:?}");
        assert!(
            v["error"].as_str().unwrap().contains("today"),
            "the message names what was wrong: {reported}"
        );
        assert_eq!(
            render_svg(DATE_TASKS, "[]", Some(bad.to_string()), None, None),
            koyori_arc_core::empty_svg(),
            "{bad:?}: a refused input draws the blank chart, the same as an unreadable task"
        );
        let canvas = render_canvas_commands(DATE_TASKS, "[]", Some(bad.to_string()), None, None);
        let c: serde_json::Value = serde_json::from_str(&canvas).unwrap();
        assert_eq!(c["code"], CODE_PARSE_ERROR, "{bad:?}");
    }
}

#[test]
fn an_absent_or_blank_today_is_not_an_error() {
    let plain = render_svg(DATE_TASKS, "[]", None, None, None);
    assert_eq!(
        render_svg(DATE_TASKS, "[]", Some(String::new()), None, None),
        plain
    );
    assert_eq!(
        render_svg(DATE_TASKS, "[]", Some("  ".to_string()), None, None),
        plain
    );
    assert_eq!(
        render_svg_error(DATE_TASKS, "[]", Some(String::new()), None),
        None
    );
}

#[test]
fn an_unreadable_task_date_is_refused_with_its_reason() {
    for bad in [
        r#"[{"id":"a","title":"A","progress_pct":0,"start":"soon"}]"#,
        r#"[{"id":"a","title":"A","progress_pct":0,"start":"2026-06-01","end":"2026-6-99"}]"#,
    ] {
        let reported = render_svg_error(bad, "[]", None, None).expect("refused");
        let v: serde_json::Value = serde_json::from_str(&reported).unwrap();
        assert_eq!(v["code"], CODE_PARSE_ERROR, "{bad}");
        assert!(
            v["error"].as_str().unwrap().contains("invalid date"),
            "{reported}"
        );
    }
}

// --- 三: end が start より前 ---

fn end_before_start_graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![
            task(
                "early",
                "締切が開始より前",
                50,
                date(2026, 6, 10),
                Some(date(2026, 6, 5)),
            ),
            task(
                "normal",
                "通常",
                50,
                date(2026, 6, 2),
                Some(date(2026, 6, 8)),
            ),
        ],
        deps: vec![],
    }
}

#[test]
fn end_before_start_is_drawn_and_its_progress_point_does_not_pass_the_start() {
    let graph = end_before_start_graph();
    let list = build_display_list(&graph, epoch(&graph), None, None);

    let early = list
        .metadata
        .task_bboxes
        .iter()
        .find(|b| b.task_id == "early")
        .unwrap();
    // 幅 0 のタスクはひし形で描く。バーが無いと見えず、押せもしない。
    let drawn_as_diamond = list.layers.iter().any(|l| {
        l.primitives.iter().any(|p| match p {
            Primitive::Group(g) if g.task_id.as_deref() == Some("early") => g
                .children
                .iter()
                .any(|c| matches!(c, Primitive::Polygon(_))),
            _ => false,
        })
    });
    assert!(drawn_as_diamond, "end < start must still draw a mark");

    // 進捗の点は start より左へ行かぬ。start の x は bar_bbox の左端。
    let pts = progress_points(&graph, None);
    let row_y = HEADER_H + early.row as f64 * ROW_H + ROW_H / 2.0;
    let point = pts
        .iter()
        .find(|p| (p.1 - row_y).abs() < 1e-9)
        .expect("a progress point on the task's row");
    assert!(
        point.0 >= early.bar_bbox.x,
        "progress x {} is left of start x {}",
        point.0,
        early.bar_bbox.x
    );
}

#[test]
fn end_before_start_does_not_widen_the_chart_or_the_bar_of_others() {
    let a = end_before_start_graph();
    let mut b = end_before_start_graph();
    b.tasks[0].end = Some(b.tasks[0].start);
    let wa = build_display_list(&a, epoch(&a), None, None).viewport.width;
    let wb = build_display_list(&b, epoch(&b), None, None).viewport.width;
    assert_eq!(wa, wb, "end < start behaves as end == start");
}

// --- 三: 進捗の線は行の順 ---

fn reordered_graph() -> GanttGraph {
    // 入力の順は C, B, A。依存で A, B, C の順に並ぶ。
    GanttGraph {
        tasks: vec![
            task("c", "C", 20, date(2026, 6, 8), Some(date(2026, 6, 14))),
            task("b", "B", 50, date(2026, 6, 4), Some(date(2026, 6, 9))),
            task("a", "A", 80, date(2026, 6, 1), Some(date(2026, 6, 5))),
        ],
        deps: vec![dep("a", "b"), dep("b", "c")],
    }
}

#[test]
fn progress_line_runs_top_to_bottom_when_dependencies_reorder_rows() {
    let graph = reordered_graph();
    let pts = progress_points(&graph, None);
    assert_eq!(pts.len(), 3);
    let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
    assert!(
        ys.windows(2).all(|w| w[0] < w[1]),
        "points must descend row by row, got {ys:?}"
    );
}

#[test]
fn today_anchors_sit_at_the_top_of_the_first_row_and_the_bottom_of_the_last() {
    let graph = reordered_graph();
    let pts = progress_points(&graph, Some(date(2026, 6, 6)));
    assert_eq!(pts.len(), 5);
    assert_eq!(pts.first().unwrap().1, HEADER_H);
    assert_eq!(pts.last().unwrap().1, HEADER_H + 3.0 * ROW_H);
    let ys: Vec<f64> = pts.iter().map(|p| p.1).collect();
    assert!(
        ys.windows(2).all(|w| w[0] <= w[1]),
        "anchors included, y never goes back up: {ys:?}"
    );
}

// --- 三: 重なった id は先勝ち ---

#[test]
fn a_repeated_id_resolves_to_its_first_occurrence_for_arrows() {
    let graph = GanttGraph {
        tasks: vec![
            task(
                "dup",
                "一つ目",
                30,
                date(2026, 6, 1),
                Some(date(2026, 6, 6)),
            ),
            task(
                "other",
                "別のタスク",
                50,
                date(2026, 6, 4),
                Some(date(2026, 6, 10)),
            ),
            task(
                "dup",
                "二つ目",
                90,
                date(2026, 6, 7),
                Some(date(2026, 6, 13)),
            ),
        ],
        deps: vec![dep("dup", "other")],
    };
    let list = build_display_list(&graph, epoch(&graph), None, None);
    let first = &list.metadata.task_bboxes[0];
    assert_eq!(first.task_id, "dup");

    let path = list
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Dependencies)
        .flat_map(|l| l.primitives.iter())
        .find_map(|p| match p {
            Primitive::Path(p) => Some(p.d.clone()),
            _ => None,
        })
        .expect("one dependency arrow");
    // "M {x} {y} ..." — 始点の y は一つ目の dup のバーの下端。
    let mut parts = path.split_whitespace().skip(1);
    let start_x: f64 = parts.next().unwrap().parse().unwrap();
    let start_y: f64 = parts.next().unwrap().parse().unwrap();
    assert_eq!(start_y, first.bar_bbox.y + first.bar_bbox.height);
    // 始点の x も一つ目のバーの上にある（二つ目は 6 月 7 日以降で、ずっと右）。
    assert!(
        start_x >= first.bar_bbox.x && start_x <= first.bar_bbox.x + first.bar_bbox.width,
        "arrow starts at x={start_x}, first bar spans {}..{}",
        first.bar_bbox.x,
        first.bar_bbox.x + first.bar_bbox.width
    );
}
