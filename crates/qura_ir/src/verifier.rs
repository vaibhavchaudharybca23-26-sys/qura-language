use crate::ir::*;
use std::collections::{HashMap, HashSet};
use std::fmt;

#[derive(Debug, Clone, PartialEq)]
pub struct QirVerificationError {
    pub function: String,
    pub block: Option<BlockId>,
    pub message: String,
}

impl fmt::Display for QirVerificationError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(b) = self.block {
            write!(f, "in func @{}, {}: {}", self.function, b, self.message)
        } else {
            write!(f, "in func @{}: {}", self.function, self.message)
        }
    }
}

pub struct Verifier<'a> {
    func: &'a Function,
    errors: Vec<QirVerificationError>,
    value_types: HashMap<Value, Type>,
    value_def_locations: HashMap<Value, (BlockId, usize)>, // (Block, instruction index or 0 for block param)
    block_map: HashMap<BlockId, &'a BasicBlock>,
    dominators: HashMap<BlockId, HashSet<BlockId>>,
}

impl<'a> Verifier<'a> {
    pub fn new(func: &'a Function) -> Self {
        Self {
            func,
            errors: Vec::new(),
            value_types: HashMap::new(),
            value_def_locations: HashMap::new(),
            block_map: HashMap::new(),
            dominators: HashMap::new(),
        }
    }

    pub fn verify(mut self) -> Result<(), Vec<QirVerificationError>> {
        if self.func.blocks.is_empty() {
            self.errors.push(QirVerificationError {
                function: self.func.name.clone(),
                block: None,
                message: "function has no basic blocks".to_string(),
            });
            return Err(self.errors);
        }

        // 1. Index all blocks
        for block in &self.func.blocks {
            if self.block_map.insert(block.id, block).is_some() {
                self.errors.push(QirVerificationError {
                    function: self.func.name.clone(),
                    block: Some(block.id),
                    message: format!("duplicate block ID '{}'", block.id),
                });
            }
        }

        let entry_id = self.func.blocks[0].id;

        if !self.func.blocks[0].params.is_empty() {
            self.errors.push(QirVerificationError {
                function: self.func.name.clone(),
                block: Some(entry_id),
                message: "entry block cannot have block parameters".to_string(),
            });
        }

        // 2. Register function parameters
        for (param_val, param_ty) in &self.func.params {
            if self
                .value_types
                .insert(*param_val, *param_ty)
                .is_some()
            {
                self.errors.push(QirVerificationError {
                    function: self.func.name.clone(),
                    block: None,
                    message: format!("duplicate definition of function parameter {}", param_val),
                });
            }
            // Param is defined at entry block, pseudo-index 0
            self.value_def_locations.insert(*param_val, (entry_id, 0));
        }

        // 3. Register block parameters & instructions (checking single definition)
        for block in &self.func.blocks {
            for (p_val, p_ty) in &block.params {
                if self.value_types.insert(*p_val, *p_ty).is_some() {
                    self.errors.push(QirVerificationError {
                        function: self.func.name.clone(),
                        block: Some(block.id),
                        message: format!("SSA violation: value {} defined more than once", p_val),
                    });
                }
                self.value_def_locations.insert(*p_val, (block.id, 0));
            }

            for (idx, (i_val, inst)) in block.instructions.iter().enumerate() {
                let inst_ty = self.type_of_instruction(inst);
                if self.value_types.insert(*i_val, inst_ty).is_some() {
                    self.errors.push(QirVerificationError {
                        function: self.func.name.clone(),
                        block: Some(block.id),
                        message: format!("SSA violation: value {} defined more than once", i_val),
                    });
                }
                // Index + 1 so instruction 0 comes after block parameters (0)
                self.value_def_locations
                    .insert(*i_val, (block.id, idx + 1));
            }
        }

        // 4. Reachability Check
        let mut reachable = HashSet::new();
        let mut queue = std::collections::VecDeque::new();
        queue.push_back(entry_id);
        reachable.insert(entry_id);

        while let Some(curr) = queue.pop_front() {
            if let Some(b) = self.block_map.get(&curr) {
                match &b.terminator {
                    Terminator::Return(_) => {}
                    Terminator::Branch { target, .. } => {
                        if reachable.insert(*target) {
                            queue.push_back(*target);
                        }
                    }
                    Terminator::CondBranch {
                        then_block,
                        else_block,
                        ..
                    } => {
                        if reachable.insert(*then_block) {
                            queue.push_back(*then_block);
                        }
                        if reachable.insert(*else_block) {
                            queue.push_back(*else_block);
                        }
                    }
                }
            }
        }

        for block in &self.func.blocks {
            if !reachable.contains(&block.id) {
                self.errors.push(QirVerificationError {
                    function: self.func.name.clone(),
                    block: Some(block.id),
                    message: format!("block {} is unreachable from entry", block.id),
                });
            }
        }

        // 5. Compute Dominance Tree
        self.compute_dominators(entry_id);

        // 6. Verify Instructions and Uses
        for block in &self.func.blocks {
            for (idx, (def_val, inst)) in block.instructions.iter().enumerate() {
                self.verify_instruction(block.id, idx + 1, *def_val, inst);
            }

            // Verify Terminator
            self.verify_terminator(block);
        }

        if self.errors.is_empty() {
            Ok(())
        } else {
            Err(self.errors)
        }
    }

