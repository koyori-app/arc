//! The lane layout: one lane per name, the bars of the tasks that belong to it packed
//! into the lane's rows. A chart is drawn this way only when it is given `lanes`; a
//! chart without them goes through `build_display_list` and is not touched by anything
//! here.
//!
//! - A lane is as tall as the rows its bars need (`ROW_H` each, at least one). The
//!   heights differ lane to lane, so a position is read off the running sum of the
//!   heights (`LaneLayout::tops`), never as `index * ROW_H`, and the window of lanes a
//!   scroll shows is found by searching that sum.
//! - A task belongs to every lane its `lane_ids` names and is drawn once in each. A task
//!   that belongs to none is not drawn. An id no lane carries is ignored.
//! - Inside a lane the bars are put in order of start day, end day, id and input
//!   position, and each goes into the topmost row whose last bar (or that bar's percent
//!   label, whichever reaches further right) ends `LANE_PACK_GAP_PX` before the bar
//!   begins; no such row opens a new one. The same input gives the same rows.
//! - Dependency arrows and the progress status line are not drawn: a task in two lanes
//!   has no single place for an arrow to start or end, and the line runs through one
//!   bar per row. The legend leaves out the progress-line sample and keeps its height.

use chrono::NaiveDate;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::graph::GanttTask;

use super::build::{
    background_layer, bar_parts, grid_layer, is_zero_duration, labels_right_edge, legend_layer,
    legend_layout, percent_label, task_bar_width_days, text_right_edge, today_layer, today_x_of,
    total_days_of, BarParts,
};
use super::constants::*;
use super::text_metrics::{text_width_px, truncate_to_width};
use super::types::*;

/// One lane: the name that is drawn in the left column, and the id tasks name in
/// `lane_ids`. The order of the lanes given is the order they are drawn, top first.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Lane {
    pub id: String,
    pub label: String,
}

/// Which row of its lane a task's bar is on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Placement {
    /// Index into the tasks the chart was given.
    pub task: usize,
    /// Row inside the lane, 0 at the top.
    pub row: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LaneBlock {
    /// How many rows the lane has: at least one.
    pub rows: usize,
    /// The bars, in the order they were packed (start day, end day, id).
    pub placements: Vec<Placement>,
}

/// Where each lane starts and how tall it is.
#[derive(Debug, Clone, PartialEq)]
pub struct LaneLayout {
    pub blocks: Vec<LaneBlock>,
    /// `tops[i]` is the distance from the top of the first lane to the top of lane `i`,
    /// and the last entry is the total height. It has one more entry than there are
    /// lanes and only grows, which is what lets a position be searched for.
    pub tops: Vec<f64>,
}

impl LaneLayout {
    /// The height the lanes take together.
    pub fn height(&self) -> f64 {
        self.tops.last().copied().unwrap_or(0.0)
    }

    pub fn lane_height(&self, lane: usize) -> f64 {
        self.tops[lane + 1] - self.tops[lane]
    }
}

/// Everything about a lane chart that has to be known before anything is drawn, and
/// that the canvas size guard asks about too.
#[derive(Debug, Clone)]
pub struct LanePlan {
    /// The first day of the chart: the earliest start among the tasks that are drawn.
    pub epoch: NaiveDate,
    /// The tasks that belong to at least one lane, in input order.
    pub drawn: Vec<GanttTask>,
    pub layout: LaneLayout,
    /// Per lane, the indices into the input tasks of the tasks that belong to it.
    members: Vec<Vec<usize>>,
}

impl LanePlan {
    /// How many bars the chart draws: a task counts once per lane it is in.
    pub fn bar_count(&self) -> usize {
        self.members.iter().map(Vec::len).sum()
    }
}

/// The left and right edge of what a bar takes up in its row: the bar (or the diamond of
/// a zero-length task) and, when it runs on past the bar, the percent label.
fn extent(task: &GanttTask, epoch: NaiveDate) -> (f64, f64) {
    let x = LABEL_W + task.start_days(epoch) * PX_PER_DAY;
    let (left, right) = if is_zero_duration(task) {
        (x - BAR_H / 2.0, x + BAR_H / 2.0)
    } else {
        (x, x + task_bar_width_days(task, epoch) * PX_PER_DAY)
    };
    let label = percent_label(task, epoch);
    let label_right = text_right_edge(
        label.x,
        label.anchor,
        &label.content,
        PROGRESS_LABEL_FONT_PX,
        Some(PROGRESS_LABEL_FONT_WEIGHT),
    );
    (left, right.max(label_right))
}

