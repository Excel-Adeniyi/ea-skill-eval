use anyhow::{Context, Result};
use serde::Serialize;

use crate::domain::{EvalReport, Metric, Platform, Prompting};

/// Machine-readable form of a run, for CI and for diffing between runs.
///
/// A separate shape from [`EvalReport`] on purpose: the aggregates are computed
/// once and written down, so a consumer reading this file does not have to
/// reimplement the averaging rules to get the same numbers.
#[derive(Debug, Serialize)]
pub struct JsonReport<'a> {
    pub judge_requested: &'a str,
    pub trace_count: usize,
    pub fallback_count: usize,
    pub overall_mean: Option<f64>,
    pub metric_means: Vec<MetricMean>,
    pub platforms: Vec<PlatformSummary>,
    pub triggering: Vec<TriggerSummary>,
    pub evaluations: &'a [crate::domain::TraceEvaluation],
}

#[derive(Debug, Serialize)]
pub struct MetricMean {
    pub metric: Metric,
    pub mean: Option<f64>,
}

#[derive(Debug, Serialize)]
pub struct PlatformSummary {
    pub platform: Platform,
    pub trace_count: usize,
    pub mean: Option<f64>,
    pub metric_means: Vec<MetricMean>,
}

#[derive(Debug, Serialize)]
pub struct TriggerSummary {
    pub prompting: Prompting,
    pub skill_fired: usize,
    pub recorded: usize,
}

pub fn build<'a>(report: &'a EvalReport) -> JsonReport<'a> {
    let metric_means: Vec<MetricMean> = Metric::all()
        .iter()
        .map(|metric| MetricMean {
            metric: *metric,
            mean: report.metric_mean(*metric),
        })
        .collect();

    let platforms: Vec<PlatformSummary> = report
        .by_platform()
        .into_iter()
        .map(|(platform, evaluations)| PlatformSummary {
            platform,
            trace_count: evaluations.len(),
            mean: report.platform_mean(platform),
            metric_means: Metric::all()
                .iter()
                .map(|metric| MetricMean {
                    metric: *metric,
                    mean: report.platform_metric_mean(platform, *metric),
                })
                .collect(),
        })
        .collect();

    let triggering: Vec<TriggerSummary> = [Prompting::Explicit, Prompting::Implicit]
        .into_iter()
        .map(|prompting| {
            let (skill_fired, recorded) = report.trigger_rate(prompting);
            TriggerSummary {
                prompting,
                skill_fired,
                recorded,
            }
        })
        .filter(|summary| summary.recorded > 0)
        .collect();

    JsonReport {
        judge_requested: &report.judge_requested,
        trace_count: report.evaluations.len(),
        fallback_count: report.fallback_count(),
        overall_mean: report.overall_mean(),
        metric_means,
        platforms,
        triggering,
        evaluations: &report.evaluations,
    }
}

pub fn render(report: &EvalReport) -> Result<String> {
    serde_json::to_string_pretty(&build(report)).context("could not serialise the report as JSON")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{MetricScore, Score, ScoreSource, Trace, TraceEvaluation, TraceId};

    fn report() -> EvalReport {
        let trace = Trace {
            id: TraceId::new("a".to_string()),
            instructions: "i".to_string(),
            task: "t".to_string(),
            output: "o".to_string(),
            platform: Platform::ClaudeCode,
            model: Some("claude-opus-5".to_string()),
            prompting: Prompting::Explicit,
            skill_triggered: Some(true),
        };

        let scores = vec![MetricScore::new(
            Metric::TaskRelevancy,
            Score::new(0.75).expect("valid"),
            ScoreSource::LlmJudge,
            "on topic".to_string(),
        )];

        EvalReport::new(
            "llm:test".to_string(),
            vec![TraceEvaluation::new(
                &trace,
                "llm:test".to_string(),
                scores,
                None,
            )],
        )
    }

    #[test]
    fn renders_valid_json() {
        let rendered = render(&report()).expect("should render");
        let parsed: serde_json::Value =
            serde_json::from_str(&rendered).expect("output should be valid JSON");

        assert_eq!(parsed["trace_count"], 1);
        assert_eq!(parsed["judge_requested"], "llm:test");
    }

    #[test]
    fn includes_precomputed_aggregates() {
        let parsed: serde_json::Value = serde_json::from_str(&render(&report()).unwrap()).unwrap();

        assert_eq!(parsed["overall_mean"], 0.75);
        assert_eq!(parsed["platforms"][0]["platform"], "claude_code");
        assert_eq!(parsed["platforms"][0]["mean"], 0.75);
    }

    #[test]
    fn unscored_metrics_serialise_as_null_not_zero() {
        let parsed: serde_json::Value = serde_json::from_str(&render(&report()).unwrap()).unwrap();
        let adherence = parsed["metric_means"]
            .as_array()
            .unwrap()
            .iter()
            .find(|entry| entry["metric"] == "InstructionAdherence")
            .expect("metric should be listed");

        assert!(adherence["mean"].is_null());
    }

    #[test]
    fn carries_provenance_through_to_the_evaluations() {
        let parsed: serde_json::Value = serde_json::from_str(&render(&report()).unwrap()).unwrap();

        assert_eq!(parsed["evaluations"][0]["platform"], "claude_code");
        assert_eq!(parsed["evaluations"][0]["model"], "claude-opus-5");
        assert_eq!(parsed["evaluations"][0]["skill_triggered"], true);
    }

    #[test]
    fn reports_trigger_counts() {
        let parsed: serde_json::Value = serde_json::from_str(&render(&report()).unwrap()).unwrap();

        assert_eq!(parsed["triggering"][0]["prompting"], "explicit");
        assert_eq!(parsed["triggering"][0]["skill_fired"], 1);
        assert_eq!(parsed["triggering"][0]["recorded"], 1);
    }
}
