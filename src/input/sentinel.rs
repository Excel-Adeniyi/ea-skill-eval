use std::sync::LazyLock;

use regex::Regex;

/// The marker `skills/trace-evaluator/SKILL.md` instructs agents to emit.
pub const SENTINEL: &str = "<!-- skill: trace-evaluator v1 -->";

/// Matches the marker regardless of version, spacing or surrounding text.
///
/// Version-tolerant on purpose: bumping the skill to v2 should still be
/// recognised as "the skill fired", just a different revision of it.
static SENTINEL_PATTERN: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)<!--\s*skill:\s*trace-evaluator[^>]*-->").expect("sentinel pattern is valid")
});

/// Did this output come from an agent that loaded the skill?
///
/// This is the whole reason the skill emits a marker. Self-reported "yes I used
/// the skill" is not evidence, and most agent surfaces give no machine-readable
/// signal that an instruction file was read. A marker in the output is the one
/// thing that can be checked after the fact.
pub fn contains_sentinel(text: &str) -> bool {
    SENTINEL_PATTERN.is_match(text)
}

/// Remove the marker so it cannot leak into scoring.
///
/// Without this the sentinel's own words ("skill", "trace", "evaluator") would
/// enter the output's term set and quietly inflate overlap against any trace
/// whose instructions mention evaluation.
pub fn strip_sentinel(text: &str) -> String {
    SENTINEL_PATTERN.replace_all(text, "").trim().to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_the_canonical_marker() {
        assert!(contains_sentinel(SENTINEL));
        assert!(contains_sentinel(&format!("{SENTINEL}\n{{\"scores\":[]}}")));
    }

    #[test]
    fn detects_the_marker_across_versions_and_spacing() {
        assert!(contains_sentinel("<!--skill:trace-evaluator v2-->"));
        assert!(contains_sentinel("<!--   SKILL:  Trace-Evaluator  v9  -->"));
    }

    #[test]
    fn does_not_match_unrelated_comments() {
        assert!(!contains_sentinel("<!-- skill: something-else v1 -->"));
        assert!(!contains_sentinel("The capital of France is Paris."));
        assert!(!contains_sentinel(""));
    }

    #[test]
    fn strips_the_marker_and_surrounding_whitespace() {
        let output = format!("{SENTINEL}\n\nThe answer is Paris.");

        assert_eq!(strip_sentinel(&output), "The answer is Paris.");
    }

    #[test]
    fn leaves_output_without_a_marker_untouched() {
        assert_eq!(strip_sentinel("Paris."), "Paris.");
    }

    #[test]
    fn stripping_removes_every_occurrence() {
        let doubled = format!("{SENTINEL} middle {SENTINEL}");

        assert_eq!(strip_sentinel(&doubled), "middle");
    }
}
