//! Result types for expect operations

mod error;

pub use error::{ExpectError, PatternError};

/// Result of a successful pattern match.
///
/// This structure contains detailed information about a successful match,
/// including the matched text, position, text before the match, and any
/// captured groups (for regex patterns).
///
/// # Examples
///
/// ```no_run
/// use expectrust::{Session, Pattern};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let mut session = Session::spawn("echo test")?;
/// let result = session.expect(Pattern::exact("test")).await?;
///
/// println!("Matched: {}", result.matched);
/// println!("Before match: {}", result.before);
/// println!("Position: {}..{}", result.start, result.end);
/// # Ok(())
/// # }
/// ```
///
/// # Regex Captures
///
/// When using a regex pattern with capture groups, the `captures` field
/// contains all matched groups:
///
/// ```no_run
/// use expectrust::{Session, Pattern};
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// # let mut session = Session::spawn("echo user@example.com")?;
/// let pattern = Pattern::regex(r"(\w+)@(\w+)\.(\w+)").unwrap();
/// let result = session.expect(pattern).await?;
///
/// // captures[0] is the full match
/// // captures[1], [2], [3] are the captured groups
/// println!("Email: {}", result.captures[0]);
/// println!("User: {}", result.captures[1]);
/// println!("Domain: {}", result.captures[2]);
/// println!("TLD: {}", result.captures[3]);
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct MatchResult {
    /// Index of the pattern that matched (for `expect_any`).
    ///
    /// When using `expect_any` with multiple patterns, this field indicates
    /// which pattern matched (0-based index into the patterns array).
    ///
    /// For `expect` with a single pattern, this is always 0.
    pub pattern_index: usize,

    /// The matched text.
    ///
    /// Contains the exact substring that matched the pattern.
    pub matched: String,

    /// Start position of the match in the buffer (byte offset).
    pub start: usize,

    /// End position of the match in the buffer (byte offset).
    pub end: usize,

    /// Text that appeared before the match.
    ///
    /// This includes all output received before the pattern matched,
    /// which is often the most useful part for extracting command output.
    ///
    /// # Example
    ///
    /// ```no_run
    /// use expectrust::{Session, Pattern};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut session = Session::spawn("echo output")?;
    /// // Send command
    /// session.send_line("uptime").await?;
    ///
    /// // Wait for prompt
    /// let result = session.expect(Pattern::exact("$ ")).await?;
    ///
    /// // result.before contains the uptime output
    /// println!("Uptime: {}", result.before);
    /// # Ok(())
    /// # }
    /// ```
    pub before: String,

    /// Captured groups (for regex patterns).
    ///
    /// For regex patterns with capture groups, this vector contains:
    /// - Index 0: The full matched text
    /// - Index 1+: Each captured group
    ///
    /// For non-regex patterns, this vector is empty.
    pub captures: Vec<String>,
}