/// Puts a lane's tasks into rows. Deterministic: see the module comment.
fn pack_lane(tasks: &[GanttTask], members: &[usize], epoch: NaiveDate) -> LaneBlock {
    let mut order = members.to_vec();
    order.sort_by(|&a, &b| {
        let (ta, tb) = (&tasks[a], &tasks[b]);
        ta.start_days(epoch)
            .total_cmp(&tb.start_days(epoch))
            .then(ta.end_days(epoch).total_cmp(&tb.end_days(epoch)))
            .then_with(|| ta.id.cmp(&tb.id))
            .then(a.cmp(&b))
    });
    // The right edge of the last bar in each row so far.
    let mut row_right: Vec<f64> = Vec::new();
    let mut placements = Vec::with_capacity(order.len());
    for task in order {
        let (left, right) = extent(&tasks[task], epoch);
        let row = row_right
            .iter()
            .position(|&edge| left >= edge + LANE_PACK_GAP_PX);
        let row = match row {
            Some(row) => {
                row_right[row] = right;
                row
            }
            None => {
                row_right.push(right);
                row_right.len() - 1
            }
        };
        placements.push(Placement { task, row });
    }
    LaneBlock {
        rows: row_right.len().max(1),
        placements,
    }
}

/// Settles which tasks are drawn, where the chart starts, and the shape of every lane.
///
/// `task_lanes` holds, for each task in `tasks`, the lane ids it names. A lane id that
/// appears twice in `lanes` is the first one; a task that names a lane twice is in it once.
pub fn plan_lanes(tasks: &[GanttTask], task_lanes: &[Vec<String>], lanes: &[Lane]) -> LanePlan {
    let mut lane_of: HashMap<&str, usize> = HashMap::with_capacity(lanes.len());
    for (i, lane) in lanes.iter().enumerate() {
        lane_of.entry(lane.id.as_str()).or_insert(i);
    }
    let mut members: Vec<Vec<usize>> = vec![Vec::new(); lanes.len()];
    let mut is_drawn = vec![false; tasks.len()];
    for (i, ids) in task_lanes.iter().enumerate().take(tasks.len()) {
        for id in ids {
            let Some(&lane) = lane_of.get(id.as_str()) else {
                continue;
            };
            // Tasks are visited in order, so a task already in this lane is its last member.
            if members[lane].last() != Some(&i) {
                members[lane].push(i);
            }
            is_drawn[i] = true;
        }
    }
    let drawn: Vec<GanttTask> = tasks
        .iter()
        .zip(&is_drawn)
        .filter(|&(_, &on)| on)
        .map(|(task, _)| task.clone())
        .collect();
    let epoch = drawn
        .iter()
        .map(|t| t.start)
        .min()
        .or_else(|| tasks.iter().map(|t| t.start).min())
        .unwrap_or_default();

    let blocks: Vec<LaneBlock> = members
        .iter()
        .map(|ids| pack_lane(tasks, ids, epoch))
        .collect();
    let mut tops = Vec::with_capacity(blocks.len() + 1);
    let mut sum = 0.0;
    tops.push(sum);
    for block in &blocks {
        sum += block.rows as f64 * ROW_H;
        tops.push(sum);
    }
    LanePlan {
        epoch,
        drawn,
        layout: LaneLayout { blocks, tops },
        members,
    }
}

