//! 課題の行の印: 状態の塗りと縁、厳守の菱形、点の形と期限への線、添えの字。
//! 設計書 docs/design/bar-marks.md の 3 と 4 にあたる。

use chrono::{Duration, NaiveDate};

use super::build::build_display_list_with_rows;
use super::constants::*;
use super::right_edge_tests::right_edges;
use super::types::*;
use crate::graph::{BarShape, BarState, GanttGraph, GanttRow, GanttTask, TaskRow};

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 11, 2).unwrap()
}

fn day(n: i64) -> NaiveDate {
    epoch() + Duration::days(n)
}

fn task(id: &str, start: i64, end: i64) -> GanttTask {
    GanttTask {
        id: id.to_string(),
        title: id.to_string(),
        progress_pct: 40,
        start: day(start),
        end: Some(day(end)),
    }
}

fn graph() -> GanttGraph {
    GanttGraph {
        tasks: vec![task("a", 0, 4), task("b", 3, 9), task("c", 9, 10)],
        deps: vec![],
    }
}

fn row(id: &str) -> TaskRow {
    TaskRow::new(id)
}

fn build(rows: Vec<TaskRow>) -> DisplayList {
    let rows: Vec<GanttRow> = rows.into_iter().map(GanttRow::Task).collect();
    build_display_list_with_rows(&graph(), Some(&rows), epoch(), &ChartOptions::default())
}

/// `id` の課題の部品の束。
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

fn bar_fill(children: &[Primitive]) -> Option<ColorId> {
    children.iter().find_map(|p| match p {
        Primitive::Rect(r) if r.semantic == RectSemantic::BarBackground => Some(r.fill),
        _ => None,
    })
}

fn outline(children: &[Primitive]) -> Option<ColorId> {
    children.iter().find_map(|p| match p {
        Primitive::Path(p) if p.semantic == PathSemantic::BarOutline => Some(p.stroke),
        _ => None,
    })
}

#[test]
fn a_row_without_marks_draws_the_bar_as_before() {
    let plain = build(vec![row("a"), row("b"), row("c")]);
    let rows = vec![t("a"), t("b"), t("c")];
    fn t(id: &str) -> GanttRow {
        GanttRow::Task(TaskRow::new(id))
    }
    let listed =
        build_display_list_with_rows(&graph(), Some(&rows), epoch(), &ChartOptions::default());
    assert_eq!(
        serde_json::to_value(&plain).unwrap(),
        serde_json::to_value(&listed).unwrap()
    );
    let a = children_of(&plain, "a");
    assert_eq!(bar_fill(a), Some(ColorId::BarBg));
    assert_eq!(outline(a), None);
}

#[test]
fn a_state_sets_the_fill_and_the_outline() {
    let list = build(vec![
        TaskRow {
            state: Some(BarState::Normal),
            ..row("a")
        },
        TaskRow {
            state: Some(BarState::Done),
            ..row("b")
        },
        TaskRow {
            state: Some(BarState::Warn),
            ..row("c")
        },
    ]);
    for (id, fill, stroke) in [
        ("a", ColorId::BarBg, ColorId::BarStroke),
        ("b", ColorId::DoneBg, ColorId::DoneStroke),
        ("c", ColorId::WarnBg, ColorId::WarnStroke),
    ] {
        let c = children_of(&list, id);
        assert_eq!(bar_fill(c), Some(fill), "{id}");
        assert_eq!(outline(c), Some(stroke), "{id}");
    }
    // 新しい色の名は既定の表に無く、落ち先の色で描く
    assert_eq!(
        list.palette.resolve(ColorId::WarnStroke),
        list.palette.resolve(ColorId::Progress)
    );
    assert_eq!(
        list.palette.resolve(ColorId::DoneBg),
        list.palette.resolve(ColorId::BarBg)
    );
}

#[test]
fn a_hard_deadline_is_a_diamond_in_the_middle_of_its_day() {
    let list = build(vec![TaskRow {
        hard_deadline: Some(day(6)),
        ..row("a")
    }]);
    let diamond = children_of(&list, "a")
        .iter()
        .find_map(|p| match p {
            Primitive::Polygon(p) if p.semantic == PolygonSemantic::HardDeadline => Some(p.clone()),
            _ => None,
        })
        .expect("菱形");
    let cx = LABEL_W + 6.0 * PX_PER_DAY + PX_PER_DAY / 2.0;
    assert_eq!(diamond.points[0].0, cx);
    assert_eq!(diamond.points[1].0, cx + HARD_DEADLINE_HALF_PX);
    assert_eq!(diamond.fill, ColorId::HardDeadline);
}

#[test]
fn a_hard_deadline_past_the_last_task_widens_the_chart_to_hold_it() {
    let plain = build(vec![row("a")]);
    let late = build(vec![TaskRow {
        hard_deadline: Some(day(20)),
        ..row("a")
    }]);
    assert!(late.viewport.width > plain.viewport.width);
    let widest = right_edges(&late)
        .into_iter()
        .map(|(_, r)| r)
        .fold(0.0, f64::max);
    assert!(widest <= late.viewport.width);
}

