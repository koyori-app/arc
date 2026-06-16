use chrono::{Datelike, Duration, NaiveDate, Weekday};
use wasm_bindgen::prelude::*;

use crate::graph::{GanttDep, GanttGraph, GanttTask};
use crate::layout::assign_rows;
use crate::progress::progress_line;

const ROW_H: f64 = 40.0;
const BAR_H: f64 = 20.0;
const BAR_PAD: f64 = (ROW_H - BAR_H) / 2.0;
const PX_PER_DAY: f64 = 30.0;
const LABEL_W: f64 = 120.0;
const HEADER_H: f64 = 30.0;
/// Gap left between the arrow's end point and the blocked task's bar (frappe-gantt's literal `13`).
const ARROW_LEAD: f64 = 13.0;
/// Radius of the rounded elbow on dependency arrows (frappe-gantt's `arrow_curve`).
const ARROW_CURVE: f64 = 4.0;
/// Half-size of the open chevron arrowhead.
const ARROW_HEAD: f64 = 4.0;
/// Vertical gap between rows, used as frappe-gantt's `padding` for both the
/// row gap and the horizontal "is the blocked task too close/behind" threshold.
const ROW_PADDING: f64 = ROW_H - BAR_H;

const COLOR_BAR_BG: &str = "#d1d5db";
/// Achieved-portion fill by progress tier (low / mid / high / done).
const COLOR_TIER_LOW: &str = "#f59e0b";
const COLOR_TIER_MID: &str = "#6366f1";
const COLOR_TIER_HIGH: &str = "#0ea5e9";
const COLOR_TIER_DONE: &str = "#22c55e";
const COLOR_DEP: &str = "#9ca3af";
const COLOR_PROGRESS: &str = "#ef4444";
const COLOR_GRID: &str = "#e5e7eb";
const COLOR_TODAY: &str = "#f59e0b";
const COLOR_HEADER_BG: &str = "#f3f4f6";
const COLOR_GRID_LABEL: &str = "#6b7280";
const COLOR_PROGRESS_TEXT_ON_FG: &str = "#ffffff";
const COLOR_PROGRESS_TEXT_ON_BG: &str = "#374151";
const TITLE_MAX_CHARS: usize = 16;
const LEGEND_H: f64 = 40.0;

/// Native entry point — accepts typed structs directly.
pub fn render(tasks: &[GanttTask], deps: &[GanttDep], today: Option<NaiveDate>) -> String {
    if tasks.is_empty() {
        return empty_svg();
    }

    let epoch = tasks.iter().map(|t| t.start).min().unwrap();
    let graph = GanttGraph { tasks: tasks.to_vec(), deps: deps.to_vec() };
    render_graph(&graph, epoch, today)
}

/// Wasm entry point — accepts JSON strings matching the task project's API response shape.
/// `today_iso` is an optional ISO 8601 date string (e.g. "2026-06-16") for the today marker.
#[wasm_bindgen]
pub fn render_svg(tasks_json: &str, deps_json: &str, today_iso: Option<String>) -> String {
    let tasks: Vec<GanttTask> = match serde_json::from_str(tasks_json) {
        Ok(v) => v,
        Err(e) => return format!("<!-- parse error: {e} -->"),
    };
    let deps: Vec<GanttDep> = match serde_json::from_str(deps_json) {
        Ok(v) => v,
        Err(e) => return format!("<!-- parse error: {e} -->"),
    };
    let today = today_iso.and_then(|s| NaiveDate::parse_from_str(&s, "%Y-%m-%d").ok());
    render(&tasks, &deps, today)
}

