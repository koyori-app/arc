use chrono::{Duration, FixedOffset, NaiveDate};
use wasm_bindgen::prelude::*;

use crate::backend::svg::empty_svg;
use crate::backend::{BackendOutput, CanvasBackend, CommandBuffer, RenderBackend, SvgBackend};
use crate::date_input::{parse_tz_offset, resolve_date};
use crate::display_list::build::chart_width_for;
use crate::display_list::constants::{
    chart_height, CHART_CHROME_H, CHART_CHROME_W, PX_PER_DAY, ROW_H,
};
use crate::display_list::legend::legend_spec;
use crate::display_list::{
    build_display_list,
    types::{ChartOptions, LegendMode, Palette},
    ScrollViewport,
};
use crate::error::{
    RenderError, CODE_CANVAS_CAPACITY, CODE_INPUT_LIMIT, CODE_PARSE_ERROR, CODE_SERIALIZE_ERROR,
};
use crate::graph::{parse_tasks_json, GanttDep, GanttGraph, GanttTask};

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
/// Upper bound on `options_json`. It holds a handful of scalar keys, so this is
/// far more than a real value needs; a larger string is refused before it is parsed.
pub const MAX_OPTIONS_JSON_BYTES: usize = 4 * 1024;

/// Conservative cross-browser Canvas2D backing-store edge limit. Browser and
/// GPU limits vary, but 16,384px is the lowest commonly supported maximum edge;
/// rejecting larger buffers avoids browser-specific blank canvases/context loss.
pub const MAX_CANVAS_SIDE_PX: usize = 16_384;
/// Keep the RGBA backing store near 128 MiB even when both dimensions are
/// individually supported. This leaves headroom for browser/GPU copies and
/// avoids a 16,384 x 16,384 canvas allocating roughly 1 GiB per buffer.
pub const MAX_CANVAS_AREA_PX: usize = 32 * 1024 * 1024;
/// The rows and days left over once the chart's fixed chrome is paid for.
/// `CHART_CHROME_H`/`_W` are the same sums `build_display_list` adds, so a term
/// added to the chart cannot slip past these guards.
pub const MAX_CANVAS_ROWS: usize = ((MAX_CANVAS_SIDE_PX as f64 - CHART_CHROME_H) / ROW_H) as usize;
pub const MAX_CANVAS_DATE_SPAN_DAYS: i64 =
    ((MAX_CANVAS_SIDE_PX as f64 - CHART_CHROME_W) / PX_PER_DAY) as i64;

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

/// Refuses a chart the canvas cannot hold. It never accepts a chart that is
/// larger than the one drawn: the width is the width `build_display_list` draws
/// for this `legend`, and the height is the taller of the two (the one with the
/// legend), so a chart without a legend, which is shorter, is not under-counted.
fn canvas_graph_limit_error(
    tasks: &[GanttTask],
    deps: &[GanttDep],
    legend: LegendMode,
) -> Option<RenderError> {
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
    // The days are checked first, so a chart of a plausible length reaches the
    // exact width. A label can run past the days, so the days alone under-count.
    let epoch = tasks.iter().map(|t| t.start).min().unwrap_or_default();
    let width_px = chart_width_for(tasks, epoch, legend);
    if width_px > MAX_CANVAS_SIDE_PX as f64 {
        return Some(RenderError::new(
            CODE_CANVAS_CAPACITY,
            format!(
                "canvas date range exceeds limit ({MAX_CANVAS_DATE_SPAN_DAYS} days / {MAX_CANVAS_SIDE_PX}px)"
            ),
        ));
    }
    let height_px = chart_height(tasks.len() as f64);
    if width_px * height_px > MAX_CANVAS_AREA_PX as f64 {
        return Some(RenderError::new(
            CODE_CANVAS_CAPACITY,
            format!("canvas area exceeds limit ({MAX_CANVAS_AREA_PX} pixels)"),
        ));
    }
    None
}

/// `today` as `options_json` carries it. Absent or blank means "no today
/// marker". A value that is present but unreadable is a refusal, not a silent
/// `None`: dropping it would also switch the progress line from the
/// today-anchored shape to the legacy one without telling anyone.
fn parse_today(
    today_iso: Option<&str>,
    offset: Option<FixedOffset>,
) -> Result<Option<NaiveDate>, RenderError> {
    match today_iso.map(str::trim) {
        None | Some("") => Ok(None),
        Some(s) => resolve_date(s, offset)
            .map(Some)
            .map_err(|e| RenderError::new(CODE_PARSE_ERROR, format!("parse error: today: {e}"))),
    }
}

/// `tz_offset_minutes` as `options_json` carries it. Absent means "use the day a
/// string is written in"; an unusable number is a refusal.
fn read_tz_offset(tz_offset_minutes: Option<f64>) -> Result<Option<FixedOffset>, RenderError> {
    parse_tz_offset(tz_offset_minutes)
        .map_err(|e| RenderError::new(CODE_PARSE_ERROR, format!("parse error: {e}")))
}

