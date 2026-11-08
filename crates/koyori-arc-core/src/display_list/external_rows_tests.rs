//! 使う側が行を並べた図（`build_display_list_with_rows`）。
//! 設計書 docs/design/external-rows.md の 3 と 4 にあたる。
//!
//! 寸法は数で書かず、渡した寸法と定数から求めて比べる。

use chrono::{Duration, NaiveDate};

use super::build::{build_display_list, build_display_list_with_rows};
use super::constants::*;
use super::types::*;
use crate::graph::{GanttDep, GanttGraph, GanttRow, GanttTask};

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
}

fn task(id: &str, pct: i16, start: i64, end: i64) -> GanttTask {
    GanttTask {
        id: id.to_string(),
        title: id.to_string(),
        progress_pct: pct,
        start: epoch() + Duration::days(start),
        end: Some(epoch() + Duration::days(end)),
    }
}

fn graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![
            task("a", 40, 0, 4),
            task("b", 0, 3, 9),
            task("c", 100, 1, 2),
            task("d", 60, 6, 12),
        ],
        deps: vec![],
    }
}

fn t(id: &str) -> GanttRow {
    GanttRow::Task {
        task_id: id.to_string(),
        height: None,
    }
}

fn group() -> GanttRow {
    GanttRow::Group { height: None }
}

fn today_options() -> ChartOptions {
    ChartOptions {
        today: Some(epoch() + Duration::days(6)),
        ..Default::default()
    }
}

fn rects(list: &DisplayList, semantic: RectSemantic) -> Vec<RectPrim> {
    fn walk(p: &Primitive, semantic: RectSemantic, out: &mut Vec<RectPrim>) {
        match p {
            Primitive::Rect(r) if r.semantic == semantic => out.push(r.clone()),
            Primitive::Group(g) => g.children.iter().for_each(|c| walk(c, semantic, out)),
            _ => {}
        }
    }
    let mut out = Vec::new();
    for layer in &list.layers {
        for p in &layer.primitives {
            walk(p, semantic, &mut out);
        }
    }
    out
}

fn bar_of<'a>(list: &'a DisplayList, id: &str) -> &'a TaskBBox {
    list.metadata
        .task_bboxes
        .iter()
        .find(|b| b.task_id == id)
        .unwrap_or_else(|| panic!("{id} が描かれておらぬ"))
}

fn progress_points(list: &DisplayList) -> Vec<(f64, f64)> {
    list.layers
        .iter()
        .filter(|l| l.kind == LayerKind::ProgressLine)
        .flat_map(|l| &l.primitives)
        .filter_map(|p| match p {
            Primitive::Polyline(p) => Some(p.points.clone()),
            _ => None,
        })
        .flatten()
        .collect()
}

#[test]
fn listing_the_tasks_in_their_order_draws_the_default_chart() {
    let g = graph();
    for options in [ChartOptions::default(), today_options()] {
        let default = build_display_list(&g, epoch(), &options);
        let listed = build_display_list_with_rows(
            &g,
            Some(&[t("a"), t("b"), t("c"), t("d")]),
            epoch(),
            &options,
        );
        assert_eq!(
            serde_json::to_value(&default).unwrap(),
            serde_json::to_value(&listed).unwrap()
        );
    }
}

#[test]
fn rows_follow_the_list_and_a_group_row_draws_a_band() {
    let g = graph();
    let rows = [group(), t("d"), t("a"), group(), t("c")];
    let list = build_display_list_with_rows(&g, Some(&rows), epoch(), &today_options());

    // 行は一覧の順に、上から ROW_H ずつ
    let row_top = |i: usize| HEADER_H + i as f64 * ROW_H;
    assert_eq!(bar_of(&list, "d").row, 1);
    assert_eq!(bar_of(&list, "a").row, 2);
    assert_eq!(bar_of(&list, "c").row, 4);
    assert_eq!(
        bar_of(&list, "d").bar_bbox.y,
        row_top(1) + (ROW_H - BAR_H) / 2.0
    );

    // 群れの行は帯だけで、日の幅いっぱい
    let bands = rects(&list, RectSemantic::GroupRow);
    assert_eq!(bands.len(), 2);
    assert_eq!((bands[0].y, bands[0].height), (row_top(0), ROW_H));
    assert_eq!((bands[1].y, bands[1].height), (row_top(3), ROW_H));
    assert_eq!(bands[0].x, LABEL_W);
    assert_eq!(bands[0].x + bands[0].width, list.viewport.width);

    // 一覧に無い課題は描かぬ
    assert!(list.metadata.task_bboxes.iter().all(|b| b.task_id != "b"));

    // 高さは見出しと五つの行と、凡例と下の余白
    assert_eq!(
        list.viewport.height,
        HEADER_H + 5.0 * ROW_H + LEGEND_H + CHART_BOTTOM_PADDING_PX
    );
}

