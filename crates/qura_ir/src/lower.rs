use crate::ir::{
    BasicBlock, BinaryOp, BlockId, Function, Instruction, Module, Terminator, Type, UnaryOp, Value,
};
use qura_sema::SymbolTable;
use qura_syntax::ast as syn;
use std::collections::HashMap;

pub struct FunctionLowerer<'a> {
    val_counter: u32,
    block_counter: u32,
    blocks: Vec<BasicBlock>,
    current_block_id: Option<BlockId>,
    current_params: Vec<(Value, Type)>,
    current_instructions: Vec<(Value, Instruction)>,
    current_is_terminated: bool,
    var_map: HashMap<String, (Value, Type, bool)>, // (Value, Type, is_mut)
    _symbols: &'a SymbolTable,
}

impl<'a> FunctionLowerer<'a> {
    pub fn new(symbols: &'a SymbolTable) -> Self {
        Self {
            val_counter: 0,
            block_counter: 0,
            blocks: Vec::new(),
            current_block_id: None,
            current_params: Vec::new(),
            current_instructions: Vec::new(),
            current_is_terminated: false,
            var_map: HashMap::new(),
            _symbols: symbols,
        }
    }

    fn alloc_val(&mut self) -> Value {
        let val = Value(self.val_counter);
        self.val_counter += 1;
        val
    }

    fn alloc_block_id(&mut self) -> BlockId {
        let id = BlockId(self.block_counter);
        self.block_counter += 1;
        id
    }

    fn start_block(&mut self, id: BlockId, params: Vec<(Value, Type)>) {
        assert!(
            self.current_block_id.is_none() || self.current_is_terminated,
            "cannot start new block while current block is unterminated"
        );
        self.current_block_id = Some(id);
        self.current_params = params;
        self.current_instructions = Vec::new();
        self.current_is_terminated = false;
    }

    fn emit(&mut self, inst: Instruction) -> Value {
        assert!(
            !self.current_is_terminated,
            "cannot emit instruction into terminated block"
        );
        let val = self.alloc_val();
        self.current_instructions.push((val, inst));
        val
    }

    fn terminate(&mut self, term: Terminator) {
        if self.current_is_terminated {
            return;
        }
        let id = self
            .current_block_id
            .take()
            .expect("cannot terminate without an active block");
        let block = BasicBlock {
            id,
            params: std::mem::take(&mut self.current_params),
            instructions: std::mem::take(&mut self.current_instructions),
            terminator: term,
        };
        self.blocks.push(block);
        self.current_is_terminated = true;
    }

    fn is_current_terminated(&self) -> bool {
        self.current_is_terminated
    }

    fn get_active_mut_vars(&self) -> Vec<(String, Type)> {
        let mut vars: Vec<(String, Type)> = self
            .var_map
            .iter()
            .filter(|(_, (_, _, is_mut))| *is_mut)
            .map(|(name, (_, ty, _))| (name.clone(), *ty))
            .collect();
        vars.sort_by(|a, b| a.0.cmp(&b.0)); // deterministic order
        vars
    }

    pub fn lower_function(mut self, func: &syn::FunctionDecl) -> Function {
        let return_type = match &func.return_type {
            Some(ty_expr) => {
                let sema_ty = qura_sema::Type::from_type_expr(ty_expr)
                    .unwrap_or(qura_sema::Type::Unit);
                Type::from(sema_ty)
            }
            None => Type::Unit,
        };

        let entry_id = self.alloc_block_id();
        self.start_block(entry_id, Vec::new());

        // Function parameter values
        let mut fn_params = Vec::new();
        for p in &func.params {
            let sema_ty = qura_sema::Type::from_type_expr(&p.ty).unwrap_or(qura_sema::Type::Unit);
            let ir_ty = Type::from(sema_ty);
            let val = self.alloc_val();
            fn_params.push((val, ir_ty));
            self.var_map
                .insert(p.name.name.clone(), (val, ir_ty, p.is_mut));
        }

        // Lower body statements
        let tail_val = self.lower_block(&func.body);

        // Terminate function if fallthrough
        if !self.is_current_terminated() {
            if return_type == Type::Unit {
                self.terminate(Terminator::Return(None));
            } else if let Some(val) = tail_val {
                self.terminate(Terminator::Return(Some(val)));
            } else {
                // If unreachable fallthrough with non-unit return, emit zero/ret
                let default_ret = self.emit(Instruction::ConstInt(0, return_type));
                self.terminate(Terminator::Return(Some(default_ret)));
            }
        }

        Function {
            name: func.name.name.clone(),
            params: fn_params,
            return_type,
            blocks: self.blocks,
        }
    }

