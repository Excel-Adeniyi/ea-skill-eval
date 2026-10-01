use crate::domain::Trace;

/// Bumped whenever the wording below changes.
///
/// Cached judge results are keyed partly on this, so a prompt edit invalidates
/// stale scores instead of silently mixing two rubrics in one report.
pub const PROMPT_VERSION: &str = "v1";

/// The rubric. One request scores all five metrics, rather than five requests
/// per trace — at ~46s per local call that is the difference between a 12
/// minute run and an hour.
pub const SYSTEM_PROMPT: &str = r#"You are a strict evaluation judge for AI agent outputs.

Score the OUTPUT against the INSTRUCTIONS and TASK on exactly these five metrics, each from 0.0 to 1.0:

- InstructionAdherence: did the output do what the instructions told it to do?
- TaskRelevancy: did the output address the task that was asked?
- InstructionPrecision: was the output free of content the instructions did not call for?
- InstructionRecall: did the output cover everything the instructions called for?
- FormatCompliance: did the output obey any stated formatting requirement? Score 1.0 if no format was requested.

Judge only what is present. Do not reward length, confidence or politeness.

Reply with ONLY a JSON object of this exact shape, and no other text:
{"scores":[{"metric":"InstructionAdherence","score":0.0,"reasoning":""},{"metric":"TaskRelevancy","score":0.0,"reasoning":""},{"metric":"InstructionPrecision","score":0.0,"reasoning":""},{"metric":"InstructionRecall","score":0.0,"reasoning":""},{"metric":"FormatCompliance","score":0.0,"reasoning":""}]}

Use exactly these five metric names. Keep each reasoning under 25 words."#;

/// Render one trace as the judge's user message.
pub fn build_user_message(trace: &Trace) -> String {
    format!(
        "INSTRUCTIONS:\n{}\n\nTASK:\n{}\n\nOUTPUT:\n{}",
        trace.instructions, trace.task, trace.output
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::TraceId;

    fn trace() -> Trace {
        Trace {
            id: TraceId::new("t".to_string()),
            instructions: "Reply as JSON.".to_string(),
            task: "Name a colour.".to_string(),
            output: "Blue.".to_string(),
            platform: Default::default(),
            model: None,
            prompting: Default::default(),
            skill_triggered: None,
        }
    }

    #[test]
    fn user_message_contains_all_three_sections() {
        let message = build_user_message(&trace());

        assert!(message.contains("INSTRUCTIONS:\nReply as JSON."));
        assert!(message.contains("TASK:\nName a colour."));
        assert!(message.contains("OUTPUT:\nBlue."));
    }

    #[test]
    fn system_prompt_names_every_metric_the_judge_produces() {
        // `Correctness` is deliberately absent: it is produced by the claim
        // verifier against a reference document, not by the judge reading from
        // its own knowledge. Asking the judge for it here would reintroduce
        // exactly the unsourced opinion the verifier exists to replace.
        for metric in crate::domain::Metric::all() {
            if metric == crate::domain::Metric::Correctness {
                continue;
            }

            let name = format!("{metric:?}");
            assert!(
                SYSTEM_PROMPT.contains(&name),
                "system prompt is missing {name}"
            );
        }
    }

    #[test]
    fn the_judge_does_not_claim_to_score_correctness() {
        assert!(!SYSTEM_PROMPT.contains("Correctness"));
    }
}
