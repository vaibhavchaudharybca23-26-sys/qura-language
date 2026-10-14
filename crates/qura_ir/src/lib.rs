pub mod ir;
pub mod lower;
pub mod verifier;

pub use ir::*;
pub use lower::lower_program;
pub use verifier::{verify_module, QirVerificationError, Verifier};

use qura_sema::SymbolTable;
use qura_syntax::ast::Program;

/// Lowers an analyzed Program into Qura IR (QIR) and verifies all SSA,
/// control flow, and dominance invariants.
pub fn compile_and_verify(
    program: &Program,
    symbols: &SymbolTable,
) -> Result<Module, Vec<QirVerificationError>> {
    let module = lower_program(program, symbols);
    verify_module(&module)?;
    Ok(module)
}
