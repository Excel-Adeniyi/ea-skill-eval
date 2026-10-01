use comfy_table::{Attribute, Cell, CellAlignment, Color, Table, presets::UTF8_FULL};

use crate::domain::{EvalReport, Metric, Platform, Prompting};

/// Metrics that at least one trace in this report actually scored.
///
/// A metric no scorer produced would otherwise render as a column of dashes —
/// `Correctness` needs a reference document, so it is absent from any run that
/// did not use the claim verifier. Showing an empty column invites the reader
/// to wonder what went wrong when nothing did.
fn measured_metrics(report: &EvalReport) -> Vec<Metric> {
    Metric::all()
        .into_iter()
        .filter(|metric| report.metric_mean(*metric).is_some())
        .collect()
}

/// Per-trace scores, one row per trace and one column per metric.
pub fn render_traces(report: &EvalReport) -> String {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);

    let metrics = measured_metrics(report);

    let mut header: Vec<Cell> = vec![header_cell("Trace"), header_cell("Platform")];
    header.extend(
        metrics
            .iter()
            .map(|metric| header_cell(&abbreviate(*metric))),
    );
    header.push(header_cell("Mean"));
    header.push(header_cell("Judge"));
    table.set_header(header);

    for evaluation in &report.evaluations {
        let mut row: Vec<Cell> = vec![
            Cell::new(evaluation.trace_id.as_str()),
            Cell::new(evaluation.platform.to_string()),
        ];

        for metric in &metrics {
            row.push(score_cell(evaluation.value_for(*metric)));
        }

        row.push(score_cell(evaluation.mean()).add_attribute(Attribute::Bold));

        // A judge that fell back is flagged in the row itself, so a reader
        // never mistakes heuristic numbers for model numbers.
        row.push(if evaluation.used_fallback() {
            Cell::new(format!("{} (fallback)", evaluation.judge)).fg(Color::Yellow)
        } else {
            Cell::new(&evaluation.judge)
        });

        table.add_row(row);
    }

    table.to_string()
}

/// Platform means, one row per platform — the side-by-side comparison.
pub fn render_platform_comparison(report: &EvalReport) -> String {
    let mut table = Table::new();
    table.load_style(UTF8_FULL);

    let metrics = measured_metrics(report);

    let mut header: Vec<Cell> = vec![header_cell("Platform"), header_cell("Traces")];
    header.extend(
        metrics
            .iter()
            .map(|metric| header_cell(&abbreviate(*metric))),
    );
    header.push(header_cell("Mean"));
    table.set_header(header);

    for (platform, evaluations) in report.by_platform() {
        let mut row: Vec<Cell> = vec![
            Cell::new(platform.to_string()),
            Cell::new(evaluations.len()).set_alignment(CellAlignment::Right),
        ];

        for metric in &metrics {
            row.push(score_cell(report.platform_metric_mean(platform, *metric)));
        }

        row.push(score_cell(report.platform_mean(platform)).add_attribute(Attribute::Bold));
        table.add_row(row);
    }

    table.to_string()
}

/// How often the skill fired, split by how it was prompted for.
///
/// Returns `None` when no trace recorded `skill_triggered` — printing an
/// all-zeroes table would read as "the skill never fired" rather than
/// "nobody wrote it down".
pub fn render_trigger_summary(report: &EvalReport) -> Option<String> {
    let rows: Vec<(Prompting, usize, usize)> = [Prompting::Explicit, Prompting::Implicit]
        .into_iter()
        .map(|prompting| {
            let (fired, recorded) = report.trigger_rate(prompting);
            (prompting, fired, recorded)
        })
        .filter(|(_, _, recorded)| *recorded > 0)
        .collect();

    if rows.is_empty() {
        return None;
    }

    let mut table = Table::new();
    table.load_style(UTF8_FULL);
    table.set_header(vec![
        header_cell("Prompting"),
        header_cell("Skill fired"),
        header_cell("Recorded"),
        header_cell("Rate"),
    ]);

    for (prompting, fired, recorded) in rows {
        table.add_row(vec![
            Cell::new(prompting.to_string()),
            Cell::new(fired).set_alignment(CellAlignment::Right),
            Cell::new(recorded).set_alignment(CellAlignment::Right),
            Cell::new(format!("{:.0}%", 100.0 * fired as f64 / recorded as f64))
                .set_alignment(CellAlignment::Right),
        ]);
    }

    Some(table.to_string())
}

fn header_cell(text: &str) -> Cell {
    Cell::new(text).add_attribute(Attribute::Bold)
}

/// A score cell, colour-banded, or "-" when the metric did not apply.
fn score_cell(value: Option<f64>) -> Cell {
    match value {
        None => Cell::new("-")
            .set_alignment(CellAlignment::Right)
            .fg(Color::DarkGrey),
        Some(value) => Cell::new(format!("{value:.2}"))
            .set_alignment(CellAlignment::Right)
            .fg(band(value)),
    }
}

