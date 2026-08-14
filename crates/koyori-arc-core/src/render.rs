use chrono::{Duration, NaiveDate};
use wasm_bindgen::prelude::*;

use crate::backend::svg::empty_svg;
use crate::backend::{BackendOutput, CanvasBackend, CommandBuffer, RenderBackend, SvgBackend};
use crate::display_list::constants::{HEADER_H, LABEL_W, LEGEND_H, PX_PER_DAY, ROW_H};
use crate::error::{
    RenderError, CODE_CANVAS_CAPACITY, CODE_INPUT_LIMIT, CODE_PARSE_ERROR, CODE_SERIALIZE_ERROR,
};
use crate::display_list::{build_display_list, types::Palette, ScrollViewport};
use crate::graph::{GanttDep, GanttGraph, GanttTask};

/// Upper bounds enforced at Wasm entry points to limit memory/CPU abuse.
pub const MAX_TASKS: usize = 10_000;
pub const MAX_DEPS: usize = 100_000;
/// Allows about 839 bytes per task at `MAX_TASKS`, enough for realistic IDs and
/// titles plus JSON overhead while rejecting multi-megabyte individual fields.
pub const MAX_TASKS_JSON_BYTES: usize = 8 * 1024 * 1024;
/// Allows about 167 bytes per dependency at `MAX_DEPS`, leaving ample room for
/// two realistic task IDs and JSON overhead without accepting unbounded input.
pub const MAX_DEPS_JSON_BYTES: usize = 16 * 1024 * 1024;
pub const MAX_DATE_SPAN_DAYS: i64 = 3_650;

/// Conservative cross-browser Canvas2D backing-store edge limit. Browser and
/// GPU limits vary, but 16,384px is the lowest commonly supported maximum edge;
/// rejecting larger buffers avoids browser-specific blank canvases/context loss.
pub const MAX_CANVAS_SIDE_PX: usize = 16_384;
/// Keep the RGBA backing store near 128 MiB even when both dimensions are
/// individually supported. This leaves headroom for browser/GPU copies and
/// avoids a 16,384 x 16,384 canvas allocating roughly 1 GiB per buffer.
pub const MAX_CANVAS_AREA_PX: usize = 32 * 1024 * 1024;
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

fn raw_json_limit_error(tasks_json: &str, deps_json: &str) -> Option<RenderError> {
    if tasks_json.len() > MAX_TASKS_JSON_BYTES {
        return Some(RenderError::new(
            CODE_INPUT_LIMIT,
            format!("tasks JSON byte size exceeds limit ({MAX_TASKS_JSON_BYTES})"),
        ));
    }
    if deps_json.len() > MAX_DEPS_JSON_BYTES {
        return Some(RenderError::new(
            CODE_INPUT_LIMIT,
            format!("dependencies JSON byte size exceeds limit ({MAX_DEPS_JSON_BYTES})"),
        ));
    }
    None
}

fn common_graph_limit_error(tasks: &[GanttTask], deps: &[GanttDep]) -> Option<RenderError> {
    if tasks.len() > MAX_TASKS {
        return Some(RenderError::new(
            CODE_INPUT_LIMIT,
            format!("task count exceeds limit ({MAX_TASKS})"),
        ));
    }
    if deps.len() > MAX_DEPS {
        return Some(RenderError::new(
            CODE_INPUT_LIMIT,
            format!("dependency count exceeds limit ({MAX_DEPS})"),
        ));
    }
    None
}

/// Days of headroom the *rendered* latest date must keep below `NaiveDate::MAX`:
/// the display list steps the weekly grid one full week past it. The implicit
/// end date the renderer derives for `end: null` is **not** folded into this
/// number — `latest_rendered_date` derives that, so this constant stays exactly
/// the grid's one-week step.
pub const DATE_HEADROOM_DAYS: i64 = 7;

/// The last date the renderer actually touches for a task: an explicit `end`
/// (never before its own `start`), or the `start + 1 day` the display list
/// derives when `end` is null. `None` means that derived end already overflows
/// `NaiveDate::MAX`, which is itself a lack of headroom.
///
/// Every date limit derives from this one function, so the guard and the span
/// arithmetic can never disagree about which date is the last one.
fn latest_rendered_date(task: &GanttTask) -> Option<NaiveDate> {
    match task.end {
        Some(end) => Some(end.max(task.start)),
        None => task.start.checked_add_signed(Duration::days(1)),
    }
}

