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
 */
export type GanttRow =
  | { kind: 'task'; task_id: string; height?: number }
  | { kind: 'group'; height?: number };

/** Mirrors `task_relations` entity. */
export interface GanttDep {
  /** `task_relations.blocker_task_id` */
  blocker_task_id: string;
  /** `task_relations.blocked_task_id` */
  blocked_task_id: string;
}
