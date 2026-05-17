/// Source location for diagnostics.
#[derive(Debug, Clone, PartialEq)]
pub struct Span {
    pub line: usize,
    pub col: usize,
    pub offset: usize,
    pub len: usize,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Token {
    // Keywords
    Fn,
    In,
    Out,
    Inv,
    Req,
    Ens,
    Do,
    Let,
    Mut,
    If,
    Elif,
    Else,
    End,
    Each,
    While,
    Struct,
    Enum,
    Match,
    Mod,
    Use,
    Mode,
    Spawn,
    AwaitKw,
    Send,
    Recv,
    Async,
    Intent,
    Confidence,
    Fallback,
    Guarantee,
    True,
    False,

    // Operators (named, not symbolic)
    Add,
    Sub,
    Mul,
    Div,
    Modulo,
    Eq,
    Neq,
    Gt,
    Lt,
    Gte,
    Lte,
    And,
    Or,
    Not,

    // Punctuation
    Colon,       // :
    Pipe,        // |
    Assign,      // =
    Arrow,       // =>
    Underscore,  // _
    LBracket,    // [
    RBracket,    // ]
    LBrace,      // {
    RBrace,      // }
    LParen,      // (
    RParen,      // )
    Comma,       // ,
    Dot,         // .
    Question,    // ?
    At,          // @

    // Literals
    IntLit(i64),
    FloatLit(f64),
    StrLit(String),

    // Identifiers
    Ident(String),

    // Structure
    Newline,
    Comment(String),
    Eof,
}

impl Token {
    pub fn keyword_from_str(s: &str) -> Option<Token> {
        match s {
            "fn" => Some(Token::Fn),
            "in" => Some(Token::In),
            "out" => Some(Token::Out),
            "inv" => Some(Token::Inv),
            "req" => Some(Token::Req),
            "ens" => Some(Token::Ens),
            "do" => Some(Token::Do),
            "let" => Some(Token::Let),
            "mut" => Some(Token::Mut),
            "if" => Some(Token::If),
            "elif" => Some(Token::Elif),
            "else" => Some(Token::Else),
            "end" => Some(Token::End),
            "each" => Some(Token::Each),
            "while" => Some(Token::While),
            "struct" => Some(Token::Struct),
            "enum" => Some(Token::Enum),
            "match" => Some(Token::Match),
            "mod" => Some(Token::Mod),
            "use" => Some(Token::Use),
            "mode" => Some(Token::Mode),
            "spawn" => Some(Token::Spawn),
            "await" => Some(Token::AwaitKw),
            "send" => Some(Token::Send),
            "recv" => Some(Token::Recv),
            "async" => Some(Token::Async),
            "intent" => Some(Token::Intent),
            "confidence" => Some(Token::Confidence),
            "fallback" => Some(Token::Fallback),
            "guarantee" => Some(Token::Guarantee),
            "true" => Some(Token::True),
            "false" => Some(Token::False),
            "add" => Some(Token::Add),
            "sub" => Some(Token::Sub),
            "mul" => Some(Token::Mul),
            "div" => Some(Token::Div),
            "modulo" => Some(Token::Modulo),
            "eq" => Some(Token::Eq),
            "neq" => Some(Token::Neq),
            "gt" => Some(Token::Gt),
            "lt" => Some(Token::Lt),
            "gte" => Some(Token::Gte),
            "lte" => Some(Token::Lte),
            "and" => Some(Token::And),
            "or" => Some(Token::Or),
            "not" => Some(Token::Not),
            _ => None,
        }
    }
}

/// A token paired with its source location.
#[derive(Debug, Clone, PartialEq)]
pub struct Spanned {
    pub token: Token,
    pub span: Span,
}
