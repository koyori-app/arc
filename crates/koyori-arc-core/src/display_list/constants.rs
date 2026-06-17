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
