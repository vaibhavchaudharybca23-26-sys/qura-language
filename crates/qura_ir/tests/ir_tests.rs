use qura_ir::{compile_and_verify, verify_module, BasicBlock, BlockId, Function, Instruction, Module, Terminator, Type, Value};
use qura_parser::parse_source;
use qura_sema::analyze;

fn lower_and_verify(src: &str) -> Module {
    let program = parse_source(src).expect("Failed to parse source");
    let symbols = analyze(&program).expect("Semantic check failed");
    compile_and_verify(&program, &symbols).expect("QIR lowering or verification failed")
}

// ============================================================================
// STRUCTURAL & CANONICAL TESTS
// ============================================================================

#[test]
fn test_qir_lowering_arithmetic() {
    let src = "
    fn calc(a: i32, b: i32) -> i32 {
        let c = a + b * 2;
        return c - 1;
    }
    ";
    let module = lower_and_verify(src);
    assert_eq!(module.functions.len(), 1);
    let func = &module.functions[0];
    assert_eq!(func.name, "calc");
    assert_eq!(func.return_type, Type::I32);
    assert_eq!(func.params.len(), 2);
    // Entry block only (straight line)
    assert_eq!(func.blocks.len(), 1);
    let entry = &func.blocks[0];
    assert!(matches!(entry.terminator, Terminator::Return(Some(_))));
}

#[test]
fn test_qir_lowering_branching_if_else() {
    let src = "
    fn max(a: i32, b: i32) -> i32 {
        let result = if a > b {
            a
        } else {
            b
        };
        return result;
    }
    ";
    let module = lower_and_verify(src);
    let func = &module.functions[0];
    assert_eq!(func.name, "max");
    // Should have: entry (with cond_br), then_block, else_block, merge_block
    assert!(func.blocks.len() >= 4);
    let entry = &func.blocks[0];
    assert!(matches!(entry.terminator, Terminator::CondBranch { .. }));
}

#[test]
fn test_qir_lowering_while_loop() {
    let src = "
    fn count_down(mut n: i32) -> i32 {
        while n > 0 {
            n = n - 1;
        }
        return n;
    }
    ";
    let module = lower_and_verify(src);
    let func = &module.functions[0];
    assert_eq!(func.name, "count_down");
    // Should have: entry, loop_header, loop_body, loop_exit
    assert!(func.blocks.len() >= 4);

    // Loop header must take SSA parameter for mutable 'n'
    let header = &func.blocks[1];
    assert_eq!(header.params.len(), 1);
    assert_eq!(header.params[0].1, Type::I32);
    assert!(matches!(header.terminator, Terminator::CondBranch { .. }));
}

#[test]
fn test_qir_canonical_factorial() {
    let src = "
    fn factorial(n: i32) -> i32 {
        let mut result: i32 = 1;
        let mut i: i32 = 1;
        while i <= n {
            result = result * i;
            i = i + 1;
        }
        return result;
    }

    fn main() -> i32 {
        let answer = factorial(5);
        return answer - 120;
    }
    ";
    let module = lower_and_verify(src);
    assert_eq!(module.functions.len(), 2);

    // 1. Structural assertions on @factorial
    let fact_fn = &module.functions[0];
    assert_eq!(fact_fn.name, "factorial");
    assert_eq!(fact_fn.return_type, Type::I32);
    assert_eq!(fact_fn.params.len(), 1); // (%0, i32)
    assert_eq!(fact_fn.blocks.len(), 4); // entry, header, body, exit

    // Block 0: entry block initializes result=1 and i=1, branches to header with [1, 1]
    let entry = &fact_fn.blocks[0];
    assert_eq!(entry.id, BlockId(0));
    assert!(entry.params.is_empty());
    assert!(matches!(entry.terminator, Terminator::Branch { target: BlockId(1), ref args } if args.len() == 2));

    // Block 1: loop header takes [i, result] parameters
    let header = &fact_fn.blocks[1];
    assert_eq!(header.id, BlockId(1));
    assert_eq!(header.params.len(), 2);
    assert_eq!(header.params[0].1, Type::I32);
    assert_eq!(header.params[1].1, Type::I32);
    // Header evaluates `i <= n` and branches to body (Block 2) or exit (Block 3)
    if let Terminator::CondBranch { then_block, else_block, else_args, .. } = &header.terminator {
        assert_eq!(*then_block, BlockId(2));
        assert_eq!(*else_block, BlockId(3));
        assert_eq!(else_args.len(), 2);
    } else {
        panic!("expected cond_br at loop header");
    }

    // Block 2: body multiplies result * i, increments i, and branches back to header (Block 1)
    let body = &fact_fn.blocks[2];
    assert_eq!(body.id, BlockId(2));
    assert!(matches!(body.terminator, Terminator::Branch { target: BlockId(1), ref args } if args.len() == 2));

    // Block 3: exit block takes [i, result], returns result
    let exit = &fact_fn.blocks[3];
    assert_eq!(exit.id, BlockId(3));
    assert_eq!(exit.params.len(), 2);
    assert!(matches!(exit.terminator, Terminator::Return(Some(_))));

    // 2. Structural assertions on @main
    let main_fn = &module.functions[1];
    assert_eq!(main_fn.name, "main");
    assert_eq!(main_fn.return_type, Type::I32);
    assert_eq!(main_fn.blocks.len(), 1);
    let main_entry = &main_fn.blocks[0];
    assert!(main_entry.instructions.iter().any(|(_, inst)| {
        matches!(inst, Instruction::Call { func, args } if func == "factorial" && args.len() == 1)
    }));
    assert!(matches!(main_entry.terminator, Terminator::Return(Some(_))));
}

