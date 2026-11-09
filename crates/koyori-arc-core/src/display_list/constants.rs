//! Every layout and dimension value the chart is built from lives here, and
//! only here. A backend that needs one reads the display list primitive that
//! carries it — it must not restate the number, because a restated number is a
//! second definition that can drift while every test stays green.

use super::types::LegendMode;

pub const ROW_H: f64 = 40.0;
pub const BAR_H: f64 = 20.0;
/// The left margin of a chart without its title column, for the default bar:
/// half a bar, so a milestone on the first day draws the left half of its
/// diamond inside the chart. `label_width` takes half of the bar it is given.
#[cfg(test)]
pub const HIDDEN_LABELS_LEFT_PX: f64 = BAR_H / 2.0;
pub const PX_PER_DAY: f64 = 30.0;
/// Half the width of a hard deadline's diamond.
pub const HARD_DEADLINE_HALF_PX: f64 = 5.0;
/// The gap between a bar (or its diamond) and its note.
pub const NOTE_GAP_PX: f64 = 6.0;
/// How far inside the view's edge the mark of a task running past it sits.
pub const OVERFLOW_INSET_PX: f64 = 4.0;
/// How far the band under a note reaches past the note's text, each side.
pub const NOTE_BAND_PAD_PX: f64 = 2.0;
/// The weekday labels of the `Day` scale when the caller gives none.
pub const DEFAULT_WEEKDAY_LABELS: [&str; 7] = ["Mo", "Tu", "We", "Th", "Fr", "Sa", "Su"];
pub const LABEL_W: f64 = 120.0;
pub const HEADER_H: f64 = 30.0;
pub const ARROW_LEAD: f64 = 13.0;
pub const ARROW_CURVE: f64 = 4.0;
pub const ARROW_HEAD: f64 = 4.0;
pub const ROW_PADDING: f64 = ROW_H - BAR_H;
/// Left margin kept clear of row-label text so a long title is cut by its width,
/// never by the viewBox edge.
pub const ROW_LABEL_LEFT_MARGIN_PX: f64 = 4.0;
pub const LEGEND_H: f64 = 40.0;
pub const CHART_BOTTOM_PADDING_PX: f64 = 10.0;
/// Free space right of the last day. `build_display_list` spends it on the
/// chart width and `render.rs` subtracts it when deciding how many days still
/// fit inside a Canvas backing store, so the two must read one value.
pub const CHART_RIGHT_PADDING_PX: f64 = 20.0;
/// Height a chart spends on everything that is not a row, and width it spends
/// on everything that is not a day. `build.rs` adds these to the rows and days
/// it has; `render.rs` subtracts them to ask how many rows and days still fit
/// in a Canvas backing store. Listing the terms in both places would let a new
/// term reach the chart and never reach the guard.
pub const CHART_CHROME_H: f64 = HEADER_H + LEGEND_H + CHART_BOTTOM_PADDING_PX;
pub const CHART_CHROME_W: f64 = LABEL_W + CHART_RIGHT_PADDING_PX;

/// How big a chart of `rows` rows over `days` days comes out. The builder sizes
/// the viewport with these and the Canvas capacity guards ask them the same
/// question, so neither can answer it differently. `MAX_CANVAS_ROWS` and
/// `MAX_CANVAS_DATE_SPAN_DAYS` in `render.rs` invert them and are the one place
/// the terms are named a second time — a new term must be added to the chrome
/// constants above, not to a call site.
#[cfg(test)]
pub const fn chart_height(rows: f64) -> f64 {
    rows * ROW_H + CHART_CHROME_H
}

/// `CHART_CHROME_H` for a chart that shows the legend, less the legend's height
/// for one that hides it. The bottom padding stays: it is the margin under the
/// last row, not the legend's.
pub const fn chart_chrome_h(legend: LegendMode) -> f64 {
    match legend {
        LegendMode::Shown => CHART_CHROME_H,
        LegendMode::Hidden => CHART_CHROME_H - LEGEND_H,
    }
}

