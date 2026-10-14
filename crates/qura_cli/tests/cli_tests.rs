use std::env;
use std::fs;
use std::path::PathBuf;
use std::process::Command;

fn get_qura_bin() -> PathBuf {
    // Check CARGO_BIN_EXE_qura or look in target/debug
    if let Ok(path) = env::var("CARGO_BIN_EXE_qura") {
        return PathBuf::from(path);
    }
    let target_dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .unwrap()
        .parent()
        .unwrap()
        .join("target")
        .join("debug")
        .join("qura.exe");
    assert!(
        target_dir.exists(),
        "qura.exe not found at {}",
        target_dir.display()
    );
    target_dir
}

fn create_temp_qura_file(name: &str, content: &str) -> (PathBuf, PathBuf) {
    let dir = env::temp_dir().join("qura_cli_test_suite").join(name);
    fs::create_dir_all(&dir).unwrap();
    let file = dir.join(format!("{}.qura", name));
    fs::write(&file, content).unwrap();
    (dir, file)
}

#[test]
fn test_cli_help_and_version() {
    let qura = get_qura_bin();

    let help_out = Command::new(&qura).arg("--help").output().unwrap();
    assert!(help_out.status.success());
    let stdout = String::from_utf8_lossy(&help_out.stdout);
    let stderr = String::from_utf8_lossy(&help_out.stderr);
    assert!(stdout.contains("USAGE:") || stderr.contains("USAGE:"));

    let version_out = Command::new(&qura).arg("--version").output().unwrap();
    assert!(version_out.status.success());
    let stdout_ver = String::from_utf8_lossy(&version_out.stdout);
    assert!(stdout_ver.contains("qura v0.1.0"));
}

#[test]
fn test_cli_check_valid_file() {
    let qura = get_qura_bin();
    let src = "
    fn calc(a: i32, b: i32) -> i32 {
        let sum = a + b;
        return sum;
    }
    ";
    let (_dir, file) = create_temp_qura_file("check_valid", src);

    let output = Command::new(&qura)
        .arg("check")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success(), "check should succeed for valid code");
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert!(stdout.contains("no errors found"));
}

#[test]
fn test_cli_check_syntax_error() {
    let qura = get_qura_bin();
    let src = "
    fn broken(a: i32 -> i32 {
        return a;
    }
    ";
    let (_dir, file) = create_temp_qura_file("check_syntax_err", src);

    let output = Command::new(&qura)
        .arg("check")
        .arg(&file)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E0001]") || stderr.contains("expected"));
    assert!(stderr.contains("broken"));
}

#[test]
fn test_cli_check_semantic_error() {
    let qura = get_qura_bin();
    let src = "
    fn type_err() -> i32 {
        let x: i32 = true;
        return x;
    }
    ";
    let (_dir, file) = create_temp_qura_file("check_sema_err", src);

    let output = Command::new(&qura)
        .arg("check")
        .arg(&file)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("error[E0101]") || stderr.contains("type mismatch"));
}

#[test]
fn test_cli_check_missing_file() {
    let qura = get_qura_bin();
    let bogus_file = PathBuf::from("does_not_exist_12345.qura");

    let output = Command::new(&qura)
        .arg("check")
        .arg(&bogus_file)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("could not read source file"));
}

#[test]
fn test_cli_build_explicit_output() {
    let qura = get_qura_bin();
    let src = "
    fn main() -> i32 {
        let x = 15;
        let y = 27;
        return x + y; // 42
    }
    ";
    let (dir, file) = create_temp_qura_file("build_explicit", src);
    let custom_exe = dir.join("my_app.exe");

    let output = Command::new(&qura)
        .arg("build")
        .arg(&file)
        .arg("-o")
        .arg(&custom_exe)
        .output()
        .unwrap();

    assert!(output.status.success(), "Build with -o should succeed");
    assert!(custom_exe.exists(), "Binary must exist at specified -o path");

    // Execute the built binary and verify its return code
    let run_out = Command::new(&custom_exe).output().unwrap();
    assert_eq!(run_out.status.code(), Some(42));
}

#[test]
fn test_cli_build_default_output() {
    let qura = get_qura_bin();
    let src = "
    fn main() -> i32 {
        return 77;
    }
    ";
    let (dir, file) = create_temp_qura_file("build_default", src);
    let expected_exe = dir.join("build_default.exe");

    // Run build inside dir so default output lands in dir
    let output = Command::new(&qura)
        .current_dir(&dir)
        .arg("build")
        .arg(&file)
        .output()
        .unwrap();

    assert!(output.status.success(), "Build with default output should succeed");
    assert!(expected_exe.exists(), "Binary must exist at default path");

    let run_out = Command::new(&expected_exe).output().unwrap();
    assert_eq!(run_out.status.code(), Some(77));
}

#[test]
fn test_cli_run_propagates_exit_code() {
    let qura = get_qura_bin();
    let src = "
    fn main() -> i32 {
        return 42;
    }
    ";
    let (_dir, file) = create_temp_qura_file("run_exit_42", src);

    let output = Command::new(&qura)
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    // The CLI should propagate the executed program's exit code
    assert_eq!(output.status.code(), Some(42));
}

#[test]
fn test_cli_run_factorial_program() {
    let qura = get_qura_bin();
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
    let (_dir, file) = create_temp_qura_file("run_factorial", src);

    let output = Command::new(&qura)
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(120));
}

#[test]
fn test_cli_run_collatz_program() {
    let qura = get_qura_bin();
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
    let (_dir, file) = create_temp_qura_file("run_collatz", src);

    let output = Command::new(&qura)
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    assert_eq!(output.status.code(), Some(111));
}

#[test]
fn test_cli_run_compiler_error_distinction() {
    let qura = get_qura_bin();
    let src = "
    fn broken() -> i32 {
        let mut undefined_val = some_missing_var + 1;
        return undefined_val;
    }
    ";
    let (_dir, file) = create_temp_qura_file("run_compile_err", src);

    let output = Command::new(&qura)
        .arg("run")
        .arg(&file)
        .output()
        .unwrap();

    // Compiler errors must exit with code 1, not crash or report success
    assert_eq!(output.status.code(), Some(1));
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("cannot find variable") || stderr.contains("E0103"));
}
