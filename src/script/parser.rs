//! Parser implementation using Pest.

use pest::Parser;
use pest_derive::Parser;

use crate::script::ast::*;
use crate::script::error::ScriptError;

#[derive(Parser)]
#[grammar = "script/grammar.pest"]
pub struct ExpectParser;

/// Parse a script from a string into an AST.
pub fn parse_script(input: &str) -> Result<Block, ScriptError> {
    let pairs = ExpectParser::parse(Rule::script, input)?;

    let mut statements = Vec::new();
    for pair in pairs {
        match pair.as_rule() {
            Rule::script => {
                for inner_pair in pair.into_inner() {
                    if let Rule::statement = inner_pair.as_rule() {
                        if let Some(stmt) = parse_statement(inner_pair)? {
                            statements.push(stmt);
                        }
                    }
                }
            }
            Rule::EOI => {}
            _ => {}
        }
    }

    Ok(statements)
}

fn parse_statement(pair: pest::iterators::Pair<Rule>) -> Result<Option<Statement>, ScriptError> {
    let inner = pair.into_inner().next();
    let Some(inner) = inner else {
        return Ok(None);
    };

    match inner.as_rule() {
        Rule::spawn_stmt => Ok(Some(parse_spawn_stmt(inner)?)),
        Rule::expect_stmt => Ok(Some(parse_expect_stmt(inner)?)),
        Rule::send_stmt => Ok(Some(parse_send_stmt(inner)?)),
        Rule::set_stmt => Ok(Some(parse_set_stmt(inner)?)),
        Rule::if_stmt => Ok(Some(parse_if_stmt(inner)?)),
        Rule::while_stmt => Ok(Some(parse_while_stmt(inner)?)),
        Rule::for_stmt => Ok(Some(parse_for_stmt(inner)?)),
        Rule::proc_stmt => Ok(Some(parse_proc_stmt(inner)?)),
        Rule::close_stmt => Ok(Some(Statement::Close)),
        Rule::wait_stmt => Ok(Some(Statement::Wait)),
        Rule::interact_stmt => Ok(Some(Statement::Interact)),
        Rule::exp_continue_stmt => Ok(Some(Statement::ExpContinue)),
        Rule::exit_stmt => Ok(Some(parse_exit_stmt(inner)?)),
        Rule::call_stmt => Ok(Some(parse_call_stmt(inner)?)),
        _ => Ok(None),
    }
}

fn parse_spawn_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let inner = pair.into_inner();
    // Each word becomes its own argument expression - NOT joined into a
    // single string, so a word containing a space (a quoted literal, or a
    // variable substituted at runtime) stays exactly one argv entry.
    let mut args = Vec::new();
    for word_pair in inner {
        let word = parse_word(word_pair)?;
        args.push(Expression::String(word));
    }
    Ok(Statement::Spawn(SpawnStmt { args }))
}

fn parse_expect_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();
    let next = inner.next().unwrap();

    let patterns = match next.as_rule() {
        Rule::expect_block => parse_expect_block(next)?,
        Rule::pattern_spec => {
            // Single pattern without action
            vec![parse_pattern_spec(next, None)?]
        }
        _ => vec![],
    };

    Ok(Statement::Expect(ExpectStmt { patterns }))
}

fn parse_expect_block(
    pair: pest::iterators::Pair<Rule>,
) -> Result<Vec<ExpectPattern>, ScriptError> {
    let mut patterns = Vec::new();

    for case in pair.into_inner() {
        if case.as_rule() == Rule::expect_case {
            let mut case_inner = case.into_inner();
            let pattern_pair = case_inner.next().unwrap();
            let block_pair = case_inner.next().unwrap();

            let action = parse_brace_block(block_pair)?;
            let pattern = parse_pattern_spec(pattern_pair, Some(action))?;
            patterns.push(pattern);
        }
    }

    Ok(patterns)
}

