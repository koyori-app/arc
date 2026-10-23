use chrono::{Datelike, Duration, NaiveDate, Weekday};
use std::collections::HashMap;

use crate::graph::{GanttGraph, GanttTask};
use crate::layout::assign_rows;
use crate::progress::progress_line;

use super::constants::*;
use super::legend::{tier_fill_color, PROGRESS_LABEL, TIER_ITEMS};
use super::text_metrics::{text_width_px, text_width_weighted_px, truncate_to_width};
use super::types::*;

/// Compute inclusive row index window `[first, last]` for virtualization.
/// Returns `None` when all rows should be rendered (`scroll_viewport` absent).
pub fn compute_row_window(
    scroll_viewport: Option<ScrollViewport>,
    total_rows: usize,
) -> Option<(usize, usize)> {
    let scroll = scroll_viewport?;
    if total_rows == 0 {
        return Some((0, 0));
    }
    let max_row = total_rows - 1;
    let first_visible = ((scroll.scroll_y - HEADER_H).max(0.0) / ROW_H).floor() as usize;
    let last_visible =
        ((scroll.scroll_y + scroll.client_height - HEADER_H).max(0.0) / ROW_H).ceil() as usize;
    let first = first_visible.saturating_sub(ROW_BUFFER as usize);
    let last = (last_visible + ROW_BUFFER as usize).min(max_row);
    Some((first, last))
}

fn row_in_window(row: usize, window: Option<(usize, usize)>) -> bool {
    match window {
        None => true,
        Some((first, last)) => row >= first && row <= last,
    }
}

fn dep_incident_to_window(from_row: usize, to_row: usize, window: Option<(usize, usize)>) -> bool {
    match window {
        None => true,
        Some((first, last)) => {
            (from_row >= first && from_row <= last) || (to_row >= first && to_row <= last)
        }
    }
}