/// `options_json` as it arrives, before the strings it holds are resolved.
struct WireOptions {
    today: Option<String>,
    scroll_viewport: Option<ScrollViewport>,
    tz_offset_minutes: Option<f64>,
    legend: LegendMode,
}

/// Every key `options_json` may carry. A key outside this list is refused so
/// that a misspelt one cannot silently do nothing.
const OPTION_KEYS: &[&str] = &["today", "scroll_viewport", "tz_offset_minutes", "legend"];

fn options_error(message: impl std::fmt::Display) -> RenderError {
    RenderError::new(CODE_PARSE_ERROR, format!("parse error: options: {message}"))
}

/// Reads `options_json`. Absent, `undefined` and `null` on the JS side reach
/// here as `None` and mean the defaults; so does `{}`. Anything that is not a
/// JSON object is a refusal, and so is a key this build does not know or a
/// value of the wrong shape. A key set to `null` is the same as absent.
fn parse_wire_options(options_json: Option<&str>) -> Result<WireOptions, RenderError> {
    let mut wire = WireOptions {
        today: None,
        scroll_viewport: None,
        tz_offset_minutes: None,
        legend: LegendMode::Shown,
    };
    let Some(json) = options_json else {
        return Ok(wire);
    };
    if json.len() > MAX_OPTIONS_JSON_BYTES {
        return Err(RenderError::new(
            CODE_INPUT_LIMIT,
            format!("options JSON byte size exceeds limit ({MAX_OPTIONS_JSON_BYTES})"),
        ));
    }
    let value: serde_json::Value = serde_json::from_str(json).map_err(options_error)?;
    let serde_json::Value::Object(map) = value else {
        return Err(options_error("expected a JSON object"));
    };
    for (key, value) in map {
        if !OPTION_KEYS.contains(&key.as_str()) {
            return Err(options_error(format!(
                "unknown key `{key}` (expected one of {})",
                OPTION_KEYS.join(", ")
            )));
        }
        if value.is_null() {
            continue;
        }
        match key.as_str() {
            "today" => match value {
                serde_json::Value::String(s) => wire.today = Some(s),
                _ => return Err(options_error("today: expected a string")),
            },
            "scroll_viewport" => {
                // `ScrollViewport` derives its reader, which also takes a bare
                // array; the shape on offer is the object.
                if !value.is_object() {
                    return Err(options_error(
                        "scroll_viewport: expected an object with scroll_y and client_height",
                    ));
                }
                wire.scroll_viewport = Some(
                    serde_json::from_value(value)
                        .map_err(|e| options_error(format!("scroll_viewport: {e}")))?,
                );
            }
            "tz_offset_minutes" => match value.as_f64() {
                Some(minutes) => wire.tz_offset_minutes = Some(minutes),
                None => return Err(options_error("tz_offset_minutes: expected a number")),
            },
            "legend" => {
                wire.legend = match value.as_str() {
                    Some("shown") => LegendMode::Shown,
                    Some("hidden") => LegendMode::Hidden,
                    _ => return Err(options_error("legend: expected \"shown\" or \"hidden\"")),
                }
            }
            _ => unreachable!("OPTION_KEYS covers every arm"),
        }
    }
    Ok(wire)
}

/// What a Wasm entry point has once every refusal about the inputs but the
/// graph's own limits is ruled out: the time zone and the resolved options.
struct ResolvedOptions {
    offset: Option<FixedOffset>,
    wire: WireOptions,
}

fn resolve_options(options_json: Option<&str>) -> Result<ResolvedOptions, RenderError> {
    let wire = parse_wire_options(options_json)?;
    let offset = read_tz_offset(wire.tz_offset_minutes)?;
    Ok(ResolvedOptions { offset, wire })
}

impl ResolvedOptions {
    fn legend(&self) -> LegendMode {
        self.wire.legend
    }

    fn chart_options(&self) -> Result<ChartOptions, RenderError> {
        Ok(ChartOptions {
            today: parse_today(self.wire.today.as_deref(), self.offset)?,
            scroll_viewport: self.wire.scroll_viewport,
            legend: self.wire.legend,
        })
    }
}

/// What `prepare_svg_input` hands on once every refusal has been ruled out.
struct PreparedSvgInput {
    tasks: Vec<GanttTask>,
    deps: Vec<GanttDep>,
    options: ChartOptions,
}

/// Every Wasm SVG rejection runs through here — `render_svg` and
/// `render_svg_error` share one implementation so they can never disagree.
fn prepare_svg_input(
    tasks_json: &str,
    deps_json: &str,
    options_json: Option<&str>,
) -> Result<PreparedSvgInput, RenderError> {
    let resolved = resolve_options(options_json)?;
    if let Some(err) = raw_json_limit_error(tasks_json, deps_json) {
        return Err(err);
    }
    let tasks = parse_tasks_json(tasks_json, resolved.offset)
        .map_err(|e| RenderError::new(CODE_PARSE_ERROR, e))?;
    let deps: Vec<GanttDep> = serde_json::from_str(deps_json)
        .map_err(|e| RenderError::new(CODE_PARSE_ERROR, format!("parse error: {e}")))?;
    if let Some(err) = svg_graph_limit_error(&tasks, &deps) {
        return Err(err);
    }
    let options = resolved.chart_options()?;
    Ok(PreparedSvgInput {
        tasks,
        deps,
        options,
    })
}

