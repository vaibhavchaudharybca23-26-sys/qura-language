use std::fmt;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs};

#[derive(Debug)]
pub enum LinkerError {
    LinkerNotFound,
    SdkLibNotFound,
    Io(std::io::Error),
    LinkFailed {
        code: Option<i32>,
        stdout: String,
        stderr: String,
    },
}

impl fmt::Display for LinkerError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            LinkerError::LinkerNotFound => write!(
                f,
                "No compatible native linker (link.exe or lld-link.exe) was found on this system.\n\
                 To resolve this:\n\
                 1. Install Visual Studio C++ Build Tools (recommended):\n\
                    winget install Microsoft.VisualStudio.2022.BuildTools --override \"--passive --add Microsoft.VisualStudio.Workload.VCTools\"\n\
                 2. Or install LLVM:\n\
                    winget install LLVM.LLVM\n\
                 3. Or set the QURA_LINKER environment variable to the path of link.exe or lld-link.exe."
            ),
            LinkerError::SdkLibNotFound => write!(
                f,
                "Windows SDK library directory containing kernel32.lib was not found.\n\
                 To resolve this:\n\
                 1. Install the Windows SDK via Visual Studio Installer or winget:\n\
                    winget install Microsoft.WindowsSDK.10.0.22621\n\
                 2. Or set QURA_SDK_LIB_DIR to the directory containing kernel32.lib\n\
                    (e.g., C:\\Program Files (x86)\\Windows Kits\\10\\Lib\\10.0.22621.0\\um\\x64)."
            ),
            LinkerError::Io(err) => write!(f, "IO error during linking: {}", err),
            LinkerError::LinkFailed {
                code,
                stdout,
                stderr,
            } => {
                write!(
                    f,
                    "Linker failed with exit code {:?}:\nStdout: {}\nStderr: {}",
                    code, stdout, stderr
                )
            }
        }
    }
}

impl std::error::Error for LinkerError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            LinkerError::Io(err) => Some(err),
            _ => None,
        }
    }
}

impl From<std::io::Error> for LinkerError {
    fn from(err: std::io::Error) -> Self {
        LinkerError::Io(err)
    }
}

/// Find a compatible native linker (`link.exe` or `lld-link.exe`) on the current Windows host.
pub fn find_native_linker() -> Result<PathBuf, LinkerError> {
    find_msvc_linker()
}

/// Find MSVC `link.exe` (or `lld-link.exe`) on the current Windows host.
pub fn find_msvc_linker() -> Result<PathBuf, LinkerError> {
    // 1. Explicit environment override
    if let Ok(path) = env::var("QURA_LINKER") {
        let p = PathBuf::from(path);
        if p.exists() {
            return Ok(p);
        }
    }

    // 2. Query official Visual Studio locator (vswhere.exe)
    let vswhere_path = r"C:\Program Files (x86)\Microsoft Visual Studio\Installer\vswhere.exe";
    if Path::new(vswhere_path).exists() {
        if let Ok(output) = Command::new(vswhere_path)
            .args(&[
                "-latest",
                "-products",
                "*",
                "-requires",
                "Microsoft.VisualStudio.Component.VC.Tools.x86.x64",
                "-property",
                "installationPath",
            ])
            .output()
        {
            if output.status.success() {
                let install_dir = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !install_dir.is_empty() {
                    let msvc_dir = Path::new(&install_dir).join("VC").join("Tools").join("MSVC");
                    if msvc_dir.exists() {
                        if let Ok(entries) = fs::read_dir(&msvc_dir) {
                            let mut versions: Vec<PathBuf> = entries
                                .filter_map(|e| e.ok())
                                .map(|e| e.path())
                                .filter(|p| p.is_dir())
                                .collect();
                            versions.sort_by(|a, b| b.cmp(a));
                            for ver in versions {
                                let link_path = ver.join("bin").join("Hostx64").join("x64").join("link.exe");
                                if link_path.exists() {
                                    return Ok(link_path);
                                }
                            }
                        }
                    }
                }
            }
        }
    }

    // 3. Check known standard installation directories for VS2022 / VS2019
    let vs_candidates = [
        r"C:\Program Files\Microsoft Visual Studio\2022\Community\VC\Tools\MSVC",
        r"C:\Program Files\Microsoft Visual Studio\2022\Professional\VC\Tools\MSVC",
        r"C:\Program Files\Microsoft Visual Studio\2022\Enterprise\VC\Tools\MSVC",
        r"C:\Program Files\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Community\VC\Tools\MSVC",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Professional\VC\Tools\MSVC",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\Enterprise\VC\Tools\MSVC",
        r"C:\Program Files (x86)\Microsoft Visual Studio\2019\BuildTools\VC\Tools\MSVC",
    ];

    for base in vs_candidates {
        let base_path = Path::new(base);
        if base_path.exists() {
            if let Ok(entries) = fs::read_dir(base_path) {
                let mut versions: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                // Sort descending to prefer latest version
                versions.sort_by(|a, b| b.cmp(a));

                for ver in versions {
                    let link_path = ver.join("bin").join("Hostx64").join("x64").join("link.exe");
                    if link_path.exists() {
                        return Ok(link_path);
                    }
                }
            }
        }
    }

    // 4. Search system PATH for `link.exe`
    if let Ok(output) = Command::new("where.exe").arg("link.exe").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let p = PathBuf::from(line.trim());
                if p.exists() {
                    return Ok(p);
                }
            }
        }
    }

    // 5. Search system PATH and LLVM installations for `lld-link.exe`
    if let Ok(output) = Command::new("where.exe").arg("lld-link.exe").output() {
        if output.status.success() {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let p = PathBuf::from(line.trim());
                if p.exists() {
                    return Ok(p);
                }
            }
        }
    }

    let llvm_candidates = [
        r"C:\Program Files\LLVM\bin\lld-link.exe",
        r"C:\Program Files (x86)\LLVM\bin\lld-link.exe",
    ];
    for llvm in llvm_candidates {
        let p = PathBuf::from(llvm);
        if p.exists() {
            return Ok(p);
        }
    }

    Err(LinkerError::LinkerNotFound)
}

