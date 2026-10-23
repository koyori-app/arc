//! 凡例を消した時（`LegendMode::Hidden`）の図が、`Shown` の図とどう違うか。
//! 設計書 docs/design/legend-opt-out.md の 4.3、4.6、4.7、8.4 にあたる。
//!
//! 幅の下支えが効くかどうかの境は、数で書かない。
//! 下支えの値は見出しの文言や定数が変われば動くので、数を直に書けば、文言が一字変わった日に
//! 黙って嘘を縛る。ここでは定数から下支えの値を求め、「日数で決まる幅が下支えを下回るか」から
//! 境の日数を導き、その前後を撃つ。

use chrono::{Duration, NaiveDate};
use serde_json::Value;

use super::build::build_display_list;
use super::constants::*;
use super::legend::{PROGRESS_LABEL, TIER_ITEMS};
use super::right_edge_tests::{build as build_spans, charts, right_edges};
use super::text_metrics::text_width_px;
use super::types::*;
use crate::backend::{BackendOutput, CanvasBackend, RenderBackend, SvgBackend};
use crate::graph::{GanttGraph, GanttTask};

fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
}

/// 日数 `days` の図。タスクは二つで、どちらも終わりが `days` 日目。
fn graph(days: i64) -> GanttGraph {
    let task = |id: &str, pct: i16| GanttTask {
        id: id.to_string(),
        title: id.to_string(),
        progress_pct: pct,
        start: epoch(),
        end: Some(epoch() + Duration::days(days)),
    };
    GanttGraph {
        tasks: vec![task("a", 40), task("b", 80)],
        deps: vec![],
    }
}

fn options(legend: LegendMode, today_day: Option<i64>) -> ChartOptions {
    ChartOptions {
        today: today_day.map(|d| epoch() + Duration::days(d)),
        legend,
        ..Default::default()
    }
}

fn list(days: i64, legend: LegendMode, today_day: Option<i64>) -> DisplayList {
    build_display_list(&graph(days), epoch(), &options(legend, today_day))
}

fn svg(days: i64, legend: LegendMode, today_day: Option<i64>) -> String {
    match SvgBackend.render(&list(days, legend, today_day)) {
        BackendOutput::Svg(s) => s,
        _ => unreachable!(),
    }
}

fn canvas(days: i64, legend: LegendMode, today_day: Option<i64>) -> Value {
    match CanvasBackend.render(&list(days, legend, today_day)) {
        BackendOutput::CanvasCommands(b) => serde_json::to_value(&b).unwrap(),
        _ => unreachable!(),
    }
}

fn ir(days: i64, legend: LegendMode, today_day: Option<i64>) -> Value {
    serde_json::to_value(list(days, legend, today_day)).unwrap()
}

/// 凡例の無い図の幅。日数で決まる幅か、右へ伸びる札の右端の遠いほう。
/// 札の右端は `right_edge_tests` の走査で、描いた部品から求める。
/// 一日の図やマイルストーンの図では、札が日数の幅を越える（design 4.7 の但し書き）。
fn hidden_width(days: i64) -> f64 {
    let l = list(days, LegendMode::Hidden, None);
    let widest_part = right_edges(&l)
        .into_iter()
        .map(|(_, right)| right)
        .fold(0.0, f64::max);
    chart_width(days as f64).max(widest_part)
}

// --- 境を規則から導く ---

/// 凡例の二つの行の幅から求めた、図の幅の下支えの値。
/// `build.rs` の `legend_layout` と同じ式を、定数から別に書き下したもの。
fn floor_px() -> f64 {
    let progress = LEGEND_LINE_SAMPLE_W_PX
        + LEGEND_LINE_TEXT_GAP_PX
        + text_width_px(PROGRESS_LABEL, LEGEND_PROGRESS_FONT_PX);
    let tiers = TIER_ITEMS
        .iter()
        .map(|(_, label)| {
            LEGEND_SWATCH_PX
                + LEGEND_SWATCH_GAP_PX
                + text_width_px(label, LEGEND_TIER_FONT_PX)
                + LEGEND_ITEM_GAP_PX
        })
        .sum::<f64>()
        - LEGEND_ITEM_GAP_PX;
    LABEL_W + LEGEND_LEFT_PAD_PX + progress.max(tiers) + CHART_RIGHT_PADDING_PX
}

