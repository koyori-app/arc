//! 表示の範囲: 最初の日と日数、範囲で切る棒、範囲の外へ続く印、範囲の外の課題。
//! 設計書 docs/design/view-range.md の 3 と 4 にあたる。

use chrono::NaiveDate;

use super::build::{build_display_list, build_display_list_with_rows};
use super::constants::*;
use super::right_edge_tests::right_edges;
use super::types::*;
use crate::graph::{BarState, GanttDep, GanttGraph, GanttRow, GanttTask, TaskRow};

fn d(m: u32, day: u32) -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, m, day).unwrap()
}

fn task(id: &str, start: NaiveDate, end: NaiveDate, pct: i16) -> GanttTask {
    GanttTask {
        id: id.to_string(),
        title: id.to_string(),
        progress_pct: pct,
        start,
        end: Some(end),
    }
}

/// 11/9（月）から 11/15（日）までの七日。
fn view() -> ViewRange {
    ViewRange {
        from: d(11, 9),
        to: d(11, 15),
    }
}

fn options(view: Option<ViewRange>) -> ChartOptions {
    ChartOptions {
        legend: LegendMode::Hidden,
        view,
        ..Default::default()
    }
}

fn graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![
            // 範囲の前から後ろへ続く。最後の日は 11/19
            task("cut", d(11, 5), d(11, 20), 50),
            task("inside", d(11, 10), d(11, 12), 40),
            // 範囲より後に始まる
            task("later", d(11, 18), d(11, 21), 0),
            // 範囲より前に終わる。最後の日は 11/3
            task("earlier", d(11, 1), d(11, 4), 100),
        ],
        deps: vec![],
    }
}

fn build(graph: &GanttGraph, view: Option<ViewRange>) -> DisplayList {
    let epoch = graph.tasks.iter().map(|t| t.start).min().unwrap();
    build_display_list(graph, epoch, &options(view))
}

fn children_of<'a>(list: &'a DisplayList, id: &str) -> &'a [Primitive] {
    list.layers
        .iter()
        .filter(|l| l.kind == LayerKind::Bars)
        .flat_map(|l| &l.primitives)
        .find_map(|p| match p {
            Primitive::Group(g) if g.task_id.as_deref() == Some(id) => Some(g.children.as_slice()),
            _ => None,
        })
        .unwrap_or_else(|| panic!("{id} が描かれておらぬ"))
}

fn texts(children: &[Primitive], semantic: TextSemantic) -> Vec<TextPrim> {
    children
        .iter()
        .filter_map(|p| match p {
            Primitive::Text(t) if t.semantic == semantic => Some(t.clone()),
            _ => None,
        })
        .collect()
}

fn bar(children: &[Primitive]) -> Option<RectPrim> {
    children.iter().find_map(|p| match p {
        Primitive::Rect(r) if r.semantic == RectSemantic::BarBackground => Some(r.clone()),
        _ => None,
    })
}

/// 範囲の右の端の x。
fn days_right() -> f64 {
    LABEL_W + 7.0 * PX_PER_DAY
}

#[test]
fn a_view_starts_the_days_at_its_first_day_and_spans_its_days() {
    let list = build(&graph(), Some(view()));
    assert_eq!(list.viewport.width, chart_width(7.0));
    let inside = bar(children_of(&list, "inside")).unwrap();
    // 11/10 は範囲の二日目
    assert_eq!(inside.x, LABEL_W + PX_PER_DAY);
    assert_eq!(inside.width, 2.0 * PX_PER_DAY);
    // 範囲の内の棒は、範囲の無い図と同じく進みの字を持つ
    assert_eq!(
        texts(children_of(&list, "inside"), TextSemantic::ProgressPercent).len(),
        1
    );
}

