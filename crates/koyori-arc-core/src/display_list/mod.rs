pub mod build;
pub mod constants;
pub mod legend;
pub(crate) mod rows;
pub mod text_metrics;
pub mod types;

#[cfg(test)]
mod external_rows_tests;
#[cfg(test)]
mod hidden_labels_tests;
#[cfg(test)]
mod hidden_legend_tests;
#[cfg(test)]
mod marks_tests;
#[cfg(test)]
mod right_edge_tests;
#[cfg(test)]
mod scale_tests;
#[cfg(test)]
mod view_range_tests;

pub use build::{build_display_list, build_display_list_with_rows, compute_row_window};
pub use types::{DisplayList, ScrollViewport};
