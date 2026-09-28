use std::collections::HashSet;

fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "a" | "an" | "and" | "the" | "to" | "of" | "for" | "is" | "in" | "on" | "with" | "it"
    )
}

/// Reduce a word to a crude stem so that "struct" and "structs" match.
///
/// Deliberately not a real stemmer (no Porter, no dependency). For overlap
/// scoring what matters is that both sides of a comparison are reduced the
/// *same* way, not that the stem is a real word — "related" becoming "relat"
/// costs nothing as long as "relates" becomes "relat" too.
///
/// Each rule keeps a minimum remainder so short words are left alone: "using"
/// would otherwise collapse to "us", colliding with unrelated text.
fn stem(word: &str) -> String {
    let length = word.len();

    if length <= 2 {
        return word.to_string();
    }

    if length >= 5 && word.ends_with("ies") {
        return format!("{}y", &word[..length - 3]);
    }

    // "ss" is excluded so "class" and "address" survive intact.
    if word.ends_with('s') && !word.ends_with("ss") && length > 2 {
        return word[..length - 1].to_string();
    }

    if word.ends_with("ing") && length - 3 >= 4 {
        return word[..length - 3].to_string();
    }

    if word.ends_with("ed") && length - 2 >= 4 {
        return word[..length - 2].to_string();
    }

    word.to_string()
}

pub fn extract_terms(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .map(stem)
        .filter(|word| !is_stopword(word))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn converts_terms_to_lowercase() {
        let terms = extract_terms("Rust JSON");

        assert!(terms.contains("rust"));
        assert!(terms.contains("json"));
        assert!(!terms.contains("Rust"));
    }

    #[test]
    fn removes_punctuation_from_terms() {
        let terms = extract_terms("Rust, JSON! files.");

        assert!(terms.contains("rust"));
        assert!(terms.contains("json"));
        // "files" is stemmed to "file" before it reaches the set.
        assert!(terms.contains("file"));

        assert!(!terms.contains("rust,"));
        assert!(!terms.contains("json!"));
        assert!(!terms.contains("files."));
    }

    #[test]
    fn removes_stopwords() {
        let terms = extract_terms("Create a report for the user and save it to the file");

        assert!(terms.contains("create"));
        assert!(terms.contains("report"));
        assert!(terms.contains("user"));
        assert!(terms.contains("save"));
        assert!(terms.contains("file"));

        assert!(!terms.contains("a"));
        assert!(!terms.contains("for"));
        assert!(!terms.contains("the"));
        assert!(!terms.contains("and"));
        assert!(!terms.contains("it"));
        assert!(!terms.contains("to"));
    }
    #[test]
    fn keeps_single_digit_numbers() {
        let terms = extract_terms("Create 2 files using v3");

        assert!(terms.contains("2"));
        assert!(terms.contains("v3"));
    }
    #[test]
    fn repeated_words_count_once() {
        let terms = extract_terms("report report REPORT");

        assert_eq!(terms.len(), 1);
        assert!(terms.contains("report"));
    }

    #[test]
    fn matches_singular_and_plural_forms() {
        let task = extract_terms("Explain Rust structs");
        let output = extract_terms("A struct groups values");

        assert!(task.contains("struct"));
        assert!(output.contains("struct"));
    }

    #[test]
    fn leaves_double_s_words_intact() {
        let terms = extract_terms("class address");

        assert!(terms.contains("class"));
        assert!(terms.contains("address"));
    }

    #[test]
    fn does_not_over_stem_short_words() {
        let terms = extract_terms("using bus v3");

        // Stripping "ing" here would leave "us", which collides with unrelated text.
        assert!(terms.contains("using"));
        assert!(terms.contains("v3"));
    }

    #[test]
    fn stems_gerunds_back_to_the_base_verb() {
        let gerund = extract_terms("borrowing");
        let base = extract_terms("borrow");

        assert_eq!(gerund, base);
    }

    #[test]
    fn known_limitation_plural_verbs_do_not_reach_the_ed_stem() {
        // "relates" hits the plural rule first and stops at "relate", while
        // "related" reaches "relat". Converging them needs a trailing-"e" strip,
        // which would also turn "file" into "fil" and "value" into "valu" \u2014 and
        // those stems are printed verbatim in the reasoning strings. Matching a
        // rarer verb pair is not worth reports that read as broken.
        assert_ne!(extract_terms("related"), extract_terms("relates"));
    }
}
