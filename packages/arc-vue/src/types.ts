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
  /**
   * The ids of the lanes this task belongs to, used only when the chart is given
   * `lanes`. A task in two lanes is drawn once in each, and clicking either bar
   * emits this same task. A task in none of the lanes is not drawn. Ignored when
   * the chart has no `lanes`.
   */
  lane_ids?: string[];
}

/**
 * One lane of a chart given `lanes`: the name drawn in the left column, and the id
 * tasks name in `lane_ids`. The order of the lanes given is the order drawn, top first.
 */
export interface GanttLane {
  id: string;
  label: string;
}

/** Mirrors `task_relations` entity. */
export interface GanttDep {
  /** `task_relations.blocker_task_id` */
  blocker_task_id: string;
  /** `task_relations.blocked_task_id` */
  blocked_task_id: string;
}
