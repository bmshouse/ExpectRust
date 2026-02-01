//! Runtime environment for script execution.

use std::collections::HashMap;
use std::time::Duration;

use crate::script::ast::PatternType;
use crate::script::context::Context;
use crate::script::error::ScriptError;
use crate::script::value::Value;
use crate::{Pattern, Session};

/// Runtime environment managing the session and execution context.
pub struct Runtime {
    /// Active session (if spawned).
    session: Option<Session>,
    /// Execution context (variables and procedures).
    context: Context,
    /// Session configuration.
    timeout: Option<Duration>,
    max_buffer_size: Option<usize>,
    strip_ansi: bool,
    pty_size: Option<(u16, u16)>,
    /// Exit status.
    exit_status: Option<i32>,
}

impl Runtime {
    /// Create a new runtime environment.
    pub fn new(
        timeout: Option<Duration>,
        max_buffer_size: Option<usize>,
        strip_ansi: bool,
        pty_size: Option<(u16, u16)>,
    ) -> Self {
        Self {
            session: None,
            context: Context::new(),
            timeout,
            max_buffer_size,
            strip_ansi,
            pty_size,
            exit_status: None,
        }
    }

    /// Get a reference to the context.
    pub fn context(&self) -> &Context {
        &self.context
    }

    /// Get a mutable reference to the context.
    pub fn context_mut(&mut self) -> &mut Context {
        &mut self.context
    }

    /// Get a mutable reference to the active session, if any.
    pub fn session_mut(&mut self) -> Result<&mut Session, ScriptError> {
        self.session.as_mut().ok_or_else(|| {
            ScriptError::RuntimeError("No active session (call spawn first)".to_string())
        })
    }

    /// Spawn a new session with the given command.
    pub fn spawn(&mut self, command: &str) -> Result<(), ScriptError> {
        let mut builder = Session::builder();

        if let Some(timeout) = self.timeout {
            builder = builder.timeout(timeout);
        }
        if let Some(max_buffer_size) = self.max_buffer_size {
            builder = builder.max_buffer_size(max_buffer_size);
        }
        if self.strip_ansi {
            builder = builder.strip_ansi(true);
        }
        if let Some((rows, cols)) = self.pty_size {
            builder = builder.pty_size(rows, cols);
        }

        let session = builder.spawn(command)?;
        self.session = Some(session);
        Ok(())
    }

    /// Close the active session.
    pub async fn close(&mut self) -> Result<(), ScriptError> {
        // Simply drop the session - the Drop implementation will handle cleanup
        self.session = None;
        Ok(())
    }

    /// Wait for the session to exit.
    pub async fn wait(&mut self) -> Result<(), ScriptError> {
        if let Some(session) = &mut self.session {
            session.wait().await?;
        }
        Ok(())
    }

    /// Convert a PatternType from the AST to an ExpectRust Pattern.
    ///
    /// This method processes pattern strings by applying variable substitution,
    /// similar to how other string values in the script are processed.
    pub fn pattern_from_ast(&self, pattern_type: &PatternType) -> Result<Pattern, ScriptError> {
        match pattern_type {
            PatternType::Exact(s) => {
                // Apply variable substitution
                let processed = substitute_pattern_string(s, self)?;
                Ok(Pattern::exact(processed))
            }
            PatternType::Regex(s) => {
                // Apply variable substitution
                let processed = substitute_pattern_string(s, self)?;
                Pattern::regex(&processed)
                    .map_err(|e| ScriptError::PatternError(crate::PatternError::InvalidRegex(e)))
            }
            PatternType::Glob(s) => {
                // Apply variable substitution
                let processed = substitute_pattern_string(s, self)?;
                Ok(Pattern::glob(&processed))
            }
            PatternType::Eof => Ok(Pattern::Eof),
            PatternType::Timeout => Ok(Pattern::Timeout),
        }
    }

    /// Set the exit status.
    pub fn set_exit_status(&mut self, status: i32) {
        self.exit_status = Some(status);
    }

    /// Get the exit status.
    pub fn exit_status(&self) -> Option<i32> {
        self.exit_status
    }

    /// Extract variables from the context.
    pub fn into_variables(self) -> HashMap<String, Value> {
        self.context.into_variables()
    }
}

/// Apply variable substitution to a pattern string.
///
/// This performs the same variable substitution that is done for other strings
/// in the script, allowing patterns to use $variable syntax.
fn substitute_pattern_string(s: &str, runtime: &Runtime) -> Result<String, ScriptError> {
    let mut result = String::new();
    let mut chars = s.chars().peekable();

    while let Some(ch) = chars.next() {
        if ch == '$' {
            // Variable substitution
            let mut var_name = String::new();
            while let Some(&next_ch) = chars.peek() {
                if next_ch.is_alphanumeric() || next_ch == '_' {
                    var_name.push(chars.next().unwrap());
                } else {
                    break;
                }
            }

            if !var_name.is_empty() {
                let value = runtime
                    .context()
                    .get_variable(&var_name)
                    .ok_or_else(|| ScriptError::UndefinedVariable(var_name.clone()))?;
                result.push_str(&value.as_string());
            } else {
                result.push('$');
            }
        } else {
            result.push(ch);
        }
    }

    Ok(result)
}
