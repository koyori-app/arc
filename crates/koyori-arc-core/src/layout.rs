use std::collections::HashSet;

use crate::graph::{GanttDep, GanttTask};

#[derive(Debug, PartialEq)]
pub struct RowLayout {
    pub task_id: String,
    pub row: usize,
}

/// Assign each task a vertical row from dependency topology.
/// Returns one entry per input task in **input order**; `row` is the topological rank.
pub fn assign_rows(tasks: &[GanttTask], deps: &[GanttDep]) -> Vec<RowLayout> {
    if tasks.is_empty() {
        return Vec::new();
    }

    let n = tasks.len();
    let id_to_idx: Vec<(&str, usize)> = tasks
        .iter()
        .enumerate()
        .map(|(i, t)| (t.id.as_str(), i))
        .collect();

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
    }

    let component = tarjan_scc(n, &adj);
    let scc_count = component.iter().copied().max().map_or(0, |m| m + 1);

    let mut scc_members: Vec<Vec<usize>> = vec![Vec::new(); scc_count];
    for i in 0..n {
        scc_members[component[i]].push(i);
    }
    for members in &mut scc_members {
        members.sort_unstable();
    }

    let mut scc_min_index = vec![usize::MAX; scc_count];
    for (scc_id, members) in scc_members.iter().enumerate() {
        scc_min_index[scc_id] = members[0];
    }

    let mut scc_adj: Vec<Vec<usize>> = vec![Vec::new(); scc_count];
    let mut scc_indegree = vec![0usize; scc_count];
    let mut scc_edges = HashSet::new();

    for u in 0..n {
        for &v in &adj[u] {
            let cu = component[u];
            let cv = component[v];
            if cu != cv && scc_edges.insert((cu, cv)) {
                scc_adj[cu].push(cv);
                scc_indegree[cv] += 1;
            }
        }
    }

    let mut ready: Vec<usize> = (0..scc_count)
        .filter(|&s| scc_indegree[s] == 0)
        .collect();
    let mut scc_order = Vec::with_capacity(scc_count);
    let mut scc_indegree = scc_indegree;

    while let Some(scc) = pop_min_scc(&mut ready, &scc_min_index) {
        scc_order.push(scc);
        for &next in &scc_adj[scc] {
            scc_indegree[next] -= 1;
            if scc_indegree[next] == 0 {
                ready.push(next);
            }
        }
    }

    for scc in 0..scc_count {
        if !scc_order.contains(&scc) {
            scc_order.push(scc);
        }
    }

    let mut row_of = vec![0usize; n];
    let mut next_row = 0usize;
    for &scc in &scc_order {
        for &idx in &scc_members[scc] {
            row_of[idx] = next_row;
            next_row += 1;
        }
    }

    (0..n)
        .map(|idx| RowLayout {
            task_id: tasks[idx].id.clone(),
            row: row_of[idx],
        })
        .collect()
}

fn pop_min_scc(ready: &mut Vec<usize>, scc_min_index: &[usize]) -> Option<usize> {
    if ready.is_empty() {
        return None;
    }
    let min_pos = ready
        .iter()
        .enumerate()
        .min_by_key(|(_, scc)| scc_min_index[**scc])
        .map(|(pos, _)| pos)?;
    Some(ready.remove(min_pos))
}