#[test]
fn a_bar_running_past_both_ends_is_cut_and_marked_inside_each_edge() {
    let list = build(&graph(), Some(view()));
    let c = children_of(&list, "cut");
    let b = bar(c).unwrap();
    assert_eq!(b.x, LABEL_W);
    assert_eq!(b.width, 7.0 * PX_PER_DAY);
    let marks = texts(c, TextSemantic::Overflow);
    let contents: Vec<&str> = marks.iter().map(|t| t.content.as_str()).collect();
    assert_eq!(contents, ["← 11/5", "→ 11/19"]);
    assert_eq!(marks[0].x, LABEL_W + OVERFLOW_INSET_PX);
    assert_eq!(marks[0].anchor, Some(TextAnchor::Start));
    assert_eq!(marks[1].x, days_right() - OVERFLOW_INSET_PX);
    assert_eq!(marks[1].anchor, Some(TextAnchor::End));
    // 切った棒は進みの字を描かぬ
    assert!(texts(c, TextSemantic::ProgressPercent).is_empty());
}

#[test]
fn the_progress_of_a_cut_bar_is_the_whole_bar_s_cut_at_the_view() {
    let list = build(&graph(), Some(view()));
    let progress = children_of(&list, "cut")
        .iter()
        .find_map(|p| match p {
            Primitive::RoundRect(r) if r.semantic == RoundRectSemantic::BarProgress => {
                Some(r.clone())
            }
            _ => None,
        })
        .unwrap();
    // 11/5 からの十五日の半ば（11/12 の半ば）。範囲の最初の日から 3.5 日
    assert_eq!(progress.x, LABEL_W);
    assert_eq!(progress.width, 3.5 * PX_PER_DAY);
}

#[test]
fn a_task_wholly_outside_is_its_mark_alone_at_the_edge_it_lies_past() {
    let list = build(&graph(), Some(view()));
    for (id, content, x) in [
        ("later", "→ 11/18", days_right() - OVERFLOW_INSET_PX),
        ("earlier", "← 11/3", LABEL_W + OVERFLOW_INSET_PX),
    ] {
        let c = children_of(&list, id);
        assert_eq!(c.len(), 1, "{id}");
        let mark = &texts(c, TextSemantic::Overflow)[0];
        assert_eq!(mark.content, content);
        assert_eq!(mark.x, x);
    }
    // 当たりの区画は印の上に置く
    let later = list
        .metadata
        .task_bboxes
        .iter()
        .find(|b| b.task_id == "later")
        .unwrap();
    assert!(later.bbox.x < days_right() && later.bbox.x + later.bbox.width <= days_right());
}

#[test]
fn nothing_but_marks_and_notes_runs_past_the_view() {
    let list = build(&graph(), Some(view()));
    // 見出しの地は図の右の余白まで引く
    for (name, right) in right_edges(&list)
        .into_iter()
        .filter(|(name, _)| name != "HeaderBackground")
    {
        assert!(right <= days_right() + 0.001, "{name} が {right} まで出る");
    }
}

#[test]
fn the_today_line_is_drawn_only_on_the_days_the_view_shows() {
    let today_layer = |today: NaiveDate| {
        let g = graph();
        let opts = ChartOptions {
            today: Some(today),
            ..options(Some(view()))
        };
        build_display_list(&g, d(11, 1), &opts)
            .layers
            .iter()
            .any(|l| l.kind == LayerKind::TodayMarker)
    };
    assert!(today_layer(d(11, 12)));
    assert!(!today_layer(d(11, 18)));
    assert!(!today_layer(d(11, 2)));
}

#[test]
fn the_progress_line_stays_on_the_days_the_view_shows() {
    let opts = ChartOptions {
        today: Some(d(11, 12)),
        ..options(Some(view()))
    };
    let list = build_display_list(&graph(), d(11, 1), &opts);
    let line = list
        .layers
        .iter()
        .find(|l| l.kind == LayerKind::ProgressLine)
        .and_then(|l| match &l.primitives[0] {
            Primitive::Polyline(p) => Some(p.clone()),
            _ => None,
        })
        .expect("進捗の線");
    for (x, _) in line.points {
        assert!((LABEL_W..=days_right()).contains(&x), "{x}");
    }
}

