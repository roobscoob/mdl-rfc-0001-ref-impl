use crate::instruction::value::Value;

/// A string that can contain interpolated expressions.
/// Used in Print (**{expr}**), Interpolation, and Strikethrough (~~expr~~).
#[derive(Debug, Clone)]
pub struct TemplateString {
    pub parts: Vec<TemplateStringPart>,
}

#[derive(Debug, Clone)]
pub enum TemplateStringPart {
    /// Literal text content.
    Literal(String),
    /// An embedded expression to be evaluated and rendered.
    Expression(Value),
}

impl TemplateString {
    pub fn literal(s: impl Into<String>) -> Self {
        TemplateString {
            parts: vec![TemplateStringPart::Literal(s.into())],
        }
    }

    pub fn single_expression(v: Value) -> Self {
        TemplateString {
            parts: vec![TemplateStringPart::Expression(v)],
        }
    }
}
