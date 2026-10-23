//! What the legend says, defined once.
//!
//! The chart draws its legend from `TIER_ITEMS` and `PROGRESS_LABEL`, and
//! `legend_spec` reads the same two, so a caller who hides the legend and draws
//! its own is shown exactly what the chart would have drawn.

use serde::Serialize;

use super::constants::{PROGRESS_LINE_DASH, PROGRESS_LINE_STROKE_W};
use super::types::{ColorId, Palette, ProgressTier};

/// The caption of the progress-line sample.
pub const PROGRESS_LABEL: &str = "進捗ステータスライン（各タスクの完了位置を結ぶ）";

/// The legend's swatches, in the order they are drawn.
pub const TIER_ITEMS: [(ProgressTier, &str); 5] = [
    (ProgressTier::None, "未達"),
    (ProgressTier::Low, "低"),
    (ProgressTier::Mid, "中"),
    (ProgressTier::High, "高"),
    (ProgressTier::Done, "完了"),
];

/// The colour a bar of this tier is filled with, and so the legend's swatch.
pub fn tier_fill_color(tier: ProgressTier) -> ColorId {
    match tier {
        ProgressTier::None => ColorId::BarBg,
        ProgressTier::Low => ColorId::TierLow,
        ProgressTier::Mid => ColorId::TierMid,
        ProgressTier::High => ColorId::TierHigh,
        ProgressTier::Done => ColorId::TierDone,
    }
}