#[test]
fn a_hard_deadline_before_the_first_day_draws_no_diamond() {
    let list = build(vec![TaskRow {
        hard_deadline: Some(day(-3)),
        ..row("a")
    }]);
    assert!(children_of(&list, "a").iter().all(|p| !matches!(
        p,
        Primitive::Polygon(p) if p.semantic == PolygonSemantic::HardDeadline
    )));
}

#[test]
fn a_point_is_a_dot_on_the_last_day_with_a_line_to_its_diamond() {
    let list = build(vec![TaskRow {
        shape: BarShape::Point,
        hard_deadline: Some(day(8)),
        state: Some(BarState::Normal),
        ..row("a")
    }]);
    let c = children_of(&list, "a");
    // 棒は描かぬ
    assert_eq!(bar_fill(c), None);
    let dot = c
        .iter()
        .find_map(|p| match p {
            Primitive::RoundRect(r) if r.semantic == RoundRectSemantic::Point => Some(r.clone()),
            _ => None,
        })
        .expect("点");
    // 期間 0〜4 の最後の日（3 日目）の真ん中
    let cx = LABEL_W + 4.0 * PX_PER_DAY - PX_PER_DAY / 2.0;
    assert_eq!(dot.x + dot.width / 2.0, cx);
    let link = c
        .iter()
        .find_map(|p| match p {
            Primitive::Line(l) if l.semantic == LineSemantic::DeadlineLink => Some(l.clone()),
            _ => None,
        })
        .expect("期限への線");
    assert_eq!(link.x1, cx);
    assert_eq!(link.x2, LABEL_W + 8.0 * PX_PER_DAY + PX_PER_DAY / 2.0);
}

#[test]
fn a_note_that_would_run_over_the_diamond_moves_right_of_it_and_holds_the_width() {
    let note = "目安 11/14 が厳守 11/12 を越えた";
    let list = build(vec![TaskRow {
        state: Some(BarState::Warn),
        hard_deadline: Some(day(10)),
        note: Some(note.to_string()),
        ..row("c")
    }]);
    let text = children_of(&list, "c")
        .iter()
        .find_map(|p| match p {
            Primitive::Text(t) if t.semantic == TextSemantic::BarNote => Some(t.clone()),
            _ => None,
        })
        .expect("添えの字");
    assert_eq!(text.content, note);
    assert_eq!(text.fill, Some(ColorId::WarnStroke));
    let diamond_right = LABEL_W + 10.0 * PX_PER_DAY + PX_PER_DAY / 2.0 + HARD_DEADLINE_HALF_PX;
    assert_eq!(text.x, diamond_right + NOTE_GAP_PX);
    let widest = right_edges(&list)
        .into_iter()
        .map(|(_, r)| r)
        .fold(0.0, f64::max);
    assert!(widest <= list.viewport.width);
}

fn note_x_of(list: &DisplayList, id: &str) -> f64 {
    children_of(list, id)
        .iter()
        .find_map(|p| match p {
            Primitive::Text(t) if t.semantic == TextSemantic::BarNote => Some(t.x),
            _ => None,
        })
        .expect("添えの字")
}

#[test]
fn a_note_sits_right_of_its_bar_when_the_diamond_is_far_past_it() {
    // c は 9〜10 日。菱形は 20 日で、短い字の右の端より遠い
    let list = build(vec![TaskRow {
        hard_deadline: Some(day(20)),
        note: Some("目安を過ぎた".to_string()),
        ..row("c")
    }]);
    assert_eq!(
        note_x_of(&list, "c"),
        LABEL_W + 10.0 * PX_PER_DAY + NOTE_GAP_PX
    );
}

#[test]
fn a_note_sits_right_of_its_bar_when_the_diamond_is_on_the_bar() {
    // b は 3〜9 日。菱形は棒の内の 5 日
    let list = build(vec![TaskRow {
        hard_deadline: Some(day(5)),
        note: Some("目安を過ぎた".to_string()),
        ..row("b")
    }]);
    assert_eq!(
        note_x_of(&list, "b"),
        LABEL_W + 9.0 * PX_PER_DAY + NOTE_GAP_PX
    );
}

#[test]
fn a_note_without_a_diamond_sits_right_of_its_bar() {
    let list = build(vec![TaskRow {
        note: Some("目安を過ぎた".to_string()),
        ..row("b")
    }]);
    assert_eq!(
        note_x_of(&list, "b"),
        LABEL_W + 9.0 * PX_PER_DAY + NOTE_GAP_PX
    );
}

#[test]
fn a_note_of_a_dot_moves_past_the_line_to_its_diamond() {
    // a は点で 3 日目に描き、8 日目の菱形へ線で結ぶ。字は線と菱形を越えた右
    let list = build(vec![TaskRow {
        shape: BarShape::Point,
        hard_deadline: Some(day(8)),
        note: Some("目安を過ぎた".to_string()),
        ..row("a")
    }]);
    assert_eq!(
        note_x_of(&list, "a"),
        LABEL_W + 8.0 * PX_PER_DAY + PX_PER_DAY / 2.0 + HARD_DEADLINE_HALF_PX + NOTE_GAP_PX
    );
}
