use qura_syntax::ast::TypeExpr;
use std::fmt;

#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Type {
    I32,
    I64,
    U32,
    U64,
    F64,
    Bool,
    Str,
    Unit,
    Error,
}

impl Type {
    pub fn from_type_expr(expr: &TypeExpr) -> Result<Self, String> {
        match expr {
            TypeExpr::Named(ident) => match ident.name.as_str() {
                "i32" => Ok(Type::I32),
                "i64" => Ok(Type::I64),
                "u32" => Ok(Type::U32),
                "u64" => Ok(Type::U64),
                "f64" => Ok(Type::F64),
                "bool" => Ok(Type::Bool),
                "str" => Ok(Type::Str),
                other => Err(format!("unknown type '{}'", other)),
            },
            TypeExpr::Unit(_) => Ok(Type::Unit),
        }
    }

    #[inline]
    pub fn is_integer(&self) -> bool {
        matches!(self, Type::I32 | Type::I64 | Type::U32 | Type::U64)
    }

    #[inline]
    pub fn is_signed_numeric(&self) -> bool {
        matches!(self, Type::I32 | Type::I64 | Type::F64)
    }

    #[inline]
    pub fn is_numeric(&self) -> bool {
        matches!(
            self,
            Type::I32 | Type::I64 | Type::U32 | Type::U64 | Type::F64
        )
    }

    #[inline]
    pub fn is_bool(&self) -> bool {
        matches!(self, Type::Bool)
    }

    #[inline]
    pub fn is_str(&self) -> bool {
        matches!(self, Type::Str)
    }

    #[inline]
    pub fn is_printable(&self) -> bool {
        matches!(
            self,
            Type::Str | Type::I32 | Type::I64 | Type::U32 | Type::U64 | Type::Bool
        )
    }

    #[inline]
    pub fn is_error(&self) -> bool {
        matches!(self, Type::Error)
    }
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Type::I32 => write!(f, "i32"),
            Type::I64 => write!(f, "i64"),
            Type::U32 => write!(f, "u32"),
            Type::U64 => write!(f, "u64"),
            Type::F64 => write!(f, "f64"),
            Type::Bool => write!(f, "bool"),
            Type::Str => write!(f, "str"),
            Type::Unit => write!(f, "()"),
            Type::Error => write!(f, "<error>"),
        }
    }
}
