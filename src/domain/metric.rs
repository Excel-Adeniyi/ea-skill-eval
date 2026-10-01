use std::fmt;
use std::str::FromStr;

use serde::{Deserialize, Serialize};
/// The dimensions an answer is scored on.
///
/// Five measure instruction-following. `Correctness` is a different axis
/// entirely — an answer can follow every instruction perfectly and still be
/// false — and is produced by the claim verifier rather than the judge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum Metric {
    Correctness,
    InstructionAdherence,
    TaskRelevancy,
    InstructionPrecision,
    InstructionRecall,
    FormatCompliance,
}

impl Metric {
    pub fn all() -> [Self; 6] {
        [
            Self::Correctness,
            Self::InstructionAdherence,
            Self::TaskRelevancy,
            Self::InstructionPrecision,
            Self::InstructionRecall,
            Self::FormatCompliance,
        ]
    }
}

/// Returned when a judge names a metric we don't recognise.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown metric name: {0}")]
pub struct UnknownMetric(pub String);

impl FromStr for Metric {
    type Err = UnknownMetric;

    /// Parse a metric name from judge output.
    ///
    /// Deliberately lenient about case and separators: a model asked for
    /// "InstructionAdherence" may return "instruction_adherence" or
    /// "Instruction Adherence", and rejecting a whole response over
    /// punctuation would waste a 46-second call.
    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let normalised: String = value
            .chars()
            .filter(|character| character.is_alphanumeric())
            .flat_map(|character| character.to_lowercase())
            .collect();

        match normalised.as_str() {
            "correctness" => Ok(Self::Correctness),
            "instructionadherence" => Ok(Self::InstructionAdherence),
            "taskrelevancy" => Ok(Self::TaskRelevancy),
            "instructionprecision" => Ok(Self::InstructionPrecision),
            "instructionrecall" => Ok(Self::InstructionRecall),
            "formatcompliance" => Ok(Self::FormatCompliance),
            _ => Err(UnknownMetric(value.to_string())),
        }
    }
}

impl fmt::Display for Metric {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        let name = match self {
            Self::Correctness => "Correctness",
            Self::InstructionAdherence => "Instruction Adherence",
            Self::TaskRelevancy => "Task Relevancy",
            Self::InstructionPrecision => "Instruction Precision",
            Self::InstructionRecall => "Instruction Recall",
            Self::FormatCompliance => "Format Compliance",
        };

        formatter.write_str(name)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]

    fn all_returns_every_metric() {
        assert_eq!(Metric::all().len(), 6);
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

    #[test]
    fn parses_the_canonical_metric_names() {
        for metric in Metric::all() {
            let name = format!("{metric:?}");
            assert_eq!(name.parse::<Metric>().unwrap(), metric);
        }
    }

    #[test]
    fn parses_loosely_formatted_names_from_judges() {
        assert_eq!(
            "instruction_adherence".parse::<Metric>().unwrap(),
            Metric::InstructionAdherence
        );
        assert_eq!(
            "Instruction Adherence".parse::<Metric>().unwrap(),
            Metric::InstructionAdherence
        );
        assert_eq!(
            "FORMAT-COMPLIANCE".parse::<Metric>().unwrap(),
            Metric::FormatCompliance
        );
    }

    #[test]
    fn rejects_names_it_does_not_recognise() {
        let error = "Helpfulness".parse::<Metric>().unwrap_err();

        assert_eq!(error, UnknownMetric("Helpfulness".to_string()));
    }

    #[test]
    fn display_round_trips_through_from_str() {
        for metric in Metric::all() {
            assert_eq!(metric.to_string().parse::<Metric>().unwrap(), metric);
        }
    }
}