fn render_graph(graph: &GanttGraph, epoch: NaiveDate, today: Option<NaiveDate>) -> String {
    let rows = assign_rows(&graph.tasks);

    let total_days = graph
        .tasks
        .iter()
        .map(|t| t.end_days(epoch))
        .fold(0.0_f64, f64::max)
        .ceil() as i64;

    let chart_w = total_days as f64 * PX_PER_DAY + LABEL_W + 20.0;
    let chart_h = rows.len() as f64 * ROW_H + HEADER_H + LEGEND_H + 10.0;

    let mut svg = svg_open(chart_w, chart_h);

    // Header background
    svg.push_str(&format!(
        r#"<rect x="{LABEL_W}" y="0" width="{gw}" height="{HEADER_H}" fill="{COLOR_HEADER_BG}"/>"#,
        gw = chart_w - LABEL_W,
    ));

    // Weekly grid lines + date labels
    // Find the first Monday on or after epoch
    let first_monday = {
        let mut d = epoch;
        while d.weekday() != Weekday::Mon {
            d += Duration::days(1);
        }
        d
    };
    let mut grid_day = first_monday;
    while (grid_day - epoch).num_days() <= total_days {
        let x = LABEL_W + (grid_day - epoch).num_days() as f64 * PX_PER_DAY;
        // Vertical grid line
        svg.push_str(&format!(
            r#"<line x1="{x}" y1="{HEADER_H}" x2="{x}" y2="{chart_h}" stroke="{COLOR_GRID}" stroke-width="1"/>"#
        ));
        // Date label in header
        let label = format!("{}/{}", grid_day.month(), grid_day.day());
        svg.push_str(&format!(
            r#"<text x="{x}" y="{ty}" fill="{COLOR_GRID_LABEL}" font-size="11">{label}</text>"#,
            ty = HEADER_H / 2.0 + 4.0,
        ));
        grid_day += Duration::weeks(1);
    }

    // Task bars + labels
    for (task, row) in graph.tasks.iter().zip(rows.iter()) {
        let y = HEADER_H + row.row as f64 * ROW_H + BAR_PAD;
        let x = LABEL_W + task.start_days(epoch) * PX_PER_DAY;
        let w = task_bar_width_days(task, epoch) * PX_PER_DAY;
        let prog = task.progress();
        let prog_w = w * prog;
        let prog_pct = task.progress_pct.clamp(0, 100);
        let tier = progress_tier(prog_pct);
        let fill_color = progress_fill_color(tier);
        let pct_label = format!("{prog_pct}%");
        let end_label = task
            .end
            .map(format_date)
            .unwrap_or_else(|| format_date(task.start + Duration::days(1)));
        let tooltip = format!(
            "{}: {} – {} ({pct_label})",
            task.title,
            format_date(task.start),
            end_label,
        );

        svg.push_str(&format!(
            r#"<g data-task-id="{id}">"#,
            id = escape_xml(&task.id),
        ));
        svg.push_str(&format!(
            r#"<title>{tooltip}</title>"#,
            tooltip = escape_xml(&tooltip),
        ));

        if is_zero_duration(task) {
            let cx = x;
            let cy = y + BAR_H / 2.0;
            let half = BAR_H / 2.0;
            svg.push_str(&format!(
                r#"<polygon class="bar-milestone bar-tier-{tier}" points="{cx},{top} {right},{cy} {cx},{bottom} {left},{cy}" fill="{fill_color}" stroke="{COLOR_BAR_BG}" stroke-width="1"/>"#,
                tier = tier.css_class(),
                fill_color = fill_color,
                top = cy - half,
                bottom = cy + half,
                right = cx + half,
                left = cx - half,
            ));
            svg.push_str(&format!(
                r#"<text x="{tx}" y="{ty}" fill="{COLOR_PROGRESS_TEXT_ON_BG}" font-size="11" font-weight="600" dominant-baseline="middle">{pct}</text>"#,
                tx = cx + half + 6.0,
                ty = cy,
                pct = escape_xml(&pct_label),
            ));
        } else {
            if w > 0.0 {
                svg.push_str(&format!(
                    r#"<rect class="bar-bg" x="{x}" y="{y}" width="{w}" height="{BAR_H}" rx="4" fill="{COLOR_BAR_BG}"/>"#
                ));
                if prog_w > 0.0 {
                    svg.push_str(&format!(
                        r#"<rect class="bar-progress bar-tier-{tier}" x="{x}" y="{y}" width="{prog_w}" height="{BAR_H}" rx="4" fill="{fill_color}"/>"#,
                        tier = tier.css_class(),
                        fill_color = fill_color,
                    ));
                }
            }
            let (tx, anchor, fill) = progress_label_style(x, w, prog_w, &pct_label);
            svg.push_str(&format!(
                r#"<text x="{tx}" y="{ty}" text-anchor="{anchor}" fill="{fill}" font-size="11" font-weight="600" dominant-baseline="middle">{pct}</text>"#,
                ty = y + BAR_H / 2.0,
                pct = escape_xml(&pct_label),
            ));
        }

        let display_title = truncate_title(&task.title, TITLE_MAX_CHARS);
        svg.push_str(&format!(
            r#"<text x="{lx}" y="{ty}" text-anchor="end" dominant-baseline="middle">{title}</text>"#,
            lx = LABEL_W - 4.0,
            ty = y + BAR_H / 2.0,
            title = escape_xml(&display_title),
        ));
        svg.push_str("</g>");
    }

    // Dependency arrows with arrowhead
    let row_map: std::collections::HashMap<&str, &crate::layout::RowLayout> =
        rows.iter().map(|r| (r.task_id.as_str(), r)).collect();
    let task_map: std::collections::HashMap<&str, &GanttTask> =
        graph.tasks.iter().map(|t| (t.id.as_str(), t)).collect();

    for dep in &graph.deps {
        let Some(from_t) = task_map.get(dep.blocker_task_id.as_str()) else { continue };
        let Some(to_t) = task_map.get(dep.blocked_task_id.as_str()) else { continue };
        let Some(from_r) = row_map.get(dep.blocker_task_id.as_str()) else { continue };
        let Some(to_r) = row_map.get(dep.blocked_task_id.as_str()) else { continue };

        // Ported from frappe-gantt's Arrow.calculate_path(): leave from the
        // bottom-center of the blocker bar (nudged left in 10px steps while
        // it overhangs the blocked bar's column), then either a simple
        // rounded elbow (blocked bar is comfortably to the right) or a
        // backward route that dips under the blocker and comes back up/down
        // into the blocked bar's left edge (blocked bar starts at/before the
        // blocker's left edge + padding).
        let from_x = LABEL_W + from_t.start_days(epoch) * PX_PER_DAY;
        let from_w = task_bar_width_days(from_t, epoch) * PX_PER_DAY;
        let to_x = LABEL_W + to_t.start_days(epoch) * PX_PER_DAY;

        let mut start_x = from_x + from_w / 2.0;
        while to_x < start_x + ROW_PADDING && start_x > from_x + ROW_PADDING {
            start_x -= 10.0;
        }
        start_x -= 10.0;

        let start_y = HEADER_H + from_r.row as f64 * ROW_H + BAR_PAD + BAR_H;
        let end_x = to_x - ARROW_LEAD;
        let end_y = HEADER_H + to_r.row as f64 * ROW_H + ROW_H / 2.0;

        let from_is_below_to = from_r.row > to_r.row;
        let mut curve = ARROW_CURVE;
        let clockwise = if from_is_below_to { 1 } else { 0 };
        let mut curve_y = if from_is_below_to { -curve } else { curve };

        let path = if to_x <= from_x + ROW_PADDING {
            let mut down_1 = ROW_PADDING / 2.0 - curve;
            if down_1 < 0.0 {
                down_1 = 0.0;
                curve = ROW_PADDING / 2.0;
                curve_y = if from_is_below_to { -curve } else { curve };
            }
            let down_2 = end_y - curve_y;
            let left = to_x - ROW_PADDING;
            let neg_curve = -curve;
            format!(
                "M {start_x} {start_y} v {down_1} a {curve} {curve} 0 0 1 {neg_curve} {curve} H {left} a {curve} {curve} 0 0 {clockwise} {neg_curve} {curve_y} V {down_2} a {curve} {curve} 0 0 {clockwise} {curve} {curve_y} L {end_x} {end_y} m -{ARROW_HEAD} -{ARROW_HEAD} l {ARROW_HEAD} {ARROW_HEAD} l -{ARROW_HEAD} {ARROW_HEAD}"
            )
        } else {
            if end_x < start_x + curve {
                curve = end_x - start_x;
            }
            let offset = if from_is_below_to { end_y + curve } else { end_y - curve };
            format!(
                "M {start_x} {start_y} V {offset} a {curve} {curve} 0 0 {clockwise} {curve} {curve_y} L {end_x} {end_y} m -{ARROW_HEAD} -{ARROW_HEAD} l {ARROW_HEAD} {ARROW_HEAD} l -{ARROW_HEAD} {ARROW_HEAD}"
            )
        };

        svg.push_str(&format!(
            r#"<path d="{path}" fill="none" stroke="{COLOR_DEP}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#
        ));
    }

    // Today marker + progress-line anchor share the same in-range x.
    let today_x = today.and_then(|today| {
        let x = LABEL_W + (today - epoch).num_days() as f64 * PX_PER_DAY;
        if x >= LABEL_W && x <= chart_w {
            Some(x)
        } else {
            None
        }
    });
    if let Some(x) = today_x {
        svg.push_str(&format!(
            r#"<line x1="{x}" y1="0" x2="{x}" y2="{chart_h}" stroke="{COLOR_TODAY}" stroke-width="2" stroke-dasharray="4,3"/>"#
        ));
    }

    // Progress line
    let prog_input: Vec<(f64, f64, f64, f64, f64)> = graph
        .tasks
        .iter()
        .zip(rows.iter())
        .map(|(t, r)| {
            let y_top = HEADER_H + r.row as f64 * ROW_H;
            let y_bot = y_top + ROW_H;
            (
                LABEL_W + t.start_days(epoch) * PX_PER_DAY,
                LABEL_W + t.end_days(epoch) * PX_PER_DAY,
                y_top,
                y_bot,
                t.progress(),
            )
        })
        .collect();

    let pts = progress_line(&prog_input, today_x);
    if pts.len() >= 2 {
        let pts_str: String =
            pts.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join(" ");
        svg.push_str(&format!(
            r#"<polyline class="progress-status-line" points="{pts_str}" fill="none" stroke="{COLOR_PROGRESS}" stroke-width="2" stroke-dasharray="6,3"/>"#
        ));
    }

    // Legend row 1: progress status line (schedule trajectory — preserved design).
    let legend_y1 = chart_h - LEGEND_H + 14.0;
    let legend_x = LABEL_W + 8.0;
    svg.push_str(&format!(
        r#"<g class="progress-line-legend" aria-hidden="true"><line x1="{legend_x}" y1="{legend_y1}" x2="{lx2}" y2="{legend_y1}" stroke="{COLOR_PROGRESS}" stroke-width="2" stroke-dasharray="6,3"/><text x="{tx}" y="{ty}" fill="{COLOR_GRID_LABEL}" font-size="10" dominant-baseline="middle">進捗ステータスライン（各タスクの完了位置を結ぶ）</text></g>"#,
        lx2 = legend_x + 28.0,
        tx = legend_x + 34.0,
        ty = legend_y1 + 4.0,
    ));

    // Legend row 2: achieved vs remaining + progress tier swatches.
    let legend_y2 = chart_h - 10.0;
    let sw = 10.0;
    let gap = 4.0;
    let mut lx = legend_x;
    let tier_items: [(&str, &str); 5] = [
        ("未達", COLOR_BAR_BG),
        ("低", COLOR_TIER_LOW),
        ("中", COLOR_TIER_MID),
        ("高", COLOR_TIER_HIGH),
        ("完了", COLOR_TIER_DONE),
    ];
    svg.push_str(r#"<g class="bar-tier-legend" aria-hidden="true">"#);
    for (label, color) in tier_items {
        svg.push_str(&format!(
            r#"<rect x="{lx}" y="{ly}" width="{sw}" height="{sw}" rx="2" fill="{color}"/>"#,
            ly = legend_y2 - sw / 2.0,
            color = color,
        ));
        lx += sw + 2.0;
        svg.push_str(&format!(
            r#"<text x="{lx}" y="{ty}" fill="{COLOR_GRID_LABEL}" font-size="9" dominant-baseline="middle">{label}</text>"#,
            ty = legend_y2 + 3.0,
            label = label,
        ));
        lx += label.len() as f64 * 9.0 + gap + sw;
    }
    svg.push_str("</g>");

    svg.push_str("</svg>");
    svg
}

fn empty_svg() -> String {
    r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0" viewBox="0 0 0 0" role="img" aria-label="Empty Gantt chart" font-family="sans-serif" font-size="12"><title>Empty Gantt chart</title><desc>No tasks to display</desc></svg>"#.to_string()
}

fn svg_open(chart_w: f64, chart_h: f64) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{chart_w}" height="{chart_h}" viewBox="0 0 {chart_w} {chart_h}" role="img" aria-label="Gantt chart" font-family="sans-serif" font-size="12"><title>Gantt chart</title><desc>Task schedule with progress bars and dependency arrows</desc>"#
    )
}

