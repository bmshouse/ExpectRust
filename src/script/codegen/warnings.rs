//! Warning detection and formatting for translation.

use crate::script::ast::*;
use std::fmt;

/// A warning about translation behavior or limitations.
#[derive(Debug, Clone, PartialEq)]
pub enum TranslationWarning {
    /// Unsupported feature that requires manual implementation.
    UnsupportedFeature {
        /// The feature name
        feature: String,
        /// The line number
        line: usize,
        /// Suggested workaround
        suggestion: String,
    },
    /// Behavioral difference between expect and Rust version.
    BehaviorDifference {
        /// Description of the difference
        description: String,
        /// The line number
        line: usize,
    },
    /// General performance or usage note.
    PerformanceNote {
        /// Description of the note
        description: String,
    },
}

impl fmt::Display for TranslationWarning {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFeature {
                feature,
                line,
                suggestion,
            } => {
                write!(
                    f,
                    "Line {}: '{}' not directly supported - {}",
                    line, feature, suggestion
                )
            }
            Self::BehaviorDifference { description, line } => {
                write!(f, "Line {}: {}", line, description)
            }
            Self::PerformanceNote { description } => {
                write!(f, "Note: {}", description)
            }
        }
    }
}

/// Detector for warnings in a script.
pub struct WarningDetector {
    warnings: Vec<TranslationWarning>,
    line: usize,
}

impl WarningDetector {
    /// Check a script and return all warnings.
    pub fn check_script(script: &Block) -> Vec<TranslationWarning> {
        let mut detector = Self {
            warnings: Vec::new(),
            line: 0,
        };

        // Add general async warning
        detector.warnings.push(TranslationWarning::PerformanceNote {
            description: "All generated code is async - main function uses #[tokio::main]"
                .to_string(),
        });

        detector.walk_block(script);
        detector.warnings
    }

    /// Walk through a block of statements.
    fn walk_block(&mut self, block: &Block) {
        for stmt in block {
            self.line += 1;
            self.check_statement(stmt);
        }
    }

    /// Check a single statement for warnings.
    fn check_statement(&mut self, stmt: &Statement) {
        match stmt {
            Statement::Spawn(_) => {
                // No warnings for basic spawn
            }
            Statement::Expect(expect_stmt) => {
                self.check_expect(expect_stmt);
            }
            Statement::Send(_) => {
                // No warnings for basic send
            }
            Statement::Set(_) => {
                // No warnings for variable assignment
            }
            Statement::If(if_stmt) => {
                self.walk_block(&if_stmt.then_block);
                if let Some(else_block) = &if_stmt.else_block {
                    self.walk_block(else_block);
                }
            }
            Statement::While(while_stmt) => {
                self.walk_block(&while_stmt.body);
            }
            Statement::For(for_stmt) => {
                self.walk_block(&for_stmt.body);
            }
            Statement::Proc(proc_stmt) => {
                let saved_line = self.line;
                self.walk_block(&proc_stmt.body);
                self.line = saved_line;
            }
            Statement::Call(_) => {
                // No warnings for procedure calls
            }
            Statement::Close => {
                // No warnings for close
            }
            Statement::Wait => {
                // No warnings for wait
            }
            Statement::Exit(_) => {
                // No warnings for exit
            }
        }
    }

    /// Check expect statement for regex patterns.
    fn check_expect(&mut self, _expect_stmt: &ExpectStmt) {
        // Could add warnings for specific pattern types if needed
        // For now, all patterns are supported
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_unsupported_warning() {
        let warning = TranslationWarning::UnsupportedFeature {
            feature: "interact".to_string(),
            line: 10,
            suggestion: "implement manual I/O loop".to_string(),
        };
        let text = format!("{}", warning);
        assert!(text.contains("Line 10"));
        assert!(text.contains("interact"));
    }

    #[test]
    fn test_check_empty_script() {
        let script = vec![];
        let warnings = WarningDetector::check_script(&script);
        // Should at least have the async note
        assert!(!warnings.is_empty());
    }
}