/// Find Windows SDK library directory containing `kernel32.lib`.
pub fn find_windows_sdk_lib() -> Result<PathBuf, LinkerError> {
    // 1. Explicit environment override
    if let Ok(dir) = env::var("QURA_SDK_LIB_DIR") {
        let p = PathBuf::from(dir);
        if p.join("kernel32.lib").exists() {
            return Ok(p);
        }
    }

    // 2. Check Visual Studio Developer Environment `LIB` variable
    if let Ok(lib_env) = env::var("LIB") {
        for part in lib_env.split(';') {
            let trimmed = part.trim();
            if !trimmed.is_empty() {
                let p = PathBuf::from(trimmed);
                if p.join("kernel32.lib").exists() {
                    return Ok(p);
                }
            }
        }
    }

    // 3. Search standard Windows Kits directories
    let sdk_bases = [
        r"C:\Program Files (x86)\Windows Kits\10\Lib",
        r"C:\Program Files\Windows Kits\10\Lib",
        r"D:\Program Files (x86)\Windows Kits\10\Lib",
        r"D:\Program Files\Windows Kits\10\Lib",
    ];

    for base in sdk_bases {
        let base_path = Path::new(base);
        if base_path.exists() {
            if let Ok(entries) = fs::read_dir(base_path) {
                let mut versions: Vec<PathBuf> = entries
                    .filter_map(|e| e.ok())
                    .map(|e| e.path())
                    .filter(|p| p.is_dir())
                    .collect();
                versions.sort_by(|a, b| b.cmp(a));

                for ver in versions {
                    let um_x64 = ver.join("um").join("x64");
                    if um_x64.join("kernel32.lib").exists() {
                        return Ok(um_x64);
                    }
                }
            }
        }
    }

    Err(LinkerError::SdkLibNotFound)
}

/// Link raw object bytes into a standalone native Windows executable.
pub fn link_executable(obj_bytes: &[u8], output_exe: &Path) -> Result<(), LinkerError> {
    let linker = find_msvc_linker()?;
    let sdk_lib = find_windows_sdk_lib()?;

    if let Some(parent) = output_exe.parent() {
        fs::create_dir_all(parent)?;
    }

    let temp_obj = output_exe.with_extension("obj");
    fs::write(&temp_obj, obj_bytes)?;

    let mut cmd = Command::new(linker);
    cmd.arg("/NOLOGO")
        .arg("/ENTRY:main")
        .arg("/SUBSYSTEM:CONSOLE")
        .arg("/NODEFAULTLIB")
        .arg(format!("/LIBPATH:{}", sdk_lib.display()))
        .arg("kernel32.lib")
        .arg(format!("/OUT:{}", output_exe.display()))
        .arg(temp_obj.display().to_string());

    let output = cmd.output()?;

    // Clean up temporary obj file
    let _ = fs::remove_file(&temp_obj);

    if !output.status.success() {
        return Err(LinkerError::LinkFailed {
            code: output.status.code(),
            stdout: String::from_utf8_lossy(&output.stdout).to_string(),
            stderr: String::from_utf8_lossy(&output.stderr).to_string(),
        });
    }

    Ok(())
}

