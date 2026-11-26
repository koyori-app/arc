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
        color_id: u16,
        radius: f64,
    },
    StrokePath {
        d: String,
        color_id: u16,
        width: f64,
    },
    StrokePolyline {
        points: Vec<(f64, f64)>,
        color_id: u16,
        width: f64,
        dash: Option<String>,
    },
    FillPolygon {
        points: Vec<(f64, f64)>,
        color_id: u16,
    },
    DrawText {
        x: f64,
        y: f64,
        text: String,
        color_id: u16,
        anchor: u8,
        size: f64,
        weight: u16,
    },
    GroupStart {
        task_id: Option<String>,
    },
    GroupEnd,
    /// A dashed path: a styled dependency edge in the `delayed` state. Added
    /// last, so the ops before it keep their place in every serialised form.
    StrokeDashedPath {
        d: String,
        color_id: u16,
        width: f64,
        dash: String,
    },
    /// `FillRect` at an opacity below 1: the faint part of a bar a task row
    /// gives a colour. Added after the ops before it, so they keep their place
    /// in every serialised form.
    FillRectAlpha {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color_id: u16,
        radius: f64,
        alpha: f64,
    },
    /// `DrawText` over a halo: the band of `halo_color_id`, `halo_width` wide,
    /// is stroked round the glyphs before they are filled. The percent label
    /// inside a bar a task row gives a colour.
    DrawHaloText {
        x: f64,
        y: f64,
        text: String,
        color_id: u16,
        anchor: u8,
        size: f64,
        weight: u16,
        halo_color_id: u16,
        halo_width: f64,
    },
}
