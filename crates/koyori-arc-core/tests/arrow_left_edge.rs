//! 題を消した図で、先頭の日に始まる課題へ後ろから回り込む依存の矢印が、図の左の端（x < 0）へ出ないこと。
//! SVG と canvas の二つの入口で同じ道を描く。

use koyori_arc_core::{render_canvas_commands, render_svg};

/// 依存の矢印の色（既定の色の表の `Dep`）と、canvas の色の番号。
const DEP_COLOR: &str = "#9ca3af";
const DEP_COLOR_ID: u64 = 5;

fn s(json: &str) -> Option<String> {
    Some(json.to_string())
}

/// 道の端の点の x を全て返す。矢印の道は `M`・`L`・`m`・`l`・`H`・`V`・`v`・`a` だけを使い、
/// 弧は四分の一の円なので、端の点が道の左右の端になる。
fn path_xs(d: &str) -> Vec<f64> {
    let mut tokens = d.split_whitespace().peekable();
    let mut x = 0.0_f64;
    let mut xs = Vec::new();
    fn num(t: &mut std::iter::Peekable<std::str::SplitWhitespace>) -> f64 {
        t.next().expect("number").parse().expect("number")
    }
    while let Some(cmd) = tokens.next() {
        match cmd {
            "M" | "L" => {
                x = num(&mut tokens);
                num(&mut tokens);
            }
            "m" | "l" => {
                x += num(&mut tokens);
                num(&mut tokens);
            }
            "H" => x = num(&mut tokens),
            "V" | "v" => {
                num(&mut tokens);
            }
            "a" => {
                for _ in 0..5 {
                    num(&mut tokens);
                }
                x += num(&mut tokens);
                num(&mut tokens);
            }
            other => panic!("unexpected path command {other} in {d}"),
        }
        xs.push(x);
    }
    xs
}

fn svg_arrow(tasks: &str, deps: &str, options: &str) -> String {
    let svg = render_svg(tasks, deps, s(options), None);
    svg.split("<path d=\"")
        .skip(1)
        .find(|rest| rest.split("/>").next().unwrap_or("").contains(DEP_COLOR))
        .and_then(|rest| rest.split('"').next())
        .unwrap_or_else(|| panic!("no dependency arrow in {svg}"))
        .to_string()
}

fn canvas_arrow(tasks: &str, deps: &str, options: &str) -> String {
    let canvas: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(tasks, deps, s(options), None)).unwrap();
    canvas["ops"]
        .as_array()
        .unwrap()
        .iter()
        .find(|op| op["StrokePath"]["color_id"] == DEP_COLOR_ID)
        .expect("dependency arrow")["StrokePath"]["d"]
        .as_str()
        .unwrap()
        .to_string()
}

const DEPS: &str = r#"[{"blocker_task_id":"a","blocked_task_id":"b"}]"#;

/// 先に始まる課題の後ろから、先頭の日に始まる課題へ回り込む形の組。
const CASES: [&str; 2] = [
    // 両方が先頭の日に始まる
    r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-06"},{"id":"b","title":"B","progress_pct":0,"start":"2026-11-02","end":"2026-11-04"}]"#,
    // 先の課題は二日目に始まり、後の課題が先頭の日に始まる
    r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-11-02","end":"2026-11-06"},{"id":"b","title":"B","progress_pct":0,"start":"2026-11-02","end":"2026-11-09"},{"id":"c","title":"C","progress_pct":0,"start":"2026-11-03","end":"2026-11-05"}]"#,
];

#[test]
fn a_detour_into_a_first_day_bar_stays_inside_a_chart_without_titles() {
    for (i, tasks) in CASES.iter().enumerate() {
        for options in [
            r#"{"row_labels":"hidden"}"#,
            r#"{"row_labels":"hidden","bar_height_px":12,"row_height_px":40}"#,
        ] {
            let d = svg_arrow(tasks, DEPS, options);
            let xs = path_xs(&d);
            let min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
            assert!(min >= 0.0, "case {i} {options}: min x {min} in {d}");
            // canvas は同じ道を描く
            assert_eq!(canvas_arrow(tasks, DEPS, options), d, "case {i} {options}");
        }
    }
}

#[test]
fn a_detour_that_fits_is_drawn_as_before() {
    // 題の列のある図は、回り込みが余白の内に収まるので、縛らない形のまま（左へ ROW_PADDING と曲がりの分だけ回る）
    let d = svg_arrow(CASES[0], DEPS, "{}");
    let xs = path_xs(&d);
    let min = xs.iter().cloned().fold(f64::INFINITY, f64::min);
    // 題の列は 120px。回り込みは先頭の日の 120 から ROW_PADDING（行の高さ 40 − 棒の高さ 20）と曲がりの 4 だけ左
    assert_eq!(min, 120.0 - 20.0 - 4.0, "{d}");
}
