use qura_diagnostics::{Diagnostic, DiagnosticLabel};
use qura_syntax::ast::*;
use qura_syntax::lexer::Lexer;
use qura_syntax::token::{Token, TokenKind};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Precedence {
    None = 0,
    Or = 1,          // ||
    And = 2,         // &&
    Equality = 3,    // == !=
    Comparison = 4,  // < <= > >=
    Term = 5,        // + -
    Factor = 6,      // * / %
    Unary = 7,       // ! -
    Call = 8,        // ()
}

impl Precedence {
    pub fn of(token: &TokenKind) -> Self {
        match token {
            TokenKind::PipePipe => Precedence::Or,
            TokenKind::AmpAmp => Precedence::And,
            TokenKind::EqEq | TokenKind::BangEq => Precedence::Equality,
            TokenKind::Lt | TokenKind::LtEq | TokenKind::Gt | TokenKind::GtEq => {
                Precedence::Comparison
            }
            TokenKind::Plus | TokenKind::Minus => Precedence::Term,
            TokenKind::Star | TokenKind::Slash | TokenKind::Percent => Precedence::Factor,
            TokenKind::OpenParen => Precedence::Call,
            _ => Precedence::None,
        }
    }
}

pub struct Parser<'a> {
    tokens: Vec<Token>,
    cursor: usize,
    diagnostics: Vec<Diagnostic>,
    _src: &'a str,
}

impl<'a> Parser<'a> {
    pub fn new(src: &'a str) -> Self {
        let mut lexer = Lexer::new(src);
        let tokens = lexer.tokenize_all();
        Self {
            tokens,
            cursor: 0,
            diagnostics: Vec::new(),
            _src: src,
        }
    }

    pub fn diagnostics(&self) -> &[Diagnostic] {
        &self.diagnostics
    }

    pub fn into_diagnostics(self) -> Vec<Diagnostic> {
        self.diagnostics
    }

    #[inline]
    fn current(&self) -> &Token {
        self.tokens.get(self.cursor).unwrap_or_else(|| {
            self.tokens
                .last()
                .expect("token list must at least contain EOF")
        })
    }

    #[inline]
    fn previous(&self) -> &Token {
        assert!(self.cursor > 0, "previous() called at start of tokens");
        &self.tokens[self.cursor - 1]
    }

    #[inline]
    fn is_at_end(&self) -> bool {
        self.current().kind == TokenKind::Eof
    }

    fn bump(&mut self) -> &Token {
        if !self.is_at_end() {
            self.cursor += 1;
        }
        self.previous()
    }

    fn check(&self, kind: &TokenKind) -> bool {
        if self.is_at_end() {
            return false;
        }
        std::mem::discriminant(&self.current().kind) == std::mem::discriminant(kind)
    }

    fn matches(&mut self, kind: &TokenKind) -> bool {
        if self.check(kind) {
            self.bump();
            true
        } else {
            false
        }
    }

    fn expect(&mut self, kind: &TokenKind, err_msg: &str, code: &'static str) -> Result<Token, ()> {
        if self.check(kind) {
            Ok(self.bump().clone())
        } else {
            let tok = self.current();
            let diag = Diagnostic::error(
                format!("{}: expected {}, found {}", err_msg, kind.description(), tok.kind.description()),
                tok.span,
            )
            .with_code(code)
            .with_label(DiagnosticLabel::primary(tok.span));
            self.diagnostics.push(diag);
            Err(())
        }
    }

    fn synchronize(&mut self) {
        self.bump();

        while !self.is_at_end() {
            if self.previous().kind == TokenKind::Semicolon {
                return;
            }

            match self.current().kind {
                TokenKind::Fn
                | TokenKind::Let
                | TokenKind::While
                | TokenKind::If
                | TokenKind::Return
                | TokenKind::CloseBrace => return,
                _ => {
                    self.bump();
                }
            }
        }
    }

