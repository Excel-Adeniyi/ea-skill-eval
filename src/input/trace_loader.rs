use anyhow::Result;
use std::fs;

use crate::Trace;

pub fn load_traces(path: &str) -> Result<Vec<Trace>> {
    let json = fs::read_to_string(path)?;
    let traces = serde_json::from_str(&json)?;

    Ok(traces)
}
