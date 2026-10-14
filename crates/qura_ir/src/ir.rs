use std::fmt;

/// Backend-independent primitive types for Qura IR.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Type {
    I32,
    I64,
    U32,
    U64,
    F64,
    Bool,
    Str,
    Unit,
}

impl Type {
    #[inline]
    pub fn is_printable(&self) -> bool {
        matches!(
            self,
            Type::Str | Type::I32 | Type::I64 | Type::U32 | Type::U64 | Type::Bool
        )
    }

    #[inline]
    pub fn is_integer(&self) -> bool {
        matches!(self, Type::I32 | Type::I64 | Type::U32 | Type::U64)
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
        }
    }
}

impl From<qura_sema::Type> for Type {
    fn from(sema_ty: qura_sema::Type) -> Self {
        match sema_ty {
            qura_sema::Type::I32 => Type::I32,
            qura_sema::Type::I64 => Type::I64,
            qura_sema::Type::U32 => Type::U32,
            qura_sema::Type::U64 => Type::U64,
            qura_sema::Type::F64 => Type::F64,
            qura_sema::Type::Bool => Type::Bool,
            qura_sema::Type::Str => Type::Str,
            qura_sema::Type::Unit => Type::Unit,
            qura_sema::Type::Error => panic!("cannot convert error type to QIR type"),
        }
    }
}

/// Unique SSA virtual register value identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Value(pub u32);

impl fmt::Display for Value {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "%{}", self.0)
    }
}

/// Basic block identifier within a function CFG.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct BlockId(pub u32);

impl fmt::Display for BlockId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "block{}", self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinaryOp {
    Add,
    Sub,
    Mul,
    Div,
    Rem,
    Eq,
    Ne,
    Lt,
    Le,
    Gt,
    Ge,
    And,
    Or,
}

impl BinaryOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            BinaryOp::Add => "add",
            BinaryOp::Sub => "sub",
            BinaryOp::Mul => "mul",
            BinaryOp::Div => "div",
            BinaryOp::Rem => "rem",
            BinaryOp::Eq => "eq",
            BinaryOp::Ne => "ne",
            BinaryOp::Lt => "lt",
            BinaryOp::Le => "le",
            BinaryOp::Gt => "gt",
            BinaryOp::Ge => "ge",
            BinaryOp::And => "and",
            BinaryOp::Or => "or",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnaryOp {
    Neg,
    Not,
}

impl UnaryOp {
    pub fn as_str(&self) -> &'static str {
        match self {
            UnaryOp::Neg => "neg",
            UnaryOp::Not => "not",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Instruction {
    ConstInt(i64, Type),
    ConstFloat(f64),
    ConstBool(bool),
    ConstStr(String),
    Binary { op: BinaryOp, lhs: Value, rhs: Value },
    Unary { op: UnaryOp, val: Value },
    Call { func: String, args: Vec<Value> },
    Print { val: Value, newline: bool },
}

impl fmt::Display for Instruction {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Instruction::ConstInt(val, ty) => write!(f, "iconst.{} {}", ty, val),
            Instruction::ConstFloat(val) => write!(f, "fconst.f64 {}", val),
            Instruction::ConstBool(val) => write!(f, "bconst.bool {}", val),
            Instruction::ConstStr(val) => write!(f, "sconst.str {:?}", val),
            Instruction::Binary { op, lhs, rhs } => {
                write!(f, "{} {}, {}", op.as_str(), lhs, rhs)
            }
            Instruction::Unary { op, val } => write!(f, "{} {}", op.as_str(), val),
            Instruction::Call { func, args } => {
                write!(f, "call @{}(", func)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Instruction::Print { val, newline: false } => write!(f, "print {}", val),
            Instruction::Print { val, newline: true } => write!(f, "println {}", val),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Terminator {
    Return(Option<Value>),
    Branch {
        target: BlockId,
        args: Vec<Value>,
    },
    CondBranch {
        cond: Value,
        then_block: BlockId,
        then_args: Vec<Value>,
        else_block: BlockId,
        else_args: Vec<Value>,
    },
}

impl fmt::Display for Terminator {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Terminator::Return(Some(val)) => write!(f, "ret {}", val),
            Terminator::Return(None) => write!(f, "ret"),
            Terminator::Branch { target, args } => {
                write!(f, "br {}(", target)?;
                for (i, arg) in args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
            Terminator::CondBranch {
                cond,
                then_block,
                then_args,
                else_block,
                else_args,
            } => {
                write!(f, "cond_br {}, {}(", cond, then_block)?;
                for (i, arg) in then_args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, "), {}(", else_block)?;
                for (i, arg) in else_args.iter().enumerate() {
                    if i > 0 {
                        write!(f, ", ")?;
                    }
                    write!(f, "{}", arg)?;
                }
                write!(f, ")")
            }
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct BasicBlock {
    pub id: BlockId,
    pub params: Vec<(Value, Type)>,
    pub instructions: Vec<(Value, Instruction)>,
    pub terminator: Terminator,
}

impl fmt::Display for BasicBlock {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}(", self.id)?;
        for (i, (val, ty)) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", val, ty)?;
        }
        writeln!(f, "):")?;

        for (val, inst) in &self.instructions {
            writeln!(f, "    {} = {}", val, inst)?;
        }
        writeln!(f, "    {}", self.terminator)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Function {
    pub name: String,
    pub params: Vec<(Value, Type)>,
    pub return_type: Type,
    pub blocks: Vec<BasicBlock>,
}

impl fmt::Display for Function {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "func @{}(", self.name)?;
        for (i, (val, ty)) in self.params.iter().enumerate() {
            if i > 0 {
                write!(f, ", ")?;
            }
            write!(f, "{}: {}", val, ty)?;
        }
        writeln!(f, ") -> {} {{", self.return_type)?;

        for block in &self.blocks {
            write!(f, "{}", block)?;
        }

        writeln!(f, "}}")
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Module {
    pub name: String,
    pub functions: Vec<Function>,
}

impl Module {
    pub fn new(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            functions: Vec::new(),
        }
    }
}

impl fmt::Display for Module {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "; Qura IR Module: {}", self.name)?;
        for func in &self.functions {
            writeln!(f)?;
            write!(f, "{}", func)?;
        }
        Ok(())
    }
}