#[test]
fn the_progress_line_runs_over_the_task_rows_top_to_bottom() {
    let g = graph();
    let rows = [group(), t("d"), t("a"), group(), t("c")];
    let list = build_display_list_with_rows(&g, Some(&rows), epoch(), &today_options());
    let pts = progress_points(&list);
    // 今日の上の端、三つの課題の点、今日の下の端。群れの行は点を足さぬ
    assert_eq!(pts.len(), 5);
    let mid = |i: usize| HEADER_H + i as f64 * ROW_H + ROW_H / 2.0;
    assert_eq!(pts[1].1, mid(1));
    assert_eq!(pts[2].1, mid(2));
    assert_eq!(pts[3].1, mid(4));
    assert_eq!(pts[0].1, HEADER_H + ROW_H);
    assert_eq!(pts[4].1, HEADER_H + 5.0 * ROW_H);
}

#[test]
fn sizes_set_the_header_rows_and_bars() {
    let g = graph();
    let (header, row, bar) = (48.0, 32.0, 12.0);
    let options = ChartOptions {
        header_height: Some(header),
        row_height: Some(row),
        bar_height: Some(bar),
        ..today_options()
    };
    let rows = [
        group(),
        t("a"),
        GanttRow::Task {
            task_id: "c".to_string(),
            height: Some(64.0),
        },
    ];
    let list = build_display_list_with_rows(&g, Some(&rows), epoch(), &options);
    assert_eq!(list.viewport.header_height, header);
    assert_eq!(list.viewport.row_height, row);
    assert_eq!(
        rects(&list, RectSemantic::HeaderBackground)[0].height,
        header
    );
    let a = bar_of(&list, "a");
    assert_eq!(a.bar_bbox.height, bar);
    assert_eq!(a.bar_bbox.y, header + row + (row - bar) / 2.0);
    // 行ごとの高さ。真ん中の棒はその行の真ん中に
    let c = bar_of(&list, "c");
    assert_eq!(c.bar_bbox.y, header + 2.0 * row + (64.0 - bar) / 2.0);
    assert_eq!(
        list.viewport.height,
        header + 2.0 * row + 64.0 + LEGEND_H + CHART_BOTTOM_PADDING_PX
    );
}

#[test]
fn a_bottom_bar_keeps_the_standard_gap_under_it() {
    let g = graph();
    let (row, bar, tall) = (40.0, 12.0, 64.0);
    let options = ChartOptions {
        row_height: Some(row),
        bar_height: Some(bar),
        bar_align: BarAlign::Bottom,
        ..Default::default()
    };
    let rows = [
        t("a"),
        GanttRow::Task {
            task_id: "b".to_string(),
            height: Some(tall),
        },
    ];
    let list = build_display_list_with_rows(&g, Some(&rows), epoch(), &options);
    let gap = (row - bar) / 2.0;
    // 標準の高さの行では、下寄せも真ん中と同じ所
    assert_eq!(bar_of(&list, "a").bar_bbox.y, HEADER_H + gap);
    // 高い行では、下の隙を標準の行と同じにして下へ寄る
    let b = bar_of(&list, "b");
    assert_eq!(b.bar_bbox.y + bar, HEADER_H + row + tall - gap);
}

#[test]
fn a_dependency_on_a_task_left_out_draws_no_arrow() {
    let mut g = graph();
    g.deps = vec![
        GanttDep {
            blocker_task_id: "a".to_string(),
            blocked_task_id: "d".to_string(),
        },
        GanttDep {
            blocker_task_id: "a".to_string(),
            blocked_task_id: "b".to_string(),
        },
    ];
    let list = build_display_list_with_rows(&g, Some(&[t("a"), t("d")]), epoch(), &today_options());
    let arrows: usize = list
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Dependencies)
        .map(|l| l.primitives.len())
        .sum();
    assert_eq!(arrows, 1);
}

#[test]
fn the_scroll_window_follows_rows_of_any_height() {
    let g = GanttGraph {
        tasks: (0..30).map(|i| task(&format!("t{i}"), 0, 0, 3)).collect(),
        deps: vec![],
    };
    // 行の高さを交互に 40 と 80 にする
    let rows: Vec<GanttRow> = (0..30)
        .map(|i| GanttRow::Task {
            task_id: format!("t{i}"),
            height: Some(if i % 2 == 0 { 40.0 } else { 80.0 }),
        })
        .collect();
    let options = ChartOptions {
        scroll_viewport: Some(ScrollViewport {
            scroll_y: 600.0,
            client_height: 300.0,
        }),
        ..Default::default()
    };
    let list = build_display_list_with_rows(&g, Some(&rows), epoch(), &options);
    let drawn: Vec<u32> = list.metadata.task_bboxes.iter().map(|b| b.row).collect();
    let top_of = |i: usize| {
        HEADER_H
            + (0..i)
                .map(|k| if k % 2 == 0 { 40.0 } else { 80.0 })
                .sum::<f64>()
    };
    let bottom_of = |i: usize| top_of(i + 1);
    let visible: Vec<u32> = (0..30)
        .filter(|&i| bottom_of(i) > 600.0 && top_of(i) < 900.0)
        .map(|i| i as u32)
        .collect();
    // 見える行は全て描き、前後に ROW_BUFFER の行を足す
    for v in &visible {
        assert!(drawn.contains(v), "row {v}");
    }
    let first = *visible.first().unwrap() as usize;
    let last = *visible.last().unwrap() as usize;
    let expected: Vec<u32> = (first.saturating_sub(ROW_BUFFER as usize)
        ..=(last + ROW_BUFFER as usize).min(29))
        .map(|i| i as u32)
        .collect();
    let mut sorted = drawn.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, expected);
}
