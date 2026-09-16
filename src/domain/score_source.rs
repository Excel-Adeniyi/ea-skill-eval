use std::fmt;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ScoreSource {
    Heuristic,
    LlmJudge,
    ClaimVerifier,
    Human,
}

impl fmt::Display for ScoreSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Heuristic => "Heuristic",
            Self::LlmJudge => "LLM Judge",
            Self::ClaimVerifier => "Claim Verifier",
            Self::Human => "Human",
        };

        formatter.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_uses_human_readable_names() {
        assert_eq!(ScoreSource::Heuristic.to_string(), "Heuristic");
        assert_eq!(ScoreSource::LlmJudge.to_string(), "LLM Judge");
        assert_eq!(ScoreSource::ClaimVerifier.to_string(), "Claim Verifier");
        assert_eq!(ScoreSource::Human.to_string(), "Human");
    }
}
