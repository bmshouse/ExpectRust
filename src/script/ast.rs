//! Abstract Syntax Tree (AST) definitions for Expect scripts.

/// A block of statements.
pub type Block = Vec<Statement>;

/// A statement in an Expect script.
#[derive(Debug, Clone, PartialEq)]
pub enum Statement {
    /// Spawn a new process: `spawn command args...`
    Spawn(SpawnStmt),
    /// Expect one or more patterns: `expect pattern` or `expect { pattern { action } ... }`
    Expect(ExpectStmt),
    /// Send data to the process: `send "data"`
    Send(SendStmt),
    /// Set a variable: `set var value`
    Set(SetStmt),
    /// Conditional statement: `if { condition } { statements } else { statements }`
    If(IfStmt),
    /// While loop: `while { condition } { statements }`
    While(WhileStmt),
    /// For loop: `for { init } { condition } { increment } { statements }`
    For(ForStmt),
    /// Procedure definition: `proc name { args } { body }`
    Proc(ProcStmt),
    /// Procedure call: `name args...`
    Call(CallStmt),
    /// Close the session: `close`
    Close,
    /// Wait for process exit: `wait`
    Wait,
    /// Exit the script: `exit` or `exit code`
    Exit(Option<Expression>),
}

/// Spawn statement.
#[derive(Debug, Clone, PartialEq)]
pub struct SpawnStmt {
    /// Command to spawn (includes command and arguments as a single expression).
    pub command: Expression,
}

/// Expect statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectStmt {
    /// Patterns to match.
    pub patterns: Vec<ExpectPattern>,
}

/// A single pattern in an expect statement.
#[derive(Debug, Clone, PartialEq)]
pub struct ExpectPattern {
    /// The pattern type and value.
    pub pattern_type: PatternType,
    /// Optional action block to execute on match.
    pub action: Option<Block>,
}

/// Type of pattern to match.
#[derive(Debug, Clone, PartialEq)]
pub enum PatternType {
    /// Exact string match.
    Exact(String),
    /// Regular expression pattern.
    Regex(String),
    /// Glob pattern.
    Glob(String),
    /// Match end of file.
    Eof,
    /// Match timeout condition.
    Timeout,
}

/// Send statement.
#[derive(Debug, Clone, PartialEq)]
pub struct SendStmt {
    /// Data to send (expression that evaluates to a string).
    pub data: Expression,
}

/// Set statement (variable assignment).
#[derive(Debug, Clone, PartialEq)]
pub struct SetStmt {
    /// Variable name.
    pub name: String,
    /// Value expression.
    pub value: Expression,
}

/// If statement.
#[derive(Debug, Clone, PartialEq)]
pub struct IfStmt {
    /// Condition expression.
    pub condition: Expression,
    /// Statements to execute if condition is true.
    pub then_block: Block,
    /// Optional else block.
    pub else_block: Option<Block>,
}

/// While loop.
#[derive(Debug, Clone, PartialEq)]
pub struct WhileStmt {
    /// Loop condition.
    pub condition: Expression,
    /// Loop body.
    pub body: Block,
}

/// For loop.
#[derive(Debug, Clone, PartialEq)]
pub struct ForStmt {
    /// Initialization statement.
    pub init: Box<Statement>,
    /// Loop condition.
    pub condition: Expression,
    /// Increment statement.
    pub increment: Box<Statement>,
    /// Loop body.
    pub body: Block,
}

/// Procedure definition.
#[derive(Debug, Clone, PartialEq)]
pub struct ProcStmt {
    /// Procedure name.
    pub name: String,
    /// Parameter names.
    pub params: Vec<String>,
    /// Procedure body.
    pub body: Block,
}

/// Procedure call.
#[derive(Debug, Clone, PartialEq)]
pub struct CallStmt {
    /// Procedure name.
    pub name: String,
    /// Arguments.
    pub args: Vec<Expression>,
}

/// An expression that evaluates to a value.
#[derive(Debug, Clone, PartialEq)]
pub enum Expression {
    /// String literal: `"text"` or `{text}`
    String(String),
    /// Number literal: `42` or `3.14`
    Number(f64),
    /// Variable reference: `$varname`
    Variable(String),
    /// List: `{item1 item2 item3}`
    List(Vec<Expression>),
    /// Binary operation: `$a + $b`
    BinaryOp {
        /// Left operand.
        left: Box<Expression>,
        /// Operator.
        op: BinaryOperator,
        /// Right operand.
        right: Box<Expression>,
    },
    /// Unary operation: `!$var`
    UnaryOp {
        /// Operator.
        op: UnaryOperator,
        /// Operand.
        operand: Box<Expression>,
    },
}

/// Binary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOperator {
    /// Addition: `+`
    Add,
    /// Subtraction: `-`
    Sub,
    /// Multiplication: `*`
    Mul,
    /// Division: `/`
    Div,
    /// Equality: `==`
    Eq,
    /// Inequality: `!=`
    Ne,
    /// Less than: `<`
    Lt,
    /// Greater than: `>`
    Gt,
    /// Less than or equal: `<=`
    Le,
    /// Greater than or equal: `>=`
    Ge,
    /// Logical AND: `&&`
    And,
    /// Logical OR: `||`
    Or,
}

/// Unary operators.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOperator {
    /// Negation: `-`
    Neg,
    /// Logical NOT: `!`
    Not,
}

/// Represents a stored procedure.
#[derive(Debug, Clone)]
pub struct Procedure {
    /// Parameter names.
    pub params: Vec<String>,
    /// Procedure body.
    pub body: Block,
}

impl Procedure {
    /// Create a new procedure.
    pub fn new(params: Vec<String>, body: Block) -> Self {
        Self { params, body }
    }
}
