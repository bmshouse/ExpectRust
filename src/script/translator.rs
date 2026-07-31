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
    fn test_translate_spawn_multiple_args() {
        let script = r#"
spawn ssh -o "StrictHostKeyChecking=no" host
expect "$ "
"#;

        let result = translate_str(script);
        assert!(result.is_ok());

        let generated = result.unwrap();
        assert!(
            generated.code.contains(
                "Session::spawn_args(\"ssh\", &[\"-o\", \"StrictHostKeyChecking=no\", \"host\"])"
            ),
            "expected spawn_args with 4 separated arguments, got:\n{}",
            generated.code
        );
    }

    #[test]
    fn test_translate_if_condition_evaluates_real_expression() {
        // Regression test for the if/while/for condition-parsing bug fixed
        // alongside expect_out: the condition used to always parse to a
        // hardcoded `1.0` stub, so translated code would have contained
        // `if 1 { ... }` (or similar) regardless of the actual condition
        // text. It should now contain the real comparison.
        let script = r#"
set x 1
if { $x == 1 } {
    set y 2
} else {
    set y 3
}
"#;

        let result = translate_str(script);
        assert!(result.is_ok());

        let generated = result.unwrap();
        assert!(
            generated.code.contains("if (x == 1)"),
            "expected the real condition to be translated, got:\n{}",
            generated.code
        );
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

    #[test]
    fn test_translate_interact() {
        // Note: avoids "@" in the spawn command - the grammar's bare_word
        // rule doesn't accept it yet (a pre-existing, already-documented
        // limitation, see docs/TRANSLATOR_README.md's "Special Characters"
        // section - unrelated to interact support itself).
        let script = r#"
spawn ssh remote-host
expect "password:"
send "secret\n"
interact
"#;

        let result = translate_str(script);
        assert!(result.is_ok());

        let generated = result.unwrap();
        assert!(generated.code.contains("session.interact().await?;"));

        // `interact` is now genuinely supported, so it must not be reported
        // as an unsupported feature.
        assert!(
            !generated
                .warnings
                .iter()
                .any(|w| format!("{}", w).contains("interact")),
            "interact should not be flagged as unsupported: {:?}",
            generated.warnings
        );
    }

    #[test]
    fn test_translate_exp_continue_wraps_loop() {
        let script = r#"
spawn cat
expect {
    "busy" {
        exp_continue
    }
    "done" {}
}
"#;

        let result = translate_str(script);
        assert!(result.is_ok());

        let generated = result.unwrap();
        assert!(
            generated.code.contains("loop {"),
            "expected a loop wrapper, got:\n{}",
            generated.code
        );
        assert!(
            generated.code.contains("continue;"),
            "expected exp_continue to translate to `continue;`, got:\n{}",
            generated.code
        );

        // Correctly-placed (directly inside an expect action), so it must
        // not be reported as an unsupported feature.
        assert!(
            !generated
                .warnings
                .iter()
                .any(|w| format!("{}", w).contains("exp_continue")),
            "exp_continue used correctly should not be flagged: {:?}",
            generated.warnings
        );
    }

    #[test]
    fn test_translate_plain_expect_block_no_loop() {
        // No exp_continue anywhere - should generate exactly what it did
        // before exp_continue support was added, with no loop boilerplate.
        let script = r#"
spawn cat
expect {
    "busy" {
        send "not busy\n"
    }
    "done" {}
}
"#;

        let result = translate_str(script);
        assert!(result.is_ok());

        let generated = result.unwrap();
        assert!(
            !generated.code.contains("loop {"),
            "expect block without exp_continue should not gain a loop wrapper, got:\n{}",
            generated.code
        );
    }

    #[test]
    fn test_translate_exp_continue_in_proc_flags_warning() {
        // exp_continue directly inside a proc body (not inside any expect
        // action) - the translator can't wire a Rust `continue` across a
        // function-call boundary, so this must be flagged rather than
        // silently emitting code that won't compile or won't do the right
        // thing.
        let script = r#"
proc bad {} {
    exp_continue
}
spawn echo hello
expect "hello"
"#;

        let result = translate_str(script);
        assert!(result.is_ok());

        let generated = result.unwrap();
        assert!(
            generated
                .warnings
                .iter()
                .any(|w| format!("{}", w).contains("exp_continue")),
            "exp_continue used outside an expect action should be flagged as unsupported: {:?}",
            generated.warnings
        );
    }
}