    fn lower_block(&mut self, block: &syn::Block) -> Option<Value> {
        for stmt in &block.stmts {
            self.lower_stmt(stmt);
            if self.is_current_terminated() {
                break;
            }
        }

        if !self.is_current_terminated() {
            if let Some(ref tail_expr) = block.tail_expr {
                Some(self.lower_expr(tail_expr))
            } else {
                None
            }
        } else {
            None
        }
    }

    fn lower_stmt(&mut self, stmt: &syn::Stmt) {
        if self.is_current_terminated() {
            return;
        }

        match stmt {
            syn::Stmt::Let {
                name,
                is_mut,
                ty,
                init,
                ..
            } => {
                let init_val = self.lower_expr(init);
                let ir_ty = if let Some(t_expr) = ty {
                    let sema_ty = qura_sema::Type::from_type_expr(t_expr)
                        .unwrap_or(qura_sema::Type::I32);
                    Type::from(sema_ty)
                } else {
                    self.type_of_val(init_val)
                };

                self.var_map
                    .insert(name.name.clone(), (init_val, ir_ty, *is_mut));
            }

            syn::Stmt::Assign { target, value, .. } => {
                let new_val = self.lower_expr(value);
                if let Some((val_slot, _, _)) = self.var_map.get_mut(&target.name) {
                    *val_slot = new_val;
                }
            }

            syn::Stmt::While {
                condition,
                body,
                ..
            } => {
                let active_muts = self.get_active_mut_vars();
                let header_id = self.alloc_block_id();
                let body_id = self.alloc_block_id();
                let exit_id = self.alloc_block_id();

                // 1. Branch to header with current mut values
                let init_args: Vec<Value> = active_muts
                    .iter()
                    .map(|(n, _)| self.var_map[n].0)
                    .collect();
                self.terminate(Terminator::Branch {
                    target: header_id,
                    args: init_args,
                });

                // 2. Start header block with parameters for each active mut var
                let mut header_params = Vec::new();
                for (name, ty) in &active_muts {
                    let p_val = self.alloc_val();
                    header_params.push((p_val, *ty));
                    self.var_map.get_mut(name).unwrap().0 = p_val;
                }
                self.start_block(header_id, header_params);

                // Check condition
                let cond_val = self.lower_expr(condition);

                // Exit args carry the current loop parameter values
                let exit_args: Vec<Value> = active_muts
                    .iter()
                    .map(|(n, _)| self.var_map[n].0)
                    .collect();

                self.terminate(Terminator::CondBranch {
                    cond: cond_val,
                    then_block: body_id,
                    then_args: Vec::new(),
                    else_block: exit_id,
                    else_args: exit_args,
                });

                // 3. Body block
                self.start_block(body_id, Vec::new());
                self.lower_block(body);

                if !self.is_current_terminated() {
                    let updated_args: Vec<Value> = active_muts
                        .iter()
                        .map(|(n, _)| self.var_map[n].0)
                        .collect();
                    self.terminate(Terminator::Branch {
                        target: header_id,
                        args: updated_args,
                    });
                }

                // 4. Exit block
                let mut exit_params = Vec::new();
                for (name, ty) in &active_muts {
                    let p_val = self.alloc_val();
                    exit_params.push((p_val, *ty));
                    self.var_map.get_mut(name).unwrap().0 = p_val;
                }
                self.start_block(exit_id, exit_params);
            }

            syn::Stmt::Return { value, .. } => {
                if let Some(expr) = value {
                    let val = self.lower_expr(expr);
                    self.terminate(Terminator::Return(Some(val)));
                } else {
                    self.terminate(Terminator::Return(None));
                }
            }

            syn::Stmt::Expr { expr, .. } => {
                self.lower_expr(expr);
            }
        }
    }