fn parse_pattern_spec(
    pair: pest::iterators::Pair<Rule>,
    action: Option<Block>,
) -> Result<ExpectPattern, ScriptError> {
    // The "-re"/"-gl"/"timeout"/"eof" keywords are plain string literals
    // inside this rule, not sub-rules of their own - only the `word`
    // alternative produces an inner pair. So they can't be distinguished by
    // inspecting `.into_inner()` (as the previous implementation tried to,
    // which either panicked on a missing inner pair for "timeout"/"eof", or
    // silently misdetected "-re"/"-gl" as plain words since the "first"
    // inner pair for `("-re" ~ word)` is actually the `word` match, never
    // the literal "-re" itself). Detect them from the pattern_spec's own
    // captured text instead, then pull the pattern word (if any) from
    // `.into_inner()` separately.
    let text = pair.as_str().trim().to_string();
    let first_word = text.split_whitespace().next().unwrap_or("").to_string();
    let mut inner = pair.into_inner();

    let pattern_type = match first_word.as_str() {
        "-re" => {
            let word = parse_word(inner.next().unwrap())?;
            PatternType::Regex(word)
        }
        "-gl" => {
            let word = parse_word(inner.next().unwrap())?;
            PatternType::Glob(word)
        }
        "timeout" if text == "timeout" => PatternType::Timeout,
        "eof" if text == "eof" => PatternType::Eof,
        _ => {
            // It's a word (exact match)
            let word = parse_word(inner.next().unwrap())?;
            PatternType::Exact(word)
        }
    };

    Ok(ExpectPattern {
        pattern_type,
        action,
    })
}

fn parse_send_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();
    let word = parse_word(inner.next().unwrap())?;
    Ok(Statement::Send(SendStmt {
        data: Expression::String(word),
    }))
}

fn parse_set_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();
    let value_pair = inner.next().unwrap();

    let value = match value_pair.as_rule() {
        Rule::paren_expr => {
            let expr_pair = value_pair.into_inner().next().unwrap();
            parse_expression(expr_pair)?
        }
        _ => {
            // Try to parse as number, otherwise string (existing behavior
            // for plain words: numbers, quoted/brace strings, and bare
            // "$var" text resolved via runtime substitution).
            let word = parse_word(value_pair)?;
            if let Ok(num) = word.parse::<f64>() {
                Expression::Number(num)
            } else {
                Expression::String(word)
            }
        }
    };

    Ok(Statement::Set(SetStmt { name, value }))
}

fn parse_if_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();

    let condition = parse_condition(inner.next().unwrap())?;
    let then_block = parse_brace_block(inner.next().unwrap())?;

    // Optional else block
    let else_block = inner.next().map(|p| parse_brace_block(p)).transpose()?;

    Ok(Statement::If(IfStmt {
        condition,
        then_block,
        else_block,
    }))
}

fn parse_while_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();

    let condition = parse_condition(inner.next().unwrap())?;
    let body = parse_brace_block(inner.next().unwrap())?;

    Ok(Statement::While(WhileStmt { condition, body }))
}

fn parse_for_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();

    let init_block = parse_brace_block(inner.next().unwrap())?;
    let init = Box::new(
        init_block
            .into_iter()
            .next()
            .unwrap_or(Statement::Set(SetStmt {
                name: "_".to_string(),
                value: Expression::Number(0.0),
            })),
    );

    let condition = parse_condition(inner.next().unwrap())?;

    let incr_block = parse_brace_block(inner.next().unwrap())?;
    let increment = Box::new(
        incr_block
            .into_iter()
            .next()
            .unwrap_or(Statement::Set(SetStmt {
                name: "_".to_string(),
                value: Expression::Number(0.0),
            })),
    );

    let body = parse_brace_block(inner.next().unwrap())?;

    Ok(Statement::For(ForStmt {
        init,
        condition,
        increment,
        body,
    }))
}

/// Parse a `condition` pair (`{ <expression> }`) into a real `Expression`.
fn parse_condition(pair: pest::iterators::Pair<Rule>) -> Result<Expression, ScriptError> {
    let inner = pair.into_inner().next().unwrap();
    parse_expression(inner)
}

