//! Tcl/Expect script parser and interpreter.
//!
//! This module provides functionality to parse and execute traditional Expect scripts
//! written in Tcl syntax, allowing automation scripts from the Unix `expect` utility
//! to run with ExpectRust.
//!
//! # Features
//!
//! - Parse Tcl/Expect script syntax
//! - Execute scripts asynchronously
//! - Support core Expect commands: spawn, expect, send, close, wait
//! - Variable substitution and basic control flow
//! - Pattern matching: exact, regex, glob, timeout, eof
//!
//! # Example
//!
//! ```rust,no_run
//! use expectrust::script::Script;
//!
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let script = Script::from_str(r#"
//!         spawn python -i
//!         expect ">>> "
//!         send "print('Hello')\n"
//!         expect ">>> "
//!     "#)?;
//!
//!     script.execute().await?;
//!     Ok(())
//! }
//! ```

mod ast;
mod context;
mod error;
mod interpreter;
pub(crate) mod parser;
mod runtime;
mod value;

#[cfg(feature = "translator")]
pub mod codegen;

#[cfg(feature = "translator")]
pub mod translator;

pub use ast::{Block, Expression, Statement};
pub use error::ScriptError;
pub use value::Value;

use std::path::Path;
use std::time::Duration;

/// Result of script execution.
#[derive(Debug)]
pub struct ScriptResult {
    /// Exit status of the script.
    pub exit_status: Option<i32>,
    /// Final variable values.
    pub variables: std::collections::HashMap<String, Value>,
}

/// A parsed Expect script ready for execution.
pub struct Script {
    ast: Block,
    timeout: Option<Duration>,
    max_buffer_size: Option<usize>,
    strip_ansi: bool,
    pty_size: Option<(u16, u16)>,
}

impl Script {
    /// Parse a script from a string.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use expectrust::script::Script;
    /// let script = Script::from_str("spawn echo hello\nexpect hello")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_str(input: &str) -> Result<Self, ScriptError> {
        let ast = parser::parse_script(input)?;
        Ok(Script {
            ast,
            timeout: None,
            max_buffer_size: None,
            strip_ansi: false,
            pty_size: None,
        })
    }

    /// Parse a script from a file.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use expectrust::script::Script;
    /// let script = Script::from_file("automation.exp")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ScriptError> {
        let content = std::fs::read_to_string(path)?;
        Self::from_str(&content)
    }

    /// Create a builder for configuring script execution.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use expectrust::script::Script;
    /// # use std::time::Duration;
    /// let script = Script::builder()
    ///     .timeout(Duration::from_secs(60))
    ///     .strip_ansi(true)
    ///     .from_str("spawn python -i")?;
    /// # Ok::<(), Box<dyn std::error::Error>>(())
    /// ```
    pub fn builder() -> ScriptBuilder {
        ScriptBuilder::new()
    }

    /// Execute the script asynchronously.
    ///
    /// # Example
    ///
    /// ```rust,no_run
    /// # use expectrust::script::Script;
    /// # async fn example() -> Result<(), Box<dyn std::error::Error>> {
    /// let script = Script::from_str("spawn echo test")?;
    /// let result = script.execute().await?;
    /// println!("Exit status: {:?}", result.exit_status);
    /// # Ok(())
    /// # }
    /// ```
    pub async fn execute(self) -> Result<ScriptResult, ScriptError> {
        let mut runtime = runtime::Runtime::new(
            self.timeout,
            self.max_buffer_size,
            self.strip_ansi,
            self.pty_size,
        );

        interpreter::execute_block(&self.ast, &mut runtime).await?;

        Ok(ScriptResult {
            exit_status: runtime.exit_status(),
            variables: runtime.into_variables(),
        })
    }
}

/// Builder for configuring script execution.
pub struct ScriptBuilder {
    timeout: Option<Duration>,
    max_buffer_size: Option<usize>,
    strip_ansi: bool,
    pty_size: Option<(u16, u16)>,
}

impl ScriptBuilder {
    /// Create a new script builder.
    pub fn new() -> Self {
        Self {
            timeout: None,
            max_buffer_size: None,
            strip_ansi: false,
            pty_size: None,
        }
    }

    /// Set the default timeout for expect operations.
    pub fn timeout(mut self, timeout: Duration) -> Self {
        self.timeout = Some(timeout);
        self
    }

    /// Set the maximum buffer size for output buffering.
    pub fn max_buffer_size(mut self, size: usize) -> Self {
        self.max_buffer_size = Some(size);
        self
    }

    /// Enable or disable ANSI escape sequence stripping.
    pub fn strip_ansi(mut self, strip: bool) -> Self {
        self.strip_ansi = strip;
        self
    }

    /// Set the PTY size (rows, columns).
    pub fn pty_size(mut self, rows: u16, cols: u16) -> Self {
        self.pty_size = Some((rows, cols));
        self
    }

    /// Parse a script from a string with the configured options.
    pub fn from_str(self, input: &str) -> Result<Script, ScriptError> {
        let ast = parser::parse_script(input)?;
        Ok(Script {
            ast,
            timeout: self.timeout,
            max_buffer_size: self.max_buffer_size,
            strip_ansi: self.strip_ansi,
            pty_size: self.pty_size,
        })
    }

    /// Parse a script from a file with the configured options.
    pub fn from_file<P: AsRef<Path>>(self, path: P) -> Result<Script, ScriptError> {
        let content = std::fs::read_to_string(path)?;
        self.from_str(&content)
    }
}

impl Default for ScriptBuilder {
    fn default() -> Self {
        Self::new()
    }
}
