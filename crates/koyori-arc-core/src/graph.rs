use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

/// Mirrors the task project's `tasks` entity (Gantt-relevant fields only).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttTask {
    /// UUID string — matches `tasks.id`
    pub id: String,
    /// `tasks.title`
    pub title: String,
    /// `tasks.progress_pct` (0–100)
    pub progress_pct: i16,
    /// Resolved from sprint.start_date or another source; tasks have no own start_date
    pub start: NaiveDate,
    /// `tasks.hard_deadline` or `tasks.soft_deadline` (caller picks)
    pub end: Option<NaiveDate>,
}

/// Mirrors `task_relations` entity.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GanttDep {
    /// `task_relations.blocker_task_id`
    pub blocker_task_id: String,
    /// `task_relations.blocked_task_id`
    pub blocked_task_id: String,
}

pub struct GanttGraph {
    pub tasks: Vec<GanttTask>,
    pub deps: Vec<GanttDep>,
}

impl GanttTask {
    /// Days from the given epoch date (used for pixel positioning).
    pub fn start_days(&self, epoch: NaiveDate) -> f64 {
        (self.start - epoch).num_days() as f64
    }

    /// Days from epoch to end. Falls back to start + 1 day when `end` is None.
    pub fn end_days(&self, epoch: NaiveDate) -> f64 {
        self.end
            .map(|d| (d - epoch).num_days() as f64)
            .unwrap_or_else(|| self.start_days(epoch) + 1.0)
    }

    /// Normalises `progress_pct` (0–100) to 0.0–1.0 for rendering.
    pub fn progress(&self) -> f64 {
        (self.progress_pct.clamp(0, 100) as f64) / 100.0
    }
}
