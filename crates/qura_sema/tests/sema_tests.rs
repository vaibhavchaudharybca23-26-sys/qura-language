use qura_parser::parse_source;
use qura_sema::{analyze, Type};

fn check_source(src: &str) -> Result<qura_sema::SymbolTable, Vec<qura_diagnostics::Diagnostic>> {
    let program = parse_source(src).expect("Source code failed to parse");
    analyze(&program)
}

// ============================================================================
// POSITIVE TESTS
// ============================================================================

#[test]
fn test_primitive_types() {
    let src = "
    fn test_primitives() {
        let a: i32 = 10;
        let b: i64 = 20;
        let c: u32 = 30;
        let d: u64 = 40;
        let e: f64 = 50.5;
        let f: bool = true;
    }
    ";
    let symbols = check_source(src).expect("Semantic check failed for primitive types");
    assert!(symbols.lookup_function("test_primitives").is_some());
}

#[test]
fn test_type_inference() {
    let src = "
    fn test_inference() -> i32 {
        let x = 100;           // infers i32
        let y = 3.14;          // infers f64
        let flag = false;      // infers bool
        let sum = x + 50;      // infers i32
        return sum;
    }
    ";
    let symbols = check_source(src).expect("Type inference should succeed");
    let fn_sym = symbols.lookup_function("test_inference").unwrap();
    assert_eq!(fn_sym.return_type, Type::I32);
}

#[test]
fn test_mutable_assignments() {
    let src = "
    fn test_mut(mut val: i32) -> i32 {
        val = val + 1;
        let mut local = 10;
        local = 20;
        local = local * val;
        return local;
    }
    ";
    let _ = check_source(src).expect("Mutable variable assignments should succeed");
}

#[test]
fn test_lexical_scoping_and_nesting() {
    let src = "
    fn test_scopes() -> i32 {
        let outer = 10;
        let mut result = 0;
        if outer > 5 {
            let inner = 20;
            result = outer + inner;
        }
        return result;
    }
    ";
    let _ = check_source(src).expect("Valid nested scopes should succeed");
}

#[test]
fn test_forward_function_calls() {
    let src = "
    fn caller() -> i32 {
        return callee(5);
    }

    fn callee(x: i32) -> i32 {
        return x * 2;
    }
    ";
    let _ = check_source(src).expect("Forward function calls should resolve");
}

#[test]
fn test_canonical_factorial_passes_sema() {
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
    let _ = check_source(src).expect("Canonical factorial program should pass semantic check");
}

#[test]
fn test_canonical_collatz_passes_sema() {
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
    let _ = check_source(src).expect("Canonical collatz program should pass semantic check");
}

// ============================================================================
// NEGATIVE TESTS (ERROR DIAGNOSTICS VERIFICATION)
// ============================================================================

#[test]
fn test_negative_assign_to_immutable() {
    let src = "
    fn test_immut() {
        let x = 10;
        x = 20;
    }
    ";
    let errs = check_source(src).expect_err("Expected error when assigning to immutable binding");
    assert!(
        errs.iter().any(|d| d.code == Some("E0102")),
        "Expected E0102 (cannot assign to immutable variable), found {:?}",
        errs
    );
}

#[test]
fn test_negative_assign_to_immutable_param() {
    let src = "
    fn test_immut_param(x: i32) {
        x = 20;
    }
    ";
    let errs = check_source(src).expect_err("Expected error when assigning to immutable param");
    assert!(
        errs.iter().any(|d| d.code == Some("E0102")),
        "Expected E0102 (cannot assign to immutable variable), found {:?}",
        errs
    );
}

#[test]
fn test_negative_type_mismatch_let() {
    let src = "
    fn test_mismatch() {
        let x: i32 = true;
    }
    ";
    let errs = check_source(src).expect_err("Expected type mismatch in let binding");
    assert!(
        errs.iter().any(|d| d.code == Some("E0101")),
        "Expected E0101 (type mismatch), found {:?}",
        errs
    );
}

#[test]
fn test_negative_type_mismatch_assign() {
    let src = "
    fn test_assign_mismatch() {
        let mut x: i32 = 10;
        x = false;
    }
    ";
    let errs = check_source(src).expect_err("Expected type mismatch in assignment");
    assert!(
        errs.iter().any(|d| d.code == Some("E0101")),
        "Expected E0101 (type mismatch), found {:?}",
        errs
    );
}

#[test]
fn test_negative_unresolved_variable() {
    let src = "
    fn test_unresolved() -> i32 {
        let x = nonexistent_var + 1;
        return x;
    }
    ";
    let errs = check_source(src).expect_err("Expected unresolved variable error");
    assert!(
        errs.iter().any(|d| d.code == Some("E0103")),
        "Expected E0103 (unresolved variable), found {:?}",
        errs
    );
}