/// Parse an `expression`-family pair into a real `Expression` AST node.
///
/// Unlike `parse_word` (which deliberately flattens everything to a `String`
/// for word-list contexts like `spawn`/`send` arguments, deferring `$var`
/// substitution to runtime text-scanning), this preserves structure -
/// `BinaryOp`/`UnaryOp`/`Variable`/etc. - which conditions need so
/// `evaluate_binary_op` can actually compare values instead of just always
/// being handed a placeholder.
fn parse_expression(pair: pest::iterators::Pair<Rule>) -> Result<Expression, ScriptError> {
    match pair.as_rule() {
        Rule::expression | Rule::primary_expr => {
            let inner = pair.into_inner().next().unwrap();
            parse_expression(inner)
        }
        Rule::binary_expr => {
            let mut inner = pair.into_inner();
            let left = parse_expression(inner.next().unwrap())?;
            let op = parse_binary_op(inner.next().unwrap().as_str())?;
            let right = parse_expression(inner.next().unwrap())?;
            Ok(Expression::BinaryOp {
                left: Box::new(left),
                op,
                right: Box::new(right),
            })
        }
        Rule::unary_expr => {
            let mut inner = pair.into_inner();
            let op = parse_unary_op(inner.next().unwrap().as_str())?;
            let operand = parse_expression(inner.next().unwrap())?;
            Ok(Expression::UnaryOp {
                op,
                operand: Box::new(operand),
            })
        }
        Rule::number => {
            let n: f64 = pair.as_str().parse().map_err(|_| {
                ScriptError::RuntimeError(format!("Invalid number: {}", pair.as_str()))
            })?;
            Ok(Expression::Number(n))
        }
        Rule::variable => {
            let name = pair
                .as_str()
                .strip_prefix('$')
                .unwrap_or(pair.as_str())
                .to_string();
            Ok(Expression::Variable(name))
        }
        Rule::string => {
            let s = pair.as_str();
            let s = &s[1..s.len() - 1];
            Ok(Expression::String(parse_string_inner(s)))
        }
        Rule::brace_string => {
            let s = pair.as_str();
            Ok(Expression::String(s[1..s.len() - 1].to_string()))
        }
        Rule::bare_word => Ok(Expression::String(pair.as_str().to_string())),
        Rule::list => {
            let mut items = Vec::new();
            for inner_pair in pair.into_inner() {
                items.push(parse_expression(inner_pair)?);
            }
            Ok(Expression::List(items))
        }
        _ => Err(ScriptError::RuntimeError(format!(
            "Unexpected expression rule: {:?}",
            pair.as_rule()
        ))),
    }
}

fn parse_binary_op(s: &str) -> Result<BinaryOperator, ScriptError> {
    match s {
        "+" => Ok(BinaryOperator::Add),
        "-" => Ok(BinaryOperator::Sub),
        "*" => Ok(BinaryOperator::Mul),
        "/" => Ok(BinaryOperator::Div),
        "==" => Ok(BinaryOperator::Eq),
        "!=" => Ok(BinaryOperator::Ne),
        "<=" => Ok(BinaryOperator::Le),
        ">=" => Ok(BinaryOperator::Ge),
        "<" => Ok(BinaryOperator::Lt),
        ">" => Ok(BinaryOperator::Gt),
        "&&" => Ok(BinaryOperator::And),
        "||" => Ok(BinaryOperator::Or),
        other => Err(ScriptError::RuntimeError(format!(
            "Unknown binary operator: {}",
            other
        ))),
    }
}

fn parse_unary_op(s: &str) -> Result<UnaryOperator, ScriptError> {
    match s {
        "-" => Ok(UnaryOperator::Neg),
        "!" => Ok(UnaryOperator::Not),
        other => Err(ScriptError::RuntimeError(format!(
            "Unknown unary operator: {}",
            other
        ))),
    }
}

fn parse_proc_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();

    let name = inner.next().unwrap().as_str().to_string();
    let params = parse_brace_list(inner.next().unwrap())?;
    let body = parse_brace_block(inner.next().unwrap())?;

    Ok(Statement::Proc(ProcStmt { name, params, body }))
}

fn parse_call_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();
    let name = inner.next().unwrap().as_str().to_string();

    let mut args = Vec::new();
    for arg_pair in inner {
        let word = parse_word(arg_pair)?;
        args.push(Expression::String(word));
    }

    Ok(Statement::Call(CallStmt { name, args }))
}

