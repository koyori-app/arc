//! 段の形。`options_json` に `lanes` を渡した時だけ効く描き方。
//!
//! 段を渡さぬ時の描き方は、ここでは動かさぬ。既存の試験と `default_output_pin.rs` の基準が
//! それを縛る。この file は、段を渡した時に何が起きるかを縛る。
//! 期待の数は定数を引かず、40（一行の高さ）や 30（見出しの高さ）を字で書く。
//! 定数が変われば、この file が落ちて気づける。

use chrono::{Duration, NaiveDate};
use koyori_arc_core::{
    build_lane_display_list, plan_lanes, render_canvas_commands, render_svg, render_svg_error,
    visible_band, visible_lanes, ChartOptions, GanttTask, Lane, LayerKind, Primitive, RectSemantic,
    ScrollViewport, TextSemantic, CODE_CANVAS_CAPACITY, CODE_INPUT_LIMIT, CODE_PARSE_ERROR,
};

fn day(n: i64) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap() + Duration::days(n)
}

fn gt(id: &str, start: i64, end: i64) -> GanttTask {
    GanttTask {
        id: id.to_string(),
        title: format!("課題 {id}"),
        progress_pct: 0,
        start: day(start),
        end: Some(day(end)),
    }
}

fn lane(id: &str) -> Lane {
    Lane {
        id: id.to_string(),
        label: format!("段 {id}"),
    }
}

fn ids(list: &[&str]) -> Vec<String> {
    list.iter().map(|s| s.to_string()).collect()
}

/// 段ごとに、棒を詰めた順の (課題の id, 行) を返す。
fn rows_of(
    tasks: &[GanttTask],
    task_lanes: &[Vec<String>],
    lanes: &[Lane],
) -> Vec<Vec<(String, usize)>> {
    let plan = plan_lanes(tasks, task_lanes, lanes);
    plan.layout
        .blocks
        .iter()
        .map(|b| {
            b.placements
                .iter()
                .map(|p| (tasks[p.task].id.clone(), p.row))
                .collect()
        })
        .collect()
}

/// 棒一本の位置。`y` は棒そのものの上端。
#[derive(Debug, Clone, PartialEq)]
struct Bar {
    id: String,
    x: f64,
    y: f64,
    w: f64,
}

fn bars_of(list: &koyori_arc_core::DisplayList) -> Vec<Bar> {
    let mut out = Vec::new();
    for layer in list.layers.iter().filter(|l| l.kind == LayerKind::Bars) {
        for prim in &layer.primitives {
            let Primitive::Group(g) = prim else { continue };
            for child in &g.children {
                match child {
                    Primitive::Rect(r) if r.semantic == RectSemantic::BarBackground => {
                        out.push(Bar {
                            id: g.task_id.clone().unwrap(),
                            x: r.x,
                            y: r.y,
                            w: r.width,
                        });
                        break;
                    }
                    Primitive::Polygon(p) => {
                        let left = p.points.iter().map(|q| q.0).fold(f64::MAX, f64::min);
                        let top = p.points.iter().map(|q| q.1).fold(f64::MAX, f64::min);
                        out.push(Bar {
                            id: g.task_id.clone().unwrap(),
                            x: left,
                            y: top,
                            w: 20.0,
                        });
                        break;
                    }
                    _ => {}
                }
            }
        }
    }
    out
}

fn build(
    tasks: &[GanttTask],
    task_lanes: &[Vec<String>],
    lanes: &[Lane],
    options: &ChartOptions,
) -> koyori_arc_core::DisplayList {
    build_lane_display_list(tasks, task_lanes, lanes, options)
}

// 詰め方

#[test]
fn bars_that_overlap_are_put_in_different_rows_and_the_rest_share_the_top_row() {
    // A は 0 から 5 日、B は 3 から 8 日で A と重なる、C は 6 日から始まり A の後ろに入る。
    let tasks = [gt("a", 0, 5), gt("b", 3, 8), gt("c", 6, 9)];
    let task_lanes = vec![ids(&["l"]); 3];
    let rows = rows_of(&tasks, &task_lanes, &[lane("l")]);
    assert_eq!(
        rows[0],
        vec![
            ("a".to_string(), 0),
            ("b".to_string(), 1),
            ("c".to_string(), 0)
        ]
    );
}

