use std::collections::{HashMap, HashSet};

use crate::graph::{GanttDep, GanttTask};

#[derive(Debug, PartialEq)]
pub struct RowLayout {
    pub task_id: String,
    pub row: usize,
}

/// Assign each task a vertical row from dependency topology.
/// Returns one entry per input task in **input order**; `row` is the clustering rank
/// (blocked tasks cluster directly under their blockers, not global topo priority).
pub fn assign_rows(tasks: &[GanttTask], deps: &[GanttDep]) -> Vec<RowLayout> {
    if tasks.is_empty() {
        return Vec::new();
    }

    let n = tasks.len();
    let mut id_to_idx = HashMap::with_capacity(n);
    for (i, t) in tasks.iter().enumerate() {
        id_to_idx.entry(t.id.as_str()).or_insert(i);
    }

    let mut adj: Vec<Vec<usize>> = vec![Vec::new(); n];
    let mut seen_edges = HashSet::new();

    for dep in deps {
        let blocker_idx = id_to_idx.get(dep.blocker_task_id.as_str()).copied();
        let blocked_idx = id_to_idx.get(dep.blocked_task_id.as_str()).copied();

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
    let mut scc_rev_adj: Vec<Vec<usize>> = vec![Vec::new(); scc_count];

    for u in 0..n {
        for &v in &adj[u] {
            let cu = component[u];
            let cv = component[v];
            if cu != cv && scc_edges.insert((cu, cv)) {
                scc_adj[cu].push(cv);
                scc_rev_adj[cv].push(cu);
                scc_indegree[cv] += 1;
            }
        }
    }

    for adj_list in &mut [scc_adj.as_mut_slice(), scc_rev_adj.as_mut_slice()] {
        for neighbors in adj_list.iter_mut() {
            neighbors.sort_unstable_by_key(|&scc| scc_min_index[scc]);
        }
    }

    let mut emitted_scc = HashSet::new();
    let mut cluster_order = Vec::with_capacity(n);

    fn emit_scc(
        scc: usize,
        scc_members: &[Vec<usize>],
        emitted_scc: &mut HashSet<usize>,
        cluster_order: &mut Vec<usize>,
    ) {
        if !emitted_scc.insert(scc) {
            return;
        }
        for &idx in &scc_members[scc] {
            cluster_order.push(idx);
        }
    }

    fn drain_dependent_sccs(
        start_scc: usize,
        scc_adj: &[Vec<usize>],
        scc_rev_adj: &[Vec<usize>],
        scc_members: &[Vec<usize>],
        emitted_scc: &mut HashSet<usize>,
        cluster_order: &mut Vec<usize>,
    ) {
        let mut stack = vec![(start_scc, 0usize)];

        while let Some((scc, mut i)) = stack.pop() {
            while i < scc_adj[scc].len() {
                let next = scc_adj[scc][i];
                i += 1;
                if emitted_scc.contains(&next) {
                    continue;
                }
                let all_blockers_emitted = scc_rev_adj[next]
                    .iter()
                    .all(|pred| emitted_scc.contains(pred));
                if !all_blockers_emitted {
                    continue;
                }
                emit_scc(next, scc_members, emitted_scc, cluster_order);
                stack.push((scc, i));
                stack.push((next, 0));
                break;
            }
        }
    }

    let mut root_sccs: Vec<usize> = (0..scc_count)
        .filter(|&s| scc_indegree[s] == 0)
        .collect();
    root_sccs.sort_unstable_by_key(|&scc| scc_min_index[scc]);

    for scc in root_sccs {
        if emitted_scc.contains(&scc) {
            continue;
        }
        emit_scc(scc, &scc_members, &mut emitted_scc, &mut cluster_order);
        drain_dependent_sccs(
            scc,
            &scc_adj,
            &scc_rev_adj,
            &scc_members,
            &mut emitted_scc,
            &mut cluster_order,
        );
    }

    for scc in 0..scc_count {
        if !emitted_scc.contains(&scc) {
            emit_scc(scc, &scc_members, &mut emitted_scc, &mut cluster_order);
            drain_dependent_sccs(
                scc,
                &scc_adj,
                &scc_rev_adj,
                &scc_members,
                &mut emitted_scc,
                &mut cluster_order,
            );
        }
    }

    let mut row_of = vec![0usize; n];
    for (row, &idx) in cluster_order.iter().enumerate() {
        row_of[idx] = row;
    }

    (0..n)
        .map(|idx| RowLayout {
            task_id: tasks[idx].id.clone(),
            row: row_of[idx],
        })
        .collect()
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
    fn disconnected_components_cluster_blocked_under_blocker() {
        let tasks = vec![task("x"), task("y"), task("z")];
        let deps = vec![dep("x", "z")];
        let rows = assign_rows(&tasks, &deps);
        assert_input_order_preserved(&rows, &tasks);
        // x emits then z clusters directly under x; y stays after the cluster.
        assert_eq!(row_map(&rows), vec![("x", 0), ("y", 2), ("z", 1)]);
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
    fn blocker_clustering_pulls_blocked_adjacent_not_global_kahn() {
        // Acceptance example: input [t1,t3,t2] with t2 blocked by t1 must cluster as
        // [t1,t2,t3], not global priority Kahn [t1,t3,t2].
        let tasks = vec![task("t1"), task("t3"), task("t2")];
        let deps = vec![dep("t1", "t2")];
        let rows = assign_rows(&tasks, &deps);
        assert_input_order_preserved(&rows, &tasks);
        assert_eq!(row_map(&rows), vec![("t1", 0), ("t3", 2), ("t2", 1)]);
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

    #[test]
    fn long_chain_does_not_stack_overflow_and_preserves_topo_order() {
        const N: usize = 10_000;
        let tasks: Vec<GanttTask> = (0..N).map(|i| task(&format!("t{i}"))).collect();
        let deps: Vec<GanttDep> = (0..N - 1)
            .map(|i| dep(&format!("t{i}"), &format!("t{}", i + 1)))
            .collect();

        let rows = assign_rows(&tasks, &deps);
        assert_input_order_preserved(&rows, &tasks);
        for i in 0..N - 1 {
            assert!(
                row_of(&rows, &format!("t{i}")) < row_of(&rows, &format!("t{}", i + 1)),
                "chain must stay topologically ordered at link {i}"
            );
        }
    }

    #[test]
    fn dense_5000_completes_within_linear_id_lookup_budget() {
        use crate::bench_fixtures::{generate_fixture, DepDensity, TaskCount};

        let fixture = generate_fixture(TaskCount::N5000, DepDensity::Dense);
        let started = std::time::Instant::now();
        let rows = assign_rows(&fixture.tasks, &fixture.deps);
        let elapsed = started.elapsed();

        assert_input_order_preserved(&rows, &fixture.tasks);
        assert!(
            elapsed.as_millis() < 2_000,
            "5000_dense assign_rows took {:?}; linear id scans would be far slower",
            elapsed
        );
        assert!(row_of(&rows, "task-00000") < row_of(&rows, "task-04999"));
        assert!(row_of(&rows, "task-01234") < row_of(&rows, "task-04567"));
    }

    #[test]
    fn duplicate_task_ids_resolve_to_first_occurrence() {
        let tasks = vec![task("dup"), task("other"), task("dup")];
        let deps = vec![dep("dup", "other")];
        let rows = assign_rows(&tasks, &deps);
        assert_input_order_preserved(&rows, &tasks);
        assert!(row_of(&rows, "dup") < row_of(&rows, "other"));
    }
}
