//! High-level translator API for converting Expect scripts to Rust code.

use crate::script::ast::Block;
use crate::script::codegen::{GeneratedCode, TranslationError, Translator as CodeGen};
use std::path::Path;

/// Translate an Expect script string to Rust code.
///
/// # Example
///
/// ```rust,no_run
/// use expectrust::script::translator::translate_str;
///
/// let expect_script = r#"
///     spawn ssh user@host
///     expect "password:"
///     send "secret\n"
///     expect "$ "
/// "#;
///
/// let generated = translate_str(expect_script)?;
/// println!("{}", generated.code);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn translate_str(script_text: &str) -> Result<GeneratedCode, TranslationError> {
    // Parse the script to get the AST
    let ast = crate::script::parser::parse_script(script_text)
        .map_err(|e| TranslationError::Internal(format!("Parse error: {}", e)))?;

    CodeGen::translate(&ast)
}

/// Translate an Expect script file to Rust code.
///
/// # Example
///
/// ```rust,no_run
/// use expectrust::script::translator::translate_file;
///
/// let generated = translate_file("automation.exp")?;
/// std::fs::write("automation.rs", generated.code)?;
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
pub fn translate_file<P: AsRef<Path>>(path: P) -> Result<GeneratedCode, TranslationError> {
    let script_text = std::fs::read_to_string(path)
        .map_err(|e| TranslationError::Internal(format!("File read error: {}", e)))?;

    translate_str(&script_text)
}

/// Translate an AST block directly to Rust code.
///
/// This is useful if you already have a parsed AST.
pub fn translate_ast(ast: &Block) -> Result<GeneratedCode, TranslationError> {
    CodeGen::translate(ast)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_translate_simple_script() {
        let script = r#"
spawn echo hello
expect "hello"
"#;

        let result = translate_str(script);
        assert!(result.is_ok());

        let generated = result.unwrap();
        assert!(generated.code.contains("Session::spawn"));
        assert!(generated.code.contains("expect"));
    }

    #[test]
    fn test_translate_with_send() {
        let script = r#"
spawn python -i
expect ">>>"
send "print('test')\n"
"#;

        let result = translate_str(script);
        assert!(result.is_ok());

        let generated = result.unwrap();
        assert!(generated.code.contains("send"));
    }
}
