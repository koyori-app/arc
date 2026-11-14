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

/// 字の箱の上と下（基線から字の大きさだけ上、字の大きさの四分の一だけ下）。
fn text_span(t: &TextPrim) -> (f64, f64) {
    let size = t.font_size.unwrap_or(11.0);
    (t.y - size, t.y + size / 4.0)
}

#[test]
fn the_day_scale_takes_room_for_its_three_tiers_by_default() {
    // 見出しの高さを渡さぬ日の段。HEADER_H（30）では月の札と日の数字が重なっておった
    let list = list_with(ChartOptions {
        scale: Scale::Day,
        ..Default::default()
    });
    assert_eq!(list.viewport.header_height, DAY_SCALE_HEADER_H);
    let months = texts(&list, TextSemantic::MonthLabel);
    let numbers = texts(&list, TextSemantic::DayNumber);
    assert!(!months.is_empty());
    let month_bottom = months
        .iter()
        .map(|t| text_span(t).1)
        .fold(f64::MIN, f64::max);
    let number_top = numbers
        .iter()
        .map(|t| text_span(t).0)
        .fold(f64::MAX, f64::min);
    assert!(
        month_bottom <= number_top,
        "the month ends at {month_bottom}, the day number starts at {number_top}"
    );
    // 行は見出しの下から始まる
    let a = list
        .metadata
        .task_bboxes
        .iter()
        .find(|t| t.task_id == "a")
        .unwrap();
    assert!(a.bar_bbox.y >= DAY_SCALE_HEADER_H);
}

#[test]
fn a_header_height_the_caller_sets_wins_on_every_scale() {
    for scale in [Scale::Week, Scale::Day, Scale::Month] {
        let list = list_with(ChartOptions {
            scale,
            header_height: Some(60.0),
            ..Default::default()
        });
        assert_eq!(list.viewport.header_height, 60.0, "{scale:?}");
    }
    // 週と月の段の既定は今までの HEADER_H のまま
    for scale in [Scale::Week, Scale::Month] {
        let list = list_with(ChartOptions {
            scale,
            ..Default::default()
        });
        assert_eq!(list.viewport.header_height, HEADER_H, "{scale:?}");
    }
}

#[test]
fn a_day_scale_header_too_low_for_the_month_leaves_the_month_out() {
    let list = list_with(ChartOptions {
        scale: Scale::Day,
        header_height: Some(HEADER_H),
        ..Default::default()
    });
    assert!(texts(&list, TextSemantic::MonthLabel).is_empty());
    assert_eq!(texts(&list, TextSemantic::DayNumber).len(), DAYS as usize);
    assert_eq!(texts(&list, TextSemantic::Weekday).len(), DAYS as usize);
}

/// 2026-11-08 は日曜。
fn sunday() -> NaiveDate {
    epoch() + Duration::days(6)
}

fn milestone_alone(options: ChartOptions) -> DisplayList {
    let graph = GanttGraph {
        tasks: vec![GanttTask {
            id: "m".to_string(),
            title: "m".to_string(),
            progress_pct: 0,
            start: sunday(),
            end: Some(sunday()),
        }],
        deps: vec![],
    };
    build_display_list(&graph, sunday(), &options)
}

fn sunday_mask() -> WeekendDays {
    WeekendDays(1 << Weekday::Sun.num_days_from_monday())
}

#[test]
fn a_milestone_alone_still_gets_its_day_on_every_scale() {
    // 日の段: 日付と曜日と年月
    let day = milestone_alone(ChartOptions {
        scale: Scale::Day,
        ..Default::default()
    });
    let contents = |list: &DisplayList, sem| {
        texts(list, sem)
            .into_iter()
            .map(|t| t.content)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        contents(&day, TextSemantic::DayNumber),
        vec!["8".to_string()]
    );
    assert_eq!(
        contents(&day, TextSemantic::Weekday),
        vec!["Su".to_string()]
    );
    assert_eq!(
        contents(&day, TextSemantic::MonthLabel),
        vec!["2026/11".to_string()]
    );
    // 月の段: 最初の年月
    let month = milestone_alone(ChartOptions {
        scale: Scale::Month,
        ..Default::default()
    });
    assert_eq!(
        contents(&month, TextSemantic::GridLabel),
        vec!["2026/11".to_string()]
    );
    // 日曜を土日に指せば、その日に帯
    let weekend = milestone_alone(ChartOptions {
        weekend: sunday_mask(),
        ..Default::default()
    });
    let bands = rects(&weekend, RectSemantic::Weekend);
    assert_eq!(bands.len(), 1);
    assert_eq!(bands[0].x, LABEL_W);
    // 帯も字も図の幅の中
    for list in [&day, &month, &weekend] {
        assert!(bands[0].x + bands[0].width <= weekend.viewport.width);
        let widest = right_edges(list)
            .into_iter()
            .map(|(_, r)| r)
            .fold(0.0, f64::max);
        assert!(widest <= list.viewport.width);
    }
}

#[test]
fn a_milestone_after_every_bar_gets_its_day() {
    // 棒は 11/2〜11/6（終わりは 11/7 の境）、マイルストーンは 11/8（日曜）
    let graph = GanttGraph {
        tasks: vec![
            task("a", 0, 5),
            GanttTask {
                id: "m".to_string(),
                title: "m".to_string(),
                progress_pct: 0,
                start: sunday(),
                end: Some(sunday()),
            },
        ],
        deps: vec![],
    };
    let list = build_display_list(
        &graph,
        epoch(),
        &ChartOptions {
            scale: Scale::Day,
            weekend: sunday_mask(),
            ..Default::default()
        },
    );
    let numbers: Vec<String> = texts(&list, TextSemantic::DayNumber)
        .into_iter()
        .map(|t| t.content)
        .collect();
    assert_eq!(numbers.last().map(String::as_str), Some("8"));
    assert_eq!(numbers.len(), 7);
    let bands = rects(&list, RectSemantic::Weekend);
    assert_eq!(bands.len(), 1);
    assert_eq!(bands[0].x, LABEL_W + 6.0 * PX_PER_DAY);
    assert!(bands[0].x + bands[0].width <= list.viewport.width);
    let widest = right_edges(&list)
        .into_iter()
        .map(|(_, r)| r)
        .fold(0.0, f64::max);
    assert!(widest <= list.viewport.width);
}
