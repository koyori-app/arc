/** Mirrors the task project's `tasks` entity (Gantt-relevant fields only). */
export interface GanttTask {
  /** `tasks.id` (UUID string) */
  id: string;
  /** `tasks.title` */
  title: string;
  /** `tasks.progress_pct` (0–100) */
  progress_pct: number;
  /**
   * Resolved from sprint.start_date or another source. ISO 8601 date
   * (`2026-06-01`) or date-time (`2026-06-01T09:00:00Z`). A date-only string is
   * never converted. A date-time with an offset is converted to the calendar
   * day of `tzOffsetMinutes`, or kept as written when that is not given.
   */
  start: string;
  /**
   * `tasks.hard_deadline` or `tasks.soft_deadline`. Same formats as `start`, so
   * a timestamptz can be passed as it is. An `end` before `start` is drawn as if
   * it were equal to `start`.
   */
  end?: string;
}

/**
 * One row of a chart whose rows the caller lays out (`rows` on `GanttChart`).
 * A task row draws that task's bar; a group row draws a band across the days
 * and no bar. `height` is in pixels; left out, the row is `rowHeight` tall.
 *
 * A task row may also carry marks; a row without them draws the bar as a chart
 * without `rows` does.
 * - `hard_deadline` (YYYY-MM-DD): a diamond in the middle of that day.
 * - `state`: fills and outlines the bar (`normal`, `done`, `warn`).
 * - `note`: a line of text past the bar and the diamond (at most 200 characters).
 * - `shape`: `point` draws a dot on the span's last day instead of a bar, with a
 *   line to the diamond when the deadline is later.
 */
export type GanttRow =
  | {
      kind: 'task';
      task_id: string;
      height?: number;
      hard_deadline?: string | null;
      state?: BarState | null;
      note?: string | null;
      shape?: BarShape;
    }
  | { kind: 'group'; height?: number };

/** How a task row's bar is filled and outlined. */
export type BarState = 'normal' | 'done' | 'warn';

/** A task row drawn as a bar (the default) or as a dot on its last day. */
export type BarShape = 'bar' | 'point';

/** Mirrors `task_relations` entity. */
export interface GanttDep {
  /** `task_relations.blocker_task_id` */
  blocker_task_id: string;
  /** `task_relations.blocked_task_id` */
  blocked_task_id: string;
}