#[test]
fn an_arrow_whose_end_the_view_does_not_show_is_left_out() {
    let mut g = graph();
    g.deps = vec![
        // 範囲より前の課題から
        GanttDep {
            blocker_task_id: "earlier".to_string(),
            blocked_task_id: "inside".to_string(),
        },
    ];
    let arrows = |list: &DisplayList| {
        list.layers
            .iter()
            .filter(|l| l.kind == LayerKind::Dependencies)
            .map(|l| l.primitives.len())
            .sum::<usize>()
    };
    assert_eq!(arrows(&build(&g, None)), 1);
    assert_eq!(arrows(&build(&g, Some(view()))), 0);
}

#[test]
fn a_deadline_outside_the_view_draws_no_diamond_and_a_note_follows_the_cut_bar() {
    let g = graph();
    let rows = vec![GanttRow::Task(TaskRow {
        hard_deadline: Some(d(11, 18)),
        note: Some("厳守 11/18".to_string()),
        ..TaskRow::new("cut")
    })];
    let list = build_display_list_with_rows(&g, Some(&rows), d(11, 1), &options(Some(view())));
    let c = children_of(&list, "cut");
    assert!(c.iter().all(|p| !matches!(
        p,
        Primitive::Polygon(p) if p.semantic == PolygonSemantic::HardDeadline
    )));
    let note = &texts(c, TextSemantic::BarNote)[0];
    assert_eq!(note.x, days_right() + NOTE_GAP_PX);
    // 字の右端まで図が広がる
    let widest = right_edges(&list)
        .into_iter()
        .map(|(_, r)| r)
        .fold(0.0, f64::max);
    assert!(widest <= list.viewport.width);
}

#[test]
fn the_weekend_bands_are_the_view_s_weekend_days() {
    let opts = ChartOptions {
        weekend: WeekendDays(0b110_0000),
        ..options(Some(view()))
    };
    let list = build_display_list(&graph(), d(11, 1), &opts);
    let bands: Vec<f64> = list
        .layers
        .iter()
        .flat_map(|l| &l.primitives)
        .filter_map(|p| match p {
            Primitive::Rect(r) if r.semantic == RectSemantic::Weekend => Some(r.x),
            _ => None,
        })
        .collect();
    // 11/14（土）と 11/15（日）
    assert_eq!(
        bands,
        [LABEL_W + 5.0 * PX_PER_DAY, LABEL_W + 6.0 * PX_PER_DAY]
    );
}

#[test]
fn a_bar_ending_on_the_view_s_last_day_is_not_cut() {
    let g = GanttGraph {
        tasks: vec![task("edge", d(11, 12), d(11, 16), 30)],
        deps: vec![],
    };
    let list = build(&g, Some(view()));
    let c = children_of(&list, "edge");
    assert!(texts(c, TextSemantic::Overflow).is_empty());
    assert_eq!(texts(c, TextSemantic::ProgressPercent).len(), 1);
    assert_eq!(bar(c).unwrap().x + bar(c).unwrap().width, days_right());
}

#[test]
fn a_bar_ending_the_day_after_the_view_s_last_day_is_cut_and_marked() {
    // 最後の日は 11/16、範囲の最後の日（11/15）の翌日
    let g = GanttGraph {
        tasks: vec![task("over", d(11, 12), d(11, 17), 30)],
        deps: vec![],
    };
    let list = build(&g, Some(view()));
    let c = children_of(&list, "over");
    let marks = texts(c, TextSemantic::Overflow);
    let contents: Vec<&str> = marks.iter().map(|t| t.content.as_str()).collect();
    assert_eq!(contents, ["→ 11/16"]);
    assert!(texts(c, TextSemantic::ProgressPercent).is_empty());
    assert_eq!(bar(c).unwrap().x + bar(c).unwrap().width, days_right());
}

