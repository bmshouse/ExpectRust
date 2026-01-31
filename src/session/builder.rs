//! Session builder for configuration

use crate::buffer::BufferManager;
use crate::result::ExpectError;
use crate::session::Session;
use portable_pty::{native_pty_system, CommandBuilder, PtySize};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;

/// Default timeout for expect operations (in seconds)
const DEFAULT_TIMEOUT_SECS: u64 = 30;

/// Default maximum buffer size (in bytes)
const DEFAULT_MAX_BUFFER_SIZE: usize = 8192;

/// Default PTY rows
const DEFAULT_PTY_ROWS: u16 = 24;

/// Default PTY columns
const DEFAULT_PTY_COLS: u16 = 80;

/// Builder for configuring and spawning sessions.
///
/// Provides a fluent interface for configuring session options before spawning a process.
///
/// # Defaults
///
/// - Timeout: 30 seconds
/// - Max buffer size: 8192 bytes
/// - ANSI stripping: disabled
/// - PTY size: 24 rows × 80 columns
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
///     .strip_ansi(true)
///     .pty_size(40, 120)
///     .spawn("python -i")?;
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone)]
pub struct SessionBuilder {
    timeout: Option<Duration>,
    max_buffer_size: usize,
    strip_ansi: bool,
    pty_size: PtySize,
}

impl Default for SessionBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl SessionBuilder {
    /// Create a new session builder with default configuration.
    ///
    /// See the [`SessionBuilder`] documentation for default values.
    pub fn new() -> Self {
        Self {
            timeout: Some(Duration::from_secs(DEFAULT_TIMEOUT_SECS)),
            max_buffer_size: DEFAULT_MAX_BUFFER_SIZE,
            strip_ansi: false,
            pty_size: PtySize {
                rows: DEFAULT_PTY_ROWS,
                cols: DEFAULT_PTY_COLS,
                pixel_width: 0,
                pixel_height: 0,
            },
        }
    }

    /// Set the timeout for expect operations.
    ///
    /// If a pattern is not matched within this duration, `expect()` will return
    /// a timeout error unless `Pattern::Timeout` is in the pattern list.
    ///
    /// # Arguments
    ///
    /// * `timeout` - The duration to wait before timing out
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
    ///     .spawn("python -i")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Disable timeout (wait indefinitely).
    ///
    /// When timeout is disabled, `expect()` will wait forever unless the pattern
    /// matches or EOF is reached.
    pub fn no_timeout(mut self) -> Self {
        self.timeout = None;
        self
    }

    /// Set maximum buffer size in bytes.
    ///
    /// When the buffer reaches this size, old data is discarded using a 2/3 strategy
    /// (discard oldest 1/3, keep newest 2/3).
    ///
    /// # Arguments
    ///
    /// * `size` - Maximum buffer size in bytes (default: 8192)
    pub fn max_buffer_size(mut self, size: usize) -> Self {
        self.max_buffer_size = size;
        self
    }

    /// Enable or disable ANSI escape sequence stripping.
    ///
    /// When enabled, ANSI escape sequences (colors, cursor movements, etc.) are
    /// automatically removed from the output before pattern matching.
    ///
    /// # Arguments
    ///
    /// * `strip` - `true` to strip ANSI sequences, `false` to keep them (default: `false`)
    pub fn strip_ansi(mut self, strip: bool) -> Self {
        self.strip_ansi = strip;
        self
    }

    /// Set PTY (terminal) size.
    ///
    /// This affects how the spawned process sees the terminal dimensions.
    ///
    /// # Arguments
    ///
    /// * `rows` - Number of rows (default: 24)
    /// * `cols` - Number of columns (default: 80)
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::Session;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let session = Session::builder()
    ///     .pty_size(40, 120)  // Larger terminal
    ///     .spawn("vi")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn pty_size(mut self, rows: u16, cols: u16) -> Self {
        self.pty_size = PtySize {
            rows,
            cols,
            pixel_width: 0,
            pixel_height: 0,
        };
        self
    }

    /// Spawn a command and return a configured session.
    ///
    /// This method consumes the builder and creates a new session with the
    /// configured options.
    ///
    /// # Arguments
    ///
    /// * `command` - The command to spawn (e.g., "python -i", "ssh user@host")
    ///
    /// # Errors
    ///
    /// Returns an error if:
    /// - The command string is empty
    /// - The PTY cannot be created
    /// - The process cannot be spawned
    ///
    /// # Examples
    ///
    /// ```no_run
    /// use expectrust::Session;
    /// use std::time::Duration;
    ///
    /// # fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let session = Session::builder()
    ///     .timeout(Duration::from_secs(30))
    ///     .spawn("python -i")?;
    /// # Ok(())
    /// # }
    /// ```
    pub fn spawn(self, command: &str) -> Result<Session, ExpectError> {
        let pty_system = native_pty_system();

        // Create PTY pair
        let pty_pair = pty_system
            .openpty(self.pty_size)
            .map_err(|e| ExpectError::PtyError(e.to_string()))?;

        // Parse command into parts
        let parts: Vec<&str> = command.split_whitespace().collect();
        if parts.is_empty() {
            return Err(ExpectError::SpawnError("Empty command".to_string()));
        }

        // Build command
        let mut cmd = CommandBuilder::new(parts[0]);
        for arg in &parts[1..] {
            cmd.arg(arg);
        }

        // Spawn child process
        let child = pty_pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| ExpectError::SpawnError(e.to_string()))?;

        // Get reader and writer from the master PTY
        let reader = pty_pair
            .master
            .try_clone_reader()
            .map_err(|e| ExpectError::PtyError(e.to_string()))?;

        // For writing, portable_pty uses take_writer() which consumes ownership
        // We need to get the writer before storing the pty_pair
        let writer = pty_pair
            .master
            .take_writer()
            .map_err(|e| ExpectError::PtyError(e.to_string()))?;

        Ok(Session {
            _pty_pair: pty_pair,
            child: Some(child),
            master_reader: Arc::new(Mutex::new(reader)),
            master_writer: Arc::new(Mutex::new(writer)),
            buffer: BufferManager::new(self.max_buffer_size, self.strip_ansi),
            timeout: self.timeout,
            eof_reached: false,
            max_buffer_size: self.max_buffer_size,
        })
    }
}
