//! AST interpreter for executing Expect scripts.

use crate::script::ast::*;
use crate::script::error::ScriptError;
use crate::script::runtime::Runtime;
use crate::script::value::Value;

/// Execute a block of statements.
pub fn execute_block<'a>(
    block: &'a Block,
    runtime: &'a mut Runtime,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ScriptError>> + 'a>> {
    Box::pin(async move {
        for statement in block {
            execute_statement(statement, runtime).await?;
        }
        Ok(())
    })
}

/// Execute a single statement.
pub fn execute_statement<'a>(
    statement: &'a Statement,
    runtime: &'a mut Runtime,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = Result<(), ScriptError>> + 'a>> {
    Box::pin(async move {
        match statement {
            Statement::Spawn(stmt) => execute_spawn(stmt, runtime).await,
            Statement::Expect(stmt) => execute_expect(stmt, runtime).await,
            Statement::Send(stmt) => execute_send(stmt, runtime).await,
            Statement::Set(stmt) => execute_set(stmt, runtime),
            Statement::If(stmt) => execute_if(stmt, runtime).await,
            Statement::While(stmt) => execute_while(stmt, runtime).await,
            Statement::For(stmt) => execute_for(stmt, runtime).await,
            Statement::Proc(stmt) => execute_proc(stmt, runtime),
            Statement::Call(stmt) => execute_call(stmt, runtime).await,
            Statement::Close => execute_close(runtime).await,
            Statement::Wait => execute_wait(runtime).await,
            Statement::Interact => execute_interact(runtime).await,
            Statement::ExpContinue => execute_exp_continue(),
            Statement::Exit(code_expr) => execute_exit(code_expr.as_ref(), runtime),
        }
    })
}

async fn execute_spawn(stmt: &SpawnStmt, runtime: &mut Runtime) -> Result<(), ScriptError> {
    // Evaluate each argument independently (rather than joining into one
    // string) so a value containing spaces - whether a literal quoted word
    // or a substituted variable - becomes exactly one argv entry.
    let mut args = Vec::with_capacity(stmt.args.len());
    for arg_expr in &stmt.args {
        let value = evaluate_expression(arg_expr, runtime)?;
        args.push(value.as_string());
    }
    runtime.spawn(&args)?;
    Ok(())
}

async fn execute_expect(stmt: &ExpectStmt, runtime: &mut Runtime) -> Result<(), ScriptError> {
    // Build patterns from the expect statement (once - reused on every
    // exp_continue reloop, matching Tcl's "re-runs the same expect").
    let mut patterns = Vec::new();
    for pattern in &stmt.patterns {
        let p = runtime.pattern_from_ast(&pattern.pattern_type)?;
        patterns.push(p);
    }

    loop {
        let session = runtime.session_mut()?;
        let result = session.expect_any(&patterns).await?;

        // Populate expect_out(...) so the matched pattern's action (and any
        // later statement, e.g. a condition or send) can reference what was
        // just matched - set before the action runs, matching Tcl's own
        // ordering. Not cleared between matches (real Tcl arrays don't reset
        // either): a later match that doesn't populate a given key (e.g. no
        // capture group 2 this time) just leaves the previous value in place.
        let ctx = runtime.context_mut();
        ctx.set_variable(
            "expect_out(0,string)".to_string(),
            Value::String(result.matched.clone()),
        );
        ctx.set_variable(
            "expect_out(buffer)".to_string(),
            Value::String(format!("{}{}", result.before, result.matched)),
        );
        // captures[0] is the whole match (already covered above by
        // expect_out(0,string)); captures[1..] are the regex capture groups.
        for (i, capture) in result.captures.iter().enumerate().skip(1) {
            ctx.set_variable(
                format!("expect_out({},string)", i),
                Value::String(capture.clone()),
            );
        }

        // If the matched pattern has an action, execute it. `exp_continue`
        // inside that action (however deeply nested - if/while/for/proc
        // calls all propagate errors upward via `?`) surfaces here as
        // `Err(ScriptError::ExpContinue)`, which reloops this same
        // expect statement instead of returning.
        if let Some(matched_pattern) = stmt.patterns.get(result.pattern_index) {
            if let Some(action) = &matched_pattern.action {
                match execute_block(action, runtime).await {
                    Err(ScriptError::ExpContinue) => continue,
                    other => return other,
                }
            }
        }

        return Ok(());
    }
}

async fn execute_send(stmt: &SendStmt, runtime: &mut Runtime) -> Result<(), ScriptError> {
    let data = evaluate_expression(&stmt.data, runtime)?;
    let data_str = data.as_string();
    let session = runtime.session_mut()?;
    session.send(data_str.as_bytes()).await?;
    Ok(())
}

