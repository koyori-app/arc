//! Public API contract: a downstream crate can implement `RenderBackend` and inspect the IR.

use koyori_arc_core::{
    BBox, BackendOutput, ChartMetadata, ColorId, Coord, DisplayList, GroupPrim, Layer, LayerKind,
    LinePrim, LineSemantic, Palette, PathPrim, PathSemantic, PolygonPrim, PolygonSemantic,
    PolylinePrim, PolylineSemantic, Primitive, ProgressTier, RectPrim, RectSemantic, RenderBackend,
    RoundRectPrim, RoundRectSemantic, TaskBBox, TextAnchor, TextBaseline, TextPrim, TextSemantic,
    Viewport,
};

struct CountingBackend;

impl RenderBackend for CountingBackend {
    fn render(&self, list: &DisplayList) -> BackendOutput {
        let primitive_count = list
            .layers
            .iter()
            .map(|layer| match layer.kind {
                LayerKind::Background
                | LayerKind::Grid
                | LayerKind::Dependencies
                | LayerKind::Bars
                | LayerKind::ProgressLine
                | LayerKind::TodayMarker
                | LayerKind::Legend
                | LayerKind::OverlayHints => count_primitives(&layer.primitives),
            })
            .sum::<usize>();
        BackendOutput::Svg(primitive_count.to_string())
    }

    fn name(&self) -> &'static str {
        "counting"
    }
}

fn count_primitives(primitives: &[Primitive]) -> usize {
    primitives
        .iter()
        .map(|primitive| match primitive {
            Primitive::Rect(RectPrim { fill, semantic, .. }) => {
                let _: ColorId = *fill;
                match semantic {
                    RectSemantic::HeaderBackground
                    | RectSemantic::BarBackground
                    | RectSemantic::LegendSwatch => 1,
                }
            }
            Primitive::RoundRect(_) => 1,
            Primitive::Line(_) => 1,
            Primitive::Path(_) => 1,
            Primitive::Polyline(_) => 1,
            Primitive::Polygon(_) => 1,
            Primitive::Text(TextPrim { .. }) => 1,
            Primitive::Group(group) => 1 + count_primitives(&group.children),
        })
        .sum()
}

// Naming these types in function signatures proves that downstream helpers can use the public IR
// without reaching through the private `display_list` module.
#[allow(dead_code, clippy::too_many_arguments)]
fn public_ir_types_are_nameable(
    _layer: &Layer,
    _line: &LinePrim,
    _rect: &RectPrim,
    _round_rect: &RoundRectPrim,
    _path: &PathPrim,
    _polyline: &PolylinePrim,
    _polygon: &PolygonPrim,
    _text: &TextPrim,
    _group: &GroupPrim,
    _color: ColorId,
    _coord: Coord,
    _palette: &Palette,
    _viewport: &Viewport,
    _bbox: &BBox,
    _task_bbox: &TaskBBox,
    _metadata: &ChartMetadata,
    _layer_kind: LayerKind,
    _line_semantic: LineSemantic,
    _path_semantic: PathSemantic,
    _polygon_semantic: PolygonSemantic,
    _polyline_semantic: PolylineSemantic,
    _progress_tier: ProgressTier,
    _rect_semantic: RectSemantic,
    _round_rect_semantic: RoundRectSemantic,
    _text_anchor: TextAnchor,
    _text_baseline: TextBaseline,
    _text_semantic: TextSemantic,
) {
}

#[test]
fn downstream_crate_can_implement_render_backend() {
    fn assert_backend<T: RenderBackend>() {}

    assert_backend::<CountingBackend>();

    let list = DisplayList {
        viewport: Viewport {
            width: 640.0,
            height: 480.0,
            label_width: 120.0,
            header_height: 30.0,
            row_height: 36.0,
        },
        palette: Palette::standard(),
        layers: vec![Layer {
            kind: LayerKind::Background,
            primitives: vec![Primitive::Rect(RectPrim {
                x: 0.0,
                y: 0.0,
                width: 640.0,
                height: 30.0,
                fill: ColorId::HeaderBg,
                rx: None,
                semantic: RectSemantic::HeaderBackground,
            })],
        }],
        metadata: ChartMetadata {
            title: "External backend contract".to_string(),
            description: String::new(),
            task_bboxes: Vec::new(),
            primitive_count: 1,
            element_count_estimate: 1,
        },
    };

    match CountingBackend.render(&list) {
        BackendOutput::Svg(count) => assert_eq!(count, "1"),
        _ => panic!("counting backend returned an unexpected output variant"),
    }
}