/// How wide a lane chart is drawn. The canvas size guard reads this too.
///
/// As wide as the days, held up by the labels that run past a bar, the titles above the
/// bars, and the legend's tier row. Every drawn bar counts, not only the lanes in view,
/// or the width would change as the chart scrolls.
pub fn lane_chart_width(plan: &LanePlan, tasks: &[GanttTask], legend_mode: LegendMode) -> f64 {
    let total_days = total_days_of(&plan.drawn, plan.epoch);
    let days_w = chart_width(total_days as f64);
    let legend_edge = match legend_mode {
        LegendMode::Shown => {
            LABEL_W + LEGEND_LEFT_PAD_PX + legend_layout().tier_width + CHART_RIGHT_PADDING_PX
        }
        LegendMode::Hidden => 0.0,
    };
    let title_edge = plan
        .layout
        .blocks
        .iter()
        .flat_map(|block| &block.placements)
        .map(|p| {
            let task = &tasks[p.task];
            let (left, _) = extent(task, plan.epoch);
            let w = text_width_px(&task.title, LANE_TITLE_FONT_PX).min(LANE_TITLE_MAX_W_PX);
            left + w
        })
        .fold(0.0, f64::max);
    days_w
        .max(legend_edge)
        .max(labels_right_edge(&plan.drawn, plan.epoch, total_days))
        .max(title_edge)
}

/// The band of the chart, measured from the top of the first lane, that a scroll shows,
/// widened by the same buffer the row layout keeps. `None` means draw everything.
pub fn visible_band(scroll: Option<ScrollViewport>) -> Option<(f64, f64)> {
    let scroll = scroll?;
    let buffer = ROW_BUFFER as f64 * ROW_H;
    let top = scroll.scroll_y - HEADER_H - buffer;
    let bottom = scroll.scroll_y + scroll.client_height - HEADER_H + buffer;
    Some((top, bottom))
}

/// The lanes `[first, end)` that reach into `band`. The tops only grow, so both ends are
/// found by binary search, not by dividing by a row height (the lanes differ in height).
/// A band that misses every lane gives an empty range.
pub fn visible_lanes(tops: &[f64], band: (f64, f64)) -> std::ops::Range<usize> {
    let lanes = tops.len().saturating_sub(1);
    let (top, bottom) = band;
    // The lane holding `top` is the one just before the first top that is past it.
    let first = tops.partition_point(|&t| t <= top).saturating_sub(1);
    // Lanes whose top is above `bottom`.
    let end = tops[..lanes].partition_point(|&t| t < bottom);
    if first >= end {
        return 0..0;
    }
    first..end
}

/// Whether the row `row` of a lane that starts at `lane_top` reaches into `band`.
fn row_in_band(lane_top: f64, row: usize, band: Option<(f64, f64)>) -> bool {
    let Some((top, bottom)) = band else {
        return true;
    };
    let row_top = lane_top + row as f64 * ROW_H;
    row_top < bottom && row_top + ROW_H > top
}