pub fn build_display_list(
    graph: &GanttGraph,
    epoch: NaiveDate,
    options: &ChartOptions,
) -> DisplayList {
    let ChartOptions {
        today,
        scroll_viewport,
        legend: legend_mode,
    } = *options;
    let rows = assign_rows(&graph.tasks, &graph.deps);

    let total_days = total_days_of(&graph.tasks, epoch);

    // The legend's width does not depend on how many days there are, so a short
    // span would otherwise leave it running past the right edge of the viewBox.
    let legend = (legend_mode == LegendMode::Shown).then(legend_layout);
    let chart_w = chart_width_for(&graph.tasks, epoch, legend_mode);
    let chart_h = chart_height_with(rows.len() as f64, legend_mode);

    let viewport = Viewport {
        width: chart_w,
        height: chart_h,
        label_width: LABEL_W,
        header_height: HEADER_H,
        row_height: ROW_H,
    };

    let row_window = compute_row_window(scroll_viewport, rows.len());

    let mut layers = Vec::new();
    let mut task_bboxes = Vec::new();

    // Background layer
    layers.push(Layer {
        kind: LayerKind::Background,
        primitives: vec![Primitive::Rect(RectPrim {
            x: LABEL_W,
            y: 0.0,
            width: chart_w - LABEL_W,
            height: HEADER_H,
            fill: ColorId::HeaderBg,
            rx: None,
            semantic: RectSemantic::HeaderBackground,
        })],
    });

    // Grid layer
    let mut grid_prims = Vec::new();
    // The Wasm entry points reject dates without headroom below `NaiveDate::MAX`,
    // but the native entry points have no such guard, so every step here is
    // checked: an overflow stops the grid instead of panicking.
    for grid_day in grid_days(epoch, total_days) {
        let x = LABEL_W + (grid_day - epoch).num_days() as f64 * PX_PER_DAY;
        grid_prims.push(Primitive::Line(LinePrim {
            x1: x,
            y1: HEADER_H,
            x2: x,
            y2: chart_h,
            stroke: ColorId::Grid,
            stroke_width: 1.0,
            stroke_dash: None,
            semantic: LineSemantic::Grid,
        }));
        let label = grid_label(grid_day);
        grid_prims.push(Primitive::Text(TextPrim {
            x,
            y: HEADER_H / 2.0 + 4.0,
            content: label,
            fill: Some(ColorId::GridLabel),
            font_size: Some(GRID_LABEL_FONT_PX),
            font_weight: None,
            anchor: None,
            baseline: TextBaseline::Auto,
            semantic: TextSemantic::GridLabel,
        }));
    }
    layers.push(Layer {
        kind: LayerKind::Grid,
        primitives: grid_prims,
    });

    // Bars layer (row-virtualized when scroll_viewport is set)
    let mut bar_prims = Vec::new();
    for (task, row) in graph.tasks.iter().zip(rows.iter()) {
        if !row_in_window(row.row, row_window) {
            continue;
        }
        let y = HEADER_H + row.row as f64 * ROW_H + BAR_PAD;
        let x = LABEL_W + task.start_days(epoch) * PX_PER_DAY;
        let w = task_bar_width_days(task, epoch) * PX_PER_DAY;
        let prog = task.progress();
        let prog_w = w * prog;
        let prog_pct = task.progress_pct.clamp(0, 100);
        let tier = ProgressTier::from_pct(prog_pct);
        let pct_label = format!("{prog_pct}%");
        let end_label = task.end.map(format_date).unwrap_or_else(|| {
            format_date(
                task.start
                    .checked_add_signed(Duration::days(1))
                    .unwrap_or(task.start),
            )
        });
        let tooltip = format!(
            "{}: {} – {} ({pct_label})",
            task.title,
            format_date(task.start),
            end_label,
        );

        let mut children = Vec::new();

        if is_zero_duration(task) {
            let cx = x;
            let cy = y + BAR_H / 2.0;
            let half = BAR_H / 2.0;
            children.push(Primitive::Polygon(PolygonPrim {
                points: vec![
                    (cx, cy - half),
                    (cx + half, cy),
                    (cx, cy + half),
                    (cx - half, cy),
                ],
                fill: tier_fill_color(tier),
                stroke: ColorId::BarBg,
                stroke_width: 1.0,
                tier,
                semantic: PolygonSemantic::Milestone,
            }));
            let label = percent_label(task, epoch);
            children.push(Primitive::Text(TextPrim {
                x: label.x,
                y: cy,
                content: label.content,
                fill: Some(label.fill),
                font_size: Some(PROGRESS_LABEL_FONT_PX),
                font_weight: Some(PROGRESS_LABEL_FONT_WEIGHT),
                anchor: label.anchor,
                baseline: TextBaseline::Middle,
                semantic: TextSemantic::ProgressPercent,
            }));
        } else {
            if w > 0.0 {
                children.push(Primitive::Rect(RectPrim {
                    x,
                    y,
                    width: w,
                    height: BAR_H,
                    fill: ColorId::BarBg,
                    rx: Some(BAR_CORNER_RADIUS_PX),
                    semantic: RectSemantic::BarBackground,
                }));
                if prog_w > 0.0 {
                    children.push(Primitive::RoundRect(RoundRectPrim {
                        x,
                        y,
                        width: prog_w,
                        height: BAR_H,
                        fill: tier_fill_color(tier),
                        rx: BAR_CORNER_RADIUS_PX,
                        tier,
                        semantic: RoundRectSemantic::BarProgress,
                    }));
                }
            }
            let label = percent_label(task, epoch);
            children.push(Primitive::Text(TextPrim {
                x: label.x,
                y: y + BAR_H / 2.0,
                content: label.content,
                fill: Some(label.fill),
                font_size: Some(PROGRESS_LABEL_FONT_PX),
                font_weight: Some(PROGRESS_LABEL_FONT_WEIGHT),
                anchor: label.anchor,
                baseline: TextBaseline::Middle,
                semantic: TextSemantic::ProgressPercent,
            }));
        }

        let display_title =
            truncate_to_width(&task.title, DEFAULT_FONT_SIZE_PX, ROW_LABEL_MAX_W_PX);
        let label_x = LABEL_W - LABEL_GAP_PX;
        let hit_left = label_x - LABEL_HIT_W;
        children.push(Primitive::Text(TextPrim {
            x: label_x,
            y: y + BAR_H / 2.0,
            content: display_title,
            fill: None,
            font_size: None,
            font_weight: None,
            anchor: Some(TextAnchor::End),
            baseline: TextBaseline::Middle,
            semantic: TextSemantic::RowLabel,
        }));

        let bar_bbox = BBox {
            x,
            y,
            width: w.max(BAR_H),
            height: BAR_H,
        };
        let group_bbox = BBox {
            x: hit_left,
            y,
            width: x + w.max(BAR_H) - hit_left,
            height: BAR_H,
        };
        task_bboxes.push(TaskBBox {
            task_id: task.id.clone(),
            row: row.row as u32,
            bbox: group_bbox.clone(),
            bar_bbox,
        });

        bar_prims.push(Primitive::Group(GroupPrim {
            task_id: Some(task.id.clone()),
            tooltip: Some(tooltip),
            bbox: group_bbox,
            children,
        }));
    }
    layers.push(Layer {
        kind: LayerKind::Bars,
        primitives: bar_prims,
    });

    // Dependencies layer
    // First occurrence wins for a repeated id, the same rule `layout::assign_rows`
    // uses to place the dependency edges. `collect()` into a map keeps the last
    // one, which would draw an arrow from a different bar than the row it was
    // laid out for.
    let mut row_map: HashMap<&str, &crate::layout::RowLayout> = HashMap::new();
    for r in &rows {
        row_map.entry(r.task_id.as_str()).or_insert(r);
    }
    let mut task_map: HashMap<&str, &GanttTask> = HashMap::new();
    for t in &graph.tasks {
        task_map.entry(t.id.as_str()).or_insert(t);
    }

    let mut dep_prims = Vec::new();
    for dep in &graph.deps {
        let Some(from_t) = task_map.get(dep.blocker_task_id.as_str()) else {
            continue;
        };
        let Some(to_t) = task_map.get(dep.blocked_task_id.as_str()) else {
            continue;
        };
        let Some(from_r) = row_map.get(dep.blocker_task_id.as_str()) else {
            continue;
        };
        let Some(to_r) = row_map.get(dep.blocked_task_id.as_str()) else {
            continue;
        };
        if !dep_incident_to_window(from_r.row, to_r.row, row_window) {
            continue;
        }

        let from_x = LABEL_W + from_t.start_days(epoch) * PX_PER_DAY;
        let from_w = task_bar_width_days(from_t, epoch) * PX_PER_DAY;
        let to_x = LABEL_W + to_t.start_days(epoch) * PX_PER_DAY;

        let mut start_x = from_x + from_w / 2.0;
        while to_x < start_x + ROW_PADDING && start_x > from_x + ROW_PADDING {
            start_x -= DEP_BACKOFF_STEP_PX;
        }
        start_x -= DEP_BACKOFF_STEP_PX;

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
            let offset = if from_is_below_to {
                end_y + curve
            } else {
                end_y - curve
            };
            format!(
                "M {start_x} {start_y} V {offset} a {curve} {curve} 0 0 {clockwise} {curve} {curve_y} L {end_x} {end_y} m -{ARROW_HEAD} -{ARROW_HEAD} l {ARROW_HEAD} {ARROW_HEAD} l -{ARROW_HEAD} {ARROW_HEAD}"
            )
        };

        dep_prims.push(Primitive::Path(PathPrim {
            d: path,
            stroke: ColorId::Dep,
            stroke_width: 1.5,
            semantic: PathSemantic::DependencyArrow,
        }));
    }
    layers.push(Layer {
        kind: LayerKind::Dependencies,
        primitives: dep_prims,
    });

    // Today marker
    let today_x = today.and_then(|today| {
        let x = LABEL_W + (today - epoch).num_days() as f64 * PX_PER_DAY;
        if x >= LABEL_W && x <= chart_w {
            Some(x)
        } else {
            None
        }
    });
    if let Some(x) = today_x {
        layers.push(Layer {
            kind: LayerKind::TodayMarker,
            primitives: vec![Primitive::Line(LinePrim {
                x1: x,
                y1: 0.0,
                x2: x,
                y2: chart_h,
                stroke: ColorId::Today,
                stroke_width: 2.0,
                stroke_dash: Some("4,3".to_string()),
                semantic: LineSemantic::TodayMarker,
            })],
        });
    }

    // Progress line layer — delegates geometry to progress_line().
    // `rows` is in input order but the line runs top to bottom, so the points are
    // ordered by row. Otherwise a dependency that reorders rows makes the line
    // run backwards, and the today anchors grow from a row in the middle.
    let mut by_row: Vec<(usize, usize)> =
        rows.iter().enumerate().map(|(i, r)| (r.row, i)).collect();
    by_row.sort_unstable();
    let prog_input: Vec<(f64, f64, f64, f64, f64)> = by_row
        .iter()
        .map(|&(row, i)| {
            let t = &graph.tasks[i];
            let y_top = HEADER_H + row as f64 * ROW_H;
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
        layers.push(Layer {
            kind: LayerKind::ProgressLine,
            primitives: vec![Primitive::Polyline(PolylinePrim {
                points: pts,
                stroke: ColorId::Progress,
                stroke_width: PROGRESS_LINE_STROKE_W,
                stroke_dash: Some(PROGRESS_LINE_DASH.to_string()),
                semantic: PolylineSemantic::ProgressStatusLine,
            })],
        });
    }

    if let Some(legend) = &legend {
        layers.push(legend_layer(chart_h, legend));
    }

    let primitive_count = layers
        .iter()
        .map(|l| count_primitive_vec(&l.primitives))
        .sum();

    DisplayList {
        viewport,
        palette: Palette::standard(),
        layers,
        metadata: ChartMetadata {
            title: "Gantt chart".to_string(),
            description: "Task schedule with progress bars and dependency arrows".to_string(),
            task_bboxes,
            primitive_count,
            element_count_estimate: primitive_count,
        },
    }
}

