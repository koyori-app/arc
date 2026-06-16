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
/// Gap left between the arrow's end point and the blocked task's bar.
const ARROW_LEAD: f64 = 10.0;
/// Radius of the rounded elbow on dependency arrows (frappe-gantt's `arrow_curve`).
const ARROW_CURVE: f64 = 4.0;
/// Half-size of the open chevron arrowhead.
const ARROW_HEAD: f64 = 4.0;

const COLOR_BAR_BG: &str = "#d1d5db";
const COLOR_BAR_FG: &str = "#6366f1";
const COLOR_DEP: &str = "#9ca3af";
const COLOR_PROGRESS: &str = "#ef4444";
const COLOR_GRID: &str = "#e5e7eb";
const COLOR_TODAY: &str = "#f59e0b";
const COLOR_HEADER_BG: &str = "#f3f4f6";
const COLOR_GRID_LABEL: &str = "#6b7280";

/// Native entry point — accepts typed structs directly.
pub fn render(tasks: &[GanttTask], deps: &[GanttDep], today: Option<NaiveDate>) -> String {
    if tasks.is_empty() {
        return r#"<svg xmlns="http://www.w3.org/2000/svg" width="0" height="0"/>"#.to_string();
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
    let chart_h = rows.len() as f64 * ROW_H + HEADER_H + 10.0;

    let mut svg = format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{chart_w}" height="{chart_h}" font-family="sans-serif" font-size="12">"#
    );

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
        let w = (task.end_days(epoch) - task.start_days(epoch)) * PX_PER_DAY;
        let prog_w = w * task.progress();

        svg.push_str(&format!(
            r#"<g data-task-id="{id}">"#,
            id = escape_xml(&task.id),
        ));
        svg.push_str(&format!(
            r#"<rect x="{x}" y="{y}" width="{w}" height="{BAR_H}" rx="4" fill="{COLOR_BAR_BG}"/>"#
        ));
        if prog_w > 0.0 {
            svg.push_str(&format!(
                r#"<rect x="{x}" y="{y}" width="{prog_w}" height="{BAR_H}" rx="4" fill="{COLOR_BAR_FG}"/>"#
            ));
        }
        svg.push_str(&format!(
            r#"<text x="{lx}" y="{ty}" text-anchor="end" dominant-baseline="middle">{title}</text>"#,
            lx = LABEL_W - 4.0,
            ty = y + BAR_H / 2.0,
            title = escape_xml(&task.title),
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

        // frappe-gantt style elbow: leave from the bottom-center of the
        // blocker bar, drop/rise to just shy of the blocked row, round the
        // corner, then run a short straight stretch into the blocked bar's
        // vertical center (open chevron arrowhead, no filled marker).
        let from_x = LABEL_W + from_t.start_days(epoch) * PX_PER_DAY;
        let from_w = (from_t.end_days(epoch) - from_t.start_days(epoch)) * PX_PER_DAY;
        let start_x = from_x + from_w / 2.0;
        let start_y = HEADER_H + from_r.row as f64 * ROW_H + BAR_PAD + BAR_H;
        let end_x = LABEL_W + to_t.start_days(epoch) * PX_PER_DAY - ARROW_LEAD;
        let end_y = HEADER_H + to_r.row as f64 * ROW_H + ROW_H / 2.0;

        let curve = if end_x < start_x + ARROW_CURVE {
            (end_x - start_x).max(0.0)
        } else {
            ARROW_CURVE
        };
        let start_x = start_x.min(end_x - curve);

        let from_is_below_to = from_r.row > to_r.row;
        let offset = if from_is_below_to { end_y + curve } else { end_y - curve };
        let clockwise = if from_is_below_to { 1 } else { 0 };
        // Vertical delta of the rounded corner must point toward end_y: down
        // when the blocker is above (offset = end_y - curve), up when below.
        let dy = if from_is_below_to { -curve } else { curve };

        svg.push_str(&format!(
            r#"<path d="M {start_x} {start_y} V {offset} a {curve} {curve} 0 0 {clockwise} {curve} {dy} L {end_x} {end_y} m -{ARROW_HEAD} -{ARROW_HEAD} l {ARROW_HEAD} {ARROW_HEAD} l -{ARROW_HEAD} {ARROW_HEAD}" fill="none" stroke="{COLOR_DEP}" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>"#
        ));
    }

    // Today marker
    if let Some(today) = today {
        let x = LABEL_W + (today - epoch).num_days() as f64 * PX_PER_DAY;
        if x >= LABEL_W && x <= chart_w {
            svg.push_str(&format!(
                r#"<line x1="{x}" y1="0" x2="{x}" y2="{chart_h}" stroke="{COLOR_TODAY}" stroke-width="2" stroke-dasharray="4,3"/>"#
            ));
        }
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

    let pts = progress_line(&prog_input);
    if pts.len() >= 2 {
        let pts_str: String =
            pts.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join(" ");
        svg.push_str(&format!(
            r#"<polyline points="{pts_str}" fill="none" stroke="{COLOR_PROGRESS}" stroke-width="2" stroke-dasharray="6,3"/>"#
        ));
    }

    svg.push_str("</svg>");
    svg
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
        assert!(!svg.contains(COLOR_TODAY));
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
    }
}