/// 日数で決まる幅が下支えに届く、最も短い日数。これ以上の図では下支えが効かない。
fn first_unfloored_days() -> i64 {
    (0..)
        .find(|d| chart_width(*d as f64) >= floor_px())
        .unwrap()
}

/// 下支えが効く図の日数のうち、最も長い日。
fn longest_floored_days() -> i64 {
    first_unfloored_days() - 1
}

/// 下支えが効く図で、`Shown` の幅の内側に収まる今日の日数のうち、最も遅い日。
fn latest_today_inside_floor() -> i64 {
    ((floor_px() - LABEL_W) / PX_PER_DAY).floor() as i64
}

#[test]
fn the_boundary_exists_and_is_derived_from_the_legend() {
    // 下支えが何も支えない設計になっていたら、以下の試験は何も撃たない。
    assert!(
        floor_px() > chart_width(0.0),
        "the legend is not wider than the shortest chart"
    );
    assert!(first_unfloored_days() >= 1);
    // 境の前は下回り、境は上回る。
    assert!(chart_width(longest_floored_days() as f64) < floor_px());
    assert!(chart_width(first_unfloored_days() as f64) >= floor_px());
}

#[test]
fn shown_is_held_up_by_the_floor_exactly_below_the_boundary() {
    for days in 0..=longest_floored_days() {
        let w = list(days, LegendMode::Shown, None).viewport.width;
        assert_eq!(w, floor_px(), "{days} days");
    }
    for days in [
        first_unfloored_days(),
        first_unfloored_days() + 1,
        first_unfloored_days() + 40,
    ] {
        let w = list(days, LegendMode::Shown, None).viewport.width;
        assert_eq!(w, chart_width(days as f64), "{days} days");
    }
}

#[test]
fn hidden_takes_the_width_its_days_give_it_unless_a_label_runs_further() {
    for days in 0..=first_unfloored_days() + 5 {
        let l = list(days, LegendMode::Hidden, None);
        assert_eq!(l.viewport.width, hidden_width(days), "{days} days");
        assert!(l.viewport.width >= chart_width(days as f64), "{days} days");
    }
}

// --- 高さ ---

#[test]
fn hidden_takes_back_the_legend_height_and_keeps_the_bottom_padding() {
    for days in [0, 3, first_unfloored_days(), first_unfloored_days() + 20] {
        let shown = list(days, LegendMode::Shown, None);
        let hidden = list(days, LegendMode::Hidden, None);
        let rows = 2.0;
        assert_eq!(shown.viewport.height, chart_height(rows));
        assert_eq!(
            hidden.viewport.height,
            rows * ROW_H + HEADER_H + CHART_BOTTOM_PADDING_PX
        );
        assert_eq!(
            shown.viewport.height - hidden.viewport.height,
            LEGEND_H,
            "{days} days"
        );
        assert_eq!(
            hidden.viewport.height,
            chart_height_with(rows, LegendMode::Hidden)
        );
    }
}

#[test]
fn shown_keeps_the_width_it_has_always_had_whatever_the_labels_do() {
    // Shown の幅は、日数の幅と凡例の下支えの大きいほう。札の右端は幅を決めない
    // (design 4.8)。札が溢れる図も含めて、全部の図でこの式のままである。
    let mut checked = 0;
    for epoch_back in 0..7 {
        for (name, spans) in charts() {
            let total_days = spans.iter().map(|s| s.1).max().unwrap() + epoch_back;
            let expected = chart_width(total_days as f64).max(floor_px());
            let l = build_spans(&spans, epoch_back, LegendMode::Shown);
            assert_eq!(
                l.viewport.width, expected,
                "epoch {epoch_back} earlier, {name}"
            );
            checked += 1;
        }
    }
    assert!(checked > 1_000);
}

#[test]
fn the_default_options_show_the_legend() {
    assert_eq!(ChartOptions::default().legend, LegendMode::Shown);
    let default = build_display_list(&graph(20), epoch(), &ChartOptions::default());
    let shown = list(20, LegendMode::Shown, None);
    assert_eq!(
        serde_json::to_string(&default).unwrap(),
        serde_json::to_string(&shown).unwrap()
    );
}

