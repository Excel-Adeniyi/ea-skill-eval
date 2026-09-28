mod openai_compatible;
mod prompt;
mod response;

use std::future::Future;

use anyhow::Result;

use crate::domain::{MetricScore, Trace};
use crate::scoring::evaluate_trace;

pub use openai_compatible::{OpenAiCompatibleJudge, OLLAMA_BASE_URL};
pub use prompt::{build_user_message, PROMPT_VERSION, SYSTEM_PROMPT};
pub use response::parse_judge_response;

/// Anything that can score a trace.
///
/// Written as `-> impl Future + Send` rather than `async fn` on purpose. A bare
/// `async fn` in a public trait cannot promise the returned future is `Send`,
/// which would block `tokio::spawn` if concurrent judging is ever added.
/// Implementations may still write a plain `async fn`.
///
/// Neither form is object-safe, so this cannot be a `Box<dyn Judge>`. That is
/// what [`AnyJudge`] is for: the trait states the contract and works through
/// generics, the enum does runtime dispatch.
pub trait Judge {
    /// Short identifier recorded alongside the scores, so a report can say
    /// which judge produced which column.
    fn name(&self) -> String;

    fn score(&self, trace: &Trace) -> impl Future<Output = Result<Vec<MetricScore>>> + Send;
}

/// The heuristic scorer, wearing the same interface as the model judges.
///
/// Never fails and never touches the network, which is what makes it usable
/// both as a baseline column and as the fallback when a judge is unreachable.
#[derive(Debug, Default, Clone, Copy)]
pub struct HeuristicJudge;

impl Judge for HeuristicJudge {
    fn name(&self) -> String {
        "heuristic".to_string()
    }

    async fn score(&self, trace: &Trace) -> Result<Vec<MetricScore>> {
        Ok(evaluate_trace(trace))
    }
}

/// Runtime-selectable judge, for when the choice comes from a CLI flag.
#[derive(Debug, Clone)]
pub enum AnyJudge {
    Heuristic(HeuristicJudge),
    Llm(OpenAiCompatibleJudge),
}

impl Judge for AnyJudge {
    fn name(&self) -> String {
        match self {
            Self::Heuristic(judge) => judge.name(),
            Self::Llm(judge) => judge.name(),
        }
    }

    async fn score(&self, trace: &Trace) -> Result<Vec<MetricScore>> {
        match self {
            Self::Heuristic(judge) => judge.score(trace).await,
            Self::Llm(judge) => judge.score(trace).await,
        }
    }
}

/// Score with `judge`, falling back to the heuristic if it fails.
///
/// Returns the scores and the name of whichever scorer actually produced them,
/// so a report can never silently present fallback numbers as judge numbers.
pub async fn score_with_fallback<J: Judge>(
    judge: &J,
    trace: &Trace,
) -> (Vec<MetricScore>, String, Option<String>) {
    match judge.score(trace).await {
        Ok(scores) => (scores, judge.name(), None),
        Err(error) => (
            evaluate_trace(trace),
            HeuristicJudge.name(),
            // `{error:#}` prints the whole anyhow context chain, not just the
            // outermost message, so the report says *why* the judge failed.
            Some(format!("{error:#}")),
        ),
    }
}
