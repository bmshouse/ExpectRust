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
        Rule::exit_stmt => Ok(Some(parse_exit_stmt(inner)?)),
        Rule::call_stmt => Ok(Some(parse_call_stmt(inner)?)),
        _ => Ok(None),
    }
}

fn parse_spawn_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let inner = pair.into_inner();
    // Collect all words into a single command string
    let mut words = Vec::new();
    for word_pair in inner {
        words.push(parse_word(word_pair)?);
    }
    let command_str = words.join(" ");
    Ok(Statement::Spawn(SpawnStmt {
        command: Expression::String(command_str),
    }))
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
    let mut inner = pair.into_inner();
    let first = inner.next().unwrap();

    let pattern_type = match first.as_str() {
        "-re" => {
            let word = parse_word(inner.next().unwrap())?;
            PatternType::Regex(word)
        }
        "-gl" => {
            let word = parse_word(inner.next().unwrap())?;
            PatternType::Glob(word)
        }
        "timeout" => PatternType::Timeout,
        "eof" => PatternType::Eof,
        _ => {
            // It's a word (exact match)
            let word = parse_word(first)?;
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
    let word = parse_word(inner.next().unwrap())?;
    // Try to parse as number, otherwise string
    let value = if let Ok(num) = word.parse::<f64>() {
        Expression::Number(num)
    } else {
        Expression::String(word)
    };
    Ok(Statement::Set(SetStmt { name, value }))
}

fn parse_if_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();

    // First brace_block is the condition
    let cond_block = parse_brace_block(inner.next().unwrap())?;
    let condition = block_to_expression(cond_block);

    // Second brace_block is the then block
    let then_block = parse_brace_block(inner.next().unwrap())?;

    // Optional third brace_block is the else block
    let else_block = inner.next().map(|p| parse_brace_block(p)).transpose()?;

    Ok(Statement::If(IfStmt {
        condition,
        then_block,
        else_block,
    }))
}

fn parse_while_stmt(pair: pest::iterators::Pair<Rule>) -> Result<Statement, ScriptError> {
    let mut inner = pair.into_inner();

    let cond_block = parse_brace_block(inner.next().unwrap())?;
    let condition = block_to_expression(cond_block);

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

    let cond_block = parse_brace_block(inner.next().unwrap())?;
    let condition = block_to_expression(cond_block);

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

fn block_to_expression(block: Block) -> Expression {
    // For simplicity, convert a block to an expression by evaluating the last statement
    // In a real implementation, this would need more sophisticated handling
    if block.is_empty() {
        Expression::Number(1.0)
    } else {
        // For now, just use a placeholder - the interpreter will handle this properly
        Expression::Number(1.0)
    }
}