    // Top level entry point
    pub fn parse_program(&mut self) -> Result<Program, Vec<Diagnostic>> {
        let mut items = Vec::new();
        let start_span = self.current().span;

        // Check for lexer-level errors first
        for tok in &self.tokens {
            if let TokenKind::Error(ref msg) = tok.kind {
                self.diagnostics.push(
                    Diagnostic::error(msg.clone(), tok.span)
                        .with_code("E0001")
                        .with_label(DiagnosticLabel::primary(tok.span)),
                );
            }
        }

        while !self.is_at_end() {
            match self.parse_item() {
                Ok(item) => items.push(item),
                Err(_) => {
                    self.synchronize();
                }
            }
        }

        let end_span = self.current().span;
        let program = Program {
            items,
            span: start_span.merge(end_span),
        };

        if self.diagnostics.is_empty() {
            Ok(program)
        } else {
            Err(self.diagnostics.clone())
        }
    }

    fn parse_item(&mut self) -> Result<Item, ()> {
        if self.check(&TokenKind::Fn) {
            let func = self.parse_function()?;
            Ok(Item::Function(func))
        } else {
            let tok = self.current();
            let diag = Diagnostic::error(
                format!("expected top-level item declaration ('fn'), found {}", tok.kind.description()),
                tok.span,
            )
            .with_code("E0002")
            .with_label(DiagnosticLabel::primary(tok.span));
            self.diagnostics.push(diag);
            Err(())
        }
    }

    // fn name(p1: T1, mut p2: T2) -> RetType { ... }
    fn parse_function(&mut self) -> Result<FunctionDecl, ()> {
        let fn_tok = self.expect(&TokenKind::Fn, "expected 'fn'", "E0003")?;
        let name_tok = match self.current().kind {
            TokenKind::Ident(_) => self.bump().clone(),
            _ => {
                let cur = self.current();
                let diag = Diagnostic::error("expected function name", cur.span)
                    .with_code("E0004")
                    .with_label(DiagnosticLabel::primary(cur.span));
                self.diagnostics.push(diag);
                return Err(());
            }
        };

        let name = Ident::new(name_tok.kind.to_string(), name_tok.span);

        self.expect(&TokenKind::OpenParen, "expected '(' after function name", "E0005")?;

        let mut params = Vec::new();
        if !self.check(&TokenKind::CloseParen) {
            loop {
                let is_mut = self.matches(&TokenKind::Mut);
                let p_name_tok = match self.current().kind {
                    TokenKind::Ident(_) => self.bump().clone(),
                    _ => {
                        let cur = self.current();
                        let diag = Diagnostic::error("expected parameter name", cur.span)
                            .with_code("E0006")
                            .with_label(DiagnosticLabel::primary(cur.span));
                        self.diagnostics.push(diag);
                        return Err(());
                    }
                };
                let p_name = Ident::new(p_name_tok.kind.to_string(), p_name_tok.span);

                self.expect(&TokenKind::Colon, "expected ':' after parameter name", "E0007")?;
                let ty = self.parse_type()?;
                let p_span = if is_mut {
                    self.previous().span.merge(ty.span())
                } else {
                    p_name.span.merge(ty.span())
                };

                params.push(Param {
                    name: p_name,
                    is_mut,
                    ty,
                    span: p_span,
                });

                if !self.matches(&TokenKind::Comma) {
                    break;
                }
            }
        }

        self.expect(&TokenKind::CloseParen, "expected ')' after parameters", "E0008")?;

        let return_type = if self.matches(&TokenKind::Arrow) {
            Some(self.parse_type()?)
        } else {
            None
        };

        let body = self.parse_block()?;
        let total_span = fn_tok.span.merge(body.span);

        Ok(FunctionDecl {
            name,
            params,
            return_type,
            body,
            span: total_span,
        })
    }

    fn parse_type(&mut self) -> Result<TypeExpr, ()> {
        let cur = self.current().clone();
        match cur.kind {
            TokenKind::TypeI32 => {
                self.bump();
                Ok(TypeExpr::Named(Ident::new("i32", cur.span)))
            }
            TokenKind::TypeI64 => {
                self.bump();
                Ok(TypeExpr::Named(Ident::new("i64", cur.span)))
            }
            TokenKind::TypeU32 => {
                self.bump();
                Ok(TypeExpr::Named(Ident::new("u32", cur.span)))
            }
            TokenKind::TypeU64 => {
                self.bump();
                Ok(TypeExpr::Named(Ident::new("u64", cur.span)))
            }
            TokenKind::TypeF64 => {
                self.bump();
                Ok(TypeExpr::Named(Ident::new("f64", cur.span)))
            }
            TokenKind::TypeBool => {
                self.bump();
                Ok(TypeExpr::Named(Ident::new("bool", cur.span)))
            }
            TokenKind::Ident(ref name) => {
                let name = name.clone();
                self.bump();
                Ok(TypeExpr::Named(Ident::new(name, cur.span)))
            }
            TokenKind::OpenParen => {
                self.bump();
                let close = self.expect(&TokenKind::CloseParen, "expected ')' for unit type '()'", "E0009")?;
                Ok(TypeExpr::Unit(cur.span.merge(close.span)))
            }
            _ => {
                let diag = Diagnostic::error(
                    format!("expected type name, found {}", cur.kind.description()),
                    cur.span,
                )
                .with_code("E0010")
                .with_label(DiagnosticLabel::primary(cur.span));
                self.diagnostics.push(diag);
                Err(())
            }
        }
    }

