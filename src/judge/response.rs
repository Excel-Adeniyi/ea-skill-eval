use anyhow::{Context, Result, anyhow, bail};
use serde::Deserialize;

use crate::domain::{Metric, MetricScore, Score, ScoreSource};

/// The shape we ask judges to return.
#[derive(Debug, Deserialize)]
struct JudgeResponse {
    scores: Vec<RawScore>,
}

#[derive(Debug, Deserialize)]
struct RawScore {
    metric: String,
    score: f64,
    #[serde(default)]
    reasoning: String,
}

/// Turn raw judge text into scored metrics.
///
/// Models wrap JSON in prose or markdown fences even when told not to, so this
/// extracts the object rather than trusting the whole string to parse. A
/// malformed entry is skipped rather than failing the batch: four good metrics
/// beat discarding a 46-second call over one bad field.
pub fn parse_judge_response(raw: &str) -> Result<Vec<MetricScore>> {
    let json = extract_json_object(raw)
        .ok_or_else(|| anyhow!("judge response contained no JSON object: {raw:?}"))?;

    let response: JudgeResponse = serde_json::from_str(json)
        .with_context(|| format!("judge response was not the expected shape: {json:?}"))?;

    let mut scores: Vec<MetricScore> = Vec::new();

    for entry in response.scores {
        let Ok(metric) = entry.metric.parse::<Metric>() else {
            // An invented metric name tells us nothing; drop it and keep going.
            continue;
        };

        // Already-scored metrics win, so a duplicated entry cannot overwrite.
        if scores.iter().any(|existing| existing.metric == metric) {
            continue;
        }

        let Ok(score) = Score::new(entry.score) else {
            continue;
        };

        scores.push(MetricScore::new(
            metric,
            score,
            ScoreSource::LlmJudge,
            entry.reasoning,
        ));
    }

    if scores.is_empty() {
        bail!("judge returned no usable metric scores: {json:?}");
    }

    // Report in a stable order regardless of what order the judge emitted.
    scores.sort_by_key(|entry| {
        Metric::all()
            .iter()
            .position(|metric| *metric == entry.metric)
            .unwrap_or(usize::MAX)
    });

    Ok(scores)
}

/// Slice out the outermost `{...}`, ignoring fences and commentary around it.
fn extract_json_object(raw: &str) -> Option<&str> {
    let start = raw.find('{')?;
    let end = raw.rfind('}')?;

    if end <= start {
        return None;
    }

    Some(&raw[start..=end])
}

#[cfg(test)]
mod tests {
    use super::*;

    const GOOD: &str = r#"{"scores":[
        {"metric":"InstructionAdherence","score":0.8,"reasoning":"Mostly followed."},
        {"metric":"TaskRelevancy","score":1.0,"reasoning":"On topic."},
        {"metric":"InstructionPrecision","score":0.6,"reasoning":"Some padding."},
        {"metric":"InstructionRecall","score":0.7,"reasoning":"Missed one point."},
        {"metric":"FormatCompliance","score":0.0,"reasoning":"Not JSON."}
    ]}"#;

    #[test]
    fn parses_a_well_formed_response() {
        let scores = parse_judge_response(GOOD).expect("should parse");

        assert_eq!(scores.len(), 5);
        assert_eq!(scores[0].metric, Metric::InstructionAdherence);
        assert_eq!(scores[0].score.value(), 0.8);
        assert_eq!(scores[0].reasoning, "Mostly followed.");
    }

    #[test]
    fn tags_every_score_as_coming_from_a_judge() {
        let scores = parse_judge_response(GOOD).expect("should parse");

        assert!(
            scores
                .iter()
                .all(|entry| entry.source == ScoreSource::LlmJudge)
        );
    }

    #[test]
    fn returns_metrics_in_canonical_order() {
        let shuffled = r#"{"scores":[
            {"metric":"FormatCompliance","score":1.0,"reasoning":"ok"},
            {"metric":"InstructionAdherence","score":0.5,"reasoning":"ok"}
        ]}"#;

        let scores = parse_judge_response(shuffled).expect("should parse");

        assert_eq!(scores[0].metric, Metric::InstructionAdherence);
        assert_eq!(scores[1].metric, Metric::FormatCompliance);
    }

    #[test]
    fn strips_markdown_fences() {
        let fenced = format!("Here you go:\n```json\n{GOOD}\n```\nHope that helps!");

        let scores = parse_judge_response(&fenced).expect("should parse through fences");

        assert_eq!(scores.len(), 5);
    }

    #[test]
    fn skips_metrics_it_does_not_recognise() {
        let extra = r#"{"scores":[
            {"metric":"Helpfulness","score":0.9,"reasoning":"invented"},
            {"metric":"TaskRelevancy","score":0.4,"reasoning":"real"}
        ]}"#;

        let scores = parse_judge_response(extra).expect("should parse");

        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].metric, Metric::TaskRelevancy);
    }

    #[test]
    fn skips_scores_outside_the_valid_range() {
        let out_of_range = r#"{"scores":[
            {"metric":"TaskRelevancy","score":5.0,"reasoning":"too high"},
            {"metric":"FormatCompliance","score":0.5,"reasoning":"fine"}
        ]}"#;

        let scores = parse_judge_response(out_of_range).expect("should parse");

        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].metric, Metric::FormatCompliance);
    }

    #[test]
    fn keeps_the_first_of_a_duplicated_metric() {
        let duplicated = r#"{"scores":[
            {"metric":"TaskRelevancy","score":1.0,"reasoning":"first"},
            {"metric":"TaskRelevancy","score":0.0,"reasoning":"second"}
        ]}"#;

        let scores = parse_judge_response(duplicated).expect("should parse");

        assert_eq!(scores.len(), 1);
        assert_eq!(scores[0].reasoning, "first");
    }

    #[test]
    fn tolerates_a_missing_reasoning_field() {
        let terse = r#"{"scores":[{"metric":"TaskRelevancy","score":0.5}]}"#;

        let scores = parse_judge_response(terse).expect("should parse");

        assert_eq!(scores[0].reasoning, "");
    }

    #[test]
    fn errors_when_there_is_no_json_at_all() {
        assert!(parse_judge_response("I cannot help with that.").is_err());
    }

    #[test]
    fn errors_when_nothing_usable_survives() {
        let useless = r#"{"scores":[{"metric":"Vibes","score":0.9,"reasoning":"nope"}]}"#;

        assert!(parse_judge_response(useless).is_err());
    }
}
