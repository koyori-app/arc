//! Public API contract: a downstream crate can implement `RenderBackend` and inspect the IR.

use koyori_arc_core::{
    BackendOutput, ChartMetadata, ColorId, DisplayList, Layer, Palette, Primitive, RectPrim,
    RenderBackend, TaskBBox, TextPrim, Viewport,
};

struct CountingBackend;

impl RenderBackend for CountingBackend {
    fn render(&self, list: &DisplayList) -> BackendOutput {
        let primitive_count = list
            .layers
            .iter()
            .map(|layer| count_primitives(&layer.primitives))
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
            Primitive::Rect(RectPrim { fill, .. }) => {
                let _: ColorId = *fill;
                1
            }
            Primitive::Text(TextPrim { .. }) => 1,
            Primitive::Group(group) => 1 + count_primitives(&group.children),
            _ => 1,
        })
        .sum()
}

// Naming these types in function signatures proves that downstream helpers can use the public IR
// without reaching through the private `display_list` module.
#[allow(dead_code, clippy::too_many_arguments)]
fn public_ir_types_are_nameable(
    _layer: &Layer,
    _rect: &RectPrim,
    _text: &TextPrim,
    _color: ColorId,
    _palette: &Palette,
    _viewport: &Viewport,
    _task_bbox: &TaskBBox,
    _metadata: &ChartMetadata,
) {
}

#[test]
fn downstream_crate_can_implement_render_backend() {
    fn assert_backend<T: RenderBackend>() {}

    assert_backend::<CountingBackend>();
}