fn execute_set(stmt: &SetStmt, runtime: &mut Runtime) -> Result<(), ScriptError> {
    let value = evaluate_expression(&stmt.value, runtime)?;
    runtime.context_mut().set_variable(stmt.name.clone(), value);
    Ok(())
}

async fn execute_if(stmt: &IfStmt, runtime: &mut Runtime) -> Result<(), ScriptError> {
    let condition_value = evaluate_expression(&stmt.condition, runtime)?;

    if condition_value.as_bool() {
        execute_block(&stmt.then_block, runtime).await?;
    } else if let Some(else_block) = &stmt.else_block {
        execute_block(else_block, runtime).await?;
    }

    Ok(())
}

async fn execute_while(stmt: &WhileStmt, runtime: &mut Runtime) -> Result<(), ScriptError> {
    loop {
        let condition_value = evaluate_expression(&stmt.condition, runtime)?;
        if !condition_value.as_bool() {
            break;
        }
        execute_block(&stmt.body, runtime).await?;
    }
    Ok(())
}

async fn execute_for(stmt: &ForStmt, runtime: &mut Runtime) -> Result<(), ScriptError> {
    // Execute initialization
    execute_statement(&stmt.init, runtime).await?;

    // Loop
    loop {
        let condition_value = evaluate_expression(&stmt.condition, runtime)?;
        if !condition_value.as_bool() {
            break;
        }

        execute_block(&stmt.body, runtime).await?;
        execute_statement(&stmt.increment, runtime).await?;
    }

    Ok(())
}

fn execute_proc(stmt: &ProcStmt, runtime: &mut Runtime) -> Result<(), ScriptError> {
    let procedure = Procedure::new(stmt.params.clone(), stmt.body.clone());
    runtime
        .context_mut()
        .define_procedure(stmt.name.clone(), procedure);
    Ok(())
}

async fn execute_call(stmt: &CallStmt, runtime: &mut Runtime) -> Result<(), ScriptError> {
    // Look up the procedure
    let procedure = runtime
        .context()
        .get_procedure(&stmt.name)
        .ok_or_else(|| ScriptError::UndefinedProcedure(stmt.name.clone()))?
        .clone();

    // Evaluate arguments
    let mut arg_values = Vec::new();
    for arg in &stmt.args {
        arg_values.push(evaluate_expression(arg, runtime)?);
    }

    // Check argument count
    if arg_values.len() != procedure.params.len() {
        return Err(ScriptError::RuntimeError(format!(
            "Procedure {} expects {} arguments, got {}",
            stmt.name,
            procedure.params.len(),
            arg_values.len()
        )));
    }

    // Create a new context with procedure parameters
    let mut proc_context = crate::script::context::Context::new();
    for (param, value) in procedure.params.iter().zip(arg_values.iter()) {
        proc_context.set_variable(param.clone(), value.clone());
    }

    // Swap contexts
    let old_context = std::mem::replace(runtime.context_mut(), proc_context);

    // Execute procedure body
    let result = execute_block(&procedure.body, runtime).await;

    // Restore old context
    *runtime.context_mut() = old_context;

    result
}

async fn execute_close(runtime: &mut Runtime) -> Result<(), ScriptError> {
    runtime.close().await
}

async fn execute_wait(runtime: &mut Runtime) -> Result<(), ScriptError> {
    runtime.wait().await
}

async fn execute_interact(runtime: &mut Runtime) -> Result<(), ScriptError> {
    runtime.session_mut()?.interact().await?;
    Ok(())
}

/// Signal that the enclosing `expect` statement should re-run its pattern
/// matching instead of falling through. `execute_expect` is the only place
/// that catches this; if it escapes all the way to `Script::execute`, it
/// means `exp_continue` was used outside of any expect action, which is a
/// genuine usage error (matches Tcl's own behavior in that case).
fn execute_exp_continue() -> Result<(), ScriptError> {
    Err(ScriptError::ExpContinue)
}

fn execute_exit(code_expr: Option<&Expression>, runtime: &mut Runtime) -> Result<(), ScriptError> {
    let code = if let Some(expr) = code_expr {
        let value = evaluate_expression(expr, runtime)?;
        value.as_number().map(|n| n as i32).unwrap_or(0)
    } else {
        0
    };

    runtime.set_exit_status(code);
    Err(ScriptError::Exit(code))
}

