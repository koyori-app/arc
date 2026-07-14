use chrono::{Datelike, Duration, NaiveDate, Weekday};
use std::collections::HashMap;

use crate::graph::{GanttGraph, GanttTask};
use crate::layout::assign_rows;
use crate::progress::progress_line;

use super::constants::*;
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
    let last_visible = ((scroll.scroll_y + scroll.client_height - HEADER_H).max(0.0) / ROW_H)
        .ceil() as usize;
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

fn dep_incident_to_window(
    from_row: usize,
    to_row: usize,
    window: Option<(usize, usize)>,
) -> bool {
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
    today: Option<NaiveDate>,
    scroll_viewport: Option<ScrollViewport>,
) -> DisplayList {
    let rows = assign_rows(&graph.tasks, &graph.deps);

    let total_days = graph
        .tasks
        .iter()
        .map(|t| t.end_days(epoch))
        .fold(0.0_f64, f64::max)
        .ceil() as i64;

    let chart_w = total_days as f64 * PX_PER_DAY + LABEL_W + 20.0;
    let chart_h = rows.len() as f64 * ROW_H + HEADER_H + LEGEND_H + 10.0;

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
        let label = format!("{}/{}", grid_day.month(), grid_day.day());
        grid_prims.push(Primitive::Text(TextPrim {
            x,
            y: HEADER_H / 2.0 + 4.0,
            content: label,
            fill: Some(ColorId::GridLabel),
            font_size: Some(11.0),
            font_weight: None,
            anchor: None,
            baseline: TextBaseline::Auto,
            semantic: TextSemantic::GridLabel,
        }));
        grid_day += Duration::weeks(1);
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
            children.push(Primitive::Text(TextPrim {
                x: cx + half + 6.0,
                y: cy,
                content: pct_label,
                fill: Some(ColorId::ProgressTextOnBg),
                font_size: Some(11.0),
                font_weight: Some(600),
                anchor: None,
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
                    rx: Some(4.0),
                    semantic: RectSemantic::BarBackground,
                }));
                if prog_w > 0.0 {
                    children.push(Primitive::RoundRect(RoundRectPrim {
                        x,
                        y,
                        width: prog_w,
                        height: BAR_H,
                        fill: tier_fill_color(tier),
                        rx: 4.0,
                        tier,
                        semantic: RoundRectSemantic::BarProgress,
                    }));
                }
            }
            let (tx, anchor, fill) = progress_label_style(x, w, prog_w, &pct_label);
            children.push(Primitive::Text(TextPrim {
                x: tx,
                y: y + BAR_H / 2.0,
                content: pct_label,
                fill: Some(fill),
                font_size: Some(11.0),
                font_weight: Some(600),
                anchor: Some(anchor),
                baseline: TextBaseline::Middle,
                semantic: TextSemantic::ProgressPercent,
            }));
        }

        let display_title = truncate_title(&task.title, TITLE_MAX_CHARS);
        children.push(Primitive::Text(TextPrim {
            x: LABEL_W - 4.0,
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
            x: LABEL_W - 4.0 - 100.0,
            y,
            width: x + w.max(BAR_H) - (LABEL_W - 104.0),
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
    let row_map: HashMap<&str, &crate::layout::RowLayout> =
        rows.iter().map(|r| (r.task_id.as_str(), r)).collect();
    let task_map: HashMap<&str, &GanttTask> =
        graph.tasks.iter().map(|t| (t.id.as_str(), t)).collect();

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

    // Progress line layer — delegates geometry to progress_line()
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
        layers.push(Layer {
            kind: LayerKind::ProgressLine,
            primitives: vec![Primitive::Polyline(PolylinePrim {
                points: pts,
                stroke: ColorId::Progress,
                stroke_width: 2.0,
                stroke_dash: Some("6,3".to_string()),
                semantic: PolylineSemantic::ProgressStatusLine,
            })],
        });
    }

    // Legend layer
    let legend_y1 = chart_h - LEGEND_H + 14.0;
    let legend_x = LABEL_W + 8.0;
    let legend_y2 = chart_h - 10.0;
    let sw = 10.0;
    let gap = 4.0;

    let mut legend_prims = vec![
        Primitive::Group(GroupPrim {
            task_id: None,
            tooltip: None,
            bbox: BBox {
                x: legend_x,
                y: legend_y1 - 4.0,
                width: 400.0,
                height: 20.0,
            },
            children: vec![
                Primitive::Line(LinePrim {
                    x1: legend_x,
                    y1: legend_y1,
                    x2: legend_x + 28.0,
                    y2: legend_y1,
                    stroke: ColorId::Progress,
                    stroke_width: 2.0,
                    stroke_dash: Some("6,3".to_string()),
                    semantic: LineSemantic::LegendProgressLine,
                }),
                Primitive::Text(TextPrim {
                    x: legend_x + 34.0,
                    y: legend_y1 + 4.0,
                    content: "進捗ステータスライン（各タスクの完了位置を結ぶ）".to_string(),
                    fill: Some(ColorId::GridLabel),
                    font_size: Some(10.0),
                    font_weight: None,
                    anchor: None,
                    baseline: TextBaseline::Middle,
                    semantic: TextSemantic::LegendProgress,
                }),
            ],
        }),
    ];

    let tier_items: [(ColorId, &str); 5] = [
        (ColorId::BarBg, "未達"),
        (ColorId::TierLow, "低"),
        (ColorId::TierMid, "中"),
        (ColorId::TierHigh, "高"),
        (ColorId::TierDone, "完了"),
    ];
    let mut tier_children = Vec::new();
    let mut lx = legend_x;
    for (color_id, label) in tier_items {
        tier_children.push(Primitive::Rect(RectPrim {
            x: lx,
            y: legend_y2 - sw / 2.0,
            width: sw,
            height: sw,
            fill: color_id,
            rx: Some(2.0),
            semantic: RectSemantic::LegendSwatch,
        }));
        lx += sw + 2.0;
        tier_children.push(Primitive::Text(TextPrim {
            x: lx,
            y: legend_y2 + 3.0,
            content: label.to_string(),
            fill: Some(ColorId::GridLabel),
            font_size: Some(9.0),
            font_weight: None,
            anchor: None,
            baseline: TextBaseline::Middle,
            semantic: TextSemantic::LegendTier,
        }));
        lx += label.len() as f64 * 9.0 + gap + sw;
    }
    legend_prims.push(Primitive::Group(GroupPrim {
        task_id: None,
        tooltip: None,
        bbox: BBox {
            x: legend_x,
            y: legend_y2 - sw,
            width: lx - legend_x,
            height: sw + 6.0,
        },
        children: tier_children,
    }));
    layers.push(Layer {
        kind: LayerKind::Legend,
        primitives: legend_prims,
    });

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

fn truncate_title(s: &str, max_chars: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max_chars {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_chars.saturating_sub(1)).collect();
        format!("{truncated}…")
    }
}

fn task_bar_width_days(task: &GanttTask, epoch: NaiveDate) -> f64 {
    (task.end_days(epoch) - task.start_days(epoch)).max(0.0)
}

fn is_zero_duration(task: &GanttTask) -> bool {
    task.end.is_some_and(|end| end == task.start)
}

fn tier_fill_color(tier: ProgressTier) -> ColorId {
    match tier {
        ProgressTier::None => ColorId::BarBg,
        ProgressTier::Low => ColorId::TierLow,
        ProgressTier::Mid => ColorId::TierMid,
        ProgressTier::High => ColorId::TierHigh,
        ProgressTier::Done => ColorId::TierDone,
    }
}

fn progress_label_style(
    x: f64,
    w: f64,
    prog_w: f64,
    label: &str,
) -> (f64, TextAnchor, ColorId) {
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
        (x + w.max(0.0) + 4.0, TextAnchor::Start, ColorId::ProgressTextOnBg)
    }
}
