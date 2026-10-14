use crate::types::Type;
use qura_diagnostics::{Diagnostic, DiagnosticLabel};
use qura_syntax::Span;
use std::collections::HashMap;

#[derive(Debug, Clone, PartialEq)]
pub struct VarSymbol {
    pub name: String,
    pub ty: Type,
    pub is_mut: bool,
    pub span: Span,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FnSymbol {
    pub name: String,
    pub params: Vec<(String, Type, bool)>,
    pub return_type: Type,
    pub span: Span,
}

#[derive(Debug, Clone, Default)]
pub struct Scope {
    variables: HashMap<String, VarSymbol>,
}

impl Scope {
    pub fn new() -> Self {
        Self {
            variables: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct SymbolTable {
    functions: HashMap<String, FnSymbol>,
    scopes: Vec<Scope>,
}

impl SymbolTable {
    pub fn new() -> Self {
        Self {
            functions: HashMap::new(),
            scopes: vec![Scope::new()], // Global scope
        }
    }

    pub fn push_scope(&mut self) {
        self.scopes.push(Scope::new());
    }

    pub fn pop_scope(&mut self) {
        assert!(
            self.scopes.len() > 1,
            "cannot pop the root global scope"
        );
        self.scopes.pop();
    }

    pub fn define_function(&mut self, symbol: FnSymbol) -> Result<(), Diagnostic> {
        if let Some(existing) = self.functions.get(&symbol.name) {
            let diag = Diagnostic::error(
                format!("duplicate function definition '{}'", symbol.name),
                symbol.span,
            )
            .with_code("E0104")
            .with_label(DiagnosticLabel::primary(symbol.span))
            .with_note(format!("first defined at span {}", existing.span));
            return Err(diag);
        }

        self.functions.insert(symbol.name.clone(), symbol);
        Ok(())
    }

    pub fn lookup_function(&self, name: &str) -> Option<&FnSymbol> {
        self.functions.get(name)
    }

    pub fn define_variable(
        &mut self,
        name: impl Into<String>,
        ty: Type,
        is_mut: bool,
        span: Span,
    ) -> Result<(), Diagnostic> {
        let name_str = name.into();
        let current_scope = self.scopes.last_mut().expect("scope stack cannot be empty");

        if let Some(existing) = current_scope.variables.get(&name_str) {
            let diag = Diagnostic::error(
                format!("variable '{}' is already defined in this scope", name_str),
                span,
            )
            .with_code("E0104")
            .with_label(DiagnosticLabel::primary(span))
            .with_note(format!("previous definition was at span {}", existing.span));
            return Err(diag);
        }

        let symbol = VarSymbol {
            name: name_str.clone(),
            ty,
            is_mut,
            span,
        };
        current_scope.variables.insert(name_str, symbol);
        Ok(())
    }

    pub fn lookup_variable(&self, name: &str) -> Option<&VarSymbol> {
        for scope in self.scopes.iter().rev() {
            if let Some(sym) = scope.variables.get(name) {
                return Some(sym);
            }
        }
        None
    }

    pub fn lookup_variable_mut(&mut self, name: &str) -> Option<&mut VarSymbol> {
        for scope in self.scopes.iter_mut().rev() {
            if let Some(sym) = scope.variables.get_mut(name) {
                return Some(sym);
            }
        }
        None
    }
}
