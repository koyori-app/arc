use std::collections::HashSet;

use crate::graph::{GanttDep, GanttTask};

#[derive(Debug, PartialEq)]
pub struct RowLayout {
    pub task_id: String,
    pub row: usize,
}

/// Assign each task to a vertical row using topological order over dependencies.
/// Blockers are placed above (lower row index) their blocked tasks.
pub fn assign_rows(tasks: &[GanttTask], deps: &[GanttDep]) -> Vec<RowLayout> {
    if tasks.is_empty() {
        return Vec::new();
    }

    let n = tasks.len();
    let id_to_idx: Vec<(&str, usize)> = tasks.iter().enumerate().map(|(i, t)| (t.id.as_str(), i)).collect();

    let mut indegree = vec![0usize; n];
    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut seen_edges = HashSet::new();

    for dep in deps {
        let blocker_idx = id_to_idx
            .iter()
            .find(|(id, _)| *id == dep.blocker_task_id.as_str())
            .map(|(_, i)| *i);
        let blocked_idx = id_to_idx
            .iter()
            .find(|(id, _)| *id == dep.blocked_task_id.as_str())
            .map(|(_, i)| *i);

        let (blocker_idx, blocked_idx) = match (blocker_idx, blocked_idx) {
            (Some(b), Some(d)) if b != d => (b, d),
            _ => continue,
        };

        if !seen_edges.insert((blocker_idx, blocked_idx)) {
            continue;
        }

        adj[blocker_idx].push(blocked_idx);
        indegree[blocked_idx] += 1;
    }

    let mut ready: Vec<usize> = (0..n).filter(|&i| indegree[i] == 0).collect();
    let mut order = Vec::with_capacity(n);
    let mut indegree = indegree;

    while let Some(idx) = pop_min_input_index(&mut ready) {
        order.push(idx);
        for &next in &adj[idx] {
            indegree[next] -= 1;
            if indegree[next] == 0 {
                ready.push(next);
            }
        }
    }

    let mut placed = vec![false; n];
    for &idx in &order {
        placed[idx] = true;
    }
    for i in 0..n {
        if !placed[i] {
            order.push(i);
        }
    }

    order
        .into_iter()
        .enumerate()
        .map(|(row, idx)| RowLayout {
            task_id: tasks[idx].id.clone(),
            row,
        })
        .collect()
}

fn pop_min_input_index(ready: &mut Vec<usize>) -> Option<usize> {
    if ready.is_empty() {
        return None;
    }
    let min_pos = ready
        .iter()
        .enumerate()
        .min_by_key(|(_, idx)| **idx)
        .map(|(pos, _)| pos)?;
    Some(ready.remove(min_pos))
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

    fn dep(blocker: &str, blocked: &str) -> GanttDep {
        GanttDep {
            blocker_task_id: blocker.to_string(),
            blocked_task_id: blocked.to_string(),
        }
    }

    fn row_map(rows: &[RowLayout]) -> Vec<(&str, usize)> {
        rows.iter().map(|r| (r.task_id.as_str(), r.row)).collect()
    }

    fn row_of<'a>(rows: &'a [RowLayout], id: &str) -> usize {
        rows.iter().find(|r| r.task_id == id).map(|r| r.row).unwrap()
    }

    #[test]
    fn empty_input_gives_empty_rows() {
        assert!(assign_rows(&[], &[]).is_empty());
    }

    #[test]
    fn no_deps_preserves_input_order() {
        let tasks = vec![task("a"), task("b"), task("c")];
        let rows = assign_rows(&tasks, &[]);
        assert_eq!(row_map(&rows), vec![("a", 0), ("b", 1), ("c", 2)]);
    }

    #[test]
    fn blocker_is_above_blocked() {
        let tasks = vec![task("blocked"), task("blocker")];
        let deps = vec![dep("blocker", "blocked")];
        let rows = assign_rows(&tasks, &deps);
        assert!(row_of(&rows, "blocker") < row_of(&rows, "blocked"));
    }

    #[test]
    fn chain_orders_topologically() {
        let tasks = vec![task("c"), task("a"), task("b")];
        let deps = vec![dep("a", "b"), dep("b", "c")];
        let rows = assign_rows(&tasks, &deps);
        assert!(row_of(&rows, "a") < row_of(&rows, "b"));
        assert!(row_of(&rows, "b") < row_of(&rows, "c"));
    }

    #[test]
    fn diamond_respects_dependencies_with_input_tie_break() {
        let tasks = vec![task("a"), task("b"), task("c"), task("d")];
        let deps = vec![dep("a", "b"), dep("a", "c"), dep("b", "d"), dep("c", "d")];
        let rows = assign_rows(&tasks, &deps);
        assert_eq!(row_of(&rows, "a"), 0);
        assert!(row_of(&rows, "b") < row_of(&rows, "d"));
        assert!(row_of(&rows, "c") < row_of(&rows, "d"));
        assert!(row_of(&rows, "b") < row_of(&rows, "c"));
    }

    #[test]
    fn disconnected_components_use_input_order_among_ready() {
        let tasks = vec![task("x"), task("y"), task("z")];
        let deps = vec![dep("x", "z")];
        let rows = assign_rows(&tasks, &deps);
        assert_eq!(row_map(&rows), vec![("x", 0), ("y", 1), ("z", 2)]);
    }

    #[test]
    fn cycle_is_handled_deterministically_without_panic() {
        let tasks = vec![task("a"), task("b"), task("c")];
        let deps = vec![dep("a", "b"), dep("b", "c"), dep("c", "a")];
        let once = assign_rows(&tasks, &deps);
        let twice = assign_rows(&tasks, &deps);
        assert_eq!(once, twice);
        assert_eq!(row_map(&once), vec![("a", 0), ("b", 1), ("c", 2)]);
    }

    #[test]
    fn unknown_dependency_ids_are_ignored() {
        let tasks = vec![task("a"), task("b")];
        let deps = vec![
            dep("missing", "b"),
            dep("a", "ghost"),
            dep("a", "b"),
        ];
        let rows = assign_rows(&tasks, &deps);
        assert!(row_of(&rows, "a") < row_of(&rows, "b"));
    }

    #[test]
    fn duplicate_edges_do_not_break_indegree() {
        let tasks = vec![task("a"), task("b")];
        let deps = vec![dep("a", "b"), dep("a", "b")];
        let rows = assign_rows(&tasks, &deps);
        assert!(row_of(&rows, "a") < row_of(&rows, "b"));
    }

    #[test]
    fn deterministic_across_repeated_calls() {
        let tasks = vec![task("t1"), task("t2"), task("t3"), task("t4")];
        let deps = vec![dep("t1", "t3"), dep("t2", "t4"), dep("t1", "t2")];
        let first = assign_rows(&tasks, &deps);
        let second = assign_rows(&tasks, &deps);
        assert_eq!(first, second);
    }
}