fn format_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn truncate_title(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

/// Bar span in days; clamps negative ranges (end &lt; start) to zero width.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ProgressTier {
    None,
    Low,
    Mid,
    High,
    Done,
}

impl ProgressTier {
    fn css_class(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Mid => "mid",
            Self::High => "high",
            Self::Done => "done",
        }
    }
}

fn progress_tier(pct: i16) -> ProgressTier {
    match pct {
        0 => ProgressTier::None,
        1..=33 => ProgressTier::Low,
        34..=66 => ProgressTier::Mid,
        67..=99 => ProgressTier::High,
        _ => ProgressTier::Done,
    }
}

fn progress_fill_color(tier: ProgressTier) -> &'static str {
    match tier {
        ProgressTier::None => COLOR_BAR_BG,
        ProgressTier::Low => COLOR_TIER_LOW,
        ProgressTier::Mid => COLOR_TIER_MID,
        ProgressTier::High => COLOR_TIER_HIGH,
        ProgressTier::Done => COLOR_TIER_DONE,
    }
}

fn task_bar_width_days(task: &GanttTask, epoch: NaiveDate) -> f64 {
    (task.end_days(epoch) - task.start_days(epoch)).max(0.0)
}

fn is_zero_duration(task: &GanttTask) -> bool {
    task.end.is_some_and(|end| end == task.start)
}

