use crate::span::Span;
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct Token {
    pub kind: TokenKind,
    pub span: Span,
}

impl Token {
    pub fn new(kind: TokenKind, span: Span) -> Self {
        Self { kind, span }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum TokenKind {
    // Keywords
    Fn,
    Let,
    Mut,
    If,
    Else,
    While,
    Return,
    True,
    False,

    // Types
    TypeI32,
    TypeI64,
    TypeU32,
    TypeU64,
    TypeF64,
    TypeBool,

    // Identifiers & Literals
    Ident(String),
    Int(i64),
    Float(f64),

    // Arithmetic Operators
    Plus,       // +
    Minus,      // -
    Star,       // *
    Slash,      // /
    Percent,    // %

    // Comparison Operators
    EqEq,       // ==
    BangEq,     // !=
    Lt,         // <
    LtEq,       // <=
    Gt,         // >
    GtEq,       // >=

    // Logical & Unary Operators
    Bang,       // !
    AmpAmp,     // &&
    PipePipe,   // ||

    // Assignment
    Eq,         // =

    // Delimiters & Punctuation
    OpenParen,    // (
    CloseParen,   // )
    OpenBrace,    // {
    CloseBrace,   // }
    OpenBracket,  // [
    CloseBracket, // ]
    Arrow,        // ->
    Colon,        // :
    Semicolon,    // ;
    Comma,        // ,

    // End-of-file & Lexical Errors
    Eof,
    Error(String),
}

impl TokenKind {
    pub fn description(&self) -> &'static str {
        match self {
            TokenKind::Fn => "'fn'",
            TokenKind::Let => "'let'",
            TokenKind::Mut => "'mut'",
            TokenKind::If => "'if'",
            TokenKind::Else => "'else'",
            TokenKind::While => "'while'",
            TokenKind::Return => "'return'",
            TokenKind::True => "'true'",
            TokenKind::False => "'false'",
            TokenKind::TypeI32 => "'i32'",
            TokenKind::TypeI64 => "'i64'",
            TokenKind::TypeU32 => "'u32'",
            TokenKind::TypeU64 => "'u64'",
            TokenKind::TypeF64 => "'f64'",
            TokenKind::TypeBool => "'bool'",
            TokenKind::Ident(_) => "identifier",
            TokenKind::Int(_) => "integer literal",
            TokenKind::Float(_) => "float literal",
            TokenKind::Plus => "'+'",
            TokenKind::Minus => "'-'",
            TokenKind::Star => "'*'",
            TokenKind::Slash => "'/'",
            TokenKind::Percent => "'%'",
            TokenKind::EqEq => "'=='",
            TokenKind::BangEq => "'!='",
            TokenKind::Lt => "'<'",
            TokenKind::LtEq => "'<='",
            TokenKind::Gt => "'>'",
            TokenKind::GtEq => "'>='",
            TokenKind::Bang => "'!'",
            TokenKind::AmpAmp => "'&&'",
            TokenKind::PipePipe => "'||'",
            TokenKind::Eq => "'='",
            TokenKind::OpenParen => "'('",
            TokenKind::CloseParen => "')'",
            TokenKind::OpenBrace => "'{'",
            TokenKind::CloseBrace => "'}'",
            TokenKind::OpenBracket => "'['",
            TokenKind::CloseBracket => "']'",
            TokenKind::Arrow => "'->'",
            TokenKind::Colon => "':'",
            TokenKind::Semicolon => "';'",
            TokenKind::Comma => "','",
            TokenKind::Eof => "end of file",
            TokenKind::Error(_) => "error token",
        }
    }
}

impl fmt::Display for TokenKind {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            TokenKind::Ident(s) => write!(f, "{}", s),
            TokenKind::Int(v) => write!(f, "{}", v),
            TokenKind::Float(v) => write!(f, "{}", v),
            TokenKind::Error(e) => write!(f, "Error({})", e),
            _ => write!(f, "{}", self.description()),
        }
    }
}
