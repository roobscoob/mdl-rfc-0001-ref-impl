use std::ops::Range;

use crate::{
    block::reference::BlockReference,
    instruction::template::{Template, template_string::TemplateString},
};

#[derive(Debug, Clone)]
pub enum UnaryOperator {
    /// Arithmetic negation: -x
    Negation,
    /// Logical not: !x
    LogicalNot,
}

#[derive(Debug, Clone)]
pub enum BinaryOperator {
    Addition,
    Subtraction,
    Multiplication,
    Division,
    Modulo,
    LogicalAnd,
    LogicalOr,
    Equality,
    Inequality,
    GreaterThan,
    LessThan,
    GreaterThanOrEqual,
    LessThanOrEqual,
}

/// An expression AST node. Represents a value-producing expression in the language.
#[derive(Debug, Clone)]
pub enum Value {
    // Literals
    StringLiteral(String),
    NumberLiteral(f64),
    BooleanLiteral(bool),
    UnitLiteral,

    // References
    VariableReference(String, Range<usize>),
    PositionalArgumentReference(usize, Range<usize>), // #0, #1, etc.
    SpreadArgumentReference,                          // #*

    // Invocations
    /// [args](#block) -- invoke block, return Document
    BlockInvocation(Vec<Value>, BlockReference),
    /// ![args](#block) -- invoke block, evaluate Document
    EvaluatedBlockInvocation(Vec<Value>, BlockReference),

    // Inline Markdown semantics
    /// **{expr}** -- print to stdout
    Print(TemplateString),
    /// String interpolation content
    Interpolation(TemplateString),
    /// ~~expr~~ -- capture as unevaluated Document (null/strikethrough)
    Strikethrough(TemplateString),

    // Operations
    UnaryOperation {
        operator: UnaryOperator,
        operand: Box<Value>,
    },
    BinaryOperation {
        operator: BinaryOperator,
        left: Box<Value>,
        right: Box<Value>,
    },

    // Control flow
    /// cond ? expr (two-operand: falsy -> Strikethrough)
    /// cond ? expr : expr (three-operand: standard ternary)
    Conditional {
        condition: Box<Value>,
        true_branch: Box<Value>,
        false_branch: Option<Box<Value>>,
    },

    /// Pattern match expression
    Match {
        value: Box<Value>,
        arms: Vec<(Template, Value)>,
        otherwise: Option<(Option<String>, Box<Value>)>,
    },
}
