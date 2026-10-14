pub mod symbol;
pub mod type_checker;
pub mod types;

pub use symbol::{FnSymbol, Scope, SymbolTable, VarSymbol};
pub use type_checker::TypeChecker;
pub use types::Type;

use qura_diagnostics::Diagnostic;
use qura_syntax::ast::Program;

/// Analyzes an AST Program, performing symbol resolution, scope checking,
/// and type checking. Returns the populated SymbolTable or semantic Diagnostics.
pub fn analyze(program: &Program) -> Result<SymbolTable, Vec<Diagnostic>> {
    let checker = TypeChecker::new();
    checker.check_program(program)
}