    fn lower_expr(&mut self, expr: &syn::Expr) -> Value {
        match expr {
            syn::Expr::Literal(lit) => match lit {
                syn::Literal::Int(v, _) => self.emit(Instruction::ConstInt(*v, Type::I32)),
                syn::Literal::Float(v, _) => self.emit(Instruction::ConstFloat(*v)),
                syn::Literal::Bool(v, _) => self.emit(Instruction::ConstBool(*v)),
            },

            syn::Expr::Variable(ident) => {
                if let Some((val, _, _)) = self.var_map.get(&ident.name) {
                    *val
                } else {
                    self.emit(Instruction::ConstInt(0, Type::I32))
                }
            }

            syn::Expr::Unary { op, operand, .. } => {
                let inner = self.lower_expr(operand);
                let ir_op = match op {
                    syn::UnaryOp::Neg => UnaryOp::Neg,
                    syn::UnaryOp::Not => UnaryOp::Not,
                };
                self.emit(Instruction::Unary {
                    op: ir_op,
                    val: inner,
                })
            }

            syn::Expr::Binary {
                op, left, right, ..
            } => {
                let l_val = self.lower_expr(left);
                let r_val = self.lower_expr(right);
                let ir_op = match op {
                    syn::BinaryOp::Add => BinaryOp::Add,
                    syn::BinaryOp::Sub => BinaryOp::Sub,
                    syn::BinaryOp::Mul => BinaryOp::Mul,
                    syn::BinaryOp::Div => BinaryOp::Div,
                    syn::BinaryOp::Rem => BinaryOp::Rem,
                    syn::BinaryOp::Eq => BinaryOp::Eq,
                    syn::BinaryOp::Ne => BinaryOp::Ne,
                    syn::BinaryOp::Lt => BinaryOp::Lt,
                    syn::BinaryOp::Le => BinaryOp::Le,
                    syn::BinaryOp::Gt => BinaryOp::Gt,
                    syn::BinaryOp::Ge => BinaryOp::Ge,
                    syn::BinaryOp::And => BinaryOp::And,
                    syn::BinaryOp::Or => BinaryOp::Or,
                };
                self.emit(Instruction::Binary {
                    op: ir_op,
                    lhs: l_val,
                    rhs: r_val,
                })
            }

            syn::Expr::Grouping { expr, .. } => self.lower_expr(expr),

            syn::Expr::Call { callee, args, .. } => {
                let mut arg_vals = Vec::new();
                for a in args {
                    arg_vals.push(self.lower_expr(a));
                }
                self.emit(Instruction::Call {
                    func: callee.name.clone(),
                    args: arg_vals,
                })
            }

            syn::Expr::Block(block) => {
                self.lower_block(block)
                    .unwrap_or_else(|| self.emit(Instruction::ConstInt(0, Type::Unit)))
            }

            syn::Expr::If {
                condition,
                then_branch,
                else_branch,
                ..
            } => self.lower_if_expr(condition, then_branch, else_branch.as_ref()),
        }
    }

