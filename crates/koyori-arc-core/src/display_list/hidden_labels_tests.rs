//! 題の列と進みの字を消した時（`row_labels` / `percent_labels` が `Hidden`）の図が、
//! `Shown` の図とどう違うか。設計書 docs/design/labels-opt-out.md の 3 と 4 にあたる。
//!
//! 題の列を消すと、図の全ての x が `LABEL_W` だけ左へ寄り、幅も `LABEL_W` だけ詰まる。
//! この試験は寄る量を数で書かず、`Shown` の図から引いて比べる。

use chrono::{Duration, NaiveDate};

use super::build::build_display_list;
use super::constants::*;
use super::right_edge_tests::right_edges;
use super::types::*;
use crate::graph::{GanttGraph, GanttTask};

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
}

/// 日数 `days` の図。進みのある棒、済んだ短い棒、マイルストーンの三つ。
/// 短い棒の `NN%` は棒に収まらず、棒の右へはみ出す。
fn graph(days: i64) -> GanttGraph {
    let task = |id: &str, pct: i16, start: i64, end: i64| GanttTask {
        id: id.to_string(),
        title: format!("{id} の題"),
        progress_pct: pct,
        start: epoch() + Duration::days(start),
        end: Some(epoch() + Duration::days(end)),
    };
    GanttGraph {
        tasks: vec![
            task("long", 40, 0, days),
            task("short", 100, days - 1, days),
            task("milestone", 100, 1, 1),
        ],
        deps: vec![],
    }
}

fn list(
    days: i64,
    legend: LegendMode,
    row_labels: LabelMode,
    percent_labels: LabelMode,
) -> DisplayList {
    build_display_list(
        &graph(days),
        epoch(),
        &ChartOptions {
            today: Some(epoch() + Duration::days(2)),
            legend,
            row_labels,
            percent_labels,
            ..Default::default()
        },
    )
}

