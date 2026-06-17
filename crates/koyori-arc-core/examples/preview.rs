use chrono::NaiveDate;
use koyori_arc_core::{render, GanttDep, GanttTask};
use std::path::PathBuf;
use std::process::Command;

fn date(y: i32, m: u32, d: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(y, m, d).unwrap()
}

fn main() {
    let tasks = vec![
        GanttTask {
            id: "t1".to_string(),
            title: "Design".to_string(),
            progress_pct: 100,
            start: date(2026, 6, 1),
            end: Some(date(2026, 6, 4)),
        },
        GanttTask {
            id: "t2".to_string(),
            title: "Backend API".to_string(),
            progress_pct: 60,
            start: date(2026, 6, 2),
            end: Some(date(2026, 6, 8)),
        },
        GanttTask {
            id: "t3".to_string(),
            title: "Frontend".to_string(),
            progress_pct: 30,
            start: date(2026, 6, 4),
            end: Some(date(2026, 6, 10)),
        },
        GanttTask {
            id: "t4".to_string(),
            title: "QA".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 9),
            end: Some(date(2026, 6, 12)),
        },
        GanttTask {
            id: "t5".to_string(),
            title: "Release".to_string(),
            progress_pct: 0,
            start: date(2026, 6, 12),
            end: Some(date(2026, 6, 13)),
        },
    ];

    let deps = vec![
        GanttDep { blocker_task_id: "t1".to_string(), blocked_task_id: "t2".to_string() },
        GanttDep { blocker_task_id: "t1".to_string(), blocked_task_id: "t3".to_string() },
        GanttDep { blocker_task_id: "t2".to_string(), blocked_task_id: "t4".to_string() },
        GanttDep { blocker_task_id: "t3".to_string(), blocked_task_id: "t4".to_string() },
        GanttDep { blocker_task_id: "t4".to_string(), blocked_task_id: "t5".to_string() },
    ];

    let today = date(2026, 6, 6);
    let svg = render(&tasks, &deps, Some(today), None);

    // go up two levels (crates/koyori-arc-core → workspace root) to share target/
    let out_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .join("target/preview");
    std::fs::create_dir_all(&out_dir).unwrap();
    let html_path = out_dir.join("index.html");

    let html = format!(
        r#"<!doctype html>
<html lang="en">
<head>
  <meta charset="utf-8">
  <title>koyori-arc preview</title>
  <style>
    body {{ font-family: sans-serif; padding: 2rem; background: #f9fafb; }}
    h1 {{ font-size: 1rem; color: #6b7280; margin-bottom: 1rem; }}
    .chart {{ background: white; border: 1px solid #e5e7eb; border-radius: 8px; padding: 1rem; display: inline-block; }}
    .legend {{ margin-top: 1rem; display: flex; gap: 1.5rem; font-size: 0.8rem; color: #6b7280; }}
    .dot {{ display: inline-block; width: 12px; height: 12px; border-radius: 2px; margin-right: 4px; vertical-align: middle; }}
  </style>
</head>
<body>
  <h1>koyori-arc — visual preview</h1>
  <div class="chart">{svg}</div>
  <div class="legend">
    <span><span class="dot" style="background:#d1d5db"></span>未達</span>
    <span><span class="dot" style="background:#f59e0b"></span>低 (1–33%)</span>
    <span><span class="dot" style="background:#6366f1"></span>中 (34–66%)</span>
    <span><span class="dot" style="background:#0ea5e9"></span>高 (67–99%)</span>
    <span><span class="dot" style="background:#22c55e"></span>完了 (100%)</span>
    <span><span class="dot" style="background:#9ca3af"></span>dependency</span>
    <span><span class="dot" style="background:#ef4444;opacity:.7"></span>progress line</span>
  </div>
</body>
</html>"#
    );

    std::fs::write(&html_path, &html).unwrap();
    println!("generated: {}", html_path.display());
    open_browser(&html_path);
}

fn open_browser(path: &PathBuf) {
    let path_str = path.to_string_lossy();
    // WSL2: wslpath converts the Linux path to a Windows path so explorer.exe can open it
    if let Ok(out) = Command::new("wslpath").args(["-w", &path_str]).output() {
        if out.status.success() {
            let win_path = String::from_utf8_lossy(&out.stdout).trim().to_string();
            let _ = Command::new("explorer.exe").arg(&win_path).spawn();
            return;
        }
    }
    // fallback for non-WSL Linux
    let _ = Command::new("xdg-open").arg(&*path_str).spawn();
}
