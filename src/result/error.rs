//! Error types for ExpectRust

use std::time::Duration;
use thiserror::Error;

/// Errors that can occur during expect operations.
///
/// This enum represents all possible errors that can occur when using ExpectRust.
/// Most methods return `Result<T, ExpectError>` to handle these error cases.
///
/// # Examples
///
/// ```no_run
/// use expectrust::{ExpectError, Pattern, Session};
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut session = Session::builder()
///     .timeout(Duration::from_secs(5))
///     .spawn("some-command")?;
///
/// match session.expect(Pattern::exact("done")).await {
///     Ok(result) => println!("Matched: {}", result.matched),
///     Err(ExpectError::Timeout { duration }) => {
///         eprintln!("Timed out after {:?}", duration);
///     }
///     Err(ExpectError::Eof) => {
///         eprintln!("Process exited unexpectedly");
///     }
///     Err(e) => return Err(e.into()),
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Error, Debug)]
pub enum ExpectError {
    /// Timeout waiting for pattern.
    ///
    /// Returned when a pattern is not matched within the configured timeout duration.
    /// To avoid this error, either increase the timeout or use `Pattern::Timeout`
    /// in `expect_any` to handle timeouts gracefully.
    #[error("Timeout waiting for pattern (after {duration:?})")]
    Timeout {
        /// Duration that was waited before timeout
        duration: Duration,
    },

    /// EOF reached before pattern matched.
    ///
    /// Returned when the process exits and closes its output stream before the
    /// expected pattern is found. To handle EOF gracefully, use `Pattern::Eof`
    /// in `expect_any`.
    #[error("EOF reached before pattern matched")]
    Eof,

    /// Buffer full before pattern matched.
    ///
    /// Returned when the internal buffer reaches its maximum size without finding
    /// a match. This usually indicates unexpected output or a pattern that doesn't
    /// match. Increase `max_buffer_size` or use `Pattern::FullBuffer` to handle
    /// this gracefully.
    #[error("Buffer full ({size} bytes)")]
    FullBuffer {
        /// Size of the buffer when it became full
        size: usize,
    },

    /// Invalid pattern.
    ///
    /// Returned when creating a pattern with invalid syntax (e.g., invalid regex).
    #[error("Invalid pattern: {0}")]
    PatternError(#[from] PatternError),

    /// I/O error.
    ///
    /// Returned when an underlying I/O operation fails (reading from PTY, writing
    /// to PTY, etc.).
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),

    /// PTY error.
    ///
    /// Returned when PTY creation or manipulation fails.
    #[error("PTY error: {0}")]
    PtyError(String),

    /// Process spawning error.
    ///
    /// Returned when the specified command cannot be spawned (command not found,
    /// permission denied, etc.).
    #[error("Failed to spawn process: {0}")]
    SpawnError(String),

    /// Process already exited.
    ///
    /// Returned when attempting to interact with a process that has already been
    /// waited on (via `Session::wait()`).
    #[error("Process has already exited")]
    ProcessExited,
}

/// Errors related to pattern creation or matching.
///
/// These errors occur when creating invalid patterns (e.g., invalid regex syntax).
#[derive(Error, Debug)]
pub enum PatternError {
    /// Invalid regex pattern.
    ///
    /// Returned when `Pattern::regex()` is called with invalid regex syntax.
    #[error("Invalid regex: {0}")]
    InvalidRegex(#[from] regex::Error),

    /// Invalid glob pattern.
    ///
    /// Returned when `Pattern::glob()` is called with invalid glob syntax.
    #[error("Invalid glob: {0}")]
    InvalidGlob(String),

    /// Empty pattern.
    ///
    /// Returned when attempting to create a pattern with an empty string.
    #[error("Pattern cannot be empty")]
    EmptyPattern,
}
