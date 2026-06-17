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
            collect_native_ops(prim, &mut ops);
        }
    }

    let palette_refs: Vec<(u8, String)> = list
        .palette
        .colors
        .iter()
        .enumerate()
        .map(|(i, (_, hex))| (i as u8, hex.clone()))
        .collect();

    NativeDrawList {
        viewport_width: list.viewport.width,
        viewport_height: list.viewport.height,
        ops,
        palette_refs,
    }
}

fn color_id_tag(id: ColorId) -> u8 {
    match id {
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
    }
}

fn collect_native_ops(prim: &Primitive, ops: &mut Vec<NativeDrawOp>) {
    match prim {
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
            ops.push(NativeDrawOp::DrawText {
                x: t.x,
                y: t.y,
                text: t.content.clone(),
                color_id: t.fill.map(color_id_tag).unwrap_or(255),
                anchor: anchor.unwrap_or(0),
                size: t.font_size.unwrap_or(12.0),
                weight: t.font_weight.unwrap_or(400),
            });
        }
        Primitive::Group(g) => {
            ops.push(NativeDrawOp::GroupStart {
                task_id: g.task_id.clone(),
            });
            for child in &g.children {
                collect_native_ops(child, ops);
            }
            ops.push(NativeDrawOp::GroupEnd);
        }
    }
}

pub fn count_draw_ops(list: &DisplayList) -> usize {
    build_native_draw_list(list).ops.len()
}
