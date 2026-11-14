//! 稲妻線の出し入れと、行ごとの決まり（`by_row`）。
//! 設計書 docs/design/progress-line.md の 3 と 4 にあたる。

use chrono::{Duration, NaiveDate};

use super::build::build_display_list_with_rows;
use super::constants::*;
use super::types::*;
use crate::graph::{BarState, GanttGraph, GanttRow, GanttTask, TaskRow};

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
}

fn day(n: i64) -> NaiveDate {
    epoch() + Duration::days(n)
}

fn task(id: &str, start: i64, end: i64, pct: i16) -> GanttTask {
    GanttTask {
        id: id.to_string(),
        title: id.to_string(),
        progress_pct: pct,
        start: day(start),
        end: Some(day(end)),
    }
}

/// 今日は 5 日目（11/7）。
const TODAY: i64 = 5;

fn graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![
            // 遅れ: 0〜10 日の 20%、進みは 2 日目
            task("late", 0, 10, 20),
            // 済み（行の state が done）: 今日の線のまま
            task("done", 0, 4, 100),
            // 0% で予定開始の 2 日目を過ぎた: 予定開始へ
            task("stalled", 2, 9, 0),
            // まだ始まらぬ 0%: 今日の線のまま
            task("future", 8, 12, 0),
            // 先行: 3〜13 日の 30%、進みは 6 日目
            task("ahead", 3, 13, 30),
        ],
        deps: vec![],
    }
}

fn x(days: f64) -> f64 {
    LABEL_W + days * PX_PER_DAY
}

fn today_x() -> f64 {
    x(TODAY as f64)
}

/// 行 `i`（0 から）の上の端。行は全て `ROW_H`。
fn top(i: usize) -> f64 {
    HEADER_H + i as f64 * ROW_H
}

fn mid(i: usize) -> f64 {
    top(i) + ROW_H / 2.0
}

fn options(mode: ProgressLineMode) -> ChartOptions {
    ChartOptions {
        today: Some(day(TODAY)),
        progress_line: mode,
        ..Default::default()
    }
}

fn build_rows(graph: &GanttGraph, rows: &[GanttRow], options: &ChartOptions) -> DisplayList {
    build_display_list_with_rows(graph, Some(rows), epoch(), options)
}

fn task_rows(ids: &[&str]) -> Vec<GanttRow> {
    ids.iter()
        .map(|id| GanttRow::Task(TaskRow::new(*id)))
        .collect()
}

/// `done` の行だけ state を done にした一覧。
fn all_rows() -> Vec<GanttRow> {
    let mut rows = task_rows(&["late", "done", "stalled", "future", "ahead"]);
    rows[1] = done_row("done");
    rows
}

fn done_row(id: &str) -> GanttRow {
    GanttRow::Task(TaskRow {
        state: Some(BarState::Done),
        ..TaskRow::new(id)
    })
}

fn line_layer(list: &DisplayList) -> Option<&Layer> {
    list.layers
        .iter()
        .find(|l| l.kind == LayerKind::ProgressLine)
}

fn line(list: &DisplayList) -> PolylinePrim {
    match &line_layer(list).expect("稲妻線").primitives[0] {
        Primitive::Polyline(p) => p.clone(),
        other => panic!("{other:?}"),
    }
}

fn dots(list: &DisplayList) -> Vec<(f64, f64)> {
    line_layer(list)
        .map(|l| {
            l.primitives
                .iter()
                .filter_map(|p| match p {
                    Primitive::RoundRect(r) if r.semantic == RoundRectSemantic::ProgressPoint => {
                        Some((r.x + r.width / 2.0, r.y + r.height / 2.0))
                    }
                    _ => None,
                })
                .collect()
        })
        .unwrap_or_default()
}

#[test]
fn shown_is_the_line_drawn_before_the_modes() {
    let plain = build_rows(
        &graph(),
        &all_rows(),
        &ChartOptions {
            today: Some(day(TODAY)),
            ..Default::default()
        },
    );
    let shown = build_rows(&graph(), &all_rows(), &options(ProgressLineMode::Shown));
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        serde_json::to_value(&shown).unwrap()
    );
    assert_eq!(
        line(&shown).stroke_dash.as_deref(),
        Some(PROGRESS_LINE_DASH)
    );
}

#[test]
fn hidden_draws_no_line() {
    let list = build_rows(&graph(), &all_rows(), &options(ProgressLineMode::Hidden));
    assert!(line_layer(&list).is_none());
}

#[test]
fn by_row_bends_only_the_rows_late_or_ahead_and_returns_at_each_row_s_bottom() {
    let list = build_rows(&graph(), &all_rows(), &options(ProgressLineMode::ByRow));
    let t = today_x();
    assert_eq!(
        line(&list).points,
        vec![
            (t, top(0)),
            // 遅れ: 進みの 2 日目へ尖り、行の下の境で今日の線へ戻る
            (x(2.0), mid(0)),
            (t, top(1)),
            // 済みの行は今日の線のまま。0% で開始を過ぎた行は開始の 2 日目へ
            (t, top(2)),
            (x(2.0), mid(2)),
            (t, top(3)),
            // まだ始まらぬ行は今日の線のまま。先行は進みの 6 日目へ
            (t, top(4)),
            (x(6.0), mid(4)),
            (t, top(5)),
        ]
    );
}

