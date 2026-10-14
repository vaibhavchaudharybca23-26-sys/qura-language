use qura_codegen_cranelift::{compile_and_link, compile_module, link_executable};
use qura_ir::{compile_and_verify, verify_module, Module};
use qura_parser::parse_source;
use qura_sema::analyze;
use std::process::Command;

fn lower_and_verify(src: &str) -> Module {
    let program = parse_source(src).expect("Failed to parse source");
    let symbols = analyze(&program).expect("Semantic check failed");
    compile_and_verify(&program, &symbols).expect("QIR lowering or verification failed")
}

fn compile_to_exe_and_run(src: &str, test_name: &str) -> i32 {
    let module = lower_and_verify(src);
    verify_module(&module).expect("QIR verification must succeed");

    let out_dir = std::env::temp_dir().join("qura_e2e_tests").join(test_name);
    std::fs::create_dir_all(&out_dir).expect("Failed to create temporary output directory");
    let exe_path = out_dir.join(format!("{}.exe", test_name));

    compile_and_link(&module, &exe_path).expect("Failed to compile and link native executable");
    assert!(exe_path.exists(), "Executable must exist on disk");

    let output = Command::new(&exe_path)
        .output()
        .expect("Failed to execute native binary");

    output.status.code().expect("Process must exit with a code")
}

#[test]
fn test_codegen_minimal_exit_code() {
    let src = "
    fn main() -> i32 {
        return 42;
    }
    ";
    let exit_code = compile_to_exe_and_run(src, "minimal_exit_code");
    assert_eq!(exit_code, 42);
}

#[test]
fn test_codegen_arithmetic_operations() {
    let src = "
    fn main() -> i32 {
        let a = 10;
        let b = 5;
        let c = 2;
        let d = 20 / 4; // 5
        let e = 17 % 5; // 2
        return a + b * c - d + e; // 10 + 10 - 5 + 2 = 17
    }
    ";
    let exit_code = compile_to_exe_and_run(src, "arithmetic_operations");
    assert_eq!(exit_code, 17);
}

#[test]
fn test_codegen_functions_and_calls() {
    let src = "
    fn add(x: i32, y: i32) -> i32 {
        return x + y;
    }

    fn mul(x: i32, y: i32) -> i32 {
        return x * y;
    }

    fn main() -> i32 {
        let sum = add(10, 20);
        let prod = mul(sum, 2);
        return prod; // (10 + 20) * 2 = 60
    }
    ";
    let exit_code = compile_to_exe_and_run(src, "functions_and_calls");
    assert_eq!(exit_code, 60);
}

#[test]
fn test_codegen_if_else_branching() {
    let src = "
    fn max(a: i32, b: i32) -> i32 {
        let result = if a > b {
            a
        } else {
            b
        };
        return result;
    }

    fn main() -> i32 {
        let m1 = max(15, 27); // 27
        let m2 = max(50, 12); // 50
        return m1 + m2; // 77
    }
    ";
    let exit_code = compile_to_exe_and_run(src, "if_else_branching");
    assert_eq!(exit_code, 77);
}

#[test]
fn test_codegen_boolean_logic() {
    let src = "
    fn check(a: bool, b: bool) -> i32 {
        let res = if a && !b {
            10
        } else {
            25
        };
        return res;
    }

    fn main() -> i32 {
        let r1 = check(true, false); // 10
        let r2 = check(true, true);  // 25
        return r1 + r2; // 35
    }
    ";
    let exit_code = compile_to_exe_and_run(src, "boolean_logic");
    assert_eq!(exit_code, 35);
}

#[test]
fn test_codegen_canonical_factorial() {
    let src = "
    fn factorial(n: i32) -> i32 {
        let mut acc = 1;
        let mut i = 1;
        while i <= n {
            acc = acc * i;
            i = i + 1;
        }
        return acc;
    }

    fn main() -> i32 {
        return factorial(5); // 120
    }
    ";
    let exit_code = compile_to_exe_and_run(src, "canonical_factorial");
    assert_eq!(exit_code, 120);
}

#[test]
fn test_codegen_canonical_collatz() {
    let src = "
    fn collatz_steps(mut n: i32) -> i32 {
        let mut steps = 0;
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
        // Collatz steps for 27 is 111
        return collatz_steps(27);
    }
    ";
    let exit_code = compile_to_exe_and_run(src, "canonical_collatz");
    assert_eq!(exit_code, 111);
}

#[test]
fn test_codegen_unsigned_integers() {
    let src = "
    fn test_u32(a: u32, b: u32) -> i32 {
        let res = if a < b {
            1
        } else {
            0
        };
        return res;
    }

    fn main() -> i32 {
        let x: u32 = 100;
        let y: u32 = 200;
        return test_u32(x, y); // 1
    }
    ";
    let exit_code = compile_to_exe_and_run(src, "unsigned_integers");
    assert_eq!(exit_code, 1);
}

#[test]
fn test_codegen_separate_object_and_link_stages() {
    // Tests step 6 requirement: object generation and linking in separate, testable stages
    let src = "
    fn main() -> i32 {
        return 7;
    }
    ";
    let module = lower_and_verify(src);
    let obj_bytes = compile_module(&module).expect("Object compilation stage must succeed");
    assert!(!obj_bytes.is_empty(), "Object bytes must be non-empty");

    let out_dir = std::env::temp_dir().join("qura_e2e_tests").join("separate_stages");
    std::fs::create_dir_all(&out_dir).expect("Create dir");
    let exe_path = out_dir.join("separate_stages.exe");

    link_executable(&obj_bytes, &exe_path).expect("Linking stage must succeed");
    let output = Command::new(&exe_path).output().expect("Execute");
    assert_eq!(output.status.code(), Some(7));
}
