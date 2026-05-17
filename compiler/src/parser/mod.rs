pub mod ast;

use crate::lexer::token::{Spanned, Token};
use ast::*;

pub struct Parser {
    tokens: Vec<Spanned>,
    pos: usize,
}

impl Parser {
    pub fn new(tokens: Vec<Spanned>) -> Self {
        Self { tokens, pos: 0 }
    }

    pub fn parse_program(&mut self) -> Result<Program, ParseError> {
        let mut items = Vec::new();

        while !self.at_eof() {
            self.skip_newlines();
            if self.at_eof() {
                break;
            }
            items.push(self.parse_item()?);
        }

        Ok(Program { items })
    }

    fn parse_item(&mut self) -> Result<Item, ParseError> {
        match self.peek() {
            Token::Fn => self.parse_function().map(Item::Function),
            Token::Struct => self.parse_struct().map(Item::Struct),
            Token::Enum => self.parse_enum().map(Item::Enum),
            Token::Mod => self.parse_module().map(Item::Module),
            Token::Use => self.parse_use().map(Item::Use),
            other => Err(self.error(format!("expected item (fn, struct, enum, mod, use), got {other:?}"))),
        }
    }

    fn parse_function(&mut self) -> Result<Function, ParseError> {
        self.expect(Token::Fn)?;
        let name = self.expect_ident()?;
        self.skip_newlines();

        let mut params = Vec::new();
        let mut returns = None;
        let mut invariants = Vec::new();
        let mut requires = Vec::new();
        let mut ensures = Vec::new();
        let mut mode = FnMode::Strict;
        let mut intent = None;
        let mut confidence = None;
        let mut fallback = None;
        let mut guarantee = None;
        let mut body = None;

        loop {
            self.skip_newlines();
            match self.peek() {
                Token::In => {
                    self.advance();
                    params = self.parse_params()?;
                }
                Token::Out => {
                    self.advance();
                    let p = self.parse_single_param()?;
                    returns = Some(p);
                }
                Token::Inv => {
                    self.advance();
                    invariants.push(self.parse_expr()?);
                }
                Token::Req => {
                    self.advance();
                    requires.push(self.parse_expr()?);
                }
                Token::Ens => {
                    self.advance();
                    ensures.push(self.parse_expr()?);
                }
                Token::Do => {
                    self.advance();
                    body = Some(self.parse_expr()?);
                }
                Token::Mode => {
                    self.advance();
                    let m = self.expect_ident()?;
                    mode = match m.as_str() {
                        "strict" => FnMode::Strict,
                        "fluid" => FnMode::Fluid,
                        "async" => FnMode::Async,
                        _ => return Err(self.error(format!("unknown mode: {m}"))),
                    };
                }
                Token::Intent => {
                    self.advance();
                    intent = Some(self.expect_string()?);
                }
                Token::Confidence => {
                    self.advance();
                    confidence = Some(self.expect_float()?);
                }
                Token::Fallback => {
                    self.advance();
                    fallback = Some(self.parse_expr()?);
                }
                Token::Guarantee => {
                    self.advance();
                    let g = self.expect_ident()?;
                    guarantee = Some(g);
                }
                _ => break,
            }
        }

        let body = body.or(fallback.clone()).unwrap_or(Expr::Block(vec![]));

        Ok(Function {
            name,
            params,
            returns,
            invariants,
            requires,
            ensures,
            mode,
            intent,
            confidence,
            fallback,
            guarantee,
            body,
        })
    }

    fn parse_params(&mut self) -> Result<Vec<Param>, ParseError> {
        let mut params = Vec::new();
        while self.peek_is_ident() && self.peek_ahead_is(Token::Colon) {
            params.push(self.parse_single_param()?);
        }
        Ok(params)
    }

    fn parse_single_param(&mut self) -> Result<Param, ParseError> {
        let name = self.expect_ident()?;
        self.expect(Token::Colon)?;
        let ty = self.parse_type()?;
        Ok(Param { name, ty })
    }