fn parse_exit_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();
    let code = if let Some(p) = inner.next() {
        let word = parse_word(p)?;
        if let Ok(num) = word.parse::<f64>() {
            Some(Expression::Number(num))
        } else {
            Some(Expression::String(word))
        }
    } else {
        None
    };
    Ok(Statement::Exit(code))
}

fn parse_brace_block(pair: pest::iterators::Pair<Rule>) -> Result<Block, ScriptError> {
    let mut statements = Vec::new();

    for inner_pair in pair.into_inner() {
        if let Rule::statement = inner_pair.as_rule() {
            if let Some(stmt) = parse_statement(inner_pair)? {
                statements.push(stmt);
            }
        }
    }

    Ok(statements)
}

fn parse_brace_list(pair: pest::iterators::Pair<Rule>) -> Result<Vec<String>, ScriptError> {
    let mut items = Vec::new();

    for inner_pair in pair.into_inner() {
        if let Rule::identifier = inner_pair.as_rule() {
            items.push(inner_pair.as_str().to_string());
        }
    }

    Ok(items)
}

fn parse_word(pair: pest::iterators::Pair<Rule>) -> Result<String, ScriptError> {
    match pair.as_rule() {
        Rule::word => {
            let inner = pair.into_inner().next().unwrap();
            parse_word(inner)
        }
        Rule::number => Ok(pair.as_str().to_string()),
        Rule::variable => {
            // Keep the $ for later substitution
            Ok(pair.as_str().to_string())
        }
        Rule::string => {
            let s = pair.as_str();
            // Remove outer quotes and parse escape sequences
            let s = &s[1..s.len() - 1];
            Ok(parse_string_inner(s))
        }
        Rule::brace_string => {
            let s = pair.as_str();
            // Remove outer braces
            Ok(s[1..s.len() - 1].to_string())
        }
        Rule::bare_word => Ok(pair.as_str().to_string()),
        Rule::list => {
            // Convert list to space-separated string
            let mut items = Vec::new();
            for inner_pair in pair.into_inner() {
                items.push(parse_word(inner_pair)?);
            }
            Ok(items.join(" "))
        }
        _ => Err(ScriptError::RuntimeError(format!(
            "Unexpected word rule: {:?}",
            pair.as_rule()
        ))),
    }
}

fn parse_string_inner(s: &str) -> String {
    let mut result = String::new();
    let mut chars = s.chars();

    while let Some(ch) = chars.next() {
        if ch == '\\' {
            if let Some(next) = chars.next() {
                match next {
                    'n' => result.push('\n'),
                    'r' => result.push('\r'),
                    't' => result.push('\t'),
                    '\\' => result.push('\\'),
                    '"' => result.push('"'),
                    '$' => result.push('$'),
                    _ => {
                        result.push('\\');
                        result.push(next);
                    }
                }
            }
        } else {
            result.push(ch);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_condition_builds_binary_op() {
        let block = parse_script("if { $x == 1 } {\n}\n").expect("parse failed");
        assert_eq!(block.len(), 1);

        match &block[0] {
            Statement::If(if_stmt) => {
                assert_eq!(
                    if_stmt.condition,
                    Expression::BinaryOp {
                        left: Box::new(Expression::Variable("x".to_string())),
                        op: BinaryOperator::Eq,
                        right: Box::new(Expression::Number(1.0)),
                    }
                );
            }
            other => panic!("Expected Statement::If, got {:?}", other),
        }
    }

    #[test]
    fn test_parse_set_paren_expr_builds_binary_op() {
        let block = parse_script("set i ($i + 1)\n").expect("parse failed");
        assert_eq!(block.len(), 1);

        match &block[0] {
            Statement::Set(set_stmt) => {
                assert_eq!(set_stmt.name, "i");
                assert_eq!(
                    set_stmt.value,
                    Expression::BinaryOp {
                        left: Box::new(Expression::Variable("i".to_string())),
                        op: BinaryOperator::Add,
                        right: Box::new(Expression::Number(1.0)),
                    }
                );
            }
            other => panic!("Expected Statement::Set, got {:?}", other),
        }
    }
}
