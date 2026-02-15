pub mod template;
pub mod value;

use std::ops::Range;

use crate::instruction::value::Value;

/// A single executable instruction parsed from an ordered list item.
#[derive(Debug, Clone)]
pub enum Instruction {
    /// Variable assignment: `variable = expression`
    Assignment {
        variable: String,
        value: Value,
        span: Range<usize>,
    },
    /// Expression evaluation (side effects only, result discarded).
    Expression {
        value: Value,
        span: Range<usize>,
    },
}

impl Instruction {
    pub fn span(&self) -> &Range<usize> {
        match self {
            Instruction::Assignment { span, .. } => span,
            Instruction::Expression { span, .. } => span,
        }
    }
}
