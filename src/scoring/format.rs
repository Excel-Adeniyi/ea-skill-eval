use std::sync::LazyLock;

use regex::Regex;

/// A formatting demand detected in a trace's instructions.
///
/// Format compliance is only meaningful when the instructions actually asked
/// for a format. If they didn't, there is nothing to comply with, and the
/// metric is skipped rather than scored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FormatRule {
    Json,
    BulletList,
    NumberedList,
    Heading,
    CodeBlock,
    MaxWords(usize),
}

impl FormatRule {
    /// Does `output` satisfy this rule?
    pub fn is_satisfied_by(self, output: &str) -> bool {
        match self {
            Self::Json => looks_like_json(output),
            Self::BulletList => BULLET_LINE.is_match(output),
            Self::NumberedList => NUMBERED_LINE.is_match(output),
            Self::Heading => HEADING_LINE.is_match(output),
            Self::CodeBlock => output.contains("```"),
            Self::MaxWords(limit) => word_count(output) <= limit,
        }
    }

    pub fn describe(self) -> String {
        match self {
            Self::Json => "JSON output".to_string(),
            Self::BulletList => "a bullet list".to_string(),
            Self::NumberedList => "a numbered list".to_string(),
            Self::Heading => "a heading".to_string(),
            Self::CodeBlock => "a code block".to_string(),
            Self::MaxWords(limit) => format!("at most {limit} words"),
        }
    }
}

// `LazyLock` compiles each regex once, on first use, and reuses it for the rest
// of the process. Compiling a `Regex` is expensive; doing it inside a function
// called once per trace per metric would dominate the runtime.
static ASKS_FOR_JSON: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(json|as json|valid json)\b").unwrap());
static ASKS_FOR_BULLETS: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(bullet|bulleted|bullet points?)\b").unwrap());
static ASKS_FOR_NUMBERED: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(numbered list|numbered steps?|ordered list)\b").unwrap());
static ASKS_FOR_HEADING: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(headings?|section titles?)\b").unwrap());
static ASKS_FOR_CODE_BLOCK: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?i)\b(code block|code fence|fenced code)\b").unwrap());
/// Matches a word limit that applies to part of the output, not all of it —
/// "keep each reasoning under 25 words". Treating that as a whole-output cap
/// fails every correct multi-field response.
static SCOPED_WORD_LIMIT: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:each|every|per)\b[^.]{0,40}?\b(?:at most|no more than|under|max(?:imum)? of|within)\s+\d+\s+words\b",
    )
    .unwrap()
});

static ASKS_FOR_MAX_WORDS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?i)\b(?:at most|no more than|under|max(?:imum)? of|within)\s+(\d+)\s+words\b")
        .unwrap()
});

static BULLET_LINE: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?m)^\s*[-*+•]\s+\S").unwrap());
static NUMBERED_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*\d+[.)]\s+\S").unwrap());
static HEADING_LINE: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?m)^\s*#{1,6}\s+\S").unwrap());

/// Extract every formatting demand stated in `instructions`.
pub fn detect_rules(instructions: &str) -> Vec<FormatRule> {
    let mut rules = Vec::new();

    if ASKS_FOR_JSON.is_match(instructions) {
        rules.push(FormatRule::Json);
    }
    if ASKS_FOR_BULLETS.is_match(instructions) {
        rules.push(FormatRule::BulletList);
    }
    if ASKS_FOR_NUMBERED.is_match(instructions) {
        rules.push(FormatRule::NumberedList);
    }
    if ASKS_FOR_HEADING.is_match(instructions) {
        rules.push(FormatRule::Heading);
    }
    if ASKS_FOR_CODE_BLOCK.is_match(instructions) {
        rules.push(FormatRule::CodeBlock);
    }
    // A scoped limit ("each reasoning under 25 words") cannot be checked
    // without knowing the output's structure, so it is not scored at all
    // rather than scored wrongly. Capture group 1 matched `\d+`, so the parse
    // only fails on a number too large for `usize`, which we ignore.
    if !SCOPED_WORD_LIMIT.is_match(instructions)
        && let Some(captures) = ASKS_FOR_MAX_WORDS.captures(instructions)
        && let Ok(limit) = captures[1].parse::<usize>()
    {
        rules.push(FormatRule::MaxWords(limit));
    }

    rules
}

/// Imperative verbs that introduce an instruction rather than being part of it.
///
/// "Mention ownership" asks for *ownership* in the output, not the word
/// "mention". Counting the verb against recall penalises every output that
/// follows the instruction without quoting it.
static DIRECTIVE_VERBS: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(
        r"(?i)\b(?:reply|respond|answer|mention|include|state|provide|write|use|using|keep|ensure|make sure|be sure|output|return|give)\b",
    )
    .unwrap()
});

