use chrono::NaiveDate;
use serde::{Deserialize, Serialize};

pub type Coord = f64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ColorId {
    BarBg,
    TierLow,
    TierMid,
    TierHigh,
    TierDone,
    Dep,
    Progress,
    Grid,
    Today,
    HeaderBg,
    GridLabel,
    ProgressTextOnFg,
    ProgressTextOnBg,
    /// The band of a group row. Not in the standard palette: it falls back to
    /// `HeaderBg` until a caller sets it (see `ColorId::fallback`).
    GroupRow,
    /// The band of a weekend day. Not in the standard palette: it falls back
    /// to `HeaderBg`.
    Weekend,
    // The colours of the bar marks (docs/design/bar-marks.md). None is in the
    // standard palette; each falls back as `ColorId::fallback` says.
    /// The outline of a bar in the `normal` state, and the line from a point
    /// to its diamond.
    BarStroke,
    /// The fill of a bar in the `done` state.
    DoneBg,
    /// The outline of a bar in the `done` state.
    DoneStroke,
    /// The fill of a bar in the `warn` state.
    WarnBg,
    /// The outline and the note of a bar in the `warn` state.
    WarnStroke,
    /// The diamond of a hard deadline.
    HardDeadline,
    /// The note right of a bar.
    Note,
}

