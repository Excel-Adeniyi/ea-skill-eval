use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Trace {
    pub id: TraceId,
    pub instructions: String,
    pub task: String,
    pub output: String,
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
            "output": "A struct groups related values."
        }
    "#;

        let trace: Trace = serde_json::from_str(json).expect("JSON should contain a valid trace");

        assert_eq!(trace.id.as_str(), "trace-001");
        assert_eq!(trace.task, "Explain Rust structs.");
    }
}
