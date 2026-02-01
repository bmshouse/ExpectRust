//! Execution context for script variables and procedures.

use std::collections::HashMap;

use crate::script::ast::Procedure;
use crate::script::value::Value;

/// Execution context containing variables and procedures.
#[derive(Debug, Default)]
pub struct Context {
    /// Variable storage.
    variables: HashMap<String, Value>,
    /// Procedure storage.
    procedures: HashMap<String, Procedure>,
    /// Parent context (for nested scopes).
    parent: Option<Box<Context>>,
}

impl Context {
    /// Create a new empty context.
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
            procedures: HashMap::new(),
            parent: None,
        }
    }

    /// Set a variable in the current context.
    pub fn set_variable(&mut self, name: String, value: Value) {
        self.variables.insert(name, value);
    }

    /// Get a variable from this context or any parent context.
    pub fn get_variable(&self, name: &str) -> Option<&Value> {
        self.variables
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_variable(name)))
    }

    /// Define a procedure in the current context.
    pub fn define_procedure(&mut self, name: String, procedure: Procedure) {
        self.procedures.insert(name, procedure);
    }

    /// Get a procedure from this context or any parent context.
    pub fn get_procedure(&self, name: &str) -> Option<&Procedure> {
        self.procedures
            .get(name)
            .or_else(|| self.parent.as_ref().and_then(|p| p.get_procedure(name)))
    }

    /// Extract all variables (for returning from script execution).
    pub fn into_variables(self) -> HashMap<String, Value> {
        self.variables
    }
}
