//! Expression code generation.

use super::{TranslationError, Translator};
use crate::script::ast::*;

/// Generate Rust code for an expression.
pub fn generate_expression(
    expr: &Expression,
    translator: &Translator,
) -> Result<String, TranslationError> {
    match expr {
        Expression::String(s) => Ok(format!("\"{}\"", escape_string(s))),
        Expression::Number(n) => {
            // Format nicely - if it's a whole number, don't show decimals
            if n.fract() == 0.0 {
                Ok(format!("{:.0}", n))
            } else {
                Ok(format!("{}", n))
            }
        }
        Expression::Variable(v) => Ok(sanitize_variable_name(v)),
        Expression::List(items) => {
            let elements: Result<Vec<_>, _> = items
                .iter()
                .map(|item| generate_expression(item, translator))
                .collect();
            Ok(format!("vec![{}]", elements?.join(", ")))
        }
        Expression::BinaryOp { left, op, right } => {
            let left_code = generate_expression(left, translator)?;
            let right_code = generate_expression(right, translator)?;
            let op_str = binary_op_to_rust(*op);
            Ok(format!("({} {} {})", left_code, op_str, right_code))
        }
        Expression::UnaryOp { op, operand } => {
            let operand_code = generate_expression(operand, translator)?;
            let op_str = unary_op_to_rust(*op);
            Ok(format!("({}{})", op_str, operand_code))
        }
    }
}

/// Convert a binary operator to Rust syntax.
fn binary_op_to_rust(op: BinaryOperator) -> &'static str {
    match op {
        BinaryOperator::Add => "+",
        BinaryOperator::Sub => "-",
        BinaryOperator::Mul => "*",
        BinaryOperator::Div => "/",
        BinaryOperator::Eq => "==",
        BinaryOperator::Ne => "!=",
        BinaryOperator::Lt => "<",
        BinaryOperator::Gt => ">",
        BinaryOperator::Le => "<=",
        BinaryOperator::Ge => ">=",
        BinaryOperator::And => "&&",
        BinaryOperator::Or => "||",
    }
}

/// Convert a unary operator to Rust syntax.
fn unary_op_to_rust(op: UnaryOperator) -> &'static str {
    match op {
        UnaryOperator::Neg => "-",
        UnaryOperator::Not => "!",
    }
}

/// Escape special characters in a string for Rust string literal.
fn escape_string(s: &str) -> String {
    s.replace('\\', "\\\\")
        .replace('"', "\\\"")
        .replace('\n', "\\n")
        .replace('\r', "\\r")
        .replace('\t', "\\t")
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
    if sanitized
        .chars()
        .next()
        .map_or(false, |c| c.is_ascii_digit())
    {
        format!("var_{}", sanitized)
    } else if sanitized.is_empty() {
        "var_unnamed".to_string()
    } else {
        sanitized
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_escape_string() {
        assert_eq!(escape_string("hello"), "hello");
        assert_eq!(escape_string("hello\nworld"), "hello\\nworld");
        assert_eq!(escape_string("say \"hi\""), "say \\\"hi\\\"");
    }

    #[test]
    fn test_sanitize_variable_name() {
        assert_eq!(sanitize_variable_name("$foo"), "foo");
        assert_eq!(sanitize_variable_name("foo"), "foo");
        assert_eq!(sanitize_variable_name("123"), "var_123");
        assert_eq!(sanitize_variable_name("foo-bar"), "foo_bar");
    }
}
