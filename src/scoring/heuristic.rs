use std::collections::HashSet;

use crate::domain::{Metric, MetricScore, Score, ScoreSource, Trace};
use crate::scoring::format::{detect_rules, FormatRule};
use crate::scoring::overlap::{coverage, f1};
use crate::scoring::tokenizer::extract_terms;

/// A trace's text, tokenised once and reused by every overlap-based metric.
///
/// Tokenising is the expensive part of the heuristic path, and four of the five
/// metrics need the same three term sets. Computing them once here keeps each
/// metric function a pure, cheap calculation over sets.
#[derive(Debug, Clone)]
pub struct TraceTerms {
    pub instructions: HashSet<String>,
    pub task: HashSet<String>,
    pub output: HashSet<String>,
    /// Instructions and task combined: everything the model was actually given.
    /// Output terms found here are grounded; anything else the model invented.
    pub grounded: HashSet<String>,
}

impl TraceTerms {
    pub fn from_trace(trace: &Trace) -> Self {
        let instructions = extract_terms(&trace.instructions);
        let task = extract_terms(&trace.task);
        let output = extract_terms(&trace.output);
        let grounded = instructions.union(&task).cloned().collect();

        Self {
            instructions,
            task,
            output,
            grounded,
        }
    }
}

/// Score every metric that applies to `trace`, in `Metric::all()` order.
///
/// Metrics with nothing to measure are omitted rather than scored zero: a trace
/// whose instructions state no format requirement has not *failed* format
/// compliance, so reporting 0.0 would be a lie.
pub fn evaluate_trace(trace: &Trace) -> Vec<MetricScore> {
    let terms = TraceTerms::from_trace(trace);

    [
        adherence(&terms),
        task_relevancy(&terms),
        precision(&terms),
        recall(&terms),
        format_compliance(trace),
    ]
    .into_iter()
    .flatten()
    .collect()
}

/// How much of what the instructions asked for actually appears in the output.
pub fn recall(terms: &TraceTerms) -> Option<MetricScore> {
    let value = coverage(&terms.instructions, &terms.output)?;
    let missing = missing_terms(&terms.instructions, &terms.output);

    let reasoning = if missing.is_empty() {
        "Every instruction term appears in the output.".to_string()
    } else {
        format!(
            "{} of {} instruction terms present; missing: {}.",
            terms.instructions.len() - missing.len(),
            terms.instructions.len(),
            truncate_list(&missing, 8)
        )
    };

    Some(metric_score(Metric::InstructionRecall, value, reasoning))
}

/// How much of the output traces back to the instructions or the task.
///
/// The mirror image of recall: this is the metric that penalises padding,
/// digression and invented detail.
pub fn precision(terms: &TraceTerms) -> Option<MetricScore> {
    let value = coverage(&terms.output, &terms.grounded)?;
    let ungrounded = missing_terms(&terms.output, &terms.grounded);

    let reasoning = if ungrounded.is_empty() {
        "Every output term traces back to the instructions or task.".to_string()
    } else {
        format!(
            "{} of {} output terms are grounded; unprompted: {}.",
            terms.output.len() - ungrounded.len(),
            terms.output.len(),
            truncate_list(&ungrounded, 8)
        )
    };

    Some(metric_score(Metric::InstructionPrecision, value, reasoning))
}

/// Overall instruction-following, as the harmonic mean of precision and recall.
///
/// A harmonic mean rather than a plain average, so that one strong half cannot
/// hide a weak one: echoing every instruction inside a wall of unrelated text
/// should not score well.
pub fn adherence(terms: &TraceTerms) -> Option<MetricScore> {
    let precision_value = coverage(&terms.output, &terms.grounded)?;
    let recall_value = coverage(&terms.instructions, &terms.output)?;
    let value = f1(precision_value, recall_value);

    Some(metric_score(
        Metric::InstructionAdherence,
        value,
        format!("Harmonic mean of precision ({precision_value:.2}) and recall ({recall_value:.2})."),
    ))
}