#[test]
fn by_row_is_a_solid_line_with_a_dot_at_each_bend() {
    let list = build_rows(&graph(), &all_rows(), &options(ProgressLineMode::ByRow));
    let l = line(&list);
    assert_eq!(l.stroke_dash, None);
    assert_eq!(l.stroke_width, BY_ROW_LINE_STROKE_W);
    assert_eq!(l.stroke, ColorId::ProgressLine);
    assert_eq!(
        dots(&list),
        vec![(x(2.0), mid(0)), (x(2.0), mid(2)), (x(6.0), mid(4))]
    );
    // 新しい色の名は既定の表に無く、進捗の線の色に落ちる
    assert_eq!(
        list.palette.resolve(ColorId::ProgressLine),
        list.palette.resolve(ColorId::Progress)
    );
}

#[test]
fn a_row_in_the_done_state_stays_on_the_today_line_whatever_its_progress() {
    let rows = vec![GanttRow::Task(TaskRow {
        state: Some(BarState::Done),
        ..TaskRow::new("late")
    })];
    let list = build_rows(&graph(), &rows, &options(ProgressLineMode::ByRow));
    assert!(line_layer(&list).is_some());
    assert!(line(&list).points.iter().all(|p| p.0 == today_x()));
    assert!(dots(&list).is_empty());
}

#[test]
fn a_group_row_keeps_the_line_on_the_today_line() {
    let rows = vec![
        GanttRow::Group { height: None },
        GanttRow::Task(TaskRow::new("late")),
    ];
    let list = build_rows(&graph(), &rows, &options(ProgressLineMode::ByRow));
    let t = today_x();
    assert_eq!(
        line(&list).points,
        vec![(t, top(0)), (t, top(1)), (x(2.0), mid(1)), (t, top(2))]
    );
}

#[test]
fn by_row_draws_no_line_without_a_today_on_the_chart() {
    for today in [None, Some(day(40)), Some(day(-3))] {
        let opts = ChartOptions {
            today,
            ..options(ProgressLineMode::ByRow)
        };
        let list = build_rows(&graph(), &all_rows(), &opts);
        assert!(line_layer(&list).is_none(), "{today:?}");
    }
}

#[test]
fn by_row_keeps_its_bends_on_the_days_a_view_shows() {
    let opts = ChartOptions {
        view: Some(ViewRange {
            from: day(4),
            to: day(10),
        }),
        ..options(ProgressLineMode::ByRow)
    };
    let list = build_rows(&graph(), &all_rows(), &opts);
    // 範囲の最初の日は 4 日目。遅れの 2 日目は範囲の左の端に留まる
    let left = LABEL_W;
    let t = LABEL_W + (TODAY - 4) as f64 * PX_PER_DAY;
    let pts = line(&list).points;
    assert_eq!(pts[1], (left, mid(0)));
    assert!(pts.iter().all(|p| p.0 >= left));
    assert_eq!(pts[0], (t, top(0)));
}

#[test]
fn a_row_at_100_percent_that_is_not_done_bends_to_its_end() {
    // 閉じ忘れ: 0〜4 日の 100% で state は無い。目安の期限（4 日目の境）へ左に尖る
    let rows = task_rows(&["done"]);
    let list = build_rows(&graph(), &rows, &options(ProgressLineMode::ByRow));
    assert_eq!(dots(&list), vec![(x(4.0), mid(0))]);
}

#[test]
fn a_row_at_0_percent_in_the_done_state_stays_on_the_today_line() {
    // 0% で予定開始を過ぎておっても、done なら尖らぬ
    let rows = vec![done_row("stalled")];
    let list = build_rows(&graph(), &rows, &options(ProgressLineMode::ByRow));
    assert!(line(&list).points.iter().all(|p| p.0 == today_x()));
    assert!(dots(&list).is_empty());
}

#[test]
fn by_row_draws_no_line_for_a_today_just_outside_the_view() {
    // 範囲は 4〜10 日目。翌日（11 日目、x は範囲の右の端）と前日（3 日目）の今日は図に無い
    let with_today = |today: i64| {
        let opts = ChartOptions {
            today: Some(day(today)),
            view: Some(ViewRange {
                from: day(4),
                to: day(10),
            }),
            ..options(ProgressLineMode::ByRow)
        };
        build_rows(&graph(), &all_rows(), &opts)
    };
    for today in [11, 3] {
        assert!(line_layer(&with_today(today)).is_none(), "day {today}");
    }
    // 最後の日と最初の日の今日は図に在り、線を引く
    for today in [10, 4] {
        assert!(line_layer(&with_today(today)).is_some(), "day {today}");
    }
}
