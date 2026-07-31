//! Session management for PTY-based process automation

mod builder;
mod spawn;

pub use builder::SessionBuilder;

use crate::buffer::BufferManager;
use crate::pattern::Pattern;
use crate::result::{ExpectError, MatchResult};
use portable_pty::{Child, ExitStatus, MasterPty};
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
    // Only the master side is kept alive - the slave side is deliberately
    // dropped right after spawning (see `SessionBuilder::spawn_argv`), so
    // the OS considers the pty fully closed once the child exits, which is
    // what lets a master-side read return real EOF.
    _pty_master: Box<dyn MasterPty + Send>,
    child: Option<Box<dyn Child + Send>>,
    master_reader: Arc<Mutex<Box<dyn Read + Send>>>,
    master_writer: Arc<Mutex<Box<dyn Write + Send>>>,
    buffer: BufferManager,
    timeout: Option<Duration>,
    eof_reached: bool,
    max_buffer_size: usize,
}

/// How often `expect_any` checks whether the child has exited while
/// otherwise waiting for more data, so it can synthesize EOF on platforms
/// where the pty itself doesn't signal it on child exit (see
/// `docs/CORE_CAPABILITIES_TODO.md` item 18) rather than only ever
/// discovering that once the whole configured timeout has elapsed.
const CHILD_LIVENESS_POLL_INTERVAL: Duration = Duration::from_millis(250);