    pub fn parse_block(&mut self) -> Result<Block, ()> {
        let open_tok = self.expect(&TokenKind::OpenBrace, "expected '{' to start block", "E0011")?;
        let mut stmts = Vec::new();
        let mut tail_expr = None;

        while !self.check(&TokenKind::CloseBrace) && !self.is_at_end() {
            // Check if this could be a statement or tail expression
            if self.check(&TokenKind::Let)
                || self.check(&TokenKind::While)
                || self.check(&TokenKind::Return)
            {
                let stmt = self.parse_stmt()?;
                stmts.push(stmt);
            } else if self.check(&TokenKind::If) {
                let if_expr = self.parse_if_expr()?;
                let has_semi = self.matches(&TokenKind::Semicolon);
                if !has_semi && self.check(&TokenKind::CloseBrace) {
                    tail_expr = Some(Box::new(if_expr));
                    break;
                } else {
                    let span = if_expr.span();
                    stmts.push(Stmt::Expr {
                        expr: if_expr,
                        has_semicolon: has_semi,
                        span,
                    });
                }
            } else {
                // Could be an assignment statement, an expression statement, or tail expression
                // Lookahead to check if identifier followed by '='
                if let TokenKind::Ident(_) = self.current().kind {
                    if self.cursor + 1 < self.tokens.len()
                        && self.tokens[self.cursor + 1].kind == TokenKind::Eq
                    {
                        let stmt = self.parse_assign_stmt()?;
                        stmts.push(stmt);
                        continue;
                    }
                }

                // Expression
                let expr = self.parse_expr(Precedence::None)?;
                if self.matches(&TokenKind::Semicolon) {
                    let span = expr.span().merge(self.previous().span);
                    stmts.push(Stmt::Expr {
                        expr,
                        has_semicolon: true,
                        span,
                    });
                } else if self.check(&TokenKind::CloseBrace) {
                    // Tail expression of block
                    tail_expr = Some(Box::new(expr));
                    break;
                } else {
                    // Missing semicolon
                    let cur = self.current();
                    let diag = Diagnostic::error(
                        "expected ';' after expression",
                        cur.span,
                    )
                    .with_code("E0012")
                    .with_label(DiagnosticLabel::primary(cur.span))
                    .with_note("statements must end with ';' or be the final return expression of a block");
                    self.diagnostics.push(diag);
                    self.synchronize();
                }
            }
        }

        let close_tok = self.expect(&TokenKind::CloseBrace, "expected '}' to close block", "E0013")?;
        Ok(Block {
            stmts,
            tail_expr,
            span: open_tok.span.merge(close_tok.span),
        })
    }

    fn parse_stmt(&mut self) -> Result<Stmt, ()> {
        if self.check(&TokenKind::Let) {
            self.parse_let_stmt()
        } else if self.check(&TokenKind::While) {
            self.parse_while_stmt()
        } else if self.check(&TokenKind::Return) {
            self.parse_return_stmt()
        } else {
            let expr = self.parse_expr(Precedence::None)?;
            let semi = self.expect(&TokenKind::Semicolon, "expected ';' after statement", "E0014")?;
            let span = expr.span().merge(semi.span);
            Ok(Stmt::Expr {
                expr,
                has_semicolon: true,
                span,
            })
        }
    }

