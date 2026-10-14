# Qura Language — Developer Quickstart Guide

> **Release:** v0.1.0-alpha (Developer Preview)  
> **Target:** Windows x86_64 (`x86_64-pc-windows-msvc`)

Welcome to the **Qura Language Developer Preview**. This guide walks you through setting up your environment, installing the Qura compiler, writing and running your first program, and setting up Visual Studio Code syntax highlighting.

---

## 1. Prerequisites

Qura generates native machine code using Cranelift and links with native Windows system libraries (`kernel32.lib`).

### Requirements:
1. **Windows 10 / 11 (64-bit)**
2. **Native Windows Linker and SDK Libraries**:
   - **Visual Studio C++ Build Tools (Recommended):**
     Install via `winget`:
     ```powershell
     winget install Microsoft.VisualStudio.2022.BuildTools --override "--passive --add Microsoft.VisualStudio.Workload.VCTools"
     ```
   - **Or LLVM (`lld-link.exe`):**
     ```powershell
     winget install LLVM.LLVM
     ```

> [!TIP]
> If you already have Visual Studio 2019 or 2022 Community installed with "Desktop development with C++", you already have the required linker and Windows SDK!

---

## 2. Installation & Quick Setup

### Step 1: Extract Release Archive
Extract `qura-v0.1.0-alpha-windows-x64.zip` to a directory of your choice, for example `C:\qura`:

```text
C:\qura\
├── bin\
│   └── qura.exe
├── examples\
│   └── hello_world.qura
├── editors\
│   └── vscode\
│       ├── qura-lang-0.1.0.vsix
│       ├── package.json
│       ├── language-configuration.json
│       └── syntaxes\
├── README.md
├── LICENSE
└── QUICKSTART.md
```

### Step 2: Add to PATH (Optional but Recommended)
Add the `bin` directory to your user `PATH` environment variable:

```powershell
# In PowerShell (temporary for current session):
$env:PATH = "C:\qura\bin;" + $env:PATH

# Or permanently in PowerShell:
[Environment]::SetEnvironmentVariable("Path", $env:Path + ";C:\qura\bin", [EnvironmentVariableTarget]::User)
```

Verify that `qura` is found:
```powershell
qura --version
```

---

## 3. Verify Environment with `qura doctor`

Run the built-in diagnostic tool to confirm that your linker and Windows SDK libraries are properly detected:

```powershell
qura doctor
```

Example output:
```text
Qura Developer Environment Diagnostic
=======================================
[OK] Target Platform: Windows x86_64 (x86_64-pc-windows-msvc)
[OK] Native Linker: C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC\...\bin\Hostx64\x64\link.exe
[OK] Windows SDK Library: C:\Program Files (x86)\Windows Kits\10\Lib\10.0.26100.0\um\x64
---------------------------------------
Status: Environment ready. You can build and run native Qura programs!
```

If the linker or SDK libraries are located in non-standard directories, you can set explicit environment variables:
- `QURA_LINKER`: Full path to `link.exe` or `lld-link.exe`.
- `QURA_SDK_LIB_DIR`: Directory containing `kernel32.lib`.

---

## 4. Run Your First Qura Program

The release includes `examples/hello_world.qura`:

```qura
fn main() -> i32 {
    println("Hello from Qura Language!");
    println(42);
    println(true);
    return 0;
}
```

### Check Syntax and Types
```powershell
qura check examples/hello_world.qura
```

### Compile and Run Directly
```powershell
qura run examples/hello_world.qura
```
Output:
```text
Hello from Qura Language!
42
true
```

### Compile to a Standalone Executable
```powershell
qura build examples/hello_world.qura -o hello.exe
.\hello.exe
```

---

## 5. Visual Studio Code Syntax Highlighting

The release package includes syntax highlighting for `.qura` files.

### Option A: Install the `.vsix` Package
From the extracted release root:
```powershell
code --install-extension editors/vscode/qura-lang-0.1.0.vsix
```

### Option B: Manual Folder Copy
Copy the `editors/vscode` directory to your VS Code extensions folder:
```powershell
Copy-Item -Recurse "editors/vscode" "$HOME/.vscode/extensions/qura-lang"
```

Reload or restart Visual Studio Code (`Ctrl+Shift+P` -> `Developer: Reload Window`). Opening any `.qura` file will now have full syntax highlighting and bracket matching.

---

## 6. Language Features Supported in v0.1.0-alpha

- **Functions & Signatures:** `fn name(param: type) -> return_type { ... }`
- **Variables & Mutability:** `let x: i32 = 10;` and `let mut y: i32 = 0;`
- **Control Flow:** `if / else` branching, `while` loops, `return` statements
- **Types:** `i32`, `i64`, `u32`, `u64`, `f64`, `bool`, `str`, unit `()`
- **Operators:** Arithmetic (`+`, `-`, `*`, `/`, `%`), comparison (`==`, `!=`, `<`, `<=`, `>`, `>=`), logical (`&&`, `||`, `!`), assignment (`=`)
- **String Literals:** Escapes `\n`, `\r`, `\t`, `\\`, `\"`
- **Standard Output:** Built-in `print(...)` and `println(...)` supporting `str`, `i32`, `i64`, `u32`, `u64`, and `bool`
- **Comments:** Line comments (`// ...`) and block comments (`/* ... */`) with nesting support

---

## 7. Next Steps

- Explore more examples in the `examples/` directory.
- Report feedback or issues at the [Qura Language Repository](https://github.com/qura-lang/qura).
