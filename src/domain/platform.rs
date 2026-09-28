use std::fmt;

use serde::{Deserialize, Serialize};

/// The agent surface a trace was captured from.
///
/// Both current variants are agentic terminal tools, which is what makes the
/// comparison fair: each can load an instruction file from the repository and
/// each lets you observe whether the skill actually fired.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Platform {
    ClaudeCode,
    CodexCli,
    QwenCode,
    /// Used for traces captured before the field existed, so old fixtures keep
    /// deserialising instead of failing the whole run.
    #[default]
    Unknown,
}

impl fmt::Display for Platform {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::ClaudeCode => "Claude Code",
            Self::CodexCli => "Codex CLI",
            Self::QwenCode => "Qwen Code",
            Self::Unknown => "Unknown",
        };

        formatter.write_str(name)
    }
}

/// How the skill was asked for in the prompt that produced a trace.
///
/// Orthogonal to whether the skill actually ran. The Day 3 experiment is
/// exactly this pairing: under `Implicit` prompting, does `skill_triggered`
/// still come back true?
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Default, Serialize, Deserialize,
)]
#[serde(rename_all = "snake_case")]
pub enum Prompting {
    /// The prompt named the skill outright.
    #[default]
    Explicit,
    /// The prompt described the task only, leaving the agent to decide.
    Implicit,
}

impl fmt::Display for Prompting {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Explicit => "Explicit",
            Self::Implicit => "Implicit",
        };

        formatter.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_uses_human_readable_names() {
        assert_eq!(Platform::ClaudeCode.to_string(), "Claude Code");
        assert_eq!(Platform::CodexCli.to_string(), "Codex CLI");
        assert_eq!(Platform::QwenCode.to_string(), "Qwen Code");
        assert_eq!(Prompting::Implicit.to_string(), "Implicit");
    }

    #[test]
    fn serialises_to_snake_case() {
        let json = serde_json::to_string(&Platform::ClaudeCode).expect("platform should serialise");

        assert_eq!(json, r#""claude_code""#);
    }

    #[test]
    fn deserialises_from_snake_case() {
        let platform: Platform =
            serde_json::from_str(r#""codex_cli""#).expect("snake_case should deserialise");

        assert_eq!(platform, Platform::CodexCli);
    }

    #[test]
    fn unknown_is_the_default_platform() {
        assert_eq!(Platform::default(), Platform::Unknown);
    }
}
