//! One-shot utility to write SVG/IR golden fixtures for display-list tests.

use chrono::NaiveDate;
use koyori_arc_core::{build_display_list, render, GanttDep, GanttGraph, GanttTask};
use std::fs;
use std::path::PathBuf;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn two_task_graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![
            GanttTask {
                id: "task-1".to_string(),
                title: "Design".to_string(),
                progress_pct: 100,
                start: date(2026, 6, 1),
                end: Some(date(2026, 6, 4)),
            },
            GanttTask {
                id: "task-2".to_string(),
                title: "Build".to_string(),
                progress_pct: 50,
                start: date(2026, 6, 4),
                end: Some(date(2026, 6, 8)),
            },
        ],
        deps: vec![GanttDep {
            blocker_task_id: "task-1".to_string(),
            blocked_task_id: "task-2".to_string(),
        }],
    }
}

fn main() {
    let manifest = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let base = manifest.join("tests/fixtures");
    fs::create_dir_all(base.join("svg_golden")).unwrap();
    fs::create_dir_all(base.join("ir_golden")).unwrap();

    let graph = two_task_graph();
    fs::write(
        base.join("svg_golden/two_tasks.svg"),
        render(&graph.tasks, &graph.deps, None),
    )
    .unwrap();
    fs::write(
        base.join("svg_golden/two_tasks_today.svg"),
        render(&graph.tasks, &graph.deps, Some(date(2026, 6, 3))),
    )
    .unwrap();

    let ms = GanttGraph {
        tasks: vec![GanttTask {
            id: "ms1".to_string(),
            title: "Launch".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 3),
            end: Some(date(2026, 6, 3)),
        }],
        deps: vec![],
    };
    fs::write(
        base.join("svg_golden/milestone.svg"),
        render(&ms.tasks, &ms.deps, None),
    )
    .unwrap();
    fs::write(
        base.join("svg_golden/empty.svg"),
        render(&[], &[], None),
    )
    .unwrap();

    let epoch = graph.tasks.iter().map(|t| t.start).min().unwrap();
    let list = build_display_list(&graph, epoch, None);
    fs::write(
        base.join("ir_golden/two_tasks.json"),
        serde_json::to_string_pretty(&list).unwrap(),
    )
    .unwrap();

    println!("Wrote golden fixtures to {}", base.display());
}