#[test]
fn test_qir_canonical_collatz() {
    let src = "
    fn collatz_steps(mut n: i32) -> i32 {
        let mut steps: i32 = 0;
        while n > 1 {
            if n % 2 == 0 {
                n = n / 2;
            } else {
                n = 3 * n + 1;
            }
            steps = steps + 1;
        }
        return steps;
    }

    fn main() -> i32 {
        let steps_for_27 = collatz_steps(27);
        return steps_for_27;
    }
    ";
    let module = lower_and_verify(src);
    assert_eq!(module.functions.len(), 2);

    let collatz_fn = &module.functions[0];
    assert_eq!(collatz_fn.name, "collatz_steps");
    assert_eq!(collatz_fn.return_type, Type::I32);
    assert_eq!(collatz_fn.params.len(), 1);

    // CFG contains: entry (0), loop_header (1), loop_body (2), if_then (3), if_else (4), if_merge (5), loop_exit (6)
    assert_eq!(collatz_fn.blocks.len(), 7);

    // Header tracks mut n and mut steps
    let header = &collatz_fn.blocks[1];
    assert_eq!(header.params.len(), 2);

    // Main calls collatz_steps(27)
    let main_fn = &module.functions[1];
    assert_eq!(main_fn.name, "main");
    let main_entry = &main_fn.blocks[0];
    assert!(main_entry.instructions.iter().any(|(_, inst)| {
        matches!(inst, Instruction::Call { func, args } if func == "collatz_steps" && args.len() == 1)
    }));
}

#[test]
fn test_qir_early_return_in_branches() {
    let src = "
    fn early_ret(x: i32) -> i32 {
        if x > 100 {
            return 1;
        }
        return 2;
    }
    ";
    let module = lower_and_verify(src);
    println!("{}", module);
    let func = &module.functions[0];
    assert_eq!(func.name, "early_ret");
    // Entry has cond_br, then has return, else has branch to merge, merge has return
    assert_eq!(func.blocks.len(), 4);
}

#[test]
fn test_qir_loop_with_internal_mut_var() {
    let src = "
    fn two_loops() -> i32 {
        let mut total = 0;
        let mut i = 0;
        while i < 5 {
            let mut inner_var = i;
            inner_var = inner_var + 10;
            total = total + inner_var;
            i = i + 1;
        }
        let mut j = 0;
        while j < 5 {
            j = j + 1;
        }
        return total;
    }
    ";
    let module = lower_and_verify(src);
    println!("{}", module);
}

// ============================================================================
// VERIFIER TESTS (NEGATIVE SSA & DOMINANCE INVARIANTS)
// ============================================================================

