pub mod codegen;
pub mod linker;

pub use codegen::{compile_module, map_type, CodegenError};
pub use linker::{find_msvc_linker, find_windows_sdk_lib, link_executable, LinkerError};

use qura_ir::ir::Module;
use std::path::Path;

#[derive(Debug)]
pub enum BackendError {
    Codegen(CodegenError),
    Linker(LinkerError),
}

impl std::fmt::Display for BackendError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            BackendError::Codegen(err) => write!(f, "Codegen error: {}", err),
            BackendError::Linker(err) => write!(f, "Linker error: {}", err),
        }
    }
}

impl std::error::Error for BackendError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            BackendError::Codegen(err) => Some(err),
            BackendError::Linker(err) => Some(err),
        }
    }
}

impl From<CodegenError> for BackendError {
    fn from(err: CodegenError) -> Self {
        BackendError::Codegen(err)
    }
}

impl From<LinkerError> for BackendError {
    fn from(err: LinkerError) -> Self {
        BackendError::Linker(err)
    }
}

/// Convenience helper to compile a Qura IR module and link it directly into an executable.
pub fn compile_and_link(module: &Module, output_exe: &Path) -> Result<(), BackendError> {
    let obj_bytes = compile_module(module)?;
    link_executable(&obj_bytes, output_exe)?;
    Ok(())
}