/// A single in-flight blocking read from the pty, reused across `expect_any`
/// loop iterations rather than respawned - see `expect_any`'s "Try to read
/// more data" section for why that matters.
type PendingRead = tokio::task::JoinHandle<std::io::Result<(usize, Vec<u8>)>>;

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

    /// Spawn a command from an already-split program and argument list
    /// (convenience method).
    ///
    /// This is a shorthand for `Session::builder().spawn_args(program, args)`.
    /// Prefer this over [`Session::spawn`] when you already have the
    /// program and its arguments as separate values (e.g. one of them
    /// contains spaces), since no string parsing happens here at all.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::Session;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let session = Session::spawn_args("ssh", &["-o", "StrictHostKeyChecking=no", "host"])?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn spawn_args<S: AsRef<str>>(program: &str, args: &[S]) -> Result<Self, ExpectError> {
        SessionBuilder::new().spawn_args(program, args)
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

        let start_time = std::time::Instant::now();

        // At most one blocking read task is kept in flight for this whole
        // call, reused across loop iterations - see the "Try to read more
        // data" section below for why that matters.
        let mut pending_read: Option<PendingRead> = None;

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

            // Try to read more data. At most one blocking read task is ever
            // outstanding at a time for this call - spawned once, then
            // polled again (not respawned) on every iteration until it
            // actually completes. This matters for correctness: if we
            // instead spawned a fresh short-lived read attempt on every
            // iteration and abandoned whichever one didn't finish in time,
            // an abandoned attempt could *still* succeed later (real bytes
            // arrive) with nobody left awaiting it - silently discarding
            // real output. Reusing the same handle means whatever it
            // eventually returns is always the thing we're waiting for.
            if pending_read.is_none() {
                let reader = self.master_reader.clone();
                pending_read = Some(tokio::task::spawn_blocking(move || {
                    let mut reader = reader.blocking_lock();
                    let mut temp_buf = vec![0u8; 4096];
                    reader.read(&mut temp_buf).map(|n| (n, temp_buf))
                }));
            }

            // Bounded by both a fixed poll interval and whatever's left of
            // the overall deadline, so we periodically get control back to
            // check the child's status without ever overshooting the
            // configured timeout by more than a negligible amount.
            let poll_duration = match timeout_duration {
                Some(timeout) => timeout
                    .saturating_sub(start_time.elapsed())
                    .min(CHILD_LIVENESS_POLL_INTERVAL),
                None => CHILD_LIVENESS_POLL_INTERVAL,
            };

            tokio::select! {
                res = pending_read.as_mut().unwrap() => {
                    pending_read = None;
                    match res {
                        Ok(Ok((0, _))) => {
                            // EOF
                            self.eof_reached = true;
                            if !has_eof {
                                return Err(ExpectError::Eof);
                            }
                        }
                        Ok(Ok((n, temp_buf))) => {
                            self.buffer.append(&temp_buf[..n])?;
                        }
                        Ok(Err(e)) if e.kind() == std::io::ErrorKind::WouldBlock => {
                            // No data available right now - a fresh read
                            // task gets spawned next iteration.
                        }
                        Ok(Err(e)) => return Err(ExpectError::IoError(e)),
                        Err(join_err) => {
                            return Err(ExpectError::IoError(std::io::Error::other(join_err)));
                        }
                    }
                }
                _ = tokio::time::sleep(poll_duration) => {
                    // No data within this window. On some platforms (ConPTY
                    // - see docs/CORE_CAPABILITIES_TODO.md item 18) the pty
                    // never signals EOF on its own once the child exits, so
                    // check independently via the child's own exit status
                    // and synthesize EOF instead of waiting for a real
                    // `read() == 0` that may never come. `pending_read` is
                    // deliberately left outstanding here (same accepted
                    // "abandoned, not killed" tradeoff already used by
                    // `interact_with`) - if the child is genuinely still
                    // alive this is a no-op and we just loop back around,
                    // with the timeout check above still enforcing the
                    // overall deadline.
                    if matches!(self.is_alive(), Ok(false)) {
                        self.eof_reached = true;
                        if !has_eof {
                            return Err(ExpectError::Eof);
                        }
                    }
                }
            }
        }
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

    /// Hand control of the process to the real user.
    ///
    /// Puts the controlling terminal into raw mode (so keystrokes, including
    /// control characters like Ctrl-C/Ctrl-D/Ctrl-Z, pass straight through
    /// to the child instead of being line-buffered or echoed by the OS) and
    /// forwards bytes bidirectionally between the real stdin/stdout and the
    /// spawned process until the process exits.
    ///
    /// Returns `Ok(())` once the child's output reaches EOF (the process
    /// exited) - this is the same default `interact` uses in the original
    /// Unix `expect`: "the default eof action is to return". The terminal's
    /// previous mode is always restored before returning, even on error.
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::{Session, Pattern};
    ///
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let mut session = Session::spawn("ssh user@example.com")?;
    /// session.expect(Pattern::exact("password: ")).await?;
    /// session.send_line("hunter2").await?;
    ///
    /// // Hand control to the user for the rest of the session.
    /// session.interact().await?;
    /// # Ok(())
    /// # }
    /// ```
    pub async fn interact(&mut self) -> Result<(), ExpectError> {
        let _raw_mode = RawModeGuard::enable()?;
        self.interact_with(std::io::stdin(), std::io::stdout())
            .await
    }

    /// Forward bytes bidirectionally between `input`/`output` and the
    /// spawned process until either side reaches EOF, without touching the
    /// real controlling terminal's mode.
    ///
    /// This is the same forwarding logic behind [`Session::interact`], but
    /// lets you supply your own I/O instead of the real stdin/stdout - e.g.
    /// to embed interact-style handoff in a GUI terminal widget, to pipe two
    /// sessions together, or (as `tests/session_interact_tests.rs` does) to
    /// exercise the forwarding behavior deterministically in tests without a
    /// real tty. Use [`Session::interact`] for the common case of handing
    /// control to the real user.
    ///
    /// Note: because the two forwarding loops each run on a blocking OS
    /// thread (`spawn_blocking`), the loop that doesn't finish first is only
    /// `abort()`ed, not actually interrupted - if it's parked in a blocking
    /// read, that thread keeps running in the background until its next read
    /// returns (e.g. the user's next keystroke, or the child producing more
    /// output) and then quietly exits. This method itself still returns as
    /// soon as either side reaches EOF or errors.
    pub async fn interact_with<R, W>(
        &mut self,
        mut input: R,
        mut output: W,
    ) -> Result<(), ExpectError>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let writer = self.master_writer.clone();
        let reader = self.master_reader.clone();

        let mut input_task = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            let mut buf = [0u8; 1024];
            loop {
                let n = input.read(&mut buf)?;
                if n == 0 {
                    // Real EOF on the input side (e.g. piped-in file closed).
                    return Ok(());
                }
                let mut w = writer.blocking_lock();
                w.write_all(&buf[..n])?;
                w.flush()?;
            }
        });

        let mut output_task = tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            let mut buf = [0u8; 4096];
            loop {
                let n = {
                    let mut r = reader.blocking_lock();
                    r.read(&mut buf)?
                };
                if n == 0 {
                    // Child exited and closed its side of the pty.
                    return Ok(());
                }
                output.write_all(&buf[..n])?;
                output.flush()?;
            }
        });

        // Grab abort handles before the tasks themselves are moved into
        // `select!` below, so whichever side finishes first can cancel the
        // other without trying to use an already-moved `JoinHandle`.
        let input_abort = input_task.abort_handle();
        let output_abort = output_task.abort_handle();

        loop {
            tokio::select! {
                res = &mut output_task => {
                    input_abort.abort();
                    res.map_err(|e| ExpectError::IoError(std::io::Error::other(e)))??;
                    break;
                }
                res = &mut input_task => {
                    output_abort.abort();
                    res.map_err(|e| ExpectError::IoError(std::io::Error::other(e)))??;
                    break;
                }
                _ = tokio::time::sleep(CHILD_LIVENESS_POLL_INTERVAL) => {
                    // Neither side has produced anything yet. On platforms
                    // where the pty doesn't signal EOF on child exit (see
                    // `docs/CORE_CAPABILITIES_TODO.md` item 18), check
                    // independently via the child's own exit status. Both
                    // blocking tasks are abandoned here rather than killed
                    // (same tradeoff already documented above) - there's
                    // nothing meaningful left to forward once the child has
                    // exited, so unlike `expect_any` there's no risk of
                    // this silently discarding data anyone still needs.
                    if matches!(self.is_alive(), Ok(false)) {
                        input_abort.abort();
                        output_abort.abort();
                        break;
                    }
                }
            }
        }

        Ok(())
    }
}

