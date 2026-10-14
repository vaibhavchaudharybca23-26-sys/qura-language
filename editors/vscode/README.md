# Qura Language for Visual Studio Code

Official syntax highlighting and language configuration for **Qura Language** (`.qura`) files in Visual Studio Code.

---

## Features

- **Syntax Highlighting:** Complete TextMate grammar for Qura declarations (`fn`, `let`, `mut`), control flow (`if`, `else`, `while`, `return`), built-in output functions (`print`, `println`), primitive types (`i32`, `i64`, `u32`, `u64`, `f64`, `bool`, `str`), numeric literals (decimal, hex, binary, float), strings with escape validation (`\n`, `\r`, `\t`, `\\`, `\"`), and nested block/line comments.
- **Language Configuration:** Automatic bracket closing (`{}`, `()`, `[]`), quote auto-closing (`""`), and comment toggling (`//`, `/* */`).

---

## Installation

### Method 1: Manual Folder Copy (Recommended for Developer Preview)

Copy or symlink the `editors/vscode` directory to your VS Code extensions folder:

#### Windows PowerShell:
```powershell
Copy-Item -Recurse "editors/vscode" "$HOME/.vscode/extensions/qura-lang"
```

#### macOS / Linux:
```bash
cp -r editors/vscode ~/.vscode/extensions/qura-lang
```

Restart or reload Visual Studio Code (`Developer: Reload Window` in the Command Palette).

---

### Method 2: Package and Install `.vsix`

If you have Node.js and `@vscode/vsce` installed:

```powershell
cd editors/vscode
npx @vscode/vsce package
code --install-extension qura-lang-0.1.0.vsix
```

---

## Example Qura Code

```qura
fn main() -> i32 {
    let greeting: str = "Hello from Qura Language!";
    println(greeting);

    let mut counter: i32 = 0;
    while counter < 5 {
        println(counter);
        counter = counter + 1;
    }

    return 0;
}
```

---

## License

Apache License 2.0. See [LICENSE](LICENSE) for details.