/// What a caller needs to draw the legend itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegendSpec {
    pub tiers: Vec<LegendTier>,
    pub progress_line: LegendProgressLine,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegendTier {
    /// The same name as the `bar-tier-*` class suffix: `none`, `low`, `mid`,
    /// `high`, `done`.
    pub key: &'static str,
    pub label: String,
    /// `#rrggbb`.
    pub color: String,
    /// The lowest and highest `progress_pct` that fall in this tier.
    pub min_pct: i16,
    pub max_pct: i16,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct LegendProgressLine {
    pub label: String,
    pub color: String,
    pub stroke_width: f64,
    /// An SVG `stroke-dasharray` value.
    pub dash: String,
}

/// The percentages 0 to 100 that `ProgressTier::from_pct` puts in `tier`.
/// The ranges are read off that function, not written down a second time.
fn pct_range(tier: ProgressTier) -> (i16, i16) {
    let mut inside = (0..=100i16).filter(|pct| ProgressTier::from_pct(*pct) == tier);
    let min = inside
        .next()
        .expect("every tier is reached by some percentage");
    let max = inside.next_back().unwrap_or(min);
    (min, max)
}

pub fn legend_spec() -> LegendSpec {
    let palette = Palette::standard();
    LegendSpec {
        tiers: TIER_ITEMS
            .iter()
            .map(|(tier, label)| {
                let (min_pct, max_pct) = pct_range(*tier);
                LegendTier {
                    key: tier.css_suffix(),
                    label: (*label).to_string(),
                    color: palette.resolve(tier_fill_color(*tier)).to_string(),
                    min_pct,
                    max_pct,
                }
            })
            .collect(),
        progress_line: LegendProgressLine {
            label: PROGRESS_LABEL.to_string(),
            color: palette.resolve(ColorId::Progress).to_string(),
            stroke_width: PROGRESS_LINE_STROKE_W,
            dash: PROGRESS_LINE_DASH.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::display_list::build::build_display_list;
    use crate::display_list::types::{
        ChartOptions, LayerKind, LineSemantic, Primitive, RectSemantic, TextSemantic,
    };
    use crate::graph::{GanttGraph, GanttTask};
    use chrono::NaiveDate;

    fn date(d: u32) -> NaiveDate {
        NaiveDate::from_ymd_opt(2026, 6, d).unwrap()
    }

    #[test]
    fn the_spec_says_what_the_chart_has_always_drawn() {
        let json = serde_json::to_value(legend_spec()).unwrap();
        assert_eq!(
            json,
            serde_json::json!({
                "tiers": [
                    { "key": "none", "label": "未達", "color": "#d1d5db", "min_pct": 0, "max_pct": 0 },
                    { "key": "low", "label": "低", "color": "#f59e0b", "min_pct": 1, "max_pct": 33 },
                    { "key": "mid", "label": "中", "color": "#6366f1", "min_pct": 34, "max_pct": 66 },
                    { "key": "high", "label": "高", "color": "#0ea5e9", "min_pct": 67, "max_pct": 99 },
                    { "key": "done", "label": "完了", "color": "#22c55e", "min_pct": 100, "max_pct": 100 }
                ],
                "progress_line": {
                    "label": "進捗ステータスライン（各タスクの完了位置を結ぶ）",
                    "color": "#ef4444",
                    "stroke_width": 2.0,
                    "dash": "6,3"
                }
            })
        );
    }

    #[test]
    fn the_ranges_split_zero_to_one_hundred_exactly_as_from_pct_does() {
        let spec = legend_spec();
        for pct in 0..=100i16 {
            let owners: Vec<&LegendTier> = spec
                .tiers
                .iter()
                .filter(|t| t.min_pct <= pct && pct <= t.max_pct)
                .collect();
            assert_eq!(owners.len(), 1, "{pct}% belongs to exactly one tier");
            assert_eq!(
                owners[0].key,
                ProgressTier::from_pct(pct).css_suffix(),
                "{pct}%"
            );
        }
        // 段は 0 から 100 まで隙間なく並ぶ。
        assert_eq!(spec.tiers.first().unwrap().min_pct, 0);
        assert_eq!(spec.tiers.last().unwrap().max_pct, 100);
        for pair in spec.tiers.windows(2) {
            assert_eq!(pair[0].max_pct + 1, pair[1].min_pct);
        }
    }

    #[test]
    fn the_legend_the_chart_draws_is_the_one_the_spec_describes() {
        let graph = GanttGraph {
            tasks: vec![GanttTask {
                id: "a".into(),
                title: "A".into(),
                progress_pct: 50,
                start: date(1),
                end: Some(date(9)),
            }],
            deps: vec![],
        };
        let list = build_display_list(&graph, date(1), &ChartOptions::default());
        let palette = &list.palette;
        let legend = list
            .layers
            .iter()
            .find(|l| l.kind == LayerKind::Legend)
            .expect("the legend is drawn by default");

        let mut swatches = Vec::new();
        let mut tier_labels = Vec::new();
        let mut progress_lines = Vec::new();
        let mut progress_labels = Vec::new();
        for group in &legend.primitives {
            let Primitive::Group(g) = group else {
                panic!("the legend is two groups");
            };
            for child in &g.children {
                match child {
                    Primitive::Rect(r) if r.semantic == RectSemantic::LegendSwatch => {
                        swatches.push(palette.resolve(r.fill).to_string());
                    }
                    Primitive::Text(t) if t.semantic == TextSemantic::LegendTier => {
                        tier_labels.push(t.content.clone());
                    }
                    Primitive::Line(l) if l.semantic == LineSemantic::LegendProgressLine => {
                        progress_lines.push((
                            palette.resolve(l.stroke).to_string(),
                            l.stroke_width,
                            l.stroke_dash.clone(),
                        ));
                    }
                    Primitive::Text(t) if t.semantic == TextSemantic::LegendProgress => {
                        progress_labels.push(t.content.clone());
                    }
                    _ => {}
                }
            }
        }

        let spec = legend_spec();
        assert_eq!(
            swatches,
            spec.tiers
                .iter()
                .map(|t| t.color.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            tier_labels,
            spec.tiers
                .iter()
                .map(|t| t.label.clone())
                .collect::<Vec<_>>()
        );
        assert_eq!(
            progress_lines,
            vec![(
                spec.progress_line.color.clone(),
                spec.progress_line.stroke_width,
                Some(spec.progress_line.dash.clone())
            )]
        );
        assert_eq!(progress_labels, vec![spec.progress_line.label.clone()]);
    }

    #[test]
    fn a_bar_of_each_tier_is_coloured_as_the_spec_says() {
        // 凡例の色は、棒の色と同じ定義から出る。
        let palette = Palette::standard();
        for tier in legend_spec().tiers {
            let pct = tier.min_pct;
            let from_pct = ProgressTier::from_pct(pct);
            assert_eq!(from_pct.css_suffix(), tier.key);
            assert_eq!(palette.resolve(tier_fill_color(from_pct)), tier.color);
        }
    }
}