impl ColorId {
    /// The colour drawn for an id the palette has no entry for. Ids added after
    /// the standard palette was pinned are left out of it, so a chart that
    /// does not use them comes out as it did; each falls back to an id that is
    /// in it.
    pub fn fallback(self) -> Option<ColorId> {
        match self {
            ColorId::GroupRow | ColorId::Weekend => Some(ColorId::HeaderBg),
            ColorId::BarStroke | ColorId::DoneStroke => Some(ColorId::GridLabel),
            ColorId::DoneBg | ColorId::WarnBg => Some(ColorId::BarBg),
            ColorId::WarnStroke | ColorId::HardDeadline => Some(ColorId::Progress),
            ColorId::Note => Some(ColorId::GridLabel),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Palette {
    pub colors: Vec<(ColorId, String)>,
}

impl Palette {
    pub fn standard() -> Self {
        Self {
            colors: vec![
                (ColorId::BarBg, "#d1d5db".to_string()),
                (ColorId::TierLow, "#f59e0b".to_string()),
                (ColorId::TierMid, "#6366f1".to_string()),
                (ColorId::TierHigh, "#0ea5e9".to_string()),
                (ColorId::TierDone, "#22c55e".to_string()),
                (ColorId::Dep, "#9ca3af".to_string()),
                (ColorId::Progress, "#ef4444".to_string()),
                (ColorId::Grid, "#e5e7eb".to_string()),
                (ColorId::Today, "#f59e0b".to_string()),
                (ColorId::HeaderBg, "#f3f4f6".to_string()),
                (ColorId::GridLabel, "#6b7280".to_string()),
                (ColorId::ProgressTextOnFg, "#ffffff".to_string()),
                (ColorId::ProgressTextOnBg, "#374151".to_string()),
            ],
        }
    }

    pub fn resolve(&self, id: ColorId) -> &str {
        match self.colors.iter().find(|(cid, _)| *cid == id) {
            Some((_, color)) => color.as_str(),
            None => self.resolve(id.fallback().expect("palette entry")),
        }
    }

    /// This palette with `overrides` in place of its own colours. An id the
    /// palette has no entry for is added at the end, in the order of `ColorId`.
    pub fn with_overrides(mut self, overrides: &[(ColorId, String)]) -> Self {
        let mut added: Vec<(ColorId, String)> = Vec::new();
        for (id, color) in overrides {
            match self.colors.iter_mut().find(|(cid, _)| cid == id) {
                Some(entry) => entry.1 = color.clone(),
                None => match added.iter_mut().find(|(cid, _)| cid == id) {
                    Some(entry) => entry.1 = color.clone(),
                    None => added.push((*id, color.clone())),
                },
            }
        }
        added.sort_by_key(|(id, _)| *id as u8);
        self.colors.extend(added);
        self
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ProgressTier {
    None,
    Low,
    Mid,
    High,
    Done,
}

impl ProgressTier {
    pub fn from_pct(pct: i16) -> Self {
        match pct {
            0 => Self::None,
            1..=33 => Self::Low,
            34..=66 => Self::Mid,
            67..=99 => Self::High,
            _ => Self::Done,
        }
    }

    pub fn css_suffix(self) -> &'static str {
        match self {
            Self::None => "none",
            Self::Low => "low",
            Self::Mid => "mid",
            Self::High => "high",
            Self::Done => "done",
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Viewport {
    pub width: Coord,
    pub height: Coord,
    pub label_width: Coord,
    pub header_height: Coord,
    pub row_height: Coord,
}

/// Vertical scroll window for row virtualization (Phase 1 axis A).
#[derive(Debug, Clone, Copy, Serialize, Deserialize, PartialEq)]
pub struct ScrollViewport {
    pub scroll_y: Coord,
    pub client_height: Coord,
}

/// Whether the chart draws its legend (the tier swatches and the progress-line
/// sample under the rows). A caller that draws its own legend asks for `Hidden`
/// and reads the contents from `legend_spec`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LegendMode {
    #[default]
    Shown,
    Hidden,
}

/// Whether the chart draws one kind of its own text. `row_labels` uses it for
/// the task titles left of the days, `percent_labels` for the `NN%` on each bar
/// and milestone. A caller that lays the chart beside a list of its own, which
/// already shows the titles and the progress, asks for `Hidden`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum LabelMode {
    #[default]
    Shown,
    Hidden,
}

/// Where a bar sits in its row. `Middle` centres it. `Bottom` keeps under it
/// the gap a middle bar has in a row of the standard height, so a tall row
/// leaves its room above the bar, for a caller that writes over it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum BarAlign {
    #[default]
    Middle,
    Bottom,
}

/// What the date header marks. `Week` is the chart drawn before scales
/// existed: a line and a `M/D` label at each Monday. `Day` labels every day
/// with its number and weekday under the month, and keeps the Monday lines.
/// `Month` puts a line and a `YYYY/M` label at the first of each month. See
/// docs/design/day-scale.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Scale {
    #[default]
    Week,
    Day,
    Month,
}

/// The weekdays drawn as weekend bands, one bit per weekday from Monday
/// (bit 0) to Sunday (bit 6). Empty by default: a chart without bands is the
/// chart drawn before they existed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct WeekendDays(pub u8);

impl WeekendDays {
    pub fn contains(self, weekday: chrono::Weekday) -> bool {
        self.0 & (1 << weekday.num_days_from_monday()) != 0
    }
}

/// What a caller chooses about one drawing. `Default` is the chart drawn with no
/// today marker, no row window, and the legend and every label shown.
///
/// There is deliberately no `#[non_exhaustive]`: nothing promises a stable shape
/// during 0.1.0-alpha, and a caller builds one with
/// `ChartOptions { today, ..Default::default() }`.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct ChartOptions {
    /// The calendar day to mark. `None` draws no marker and the progress line
    /// takes its legacy shape.
    pub today: Option<NaiveDate>,
    /// The scroll window for row virtualization. `None` draws every row.
    pub scroll_viewport: Option<ScrollViewport>,
    /// `Hidden` leaves the legend out and takes its height back; see
    /// `docs/design/legend-opt-out.md` (4) for what else moves.
    pub legend: LegendMode,
    /// `Hidden` leaves the title column out: the days start at x = 0 and the
    /// rows have no title text and no hit area left of the bar. See
    /// `docs/design/labels-opt-out.md`.
    pub row_labels: LabelMode,
    /// `Hidden` leaves out the `NN%` on every bar and milestone, and the width
    /// a label running past the last day would have held up.
    pub percent_labels: LabelMode,
    /// The height of a row in pixels; `None` is `ROW_H`. A row of a caller's
    /// row list may ask for its own height. See docs/design/external-rows.md.
    pub row_height: Option<f64>,
    /// The height of the date header in pixels; `None` is `HEADER_H`.
    pub header_height: Option<f64>,
    /// The height of a bar in pixels; `None` is `BAR_H`.
    pub bar_height: Option<f64>,
    /// Where a bar sits in its row.
    pub bar_align: BarAlign,
    /// The width of a day in pixels; `None` is `PX_PER_DAY`.
    pub px_per_day: Option<f64>,
    /// What the date header marks.
    pub scale: Scale,
    /// The weekdays drawn as weekend bands.
    pub weekend: WeekendDays,
    /// The weekday labels of the `Day` scale, Monday first; `None` is
    /// `DEFAULT_WEEKDAY_LABELS`.
    pub weekday_labels: Option<[String; 7]>,
    /// The days the chart shows; `None` shows every task's days. See
    /// docs/design/view-range.md.
    pub view: Option<ViewRange>,
}

/// The days a chart shows, `from` through `to`, both included. A bar that runs
/// past either end is cut there and marked; a task wholly outside is drawn as
/// its mark alone.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ViewRange {
    pub from: NaiveDate,
    pub to: NaiveDate,
}

impl ViewRange {
    /// How many days the view shows.
    pub fn days(&self) -> i64 {
        (self.to - self.from).num_days() + 1
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BBox {
    pub x: Coord,
    pub y: Coord,
    pub width: Coord,
    pub height: Coord,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskBBox {
    pub task_id: String,
    pub row: u32,
    pub bbox: BBox,
    pub bar_bbox: BBox,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChartMetadata {
    pub title: String,
    pub description: String,
    pub task_bboxes: Vec<TaskBBox>,
    pub primitive_count: u32,
    pub element_count_estimate: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LayerKind {
    Background,
    Grid,
    Dependencies,
    Bars,
    ProgressLine,
    TodayMarker,
    Legend,
    OverlayHints,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextAnchor {
    Start,
    Middle,
    End,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextBaseline {
    Auto,
    Middle,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RectPrim {
    pub x: Coord,
    pub y: Coord,
    pub width: Coord,
    pub height: Coord,
    pub fill: ColorId,
    pub rx: Option<Coord>,
    pub semantic: RectSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RectSemantic {
    HeaderBackground,
    BarBackground,
    LegendSwatch,
    /// The band of a group row in a caller's row list.
    GroupRow,
    /// The band of a weekend day.
    Weekend,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RoundRectPrim {
    pub x: Coord,
    pub y: Coord,
    pub width: Coord,
    pub height: Coord,
    pub fill: ColorId,
    pub rx: Coord,
    pub tier: ProgressTier,
    pub semantic: RoundRectSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum RoundRectSemantic {
    BarProgress,
    /// The dot of a task row whose shape is `point`.
    Point,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct LinePrim {
    pub x1: Coord,
    pub y1: Coord,
    pub x2: Coord,
    pub y2: Coord,
    pub stroke: ColorId,
    pub stroke_width: Coord,
    pub stroke_dash: Option<String>,
    pub semantic: LineSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum LineSemantic {
    Grid,
    TodayMarker,
    LegendProgressLine,
    /// From a point to the diamond of its hard deadline.
    DeadlineLink,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PathPrim {
    pub d: String,
    pub stroke: ColorId,
    pub stroke_width: Coord,
    pub semantic: PathSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PathSemantic {
    DependencyArrow,
    /// The outline of a bar whose task row gives a state.
    BarOutline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolylinePrim {
    pub points: Vec<(Coord, Coord)>,
    pub stroke: ColorId,
    pub stroke_width: Coord,
    pub stroke_dash: Option<String>,
    pub semantic: PolylineSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolylineSemantic {
    ProgressStatusLine,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PolygonPrim {
    pub points: Vec<(Coord, Coord)>,
    pub fill: ColorId,
    pub stroke: ColorId,
    pub stroke_width: Coord,
    pub tier: ProgressTier,
    pub semantic: PolygonSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum PolygonSemantic {
    Milestone,
    /// The diamond of a task row's hard deadline.
    HardDeadline,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TextPrim {
    pub x: Coord,
    pub y: Coord,
    pub content: String,
    pub fill: Option<ColorId>,
    pub font_size: Option<Coord>,
    pub font_weight: Option<u16>,
    pub anchor: Option<TextAnchor>,
    pub baseline: TextBaseline,
    pub semantic: TextSemantic,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TextSemantic {
    GridLabel,
    ProgressPercent,
    RowLabel,
    LegendProgress,
    LegendTier,
    /// The day of the month over a day column (`Day` scale).
    DayNumber,
    /// The weekday under the day number (`Day` scale).
    Weekday,
    /// `YYYY/M` above the first day of a month (`Day` scale).
    MonthLabel,
    /// The note right of a bar.
    BarNote,
    /// The mark of a task that runs on past the days a view shows.
    Overflow,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GroupPrim {
    pub task_id: Option<String>,
    pub tooltip: Option<String>,
    pub bbox: BBox,
    pub children: Vec<Primitive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum Primitive {
    Rect(RectPrim),
    RoundRect(RoundRectPrim),
    Line(LinePrim),
    Path(PathPrim),
    Polyline(PolylinePrim),
    Polygon(PolygonPrim),
    Text(TextPrim),
    Group(GroupPrim),
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Layer {
    pub kind: LayerKind,
    pub primitives: Vec<Primitive>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DisplayList {
    pub viewport: Viewport,
    pub palette: Palette,
    pub layers: Vec<Layer>,
    pub metadata: ChartMetadata,
}

impl DisplayList {
    pub fn count_primitives(&self) -> u32 {
        self.layers
            .iter()
            .map(|l| count_primitive_vec(&l.primitives))
            .sum()
    }
}

pub fn count_primitive_vec(prims: &[Primitive]) -> u32 {
    prims
        .iter()
        .map(|p| match p {
            Primitive::Group(g) => 1 + count_primitive_vec(&g.children),
            _ => 1,
        })
        .sum()
}