/// `chart_height` for a given legend mode. The Canvas capacity guard keeps
/// asking `chart_height`, which is the `Shown` answer, so a hidden legend never
/// loosens it.
#[cfg(test)]
pub const fn chart_height_with(rows: f64, legend: LegendMode) -> f64 {
    rows * ROW_H + chart_chrome_h(legend)
}

/// The width of `days` days of `px_per_day` for a chart whose title column is `label_w` wide:
/// `LABEL_W` with the row labels shown, 0 with them hidden. With `LABEL_W` it
/// is `days * PX_PER_DAY + CHART_CHROME_W`, the width the Canvas guard inverts.
pub const fn chart_width_with_label(days: f64, label_w: f64, px_per_day: f64) -> f64 {
    days * px_per_day + label_w + CHART_RIGHT_PADDING_PX
}

/// `chart_width_with_label` with the title column shown, the chart the tests
/// written before the column could be hidden compare against.
#[cfg(test)]
pub const fn chart_width(days: f64) -> f64 {
    chart_width_with_label(days, LABEL_W, PX_PER_DAY)
}
/// Corner radius shared by a bar's background and its progress fill: they are
/// stacked, so a mismatch shows as a sliver of background at every corner.
pub const BAR_CORNER_RADIUS_PX: f64 = 4.0;
/// Gap between the row label's right edge and the bar column.
pub const LABEL_GAP_PX: f64 = 4.0;
/// How far left of the label text the row's hover box reaches.
pub const LABEL_HIT_W: f64 = 100.0;
/// Widest a row label may be: the label column minus the gap before the bars and
/// the left margin. A title wider than this is cut with `…` by `text_metrics`.
pub const ROW_LABEL_MAX_W_PX: f64 = LABEL_W - LABEL_GAP_PX - ROW_LABEL_LEFT_MARGIN_PX;
/// Step by which a dependency arrow backs off from its blocker's centre.
pub const DEP_BACKOFF_STEP_PX: f64 = 10.0;
/// Legend geometry. Text is laid out by its estimated width (see `text_metrics`),
/// so these are the only numbers the legend spends besides the glyph widths.
pub const LEGEND_LEFT_PAD_PX: f64 = 8.0;
pub const LEGEND_SWATCH_PX: f64 = 10.0;
pub const LEGEND_SWATCH_GAP_PX: f64 = 2.0;
pub const LEGEND_ITEM_GAP_PX: f64 = 12.0;
pub const LEGEND_LINE_SAMPLE_W_PX: f64 = 28.0;
pub const LEGEND_LINE_TEXT_GAP_PX: f64 = 6.0;
pub const LEGEND_PROGRESS_FONT_PX: f64 = 10.0;
pub const LEGEND_TIER_FONT_PX: f64 = 9.0;
/// Stroke of the progress status line. The legend draws a sample of that same
/// line, so both read these two values or the legend stops depicting it.
pub const PROGRESS_LINE_STROKE_W: f64 = 2.0;
pub const PROGRESS_LINE_DASH: &str = "6,3";
/// The `NN%` label on a bar and on a milestone are the same label.
pub const PROGRESS_LABEL_FONT_PX: f64 = 11.0;
pub const PROGRESS_LABEL_FONT_WEIGHT: u16 = 600;
/// What a `TextPrim` means by "no font of my own": the value every backend
/// falls back to, matching the `font-size`/`font-weight` on the SVG root.
pub const DEFAULT_FONT_SIZE_PX: f64 = 12.0;
pub const DEFAULT_FONT_WEIGHT: u16 = 400;
/// Visible rows ± buffer for row virtualization (§5.2, §6.5).
pub const ROW_BUFFER: u32 = 2;
/// Phase 1 DOM_CAP design target (§6.5.3).
pub const DOM_CAP: u32 = 500;
