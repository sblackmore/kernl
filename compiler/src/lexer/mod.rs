pub mod token;

use token::{Span, Spanned, Token};

pub struct Lexer<'src> {
    src: &'src [u8],
    pos: usize,
    line: usize,
    col: usize,
}

impl<'src> Lexer<'src> {
    pub fn new(source: &'src str) -> Self {
        Self {
            src: source.as_bytes(),
            pos: 0,
            line: 1,
            col: 1,
        }
    }

    pub fn tokenize(&mut self) -> Result<Vec<Spanned>, LexError> {
        let mut tokens = Vec::new();

        while self.pos < self.src.len() {
            self.skip_whitespace();
            if self.pos >= self.src.len() {
                break;
            }

            let ch = self.src[self.pos];

            match ch {
                b'\n' => {
                    tokens.push(self.make_span(Token::Newline, 1));
                    self.advance();
                    self.line += 1;
                    self.col = 1;
                }
                b'#' => {
                    let start = self.pos;
                    self.advance(); // skip #
                    let comment_start = self.pos;
                    while self.pos < self.src.len() && self.src[self.pos] != b'\n' {
                        self.advance();
                    }
                    let text = String::from_utf8_lossy(&self.src[comment_start..self.pos]).trim().to_string();
                    tokens.push(self.make_span_at(Token::Comment(text), start, self.pos - start));
                }
                b'"' => tokens.push(self.lex_string()?),
                b':' => { tokens.push(self.make_span(Token::Colon, 1)); self.advance(); }
                b'|' => { tokens.push(self.make_span(Token::Pipe, 1)); self.advance(); }
                b'=' => {
                    if self.pos + 1 < self.src.len() && self.src[self.pos + 1] == b'>' {
                        tokens.push(self.make_span(Token::Arrow, 2));
                        self.advance();
                        self.advance();
                    } else {
                        tokens.push(self.make_span(Token::Assign, 1));
                        self.advance();
                    }
                }
                b'[' => { tokens.push(self.make_span(Token::LBracket, 1)); self.advance(); }
                b']' => { tokens.push(self.make_span(Token::RBracket, 1)); self.advance(); }
                b'{' => { tokens.push(self.make_span(Token::LBrace, 1)); self.advance(); }
                b'}' => { tokens.push(self.make_span(Token::RBrace, 1)); self.advance(); }
                b'(' => { tokens.push(self.make_span(Token::LParen, 1)); self.advance(); }
                b')' => { tokens.push(self.make_span(Token::RParen, 1)); self.advance(); }
                b',' => { tokens.push(self.make_span(Token::Comma, 1)); self.advance(); }
                b'.' => { tokens.push(self.make_span(Token::Dot, 1)); self.advance(); }
                b'?' => { tokens.push(self.make_span(Token::Question, 1)); self.advance(); }
                b'@' => { tokens.push(self.make_span(Token::At, 1)); self.advance(); }
                _ if ch.is_ascii_digit() => tokens.push(self.lex_number()?),
                _ if is_ident_start(ch) => tokens.push(self.lex_ident_or_keyword()),
                _ => {
                    return Err(LexError {
                        message: format!("unexpected character: '{}'", ch as char),
                        line: self.line,
                        col: self.col,
                    });
                }
            }
        }

        tokens.push(Spanned {
            token: Token::Eof,
            span: Span { line: self.line, col: self.col, offset: self.pos, len: 0 },
        });

        Ok(tokens)
    }

    fn skip_whitespace(&mut self) {
        while self.pos < self.src.len() {
            match self.src[self.pos] {
                b' ' | b'\t' | b'\r' => self.advance(),
                _ => break,
            }
        }
    }

    fn advance(&mut self) {
        self.pos += 1;
        self.col += 1;
    }

    fn make_span(&self, token: Token, len: usize) -> Spanned {
        Spanned {
            token,
            span: Span { line: self.line, col: self.col, offset: self.pos, len },
        }
    }

    fn make_span_at(&self, token: Token, offset: usize, len: usize) -> Spanned {
        Spanned {
            token,
            span: Span { line: self.line, col: self.col, offset, len },
        }
    }

    fn lex_string(&mut self) -> Result<Spanned, LexError> {
        let start = self.pos;
        let start_line = self.line;
        let start_col = self.col;
        self.advance(); // skip opening "

        let mut value = String::new();
        while self.pos < self.src.len() && self.src[self.pos] != b'"' {
            if self.src[self.pos] == b'\\' && self.pos + 1 < self.src.len() {
                self.advance();
                match self.src[self.pos] {
                    b'n' => value.push('\n'),
                    b't' => value.push('\t'),
                    b'\\' => value.push('\\'),
                    b'"' => value.push('"'),
                    _ => value.push(self.src[self.pos] as char),
                }
            } else {
                value.push(self.src[self.pos] as char);
            }
            self.advance();
        }

        if self.pos >= self.src.len() {
            return Err(LexError {
                message: "unterminated string literal".into(),
                line: start_line,
                col: start_col,
            });
        }

        self.advance(); // skip closing "
        Ok(Spanned {
            token: Token::StrLit(value),
            span: Span { line: start_line, col: start_col, offset: start, len: self.pos - start },
        })
    }