// --- 下支えが効かない図（境以上）では、Shown から機械的に導ける ---

/// `Shown` の SVG から `Hidden` の SVG を導く。
/// 凡例の二つの `<g>` を取り除き、高さ（ルートの `height` と `viewBox` の第四の数）と
/// 縦線の `y2` を `LEGEND_H` 減らす。
fn hidden_svg_from_shown(shown: &str, width: f64, height: f64) -> String {
    let cut = shown
        .find(r#"<g class="progress-line-legend""#)
        .expect("the legend is the last thing drawn");
    let without_legend = format!("{}</svg>", &shown[..cut]);
    let new_height = height - LEGEND_H;
    let root_old = format!(r#"height="{height}" viewBox="0 0 {width} {height}""#);
    let root_new = format!(r#"height="{new_height}" viewBox="0 0 {width} {new_height}""#);
    assert!(without_legend.contains(&root_old));
    without_legend.replacen(&root_old, &root_new, 1).replace(
        &format!(r#"y2="{height}""#),
        &format!(r#"y2="{new_height}""#),
    )
}

/// IR の線の `y2` が `height` なら `LEGEND_H` 減らす。縦線の終わりだけがこの値を持つ。
fn lower_y2(value: &mut Value, height: f64) {
    match value {
        Value::Array(items) => items.iter_mut().for_each(|v| lower_y2(v, height)),
        Value::Object(map) => {
            for (key, v) in map.iter_mut() {
                if key == "y2" && v.as_f64() == Some(height) {
                    *v = serde_json::json!(height - LEGEND_H);
                } else {
                    lower_y2(v, height);
                }
            }
        }
        _ => {}
    }
}

/// canvas の折れ線の終わりの点 `[x, height]` を、`LEGEND_H` 減らす。
fn lower_polyline_ends(value: &mut Value, height: f64) {
    match value {
        Value::Array(items) => {
            let is_point = items.len() == 2 && items[1].as_f64() == Some(height);
            if is_point && items[0].is_number() {
                items[1] = serde_json::json!(height - LEGEND_H);
            } else {
                items
                    .iter_mut()
                    .for_each(|v| lower_polyline_ends(v, height));
            }
        }
        Value::Object(map) => map
            .values_mut()
            .for_each(|v| lower_polyline_ends(v, height)),
        _ => {}
    }
}

#[test]
fn hidden_svg_is_the_shown_svg_less_the_legend_when_no_floor_applies() {
    for days in [
        first_unfloored_days(),
        first_unfloored_days() + 1,
        first_unfloored_days() + 40,
    ] {
        for today in [None, Some(0), Some(days), Some(days / 2)] {
            let shown_list = list(days, LegendMode::Shown, today);
            let (w, h) = (shown_list.viewport.width, shown_list.viewport.height);
            let shown = svg(days, LegendMode::Shown, today);
            let hidden = svg(days, LegendMode::Hidden, today);
            assert_eq!(
                hidden,
                hidden_svg_from_shown(&shown, w, h),
                "{days} days, today {today:?}"
            );
        }
    }
}

#[test]
fn hidden_ir_is_the_shown_ir_less_the_legend_when_no_floor_applies() {
    for days in [first_unfloored_days(), first_unfloored_days() + 40] {
        for today in [None, Some(days / 2)] {
            let shown_list = list(days, LegendMode::Shown, today);
            let height = shown_list.viewport.height;
            let legend_count: u32 = shown_list
                .layers
                .iter()
                .filter(|l| l.kind == LayerKind::Legend)
                .map(|l| count_primitive_vec(&l.primitives))
                .sum();
            assert!(legend_count > 0);

            let mut derived = ir(days, LegendMode::Shown, today);
            derived["layers"]
                .as_array_mut()
                .unwrap()
                .retain(|l| l["kind"] != "Legend");
            lower_y2(&mut derived["layers"], height);
            derived["viewport"]["height"] = serde_json::json!(height - LEGEND_H);
            for key in ["primitive_count", "element_count_estimate"] {
                let n = derived["metadata"][key].as_u64().unwrap();
                derived["metadata"][key] = serde_json::json!(n - u64::from(legend_count));
            }
            assert_eq!(
                ir(days, LegendMode::Hidden, today),
                derived,
                "{days} days, today {today:?}"
            );
        }
    }
}

#[test]
fn hidden_canvas_is_the_shown_canvas_less_the_legend_when_no_floor_applies() {
    for days in [first_unfloored_days(), first_unfloored_days() + 40] {
        for today in [None, Some(days / 2)] {
            let shown = canvas(days, LegendMode::Shown, today);
            let hidden = canvas(days, LegendMode::Hidden, today);
            let height = shown["viewport_height"].as_f64().unwrap();
            let legend_ops =
                shown["ops"].as_array().unwrap().len() - hidden["ops"].as_array().unwrap().len();
            assert!(legend_ops > 0, "the legend draws something");

            let mut derived = shown.clone();
            let ops = derived["ops"].as_array_mut().unwrap();
            ops.truncate(ops.len() - legend_ops);
            lower_polyline_ends(&mut derived["ops"], height);
            derived["viewport_height"] = serde_json::json!(height - LEGEND_H);
            assert_eq!(hidden, derived, "{days} days, today {today:?}");
        }
    }
}

#[test]
fn hidden_has_no_legend_layer_and_counts_fewer_primitives() {
    let shown = list(30, LegendMode::Shown, None);
    let hidden = list(30, LegendMode::Hidden, None);
    assert!(shown.layers.iter().any(|l| l.kind == LayerKind::Legend));
    assert!(!hidden.layers.iter().any(|l| l.kind == LayerKind::Legend));
    assert!(hidden.metadata.primitive_count < shown.metadata.primitive_count);
    assert!(!svg(30, LegendMode::Hidden, None).contains("bar-tier-legend"));
    assert!(!svg(30, LegendMode::Hidden, None).contains("progress-line-legend"));
    assert!(svg(30, LegendMode::Shown, None).contains("bar-tier-legend"));
}

// --- 下支えが効く図（境の手前）では、幅などが動く ---

fn vertical_x_of_today(l: &DisplayList) -> Option<f64> {
    l.layers
        .iter()
        .filter(|layer| layer.kind == LayerKind::TodayMarker)
        .flat_map(|layer| layer.primitives.iter())
        .find_map(|p| match p {
            Primitive::Line(line) => Some(line.x1),
            _ => None,
        })
}

fn progress_points(l: &DisplayList) -> Vec<(f64, f64)> {
    l.layers
        .iter()
        .filter(|layer| layer.kind == LayerKind::ProgressLine)
        .flat_map(|layer| layer.primitives.iter())
        .find_map(|p| match p {
            Primitive::Polyline(line) => Some(line.points.clone()),
            _ => None,
        })
        .expect("a progress line")
}

fn header_band_width(l: &DisplayList) -> f64 {
    l.layers
        .iter()
        .filter(|layer| layer.kind == LayerKind::Background)
        .flat_map(|layer| layer.primitives.iter())
        .find_map(|p| match p {
            Primitive::Rect(r) if r.semantic == RectSemantic::HeaderBackground => Some(r.width),
            _ => None,
        })
        .expect("a header band")
}

fn floored_day_counts() -> Vec<i64> {
    let mut days = vec![0, 1, longest_floored_days()];
    days.dedup();
    days
}

#[test]
fn the_width_the_viewbox_and_the_header_band_follow_the_days_when_hidden() {
    for days in floored_day_counts() {
        let shown = list(days, LegendMode::Shown, None);
        let hidden = list(days, LegendMode::Hidden, None);
        assert!(hidden.viewport.width < shown.viewport.width, "{days} days");
        assert_eq!(hidden.viewport.width, hidden_width(days));
        assert_eq!(header_band_width(&shown), shown.viewport.width - LABEL_W);
        assert_eq!(header_band_width(&hidden), hidden_width(days) - LABEL_W);

        let shown_svg = svg(days, LegendMode::Shown, None);
        let hidden_svg = svg(days, LegendMode::Hidden, None);
        let w = hidden_width(days);
        let h = hidden.viewport.height;
        assert!(hidden_svg.contains(&format!(
            r#"width="{w}" height="{h}" viewBox="0 0 {w} {h}""#
        )));
        assert!(shown_svg.contains(&format!(r#"viewBox="0 0 {} "#, floor_px())));
    }
}

#[test]
fn a_today_after_the_last_day_but_inside_the_floor_is_drawn_only_when_shown() {
    let mut dropped_when_hidden = 0;
    for days in floored_day_counts() {
        for today in (days + 1)..=latest_today_inside_floor() {
            let shown = list(days, LegendMode::Shown, Some(today));
            let hidden = list(days, LegendMode::Hidden, Some(today));
            let x = LABEL_W + today as f64 * PX_PER_DAY;
            assert_eq!(
                vertical_x_of_today(&shown),
                Some(x),
                "{days} days, today {today}"
            );
            // Hidden の幅は日数か札が決める。その幅の内側の今日だけ描かれる。
            let expected = (x <= hidden_width(days)).then_some(x);
            assert_eq!(
                vertical_x_of_today(&hidden),
                expected,
                "{days} days, today {today}"
            );
            dropped_when_hidden += usize::from(expected.is_none());
        }
        // 範囲の後ろの今日が、そもそも広げた幅の内側にあった、という筋が成り立つ。
        // 日付の目盛りは `days` 日までしか引かれない。
        let shown = list(days, LegendMode::Shown, Some(days + 1));
        let grid_xs: Vec<f64> = shown
            .layers
            .iter()
            .filter(|l| l.kind == LayerKind::Grid)
            .flat_map(|l| l.primitives.iter())
            .filter_map(|p| match p {
                Primitive::Line(line) => Some(line.x1),
                _ => None,
            })
            .collect();
        let today_x = LABEL_W + (days + 1) as f64 * PX_PER_DAY;
        assert!(
            grid_xs.iter().all(|x| *x < today_x),
            "the grid stops before that today"
        );
    }
    // 消える今日が一つも無ければ、この試験は何も撃っていない。
    assert!(dropped_when_hidden > 0);
}

#[test]
fn a_today_inside_the_days_is_at_the_same_place_shown_or_hidden() {
    for days in floored_day_counts() {
        for today in 0..=days {
            let shown = list(days, LegendMode::Shown, Some(today));
            let hidden = list(days, LegendMode::Hidden, Some(today));
            let x = LABEL_W + today as f64 * PX_PER_DAY;
            assert_eq!(
                vertical_x_of_today(&shown),
                Some(x),
                "{days} days, today {today}"
            );
            assert_eq!(
                vertical_x_of_today(&hidden),
                Some(x),
                "{days} days, today {today}"
            );
            assert_eq!(progress_points(&shown), progress_points(&hidden));
        }
    }
}

#[test]
fn the_progress_line_takes_its_old_shape_when_the_today_is_gone() {
    let mut exercised = 0;
    for days in floored_day_counts() {
        let today = days + 1;
        if today > latest_today_inside_floor() {
            continue;
        }
        let shown_with_today = list(days, LegendMode::Shown, Some(today));
        let hidden_with_today = list(days, LegendMode::Hidden, Some(today));
        if vertical_x_of_today(&hidden_with_today).is_some() {
            // 札が幅を伸ばし、今日が収まった図。消えていないので旧い形にならない。
            continue;
        }
        exercised += 1;
        let legacy = list(days, LegendMode::Shown, None);
        // Shown は今日に繋ぐ形、Hidden は今日が無い時の旧い形。
        assert_ne!(
            progress_points(&shown_with_today),
            progress_points(&hidden_with_today)
        );
        assert_eq!(
            progress_points(&hidden_with_today),
            progress_points(&legacy),
            "{days} days"
        );
    }
    assert!(exercised > 0);
}

#[test]
fn the_boundary_days_themselves() {
    // 境の一日前は広がり、境の日は広がらない。
    let before = longest_floored_days();
    let at = first_unfloored_days();
    assert_ne!(
        list(before, LegendMode::Shown, None).viewport.width,
        list(before, LegendMode::Hidden, None).viewport.width
    );
    assert_eq!(
        list(at, LegendMode::Shown, None).viewport.width,
        list(at, LegendMode::Hidden, None).viewport.width
    );
}
