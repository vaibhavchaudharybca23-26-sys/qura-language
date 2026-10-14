use cranelift_codegen::ir::condcodes::{FloatCC, IntCC};
use cranelift_codegen::ir::{types, AbiParam, BlockArg, Function, InstBuilder, UserFuncName};
use cranelift_codegen::isa;
use cranelift_codegen::settings::{self, Configurable};
use cranelift_frontend::{FunctionBuilder, FunctionBuilderContext};
use cranelift_module::{default_libcall_names, FuncId, Linkage, Module as _};
use cranelift_object::{ObjectBuilder, ObjectModule};
use qura_ir::ir::{BinaryOp, BlockId, Instruction, Module, Terminator, Type, UnaryOp, Value};
use std::collections::HashMap;
use std::fmt;
use std::str::FromStr;
use target_lexicon::Triple;

#[derive(Debug)]
pub enum CodegenError {
    TargetLookup(String),
    ModuleInit(String),
    Cranelift(String),
    UnknownFunction(String),
    UndefinedValue(Value),
    UnknownBlock(BlockId),
    TypeMismatch(String),
    UnsupportedOperation(String),
}

impl fmt::Display for CodegenError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            CodegenError::TargetLookup(msg) => write!(f, "target lookup error: {}", msg),
            CodegenError::ModuleInit(msg) => write!(f, "module initialization error: {}", msg),
            CodegenError::Cranelift(msg) => write!(f, "cranelift error: {}", msg),
            CodegenError::UnknownFunction(name) => write!(f, "unknown function '@{}'", name),
            CodegenError::UndefinedValue(val) => write!(f, "undefined SSA value {}", val),
            CodegenError::UnknownBlock(b) => write!(f, "unknown basic block {}", b),
            CodegenError::TypeMismatch(msg) => write!(f, "type mismatch: {}", msg),
            CodegenError::UnsupportedOperation(msg) => write!(f, "unsupported operation: {}", msg),
        }
    }
}

impl std::error::Error for CodegenError {}

impl From<cranelift_module::ModuleError> for CodegenError {
    fn from(err: cranelift_module::ModuleError) -> Self {
        CodegenError::Cranelift(err.to_string())
    }
}

pub fn map_type(ty: Type) -> types::Type {
    match ty {
        Type::I32 => types::I32,
        Type::I64 => types::I64,
        Type::U32 => types::I32,
        Type::U64 => types::I64,
        Type::F64 => types::F64,
        Type::Bool => types::I8,
        Type::Unit => panic!("Unit type does not map to a Cranelift register type"),
    }
}