    fn parse_type(&mut self) -> Result<Type, ParseError> {
        let ty = match self.peek() {
            Token::LBracket => {
                self.advance();
                let inner = self.parse_type()?;
                self.expect(Token::RBracket)?;
                Type::List(Box::new(inner))
            }
            Token::LBrace => {
                self.advance();
                let key = self.parse_type()?;
                self.expect(Token::Colon)?;
                let val = self.parse_type()?;
                self.expect(Token::RBrace)?;
                Type::Map(Box::new(key), Box::new(val))
            }
            Token::LParen => {
                self.advance();
                let mut types = vec![self.parse_type()?];
                while self.peek() == Token::Comma {
                    self.advance();
                    types.push(self.parse_type()?);
                }
                self.expect(Token::RParen)?;
                Type::Tuple(types)
            }
            _ => {
                let name = self.expect_ident()?;
                Type::Named(name)
            }
        };

        if self.peek() == Token::Question {
            self.advance();
            Ok(Type::Optional(Box::new(ty)))
        } else {
            Ok(ty)
        }
    }

    fn parse_expr(&mut self) -> Result<Expr, ParseError> {
        let left = self.parse_primary()?;

        if self.peek() == Token::Pipe {
            self.advance();
            let right = self.parse_expr()?;
            return Ok(Expr::Pipe(Box::new(left), Box::new(right)));
        }

        Ok(left)
    }

