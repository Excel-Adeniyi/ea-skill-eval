use std::process::ExitCode;

use anyhow::{Context, Result};
use clap::Parser;

use ea_skill_eval::cli::{Cli, Format, JudgeChoice};
use ea_skill_eval::domain::{EvalReport, Metric, Trace, TraceEvaluation};
use ea_skill_eval::judge::{HeuristicJudge, Judge, OpenAiCompatibleJudge, score_with_fallback};
use ea_skill_eval::load_traces;
use ea_skill_eval::report;

/// Exit code used when traces fall below `--threshold`, so CI can gate on it.
const EXIT_BELOW_THRESHOLD: u8 = 2;

#[tokio::main]
async fn main() -> Result<ExitCode> {
    let cli = Cli::parse();

    let path = cli.input.to_string_lossy().to_string();
    let traces =
        load_traces(&path).with_context(|| format!("could not load traces from {path}"))?;

    let report = match cli.judge {
        JudgeChoice::Heuristic => evaluate(&traces, &HeuristicJudge, cli.metric_filter()).await,
        JudgeChoice::Llm => {
            let mut judge = OpenAiCompatibleJudge::new(&cli.base_url, &cli.model)?;
            if let Some(key) = &cli.api_key {
                judge = judge.with_api_key(key);
            }
            evaluate(&traces, &judge, cli.metric_filter()).await
        }
    };

    match cli.format {
        Format::Json => println!("{}", report::render_json(&report)?),
        Format::Table => print_tables(&report),
    }

    // Threshold is checked after printing, so a failing run still shows why.
    if let Some(threshold) = cli.threshold {
        let failing = report.below_threshold(threshold);

        if !failing.is_empty() {
            eprintln!(
                "\n{} of {} trace(s) scored below {threshold:.2}: {}",
                failing.len(),
                report.evaluations.len(),
                failing
                    .iter()
                    .map(|evaluation| evaluation.trace_id.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            );

            return Ok(ExitCode::from(EXIT_BELOW_THRESHOLD));
        }
    }

    Ok(ExitCode::SUCCESS)
}

/// Score every trace, keeping only the requested metrics.
async fn evaluate<J: Judge>(
    traces: &[Trace],
    judge: &J,
    metric_filter: Option<Vec<Metric>>,
) -> EvalReport {
    let mut evaluations = Vec::with_capacity(traces.len());

    for trace in traces {
        let (mut scores, actual_judge, failure) = score_with_fallback(judge, trace).await;

        if let Some(wanted) = &metric_filter {
            scores.retain(|entry| wanted.contains(&entry.metric));
        }

        evaluations.push(TraceEvaluation::new(trace, actual_judge, scores, failure));
    }

    EvalReport::new(judge.name(), evaluations)
}

fn print_tables(report: &EvalReport) {
    if report.is_empty() {
        println!("No traces to evaluate.");
        return;
    }

    println!(
        "\nPer-trace scores  ·  judge requested: {}",
        report.judge_requested
    );
    println!("{}", report::render_traces(report));

    println!("\nPlatform comparison");
    println!("{}", report::render_platform_comparison(report));

    if let Some(triggers) = report::render_trigger_summary(report) {
        println!("\nSkill triggering");
        println!("{triggers}");
    }

    if let Some(overall) = report.overall_mean() {
        println!("\nOverall mean: {overall:.2}");
    }

    let fallbacks = report.fallback_count();
    if fallbacks > 0 {
        println!(
            "Warning: {fallbacks} trace(s) fell back to the heuristic — those rows are not judge scores."
        );
    }
}
