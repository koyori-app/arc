use chrono::{Duration, NaiveDate};
use wasm_bindgen::prelude::*;

use crate::backend::{BackendOutput, CanvasBackend, CommandBuffer, RenderBackend, SvgBackend};
use crate::backend::svg::empty_svg;
use crate::display_list::constants::{HEADER_H, LABEL_W, LEGEND_H, PX_PER_DAY, ROW_H};
use crate::display_list::{build_display_list, types::Palette, ScrollViewport};
use crate::graph::{GanttDep, GanttGraph, GanttTask};

/// Upper bounds enforced at Wasm entry points to limit memory/CPU abuse.
pub const MAX_TASKS: usize = 10_000;
pub const MAX_DEPS: usize = 100_000;
pub const MAX_DATE_SPAN_DAYS: i64 = 3_650;

/// Conservative cross-browser Canvas2D backing-store edge limit. Browser and
/// GPU limits vary, but 16,384px is the lowest commonly supported maximum edge;
/// rejecting larger buffers avoids browser-specific blank canvases/context loss.
pub const MAX_CANVAS_SIDE_PX: usize = 16_384;
const CHART_RIGHT_PADDING_PX: f64 = 20.0;
const CHART_BOTTOM_PADDING_PX: f64 = 10.0;
pub const MAX_CANVAS_ROWS: usize = ((MAX_CANVAS_SIDE_PX as f64
    - HEADER_H
    - LEGEND_H
    - CHART_BOTTOM_PADDING_PX)
    / ROW_H) as usize;
pub const MAX_CANVAS_DATE_SPAN_DAYS: i64 = ((MAX_CANVAS_SIDE_PX as f64
    - LABEL_W
    - CHART_RIGHT_PADDING_PX)
    / PX_PER_DAY) as i64;

fn json_error(msg: impl Into<String>) -> String {
    serde_json::json!({ "error": msg.into() }).to_string()
}

fn common_graph_limit_error(tasks: &[GanttTask], deps: &[GanttDep]) -> Option<String> {
    if tasks.len() > MAX_TASKS {
        return Some(format!("task count exceeds limit ({MAX_TASKS})"));
    }
    if deps.len() > MAX_DEPS {
        return Some(format!("dependency count exceeds limit ({MAX_DEPS})"));
    }
    None
}

fn rendered_date_span_days(tasks: &[GanttTask]) -> i64 {
    if tasks.is_empty() {
        return 0;
    }
    let min_start = tasks.iter().map(|t| t.start).min().unwrap();
    let max_date = tasks
        .iter()
        .map(|t| t.end.unwrap_or_else(|| t.start + Duration::days(1)))
        .max()
        .unwrap();
    (max_date - min_start).num_days().max(0)
}

fn svg_graph_limit_error(tasks: &[GanttTask], deps: &[GanttDep]) -> Option<String> {
    if let Some(msg) = common_graph_limit_error(tasks, deps) {
        return Some(msg);
    }
    if rendered_date_span_days(tasks) > MAX_DATE_SPAN_DAYS {
        return Some(format!("date range exceeds limit ({MAX_DATE_SPAN_DAYS} days)"));
    }
    None
}

fn canvas_graph_limit_error(tasks: &[GanttTask], deps: &[GanttDep]) -> Option<String> {
    if let Some(msg) = common_graph_limit_error(tasks, deps) {
        return Some(msg);
    }
    if tasks.len() > MAX_CANVAS_ROWS {
        return Some(format!(
            "canvas row count exceeds limit ({MAX_CANVAS_ROWS} rows / {MAX_CANVAS_SIDE_PX}px)"
        ));
    }
    if rendered_date_span_days(tasks) > MAX_CANVAS_DATE_SPAN_DAYS {
        return Some(format!(
            "canvas date range exceeds limit ({MAX_CANVAS_DATE_SPAN_DAYS} days / {MAX_CANVAS_SIDE_PX}px)"
        ));
    }
    None
}