/// Detects ConPTY's startup cursor-position query (`ESC [ 6 n`, ANSI Device
/// Status Report) and returns the fixed reply that unblocks it.
///
/// `portable-pty` >= 0.9 creates ConPTY with `PSUEDOCONSOLE_INHERIT_CURSOR`,
/// which makes ConPTY block *all* child I/O until something answers this
/// query. We aren't a real terminal emulator that tracks actual cursor
/// position, so - matching the workaround other `portable-pty` consumers
/// facing the same issue have independently landed, and the upstream
/// maintainer's stated position that answering this is the PTY consumer's
/// responsibility, not a library bug - we always answer "row 1, col 1".
/// That's wrong for a program that genuinely depends on real cursor
/// tracking (rare for one-shot commands, more plausible for a full TUI
/// under `interact()`), but far better than never responding and
/// deadlocking. Unix ptys don't emit this query at all, so callers only
/// need to act on a match under `#[cfg(windows)]`; this detector itself has
/// no OS dependency, so it's exercised by a test on any platform.
#[cfg_attr(not(windows), allow(dead_code))]
fn conpty_cursor_query_response(data: &[u8]) -> Option<&'static [u8]> {
    data.windows(4)
        .any(|w| w == b"\x1b[6n")
        .then_some(b"\x1b[1;1R")
}

/// A `Read` adapter that owns the real pty reader on a dedicated background
/// thread for its entire lifetime, forwarding chunks through a channel.
///
/// This exists so ConPTY's startup cursor-position query gets answered as
/// soon as it arrives, *regardless* of whether or when the `Session`'s
/// owner ever actually reads (e.g. a caller that goes straight from
/// `spawn` to `wait()` with no `expect()` in between would otherwise never
/// see the query at all, leaving the child permanently blocked). A
/// one-shot "read once right after spawn" approach doesn't work here:
/// `portable-pty`'s Windows `try_clone_reader()` hands back a real OS-level
/// duplicate handle, and two independent handles reading the same pipe
/// race for bytes rather than each seeing a copy - so there's no safe way
/// to peek without risking stealing real output from whatever reader would
/// normally consume it. Running one thread as the sole, permanent reader
/// and forwarding everything it sees (after checking each chunk) avoids
/// that entirely.
#[cfg(windows)]
struct ConptyReader {
    rx: std::sync::mpsc::Receiver<std::io::Result<Vec<u8>>>,
    pending: Vec<u8>,
    pending_pos: usize,
}

