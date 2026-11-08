//! 目盛りの段（`scale`）、一日の幅（`px_per_day`）、土日の帯（`weekend`）。
//! 設計書 docs/design/day-scale.md の 3 と 4 にあたる。

use chrono::{Datelike, Duration, NaiveDate, Weekday};

use super::build::build_display_list;
use super::constants::*;
use super::right_edge_tests::right_edges;
use super::types::*;
use crate::graph::{GanttGraph, GanttTask};

/// 2026-11-02 は月曜。
fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
}

fn task(id: &str, start: i64, end: i64) -> GanttTask {
    GanttTask {
        id: id.to_string(),
        title: id.to_string(),
        progress_pct: 40,
        start: epoch() + Duration::days(start),
        end: Some(epoch() + Duration::days(end)),
    }
}

/// 2026-11-02 から 12-05 まで（33 日）。月を一つ跨ぐ。
fn graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![task("a", 0, 10), task("b", 5, 33), task("c", 12, 13)],
        deps: vec![],
    }
}

fn texts(list: &DisplayList, semantic: TextSemantic) -> Vec<TextPrim> {
    fn walk(p: &Primitive, semantic: TextSemantic, out: &mut Vec<TextPrim>) {
        match p {
            Primitive::Text(t) if t.semantic == semantic => out.push(t.clone()),
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

fn rects(list: &DisplayList, semantic: RectSemantic) -> Vec<RectPrim> {
    list.layers
        .iter()
        .flat_map(|l| &l.primitives)
        .filter_map(|p| match p {
            Primitive::Rect(r) if r.semantic == semantic => Some(r.clone()),
            _ => None,
        })
        .collect()
}

fn grid_xs(list: &DisplayList) -> Vec<f64> {
    list.layers
        .iter()
        .filter(|l| l.kind == LayerKind::Grid)
        .flat_map(|l| &l.primitives)
        .filter_map(|p| match p {
            Primitive::Line(l) => Some(l.x1),
            _ => None,
        })
        .collect()
}

fn list_with(options: ChartOptions) -> DisplayList {
    build_display_list(&graph(), epoch(), &options)
}

const DAYS: i64 = 33;

#[test]
fn the_week_scale_and_the_standard_day_width_are_the_default_chart() {
    let default = list_with(ChartOptions::default());
    let named = list_with(ChartOptions {
        scale: Scale::Week,
        px_per_day: Some(PX_PER_DAY),
        weekend: WeekendDays(0),
        ..Default::default()
    });
    assert_eq!(
        serde_json::to_value(&default).unwrap(),
        serde_json::to_value(&named).unwrap()
    );
}

#[test]
fn a_day_width_moves_every_day_and_bar() {
    let px = 48.0;
    let list = list_with(ChartOptions {
        px_per_day: Some(px),
        ..Default::default()
    });
    let b = list
        .metadata
        .task_bboxes
        .iter()
        .find(|t| t.task_id == "b")
        .unwrap();
    assert_eq!(b.bar_bbox.x, LABEL_W + 5.0 * px);
    assert_eq!(b.bar_bbox.width, 28.0 * px);
    // 月曜の線は七日ごと
    let xs = grid_xs(&list);
    assert_eq!(xs[0], LABEL_W);
    assert_eq!(xs[1] - xs[0], 7.0 * px);
    assert!(list.viewport.width >= chart_width_with_label(DAYS as f64, LABEL_W, px));
}

#[test]
fn the_day_scale_labels_every_day_with_its_number_and_weekday() {
    let px = 32.0;
    let header = 48.0;
    let list = list_with(ChartOptions {
        scale: Scale::Day,
        px_per_day: Some(px),
        header_height: Some(header),
        ..Default::default()
    });
    let numbers = texts(&list, TextSemantic::DayNumber);
    let weekdays = texts(&list, TextSemantic::Weekday);
    assert_eq!(numbers.len(), DAYS as usize);
    assert_eq!(weekdays.len(), DAYS as usize);
    for (i, (n, w)) in numbers.iter().zip(&weekdays).enumerate() {
        let day = epoch() + Duration::days(i as i64);
        assert_eq!(n.content, day.day().to_string());
        assert_eq!(
            w.content,
            DEFAULT_WEEKDAY_LABELS[day.weekday().num_days_from_monday() as usize]
        );
        // 日の欄の真ん中、見出しの下の方
        assert_eq!(n.x, LABEL_W + i as f64 * px + px / 2.0);
        assert_eq!(n.anchor, Some(TextAnchor::Middle));
        assert!(n.y < w.y && w.y < header);
    }
    // 月の札は最初の日と、月の頭
    let months: Vec<String> = texts(&list, TextSemantic::MonthLabel)
        .into_iter()
        .map(|t| t.content)
        .collect();
    assert_eq!(months, vec!["2026/11".to_string(), "2026/12".to_string()]);
    // 週の頭の札は出さず、月曜の線は残す
    assert!(texts(&list, TextSemantic::GridLabel).is_empty());
    assert_eq!(grid_xs(&list).len(), 5);
    // 札はどれも図の外へ出ぬ
    let widest = right_edges(&list)
        .into_iter()
        .map(|(_, r)| r)
        .fold(0.0, f64::max);
    assert!(widest <= list.viewport.width);
}

#[test]
fn the_day_scale_takes_the_callers_weekday_labels() {
    let labels = ["月", "火", "水", "木", "金", "土", "日"].map(String::from);
    let list = list_with(ChartOptions {
        scale: Scale::Day,
        weekday_labels: Some(labels.clone()),
        ..Default::default()
    });
    let weekdays: Vec<String> = texts(&list, TextSemantic::Weekday)
        .into_iter()
        .map(|t| t.content)
        .take(7)
        .collect();
    assert_eq!(weekdays, labels.to_vec());
}

#[test]
fn the_month_scale_marks_the_first_of_each_month() {
    let list = list_with(ChartOptions {
        scale: Scale::Month,
        ..Default::default()
    });
    let labels: Vec<String> = texts(&list, TextSemantic::GridLabel)
        .into_iter()
        .map(|t| t.content)
        .collect();
    assert_eq!(labels, vec!["2026/11".to_string(), "2026/12".to_string()]);
    let dec_1 = NaiveDate::from_ymd_opt(2026, 12, 1).unwrap();
    assert_eq!(
        grid_xs(&list),
        vec![
            LABEL_W,
            LABEL_W + (dec_1 - epoch()).num_days() as f64 * PX_PER_DAY
        ]
    );
}

#[test]
fn weekend_days_draw_a_band_over_the_rows() {
    let none = list_with(ChartOptions::default());
    assert!(rects(&none, RectSemantic::Weekend).is_empty());

    let px = 32.0;
    let mut mask = 0u8;
    for d in [Weekday::Sat, Weekday::Sun] {
        mask |= 1 << d.num_days_from_monday();
    }
    let list = list_with(ChartOptions {
        px_per_day: Some(px),
        weekend: WeekendDays(mask),
        ..Default::default()
    });
    let bands = rects(&list, RectSemantic::Weekend);
    let expected: Vec<f64> = (0..DAYS)
        .filter(|i| {
            matches!(
                (epoch() + Duration::days(*i)).weekday(),
                Weekday::Sat | Weekday::Sun
            )
        })
        .map(|i| LABEL_W + i as f64 * px)
        .collect();
    assert_eq!(bands.iter().map(|b| b.x).collect::<Vec<_>>(), expected);
    for b in &bands {
        assert_eq!(b.width, px);
        assert_eq!(b.y, HEADER_H);
        assert_eq!(b.height, 3.0 * ROW_H);
        assert_eq!(b.fill, ColorId::Weekend);
    }
    // 土日の色は既定の表に無く、見出しの帯の色に落ちる
    assert_eq!(
        list.palette.resolve(ColorId::Weekend),
        list.palette.resolve(ColorId::HeaderBg)
    );
}