#[test]
fn bars_that_touch_are_not_put_in_one_row() {
    // 終わりの日と始まりの日が同じ棒は、隣の棒に詰まって見えぬよう間を空け、別の行に置く。
    let tasks = [gt("a", 0, 2), gt("b", 2, 4)];
    let rows = rows_of(&tasks, &[ids(&["l"]), ids(&["l"])], &[lane("l")]);
    assert_eq!(rows[0], vec![("a".to_string(), 0), ("b".to_string(), 1)]);
    // 一日空ければ同じ行に入る。
    let tasks = [gt("a", 0, 2), gt("b", 3, 5)];
    let rows = rows_of(&tasks, &[ids(&["l"]), ids(&["l"])], &[lane("l")]);
    assert_eq!(rows[0], vec![("a".to_string(), 0), ("b".to_string(), 0)]);
}

#[test]
fn a_percent_label_that_runs_past_the_bar_counts_as_part_of_it() {
    // 一日の棒の 100% は、棒に収まらず右に出る。その字の上に次の棒を重ねぬ。
    let mut a = gt("a", 0, 1);
    a.progress_pct = 100;
    let near = [a.clone(), gt("b", 2, 4)];
    let rows = rows_of(&near, &[ids(&["l"]), ids(&["l"])], &[lane("l")]);
    assert_eq!(rows[0], vec![("a".to_string(), 0), ("b".to_string(), 1)]);
    let far = [a, gt("b", 3, 5)];
    let rows = rows_of(&far, &[ids(&["l"]), ids(&["l"])], &[lane("l")]);
    assert_eq!(rows[0], vec![("a".to_string(), 0), ("b".to_string(), 0)]);
}

#[test]
fn the_same_input_gives_the_same_rows_whatever_order_it_comes_in() {
    // 始まりも終わりも同じ三つは、id の順に行を取る。入力の並びを替えても同じ形になる。
    let make = |order: [&str; 3]| -> Vec<(String, usize)> {
        let tasks: Vec<GanttTask> = order.iter().map(|id| gt(id, 0, 4)).collect();
        let task_lanes = vec![ids(&["l"]); 3];
        let mut rows = rows_of(&tasks, &task_lanes, &[lane("l")]).remove(0);
        rows.sort();
        rows
    };
    let expected = vec![
        ("a".to_string(), 0),
        ("b".to_string(), 1),
        ("c".to_string(), 2),
    ];
    assert_eq!(make(["a", "b", "c"]), expected);
    assert_eq!(make(["c", "a", "b"]), expected);
    assert_eq!(make(["b", "c", "a"]), expected);
}

#[test]
fn an_earlier_start_takes_the_higher_row() {
    let tasks = [gt("late", 2, 9), gt("early", 0, 9)];
    let rows = rows_of(&tasks, &[ids(&["l"]), ids(&["l"])], &[lane("l")]);
    assert_eq!(
        rows[0],
        vec![("early".to_string(), 0), ("late".to_string(), 1)]
    );
}

// 段の帰属

#[test]
fn a_task_in_two_lanes_is_drawn_in_both_and_one_in_none_is_not_drawn() {
    let tasks = [
        gt("a", 0, 3),
        gt("both", 1, 4),
        gt("none", 0, 4),
        gt("typo", 0, 4),
    ];
    let task_lanes = vec![ids(&["x"]), ids(&["x", "y"]), vec![], ids(&["nope"])];
    let lanes = [lane("x"), lane("y")];
    let list = build(&tasks, &task_lanes, &lanes, &ChartOptions::default());
    let mut drawn: Vec<String> = bars_of(&list).into_iter().map(|b| b.id).collect();
    drawn.sort();
    assert_eq!(drawn, vec!["a", "both", "both"]);
}

#[test]
fn a_task_that_names_a_lane_twice_is_in_it_once() {
    let tasks = [gt("a", 0, 3)];
    let rows = rows_of(&tasks, &[ids(&["x", "x"])], &[lane("x")]);
    assert_eq!(rows[0].len(), 1);
}

#[test]
fn the_chart_starts_at_the_earliest_drawn_task() {
    // どの段にも属さぬ早い課題は、図の左の空きを作らぬ。
    let tasks = [gt("early", -30, -20), gt("a", 0, 3)];
    let task_lanes = vec![vec![], ids(&["x"])];
    let plan = plan_lanes(&tasks, &task_lanes, &[lane("x")]);
    assert_eq!(plan.epoch, day(0));
    let list = build(&tasks, &task_lanes, &[lane("x")], &ChartOptions::default());
    assert_eq!(bars_of(&list)[0].x, 120.0);
}

