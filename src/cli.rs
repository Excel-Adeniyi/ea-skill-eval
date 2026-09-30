use std::path::PathBuf;

use clap::{Parser, ValueEnum};

use crate::judge::{DEFAULT_CACHE_DIR, OLLAMA_BASE_URL};

/// Evaluate captured agent traces on five instruction-following metrics.
#[derive(Debug, Parser)]
#[command(name = "ea-skill-eval", version, about)]
pub struct Cli {
    /// JSON file of captured traces.
    #[arg(default_value = "samples/traces.json")]
    pub input: PathBuf,

    /// Which scorer to use.
    #[arg(long, value_enum, default_value_t = JudgeChoice::Heuristic)]
    pub judge: JudgeChoice,

    /// Model name for `--judge llm`.
    #[arg(long, default_value = "qwen3.6:latest")]
    pub model: String,

    /// OpenAI-compatible endpoint for `--judge llm`.
    #[arg(long, default_value = OLLAMA_BASE_URL)]
    pub base_url: String,

    /// Bearer token for hosted endpoints. Local Ollama needs none.
    #[arg(long, env = "SKILL_EVAL_API_KEY", hide_env_values = true)]
    pub api_key: Option<String>,

    /// How to print the report.
    #[arg(long, value_enum, default_value_t = Format::Table)]
    pub format: Format,

    /// Exit non-zero if any trace's mean score falls below this.
    #[arg(long)]
    pub threshold: Option<f64>,

    /// Only score these metrics. Repeatable; defaults to all five.
    #[arg(long = "metric", value_enum)]
    pub metrics: Vec<MetricChoice>,

    /// Where to store judged results between runs.
    #[arg(long, default_value = DEFAULT_CACHE_DIR)]
    pub cache_dir: PathBuf,

    /// Judge every trace afresh, ignoring and bypassing the cache.
    #[arg(long)]
    pub no_cache: bool,

    /// Delete every cached result, then exit.
    #[arg(long)]
    pub clear_cache: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum JudgeChoice {
    /// Term-overlap heuristics. Instant, offline, deterministic.
    Heuristic,
    /// An OpenAI-compatible model endpoint.
    Llm,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum Format {
    Table,
    Json,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ValueEnum)]
pub enum MetricChoice {
    Adherence,
    Relevancy,
    Precision,
    Recall,
    Format,
}

impl From<MetricChoice> for crate::domain::Metric {
    fn from(choice: MetricChoice) -> Self {
        match choice {
            MetricChoice::Adherence => Self::InstructionAdherence,
            MetricChoice::Relevancy => Self::TaskRelevancy,
            MetricChoice::Precision => Self::InstructionPrecision,
            MetricChoice::Recall => Self::InstructionRecall,
            MetricChoice::Format => Self::FormatCompliance,
        }
    }
}

impl Cli {
    /// The cache this run should use.
    ///
    /// Only the LLM judge is cached. The heuristic recomputes in microseconds,
    /// so a disk round-trip would be slower than the work it saves.
    pub fn cache(&self) -> crate::judge::JudgeCache {
        if self.no_cache || self.judge == JudgeChoice::Heuristic {
            return crate::judge::JudgeCache::disabled();
        }

        crate::judge::JudgeCache::new(self.cache_dir.clone())
    }

    /// Metrics to keep, or `None` for all of them.
    pub fn metric_filter(&self) -> Option<Vec<crate::domain::Metric>> {
        if self.metrics.is_empty() {
            return None;
        }

        Some(self.metrics.iter().map(|choice| (*choice).into()).collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use clap::CommandFactory;

    #[test]
    fn cli_definition_is_valid() {
        Cli::command().debug_assert();
    }

    #[test]
    fn defaults_to_the_offline_heuristic() {
        let cli = Cli::try_parse_from(["ea-skill-eval"]).expect("should parse");

        assert_eq!(cli.judge, JudgeChoice::Heuristic);
        assert_eq!(cli.format, Format::Table);
        assert_eq!(cli.input, PathBuf::from("samples/traces.json"));
        assert!(cli.threshold.is_none());
    }

    #[test]
    fn accepts_a_positional_input_path() {
        let cli = Cli::try_parse_from(["ea-skill-eval", "custom.json"]).expect("should parse");

        assert_eq!(cli.input, PathBuf::from("custom.json"));
    }

    #[test]
    fn parses_the_llm_judge_with_overrides() {
        let cli = Cli::try_parse_from([
            "ea-skill-eval",
            "--judge",
            "llm",
            "--model",
            "glm-4",
            "--base-url",
            "https://example.test/v4",
        ])
        .expect("should parse");

        assert_eq!(cli.judge, JudgeChoice::Llm);
        assert_eq!(cli.model, "glm-4");
        assert_eq!(cli.base_url, "https://example.test/v4");
    }

    #[test]
    fn no_metric_flags_means_every_metric() {
        let cli = Cli::try_parse_from(["ea-skill-eval"]).expect("should parse");

        assert!(cli.metric_filter().is_none());
    }

    #[test]
    fn metric_flags_are_repeatable() {
        let cli = Cli::try_parse_from([
            "ea-skill-eval",
            "--metric",
            "adherence",
            "--metric",
            "format",
        ])
        .expect("should parse");

        assert_eq!(
            cli.metric_filter(),
            Some(vec![
                crate::domain::Metric::InstructionAdherence,
                crate::domain::Metric::FormatCompliance,
            ])
        );
    }

    #[test]
    fn caching_is_on_by_default_for_the_llm_judge() {
        let cli = Cli::try_parse_from(["ea-skill-eval", "--judge", "llm"]).expect("should parse");

        assert!(cli.cache().is_enabled());
        assert_eq!(cli.cache_dir, PathBuf::from(DEFAULT_CACHE_DIR));
    }

    #[test]
    fn the_heuristic_judge_is_never_cached() {
        let cli = Cli::try_parse_from(["ea-skill-eval"]).expect("should parse");

        assert!(!cli.cache().is_enabled());
    }

    #[test]
    fn no_cache_disables_caching_for_the_llm_judge() {
        let cli = Cli::try_parse_from(["ea-skill-eval", "--judge", "llm", "--no-cache"])
            .expect("should parse");

        assert!(!cli.cache().is_enabled());
    }

    #[test]
    fn rejects_an_unknown_judge() {
        assert!(Cli::try_parse_from(["ea-skill-eval", "--judge", "magic"]).is_err());
    }
}
