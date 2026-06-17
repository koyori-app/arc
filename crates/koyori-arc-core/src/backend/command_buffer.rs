use serde::{Deserialize, Serialize};

use crate::display_list::types::Palette;

/// Compact draw commands for Canvas2D replay (§4.3.1).
/// Variants mirror `NativeDrawOp` for in-process parity; IDs are inlined for Phase 2.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CommandBuffer {
    pub viewport_width: f64,
    pub viewport_height: f64,
    pub ops: Vec<DrawOp>,
    pub palette: Palette,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum DrawOp {
    FillRect {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color_id: u8,
        radius: f64,
    },
    StrokePath {
        d: String,
        color_id: u8,
        width: f64,
    },
    StrokePolyline {
        points: Vec<(f64, f64)>,
        color_id: u8,
        width: f64,
        dash: Option<String>,
    },
    FillPolygon {
        points: Vec<(f64, f64)>,
        color_id: u8,
    },
    DrawText {
        x: f64,
        y: f64,
        text: String,
        color_id: u8,
        anchor: u8,
        size: f64,
        weight: u16,
    },
    GroupStart {
        task_id: Option<String>,
    },
    GroupEnd,
}