fn band(value: f64) -> Color {
    if value >= 0.8 {
        Color::Green
    } else if value >= 0.5 {
        Color::Yellow
    } else {
        Color::Red
    }
}

/// Metric names are too wide for a five-column table at terminal width.
fn abbreviate(metric: Metric) -> String {
    match metric {
        Metric::Correctness => "Correct",
        Metric::InstructionAdherence => "Adher",
        Metric::TaskRelevancy => "Relev",
        Metric::InstructionPrecision => "Prec",
        Metric::InstructionRecall => "Recall",
        Metric::FormatCompliance => "Format",
    }
    .to_string()
}

/// Every platform the report mentions, for callers that want a quick summary.
pub fn platforms(report: &EvalReport) -> Vec<Platform> {
    report.by_platform().keys().copied().collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{MetricScore, Score, ScoreSource, Trace, TraceEvaluation, TraceId};

    fn evaluation(id: &str, platform: Platform, value: f64) -> TraceEvaluation {
        let trace = Trace {
            id: TraceId::new(id.to_string()),
            instructions: "i".to_string(),
            task: "t".to_string(),
            output: "o".to_string(),
            platform,
            model: None,
            prompting: Prompting::Explicit,
            skill_triggered: Some(true),
        };

        // Mirrors a real judge run: the five instruction metrics, but not
        // Correctness, which only the claim verifier produces.
        let scores = Metric::all()
            .iter()
            .filter(|metric| **metric != Metric::Correctness)
            .map(|metric| {
                MetricScore::new(
                    *metric,
                    Score::new(value).expect("valid"),
                    ScoreSource::LlmJudge,
                    "because".to_string(),
                )
            })
            .collect();

        TraceEvaluation::new(&trace, "llm:test".to_string(), scores, None)
    }

    fn report() -> EvalReport {
        EvalReport::new(
            "llm:test".to_string(),
            vec![
                evaluation("a", Platform::ClaudeCode, 0.9),
                evaluation("b", Platform::CodexCli, 0.4),
            ],
        )
    }

    #[test]
    fn trace_table_lists_every_trace_and_metric() {
        let rendered = render_traces(&report());

        assert!(rendered.contains('a'));
        assert!(rendered.contains('b'));
        assert!(rendered.contains("Adher"));
        assert!(rendered.contains("Format"));
        assert!(rendered.contains("0.90"));
        assert!(rendered.contains("0.40"));
    }

    #[test]
    fn comparison_table_has_one_row_per_platform() {
        let rendered = render_platform_comparison(&report());

        assert!(rendered.contains("Claude Code"));
        assert!(rendered.contains("Codex CLI"));
    }

    #[test]
    fn missing_metrics_render_as_a_dash() {
        let mut report = report();
        report.evaluations[0]
            .scores
            .retain(|s| s.metric != Metric::FormatCompliance);

        let rendered = render_traces(&report);

        assert!(rendered.contains('-'));
    }

    #[test]
    fn fallback_rows_are_labelled() {
        let mut report = report();
        report.evaluations[0].judge = "heuristic".to_string();
        report.evaluations[0].fallback_reason = Some("unreachable".to_string());

        assert!(render_traces(&report).contains("fallback"));
    }

    #[test]
    fn trigger_summary_is_none_when_nothing_was_recorded() {
        let mut report = report();
        for evaluation in &mut report.evaluations {
            evaluation.skill_triggered = None;
        }

        assert!(render_trigger_summary(&report).is_none());
    }

    #[test]
    fn trigger_summary_reports_a_rate_when_recorded() {
        let rendered = render_trigger_summary(&report()).expect("rates were recorded");

        assert!(rendered.contains("Explicit"));
        assert!(rendered.contains("100%"));
    }

    #[test]
    fn lists_platforms_present_in_the_report() {
        assert_eq!(
            platforms(&report()),
            vec![Platform::ClaudeCode, Platform::CodexCli]
        );
    }

    #[test]
    fn unmeasured_metrics_get_no_column() {
        let rendered = render_traces(&report());

        // Nothing in this report scored Correctness, so the column is absent
        // rather than rendered as dashes.
        assert!(!rendered.contains("Correct"));
        assert!(rendered.contains("Adher"));
    }

    #[test]
    fn a_measured_metric_does_get_a_column() {
        let mut report = report();
        report.evaluations[0].scores.push(MetricScore::new(
            Metric::Correctness,
            Score::new(1.0).expect("valid"),
            ScoreSource::ClaimVerifier,
            "verified".to_string(),
        ));

        assert!(render_traces(&report).contains("Correct"));
    }
}
