//! Session management for PTY-based process automation

mod builder;
mod spawn;

pub use builder::SessionBuilder;

use crate::buffer::BufferManager;
use crate::pattern::Pattern;
use crate::result::{ExpectError, MatchResult};
use portable_pty::{Child, ExitStatus, PtyPair};
use std::io::{Read, Write};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Main session for interacting with a spawned process.
///
/// A `Session` represents a running process with an attached PTY (pseudo-terminal).
/// It provides methods to send input to the process and wait for expected output patterns.
///
/// # Examples
///
/// ```no_run
/// use expectrust::{Session, Pattern};
/// use std::time::Duration;
///
/// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let mut session = Session::builder()
///     .timeout(Duration::from_secs(30))
///     .spawn("python -i")?;
///
/// session.expect(Pattern::exact(">>> ")).await?;
/// session.send_line("print('Hello')").await?;
/// # Ok(())
/// # }
/// ```
pub struct Session {
    _pty_pair: PtyPair,
    child: Option<Box<dyn Child + Send>>,
    master_reader: Arc<Mutex<Box<dyn Read + Send>>>,
    master_writer: Arc<Mutex<Box<dyn Write + Send>>>,
    buffer: BufferManager,
    timeout: Option<Duration>,
    eof_reached: bool,
    max_buffer_size: usize,
}