#[test]
fn test_negative_unresolved_function() {
    let src = "
    fn test_unresolved_fn() -> i32 {
        return nonexistent_fn(42);
    }
    ";
    let errs = check_source(src).expect_err("Expected unresolved function error");
    assert!(
        errs.iter().any(|d| d.code == Some("E0103")),
        "Expected E0103 (unresolved function), found {:?}",
        errs
    );
}

#[test]
fn test_negative_variable_shadowing_same_scope() {
    let src = "
    fn test_shadowing() {
        let x = 1;
        let x = 2;
    }
    ";
    let errs = check_source(src).expect_err("Expected error for variable shadowing in same scope");
    assert!(
        errs.iter().any(|d| d.code == Some("E0104")),
        "Expected E0104 (variable already defined in this scope), found {:?}",
        errs
    );
}

#[test]
fn test_negative_duplicate_function_names() {
    let src = "
    fn dup() {}
    fn dup() {}
    ";
    let errs = check_source(src).expect_err("Expected duplicate function definition error");
    assert!(
        errs.iter().any(|d| d.code == Some("E0104")),
        "Expected E0104 (duplicate function definition), found {:?}",
        errs
    );
}

#[test]
fn test_negative_function_arg_count_mismatch() {
    let src = "
    fn add(a: i32, b: i32) -> i32 {
        return a + b;
    }

    fn caller() -> i32 {
        return add(1);
    }
    ";
    let errs = check_source(src).expect_err("Expected argument count mismatch error");
    assert!(
        errs.iter().any(|d| d.code == Some("E0105")),
        "Expected E0105 (argument count mismatch), found {:?}",
        errs
    );
}

#[test]
fn test_negative_function_arg_type_mismatch() {
    let src = "
    fn add(a: i32, b: i32) -> i32 {
        return a + b;
    }

    fn caller() -> i32 {
        return add(1, true);
    }
    ";
    let errs = check_source(src).expect_err("Expected argument type mismatch error");
    assert!(
        errs.iter().any(|d| d.code == Some("E0105")),
        "Expected E0105 (argument type mismatch), found {:?}",
        errs
    );
}

#[test]
fn test_negative_return_type_mismatch() {
    let src = "
    fn bad_return() -> i32 {
        return false;
    }
    ";
    let errs = check_source(src).expect_err("Expected return type mismatch error");
    assert!(
        errs.iter().any(|d| d.code == Some("E0106")),
        "Expected E0106 (mismatched return type), found {:?}",
        errs
    );
}

#[test]
fn test_negative_while_condition_not_bool() {
    let src = "
    fn bad_while() {
        while 42 {
            let x = 1;
        }
    }
    ";
    let errs = check_source(src).expect_err("Expected while condition error");
    assert!(
        errs.iter().any(|d| d.code == Some("E0107")),
        "Expected E0107 (condition must be bool), found {:?}",
        errs
    );
}

#[test]
fn test_negative_if_condition_not_bool() {
    let src = "
    fn bad_if() {
        if 100 {
            let x = 1;
        }
    }
    ";
    let errs = check_source(src).expect_err("Expected if condition error");
    assert!(
        errs.iter().any(|d| d.code == Some("E0107")),
        "Expected E0107 (condition must be bool), found {:?}",
        errs
    );
}

#[test]
fn test_negative_if_branches_type_mismatch() {
    let src = "
    fn branch_mismatch() -> i32 {
        let x = if true {
            10
        } else {
            false
        };
        return 0;
    }
    ";
    let errs = check_source(src).expect_err("Expected branch type mismatch error");
    assert!(
        errs.iter().any(|d| d.code == Some("E0101")),
        "Expected E0101 (incompatible branch types), found {:?}",
        errs
    );
}

#[test]
fn test_negative_invalid_unary_operator() {
    let src = "
    fn bad_unary() {
        let a = -true;
        let b = !42;
    }
    ";
    let errs = check_source(src).expect_err("Expected unary operator errors");
    assert!(
        errs.iter().any(|d| d.code == Some("E0108")),
        "Expected E0108 (invalid unary operator), found {:?}",
        errs
    );
}

#[test]
fn test_negative_mismatched_binary_operands() {
    let src = "
    fn bad_binary() {
        let x = 10 + 3.14;
    }
    ";
    let errs = check_source(src).expect_err("Expected binary operand mismatch");
    assert!(
        errs.iter().any(|d| d.code == Some("E0101")),
        "Expected E0101 (mismatched operand types), found {:?}",
        errs
    );
}
