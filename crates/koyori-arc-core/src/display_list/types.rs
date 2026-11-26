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
    /// The band under a note, so the today line and the grid do not run
    /// through it: the colour of the chart's ground.
    NoteBg,
    /// The progress line drawn `by_row`, and the dots at its bends.
    ProgressLine,
    /// The day's number in the `Day` scale's header. It has no entry in the
    /// standard palette and no fallback: until a caller sets it, the number is
    /// drawn with no colour of its own, as it was before it had a name (see
    /// `Palette::get`).
    DayNumber,
    /// A styled dependency edge whose blocker is done. Not in the standard
    /// palette: it falls back to `Dep`.
    DepDone,
    /// A styled dependency edge whose blocked task's start has come while its
    /// blocker is not done, and that edge's note. Not in the standard palette:
    /// it falls back to `Dep`.
    DepDelayed,
    /// The percent label inside a bar a task row gives a colour, drawn over a
    /// halo (docs/design/row-bar-color.md). Not in the standard palette: it
    /// falls back to `ProgressTextOnBg`.
    ColoredBarText,
    /// The halo round `ColoredBarText`. Not in the standard palette: it falls
    /// back to `ProgressTextOnFg`.
    ColoredBarHalo,
    /// The colour a task row gives its bar: the `n`th distinct colour of the
    /// chart's rows, added at the end of the palette as the chart is built.
    /// Never a key of `palette`. Added last, so the ids before it keep their
    /// place in every serialised form.
    Row(u16),
}

/// The number of `ColorId::Row` colours a chart can hold. `MAX_ROWS` keeps the
/// Wasm entry points well under it; a native caller with more distinct colours
/// draws the rest as rows without one.
pub const MAX_ROW_COLORS: usize = (u16::MAX - ROW_COLOR_TAG_BASE) as usize;

/// The canvas tag of `ColorId::Row(0)`. The tags below it are the named ids and
/// 255, the text drawn with no colour of its own.
pub const ROW_COLOR_TAG_BASE: u16 = 256;

impl ColorId {
    /// The number the canvas and native command lists carry for this id, and
    /// the order ids added to a palette are kept in. A named id's number is its
    /// place in the enum; `Row(n)` is `ROW_COLOR_TAG_BASE + n`.
    pub fn tag(self) -> u16 {
        match self {
            ColorId::BarBg => 0,
            ColorId::TierLow => 1,
            ColorId::TierMid => 2,
            ColorId::TierHigh => 3,
            ColorId::TierDone => 4,
            ColorId::Dep => 5,
            ColorId::Progress => 6,
            ColorId::Grid => 7,
            ColorId::Today => 8,
            ColorId::HeaderBg => 9,
            ColorId::GridLabel => 10,
            ColorId::ProgressTextOnFg => 11,
            ColorId::ProgressTextOnBg => 12,
            ColorId::GroupRow => 13,
            ColorId::Weekend => 14,
            ColorId::BarStroke => 15,
            ColorId::DoneBg => 16,
            ColorId::DoneStroke => 17,
            ColorId::WarnBg => 18,
            ColorId::WarnStroke => 19,
            ColorId::HardDeadline => 20,
            ColorId::Note => 21,
            ColorId::NoteBg => 22,
            ColorId::ProgressLine => 23,
            ColorId::DayNumber => 24,
            ColorId::DepDone => 25,
            ColorId::DepDelayed => 26,
            ColorId::ColoredBarText => 27,
            ColorId::ColoredBarHalo => 28,
            ColorId::Row(n) => ROW_COLOR_TAG_BASE.saturating_add(n),
        }
    }

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
            ColorId::NoteBg => Some(ColorId::HeaderBg),
            ColorId::ProgressLine => Some(ColorId::Progress),
            ColorId::DepDone | ColorId::DepDelayed => Some(ColorId::Dep),
            ColorId::ColoredBarText => Some(ColorId::ProgressTextOnBg),
            ColorId::ColoredBarHalo => Some(ColorId::ProgressTextOnFg),
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

    /// The colour of `id` through its fallbacks, or `None` when neither the
    /// palette nor a fallback has one. A text whose colour is `None` here is
    /// drawn with no colour of its own (the SVG's inherited fill, the canvas's
    /// default text colour).
    pub fn get(&self, id: ColorId) -> Option<&str> {
        match self.colors.iter().find(|(cid, _)| *cid == id) {
            Some((_, color)) => Some(color.as_str()),
            None => self.get(id.fallback()?),
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
        added.sort_by_key(|(id, _)| id.tag());
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
/// How the chart draws the progress line. See docs/design/progress-line.md.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ProgressLineMode {
    /// The line as drawn before the modes: through every task's progress.
    #[default]
    Shown,
    /// No line.
    Hidden,
    /// Out from the today line to each row that is late or ahead, and back to
    /// it at the row's bottom edge; on the today line for every other row.
    ByRow,
}

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
    /// `Hidden` leaves the title column out: the days start at
    /// x = `HIDDEN_LABELS_LEFT_PX` (half a bar; half of `bar_height` when that
    /// is set) and the rows have no title text and no hit area left of the bar
    /// or the milestone's diamond. See
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
    /// How the progress line is drawn; `Shown` is the line as before.
    pub progress_line: ProgressLineMode,
    /// How a bar a task row gives a colour shows its progress.
    pub row_color_progress: RowColorProgress,
    /// How a bar a task row gives a colour shows its percent label.
    pub row_color_percent: RowColorPercent,
}

/// How a bar a task row gives a colour shows its progress. Only rows with a
/// colour read it. See docs/design/row-bar-color.md (4.2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowColorProgress {
    /// The colour over the progress, the same colour faint over the rest: the
    /// way an uncoloured bar reads.
    #[default]
    Shade,
    /// The whole bar in the colour, the progress as a thin line under it.
    Line,
}

/// How a bar a task row gives a colour shows its percent label. Only rows with
/// a colour read it. See docs/design/row-bar-color.md (4.3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum RowColorPercent {
    /// The label inside the bar over a halo, so it reads on any colour.
    #[default]
    Halo,
    /// No label on a row with a colour.
    Hidden,
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
    /// The notes of task rows with their bands, over the today line and the
    /// grid. Only a chart with a note has this layer.
    Notes,
    /// The styled dependency edges, under the bars and over the group bands.
    /// Only a chart with a styled edge has this layer. Added last, so the
    /// kinds before it keep their place in every serialised form.
    DependencyLinks,
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
    /// The band under a note.
    NoteBackground,
    /// The band under the mark at an edge the view cuts a bar.
    OverflowBackground,
    /// The groove under a bar a task row gives a colour, drawn
    /// `RowColorProgress::Line`. Added last with the two after it, so the
    /// variants before keep their place in every serialised form.
    RowProgressTrack,
    /// The progress in that groove.
    RowProgress,
    /// The part of a bar a task row gives a colour, drawn
    /// `RowColorProgress::Shade`: the whole bar in the colour at
    /// `ROW_COLOR_REST_OPACITY`, under the progress.
    ColoredBarRest,
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
    /// A dot at a bend of the progress line drawn `by_row`.
    ProgressPoint,
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
    /// A styled dependency edge in the `delayed` state, drawn dashed
    /// (`DEP_DELAYED_DASH`). Added last, so the variants before it keep their
    /// place in every serialised form.
    DelayedDependencyArrow,
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
    /// The percent label inside a bar a task row gives a colour, drawn over a
    /// halo of `ColorId::ColoredBarHalo`, `ROW_COLOR_HALO_PX` wide. Added
    /// last, so the variants before it keep their place in every serialised
    /// form.
    ColoredBarPercent,
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