    // let [mut] name [: type] = expr;
    fn parse_let_stmt(&mut self) -> Result<Stmt, ()> {
        let let_tok = self.expect(&TokenKind::Let, "expected 'let'", "E0015")?;
        let is_mut = self.matches(&TokenKind::Mut);

        let name_tok = match self.current().kind {
            TokenKind::Ident(_) => self.bump().clone(),
            _ => {
                let cur = self.current();
                let diag = Diagnostic::error("expected variable name in let binding", cur.span)
                    .with_code("E0016")
                    .with_label(DiagnosticLabel::primary(cur.span));
                self.diagnostics.push(diag);
                return Err(());
            }
        };
        let name = Ident::new(name_tok.kind.to_string(), name_tok.span);

        let ty = if self.matches(&TokenKind::Colon) {
            Some(self.parse_type()?)
        } else {
            None
        };

        self.expect(&TokenKind::Eq, "expected '=' in let binding", "E0017")?;
        let init = self.parse_expr(Precedence::None)?;
        let semi = self.expect(&TokenKind::Semicolon, "expected ';' after let binding", "E0018")?;

        let total_span = let_tok.span.merge(semi.span);
        Ok(Stmt::Let {
            name,
            is_mut,
            ty,
            init,
            span: total_span,
        })
    }

    // target = expr;
    fn parse_assign_stmt(&mut self) -> Result<Stmt, ()> {
        let name_tok = self.bump().clone();
        let target = Ident::new(name_tok.kind.to_string(), name_tok.span);

        self.expect(&TokenKind::Eq, "expected '=' in assignment", "E0019")?;
        let value = self.parse_expr(Precedence::None)?;
        let semi = self.expect(&TokenKind::Semicolon, "expected ';' after assignment", "E0020")?;

        let span = target.span.merge(semi.span);
        Ok(Stmt::Assign {
            target,
            value,
            span,
        })
    }

    // while cond { ... }
    fn parse_while_stmt(&mut self) -> Result<Stmt, ()> {
        let while_tok = self.expect(&TokenKind::While, "expected 'while'", "E0021")?;
        let condition = self.parse_expr(Precedence::None)?;
        let body = self.parse_block()?;
        let span = while_tok.span.merge(body.span);
        Ok(Stmt::While {
            condition,
            body,
            span,
        })
    }

    // return [expr];
    fn parse_return_stmt(&mut self) -> Result<Stmt, ()> {
        let ret_tok = self.expect(&TokenKind::Return, "expected 'return'", "E0022")?;
        let value = if self.check(&TokenKind::Semicolon) {
            None
        } else {
            Some(self.parse_expr(Precedence::None)?)
        };
        let semi = self.expect(&TokenKind::Semicolon, "expected ';' after return", "E0023")?;
        let span = ret_tok.span.merge(semi.span);
        Ok(Stmt::Return { value, span })
    }