    fn lex_number(&mut self) -> Result<Spanned, LexError> {
        let start = self.pos;
        let start_col = self.col;
        let mut is_float = false;

        while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() {
            self.advance();
        }

        if self.pos < self.src.len() && self.src[self.pos] == b'.'
            && self.pos + 1 < self.src.len() && self.src[self.pos + 1].is_ascii_digit()
        {
            is_float = true;
            self.advance(); // skip .
            while self.pos < self.src.len() && self.src[self.pos].is_ascii_digit() {
                self.advance();
            }
        }

        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        let token = if is_float {
            Token::FloatLit(text.parse::<f64>().map_err(|_| LexError {
                message: format!("invalid float: {text}"),
                line: self.line,
                col: start_col,
            })?)
        } else {
            Token::IntLit(text.parse::<i64>().map_err(|_| LexError {
                message: format!("invalid integer: {text}"),
                line: self.line,
                col: start_col,
            })?)
        };

        Ok(Spanned {
            token,
            span: Span { line: self.line, col: start_col, offset: start, len: self.pos - start },
        })
    }

    fn lex_ident_or_keyword(&mut self) -> Spanned {
        let start = self.pos;
        let start_col = self.col;

        while self.pos < self.src.len() && is_ident_continue(self.src[self.pos]) {
            self.advance();
        }

        let text = std::str::from_utf8(&self.src[start..self.pos]).unwrap();
        let token = if text == "_" {
            Token::Underscore
        } else {
            Token::keyword_from_str(text)
                .unwrap_or_else(|| Token::Ident(text.to_string()))
        };

        Spanned {
            token,
            span: Span { line: self.line, col: start_col, offset: start, len: self.pos - start },
        }
    }
}

fn is_ident_start(ch: u8) -> bool {
    ch.is_ascii_alphabetic() || ch == b'_'
}

fn is_ident_continue(ch: u8) -> bool {
    ch.is_ascii_alphanumeric() || ch == b'_'
}

#[derive(Debug)]
pub struct LexError {
    pub message: String,
    pub line: usize,
    pub col: usize,
}

impl std::fmt::Display for LexError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "lex error at {}:{}: {}", self.line, self.col, self.message)
    }
}

impl std::error::Error for LexError {}

#[cfg(test)]
mod tests {
    use super::*;
    use token::Token;

    fn lex(input: &str) -> Vec<Token> {
        Lexer::new(input)
            .tokenize()
            .unwrap()
            .into_iter()
            .map(|s| s.token)
            .filter(|t| !matches!(t, Token::Eof | Token::Newline))
            .collect()
    }

    #[test]
    fn keywords() {
        assert_eq!(lex("fn in out inv do"), vec![
            Token::Fn, Token::In, Token::Out, Token::Inv, Token::Do,
        ]);
    }

    #[test]
    fn req_ens_keywords() {
        assert_eq!(lex("req ens"), vec![Token::Req, Token::Ens]);
    }

    #[test]
    fn req_ens_in_function_context() {
        let tokens = lex("fn test\n  in x: int\n  req gt x 0\n  ens gt result 0");
        assert!(tokens.contains(&Token::Req));
        assert!(tokens.contains(&Token::Ens));
    }

    #[test]
    fn operators() {
        assert_eq!(lex("add sub mul eq gt"), vec![
            Token::Add, Token::Sub, Token::Mul, Token::Eq, Token::Gt,
        ]);
    }

    #[test]
    fn literals() {
        assert_eq!(lex("42 3.14 \"hello\""), vec![
            Token::IntLit(42),
            Token::FloatLit(3.14),
            Token::StrLit("hello".into()),
        ]);
    }

    #[test]
    fn punctuation() {
        assert_eq!(lex(": | = [ ] ."), vec![
            Token::Colon, Token::Pipe, Token::Assign,
            Token::LBracket, Token::RBracket, Token::Dot,
        ]);
    }

    #[test]
    fn identifiers() {
        assert_eq!(lex("foo bar_baz x1"), vec![
            Token::Ident("foo".into()),
            Token::Ident("bar_baz".into()),
            Token::Ident("x1".into()),
        ]);
    }

    #[test]
    fn function_declaration() {
        let tokens = lex("fn clamp\n  in val: int lo: int hi: int\n  out result: int\n  do max lo min hi val");
        assert!(tokens.starts_with(&[Token::Fn, Token::Ident("clamp".into())]));
    }

    #[test]
    fn comments() {
        let tokens = lex("# this is a comment\nfn test");
        assert_eq!(tokens, vec![
            Token::Comment("this is a comment".into()),
            Token::Fn,
            Token::Ident("test".into()),
        ]);
    }

    #[test]
    fn pipe_chain() {
        assert_eq!(lex("filter nums gt 0 | reduce add"), vec![
            Token::Ident("filter".into()),
            Token::Ident("nums".into()),
            Token::Gt,
            Token::IntLit(0),
            Token::Pipe,
            Token::Ident("reduce".into()),
            Token::Add,
        ]);
    }

    #[test]
    fn enum_match_keywords() {
        assert_eq!(lex("enum match"), vec![Token::Enum, Token::Match]);
    }

    #[test]
    fn spawn_await_send_recv_tokens() {
        assert_eq!(lex("spawn await send recv async"), vec![
            Token::Spawn, Token::AwaitKw, Token::Send, Token::Recv, Token::Async,
        ]);
    }

    #[test]
    fn arrow_token() {
        assert_eq!(lex("=>"), vec![Token::Arrow]);
    }

    #[test]
    fn underscore_token() {
        assert_eq!(lex("_"), vec![Token::Underscore]);
    }

    #[test]
    fn underscore_in_ident_is_ident() {
        assert_eq!(lex("foo_bar"), vec![Token::Ident("foo_bar".into())]);
    }
}