/// Compiles a backend-independent Qura IR module into Windows x86_64 COFF object bytes.
pub fn compile_module(module: &Module) -> Result<Vec<u8>, CodegenError> {
    let mut flag_builder = settings::builder();
    flag_builder
        .set("use_colocated_libcalls", "false")
        .map_err(|e| CodegenError::ModuleInit(e.to_string()))?;
    flag_builder
        .set("is_pic", "false")
        .map_err(|e| CodegenError::ModuleInit(e.to_string()))?;

    let triple = Triple::from_str("x86_64-pc-windows-msvc")
        .map_err(|e| CodegenError::TargetLookup(e.to_string()))?;
    let isa_builder = isa::lookup(triple).map_err(|e| CodegenError::TargetLookup(e.to_string()))?;
    let isa = isa_builder.finish(settings::Flags::new(flag_builder)).map_err(|e| CodegenError::TargetLookup(e.to_string()))?;

    let obj_builder = ObjectBuilder::new(isa, module.name.clone().into_bytes(), default_libcall_names())
        .map_err(|e| CodegenError::ModuleInit(e.to_string()))?;
    let mut cl_module = ObjectModule::new(obj_builder);

    // Collect all function return types for fast signature resolution
    let mut func_return_types: HashMap<String, Type> = HashMap::new();
    for func in &module.functions {
        func_return_types.insert(func.name.clone(), func.return_type);
    }

    // Step 1: Declare all functions in the module
    let mut func_ids: HashMap<String, FuncId> = HashMap::new();
    for func in &module.functions {
        let mut sig = cl_module.make_signature();
        for (_, p_ty) in &func.params {
            sig.params.push(AbiParam::new(map_type(*p_ty)));
        }
        if func.return_type != Type::Unit {
            sig.returns.push(AbiParam::new(map_type(func.return_type)));
        }

        let func_id = cl_module.declare_function(&func.name, Linkage::Export, &sig)?;
        func_ids.insert(func.name.clone(), func_id);
    }

    // Step 2: Define each function body
    let mut ctx = cl_module.make_context();
    let mut fn_builder_ctx = FunctionBuilderContext::new();

    for func in &module.functions {
        let func_id = func_ids[&func.name];

        // Compute QIR types for every SSA virtual value in this function
        let mut value_types: HashMap<Value, Type> = HashMap::new();
        for (p_val, p_ty) in &func.params {
            value_types.insert(*p_val, *p_ty);
        }
        for block in &func.blocks {
            for (p_val, p_ty) in &block.params {
                value_types.insert(*p_val, *p_ty);
            }
            for (def_val, inst) in &block.instructions {
                let ty = match inst {
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
                        _ => value_types.get(lhs).copied().unwrap_or(Type::I32),
                    },
                    Instruction::Unary { op, val } => match op {
                        UnaryOp::Not => Type::Bool,
                        UnaryOp::Neg => value_types.get(val).copied().unwrap_or(Type::I32),
                    },
                    Instruction::Call { func: target_name, .. } => {
                        func_return_types.get(target_name).copied().unwrap_or(Type::I32)
                    }
                };
                value_types.insert(*def_val, ty);
            }
        }

        // Setup Cranelift function context
        let mut sig = cl_module.make_signature();
        for (_, p_ty) in &func.params {
            sig.params.push(AbiParam::new(map_type(*p_ty)));
        }
        if func.return_type != Type::Unit {
            sig.returns.push(AbiParam::new(map_type(func.return_type)));
        }
        ctx.func = Function::with_name_signature(UserFuncName::user(0, func_id.as_u32()), sig);

        let mut builder = FunctionBuilder::new(&mut ctx.func, &mut fn_builder_ctx);

        // Pre-create all Cranelift basic blocks
        let mut block_map: HashMap<BlockId, cranelift_codegen::ir::Block> = HashMap::new();
        for block in &func.blocks {
            let cl_block = builder.create_block();
            block_map.insert(block.id, cl_block);
        }

        let mut val_map: HashMap<Value, cranelift_codegen::ir::Value> = HashMap::new();

        // Setup block parameters
        for (i, block) in func.blocks.iter().enumerate() {
            let cl_block = block_map[&block.id];
            if i == 0 {
                // Entry block receives function parameters
                builder.append_block_params_for_function_params(cl_block);
                let cl_params = builder.block_params(cl_block);
                for (param_idx, (p_val, _)) in func.params.iter().enumerate() {
                    val_map.insert(*p_val, cl_params[param_idx]);
                }
            } else {
                // Non-entry block parameters
                for (p_val, p_ty) in &block.params {
                    let cl_param = builder.append_block_param(cl_block, map_type(*p_ty));
                    val_map.insert(*p_val, cl_param);
                }
            }
        }

        // Translate instructions and terminators in each block
        for block in &func.blocks {
            let cl_block = block_map[&block.id];
            builder.switch_to_block(cl_block);

            for (def_val, inst) in &block.instructions {
                let cl_val = match inst {
                    Instruction::ConstInt(val, ty) => {
                        builder.ins().iconst(map_type(*ty), *val)
                    }
                    Instruction::ConstFloat(val) => {
                        builder.ins().f64const(*val)
                    }
                    Instruction::ConstBool(val) => {
                        builder.ins().iconst(types::I8, if *val { 1 } else { 0 })
                    }
                    Instruction::Binary { op, lhs, rhs } => {
                        let l = *val_map.get(lhs).ok_or(CodegenError::UndefinedValue(*lhs))?;
                        let r = *val_map.get(rhs).ok_or(CodegenError::UndefinedValue(*rhs))?;
                        let operand_ty = *value_types.get(lhs).ok_or(CodegenError::UndefinedValue(*lhs))?;

                        match op {
                            BinaryOp::Add => match operand_ty {
                                Type::F64 => builder.ins().fadd(l, r),
                                _ => builder.ins().iadd(l, r),
                            },
                            BinaryOp::Sub => match operand_ty {
                                Type::F64 => builder.ins().fsub(l, r),
                                _ => builder.ins().isub(l, r),
                            },
                            BinaryOp::Mul => match operand_ty {
                                Type::F64 => builder.ins().fmul(l, r),
                                _ => builder.ins().imul(l, r),
                            },
                            BinaryOp::Div => match operand_ty {
                                Type::I32 | Type::I64 => builder.ins().sdiv(l, r),
                                Type::U32 | Type::U64 => builder.ins().udiv(l, r),
                                Type::F64 => builder.ins().fdiv(l, r),
                                other => return Err(CodegenError::UnsupportedOperation(format!("division on {}", other))),
                            },
                            BinaryOp::Rem => match operand_ty {
                                Type::I32 | Type::I64 => builder.ins().srem(l, r),
                                Type::U32 | Type::U64 => builder.ins().urem(l, r),
                                other => return Err(CodegenError::UnsupportedOperation(format!("remainder on {}", other))),
                            },
                            BinaryOp::Eq => match operand_ty {
                                Type::F64 => builder.ins().fcmp(FloatCC::Equal, l, r),
                                _ => builder.ins().icmp(IntCC::Equal, l, r),
                            },
                            BinaryOp::Ne => match operand_ty {
                                Type::F64 => builder.ins().fcmp(FloatCC::NotEqual, l, r),
                                _ => builder.ins().icmp(IntCC::NotEqual, l, r),
                            },
                            BinaryOp::Lt => match operand_ty {
                                Type::I32 | Type::I64 => builder.ins().icmp(IntCC::SignedLessThan, l, r),
                                Type::U32 | Type::U64 | Type::Bool => builder.ins().icmp(IntCC::UnsignedLessThan, l, r),
                                Type::F64 => builder.ins().fcmp(FloatCC::LessThan, l, r),
                                other => return Err(CodegenError::UnsupportedOperation(format!("less-than on {}", other))),
                            },
                            BinaryOp::Le => match operand_ty {
                                Type::I32 | Type::I64 => builder.ins().icmp(IntCC::SignedLessThanOrEqual, l, r),
                                Type::U32 | Type::U64 | Type::Bool => builder.ins().icmp(IntCC::UnsignedLessThanOrEqual, l, r),
                                Type::F64 => builder.ins().fcmp(FloatCC::LessThanOrEqual, l, r),
                                other => return Err(CodegenError::UnsupportedOperation(format!("less-than-or-equal on {}", other))),
                            },
                            BinaryOp::Gt => match operand_ty {
                                Type::I32 | Type::I64 => builder.ins().icmp(IntCC::SignedGreaterThan, l, r),
                                Type::U32 | Type::U64 | Type::Bool => builder.ins().icmp(IntCC::UnsignedGreaterThan, l, r),
                                Type::F64 => builder.ins().fcmp(FloatCC::GreaterThan, l, r),
                                other => return Err(CodegenError::UnsupportedOperation(format!("greater-than on {}", other))),
                            },
                            BinaryOp::Ge => match operand_ty {
                                Type::I32 | Type::I64 => builder.ins().icmp(IntCC::SignedGreaterThanOrEqual, l, r),
                                Type::U32 | Type::U64 | Type::Bool => builder.ins().icmp(IntCC::UnsignedGreaterThanOrEqual, l, r),
                                Type::F64 => builder.ins().fcmp(FloatCC::GreaterThanOrEqual, l, r),
                                other => return Err(CodegenError::UnsupportedOperation(format!("greater-than-or-equal on {}", other))),
                            },
                            BinaryOp::And => builder.ins().band(l, r),
                            BinaryOp::Or => builder.ins().bor(l, r),
                        }
                    }
                    Instruction::Unary { op, val } => {
                        let v = *val_map.get(val).ok_or(CodegenError::UndefinedValue(*val))?;
                        let operand_ty = *value_types.get(val).ok_or(CodegenError::UndefinedValue(*val))?;

                        match op {
                            UnaryOp::Neg => match operand_ty {
                                Type::F64 => builder.ins().fneg(v),
                                _ => builder.ins().ineg(v),
                            },
                            UnaryOp::Not => {
                                // Robust boolean inversion for 0/1 representation
                                builder.ins().icmp_imm(IntCC::Equal, v, 0)
                            }
                        }
                    }
                    Instruction::Call { func: target_name, args } => {
                        let target_id = *func_ids.get(target_name).ok_or_else(|| {
                            CodegenError::UnknownFunction(target_name.clone())
                        })?;
                        let local_callee = cl_module.declare_func_in_func(target_id, &mut builder.func);

                        let mut arg_vals = Vec::with_capacity(args.len());
                        for arg in args {
                            let val = *val_map.get(arg).ok_or(CodegenError::UndefinedValue(*arg))?;
                            arg_vals.push(val);
                        }

                        let call_inst = builder.ins().call(local_callee, &arg_vals);
                        let results = builder.inst_results(call_inst);
                        if results.is_empty() {
                            // Call returns unit; produce a dummy iconst for value registration
                            builder.ins().iconst(types::I32, 0)
                        } else {
                            results[0]
                        }
                    }
                };

                val_map.insert(*def_val, cl_val);
            }

            // Translate terminator
            match &block.terminator {
                Terminator::Return(opt_val) => {
                    if let Some(val) = opt_val {
                        let cl_val = *val_map.get(val).ok_or(CodegenError::UndefinedValue(*val))?;
                        builder.ins().return_(&[cl_val]);
                    } else {
                        builder.ins().return_(&[]);
                    }
                }
                Terminator::Branch { target, args } => {
                    let cl_target = *block_map.get(target).ok_or(CodegenError::UnknownBlock(*target))?;
                    let mut cl_args: Vec<BlockArg> = Vec::with_capacity(args.len());
                    for arg in args {
                        let val = *val_map.get(arg).ok_or(CodegenError::UndefinedValue(*arg))?;
                        cl_args.push(BlockArg::from(val));
                    }
                    builder.ins().jump(cl_target, &cl_args);
                }
                Terminator::CondBranch {
                    cond,
                    then_block,
                    then_args,
                    else_block,
                    else_args,
                } => {
                    let cl_cond = *val_map.get(cond).ok_or(CodegenError::UndefinedValue(*cond))?;
                    let cl_then = *block_map.get(then_block).ok_or(CodegenError::UnknownBlock(*then_block))?;
                    let cl_else = *block_map.get(else_block).ok_or(CodegenError::UnknownBlock(*else_block))?;

                    let mut cl_then_args: Vec<BlockArg> = Vec::with_capacity(then_args.len());
                    for arg in then_args {
                        let val = *val_map.get(arg).ok_or(CodegenError::UndefinedValue(*arg))?;
                        cl_then_args.push(BlockArg::from(val));
                    }

                    let mut cl_else_args: Vec<BlockArg> = Vec::with_capacity(else_args.len());
                    for arg in else_args {
                        let val = *val_map.get(arg).ok_or(CodegenError::UndefinedValue(*arg))?;
                        cl_else_args.push(BlockArg::from(val));
                    }

                    builder.ins().brif(cl_cond, cl_then, &cl_then_args, cl_else, &cl_else_args);
                }
            }
        }

        builder.seal_all_blocks();
        builder.finalize();

        cl_module.define_function(func_id, &mut ctx)?;
        cl_module.clear_context(&mut ctx);
    }

    let product = cl_module.finish();
    let obj_bytes = product.emit().map_err(|e| CodegenError::Cranelift(e.to_string()))?;
    Ok(obj_bytes)
}
