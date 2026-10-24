//! 図の幅は、描いた物の右端を覆う。
//!
//! 幅を決める規則が個別の場合を数え上げていると、右へ伸びる物が増えた時に黙って漏れる。
//! ここでは、できあがった図の全ての部品を走査し、右端が `viewport.width` を超えないことを縛る。
//! 走査は部品の種類で網羅する。新しい種類の部品が増えれば、この `match` がコンパイルを通らず、
//! 右端の求め方を決めるまで先へ進めない。

use chrono::{Duration, NaiveDate};

use super::build::build_display_list;
use super::constants::*;
use super::text_metrics::text_width_weighted_px;
use super::types::*;
use crate::graph::{GanttGraph, GanttTask};

pub(super) fn epoch() -> NaiveDate {
    NaiveDate::from_ymd_opt(2026, 6, 1).unwrap()
}

/// `(開始日, 終了日, 進捗)`。終了日が開始日と同じならマイルストーン。
pub(super) type Span = (i64, i64, i16);

pub(super) fn graph_of(spans: &[Span]) -> GanttGraph {
    GanttGraph {
        tasks: spans
            .iter()
            .enumerate()
            .map(|(i, &(s, e, pct))| GanttTask {
                id: format!("t{i}"),
                title: format!("t{i}"),
                progress_pct: pct,
                start: epoch() + Duration::days(s),
                end: Some(epoch() + Duration::days(e)),
            })
            .collect(),
        deps: vec![],
    }
}

/// SVG パスの右端。`build.rs` が出す命令（M V H L の絶対、a v m l の相対）だけを読む。
fn path_right(d: &str) -> f64 {
    let toks = d.split_whitespace();
    let (mut x, mut y) = (0.0_f64, 0.0_f64);
    let mut right = f64::MIN;
    let mut cmd = ' ';
    let mut args: Vec<f64> = Vec::new();
    let flush = |cmd: char, args: &mut Vec<f64>, x: &mut f64, y: &mut f64, right: &mut f64| {
        match (cmd, args.as_slice()) {
            ('M', [a, b]) | ('L', [a, b]) => (*x, *y) = (*a, *b),
            ('V', [b]) => *y = *b,
            ('H', [a]) => *x = *a,
            ('v', [b]) => *y += *b,
            ('m', [a, b]) | ('l', [a, b]) => (*x, *y) = (*x + *a, *y + *b),
            ('a', [_, _, _, _, _, a, b]) => (*x, *y) = (*x + *a, *y + *b),
            (' ', []) => {}
            other => panic!("path command the scan does not read: {other:?} in {d}"),
        }
        if cmd != ' ' {
            *right = right.max(*x);
        }
        args.clear();
    };
    for t in toks {
        if t.len() == 1 && t.chars().next().unwrap().is_ascii_alphabetic() {
            flush(cmd, &mut args, &mut x, &mut y, &mut right);
            cmd = t.chars().next().unwrap();
        } else {
            args.push(t.parse().unwrap_or_else(|_| panic!("number {t} in {d}")));
        }
    }
    flush(cmd, &mut args, &mut x, &mut y, &mut right);
    right
}

