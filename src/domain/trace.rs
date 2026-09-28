use serde::{Deserialize, Serialize};

use super::{Platform, Prompting};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct TraceId(String);

// Constructor
impl TraceId {
    pub fn new(value: String) -> Self {
        Self(value)
    }
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// One captured agent run: what it was told, what it was asked, what it said,
/// and the circumstances of the capture.
///
/// Every field added for the cross-platform comparison is `#[serde(default)]`,
/// so trace files written before those fields existed still load. That matters
/// more than it looks: a hand-maintained fixture set is expensive to recapture,
/// and a schema change that invalidates it costs a day.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub id: TraceId,
    pub instructions: String,
    pub task: String,
    pub output: String,

    /// Which agent surface produced this output.
    #[serde(default)]
    pub platform: Platform,

    /// The specific model behind that surface, as reported by the tool
    /// (e.g. "claude-opus-5"). `None` when it wasn't recorded.
    #[serde(default)]
    pub model: Option<String>,

    /// Whether the prompt named the skill or left the agent to infer it.
    #[serde(default)]
    pub prompting: Prompting,

    /// Whether the skill actually fired, determined from the sentinel marker
    /// the skill emits rather than from self-report. `None` means the capture
    /// didn't record it.
    #[serde(default)]
    pub skill_triggered: Option<bool>,
}

impl Trace {
    /// A short label for report rows: platform, plus model when it's known.
    pub fn provenance(&self) -> String {
        match &self.model {
            Some(model) => format!("{} ({model})", self.platform),
            None => self.platform.to_string(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn deserializes_a_trace_from_json() {
        let json = r#"
        {
            "id": "trace-001",
            "instructions": "Answer using the supplied context.",
            "task": "Explain Rust structs.",
            "output": "A struct groups related values.",
            "platform": "claude_code",
            "model": "claude-opus-5",
            "prompting": "explicit",
            "skill_triggered": true
        }
    "#;

        let trace: Trace = serde_json::from_str(json).expect("JSON should contain a valid trace");

        assert_eq!(trace.id.as_str(), "trace-001");
        assert_eq!(trace.task, "Explain Rust structs.");
        assert_eq!(trace.platform, Platform::ClaudeCode);
        assert_eq!(trace.model.as_deref(), Some("claude-opus-5"));
        assert_eq!(trace.prompting, Prompting::Explicit);
        assert_eq!(trace.skill_triggered, Some(true));
    }

    #[test]
    fn deserializes_a_trace_captured_before_the_new_fields_existed() {
        let json = r#"
        {
            "id": "trace-legacy",
            "instructions": "Be brief.",
            "task": "Explain enums.",
            "output": "An enum is one of several variants."
        }
    "#;

        let trace: Trace = serde_json::from_str(json).expect("older traces should still load");

        assert_eq!(trace.platform, Platform::Unknown);
        assert_eq!(trace.model, None);
        assert_eq!(trace.prompting, Prompting::Explicit);
        assert_eq!(trace.skill_triggered, None);
    }

    #[test]
    fn provenance_includes_the_model_when_known() {
        let mut trace: Trace = serde_json::from_str(
            r#"{"id":"t","instructions":"i","task":"t","output":"o","platform":"codex_cli"}"#,
        )
        .expect("trace should load");

        assert_eq!(trace.provenance(), "Codex CLI");

        trace.model = Some("gpt-5-codex".to_string());
        assert_eq!(trace.provenance(), "Codex CLI (gpt-5-codex)");
    }
}