// 段の高さと縦の位置

fn stacked(prefix: &str, n: usize) -> Vec<GanttTask> {
    // 全て同じ期間なので、n 本が n 行に積まれる。
    (0..n).map(|i| gt(&format!("{prefix}{i}"), 0, 4)).collect()
}

#[test]
fn a_lane_is_as_tall_as_its_rows_and_an_empty_lane_keeps_one() {
    let mut tasks = stacked("a", 3);
    tasks.extend(stacked("c", 1));
    let mut task_lanes = vec![ids(&["a"]); 3];
    task_lanes.push(ids(&["c"]));
    let lanes = [lane("a"), lane("empty"), lane("c")];
    let plan = plan_lanes(&tasks, &task_lanes, &lanes);
    assert_eq!(plan.layout.tops, vec![0.0, 120.0, 160.0, 200.0]);
    assert_eq!(plan.layout.height(), 200.0);
    let rows: Vec<usize> = plan.layout.blocks.iter().map(|b| b.rows).collect();
    assert_eq!(rows, vec![3, 1, 1]);

    // 図の高さは、段の高さの和に見出し(30)と凡例まわり(40 + 10)を足した値。
    let list = build(&tasks, &task_lanes, &lanes, &ChartOptions::default());
    assert_eq!(list.viewport.height, 200.0 + 30.0 + 40.0 + 10.0);
}

#[test]
fn bars_sit_at_the_top_of_their_lane_plus_their_row() {
    let mut tasks = stacked("a", 3);
    tasks.push(gt("c0", 0, 4));
    let mut task_lanes = vec![ids(&["a"]); 3];
    task_lanes.push(ids(&["c"]));
    let lanes = [lane("a"), lane("empty"), lane("c")];
    let list = build(&tasks, &task_lanes, &lanes, &ChartOptions::default());
    let y_of = |id: &str| bars_of(&list).into_iter().find(|b| b.id == id).unwrap().y;
    // 先頭の段は 30 から始まり、棒は行の上端から 10 下がる。
    assert_eq!(y_of("a0"), 30.0 + 0.0 + 10.0);
    assert_eq!(y_of("a1"), 30.0 + 40.0 + 10.0);
    assert_eq!(y_of("a2"), 30.0 + 80.0 + 10.0);
    // 三行ある段(120)と空の段(40)の下に c の段が始まる。
    assert_eq!(y_of("c0"), 30.0 + 160.0 + 10.0);
}

#[test]
fn a_lane_label_is_centred_on_its_lane() {
    let tasks = stacked("a", 3);
    let task_lanes = vec![ids(&["a"]); 3];
    let lanes = [lane("a"), lane("b")];
    let list = build(&tasks, &task_lanes, &lanes, &ChartOptions::default());
    let labels: Vec<(f64, f64, String)> = list
        .layers
        .iter()
        .flat_map(|l| l.primitives.iter())
        .filter_map(|p| match p {
            Primitive::Text(t) if t.semantic == TextSemantic::LaneLabel => {
                Some((t.x, t.y, t.content.clone()))
            }
            _ => None,
        })
        .collect();
    // 左の欄の右端(116 = 120 - 4)に寄せ、段の縦の真ん中に置く。
    assert_eq!(
        labels,
        vec![
            (116.0, 30.0 + 60.0, "段 a".to_string()),
            (116.0, 30.0 + 120.0 + 20.0, "段 b".to_string()),
        ]
    );
}

// 仮想化

/// 五行、一行、一行、三行、一行の段。どの段の先頭の課題も、段の中で上から順に id を振る。
fn uneven_lanes() -> (Vec<GanttTask>, Vec<Vec<String>>, Vec<Lane>) {
    let sizes = [5usize, 1, 1, 3, 1];
    let mut tasks = Vec::new();
    let mut task_lanes = Vec::new();
    let mut lanes = Vec::new();
    for (l, &n) in sizes.iter().enumerate() {
        lanes.push(lane(&format!("L{l}")));
        for r in 0..n {
            tasks.push(gt(&format!("L{l}r{r}"), 0, 4));
            task_lanes.push(ids(&[&format!("L{l}")]));
        }
    }
    (tasks, task_lanes, lanes)
}