/// Builds the display list of a lane chart. `task_lanes[i]` is the lane ids of `tasks[i]`.
pub fn build_lane_display_list(
    tasks: &[GanttTask],
    task_lanes: &[Vec<String>],
    lanes: &[Lane],
    options: &ChartOptions,
) -> DisplayList {
    let ChartOptions {
        today,
        scroll_viewport,
        legend: legend_mode,
    } = *options;
    let plan = plan_lanes(tasks, task_lanes, lanes);
    let epoch = plan.epoch;
    let layout = &plan.layout;

    let legend = (legend_mode == LegendMode::Shown).then(legend_layout);
    let chart_w = lane_chart_width(&plan, tasks, legend_mode);
    let chart_h = layout.height() + chart_chrome_h(legend_mode);
    let total_days = total_days_of(&plan.drawn, epoch);

    let viewport = Viewport {
        width: chart_w,
        height: chart_h,
        label_width: LABEL_W,
        header_height: HEADER_H,
        row_height: ROW_H,
    };

    let band = visible_band(scroll_viewport);
    let lane_range = match band {
        Some(band) => visible_lanes(&layout.tops, band),
        None => 0..lanes.len(),
    };

    let mut layers = Vec::new();

    // The stripe behind every other lane, and the lane names in the left column, go in the
    // background layer: it is drawn first, under the grid lines and the bars.
    let mut background = background_layer(chart_w);
    for lane in lane_range.clone() {
        let top = HEADER_H + layout.tops[lane];
        let height = layout.lane_height(lane);
        if lane % 2 == 1 {
            background.primitives.push(Primitive::Rect(RectPrim {
                x: 0.0,
                y: top,
                width: chart_w,
                height,
                fill: ColorId::HeaderBg,
                rx: None,
                semantic: RectSemantic::HeaderBackground,
            }));
        }
        if !lanes[lane].label.is_empty() {
            background.primitives.push(Primitive::Text(TextPrim {
                x: LABEL_W - LABEL_GAP_PX,
                y: top + height / 2.0,
                content: truncate_to_width(
                    &lanes[lane].label,
                    DEFAULT_FONT_SIZE_PX,
                    ROW_LABEL_MAX_W_PX,
                ),
                fill: None,
                font_size: None,
                font_weight: None,
                anchor: Some(TextAnchor::End),
                baseline: TextBaseline::Middle,
                semantic: TextSemantic::LaneLabel,
            }));
        }
    }
    layers.push(background);
    layers.push(grid_layer(epoch, total_days, chart_h));

    let mut bar_prims = Vec::new();
    let mut task_bboxes = Vec::new();
    for lane in lane_range {
        let block = &layout.blocks[lane];
        let lane_top = layout.tops[lane];
        // The first row of this lane, counted from the top over all lanes.
        let first_slot = (lane_top / ROW_H).round() as u32;
        // The next bar to the right in the same row bounds how far a title may run.
        let mut next_left: HashMap<usize, f64> = HashMap::new();
        let mut title_room: Vec<f64> = vec![LANE_TITLE_MAX_W_PX; block.placements.len()];
        for (i, p) in block.placements.iter().enumerate().rev() {
            let (left, _) = extent(&tasks[p.task], epoch);
            if let Some(&next) = next_left.get(&p.row) {
                title_room[i] = (next - left - LANE_TITLE_GAP_PX).clamp(0.0, LANE_TITLE_MAX_W_PX);
            }
            next_left.insert(p.row, left);
        }
        for (i, p) in block.placements.iter().enumerate() {
            if !row_in_band(lane_top, p.row, band) {
                continue;
            }
            let task = &tasks[p.task];
            let slot_top = HEADER_H + lane_top + p.row as f64 * ROW_H;
            let y = slot_top + BAR_PAD;
            let BarParts {
                mut children,
                tooltip,
                x: _,
                w: _,
            } = bar_parts(task, epoch, y);
            let (left, _) = extent(task, epoch);
            let visual_right = if is_zero_duration(task) {
                left + BAR_H
            } else {
                left + task_bar_width_days(task, epoch) * PX_PER_DAY
            };

            // The title sits above the bar, in the padding over it, and is cut at the next
            // bar in the row so it never runs into it.
            let title = truncate_to_width(&task.title, LANE_TITLE_FONT_PX, title_room[i]);
            if title_room[i] >= LANE_TITLE_FONT_PX && !title.is_empty() {
                children.push(Primitive::Text(TextPrim {
                    x: left,
                    y: slot_top + BAR_PAD / 2.0,
                    content: title,
                    fill: None,
                    font_size: Some(LANE_TITLE_FONT_PX),
                    font_weight: None,
                    anchor: None,
                    baseline: TextBaseline::Middle,
                    semantic: TextSemantic::BarTitle,
                }));
            }

            let bbox = BBox {
                x: left,
                y,
                width: visual_right - left,
                height: BAR_H,
            };
            task_bboxes.push(TaskBBox {
                task_id: task.id.clone(),
                row: first_slot + p.row as u32,
                bbox: bbox.clone(),
                bar_bbox: bbox.clone(),
            });
            bar_prims.push(Primitive::Group(GroupPrim {
                task_id: Some(task.id.clone()),
                tooltip: Some(tooltip),
                bbox,
                children,
            }));
        }
    }
    layers.push(Layer {
        kind: LayerKind::Bars,
        primitives: bar_prims,
    });

    if let Some(x) = today_x_of(today, epoch, chart_w) {
        layers.push(today_layer(x, chart_h));
    }
    if let Some(legend) = &legend {
        layers.push(legend_layer(chart_h, legend, false));
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
            description: "Task schedule with progress bars grouped in lanes".to_string(),
            task_bboxes,
            primitive_count,
            element_count_estimate: primitive_count,
        },
    }
}
