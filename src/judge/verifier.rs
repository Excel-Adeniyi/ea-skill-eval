use anyhow::{Result, anyhow, bail};
use serde::{Deserialize, Serialize};

use crate::domain::{Metric, MetricScore, Score, ScoreSource};

/// What a reference document says about one claim.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Verdict {
    /// The reference states this.
    Supported,
    /// The reference states the opposite.
    Contradicted,
    /// The reference is silent. Not evidence either way.
    NotFound,
}

/// One factual claim pulled out of an answer, with its verdict.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct VerifiedClaim {
    pub claim: String,
    pub verdict: Verdict,
    #[serde(default)]
    pub evidence: String,
}

/// The outcome of checking every claim in an answer.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Verification {
    pub claims: Vec<VerifiedClaim>,
}

impl Verification {
    pub fn count(&self, verdict: Verdict) -> usize {
        self.claims
            .iter()
            .filter(|claim| claim.verdict == verdict)
            .count()
    }

    /// Fraction of *checkable* claims the reference supports.
    ///
    /// `NotFound` claims are excluded from both halves rather than counted as
    /// failures. A reference that simply does not mention something is not
    /// evidence that the answer is wrong — treating silence as a falsehood
    /// would punish answers for being more complete than the source.
    ///
    /// Returns `None` when nothing was checkable, so "no claims to verify"
    /// stays distinct from "every claim was false".
    pub fn score(&self) -> Option<f64> {
        let supported = self.count(Verdict::Supported);
        let contradicted = self.count(Verdict::Contradicted);
        let checkable = supported + contradicted;

        if checkable == 0 {
            return None;
        }

        Some(supported as f64 / checkable as f64)
    }

    /// Render as a `Correctness` score tagged [`ScoreSource::ClaimVerifier`].
    pub fn as_metric_score(&self) -> Option<MetricScore> {
        let value = self.score()?;
        let supported = self.count(Verdict::Supported);
        let contradicted = self.count(Verdict::Contradicted);
        let unknown = self.count(Verdict::NotFound);

        let mut reasoning = format!(
            "{supported} of {} checkable claim(s) supported by the reference",
            supported + contradicted
        );

        if contradicted > 0 {
            let first = self
                .claims
                .iter()
                .find(|claim| claim.verdict == Verdict::Contradicted)
                .map(|claim| claim.claim.as_str())
                .unwrap_or("");
            reasoning.push_str(&format!("; contradicted: \"{first}\""));
        }

        if unknown > 0 {
            reasoning.push_str(&format!("; {unknown} not covered by the reference"));
        }

        let score = Score::new(value).ok()?;

        Some(MetricScore::new(
            Metric::Correctness,
            score,
            ScoreSource::ClaimVerifier,
            reasoning,
        ))
    }
}

/// Instructions for the model doing the checking.
///
/// The two rules that matter: judge against the reference *only*, and say
/// `not_found` rather than guessing. A verifier that falls back on its own
/// memory is just the judge again, which is the thing this replaces.
pub const VERIFIER_SYSTEM_PROMPT: &str = r#"You verify factual claims against a reference document.

Steps:
1. Extract every checkable factual claim from the ANSWER. Ignore opinions,
   hedges, and statements about what the answer itself is doing.
2. For each claim, decide using ONLY the REFERENCE:
   - "supported": the reference states this.
   - "contradicted": the reference states the opposite.
   - "not_found": the reference does not address it.

Do not use your own knowledge. If the reference is silent, the verdict is
"not_found", never "supported". Quote the reference in "evidence" for any
supported or contradicted claim.

Reply with ONLY this JSON and no other text:
{"claims":[{"claim":"","verdict":"supported","evidence":""}]}"#;

pub fn build_verifier_message(reference: &str, answer: &str) -> String {
    format!("REFERENCE:\n{reference}\n\n---\n\nANSWER:\n{answer}")
}

