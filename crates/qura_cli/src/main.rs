use qura_cli::Driver;
use std::env;
use std::io;
use std::path::{Path, PathBuf};
use std::process;

const VERSION: &str = env!("CARGO_PKG_VERSION");

fn print_usage() {
    eprintln!(
        r#"Qura Language Compiler (qura) v{} - Developer Preview

USAGE:
    qura <COMMAND> [OPTIONS] <FILE.qura>

COMMANDS:
    check <FILE>             Check syntax and types without code generation
    build <FILE> [-o <OUT>]  Compile source into a native Windows executable
    run   <FILE>             Compile, execute, and propagate process exit code
    help                     Print this help message
    version                  Print version information

OPTIONS:
    -o, --output <PATH>      Specify output executable path for `build`
    -h, --help               Print help information
    -V, --version            Print version information
"#,
        VERSION
    );
}

fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() < 2 {
        print_usage();
        process::exit(1);
    }

    let command = args[1].as_str();

    match command {
        "-h" | "--help" | "help" => {
            print_usage();
            process::exit(0);
        }
        "-V" | "--version" | "version" => {
            println!("qura v{}", VERSION);
            process::exit(0);
        }
        "check" => {
            if args.len() < 3 {
                eprintln!("error: 'qura check' requires a source file argument\n");
                eprintln!("Usage: qura check <file.qura>");
                process::exit(1);
            }
            let file_path = Path::new(&args[2]);
            let mut stderr = io::stderr();
            match Driver::check(file_path, &mut stderr) {
                Ok(()) => {
                    println!("check: no errors found in '{}'", file_path.display());
                    process::exit(0);
                }
                Err(code) => process::exit(code),
            }
        }
        "build" => {
            if args.len() < 3 {
                eprintln!("error: 'qura build' requires a source file argument\n");
                eprintln!("Usage: qura build <file.qura> [-o output.exe]");
                process::exit(1);
            }

            let mut input_file: Option<&str> = None;
            let mut output_path: Option<PathBuf> = None;

            let mut i = 2;
            while i < args.len() {
                let arg = &args[i];
                if arg == "-o" || arg == "--output" {
                    if i + 1 >= args.len() {
                        eprintln!("error: missing path argument after '{}'", arg);
                        process::exit(1);
                    }
                    output_path = Some(PathBuf::from(&args[i + 1]));
                    i += 2;
                } else if arg.starts_with("-") {
                    eprintln!("error: unrecognized option '{}'", arg);
                    process::exit(1);
                } else {
                    if input_file.is_some() {
                        eprintln!("error: unexpected multiple input files provided: '{}'", arg);
                        process::exit(1);
                    }
                    input_file = Some(arg);
                    i += 1;
                }
            }

            let file_str = match input_file {
                Some(f) => f,
                None => {
                    eprintln!("error: no input file provided for 'qura build'");
                    process::exit(1);
                }
            };

            let file_path = Path::new(file_str);
            let mut stderr = io::stderr();
            match Driver::build(file_path, output_path.as_deref(), &mut stderr) {
                Ok(out_exe) => {
                    println!("build: generated executable '{}'", out_exe.display());
                    process::exit(0);
                }
                Err(code) => process::exit(code),
            }
        }
        "run" => {
            if args.len() < 3 {
                eprintln!("error: 'qura run' requires a source file argument\n");
                eprintln!("Usage: qura run <file.qura>");
                process::exit(1);
            }
            let file_path = Path::new(&args[2]);
            let mut stderr = io::stderr();
            match Driver::run(file_path, &mut stderr) {
                Ok(exit_code) => {
                    process::exit(exit_code);
                }
                Err(code) => process::exit(code),
            }
        }
        other => {
            eprintln!("error: unknown command '{}'\n", other);
            print_usage();
            process::exit(1);
        }
    }
}
