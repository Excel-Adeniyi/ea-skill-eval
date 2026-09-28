use std::collections::HashSet;

/// Fraction of `reference` terms that also appear in `observed`.
///
/// Returns `None` when `reference` is empty, because "how much of nothing was
/// covered?" has no meaningful answer. Callers decide what to do with that:
/// skipping the metric is usually more honest than scoring it 0.0.
pub fn coverage(reference: &HashSet<String>, observed: &HashSet<String>) -> Option<f64> {
    if reference.is_empty() {
        return None;
    }

    let matched = reference.intersection(observed).count();

    Some(matched as f64 / reference.len() as f64)
}

/// Harmonic mean of precision and recall.
///
/// Used instead of a plain average because it punishes imbalance: an output
/// that echoes every instruction term but buries them in unrelated text should
/// not score well just because recall is perfect.
pub fn f1(precision: f64, recall: f64) -> f64 {
    let total = precision + recall;

    if total == 0.0 {
        return 0.0;
    }

    2.0 * precision * recall / total
}

#[cfg(test)]
mod tests {
    use super::*;

    fn terms(words: &[&str]) -> HashSet<String> {
        words.iter().map(|word| word.to_string()).collect()
    }

    #[test]
    fn full_coverage_scores_one() {
        let reference = terms(&["report", "save"]);
        let observed = terms(&["report", "save", "extra"]);

        assert_eq!(coverage(&reference, &observed), Some(1.0));
    }

    #[test]
    fn partial_coverage_scores_the_matching_fraction() {
        let reference = terms(&["report", "save", "json"]);
        let observed = terms(&["report", "save"]);

        assert_eq!(
            coverage(&reference, &observed),
            Some(2.0 / 3.0)
        );
    }

    #[test]
    fn no_shared_terms_scores_zero() {
        let reference = terms(&["report"]);
        let observed = terms(&["unrelated"]);

        assert_eq!(coverage(&reference, &observed), Some(0.0));
    }

    #[test]
    fn empty_reference_has_no_score() {
        let reference = terms(&[]);
        let observed = terms(&["report"]);

        assert_eq!(coverage(&reference, &observed), None);
    }

    #[test]
    fn coverage_is_directional() {
        let instructions = terms(&["report"]);
        let output = terms(&["report", "padding", "filler"]);

        // Every instruction term appears: recall is perfect.
        assert_eq!(coverage(&instructions, &output), Some(1.0));
        // But most of the output was not asked for: precision is poor.
        assert_eq!(coverage(&output, &instructions), Some(1.0 / 3.0));
    }

    #[test]
    fn f1_balances_precision_and_recall() {
        assert_eq!(f1(1.0, 1.0), 1.0);
        assert_eq!(f1(0.0, 0.0), 0.0);
        // Perfect recall with poor precision must not score highly.
        assert!(f1(0.25, 1.0) < 0.5);
    }
}