impl Session {
    /// Create a new session builder.
    ///
    /// This is the recommended way to create a session as it allows you to configure
    /// various options like timeout, buffer size, and PTY size.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::Session;
    /// use std::time::Duration;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let session = Session::builder()
    ///     .timeout(Duration::from_secs(60))
    ///     .max_buffer_size(16384)
    ///     .spawn("ssh user@host")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn builder() -> SessionBuilder {
        SessionBuilder::new()
    }

    /// Spawn a command and return a session (convenience method).
    ///
    /// This is a shorthand for `Session::builder().spawn(command)`.
    /// Use `Session::builder()` if you need to configure options.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to spawn (e.g., "python -i" or "ssh user@host")
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::Session;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let session = Session::spawn("echo Hello")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn spawn(command: &str) -> Result<Self, ExpectError> {
        SessionBuilder::new().spawn(command)
    }

    /// Wait for a pattern to appear in the output.
    ///
    /// This method blocks until the pattern is matched, EOF is reached, or a timeout occurs.
    ///
    /// # Arguments
    ///
    /// * `pattern` - The pattern to wait for
    ///
    /// # Returns
    ///
    /// A `MatchResult` containing information about the match, including the matched text,
    /// position, and text before the match.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - Timeout occurs before the pattern matches
    /// - EOF is reached before the pattern matches
    /// - An I/O error occurs
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::{Session, Pattern};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut session = Session::spawn("echo test")?;
    /// let result = session.expect(Pattern::exact("test")).await?;
    /// println!("Matched: {}", result.matched);
    /// println!("Before: {}", result.before);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn expect(&mut self, pattern: Pattern) -> Result<MatchResult, ExpectError> {
        self.expect_any(&[pattern]).await
    }

    /// Wait for any of the given patterns to appear (first-match-wins).
    ///
    /// This method checks multiple patterns concurrently and returns as soon as
    /// any one of them matches. The returned `MatchResult` includes a `pattern_index`
    /// field indicating which pattern matched.
    ///
    /// # Arguments
    ///
    /// * `patterns` - Slice of patterns to wait for
    ///
    /// # Returns
    ///
    /// A `MatchResult` with `pattern_index` indicating which pattern matched (0-based index).
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::{Session, Pattern};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut session = Session::spawn("echo test")?;
    /// let patterns = [
    ///     Pattern::exact("success"),
    ///     Pattern::exact("error"),
    ///     Pattern::Eof,
    /// ];
    ///
    /// let result = session.expect_any(&patterns).await?;
    /// match result.pattern_index {
    ///     0 => println!("Success!"),
    ///     1 => println!("Error occurred"),
    ///     2 => println!("Process ended"),
    ///     _ => unreachable!(),
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub async fn expect_any(&mut self, patterns: &[Pattern]) -> Result<MatchResult, ExpectError> {
        use crate::pattern::Matcher;

        // Build matchers for regular patterns
        let mut matchers: Vec<(usize, Box<dyn Matcher>)> = Vec::new();
        let mut has_eof = false;
        let mut has_timeout = false;
        let mut has_fullbuffer = false;

        for (idx, pattern) in patterns.iter().enumerate() {
            match pattern {
                Pattern::Eof => has_eof = true,
                Pattern::Timeout => has_timeout = true,
                Pattern::FullBuffer => has_fullbuffer = true,
                _ => {
                    if let Ok(matcher) = pattern.to_matcher() {
                        matchers.push((idx, matcher));
                    }
                }
            }
        }

        let timeout_duration = self.timeout;

        let mut read_buf = vec![0u8; 4096];
        let start_time = std::time::Instant::now();

        loop {
            // Check for matches in current buffer
            for (pattern_idx, matcher) in &matchers {
                if let Some(m) = matcher.find(self.buffer.unmatched()) {
                    // Found a match!
                    let absolute_start = self.buffer.matched_position() + m.start;
                    let absolute_end = self.buffer.matched_position() + m.end;

                    let matched = String::from_utf8_lossy(
                        &self.buffer.as_bytes()[absolute_start..absolute_end],
                    )
                    .into_owned();

                    let before =
                        String::from_utf8_lossy(self.buffer.before(absolute_start)).into_owned();

                    self.buffer.mark_matched(absolute_end);

                    return Ok(MatchResult {
                        pattern_index: *pattern_idx,
                        matched,
                        start: absolute_start,
                        end: absolute_end,
                        before,
                        captures: m.captures,
                    });
                }
            }

            // Check special patterns
            if self.eof_reached && has_eof {
                let pattern_idx = patterns
                    .iter()
                    .position(|p| matches!(p, Pattern::Eof))
                    .unwrap();
                return Ok(MatchResult {
                    pattern_index: pattern_idx,
                    matched: String::new(),
                    start: self.buffer.len(),
                    end: self.buffer.len(),
                    before: self.buffer.as_str().to_owned(),
                    captures: vec![],
                });
            }

            if self.buffer.len() >= self.max_buffer_size && has_fullbuffer {
                return Err(ExpectError::FullBuffer {
                    size: self.buffer.len(),
                });
            }

            // Check timeout
            if let Some(timeout) = timeout_duration {
                if start_time.elapsed() >= timeout {
                    if has_timeout {
                        let pattern_idx = patterns
                            .iter()
                            .position(|p| matches!(p, Pattern::Timeout))
                            .unwrap();
                        return Ok(MatchResult {
                            pattern_index: pattern_idx,
                            matched: String::new(),
                            start: self.buffer.len(),
                            end: self.buffer.len(),
                            before: self.buffer.as_str().to_owned(),
                            captures: vec![],
                        });
                    } else {
                        return Err(ExpectError::Timeout { duration: timeout });
                    }
                }
            }

            // Try to read more data
            let remaining_timeout =
                timeout_duration.map(|t| t.saturating_sub(start_time.elapsed()));

            match self
                .read_with_timeout(&mut read_buf, remaining_timeout)
                .await
            {
                Ok(0) => {
                    // EOF
                    self.eof_reached = true;
                    if !has_eof {
                        return Err(ExpectError::Eof);
                    }
                }
                Ok(n) => {
                    self.buffer.append(&read_buf[..n])?;
                }
                Err(e) if e.kind() == std::io::ErrorKind::WouldBlock => {
                    // No data available, continue loop
                    tokio::time::sleep(Duration::from_millis(10)).await;
                }
                Err(e) if e.kind() == std::io::ErrorKind::TimedOut => {
                    // Timeout from read operation
                    if has_timeout {
                        let pattern_idx = patterns
                            .iter()
                            .position(|p| matches!(p, Pattern::Timeout))
                            .unwrap();
                        return Ok(MatchResult {
                            pattern_index: pattern_idx,
                            matched: String::new(),
                            start: self.buffer.len(),
                            end: self.buffer.len(),
                            before: self.buffer.as_str().to_owned(),
                            captures: vec![],
                        });
                    } else if let Some(timeout) = timeout_duration {
                        return Err(ExpectError::Timeout { duration: timeout });
                    } else {
                        return Err(ExpectError::IoError(e));
                    }
                }
                Err(e) => return Err(ExpectError::IoError(e)),
            }
        }
    }

    /// Read with timeout
    async fn read_with_timeout(
        &mut self,
        buf: &mut [u8],
        timeout: Option<Duration>,
    ) -> std::io::Result<usize> {
        let reader = self.master_reader.clone();
        let buf_len = buf.len();

        let read_future = tokio::task::spawn_blocking(move || {
            let mut reader = reader.blocking_lock();
            let mut temp_buf = vec![0u8; buf_len];
            reader.read(&mut temp_buf).map(|n| (n, temp_buf))
        });

        let result = if let Some(timeout) = timeout {
            tokio::time::timeout(timeout, read_future)
                .await
                .map_err(|_| std::io::Error::new(std::io::ErrorKind::TimedOut, "Read timeout"))??
        } else {
            read_future.await.map_err(std::io::Error::other)?
        }?;

        let (n, temp_buf) = result;
        buf[..n].copy_from_slice(&temp_buf[..n]);
        Ok(n)
    }

    /// Send data to the process.
    ///
    /// Writes the given bytes to the process's stdin. This method flushes
    /// the output to ensure the data is sent immediately.
    ///
    /// # Arguments
    ///
    /// * `data` - The bytes to send to the process
    ///
    /// # Control Characters
    ///
    /// You can send control characters and escape sequences directly using Rust's
    /// byte string literals or byte arrays:
    ///
    /// ```no_run
    /// use expectrust::Session;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut session = Session::spawn("bash")?;
    /// // Send Ctrl-C (interrupt signal)
    /// session.send(&[0x03]).await?;
    ///
    /// // Send Ctrl-D (EOF)
    /// session.send(&[0x04]).await?;
    ///
    /// // Send carriage return
    /// session.send(b"\r").await?;
    ///
    /// // Send text with carriage return
    /// session.send(b"password\r").await?;
    ///
    /// // Send ANSI escape sequences (e.g., clear screen)
    /// session.send(b"\x1b[2J").await?;
    ///
    /// // Send arrow key (up arrow ANSI sequence)
    /// session.send(b"\x1b[A").await?;
    ///
    /// // Send null byte
    /// session.send(&[0x00]).await?;
    ///
    /// // Send multiple control characters
    /// session.send(&[0x1b, 0x5b, 0x41]).await?; // ESC [ A (up arrow)
    /// # Ok(())
    /// # }
    /// ```
    ///
    /// # Common Control Characters
    ///
    /// - `\r` (0x0D) - Carriage return
    /// - `\n` (0x0A) - Line feed (newline)
    /// - `\t` (0x09) - Tab
    /// - `0x03` - Ctrl-C (interrupt)
    /// - `0x04` - Ctrl-D (EOF)
    /// - `0x1a` - Ctrl-Z (suspend)
    /// - `0x1b` - Escape (ESC)
    /// - `0x00` - Null byte
    ///
    /// # Basic Examples
    ///
    /// ```no_run
    /// use expectrust::Session;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut session = Session::spawn("cat")?;
    /// // Send simple text
    /// session.send(b"Hello").await?;
    ///
    /// // Send text with newline
    /// session.send(b"Hello\n").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send(&mut self, data: &[u8]) -> Result<(), ExpectError> {
        let writer = self.master_writer.clone();
        let data = data.to_vec();

        tokio::task::spawn_blocking(move || {
            let mut writer = writer.blocking_lock();
            writer.write_all(&data)?;
            writer.flush()
        })
        .await
        .map_err(|e| ExpectError::IoError(std::io::Error::other(e)))??;

        Ok(())
    }

    /// Send a line to the process (appends newline).
    ///
    /// Convenience method that sends the given string followed by a newline character.
    /// Equivalent to `send(format!("{}\n", line).as_bytes())`.
    ///
    /// # Arguments
    ///
    /// * `line` - The text to send (newline will be appended)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::{Session, Pattern};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// # let mut session = Session::spawn("python -i")?;
    /// session.expect(Pattern::exact(">>> ")).await?;
    /// session.send_line("print('Hello')").await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn send_line(&mut self, line: &str) -> Result<(), ExpectError> {
        self.send(line.as_bytes()).await?;
        self.send(b"\n").await?;
        Ok(())
    }

    /// Check if the process is still alive.
    ///
    /// Returns `true` if the process is still running, `false` if it has exited.
    ///
    /// # Errors
    ///
    /// Returns an error if the process handle has been consumed by a previous
    /// call to `wait()`.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::Session;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut session = Session::spawn("sleep 10")?;
    ///
    /// if session.is_alive()? {
    ///     println!("Process is still running");
    /// }
    /// # Ok(())
    /// # }
    /// ```
    pub fn is_alive(&mut self) -> Result<bool, ExpectError> {
        match &mut self.child {
            Some(child) => spawn::is_alive(child),
            None => Err(ExpectError::ProcessExited),
        }
    }

    /// Wait for the process to exit and return its exit status.
    ///
    /// This method blocks until the process exits. After calling this method,
    /// the child process handle is consumed and subsequent calls will fail.
    ///
    /// # Returns
    ///
    /// The exit status of the process.
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The process handle has already been consumed
    /// - An I/O error occurs while waiting
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::Session;
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut session = Session::spawn("echo done")?;
    ///
    /// // ... interact with the process ...
    ///
    /// let status = session.wait().await?;
    /// println!("Process exited with: {}", status.exit_code());
    /// # Ok(())
    /// # }
    /// ```
    pub async fn wait(&mut self) -> Result<ExitStatus, ExpectError> {
        let mut child = self.child.take().ok_or(ExpectError::ProcessExited)?;

        let status = tokio::task::spawn_blocking(move || child.wait())
            .await
            .map_err(|e| ExpectError::IoError(std::io::Error::other(e)))??;

        Ok(status)
    }
}
