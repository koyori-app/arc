use chrono::NaiveDate;
use wasm_bindgen::prelude::*;

use crate::backend::{BackendOutput, CanvasBackend, CommandBuffer, RenderBackend, SvgBackend};
use crate::display_list::{build_display_list, types::Palette, ScrollViewport};
use crate::graph::{GanttDep, GanttGraph, GanttTask};

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
        Err(e) => return format!("<!-- parse error: {e} -->"),
    };
    let deps: Vec<GanttDep> = match serde_json::from_str(deps_json) {
        Ok(v) => v,
        Err(e) => return format!("<!-- parse error: {e} -->"),
    };
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
        Err(e) => return format!(r#"{{"error":"parse error: {e}"}}"#),
    };
    let deps: Vec<GanttDep> = match serde_json::from_str(deps_json) {
        Ok(v) => v,
        Err(e) => return format!(r#"{{"error":"parse error: {e}"}}"#),
    };
    let today = today_iso.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let scroll_viewport = viewport_json.and_then(|s| serde_json::from_str(&s).ok());
    let buffer = render_canvas(&tasks, &deps, today, scroll_viewport);
    serde_json::to_string(&buffer).unwrap_or_else(|e| format!(r#"{{"error":"serialize error: {e}"}}"#))
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
    fn parse_error_returns_comment() {
        let svg = render_svg("not json", "[]", None, None);
        assert!(svg.starts_with("<!-- parse error:"));
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
