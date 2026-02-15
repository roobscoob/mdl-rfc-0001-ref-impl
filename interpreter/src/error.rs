use std::fmt;
use std::ops::Range;

#[derive(Debug)]
pub enum RuntimeError {
    TypeError { expected: String, got: String },
    UndefinedVariable(String),
    UndefinedBlock(String),
    ArgumentOutOfBounds(usize),
    NonExhaustiveMatch,
    DivisionByZero,
    NoEntryPoint,
    ImportNotImplemented(String),
    IoError(String),
    StackOverflow,
    Custom(String),
}

impl fmt::Display for RuntimeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeError::TypeError { expected, got } => {
                write!(f, "type error: expected {}, got {}", expected, got)
            }
            RuntimeError::UndefinedVariable(name) => write!(f, "undefined variable: {}", name),
            RuntimeError::UndefinedBlock(name) => write!(f, "undefined block: {}", name),
            RuntimeError::ArgumentOutOfBounds(idx) => {
                write!(f, "argument index {} out of bounds", idx)
            }
            RuntimeError::NonExhaustiveMatch => {
                write!(f, "non-exhaustive match: no arm matched")
            }
            RuntimeError::DivisionByZero => write!(f, "division by zero"),
            RuntimeError::NoEntryPoint => write!(f, "no entry point: no top-level block"),
            RuntimeError::ImportNotImplemented(path) => {
                write!(f, "imports not yet implemented: {}", path)
            }
            RuntimeError::IoError(msg) => write!(f, "I/O error: {}", msg),
            RuntimeError::StackOverflow => write!(f, "stack overflow"),
            RuntimeError::Custom(msg) => write!(f, "{}", msg),
        }
    }
}

impl std::error::Error for RuntimeError {}

/// A runtime error or warning enriched with source location information.
#[derive(Debug)]
pub struct DiagnosticError {
    pub error: RuntimeError,
    pub span: Option<Range<usize>>,
    pub source_id: usize,
    pub is_warning: bool,
}

impl DiagnosticError {
    /// Create a warning diagnostic with a source span.
    pub fn warning(message: String, span: Range<usize>, source_id: usize) -> Self {
        DiagnosticError {
            error: RuntimeError::Custom(message),
            span: Some(span),
            source_id,
            is_warning: true,
        }
    }
}

impl From<RuntimeError> for DiagnosticError {
    fn from(error: RuntimeError) -> Self {
        DiagnosticError {
            error,
            span: None,
            source_id: 0,
            is_warning: false,
        }
    }
}

impl fmt::Display for DiagnosticError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.error.fmt(f)
    }
}

impl std::error::Error for DiagnosticError {}
