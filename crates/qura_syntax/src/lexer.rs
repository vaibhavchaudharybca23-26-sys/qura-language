use crate::span::Span;
use crate::token::{Token, TokenKind};

pub struct Lexer<'a> {
    src: &'a str,
    bytes: &'a [u8],
    cursor: usize,
}

impl<'a> Lexer<'a> {
    pub fn new(src: &'a str) -> Self {
        Self {
            src,
            bytes: src.as_bytes(),
            cursor: 0,
        }
    }

    #[inline]
    fn is_eof(&self) -> bool {
        self.cursor >= self.bytes.len()
    }

    #[inline]
    fn peek(&self) -> Option<u8> {
        self.bytes.get(self.cursor).copied()
    }

    #[inline]
    fn peek_next(&self) -> Option<u8> {
        self.bytes.get(self.cursor + 1).copied()
    }

    #[inline]
    fn bump(&mut self) -> Option<u8> {
        let b = self.bytes.get(self.cursor).copied()?;
        self.cursor += 1;
        Some(b)
    }

    fn skip_whitespace_and_comments(&mut self) -> Option<Token> {
        while !self.is_eof() {
            match self.peek()? {
                b' ' | b'\t' | b'\r' | b'\n' => {
                    self.bump();
                }
                b'/' => {
                    if self.peek_next() == Some(b'/') {
                        // Single-line comment: skip until newline or EOF
                        self.bump();
                        self.bump();
                        while let Some(b) = self.peek() {
                            self.bump();
                            if b == b'\n' {
                                break;
                            }
                        }
                    } else if self.peek_next() == Some(b'*') {
                        // Multi-line block comment with support for nesting
                        let start = self.cursor as u32;
                        self.bump(); // '/'
                        self.bump(); // '*'
                        let mut depth = 1;

                        while !self.is_eof() && depth > 0 {
                            if self.peek() == Some(b'/') && self.peek_next() == Some(b'*') {
                                self.bump();
                                self.bump();
                                depth += 1;
                            } else if self.peek() == Some(b'*') && self.peek_next() == Some(b'/') {
                                self.bump();
                                self.bump();
                                depth -= 1;
                            } else {
                                self.bump();
                            }
                        }

                        if depth > 0 {
                            let end = self.cursor as u32;
                            return Some(Token::new(
                                TokenKind::Error("Unterminated block comment".to_string()),
                                Span::new(start, end),
                            ));
                        }
                    } else {
                        break;
                    }
                }
                _ => break,
            }
        }
        None
    }

    pub fn next_token(&mut self) -> Token {
        if let Some(err_token) = self.skip_whitespace_and_comments() {
            return err_token;
        }

        let start = self.cursor as u32;
        let byte = match self.bump() {
            Some(b) => b,
            None => {
                return Token::new(TokenKind::Eof, Span::new(start, start));
            }
        };

        let kind = match byte {
            b'(' => TokenKind::OpenParen,
            b')' => TokenKind::CloseParen,
            b'{' => TokenKind::OpenBrace,
            b'}' => TokenKind::CloseBrace,
            b'[' => TokenKind::OpenBracket,
            b']' => TokenKind::CloseBracket,
            b';' => TokenKind::Semicolon,
            b':' => TokenKind::Colon,
            b',' => TokenKind::Comma,
            b'+' => TokenKind::Plus,
            b'*' => TokenKind::Star,
            b'/' => TokenKind::Slash,
            b'%' => TokenKind::Percent,

            b'-' => {
                if self.peek() == Some(b'>') {
                    self.bump();
                    TokenKind::Arrow
                } else {
                    TokenKind::Minus
                }
            }

            b'=' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    TokenKind::EqEq
                } else {
                    TokenKind::Eq
                }
            }

