use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

use crate::domain::{MetricScore, Trace};
use crate::judge::prompt::PROMPT_VERSION;

/// Default on-disk location, relative to the working directory.
pub const DEFAULT_CACHE_DIR: &str = ".skill-eval-cache";

/// Identifies one judged result.
///
/// Deliberately includes a hash of the trace *content*, not just its id. Traces
/// are hand-edited while a fixture set is being built, and keying on id alone
/// would happily serve a score for text that has since changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CacheKey {
    trace_id: String,
    fingerprint: u64,
}

impl CacheKey {
    pub fn new(trace: &Trace, judge_name: &str) -> Self {
        // Everything that could change the judge's answer goes into the hash.
        let material = format!(
            "{}\u{1f}{}\u{1f}{}\u{1f}{}\u{1f}{}",
            trace.instructions, trace.task, trace.output, judge_name, PROMPT_VERSION
        );

        Self {
            trace_id: sanitise(trace.id.as_str()),
            fingerprint: fnv1a(material.as_bytes()),
        }
    }

    pub fn file_name(&self) -> String {
        format!("{}-{:016x}.json", self.trace_id, self.fingerprint)
    }
}

/// FNV-1a, so keys stay identical across machines and Rust versions.
///
/// `DefaultHasher` would be easier but its output is explicitly not stable
/// between releases, which would silently invalidate the whole cache on a
/// toolchain upgrade.
fn fnv1a(bytes: &[u8]) -> u64 {
    let mut hash: u64 = 0xcbf2_9ce4_8422_2325;

    for byte in bytes {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(0x0000_0100_0000_01b3);
    }

    hash
}

/// Keep file names predictable regardless of what a trace id contains.
fn sanitise(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || character == '-' || character == '_' {
                character
            } else {
                '_'
            }
        })
        .take(64)
        .collect()
}

#[derive(Debug, Serialize, Deserialize)]
struct CacheEntry {
    judge: String,
    prompt_version: String,
    scores: Vec<MetricScore>,
}

/// A directory of previously judged results.
#[derive(Debug, Clone)]
pub struct JudgeCache {
    root: Option<PathBuf>,
}

impl JudgeCache {
    pub fn new(root: impl Into<PathBuf>) -> Self {
        Self {
            root: Some(root.into()),
        }
    }

    /// A cache that stores nothing, for `--no-cache` and for the heuristic
    /// judge, where a disk round-trip costs more than recomputing.
    pub fn disabled() -> Self {
        Self { root: None }
    }

    pub fn is_enabled(&self) -> bool {
        self.root.is_some()
    }

    fn path_for(&self, key: &CacheKey) -> Option<PathBuf> {
        self.root.as_ref().map(|root| root.join(key.file_name()))
    }

    /// Previously stored scores, if any.
    ///
    /// A corrupt or unreadable entry is treated as a miss rather than an error:
    /// the worst case is one recomputation, and failing a run over a damaged
    /// cache file would be a poor trade.
    pub fn get(&self, key: &CacheKey) -> Option<Vec<MetricScore>> {
        let path = self.path_for(key)?;
        let body = fs::read_to_string(path).ok()?;
        let entry: CacheEntry = serde_json::from_str(&body).ok()?;

        if entry.prompt_version != PROMPT_VERSION {
            return None;
        }

        Some(entry.scores)
    }