    fn type_of_instruction(&self, inst: &Instruction) -> Type {
        match inst {
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
                _ => self.value_types.get(lhs).copied().unwrap_or(Type::I32),
            },
            Instruction::Unary { op, val } => match op {
                UnaryOp::Not => Type::Bool,
                UnaryOp::Neg => self.value_types.get(val).copied().unwrap_or(Type::I32),
            },
            Instruction::Call { .. } => Type::I32,
        }
    }

    fn compute_dominators(&mut self, entry: BlockId) {
        // Collect predecessors
        let mut preds: HashMap<BlockId, Vec<BlockId>> = HashMap::new();
        for b in &self.func.blocks {
            preds.entry(b.id).or_default();
        }

        for b in &self.func.blocks {
            match &b.terminator {
                Terminator::Return(_) => {}
                Terminator::Branch { target, .. } => {
                    preds.entry(*target).or_default().push(b.id);
                }
                Terminator::CondBranch {
                    then_block,
                    else_block,
                    ..
                } => {
                    preds.entry(*then_block).or_default().push(b.id);
                    preds.entry(*else_block).or_default().push(b.id);
                }
            }
        }

        // Initialize dominators
        let all_blocks: HashSet<BlockId> = self.func.blocks.iter().map(|b| b.id).collect();
        for b in &self.func.blocks {
            if b.id == entry {
                let mut set = HashSet::new();
                set.insert(entry);
                self.dominators.insert(entry, set);
            } else {
                self.dominators.insert(b.id, all_blocks.clone());
            }
        }

        // Iterative dataflow fixed-point
        let mut changed = true;
        while changed {
            changed = false;
            for b in &self.func.blocks {
                if b.id == entry {
                    continue;
                }

                let b_preds = &preds[&b.id];
                if b_preds.is_empty() {
                    continue;
                }

                let mut new_dom = self.dominators[&b_preds[0]].clone();
                for p in &b_preds[1..] {
                    new_dom = new_dom
                        .intersection(&self.dominators[p])
                        .copied()
                        .collect();
                }
                new_dom.insert(b.id);

                if new_dom != self.dominators[&b.id] {
                    self.dominators.insert(b.id, new_dom);
                    changed = true;
                }
            }
        }
    }

    fn check_dominance(&mut self, used_val: Value, user_block: BlockId, user_inst_idx: usize) {
        let (def_block, def_inst_idx) = match self.value_def_locations.get(&used_val) {
            Some(loc) => *loc,
            None => {
                self.errors.push(QirVerificationError {
                    function: self.func.name.clone(),
                    block: Some(user_block),
                    message: format!("use of undefined value {}", used_val),
                });
                return;
            }
        };

        if def_block == user_block {
            // Must be defined prior in the same block
            if def_inst_idx >= user_inst_idx {
                self.errors.push(QirVerificationError {
                    function: self.func.name.clone(),
                    block: Some(user_block),
                    message: format!(
                        "value {} used before its definition in the same block",
                        used_val
                    ),
                });
            }
        } else {
            // Must strictly dominate the user block
            let doms = match self.dominators.get(&user_block) {
                Some(d) => d,
                None => {
                    self.errors.push(QirVerificationError {
                        function: self.func.name.clone(),
                        block: Some(user_block),
                        message: format!("unreachable block {}", user_block),
                    });
                    return;
                }
            };

            if !doms.contains(&def_block) {
                self.errors.push(QirVerificationError {
                    function: self.func.name.clone(),
                    block: Some(user_block),
                    message: format!(
                        "dominance violation: definition of {} in {} does not dominate use in {}",
                        used_val, def_block, user_block
                    ),
                });
            }
        }
    }

    fn verify_instruction(
        &mut self,
        block_id: BlockId,
        inst_idx: usize,
        _def_val: Value,
        inst: &Instruction,
    ) {
        match inst {
            Instruction::ConstInt(_, _)
            | Instruction::ConstFloat(_)
            | Instruction::ConstBool(_) => {}

            Instruction::Binary { op, lhs, rhs } => {
                self.check_dominance(*lhs, block_id, inst_idx);
                self.check_dominance(*rhs, block_id, inst_idx);

                if let (Some(l_ty), Some(r_ty)) = (self.value_types.get(lhs), self.value_types.get(rhs)) {
                    if l_ty != r_ty {
                        self.errors.push(QirVerificationError {
                            function: self.func.name.clone(),
                            block: Some(block_id),
                            message: format!(
                                "binary '{}' operand type mismatch: left is {}, right is {}",
                                op.as_str(),
                                l_ty,
                                r_ty
                            ),
                        });
                    }
                }
            }

            Instruction::Unary { op, val } => {
                self.check_dominance(*val, block_id, inst_idx);
                if *op == UnaryOp::Not {
                    if let Some(val_ty) = self.value_types.get(val) {
                        if *val_ty != Type::Bool {
                            self.errors.push(QirVerificationError {
                                function: self.func.name.clone(),
                                block: Some(block_id),
                                message: format!("unary 'not' expects bool operand, found {}", val_ty),
                            });
                        }
                    }
                }
            }

            Instruction::Call { args, .. } => {
                for arg in args {
                    self.check_dominance(*arg, block_id, inst_idx);
                }
            }
        }
    }

    fn verify_terminator(&mut self, block: &BasicBlock) {
        let term_idx = block.instructions.len() + 1;
        let entry_id = self.func.blocks[0].id;

        match &block.terminator {
            Terminator::Return(opt_val) => {
                if let Some(val) = opt_val {
                    self.check_dominance(*val, block.id, term_idx);
                    if let Some(val_ty) = self.value_types.get(val) {
                        if *val_ty != self.func.return_type {
                            self.errors.push(QirVerificationError {
                                function: self.func.name.clone(),
                                block: Some(block.id),
                                message: format!(
                                    "return type mismatch: function expects {}, returned {}",
                                    self.func.return_type, val_ty
                                ),
                            });
                        }
                    }
                } else if self.func.return_type != Type::Unit {
                    self.errors.push(QirVerificationError {
                        function: self.func.name.clone(),
                        block: Some(block.id),
                        message: format!(
                            "empty return in function with non-unit return type {}",
                            self.func.return_type
                        ),
                    });
                }
            }

            Terminator::Branch { target, args } => {
                if *target == entry_id {
                    self.errors.push(QirVerificationError {
                        function: self.func.name.clone(),
                        block: Some(block.id),
                        message: format!("cannot branch to entry block {}", entry_id),
                    });
                }

                let target_block = match self.block_map.get(target) {
                    Some(b) => *b,
                    None => {
                        self.errors.push(QirVerificationError {
                            function: self.func.name.clone(),
                            block: Some(block.id),
                            message: format!("branch target {} does not exist", target),
                        });
                        return;
                    }
                };

                // Check dominance for ALL arguments passed to branch
                for arg in args {
                    self.check_dominance(*arg, block.id, term_idx);
                }

                if args.len() != target_block.params.len() {
                    self.errors.push(QirVerificationError {
                        function: self.func.name.clone(),
                        block: Some(block.id),
                        message: format!(
                            "branch to {} passes {} argument(s), but block expects {}",
                            target,
                            args.len(),
                            target_block.params.len()
                        ),
                    });
                }

                for (arg, (_, expected_ty)) in args.iter().zip(&target_block.params) {
                    if let Some(actual_ty) = self.value_types.get(arg) {
                        if actual_ty != expected_ty {
                            self.errors.push(QirVerificationError {
                                function: self.func.name.clone(),
                                block: Some(block.id),
                                message: format!(
                                    "type mismatch for argument passed to {}: expected {}, found {}",
                                    target, expected_ty, actual_ty
                                ),
                            });
                        }
                    }
                }
            }

            Terminator::CondBranch {
                cond,
                then_block,
                then_args,
                else_block,
                else_args,
            } => {
                if *then_block == entry_id || *else_block == entry_id {
                    self.errors.push(QirVerificationError {
                        function: self.func.name.clone(),
                        block: Some(block.id),
                        message: format!("cannot branch to entry block {}", entry_id),
                    });
                }

                self.check_dominance(*cond, block.id, term_idx);

                if let Some(cond_ty) = self.value_types.get(cond) {
                    if *cond_ty != Type::Bool {
                        self.errors.push(QirVerificationError {
                            function: self.func.name.clone(),
                            block: Some(block.id),
                            message: format!(
                                "cond_br condition must be bool, found {}",
                                cond_ty
                            ),
                        });
                    }
                }

                // Check dominance for all arguments in both branches
                for arg in then_args {
                    self.check_dominance(*arg, block.id, term_idx);
                }
                for arg in else_args {
                    self.check_dominance(*arg, block.id, term_idx);
                }

                // Verify then_block
                let t_block = match self.block_map.get(then_block) {
                    Some(b) => Some(*b),
                    None => {
                        self.errors.push(QirVerificationError {
                            function: self.func.name.clone(),
                            block: Some(block.id),
                            message: format!("then branch target {} does not exist", then_block),
                        });
                        None
                    }
                };

                if let Some(tb) = t_block {
                    if then_args.len() != tb.params.len() {
                        self.errors.push(QirVerificationError {
                            function: self.func.name.clone(),
                            block: Some(block.id),
                            message: format!(
                                "then branch to {} passes {} argument(s), but block expects {}",
                                then_block,
                                then_args.len(),
                                tb.params.len()
                            ),
                        });
                    }
                    for (arg, (_, expected_ty)) in then_args.iter().zip(&tb.params) {
                        self.check_dominance(*arg, block.id, term_idx);
                        if let Some(actual_ty) = self.value_types.get(arg) {
                            if actual_ty != expected_ty {
                                self.errors.push(QirVerificationError {
                                    function: self.func.name.clone(),
                                    block: Some(block.id),
                                    message: format!(
                                        "type mismatch for argument passed to {}: expected {}, found {}",
                                        then_block, expected_ty, actual_ty
                                    ),
                                });
                            }
                        }
                    }
                }

                // Verify else_block
                let e_block = match self.block_map.get(else_block) {
                    Some(b) => Some(*b),
                    None => {
                        self.errors.push(QirVerificationError {
                            function: self.func.name.clone(),
                            block: Some(block.id),
                            message: format!("else branch target {} does not exist", else_block),
                        });
                        None
                    }
                };

                if let Some(eb) = e_block {
                    if else_args.len() != eb.params.len() {
                        self.errors.push(QirVerificationError {
                            function: self.func.name.clone(),
                            block: Some(block.id),
                            message: format!(
                                "else branch to {} passes {} argument(s), but block expects {}",
                                else_block,
                                else_args.len(),
                                eb.params.len()
                            ),
                        });
                    }
                    for (arg, (_, expected_ty)) in else_args.iter().zip(&eb.params) {
                        self.check_dominance(*arg, block.id, term_idx);
                        if let Some(actual_ty) = self.value_types.get(arg) {
                            if actual_ty != expected_ty {
                                self.errors.push(QirVerificationError {
                                    function: self.func.name.clone(),
                                    block: Some(block.id),
                                    message: format!(
                                        "type mismatch for argument passed to {}: expected {}, found {}",
                                        else_block, expected_ty, actual_ty
                                    ),
                                });
                            }
                        }
                    }
                }
            }
        }
    }
}

pub fn verify_module(module: &Module) -> Result<(), Vec<QirVerificationError>> {
    let mut all_errors = Vec::new();

    for func in &module.functions {
        let verifier = Verifier::new(func);
        if let Err(errors) = verifier.verify() {
            all_errors.extend(errors);
        }
    }

    if all_errors.is_empty() {
        Ok(())
    } else {
        Err(all_errors)
    }
}
