use std::collections::HashSet;

fn is_stopword(word: &str) -> bool {
    matches!(
        word,
        "a" | "an" | "and" | "the" | "to" | "of" | "for" | "is" | "in" | "on" | "with" | "it"
    )
}

pub fn extract_terms(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
        .filter(|word| !is_stopword(word))
        .map(|word| word.to_string())
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
        assert!(terms.contains("files"));

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
}