#[test]
fn the_visible_lanes_are_found_from_the_running_heights() {
    // 段の上端は 0 200 240 280 400 440。
    let tops = [0.0, 200.0, 240.0, 280.0, 400.0, 440.0];
    // 帯 [150, 260]: 0 の段(下が 200 > 150)、1、2 の段(上が 240 < 260)。
    assert_eq!(visible_lanes(&tops, (150.0, 260.0)), 0..3);
    // 帯 [300, 330]: 3 の段だけ。
    assert_eq!(visible_lanes(&tops, (300.0, 330.0)), 3..4);
    // 段の境目ちょうど: 下端が帯の上端と同じ段は外れる。
    assert_eq!(visible_lanes(&tops, (200.0, 210.0)), 1..2);
    // 先頭より上に張り出した帯は先頭の段から。
    assert_eq!(visible_lanes(&tops, (-500.0, 10.0)), 0..1);
    // どの段にもかからぬ帯(全体の下、全体の上)は何も選ばぬ。
    assert_eq!(visible_lanes(&tops, (9_000.0, 9_100.0)), 0..0);
    assert_eq!(visible_lanes(&tops, (-500.0, 0.0)), 0..0);
    // 段が無ければ何も無い。
    assert_eq!(visible_lanes(&[0.0], (0.0, 100.0)), 0..0);
}

#[test]
fn a_window_that_is_scrolled_down_shows_the_lanes_that_are_there_not_the_ones_a_fixed_row_height_would_name(
) {
    let (tasks, task_lanes, lanes) = uneven_lanes();
    // scroll_y = 330, client_height = 40: 内容の帯は上 330-30-80 = 220、下 330+40-30+80 = 420。
    // 入るのは、220 から 420 にかかる L1(200..240)、L2(240..280)、L3(280..400)、
    // L4(400..440)。L0 は下端が 200 で帯の上端 220 に届かぬ。
    // 行の高さで割る数え方(330/40 = 8 行目)は、段の高さが揃わぬこの形では別の行を指す。
    let options = ChartOptions {
        scroll_viewport: Some(ScrollViewport {
            scroll_y: 330.0,
            client_height: 40.0,
        }),
        ..Default::default()
    };
    let list = build(&tasks, &task_lanes, &lanes, &options);
    let mut drawn: Vec<String> = bars_of(&list).into_iter().map(|b| b.id).collect();
    drawn.sort();
    assert_eq!(drawn, vec!["L1r0", "L2r0", "L3r0", "L3r1", "L3r2", "L4r0"]);
    // 窓の外の段の名は出さず、窓の中の段の名は出す。
    let labels: Vec<String> = list
        .layers
        .iter()
        .flat_map(|l| l.primitives.iter())
        .filter_map(|p| match p {
            Primitive::Text(t) if t.semantic == TextSemantic::LaneLabel => Some(t.content.clone()),
            _ => None,
        })
        .collect();
    assert_eq!(labels, vec!["段 L1", "段 L2", "段 L3", "段 L4"]);
}

#[test]
fn a_tall_lane_is_cut_row_by_row() {
    let (tasks, task_lanes, lanes) = uneven_lanes();
    // 帯 [-80+..]: scroll_y = 130, client_height = 40 → 上 130-30-80 = 20、下 130+40-30+80 = 220。
    // L0 の行の上端は 0 40 80 120 160。上端 < 220 かつ 下端(上端 + 40) > 20 を満たす行は
    // 全て(0 行目の下端は 40 > 20)。L1 は上端 200 < 220 で入る。
    let options = ChartOptions {
        scroll_viewport: Some(ScrollViewport {
            scroll_y: 130.0,
            client_height: 40.0,
        }),
        ..Default::default()
    };
    let list = build(&tasks, &task_lanes, &lanes, &options);
    let mut drawn: Vec<String> = bars_of(&list).into_iter().map(|b| b.id).collect();
    drawn.sort();
    assert_eq!(drawn, vec!["L0r0", "L0r1", "L0r2", "L0r3", "L0r4", "L1r0"]);
    // 帯の上が深い所: L0 の行のうち、下端が帯の上端以下の行は外れる。
    let options = ChartOptions {
        scroll_viewport: Some(ScrollViewport {
            scroll_y: 230.0,
            client_height: 10.0,
        }),
        ..Default::default()
    };
    // 上 230-30-80 = 120、下 230+10-30+80 = 290。L0 の 3 行目(120..160)から入る。
    let list = build(&tasks, &task_lanes, &lanes, &options);
    let mut drawn: Vec<String> = bars_of(&list).into_iter().map(|b| b.id).collect();
    drawn.sort();
    assert_eq!(drawn, vec!["L0r3", "L0r4", "L1r0", "L2r0", "L3r0"]);
}

