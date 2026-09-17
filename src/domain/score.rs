use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Error, PartialEq)]

pub enum ScoreError {
    #[error("score must be a finite number")]
    NotFinite,

    #[error("score must be between 0.0 and 1.0, received {0}")]
    OutOfRange(f64),
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64", into = "f64")]
pub struct Score(f64);

impl Score {
    pub fn new(value: f64) -> Result<Self, ScoreError> {
        Self::try_from(value)
    }

    pub fn value(self) -> f64 {
        self.0
    }
}
impl TryFrom<f64> for Score {
    type Error = ScoreError;

    fn try_from(value: f64) -> Result<Self, Self::Error> {
        if !value.is_finite() {
            return Err(ScoreError::NotFinite);
        }

        if !(0.0..=1.0).contains(&value) {
            return Err(ScoreError::OutOfRange(value));
        }

        Ok(Self(value))
    }
}

impl From<Score> for f64 {
    fn from(score: Score) -> Self {
        score.0
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_scores_inside_the_valid_range() {
        assert!(Score::new(0.0).is_ok());
        assert!(Score::new(0.5).is_ok());
        assert!(Score::new(1.0).is_ok());
    }

    #[test]
    fn rejects_scores_outside_the_valid_range() {
        assert_eq!(Score::new(-0.1), Err(ScoreError::OutOfRange(-0.1)));
        assert_eq!(Score::new(1.1), Err(ScoreError::OutOfRange(1.1)))
    }

    #[test]
    fn rejects_non_finite_scores() {
        assert_eq!(Score::new(f64::NAN), Err(ScoreError::NotFinite));
        assert_eq!(Score::new(f64::INFINITY), Err(ScoreError::NotFinite));
    }

    #[test]
    fn returns_the_inner_value() {
        let score = Score::new(0.75).expect("0.75 should be valid");

        assert_eq!(score.value(), 0.75);
    }
}