/// How much of the task the output actually engages with.
pub fn task_relevancy(terms: &TraceTerms) -> Option<MetricScore> {
    let value = coverage(&terms.task, &terms.output)?;
    let missing = missing_terms(&terms.task, &terms.output);

    let reasoning = if missing.is_empty() {
        "The output addresses every term in the task.".to_string()
    } else {
        format!(
            "Task terms absent from the output: {}.",
            truncate_list(&missing, 8)
        )
    };

    Some(metric_score(Metric::TaskRelevancy, value, reasoning))
}

/// The fraction of stated formatting requirements the output satisfies.
///
/// Takes the whole `Trace` rather than [`TraceTerms`] because formatting lives
/// in the raw text — bullets, fences and word counts vanish once tokenised.
pub fn format_compliance(trace: &Trace) -> Option<MetricScore> {
    let rules = detect_rules(&trace.instructions);

    if rules.is_empty() {
        return None;
    }

    let (satisfied, broken): (Vec<FormatRule>, Vec<FormatRule>) = rules
        .iter()
        .partition(|rule| rule.is_satisfied_by(&trace.output));

    let value = satisfied.len() as f64 / rules.len() as f64;

    let reasoning = if broken.is_empty() {
        format!("Output satisfies all {} format requirement(s).", rules.len())
    } else {
        let unmet: Vec<String> = broken.iter().map(|rule| rule.describe()).collect();
        format!(
            "{} of {} format requirement(s) met; missing: {}.",
            satisfied.len(),
            rules.len(),
            unmet.join(", ")
        )
    };

    Some(metric_score(Metric::FormatCompliance, value, reasoning))
}

/// Terms in `reference` absent from `observed`, sorted so output is stable.
///
/// `HashSet` iteration order is deliberately randomised in Rust, so without the
/// sort the same trace would produce differently-worded reasoning each run.
fn missing_terms(reference: &HashSet<String>, observed: &HashSet<String>) -> Vec<String> {
    let mut missing: Vec<String> = reference.difference(observed).cloned().collect();
    missing.sort();
    missing
}

fn truncate_list(items: &[String], limit: usize) -> String {
    if items.len() <= limit {
        return items.join(", ");
    }

    format!(
        "{}, and {} more",
        items[..limit].join(", "),
        items.len() - limit
    )
}