fn texts_of(list: &DisplayList, semantic: TextSemantic) -> Vec<String> {
    fn walk(p: &Primitive, semantic: TextSemantic, out: &mut Vec<String>) {
        match p {
            Primitive::Text(t) if t.semantic == semantic => out.push(t.content.clone()),
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

/// 字を除いた全ての部品の x（左端と、線なら両端、多角形なら全ての頂点）を描いた順に並べる。
fn shape_xs(list: &DisplayList) -> Vec<f64> {
    fn walk(p: &Primitive, out: &mut Vec<f64>) {
        match p {
            Primitive::Rect(r) => out.push(r.x),
            Primitive::RoundRect(r) => out.push(r.x),
            Primitive::Line(l) => out.extend([l.x1, l.x2]),
            Primitive::Polyline(p) => out.extend(p.points.iter().map(|q| q.0)),
            Primitive::Polygon(p) => out.extend(p.points.iter().map(|q| q.0)),
            Primitive::Group(g) => g.children.iter().for_each(|c| walk(c, out)),
            Primitive::Path(_) | Primitive::Text(_) => {}
        }
    }
    let mut out = Vec::new();
    for layer in &list.layers {
        for p in &layer.primitives {
            walk(p, &mut out);
        }
    }
    out
}

/// 字の幅の見積もりで決まる幅は、足し引きの順で最後の桁が揺れる。
fn assert_close(a: f64, b: f64, what: &str) {
    assert!((a - b).abs() < 1e-9, "{what}: {a} != {b}");
}

const LEGENDS: [LegendMode; 2] = [LegendMode::Shown, LegendMode::Hidden];
const LABELS: [LabelMode; 2] = [LabelMode::Shown, LabelMode::Hidden];

#[test]
fn shown_labels_draw_the_same_chart_as_the_default() {
    for days in [1, 3, 14, 40] {
        let default = build_display_list(&graph(days), epoch(), &ChartOptions::default());
        let shown = build_display_list(
            &graph(days),
            epoch(),
            &ChartOptions {
                row_labels: LabelMode::Shown,
                percent_labels: LabelMode::Shown,
                ..Default::default()
            },
        );
        assert_eq!(
            serde_json::to_value(&default).unwrap(),
            serde_json::to_value(&shown).unwrap(),
            "{days} 日"
        );
    }
}

#[test]
fn hidden_row_labels_draw_no_titles() {
    for legend in LEGENDS {
        for percent in LABELS {
            let shown = list(14, legend, LabelMode::Shown, percent);
            let hidden = list(14, legend, LabelMode::Hidden, percent);
            assert_eq!(texts_of(&shown, TextSemantic::RowLabel).len(), 3);
            assert!(texts_of(&hidden, TextSemantic::RowLabel).is_empty());
            // 進みの字は題の列と別に選ぶ
            assert_eq!(
                texts_of(&hidden, TextSemantic::ProgressPercent),
                texts_of(&shown, TextSemantic::ProgressPercent)
            );
        }
    }
}

#[test]
fn hidden_row_labels_move_every_shape_left_by_the_title_column_less_the_left_margin() {
    // 題の列（LABEL_W）を外し、先頭の菱形の左半分の余白（BAR_H / 2）だけを残す
    let shift = LABEL_W - HIDDEN_LABELS_LEFT_PX;
    for days in [1, 3, 14, 40] {
        for legend in LEGENDS {
            for percent in LABELS {
                let shown = list(days, legend, LabelMode::Shown, percent);
                let hidden = list(days, legend, LabelMode::Hidden, percent);
                assert_eq!(hidden.viewport.label_width, HIDDEN_LABELS_LEFT_PX);
                assert_eq!(shown.viewport.label_width, LABEL_W);
                let what = format!("{days} 日 {legend:?} {percent:?}");
                let (a, b) = (shape_xs(&shown), shape_xs(&hidden));
                assert_eq!(a.len(), b.len(), "{what}");
                for (x_shown, x_hidden) in a.iter().zip(&b) {
                    assert_close(x_shown - shift, *x_hidden, &what);
                }
                assert_close(shown.viewport.width - shift, hidden.viewport.width, &what);
                assert_eq!(shown.viewport.height, hidden.viewport.height);
            }
        }
    }
}

#[test]
fn hidden_row_labels_leave_only_the_drawn_shape_to_hit() {
    let shown = list(14, LegendMode::Hidden, LabelMode::Shown, LabelMode::Shown);
    let hidden = list(14, LegendMode::Hidden, LabelMode::Hidden, LabelMode::Shown);
    for t in &shown.metadata.task_bboxes {
        assert!(
            t.bbox.x < t.bar_bbox.x,
            "{}: 題の列の分だけ左へ伸びる",
            t.task_id
        );
    }
    for t in &hidden.metadata.task_bboxes {
        // 棒は棒の範囲。マイルストーンは菱形の左半分（BAR_H / 2）を足した範囲
        let left = if t.task_id == "milestone" {
            t.bar_bbox.x - BAR_H / 2.0
        } else {
            t.bar_bbox.x
        };
        assert_eq!(t.bbox.x, left, "{}", t.task_id);
        assert_eq!(
            t.bbox.x + t.bbox.width,
            t.bar_bbox.x + t.bar_bbox.width,
            "{}",
            t.task_id
        );
    }
}

/// マイルストーンの菱形の x の範囲。
fn diamond_span(list: &DisplayList, id: &str) -> (f64, f64) {
    let points = list
        .layers
        .iter()
        .flat_map(|l| &l.primitives)
        .find_map(|p| match p {
            Primitive::Group(g) if g.task_id.as_deref() == Some(id) => {
                g.children.iter().find_map(|c| match c {
                    Primitive::Polygon(p) if p.semantic == PolygonSemantic::Milestone => {
                        Some(p.points.clone())
                    }
                    _ => None,
                })
            }
            _ => None,
        })
        .expect("菱形");
    let xs = points.iter().map(|p| p.0);
    (
        xs.clone().fold(f64::MAX, f64::min),
        xs.fold(f64::MIN, f64::max),
    )
}

#[test]
fn a_milestone_on_the_first_day_draws_its_whole_diamond_without_the_title_column() {
    let graph = GanttGraph {
        tasks: vec![GanttTask {
            id: "first".to_string(),
            title: "first".to_string(),
            progress_pct: 0,
            start: epoch(),
            end: Some(epoch()),
        }],
        deps: vec![],
    };
    let list = build_display_list(
        &graph,
        epoch(),
        &ChartOptions {
            row_labels: LabelMode::Hidden,
            ..Default::default()
        },
    );
    let (left, right) = diamond_span(&list, "first");
    assert_eq!(left, 0.0, "菱形の左の端が図の左の端に収まる");
    assert!(right <= list.viewport.width);
}

#[test]
fn pressing_the_left_half_of_a_milestone_hits_its_task() {
    for row_labels in [LabelMode::Shown, LabelMode::Hidden] {
        let list = list(14, LegendMode::Hidden, row_labels, LabelMode::Shown);
        let (left, right) = diamond_span(&list, "milestone");
        let centre = (left + right) / 2.0;
        // 菱形の左半分の真ん中を押す
        let x = (left + centre) / 2.0;
        let hit = list
            .metadata
            .task_bboxes
            .iter()
            .find(|t| t.task_id == "milestone")
            .unwrap();
        assert!(
            hit.bbox.x <= x && x <= hit.bbox.x + hit.bbox.width,
            "{row_labels:?}: {x} が当たりの範囲 {} 〜 {} の外",
            hit.bbox.x,
            hit.bbox.x + hit.bbox.width
        );
    }
}

#[test]
fn hidden_percent_labels_draw_no_percent_and_hold_no_width() {
    for days in [1, 3, 14] {
        for legend in LEGENDS {
            for rows in LABELS {
                let shown = list(days, legend, rows, LabelMode::Shown);
                let hidden = list(days, legend, rows, LabelMode::Hidden);
                assert_eq!(texts_of(&shown, TextSemantic::ProgressPercent).len(), 3);
                assert!(texts_of(&hidden, TextSemantic::ProgressPercent).is_empty());
                // 題の字は進みの字と別に選ぶ
                assert_eq!(
                    texts_of(&hidden, TextSemantic::RowLabel),
                    texts_of(&shown, TextSemantic::RowLabel)
                );
                // 進みの字を消した図の幅は、描いた部品のどれよりも狭くならず、
                // 日数の幅、凡例、目盛りの字のどれかで決まる
                let widest = right_edges(&hidden)
                    .into_iter()
                    .map(|(_, r)| r)
                    .fold(0.0, f64::max);
                assert!(
                    widest <= hidden.viewport.width,
                    "{days} 日 {legend:?} {rows:?}"
                );
                assert!(hidden.viewport.width <= shown.viewport.width);
                let label_w = shown.viewport.label_width;
                let days_w = chart_width_with_label(days as f64, label_w);
                assert!(hidden.viewport.width >= days_w);
            }
        }
    }
    // 最後の日に終わる短い棒の字は日数の幅を越えるので、消せば幅が詰まる
    let shown = list(3, LegendMode::Hidden, LabelMode::Hidden, LabelMode::Shown);
    let hidden = list(3, LegendMode::Hidden, LabelMode::Hidden, LabelMode::Hidden);
    assert!(hidden.viewport.width < shown.viewport.width);
}