/// Native entry point — accepts typed structs directly.
pub fn render(tasks: &[GanttTask], deps: &[GanttDep], options: &ChartOptions) -> String {
    if tasks.is_empty() {
        return crate::backend::svg::empty_svg();
    }

    let epoch = tasks.iter().map(|t| t.start).min().unwrap();
    let graph = GanttGraph {
        tasks: tasks.to_vec(),
        deps: deps.to_vec(),
    };
    let list = build_display_list(&graph, epoch, options);
    match SvgBackend.render(&list) {
        BackendOutput::Svg(s) => s,
        _ => unreachable!(),
    }
}

/// Native entry point — returns a `CommandBuffer` for Canvas2D replay.
pub fn render_canvas(
    tasks: &[GanttTask],
    deps: &[GanttDep],
    options: &ChartOptions,
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
    let list = build_display_list(&graph, epoch, options);
    match CanvasBackend.render(&list) {
        BackendOutput::CanvasCommands(b) => b,
        _ => unreachable!(),
    }
}

/// Wasm entry point — accepts JSON strings matching the task project's API response shape.
///
/// `options_json` is an optional JSON object. Absent, `null` and `{}` draw the default chart.
/// - `today`: an ISO 8601 date (e.g. "2026-06-16") or date-time (e.g.
///   "2026-06-16T09:00:00+09:00") for the today marker; the calendar day as written is used.
/// - `scroll_viewport`: `{"scroll_y":f64,"client_height":f64}` for row virtualization.
/// - `tz_offset_minutes`: the caller's time zone as a signed integer of minutes east of UTC
///   (JST is 540). Absent keeps the day a date-time string is written in; present converts
///   date-time strings that carry an offset to that zone's calendar day. Date-only strings
///   are never converted.
///
/// A key that is unknown, or a value that cannot be read (an unreadable `today`, an offset
/// that is not an integer or is beyond ±1439, a `scroll_viewport` of the wrong shape), refuses
/// the render (see `render_svg_error`). A string longer than `MAX_OPTIONS_JSON_BYTES` is
/// refused with the `input_limit` code.
#[wasm_bindgen]
pub fn render_svg(tasks_json: &str, deps_json: &str, options_json: Option<String>) -> String {
    let Ok(PreparedSvgInput {
        tasks,
        deps,
        options,
    }) = prepare_svg_input(tasks_json, deps_json, options_json.as_deref())
    else {
        return empty_svg();
    };
    render(&tasks, &deps, &options)
}

/// Wasm entry point — why `render_svg` would refuse this input, as the same
/// `{"error":…,"code":…}` JSON `render_canvas_commands` returns, or `undefined`
/// when the input is renderable.
///
/// `render_svg` deliberately returns a blank chart for both "nothing to draw"
/// and "refused", which are not the same thing to a user. This is the channel
/// that tells them apart; it shares `prepare_svg_input` with `render_svg`
/// rather than re-deriving the rules.
///
/// Pass the same `options_json` given to `render_svg`: an options string that
/// cannot be read is a refusal too.
#[wasm_bindgen]
pub fn render_svg_error(
    tasks_json: &str,
    deps_json: &str,
    options_json: Option<String>,
) -> Option<String> {
    prepare_svg_input(tasks_json, deps_json, options_json.as_deref())
        .err()
        .map(|e| e.to_json())
}

/// Wasm entry point — what the legend says, as JSON: the five tiers (key, label,
/// colour, the percentages they cover) and the progress-line sample. For a caller
/// that hides the legend (`"legend":"hidden"`) and draws its own.
#[wasm_bindgen]
pub fn legend_spec_json() -> String {
    serde_json::to_string(&legend_spec()).unwrap_or_else(|e| {
        RenderError::new(CODE_SERIALIZE_ERROR, format!("serialize error: {e}")).to_json()
    })
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
    options_json: Option<String>,
) -> String {
    let resolved = match resolve_options(options_json.as_deref()) {
        Ok(resolved) => resolved,
        Err(err) => return err.to_json(),
    };
    if let Some(err) = raw_json_limit_error(tasks_json, deps_json) {
        return err.to_json();
    }
    let tasks = match parse_tasks_json(tasks_json, resolved.offset) {
        Ok(v) => v,
        Err(e) => return RenderError::new(CODE_PARSE_ERROR, e).to_json(),
    };
    let deps: Vec<GanttDep> = match serde_json::from_str(deps_json) {
        Ok(v) => v,
        Err(e) => return RenderError::new(CODE_PARSE_ERROR, format!("parse error: {e}")).to_json(),
    };
    if let Some(err) = canvas_graph_limit_error(&tasks, &deps, resolved.legend()) {
        return err.to_json();
    }
    let options = match resolved.chart_options() {
        Ok(options) => options,
        Err(err) => return err.to_json(),
    };
    let buffer = render_canvas(&tasks, &deps, &options);
    serde_json::to_string(&buffer).unwrap_or_else(|e| {
        RenderError::new(CODE_SERIALIZE_ERROR, format!("serialize error: {e}")).to_json()
    })
}