fn progress_label_style(x: f64, w: f64, prog_w: f64, label: &str) -> (f64, &'static str, &'static str) {
    let approx_text_w = label.len() as f64 * 6.5;
    if w >= approx_text_w + 8.0 {
        let on_fg = prog_w >= w * 0.45;
        let fill = if on_fg {
            COLOR_PROGRESS_TEXT_ON_FG
        } else {
            COLOR_PROGRESS_TEXT_ON_BG
        };
        (x + w / 2.0, "middle", fill)
    } else {
        (x + w.max(0.0) + 4.0, "start", COLOR_PROGRESS_TEXT_ON_BG)
    }
}

fn escape_xml(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

#[cfg(test)]
mod tests {
    use super::*;
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
        let svg = render(&t, &d, None);
        assert!(svg.starts_with("<svg "), "expected <svg ...>, got: {svg:.80}");
        assert!(svg.ends_with("</svg>"));
    }

    #[test]
    fn contains_task_titles() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
        assert!(svg.contains("Design"));
        assert!(svg.contains("Build"));
    }

    #[test]
    fn contains_progress_polyline() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
        assert!(svg.contains("stroke-dasharray"));
    }

    #[test]
    fn dependency_arrow_ends_with_open_chevron() {
        // task-1 ends exactly when task-2 starts (x1 == x2). The arrow is now
        // a frappe-gantt-style elbow path with an open chevron drawn via
        // relative move/line commands, not a <marker>/<polyline> pair.
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
        assert!(svg.contains("<path d=\"M "));
        assert!(svg.contains(&format!("m -{ARROW_HEAD} -{ARROW_HEAD}")));
        assert!(!svg.contains("<marker"));
    }

    #[test]
    fn dependency_arrow_routes_backward_when_blocked_starts_before_blocker_exit() {
        // task-2 starts only 1 day after task-1 but task-1 spans 3 days, so
        // the natural exit point (task-1's bar center) sits to the right of
        // where the arrow must enter task-2 — this used to collapse the
        // rounded elbow into a flat right-angle corner (curve == 0).
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
        let svg = render(&tasks, &deps, None);
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
        let svg = render(&t, &d, None);
        assert!(svg.contains(r#"data-task-id="task-1""#));
        assert!(svg.contains(r#"data-task-id="task-2""#));
    }

    #[test]
    fn today_marker_rendered_when_in_range() {
        let (t, d) = two_tasks();
        let today = date(2026, 6, 3);
        let svg = render(&t, &d, Some(today));
        assert!(svg.contains(COLOR_TODAY));
    }

    #[test]
    fn today_marker_absent_when_none() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
        assert!(!svg.contains(&format!(
            r#"stroke="{COLOR_TODAY}" stroke-width="2" stroke-dasharray="4,3""#
        )));
    }

    #[test]
    fn date_header_present() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
        assert!(svg.contains(COLOR_HEADER_BG));
    }

    #[test]
    fn parse_error_returns_comment() {
        let svg = render_svg("not json", "[]", None);
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
        let svg = render(&tasks, &[], None);
        assert!(svg.contains("A &amp; B &lt; C"));
        assert!(!svg.contains("A & B"));
    }

    #[test]
    fn empty_tasks_returns_empty_svg() {
        let svg = render(&[], &[], None);
        assert!(svg.contains("width=\"0\""));
        assert!(svg.contains("viewBox=\"0 0 0 0\""));
        assert!(svg.contains(r#"role="img""#));
    }

    #[test]
    fn svg_has_viewbox_and_a11y() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
        assert!(svg.contains("viewBox="));
        assert!(svg.contains(r#"role="img""#));
        assert!(svg.contains(r#"aria-label="Gantt chart""#));
        assert!(svg.contains("<title>Gantt chart</title>"));
        assert!(svg.contains("<desc>"));
    }

    #[test]
    fn each_bar_shows_progress_percent() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
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
        let svg = render(&tasks, &[], None);
        assert!(svg.contains("0%"));
        assert!(svg.contains("100%"));
    }

    #[test]
    fn task_group_has_hover_title_tooltip() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
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
        let svg = render(&tasks, &[], None);
        assert!(svg.contains("Very Long Task …"));
        // Full title remains in hover tooltip only.
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
        let svg = render(&tasks, &[], None);
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
        let svg = render(&tasks, &[], None);
        assert!(svg.contains("<polygon"));
        assert!(svg.contains("0%"));
        assert!(!svg.contains(r#"width="0.0""#));
    }

    #[test]
    fn progress_line_legend_present() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
        assert!(svg.contains("progress-line-legend"));
        assert!(svg.contains("進捗ステータスライン"));
        assert!(svg.contains("bar-tier-legend"));
    }

    #[test]
    fn progress_line_legacy_when_today_absent() {
        let (t, d) = two_tasks();
        let svg = render(&t, &d, None);
        let poly = svg
            .split(r#"class="progress-status-line" points=""#)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("progress polyline");
        // Legacy: first point x is first task progress (100% of task-1 → end of bar).
        assert!(poly.starts_with("210,"), "legacy start at progress x, got: {poly}");
    }

    #[test]
    fn progress_line_today_anchored_when_today_in_range() {
        let (t, d) = two_tasks();
        let today = date(2026, 6, 3);
        let svg = render(&t, &d, Some(today));
        let poly = svg
            .split(r#"class="progress-status-line" points=""#)
            .nth(1)
            .and_then(|s| s.split('"').next())
            .expect("progress polyline");
        // today x = LABEL_W(120) + 2 days * 30 = 180
        assert!(poly.starts_with("180,"), "anchored start at today x, got: {poly}");
        let last_pt = poly.split(' ').next_back().expect("last point");
        assert!(
            last_pt.starts_with("180,"),
            "anchored end at today x, got last: {last_pt}"
        );
        // Tier fills and progress line coexist.
        assert!(svg.contains(r#"class="bar-progress bar-tier-done""#));
        assert!(svg.contains(r#"class="bar-progress bar-tier-mid""#));
    }

    #[test]
    fn progress_line_legacy_when_today_out_of_range() {
        let (t, d) = two_tasks();
        let today = date(2020, 1, 1);
        let svg = render(&t, &d, Some(today));
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
        let svg = render(&tasks, &[], None);
        assert!(svg.contains(&format!(r#"fill="{COLOR_TIER_LOW}""#)));
        assert!(svg.contains(&format!(r#"fill="{COLOR_TIER_MID}""#)));
        assert!(svg.contains(&format!(r#"fill="{COLOR_TIER_HIGH}""#)));
        assert!(svg.contains(&format!(r#"fill="{COLOR_TIER_DONE}""#)));
        assert!(svg.contains(r#"class="bar-progress bar-tier-low""#));
        assert!(svg.contains(r#"class="bar-progress bar-tier-done""#));
        // Achieved vs remaining: background gray behind colored progress rects.
        assert!(svg.contains(r#"class="bar-bg""#));
    }
}
