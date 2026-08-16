//! Every layout and dimension value the chart is built from lives here, and
//! only here. A backend that needs one reads the display list primitive that
//! carries it — it must not restate the number, because a restated number is a
//! second definition that can drift while every test stays green.

pub const ROW_H: f64 = 40.0;
pub const BAR_H: f64 = 20.0;
pub const BAR_PAD: f64 = (ROW_H - BAR_H) / 2.0;
pub const PX_PER_DAY: f64 = 30.0;
pub const LABEL_W: f64 = 120.0;
pub const HEADER_H: f64 = 30.0;
pub const ARROW_LEAD: f64 = 13.0;
pub const ARROW_CURVE: f64 = 4.0;
pub const ARROW_HEAD: f64 = 4.0;
pub const ROW_PADDING: f64 = ROW_H - BAR_H;
pub const TITLE_MAX_CHARS: usize = 16;
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
pub const fn chart_height(rows: f64) -> f64 {
    rows * ROW_H + CHART_CHROME_H
}

pub const fn chart_width(days: f64) -> f64 {
    days * PX_PER_DAY + CHART_CHROME_W
}
/// Corner radius shared by a bar's background and its progress fill: they are
/// stacked, so a mismatch shows as a sliver of background at every corner.
pub const BAR_CORNER_RADIUS_PX: f64 = 4.0;
/// Gap between the row label's right edge and the bar column.
pub const LABEL_GAP_PX: f64 = 4.0;
/// How far left of the label text the row's hover box reaches.
pub const LABEL_HIT_W: f64 = 100.0;
/// Step by which a dependency arrow backs off from its blocker's centre.
pub const DEP_BACKOFF_STEP_PX: f64 = 10.0;
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
pub const ELEMS_PER_ROW_MAX: u32 = 15;
pub const ELEMS_CHROME: u32 = 200;

pub const COLOR_BAR_BG: &str = "#d1d5db";
pub const COLOR_TIER_LOW: &str = "#f59e0b";
pub const COLOR_TIER_MID: &str = "#6366f1";
pub const COLOR_TIER_HIGH: &str = "#0ea5e9";
pub const COLOR_TIER_DONE: &str = "#22c55e";
pub const COLOR_TODAY: &str = "#f59e0b";
pub const COLOR_HEADER_BG: &str = "#f3f4f6";
