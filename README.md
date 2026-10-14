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
2. **Native Linker & Windows SDK:**
   Qura features multi-strategy automatic discovery for host linkers and SDK libraries:
   - **Visual Studio C++ Build Tools (Recommended):** Visual Studio 2022 or 2019 (Community, Pro, Enterprise, or standalone Build Tools). Can be installed in one command:
     ```powershell
     winget install Microsoft.VisualStudio.2022.BuildTools --override "--passive --add Microsoft.VisualStudio.Workload.VCTools"
     ```
   - **LLVM `lld-link`:** If LLVM is installed, Qura automatically discovers and uses `lld-link.exe`:
     ```powershell
     winget install LLVM.LLVM
     ```
   - **Automatic detection:** Qura queries `vswhere.exe`, Visual Studio Developer Environment (`LIB`), system `PATH`, and standard SDK locations.
   - **Explicit overrides:** You can optionally configure environment variables:
     - `QURA_LINKER`: Full path to `link.exe` or `lld-link.exe`.
     - `QURA_SDK_LIB_DIR`: Directory containing `kernel32.lib` (e.g. `C:\Program Files (x86)\Windows Kits\10\Lib\10.0.22621.0\um\x64`).

Run `qura doctor` at any time to verify that your environment is ready to build native executables.

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

### 1. `qura doctor`
Inspects the host environment, toolchain prerequisites, linker availability, and Windows SDK library paths:

```powershell
qura doctor
```

Output example:
```text
Qura Developer Environment Diagnostic
=======================================
[OK] Target Platform: Windows x86_64 (x86_64-pc-windows-msvc)
[OK] Native Linker: C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\...\bin\Hostx64\x64\link.exe
[OK] Windows SDK Library: C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64
---------------------------------------
Status: Environment ready. You can build and run native Qura programs!
```

### 2. `qura check <file.qura>`
Verifies syntax and type correctness without generating native code:

```powershell
qura check examples/hello_world.qura
```

### 3. `qura build <file.qura> [-o output.exe]`
Compiles source code into a standalone native Windows x86_64 executable:

```powershell
# Default output name: hello_world.exe in current directory
qura build examples/hello_world.qura

# Explicit output name:
qura build examples/hello_world.qura -o my_app.exe
```

### 4. `qura run <file.qura>`
Compiles source code to a temporary binary, executes it immediately, streams stdout/stderr to the console, and propagates the process exit code:

```powershell
qura run examples/hello_world.qura
```

---

## Language Features (v0.1.0-alpha)

### Supported Types
- **Signed Integers:** `i32`, `i64`
- **Unsigned Integers:** `u32`, `u64`
- **Floating-Point:** `f64`
- **Boolean:** `bool` (`true`, `false`)
- **Strings:** `str` (double-quoted string literals with escapes `\n`, `\r`, `\t`, `\\`, `\"`)
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
- **Console Output:** Built-in `print(val)` and `println(val)` supporting strings, integers, and booleans:
  ```qura
  print("Count: ");
  println(42);
  println(true);
  ```
- **Control Flow:**
  - `if` / `else` branching and expressions.
  - `while` loops with loop-carried mutable variables.
- **Binary & Unary Operators:**
  - Arithmetic: `+`, `-`, `*`, `/`, `%` (signed and unsigned division/modulo).
  - Comparisons: `==`, `!=`, `<`, `<=`, `>`, `>=`.
  - Logic: `&&`, `||`, `!`.
- **Inter-Function Calls:** Calling user-defined functions with type-checked arguments.
- **Entry Point:** Program execution begins in `fn main() -> i32` or `fn main()`. When returning unit, the process exits with code 0. When returning `i32`, the return value determines the native process exit code.

---

## Verified Examples

Sample programs are available in the [`examples/`](examples) directory:

| File | Description | Output / Exit Code |
| :--- | :--- | :--- |
| [`examples/hello_world.qura`](examples/hello_world.qura) | Hello world terminal output for strings, integers, and booleans | Prints output, exits with `0` |
| [`examples/hello_math.qura`](examples/hello_math.qura) | Arithmetic operations and function calling | Exits with `42` |
| [`examples/factorial.qura`](examples/factorial.qura) | Iterative computation of `5!` using a `while` loop | Exits with `120` |
| [`examples/collatz.qura`](examples/collatz.qura) | Collatz conjecture step counting for `n = 27` | Exits with `111` |

---

## Current Limitations & Roadmap

1. **Compound Types:** Structures (`struct`), enumerations (`enum`), and records are scheduled for Milestone 7.
2. **Dynamic Memory & Collections:** Dynamic heap allocation, growable lists (`List<T>`), and key-value maps (`Map<K, V>`) are scheduled for Milestone 8.
3. **C-ABI FFI:** User-defined `extern "C"` bindings to call external system DLLs (such as Win32 Sockets and SQLite) are scheduled for Milestone 9.
4. **Multi-file Projects:** Multi-file compilation and the `import` system are scheduled for Milestone 10.

