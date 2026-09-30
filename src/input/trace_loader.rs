use std::fs;

use anyhow::Result;

use crate::Trace;
use crate::input::sentinel::{contains_sentinel, strip_sentinel};

/// Load traces from a JSON file, resolving the skill sentinel as we go.
///
/// Two things happen to each trace on the way in:
///
/// 1. If the capture did not record `skill_triggered`, it is inferred from
///    whether the output carries the sentinel marker. An explicit value in the
///    file always wins — the person who watched the run knows more than a
///    regular expression does.
/// 2. The marker is stripped from the output either way, so it cannot leak into
///    term-overlap scoring.
pub fn load_traces(path: &str) -> Result<Vec<Trace>> {
    let json = fs::read_to_string(path)?;
    let mut traces: Vec<Trace> = serde_json::from_str(&json)?;

    for trace in &mut traces {
        if trace.skill_triggered.is_none() {
            trace.skill_triggered = Some(contains_sentinel(&trace.output));
        }

        trace.output = strip_sentinel(&trace.output);
    }

    Ok(traces)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::input::sentinel::SENTINEL;

    fn write(name: &str, body: &str) -> String {
        let path = std::env::temp_dir().join(name);
        fs::write(&path, body).expect("fixture should be writable");
        path.to_string_lossy().to_string()
    }

    #[test]
    fn infers_skill_triggered_from_the_sentinel() {
        let path = write(
            "loader_sentinel.json",
            &format!(
                r#"[{{"id":"a","instructions":"i","task":"t","output":"{SENTINEL} Paris."}}]"#
            ),
        );

        let traces = load_traces(&path).expect("should load");

        assert_eq!(traces[0].skill_triggered, Some(true));
        assert_eq!(traces[0].output, "Paris.");
    }

    #[test]
    fn records_a_missing_sentinel_as_not_triggered() {
        let path = write(
            "loader_no_sentinel.json",
            r#"[{"id":"a","instructions":"i","task":"t","output":"Paris."}]"#,
        );

        let traces = load_traces(&path).expect("should load");

        assert_eq!(traces[0].skill_triggered, Some(false));
    }

    #[test]
    fn an_explicit_value_overrides_the_sentinel() {
        // The capture said the skill did not fire, even though the output
        // carries a marker. Trust the person who watched the run.
        let path = write(
            "loader_explicit.json",
            &format!(
                r#"[{{"id":"a","instructions":"i","task":"t","output":"{SENTINEL} Paris.","skill_triggered":false}}]"#
            ),
        );

        let traces = load_traces(&path).expect("should load");

        assert_eq!(traces[0].skill_triggered, Some(false));
        // Stripping still happens, so scoring is unaffected either way.
        assert_eq!(traces[0].output, "Paris.");
    }

    #[test]
    fn reports_a_missing_file_as_an_error() {
        assert!(load_traces("definitely-not-here.json").is_err());
    }
}
