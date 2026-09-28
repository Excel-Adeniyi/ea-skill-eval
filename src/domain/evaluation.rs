use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::{Metric, MetricScore, Platform, Prompting, Trace, TraceId};

/// One trace, as scored by one judge.
///
/// Carries the trace's provenance rather than a reference to the trace, so a
/// report serialises to JSON on its own without dragging the full instruction
/// and output text along with it.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TraceEvaluation {
    pub trace_id: TraceId,
    pub platform: Platform,
    pub model: Option<String>,
    pub prompting: Prompting,
    pub skill_triggered: Option<bool>,
    /// Which scorer actually produced these numbers. Not which one was asked:
    /// if a judge failed and the heuristic stepped in, this says "heuristic".
    pub judge: String,
    pub scores: Vec<MetricScore>,
    /// Why the requested judge was not used, when it wasn't.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

impl TraceEvaluation {
    pub fn new(
        trace: &Trace,
        judge: String,
        scores: Vec<MetricScore>,
        fallback_reason: Option<String>,
    ) -> Self {
        Self {
            trace_id: trace.id.clone(),
            platform: trace.platform,
            model: trace.model.clone(),
            prompting: trace.prompting,
            skill_triggered: trace.skill_triggered,
            judge,
            scores,
            fallback_reason,
        }
    }

    pub fn value_for(&self, metric: Metric) -> Option<f64> {
        self.scores
            .iter()
            .find(|entry| entry.metric == metric)
            .map(|entry| entry.score.value())
    }

    /// Mean across the metrics that applied. `None` when none did.
    pub fn mean(&self) -> Option<f64> {
        mean(self.scores.iter().map(|entry| entry.score.value()))
    }

    pub fn used_fallback(&self) -> bool {
        self.fallback_reason.is_some()
    }
}

/// Every evaluation from one run.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EvalReport {
    pub judge_requested: String,
    pub evaluations: Vec<TraceEvaluation>,
}

impl EvalReport {
    pub fn new(judge_requested: String, evaluations: Vec<TraceEvaluation>) -> Self {
        Self {
            judge_requested,
            evaluations,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.evaluations.is_empty()
    }

    /// Mean of one metric across every trace that scored it.
    pub fn metric_mean(&self, metric: Metric) -> Option<f64> {
        mean(
            self.evaluations
                .iter()
                .filter_map(|evaluation| evaluation.value_for(metric)),
        )
    }

    /// Mean of each trace's own mean — so a trace with four applicable metrics
    /// counts the same as one with five, rather than being weighted down.
    pub fn overall_mean(&self) -> Option<f64> {
        mean(self.evaluations.iter().filter_map(|e| e.mean()))
    }

    /// Evaluations grouped by platform, ordered for stable report rows.
    pub fn by_platform(&self) -> BTreeMap<Platform, Vec<&TraceEvaluation>> {
        let mut grouped: BTreeMap<Platform, Vec<&TraceEvaluation>> = BTreeMap::new();

        for evaluation in &self.evaluations {
            grouped
                .entry(evaluation.platform)
                .or_default()
                .push(evaluation);
        }

        grouped
    }

    pub fn platform_metric_mean(&self, platform: Platform, metric: Metric) -> Option<f64> {
        mean(
            self.evaluations
                .iter()
                .filter(|evaluation| evaluation.platform == platform)
                .filter_map(|evaluation| evaluation.value_for(metric)),
        )
    }

    pub fn platform_mean(&self, platform: Platform) -> Option<f64> {
        mean(
            self.evaluations
                .iter()
                .filter(|evaluation| evaluation.platform == platform)
                .filter_map(|evaluation| evaluation.mean()),
        )
    }

    /// Traces whose mean fell below `threshold`. Drives the exit code.
    pub fn below_threshold(&self, threshold: f64) -> Vec<&TraceEvaluation> {
        self.evaluations
            .iter()
            .filter(|evaluation| evaluation.mean().is_some_and(|value| value < threshold))
            .collect()
    }

    pub fn fallback_count(&self) -> usize {
        self.evaluations
            .iter()
            .filter(|evaluation| evaluation.used_fallback())
            .count()
    }

    /// How often the skill fired, among traces where it was recorded.
    /// Returns `(fired, recorded)` — the Day 3 implicit-triggering number.
    pub fn trigger_rate(&self, prompting: Prompting) -> (usize, usize) {
        let recorded: Vec<bool> = self
            .evaluations
            .iter()
            .filter(|evaluation| evaluation.prompting == prompting)
            .filter_map(|evaluation| evaluation.skill_triggered)
            .collect();

        (
            recorded.iter().filter(|fired| **fired).count(),
            recorded.len(),
        )
    }
}

/// Mean of an iterator, or `None` when it is empty.
///
/// Every aggregate above funnels through here so that "no data" stays distinct
/// from "scored zero" all the way to the report.
fn mean(values: impl Iterator<Item = f64>) -> Option<f64> {
    let collected: Vec<f64> = values.collect();

    if collected.is_empty() {
        return None;
    }

    Some(collected.iter().sum::<f64>() / collected.len() as f64)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Score, ScoreSource};

