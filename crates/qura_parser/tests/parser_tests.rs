use qura_parser::parse_source;
use qura_syntax::ast::*;

// ============================================================================
// POSITIVE TESTS
// ============================================================================

#[test]
fn test_variable_declarations() {
    let src = "
    fn main() -> i32 {
        let x: i32 = 42;
        let mut y = 100;
        let is_valid: bool = true;
        let float_val: f64 = 3.14159;
        return 0;
    }
    ";
    let program = parse_source(src).expect("Failed to parse valid variable declarations");
    assert_eq!(program.items.len(), 1);

    let Item::Function(f) = &program.items[0];
    assert_eq!(f.name.name, "main");
    assert_eq!(f.body.stmts.len(), 5);

        // Check statement 1: let x: i32 = 42;
        if let Stmt::Let { name, is_mut, ty, init, .. } = &f.body.stmts[0] {
            assert_eq!(name.name, "x");
            assert!(!is_mut);
            assert!(matches!(ty, Some(TypeExpr::Named(ident)) if ident.name == "i32"));
            assert!(matches!(init, Expr::Literal(Literal::Int(42, _))));
        } else {
            panic!("expected let statement for x");
        }

        // Check statement 2: let mut y = 100;
        if let Stmt::Let { name, is_mut, ty, init, .. } = &f.body.stmts[1] {
            assert_eq!(name.name, "y");
            assert!(is_mut);
            assert!(ty.is_none());
            assert!(matches!(init, Expr::Literal(Literal::Int(100, _))));
        } else {
            panic!("expected let statement for y");
        }
    }

    #[test]
    fn test_function_syntax() {
        let src = "
        fn no_args() {
            return;
        }

        fn add(a: i32, mut b: i64) -> i64 {
            return a + b;
        }
        ";
        let program = parse_source(src).expect("Failed to parse valid functions");
        assert_eq!(program.items.len(), 2);

        // Function 1
        let Item::Function(f1) = &program.items[0];
        assert_eq!(f1.name.name, "no_args");
        assert_eq!(f1.params.len(), 0);
        assert!(f1.return_type.is_none());

        // Function 2
        let Item::Function(f2) = &program.items[1];
        assert_eq!(f2.name.name, "add");
        assert_eq!(f2.params.len(), 2);
        assert_eq!(f2.params[0].name.name, "a");
        assert!(!f2.params[0].is_mut);
        assert_eq!(f2.params[1].name.name, "b");
        assert!(f2.params[1].is_mut);
        assert!(matches!(&f2.return_type, Some(TypeExpr::Named(id)) if id.name == "i64"));
    }

    #[test]
    fn test_operator_precedence_arithmetic() {
        let src = "
        fn main() -> i32 {
            let res = 1 + 2 * 3;
            let group = (1 + 2) * 3;
            return 0;
        }
        ";
        let program = parse_source(src).expect("Failed to parse expressions");
        let Item::Function(f) = &program.items[0];
        // res = 1 + (2 * 3) -> Top-level is Add, right child is Mul
        if let Stmt::Let { init, .. } = &f.body.stmts[0] {
            if let Expr::Binary { op, left, right, .. } = init {
                assert_eq!(*op, BinaryOp::Add);
                assert!(matches!(**left, Expr::Literal(Literal::Int(1, _))));
                if let Expr::Binary { op: r_op, .. } = &**right {
                    assert_eq!(*r_op, BinaryOp::Mul);
                } else {
                    panic!("expected right operand to be a multiplication");
                }
            } else {
                panic!("expected binary addition at root");
            }
        }

        // group = (1 + 2) * 3 -> Top-level is Mul, left child is Grouping(Add)
        if let Stmt::Let { init, .. } = &f.body.stmts[1] {
            if let Expr::Binary { op, left, right, .. } = init {
                assert_eq!(*op, BinaryOp::Mul);
                assert!(matches!(**right, Expr::Literal(Literal::Int(3, _))));
                assert!(matches!(**left, Expr::Grouping { .. }));
            } else {
                panic!("expected binary multiplication at root");
            }
        }
    }

    #[test]
    fn test_operator_precedence_logical_and_comparison() {
        let src = "
        fn main() -> bool {
            let cond = a < b && c == d || !flag;
            return cond;
        }
        ";
        let program = parse_source(src).expect("Failed to parse logical expressions");
        let Item::Function(f) = &program.items[0];
        if let Stmt::Let { init, .. } = &f.body.stmts[0] {
            // Root should be || because || has lower precedence than &&
            if let Expr::Binary { op, left, right, .. } = init {
                assert_eq!(*op, BinaryOp::Or);

                // Left of || is (a < b && c == d)
                if let Expr::Binary { op: l_op, left: ll, right: lr, .. } = &**left {
                    assert_eq!(*l_op, BinaryOp::And);
                    assert!(matches!(**ll, Expr::Binary { op: BinaryOp::Lt, .. }));
                    assert!(matches!(**lr, Expr::Binary { op: BinaryOp::Eq, .. }));
                } else {
                    panic!("expected && on left of ||");
                }

                // Right of || is !flag
                assert!(matches!(**right, Expr::Unary { op: UnaryOp::Not, .. }));
            } else {
                panic!("expected || at root");
            }
        }
    }

