use crate::display_list::constants::{
    DEFAULT_FONT_SIZE_PX, DEFAULT_FONT_WEIGHT, ROW_COLOR_HALO_PX, ROW_COLOR_REST_OPACITY,
};
use crate::display_list::types::*;
use crate::display_list::DisplayList;

use super::traits::{BackendOutput, NativeDrawList, NativeDrawOp, RenderBackend};

pub struct NativeBackend;

impl RenderBackend for NativeBackend {
    fn render(&self, list: &DisplayList) -> BackendOutput {
        BackendOutput::NativeDrawList(build_native_draw_list(list))
    }

    fn name(&self) -> &'static str {
        "native-stub"
    }
}

pub fn build_native_draw_list(list: &DisplayList) -> NativeDrawList {
    let mut ops = Vec::new();
    for layer in &list.layers {
        for prim in &layer.primitives {
            collect_native_ops(prim, &list.palette, &mut ops);
        }
    }

    let palette_refs: Vec<(u16, String)> = list
        .palette
        .colors
        .iter()
        .enumerate()
        .map(|(i, (_, hex))| (i as u16, hex.clone()))
        .collect();

    NativeDrawList {
        viewport_width: list.viewport.width,
        viewport_height: list.viewport.height,
        ops,
        palette_refs,
    }
}

fn color_id_tag(id: ColorId) -> u16 {
    id.tag()
}

/// A text's colour tag: 255, the default text colour, when the text has no
/// colour or its colour has no entry in the palette and no fallback.
fn text_color_tag(fill: Option<ColorId>, palette: &Palette) -> u16 {
    fill.filter(|id| palette.get(*id).is_some())
        .map(color_id_tag)
        .unwrap_or(255)
}

fn collect_native_ops(prim: &Primitive, palette: &Palette, ops: &mut Vec<NativeDrawOp>) {
    match prim {
        Primitive::Rect(r) if r.semantic == RectSemantic::ColoredBarRest => {
            ops.push(NativeDrawOp::FillRectAlpha {
                x: r.x,
                y: r.y,
                w: r.width,
                h: r.height,
                color_id: color_id_tag(r.fill),
                radius: r.rx.unwrap_or(0.0),
                alpha: ROW_COLOR_REST_OPACITY,
            });
        }
        Primitive::Rect(r) => {
            ops.push(NativeDrawOp::FillRect {
                x: r.x,
                y: r.y,
                w: r.width,
                h: r.height,
                color_id: color_id_tag(r.fill),
                radius: r.rx.unwrap_or(0.0),
            });
        }
        Primitive::RoundRect(r) => {
            ops.push(NativeDrawOp::FillRect {
                x: r.x,
                y: r.y,
                w: r.width,
                h: r.height,
                color_id: color_id_tag(r.fill),
                radius: r.rx,
            });
        }
        Primitive::Line(l) => {
            ops.push(NativeDrawOp::StrokePolyline {
                points: vec![(l.x1, l.y1), (l.x2, l.y2)],
                color_id: color_id_tag(l.stroke),
                width: l.stroke_width,
                dash: l.stroke_dash.clone(),
            });
        }
        Primitive::Path(p) => {
            ops.push(NativeDrawOp::StrokePath {
                d: p.d.clone(),
                color_id: color_id_tag(p.stroke),
                width: p.stroke_width,
            });
        }
        Primitive::Polyline(p) => {
            ops.push(NativeDrawOp::StrokePolyline {
                points: p.points.clone(),
                color_id: color_id_tag(p.stroke),
                width: p.stroke_width,
                dash: p.stroke_dash.clone(),
            });
        }
        Primitive::Polygon(p) => {
            ops.push(NativeDrawOp::FillPolygon {
                points: p.points.clone(),
                color_id: color_id_tag(p.fill),
            });
        }
        Primitive::Text(t) => {
            let anchor = t.anchor.map(|a| match a {
                TextAnchor::Start => 0,
                TextAnchor::Middle => 1,
                TextAnchor::End => 2,
            });
            let color_id = text_color_tag(t.fill, palette);
            let anchor = anchor.unwrap_or(0);
            let size = t.font_size.unwrap_or(DEFAULT_FONT_SIZE_PX);
            let weight = t.font_weight.unwrap_or(DEFAULT_FONT_WEIGHT);
            if t.semantic == TextSemantic::ColoredBarPercent {
                ops.push(NativeDrawOp::DrawHaloText {
                    x: t.x,
                    y: t.y,
                    text: t.content.clone(),
                    color_id,
                    anchor,
                    size,
                    weight,
                    halo_color_id: text_color_tag(Some(ColorId::ColoredBarHalo), palette),
                    halo_width: ROW_COLOR_HALO_PX,
                });
            } else {
                ops.push(NativeDrawOp::DrawText {
                    x: t.x,
                    y: t.y,
                    text: t.content.clone(),
                    color_id,
                    anchor,
                    size,
                    weight,
                });
            }
        }
        Primitive::Group(g) => {
            ops.push(NativeDrawOp::GroupStart {
                task_id: g.task_id.clone(),
            });
            for child in &g.children {
                collect_native_ops(child, palette, ops);
            }
            ops.push(NativeDrawOp::GroupEnd);
        }
    }
}
