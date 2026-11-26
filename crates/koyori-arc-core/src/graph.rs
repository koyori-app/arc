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
///
/// An edge with none of the optional keys is drawn as dependency arrows were
/// before them. One with any of them is a *styled* edge
/// (docs/design/dependency-states.md): drawn under the bars from the end of the
/// blocker's bar down to the blocked row and right into the blocked bar, in the
/// colour of its state, with its note right of the blocked bar, and with an end
/// the chart does not draw replaced by the stand-in the caller gives.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct GanttDep {
    /// `task_relations.blocker_task_id`
    pub blocker_task_id: String,
    /// `task_relations.blocked_task_id`
    pub blocked_task_id: String,
    /// The edge's state: its colour, and a dashed line for `delayed`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub state: Option<DepState>,
    /// Text drawn right of the blocked bar, such as `REL-17 待ち`.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
    /// Where the edge starts when the blocker has no bar on the chart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocker_at: Option<DepEnd>,
    /// Where the edge ends when the blocked task has no bar on the chart.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub blocked_at: Option<DepEnd>,
}

impl GanttDep {
    /// An edge from `blocker` to `blocked` with none of the optional keys.
    pub fn new(blocker: impl Into<String>, blocked: impl Into<String>) -> Self {
        Self {
            blocker_task_id: blocker.into(),
            blocked_task_id: blocked.into(),
            ..Default::default()
        }
    }

    /// Whether the edge gives any of the optional keys, and so is drawn in
    /// the styled form.
    pub fn is_styled(&self) -> bool {
        self.state.is_some()
            || self.note.is_some()
            || self.blocker_at.is_some()
            || self.blocked_at.is_some()
    }
}

/// The state an edge is drawn in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepState {
    /// The blocker is not done and the blocked task has not reached its start.
    Normal,
    /// The blocker is done.
    Done,
    /// The blocker is not done though the blocked task's start has come: a
    /// dashed line in the warning colour.
    Delayed,
}

/// The stand-in for an end of an edge the chart does not draw.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum DepEnd {
    /// A point on one of the caller's rows (`row` counts the rows of
    /// `rows_json` from 0), on `day`: the header row of a collapsed group the
    /// task is in. `label` is drawn beside the point, such as the task's key.
    Row {
        row: usize,
        #[serde(deserialize_with = "crate::date_input::deserialize_date")]
        day: NaiveDate,
        #[serde(default)]
        label: Option<String>,
    },
    /// An edge of the chart: the task lies outside the days or the filter the
    /// chart shows. The edge is drawn along the drawn end's row to this edge,
    /// and `label` is drawn at it, such as the task's key and start.
    Edge {
        edge: DepEdge,
        #[serde(default)]
        label: Option<String>,
    },
}

impl DepEnd {
    pub fn label(&self) -> Option<&str> {
        match self {
            DepEnd::Row { label, .. } | DepEnd::Edge { label, .. } => label.as_deref(),
        }
    }
}

/// Which edge of the chart a stand-in end lies at.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum DepEdge {
    /// The left edge: before the first day the chart shows.
    Start,
    /// The right edge: after the last day the chart shows.
    End,
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
    /// The colour of the bar, a CSS colour of the forms `palette` takes. Given,
    /// it fills the bar in place of the state's fill and the bar always gets an
    /// outline (docs/design/row-bar-color.md). A colour those forms refuse is
    /// refused by the Wasm entry points and drawn as no colour by the native
    /// ones.
    #[serde(default)]
    pub color: Option<String>,
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
