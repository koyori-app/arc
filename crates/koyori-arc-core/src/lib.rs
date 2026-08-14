mod backend;
mod display_list;
mod error;
mod graph;
mod layout;
mod progress;
mod render;

#[doc(hidden)]
pub mod bench_fixtures;

pub use backend::{
    BackendOutput, CanvasBackend, CommandBuffer, DrawOp, NativeBackend, NativeDrawList,
    NativeDrawOp, RenderBackend, SvgBackend,
};
pub use backend::svg::empty_svg;
pub use display_list::constants::{DOM_CAP, HEADER_H, ROW_H};
pub use display_list::{build_display_list, compute_row_window, DisplayList, ScrollViewport};
pub use error::{
    RenderError, CODE_CANVAS_CAPACITY, CODE_INPUT_LIMIT, CODE_PARSE_ERROR, CODE_SERIALIZE_ERROR,
};
pub use graph::{GanttDep, GanttGraph, GanttTask};
pub use render::{
    empty_svg_markup, render, render_canvas, render_canvas_commands, render_svg, render_svg_error,
};