fn format_date(d: NaiveDate) -> String {
    d.format("%Y-%m-%d").to_string()
}

fn task_bar_width_days(task: &GanttTask, epoch: NaiveDate) -> f64 {
    (task.end_days(epoch) - task.start_days(epoch)).max(0.0)
}

/// A task with no width is drawn as a diamond. That covers `end == start`, and
/// also `end < start`: `end_days` cuts the latter to `start`, so without this it
/// would have no bar at all and could not be seen or clicked.
fn is_zero_duration(task: &GanttTask) -> bool {
    task.end.is_some_and(|end| end <= task.start)
}

fn total_days_of(tasks: &[GanttTask], epoch: NaiveDate) -> i64 {
    tasks
        .iter()
        .map(|t| t.end_days(epoch))
        .fold(0.0_f64, f64::max)
        .ceil() as i64
}

/// The width the chart is drawn at. Drawing and the canvas size guard
/// (`render::canvas_graph_limit_error`) both read this, so the guard cannot
/// accept a chart wider than the one that is drawn.
///
/// A chart with a legend is as wide as its days, held up by the legend's right
/// edge so the legend does not run past the viewBox. A chart without one has
/// nothing to hold up, and takes the width that covers the right edge of
/// everything it puts to the right of the days: the labels that run on past a
/// bar's end (design/legend-opt-out.md, 4.7). Every task counts, not only the
/// rows in the window, or the width would change as the chart scrolls.
///
/// A chart with a legend does not take the labels' right edge. Its labels run
/// past the viewBox for some inputs (a milestone or a one day bar that ends on
/// the last day, or a last day that is a Monday), and that has been so since
/// before the legend could be hidden. Widening it would change the default
/// output, which this change promises not to do, so it is left to its own
/// change (design/legend-opt-out.md, 4.8).
pub(crate) fn chart_width_for(
    tasks: &[GanttTask],
    epoch: NaiveDate,
    legend_mode: LegendMode,
) -> f64 {
    let total_days = total_days_of(tasks, epoch);
    let days_w = chart_width(total_days as f64);
    match legend_mode {
        LegendMode::Shown => days_w
            .max(LABEL_W + LEGEND_LEFT_PAD_PX + legend_layout().width + CHART_RIGHT_PADDING_PX),
        LegendMode::Hidden => days_w.max(labels_right_edge(tasks, epoch, total_days)),
    }
}

