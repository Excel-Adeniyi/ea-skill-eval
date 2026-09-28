use anyhow::Result;

use ea_skill_eval::{evaluate_trace, load_traces};

fn main() -> Result<()> {
    let path = "samples/traces.json";
    let traces = load_traces(path)?;

    println!("Loaded {} trace(s) from {path}\n", traces.len());

    for trace in &traces {
        println!("{}", "=".repeat(72));
        println!("{}  —  {}", trace.id.as_str(), trace.task);
        println!("{}", "=".repeat(72));

        let scores = evaluate_trace(trace);

        for entry in &scores {
            println!(
                "  {:<22} {:>5.2}  [{}]",
                entry.metric.to_string(),
                entry.score.value(),
                entry.source
            );
            println!("      {}", entry.reasoning);
        }

        if scores.len() < 5 {
            println!("  (metrics not applicable to this trace were skipped)");
        }

        println!();
    }

    Ok(())
}
