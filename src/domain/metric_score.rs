use serde::{Deserialize, Serialize};

use super::{Metric, Score, ScoreSource};

#[derive(Debug, Serialize, Deserialize)]

pub struct MetricScore {
    pub metric: Metric,
    pub score: Score,
    pub source: ScoreSource,
    pub reasoning: String,
}

impl MetricScore {
    pub fn new(metric: Metric, score: Score, source: ScoreSource, reasoning: String) -> Self {
        Self {
            metric,
            score,
            source,
            reasoning,
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::domain::metric;

    use super::*;

    #[test]
    fn creates_a_metric_score() {
        let score = Score::new(0.8).expect("0.8 should be a valid score");

        let result: MetricScore = MetricScore::new(
            Metric::InstructionAdherence,
            score,
            ScoreSource::Heuristic,
            String::from("The output followed the instructions"),
        );
        assert_eq!(result.metric, Metric::InstructionAdherence);
        assert_eq!(result.score.value(), 0.8);
        assert_eq!(result.source, ScoreSource::Heuristic);
        assert_eq!(result.reasoning, "The output followed the instructions");
    }
}
