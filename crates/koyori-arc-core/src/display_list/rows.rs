//! Where each row of a chart sits. Without a row list from the caller the rows
//! are the tasks, placed by `layout::assign_rows`, all `ROW_H` tall under a
//! `HEADER_H` header: the chart drawn before row lists existed. With one, the
//! rows are the list, in its order, at the heights it asks for
//! (docs/design/external-rows.md).

use std::collections::HashMap;

use crate::graph::{GanttDep, GanttRow, GanttTask};
use crate::layout::assign_rows;

use super::constants::*;
use super::types::{BarAlign, ChartOptions};

/// One drawn row.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Slot {
    /// The index into `tasks` of the task drawn on this row; `None` for a group
    /// row, which draws a band and no bar.
    pub task: Option<usize>,
    pub y_top: f64,
    pub height: f64,
}

impl Slot {
    pub fn y_bottom(&self) -> f64 {
        self.y_top + self.height
    }
}

/// Every row of a chart, top to bottom, and the sizes the rows were laid out
/// with.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct RowPlan {
    pub slots: Vec<Slot>,
    /// The slot each task is drawn on, by task index; `None` for a task a row
    /// list left out.
    pub slot_of_task: Vec<Option<usize>>,
    pub header_h: f64,
    pub row_h: f64,
    pub bar_h: f64,
    pub bar_align: BarAlign,
    /// Whether the rows are the caller's list. The scroll window of a chart
    /// without one, at the standard row and header heights, keeps the
    /// arithmetic it was pinned with.
    pub external: bool,
}

impl RowPlan {
    pub fn new(
        tasks: &[GanttTask],
        deps: &[GanttDep],
        rows: Option<&[GanttRow]>,
        options: &ChartOptions,
    ) -> Self {
        let header_h = super::build::header_height_of(options);
        let row_h = options.row_height.unwrap_or(ROW_H);
        let mut slots = Vec::new();
        let mut slot_of_task = vec![None; tasks.len()];
        match rows {
            // One row per task at the rank `assign_rows` gives it, so a blocker
            // sits above what it blocks.
            None => {
                let mut by_row: Vec<(usize, usize)> = assign_rows(tasks, deps)
                    .iter()
                    .enumerate()
                    .map(|(i, r)| (r.row, i))
                    .collect();
                by_row.sort_unstable();
                for (row, i) in by_row {
                    slot_of_task[i] = Some(slots.len());
                    slots.push(Slot {
                        task: Some(i),
                        y_top: header_h + row as f64 * row_h,
                        height: row_h,
                    });
                }
            }
            Some(rows) => {
                // First occurrence wins for a repeated task id, the rule the
                // rest of the chart uses for repeated ids.
                let mut index_of: HashMap<&str, usize> = HashMap::new();
                for (i, t) in tasks.iter().enumerate() {
                    index_of.entry(t.id.as_str()).or_insert(i);
                }
                let mut y = header_h;
                for row in rows {
                    let task = match row {
                        GanttRow::Group { .. } => None,
                        // The entry points refuse an id that names no task, or
                        // one listed twice; the native builder skips them.
                        GanttRow::Task { task_id, .. } => match index_of.get(task_id.as_str()) {
                            Some(&i) if slot_of_task[i].is_none() => Some(i),
                            _ => continue,
                        },
                    };
                    if let Some(i) = task {
                        slot_of_task[i] = Some(slots.len());
                    }
                    let height = row.height().unwrap_or(row_h);
                    slots.push(Slot {
                        task,
                        y_top: y,
                        height,
                    });
                    y += height;
                }
            }
        }
        Self {
            slots,
            slot_of_task,
            header_h,
            row_h,
            bar_h: options.bar_height.unwrap_or(BAR_H),
            bar_align: options.bar_align,
            external: rows.is_some(),
        }
    }

    /// The height every row takes, summed.
    pub fn rows_height(&self) -> f64 {
        self.slots
            .last()
            .map_or(0.0, |s| s.y_bottom() - self.header_h)
    }

    /// The top of the bar drawn on `slot`. A middle bar is centred in its row.
    /// A bottom bar keeps under it the gap a middle bar has in a row of the
    /// standard height, so in a tall row it sits low and leaves room above it.
    /// A row too short for that gap gets only the room it has, so the bar stays
    /// inside the row.
    pub fn bar_top(&self, slot: &Slot) -> f64 {
        match self.bar_align {
            BarAlign::Middle => slot.y_top + (slot.height - self.bar_h) / 2.0,
            BarAlign::Bottom => {
                let gap = ((self.row_h - self.bar_h) / 2.0)
                    .min(slot.height - self.bar_h)
                    .max(0.0);
                slot.y_bottom() - gap - self.bar_h
            }
        }
    }

    /// The rows drawn when the chart scrolls to `scroll`, as an inclusive slot
    /// range. A chart without a row list, at the standard `ROW_H` and
    /// `HEADER_H`, answers with `compute_row_window`, whose arithmetic is
    /// pinned. Any other chart, a row list or a chart that sets
    /// `row_height_px` or `header_height_px`, has rows that arithmetic would
    /// divide wrongly, so its window is found from the rows' own edges.
    pub fn window(&self, scroll: Option<super::types::ScrollViewport>) -> Option<(usize, usize)> {
        if !self.external && self.row_h == ROW_H && self.header_h == HEADER_H {
            return super::build::compute_row_window(scroll, self.slots.len());
        }
        let scroll = scroll?;
        if self.slots.is_empty() {
            return Some((0, 0));
        }
        let top = scroll.scroll_y;
        let bottom = scroll.scroll_y + scroll.client_height;
        let first_visible = self
            .slots
            .iter()
            .position(|s| s.y_bottom() > top)
            .unwrap_or(self.slots.len() - 1);
        let last_visible = self
            .slots
            .iter()
            .rposition(|s| s.y_top < bottom)
            .unwrap_or(first_visible)
            .max(first_visible);
        let buffer = ROW_BUFFER as usize;
        Some((
            first_visible.saturating_sub(buffer),
            (last_visible + buffer).min(self.slots.len() - 1),
        ))
    }
}