const GRID_LABEL_FONT_PX: f64 = 11.0;

/// The Mondays the grid draws, from the first one on or after `epoch` to the
/// last one within `total_days`. Every step is checked: an overflow stops the
/// grid instead of panicking.
fn grid_days(epoch: NaiveDate, total_days: i64) -> Vec<NaiveDate> {
    let mut d = epoch;
    while d.weekday() != Weekday::Mon {
        match d.checked_add_signed(Duration::days(1)) {
            Some(next) => d = next,
            None => break,
        }
    }
    let mut days = Vec::new();
    while (d - epoch).num_days() <= total_days {
        days.push(d);
        d = match d.checked_add_signed(Duration::weeks(1)) {
            Some(next) => next,
            None => break,
        };
    }
    days
}

fn grid_label(day: NaiveDate) -> String {
    format!("{}/{}", day.month(), day.day())
}

/// A task's percent label as it is drawn.
struct PercentLabel {
    x: f64,
    anchor: Option<TextAnchor>,
    fill: ColorId,
    content: String,
}

/// Where a task's percent label sits. A milestone's runs on to the right of the
/// diamond. A bar's sits inside the bar when it fits, and runs on past the bar's
/// end when it does not. Drawing and `labels_right_edge` both read this, so the
/// width cannot disagree with what is drawn.
fn percent_label(task: &GanttTask, epoch: NaiveDate) -> PercentLabel {
    let x = LABEL_W + task.start_days(epoch) * PX_PER_DAY;
    let content = format!("{}%", task.progress_pct.clamp(0, 100));
    if is_zero_duration(task) {
        return PercentLabel {
            x: x + BAR_H / 2.0 + 6.0,
            anchor: None,
            fill: ColorId::ProgressTextOnBg,
            content,
        };
    }
    let w = task_bar_width_days(task, epoch) * PX_PER_DAY;
    let (x, anchor, fill) = progress_label_style(x, w, w * task.progress(), &content);
    PercentLabel {
        x,
        anchor: Some(anchor),
        fill,
        content,
    }
}