            b'!' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    TokenKind::BangEq
                } else {
                    TokenKind::Bang
                }
            }

            b'<' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    TokenKind::LtEq
                } else {
                    TokenKind::Lt
                }
            }

            b'>' => {
                if self.peek() == Some(b'=') {
                    self.bump();
                    TokenKind::GtEq
                } else {
                    TokenKind::Gt
                }
            }

            b'&' => {
                if self.peek() == Some(b'&') {
                    self.bump();
                    TokenKind::AmpAmp
                } else {
                    TokenKind::Error("Unexpected single '&', expected '&&'".to_string())
                }
            }

            b'|' => {
                if self.peek() == Some(b'|') {
                    self.bump();
                    TokenKind::PipePipe
                } else {
                    TokenKind::Error("Unexpected single '|', expected '||'".to_string())
                }
            }

            // Number literals
            b'0'..=b'9' => self.lex_number(start, byte),

            // Identifiers or keywords
            b'a'..=b'z' | b'A'..=b'Z' | b'_' => self.lex_identifier(start),

            other => TokenKind::Error(format!("Unexpected character: '{}'", other as char)),
        };

        let end = self.cursor as u32;
        Token::new(kind, Span::new(start, end))
    }

    fn lex_number(&mut self, start: u32, first_byte: u8) -> TokenKind {
        // Hexadecimal
        if first_byte == b'0' && (self.peek() == Some(b'x') || self.peek() == Some(b'X')) {
            self.bump();
            let hex_start = self.cursor;
            while let Some(b) = self.peek() {
                if b.is_ascii_hexdigit() || b == b'_' {
                    self.bump();
                } else {
                    break;
                }
            }
            let raw: String = self.src[hex_start..self.cursor]
                .chars()
                .filter(|c| *c != '_')
                .collect();
            return match i64::from_str_radix(&raw, 16) {
                Ok(val) => TokenKind::Int(val),
                Err(_) => TokenKind::Error(format!("Invalid hexadecimal integer: 0x{}", raw)),
            };
        }

        // Decimal or Float
        while let Some(b) = self.peek() {
            if b.is_ascii_digit() || b == b'_' {
                self.bump();
            } else {
                break;
            }
        }

        // Check for decimal point for float
        if self.peek() == Some(b'.') && self.peek_next().map_or(false, |b| b.is_ascii_digit()) {
            self.bump(); // consume '.'
            while let Some(b) = self.peek() {
                if b.is_ascii_digit() || b == b'_' {
                    self.bump();
                } else {
                    break;
                }
            }
            let raw: String = self.src[start as usize..self.cursor]
                .chars()
                .filter(|c| *c != '_')
                .collect();
            return match raw.parse::<f64>() {
                Ok(val) => TokenKind::Float(val),
                Err(_) => TokenKind::Error(format!("Invalid float literal: {}", raw)),
            };
        }

        let raw: String = self.src[start as usize..self.cursor]
            .chars()
            .filter(|c| *c != '_')
            .collect();
        match raw.parse::<i64>() {
            Ok(val) => TokenKind::Int(val),
            Err(_) => TokenKind::Error(format!("Integer literal out of range: {}", raw)),
        }
    }

    fn lex_identifier(&mut self, start: u32) -> TokenKind {
        while let Some(b) = self.peek() {
            if b.is_ascii_alphanumeric() || b == b'_' {
                self.bump();
            } else {
                break;
            }
        }

        let ident = &self.src[start as usize..self.cursor];
        match ident {
            "fn" => TokenKind::Fn,
            "let" => TokenKind::Let,
            "mut" => TokenKind::Mut,
            "if" => TokenKind::If,
            "else" => TokenKind::Else,
            "while" => TokenKind::While,
            "return" => TokenKind::Return,
            "true" => TokenKind::True,
            "false" => TokenKind::False,

            // Primitive types
            "i32" => TokenKind::TypeI32,
            "i64" => TokenKind::TypeI64,
            "u32" => TokenKind::TypeU32,
            "u64" => TokenKind::TypeU64,
            "f64" => TokenKind::TypeF64,
            "bool" => TokenKind::TypeBool,

            _ => TokenKind::Ident(ident.to_string()),
        }
    }

    pub fn tokenize_all(&mut self) -> Vec<Token> {
        let mut tokens = Vec::new();
        loop {
            let tok = self.next_token();
            let is_eof = tok.kind == TokenKind::Eof;
            tokens.push(tok);
            if is_eof {
                break;
            }
        }
        tokens
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_lex_keywords_and_types() {
        let src = "fn let mut if else while return true false i32 i64 bool";
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize_all();
        let kinds: Vec<_> = tokens.into_iter().map(|t| t.kind).collect();
        assert_eq!(
            kinds,
            vec![
                TokenKind::Fn,
                TokenKind::Let,
                TokenKind::Mut,
                TokenKind::If,
                TokenKind::Else,
                TokenKind::While,
                TokenKind::Return,
                TokenKind::True,
                TokenKind::False,
                TokenKind::TypeI32,
                TokenKind::TypeI64,
                TokenKind::TypeBool,
                TokenKind::Eof,
            ]
        );
    }

    #[test]
    fn test_lex_operators_and_comments() {
        let src = "
            // Single line comment
            let x = 42 + 0x10; /* block comment */
            let y = 3.14;
            if x <= y && !false || true { x != y; }
        ";
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize_all();
        assert!(tokens.iter().all(|t| !matches!(t.kind, TokenKind::Error(_))));
    }

    #[test]
    fn test_lex_unterminated_block_comment() {
        let src = "/* unclosed comment";
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize_all();
        assert!(tokens.iter().any(|t| matches!(t.kind, TokenKind::Error(_))));
    }
}