    pub fn put(&self, key: &CacheKey, judge: &str, scores: &[MetricScore]) -> Result<()> {
        let Some(path) = self.path_for(key) else {
            return Ok(());
        };

        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).with_context(|| {
                format!("could not create cache directory {}", parent.display())
            })?;
        }

        let entry = CacheEntry {
            judge: judge.to_string(),
            prompt_version: PROMPT_VERSION.to_string(),
            scores: scores.to_vec(),
        };

        fs::write(&path, serde_json::to_string_pretty(&entry)?)
            .with_context(|| format!("could not write cache entry {}", path.display()))
    }

    /// Remove every stored entry. Backs `--clear-cache`.
    pub fn clear(&self) -> Result<usize> {
        let Some(root) = &self.root else {
            return Ok(0);
        };

        if !root.exists() {
            return Ok(0);
        }

        let mut removed = 0;
        for entry in fs::read_dir(root)
            .with_context(|| format!("could not read cache directory {}", root.display()))?
        {
            let path = entry?.path();
            if path
                .extension()
                .is_some_and(|extension| extension == "json")
            {
                fs::remove_file(&path)?;
                removed += 1;
            }
        }

        Ok(removed)
    }

    pub fn root(&self) -> Option<&Path> {
        self.root.as_deref()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::{Metric, Score, ScoreSource, TraceId};

    fn trace(id: &str, output: &str) -> Trace {
        Trace {
            id: TraceId::new(id.to_string()),
            instructions: "Reply as JSON.".to_string(),
            task: "Name a colour.".to_string(),
            output: output.to_string(),
            platform: Default::default(),
            model: None,
            prompting: Default::default(),
            skill_triggered: None,
        }
    }

    fn scores() -> Vec<MetricScore> {
        vec![MetricScore::new(
            Metric::TaskRelevancy,
            Score::new(0.75).expect("valid"),
            ScoreSource::LlmJudge,
            "on topic".to_string(),
        )]
    }

    fn temp_cache(name: &str) -> JudgeCache {
        let root = std::env::temp_dir().join(format!("skill_eval_cache_{name}"));
        let _ = fs::remove_dir_all(&root);
        JudgeCache::new(root)
    }

    #[test]
    fn stores_and_retrieves_scores() {
        let cache = temp_cache("roundtrip");
        let key = CacheKey::new(&trace("a", "Blue."), "llm:test");

        assert!(cache.get(&key).is_none());

        cache
            .put(&key, "llm:test", &scores())
            .expect("should write");

        assert_eq!(cache.get(&key), Some(scores()));
    }

    #[test]
    fn editing_the_trace_text_invalidates_the_entry() {
        let cache = temp_cache("content");
        let original = CacheKey::new(&trace("a", "Blue."), "llm:test");
        cache.put(&original, "llm:test", &scores()).expect("write");

        // Same id, different output — must not reuse the old score.
        let edited = CacheKey::new(&trace("a", "Red."), "llm:test");

        assert!(cache.get(&edited).is_none());
    }

    #[test]
    fn different_judges_do_not_share_entries() {
        let cache = temp_cache("judges");
        let first = CacheKey::new(&trace("a", "Blue."), "llm:qwen");
        cache.put(&first, "llm:qwen", &scores()).expect("write");

        let second = CacheKey::new(&trace("a", "Blue."), "llm:glm");

        assert!(cache.get(&second).is_none());
    }

    #[test]
    fn a_disabled_cache_never_stores_anything() {
        let cache = JudgeCache::disabled();
        let key = CacheKey::new(&trace("a", "Blue."), "llm:test");

        cache
            .put(&key, "llm:test", &scores())
            .expect("should be a no-op");

        assert!(!cache.is_enabled());
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn a_corrupt_entry_reads_as_a_miss() {
        let cache = temp_cache("corrupt");
        let key = CacheKey::new(&trace("a", "Blue."), "llm:test");
        let path = cache.root().unwrap().join(key.file_name());

        fs::create_dir_all(cache.root().unwrap()).expect("mkdir");
        fs::write(&path, "{ not json").expect("write garbage");

        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn clear_removes_stored_entries() {
        let cache = temp_cache("clear");
        let key = CacheKey::new(&trace("a", "Blue."), "llm:test");
        cache.put(&key, "llm:test", &scores()).expect("write");

        assert_eq!(cache.clear().expect("should clear"), 1);
        assert!(cache.get(&key).is_none());
    }

    #[test]
    fn file_names_are_filesystem_safe() {
        let key = CacheKey::new(&trace("a/b c:d", "Blue."), "llm:test");

        assert!(key.file_name().starts_with("a_b_c_d-"));
        assert!(key.file_name().ends_with(".json"));
    }

    #[test]
    fn the_fingerprint_is_stable_across_runs() {
        // Pinned so a refactor of the hashing cannot silently orphan every
        // cache entry on every user's machine.
        assert_eq!(fnv1a(b"skill-eval"), 0x8449_0107_c831_9603);
    }
}
