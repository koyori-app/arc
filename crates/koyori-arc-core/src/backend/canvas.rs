use crate::display_list::types::*;
use crate::display_list::DisplayList;

use super::command_buffer::{CommandBuffer, DrawOp};
use super::traits::{BackendOutput, RenderBackend};

pub struct CanvasBackend;

impl RenderBackend for CanvasBackend {
    fn render(&self, list: &DisplayList) -> BackendOutput {
        BackendOutput::CanvasCommands(build_command_buffer(list))
    }

    fn name(&self) -> &'static str {
        "canvas"
    }
}

pub fn build_command_buffer(list: &DisplayList) -> CommandBuffer {
    let mut ops = Vec::new();
    for layer in &list.layers {
        for prim in &layer.primitives {
            collect_draw_ops(prim, &mut ops);
        }
    }

    CommandBuffer {
        viewport_width: list.viewport.width,
        viewport_height: list.viewport.height,
        ops,
        palette: list.palette.clone(),
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

fn collect_draw_ops(prim: &Primitive, ops: &mut Vec<DrawOp>) {
    match prim {
        Primitive::Rect(r) => {
            ops.push(DrawOp::FillRect {
                x: r.x,
                y: r.y,
                w: r.width,
                h: r.height,
                color_id: color_id_tag(r.fill),
                radius: r.rx.unwrap_or(0.0),
            });
        }
        Primitive::RoundRect(r) => {
            ops.push(DrawOp::FillRect {
                x: r.x,
                y: r.y,
                w: r.width,
                h: r.height,
                color_id: color_id_tag(r.fill),
                radius: r.rx,
            });
        }
        Primitive::Line(l) => {
            ops.push(DrawOp::StrokePolyline {
                points: vec![(l.x1, l.y1), (l.x2, l.y2)],
                color_id: color_id_tag(l.stroke),
                width: l.stroke_width,
                dash: l.stroke_dash.clone(),
            });
        }
        Primitive::Path(p) => {
            ops.push(DrawOp::StrokePath {
                d: p.d.clone(),
                color_id: color_id_tag(p.stroke),
                width: p.stroke_width,
            });
        }
        Primitive::Polyline(p) => {
            ops.push(DrawOp::StrokePolyline {
                points: p.points.clone(),
                color_id: color_id_tag(p.stroke),
                width: p.stroke_width,
                dash: p.stroke_dash.clone(),
            });
        }
        Primitive::Polygon(p) => {
            ops.push(DrawOp::FillPolygon {
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
            ops.push(DrawOp::DrawText {
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
            ops.push(DrawOp::GroupStart {
                task_id: g.task_id.clone(),
            });
            for child in &g.children {
                collect_draw_ops(child, ops);
            }
            ops.push(DrawOp::GroupEnd);
        }
    }
}