/// Every heuristic above divides a count by a positive count, or feeds two such
/// ratios to `f1`, so the value is always finite and within 0.0..=1.0. A panic
/// here means a scoring bug, not bad input.
fn metric_score(metric: Metric, value: f64, reasoning: String) -> MetricScore {
    let score = Score::new(value)
        .unwrap_or_else(|error| panic!("heuristic for {metric} produced an invalid score: {error}"));

    MetricScore::new(metric, score, ScoreSource::Heuristic, reasoning)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Platform, Prompting, TraceId};

    fn trace(instructions: &str, task: &str, output: &str) -> Trace {
        Trace {
            id: TraceId::new("test".to_string()),
            instructions: instructions.to_string(),
            task: task.to_string(),
            output: output.to_string(),
            platform: Platform::Unknown,
            model: None,
            prompting: Prompting::Explicit,
            skill_triggered: None,
        }
    }

    fn score_for(scores: &[MetricScore], metric: Metric) -> Option<f64> {
        scores
            .iter()
            .find(|entry| entry.metric == metric)
            .map(|entry| entry.score.value())
    }

    #[test]
    fn scores_all_five_metrics_when_a_format_is_requested() {
        let scores = evaluate_trace(&trace(
            "Reply as JSON.",
            "List two colours.",
            r#"{"colours": ["red", "blue"]}"#,
        ));

        assert_eq!(scores.len(), 5);
    }

    #[test]
    fn returns_metrics_in_a_stable_order() {
        let scores = evaluate_trace(&trace(
            "Reply as JSON.",
            "List colours.",
            r#"{"colours": []}"#,
        ));
        let order: Vec<Metric> = scores.iter().map(|entry| entry.metric).collect();

        assert_eq!(order, Metric::all().to_vec());
    }

    #[test]
    fn skips_format_compliance_when_no_format_was_requested() {
        let scores = evaluate_trace(&trace(
            "Be helpful.",
            "Explain structs.",
            "A struct groups values.",
        ));

        assert_eq!(score_for(&scores, Metric::FormatCompliance), None);
        assert_eq!(scores.len(), 4);
    }

    #[test]
    fn perfect_recall_when_the_output_covers_every_instruction_term() {
        // "and" is a stopword, so the instruction terms are {cover, ownership,
        // borrowing} — all three appear in the output.
        let terms = TraceTerms::from_trace(&trace(
            "Cover ownership and borrowing.",
            "Explain memory.",
            "Ownership and borrowing cover Rust memory.",
        ));

        assert_eq!(recall(&terms).unwrap().score.value(), 1.0);
    }

    #[test]
    fn recall_drops_for_each_instruction_term_the_output_omits() {
        // Instruction terms are {mention, ownership, borrowing}; the output has
        // two of the three, so recall is 2/3 rather than 1.0.
        let terms = TraceTerms::from_trace(&trace(
            "Mention ownership and borrowing.",
            "Explain memory.",
            "Rust memory uses ownership and borrowing.",
        ));

        assert_eq!(recall(&terms).unwrap().score.value(), 2.0 / 3.0);
    }

    #[test]
    fn zero_recall_when_the_output_ignores_the_instructions() {
        let terms = TraceTerms::from_trace(&trace(
            "Mention ownership.",
            "Explain memory.",
            "Bananas are yellow.",
        ));

        assert_eq!(recall(&terms).unwrap().score.value(), 0.0);
    }

    #[test]
    fn precision_falls_when_the_output_adds_unprompted_content() {
        let focused = TraceTerms::from_trace(&trace(
            "Mention ownership.",
            "Explain memory.",
            "Ownership explains memory.",
        ));
        let padded = TraceTerms::from_trace(&trace(
            "Mention ownership.",
            "Explain memory.",
            "Ownership explains memory. Also here are unrelated thoughts about \
             weather, cooking, football, and holidays.",
        ));

        let focused_precision = precision(&focused).unwrap().score.value();
        let padded_precision = precision(&padded).unwrap().score.value();

        assert!(
            padded_precision < focused_precision,
            "padding should lower precision: {padded_precision} vs {focused_precision}"
        );
    }

    #[test]
    fn adherence_sits_between_precision_and_recall() {
        let terms = TraceTerms::from_trace(&trace(
            "Mention ownership and borrowing.",
            "Explain memory.",
            "Ownership and borrowing matter. Unrelated: weather, cooking, football.",
        ));

        let precision_value = precision(&terms).unwrap().score.value();
        let recall_value = recall(&terms).unwrap().score.value();
        let adherence_value = adherence(&terms).unwrap().score.value();
        let low = precision_value.min(recall_value);
        let high = precision_value.max(recall_value);

        assert!(
            adherence_value >= low && adherence_value <= high,
            "F1 {adherence_value} should lie within {low}..={high}"
        );
    }

    #[test]
    fn task_relevancy_rewards_addressing_the_task() {
        let on_topic = TraceTerms::from_trace(&trace(
            "Be brief.",
            "Explain Rust enums.",
            "Rust enums represent one of several variants.",
        ));
        let off_topic = TraceTerms::from_trace(&trace(
            "Be brief.",
            "Explain Rust enums.",
            "The weather is pleasant today.",
        ));

        assert!(
            task_relevancy(&on_topic).unwrap().score.value()
                > task_relevancy(&off_topic).unwrap().score.value()
        );
    }

    #[test]
    fn format_compliance_scores_the_fraction_of_rules_met() {
        // Asks for JSON and bullets; the output is valid JSON but has no bullets.
        let scored = format_compliance(&trace(
            "Reply as JSON using bullet points.",
            "List colours.",
            r#"{"colours": ["red"]}"#,
        ));

        assert_eq!(scored.unwrap().score.value(), 0.5);
    }

    #[test]
    fn every_score_is_tagged_as_heuristic_and_explained() {
        let scores = evaluate_trace(&trace("Reply as JSON.", "List colours.", "nope"));

        assert!(scores
            .iter()
            .all(|entry| entry.source == ScoreSource::Heuristic));
        assert!(scores.iter().all(|entry| !entry.reasoning.is_empty()));
    }
}
