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
        self.colors
            .iter()
            .find(|(cid, _)| *cid == id)
            .map(|(_, hex)| hex.as_str())
            .expect("palette entry")
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