/// Remove phrases that state *how* to answer, leaving *what* to say.
///
/// Instruction recall asks whether the output covered the content it was asked
/// for. Format directives are a different dimension, already scored by
/// [`detect_rules`] — leaving them in the term set means an output that
/// correctly replies in JSON is penalised for not containing the word "JSON".
pub fn strip_format_phrases(instructions: &str) -> String {
    let mut cleaned = instructions.to_string();

    for pattern in [
        &*ASKS_FOR_JSON,
        &*ASKS_FOR_BULLETS,
        &*ASKS_FOR_NUMBERED,
        &*ASKS_FOR_HEADING,
        &*ASKS_FOR_CODE_BLOCK,
        &*ASKS_FOR_MAX_WORDS,
        &*SCOPED_WORD_LIMIT,
        &*DIRECTIVE_VERBS,
    ] {
        cleaned = pattern.replace_all(&cleaned, " ").to_string();
    }

    cleaned
}

fn word_count(text: &str) -> usize {
    text.split_whitespace().count()
}

fn looks_like_json(text: &str) -> bool {
    let trimmed = text.trim();
    let fenced = trimmed
        .trim_start_matches("```json")
        .trim_start_matches("```")
        .trim_end_matches("```")
        .trim();

    serde_json::from_str::<serde_json::Value>(fenced).is_ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_no_rules_when_no_format_was_requested() {
        assert!(detect_rules("Explain Rust structs clearly.").is_empty());
    }

    #[test]
    fn detects_a_json_requirement() {
        assert_eq!(detect_rules("Reply as JSON."), vec![FormatRule::Json]);
    }

    #[test]
    fn detects_a_word_limit() {
        assert_eq!(
            detect_rules("Summarise in at most 20 words."),
            vec![FormatRule::MaxWords(20)]
        );
        assert_eq!(
            detect_rules("Keep it under 5 words."),
            vec![FormatRule::MaxWords(5)]
        );
    }

    #[test]
    fn detects_several_rules_at_once() {
        let rules = detect_rules("Use bullet points with headings, no more than 100 words.");

        assert!(rules.contains(&FormatRule::BulletList));
        assert!(rules.contains(&FormatRule::Heading));
        assert!(rules.contains(&FormatRule::MaxWords(100)));
    }

    #[test]
    fn json_rule_accepts_valid_json_only() {
        assert!(FormatRule::Json.is_satisfied_by(r#"{"score": 1}"#));
        assert!(FormatRule::Json.is_satisfied_by("```json\n[1, 2]\n```"));
        assert!(!FormatRule::Json.is_satisfied_by("Sure! Here is the answer."));
    }

    #[test]
    fn bullet_rule_accepts_common_bullet_characters() {
        assert!(FormatRule::BulletList.is_satisfied_by("- first\n- second"));
        assert!(FormatRule::BulletList.is_satisfied_by("* first"));
        assert!(!FormatRule::BulletList.is_satisfied_by("first, second"));
    }

    #[test]
    fn numbered_rule_requires_leading_numbers() {
        assert!(FormatRule::NumberedList.is_satisfied_by("1. first\n2. second"));
        assert!(FormatRule::NumberedList.is_satisfied_by("1) first"));
        assert!(!FormatRule::NumberedList.is_satisfied_by("- first"));
    }

    #[test]
    fn word_limit_rule_counts_words() {
        assert!(FormatRule::MaxWords(3).is_satisfied_by("one two three"));
        assert!(!FormatRule::MaxWords(3).is_satisfied_by("one two three four"));
    }

    #[test]
    fn a_scoped_word_limit_is_not_a_whole_output_limit() {
        let rules = detect_rules("Reply as JSON. Keep each reasoning under 25 words.");

        assert!(rules.contains(&FormatRule::Json));
        assert!(
            !rules
                .iter()
                .any(|rule| matches!(rule, FormatRule::MaxWords(_))),
            "a per-field limit must not be scored against the whole output"
        );
    }

    #[test]
    fn an_unscoped_word_limit_still_applies() {
        assert_eq!(
            detect_rules("Answer in under 20 words."),
            vec![FormatRule::MaxWords(20)]
        );
    }

    #[test]
    fn strips_format_directives_from_instructions() {
        let stripped =
            strip_format_phrases("Reply as JSON using bullet points. Mention ownership.");

        assert!(!stripped.to_lowercase().contains("json"));
        assert!(!stripped.to_lowercase().contains("bullet"));
        // The directive verb goes too; its object is what recall should want.
        assert!(!stripped.to_lowercase().contains("mention"));
        assert!(stripped.contains("ownership"));
    }

    #[test]
    fn strips_directive_verbs_but_keeps_their_object() {
        let stripped = strip_format_phrases("Mention ownership and borrowing.");

        assert!(!stripped.to_lowercase().contains("mention"));
        assert!(stripped.contains("ownership"));
        assert!(stripped.contains("borrowing"));
    }
}