/// The right edge of the text estimate for a label drawn at `x` with `anchor`.
fn text_right_edge(
    x: f64,
    anchor: Option<TextAnchor>,
    content: &str,
    font_px: f64,
    weight: Option<u16>,
) -> f64 {
    let w = text_width_weighted_px(content, font_px, weight);
    match anchor {
        None | Some(TextAnchor::Start) => x + w,
        Some(TextAnchor::Middle) => x + w / 2.0,
        Some(TextAnchor::End) => x,
    }
}

/// The furthest right any label reaches, over all tasks and the grid. A label
/// that runs on past the end of the days, such as a percent label to the right of
/// a bar that ends on the last day, needs the width to reach it. The bars and the
/// lines stop at the last day and need nothing.
fn labels_right_edge(tasks: &[GanttTask], epoch: NaiveDate, total_days: i64) -> f64 {
    let task_edges = tasks.iter().map(|task| {
        let label = percent_label(task, epoch);
        text_right_edge(
            label.x,
            label.anchor,
            &label.content,
            PROGRESS_LABEL_FONT_PX,
            Some(PROGRESS_LABEL_FONT_WEIGHT),
        )
    });
    let grid = grid_days(epoch, total_days).into_iter().map(|day| {
        text_right_edge(
            LABEL_W + (day - epoch).num_days() as f64 * PX_PER_DAY,
            None,
            &grid_label(day),
            GRID_LABEL_FONT_PX,
            None,
        )
    });
    task_edges.chain(grid).fold(0.0, f64::max)
}

fn progress_label_style(x: f64, w: f64, prog_w: f64, label: &str) -> (f64, TextAnchor, ColorId) {
    let approx_text_w = label.len() as f64 * 6.5;
    if w >= approx_text_w + 8.0 {
        let on_fg = prog_w >= w * 0.45;
        let fill = if on_fg {
            ColorId::ProgressTextOnFg
        } else {
            ColorId::ProgressTextOnBg
        };
        (x + w / 2.0, TextAnchor::Middle, fill)
    } else {
        (
            x + w.max(0.0) + 4.0,
            TextAnchor::Start,
            ColorId::ProgressTextOnBg,
        )
    }
}

