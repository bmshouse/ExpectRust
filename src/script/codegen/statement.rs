//! Statement code generation.

use super::{expression, pattern, TranslationError, Translator};
use crate::script::ast::*;

/// Generate code for spawn statement.
///
/// Generates a `Session::spawn_args(program, &[arg1, arg2, ...])` call
/// rather than joining everything into one `Session::spawn("...")` string -
/// each argument is generated independently so a value containing spaces
/// (a literal quoted word, or a variable that turns out to contain spaces
/// at runtime) stays exactly one argv entry instead of being re-split.
pub fn gen_spawn(
    stmt: &SpawnStmt,
    translator: &mut Translator,
) -> Result<String, TranslationError> {
    let Some((program, rest)) = stmt.args.split_first() else {
        return Err(TranslationError::InvalidExpression {
            message: "spawn statement must have a program".to_string(),
            line: translator.line(),
        });
    };

    let program_code = expression::generate_expression(program, translator)?;

    let mut arg_codes = Vec::with_capacity(rest.len());
    for arg in rest {
        arg_codes.push(expression::generate_expression(arg, translator)?);
    }

    let code = if arg_codes.is_empty() {
        // Turbofish needed: an empty array literal alone doesn't give the
        // compiler enough to infer `S` in `spawn_args<S: AsRef<str>>`.
        format!(
            "let mut session = Session::spawn_args::<&str>({}, &[])?;",
            program_code
        )
    } else {
        format!(
            "let mut session = Session::spawn_args({}, &[{}])?;",
            program_code,
            arg_codes.join(", ")
        )
    };

    Ok(code)
}

/// Generate code for expect statement.
pub fn gen_expect(
    stmt: &ExpectStmt,
    translator: &mut Translator,
) -> Result<String, TranslationError> {
    if stmt.patterns.is_empty() {
        return Err(TranslationError::InvalidExpression {
            message: "expect statement must have at least one pattern".to_string(),
            line: translator.line(),
        });
    }

    // Single pattern without action
    if stmt.patterns.len() == 1 && stmt.patterns[0].action.is_none() {
        let pattern = pattern::generate_pattern(&stmt.patterns[0].pattern_type)?;
        return Ok(format!("session.expect({}).await?;", pattern));
    }

    // Multiple patterns or patterns with actions
    gen_expect_multi(&stmt.patterns, translator)
}

/// Generate code for multi-pattern expect with actions.
fn gen_expect_multi(
    patterns: &[ExpectPattern],
    translator: &mut Translator,
) -> Result<String, TranslationError> {
    // Only wrap in a `loop` (and translate `exp_continue` to `continue;`)
    // when an action actually uses it - keeps generated code free of
    // boilerplate for the common case where it isn't needed.
    let needs_loop = patterns
        .iter()
        .any(|p| p.action.as_ref().is_some_and(|a| block_contains_exp_continue(a)));

    let mut code = String::new();

    // Generate pattern array
    code.push_str("{\n");
    translator.push_indent();

    code.push_str(&translator.indent("let patterns = [\n"));
    translator.push_indent();
    for pattern in patterns {
        let pat = pattern::generate_pattern(&pattern.pattern_type)?;
        code.push_str(&translator.indent(&format!("{},\n", pat)));
    }
    translator.pop_indent();
    code.push_str(&translator.indent("];\n"));

    if needs_loop {
        code.push_str(&translator.indent("loop {\n"));
        translator.push_indent();
    }

    // Generate expect_any call
    code.push_str(&translator.indent("let result = session.expect_any(&patterns).await?;\n"));

    // Generate match statement if any patterns have actions
    let has_actions = patterns.iter().any(|p| p.action.is_some());
    if has_actions {
        code.push_str(&translator.indent("match result.pattern_index {\n"));
        translator.push_indent();

        for (idx, pattern) in patterns.iter().enumerate() {
            if let Some(action) = &pattern.action {
                code.push_str(&translator.indent(&format!("{} => {{\n", idx)));
                translator.push_indent();
                let action_code = translator.generate_block(action)?;
                code.push_str(&action_code);
                translator.pop_indent();
                code.push_str(&translator.indent("}\n"));
            }
        }

        code.push_str(&translator.indent("_ => {}\n"));
        translator.pop_indent();
        code.push_str(&translator.indent("}\n"));
    }

    if needs_loop {
        code.push_str(&translator.indent("break;\n"));
        translator.pop_indent();
        code.push_str(&translator.indent("}\n"));
    }

    translator.pop_indent();
    code.push_str(&translator.indent("}"));

    Ok(code)
}

/// Whether `block` contains `exp_continue` reachable via inline control flow
/// only (`if`/`while`/`for`) - deliberately does *not* descend into `Call`
/// (a call to a separately-generated `async fn`, where a Rust `continue`
/// couldn't reach back into this loop anyway).
fn block_contains_exp_continue(block: &Block) -> bool {
    block.iter().any(statement_contains_exp_continue)
}

fn statement_contains_exp_continue(stmt: &Statement) -> bool {
    match stmt {
        Statement::ExpContinue => true,
        Statement::If(if_stmt) => {
            block_contains_exp_continue(&if_stmt.then_block)
                || if_stmt
                    .else_block
                    .as_ref()
                    .is_some_and(|b| block_contains_exp_continue(b))
        }
        Statement::While(while_stmt) => block_contains_exp_continue(&while_stmt.body),
        Statement::For(for_stmt) => block_contains_exp_continue(&for_stmt.body),
        _ => false,
    }
}

