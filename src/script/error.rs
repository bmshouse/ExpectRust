//! Error types for script parsing and execution.

use std::fmt;

/// Errors that can occur during script parsing and execution.
#[derive(Debug)]
pub enum ScriptError {
    /// Error parsing the script.
    ParseError {
        /// Line number where the error occurred.
        line: usize,
        /// Column number where the error occurred.
        col: usize,
        /// Error message.
        message: String,
    },
    /// Runtime error during script execution.
    RuntimeError(String),
    /// Undefined variable referenced.
    UndefinedVariable(String),
    /// Undefined procedure called.
    UndefinedProcedure(String),
    /// Type error during evaluation.
    TypeError {
        /// Expected type.
        expected: String,
        /// Actual type.
        actual: String,
    },
    /// Error from the Expect session.
    ExpectError(crate::ExpectError),
    /// I/O error.
    IoError(std::io::Error),
    /// Pattern compilation error.
    PatternError(crate::PatternError),
    /// Script exited with a code.
    Exit(i32),
}

impl fmt::Display for ScriptError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ScriptError::ParseError { line, col, message } => {
                write!(
                    f,
                    "Parse error at line {}, column {}: {}",
                    line, col, message
                )
            }
            ScriptError::RuntimeError(msg) => write!(f, "Runtime error: {}", msg),
            ScriptError::UndefinedVariable(name) => {
                write!(f, "Undefined variable: {}", name)
            }
            ScriptError::UndefinedProcedure(name) => {
                write!(f, "Undefined procedure: {}", name)
            }
            ScriptError::TypeError { expected, actual } => {
                write!(f, "Type error: expected {}, got {}", expected, actual)
            }
            ScriptError::ExpectError(e) => write!(f, "Expect error: {}", e),
            ScriptError::IoError(e) => write!(f, "I/O error: {}", e),
            ScriptError::PatternError(e) => write!(f, "Pattern error: {}", e),
            ScriptError::Exit(code) => write!(f, "Script exited with code {}", code),
        }
    }
}

impl std::error::Error for ScriptError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            ScriptError::ExpectError(e) => Some(e),
            ScriptError::IoError(e) => Some(e),
            ScriptError::PatternError(e) => Some(e),
            _ => None,
        }
    }
}

impl From<crate::ExpectError> for ScriptError {
    fn from(e: crate::ExpectError) -> Self {
        ScriptError::ExpectError(e)
    }
}

impl From<std::io::Error> for ScriptError {
    fn from(e: std::io::Error) -> Self {
        ScriptError::IoError(e)
    }
}

impl From<crate::PatternError> for ScriptError {
    fn from(e: crate::PatternError) -> Self {
        ScriptError::PatternError(e)
    }
}

impl From<pest::error::Error<crate::script::parser::Rule>> for ScriptError {
    fn from(e: pest::error::Error<crate::script::parser::Rule>) -> Self {
        let (line, col) = match e.line_col {
            pest::error::LineColLocation::Pos((line, col)) => (line, col),
            pest::error::LineColLocation::Span((line, col), _) => (line, col),
        };
        ScriptError::ParseError {
            line,
            col,
            message: e.variant.to_string(),
        }
    }
}