/// 部品の右端と、その部品の名。
pub(super) fn right_edges(list: &DisplayList) -> Vec<(String, f64)> {
    fn walk(p: &Primitive, out: &mut Vec<(String, f64)>) {
        match p {
            Primitive::Rect(r) => out.push((format!("{:?}", r.semantic), r.x + r.width)),
            Primitive::RoundRect(r) => out.push((format!("{:?}", r.semantic), r.x + r.width)),
            Primitive::Line(l) => out.push((format!("{:?}", l.semantic), l.x1.max(l.x2))),
            Primitive::Path(p) => out.push((format!("{:?}", p.semantic), path_right(&p.d))),
            Primitive::Polyline(p) => out.push((
                format!("{:?}", p.semantic),
                p.points.iter().map(|q| q.0).fold(f64::MIN, f64::max),
            )),
            Primitive::Polygon(p) => out.push((
                format!("{:?}", p.semantic),
                p.points.iter().map(|q| q.0).fold(f64::MIN, f64::max),
            )),
            Primitive::Text(t) => {
                let w = text_width_weighted_px(
                    &t.content,
                    t.font_size.unwrap_or(DEFAULT_FONT_SIZE_PX),
                    t.font_weight,
                );
                let right = match t.anchor {
                    None | Some(TextAnchor::Start) => t.x + w,
                    Some(TextAnchor::Middle) => t.x + w / 2.0,
                    Some(TextAnchor::End) => t.x,
                };
                out.push((format!("{:?} {:?}", t.semantic, t.content), right));
            }
            Primitive::Group(g) => g.children.iter().for_each(|c| walk(c, out)),
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

pub(super) fn build(spans: &[Span], epoch_back: i64, legend: LegendMode) -> DisplayList {
    build_display_list(
        &graph_of(spans),
        epoch() - Duration::days(epoch_back),
        &ChartOptions {
            legend,
            ..Default::default()
        },
    )
}

/// 長さと終わり方の違う図の集まり。終わりの日が最後の日になるものを多く含める。
/// 開始の日を変えて、最後の日が月曜になる図（目盛りの札が最後の日に立つ）も含める。
pub(super) fn charts() -> Vec<(String, Vec<Span>)> {
    let mut cases = Vec::new();
    for d in 0..=40 {
        cases.push((format!("bar 0..{d}, 100%"), vec![(0, d, 100)]));
        cases.push((format!("bar 0..{d}, 0%"), vec![(0, d, 0)]));
        cases.push((
            format!("milestone on {d}, 100%"),
            vec![(d, d, 100), (0, d, 50)],
        ));
        cases.push((format!("milestone alone on {d}"), vec![(d, d, 100)]));
        cases.push((
            format!("one day bar ending {}", d + 1),
            vec![(d, d + 1, 100)],
        ));
        cases.push((
            format!("one day bar ending {} after a long one", d + 1),
            vec![(0, d, 40), (d, d + 1, 100)],
        ));
    }
    cases
}

#[test]
fn nothing_drawn_runs_past_the_viewbox() {
    let mut checked = 0;
    for legend in [LegendMode::Shown, LegendMode::Hidden] {
        for epoch_back in 0..7 {
            for (name, spans) in charts() {
                let list = build(&spans, epoch_back, legend);
                for (what, right) in right_edges(&list) {
                    checked += 1;
                    assert!(
                        right <= list.viewport.width + 1e-9,
                        "{legend:?}, epoch {epoch_back} days earlier, {name}: {what} ends at {right}, the viewBox is {}",
                        list.viewport.width
                    );
                }
            }
        }
    }
    // 何も走査していない試験は何も縛らない。
    assert!(checked > 10_000, "{checked} parts scanned");
}

#[test]
fn the_scan_sees_an_overflow_when_there_is_one() {
    // 走査が部品を見落としていないことの陽性対照。幅を狭めた図は、必ず落ちる。
    let mut list = build(&[(0, 1, 100)], 0, LegendMode::Hidden);
    list.viewport.width = LABEL_W;
    assert!(right_edges(&list)
        .iter()
        .any(|(_, right)| *right > list.viewport.width));
    let kinds: Vec<String> = right_edges(&list).into_iter().map(|(w, _)| w).collect();
    for expected in ["ProgressPercent", "GridLabel", "BarBackground"] {
        assert!(
            kinds.iter().any(|k| k.starts_with(expected)),
            "{expected} is scanned"
        );
    }
}

#[test]
fn the_width_does_not_follow_the_scroll_window() {
    // 窓の外の行の札も、幅を決める。窓が動くたびに図の幅が変われば、横に揺れる。
    let spans: Vec<Span> = (0..30)
        .map(|i| if i == 29 { (20, 21, 100) } else { (0, 5, 50) })
        .collect();
    for legend in [LegendMode::Shown, LegendMode::Hidden] {
        let whole = build(&spans, 0, legend).viewport.width;
        for scroll_y in [0.0, 300.0, 600.0, 1100.0] {
            let windowed = build_display_list(
                &graph_of(&spans),
                epoch(),
                &ChartOptions {
                    legend,
                    scroll_viewport: Some(ScrollViewport {
                        scroll_y,
                        client_height: 200.0,
                    }),
                    ..Default::default()
                },
            );
            assert_eq!(
                windowed.viewport.width, whole,
                "{legend:?}, scroll {scroll_y}"
            );
        }
    }
}

fn percent_label(list: &DisplayList) -> (f64, f64) {
    // (左端, 右端)
    let (x, right) = list
        .layers
        .iter()
        .filter(|l| l.kind == LayerKind::Bars)
        .flat_map(|l| l.primitives.iter())
        .find_map(|p| match p {
            Primitive::Group(g) => g.children.iter().find_map(|c| match c {
                Primitive::Text(t) if t.semantic == TextSemantic::ProgressPercent => {
                    let w = text_width_weighted_px(&t.content, t.font_size.unwrap(), t.font_weight);
                    Some((t.x, t.x + w))
                }
                _ => None,
            }),
            _ => None,
        })
        .expect("a percent label");
    (x, right)
}

#[test]
fn a_milestone_alone_on_day_zero_keeps_its_whole_label_inside_when_hidden() {
    // 開始と終了が同じマイルストーン一つ。日数は 0 で、日数の幅は 140px しかない。
    let list = build(&[(0, 0, 100)], 0, LegendMode::Hidden);
    let (x, right) = percent_label(&list);
    assert!(x > LABEL_W, "the label is to the right of the diamond");
    assert!(
        right <= list.viewport.width,
        "label ends at {right}, the viewBox is {}",
        list.viewport.width
    );
    assert!(
        list.viewport.width > chart_width(0.0),
        "the days alone were not enough"
    );
}

#[test]
fn a_bar_narrower_than_its_label_keeps_the_label_inside_when_hidden() {
    // 一日の棒は 30px で、100% の札が棒の中に入らず、棒の右に出る。
    for legend in [LegendMode::Shown, LegendMode::Hidden] {
        let list = build(&[(0, 1, 100)], 0, legend);
        let (x, right) = percent_label(&list);
        assert!(
            x >= LABEL_W + PX_PER_DAY,
            "the label is to the right of the bar"
        );
        assert!(
            right <= list.viewport.width,
            "{legend:?}: label ends at {right}, the viewBox is {}",
            list.viewport.width
        );
    }
}

#[test]
fn a_label_that_fits_inside_its_bar_does_not_widen_the_chart() {
    // 札が棒の内側に収まる図は、日数が決めた幅のままである。
    let list = build(&[(0, 20, 100)], 0, LegendMode::Hidden);
    assert_eq!(list.viewport.width, chart_width(20.0));
}
