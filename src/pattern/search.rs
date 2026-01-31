//! Partial match tracking for patterns split across buffer boundaries

#[cfg(test)]
mod tests {
    use crate::pattern::matcher::ExactMatcher;
    use crate::pattern::Matcher;

    #[test]
    fn test_partial_match_detection() {
        let matcher = ExactMatcher::new(b"password:").unwrap();
        let buffer = b"Please enter pass";

        assert!(matcher.partial_match(buffer));
    }

    #[test]
    fn test_no_partial_match() {
        let matcher = ExactMatcher::new(b"password:").unwrap();
        let buffer = b"Please enter username";

        assert!(!matcher.partial_match(buffer));
    }
}
