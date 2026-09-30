mod sentinel;
mod trace_loader;

pub use sentinel::{SENTINEL, contains_sentinel, strip_sentinel};
pub use trace_loader::load_traces;
