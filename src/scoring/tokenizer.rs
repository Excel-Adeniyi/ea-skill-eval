use std::collections::HashSet;

pub fn extract_terms(_text: &str) -> HashSet<String> {
    _text
        .to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|word| !word.is_empty())
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
        assert!(terms.contains("Rust"));
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
}
