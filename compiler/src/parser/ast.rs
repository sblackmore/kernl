/// Top-level program — a sequence of items.
#[derive(Debug, Clone)]
pub struct Program {
    pub items: Vec<Item>,
}

/// A top-level declaration.
#[derive(Debug, Clone)]
pub enum Item {
    Function(Function),
    Struct(StructDef),
    Enum(EnumDef),
    Module(ModuleDecl),
    Use(UseDecl),
}

/// Function declaration.
#[derive(Debug, Clone)]
pub struct Function {
    pub name: String,
    pub params: Vec<Param>,
    pub returns: Option<Param>,
    pub invariants: Vec<Expr>,
    pub requires: Vec<Expr>,
    pub ensures: Vec<Expr>,
    pub mode: FnMode,
    pub intent: Option<String>,
    pub confidence: Option<f64>,
    pub fallback: Option<Expr>,
    pub guarantee: Option<String>,
    pub body: Expr,
}

#[derive(Debug, Clone, PartialEq)]
pub enum FnMode {
    Strict,
    Fluid,
    Async,
}

/// Parameter (used in both `in` and `out` clauses).
#[derive(Debug, Clone)]
pub struct Param {
    pub name: String,
    pub ty: Type,
}

/// Type representation.
#[derive(Debug, Clone, PartialEq)]
pub enum Type {
    Named(String),              // int, uint, float, bool, str, void, or user-defined
    List(Box<Type>),            // [T]
    Map(Box<Type>, Box<Type>),  // {K: V}
    Tuple(Vec<Type>),           // (T, U)
    Optional(Box<Type>),        // T?
}

/// Expressions — the core of computation.
#[derive(Debug, Clone)]
pub enum Expr {
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),
    BoolLit(bool),
    Ident(String),

    /// Named operator applied to operands: `add x y`
    Op(Op, Vec<Expr>),

    /// Function / builtin call: `filter nums gt 0`
    Call(String, Vec<Expr>),

    /// Pipe chain: `expr | expr`
    Pipe(Box<Expr>, Box<Expr>),

    /// Field access: `account.balance`
    Field(Box<Expr>, String),

    /// Temporal reference: `balance@pre`
    Temporal(Box<Expr>, String),

    /// Let binding: `let x: int = expr`
    Let {
        name: String,
        ty: Option<Type>,
        mutable: bool,
        value: Box<Expr>,
    },

    /// If expression
    If {
        condition: Box<Expr>,
        then_body: Vec<Expr>,
        elif_branches: Vec<(Expr, Vec<Expr>)>,
        else_body: Option<Vec<Expr>>,
    },

    /// Each loop
    Each {
        binding: String,
        iter: Box<Expr>,
        body: Vec<Expr>,
    },

    /// While loop
    While {
        condition: Box<Expr>,
        body: Vec<Expr>,
    },

    /// Block (sequence of expressions, last is the value)
    Block(Vec<Expr>),

    /// Enum variant constructor: `Some 42`, `None`, `Ok value`
    EnumVariant(String, String, Vec<Expr>),

    /// Pattern match expression
    Match {
        scrutinee: Box<Expr>,
        arms: Vec<MatchArm>,
    },

    /// Spawn async task: `spawn expr`
    Spawn(Box<Expr>),

    /// Await a spawned task: `await handle`
    Await(Box<Expr>),

    /// Channel send: `send channel value`
    Send(Box<Expr>, Box<Expr>),

    /// Channel receive: `recv channel`
    Recv(Box<Expr>),
}

/// Named operators.
#[derive(Debug, Clone, PartialEq)]
pub enum Op {
    Add, Sub, Mul, Div, Modulo,
    Eq, Neq, Gt, Lt, Gte, Lte,
    And, Or, Not,
}

/// A single arm in a match expression
#[derive(Debug, Clone)]
pub struct MatchArm {
    pub pattern: Pattern,
    pub body: Vec<Expr>,
}

/// Patterns for match expressions
#[derive(Debug, Clone)]
pub enum Pattern {
    /// Wildcard: `_`
    Wildcard,
    /// Literal: `42`, `"hello"`, `true`
    Literal(Expr),
    /// Variable binding: `x`
    Binding(String),
    /// Enum variant: `Some x`, `None`, `Ok val`
    Variant(String, Vec<Pattern>),
    /// Tuple: `(a, b)`
    Tuple(Vec<Pattern>),
}

/// Struct definition.
#[derive(Debug, Clone)]
pub struct StructDef {
    pub name: String,
    pub fields: Vec<Param>,
}

/// Enum (algebraic data type) definition
#[derive(Debug, Clone)]
pub struct EnumDef {
    pub name: String,
    pub variants: Vec<Variant>,
}

/// A single variant of an enum
#[derive(Debug, Clone)]
pub struct Variant {
    pub name: String,
    pub fields: Vec<Type>,
}

/// Module declaration: `mod math`
#[derive(Debug, Clone)]
pub struct ModuleDecl {
    pub name: String,
}

/// Use / import: `use io.print`
#[derive(Debug, Clone)]
pub struct UseDecl {
    pub path: Vec<String>,
}