#[cfg(windows)]
impl ConptyReader {
    fn new(mut inner: Box<dyn Read + Send>, writer: Arc<Mutex<Box<dyn Write + Send>>>) -> Self {
        let (tx, rx) = std::sync::mpsc::channel();
        std::thread::spawn(move || {
            let mut buf = [0u8; 4096];
            loop {
                match inner.read(&mut buf) {
                    Ok(0) => {
                        // EOF: an empty chunk is an unambiguous sentinel,
                        // since `Read::read` returning `Ok(0)` is only ever
                        // defined to mean EOF, never a real zero-byte read.
                        let _ = tx.send(Ok(Vec::new()));
                        break;
                    }
                    Ok(n) => {
                        let chunk = buf[..n].to_vec();
                        if let Some(response) = conpty_cursor_query_response(&chunk) {
                            let mut w = writer.blocking_lock();
                            let _ = w.write_all(response);
                            let _ = w.flush();
                        }
                        if tx.send(Ok(chunk)).is_err() {
                            // Reader side dropped; nothing left to forward to.
                            break;
                        }
                    }
                    Err(e) => {
                        let _ = tx.send(Err(e));
                        break;
                    }
                }
            }
        });

        Self {
            rx,
            pending: Vec::new(),
            pending_pos: 0,
        }
    }
}

#[cfg(windows)]
impl Read for ConptyReader {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        if self.pending_pos >= self.pending.len() {
            match self.rx.recv() {
                Ok(Ok(chunk)) if chunk.is_empty() => return Ok(0),
                Ok(Ok(chunk)) => {
                    self.pending = chunk;
                    self.pending_pos = 0;
                }
                Ok(Err(e)) => return Err(e),
                // Background thread ended without sending (shouldn't happen
                // given the loop above always sends before breaking, but
                // treat a closed channel as EOF rather than panicking).
                Err(_) => return Ok(0),
            }
        }

        let available = &self.pending[self.pending_pos..];
        let n = available.len().min(buf.len());
        buf[..n].copy_from_slice(&available[..n]);
        self.pending_pos += n;
        Ok(n)
    }
}

/// RAII guard that puts the controlling terminal into raw mode and restores
/// its previous mode when dropped, even if the guarded code returns early
/// via an error.
struct RawModeGuard;

impl RawModeGuard {
    fn enable() -> Result<Self, ExpectError> {
        crossterm::terminal::enable_raw_mode()?;
        Ok(Self)
    }
}

impl Drop for RawModeGuard {
    fn drop(&mut self) {
        let _ = crossterm::terminal::disable_raw_mode();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_raw_mode_guard_restores_on_drop() {
        // Raw mode is inherently tied to a real controlling terminal, which
        // isn't guaranteed to exist in every CI environment (e.g. output
        // fully redirected/piped). Skip rather than fail when unavailable -
        // this is an environment limitation, not a behavior we can fake.
        let guard = match RawModeGuard::enable() {
            Ok(guard) => guard,
            Err(_) => return,
        };

        assert!(
            crossterm::terminal::is_raw_mode_enabled().unwrap_or(false),
            "raw mode should be enabled while the guard is held"
        );

        drop(guard);

        assert!(
            !crossterm::terminal::is_raw_mode_enabled().unwrap_or(true),
            "raw mode should be restored once the guard is dropped"
        );
    }

    #[test]
    fn test_conpty_cursor_query_response_detects_query() {
        assert_eq!(
            conpty_cursor_query_response(b"\x1b[6n"),
            Some(&b"\x1b[1;1R"[..])
        );
    }

    #[test]
    fn test_conpty_cursor_query_response_detects_query_with_surrounding_bytes() {
        // The query can arrive amid other ConPTY setup output, not
        // necessarily as the very first/only bytes in the chunk.
        let mut data = b"\x1b[?9001h".to_vec();
        data.extend_from_slice(b"\x1b[6n");
        data.extend_from_slice(b"more output");

        assert_eq!(conpty_cursor_query_response(&data), Some(&b"\x1b[1;1R"[..]));
    }

    #[test]
    fn test_conpty_cursor_query_response_none_when_absent() {
        assert_eq!(conpty_cursor_query_response(b"hello world"), None);
        assert_eq!(conpty_cursor_query_response(b""), None);
    }
}
