//! Synthetic Gantt fixtures for render-pipeline benchmarks.
//!
//! Scales: 100 / 500 / 2000 / 5000 tasks × sparse / dense dependency graphs.

use chrono::{Duration, NaiveDate};
use serde::Serialize;

use crate::graph::{GanttDep, GanttTask};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TaskCount {
    N100 = 100,
    N500 = 500,
    N2000 = 2000,
    N5000 = 5000,
}

impl TaskCount {
    pub const ALL: [TaskCount; 4] =
        [TaskCount::N100, TaskCount::N500, TaskCount::N2000, TaskCount::N5000];

    pub fn get(self) -> usize {
        self as usize
    }

    pub fn label(self) -> &'static str {
        match self {
            TaskCount::N100 => "100",
            TaskCount::N500 => "500",
            TaskCount::N2000 => "2000",
            TaskCount::N5000 => "5000",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DepDensity {
    Sparse,
    Dense,
}

impl DepDensity {
    pub const ALL: [DepDensity; 2] = [DepDensity::Sparse, DepDensity::Dense];

    pub fn label(self) -> &'static str {
        match self {
            DepDensity::Sparse => "sparse",
            DepDensity::Dense => "dense",
        }
    }
}

impl std::fmt::Display for DepDensity {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.label())
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct BenchFixture {
    pub tasks: Vec<GanttTask>,
    pub deps: Vec<GanttDep>,
    pub today: String,
}

pub fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 1, 1).unwrap()
}

pub fn today() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 16).unwrap()
}

/// Generate synthetic tasks spread across a 90-day window with varied progress.
pub fn generate_tasks(count: usize) -> Vec<GanttTask> {
    let base = epoch();
    (0..count)
        .map(|i| {
            let start_offset = (i % 60) as i64;
            let duration = 3 + (i % 7) as i64;
            GanttTask {
                id: format!("task-{i:05}"),
                title: format!("Task {i}"),
                progress_pct: ((i * 37) % 101) as i16,
                start: base + Duration::days(start_offset),
                end: Some(base + Duration::days(start_offset + duration)),
            }
        })
        .collect()
}

/// Sparse: linear chain (each task blocked by its predecessor).
pub fn generate_sparse_deps(tasks: &[GanttTask]) -> Vec<GanttDep> {
    tasks
        .windows(2)
        .map(|w| GanttDep {
            blocker_task_id: w[0].id.clone(),
            blocked_task_id: w[1].id.clone(),
        })
        .collect()
}

/// Dense: each task depends on up to 5 predecessors plus periodic cross-links.
pub fn generate_dense_deps(tasks: &[GanttTask]) -> Vec<GanttDep> {
    let mut deps = Vec::new();
    for (i, task) in tasks.iter().enumerate() {
        if i == 0 {
            continue;
        }
        let start = i.saturating_sub(5);
        for j in start..i {
            deps.push(GanttDep {
                blocker_task_id: tasks[j].id.clone(),
                blocked_task_id: task.id.clone(),
            });
        }
        if i % 10 == 0 {
            deps.push(GanttDep {
                blocker_task_id: tasks[0].id.clone(),
                blocked_task_id: task.id.clone(),
            });
        }
    }
    deps
}

pub fn generate_deps(tasks: &[GanttTask], density: DepDensity) -> Vec<GanttDep> {
    match density {
        DepDensity::Sparse => generate_sparse_deps(tasks),
        DepDensity::Dense => generate_dense_deps(tasks),
    }
}

pub fn generate_fixture(count: TaskCount, density: DepDensity) -> BenchFixture {
    let tasks = generate_tasks(count.get());
    let deps = generate_deps(&tasks, density);
    BenchFixture { tasks, deps, today: today().format("%Y-%m-%d").to_string() }
}

pub fn fixture_id(count: TaskCount, density: DepDensity) -> String {
    format!("{}_{}", count.label(), density.label())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sparse_chain_length() {
        let tasks = generate_tasks(100);
        let deps = generate_sparse_deps(&tasks);
        assert_eq!(deps.len(), 99);
    }

    #[test]
    fn dense_has_more_deps_than_sparse() {
        let tasks = generate_tasks(500);
        let sparse = generate_sparse_deps(&tasks);
        let dense = generate_dense_deps(&tasks);
        assert!(dense.len() > sparse.len());
        assert!(dense.len() > 1000);
    }
}
