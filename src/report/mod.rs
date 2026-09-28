mod json;
mod table;

pub use json::{JsonReport, build as build_json, render as render_json};
pub use table::{platforms, render_platform_comparison, render_traces, render_trigger_summary};
