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

    let fact_fn = &module.functions[0];
    assert_eq!(fact_fn.name, "factorial");
    // Two mutable variables ('result', 'i') tracked across loop header
    let header = &fact_fn.blocks[1];
    assert_eq!(header.params.len(), 2);
    assert_eq!(header.params[0].1, Type::I32);
    assert_eq!(header.params[1].1, Type::I32);

    let main_fn = &module.functions[1];
    assert_eq!(main_fn.name, "main");
    assert_eq!(main_fn.return_type, Type::I32);
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
    // Verifier ran and established SSA correctness, valid CFG, and dominance!
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
