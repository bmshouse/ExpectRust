//! Runtime value types for script execution.

use std::fmt;

/// A runtime value in an Expect script.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    /// String value.
    String(String),
    /// Numeric value.
    Number(f64),
    /// List of values.
    List(Vec<Value>),
    /// Boolean value.
    Bool(bool),
    /// Null/empty value.
    Null,
}

impl Value {
    /// Convert the value to a string.
    pub fn as_string(&self) -> String {
        match self {
            Value::String(s) => s.clone(),
            Value::Number(n) => {
                if n.fract() == 0.0 {
                    format!("{}", *n as i64)
                } else {
                    format!("{}", n)
                }
            }
            Value::List(items) => items
                .iter()
                .map(|v| v.as_string())
                .collect::<Vec<_>>()
                .join(" "),
            Value::Bool(b) => if *b { "1" } else { "0" }.to_string(),
            Value::Null => String::new(),
        }
    }

    /// Try to convert the value to a number.
    pub fn as_number(&self) -> Result<f64, String> {
        match self {
            Value::Number(n) => Ok(*n),
            Value::String(s) => s
                .parse::<f64>()
                .map_err(|_| format!("Cannot convert '{}' to number", s)),
            Value::Bool(b) => Ok(if *b { 1.0 } else { 0.0 }),
            Value::Null => Ok(0.0),
            Value::List(_) => Err("Cannot convert list to number".to_string()),
        }
    }

    /// Try to convert the value to a boolean.
    pub fn as_bool(&self) -> bool {
        match self {
            Value::Bool(b) => *b,
            Value::Number(n) => *n != 0.0,
            Value::String(s) => !s.is_empty() && s != "0" && s != "false",
            Value::List(items) => !items.is_empty(),
            Value::Null => false,
        }
    }

    /// Try to convert the value to a list.
    pub fn as_list(&self) -> Vec<Value> {
        match self {
            Value::List(items) => items.clone(),
            other => vec![other.clone()],
        }
    }

    /// Get the type name of this value.
    pub fn type_name(&self) -> &'static str {
        match self {
            Value::String(_) => "string",
            Value::Number(_) => "number",
            Value::List(_) => "list",
            Value::Bool(_) => "bool",
            Value::Null => "null",
        }
    }
}

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_string())
    }
}

impl From<String> for Value {
    fn from(s: String) -> Self {
        Value::String(s)
    }
}

impl From<&str> for Value {
    fn from(s: &str) -> Self {
        Value::String(s.to_string())
    }
}

impl From<f64> for Value {
    fn from(n: f64) -> Self {
        Value::Number(n)
    }
}

impl From<i32> for Value {
    fn from(n: i32) -> Self {
        Value::Number(n as f64)
    }
}

impl From<bool> for Value {
    fn from(b: bool) -> Self {
        Value::Bool(b)
    }
}

impl From<Vec<Value>> for Value {
    fn from(items: Vec<Value>) -> Self {
        Value::List(items)
    }
}
