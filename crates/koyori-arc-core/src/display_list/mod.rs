pub mod build;
pub mod constants;
pub mod legend;
pub mod text_metrics;
pub mod types;

#[cfg(test)]
mod hidden_legend_tests;
#[cfg(test)]
mod right_edge_tests;

pub use build::{build_display_list, compute_row_window};
pub use types::{DisplayList, ScrollViewport};