/// The legend: the progress-line sample, then one swatch per tier.
fn legend_layer(chart_h: f64, legend: &LegendLayout) -> Layer {
    let legend_y1 = chart_h - LEGEND_H + 14.0;
    let legend_x = LABEL_W + LEGEND_LEFT_PAD_PX;
    let legend_y2 = chart_h - CHART_BOTTOM_PADDING_PX;
    let sw = LEGEND_SWATCH_PX;

    let mut legend_prims = vec![Primitive::Group(GroupPrim {
        task_id: None,
        tooltip: None,
        bbox: BBox {
            x: legend_x,
            y: legend_y1 - 4.0,
            width: legend.progress_width,
            height: 20.0,
        },
        children: vec![
            Primitive::Line(LinePrim {
                x1: legend_x,
                y1: legend_y1,
                x2: legend_x + LEGEND_LINE_SAMPLE_W_PX,
                y2: legend_y1,
                stroke: ColorId::Progress,
                stroke_width: PROGRESS_LINE_STROKE_W,
                stroke_dash: Some(PROGRESS_LINE_DASH.to_string()),
                semantic: LineSemantic::LegendProgressLine,
            }),
            Primitive::Text(TextPrim {
                x: legend_x + LEGEND_LINE_SAMPLE_W_PX + LEGEND_LINE_TEXT_GAP_PX,
                y: legend_y1 + 4.0,
                content: PROGRESS_LABEL.to_string(),
                fill: Some(ColorId::GridLabel),
                font_size: Some(LEGEND_PROGRESS_FONT_PX),
                font_weight: None,
                anchor: None,
                baseline: TextBaseline::Middle,
                semantic: TextSemantic::LegendProgress,
            }),
        ],
    })];

    let mut tier_children = Vec::new();
    let mut lx = legend_x;
    for (tier, label) in TIER_ITEMS {
        tier_children.push(Primitive::Rect(RectPrim {
            x: lx,
            y: legend_y2 - sw / 2.0,
            width: sw,
            height: sw,
            fill: tier_fill_color(tier),
            rx: Some(2.0),
            semantic: RectSemantic::LegendSwatch,
        }));
        lx += sw + LEGEND_SWATCH_GAP_PX;
        tier_children.push(Primitive::Text(TextPrim {
            x: lx,
            y: legend_y2 + 3.0,
            content: label.to_string(),
            fill: Some(ColorId::GridLabel),
            font_size: Some(LEGEND_TIER_FONT_PX),
            font_weight: None,
            anchor: None,
            baseline: TextBaseline::Middle,
            semantic: TextSemantic::LegendTier,
        }));
        // Advance by the label's estimated width, not by `label.len()`: that is
        // the UTF-8 byte count, three per Japanese glyph, and it pushed the row
        // far past the viewBox (issue #28).
        lx += text_width_px(label, LEGEND_TIER_FONT_PX) + LEGEND_ITEM_GAP_PX;
    }
    legend_prims.push(Primitive::Group(GroupPrim {
        task_id: None,
        tooltip: None,
        bbox: BBox {
            x: legend_x,
            y: legend_y2 - sw,
            width: legend.tier_width,
            height: sw + 6.0,
        },
        children: tier_children,
    }));
    Layer {
        kind: LayerKind::Legend,
        primitives: legend_prims,
    }
}

/// How wide the legend's two rows are, measured from `legend_x`. They depend on
/// the text only, never on the chart, so the chart width can be raised to hold
/// them before anything is laid out.
struct LegendLayout {
    /// The dashed sample, the gap, and the progress-line caption.
    progress_width: f64,
    /// The five tier swatches with their captions, up to the last caption's end.
    tier_width: f64,
    /// The wider of the two rows.
    width: f64,
}