    // Pratt parser for expressions
    pub fn parse_expr(&mut self, min_prec: Precedence) -> Result<Expr, ()> {
        let mut left = self.parse_prefix()?;

        while !self.is_at_end() {
            let next_prec = Precedence::of(&self.current().kind);
            if next_prec <= min_prec {
                break;
            }

            left = self.parse_infix(left, next_prec)?;
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<Expr, ()> {
        let tok = self.current().clone();
        match tok.kind {
            TokenKind::Int(v) => {
                self.bump();
                Ok(Expr::Literal(Literal::Int(v, tok.span)))
            }
            TokenKind::Float(v) => {
                self.bump();
                Ok(Expr::Literal(Literal::Float(v, tok.span)))
            }
            TokenKind::True => {
                self.bump();
                Ok(Expr::Literal(Literal::Bool(true, tok.span)))
            }
            TokenKind::False => {
                self.bump();
                Ok(Expr::Literal(Literal::Bool(false, tok.span)))
            }
            TokenKind::String(ref s) => {
                self.bump();
                Ok(Expr::Literal(Literal::String(s.clone(), tok.span)))
            }
            TokenKind::Ident(ref name) => {
                self.bump();
                Ok(Expr::Variable(Ident::new(name, tok.span)))
            }
            TokenKind::Minus => {
                self.bump();
                let operand = self.parse_expr(Precedence::Unary)?;
                let span = tok.span.merge(operand.span());
                Ok(Expr::Unary {
                    op: UnaryOp::Neg,
                    operand: Box::new(operand),
                    span,
                })
            }
            TokenKind::Bang => {
                self.bump();
                let operand = self.parse_expr(Precedence::Unary)?;
                let span = tok.span.merge(operand.span());
                Ok(Expr::Unary {
                    op: UnaryOp::Not,
                    operand: Box::new(operand),
                    span,
                })
            }
            TokenKind::OpenParen => {
                self.bump();
                let inner = self.parse_expr(Precedence::None)?;
                let close = self.expect(&TokenKind::CloseParen, "expected ')' after expression", "E0024")?;
                let span = tok.span.merge(close.span);
                Ok(Expr::Grouping {
                    expr: Box::new(inner),
                    span,
                })
            }
            TokenKind::If => self.parse_if_expr(),
            TokenKind::OpenBrace => {
                let block = self.parse_block()?;
                Ok(Expr::Block(block))
            }
            _ => {
                let diag = Diagnostic::error(
                    format!("expected expression, found {}", tok.kind.description()),
                    tok.span,
                )
                .with_code("E0025")
                .with_label(DiagnosticLabel::primary(tok.span));
                self.diagnostics.push(diag);
                Err(())
            }
        }
    }

    fn parse_infix(&mut self, left: Expr, prec: Precedence) -> Result<Expr, ()> {
        let op_tok = self.bump().clone();

        if op_tok.kind == TokenKind::OpenParen {
            // Function call: left(arg1, arg2)
            let callee_ident = match left {
                Expr::Variable(ident) => ident,
                other => {
                    let diag = Diagnostic::error("callee must be an identifier in v0.1", other.span())
                        .with_code("E0026");
                    self.diagnostics.push(diag);
                    return Err(());
                }
            };

            let mut args = Vec::new();
            if !self.check(&TokenKind::CloseParen) {
                loop {
                    args.push(self.parse_expr(Precedence::None)?);
                    if !self.matches(&TokenKind::Comma) {
                        break;
                    }
                }
            }

            let close = self.expect(&TokenKind::CloseParen, "expected ')' after call arguments", "E0027")?;
            let span = callee_ident.span.merge(close.span);
            return Ok(Expr::Call {
                callee: callee_ident,
                args,
                span,
            });
        }

        let binary_op = match op_tok.kind {
            TokenKind::Plus => BinaryOp::Add,
            TokenKind::Minus => BinaryOp::Sub,
            TokenKind::Star => BinaryOp::Mul,
            TokenKind::Slash => BinaryOp::Div,
            TokenKind::Percent => BinaryOp::Rem,
            TokenKind::EqEq => BinaryOp::Eq,
            TokenKind::BangEq => BinaryOp::Ne,
            TokenKind::Lt => BinaryOp::Lt,
            TokenKind::LtEq => BinaryOp::Le,
            TokenKind::Gt => BinaryOp::Gt,
            TokenKind::GtEq => BinaryOp::Ge,
            TokenKind::AmpAmp => BinaryOp::And,
            TokenKind::PipePipe => BinaryOp::Or,
            _ => unreachable!("unhandled infix token: {:?}", op_tok.kind),
        };

        // Binary operators are left-associative in Qura v0.1
        let right = self.parse_expr(prec)?;
        let span = left.span().merge(right.span());

        Ok(Expr::Binary {
            op: binary_op,
            left: Box::new(left),
            right: Box::new(right),
            span,
        })
    }

    // if cond { ... } else { ... }
    fn parse_if_expr(&mut self) -> Result<Expr, ()> {
        let if_tok = self.expect(&TokenKind::If, "expected 'if'", "E0028")?;
        let condition = self.parse_expr(Precedence::None)?;
        let then_branch = self.parse_block()?;

        let (else_branch, end_span) = if self.matches(&TokenKind::Else) {
            if self.check(&TokenKind::If) {
                // else if -> nest inside a synthesized block
                let nested_if = self.parse_if_expr()?;
                let span = nested_if.span();
                let block = Block {
                    stmts: Vec::new(),
                    tail_expr: Some(Box::new(nested_if)),
                    span,
                };
                (Some(block), span)
            } else {
                let else_block = self.parse_block()?;
                let span = else_block.span;
                (Some(else_block), span)
            }
        } else {
            (None, then_branch.span)
        };

        let span = if_tok.span.merge(end_span);
        Ok(Expr::If {
            condition: Box::new(condition),
            then_branch,
            else_branch,
            span,
        })
    }
}
