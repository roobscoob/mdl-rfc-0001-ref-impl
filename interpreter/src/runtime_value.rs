use std::fmt;

use mdl::document::Document;

/// A runtime value produced by evaluating an expression.
#[derive(Debug, Clone)]
pub enum RuntimeValue {
    Number(f64),
    Boolean(bool),
    String(String),
    Unit,
    Document(Document),
    /// Strikethrough/null: contains the unevaluated Document payload.
    Strikethrough(Document),
    /// Table: structured data with named columns and rows.
    /// One-row table = record, one-column table = array.
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<RuntimeValue>>,
    },
}

impl RuntimeValue {
    pub fn is_truthy(&self) -> bool {
        !self.is_falsy()
    }

    pub fn is_falsy(&self) -> bool {
        matches!(
            self,
            RuntimeValue::Boolean(false) | RuntimeValue::Strikethrough(_) | RuntimeValue::Unit
        )
    }

    pub fn type_name(&self) -> &'static str {
        match self {
            RuntimeValue::Number(_) => "Number",
            RuntimeValue::Boolean(_) => "Boolean",
            RuntimeValue::String(_) => "String",
            RuntimeValue::Unit => "Unit",
            RuntimeValue::Document(_) => "Document",
            RuntimeValue::Strikethrough(_) => "Strikethrough",
            RuntimeValue::Table { .. } => "Table",
        }
    }
}

impl fmt::Display for RuntimeValue {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RuntimeValue::Number(n) => {
                if n.is_finite() && *n == n.floor() && n.abs() < 1e15 {
                    write!(f, "{}", *n as i64)
                } else {
                    write!(f, "{}", n)
                }
            }
            RuntimeValue::Boolean(b) => write!(f, "{}", b),
            RuntimeValue::String(s) => write!(f, "{}", s),
            RuntimeValue::Unit => write!(f, "()"),
            RuntimeValue::Document(doc) => write!(f, "{}", doc),
            RuntimeValue::Strikethrough(doc) => {
                let s = format!("{}", doc);
                write!(f, "~~{}~~", s.trim())
            }
            RuntimeValue::Table { headers, rows } => {
                // Render as Markdown table
                write!(f, "|")?;
                for h in headers {
                    write!(f, " {} |", h)?;
                }
                writeln!(f)?;
                write!(f, "|")?;
                for _ in headers {
                    write!(f, "---|")?;
                }
                writeln!(f)?;
                for row in rows {
                    write!(f, "|")?;
                    for cell in row {
                        write!(f, " {} |", cell)?;
                    }
                    writeln!(f)?;
                }
                Ok(())
            }
        }
    }
}

impl PartialEq for RuntimeValue {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (RuntimeValue::Number(a), RuntimeValue::Number(b)) => a == b, // NaN != NaN per IEEE 754
            (RuntimeValue::Boolean(a), RuntimeValue::Boolean(b)) => a == b,
            (RuntimeValue::String(a), RuntimeValue::String(b)) => a == b,
            (RuntimeValue::Unit, RuntimeValue::Unit) => true,
            (RuntimeValue::Document(a), RuntimeValue::Document(b)) => a == b,
            (RuntimeValue::Strikethrough(a), RuntimeValue::Strikethrough(b)) => a == b,
            (
                RuntimeValue::Table {
                    headers: h1,
                    rows: r1,
                },
                RuntimeValue::Table {
                    headers: h2,
                    rows: r2,
                },
            ) => h1 == h2 && r1 == r2,
            _ => false,
        }
    }
}
