use qura_codegen_cranelift::compile_and_link;
use qura_diagnostics::DiagnosticEmitter;
use qura_ir::compile_and_verify;
use qura_parser::parse_source;
use qura_sema::analyze;
use qura_syntax::SourceFile;
use std::fs;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

static RUN_COUNTER: AtomicU64 = AtomicU64::new(0);

/// Core compiler driver for Qura CLI commands.
pub struct Driver;

impl Driver {
    /// Executes `qura check <file.qura>`:
    /// Parses and semantically checks the source file without code generation.
    pub fn check<W: Write>(file_path: &Path, err_writer: &mut W) -> Result<(), i32> {
        let (source_file, source_text) = Self::read_source(file_path, err_writer)?;

        let program = match parse_source(&source_text) {
            Ok(p) => p,
            Err(diags) => {
                let emitter = DiagnosticEmitter::new(&source_file);
                for diag in &diags {
                    let _ = write!(err_writer, "{}", emitter.emit_to_string(diag));
                }
                return Err(1);
            }
        };

        if let Err(diags) = analyze(&program) {
            let emitter = DiagnosticEmitter::new(&source_file);
            for diag in &diags {
                let _ = write!(err_writer, "{}", emitter.emit_to_string(diag));
            }
            return Err(1);
        }

        Ok(())
    }

    /// Executes `qura build <file.qura> [-o output.exe]`:
    /// Runs parsing, semantic analysis, QIR lowering, QIR verification,
    /// Cranelift codegen, and MSVC linking to produce a native Windows executable.
    pub fn build<W: Write>(
        file_path: &Path,
        output_path: Option<&Path>,
        err_writer: &mut W,
    ) -> Result<PathBuf, i32> {
        let (source_file, source_text) = Self::read_source(file_path, err_writer)?;

        // 1. Parsing
        let program = match parse_source(&source_text) {
            Ok(p) => p,
            Err(diags) => {
                let emitter = DiagnosticEmitter::new(&source_file);
                for diag in &diags {
                    let _ = write!(err_writer, "{}", emitter.emit_to_string(diag));
                }
                return Err(1);
            }
        };

        // 2. Semantic Analysis
        let symbols = match analyze(&program) {
            Ok(s) => s,
            Err(diags) => {
                let emitter = DiagnosticEmitter::new(&source_file);
                for diag in &diags {
                    let _ = write!(err_writer, "{}", emitter.emit_to_string(diag));
                }
                return Err(1);
            }
        };

        // 3. QIR Lowering & Verification
        let module = match compile_and_verify(&program, &symbols) {
            Ok(m) => m,
            Err(errors) => {
                for err in &errors {
                    let _ = writeln!(err_writer, "qir verification error: {}", err);
                }
                return Err(1);
            }
        };

        // 4. Determine output binary path
        let out_exe = match output_path {
            Some(p) => p.to_path_buf(),
            None => {
                let stem = file_path
                    .file_stem()
                    .and_then(|s| s.to_str())
                    .unwrap_or("output");
                PathBuf::from(format!("{}.exe", stem))
            }
        };

        // 5. Code generation & Linking
        if let Err(err) = compile_and_link(&module, &out_exe) {
            let _ = writeln!(err_writer, "error: compilation failed: {}", err);
            return Err(1);
        }

        Ok(out_exe)
    }

    /// Executes `qura run <file.qura>`:
    /// Compiles the program into a temporary executable, runs it,
    /// cleans up the artifact, and returns the executed process's exit code.
    pub fn run<W: Write>(file_path: &Path, err_writer: &mut W) -> Result<i32, i32> {
        let count = RUN_COUNTER.fetch_add(1, Ordering::SeqCst);
        let temp_dir = std::env::temp_dir().join(format!("qura_run_{}_{}", std::process::id(), count));
        if let Err(e) = fs::create_dir_all(&temp_dir) {
            let _ = writeln!(err_writer, "error: failed to create temporary directory: {}", e);
            return Err(1);
        }

        let temp_exe = temp_dir.join("program.exe");

        // Build into temp_exe
        let build_res = Self::build(file_path, Some(&temp_exe), err_writer);
        if let Err(e) = build_res {
            let _ = fs::remove_dir_all(&temp_dir);
            return Err(e);
        }

        // Execute the native binary
        let run_result = Command::new(&temp_exe).output();

        // Safely clean up temporary artifacts
        let _ = fs::remove_file(&temp_exe);
        let _ = fs::remove_dir_all(&temp_dir);

        match run_result {
            Ok(output) => {
                // Propagate actual exit code
                let code = output.status.code().unwrap_or(-1);
                Ok(code)
            }
            Err(e) => {
                let _ = writeln!(err_writer, "error: failed to execute native binary: {}", e);
                Err(1)
            }
        }
    }

    fn read_source<W: Write>(
        file_path: &Path,
        err_writer: &mut W,
    ) -> Result<(SourceFile, String), i32> {
        let content = match fs::read_to_string(file_path) {
            Ok(c) => c,
            Err(err) => {
                let _ = writeln!(
                    err_writer,
                    "error: could not read source file '{}': {}",
                    file_path.display(),
                    err
                );
                return Err(1);
            }
        };

        let file_name = file_path.to_string_lossy().to_string();
        let source_file = SourceFile::new(file_name, &content);
        Ok((source_file, content))
    }
}
