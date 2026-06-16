/** Mirrors the task project's `tasks` entity (Gantt-relevant fields only). */
export interface GanttTask {
  /** `tasks.id` (UUID string) */
  id: string;
  /** `tasks.title` */
  title: string;
  /** `tasks.progress_pct` (0–100) */
  progress_pct: number;
  /** Resolved from sprint.start_date or another source; ISO 8601 date string */
  start: string;
  /** `tasks.hard_deadline` or `tasks.soft_deadline`; ISO 8601 date string */
  end?: string;
}

/** Mirrors `task_relations` entity. */
export interface GanttDep {
  /** `task_relations.blocker_task_id` */
  blocker_task_id: string;
  /** `task_relations.blocked_task_id` */
  blocked_task_id: string;
}
