//! Code generation for translating Expect scripts to Rust.

mod expression;
mod pattern;
mod statement;
mod warnings;

pub use warnings::{TranslationWarning, WarningDetector};

use crate::script::ast::*;
use std::fmt;

/// Result of translating an Expect script to Rust code.
#[derive(Debug)]
pub struct GeneratedCode {
    /// The generated Rust code.
    pub code: String,
    /// Warnings about unsupported features or behavioral differences.
    pub warnings: Vec<TranslationWarning>,
    /// Additional crate dependencies needed.
    pub dependencies: Vec<String>,
}

impl GeneratedCode {
    /// Create a new generated code result.
    pub fn new(code: String, warnings: Vec<TranslationWarning>) -> Self {
        Self {
            code,
            warnings,
            dependencies: vec!["expectrust".to_string(), "tokio".to_string()],
        }
    }
}

/// Translator context for code generation.
pub struct Translator {
    /// Accumulated warnings during translation.
    warnings: Vec<TranslationWarning>,
    /// Current indentation level.
    indent_level: usize,
    /// Whether we're inside a procedure.
    in_procedure: bool,
    /// Line number tracking for warnings.
    current_line: usize,
}

impl Translator {
    /// Create a new translator.
    pub fn new() -> Self {
        Self {
            warnings: Vec::new(),
            indent_level: 1,
            in_procedure: false,
            current_line: 0,
        }
    }

    /// Translate a script block to Rust code.
    pub fn translate(block: &Block) -> Result<GeneratedCode, TranslationError> {
        let mut translator = Self::new();

        // Detect warnings upfront
        let detected_warnings = WarningDetector::check_script(block);
        translator.warnings.extend(detected_warnings);

        // Generate main function body
        let mut body = String::new();
        for stmt in block {
            translator.current_line += 1;
            let code = translator.generate_statement(stmt)?;
            if !code.is_empty() {
                body.push_str(&translator.indent(&code));
                body.push('\n');
            }
        }

        // Build full code
        let mut code = String::new();

        // Add warning header if there are warnings
        if !translator.warnings.is_empty() {
            code.push_str("// WARNING: This code was auto-generated from an expect script\n");
            code.push_str("// Review and test thoroughly before using in production\n\n");
        }

        // Add imports
        code.push_str("use expectrust::{Session, Pattern};\n");
        code.push_str("use std::time::Duration;\n\n");

        // Add main function
        code.push_str("#[tokio::main]\n");
        code.push_str("async fn main() -> Result<(), Box<dyn std::error::Error>> {\n");
        code.push_str(&body);
        code.push_str("    Ok(())\n");
        code.push_str("}\n");

        // Add warning comments at the end
        if !translator.warnings.is_empty() {
            code.push_str("\n// Translation warnings:\n");
            for warning in &translator.warnings {
                code.push_str(&format!("// - {}\n", warning));
            }
        }

        Ok(GeneratedCode::new(code, translator.warnings))
    }

    /// Generate code for a single statement.
    fn generate_statement(&mut self, stmt: &Statement) -> Result<String, TranslationError> {
        match stmt {
            Statement::Spawn(s) => statement::gen_spawn(s, self),
            Statement::Expect(s) => statement::gen_expect(s, self),
            Statement::Send(s) => statement::gen_send(s, self),
            Statement::Set(s) => statement::gen_set(s, self),
            Statement::If(s) => statement::gen_if(s, self),
            Statement::While(s) => statement::gen_while(s, self),
            Statement::For(s) => statement::gen_for(s, self),
            Statement::Proc(s) => statement::gen_proc(s, self),
            Statement::Call(s) => statement::gen_call(s, self),
            Statement::Close => Ok("drop(session);".to_string()),
            Statement::Wait => Ok("session.wait().await?;".to_string()),
            Statement::Exit(code) => {
                if let Some(expr) = code {
                    let code_expr = expression::generate_expression(expr, self)?;
                    Ok(format!("std::process::exit({} as i32);", code_expr))
                } else {
                    Ok("std::process::exit(0);".to_string())
                }
            }
        }
    }

    /// Generate code for a block of statements.
    fn generate_block(&mut self, block: &Block) -> Result<String, TranslationError> {
        let mut code = String::new();
        for stmt in block {
            let stmt_code = self.generate_statement(stmt)?;
            if !stmt_code.is_empty() {
                code.push_str(&self.indent(&stmt_code));
                code.push('\n');
            }
        }
        Ok(code)
    }

    /// Add a warning.
    fn add_warning(&mut self, warning: TranslationWarning) {
        self.warnings.push(warning);
    }

    /// Increase indentation level.
    fn push_indent(&mut self) {
        self.indent_level += 1;
    }

    /// Decrease indentation level.
    fn pop_indent(&mut self) {
        if self.indent_level > 0 {
            self.indent_level -= 1;
        }
    }

    /// Apply current indentation to a string.
    fn indent(&self, s: &str) -> String {
        let indent = "    ".repeat(self.indent_level);
        s.lines()
            .map(|line| {
                if line.is_empty() {
                    String::new()
                } else {
                    format!("{}{}", indent, line)
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Get current line number.
    fn line(&self) -> usize {
        self.current_line
    }
}

impl Default for Translator {
    fn default() -> Self {
        Self::new()
    }
}

/// Error during translation.
#[derive(Debug)]
pub enum TranslationError {
    /// Unsupported feature that cannot be translated.
    UnsupportedFeature { feature: String, line: usize },
    /// Invalid expression.
    InvalidExpression { message: String, line: usize },
    /// Internal error.
    Internal(String),
}

impl fmt::Display for TranslationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnsupportedFeature { feature, line } => {
                write!(f, "Line {}: Unsupported feature: {}", line, feature)
            }
            Self::InvalidExpression { message, line } => {
                write!(f, "Line {}: Invalid expression: {}", line, message)
            }
            Self::Internal(msg) => write!(f, "Internal error: {}", msg),
        }
    }
}

impl std::error::Error for TranslationError {}