    fn lower_if_expr(
        &mut self,
        condition: &syn::Expr,
        then_branch: &syn::Block,
        else_branch: Option<&syn::Block>,
    ) -> Value {
        let active_muts = self.get_active_mut_vars();
        let cond_val = self.lower_expr(condition);

        let then_id = self.alloc_block_id();
        let else_id = self.alloc_block_id();
        let merge_id = self.alloc_block_id();

        // Branch from current
        self.terminate(Terminator::CondBranch {
            cond: cond_val,
            then_block: then_id,
            then_args: Vec::new(),
            else_block: else_id,
            else_args: Vec::new(),
        });

        // 1. Lower then branch
        let saved_vars = self.var_map.clone();
        self.start_block(then_id, Vec::new());
        let then_tail = self.lower_block(then_branch);
        let then_terminated = self.is_current_terminated();
        let mut then_merge_args = Vec::new();
        if !then_terminated {
            for (name, _) in &active_muts {
                then_merge_args.push(self.var_map[name].0);
            }
            if let Some(t_val) = then_tail {
                then_merge_args.push(t_val);
            }
            self.terminate(Terminator::Branch {
                target: merge_id,
                args: then_merge_args,
            });
        }
        let then_vars = std::mem::replace(&mut self.var_map, saved_vars);

        // 2. Lower else branch
        self.start_block(else_id, Vec::new());
        let else_tail = if let Some(else_b) = else_branch {
            self.lower_block(else_b)
        } else {
            None
        };
        let else_terminated = self.is_current_terminated();
        let mut else_merge_args = Vec::new();
        if !else_terminated {
            for (name, _) in &active_muts {
                else_merge_args.push(self.var_map[name].0);
            }
            if let Some(e_val) = else_tail {
                else_merge_args.push(e_val);
            } else if then_tail.is_some() {
                // Default zero if then had value but else was empty
                let dummy = self.emit(Instruction::ConstInt(0, Type::I32));
                else_merge_args.push(dummy);
            }
            self.terminate(Terminator::Branch {
                target: merge_id,
                args: else_merge_args,
            });
        }

        // 3. Start merge block
        let mut merge_params = Vec::new();
        for (name, ty) in &active_muts {
            let p_val = self.alloc_val();
            merge_params.push((p_val, *ty));
            self.var_map.get_mut(name).unwrap().0 = p_val;
        }

        let result_val = if then_tail.is_some() || else_tail.is_some() {
            let res_ty = then_tail
                .map(|v| self.type_of_val(v))
                .or_else(|| else_tail.map(|v| self.type_of_val(v)))
                .unwrap_or(Type::I32);
            let res_val = self.alloc_val();
            merge_params.push((res_val, res_ty));
            res_val
        } else {
            self.val_counter += 1;
            Value(self.val_counter - 1)
        };

        if !then_terminated || !else_terminated {
            self.start_block(merge_id, merge_params);
        } else {
            self.var_map = then_vars;
        }

        result_val
    }

    fn type_of_val(&self, val: Value) -> Type {
        for block in &self.blocks {
            for (p_val, p_ty) in &block.params {
                if *p_val == val {
                    return *p_ty;
                }
            }
            for (i_val, inst) in &block.instructions {
                if *i_val == val {
                    return match inst {
                        Instruction::ConstInt(_, ty) => *ty,
                        Instruction::ConstFloat(_) => Type::F64,
                        Instruction::ConstBool(_) => Type::Bool,
                        Instruction::Binary { op, lhs, .. } => match op {
                            BinaryOp::Eq
                            | BinaryOp::Ne
                            | BinaryOp::Lt
                            | BinaryOp::Le
                            | BinaryOp::Gt
                            | BinaryOp::Ge
                            | BinaryOp::And
                            | BinaryOp::Or => Type::Bool,
                            _ => self.type_of_val(*lhs),
                        },
                        Instruction::Unary { op, val } => match op {
                            UnaryOp::Not => Type::Bool,
                            UnaryOp::Neg => self.type_of_val(*val),
                        },
                        Instruction::Call { .. } => Type::I32,
                    };
                }
            }
        }

        for (p_val, p_ty) in &self.current_params {
            if *p_val == val {
                return *p_ty;
            }
        }

        for (i_val, inst) in &self.current_instructions {
            if *i_val == val {
                return match inst {
                    Instruction::ConstInt(_, ty) => *ty,
                    Instruction::ConstFloat(_) => Type::F64,
                    Instruction::ConstBool(_) => Type::Bool,
                    Instruction::Binary { op, lhs, .. } => match op {
                        BinaryOp::Eq
                        | BinaryOp::Ne
                        | BinaryOp::Lt
                        | BinaryOp::Le
                        | BinaryOp::Gt
                        | BinaryOp::Ge
                        | BinaryOp::And
                        | BinaryOp::Or => Type::Bool,
                        _ => self.type_of_val(*lhs),
                    },
                    Instruction::Unary { op, val } => match op {
                        UnaryOp::Not => Type::Bool,
                        UnaryOp::Neg => self.type_of_val(*val),
                    },
                    Instruction::Call { .. } => Type::I32,
                };
            }
        }

        Type::I32
    }
}

pub fn lower_program(program: &syn::Program, symbols: &SymbolTable) -> Module {
    let mut module = Module::new("qura_module");

    for item in &program.items {
        match item {
            syn::Item::Function(func) => {
                let lowerer = FunctionLowerer::new(symbols);
                let lowered_func = lowerer.lower_function(func);
                module.functions.push(lowered_func);
            }
        }
    }

    module
}
