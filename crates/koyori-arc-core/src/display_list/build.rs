use chrono::{Datelike, Duration, NaiveDate, Weekday};
use std::collections::HashMap;

use crate::graph::{BarShape, BarState, GanttGraph, GanttRow, GanttTask, TaskRow};
use crate::progress::{by_row_bend, progress_line, progress_line_by_row};

use super::constants::*;
use super::legend::{tier_fill_color, PROGRESS_LABEL, TIER_ITEMS};
use super::rows::RowPlan;
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
    build_display_list_with_rows(graph, None, epoch, options)
}

/// `build_display_list` for a caller that lays out the rows itself: `rows` is
/// the chart's rows, top to bottom, task rows and group rows, and a task it
/// leaves out is not drawn. `None` is `build_display_list`. See
/// docs/design/external-rows.md.
pub fn build_display_list_with_rows(
    graph: &GanttGraph,
    rows: Option<&[GanttRow]>,
    epoch: NaiveDate,
    options: &ChartOptions,
) -> DisplayList {
    let ChartOptions {
        today,
        scroll_viewport,
        legend: legend_mode,
        percent_labels,
        scale,
        weekend,
        ..
    } = *options;
    let epoch = chart_epoch(epoch, options);
    let plan = RowPlan::new(&graph.tasks, &graph.deps, rows, options);
    let header_h = plan.header_h;
    let bar_h = plan.bar_h;
    let axis = XAxis::of(options);
    let label_w = axis.label_w;

    let total_days = chart_days(&graph.tasks, rows, epoch, options);
    let view_days = options.view.map(|_| total_days);
    // The x where the days a view shows end; past it only marks and notes.
    let days_right = axis.x(total_days as f64);
    let drawn_days = chart_drawn_days(&graph.tasks, epoch, total_days, options);

    // The legend's width does not depend on how many days there are, so a short
    // span would otherwise leave it running past the right edge of the viewBox.
    let legend = (legend_mode == LegendMode::Shown).then(legend_layout);
    let chart_w = chart_width_for(&graph.tasks, rows, epoch, options);
    let chart_h = chart_height_of(&plan, legend_mode);

    let viewport = Viewport {
        width: chart_w,
        height: chart_h,
        label_width: label_w,
        header_height: header_h,
        row_height: plan.row_h,
    };

    let row_window = plan.window(scroll_viewport);

    let mut layers = Vec::new();
    let mut task_bboxes = Vec::new();

    // Background layer
    layers.push(Layer {
        kind: LayerKind::Background,
        primitives: vec![Primitive::Rect(RectPrim {
            x: label_w,
            y: 0.0,
            width: chart_w - label_w,
            height: header_h,
            fill: ColorId::HeaderBg,
            rx: None,
            semantic: RectSemantic::HeaderBackground,
        })],
    });

    // Group rows: a band across the days. Only a caller's row list has them.
    // In a view the band stops at the view's last day, as the weekend bands
    // do; past it is only the room a mark or a note runs into.
    let band_right = if view_days.is_some() {
        days_right
    } else {
        chart_w
    };
    let group_bands: Vec<Primitive> = plan
        .slots
        .iter()
        .enumerate()
        .filter(|(i, s)| s.task.is_none() && row_in_window(*i, row_window))
        .map(|(_, s)| {
            Primitive::Rect(RectPrim {
                x: label_w,
                y: s.y_top,
                width: band_right - label_w,
                height: s.height,
                fill: ColorId::GroupRow,
                rx: None,
                semantic: RectSemantic::GroupRow,
            })
        })
        .collect();
    if !group_bands.is_empty() {
        layers.push(Layer {
            kind: LayerKind::Background,
            primitives: group_bands,
        });
    }

    // Weekend bands over the rows, over the group bands too so a weekend reads
    // through a group's heading. None unless the caller names weekend days.
    let weekend_bands: Vec<Primitive> = calendar_days(epoch, drawn_days)
        .filter(|d| weekend.contains(d.weekday()))
        .map(|d| {
            Primitive::Rect(RectPrim {
                x: axis.day_x(epoch, d),
                y: header_h,
                width: axis.px_per_day,
                height: plan.rows_height(),
                fill: ColorId::Weekend,
                rx: None,
                semantic: RectSemantic::Weekend,
            })
        })
        .collect();
    if !weekend_bands.is_empty() {
        layers.push(Layer {
            kind: LayerKind::Background,
            primitives: weekend_bands,
        });
    }

    // Grid layer
    let mut grid_prims = Vec::new();
    // The Wasm entry points reject dates without headroom below `NaiveDate::MAX`,
    // but the native entry points have no such guard, so every step here is
    // checked: an overflow stops the grid instead of panicking.
    let grid_line = |x: f64| {
        Primitive::Line(LinePrim {
            x1: x,
            y1: header_h,
            x2: x,
            y2: chart_h,
            stroke: ColorId::Grid,
            stroke_width: 1.0,
            stroke_dash: None,
            semantic: LineSemantic::Grid,
        })
    };
    match scale {
        // A line and its label in turn, as the chart drawn before scales.
        Scale::Week | Scale::Month => {
            for grid_day in grid_days_in(scale, epoch, total_days, drawn_days, options) {
                grid_prims.push(grid_line(axis.day_x(epoch, grid_day)));
                grid_prims.push(Primitive::Text(grid_text_of(
                    &axis, epoch, grid_day, scale, header_h,
                )));
            }
        }
        // The Monday lines, then every day's labels.
        Scale::Day => {
            for grid_day in grid_days_in(scale, epoch, total_days, drawn_days, options) {
                grid_prims.push(grid_line(axis.day_x(epoch, grid_day)));
            }
            grid_prims.extend(
                day_texts(&axis, epoch, drawn_days, header_h, options)
                    .into_iter()
                    .map(Primitive::Text),
            );
        }
    }
    layers.push(Layer {
        kind: LayerKind::Grid,
        primitives: grid_prims,
    });

    // Bars layer (row-virtualized when scroll_viewport is set)
    let mut bar_prims = Vec::new();
    let mut note_prims = Vec::new();
    // In the order the tasks came in, as before row lists existed, so the
    // output of a chart without one keeps its order.
    for (task_index, task) in graph.tasks.iter().enumerate() {
        let Some(slot_index) = plan.slot_of_task[task_index] else {
            continue;
        };
        if !row_in_window(slot_index, row_window) {
            continue;
        }
        let y = plan.bar_top(&plan.slots[slot_index]);
        let marks = task_row_of(rows, &plan.slots[slot_index]);
        let placement = Placement::of(task, marks, epoch, view_days);
        let shown = match placement {
            Placement::Shown(shown) => shown,
            // A task wholly outside the view: its mark alone, at the edge it
            // lies past, so the row says where the task is.
            Placement::Before(day) | Placement::After(day) => {
                let cy = y + bar_h / 2.0;
                let mark = outside_mark(&placement, day, &axis, total_days, cy, marks);
                let bbox = overflow_bbox(&mark, y, bar_h);
                let mut children = Vec::new();
                // A hard deadline inside the view still shows, and so does the
                // note of a task that ended before it: a task late against its
                // dates is mostly one whose bar is already behind the view.
                if let Some(m) = marks {
                    let g = MarkGeometry::of(task, m, &axis, epoch, view_days, bar_h);
                    if let Some(dx) = g.diamond_cx {
                        let tier = ProgressTier::from_pct(task.progress_pct.clamp(0, 100));
                        children.push(hard_deadline_prim(dx, cy, tier));
                    }
                    if let (Placement::Before(_), Some(note)) = (&placement, &m.note) {
                        let x = outside_note_x(&mark, &g, note);
                        note_prims.extend(note_with_band(x, cy, note, m.state));
                    }
                }
                children.insert(0, Primitive::Text(mark));
                task_bboxes.push(TaskBBox {
                    task_id: task.id.clone(),
                    row: slot_index as u32,
                    bbox: bbox.clone(),
                    bar_bbox: bbox.clone(),
                });
                bar_prims.push(Primitive::Group(GroupPrim {
                    task_id: Some(task.id.clone()),
                    tooltip: Some(task_tooltip(task)),
                    bbox,
                    children,
                }));
                continue;
            }
        };
        let cut = shown.cut_left || shown.cut_right;
        let x = axis.x(shown.start);
        let w = axis.len((shown.end - shown.start).max(0.0));
        let prog = task.progress();
        let prog_w = if cut {
            // The progress of the whole bar, cut where the view cuts the bar.
            let full_x = axis.x(task.start_days(epoch));
            let full_w = axis.len(task_bar_width_days(task, epoch));
            ((full_x + full_w * prog).min(x + w) - x).max(0.0)
        } else {
            w * prog
        };
        let prog_pct = task.progress_pct.clamp(0, 100);
        let tier = ProgressTier::from_pct(prog_pct);
        let tooltip = task_tooltip(task);
        // A bar the view cuts shows no percent: the label sits by the bar's
        // ends, and one of them is out of the view.
        let percent_labels = if cut {
            LabelMode::Hidden
        } else {
            percent_labels
        };

        let mut children = Vec::new();
        let marks_geometry =
            marks.map(|m| MarkGeometry::of(task, m, &axis, epoch, view_days, bar_h));

        if let Some(MarkGeometry {
            point_cx: Some(cx), ..
        }) = marks_geometry
        {
            // A task with only a deadline: a dot on its last day, no bar.
            let state = marks.and_then(|m| m.state);
            let d = 2.0 * point_radius(bar_h);
            children.push(Primitive::RoundRect(RoundRectPrim {
                x: cx - d / 2.0,
                y: y + (bar_h - d) / 2.0,
                width: d,
                height: d,
                fill: outline_color(state),
                rx: d / 2.0,
                tier,
                semantic: RoundRectSemantic::Point,
            }));
        } else if is_zero_duration(task) {
            let cx = x;
            let cy = y + bar_h / 2.0;
            let half = bar_h / 2.0;
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
            if percent_labels == LabelMode::Shown {
                let label = percent_label(task, epoch, &axis, bar_h);
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
            }
        } else {
            if w > 0.0 {
                let state = marks.and_then(|m| m.state);
                children.push(Primitive::Rect(RectPrim {
                    x,
                    y,
                    width: w,
                    height: bar_h,
                    fill: bar_fill_color(state),
                    rx: Some(BAR_CORNER_RADIUS_PX),
                    semantic: RectSemantic::BarBackground,
                }));
                if prog_w > 0.0 {
                    children.push(Primitive::RoundRect(RoundRectPrim {
                        x,
                        y,
                        width: prog_w,
                        height: bar_h,
                        fill: tier_fill_color(tier),
                        rx: BAR_CORNER_RADIUS_PX,
                        tier,
                        semantic: RoundRectSemantic::BarProgress,
                    }));
                }
                // A state, given at all, outlines the bar in its colour.
                if state.is_some() {
                    children.push(Primitive::Path(PathPrim {
                        d: rounded_rect_path(x, y, w, bar_h, BAR_CORNER_RADIUS_PX),
                        stroke: outline_color(state),
                        stroke_width: 1.0,
                        semantic: PathSemantic::BarOutline,
                    }));
                }
            }
            // The edges a view cuts: the day the bar runs on to, inside the edge.
            let cy = y + bar_h / 2.0;
            if shown.cut_left {
                let mark = Placement::Before(task.start);
                children.push(Primitive::Text(overflow_mark(
                    &mark, task.start, &axis, total_days, cy,
                )));
            }
            if shown.cut_right {
                let last = last_day(task, epoch);
                let mark = Placement::After(last);
                children.push(Primitive::Text(overflow_mark(
                    &mark, last, &axis, total_days, cy,
                )));
            }
            if percent_labels == LabelMode::Shown {
                let label = percent_label(task, epoch, &axis, bar_h);
                children.push(Primitive::Text(TextPrim {
                    x: label.x,
                    y: y + bar_h / 2.0,
                    content: label.content,
                    fill: Some(label.fill),
                    font_size: Some(PROGRESS_LABEL_FONT_PX),
                    font_weight: Some(PROGRESS_LABEL_FONT_WEIGHT),
                    anchor: label.anchor,
                    baseline: TextBaseline::Middle,
                    semantic: TextSemantic::ProgressPercent,
                }));
            }
        }

        // The marks a task row adds after the bar: the hard deadline's diamond,
        // the line from a point to it, and the note.
        if let (Some(m), Some(g)) = (marks, marks_geometry) {
            let cy = y + bar_h / 2.0;
            if let (Some(px), Some(dx)) = (g.point_cx, g.diamond_cx) {
                if dx > px {
                    children.push(Primitive::Line(LinePrim {
                        x1: px,
                        y1: cy,
                        x2: dx,
                        y2: cy,
                        stroke: outline_color(m.state),
                        stroke_width: 1.0,
                        stroke_dash: None,
                        semantic: LineSemantic::DeadlineLink,
                    }));
                }
            }
            if let Some(dx) = g.diamond_cx {
                children.push(hard_deadline_prim(dx, cy, tier));
            }
            if let Some(note) = &m.note {
                // The note goes on its own layer over the today line, with a
                // band of the ground's colour under it; where it sits is the
                // same as before.
                note_prims.extend(note_with_band(g.note_x, cy, note, m.state));
            }
        }

        // The left of what the row draws: a milestone's diamond reaches half a
        // bar left of its day.
        let shape_left = if is_zero_duration(task) {
            x - bar_h / 2.0
        } else {
            x
        };
        // Without the title column the row has nothing left of the shape to
        // hit, so the hit area starts where the shape does. With it, the area
        // starts left of the title, which is left of any shape.
        let hit_left = match options.row_labels {
            LabelMode::Shown => {
                let display_title =
                    truncate_to_width(&task.title, DEFAULT_FONT_SIZE_PX, ROW_LABEL_MAX_W_PX);
                let label_x = LABEL_W - LABEL_GAP_PX;
                children.push(Primitive::Text(TextPrim {
                    x: label_x,
                    y: y + bar_h / 2.0,
                    content: display_title,
                    fill: None,
                    font_size: None,
                    font_weight: None,
                    anchor: Some(TextAnchor::End),
                    baseline: TextBaseline::Middle,
                    semantic: TextSemantic::RowLabel,
                }));
                (label_x - LABEL_HIT_W).min(shape_left)
            }
            LabelMode::Hidden => shape_left,
        };

        let bar_bbox = BBox {
            x,
            y,
            width: w.max(bar_h),
            height: bar_h,
        };
        let group_bbox = BBox {
            x: hit_left,
            y,
            width: x + w.max(bar_h) - hit_left,
            height: bar_h,
        };
        task_bboxes.push(TaskBBox {
            task_id: task.id.clone(),
            row: slot_index as u32,
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
    let mut row_map: HashMap<&str, usize> = HashMap::new();
    for (i, t) in graph.tasks.iter().enumerate() {
        if let Some(slot) = plan.slot_of_task[i] {
            row_map.entry(t.id.as_str()).or_insert(slot);
        }
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
        let (from_r, to_r) = (*from_r, *to_r);
        if !dep_incident_to_window(from_r, to_r, row_window) {
            continue;
        }

        let from_x = axis.x(from_t.start_days(epoch));
        let from_w = axis.len(task_bar_width_days(from_t, epoch));
        let to_x = axis.x(to_t.start_days(epoch));

        if view_days.is_some() {
            let in_view = |x: f64| x >= label_w && x <= days_right;
            if !in_view(from_x + from_w / 2.0) || !in_view(to_x) {
                continue;
            }
        }

        let mut start_x = from_x + from_w / 2.0;
        while to_x < start_x + ROW_PADDING && start_x > from_x + ROW_PADDING {
            start_x -= DEP_BACKOFF_STEP_PX;
        }
        start_x -= DEP_BACKOFF_STEP_PX;

        let (from_slot, to_slot) = (&plan.slots[from_r], &plan.slots[to_r]);
        let start_y = plan.bar_top(from_slot) + bar_h;
        let end_x = to_x - ARROW_LEAD;
        let end_y = to_slot.y_top + to_slot.height / 2.0;

        let from_is_below_to = from_r > to_r;
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

    // Today marker. A view shows `from` through `to`, so its today is read by
    // the date: the x of the day right after `to` is the view's right edge,
    // and a test on the x would draw that day's today there. A today the chart
    // does not show draws no line, and the progress line, which reads this x,
    // falls back to its form for a chart without today. A chart without a view
    // keeps the test on the x it was pinned with.
    let today_x = today.and_then(|today| {
        let x = axis.day_x(epoch, today);
        let shown = match options.view {
            Some(view) => view.from <= today && today <= view.to,
            None => x >= label_w && x <= chart_w,
        };
        shown.then_some(x)
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

    // Notes over the today line and the grid. No note, no layer: a chart
    // without notes is the chart drawn before them.
    if !note_prims.is_empty() {
        layers.push(Layer {
            kind: LayerKind::Notes,
            primitives: note_prims,
        });
    }

    // Progress line layer — delegates geometry to progress_line().
    // The tasks come in input order but the line runs top to bottom, so the
    // points follow the rows. Otherwise a dependency that reorders rows makes
    // the line run backwards, and the today anchors grow from a row in the
    // middle. A group row has no task and adds no point.
    let prog_input: Vec<(f64, f64, f64, f64, f64)> = plan
        .slots
        .iter()
        .filter_map(|slot| {
            let t = &graph.tasks[slot.task?];
            Some((
                axis.x(t.start_days(epoch)),
                axis.x(t.end_days(epoch)),
                slot.y_top,
                slot.y_bottom(),
                t.progress(),
            ))
        })
        .collect();

    match options.progress_line {
        ProgressLineMode::Hidden => {}
        ProgressLineMode::ByRow => {
            // Without a today line on the chart there is nothing to measure
            // against, so no line.
            if let Some(today_x) = today_x {
                let line = by_row_progress_line(graph, rows, &plan, &axis, epoch, today_x);
                let in_view = |x: f64| match view_days {
                    Some(_) => x.clamp(label_w, days_right),
                    None => x,
                };
                let pts: Vec<(f64, f64)> = line.iter().map(|&(x, y)| (in_view(x), y)).collect();
                if pts.len() >= 2 {
                    let mut primitives = vec![Primitive::Polyline(PolylinePrim {
                        points: pts.clone(),
                        stroke: ColorId::ProgressLine,
                        stroke_width: BY_ROW_LINE_STROKE_W,
                        stroke_dash: None,
                        semantic: PolylineSemantic::ProgressStatusLine,
                    })];
                    // A dot at each bend off the today line.
                    let r = BY_ROW_POINT_RADIUS_PX;
                    primitives.extend(pts.iter().filter(|p| p.0 != today_x).map(|&(x, y)| {
                        Primitive::RoundRect(RoundRectPrim {
                            x: x - r,
                            y: y - r,
                            width: 2.0 * r,
                            height: 2.0 * r,
                            fill: ColorId::ProgressLine,
                            rx: r,
                            tier: ProgressTier::Low,
                            semantic: RoundRectSemantic::ProgressPoint,
                        })
                    }));
                    layers.push(Layer {
                        kind: LayerKind::ProgressLine,
                        primitives,
                    });
                }
            }
        }
        ProgressLineMode::Shown => {
            let mut pts = progress_line(&prog_input, today_x);
            if view_days.is_some() {
                for p in &mut pts {
                    p.0 = p.0.clamp(label_w, days_right);
                }
            }
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
        }
    }

    if let Some(legend) = &legend {
        layers.push(legend_layer(chart_h, legend, label_w));
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

/// The height the chart is drawn at: its header, its rows, and below them the
/// legend (when shown) and the bottom padding. For a chart without a row list
/// this is `chart_height_with(tasks.len(), legend)`.
pub(crate) fn chart_height_of(plan: &RowPlan, legend: LegendMode) -> f64 {
    plan.header_h + plan.rows_height() + chart_chrome_h(legend) - HEADER_H
}

/// The width the chart is drawn at. Drawing and the canvas size guard
/// (`render::canvas_graph_limit_error`) both read this, so the guard cannot
/// accept a chart wider than the one that is drawn.
///
/// The chart is as wide as its days, held up by what it puts to the right of
/// them: the right edge of the labels that run on past a bar's end
/// (design/legend-opt-out.md, 4.7), and, for a chart with a legend, the
/// legend's right edge, so the legend does not run past the viewBox. A chart
/// without a legend has no legend edge to hold up. Every task counts, not only
/// the rows in the window, or the width would change as the chart scrolls.
pub(crate) fn chart_width_for(
    tasks: &[GanttTask],
    rows: Option<&[GanttRow]>,
    epoch: NaiveDate,
    options: &ChartOptions,
) -> f64 {
    let epoch = chart_epoch(epoch, options);
    let axis = XAxis::of(options);
    let label_w = axis.label_w;
    let total_days = chart_days(tasks, rows, epoch, options);
    let view_days = options.view.map(|_| total_days);
    // A chart that draws a column for every day (the `Day` scale, or weekend
    // bands) holds up the column of a milestone on its last day as well.
    let column_days = if options.scale == Scale::Day || options.weekend.0 != 0 {
        chart_drawn_days(tasks, epoch, total_days, options)
    } else {
        total_days
    };
    let days_w = chart_width_with_label(column_days as f64, label_w, axis.px_per_day);
    let legend_edge = match options.legend {
        LegendMode::Shown => {
            label_w + LEGEND_LEFT_PAD_PX + legend_layout().width + CHART_RIGHT_PADDING_PX
        }
        LegendMode::Hidden => 0.0,
    };
    // A milestone's diamond reaches half a bar past its day. The right padding
    // holds that for the default bar, not for a much taller one.
    let bar_h = options.bar_height.unwrap_or(BAR_H);
    // Only a milestone the view shows draws its diamond.
    let diamond_edge = tasks
        .iter()
        .filter(|t| is_zero_duration(t))
        .filter(|t| {
            matches!(
                Placement::of(t, None, epoch, view_days),
                Placement::Shown(_)
            )
        })
        .map(|t| axis.x(t.start_days(epoch)) + bar_h / 2.0)
        .fold(0.0, f64::max);
    days_w
        .max(legend_edge)
        .max(diamond_edge)
        .max(labels_right_edge(
            &whole_in_view(tasks, epoch, view_days),
            epoch,
            total_days,
            &axis,
            options,
        ))
        .max(marks_right_edge(
            tasks, rows, epoch, &axis, view_days, bar_h,
        ))
}

/// The days the grid draws a line on. A view draws none on the day past its
/// last, whose label would name a day it does not show.
fn grid_days_in(
    scale: Scale,
    epoch: NaiveDate,
    total_days: i64,
    drawn_days: i64,
    options: &ChartOptions,
) -> Vec<NaiveDate> {
    let days = grid_days_of(scale, epoch, total_days, drawn_days);
    match options.view {
        Some(_) => days
            .into_iter()
            .filter(|d| (*d - epoch).num_days() < total_days)
            .collect(),
        None => days,
    }
}

/// The progress line drawn `by_row` over every row of the plan. A task is done
/// only when its row's state is `done`: one at 100% that is not done bends to its
/// end, so a task left open shows.
fn by_row_progress_line(
    graph: &GanttGraph,
    rows: Option<&[GanttRow]>,
    plan: &RowPlan,
    axis: &XAxis,
    epoch: NaiveDate,
    today_x: f64,
) -> Vec<(f64, f64)> {
    let line_rows: Vec<(f64, f64, Option<f64>)> = plan
        .slots
        .iter()
        .map(|slot| {
            let bend = slot.task.and_then(|i| {
                let t = &graph.tasks[i];
                let state = task_row_of(rows, slot).and_then(|m| m.state);
                let done = state == Some(BarState::Done);
                by_row_bend(
                    axis.x(t.start_days(epoch)),
                    axis.x(t.end_days(epoch)),
                    t.progress(),
                    done,
                    today_x,
                )
            });
            (slot.y_top, slot.y_bottom(), bend)
        })
        .collect();
    progress_line_by_row(&line_rows, today_x)
}

/// The chart's first day: the view's, or the one the caller passed.
fn chart_epoch(epoch: NaiveDate, options: &ChartOptions) -> NaiveDate {
    options.view.map_or(epoch, |v| v.from)
}

/// The days the chart spans: the view's, or every task's and deadline's.
fn chart_days(
    tasks: &[GanttTask],
    rows: Option<&[GanttRow]>,
    epoch: NaiveDate,
    options: &ChartOptions,
) -> i64 {
    match options.view {
        Some(view) => view.days(),
        None => span_days(tasks, rows, epoch),
    }
}

/// The days the chart draws a column for (`drawn_days_of`). A view's days are
/// its own: a milestone past them is not drawn, and one inside is in them.
fn chart_drawn_days(
    tasks: &[GanttTask],
    epoch: NaiveDate,
    total_days: i64,
    options: &ChartOptions,
) -> i64 {
    match options.view {
        Some(_) => total_days,
        None => drawn_days_of(tasks, epoch, total_days),
    }
}

/// The tasks a view shows whole, whose percent labels are drawn; every task
/// without a view.
fn whole_in_view(tasks: &[GanttTask], epoch: NaiveDate, view_days: Option<i64>) -> Vec<GanttTask> {
    tasks
        .iter()
        .filter(|t| match Placement::of(t, None, epoch, view_days) {
            Placement::Shown(s) => !s.cut_left && !s.cut_right,
            _ => false,
        })
        .cloned()
        .collect()
}

/// Where a task sits against the days a view shows.
#[derive(Debug, Clone, Copy)]
enum Placement {
    /// Drawn, perhaps cut at the view's ends.
    Shown(ShownSpan),
    /// Wholly before the view; the day is its last.
    Before(NaiveDate),
    /// Wholly after the view; the day is its first.
    After(NaiveDate),
}

/// The part of a task a chart draws, in days from the epoch.
#[derive(Debug, Clone, Copy)]
struct ShownSpan {
    start: f64,
    end: f64,
    /// The bar runs on before the view's first day.
    cut_left: bool,
    /// The bar runs on past the view's last day.
    cut_right: bool,
}

impl Placement {
    /// Without a view every task is shown whole, at the days it always had.
    fn of(
        task: &GanttTask,
        row: Option<&TaskRow>,
        epoch: NaiveDate,
        view_days: Option<i64>,
    ) -> Self {
        let start = task.start_days(epoch);
        let end = task.end_days(epoch);
        let whole = ShownSpan {
            start,
            end,
            cut_left: false,
            cut_right: false,
        };
        let Some(days) = view_days else {
            return Placement::Shown(whole);
        };
        let days = days as f64;
        let is_point = row.is_some_and(|r| r.shape == BarShape::Point);
        if is_point || is_zero_duration(task) {
            // A dot or a milestone is a single day: in the view or not.
            let day = if is_point && end > start {
                end - 1.0
            } else {
                start
            };
            let date = epoch + Duration::days(day as i64);
            return if day < 0.0 {
                Placement::Before(date)
            } else if day >= days {
                Placement::After(date)
            } else {
                Placement::Shown(whole)
            };
        }
        if end <= 0.0 {
            Placement::Before(last_day(task, epoch))
        } else if start >= days {
            Placement::After(task.start)
        } else {
            Placement::Shown(ShownSpan {
                start: start.max(0.0),
                end: end.min(days),
                cut_left: start < 0.0,
                cut_right: end > days,
            })
        }
    }
}

/// The last day a task's bar covers.
fn last_day(task: &GanttTask, epoch: NaiveDate) -> NaiveDate {
    let end = task.end_days(epoch);
    let start = task.start_days(epoch);
    let last = if end > start { end - 1.0 } else { start };
    epoch + Duration::days(last as i64)
}

/// The mark at the edge of the view a task runs past: `← M/D` inside the
/// first day, `→ M/D` inside the last.
fn overflow_mark(
    placement: &Placement,
    day: NaiveDate,
    axis: &XAxis,
    total_days: i64,
    cy: f64,
) -> TextPrim {
    let label = format!("{}/{}", day.month(), day.day());
    let (x, anchor, content) = match placement {
        Placement::Before(_) => (
            axis.x(0.0) + OVERFLOW_INSET_PX,
            TextAnchor::Start,
            format!("← {label}"),
        ),
        _ => (
            axis.x(total_days as f64) - OVERFLOW_INSET_PX,
            TextAnchor::End,
            format!("→ {label}"),
        ),
    };
    TextPrim {
        x,
        y: cy,
        content,
        fill: Some(ColorId::Note),
        font_size: Some(GRID_LABEL_FONT_PX),
        font_weight: None,
        anchor: Some(anchor),
        baseline: TextBaseline::Middle,
        semantic: TextSemantic::Overflow,
    }
}

/// The mark of a task wholly outside the view, in the warn colour when its
/// row is in the warn state.
fn outside_mark(
    placement: &Placement,
    day: NaiveDate,
    axis: &XAxis,
    total_days: i64,
    cy: f64,
    row: Option<&TaskRow>,
) -> TextPrim {
    let mut mark = overflow_mark(placement, day, axis, total_days, cy);
    if row.and_then(|r| r.state) == Some(BarState::Warn) {
        mark.fill = Some(ColorId::WarnStroke);
    }
    mark
}

/// Where the note of a task before the view starts. The mark stands for the
/// bar the view does not show, so the note sits right of the mark, as a note
/// sits right of its bar, and moves right of the diamond only when it would run
/// over it.
fn outside_note_x(mark: &TextPrim, g: &MarkGeometry, note: &str) -> f64 {
    let mark_right = text_right_edge(mark.x, mark.anchor, &mark.content, GRID_LABEL_FONT_PX, None);
    note_x_beside(mark_right, Some(note), g.diamond_cx, None)
}

/// The diamond of a hard deadline centred on `dx`.
fn hard_deadline_prim(dx: f64, cy: f64, tier: ProgressTier) -> Primitive {
    let half = HARD_DEADLINE_HALF_PX;
    Primitive::Polygon(PolygonPrim {
        points: vec![
            (dx, cy - half),
            (dx + half, cy),
            (dx, cy + half),
            (dx - half, cy),
        ],
        fill: ColorId::HardDeadline,
        stroke: ColorId::HardDeadline,
        stroke_width: 1.0,
        tier,
        semantic: PolygonSemantic::HardDeadline,
    })
}

/// The hit area of a task drawn as its mark alone.
fn overflow_bbox(mark: &TextPrim, y: f64, bar_h: f64) -> BBox {
    let w = text_width_weighted_px(&mark.content, GRID_LABEL_FONT_PX, None);
    let x = match mark.anchor {
        Some(TextAnchor::End) => mark.x - w,
        _ => mark.x,
    };
    BBox {
        x,
        y,
        width: w,
        height: bar_h,
    }
}

/// The tooltip of a task's group.
fn task_tooltip(task: &GanttTask) -> String {
    let prog_pct = task.progress_pct.clamp(0, 100);
    let end_label = task.end.map(format_date).unwrap_or_else(|| {
        format_date(
            task.start
                .checked_add_signed(Duration::days(1))
                .unwrap_or(task.start),
        )
    });
    format!(
        "{}: {} – {} ({prog_pct}%)",
        task.title,
        format_date(task.start),
        end_label,
    )
}

/// The days the chart spans: through the last task's end, and through the
/// last hard deadline a task row gives, so its diamond is on the chart. A chart
/// without marks spans what it spanned before them.
fn span_days(tasks: &[GanttTask], rows: Option<&[GanttRow]>, epoch: NaiveDate) -> i64 {
    let deadlines = rows
        .into_iter()
        .flatten()
        .filter_map(|r| match r {
            GanttRow::Task(t) => t.hard_deadline,
            GanttRow::Group { .. } => None,
        })
        .map(|d| (d - epoch).num_days() + 1);
    deadlines.fold(total_days_of(tasks, epoch), i64::max)
}

/// The task row drawn on `slot`, when the caller's row list gives one.
fn task_row_of<'a>(rows: Option<&'a [GanttRow]>, slot: &super::rows::Slot) -> Option<&'a TaskRow> {
    match rows?.get(slot.row?)? {
        GanttRow::Task(t) => Some(t),
        GanttRow::Group { .. } => None,
    }
}

/// The radius of a `point` row's dot: a quarter of the bar, so the dot is half
/// the bar across. Drawing the dot and placing its note both read this, so a
/// note starts right of the dot however tall the bar or narrow the day.
fn point_radius(bar_h: f64) -> f64 {
    bar_h / 4.0
}

/// Where a task row's marks sit across the chart. Drawing and the width both
/// read this, so a note cannot run past the chart.
#[derive(Debug, Clone, Copy)]
struct MarkGeometry {
    /// The centre of the dot of a `point` row.
    point_cx: Option<f64>,
    /// The centre of the hard deadline's diamond: the middle of its day.
    diamond_cx: Option<f64>,
    /// Where the note starts: right of the bar (or dot), or right of the
    /// diamond when the note would run over it.
    note_x: f64,
}

impl MarkGeometry {
    fn of(
        task: &GanttTask,
        row: &TaskRow,
        axis: &XAxis,
        epoch: NaiveDate,
        view_days: Option<i64>,
        bar_h: f64,
    ) -> Self {
        let start = task.start_days(epoch);
        let end = task.end_days(epoch);
        // The middle of the span's last day; a span of no days is its start day.
        let last_day_mid = if end > start {
            axis.x(end) - axis.px_per_day / 2.0
        } else {
            axis.x(start) + axis.px_per_day / 2.0
        };
        let point_cx = (row.shape == BarShape::Point).then_some(last_day_mid);
        // A deadline before the chart's first day has no column to sit in.
        let diamond_cx = row
            .hard_deadline
            .filter(|d| *d >= epoch)
            .filter(|d| view_days.is_none_or(|days| (*d - epoch).num_days() < days))
            .map(|d| axis.day_x(epoch, d) + axis.px_per_day / 2.0);
        let bar_right = match point_cx {
            Some(cx) => cx + point_radius(bar_h),
            None => axis.x(end).max(axis.x(start)),
        };
        // A bar a view cuts ends at the view's last day.
        let bar_right = match view_days {
            Some(days) => bar_right.min(axis.x(days as f64)),
            None => bar_right,
        };
        Self {
            point_cx,
            diamond_cx,
            note_x: note_x_beside(bar_right, row.note.as_deref(), diamond_cx, point_cx),
        }
    }
}

/// Where a note starts. A note speaks of its bar, so it sits right of the bar
/// (`bar_right`); only when it would run over the hard deadline's diamond does
/// it move right of the diamond. For a dot, the line from the dot to a later
/// diamond is in the way as the diamond is.
fn note_x_beside(
    bar_right: f64,
    note: Option<&str>,
    diamond_cx: Option<f64>,
    point_cx: Option<f64>,
) -> f64 {
    let x = bar_right + NOTE_GAP_PX;
    let Some(c) = diamond_cx else {
        return x;
    };
    let half = HARD_DEADLINE_HALF_PX;
    let left = match point_cx {
        Some(px) if c > px => px,
        _ => c - half,
    };
    let right = c + half;
    let width = note.map_or(0.0, |n| text_width_weighted_px(n, GRID_LABEL_FONT_PX, None));
    if x < right && x + width > left {
        right + NOTE_GAP_PX
    } else {
        x
    }
}

/// A row's note at `x` and the band under it. The band takes the note's width
/// from the same estimate the chart's width does, so it adds no width.
fn note_with_band(x: f64, cy: f64, note: &str, state: Option<BarState>) -> [Primitive; 2] {
    let width = text_width_weighted_px(note, GRID_LABEL_FONT_PX, None);
    let height = GRID_LABEL_FONT_PX + 2.0 * NOTE_BAND_PAD_PX;
    [
        Primitive::Rect(RectPrim {
            x: x - NOTE_BAND_PAD_PX,
            y: cy - height / 2.0,
            width: width + 2.0 * NOTE_BAND_PAD_PX,
            height,
            fill: ColorId::NoteBg,
            rx: Some(NOTE_BAND_PAD_PX),
            semantic: RectSemantic::NoteBackground,
        }),
        Primitive::Text(TextPrim {
            x,
            y: cy,
            content: note.to_string(),
            fill: Some(match state {
                Some(BarState::Warn) => ColorId::WarnStroke,
                _ => ColorId::Note,
            }),
            font_size: Some(GRID_LABEL_FONT_PX),
            font_weight: None,
            anchor: None,
            baseline: TextBaseline::Middle,
            semantic: TextSemantic::BarNote,
        }),
    ]
}

/// The furthest right a task row's diamond or note reaches, with the right
/// padding past it.
fn marks_right_edge(
    tasks: &[GanttTask],
    rows: Option<&[GanttRow]>,
    epoch: NaiveDate,
    axis: &XAxis,
    view_days: Option<i64>,
    bar_h: f64,
) -> f64 {
    let Some(rows) = rows else {
        return 0.0;
    };
    let mut by_id: HashMap<&str, &GanttTask> = HashMap::new();
    for t in tasks {
        by_id.entry(t.id.as_str()).or_insert(t);
    }
    rows.iter()
        .filter_map(|r| match r {
            GanttRow::Task(row) => Some((row, *by_id.get(row.task_id.as_str())?)),
            GanttRow::Group { .. } => None,
        })
        .map(|(row, task)| {
            let g = MarkGeometry::of(task, row, axis, epoch, view_days, bar_h);
            let diamond = g.diamond_cx.map_or(0.0, |c| c + HARD_DEADLINE_HALF_PX);
            // A task wholly outside a view: its note only before it, right of
            // the mark at the view's first day.
            let note_x = match Placement::of(task, Some(row), epoch, view_days) {
                Placement::Shown(_) => Some(g.note_x),
                placement @ Placement::Before(day) => view_days.map(|days| {
                    let mark = outside_mark(&placement, day, axis, days, 0.0, Some(row));
                    outside_note_x(&mark, &g, row.note.as_deref().unwrap_or(""))
                }),
                Placement::After(_) => None,
            };
            let note = match (note_x, row.note.as_deref()) {
                (Some(x), Some(n)) => text_right_edge(x, None, n, GRID_LABEL_FONT_PX, None),
                _ => 0.0,
            };
            diamond.max(note) + CHART_RIGHT_PADDING_PX
        })
        .fold(0.0, f64::max)
}

/// The fill of a bar in `state`; no state is the fill before states existed.
fn bar_fill_color(state: Option<BarState>) -> ColorId {
    match state {
        None | Some(BarState::Normal) => ColorId::BarBg,
        Some(BarState::Done) => ColorId::DoneBg,
        Some(BarState::Warn) => ColorId::WarnBg,
    }
}

/// The outline of a bar in `state`, and the colour of a dot.
fn outline_color(state: Option<BarState>) -> ColorId {
    match state {
        None | Some(BarState::Normal) => ColorId::BarStroke,
        Some(BarState::Done) => ColorId::DoneStroke,
        Some(BarState::Warn) => ColorId::WarnStroke,
    }
}

/// A rounded rectangle as an SVG path, for an outline drawn over a bar.
fn rounded_rect_path(x: f64, y: f64, w: f64, h: f64, r: f64) -> String {
    let r = r.min(w / 2.0).min(h / 2.0).max(0.0);
    let n = -r;
    format!(
        "M {x0} {y} H {x1} a {r} {r} 0 0 1 {r} {r} V {y1} a {r} {r} 0 0 1 {n} {r} H {x0} a {r} {r} 0 0 1 {n} {n} V {y0} a {r} {r} 0 0 1 {r} {n} Z",
        x0 = x + r,
        x1 = x + w - r,
        y0 = y + r,
        y1 = y + h - r,
    )
}

/// Where a day sits across the chart: after the title column, `px_per_day`
/// for each day from the epoch. Every x the chart draws is read from here.
#[derive(Debug, Clone, Copy)]
pub(crate) struct XAxis {
    pub label_w: f64,
    pub px_per_day: f64,
}

impl XAxis {
    pub fn of(options: &ChartOptions) -> Self {
        Self {
            label_w: label_width(options.row_labels, options.bar_height.unwrap_or(BAR_H)),
            px_per_day: options.px_per_day.unwrap_or(PX_PER_DAY),
        }
    }

    /// The x of a point `days` days after the epoch.
    pub fn x(&self, days: f64) -> f64 {
        self.label_w + days * self.px_per_day
    }

    /// The x of the start of `day`.
    pub fn day_x(&self, epoch: NaiveDate, day: NaiveDate) -> f64 {
        self.x((day - epoch).num_days() as f64)
    }

    /// The width of `days` days.
    pub fn len(&self, days: f64) -> f64 {
        days * self.px_per_day
    }
}

/// How wide the title column is: `LABEL_W` with the row labels shown, and with
/// them hidden only the room a milestone on the first day needs for the left
/// half of its diamond: half the bar's height (`HIDDEN_LABELS_LEFT_PX` for the
/// default bar). Every x the chart draws starts from this.
///
/// With the labels shown the column is `LABEL_W` unless half the bar is wider,
/// so a first-day diamond of a bar taller than `2 * LABEL_W` is not cut at the
/// left either. The larger of the two, not their sum: the column only grows
/// for a bar that would otherwise be cut, and every other chart keeps its x.
pub(crate) fn label_width(row_labels: LabelMode, bar_h: f64) -> f64 {
    match row_labels {
        LabelMode::Shown => LABEL_W.max(bar_h / 2.0),
        LabelMode::Hidden => bar_h / 2.0,
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

fn month_label(day: NaiveDate) -> String {
    format!("{}/{}", day.year(), day.month())
}

/// The days the grid of `scale` draws a line at: the Mondays for `Week`, the
/// first of each month (and the epoch, so the first month has its label) for
/// `Month`. `Day` draws the Monday lines and labels every day. The Mondays
/// reach the end of the span already (`grid_days` counts its last boundary);
/// the months go through `drawn_days`, so a milestone's day is in them.
fn grid_days_of(
    scale: Scale,
    epoch: NaiveDate,
    total_days: i64,
    drawn_days: i64,
) -> Vec<NaiveDate> {
    match scale {
        Scale::Week | Scale::Day => grid_days(epoch, total_days),
        Scale::Month => calendar_days(epoch, drawn_days)
            .filter(|d| d.day() == 1 || *d == epoch)
            .collect(),
    }
}

/// The label at a grid line of the `Week` or `Month` scale.
fn grid_text_of(
    axis: &XAxis,
    epoch: NaiveDate,
    day: NaiveDate,
    scale: Scale,
    header_h: f64,
) -> TextPrim {
    TextPrim {
        x: axis.day_x(epoch, day),
        y: header_h / 2.0 + 4.0,
        content: match scale {
            Scale::Month => month_label(day),
            _ => grid_label(day),
        },
        fill: Some(ColorId::GridLabel),
        font_size: Some(GRID_LABEL_FONT_PX),
        font_weight: None,
        anchor: None,
        baseline: TextBaseline::Auto,
        semantic: TextSemantic::GridLabel,
    }
}

/// The days the chart draws a column for, counted from the epoch. A bar's end
/// is the boundary after its last day, so `total_days` covers every bar; a
/// milestone sits on its day and ends where it starts, so a milestone on the
/// last day (or alone in the chart) would fall outside `total_days`. Its day
/// counts here, for the labels of each day, the months and the weekend bands.
fn drawn_days_of(tasks: &[GanttTask], epoch: NaiveDate, total_days: i64) -> i64 {
    tasks
        .iter()
        .filter(|t| is_zero_duration(t))
        .map(|t| t.start_days(epoch).floor() as i64 + 1)
        .fold(total_days, i64::max)
}

/// Every day from the epoch through `total_days` days, the days the chart
/// spans. Stops instead of panicking at the end of the calendar.
fn calendar_days(epoch: NaiveDate, total_days: i64) -> impl Iterator<Item = NaiveDate> {
    (0..total_days.max(0)).map_while(move |i| epoch.checked_add_signed(Duration::days(i)))
}

/// The header's height: `header_height_px` when the caller sets it, and
/// otherwise `DAY_SCALE_HEADER_H` for the `Day` scale, whose three tiers do
/// not fit `HEADER_H`, and `HEADER_H` for the others. The rows and the width
/// both read this, so the two cannot disagree on whether the month is drawn.
pub(crate) fn header_height_of(options: &ChartOptions) -> f64 {
    options.header_height.unwrap_or(match options.scale {
        Scale::Day => DAY_SCALE_HEADER_H,
        Scale::Week | Scale::Month => HEADER_H,
    })
}

/// The labels of the `Day` scale: over each day its number, under it the
/// weekday, and above the first day of each month (and the first day drawn)
/// `YYYY/M`. The month sits at the top, the number and weekday at the foot of
/// the header, so a taller header leaves its room in the middle.
///
/// A header the caller sets lower than `DAY_SCALE_HEADER_H` has no room for
/// the month above the day, so the month is left out rather than drawn over
/// the day: the day and weekday still read, two labels on top of each other
/// do not.
fn day_texts(
    axis: &XAxis,
    epoch: NaiveDate,
    total_days: i64,
    header_h: f64,
    options: &ChartOptions,
) -> Vec<TextPrim> {
    let mut out = Vec::new();
    for day in calendar_days(epoch, total_days) {
        let x = axis.day_x(epoch, day);
        let mid = x + axis.px_per_day / 2.0;
        let weekday = day.weekday().num_days_from_monday() as usize;
        let weekday_label = match &options.weekday_labels {
            Some(labels) => labels[weekday].clone(),
            None => DEFAULT_WEEKDAY_LABELS[weekday].to_string(),
        };
        if header_h >= DAY_SCALE_HEADER_H && (day == epoch || day.day() == 1) {
            out.push(TextPrim {
                x: x + 2.0,
                y: 12.0,
                content: month_label(day),
                fill: Some(ColorId::GridLabel),
                font_size: Some(GRID_LABEL_FONT_PX),
                font_weight: None,
                anchor: None,
                baseline: TextBaseline::Auto,
                semantic: TextSemantic::MonthLabel,
            });
        }
        out.push(TextPrim {
            x: mid,
            y: header_h - 16.0,
            content: day.day().to_string(),
            fill: None,
            font_size: Some(GRID_LABEL_FONT_PX),
            font_weight: None,
            anchor: Some(TextAnchor::Middle),
            baseline: TextBaseline::Auto,
            semantic: TextSemantic::DayNumber,
        });
        out.push(TextPrim {
            x: mid,
            y: header_h - 4.0,
            content: weekday_label,
            fill: Some(ColorId::GridLabel),
            font_size: Some(GRID_LABEL_FONT_PX),
            font_weight: None,
            anchor: Some(TextAnchor::Middle),
            baseline: TextBaseline::Auto,
            semantic: TextSemantic::Weekday,
        });
    }
    out
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
fn percent_label(task: &GanttTask, epoch: NaiveDate, axis: &XAxis, bar_h: f64) -> PercentLabel {
    let x = axis.x(task.start_days(epoch));
    let content = format!("{}%", task.progress_pct.clamp(0, 100));
    if is_zero_duration(task) {
        // Clear of the diamond, whose half-width is half the bar drawn.
        return PercentLabel {
            x: x + bar_h / 2.0 + 6.0,
            anchor: None,
            fill: ColorId::ProgressTextOnBg,
            content,
        };
    }
    let w = axis.len(task_bar_width_days(task, epoch));
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
fn labels_right_edge(
    tasks: &[GanttTask],
    epoch: NaiveDate,
    total_days: i64,
    axis: &XAxis,
    options: &ChartOptions,
) -> f64 {
    // Hidden percent labels hold nothing up.
    let shown_tasks = match options.percent_labels {
        LabelMode::Shown => tasks,
        LabelMode::Hidden => &[],
    };
    let task_edges = shown_tasks.iter().map(|task| {
        let label = percent_label(task, epoch, axis, options.bar_height.unwrap_or(BAR_H));
        text_right_edge(
            label.x,
            label.anchor,
            &label.content,
            PROGRESS_LABEL_FONT_PX,
            Some(PROGRESS_LABEL_FONT_WEIGHT),
        )
    });
    // The grid's labels, read from what the grid draws so the two cannot differ.
    // The header's height moves no label sideways, but it decides whether the
    // `Day` scale draws its months, so that scale reads the height drawn.
    let drawn_days = chart_drawn_days(tasks, epoch, total_days, options);
    let grid_texts: Vec<TextPrim> = match options.scale {
        Scale::Week | Scale::Month => {
            grid_days_in(options.scale, epoch, total_days, drawn_days, options)
                .into_iter()
                .map(|day| grid_text_of(axis, epoch, day, options.scale, HEADER_H))
                .collect()
        }
        Scale::Day => day_texts(axis, epoch, drawn_days, header_height_of(options), options),
    };
    let grid = grid_texts
        .iter()
        .map(|t| text_right_edge(t.x, t.anchor, &t.content, GRID_LABEL_FONT_PX, None));
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
fn legend_layer(chart_h: f64, legend: &LegendLayout, label_w: f64) -> Layer {
    let legend_y1 = chart_h - LEGEND_H + 14.0;
    let legend_x = label_w + LEGEND_LEFT_PAD_PX;
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