#[test]
fn test_control_flow_and_blocks() {
    let src = "
    fn abs(x: i32) -> i32 {
        if x < 0 {
            return -x;
        } else {
            return x;
        }
    }

    fn loop_test() -> i32 {
        let mut i = 0;
        while i < 10 {
            i = i + 1;
        }
        return i;
    }
    ";
    let program = parse_source(src).expect("Failed to parse control flow");
    assert_eq!(program.items.len(), 2);
}

#[test]
fn test_canonical_factorial_program() {
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
    let program = parse_source(src).expect("Failed to parse canonical factorial program");
    assert_eq!(program.items.len(), 2);
}

#[test]
fn test_canonical_collatz_program() {
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
    let program = parse_source(src).expect("Failed to parse canonical collatz program");
    assert_eq!(program.items.len(), 2);
}

// ============================================================================
// NEGATIVE TESTS (MALFORMED INPUT & DIAGNOSTIC VERIFICATION)
// ============================================================================

#[test]
fn test_negative_malformed_let_missing_eq() {
    let src = "
    fn main() -> i32 {
        let x: i32 10;
        return 0;
    }
    ";
    let errs = parse_source(src).expect_err("Expected syntax error for missing '='");
    assert!(
        errs.iter().any(|d| d.code == Some("E0017")),
        "Expected diagnostic E0017 (missing '=' in let), found {:?}",
        errs
    );
}

#[test]
fn test_negative_malformed_let_missing_name() {
    let src = "
    fn main() -> i32 {
        let : i32 = 10;
        return 0;
    }
    ";
    let errs = parse_source(src).expect_err("Expected syntax error for missing variable name");
    assert!(
        errs.iter().any(|d| d.code == Some("E0016")),
        "Expected diagnostic E0016 (missing variable name), found {:?}",
        errs
    );
}

#[test]
fn test_negative_malformed_function_missing_param_type() {
    let src = "
    fn foo(x) -> i32 {
        return 0;
    }
    ";
    let errs = parse_source(src).expect_err("Expected syntax error for missing param type");
    assert!(
        errs.iter().any(|d| d.code == Some("E0007")),
        "Expected diagnostic E0007 (missing ':' after param name), found {:?}",
        errs
    );
}

#[test]
fn test_negative_malformed_function_missing_name() {
    let src = "
    fn () -> i32 {
        return 0;
    }
    ";
    let errs = parse_source(src).expect_err("Expected syntax error for missing function name");
    assert!(
        errs.iter().any(|d| d.code == Some("E0004")),
        "Expected diagnostic E0004 (missing function name), found {:?}",
        errs
    );
}