fn date_headroom_error(tasks: &[GanttTask]) -> Option<RenderError> {
    let lacks_headroom = tasks.iter().any(|t| {
        latest_rendered_date(t)
            .and_then(|latest| latest.checked_add_signed(Duration::days(DATE_HEADROOM_DAYS)))
            .is_none()
    });
    lacks_headroom.then(|| {
        RenderError::new(
            CODE_INPUT_LIMIT,
            format!("date is too close to the maximum supported date (needs {DATE_HEADROOM_DAYS} days of headroom past the last rendered date)"),
        )
    })
}

/// Rejects dates without headroom *before* computing anything, then computes
/// the span from the same derived latest date the guard checked.
fn rendered_date_span_days(tasks: &[GanttTask]) -> Result<i64, RenderError> {
    if tasks.is_empty() {
        return Ok(0);
    }
    if let Some(err) = date_headroom_error(tasks) {
        return Err(err);
    }
    let min_start = tasks.iter().map(|t| t.start).min().unwrap();
    // `date_headroom_error` returned `None`, so every task has a latest date.
    let max_date = tasks
        .iter()
        .filter_map(latest_rendered_date)
        .max()
        .unwrap_or(min_start);
    Ok((max_date - min_start).num_days().max(0))
}

fn svg_graph_limit_error(tasks: &[GanttTask], deps: &[GanttDep]) -> Option<RenderError> {
    if let Some(err) = common_graph_limit_error(tasks, deps) {
        return Some(err);
    }
    let span_days = match rendered_date_span_days(tasks) {
        Ok(days) => days,
        Err(err) => return Some(err),
    };
    if span_days > MAX_DATE_SPAN_DAYS {
        return Some(RenderError::new(
            CODE_INPUT_LIMIT,
            format!("date range exceeds limit ({MAX_DATE_SPAN_DAYS} days)"),
        ));
    }
    None
}

fn canvas_graph_limit_error(tasks: &[GanttTask], deps: &[GanttDep]) -> Option<RenderError> {
    if let Some(err) = common_graph_limit_error(tasks, deps) {
        return Some(err);
    }
    if tasks.len() > MAX_CANVAS_ROWS {
        return Some(RenderError::new(
            CODE_CANVAS_CAPACITY,
            format!(
                "canvas row count exceeds limit ({MAX_CANVAS_ROWS} rows / {MAX_CANVAS_SIDE_PX}px)"
            ),
        ));
    }
    let span_days = match rendered_date_span_days(tasks) {
        Ok(days) => days,
        Err(err) => return Some(err),
    };
    if span_days > MAX_CANVAS_DATE_SPAN_DAYS {
        return Some(RenderError::new(
            CODE_CANVAS_CAPACITY,
            format!(
                "canvas date range exceeds limit ({MAX_CANVAS_DATE_SPAN_DAYS} days / {MAX_CANVAS_SIDE_PX}px)"
            ),
        ));
    }
    let width_px = span_days as f64 * PX_PER_DAY
        + LABEL_W
        + CHART_RIGHT_PADDING_PX;
    let height_px = tasks.len() as f64 * ROW_H
        + HEADER_H
        + LEGEND_H
        + CHART_BOTTOM_PADDING_PX;
    if width_px * height_px > MAX_CANVAS_AREA_PX as f64 {
        return Some(RenderError::new(
            CODE_CANVAS_CAPACITY,
            format!("canvas area exceeds limit ({MAX_CANVAS_AREA_PX} pixels)"),
        ));
    }
    None
}

/// Every Wasm SVG rejection runs through here — `render_svg` and
/// `render_svg_error` share one implementation so they can never disagree.
fn prepare_svg_input(
    tasks_json: &str,
    deps_json: &str,
) -> Result<(Vec<GanttTask>, Vec<GanttDep>), RenderError> {
    if let Some(err) = raw_json_limit_error(tasks_json, deps_json) {
        return Err(err);
    }
    let tasks: Vec<GanttTask> = serde_json::from_str(tasks_json)
        .map_err(|e| RenderError::new(CODE_PARSE_ERROR, format!("parse error: {e}")))?;
    let deps: Vec<GanttDep> = serde_json::from_str(deps_json)
        .map_err(|e| RenderError::new(CODE_PARSE_ERROR, format!("parse error: {e}")))?;
    if let Some(err) = svg_graph_limit_error(&tasks, &deps) {
        return Err(err);
    }
    Ok((tasks, deps))
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
    let Ok((tasks, deps)) = prepare_svg_input(tasks_json, deps_json) else {
        return empty_svg();
    };
    let today = today_iso.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let scroll_viewport = viewport_json.and_then(|s| serde_json::from_str(&s).ok());
    render(&tasks, &deps, today, scroll_viewport)
}

