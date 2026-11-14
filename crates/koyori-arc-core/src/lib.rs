mod backend;
mod date_input;
mod display_list;
mod error;
mod graph;
mod layout;
mod progress;
mod render;

#[doc(hidden)]
pub mod bench_fixtures;

pub use backend::svg::empty_svg;
pub use backend::{
    BackendOutput, CanvasBackend, CommandBuffer, DrawOp, NativeBackend, NativeDrawList,
    NativeDrawOp, RenderBackend, SvgBackend,
};
pub use display_list::constants::{DOM_CAP, HEADER_H, ROW_H};
pub use display_list::legend::{legend_spec, LegendProgressLine, LegendSpec, LegendTier};
pub use display_list::types::{
    BBox, BarAlign, ChartMetadata, ChartOptions, ColorId, Coord, GroupPrim, LabelMode, Layer,
    LayerKind, LegendMode, LinePrim, LineSemantic, Palette, PathPrim, PathSemantic, PolygonPrim,
    PolygonSemantic, PolylinePrim, PolylineSemantic, Primitive, ProgressLineMode, ProgressTier,
    RectPrim, RectSemantic, RoundRectPrim, RoundRectSemantic, Scale, TaskBBox, TextAnchor,
    TextBaseline, TextPrim, TextSemantic, ViewRange, Viewport, WeekendDays,
};
pub use display_list::{
    build_display_list, build_display_list_with_rows, compute_row_window, DisplayList,
    ScrollViewport,
};
pub use error::{
    RenderError, CODE_CANVAS_CAPACITY, CODE_INPUT_LIMIT, CODE_PARSE_ERROR, CODE_SERIALIZE_ERROR,
};
pub use graph::{BarShape, BarState, GanttDep, GanttGraph, GanttRow, GanttTask, TaskRow};
pub use render::{
    empty_svg_markup, legend_spec_json, render, render_canvas, render_canvas_commands,
    render_canvas_with_rows, render_svg, render_svg_error, render_with_rows,
};