#[test]
fn test_verifier_rejects_undefined_value() {
    let mut module = Module::new("test");
    let func = Function {
        name: "bad".to_string(),
        params: Vec::new(),
        return_type: Type::I32,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            instructions: Vec::new(),
            terminator: Terminator::Return(Some(Value(999))), // Undefined Value 999!
        }],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for undefined value");
    assert!(errs.iter().any(|e| e.message.contains("use of undefined value %999")));
}

#[test]
fn test_verifier_rejects_ssa_redefinition() {
    let mut module = Module::new("test");
    let func = Function {
        name: "duplicate_def".to_string(),
        params: Vec::new(),
        return_type: Type::I32,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            instructions: vec![
                (Value(0), Instruction::ConstInt(10, Type::I32)),
                (Value(0), Instruction::ConstInt(20, Type::I32)), // Value(0) defined twice!
            ],
            terminator: Terminator::Return(Some(Value(0))),
        }],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for duplicate value definition");
    assert!(errs.iter().any(|e| e.message.contains("SSA violation: value %0 defined more than once")));
}

#[test]
fn test_verifier_rejects_dominance_violation() {
    let mut module = Module::new("test");
    // Value %0 is defined in block1, but block0 tries to use it!
    let func = Function {
        name: "dom_violation".to_string(),
        params: Vec::new(),
        return_type: Type::I32,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                instructions: Vec::new(),
                terminator: Terminator::Branch {
                    target: BlockId(1),
                    args: vec![Value(0)], // Value(0) is not defined yet in block0!
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: vec![(Value(1), Type::I32)],
                instructions: vec![(Value(0), Instruction::ConstInt(42, Type::I32))],
                terminator: Terminator::Return(Some(Value(1))),
            },
        ],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for dominance violation");
    assert!(errs.iter().any(|e| e.message.contains("use of undefined value %0") || e.message.contains("dominance violation")));
}

#[test]
fn test_verifier_rejects_block_argument_count_mismatch() {
    let mut module = Module::new("test");
    let func = Function {
        name: "arg_mismatch".to_string(),
        params: Vec::new(),
        return_type: Type::I32,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                instructions: vec![(Value(0), Instruction::ConstInt(1, Type::I32))],
                terminator: Terminator::Branch {
                    target: BlockId(1),
                    args: vec![Value(0)], // Passes 1 arg
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: vec![
                    (Value(1), Type::I32),
                    (Value(2), Type::I32), // Block expects 2 args!
                ],
                instructions: Vec::new(),
                terminator: Terminator::Return(Some(Value(1))),
            },
        ],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected error for argument count mismatch");
    assert!(errs.iter().any(|e| e.message.contains("passes 1 argument(s), but block expects 2")));
}

#[test]
fn test_verifier_rejects_non_boolean_condition() {
    let mut module = Module::new("test");
    let func = Function {
        name: "bad_cond".to_string(),
        params: Vec::new(),
        return_type: Type::Unit,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                instructions: vec![(Value(0), Instruction::ConstInt(123, Type::I32))],
                terminator: Terminator::CondBranch {
                    cond: Value(0), // Type is I32, not Bool!
                    then_block: BlockId(1),
                    then_args: Vec::new(),
                    else_block: BlockId(1),
                    else_args: Vec::new(),
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: Vec::new(),
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            },
        ],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected error for non-bool cond_br");
    assert!(errs.iter().any(|e| e.message.contains("cond_br condition must be bool, found i32")));
}

#[test]
fn test_verifier_rejects_unreachable_block() {
    let mut module = Module::new("test");
    let func = Function {
        name: "unreachable_block".to_string(),
        params: Vec::new(),
        return_type: Type::Unit,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            },
            BasicBlock {
                id: BlockId(1), // Block 1 is unreachable!
                params: Vec::new(),
                instructions: Vec::new(),
                terminator: Terminator::Return(None),
            },
        ],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for unreachable block");
    assert!(errs.iter().any(|e| e.message.contains("is unreachable from entry")));
}

#[test]
fn test_verifier_rejects_entry_block_parameters() {
    let mut module = Module::new("test");
    let func = Function {
        name: "entry_with_params".to_string(),
        params: Vec::new(),
        return_type: Type::Unit,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: vec![(Value(0), Type::I32)], // Entry block cannot have block params!
            instructions: Vec::new(),
            terminator: Terminator::Return(None),
        }],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for entry block parameters");
    assert!(errs.iter().any(|e| e.message.contains("entry block cannot have block parameters")));
}