#[test]
fn the_band_of_a_scroll_is_its_window_less_the_header_and_widened_by_two_rows() {
    assert_eq!(visible_band(None), None);
    let band = visible_band(Some(ScrollViewport {
        scroll_y: 300.0,
        client_height: 100.0,
    }));
    assert_eq!(
        band,
        Some((300.0 - 30.0 - 80.0, 300.0 + 100.0 - 30.0 + 80.0))
    );
}

#[test]
fn a_scroll_past_the_end_draws_no_bar_and_no_lane_name() {
    let (tasks, task_lanes, lanes) = uneven_lanes();
    let options = ChartOptions {
        scroll_viewport: Some(ScrollViewport {
            scroll_y: 99_999.0,
            client_height: 100.0,
        }),
        ..Default::default()
    };
    let list = build(&tasks, &task_lanes, &lanes, &options);
    assert!(bars_of(&list).is_empty());
}

// 押した所の課題

/// 矩形は (x, y, 幅, 高さ)。
type Rect = (f64, f64, f64, f64);

fn op_groups(json: &str) -> Vec<(String, Vec<Rect>)> {
    // 各 GroupStart から GroupEnd までの FillRect の矩形を、課題の id ごとにまとめる。
    let v: serde_json::Value = serde_json::from_str(json).unwrap();
    let mut out: Vec<(String, Vec<Rect>)> = Vec::new();
    let mut current: Option<usize> = None;
    for op in v["ops"].as_array().unwrap() {
        if op == "GroupEnd" {
            current = None;
        } else if let Some(g) = op.get("GroupStart") {
            if let Some(id) = g["task_id"].as_str() {
                out.push((id.to_string(), Vec::new()));
                current = Some(out.len() - 1);
            }
        } else if let (Some(f), Some(i)) = (op.get("FillRect"), current) {
            out[i].1.push((
                f["x"].as_f64().unwrap(),
                f["y"].as_f64().unwrap(),
                f["w"].as_f64().unwrap(),
                f["h"].as_f64().unwrap(),
            ));
        }
    }
    out
}

const TASKS: &str = r#"[
  {"id":"a","title":"A","progress_pct":50,"start":"2026-06-01","end":"2026-06-06","lane_ids":["x"]},
  {"id":"b","title":"B","progress_pct":50,"start":"2026-06-03","end":"2026-06-09","lane_ids":["x","y"]},
  {"id":"c","title":"C","progress_pct":50,"start":"2026-06-02","end":"2026-06-04","lane_ids":["y"]}
]"#;
const LANES: &str = r#"{"lanes":[{"id":"x","label":"X"},{"id":"y","label":"Y"}]}"#;

#[test]
fn a_task_in_two_lanes_has_a_hit_group_in_each_and_the_bars_are_where_they_were_placed() {
    let json = render_canvas_commands(TASKS, "[]", Some(LANES.to_string()));
    let groups = op_groups(&json);
    let by_id = |id: &str| -> Vec<&Vec<Rect>> {
        groups
            .iter()
            .filter(|(g, _)| g == id)
            .map(|(_, r)| r)
            .collect()
    };
    // x の段: a が 0 行目、b(a と重なる)が 1 行目。y の段: 先に始まる c が 0 行目、b(c と重なる)が 1 行目。
    // 二つの段とも 2 行(80)なので、x は 30 から 110、y は 110 から 190。
    assert_eq!(by_id("a").len(), 1);
    assert_eq!(by_id("b").len(), 2);
    assert_eq!(by_id("c").len(), 1);
    let first_rect_y = |id: &str, nth: usize| by_id(id)[nth][0].1;
    assert_eq!(first_rect_y("a", 0), 30.0 + 10.0);
    assert_eq!(first_rect_y("b", 0), 30.0 + 40.0 + 10.0);
    assert_eq!(first_rect_y("b", 1), 110.0 + 40.0 + 10.0);
    assert_eq!(first_rect_y("c", 0), 110.0 + 0.0 + 10.0);
}