/// Native entry point — accepts typed structs directly.
pub fn render(
    tasks: &[GanttTask],
    deps: &[GanttDep],
    today: Option<NaiveDate>,
    scroll_viewport: Option<ScrollViewport>,
) -> String {
    if tasks.is_empty() {
        return crate::backend::svg::empty_svg();
    }

    let epoch = tasks.iter().map(|t| t.start).min().unwrap();
    let graph = GanttGraph {
        tasks: tasks.to_vec(),
        deps: deps.to_vec(),
    };
    let list = build_display_list(&graph, epoch, today, scroll_viewport);
    match SvgBackend.render(&list) {
        BackendOutput::Svg(s) => s,
        _ => unreachable!(),
    }
}

/// Native entry point — returns a `CommandBuffer` for Canvas2D replay.
pub fn render_canvas(
    tasks: &[GanttTask],
    deps: &[GanttDep],
    today: Option<NaiveDate>,
    scroll_viewport: Option<ScrollViewport>,
) -> CommandBuffer {
    if tasks.is_empty() {
        return CommandBuffer {
            viewport_width: 0.0,
            viewport_height: 0.0,
            ops: vec![],
            palette: Palette::standard(),
        };
    }

    let epoch = tasks.iter().map(|t| t.start).min().unwrap();
    let graph = GanttGraph {
        tasks: tasks.to_vec(),
        deps: deps.to_vec(),
    };
    let list = build_display_list(&graph, epoch, today, scroll_viewport);
    match CanvasBackend.render(&list) {
        BackendOutput::CanvasCommands(b) => b,
        _ => unreachable!(),
    }
}

/// Wasm entry point — accepts JSON strings matching the task project's API response shape.
/// `today_iso` is an optional ISO 8601 date string (e.g. "2026-06-16") for the today marker.
/// `viewport_json` is an optional `{"scroll_y":f64,"client_height":f64}` for row virtualization.
#[wasm_bindgen]
pub fn render_svg(
    tasks_json: &str,
    deps_json: &str,
    today_iso: Option<String>,
    viewport_json: Option<String>,
) -> String {
    let tasks: Vec<GanttTask> = match serde_json::from_str(tasks_json) {
        Ok(v) => v,
        Err(_) => return empty_svg(),
    };
    let deps: Vec<GanttDep> = match serde_json::from_str(deps_json) {
        Ok(v) => v,
        Err(_) => return empty_svg(),
    };
    if svg_graph_limit_error(&tasks, &deps).is_some() {
        return empty_svg();
    }
    let today = today_iso.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let scroll_viewport = viewport_json.and_then(|s| serde_json::from_str(&s).ok());
    render(&tasks, &deps, today, scroll_viewport)
}

