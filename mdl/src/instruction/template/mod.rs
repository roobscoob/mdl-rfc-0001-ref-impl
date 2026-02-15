pub mod template_string;

/// A pattern template used in match arms.
#[derive(Debug, Clone)]
pub enum Template {
    /// Match a specific number literal.
    NumberLiteral(f64),
    /// Match a specific string literal.
    StringLiteral(String),
    /// Match a specific boolean.
    BooleanLiteral(bool),
    /// Match unit.
    UnitLiteral,
    /// Match a strikethrough (null) value, optionally matching the inner document.
    Strikethrough(Option<Box<Template>>),
    /// Bind the matched value to a variable name.
    Binding(String),
    /// Match a Document structure.
    DocumentPattern(DocumentPattern),
    /// Compound pattern: positional sub-patterns for compound match expressions.
    Compound(Vec<Template>),
    /// Wildcard: matches anything.
    Wildcard,
    /// Alternation: matches if any sub-pattern matches (a | b | c).
    Alternation(Vec<Template>),
}

/// Pattern for matching Markdown document structure.
#[derive(Debug, Clone)]
pub enum DocumentPattern {
    Inline(InlinePattern),
    Block(BlockPattern),
}

/// Pattern for matching inline Markdown elements.
#[derive(Debug, Clone)]
pub enum InlinePattern {
    Text(String),
    Strong(Vec<InlinePattern>),
    Emphasis(Vec<InlinePattern>),
    Strikethrough(Vec<InlinePattern>),
    CodeSpan(String),
    Link {
        dest: String,
        content: Vec<InlinePattern>,
    },
    /// Capture binding within a document pattern: {name}
    Capture(String),
}

/// Pattern for matching block-level Markdown elements.
#[derive(Debug, Clone)]
pub enum BlockPattern {
    Paragraph(Vec<InlinePattern>),
    Heading {
        level: u8,
        content: Vec<InlinePattern>,
    },
    CodeBlock {
        language: Option<String>,
        content: Option<String>,
    },
}