#[test]
fn test_verifier_rejects_branch_to_entry_block() {
    let mut module = Module::new("test");
    let func = Function {
        name: "loop_to_entry".to_string(),
        params: Vec::new(),
        return_type: Type::Unit,
        blocks: vec![
            BasicBlock {
                id: BlockId(0),
                params: Vec::new(),
                instructions: Vec::new(),
                terminator: Terminator::Branch {
                    target: BlockId(1),
                    args: Vec::new(),
                },
            },
            BasicBlock {
                id: BlockId(1),
                params: Vec::new(),
                instructions: Vec::new(),
                terminator: Terminator::Branch {
                    target: BlockId(0), // Cannot branch to entry block!
                    args: Vec::new(),
                },
            },
        ],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for branching to entry");
    assert!(errs.iter().any(|e| e.message.contains("cannot branch to entry block")));
}

#[test]
fn test_verifier_rejects_binary_operand_type_mismatch() {
    let mut module = Module::new("test");
    let func = Function {
        name: "type_mismatch_inst".to_string(),
        params: Vec::new(),
        return_type: Type::I32,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            instructions: vec![
                (Value(0), Instruction::ConstInt(10, Type::I32)),
                (Value(1), Instruction::ConstBool(true)),
                // Add %0 (i32) and %1 (bool) -> Mismatched operand types!
                (
                    Value(2),
                    Instruction::Binary {
                        op: qura_ir::BinaryOp::Add,
                        lhs: Value(0),
                        rhs: Value(1),
                    },
                ),
            ],
            terminator: Terminator::Return(Some(Value(0))),
        }],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for mismatched binary operand types");
    assert!(errs.iter().any(|e| e.message.contains("binary 'add' operand type mismatch")));
}

#[test]
fn test_qir_lowering_string_constant() {
    let src = "
    fn test_strings() {
        let a = \"hello\";
        let b: str = \"world\\n\";
    }
    ";
    let module = lower_and_verify(src);
    let func = &module.functions[0];
    let entry = &func.blocks[0];

    let has_hello = entry.instructions.iter().any(|(_, inst)| {
        matches!(inst, Instruction::ConstStr(s) if s == "hello")
    });
    let has_world = entry.instructions.iter().any(|(_, inst)| {
        matches!(inst, Instruction::ConstStr(s) if s == "world\n")
    });

    assert!(has_hello, "Expected ConstStr(\"hello\") in QIR");
    assert!(has_world, "Expected ConstStr(\"world\\n\") in QIR");
}