fn legend_layout() -> LegendLayout {
    let progress_width = LEGEND_LINE_SAMPLE_W_PX
        + LEGEND_LINE_TEXT_GAP_PX
        + text_width_px(PROGRESS_LABEL, LEGEND_PROGRESS_FONT_PX);
    let mut tier_width = 0.0;
    for (_, label) in TIER_ITEMS {
        tier_width += LEGEND_SWATCH_PX
            + LEGEND_SWATCH_GAP_PX
            + text_width_px(label, LEGEND_TIER_FONT_PX)
            + LEGEND_ITEM_GAP_PX;
    }
    // The advance after the last caption is a gap to nothing.
    tier_width -= LEGEND_ITEM_GAP_PX;
    LegendLayout {
        progress_width,
        tier_width,
        width: progress_width.max(tier_width),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_list::text_metrics::glyph_em;
    use crate::graph::{GanttDep, GanttGraph};

    fn date(y: i32, m: u32, d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(y, m, d).unwrap()
    }

    fn graph_of(titles: &[&str], days: i64) -> GanttGraph {
        let start = date(2026, 6, 1);
        GanttGraph {
            tasks: titles
                .iter()
                .enumerate()
                .map(|(i, t)| GanttTask {
                    id: format!("t{i}"),
                    title: (*t).to_string(),
                    progress_pct: 50,
                    start,
                    end: Some(start + Duration::days(days)),
                })
                .collect(),
            deps: Vec::<GanttDep>::new(),
        }
    }

    fn legend_prims(list: &DisplayList) -> Vec<&Primitive> {
        fn walk<'a>(p: &'a Primitive, out: &mut Vec<&'a Primitive>) {
            match p {
                Primitive::Group(g) => g.children.iter().for_each(|c| walk(c, out)),
                other => out.push(other),
            }
        }
        let mut out = Vec::new();
        for layer in list.layers.iter().filter(|l| l.kind == LayerKind::Legend) {
            layer.primitives.iter().for_each(|p| walk(p, &mut out));
        }
        out
    }

    /// 凡例のどの部品の右端も viewBox の中に収まる。
    fn assert_legend_inside(list: &DisplayList) {
        let width = list.viewport.width;
        for p in legend_prims(list) {
            let right = match p {
                Primitive::Rect(r) => r.x + r.width,
                Primitive::Line(l) => l.x1.max(l.x2),
                Primitive::Text(t) => {
                    t.x + text_width_px(&t.content, t.font_size.unwrap_or(DEFAULT_FONT_SIZE_PX))
                }
                _ => continue,
            };
            assert!(
                right <= width,
                "legend part ends at {right}, viewBox is {width}"
            );
        }
    }

    #[test]
    fn legend_stays_inside_the_viewbox_for_short_spans() {
        for days in [0, 1, 2, 4, 7, 14, 60] {
            let g = graph_of(&["a"], days);
            let list = build_display_list(&g, date(2026, 6, 1), &ChartOptions::default());
            assert_legend_inside(&list);
        }
    }

    #[test]
    fn a_wide_chart_is_not_widened_by_the_legend() {
        let g = graph_of(&["a"], 60);
        let list = build_display_list(&g, date(2026, 6, 1), &ChartOptions::default());
        assert_eq!(list.viewport.width, chart_width(60.0));
    }

    #[test]
    fn a_short_chart_is_widened_to_hold_the_legend() {
        let g = graph_of(&["a"], 2);
        let list = build_display_list(&g, date(2026, 6, 1), &ChartOptions::default());
        assert!(list.viewport.width > chart_width(2.0));
        // 背景の見出し帯も同じ幅まで伸びる。
        let header_right = list
            .layers
            .iter()
            .filter(|l| l.kind == LayerKind::Background)
            .flat_map(|l| l.primitives.iter())
            .find_map(|p| match p {
                Primitive::Rect(r) => Some(r.x + r.width),
                _ => None,
            })
            .unwrap();
        assert_eq!(header_right, list.viewport.width);
    }

    #[test]
    fn legend_advance_is_not_the_utf8_byte_count() {
        let w = legend_layout().tier_width;
        // 旧実装は五つの見出しで約 319px 進めた。字数で数えれば 171px になる。
        assert!(w < 200.0, "tier row is {w}px wide");
    }

    fn row_label_texts(list: &DisplayList) -> Vec<&TextPrim> {
        let mut out = Vec::new();
        for layer in list.layers.iter().filter(|l| l.kind == LayerKind::Bars) {
            for p in &layer.primitives {
                if let Primitive::Group(g) = p {
                    for c in &g.children {
                        if let Primitive::Text(t) = c {
                            if t.semantic == TextSemantic::RowLabel {
                                out.push(t);
                            }
                        }
                    }
                }
            }
        }
        out
    }

    #[test]
    fn row_labels_fit_the_label_column() {
        let titles = [
            "とても長い題名のタスクでどこまで表示されるかを確かめるための題",
            "DBスキーマ変更とマイグレーション手順の整備",
            "APIのレート制限を設計する",
            "Refactor the authentication middleware to support multiple providers",
            "WWWWWWWWWWWWWWWWWWWW",
            "短い",
        ];
        let g = graph_of(&titles, 8);
        let list = build_display_list(&g, date(2026, 6, 1), &ChartOptions::default());
        for t in row_label_texts(&list) {
            let w = text_width_px(&t.content, DEFAULT_FONT_SIZE_PX);
            // anchor は End。左端は x - w。0 を割ると頭の字が消える。
            assert!(
                t.x - w >= ROW_LABEL_LEFT_MARGIN_PX - 1e-9,
                "{:?}: left edge {}",
                t.content,
                t.x - w
            );
        }
    }

    /// 切り詰めと同じ見積もり（`text_width_px`）で左端を検めると、見積もりが外れた時に
    /// 切り詰めも検めも同じだけ外れ、試験が通ったまま実際の図がはみ出す。
    /// そこで、曖昧幅の字を含む題は、見積もりを使わずに全角 1 字を `DEFAULT_FONT_SIZE_PX` と置いて検める。
    /// これらの字は日本語の字体で全角に描かれるので、この置き方が実際の幅の上限になる。
    /// 字体の実測は wasm の中では取れず、この検めも実測ではない。
    #[test]
    fn row_labels_with_ambiguous_width_glyphs_fit_without_the_estimate() {
        let titles = [
            "①②③④⑤⑥⑦⑧⑨⑩",
            "※要確認",
            "→ 次工程",
            "■ 設計",
            "①②③④⑤⑥⑦⑧⑨⑩⑪⑫⑬⑭⑮⑯⑰⑱⑲⑳",
            "→→→→→→→→→→→→→→→→→→→→",
            "※※※※※※※※※※※※※※※※※※※※",
            "± × ÷ ° § ± × ÷ ° § ± × ÷ ° §",
        ];
        let g = graph_of(&titles, 8);
        let list = build_display_list(&g, date(2026, 6, 1), &ChartOptions::default());
        let labels = row_label_texts(&list);
        assert_eq!(labels.len(), titles.len());
        for t in labels {
            // 字の数 × 全角。半角の空白だけは 0.3 とする（空白は全角にならぬ字体が無い）。
            let w: f64 = t
                .content
                .chars()
                .map(|c| if c == ' ' { 0.3 } else { 1.0 } * DEFAULT_FONT_SIZE_PX)
                .sum();
            assert!(
                t.x - w >= ROW_LABEL_LEFT_MARGIN_PX - 1e-9,
                "{:?}: left edge {}",
                t.content,
                t.x - w
            );
        }
    }

    /// 検め手の挙げた例が、切られる位置も含めて意図の通りに収まる。
    /// 期待は `text_width_px` を通さず、文字列で直に書く。
    #[test]
    fn ambiguous_width_titles_are_cut_where_they_are_expected() {
        let titles = [
            "①②③④⑤⑥⑦⑧⑨⑩",
            "※要確認",
            "→ 次工程",
            "■ 設計",
            "①②③④⑤⑥⑦⑧⑨⑩⑪⑫",
        ];
        let g = graph_of(&titles, 8);
        let list = build_display_list(&g, date(2026, 6, 1), &ChartOptions::default());
        let labels: Vec<String> = row_label_texts(&list)
            .iter()
            .map(|t| t.content.clone())
            .collect();
        // 112px は全角 9 字分。10 字の ①..⑩ は 8 字と … に切られる。
        assert_eq!(
            labels,
            vec!["①②③④⑤⑥⑦⑧…", "※要確認", "→ 次工程", "■ 設計", "①②③④⑤⑥⑦⑧…",]
        );
    }

    /// 凡例の文言は固定の字面で、使う手が差し替えられない。
    /// どの字も全角に数えられ、曖昧幅の字は含まれない。
    /// 文言を書き換えて曖昧幅や比例幅の字を入れた時に、気づけるようにする。
    #[test]
    fn legend_captions_are_all_full_width_glyphs() {
        let captions =
            std::iter::once(PROGRESS_LABEL).chain(TIER_ITEMS.iter().map(|(_, label)| *label));
        for caption in captions {
            for c in caption.chars() {
                assert_eq!(glyph_em(c), 1.0, "{caption:?}: U+{:04X} {c}", c as u32);
            }
        }
    }

    #[test]
    fn a_title_that_fits_is_not_touched() {
        let g = graph_of(&["短い", "Design"], 8);
        let list = build_display_list(&g, date(2026, 6, 1), &ChartOptions::default());
        let labels: Vec<&str> = row_label_texts(&list)
            .iter()
            .map(|t| t.content.as_str())
            .collect();
        assert_eq!(labels, vec!["短い", "Design"]);
    }

    #[test]
    fn a_cut_title_ends_with_an_ellipsis_and_keeps_its_head() {
        let g = graph_of(&["とても長い題名のタスクでどこまで表示されるか"], 8);
        let list = build_display_list(&g, date(2026, 6, 1), &ChartOptions::default());
        let label = &row_label_texts(&list)[0].content;
        assert!(label.starts_with("とても長い"), "{label}");
        assert!(label.ends_with('…'));
    }
}