/// Wasm entry point — why `render_svg` would refuse this input, as the same
/// `{"error":…,"code":…}` JSON `render_canvas_commands` returns, or `undefined`
/// when the input is renderable.
///
/// `render_svg` deliberately returns a blank chart for both "nothing to draw"
/// and "refused", which are not the same thing to a user. This is the channel
/// that tells them apart; it shares `prepare_svg_input` with `render_svg`
/// rather than re-deriving the rules.
#[wasm_bindgen]
pub fn render_svg_error(tasks_json: &str, deps_json: &str) -> Option<String> {
    prepare_svg_input(tasks_json, deps_json)
        .err()
        .map(|e| e.to_json())
}

/// Wasm entry point — the exact markup `render_svg` returns when there is
/// nothing to draw. JS compares against this instead of sniffing the markup
/// for `width="0"`, which a real chart with a 0% bar also contains.
#[wasm_bindgen]
pub fn empty_svg_markup() -> String {
    empty_svg()
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
    if let Some(err) = raw_json_limit_error(tasks_json, deps_json) {
        return err.to_json();
    }
    let tasks: Vec<GanttTask> = match serde_json::from_str(tasks_json) {
        Ok(v) => v,
        Err(e) => return RenderError::new(CODE_PARSE_ERROR, format!("parse error: {e}")).to_json(),
    };
    let deps: Vec<GanttDep> = match serde_json::from_str(deps_json) {
        Ok(v) => v,
        Err(e) => return RenderError::new(CODE_PARSE_ERROR, format!("parse error: {e}")).to_json(),
    };
    if let Some(err) = canvas_graph_limit_error(&tasks, &deps) {
        return err.to_json();
    }
    let today = today_iso.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    let scroll_viewport = viewport_json.and_then(|s| serde_json::from_str(&s).ok());
    let buffer = render_canvas(&tasks, &deps, today, scroll_viewport);
    serde_json::to_string(&buffer).unwrap_or_else(|e| {
        RenderError::new(CODE_SERIALIZE_ERROR, format!("serialize error: {e}")).to_json()
    })
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
        let s = RenderError::new(CODE_PARSE_ERROR, r#"parse error: bad "quote""#).to_json();
        let v: serde_json::Value = serde_json::from_str(&s).expect("valid json");
        assert_eq!(v["error"].as_str().unwrap(), r#"parse error: bad "quote""#);
        assert_eq!(v["code"].as_str(), Some("parse_error"));
    }

    #[test]
    fn canvas_parse_error_json_is_valid() {
        let json = render_canvas_commands("not json", "[]", None, None);
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(v.get("error").is_some());
    }

    #[test]
    fn raw_byte_limits_reject_tasks_before_deserialization() {
        // Deliberately invalid JSON: the byte-limit error proves parsing was not attempted.
        let oversized_tasks = "x".repeat(MAX_TASKS_JSON_BYTES + 1);
        let svg = render_svg(&oversized_tasks, "[]", None, None);
        assert_eq!(svg, crate::backend::svg::empty_svg());

        let json = render_canvas_commands(&oversized_tasks, "[]", None, None);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value["error"]
            .as_str()
            .unwrap()
            .contains("tasks JSON byte size"));
        assert!(!value["error"].as_str().unwrap().contains("parse error"));
    }

    #[test]
    fn raw_byte_limits_reject_dependencies_before_deserialization() {
        // Deliberately invalid JSON: the byte-limit error proves parsing was not attempted.
        let oversized_deps = "x".repeat(MAX_DEPS_JSON_BYTES + 1);
        let svg = render_svg("[]", &oversized_deps, None, None);
        assert_eq!(svg, crate::backend::svg::empty_svg());

        let json = render_canvas_commands("[]", &oversized_deps, None, None);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value["error"]
            .as_str()
            .unwrap()
            .contains("dependencies JSON byte size"));
        assert!(!value["error"].as_str().unwrap().contains("parse error"));
    }

    #[test]
    fn raw_byte_limit_rejects_single_giant_title() {
        let giant_title = "x".repeat(MAX_TASKS_JSON_BYTES);
        let tasks = serde_json::json!([{
            "id": "giant-title",
            "title": giant_title,
            "progress_pct": 0,
            "start": "2026-06-01",
            "end": "2026-06-02"
        }])
        .to_string();
        assert!(tasks.len() > MAX_TASKS_JSON_BYTES);

        let svg = render_svg(&tasks, "[]", None, None);
        assert_eq!(svg, crate::backend::svg::empty_svg());
        let json = render_canvas_commands(&tasks, "[]", None, None);
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value["error"]
            .as_str()
            .unwrap()
            .contains("tasks JSON byte size"));
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
    fn canvas_rejects_near_maximum_sides_when_area_is_too_large() {
        let tasks = canvas_tasks(MAX_CANVAS_ROWS, MAX_CANVAS_DATE_SPAN_DAYS);
        let width_px = MAX_CANVAS_DATE_SPAN_DAYS as f64 * PX_PER_DAY
            + LABEL_W
            + CHART_RIGHT_PADDING_PX;
        let height_px = MAX_CANVAS_ROWS as f64 * ROW_H
            + HEADER_H
            + LEGEND_H
            + CHART_BOTTOM_PADDING_PX;
        assert!(width_px <= MAX_CANVAS_SIDE_PX as f64);
        assert!(height_px <= MAX_CANVAS_SIDE_PX as f64);
        assert!(width_px * height_px > 267_000_000.0);

        let json = render_canvas_commands(
            &serde_json::to_string(&tasks).unwrap(),
            "[]",
            None,
            None,
        );
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(value["error"].as_str().unwrap().contains("canvas area"));
    }

    fn max_date_tasks_json(start: NaiveDate, end: Option<NaiveDate>) -> String {
        let tasks = vec![GanttTask {
            id: "max-date".to_string(),
            title: "Max date".to_string(),
            progress_pct: 0,
            start,
            end,
        }];
        serde_json::to_string(&tasks).unwrap()
    }

    fn near_max_start() -> NaiveDate {
        NaiveDate::MAX
            .checked_sub_signed(Duration::days(10))
            .expect("10 days below NaiveDate::MAX")
    }

    #[test]
    fn svg_entry_rejects_maximum_start_date_with_null_end() {
        // `end: None` makes the guard itself compute `start + 1 day`, which
        // overflows `NaiveDate::MAX`. Before the fix this panics inside the guard.
        let tasks_json = max_date_tasks_json(NaiveDate::MAX, None);
        let svg = render_svg(&tasks_json, "[]", None, None);
        assert_eq!(svg, crate::backend::svg::empty_svg());
    }

    #[test]
    fn canvas_entry_rejects_maximum_start_date_with_null_end() {
        let tasks_json = max_date_tasks_json(NaiveDate::MAX, None);
        let json = render_canvas_commands(&tasks_json, "[]", None, None);
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(
            value["error"].as_str().unwrap().contains("date"),
            "expected a date-related rejection, got: {json:.200}"
        );
    }

    #[test]
    fn svg_entry_rejects_explicit_maximum_end_date() {
        // Span is only 10 days, so the span limit accepts it; the panic before
        // the fix comes from the weekly grid stepping past `NaiveDate::MAX`.
        let tasks_json = max_date_tasks_json(near_max_start(), Some(NaiveDate::MAX));
        let svg = render_svg(&tasks_json, "[]", None, None);
        assert_eq!(svg, crate::backend::svg::empty_svg());
    }

    #[test]
    fn canvas_entry_rejects_explicit_maximum_end_date() {
        let tasks_json = max_date_tasks_json(near_max_start(), Some(NaiveDate::MAX));
        let json = render_canvas_commands(&tasks_json, "[]", None, None);
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(
            value["error"].as_str().unwrap().contains("date"),
            "expected a date-related rejection, got: {json:.200}"
        );
    }

    #[test]
    fn svg_date_span_boundary_accepts_limit_and_rejects_one_more() {
        let start = date(2026, 1, 1);
        let at_limit = max_date_tasks_json(start, Some(start + Duration::days(MAX_DATE_SPAN_DAYS)));
        let svg = render_svg(&at_limit, "[]", None, None);
        assert_ne!(svg, crate::backend::svg::empty_svg());

        let over_limit =
            max_date_tasks_json(start, Some(start + Duration::days(MAX_DATE_SPAN_DAYS + 1)));
        let svg = render_svg(&over_limit, "[]", None, None);
        assert_eq!(svg, crate::backend::svg::empty_svg());
    }

    #[test]
    fn canvas_reports_date_span_overflow_instead_of_panicking() {
        let start = date(2026, 1, 1);
        let over_limit = max_date_tasks_json(
            start,
            Some(start + Duration::days(MAX_CANVAS_DATE_SPAN_DAYS + 1)),
        );
        let json = render_canvas_commands(&over_limit, "[]", None, None);
        let value: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(value["error"]
            .as_str()
            .unwrap()
            .contains("canvas date range"));
    }

    #[test]
    fn wasm_errors_carry_machine_readable_codes() {
        // JS matches on `code`; the message beside it is display-only. These
        // assertions pin the wire values, so renaming a code constant fails here
        // as well as in the Vue contract test that reads `error.rs`.
        let parse = render_canvas_commands("not json", "[]", None, None);
        let v: serde_json::Value = serde_json::from_str(&parse).expect("valid json");
        assert_eq!(v["code"].as_str(), Some("parse_error"), "got: {parse:.200}");

        let oversized = "x".repeat(MAX_TASKS_JSON_BYTES + 1);
        let raw_limit = render_canvas_commands(&oversized, "[]", None, None);
        let v: serde_json::Value = serde_json::from_str(&raw_limit).expect("valid json");
        assert_eq!(v["code"].as_str(), Some("input_limit"));

        let too_many = canvas_tasks(MAX_CANVAS_ROWS + 1, 1);
        let capacity = render_canvas_commands(
            &serde_json::to_string(&too_many).unwrap(),
            "[]",
            None,
            None,
        );
        let v: serde_json::Value = serde_json::from_str(&capacity).expect("valid json");
        assert_eq!(v["code"].as_str(), Some("canvas_capacity"));
    }

    #[test]
    fn svg_entry_separates_nothing_to_draw_from_refused_input() {
        // Both still render the same empty chart, so the reason has to travel
        // on its own channel instead of being inferred from the markup.
        let refused = max_date_tasks_json(NaiveDate::MAX, None);
        assert_eq!(
            render_svg(&refused, "[]", None, None),
            crate::backend::svg::empty_svg()
        );
        assert_eq!(
            render_svg("[]", "[]", None, None),
            crate::backend::svg::empty_svg()
        );

        assert_eq!(render_svg_error("[]", "[]"), None, "nothing to draw is not an error");

        let reported = render_svg_error(&refused, "[]").expect("refusal is reported");
        let v: serde_json::Value = serde_json::from_str(&reported).expect("valid json");
        assert_eq!(v["code"].as_str(), Some("input_limit"));
        assert!(v["error"].as_str().unwrap().contains("date"), "got: {reported}");
    }

    #[test]
    fn svg_entry_rejects_start_seven_days_below_maximum_with_null_end() {
        // `end: None` makes the display list derive `start + 1 day`, so the
        // weekly grid steps up to `start + 8 days`. A guard that reserves
        // headroom above `start` alone accepts this input. The existing
        // `NaiveDate::MAX` cases start one day higher and therefore never
        // exercise this one-day gap.
        let start = NaiveDate::MAX
            .checked_sub_signed(Duration::days(DATE_HEADROOM_DAYS))
            .expect("7 days below NaiveDate::MAX");
        let tasks_json = max_date_tasks_json(start, None);
        assert_eq!(
            render_svg(&tasks_json, "[]", None, None),
            crate::backend::svg::empty_svg()
        );
    }

    #[test]
    fn native_render_survives_maximum_dates() {
        // The native entry points have no limit guards, so the checked date
        // arithmetic in the display list is what keeps them from panicking.
        let tasks = vec![GanttTask {
            id: "native-max".to_string(),
            title: "Native max".to_string(),
            progress_pct: 0,
            start: near_max_start(),
            end: Some(NaiveDate::MAX),
        }];
        let svg = render(&tasks, &[], None, None);
        assert!(svg.starts_with("<svg "));

        let no_end = vec![GanttTask {
            id: "native-max-null-end".to_string(),
            title: "Native max null end".to_string(),
            progress_pct: 0,
            start: NaiveDate::MAX,
            end: None,
        }];
        let svg = render(&no_end, &[], None, None);
        assert!(svg.starts_with("<svg "));
        let buffer = render_canvas(&no_end, &[], None, None);
        assert!(buffer.viewport_width.is_finite());
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
