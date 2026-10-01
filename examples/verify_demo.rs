//! Check two answers — one true, one false — against the same reference.
//!
//! Run with: cargo run --example verify_demo

use ea_skill_eval::judge::OpenAiCompatibleJudge;

const REFERENCE: &str = "\
Rust manages memory through ownership. Each value has exactly one owner, and the
value is dropped when its owner goes out of scope. Rust has no garbage collector;
memory is freed deterministically at compile-time-known points. References let
code borrow a value without taking ownership. The borrow checker allows either
one mutable reference or any number of shared references, never both at once.";

const TRUE_ANSWER: &str = "\
In Rust each value has exactly one owner and is dropped when that owner goes out
of scope. Borrowing lets other code use a value without owning it.";

const FALSE_ANSWER: &str = "\
Rust uses a garbage collector that runs periodically to free unused memory. A
value can have many simultaneous owners and the runtime reconciles them.";

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let verifier = OpenAiCompatibleJudge::ollama("qwen3.6:latest")?;

    for (label, answer) in [("TRUE answer", TRUE_ANSWER), ("FALSE answer", FALSE_ANSWER)] {
        println!("\n=== {label} ===");

        let verification = verifier.verify(REFERENCE, answer).await?;

        for claim in &verification.claims {
            println!("  [{:?}] {}", claim.verdict, claim.claim);
        }

        match verification.as_metric_score() {
            Some(score) => println!(
                "  -> Correctness {:.2}  ({})",
                score.score.value(),
                score.reasoning
            ),
            None => println!("  -> no checkable claims"),
        }
    }

    Ok(())
}
