# Qura Language

> **Status: Developer Preview (v0.1.0)**  
> Qura Language is currently in active foundational development. It is an experimental native programming language designed for clear code, high performance, and future use in backend services, developer tools, and AI-assisted workflows. It is **not** yet production-ready.

---

## Overview

Qura is a statically typed, compiled programming language. The current compiler implementation features:
- Handcrafted recursive-descent and Pratt parser (`qura_parser`) with precise source span tracking (`qura_syntax`).
- Rich compiler diagnostics with source carets and error codes (`qura_diagnostics`).
- Two-pass lexical scope resolver and type checker (`qura_sema`).
- Backend-independent Static Single Assignment (SSA) intermediate representation with dominance verification (`qura_ir`).
- Native Windows x86_64 code generation powered by Cranelift (`qura_codegen_cranelift`).
- Developer-facing CLI (`qura_cli`) providing `check`, `build`, and `run` commands.

---

## Windows Prerequisites

To compile the Qura compiler and generate native executables on Windows:
1. **Rust Toolchain:** Rust 1.87+ (MSVC target: `x86_64-pc-windows-msvc`).
2. **Visual Studio C++ Build Tools:**
   - Visual Studio 2022 or 2019 (Community, Professional, Enterprise, or standalone Build Tools).
   - "Desktop development with C++" workload installed (provides `link.exe` and Windows 10/11 SDK libraries including `kernel32.lib`).
3. *(Optional)* If your MSVC linker or SDK is in a custom path, configure environment variables:
   - `QURA_LINKER`: Full path to `link.exe`.
   - `QURA_SDK_LIB_DIR`: Directory containing `kernel32.lib` (e.g. `...\Windows Kits\10\Lib\10.0.22621.0\um\x64`).

---

## Building the Compiler

From the workspace root:

```powershell
# Build all compiler crates and the `qura` CLI
cargo build --release

# The `qura` executable is placed in target/release/qura.exe
.\target\release\qura.exe --help
```

To run all automated tests across the workspace:

```powershell
cargo test --workspace
```

---

## Using the Qura CLI

### 1. `qura check <file.qura>`
Verifies syntax and type correctness without generating native code:

```powershell
qura check examples/hello_math.qura
```

### 2. `qura build <file.qura> [-o output.exe]`
Compiles source code into a standalone native Windows x86_64 executable:

```powershell
# Default output name: hello_math.exe in current directory
qura build examples/hello_math.qura

# Explicit output name:
qura build examples/hello_math.qura -o my_math.exe
```

### 3. `qura run <file.qura>`
Compiles source code to a temporary binary, executes it immediately, and propagates the process exit code:

```powershell
qura run examples/factorial.qura
Write-Host "Process Exit Code:" $LASTEXITCODE
```

---

## Language Features (v0.1.0)

### Supported Types
- **Signed Integers:** `i32`, `i64`
- **Unsigned Integers:** `u32`, `u64`
- **Floating-Point:** `f64`
- **Boolean:** `bool` (`true`, `false`)
- **Unit:** `()`

### Supported Syntax & Semantics
- **Functions:** Explicit parameter types and return type annotations:
  ```qura
  fn add(x: i32, y: i32) -> i32 {
      return x + y;
  }
  ```
- **Variables & Mutability:** Immutable by default (`let`), explicit mutable bindings with `let mut`:
  ```qura
  let a = 10;          // Immutable
  let mut counter = 0; // Mutable
  counter = counter + 1;
  ```
- **Control Flow:**
  - `if` / `else` branching and expressions.
  - `while` loops with loop-carried mutable variables.
- **Binary & Unary Operators:**
  - Arithmetic: `+`, `-`, `*`, `/`, `%` (signed and unsigned division/modulo).
  - Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`.
  - Logic: `&&`, `||`, `!`.
- **Inter-Function Calls:** Calling user-defined functions with type-checked arguments.
- **Entry Point:** Program execution begins in `fn main() -> i32`, whose return value determines the native process exit code.

---

## Verified Examples

Sample programs are available in the [`examples/`](examples) directory:

| File | Description | Expected Exit Code |
| :--- | :--- | :--- |
| [`examples/hello_math.qura`](examples/hello_math.qura) | Arithmetic operations and function calling | `42` |
| [`examples/factorial.qura`](examples/factorial.qura) | Iterative computation of `5!` using a `while` loop | `120` |
| [`examples/collatz.qura`](examples/collatz.qura) | Collatz conjecture step counting for `n = 27` | `111` |

---

## Current Limitations

1. **Standard Output / I/O:** Standard library and standard I/O (e.g. `println!`) are not yet implemented. Programs currently communicate results to the operating system via `main` process exit codes.
2. **Strings and Compound Data:** Strings, heap allocation, structures (`struct`), arrays, and enums are planned for subsequent milestones.
3. **Linker Runtime:** Programs currently link against `kernel32.lib` with `/NODEFAULTLIB`. Full C runtime (`msvcrt`) integration will be introduced alongside standard library facilities.
