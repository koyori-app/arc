use crate::display_list::DisplayList;

pub enum BackendOutput {
    Svg(String),
    CanvasCommands(super::command_buffer::CommandBuffer),
    NativeDrawList(NativeDrawList),
}

pub trait RenderBackend {
    fn render(&self, list: &DisplayList) -> BackendOutput;
    fn name(&self) -> &'static str;
}

#[derive(Debug, Clone, PartialEq)]
pub struct NativeDrawList {
    pub viewport_width: f64,
    pub viewport_height: f64,
    pub ops: Vec<NativeDrawOp>,
    pub palette_refs: Vec<(u16, String)>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum NativeDrawOp {
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
    /// `FillRect` at an opacity below 1 (`DrawOp::FillRectAlpha`).
    FillRectAlpha {
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        color_id: u16,
        radius: f64,
        alpha: f64,
    },
    /// `DrawText` over a halo (`DrawOp::DrawHaloText`).
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
