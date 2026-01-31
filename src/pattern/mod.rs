//! Pattern matching for expect operations

mod matcher;
mod search;

pub use matcher::Matcher;

use regex::Regex;

/// Pattern types for matching process output.
///
/// ExpectRust supports multiple types of patterns for flexible matching of process output.
/// Each pattern type has different performance characteristics and use cases.
///
/// # Pattern Types
///
/// - **Exact**: Fast exact string matching using Boyer-Moore-Horspool algorithm
/// - **Regex**: Full regular expression support with capture groups
/// - **Glob**: Shell-style wildcard patterns (*, ?, etc.)
/// - **Eof**: Special pattern that matches when the process exits
/// - **Timeout**: Special pattern that matches when a timeout occurs
/// - **FullBuffer**: Special pattern that matches when the buffer is full
/// - **Null**: Matches a null byte (\0)
///
/// # Examples
///
/// ```
/// use expectrust::Pattern;
///
/// // Exact string (fastest)
/// let p1 = Pattern::exact("password: ");
///
/// // Regular expression
/// let p2 = Pattern::regex(r"\d+").unwrap();
///
/// // Glob pattern
/// let p3 = Pattern::glob("*.txt");
///
/// // Special patterns
/// let p4 = Pattern::Eof;
/// let p5 = Pattern::Timeout;
/// ```
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Exact string match (most efficient).
    ///
    /// Uses Boyer-Moore-Horspool algorithm for O(n/m) average-case performance.
    /// This is the fastest pattern type and should be preferred when possible.
    Exact(String),

    /// Regular expression match.
    ///
    /// Supports full regex syntax including capture groups. The matched text and
    /// all capture groups are returned in the `MatchResult`.
    Regex(Regex),

    /// Glob pattern match (shell-style wildcards).
    ///
    /// Supports patterns like `*.txt`, `test?.log`, etc.
    ///
    /// **Performance Note**: Glob matching uses an O(n²) algorithm and is
    /// significantly less efficient than exact or regex matching. For performance-
    /// critical code, prefer `Pattern::exact()` or `Pattern::regex()`.
    Glob(String),

    /// Match end of file.
    ///
    /// This pattern matches when the process exits and no more output is available.
    /// Useful for waiting until a process completes.
    Eof,

    /// Match timeout condition.
    ///
    /// This pattern matches when the configured timeout expires. When used with
    /// `expect_any`, it allows graceful handling of timeouts instead of errors.
    Timeout,

    /// Match when buffer is full.
    ///
    /// This pattern matches when the internal buffer reaches its maximum size
    /// without finding a match. Useful for detecting unexpected output floods.
    FullBuffer,

    /// Match null byte.
    ///
    /// Matches the first occurrence of a null byte (\0) in the output.
    Null,
}

impl Pattern {
    /// Create an exact string pattern.
    ///
    /// This is the most efficient pattern type and should be used when you know
    /// the exact string you're looking for.
    ///
    /// # Examples
    ///
    /// ```
    /// use expectrust::Pattern;
    ///
    /// let pattern = Pattern::exact("$ ");
    /// let pattern2 = Pattern::exact(String::from(">>> "));
    /// ```
    pub fn exact(s: impl Into<String>) -> Self {
        Pattern::Exact(s.into())
    }

    /// Create a regex pattern.
    ///
    /// Supports full regex syntax. Returns an error if the pattern is invalid.
    ///
    /// # Arguments
    ///
    /// * `pattern` - A valid regular expression string
    ///
    /// # Errors
    ///
    /// Returns a regex error if the pattern is invalid.
    ///
    /// # Examples
    ///
    /// ```
    /// use expectrust::Pattern;
    ///
    /// // Match digits
    /// let pattern = Pattern::regex(r"\d+").unwrap();
    ///
    /// // Match email with capture groups
    /// let pattern = Pattern::regex(r"(\w+)@(\w+)\.(\w+)").unwrap();
    ///
    /// // Case-insensitive
    /// let pattern = Pattern::regex(r"(?i)hello").unwrap();
    /// ```
    pub fn regex(pattern: &str) -> Result<Self, regex::Error> {
        Ok(Pattern::Regex(Regex::new(pattern)?))
    }

    /// Create a glob pattern.
    ///
    /// Supports shell-style wildcards like `*`, `?`, etc.
    ///
    /// # Examples
    ///
    /// ```
    /// use expectrust::Pattern;
    ///
    /// let pattern = Pattern::glob("*.txt");
    /// let pattern2 = Pattern::glob("test?.log");
    /// ```
    pub fn glob(pattern: &str) -> Self {
        Pattern::Glob(pattern.to_string())
    }

    /// Convert pattern to a matcher implementation
    pub fn to_matcher(&self) -> Result<Box<dyn Matcher>, crate::result::PatternError> {
        use matcher::{ExactMatcher, GlobMatcher as GlobMatcherImpl, NullMatcher, RegexMatcher};

        match self {
            Pattern::Exact(s) => Ok(Box::new(ExactMatcher::new(s.as_bytes())?)),
            Pattern::Regex(r) => Ok(Box::new(RegexMatcher::new(r.as_str())?)),
            Pattern::Glob(g) => Ok(Box::new(GlobMatcherImpl::new(g)?)),
            Pattern::Null => Ok(Box::new(NullMatcher)),
            Pattern::Eof | Pattern::Timeout | Pattern::FullBuffer => {
                // These are handled specially in expect logic
                Err(crate::result::PatternError::InvalidGlob(
                    "Special patterns don't have matchers".to_string(),
                ))
            }
        }
    }

    /// Check if this is a special pattern (EOF, Timeout, FullBuffer)
    pub fn is_special(&self) -> bool {
        matches!(self, Pattern::Eof | Pattern::Timeout | Pattern::FullBuffer)
    }
}
