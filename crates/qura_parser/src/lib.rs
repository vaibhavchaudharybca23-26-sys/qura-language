pub mod parser;

pub use parser::{Parser, Precedence};

use qura_diagnostics::Diagnostic;
use qura_syntax::ast::Program;

/// Convenience function to parse a source string into an AST Program.
pub fn parse_source(src: &str) -> Result<Program, Vec<Diagnostic>> {
    let mut parser = Parser::new(src);
    parser.parse_program()
}