/// Wasm entry point — returns a JSON-serialized `CommandBuffer` for JS-side Canvas2D replay.
/// On parse failure returns `{"error":"parse error: ..."}` (valid JSON, no draw ops).
#[wasm_bindgen]
pub fn render_canvas_commands(
    tasks_json: &str,
    deps_json: &str,
    today_iso: Option<String>,
    viewport_json: Option<String>,
) -> String {
    let tasks: Vec<GanttTask> = match serde_json::from_str(tasks_json) {
        Ok(v) => v,
        Err(e) => return json_error(format!("parse error: {e}")),
    };
    let deps: Vec<GanttDep> = match serde_json::from_str(deps_json) {
        Ok(v) => v,
        Err(e) => return json_error(format!("parse error: {e}")),
    };
    if let Some(msg) = canvas_graph_limit_error(&tasks, &deps) {
        return json_error(msg);
    }
    let today = today_iso.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let scroll_viewport = viewport_json.and_then(|s| serde_json::from_str(&s).ok());
    let buffer = render_canvas(&tasks, &deps, today, scroll_viewport);
    serde_json::to_string(&buffer).unwrap_or_else(|e| json_error(format!("serialize error: {e}")))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use crate::display_list::constants::{ARROW_CURVE, ARROW_HEAD, COLOR_TODAY};
    use crate::display_list::constants::{
        COLOR_HEADER_BG, COLOR_TIER_DONE, COLOR_TIER_HIGH, COLOR_TIER_LOW, COLOR_TIER_MID,
    };
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn two_tasks() -> (Vec<GanttTask>, Vec<GanttDep>) {
        let tasks = vec![
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
        ];
        let deps = vec![GanttDep {
            blocker_task_id: "task-1".to_string(),
            blocked_task_id: "task-2".to_string(),
        }];
        (tasks, deps)
    }

    #[test]
    fn output_is_valid_svg_root() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.starts_with("<svg "), "expected <svg ...>, got: {svg:.80}");
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn contains_task_titles() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.contains("Design"));
        assert!(svg.contains("Build"));
    }

    #[test]
    fn contains_progress_polyline() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.contains("stroke-dasharray"));
    }

    #[test]
    fn dependency_arrow_ends_with_open_chevron() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.contains("<path d=\"M "));
        assert!(svg.contains(&format!("m -{ARROW_HEAD} -{ARROW_HEAD}")));
        assert!(!svg.contains("<marker"));
    }

    #[test]
    fn dependency_arrow_routes_backward_when_blocked_starts_before_blocker_exit() {
        let tasks = vec![
            GanttTask {
                id: "t1".to_string(),
                title: "Design".to_string(),
                progress_pct: 100,
                start: date(2026, 6, 1),
                end: Some(date(2026, 6, 4)),
            },
            GanttTask {
                id: "t2".to_string(),
                title: "Backend".to_string(),
                progress_pct: 60,
                start: date(2026, 6, 2),
                end: Some(date(2026, 6, 8)),
            },
        ];
        let deps = vec![GanttDep {
            blocker_task_id: "t1".to_string(),
            blocked_task_id: "t2".to_string(),
        }];
        let svg = render(&tasks, &deps, None, None);
        let path = svg
            .split("<path d=\"")
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("dependency path present");
        assert!(
            path.contains(&format!("a {ARROW_CURVE} {ARROW_CURVE}")),
            "expected a non-degenerate rounded elbow, got: {path}"
        );
    }

    #[test]
    fn task_bars_have_data_task_id() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.contains(r#"data-task-id="task-1""#));
        assert!(svg.contains(r#"data-task-id="task-2""#));
    }

    #[test]
    fn today_marker_rendered_when_in_range() {
        let (t, d) = two_tasks();
        let today = date(2026, 6, 3);
        let svg = render(&t, &d, Some(today), None);
        assert!(svg.contains(COLOR_TODAY));
    }

    #[test]
    fn today_marker_absent_when_none() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(!svg.contains(&format!(
            r#"stroke="{COLOR_TODAY}" stroke-width="2" stroke-dasharray="4,3""#
        )));
    }

    #[test]
    fn date_header_present() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.contains(COLOR_HEADER_BG));
    }

    #[test]
    fn parse_error_returns_safe_empty_svg() {
        let svg = render_svg("not json", "[]", None, None);
        assert_eq!(svg, crate::backend::svg::empty_svg());
        assert!(!svg.contains("parse error"));
        assert!(!svg.contains("<!--"));
    }

    #[test]
    fn parse_error_comment_injection_does_not_break_markup() {
        let payload = r#"not json --><img onerror=alert(1)><!--"#;
        let svg = render_svg(payload, "[]", None, None);
        assert_eq!(svg, crate::backend::svg::empty_svg());
        assert!(!svg.contains("<img"));
    }

    #[test]
    fn wasm_id_xss_payload_escaped_in_output() {
        let tasks = vec![GanttTask {
            id: "x\" onmouseover=\"alert(1)\"".to_string(),
            title: "Safe".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 1),
            end: Some(date(2026, 6, 2)),
        }];
        let svg = render(&tasks, &[], None, None);
        assert!(svg.contains(r#"data-task-id="x&quot; onmouseover=&quot;alert(1)&quot;""#));
        assert!(!svg.contains(r#"onmouseover="alert"#));
    }

    #[test]
    fn json_error_produces_valid_json_with_quotes() {
        let s = super::json_error(r#"parse error: bad "quote""#);
        let v: serde_json::Value = serde_json::from_str(&s).expect("valid json");
        assert_eq!(v["error"].as_str().unwrap(), r#"parse error: bad "quote""#);
    }

    #[test]
    fn canvas_parse_error_json_is_valid() {
        let json = render_canvas_commands("not json", "[]", None, None);
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(v.get("error").is_some());
    }

    #[test]
    fn graph_limit_rejects_excessive_tasks() {
        let tasks: Vec<GanttTask> = (0..=super::MAX_TASKS)
            .map(|i| GanttTask {
                id: format!("t{i}"),
                title: format!("Task {i}"),
                progress_pct: 0,
                start: date(2026, 6, 1),
                end: Some(date(2026, 6, 2)),
            })
            .collect();
        let svg = render_svg(
            &serde_json::to_string(&tasks).unwrap(),
            "[]",
            None,
            None,
        );
        assert_eq!(svg, crate::backend::svg::empty_svg());
    }

    #[test]
    fn graph_limit_rejects_excessive_deps() {
        let tasks = vec![GanttTask {
            id: "t0".to_string(),
            title: "Only".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 1),
            end: Some(date(2026, 6, 2)),
        }];
        let deps: Vec<GanttDep> = (0..=super::MAX_DEPS)
            .map(|i| GanttDep {
                blocker_task_id: "t0".to_string(),
                blocked_task_id: format!("t{i}"),
            })
            .collect();
        let json = render_canvas_commands(
            &serde_json::to_string(&tasks).unwrap(),
            &serde_json::to_string(&deps).unwrap(),
            None,
            None,
        );
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(v.get("error").is_some());
    }

    fn canvas_tasks(count: usize, end_offset_days: i64) -> Vec<GanttTask> {
        (0..count)
            .map(|i| GanttTask {
                id: format!("canvas-{i}"),
                title: format!("Canvas task {i}"),
                progress_pct: 0,
                start: date(2026, 1, 1),
                end: Some(date(2026, 1, 1) + Duration::days(end_offset_days)),
            })
            .collect()
    }

    #[test]
    fn canvas_row_limit_is_derived_from_max_side() {
        let accepted_height = MAX_CANVAS_ROWS as f64 * ROW_H
            + HEADER_H
            + LEGEND_H
            + CHART_BOTTOM_PADDING_PX;
        let rejected_height = (MAX_CANVAS_ROWS + 1) as f64 * ROW_H
            + HEADER_H
            + LEGEND_H
            + CHART_BOTTOM_PADDING_PX;
        assert!(accepted_height <= MAX_CANVAS_SIDE_PX as f64);
        assert!(rejected_height > MAX_CANVAS_SIDE_PX as f64);

        let accepted = canvas_tasks(MAX_CANVAS_ROWS, 1);
        let accepted_json = render_canvas_commands(
            &serde_json::to_string(&accepted).unwrap(),
            "[]",
            None,
            None,
        );
        let accepted_value: serde_json::Value = serde_json::from_str(&accepted_json).unwrap();
        assert!(accepted_value.get("error").is_none());
        assert!(accepted_value["viewport_height"].as_f64().unwrap() <= MAX_CANVAS_SIDE_PX as f64);

        let rejected = canvas_tasks(MAX_CANVAS_ROWS + 1, 1);
        let rejected_json = render_canvas_commands(
            &serde_json::to_string(&rejected).unwrap(),
            "[]",
            None,
            None,
        );
        let rejected_value: serde_json::Value = serde_json::from_str(&rejected_json).unwrap();
        assert!(rejected_value["error"].as_str().unwrap().contains("canvas row count"));
    }

    #[test]
    fn canvas_date_limit_is_derived_from_max_side_and_does_not_reduce_svg_limit() {
        let accepted_width = MAX_CANVAS_DATE_SPAN_DAYS as f64 * PX_PER_DAY
            + LABEL_W
            + CHART_RIGHT_PADDING_PX;
        let rejected_width = (MAX_CANVAS_DATE_SPAN_DAYS + 1) as f64 * PX_PER_DAY
            + LABEL_W
            + CHART_RIGHT_PADDING_PX;
        assert!(accepted_width <= MAX_CANVAS_SIDE_PX as f64);
        assert!(rejected_width > MAX_CANVAS_SIDE_PX as f64);

        let accepted = canvas_tasks(1, MAX_CANVAS_DATE_SPAN_DAYS);
        let accepted_tasks_json = serde_json::to_string(&accepted).unwrap();
        let accepted_json = render_canvas_commands(&accepted_tasks_json, "[]", None, None);
        let accepted_value: serde_json::Value = serde_json::from_str(&accepted_json).unwrap();
        assert!(accepted_value.get("error").is_none());
        assert!(accepted_value["viewport_width"].as_f64().unwrap() <= MAX_CANVAS_SIDE_PX as f64);

        let rejected = canvas_tasks(1, MAX_CANVAS_DATE_SPAN_DAYS + 1);
        let rejected_tasks_json = serde_json::to_string(&rejected).unwrap();
        let rejected_json = render_canvas_commands(&rejected_tasks_json, "[]", None, None);
        let rejected_value: serde_json::Value = serde_json::from_str(&rejected_json).unwrap();
        assert!(rejected_value["error"].as_str().unwrap().contains("canvas date range"));

        let svg = render_svg(&rejected_tasks_json, "[]", None, None);
        assert_ne!(svg, crate::backend::svg::empty_svg());
    }

    #[test]
    fn xml_special_chars_escaped() {
        let tasks = vec![GanttTask {
            id: "t1".to_string(),
            title: "A & B < C".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 1),
            end: Some(date(2026, 6, 2)),
        }];
        let svg = render(&tasks, &[], None, None);
        assert!(svg.contains("A &amp; B &lt; C"));
        assert!(!svg.contains("A & B"));
    }

    #[test]
    fn empty_tasks_returns_empty_svg() {
        let svg = render(&[], &[], None, None);
        assert!(svg.contains("width=\"0\""));
        assert!(svg.contains("viewBox=\"0 0 0 0\""));
        assert!(svg.contains(r#"role="img""#));
    }

    #[test]
    fn svg_has_viewbox_and_a11y() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.contains("viewBox="));
        assert!(svg.contains(r#"role="img""#));
        assert!(svg.contains(r#"aria-label="Gantt chart""#));
        assert!(svg.contains("<title>Gantt chart</title>"));
        assert!(svg.contains("<desc>"));
    }

    #[test]
    fn each_bar_shows_progress_percent() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.contains("100%"));
        assert!(svg.contains("50%"));
    }

    #[test]
    fn zero_and_full_progress_explicit() {
        let tasks = vec![
            GanttTask {
                id: "t0".to_string(),
                title: "Not started".to_string(),
                progress_pct: 0,
                start: date(2026, 6, 1),
                end: Some(date(2026, 6, 3)),
            },
            GanttTask {
                id: "t1".to_string(),
                title: "Done".to_string(),
                progress_pct: 100,
                start: date(2026, 6, 3),
                end: Some(date(2026, 6, 5)),
            },
        ];
        let svg = render(&tasks, &[], None, None);
        assert!(svg.contains("0%"));
        assert!(svg.contains("100%"));
    }

    #[test]
    fn task_group_has_hover_title_tooltip() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.contains("<title>Design: 2026-06-01 – 2026-06-04 (100%)</title>"));
        assert!(svg.contains("<title>Build: 2026-06-04 – 2026-06-08 (50%)</title>"));
    }

    #[test]
    fn long_title_truncated_with_ellipsis() {
        let tasks = vec![GanttTask {
            id: "t1".to_string(),
            title: "Very Long Task Title That Should Be Truncated".to_string(),
            progress_pct: 25,
            start: date(2026, 6, 1),
            end: Some(date(2026, 6, 5)),
        }];
        let svg = render(&tasks, &[], None, None);
        assert!(svg.contains("Very Long Task …"));
        assert!(svg.contains(
            "<title>Very Long Task Title That Should Be Truncated: 2026-06-01 – 2026-06-05 (25%)</title>"
        ));
        let label_count = svg.matches("Should Be Truncated").count();
        assert_eq!(label_count, 1, "full title should appear only in <title>");
    }

    #[test]
    fn end_before_start_produces_no_negative_width_bar() {
        let tasks = vec![GanttTask {
            id: "t1".to_string(),
            title: "Inverted".to_string(),
            progress_pct: 50,
            start: date(2026, 6, 5),
            end: Some(date(2026, 6, 1)),
        }];
        let svg = render(&tasks, &[], None, None);
        assert!(!svg.contains("width=\"-"));
        assert!(svg.contains("50%"));
    }

    #[test]
    fn zero_duration_task_renders_milestone_diamond() {
        let tasks = vec![GanttTask {
            id: "ms1".to_string(),
            title: "Launch".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 3),
            end: Some(date(2026, 6, 3)),
        }];
        let svg = render(&tasks, &[], None, None);
        assert!(svg.contains("<polygon"));
        assert!(svg.contains("0%"));
        assert!(!svg.contains(r#"width="0.0""#));
    }

    #[test]
    fn progress_line_legend_present() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        assert!(svg.contains("progress-line-legend"));
        assert!(svg.contains("進捗ステータスライン"));
        assert!(svg.contains("bar-tier-legend"));
    }

    #[test]
    fn progress_line_legacy_when_today_absent() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None, None);
        let poly = svg
            .split(r#"class="progress-status-line" points=""#)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("progress polyline");
        assert!(poly.starts_with("210,"), "legacy start at progress x, got: {poly}");
    }

    #[test]
    fn progress_line_today_anchored_when_today_in_range() {
        let (t, d) = two_tasks();
        let today = date(2026, 6, 3);
        let svg = render(&t, &d, Some(today), None);
        let poly = svg
            .split(r#"class="progress-status-line" points=""#)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("progress polyline");
        assert!(poly.starts_with("180,"), "anchored start at today x, got: {poly}");
        let last_pt = poly.split(' ').next_back().expect("last point");
        assert!(
            last_pt.starts_with("180,"),
            "anchored end at today x, got last: {last_pt}"
        );
        assert!(svg.contains(r#"class="bar-progress bar-tier-done""#));
        assert!(svg.contains(r#"class="bar-progress bar-tier-mid""#));
    }

    #[test]
    fn progress_line_legacy_when_today_out_of_range() {
        let (t, d) = two_tasks();
        let today = date(2020, 1, 1);
        let svg = render(&t, &d, Some(today), None);
        let poly = svg
            .split(r#"class="progress-status-line" points=""#)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("progress polyline");
        assert!(poly.starts_with("210,"), "out-of-range today falls back to legacy");
        assert!(!svg.contains(&format!(
            r#"stroke="{COLOR_TODAY}" stroke-width="2" stroke-dasharray="4,3""#
        )));
    }

    #[test]
    fn progress_tier_colors_distinguish_achievement_bands() {
        let tasks = vec![
            GanttTask {
                id: "low".to_string(),
                title: "Low".to_string(),
                progress_pct: 20,
                start: date(2026, 6, 1),
                end: Some(date(2026, 6, 3)),
            },
            GanttTask {
                id: "mid".to_string(),
                title: "Mid".to_string(),
                progress_pct: 50,
                start: date(2026, 6, 3),
                end: Some(date(2026, 6, 5)),
            },
            GanttTask {
                id: "high".to_string(),
                title: "High".to_string(),
                progress_pct: 80,
                start: date(2026, 6, 5),
                end: Some(date(2026, 6, 7)),
            },
            GanttTask {
                id: "done".to_string(),
                title: "Done".to_string(),
                progress_pct: 100,
                start: date(2026, 6, 7),
                end: Some(date(2026, 6, 9)),
            },
        ];
        let svg = render(&tasks, &[], None, None);
        assert!(svg.contains(&format!(r#"fill="{COLOR_TIER_LOW}""#)));
        assert!(svg.contains(&format!(r#"fill="{COLOR_TIER_MID}""#)));
        assert!(svg.contains(&format!(r#"fill="{COLOR_TIER_HIGH}""#)));
        assert!(svg.contains(&format!(r#"fill="{COLOR_TIER_DONE}""#)));
        assert!(svg.contains(r#"class="bar-progress bar-tier-low""#));
        assert!(svg.contains(r#"class="bar-progress bar-tier-done""#));
        assert!(svg.contains(r#"class="bar-bg""#));
    }
}