#[cfg(test)]
#[path = "../tests/common/mod.rs"]
mod test_common;

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(test)]
    use crate::display_list::constants::{chart_width, ARROW_CURVE, ARROW_HEAD, LEGEND_H};
    use crate::display_list::types::{ColorId, Palette};
    use crate::render::test_common;
    use chrono::NaiveDate;

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn palette_hex(id: ColorId) -> String {
        Palette::standard().resolve(id).to_string()
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
        let svg = render(&t, &d, &ChartOptions::default());
        assert!(
            svg.starts_with("<svg "),
            "expected <svg ...>, got: {svg:.80}"
        );
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn contains_task_titles() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
        assert!(svg.contains("Design"));
        assert!(svg.contains("Build"));
    }

    #[test]
    fn contains_progress_polyline() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
        assert!(svg.contains("stroke-dasharray"));
    }

    #[test]
    fn dependency_arrow_ends_with_open_chevron() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
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
        let svg = render(&tasks, &deps, &ChartOptions::default());
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
        let svg = render(&t, &d, &ChartOptions::default());
        assert!(svg.contains(r#"data-task-id="task-1""#));
        assert!(svg.contains(r#"data-task-id="task-2""#));
    }

    #[test]
    fn today_marker_rendered_when_in_range() {
        let (t, d) = two_tasks();
        let today = date(2026, 6, 3);
        let svg = render(
            &t,
            &d,
            &ChartOptions {
                today: Some(today),
                ..Default::default()
            },
        );
        assert!(svg.contains(&palette_hex(ColorId::Today)));
    }

    #[test]
    fn today_marker_absent_when_none() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
        assert!(!svg.contains(&format!(
            r#"stroke="{}" stroke-width="2" stroke-dasharray="4,3""#,
            palette_hex(ColorId::Today)
        )));
    }

    #[test]
    fn date_header_present() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
        assert!(svg.contains(&palette_hex(ColorId::HeaderBg)));
    }

    #[test]
    fn parse_error_returns_safe_empty_svg() {
        let svg = render_svg(
            "not json",
            "[]",
            test_common::options_json(None, None, None),
        );
        assert_eq!(svg, crate::backend::svg::empty_svg());
        assert!(!svg.contains("parse error"));
        assert!(!svg.contains("<!--"));
    }

    #[test]
    fn parse_error_comment_injection_does_not_break_markup() {
        let payload = r#"not json --><img onerror=alert(1)><!--"#;
        let svg = render_svg(payload, "[]", test_common::options_json(None, None, None));
        assert_eq!(svg, crate::backend::svg::empty_svg());
        assert!(!svg.contains("<img"));
    }

    /// Named for the entry it actually exercises: `render()` takes typed structs
    /// and is not what `arc-vue` calls. The JSON entry has its own test below.
    #[test]
    fn native_id_xss_payload_escaped_in_output() {
        let tasks = vec![GanttTask {
            id: "x\" onmouseover=\"alert(1)\"".to_string(),
            title: "Safe".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 1),
            end: Some(date(2026, 6, 2)),
        }];
        let svg = render(&tasks, &[], &ChartOptions::default());
        assert!(svg.contains(r#"data-task-id="x&quot; onmouseover=&quot;alert(1)&quot;""#));
        assert!(!svg.contains(r#"onmouseover="alert"#));
    }

    /// The Wasm entry `arc-vue` really calls. The struct above arrives here as
    /// JSON, so the hostile id crosses `serde_json` before reaching the writer —
    /// a hop the native test does not take.
    #[test]
    fn wasm_entry_id_xss_payload_escaped_in_output() {
        let tasks_json = r#"[{"id":"x\" onmouseover=\"alert(1)\"","title":"Safe","progress_pct":0,"start":"2026-06-01","end":"2026-06-02"}]"#;
        let svg = render_svg(
            tasks_json,
            "[]",
            test_common::options_json(None, None, None),
        );
        assert_ne!(
            svg,
            crate::backend::svg::empty_svg(),
            "the payload must be rendered, not refused, or this proves nothing: {svg}"
        );
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
        let json = render_canvas_commands(
            "not json",
            "[]",
            test_common::options_json(None, None, None),
        );
        let v: serde_json::Value = serde_json::from_str(&json).expect("valid json");
        assert!(v.get("error").is_some());
    }

    #[test]
    fn raw_byte_limits_reject_tasks_before_deserialization() {
        // Deliberately invalid JSON: the byte-limit error proves parsing was not attempted.
        let oversized_tasks = "x".repeat(MAX_TASKS_JSON_BYTES + 1);
        let svg = render_svg(
            &oversized_tasks,
            "[]",
            test_common::options_json(None, None, None),
        );
        assert_eq!(svg, crate::backend::svg::empty_svg());

        let json = render_canvas_commands(
            &oversized_tasks,
            "[]",
            test_common::options_json(None, None, None),
        );
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
        let svg = render_svg(
            "[]",
            &oversized_deps,
            test_common::options_json(None, None, None),
        );
        assert_eq!(svg, crate::backend::svg::empty_svg());

        let json = render_canvas_commands(
            "[]",
            &oversized_deps,
            test_common::options_json(None, None, None),
        );
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

        let svg = render_svg(&tasks, "[]", test_common::options_json(None, None, None));
        assert_eq!(svg, crate::backend::svg::empty_svg());
        let json =
            render_canvas_commands(&tasks, "[]", test_common::options_json(None, None, None));
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
            test_common::options_json(None, None, None),
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
            test_common::options_json(None, None, None),
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

    // --- The canvas guard is never looser than the chart that is drawn ---

    /// What the guard sees of the end of the chart: nothing special, a milestone
    /// on the last day, or a one day bar that ends on it. The last two draw their
    /// percent label to the right of the days, so a chart without a legend is
    /// wider than its days.
    #[derive(Clone, Copy, Debug)]
    enum Tail {
        Plain,
        Milestone,
        OneDayBar,
    }

    const TAILS: [Tail; 3] = [Tail::Plain, Tail::Milestone, Tail::OneDayBar];

    /// `rows` tasks, one of which runs `span` days. With a tail, one more task
    /// (so `rows` tasks in all) ends on the last day with a 100% label.
    fn tasks_with_tail(rows: usize, span: i64, tail: Tail) -> Vec<GanttTask> {
        let mut tasks = canvas_tasks(rows, span);
        let last = tasks.last_mut().unwrap();
        match tail {
            Tail::Plain => {}
            Tail::Milestone => {
                last.start = date(2026, 1, 1) + Duration::days(span);
                last.end = Some(last.start);
                last.progress_pct = 100;
            }
            Tail::OneDayBar => {
                last.start = date(2026, 1, 1) + Duration::days(span - 1);
                last.end = Some(last.start + Duration::days(1));
                last.progress_pct = 100;
            }
        }
        tasks
    }

    fn legend_options(legend: LegendMode) -> Option<String> {
        (legend == LegendMode::Hidden).then(|| r#"{"legend":"hidden"}"#.to_string())
    }

    /// The width, height and area of the chart the canvas would be given,
    /// measured on the display list and not on the guard's own arithmetic.
    fn drawn_size(tasks: &[GanttTask], legend: LegendMode) -> (f64, f64) {
        let epoch = tasks.iter().map(|t| t.start).min().unwrap();
        let graph = GanttGraph {
            tasks: tasks.to_vec(),
            deps: vec![],
        };
        let list = build_display_list(
            &graph,
            epoch,
            &ChartOptions {
                legend,
                ..Default::default()
            },
        );
        (list.viewport.width, list.viewport.height)
    }

    fn canvas_accepts(tasks: &[GanttTask], legend: LegendMode) -> bool {
        let json = render_canvas_commands(
            &serde_json::to_string(tasks).unwrap(),
            "[]",
            legend_options(legend),
        );
        let value: serde_json::Value = serde_json::from_str(&json).unwrap();
        value.get("error").is_none()
    }

    #[test]
    fn the_canvas_guard_never_accepts_a_chart_larger_than_the_one_drawn() {
        let side = MAX_CANVAS_SIDE_PX as f64;
        let area = MAX_CANVAS_AREA_PX as f64;
        let (mut accepted, mut refused) = (0, 0);
        // The days limit and a few days either side of it: the labels push the
        // width of a chart without a legend over the side limit while its days
        // are still under it.
        for span in (MAX_CANVAS_DATE_SPAN_DAYS - 3)..=(MAX_CANVAS_DATE_SPAN_DAYS + 1) {
            for tail in TAILS {
                for legend in [LegendMode::Shown, LegendMode::Hidden] {
                    let tasks = tasks_with_tail(2, span, tail);
                    let (w, h) = drawn_size(&tasks, legend);
                    if canvas_accepts(&tasks, legend) {
                        accepted += 1;
                        assert!(w <= side, "{legend:?} {tail:?} span {span}: width {w}");
                        assert!(
                            h <= side && w * h <= area,
                            "{legend:?} {tail:?} span {span}"
                        );
                    } else {
                        refused += 1;
                    }
                }
            }
        }
        // A window that never tests both sides does not test the guard.
        assert!(accepted > 0 && refused > 0);
    }

    #[test]
    fn the_canvas_guard_counts_the_labels_that_run_past_the_days() {
        // The days fit, the milestone's label does not. The chart without a
        // legend is wider than its days, and the guard has to refuse it.
        let span = MAX_CANVAS_DATE_SPAN_DAYS;
        assert!(chart_width(span as f64) <= MAX_CANVAS_SIDE_PX as f64);
        let tasks = tasks_with_tail(2, span, Tail::Milestone);
        let (hidden_w, _) = drawn_size(&tasks, LegendMode::Hidden);
        assert!(
            hidden_w > MAX_CANVAS_SIDE_PX as f64,
            "the example is on the wrong side"
        );
        assert!(!canvas_accepts(&tasks, LegendMode::Hidden));
        // The same chart with a legend is drawn at its days, and is still accepted.
        let (shown_w, _) = drawn_size(&tasks, LegendMode::Shown);
        assert!(shown_w <= MAX_CANVAS_SIDE_PX as f64);
        assert!(canvas_accepts(&tasks, LegendMode::Shown));
        // One day less, the label fits too, and the chart without a legend is accepted.
        let shorter = tasks_with_tail(2, span - 1, Tail::Milestone);
        let (w, _) = drawn_size(&shorter, LegendMode::Hidden);
        assert!(w <= MAX_CANVAS_SIDE_PX as f64);
        assert!(canvas_accepts(&shorter, LegendMode::Hidden));
    }

    #[test]
    fn a_chart_with_a_legend_is_judged_by_its_days_whatever_its_labels_do() {
        // The guard of a chart with a legend is what it was before the legend
        // could be hidden: the days decide the side limit.
        for span in (MAX_CANVAS_DATE_SPAN_DAYS - 3)..=(MAX_CANVAS_DATE_SPAN_DAYS + 1) {
            for tail in TAILS {
                let tasks = tasks_with_tail(2, span, tail);
                assert_eq!(
                    canvas_accepts(&tasks, LegendMode::Shown),
                    span <= MAX_CANVAS_DATE_SPAN_DAYS,
                    "{tail:?} span {span}"
                );
            }
        }
    }

    #[test]
    fn the_canvas_area_guard_is_not_looser_than_the_chart_drawn_without_a_legend() {
        // A chart without a legend is 40px shorter than the guard's height and
        // wider than its days. Neither may let the area slip past the limit, at
        // any number of rows. Only the guard runs here, so many charts are tried.
        let area = MAX_CANVAS_AREA_PX as f64;
        let (mut accepted, mut refused) = (0, 0);
        for rows in (20..=MAX_CANVAS_ROWS).step_by(23).chain([MAX_CANVAS_ROWS]) {
            for span in 0..=MAX_CANVAS_DATE_SPAN_DAYS {
                // Only spans near the area limit can tell the guard apart.
                let days_w = chart_width(span as f64);
                if days_w * chart_height(rows as f64) < area * 0.97 {
                    continue;
                }
                for tail in TAILS {
                    let tasks = tasks_with_tail(rows, span, tail);
                    let (w, h) = drawn_size(&tasks, LegendMode::Hidden);
                    let verdict = canvas_graph_limit_error(&tasks, &[], LegendMode::Hidden);
                    if verdict.is_none() {
                        accepted += 1;
                        assert!(
                            w * h <= area,
                            "{rows} rows, {tail:?} span {span}: {w} x {h}"
                        );
                    } else {
                        refused += 1;
                    }
                }
                if days_w * chart_height(rows as f64) > area * 1.03 {
                    break;
                }
            }
        }
        assert!(accepted > 0 && refused > 0);
        // The height of a chart without a legend is shorter by the legend's.
        assert_eq!(
            drawn_size(&tasks_with_tail(5, 1, Tail::Plain), LegendMode::Hidden).1,
            drawn_size(&tasks_with_tail(5, 1, Tail::Plain), LegendMode::Shown).1 - LEGEND_H
        );
    }

    #[test]
    fn canvas_row_limit_is_derived_from_max_side() {
        let accepted_height = chart_height(MAX_CANVAS_ROWS as f64);
        let rejected_height = chart_height((MAX_CANVAS_ROWS + 1) as f64);
        assert!(accepted_height <= MAX_CANVAS_SIDE_PX as f64);
        assert!(rejected_height > MAX_CANVAS_SIDE_PX as f64);

        let accepted = canvas_tasks(MAX_CANVAS_ROWS, 1);
        let accepted_json = render_canvas_commands(
            &serde_json::to_string(&accepted).unwrap(),
            "[]",
            test_common::options_json(None, None, None),
        );
        let accepted_value: serde_json::Value = serde_json::from_str(&accepted_json).unwrap();
        assert!(accepted_value.get("error").is_none());
        assert!(accepted_value["viewport_height"].as_f64().unwrap() <= MAX_CANVAS_SIDE_PX as f64);

        let rejected = canvas_tasks(MAX_CANVAS_ROWS + 1, 1);
        let rejected_json = render_canvas_commands(
            &serde_json::to_string(&rejected).unwrap(),
            "[]",
            test_common::options_json(None, None, None),
        );
        let rejected_value: serde_json::Value = serde_json::from_str(&rejected_json).unwrap();
        assert!(rejected_value["error"]
            .as_str()
            .unwrap()
            .contains("canvas row count"));
    }

    #[test]
    fn canvas_date_limit_is_derived_from_max_side_and_does_not_reduce_svg_limit() {
        let accepted_width = chart_width(MAX_CANVAS_DATE_SPAN_DAYS as f64);
        let rejected_width = chart_width((MAX_CANVAS_DATE_SPAN_DAYS + 1) as f64);
        assert!(accepted_width <= MAX_CANVAS_SIDE_PX as f64);
        assert!(rejected_width > MAX_CANVAS_SIDE_PX as f64);

        let accepted = canvas_tasks(1, MAX_CANVAS_DATE_SPAN_DAYS);
        let accepted_tasks_json = serde_json::to_string(&accepted).unwrap();
        let accepted_json = render_canvas_commands(
            &accepted_tasks_json,
            "[]",
            test_common::options_json(None, None, None),
        );
        let accepted_value: serde_json::Value = serde_json::from_str(&accepted_json).unwrap();
        assert!(accepted_value.get("error").is_none());
        assert!(accepted_value["viewport_width"].as_f64().unwrap() <= MAX_CANVAS_SIDE_PX as f64);

        let rejected = canvas_tasks(1, MAX_CANVAS_DATE_SPAN_DAYS + 1);
        let rejected_tasks_json = serde_json::to_string(&rejected).unwrap();
        let rejected_json = render_canvas_commands(
            &rejected_tasks_json,
            "[]",
            test_common::options_json(None, None, None),
        );
        let rejected_value: serde_json::Value = serde_json::from_str(&rejected_json).unwrap();
        assert!(rejected_value["error"]
            .as_str()
            .unwrap()
            .contains("canvas date range"));

        let svg = render_svg(
            &rejected_tasks_json,
            "[]",
            test_common::options_json(None, None, None),
        );
        assert_ne!(svg, crate::backend::svg::empty_svg());
    }

    #[test]
    fn canvas_rejects_near_maximum_sides_when_area_is_too_large() {
        let tasks = canvas_tasks(MAX_CANVAS_ROWS, MAX_CANVAS_DATE_SPAN_DAYS);
        let width_px = chart_width(MAX_CANVAS_DATE_SPAN_DAYS as f64);
        let height_px = chart_height(MAX_CANVAS_ROWS as f64);
        assert!(width_px <= MAX_CANVAS_SIDE_PX as f64);
        assert!(height_px <= MAX_CANVAS_SIDE_PX as f64);
        assert!(width_px * height_px > 267_000_000.0);

        let json = render_canvas_commands(
            &serde_json::to_string(&tasks).unwrap(),
            "[]",
            test_common::options_json(None, None, None),
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
        let svg = render_svg(
            &tasks_json,
            "[]",
            test_common::options_json(None, None, None),
        );
        assert_eq!(svg, crate::backend::svg::empty_svg());
    }

    #[test]
    fn canvas_entry_rejects_maximum_start_date_with_null_end() {
        let tasks_json = max_date_tasks_json(NaiveDate::MAX, None);
        let json = render_canvas_commands(
            &tasks_json,
            "[]",
            test_common::options_json(None, None, None),
        );
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
        let svg = render_svg(
            &tasks_json,
            "[]",
            test_common::options_json(None, None, None),
        );
        assert_eq!(svg, crate::backend::svg::empty_svg());
    }

    #[test]
    fn canvas_entry_rejects_explicit_maximum_end_date() {
        let tasks_json = max_date_tasks_json(near_max_start(), Some(NaiveDate::MAX));
        let json = render_canvas_commands(
            &tasks_json,
            "[]",
            test_common::options_json(None, None, None),
        );
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
        let svg = render_svg(&at_limit, "[]", test_common::options_json(None, None, None));
        assert_ne!(svg, crate::backend::svg::empty_svg());

        let over_limit =
            max_date_tasks_json(start, Some(start + Duration::days(MAX_DATE_SPAN_DAYS + 1)));
        let svg = render_svg(
            &over_limit,
            "[]",
            test_common::options_json(None, None, None),
        );
        assert_eq!(svg, crate::backend::svg::empty_svg());
    }

    #[test]
    fn canvas_reports_date_span_overflow_instead_of_panicking() {
        let start = date(2026, 1, 1);
        let over_limit = max_date_tasks_json(
            start,
            Some(start + Duration::days(MAX_CANVAS_DATE_SPAN_DAYS + 1)),
        );
        let json = render_canvas_commands(
            &over_limit,
            "[]",
            test_common::options_json(None, None, None),
        );
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
        let parse = render_canvas_commands(
            "not json",
            "[]",
            test_common::options_json(None, None, None),
        );
        let v: serde_json::Value = serde_json::from_str(&parse).expect("valid json");
        assert_eq!(v["code"].as_str(), Some("parse_error"), "got: {parse:.200}");

        let oversized = "x".repeat(MAX_TASKS_JSON_BYTES + 1);
        let raw_limit = render_canvas_commands(
            &oversized,
            "[]",
            test_common::options_json(None, None, None),
        );
        let v: serde_json::Value = serde_json::from_str(&raw_limit).expect("valid json");
        assert_eq!(v["code"].as_str(), Some("input_limit"));

        let too_many = canvas_tasks(MAX_CANVAS_ROWS + 1, 1);
        let capacity = render_canvas_commands(
            &serde_json::to_string(&too_many).unwrap(),
            "[]",
            test_common::options_json(None, None, None),
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
            render_svg(&refused, "[]", test_common::options_json(None, None, None)),
            crate::backend::svg::empty_svg()
        );
        assert_eq!(
            render_svg("[]", "[]", test_common::options_json(None, None, None)),
            crate::backend::svg::empty_svg()
        );

        assert_eq!(
            render_svg_error("[]", "[]", test_common::options_json(None, None, None)),
            None,
            "nothing to draw is not an error"
        );

        let reported =
            render_svg_error(&refused, "[]", test_common::options_json(None, None, None))
                .expect("refusal is reported");
        let v: serde_json::Value = serde_json::from_str(&reported).expect("valid json");
        assert_eq!(v["code"].as_str(), Some("input_limit"));
        assert!(
            v["error"].as_str().unwrap().contains("date"),
            "got: {reported}"
        );
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
            render_svg(
                &tasks_json,
                "[]",
                test_common::options_json(None, None, None)
            ),
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
        let svg = render(&tasks, &[], &ChartOptions::default());
        assert!(svg.starts_with("<svg "));

        let no_end = vec![GanttTask {
            id: "native-max-null-end".to_string(),
            title: "Native max null end".to_string(),
            progress_pct: 0,
            start: NaiveDate::MAX,
            end: None,
        }];
        let svg = render(&no_end, &[], &ChartOptions::default());
        assert!(svg.starts_with("<svg "));
        let buffer = render_canvas(&no_end, &[], &ChartOptions::default());
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
        let svg = render(&tasks, &[], &ChartOptions::default());
        assert!(svg.contains("A &amp; B &lt; C"));
        assert!(!svg.contains("A & B"));
    }

    #[test]
    fn empty_tasks_returns_empty_svg() {
        let svg = render(&[], &[], &ChartOptions::default());
        assert!(svg.contains("width=\"0\""));
        assert!(svg.contains("viewBox=\"0 0 0 0\""));
        assert!(svg.contains(r#"role="img""#));
    }

    #[test]
    fn svg_has_viewbox_and_a11y() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
        assert!(svg.contains("viewBox="));
        assert!(svg.contains(r#"role="img""#));
        assert!(svg.contains(r#"aria-label="Gantt chart""#));
        assert!(svg.contains("<title>Gantt chart</title>"));
        assert!(svg.contains("<desc>"));
    }

    #[test]
    fn each_bar_shows_progress_percent() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
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
        let svg = render(&tasks, &[], &ChartOptions::default());
        assert!(svg.contains("0%"));
        assert!(svg.contains("100%"));
    }

    #[test]
    fn task_group_has_hover_title_tooltip() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
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
        let svg = render(&tasks, &[], &ChartOptions::default());
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
        let svg = render(&tasks, &[], &ChartOptions::default());
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
        let svg = render(&tasks, &[], &ChartOptions::default());
        assert!(svg.contains("<polygon"));
        assert!(svg.contains("0%"));
        assert!(!svg.contains(r#"width="0.0""#));
    }

    #[test]
    fn progress_line_legend_present() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
        assert!(svg.contains("progress-line-legend"));
        assert!(svg.contains("進捗ステータスライン"));
        assert!(svg.contains("bar-tier-legend"));
    }

    #[test]
    fn progress_line_legacy_when_today_absent() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, &ChartOptions::default());
        let poly = svg
            .split(r#"class="progress-status-line" points=""#)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("progress polyline");
        assert!(
            poly.starts_with("210,"),
            "legacy start at progress x, got: {poly}"
        );
    }

    #[test]
    fn progress_line_today_anchored_when_today_in_range() {
        let (t, d) = two_tasks();
        let today = date(2026, 6, 3);
        let svg = render(
            &t,
            &d,
            &ChartOptions {
                today: Some(today),
                ..Default::default()
            },
        );
        let poly = svg
            .split(r#"class="progress-status-line" points=""#)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("progress polyline");
        assert!(
            poly.starts_with("180,"),
            "anchored start at today x, got: {poly}"
        );
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
        let svg = render(
            &t,
            &d,
            &ChartOptions {
                today: Some(today),
                ..Default::default()
            },
        );
        let poly = svg
            .split(r#"class="progress-status-line" points=""#)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("progress polyline");
        assert!(
            poly.starts_with("210,"),
            "out-of-range today falls back to legacy"
        );
        assert!(!svg.contains(&format!(
            r#"stroke="{}" stroke-width="2" stroke-dasharray="4,3""#,
            palette_hex(ColorId::Today)
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
        let svg = render(&tasks, &[], &ChartOptions::default());
        assert!(svg.contains(&format!(r#"fill="{}""#, palette_hex(ColorId::TierLow))));
        assert!(svg.contains(&format!(r#"fill="{}""#, palette_hex(ColorId::TierMid))));
        assert!(svg.contains(&format!(r#"fill="{}""#, palette_hex(ColorId::TierHigh))));
        assert!(svg.contains(&format!(r#"fill="{}""#, palette_hex(ColorId::TierDone))));
        assert!(svg.contains(r#"class="bar-progress bar-tier-low""#));
        assert!(svg.contains(r#"class="bar-progress bar-tier-done""#));
        assert!(svg.contains(r#"class="bar-bg""#));
    }
}
