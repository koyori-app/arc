use crate::graph::GanttTask;

pub struct RowLayout {
    pub task_id: String,
    pub row: usize,
}

/// Assign each task to a vertical row (simple sequential layout for now).
/// Future: topological sort + row packing to minimise chart height.
pub fn assign_rows(tasks: &[GanttTask]) -> Vec<RowLayout> {
    tasks
        .iter()
        .enumerate()
        .map(|(i, t)| RowLayout { task_id: t.id.clone(), row: i })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::NaiveDate;

    fn task(id: &str) -> GanttTask {
        GanttTask {
            id: id.to_string(),
            title: format!("task-{id}"),
            progress_pct: 0,
            start: NaiveDate::from_ymd_opt(2026, 1, 1).unwrap(),
            end: None,
        }
    }

    #[test]
    fn empty_input_gives_empty_rows() {
        assert!(assign_rows(&[]).is_empty());
    }

    #[test]
    fn rows_are_sequential() {
        let tasks = vec![task("a"), task("b"), task("c")];
        let rows = assign_rows(&tasks);
        assert_eq!(rows.len(), 3);
        assert_eq!(rows[0].task_id, "a");
        assert_eq!(rows[0].row, 0);
        assert_eq!(rows[2].task_id, "c");
        assert_eq!(rows[2].row, 2);
    }
}