#[test]
fn test_qir_lowering_print_and_println() {
    let src = "
    fn main() {
        print(\"hello\");
        println(\"world\");
    }
    ";
    let module = lower_and_verify(src);
    let func = &module.functions[0];
    let entry = &func.blocks[0];

    let has_print = entry.instructions.iter().any(|(_, inst)| {
        matches!(inst, Instruction::Print { newline: false, .. })
    });
    let has_println = entry.instructions.iter().any(|(_, inst)| {
        matches!(inst, Instruction::Print { newline: true, .. })
    });

    assert!(has_print, "Expected Print {{ newline: false }} in QIR");
    assert!(has_println, "Expected Print {{ newline: true }} in QIR");
}

#[test]
fn test_qir_lowering_print_all_printable_types() {
    let src = "
    fn main() {
        print(\"string\");
        println(42);
        let a: i64 = 100;
        print(a);
        let b: u32 = 200;
        println(b);
        let c: u64 = 300;
        print(c);
        println(true);
        print(false);
    }
    ";
    let module = lower_and_verify(src);
    let func = &module.functions[0];
    let entry = &func.blocks[0];

    let print_count = entry.instructions.iter().filter(|(_, inst)| {
        matches!(inst, Instruction::Print { .. })
    }).count();

    assert_eq!(print_count, 7, "Expected 7 Print instructions in QIR");
}

#[test]
fn test_qir_user_function_named_print_lowers_to_call() {
    let src = "
    fn print(x: i32) -> i32 {
        return x * 2;
    }

    fn main() -> i32 {
        return print(21);
    }
    ";
    let module = lower_and_verify(src);
    let main_func = module.functions.iter().find(|f| f.name == "main").unwrap();
    let entry = &main_func.blocks[0];

    let has_call = entry.instructions.iter().any(|(_, inst)| {
        matches!(inst, Instruction::Call { func, .. } if func == "print")
    });
    let has_builtin_print = entry.instructions.iter().any(|(_, inst)| {
        matches!(inst, Instruction::Print { .. })
    });

    assert!(has_call, "Expected user function 'print' to lower to Instruction::Call");
    assert!(!has_builtin_print, "User function 'print' must not lower to Instruction::Print");
}

#[test]
fn test_qir_str_parameter_and_return() {
    let src = "
    fn echo(s: str) -> str {
        return s;
    }

    fn main() {
        let msg = echo(\"test\");
        println(msg);
    }
    ";
    let module = lower_and_verify(src);
    let echo_func = module.functions.iter().find(|f| f.name == "echo").unwrap();
    assert_eq!(echo_func.return_type, Type::Str);
    assert_eq!(echo_func.params[0].1, Type::Str);
}

#[test]
fn test_verifier_rejects_print_unprintable_type_f64() {
    let mut module = Module::new("test");
    let func = Function {
        name: "bad_print_f64".to_string(),
        params: Vec::new(),
        return_type: Type::Unit,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            instructions: vec![
                (Value(0), Instruction::ConstFloat(3.14)),
                (Value(1), Instruction::Print { val: Value(0), newline: true }),
            ],
            terminator: Terminator::Return(None),
        }],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for printing f64");
    assert!(errs.iter().any(|e| e.message.contains("print operand must be a printable type")));
}

#[test]
fn test_verifier_rejects_print_unprintable_type_unit() {
    let mut module = Module::new("test");
    let func = Function {
        name: "bad_print_unit".to_string(),
        params: Vec::new(),
        return_type: Type::Unit,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            instructions: vec![
                (Value(0), Instruction::ConstInt(0, Type::Unit)),
                (Value(1), Instruction::Print { val: Value(0), newline: false }),
            ],
            terminator: Terminator::Return(None),
        }],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for printing ()");
    assert!(errs.iter().any(|e| e.message.contains("print operand must be a printable type")));
}

#[test]
fn test_verifier_rejects_binary_op_on_str() {
    let mut module = Module::new("test");
    let func = Function {
        name: "bad_str_binary".to_string(),
        params: Vec::new(),
        return_type: Type::Unit,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            instructions: vec![
                (Value(0), Instruction::ConstStr("a".to_string())),
                (Value(1), Instruction::ConstStr("b".to_string())),
                (
                    Value(2),
                    Instruction::Binary {
                        op: qura_ir::BinaryOp::Add,
                        lhs: Value(0),
                        rhs: Value(1),
                    },
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for binary op on str");
    assert!(errs.iter().any(|e| e.message.contains("binary 'add' does not support operand type str")));
}

#[test]
fn test_verifier_rejects_unary_op_on_str() {
    let mut module = Module::new("test");
    let func = Function {
        name: "bad_str_unary".to_string(),
        params: Vec::new(),
        return_type: Type::Unit,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            instructions: vec![
                (Value(0), Instruction::ConstStr("a".to_string())),
                (
                    Value(1),
                    Instruction::Unary {
                        op: qura_ir::UnaryOp::Not,
                        val: Value(0),
                    },
                ),
            ],
            terminator: Terminator::Return(None),
        }],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for unary op on str");
    assert!(errs.iter().any(|e| e.message.contains("unary 'not' does not support operand type str")));
}

#[test]
fn test_verifier_rejects_print_use_before_def() {
    let mut module = Module::new("test");
    let func = Function {
        name: "print_use_before_def".to_string(),
        params: Vec::new(),
        return_type: Type::Unit,
        blocks: vec![BasicBlock {
            id: BlockId(0),
            params: Vec::new(),
            instructions: vec![
                // Use Value(1) before Value(1) is defined
                (Value(0), Instruction::Print { val: Value(1), newline: true }),
                (Value(1), Instruction::ConstStr("hello".to_string())),
            ],
            terminator: Terminator::Return(None),
        }],
    };
    module.functions.push(func);

    let errs = verify_module(&module).expect_err("Expected verifier error for use before def");
    assert!(errs.iter().any(|e| e.message.contains("used before its definition")));
}

