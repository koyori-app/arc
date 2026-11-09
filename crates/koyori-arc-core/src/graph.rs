use chrono::{FixedOffset, NaiveDate};
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
    /// Resolved from sprint.start_date or another source; tasks have no own start_date.
    /// JSON accepts `YYYY-MM-DD` or an ISO 8601 date-time (see `date_input`).
    #[serde(deserialize_with = "crate::date_input::deserialize_date")]
    pub start: NaiveDate,
    /// `tasks.hard_deadline` or `tasks.soft_deadline` (caller picks).
    /// JSON accepts `YYYY-MM-DD` or an ISO 8601 date-time (see `date_input`).
    #[serde(
        default,
        deserialize_with = "crate::date_input::deserialize_optional_date"
    )]
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

/// One row of a chart whose rows the caller lays out
/// (docs/design/external-rows.md). A task row draws that task's bar; a group
/// row draws a band across the days and no bar, for a caller that heads a group
/// of tasks in a list of its own. `height` is in pixels; left out, the row is
/// `row_height_px` tall.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum GanttRow {
    Task(TaskRow),
    Group {
        #[serde(default)]
        height: Option<f64>,
    },
}

/// A task row of a caller's row list, and the marks drawn on its bar
/// (docs/design/bar-marks.md). Every mark is optional; a row with none draws
/// the bar as a chart without row lists draws it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct TaskRow {
    pub task_id: String,
    #[serde(default)]
    pub height: Option<f64>,
    /// A deadline that may not move, drawn as a diamond in the middle of its
    /// day. A date-only string, the day as written.
    #[serde(
        default,
        deserialize_with = "crate::date_input::deserialize_optional_date"
    )]
    pub hard_deadline: Option<NaiveDate>,
    /// How the bar is drawn: given at all, the bar gets an outline, and its
    /// fill and outline follow the state.
    #[serde(default)]
    pub state: Option<BarState>,
    /// Text drawn right of the bar and its diamond, such as why it is in warning.
    #[serde(default)]
    pub note: Option<String>,
    /// `Point` draws a dot at the task's end day in place of a bar, for a task
    /// with only a deadline.
    #[serde(default)]
    pub shape: BarShape,
}

impl TaskRow {
    pub fn new(task_id: impl Into<String>) -> Self {
        Self {
            task_id: task_id.into(),
            ..Default::default()
        }
    }
}

/// The state a task row's bar is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarState {
    Normal,
    Done,
    Warn,
}

/// What a task row draws for its task.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarShape {
    #[default]
    Bar,
    Point,
}

impl GanttRow {
    pub fn height(&self) -> Option<f64> {
        match self {
            GanttRow::Task(TaskRow { height, .. }) | GanttRow::Group { height } => *height,
        }
    }
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
    /// An `end` before `start` is cut to `start`: a deadline that precedes the
    /// start must not push the bar or the progress point left of where it begins.
    pub fn end_days(&self, epoch: NaiveDate) -> f64 {
        let start = self.start_days(epoch);
        self.end
            .map(|d| ((d - epoch).num_days() as f64).max(start))
            .unwrap_or(start + 1.0)
    }

    /// Normalises `progress_pct` (0–100) to 0.0–1.0 for rendering.
    pub fn progress(&self) -> f64 {
        (self.progress_pct.clamp(0, 100) as f64) / 100.0
    }
}

/// The task JSON as it arrives, with the dates still strings. The Wasm entry
/// points read tasks through this so a time zone can decide which calendar day
/// a date-time string means; `GanttTask`'s own `Deserialize` has no way to be
/// given one.
#[derive(Deserialize)]
struct RawTask {
    id: String,
    title: String,
    progress_pct: i16,
    start: String,
    #[serde(default)]
    end: Option<String>,
}

/// Reads a tasks JSON array. `offset` is the caller's time zone, or `None` for
/// "use the day as written" (see `date_input`). Errors are `parse error: ...`
/// messages; a bad date names the task index and the field.
pub fn parse_tasks_json(json: &str, offset: Option<FixedOffset>) -> Result<Vec<GanttTask>, String> {
    let raw: Vec<RawTask> = serde_json::from_str(json).map_err(|e| format!("parse error: {e}"))?;
    raw.into_iter()
        .enumerate()
        .map(|(i, t)| {
            let start = crate::date_input::resolve_date(&t.start, offset)
                .map_err(|e| format!("parse error: tasks[{i}].start: {e}"))?;
            let end = t
                .end
                .as_deref()
                .map(|s| crate::date_input::resolve_date(s, offset))
                .transpose()
                .map_err(|e| format!("parse error: tasks[{i}].end: {e}"))?;
            Ok(GanttTask {
                id: t.id,
                title: t.title,
                progress_pct: t.progress_pct,
                start,
                end,
            })
        })
        .collect()
}