/// Generate code for send statement.
pub fn gen_send(stmt: &SendStmt, translator: &mut Translator) -> Result<String, TranslationError> {
    if let Expression::String(s) = &stmt.data {
        Ok(format!("session.send(b\"{}\").await?;", escape_bytes(s)))
    } else {
        let data = expression::generate_expression(&stmt.data, translator)?;
        Ok(format!("session.send({}.as_bytes()).await?;", data))
    }
}

/// Generate code for set statement.
pub fn gen_set(stmt: &SetStmt, translator: &mut Translator) -> Result<String, TranslationError> {
    let value = expression::generate_expression(&stmt.value, translator)?;
    let var_name = sanitize_variable_name(&stmt.name);
    Ok(format!("let {} = {};", var_name, value))
}

/// Generate code for if statement.
pub fn gen_if(stmt: &IfStmt, translator: &mut Translator) -> Result<String, TranslationError> {
    let cond = expression::generate_expression(&stmt.condition, translator)?;

    let mut code = format!("if {} {{\n", cond);
    translator.push_indent();
    let then_block = translator.generate_block(&stmt.then_block)?;
    code.push_str(&then_block);
    translator.pop_indent();
    code.push_str(&translator.indent("}"));

    if let Some(else_block) = &stmt.else_block {
        code.push_str(" else {\n");
        translator.push_indent();
        let else_code = translator.generate_block(else_block)?;
        code.push_str(&else_code);
        translator.pop_indent();
        code.push_str(&translator.indent("}"));
    }

    Ok(code)
}

/// Generate code for while statement.
pub fn gen_while(
    stmt: &WhileStmt,
    translator: &mut Translator,
) -> Result<String, TranslationError> {
    let cond = expression::generate_expression(&stmt.condition, translator)?;

    let mut code = format!("while {} {{\n", cond);
    translator.push_indent();
    let body = translator.generate_block(&stmt.body)?;
    code.push_str(&body);
    translator.pop_indent();
    code.push_str(&translator.indent("}"));

    Ok(code)
}

/// Generate code for for statement.
pub fn gen_for(stmt: &ForStmt, translator: &mut Translator) -> Result<String, TranslationError> {
    let mut code = String::new();

    // Generate initialization
    code.push_str("{\n");
    translator.push_indent();
    let init_code = translator.generate_statement(&stmt.init)?;
    code.push_str(&translator.indent(&init_code));
    code.push('\n');

    // Generate while loop
    let cond = expression::generate_expression(&stmt.condition, translator)?;
    code.push_str(&translator.indent(&format!("while {} {{\n", cond)));
    translator.push_indent();

    // Loop body
    let body = translator.generate_block(&stmt.body)?;
    code.push_str(&body);

    // Increment
    let incr_code = translator.generate_statement(&stmt.increment)?;
    code.push_str(&translator.indent(&incr_code));
    code.push('\n');

    translator.pop_indent();
    code.push_str(&translator.indent("}\n"));
    translator.pop_indent();
    code.push_str(&translator.indent("}"));

    Ok(code)
}

/// Generate code for procedure definition.
pub fn gen_proc(stmt: &ProcStmt, translator: &mut Translator) -> Result<String, TranslationError> {
    let params = stmt.params.join(", ");

    let mut code = format!(
        "async fn {}({}) -> Result<(), Box<dyn std::error::Error>> {{\n",
        sanitize_variable_name(&stmt.name),
        params
    );
    translator.push_indent();

    let old_in_proc = translator.in_procedure;
    translator.in_procedure = true;
    let body = translator.generate_block(&stmt.body)?;
    translator.in_procedure = old_in_proc;

    code.push_str(&body);

    // Add Ok(()) if not already present
    code.push_str(&translator.indent("Ok(())\n"));

    translator.pop_indent();
    code.push_str(&translator.indent("}"));

    Ok(code)
}

/// Generate code for procedure call.
pub fn gen_call(stmt: &CallStmt, translator: &mut Translator) -> Result<String, TranslationError> {
    let mut args = Vec::new();
    for arg in &stmt.args {
        args.push(expression::generate_expression(arg, translator)?);
    }

    let call = if args.is_empty() {
        format!("{}().await?;", sanitize_variable_name(&stmt.name))
    } else {
        format!(
            "{}({}).await?;",
            sanitize_variable_name(&stmt.name),
            args.join(", ")
        )
    };

    Ok(call)
}

/// Escape special characters in a string for Rust string literal.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
}

/// Escape special characters for byte string literals.
fn escape_bytes(s: &str) -> String {
    escape_string(s)
}

/// Sanitize a variable name to be a valid Rust identifier.
fn sanitize_variable_name(name: &str) -> String {
    // Remove leading $ if present
    let name = name.strip_prefix('$').unwrap_or(name);

    // Replace invalid characters with underscores
    let sanitized: String = name
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();

    // Ensure it doesn't start with a number
    if sanitized.chars().next().is_some_and(|c| c.is_ascii_digit()) {
        format!("var_{}", sanitized)
    } else if sanitized.is_empty() {
        "var_unnamed".to_string()
    } else {
        sanitized
    }
}
