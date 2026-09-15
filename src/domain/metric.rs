use std::fmt;

use serde::{Deserialize, Serialize};
//  This is a enum for all 5 metrics we need for the eval
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Metric {
    InstructionAdherence,
    TaskRelevancy,
    InstructionPrecision,
    InstructionRecall,
    FormatCompliance,
}

impl Metric {
    pub fn all() -> [Self; 5] {
        [
            Self::InstructionAdherence,
            Self::TaskRelevancy,
            Self::InstructionPrecision,
            Self::InstructionRecall,
            Self::FormatCompliance,
        ]
    }
}

impl fmt::Display for Metric {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::InstructionAdherence => "Instruction Adherence",
            Self::TaskRelevancy => "Task Relevancy",
            Self::InstructionPrecision => "Instruction Precision",
            Self::InstructionRecall => "instruction Recall",
            Self::FormatCompliance => "Format Compliance",
        };

        formatter.write_str(name)
    }
}

#[cfg(test)]

mod tests {
    use super::*;

    #[test]

    fn all_returns_the_five_metrics() {
        assert_eq!(Metric::all().len(), 5);
    }

    #[test]
    fn display_uses_a_human_readable_name() {
        let metric = Metric::InstructionAdherence;

        assert_eq!(metric.to_string(), "Instruction Adherence");
    }

    #[test]
    fn debug_uses_the_rust_variant_name() {
        let metric = Metric::InstructionAdherence;

        assert_eq!(format!("{metric:?}"), "InstructionAdherence")
    }
}