#[test]
fn test_negative_unclosed_parenthesis() {
    let src = "
    fn main() -> i32 {
        let x = (1 + 2;
        return 0;
    }
    ";
    let errs = parse_source(src).expect_err("Expected syntax error for unclosed '('");
    assert!(
        errs.iter().any(|d| d.code == Some("E0024")),
        "Expected diagnostic E0024 (expected ')' after grouped expression), found {:?}",
        errs
    );
}

#[test]
fn test_negative_unclosed_block_brace() {
    let src = "
    fn main() -> i32 {
        return 0;
    ";
    let errs = parse_source(src).expect_err("Expected syntax error for unclosed block '{'");
    assert!(
        errs.iter().any(|d| d.code == Some("E0013")),
        "Expected diagnostic E0013 (expected '}}' to close block), found {:?}",
        errs
    );
}

#[test]
fn test_negative_unexpected_top_level_statement() {
    let src = "let global_x = 10;";
    let errs = parse_source(src).expect_err("Expected error for top level statement");
    assert!(
        errs.iter().any(|d| d.code == Some("E0002")),
        "Expected diagnostic E0002 (expected top-level item 'fn'), found {:?}",
        errs
    );
}

#[test]
fn test_negative_unterminated_block_comment() {
    let src = "
    /* unclosed comment
    fn main() -> i32 {
        return 0;
    }
    ";
    let errs = parse_source(src).expect_err("Expected error for unclosed comment");
    assert!(
        errs.iter().any(|d| d.code == Some("E0001")),
        "Expected diagnostic E0001 (unterminated comment), found {:?}",
        errs
    );
}

#[test]
fn test_parse_string_literals() {
    let src = r#"
    fn main() -> i32 {
        let empty = "";
        let greeting = "Hello, World!";
        let escaped = "line1\nline2\r\ttab\\quote\"";
        let punctuation = "Spaces & Punctuation: [!@#$%^&*()]";
        println(greeting);
        println("Direct argument string");
        return 0;
    }
    "#;
    let program = parse_source(src).expect("Failed to parse program with string literals");
    assert_eq!(program.items.len(), 1);
    let Item::Function(func) = &program.items[0];
    assert_eq!(func.name.name, "main");
    // Verify AST contains Literal::String with correct contents
    let stmts = &func.body.stmts;
    assert_eq!(stmts.len(), 7); // 4 lets, 2 expr calls, 1 return

    match &stmts[0] {
        Stmt::Let { init: Expr::Literal(Literal::String(s, _)), .. } => {
            assert_eq!(s, "");
        }
        other => panic!("Expected empty string literal, found {:?}", other),
    }

    match &stmts[1] {
        Stmt::Let { init: Expr::Literal(Literal::String(s, _)), .. } => {
            assert_eq!(s, "Hello, World!");
        }
        other => panic!("Expected greeting string literal, found {:?}", other),
    }

    match &stmts[2] {
        Stmt::Let { init: Expr::Literal(Literal::String(s, _)), .. } => {
            assert_eq!(s, "line1\nline2\r\ttab\\quote\"");
        }
        other => panic!("Expected escaped string literal, found {:?}", other),
    }

    match &stmts[4] {
        Stmt::Expr { expr: Expr::Call { callee, args, .. }, .. } => {
            assert_eq!(callee.name, "println");
            assert_eq!(args.len(), 1);
        }
        other => panic!("Expected call statement, found {:?}", other),
    }

    match &stmts[5] {
        Stmt::Expr { expr: Expr::Call { callee, args, .. }, .. } => {
            assert_eq!(callee.name, "println");
            assert_eq!(args.len(), 1);
            match &args[0] {
                Expr::Literal(Literal::String(s, _)) => {
                    assert_eq!(s, "Direct argument string");
                }
                other => panic!("Expected string literal arg, found {:?}", other),
            }
        }
        other => panic!("Expected call statement with string literal, found {:?}", other),
    }
}

#[test]
fn test_negative_unterminated_string_literal() {
    let src = r#"
    fn main() -> i32 {
        let msg = "unterminated string;
        return 0;
    }
    "#;
    let errs = parse_source(src).expect_err("Expected error for unterminated string");
    assert!(
        errs.iter().any(|d| d.code == Some("E0001") && d.message.contains("unterminated string literal")),
        "Expected E0001 unterminated string literal, found {:?}",
        errs
    );
}

#[test]
fn test_negative_invalid_string_escape() {
    let src = r#"
    fn main() -> i32 {
        let msg = "invalid \q escape";
        return 0;
    }
    "#;
    let errs = parse_source(src).expect_err("Expected error for invalid escape");
    assert!(
        errs.iter().any(|d| d.code == Some("E0001") && d.message.contains("invalid escape sequence '\\q'")),
        "Expected E0001 invalid escape sequence, found {:?}",
        errs
    );
}