    fn parse_primary(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Token::IntLit(_) | Token::FloatLit(_) | Token::StrLit(_) |
            Token::True | Token::False => self.parse_atom(),
            Token::Let | Token::Mut => self.parse_let(),
            Token::If => self.parse_if(),
            Token::Each => self.parse_each(),
            Token::While => self.parse_while(),
            Token::Match => self.parse_match(),
            Token::Spawn => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Expr::Spawn(Box::new(expr)))
            }
            Token::AwaitKw => {
                self.advance();
                let expr = self.parse_expr()?;
                Ok(Expr::Await(Box::new(expr)))
            }
            Token::Send => {
                self.advance();
                let chan = self.parse_atom()?;
                let val = self.parse_expr()?;
                Ok(Expr::Send(Box::new(chan), Box::new(val)))
            }
            Token::Recv => {
                self.advance();
                let chan = self.parse_expr()?;
                Ok(Expr::Recv(Box::new(chan)))
            }
            Token::Add | Token::Sub | Token::Mul | Token::Div | Token::Modulo |
            Token::Eq | Token::Neq | Token::Gt | Token::Lt | Token::Gte |
            Token::Lte | Token::And | Token::Or | Token::Not => {
                self.parse_op_expr()
            }
            Token::Ident(_) => {
                let name = self.expect_ident()?;

                if self.peek() == Token::Dot {
                    self.advance();
                    let field = self.expect_ident()?;
                    let mut expr = Expr::Field(Box::new(Expr::Ident(name)), field);

                    if self.peek() == Token::At {
                        self.advance();
                        let temporal = self.expect_ident()?;
                        expr = Expr::Temporal(Box::new(expr), temporal);
                    }
                    return Ok(expr);
                }

                if self.peek() == Token::At {
                    self.advance();
                    let temporal = self.expect_ident()?;
                    return Ok(Expr::Temporal(Box::new(Expr::Ident(name)), temporal));
                }

                if self.is_arg_start() {
                    let mut args = Vec::new();
                    while self.is_arg_start() {
                        args.push(self.parse_arg()?);
                    }
                    Ok(Expr::Call(name, args))
                } else {
                    Ok(Expr::Ident(name))
                }
            }
            other => Err(self.error(format!("expected expression, got {other:?}"))),
        }
    }

    /// Parse an atom: a simple value with no argument consumption.
    /// Atoms are literals, identifiers (with optional field/temporal access), and booleans.
    fn parse_atom(&mut self) -> Result<Expr, ParseError> {
        match self.peek() {
            Token::IntLit(_) => {
                if let Token::IntLit(n) = self.advance_and_get() {
                    Ok(Expr::IntLit(n))
                } else { unreachable!() }
            }
            Token::FloatLit(_) => {
                if let Token::FloatLit(n) = self.advance_and_get() {
                    Ok(Expr::FloatLit(n))
                } else { unreachable!() }
            }
            Token::StrLit(_) => {
                if let Token::StrLit(s) = self.advance_and_get() {
                    Ok(Expr::StrLit(s))
                } else { unreachable!() }
            }
            Token::True => { self.advance(); Ok(Expr::BoolLit(true)) }
            Token::False => { self.advance(); Ok(Expr::BoolLit(false)) }
            Token::Ident(_) => {
                let name = self.expect_ident()?;

                if self.peek() == Token::Dot {
                    self.advance();
                    let field = self.expect_ident()?;
                    let mut expr = Expr::Field(Box::new(Expr::Ident(name)), field);
                    if self.peek() == Token::At {
                        self.advance();
                        let temporal = self.expect_ident()?;
                        expr = Expr::Temporal(Box::new(expr), temporal);
                    }
                    return Ok(expr);
                }

                if self.peek() == Token::At {
                    self.advance();
                    let temporal = self.expect_ident()?;
                    return Ok(Expr::Temporal(Box::new(Expr::Ident(name)), temporal));
                }

                Ok(Expr::Ident(name))
            }
            other => Err(self.error(format!("expected value, got {other:?}"))),
        }
    }

    /// Parse an operator expression with fixed arity.
    /// Binary ops consume exactly 2 atoms, unary ops consume 1.
    fn parse_op_expr(&mut self) -> Result<Expr, ParseError> {
        let op = self.parse_op()?;
        let arity = match op {
            Op::Not => 1,
            _ => 2,
        };
        let mut args = Vec::new();
        for _ in 0..arity {
            if self.is_atom_start() {
                args.push(self.parse_atom()?);
            } else {
                break;
            }
        }
        Ok(Expr::Op(op, args))
    }

    /// Parse a function argument: either an op-with-atoms or a plain atom.
    /// Does NOT allow nested function calls (use pipes for composition).
    fn parse_arg(&mut self) -> Result<Expr, ParseError> {
        if self.is_op_start() {
            self.parse_op_expr()
        } else {
            self.parse_atom()
        }
    }

    fn parse_let(&mut self) -> Result<Expr, ParseError> {
        let mutable = self.peek() == Token::Mut;
        self.advance(); // skip let or mut

        let name = self.expect_ident()?;
        let ty = if self.peek() == Token::Colon {
            self.advance();
            Some(self.parse_type()?)
        } else {
            None
        };
        self.expect(Token::Assign)?;
        let value = self.parse_expr()?;

        Ok(Expr::Let {
            name,
            ty,
            mutable,
            value: Box::new(value),
        })
    }

    fn parse_if(&mut self) -> Result<Expr, ParseError> {
        self.expect(Token::If)?;
        let condition = self.parse_expr()?;
        self.skip_newlines();

        let mut then_body = Vec::new();
        while !matches!(self.peek(), Token::Elif | Token::Else | Token::End | Token::Eof) {
            self.skip_newlines();
            if matches!(self.peek(), Token::Elif | Token::Else | Token::End | Token::Eof) {
                break;
            }
            then_body.push(self.parse_expr()?);
            self.skip_newlines();
        }

        let mut elif_branches = Vec::new();
        while self.peek() == Token::Elif {
            self.advance();
            let cond = self.parse_expr()?;
            self.skip_newlines();
            let mut body = Vec::new();
            while !matches!(self.peek(), Token::Elif | Token::Else | Token::End | Token::Eof) {
                self.skip_newlines();
                if matches!(self.peek(), Token::Elif | Token::Else | Token::End | Token::Eof) {
                    break;
                }
                body.push(self.parse_expr()?);
                self.skip_newlines();
            }
            elif_branches.push((cond, body));
        }

        let else_body = if self.peek() == Token::Else {
            self.advance();
            self.skip_newlines();
            let mut body = Vec::new();
            while !matches!(self.peek(), Token::End | Token::Eof) {
                self.skip_newlines();
                if matches!(self.peek(), Token::End | Token::Eof) {
                    break;
                }
                body.push(self.parse_expr()?);
                self.skip_newlines();
            }
            Some(body)
        } else {
            None
        };

        self.expect(Token::End)?;

        Ok(Expr::If {
            condition: Box::new(condition),
            then_body,
            elif_branches,
            else_body,
        })
    }

    fn parse_each(&mut self) -> Result<Expr, ParseError> {
        self.expect(Token::Each)?;
        let binding = self.expect_ident()?;
        self.expect_ident_value("in")?;
        let iter = self.parse_expr()?;
        self.skip_newlines();

        let mut body = Vec::new();
        while !matches!(self.peek(), Token::End | Token::Eof) {
            self.skip_newlines();
            if matches!(self.peek(), Token::End | Token::Eof) {
                break;
            }
            body.push(self.parse_expr()?);
            self.skip_newlines();
        }
        self.expect(Token::End)?;

        Ok(Expr::Each {
            binding,
            iter: Box::new(iter),
            body,
        })
    }

    fn parse_while(&mut self) -> Result<Expr, ParseError> {
        self.expect(Token::While)?;
        let condition = self.parse_expr()?;
        self.skip_newlines();

        let mut body = Vec::new();
        while !matches!(self.peek(), Token::End | Token::Eof) {
            self.skip_newlines();
            if matches!(self.peek(), Token::End | Token::Eof) {
                break;
            }
            body.push(self.parse_expr()?);
            self.skip_newlines();
        }
        self.expect(Token::End)?;

        Ok(Expr::While {
            condition: Box::new(condition),
            body,
        })
    }

    fn parse_struct(&mut self) -> Result<StructDef, ParseError> {
        self.expect(Token::Struct)?;
        let name = self.expect_ident()?;
        self.skip_newlines();

        let mut fields = Vec::new();
        while self.peek() != Token::End && !self.at_eof() {
            self.skip_newlines();
            if self.peek() == Token::End {
                break;
            }
            fields.push(self.parse_single_param()?);
            self.skip_newlines();
        }
        self.expect(Token::End)?;

        Ok(StructDef { name, fields })
    }

    fn parse_enum(&mut self) -> Result<EnumDef, ParseError> {
        self.expect(Token::Enum)?;
        let name = self.expect_ident()?;
        self.skip_newlines();

        let mut variants = Vec::new();
        while self.peek() != Token::End && !self.at_eof() {
            self.skip_newlines();
            if self.peek() == Token::End {
                break;
            }
            let variant_name = self.expect_ident()?;
            let mut fields = Vec::new();
            while self.peek_is_ident() && !self.is_at_newline_or_end() {
                fields.push(self.parse_type()?);
            }
            variants.push(Variant { name: variant_name, fields });
            self.skip_newlines();
        }
        self.expect(Token::End)?;

        Ok(EnumDef { name, variants })
    }

    fn parse_match(&mut self) -> Result<Expr, ParseError> {
        self.expect(Token::Match)?;
        let scrutinee = self.parse_expr()?;
        self.skip_newlines();

        let mut arms = Vec::new();
        while !matches!(self.peek(), Token::End | Token::Eof) {
            self.skip_newlines();
            if matches!(self.peek(), Token::End | Token::Eof) {
                break;
            }
            let pattern = self.parse_pattern()?;
            self.expect(Token::Arrow)?;
            let mut body = Vec::new();
            while !self.is_at_newline_or_end() && !matches!(self.peek(), Token::End | Token::Eof) {
                body.push(self.parse_expr()?);
            }
            if body.is_empty() {
                body.push(Expr::Block(vec![]));
            }
            arms.push(MatchArm { pattern, body });
            self.skip_newlines();
        }
        self.expect(Token::End)?;

        Ok(Expr::Match {
            scrutinee: Box::new(scrutinee),
            arms,
        })
    }

    fn parse_pattern(&mut self) -> Result<Pattern, ParseError> {
        match self.peek() {
            Token::Underscore => {
                self.advance();
                Ok(Pattern::Wildcard)
            }
            Token::IntLit(_) => {
                let expr = self.parse_atom()?;
                Ok(Pattern::Literal(expr))
            }
            Token::FloatLit(_) => {
                let expr = self.parse_atom()?;
                Ok(Pattern::Literal(expr))
            }
            Token::StrLit(_) => {
                let expr = self.parse_atom()?;
                Ok(Pattern::Literal(expr))
            }
            Token::True | Token::False => {
                let expr = self.parse_atom()?;
                Ok(Pattern::Literal(expr))
            }
            Token::LParen => {
                self.advance();
                let mut pats = vec![self.parse_pattern()?];
                while self.peek() == Token::Comma {
                    self.advance();
                    pats.push(self.parse_pattern()?);
                }
                self.expect(Token::RParen)?;
                Ok(Pattern::Tuple(pats))
            }
            Token::Ident(_) => {
                let name = self.expect_ident()?;
                if name.chars().next().is_some_and(|c| c.is_uppercase()) {
                    let mut sub_patterns = Vec::new();
                    while self.is_pattern_arg_start() {
                        sub_patterns.push(self.parse_pattern()?);
                    }
                    Ok(Pattern::Variant(name, sub_patterns))
                } else {
                    Ok(Pattern::Binding(name))
                }
            }
            other => Err(self.error(format!("expected pattern, got {other:?}"))),
        }
    }

    fn is_pattern_arg_start(&self) -> bool {
        matches!(self.peek(),
            Token::Ident(_) | Token::Underscore | Token::IntLit(_) |
            Token::FloatLit(_) | Token::StrLit(_) | Token::True |
            Token::False | Token::LParen
        ) && !matches!(self.peek(), Token::Arrow)
    }

    fn is_at_newline_or_end(&self) -> bool {
        matches!(self.peek(), Token::Newline | Token::End | Token::Eof)
            || self.pos < self.tokens.len()
                && matches!(self.tokens[self.pos].token, Token::Newline)
    }

    fn parse_module(&mut self) -> Result<ModuleDecl, ParseError> {
        self.expect(Token::Mod)?;
        let name = self.expect_ident()?;
        Ok(ModuleDecl { name })
    }

    fn parse_use(&mut self) -> Result<UseDecl, ParseError> {
        self.expect(Token::Use)?;
        let mut path = vec![self.expect_ident()?];
        while self.peek() == Token::Dot {
            self.advance();
            path.push(self.expect_ident()?);
        }
        Ok(UseDecl { path })
    }

    fn parse_op(&mut self) -> Result<Op, ParseError> {
        let op = match self.peek() {
            Token::Add => Op::Add,
            Token::Sub => Op::Sub,
            Token::Mul => Op::Mul,
            Token::Div => Op::Div,
            Token::Modulo => Op::Modulo,
            Token::Eq  => Op::Eq,
            Token::Neq => Op::Neq,
            Token::Gt  => Op::Gt,
            Token::Lt  => Op::Lt,
            Token::Gte => Op::Gte,
            Token::Lte => Op::Lte,
            Token::And => Op::And,
            Token::Or  => Op::Or,
            Token::Not => Op::Not,
            other => return Err(self.error(format!("expected operator, got {other:?}"))),
        };
        self.advance();
        Ok(op)
    }

    // ── helpers ──

    fn peek(&self) -> Token {
        self.tokens.get(self.pos).map(|s| s.token.clone()).unwrap_or(Token::Eof)
    }

    fn peek_is_ident(&self) -> bool {
        matches!(self.peek(), Token::Ident(_))
    }

    fn peek_ahead_is(&self, expected: Token) -> bool {
        self.tokens.get(self.pos + 1).map(|s| s.token == expected).unwrap_or(false)
    }

    fn advance(&mut self) {
        self.pos += 1;
    }

    fn advance_and_get(&mut self) -> Token {
        let t = self.tokens[self.pos].token.clone();
        self.pos += 1;
        t
    }

    fn at_eof(&self) -> bool {
        self.pos >= self.tokens.len() || self.tokens[self.pos].token == Token::Eof
    }

    fn skip_newlines(&mut self) {
        while self.pos < self.tokens.len() && matches!(self.tokens[self.pos].token, Token::Newline | Token::Comment(_)) {
            self.pos += 1;
        }
    }

    fn expect(&mut self, expected: Token) -> Result<(), ParseError> {
        let got = self.peek();
        if std::mem::discriminant(&got) == std::mem::discriminant(&expected) {
            self.advance();
            Ok(())
        } else {
            Err(self.error(format!("expected {expected:?}, got {got:?}")))
        }
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Token::Ident(s) => { self.advance(); Ok(s) }
            // Allow `in` keyword when used as "each x in collection" context
            Token::In => { self.advance(); Ok("in".to_string()) }
            other => Err(self.error(format!("expected identifier, got {other:?}")))
        }
    }

    fn expect_ident_value(&mut self, expected: &str) -> Result<(), ParseError> {
        match self.peek() {
            Token::Ident(ref s) if s == expected => { self.advance(); Ok(()) }
            Token::In if expected == "in" => { self.advance(); Ok(()) }
            other => Err(self.error(format!("expected '{expected}', got {other:?}")))
        }
    }

    fn expect_string(&mut self) -> Result<String, ParseError> {
        match self.peek() {
            Token::StrLit(s) => { self.advance(); Ok(s) }
            other => Err(self.error(format!("expected string literal, got {other:?}")))
        }
    }

    fn expect_float(&mut self) -> Result<f64, ParseError> {
        match self.peek() {
            Token::FloatLit(f) => { self.advance(); Ok(f) }
            Token::IntLit(i) => { self.advance(); Ok(i as f64) }
            other => Err(self.error(format!("expected number, got {other:?}")))
        }
    }

    #[allow(dead_code)]
    fn is_expr_start(&self) -> bool {
        self.is_arg_start() || matches!(self.peek(),
            Token::Let | Token::Mut | Token::If | Token::Each | Token::While |
            Token::Match | Token::Spawn | Token::AwaitKw | Token::Send | Token::Recv
        )
    }

    fn is_atom_start(&self) -> bool {
        matches!(self.peek(),
            Token::IntLit(_) | Token::FloatLit(_) | Token::StrLit(_) |
            Token::True | Token::False | Token::Ident(_)
        )
    }

    fn is_op_start(&self) -> bool {
        matches!(self.peek(),
            Token::Add | Token::Sub | Token::Mul | Token::Div | Token::Modulo |
            Token::Eq | Token::Neq | Token::Gt | Token::Lt | Token::Gte |
            Token::Lte | Token::And | Token::Or | Token::Not
        )
    }

    fn is_arg_start(&self) -> bool {
        self.is_atom_start() || self.is_op_start()
    }

    fn error(&self, message: String) -> ParseError {
        let (line, col) = self.tokens.get(self.pos)
            .map(|s| (s.span.line, s.span.col))
            .unwrap_or((0, 0));
        ParseError { message, line, col }
    }
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "parse error at {}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for ParseError {}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lexer::Lexer;

    fn parse(input: &str) -> Program {
        let tokens = Lexer::new(input).tokenize().unwrap();
        Parser::new(tokens).parse_program().unwrap()
    }

    #[test]
    fn parse_simple_function() {
        let prog = parse("fn add_one\n  in x: int\n  out result: int\n  do add x 1");
        assert_eq!(prog.items.len(), 1);
        match &prog.items[0] {
            Item::Function(f) => {
                assert_eq!(f.name, "add_one");
                assert_eq!(f.params.len(), 1);
                assert!(f.returns.is_some());
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_struct() {
        let prog = parse("struct Account\n  id: uint\n  balance: int\nend");
        assert_eq!(prog.items.len(), 1);
        match &prog.items[0] {
            Item::Struct(s) => {
                assert_eq!(s.name, "Account");
                assert_eq!(s.fields.len(), 2);
            }
            _ => panic!("expected struct"),
        }
    }

    #[test]
    fn parse_use() {
        let prog = parse("use io.print");
        match &prog.items[0] {
            Item::Use(u) => assert_eq!(u.path, vec!["io", "print"]),
            _ => panic!("expected use"),
        }
    }

    #[test]
    fn parse_pipe() {
        let prog = parse("fn test\n  do filter nums gt 0 | reduce add");
        match &prog.items[0] {
            Item::Function(f) => {
                assert!(matches!(f.body, Expr::Pipe(_, _)));
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_with_invariants() {
        let prog = parse("fn clamp\n  in val: int lo: int hi: int\n  out result: int\n  inv gte result lo\n  inv lte result hi\n  do max lo min hi val");
        match &prog.items[0] {
            Item::Function(f) => {
                assert_eq!(f.invariants.len(), 2);
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_requires() {
        let prog = parse("fn safe_div\n  in a: int b: int\n  out result: int\n  req neq b 0\n  do div a b");
        match &prog.items[0] {
            Item::Function(f) => {
                assert_eq!(f.name, "safe_div");
                assert_eq!(f.requires.len(), 1);
                assert!(f.ensures.is_empty());
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_ensures() {
        let prog = parse("fn abs_val\n  in x: int\n  out result: int\n  ens gte result 0\n  do abs x");
        match &prog.items[0] {
            Item::Function(f) => {
                assert_eq!(f.name, "abs_val");
                assert!(f.requires.is_empty());
                assert_eq!(f.ensures.len(), 1);
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_requires_and_ensures() {
        let prog = parse("fn bounded\n  in x: int\n  out result: int\n  req gte x 0\n  ens gte result 0\n  ens lte result 100\n  do x");
        match &prog.items[0] {
            Item::Function(f) => {
                assert_eq!(f.requires.len(), 1);
                assert_eq!(f.ensures.len(), 2);
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_no_contracts() {
        let prog = parse("fn identity\n  in x: int\n  out result: int\n  do x");
        match &prog.items[0] {
            Item::Function(f) => {
                assert!(f.requires.is_empty());
                assert!(f.ensures.is_empty());
                assert!(f.invariants.is_empty());
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_enum_definition() {
        let prog = parse("enum Option\n  Some int\n  None\nend");
        assert_eq!(prog.items.len(), 1);
        match &prog.items[0] {
            Item::Enum(e) => {
                assert_eq!(e.name, "Option");
                assert_eq!(e.variants.len(), 2);
                assert_eq!(e.variants[0].name, "Some");
                assert_eq!(e.variants[0].fields.len(), 1);
                assert_eq!(e.variants[1].name, "None");
                assert!(e.variants[1].fields.is_empty());
            }
            _ => panic!("expected enum"),
        }
    }

    #[test]
    fn parse_match_expression() {
        let prog = parse("fn test\n  in x: int\n  do match x\n    0 => 1\n    _ => 2\n  end");
        match &prog.items[0] {
            Item::Function(f) => {
                assert!(matches!(f.body, Expr::Match { .. }));
                if let Expr::Match { ref arms, .. } = f.body {
                    assert_eq!(arms.len(), 2);
                    assert!(matches!(arms[0].pattern, Pattern::Literal(_)));
                    assert!(matches!(arms[1].pattern, Pattern::Wildcard));
                }
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_spawn_await() {
        let prog = parse("fn test\n  do await spawn add 1 2");
        match &prog.items[0] {
            Item::Function(f) => {
                assert!(matches!(f.body, Expr::Await(_)));
            }
            _ => panic!("expected function"),
        }
    }

    #[test]
    fn parse_option_like_type() {
        let prog = parse("enum Result\n  Ok int\n  Err str\nend");
        match &prog.items[0] {
            Item::Enum(e) => {
                assert_eq!(e.name, "Result");
                assert_eq!(e.variants.len(), 2);
                assert_eq!(e.variants[0].name, "Ok");
                assert_eq!(e.variants[1].name, "Err");
            }
            _ => panic!("expected enum"),
        }
    }
}