/// Evaluate an expression to a value.
pub fn evaluate_expression(expr: &Expression, runtime: &Runtime) -> Result<Value, ScriptError> {
    match expr {
        Expression::String(s) => {
            // Handle variable substitution in strings
            Ok(Value::String(substitute_variables(s, runtime)?))
        }
        Expression::Number(n) => Ok(Value::Number(*n)),
        Expression::Variable(name) => runtime
            .context()
            .get_variable(name)
            .cloned()
            .ok_or_else(|| ScriptError::UndefinedVariable(name.clone())),
        Expression::List(items) => {
            let mut values = Vec::new();
            for item in items {
                values.push(evaluate_expression(item, runtime)?);
            }
            Ok(Value::List(values))
        }
        Expression::BinaryOp { left, op, right } => {
            let left_val = evaluate_expression(left, runtime)?;
            let right_val = evaluate_expression(right, runtime)?;
            evaluate_binary_op(&left_val, *op, &right_val)
        }
        Expression::UnaryOp { op, operand } => {
            let val = evaluate_expression(operand, runtime)?;
            evaluate_unary_op(*op, &val)
        }
    }
}

fn substitute_variables(s: &str, runtime: &Runtime) -> Result<String, ScriptError> {
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

            // Array-style access, e.g. `$expect_out(0,string)` - consume
            // through the matching ")" as part of the variable's name, same
            // composite-key convention `execute_expect` stores expect_out
            // under (see grammar.pest's `variable`/`array_index` rules,
            // which do the same for a bare `$var(...)` reference).
            if !var_name.is_empty() && chars.peek() == Some(&'(') {
                var_name.push(chars.next().unwrap());
                for c in chars.by_ref() {
                    var_name.push(c);
                    if c == ')' {
                        break;
                    }
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

fn evaluate_binary_op(
    left: &Value,
    op: BinaryOperator,
    right: &Value,
) -> Result<Value, ScriptError> {
    match op {
        BinaryOperator::Add => {
            let l = left.as_number().map_err(ScriptError::RuntimeError)?;
            let r = right.as_number().map_err(ScriptError::RuntimeError)?;
            Ok(Value::Number(l + r))
        }
        BinaryOperator::Sub => {
            let l = left.as_number().map_err(ScriptError::RuntimeError)?;
            let r = right.as_number().map_err(ScriptError::RuntimeError)?;
            Ok(Value::Number(l - r))
        }
        BinaryOperator::Mul => {
            let l = left.as_number().map_err(ScriptError::RuntimeError)?;
            let r = right.as_number().map_err(ScriptError::RuntimeError)?;
            Ok(Value::Number(l * r))
        }
        BinaryOperator::Div => {
            let l = left.as_number().map_err(ScriptError::RuntimeError)?;
            let r = right.as_number().map_err(ScriptError::RuntimeError)?;
            if r == 0.0 {
                return Err(ScriptError::RuntimeError("Division by zero".to_string()));
            }
            Ok(Value::Number(l / r))
        }
        BinaryOperator::Eq => Ok(Value::Bool(left.as_string() == right.as_string())),
        BinaryOperator::Ne => Ok(Value::Bool(left.as_string() != right.as_string())),
        BinaryOperator::Lt => {
            let l = left.as_number().map_err(ScriptError::RuntimeError)?;
            let r = right.as_number().map_err(ScriptError::RuntimeError)?;
            Ok(Value::Bool(l < r))
        }
        BinaryOperator::Gt => {
            let l = left.as_number().map_err(ScriptError::RuntimeError)?;
            let r = right.as_number().map_err(ScriptError::RuntimeError)?;
            Ok(Value::Bool(l > r))
        }
        BinaryOperator::Le => {
            let l = left.as_number().map_err(ScriptError::RuntimeError)?;
            let r = right.as_number().map_err(ScriptError::RuntimeError)?;
            Ok(Value::Bool(l <= r))
        }
        BinaryOperator::Ge => {
            let l = left.as_number().map_err(ScriptError::RuntimeError)?;
            let r = right.as_number().map_err(ScriptError::RuntimeError)?;
            Ok(Value::Bool(l >= r))
        }
        BinaryOperator::And => Ok(Value::Bool(left.as_bool() && right.as_bool())),
        BinaryOperator::Or => Ok(Value::Bool(left.as_bool() || right.as_bool())),
    }
}

fn evaluate_unary_op(op: UnaryOperator, operand: &Value) -> Result<Value, ScriptError> {
    match op {
        UnaryOperator::Neg => {
            let n = operand.as_number().map_err(ScriptError::RuntimeError)?;
            Ok(Value::Number(-n))
        }
        UnaryOperator::Not => Ok(Value::Bool(!operand.as_bool())),
    }
}