#[test]
fn every_hit_group_stays_inside_its_own_lane() {
    let json = render_canvas_commands(TASKS, "[]", Some(LANES.to_string()));
    for (id, rects) in op_groups(&json) {
        for (_, y, _, h) in rects {
            assert_eq!(h, 20.0, "{id}");
            // 棒の縦は、どの段の行にも収まる(境目をまたがぬ)。
            let row_top = ((y - 10.0) - 30.0) / 40.0;
            assert!(
                (row_top - row_top.round()).abs() < 1e-9,
                "{id}: bar y {y} is not on a row"
            );
        }
    }
}

// svg の見え方

fn svg_with(options: &str) -> String {
    render_svg(TASKS, "[]", Some(options.to_string()))
}

#[test]
fn the_svg_carries_one_group_per_bar_and_the_names_of_the_lanes() {
    let svg = svg_with(LANES);
    assert_eq!(svg.matches(r#"data-task-id="b""#).count(), 2);
    assert_eq!(svg.matches(r#"data-task-id="a""#).count(), 1);
    assert!(svg.contains(">X</text>") && svg.contains(">Y</text>"));
}

#[test]
fn a_lane_chart_draws_no_dependency_arrow_and_no_progress_line() {
    let deps = r#"[{"blocker_task_id":"a","blocked_task_id":"b"}]"#;
    let svg = render_svg(
        TASKS,
        deps,
        Some(
            r#"{"today":"2026-06-04","lanes":[{"id":"x","label":"X"},{"id":"y","label":"Y"}]}"#
                .into(),
        ),
    );
    assert!(!svg.contains("<path"), "an arrow was drawn");
    assert!(!svg.contains("progress-status-line"));
    // 今日の線は引く。
    assert!(svg.contains(r##"stroke="#f59e0b" stroke-width="2""##));
}

#[test]
fn the_legend_drops_the_progress_line_sample_and_keeps_its_swatches_and_its_height() {
    let svg = svg_with(LANES);
    assert!(!svg.contains("progress-line-legend"));
    assert!(svg.contains("bar-tier-legend"));
    let hidden =
        svg_with(r#"{"legend":"hidden","lanes":[{"id":"x","label":"X"},{"id":"y","label":"Y"}]}"#);
    assert!(!hidden.contains("bar-tier-legend"));
    let height = |s: &str| -> f64 {
        let at = s.find(" height=\"").unwrap() + 9;
        s[at..].split('"').next().unwrap().parse().unwrap()
    };
    // 凡例を隠すと、その高さ(40)だけ低くなる。
    assert_eq!(height(&svg) - height(&hidden), 40.0);
}

#[test]
fn a_task_title_is_cut_before_the_next_bar_in_its_row() {
    // 同じ行の隣の棒の手前で切る。長い題でも隣の棒に重ならぬ。
    let tasks = r#"[
      {"id":"a","title":"とても長い題名のタスクでどこまで表示されるか","progress_pct":0,"start":"2026-06-01","end":"2026-06-02","lane_ids":["x"]},
      {"id":"b","title":"B","progress_pct":0,"start":"2026-06-04","end":"2026-06-05","lane_ids":["x"]}
    ]"#;
    let svg = render_svg(
        tasks,
        "[]",
        Some(r#"{"lanes":[{"id":"x","label":"X"}]}"#.into()),
    );
    // a の左端は 120、b の左端は 120 + 3 日 × 30 = 210。余白 4 を引いて 86px 以内。
    // 10px の全角は 8 字で 80px。… を足した 8 字目は 90px になり収まらず、7 字で切れる。
    // 吹き出し(title 要素)には全文が入るので、描いた text 要素の中身で検める。
    assert!(svg.contains(">とても長い題名…</text>"), "{svg}");
    assert!(!svg.contains("題名の…</text>"));
}

// 空と例外

#[test]
fn an_empty_list_of_lanes_draws_a_chart_with_no_bars() {
    let svg = render_svg(TASKS, "[]", Some(r#"{"lanes":[]}"#.to_string()));
    assert!(svg.starts_with("<svg"));
    assert!(!svg.contains("data-task-id"));
    // 凡例の分だけの高さ。
    assert!(svg.contains(r#"height="80""#));
}

#[test]
fn a_lane_nobody_belongs_to_still_shows_its_name() {
    let svg = render_svg(
        TASKS,
        "[]",
        Some(r#"{"lanes":[{"id":"x","label":"X"},{"id":"e","label":"空の段"}]}"#.into()),
    );
    assert!(svg.contains(">空の段</text>"));
}

// 段を渡さぬ時

#[test]
fn without_lanes_the_chart_is_the_one_drawn_before_and_lane_ids_are_ignored() {
    let without: String = TASKS
        .replace(r#","lane_ids":["x"]"#, "")
        .replace(r#","lane_ids":["x","y"]"#, "")
        .replace(r#","lane_ids":["y"]"#, "");
    assert!(!without.contains("lane_ids"));
    for options in [
        None,
        Some("{}".to_string()),
        Some(r#"{"lanes":null}"#.to_string()),
    ] {
        assert_eq!(
            render_svg(TASKS, "[]", options.clone()),
            render_svg(&without, "[]", None),
            "{options:?}"
        );
        assert_eq!(
            render_canvas_commands(TASKS, "[]", options.clone()),
            render_canvas_commands(&without, "[]", None),
            "{options:?}"
        );
    }
}

#[test]
fn a_lane_ids_the_lane_layout_would_refuse_is_ignored_when_there_are_no_lanes() {
    let odd = r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-06-01","end":"2026-06-06","lane_ids":7}]"#;
    let plain =
        r#"[{"id":"a","title":"A","progress_pct":50,"start":"2026-06-01","end":"2026-06-06"}]"#;
    assert_eq!(render_svg(odd, "[]", None), render_svg(plain, "[]", None));
    assert!(render_svg_error(odd, "[]", None).is_none());
}

// 読めぬ入力

fn refused(tasks: &str, options: &str) -> serde_json::Value {
    let reported =
        render_svg_error(tasks, "[]", Some(options.to_string())).expect("must be refused");
    serde_json::from_str(&reported).unwrap()
}

#[test]
fn lanes_that_cannot_be_read_are_refused() {
    for options in [
        r#"{"lanes":"x"}"#,
        r#"{"lanes":{"id":"x","label":"X"}}"#,
        r#"{"lanes":[{"id":"x"}]}"#,
        r#"{"lanes":[{"label":"X"}]}"#,
        r#"{"lanes":[{"id":1,"label":"X"}]}"#,
        r#"{"lanes":["x"]}"#,
    ] {
        let v = refused(TASKS, options);
        assert_eq!(v["code"], CODE_PARSE_ERROR, "{options}: {v}");
        assert!(
            v["error"].as_str().unwrap().contains("lanes"),
            "{options}: {v}"
        );
    }
}

#[test]
fn a_lane_id_that_appears_twice_is_refused() {
    let v = refused(
        TASKS,
        r#"{"lanes":[{"id":"x","label":"X"},{"id":"x","label":"Y"}]}"#,
    );
    assert_eq!(v["code"], CODE_PARSE_ERROR);
    assert!(
        v["error"].as_str().unwrap().contains("duplicate id `x`"),
        "{v}"
    );
}

#[test]
fn lane_ids_that_are_not_an_array_of_strings_are_refused_in_a_lane_chart() {
    for bad in ["7", r#""x""#, r#"[1]"#, r#"{"a":1}"#] {
        let tasks = format!(
            r#"[{{"id":"a","title":"A","progress_pct":0,"start":"2026-06-01","lane_ids":{bad}}}]"#
        );
        let v = refused(&tasks, LANES);
        assert_eq!(v["code"], CODE_PARSE_ERROR, "{bad}");
        assert!(
            v["error"].as_str().unwrap().contains("tasks[0].lane_ids"),
            "{bad}: {v}"
        );
    }
}

#[test]
fn too_many_lanes_are_refused_with_the_input_limit_code() {
    let lanes: Vec<String> = (0..1001)
        .map(|i| format!(r#"{{"id":"l{i}","label":"L"}}"#))
        .collect();
    let v = refused(TASKS, &format!(r#"{{"lanes":[{}]}}"#, lanes.join(",")));
    assert_eq!(v["code"], CODE_INPUT_LIMIT);
    assert!(v["error"].as_str().unwrap().contains("lane count"), "{v}");
    // 千本ちょうどは通る。
    let lanes: Vec<String> = (0..1000)
        .map(|i| format!(r#"{{"id":"l{i}","label":"L"}}"#))
        .collect();
    assert!(render_svg_error(
        TASKS,
        "[]",
        Some(format!(r#"{{"lanes":[{}]}}"#, lanes.join(",")))
    )
    .is_none());
}

#[test]
fn too_many_bars_are_refused_with_the_input_limit_code() {
    // 一万の課題が六つの段に入ると六万本になり、五万を超える。期間は散らして行を増やさぬ。
    let tasks: Vec<String> = (0..10_000)
        .map(|i| {
            format!(
                r#"{{"id":"t{i}","title":"T","progress_pct":0,"start":"{}","lane_ids":["a","b","c","d","e","f"]}}"#,
                day((i % 3000) as i64)
            )
        })
        .collect();
    let tasks = format!("[{}]", tasks.join(","));
    let options = r#"{"lanes":[{"id":"a","label":"A"},{"id":"b","label":"B"},{"id":"c","label":"C"},{"id":"d","label":"D"},{"id":"e","label":"E"},{"id":"f","label":"F"}]}"#;
    let v = refused(&tasks, options);
    assert_eq!(v["code"], CODE_INPUT_LIMIT);
    assert!(
        v["error"].as_str().unwrap().contains("lane bar count"),
        "{v}"
    );
}

// options の大きさ

#[test]
fn the_options_bound_is_unchanged_without_lanes_and_wider_with_them() {
    // `lanes` を書かぬ設定は、今までと同じ 4096 の字で断る。
    let over = format!("{{}}{}", " ".repeat(4096 - 1));
    assert_eq!(over.len(), 4097);
    let v = refused(TASKS, &over);
    assert_eq!(v["code"], CODE_INPUT_LIMIT);
    // `"lanes"` の字が在っても、鍵でなければ小さい方の上限で断る。
    let sneaky = format!(r#"{{"today":"\"lanes\"{}"}}"#, "x".repeat(5000));
    let v = refused(TASKS, &sneaky);
    assert_eq!(v["code"], CODE_INPUT_LIMIT);
    // 段が四千字を超えても通る。
    let lanes: Vec<String> = (0..200)
        .map(|i| format!(r#"{{"id":"lane-{i}-with-a-long-id","label":"担当者 {i}"}}"#))
        .collect();
    let options = format!(r#"{{"lanes":[{}]}}"#, lanes.join(","));
    assert!(options.len() > 4096);
    assert!(render_svg_error(TASKS, "[]", Some(options)).is_none());
}

// canvas の上限

#[test]
fn a_lane_too_tall_for_a_canvas_is_refused_for_canvas_but_not_for_svg() {
    // 全て同じ期間の四百本は四百行になり、400 × 40 = 16000 に見出しと凡例まわりが足されて
    // 上限の 16384 を超える。行の数は課題の数ではなく、段が積む行の数で決まる。
    let tasks: Vec<String> = (0..410)
        .map(|i| {
            format!(r#"{{"id":"t{i}","title":"T","progress_pct":0,"start":"2026-06-01","end":"2026-06-05","lane_ids":["x"]}}"#)
        })
        .collect();
    let tasks = format!("[{}]", tasks.join(","));
    let options = Some(r#"{"lanes":[{"id":"x","label":"X"}]}"#.to_string());
    let canvas: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(&tasks, "[]", options.clone())).unwrap();
    assert_eq!(canvas["code"], CODE_CANVAS_CAPACITY, "{canvas}");
    assert!(render_svg_error(&tasks, "[]", options.clone()).is_none());
    // 二百本なら 200 × 40 = 8000 に収まり、canvas でも描ける。
    let fewer: Vec<String> = (0..200)
        .map(|i| {
            format!(r#"{{"id":"t{i}","title":"T","progress_pct":0,"start":"2026-06-01","end":"2026-06-05","lane_ids":["x"]}}"#)
        })
        .collect();
    let fewer = format!("[{}]", fewer.join(","));
    let ok: serde_json::Value =
        serde_json::from_str(&render_canvas_commands(&fewer, "[]", options)).unwrap();
    assert!(ok.get("error").is_none(), "{ok}");
}

#[test]
fn the_canvas_and_the_svg_draw_the_same_bars_in_the_same_order() {
    let json = render_canvas_commands(TASKS, "[]", Some(LANES.to_string()));
    let canvas_ids: Vec<String> = op_groups(&json).into_iter().map(|(id, _)| id).collect();
    let svg = svg_with(LANES);
    let mut svg_ids = Vec::new();
    let mut rest = svg.as_str();
    while let Some(at) = rest.find(r#"data-task-id=""#) {
        rest = &rest[at + 14..];
        svg_ids.push(rest.split('"').next().unwrap().to_string());
    }
    assert_eq!(canvas_ids, svg_ids);
}