    fn trace(id: &str, platform: Platform) -> Trace {
        Trace {
            id: TraceId::new(id.to_string()),
            instructions: "i".to_string(),
            task: "t".to_string(),
            output: "o".to_string(),
            platform,
            model: None,
            prompting: Prompting::Explicit,
            skill_triggered: None,
        }
    }

    fn scored(values: &[(Metric, f64)]) -> Vec<MetricScore> {
        values
            .iter()
            .map(|(metric, value)| {
                MetricScore::new(
                    *metric,
                    Score::new(*value).expect("test value should be valid"),
                    ScoreSource::LlmJudge,
                    String::new(),
                )
            })
            .collect()
    }

    fn report() -> EvalReport {
        EvalReport::new(
            "llm:test".to_string(),
            vec![
                TraceEvaluation::new(
                    &trace("a", Platform::ClaudeCode),
                    "llm:test".to_string(),
                    scored(&[
                        (Metric::InstructionAdherence, 1.0),
                        (Metric::TaskRelevancy, 0.8),
                    ]),
                    None,
                ),
                TraceEvaluation::new(
                    &trace("b", Platform::CodexCli),
                    "llm:test".to_string(),
                    scored(&[
                        (Metric::InstructionAdherence, 0.4),
                        (Metric::TaskRelevancy, 0.2),
                    ]),
                    None,
                ),
            ],
        )
    }

    #[test]
    fn metric_mean_averages_across_traces() {
        assert_eq!(
            report().metric_mean(Metric::InstructionAdherence),
            Some(0.7)
        );
    }

    #[test]
    fn metric_mean_is_none_when_no_trace_scored_it() {
        assert_eq!(report().metric_mean(Metric::FormatCompliance), None);
    }

    #[test]
    fn overall_mean_averages_per_trace_not_per_score() {
        // Trace means are 0.9 and 0.3, so the overall mean is 0.6.
        // Compared with a tolerance: 0.9 and 0.3 are not exactly representable
        // in binary floating point, so the sum lands a hair off 0.6.
        let overall = report().overall_mean().expect("report has scores");

        assert!((overall - 0.6).abs() < 1e-9, "expected ~0.6, got {overall}");
    }

    #[test]
    fn groups_evaluations_by_platform() {
        let report = report();
        let grouped = report.by_platform();

        assert_eq!(grouped.len(), 2);
        assert_eq!(grouped[&Platform::ClaudeCode].len(), 1);
        assert_eq!(grouped[&Platform::CodexCli].len(), 1);
    }

    #[test]
    fn compares_platforms_on_one_metric() {
        let report = report();

        assert_eq!(
            report.platform_metric_mean(Platform::ClaudeCode, Metric::TaskRelevancy),
            Some(0.8)
        );
        assert_eq!(
            report.platform_metric_mean(Platform::CodexCli, Metric::TaskRelevancy),
            Some(0.2)
        );
    }

    #[test]
    fn finds_traces_below_a_threshold() {
        let report = report();
        let failing = report.below_threshold(0.5);

        assert_eq!(failing.len(), 1);
        assert_eq!(failing[0].trace_id.as_str(), "b");
    }

    #[test]
    fn no_traces_fail_a_threshold_of_zero() {
        let report = report();

        assert!(report.below_threshold(0.0).is_empty());
    }

    #[test]
    fn counts_fallbacks_separately_from_successes() {
        let mut report = report();
        report.evaluations[0].fallback_reason = Some("judge unreachable".to_string());

        assert_eq!(report.fallback_count(), 1);
        assert!(report.evaluations[0].used_fallback());
        assert!(!report.evaluations[1].used_fallback());
    }

    #[test]
    fn trigger_rate_counts_only_recorded_traces() {
        let mut report = report();
        report.evaluations[0].skill_triggered = Some(true);
        report.evaluations[1].skill_triggered = None;

        assert_eq!(report.trigger_rate(Prompting::Explicit), (1, 1));
    }

    #[test]
    fn an_empty_report_has_no_means() {
        let empty = EvalReport::new("heuristic".to_string(), Vec::new());

        assert!(empty.is_empty());
        assert_eq!(empty.overall_mean(), None);
    }
}
