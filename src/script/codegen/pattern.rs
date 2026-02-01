//! Pattern code generation.

use super::TranslationError;
use crate::script::ast::*;

/// Generate Rust code for a pattern.
pub fn generate_pattern(pattern_type: &PatternType) -> Result<String, TranslationError> {
    match pattern_type {
        PatternType::Exact(s) => Ok(format!("Pattern::exact(\"{}\")", escape_string(s))),
        PatternType::Regex(r) => Ok(format!("Pattern::regex(r\"{}\")?", escape_regex(r))),
        PatternType::Glob(g) => Ok(format!("Pattern::glob(\"{}\")", escape_string(g))),
        PatternType::Eof => Ok("Pattern::Eof".to_string()),
        PatternType::Timeout => Ok("Pattern::Timeout".to_string()),
    }
}

/// Escape special characters in a string for Rust string literal.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Escape special characters in a regex for Rust raw string literal.
fn escape_regex(s: &str) -> String {
    // In raw strings, we only need to escape quotes that would end the string
    // For now, we'll just return as-is since we're using r"..." notation
    s.replace('"', "\\\"")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_exact_pattern() {
        let result = generate_pattern(&PatternType::Exact("hello".to_string())).unwrap();
        assert_eq!(result, "Pattern::exact(\"hello\")");
    }

    #[test]
    fn test_generate_regex_pattern() {
        let result = generate_pattern(&PatternType::Regex("\\d+".to_string())).unwrap();
        assert_eq!(result, "Pattern::regex(r\"\\d+\")?");
    }

    #[test]
    fn test_generate_eof_pattern() {
        let result = generate_pattern(&PatternType::Eof).unwrap();
        assert_eq!(result, "Pattern::Eof");
    }
}
