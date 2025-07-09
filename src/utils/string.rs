//! String manipulation utilities

/// Pluralize a word based on count
pub fn pluralize(word: &str, count: usize) -> String {
    if count == 1 {
        word.to_string()
    } else {
        format!("{word}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pluralize() {
        assert_eq!(pluralize("crate", 0), "crates");
        assert_eq!(pluralize("crate", 1), "crate");
        assert_eq!(pluralize("crate", 5), "crates");
    }
}
