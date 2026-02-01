//! ExpectRust: Process automation library for Rust
//!
//! ExpectRust is a cross-platform library for automating interactive programs,
//! inspired by the Unix `expect` utility. It provides a clean, async API for
//! spawning processes, sending input, and waiting for expected output patterns.
//!
//! # Features
//!
//! - **Cross-platform**: Works on Windows, Linux, and macOS
//! - **Async/await**: Built on tokio for efficient async I/O
//! - **Pattern matching**: Supports exact strings, regex, and glob patterns
//! - **Intelligent buffering**: Handles partial matches across buffer boundaries
//! - **Timeout support**: Built-in timeout handling for all operations
//! - **ANSI stripping**: Optional removal of ANSI escape sequences
//!
//! # Quick Start
//!
//! ```rust,no_run
//! use expectrust::{Session, Pattern};
//! use std::time::Duration;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     // Spawn a process
//!     let mut session = Session::builder()
//!         .timeout(Duration::from_secs(30))
//!         .spawn("python -i")?;
//!
//!     // Wait for the Python prompt
//!     session.expect(Pattern::exact(">>> ")).await?;
//!
//!     // Send a command
//!     session.send_line("print('Hello, World!')").await?;
//!
//!     // Wait for output
//!     let result = session.expect(Pattern::exact(">>> ")).await?;
//!     println!("Output: {}", result.before);
//!
//!     Ok(())
//! }
//! ```
//!
//! # Pattern Matching
//!
//! ExpectRust supports multiple pattern types:
//!
//! - **Exact**: Fast string matching using Boyer-Moore-Horspool
//! - **Regex**: Full regular expression support
//! - **Glob**: Shell-style wildcard patterns
//! - **EOF**: Match end of file
//! - **Timeout**: Match timeout condition
//!
//! ```rust,no_run
//! use expectrust::{Session, Pattern};
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut session = Session::spawn("echo test")?;
//! // Exact string
//! session.expect(Pattern::exact("password: ")).await?;
//!
//! // Regex
//! session.expect(Pattern::regex(r"\d+")?) .await?;
//!
//! // Glob
//! session.expect(Pattern::glob("*.txt")).await?;
//!
//! // Multiple patterns (first match wins)
//! let patterns = [
//!     Pattern::exact("success"),
//!     Pattern::exact("error"),
//!     Pattern::Eof,
//! ];
//! let result = session.expect_any(&patterns).await?;
//! match result.pattern_index {
//!     0 => println!("Success!"),
//!     1 => println!("Error occurred"),
//!     2 => println!("Process ended"),
//!     _ => unreachable!(),
//! }
//! # Ok(())
//! # }
//! ```
//!
//! # Sending Control Characters
//!
//! ExpectRust fully supports sending control characters and escape sequences:
//!
//! ```rust,no_run
//! use expectrust::Session;
//!
//! # async fn example() -> Result<(), Box<dyn std::error::Error>> {
//! # let mut session = Session::spawn("bash")?;
//! // Send Ctrl-C (interrupt)
//! session.send(&[0x03]).await?;
//!
//! // Send Ctrl-D (EOF)
//! session.send(&[0x04]).await?;
//!
//! // Send text with carriage return
//! session.send(b"password\r").await?;
//!
//! // Send ANSI escape sequences (clear screen)
//! session.send(b"\x1b[2J").await?;
//!
//! // Send arrow keys
//! session.send(b"\x1b[A").await?; // Up arrow
//! # Ok(())
//! # }
//! ```
//!
//! # Configuration
//!
//! Use `SessionBuilder` to configure sessions:
//!
//! ```rust,no_run
//! use expectrust::Session;
//! use std::time::Duration;
//!
//! # fn example() -> Result<(), Box<dyn std::error::Error>> {
//! let session = Session::builder()
//!     .timeout(Duration::from_secs(60))
//!     .max_buffer_size(16384)
//!     .strip_ansi(true)
//!     .pty_size(24, 80)
//!     .spawn("ssh user@example.com")?;
//! # Ok(())
//! # }
//! ```

#![warn(missing_docs)]

mod buffer;
mod pattern;
mod result;
mod session;

// Optional script module
#[cfg(feature = "script")]
pub mod script;

// Public API exports
pub use pattern::Pattern;
pub use result::{ExpectError, MatchResult, PatternError};
pub use session::{Session, SessionBuilder};

// Re-export commonly used types
pub use portable_pty::ExitStatus;