#[test]
fn a_task_before_the_view_keeps_its_diamond_and_its_note_past_the_mark() {
    // 11/1〜11/3 の課題。目安を過ぎた（warn）行で、厳守の期限 11/12 は範囲の内
    let rows = vec![GanttRow::Task(TaskRow {
        state: Some(BarState::Warn),
        hard_deadline: Some(d(11, 12)),
        note: Some("目安 11/3 を過ぎた".to_string()),
        ..TaskRow::new("earlier")
    })];
    let list =
        build_display_list_with_rows(&graph(), Some(&rows), d(11, 1), &options(Some(view())));
    let c = children_of(&list, "earlier");
    let mark = &texts(c, TextSemantic::Overflow)[0];
    assert_eq!(mark.content, "← 11/3");
    assert_eq!(mark.fill, Some(ColorId::WarnStroke));
    let diamond = c
        .iter()
        .find_map(|p| match p {
            Primitive::Polygon(p) if p.semantic == PolygonSemantic::HardDeadline => Some(p.clone()),
            _ => None,
        })
        .expect("範囲の内の厳守の菱形");
    // 11/12 は範囲の 4 日目
    assert_eq!(diamond.points[0].0, LABEL_W + 3.5 * PX_PER_DAY);
    let note = &texts(c, TextSemantic::BarNote)[0];
    assert_eq!(note.fill, Some(ColorId::WarnStroke));
    // 字は印と菱形の右
    assert_eq!(
        note.x,
        LABEL_W + 3.5 * PX_PER_DAY + HARD_DEADLINE_HALF_PX + NOTE_GAP_PX
    );
    let widest = right_edges(&list)
        .into_iter()
        .map(|(_, r)| r)
        .fold(0.0, f64::max);
    assert!(widest <= list.viewport.width);
}

#[test]
fn a_task_after_the_view_draws_no_note() {
    let rows = vec![GanttRow::Task(TaskRow {
        note: Some("添え".to_string()),
        ..TaskRow::new("later")
    })];
    let list =
        build_display_list_with_rows(&graph(), Some(&rows), d(11, 1), &options(Some(view())));
    assert!(texts(children_of(&list, "later"), TextSemantic::BarNote).is_empty());
}

#[test]
fn a_long_note_before_the_view_widens_the_chart_to_hold_it() {
    let note = "目安を過ぎた行の、図の幅を越える長さの添えの字".repeat(3);
    let rows = vec![GanttRow::Task(TaskRow {
        note: Some(note),
        ..TaskRow::new("earlier")
    })];
    let list =
        build_display_list_with_rows(&graph(), Some(&rows), d(11, 1), &options(Some(view())));
    assert!(list.viewport.width > chart_width(7.0));
    let widest = right_edges(&list)
        .into_iter()
        .map(|(_, r)| r)
        .fold(0.0, f64::max);
    assert!(widest <= list.viewport.width);
}

#[test]
fn a_group_band_stops_at_the_view_s_last_day() {
    let rows = vec![
        GanttRow::Group { height: None },
        GanttRow::Task(TaskRow::new("inside")),
    ];
    let list =
        build_display_list_with_rows(&graph(), Some(&rows), d(11, 1), &options(Some(view())));
    let band = list
        .layers
        .iter()
        .flat_map(|l| &l.primitives)
        .find_map(|p| match p {
            Primitive::Rect(r) if r.semantic == RectSemantic::GroupRow => Some(r.clone()),
            _ => None,
        })
        .expect("群れの帯");
    assert_eq!(band.x + band.width, days_right());
    // 図の右の余白（字の延びる場所）は帯の外
    assert!(list.viewport.width > days_right());
}

#[test]
fn a_note_before_the_view_sits_right_of_its_mark_when_the_diamond_is_far() {
    // 範囲の最後の日（11/15）の菱形は、印の右の短い字より遠い
    let rows = vec![GanttRow::Task(TaskRow {
        state: Some(BarState::Warn),
        hard_deadline: Some(d(11, 15)),
        note: Some("過ぎた".to_string()),
        ..TaskRow::new("earlier")
    })];
    let list =
        build_display_list_with_rows(&graph(), Some(&rows), d(11, 1), &options(Some(view())));
    let c = children_of(&list, "earlier");
    let mark = &texts(c, TextSemantic::Overflow)[0];
    let note = &texts(c, TextSemantic::BarNote)[0];
    let diamond_left = LABEL_W + 6.5 * PX_PER_DAY - HARD_DEADLINE_HALF_PX;
    assert!(note.x > mark.x, "字は印の右");
    assert!(note.x < diamond_left, "字は菱形の左（菱形の右へ逃がさぬ）");
}