fn tarjan_scc(n: usize, adj: &[Vec<usize>]) -> Vec<usize> {
    let mut index = 0usize;
    let mut indices = vec![None; n];
    let mut lowlink = vec![0usize; n];
    let mut on_stack = vec![false; n];
    let mut stack = Vec::new();
    let mut component = vec![usize::MAX; n];
    let mut comp_id = 0usize;

    for start in 0..n {
        if indices[start].is_some() {
            continue;
        }
        let mut stack_frame: Vec<(usize, usize)> = Vec::new();
        stack_frame.push((start, 0));
        indices[start] = Some(index);
        lowlink[start] = index;
        index += 1;
        stack.push(start);
        on_stack[start] = true;

        while let Some((v, edge_i)) = stack_frame.pop() {
            if edge_i < adj[v].len() {
                let w = adj[v][edge_i];
                stack_frame.push((v, edge_i + 1));
                if indices[w].is_none() {
                    indices[w] = Some(index);
                    lowlink[w] = index;
                    index += 1;
                    stack.push(w);
                    on_stack[w] = true;
                    stack_frame.push((w, 0));
                } else if on_stack[w] {
                    lowlink[v] = lowlink[v].min(lowlink[w]);
                }
            } else {
                if lowlink[v] == indices[v].unwrap() {
                    loop {
                        let w = stack.pop().unwrap();
                        on_stack[w] = false;
                        component[w] = comp_id;
                        if w == v {
                            break;
                        }
                    }
                    comp_id += 1;
                }
                if let Some((parent, _)) = stack_frame.last() {
                    lowlink[*parent] = lowlink[*parent].min(lowlink[v]);
                }
            }
        }
    }

    component
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

    fn assert_input_order_preserved(rows: &[RowLayout], tasks: &[GanttTask]) {
        let ids: Vec<&str> = rows.iter().map(|r| r.task_id.as_str()).collect();
        let expected: Vec<&str> = tasks.iter().map(|t| t.id.as_str()).collect();
        assert_eq!(ids, expected, "assign_rows must preserve input task order");
    }

    #[test]
    fn empty_input_gives_empty_rows() {
        assert!(assign_rows(&[], &[]).is_empty());
    }

    #[test]
    fn no_deps_preserves_input_order() {
        let tasks = vec![task("a"), task("b"), task("c")];
        let rows = assign_rows(&tasks, &[]);
        assert_input_order_preserved(&rows, &tasks);
        assert_eq!(row_map(&rows), vec![("a", 0), ("b", 1), ("c", 2)]);
    }

    #[test]
    fn blocker_is_above_blocked() {
        let tasks = vec![task("blocked"), task("blocker")];
        let deps = vec![dep("blocker", "blocked")];
        let rows = assign_rows(&tasks, &deps);
        assert_input_order_preserved(&rows, &tasks);
        assert!(row_of(&rows, "blocker") < row_of(&rows, "blocked"));
    }

    #[test]
    fn chain_orders_topologically() {
        let tasks = vec![task("c"), task("a"), task("b")];
        let deps = vec![dep("a", "b"), dep("b", "c")];
        let rows = assign_rows(&tasks, &deps);
        assert_input_order_preserved(&rows, &tasks);
        assert!(row_of(&rows, "a") < row_of(&rows, "b"));
        assert!(row_of(&rows, "b") < row_of(&rows, "c"));
    }

    #[test]
    fn diamond_respects_dependencies_with_input_tie_break() {
        let tasks = vec![task("a"), task("b"), task("c"), task("d")];
        let deps = vec![dep("a", "b"), dep("a", "c"), dep("b", "d"), dep("c", "d")];
        let rows = assign_rows(&tasks, &deps);
        assert_input_order_preserved(&rows, &tasks);
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
        assert_input_order_preserved(&rows, &tasks);
        assert_eq!(row_map(&rows), vec![("x", 0), ("y", 1), ("z", 2)]);
    }

    #[test]
    fn cycle_is_handled_deterministically_without_panic() {
        let tasks = vec![task("a"), task("b"), task("c")];
        let deps = vec![dep("a", "b"), dep("b", "c"), dep("c", "a")];
        let once = assign_rows(&tasks, &deps);
        let twice = assign_rows(&tasks, &deps);
        assert_eq!(once, twice);
        assert_input_order_preserved(&once, &tasks);
        assert_eq!(row_map(&once), vec![("a", 0), ("b", 1), ("c", 2)]);
    }

    #[test]
    fn cycle_downstream_keeps_blocker_above_blocked() {
        // D listed first but depends on B inside cycle {A,B,C}
        let tasks = vec![task("d"), task("a"), task("b"), task("c")];
        let deps = vec![
            dep("a", "b"),
            dep("b", "c"),
            dep("c", "a"),
            dep("b", "d"),
        ];
        let rows = assign_rows(&tasks, &deps);
        assert_input_order_preserved(&rows, &tasks);
        assert!(
            row_of(&rows, "b") < row_of(&rows, "d"),
            "downstream of cycle must stay below its blocker"
        );
        assert!(row_of(&rows, "a") < row_of(&rows, "d"));
        assert!(row_of(&rows, "c") < row_of(&rows, "d"));
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
        assert_input_order_preserved(&rows, &tasks);
        assert!(row_of(&rows, "a") < row_of(&rows, "b"));
    }

    #[test]
    fn duplicate_edges_do_not_break_indegree() {
        let tasks = vec![task("a"), task("b")];
        let deps = vec![dep("a", "b"), dep("a", "b")];
        let rows = assign_rows(&tasks, &deps);
        assert_input_order_preserved(&rows, &tasks);
        assert!(row_of(&rows, "a") < row_of(&rows, "b"));
    }

    #[test]
    fn deterministic_across_repeated_calls() {
        let tasks = vec![task("t1"), task("t2"), task("t3"), task("t4")];
        let deps = vec![dep("t1", "t3"), dep("t2", "t4"), dep("t1", "t2")];
        let first = assign_rows(&tasks, &deps);
        let second = assign_rows(&tasks, &deps);
        assert_eq!(first, second);
        assert_input_order_preserved(&first, &tasks);
        assert!(row_of(&first, "t1") < row_of(&first, "t2"));
        assert!(row_of(&first, "t1") < row_of(&first, "t3"));
        assert!(row_of(&first, "t2") < row_of(&first, "t4"));
    }
}
