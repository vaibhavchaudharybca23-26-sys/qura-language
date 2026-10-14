use crate::symbol::{FnSymbol, SymbolTable};
use crate::types::Type;
use qura_diagnostics::{Diagnostic, DiagnosticLabel};
use qura_syntax::ast::*;
use qura_syntax::span::Span;

pub struct TypeChecker {
    symbols: SymbolTable,
    diagnostics: Vec<Diagnostic>,
    current_return_type: Option<Type>,
}

impl TypeChecker {
    pub fn new() -> Self {
        Self {
            symbols: SymbolTable::new(),
            diagnostics: Vec::new(),
            current_return_type: None,
        }
    }

    pub fn check_program(mut self, program: &Program) -> Result<SymbolTable, Vec<Diagnostic>> {
        // Pass 1: Collect function signatures
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.register_function_signature(func);
                }
            }
        }

        // Pass 2: Type check function bodies
        for item in &program.items {
            match item {
                Item::Function(func) => {
                    self.check_function_body(func);
                }
            }
        }

        if self.diagnostics.is_empty() {
            Ok(self.symbols)
        } else {
            Err(self.diagnostics)
        }
    }

    fn register_function_signature(&mut self, func: &FunctionDecl) {
        let return_type = match &func.return_type {
            Some(ty_expr) => match Type::from_type_expr(ty_expr) {
                Ok(t) => t,
                Err(err_msg) => {
                    self.diagnostics.push(
                        Diagnostic::error(err_msg, ty_expr.span())
                            .with_code("E0100")
                            .with_label(DiagnosticLabel::primary(ty_expr.span())),
                    );
                    Type::Error
                }
            },
            None => Type::Unit,
        };

        let mut params = Vec::new();
        for p in &func.params {
            let param_type = match Type::from_type_expr(&p.ty) {
                Ok(t) => t,
                Err(err_msg) => {
                    self.diagnostics.push(
                        Diagnostic::error(err_msg, p.ty.span())
                            .with_code("E0100")
                            .with_label(DiagnosticLabel::primary(p.ty.span())),
                    );
                    Type::Error
                }
            };
            params.push((p.name.name.clone(), param_type, p.is_mut));
        }

        let fn_symbol = FnSymbol {
            name: func.name.name.clone(),
            params,
            return_type,
            span: func.span,
        };

        if let Err(diag) = self.symbols.define_function(fn_symbol) {
            self.diagnostics.push(diag);
        }
    }

    fn check_function_body(&mut self, func: &FunctionDecl) {
        let fn_sym = match self.symbols.lookup_function(&func.name.name) {
            Some(sym) => sym.clone(),
            None => return,
        };

        self.current_return_type = Some(fn_sym.return_type.clone());
        self.symbols.push_scope();

        // Register parameters in function scope
        for (param_name, param_ty, is_mut) in &fn_sym.params {
            if let Err(diag) = self.symbols.define_variable(
                param_name.clone(),
                param_ty.clone(),
                *is_mut,
                func.span,
            ) {
                self.diagnostics.push(diag);
            }
        }

        // Check body block
        let body_ty = self.check_block(&func.body, false);

        // If function has a non-unit return type, check if body ended with matching tail expr
        if fn_sym.return_type != Type::Unit && fn_sym.return_type != Type::Error {
            if func.body.tail_expr.is_some() && body_ty != fn_sym.return_type && body_ty != Type::Error {
                self.diagnostics.push(
                    Diagnostic::error(
                        format!(
                            "mismatched tail expression return type in function '{}': expected '{}', found '{}'",
                            func.name.name, fn_sym.return_type, body_ty
                        ),
                        func.body.span,
                    )
                    .with_code("E0106")
                    .with_label(DiagnosticLabel::primary(func.body.span)),
                );
            }
        }

        self.symbols.pop_scope();
        self.current_return_type = None;
    }

    fn check_block(&mut self, block: &Block, new_scope: bool) -> Type {
        if new_scope {
            self.symbols.push_scope();
        }

        for stmt in &block.stmts {
            self.check_stmt(stmt);
        }

        let tail_type = if let Some(ref tail_expr) = block.tail_expr {
            self.check_expr(tail_expr, None)
        } else {
            Type::Unit
        };

        if new_scope {
            self.symbols.pop_scope();
        }

        tail_type
    }

    fn check_stmt(&mut self, stmt: &Stmt) {
        match stmt {
            Stmt::Let {
                name,
                is_mut,
                ty,
                init,
                span,
            } => {
                let declared_ty = ty.as_ref().and_then(|t_expr| match Type::from_type_expr(t_expr) {
                    Ok(t) => Some(t),
                    Err(err_msg) => {
                        self.diagnostics.push(
                            Diagnostic::error(err_msg, t_expr.span())
                                .with_code("E0100")
                                .with_label(DiagnosticLabel::primary(t_expr.span())),
                        );
                        None
                    }
                });

                let init_ty = self.check_expr(init, declared_ty.as_ref());

                let final_ty = if let Some(expected) = declared_ty {
                    if init_ty != expected && init_ty != Type::Error {
                        self.diagnostics.push(
                            Diagnostic::error(
                                format!(
                                    "mismatched types in let binding for '{}': expected '{}', found '{}'",
                                    name.name, expected, init_ty
                                ),
                                init.span(),
                            )
                            .with_code("E0101")
                            .with_label(DiagnosticLabel::primary(init.span())),
                        );
                    }
                    expected
                } else {
                    // Type inference: use inferred type of init
                    init_ty
                };

                if let Err(diag) =
                    self.symbols
                        .define_variable(&name.name, final_ty, *is_mut, *span)
                {
                    self.diagnostics.push(diag);
                }
            }

            Stmt::Assign {
                target,
                value,
                span,
            } => {
                let target_sym = match self.symbols.lookup_variable(&target.name) {
                    Some(sym) => sym.clone(),
                    None => {
                        self.diagnostics.push(
                            Diagnostic::error(
                                format!("cannot find variable '{}' in this scope", target.name),
                                target.span,
                            )
                            .with_code("E0103")
                            .with_label(DiagnosticLabel::primary(target.span)),
                        );
                        return;
                    }
                };

                // Enforcement of let versus let mut
                if !target_sym.is_mut {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "cannot assign to immutable variable '{}', declared with 'let'",
                                target.name
                            ),
                            *span,
                        )
                        .with_code("E0102")
                        .with_label(DiagnosticLabel::primary(target.span))
                        .with_note(format!(
                            "variable '{}' is not mutable; declare with 'let mut {}' to make it mutable",
                            target.name, target.name
                        )),
                    );
                }

                let value_ty = self.check_expr(value, Some(&target_sym.ty));
                if value_ty != target_sym.ty && value_ty != Type::Error && target_sym.ty != Type::Error {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "mismatched types in assignment to '{}': expected '{}', found '{}'",
                                target.name, target_sym.ty, value_ty
                            ),
                            value.span(),
                        )
                        .with_code("E0101")
                        .with_label(DiagnosticLabel::primary(value.span())),
                    );
                }
            }

            Stmt::While {
                condition,
                body,
                span: _,
            } => {
                let cond_ty = self.check_expr(condition, Some(&Type::Bool));
                if cond_ty != Type::Bool && cond_ty != Type::Error {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "while condition must evaluate to 'bool', found '{}'",
                                cond_ty
                            ),
                            condition.span(),
                        )
                        .with_code("E0107")
                        .with_label(DiagnosticLabel::primary(condition.span())),
                    );
                }

                self.check_block(body, true);
            }

            Stmt::Return { value, span } => {
                let expected = self
                    .current_return_type
                    .clone()
                    .unwrap_or(Type::Unit);

                if let Some(val_expr) = value {
                    let actual = self.check_expr(val_expr, Some(&expected));
                    if actual != expected && actual != Type::Error && expected != Type::Error {
                        self.diagnostics.push(
                            Diagnostic::error(
                                format!(
                                    "mismatched return type: expected '{}', found '{}'",
                                    expected, actual
                                ),
                                val_expr.span(),
                            )
                            .with_code("E0106")
                            .with_label(DiagnosticLabel::primary(val_expr.span())),
                        );
                    }
                } else if expected != Type::Unit && expected != Type::Error {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "return statement has no value, but function expects return type '{}'",
                                expected
                            ),
                            *span,
                        )
                        .with_code("E0106")
                        .with_label(DiagnosticLabel::primary(*span)),
                    );
                }
            }

            Stmt::Expr { expr, .. } => {
                self.check_expr(expr, None);
            }
        }
    }

    fn check_expr(&mut self, expr: &Expr, expected_type: Option<&Type>) -> Type {
        match expr {
            Expr::Literal(lit) => match lit {
                Literal::Int(val, span) => {
                    if let Some(expected) = expected_type {
                        if expected.is_integer() {
                            // Check unsigned bounds
                            if *val < 0 && matches!(expected, Type::U32 | Type::U64) {
                                self.diagnostics.push(
                                    Diagnostic::error(
                                        format!(
                                            "negative integer literal '{}' cannot be assigned to unsigned type '{}'",
                                            val, expected
                                        ),
                                        *span,
                                    )
                                    .with_code("E0101")
                                    .with_label(DiagnosticLabel::primary(*span)),
                                );
                                return Type::Error;
                            }
                            return expected.clone();
                        }
                    }
                    // Default integer literal to i32
                    Type::I32
                }
                Literal::Float(_, _) => Type::F64,
                Literal::Bool(_, _) => Type::Bool,
                Literal::String(_, _) => Type::Str,
            },

            Expr::Variable(ident) => match self.symbols.lookup_variable(&ident.name) {
                Some(sym) => sym.ty.clone(),
                None => {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!("cannot find variable '{}' in this scope", ident.name),
                            ident.span,
                        )
                        .with_code("E0103")
                        .with_label(DiagnosticLabel::primary(ident.span)),
                    );
                    Type::Error
                }
            },

            Expr::Unary { op, operand, span } => {
                let inner_ty = self.check_expr(operand, None);
                if inner_ty == Type::Error {
                    return Type::Error;
                }

                match op {
                    UnaryOp::Neg => {
                        if !inner_ty.is_signed_numeric() {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    format!(
                                        "cannot apply unary '-' to non-signed/non-numeric type '{}'",
                                        inner_ty
                                    ),
                                    *span,
                                )
                                .with_code("E0108")
                                .with_label(DiagnosticLabel::primary(*span)),
                            );
                            Type::Error
                        } else {
                            inner_ty
                        }
                    }
                    UnaryOp::Not => {
                        if inner_ty != Type::Bool {
                            self.diagnostics.push(
                                Diagnostic::error(
                                    format!("cannot apply unary '!' to non-bool type '{}'", inner_ty),
                                    *span,
                                )
                                .with_code("E0108")
                                .with_label(DiagnosticLabel::primary(*span)),
                            );
                            Type::Error
                        } else {
                            Type::Bool
                        }
                    }
                }
            }

            Expr::Binary {
                op,
                left,
                right,
                span,
            } => self.check_binary_expr(*op, left, right, *span),

            Expr::Grouping { expr, .. } => self.check_expr(expr, expected_type),

            Expr::Call {
                callee,
                args,
                span,
            } => {
                let fn_sym = match self.symbols.lookup_function(&callee.name) {
                    Some(sym) => sym.clone(),
                    None => {
                        if callee.name == "print" || callee.name == "println" {
                            return self.check_builtin_print(&callee.name, args, *span);
                        }

                        self.diagnostics.push(
                            Diagnostic::error(
                                format!("cannot find function '{}' in this scope", callee.name),
                                callee.span,
                            )
                            .with_code("E0103")
                            .with_label(DiagnosticLabel::primary(callee.span)),
                        );
                        return Type::Error;
                    }
                };

                if args.len() != fn_sym.params.len() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "function '{}' expects {} argument(s), but {} were supplied",
                                callee.name,
                                fn_sym.params.len(),
                                args.len()
                            ),
                            *span,
                        )
                        .with_code("E0105")
                        .with_label(DiagnosticLabel::primary(*span)),
                    );
                    return fn_sym.return_type;
                }

                for (idx, (arg_expr, (_, expected_param_ty, _))) in
                    args.iter().zip(&fn_sym.params).enumerate()
                {
                    let arg_ty = self.check_expr(arg_expr, Some(expected_param_ty));
                    if arg_ty != *expected_param_ty
                        && arg_ty != Type::Error
                        && *expected_param_ty != Type::Error
                    {
                        self.diagnostics.push(
                            Diagnostic::error(
                                format!(
                                    "mismatched type for argument {} in call to '{}': expected '{}', found '{}'",
                                    idx + 1,
                                    callee.name,
                                    expected_param_ty,
                                    arg_ty
                                ),
                                arg_expr.span(),
                            )
                            .with_code("E0105")
                            .with_label(DiagnosticLabel::primary(arg_expr.span())),
                        );
                    }
                }

                fn_sym.return_type
            }

            Expr::If {
                condition,
                then_branch,
                else_branch,
                span,
            } => {
                let cond_ty = self.check_expr(condition, Some(&Type::Bool));
                if cond_ty != Type::Bool && cond_ty != Type::Error {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!("if condition must evaluate to 'bool', found '{}'", cond_ty),
                            condition.span(),
                        )
                        .with_code("E0107")
                        .with_label(DiagnosticLabel::primary(condition.span())),
                    );
                }

                let then_ty = self.check_block(then_branch, true);

                if let Some(else_b) = else_branch {
                    let else_ty = self.check_block(else_b, true);
                    if then_ty != else_ty && then_ty != Type::Error && else_ty != Type::Error {
                        self.diagnostics.push(
                            Diagnostic::error(
                                format!(
                                    "incompatible types in 'if/else' branches: then branch has type '{}', but else branch has type '{}'",
                                    then_ty, else_ty
                                ),
                                *span,
                            )
                            .with_code("E0101")
                            .with_label(DiagnosticLabel::primary(*span)),
                        );
                        Type::Error
                    } else {
                        then_ty
                    }
                } else {
                    Type::Unit
                }
            }

            Expr::Block(block) => self.check_block(block, true),
        }
    }

    fn check_binary_expr(
        &mut self,
        op: BinaryOp,
        left: &Expr,
        right: &Expr,
        span: Span,
    ) -> Type {
        match op {
            BinaryOp::Add | BinaryOp::Sub | BinaryOp::Mul | BinaryOp::Div => {
                let left_ty = self.check_expr(left, None);
                let right_ty = self.check_expr(right, Some(&left_ty));

                if left_ty == Type::Error || right_ty == Type::Error {
                    return Type::Error;
                }

                if !left_ty.is_numeric() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "cannot apply arithmetic operator '{}' to non-numeric type '{}'",
                                op.as_str(),
                                left_ty
                            ),
                            left.span(),
                        )
                        .with_code("E0108")
                        .with_label(DiagnosticLabel::primary(left.span())),
                    );
                    return Type::Error;
                }

                if left_ty != right_ty {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "mismatched operand types for '{}': left is '{}', but right is '{}'",
                                op.as_str(),
                                left_ty,
                                right_ty
                            ),
                            span,
                        )
                        .with_code("E0101")
                        .with_label(DiagnosticLabel::primary(span)),
                    );
                    return Type::Error;
                }

                left_ty
            }

            BinaryOp::Rem => {
                let left_ty = self.check_expr(left, None);
                let right_ty = self.check_expr(right, Some(&left_ty));

                if left_ty == Type::Error || right_ty == Type::Error {
                    return Type::Error;
                }

                if !left_ty.is_integer() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "cannot apply remainder operator '%' to non-integer type '{}'",
                                left_ty
                            ),
                            left.span(),
                        )
                        .with_code("E0108")
                        .with_label(DiagnosticLabel::primary(left.span())),
                    );
                    return Type::Error;
                }

                if left_ty != right_ty {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "mismatched operand types for '%': left is '{}', but right is '{}'",
                                left_ty, right_ty
                            ),
                            span,
                        )
                        .with_code("E0101")
                        .with_label(DiagnosticLabel::primary(span)),
                    );
                    return Type::Error;
                }

                left_ty
            }

            BinaryOp::Lt | BinaryOp::Le | BinaryOp::Gt | BinaryOp::Ge => {
                let left_ty = self.check_expr(left, None);
                let right_ty = self.check_expr(right, Some(&left_ty));

                if left_ty == Type::Error || right_ty == Type::Error {
                    return Type::Error;
                }

                if !left_ty.is_numeric() {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "cannot apply comparison operator '{}' to non-numeric type '{}'",
                                op.as_str(),
                                left_ty
                            ),
                            left.span(),
                        )
                        .with_code("E0108")
                        .with_label(DiagnosticLabel::primary(left.span())),
                    );
                    return Type::Error;
                }

                if left_ty != right_ty {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "mismatched operand types for '{}': left is '{}', but right is '{}'",
                                op.as_str(),
                                left_ty,
                                right_ty
                            ),
                            span,
                        )
                        .with_code("E0101")
                        .with_label(DiagnosticLabel::primary(span)),
                    );
                    return Type::Error;
                }

                Type::Bool
            }

            BinaryOp::Eq | BinaryOp::Ne => {
                let left_ty = self.check_expr(left, None);
                let right_ty = self.check_expr(right, Some(&left_ty));

                if left_ty == Type::Error || right_ty == Type::Error {
                    return Type::Error;
                }

                if left_ty != right_ty {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "mismatched operand types for equality check '{}': left is '{}', but right is '{}'",
                                op.as_str(),
                                left_ty,
                                right_ty
                            ),
                            span,
                        )
                        .with_code("E0101")
                        .with_label(DiagnosticLabel::primary(span)),
                    );
                    return Type::Error;
                }

                Type::Bool
            }

            BinaryOp::And | BinaryOp::Or => {
                let left_ty = self.check_expr(left, Some(&Type::Bool));
                let right_ty = self.check_expr(right, Some(&Type::Bool));

                if left_ty == Type::Error || right_ty == Type::Error {
                    return Type::Error;
                }

                if left_ty != Type::Bool {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "left operand of logical operator '{}' must be 'bool', found '{}'",
                                op.as_str(),
                                left_ty
                            ),
                            left.span(),
                        )
                        .with_code("E0108")
                        .with_label(DiagnosticLabel::primary(left.span())),
                    );
                }

                if right_ty != Type::Bool {
                    self.diagnostics.push(
                        Diagnostic::error(
                            format!(
                                "right operand of logical operator '{}' must be 'bool', found '{}'",
                                op.as_str(),
                                right_ty
                            ),
                            right.span(),
                        )
                        .with_code("E0108")
                        .with_label(DiagnosticLabel::primary(right.span())),
                    );
                }

                Type::Bool
            }
        }
    }

    fn check_builtin_print(&mut self, name: &str, args: &[Expr], span: Span) -> Type {
        if args.len() != 1 {
            self.diagnostics.push(
                Diagnostic::error(
                    format!(
                        "function '{}' expects 1 argument(s), but {} were supplied",
                        name,
                        args.len()
                    ),
                    span,
                )
                .with_code("E0105")
                .with_label(DiagnosticLabel::primary(span)),
            );
            return Type::Unit;
        }

        let arg = &args[0];
        let arg_ty = self.check_expr(arg, None);
        if arg_ty == Type::Error {
            return Type::Unit;
        }

        if !arg_ty.is_printable() {
            self.diagnostics.push(
                Diagnostic::error(
                    format!(
                        "cannot print value of type '{}': '{}' only accepts str, i32, i64, u32, u64, or bool",
                        arg_ty, name
                    ),
                    arg.span(),
                )
                .with_code("E0109")
                .with_label(DiagnosticLabel::primary(arg.span())),
            );
        }

        Type::Unit
    }
}