/// Parse verifier output, tolerating fences and surrounding prose.
pub fn parse_verification(raw: &str) -> Result<Verification> {
    let start = raw
        .find('{')
        .ok_or_else(|| anyhow!("verifier returned no JSON object: {raw:?}"))?;
    let end = raw
        .rfind('}')
        .ok_or_else(|| anyhow!("verifier returned no JSON object: {raw:?}"))?;

    if end <= start {
        bail!("verifier returned no JSON object: {raw:?}");
    }

    let verification: Verification = serde_json::from_str(&raw[start..=end])
        .map_err(|error| anyhow!("verifier response was not the expected shape: {error}"))?;

    Ok(verification)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn claim(text: &str, verdict: Verdict) -> VerifiedClaim {
        VerifiedClaim {
            claim: text.to_string(),
            verdict,
            evidence: String::new(),
        }
    }

    #[test]
    fn all_supported_scores_one() {
        let verification = Verification {
            claims: vec![
                claim("a", Verdict::Supported),
                claim("b", Verdict::Supported),
            ],
        };

        assert_eq!(verification.score(), Some(1.0));
    }

    #[test]
    fn a_contradicted_claim_lowers_the_score() {
        let verification = Verification {
            claims: vec![
                claim("a", Verdict::Supported),
                claim("b", Verdict::Contradicted),
            ],
        };

        assert_eq!(verification.score(), Some(0.5));
    }

    #[test]
    fn not_found_claims_are_excluded_rather_than_penalised() {
        // One supported, two the reference never mentions. Silence is not
        // evidence of falsehood, so the score stays 1.0.
        let verification = Verification {
            claims: vec![
                claim("a", Verdict::Supported),
                claim("b", Verdict::NotFound),
                claim("c", Verdict::NotFound),
            ],
        };

        assert_eq!(verification.score(), Some(1.0));
    }

    #[test]
    fn nothing_checkable_has_no_score() {
        let verification = Verification {
            claims: vec![claim("a", Verdict::NotFound)],
        };

        assert_eq!(verification.score(), None);
        assert!(verification.as_metric_score().is_none());
    }

    #[test]
    fn metric_score_is_tagged_as_claim_verified() {
        let verification = Verification {
            claims: vec![claim("a", Verdict::Supported)],
        };
        let scored = verification.as_metric_score().expect("has a score");

        assert_eq!(scored.metric, Metric::Correctness);
        assert_eq!(scored.source, ScoreSource::ClaimVerifier);
    }

    #[test]
    fn reasoning_names_a_contradicted_claim() {
        let verification = Verification {
            claims: vec![
                claim("Rust has a garbage collector", Verdict::Contradicted),
                claim("Rust is compiled", Verdict::Supported),
            ],
        };
        let scored = verification.as_metric_score().expect("has a score");

        assert!(scored.reasoning.contains("garbage collector"));
    }

    #[test]
    fn reasoning_reports_uncovered_claims() {
        let verification = Verification {
            claims: vec![
                claim("a", Verdict::Supported),
                claim("b", Verdict::NotFound),
            ],
        };
        let scored = verification.as_metric_score().expect("has a score");

        assert!(scored.reasoning.contains("1 not covered"));
    }

    #[test]
    fn parses_a_well_formed_response() {
        let raw = r#"{"claims":[
            {"claim":"Rust is compiled","verdict":"supported","evidence":"Rust compiles to native code"},
            {"claim":"Rust uses a GC","verdict":"contradicted","evidence":"Rust has no garbage collector"}
        ]}"#;

        let verification = parse_verification(raw).expect("should parse");

        assert_eq!(verification.claims.len(), 2);
        assert_eq!(verification.count(Verdict::Supported), 1);
        assert_eq!(verification.count(Verdict::Contradicted), 1);
        assert_eq!(verification.score(), Some(0.5));
    }

    #[test]
    fn parses_through_markdown_fences() {
        let raw = "Here you go:\n```json\n{\"claims\":[{\"claim\":\"x\",\"verdict\":\"not_found\"}]}\n```";

        let verification = parse_verification(raw).expect("should parse");

        assert_eq!(verification.count(Verdict::NotFound), 1);
    }

    #[test]
    fn errors_when_there_is_no_json() {
        assert!(parse_verification("I could not verify that.").is_err());
    }

    #[test]
    fn the_prompt_forbids_using_model_knowledge() {
        assert!(VERIFIER_SYSTEM_PROMPT.contains("Do not use your own knowledge"));
    }

    #[test]
    fn the_message_separates_reference_from_answer() {
        let message = build_verifier_message("REF TEXT", "ANSWER TEXT");

        assert!(message.contains("REFERENCE:\nREF TEXT"));
        assert!(message.contains("ANSWER:\nANSWER TEXT"));
    }
}
