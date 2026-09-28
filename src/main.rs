use anyhow::Result;

use ea_skill_eval::domain::Metric;
use ea_skill_eval::judge::{score_with_fallback, HeuristicJudge, Judge, OpenAiCompatibleJudge};
use ea_skill_eval::load_traces;

/// Temporary entry point: Day 2 proves the judge path end to end.
/// The clap CLI and the table/JSON reports land on Day 3.
#[tokio::main]
async fn main() -> Result<()> {
    let path = "samples/traces.json";
    let traces = load_traces(path)?;

    // `--heuristic` skips the network entirely, for fast iteration on output.
    let use_heuristic = std::env::args().any(|argument| argument == "--heuristic");

    println!("Loaded {} trace(s) from {path}", traces.len());

    if use_heuristic {
        let judge = HeuristicJudge;
        println!("Judge: {}\n", judge.name());
        report(&traces, &judge).await;
    } else {
        let judge = OpenAiCompatibleJudge::ollama("qwen3.6:latest")?;
        println!("Judge: {} (pass --heuristic to skip the model)\n", judge.name());
        report(&traces, &judge).await;
    }

    Ok(())
}

async fn report<J: Judge>(traces: &[ea_skill_eval::Trace], judge: &J) {
    for trace in traces {
        let started = std::time::Instant::now();
        let (scores, actual_judge, failure) = score_with_fallback(judge, trace).await;
        let elapsed = started.elapsed();

        println!("{}", "=".repeat(74));
        println!("{}  —  {}", trace.id.as_str(), trace.task);
        println!("{}  ·  {actual_judge}  ·  {:.1}s", trace.provenance(), elapsed.as_secs_f64());
        println!("{}", "=".repeat(74));

        if let Some(reason) = failure {
            println!("  !! judge failed, fell back to heuristic: {reason}");
        }

        for metric in Metric::all() {
            match scores.iter().find(|entry| entry.metric == metric) {
                Some(entry) => {
                    println!("  {:<22} {:>5.2}", metric.to_string(), entry.score.value());
                    if !entry.reasoning.is_empty() {
                        println!("      {}", entry.reasoning);
                    }
                }
                None => println!("  {:<22}     -  (not applicable)", metric.to_string()),
            }
        }

        println!();
    }
}
