use std::fmt;

/// A Document is a sequence of document nodes representing Markdown content.
/// This is the first-class Markdown AST type in markdownlang.
#[derive(Debug, Clone, PartialEq)]
pub struct Document {
    pub nodes: Vec<DocumentNode>,
}

impl Document {
    pub fn empty() -> Self {
        Document { nodes: Vec::new() }
    }

    pub fn is_empty(&self) -> bool {
        self.nodes.is_empty()
    }
}

/// A single node in the Markdown AST.
#[derive(Debug, Clone, PartialEq)]
pub enum DocumentNode {
    // Block-level
    Paragraph(Vec<InlineNode>),
    Heading {
        level: u8,
        content: Vec<InlineNode>,
    },
    CodeBlock {
        language: Option<String>,
        content: String,
    },
    Blockquote(Document),
    Table {
        alignments: Vec<ColumnAlignment>,
        headers: Vec<Vec<InlineNode>>,
        rows: Vec<Vec<Vec<InlineNode>>>,
    },
    OrderedList {
        start: u64,
        items: Vec<Document>,
    },
    UnorderedList {
        items: Vec<Document>,
    },

    // Separator
    HorizontalRule,
}

/// Inline elements that appear within a line of text.
/// Inline types nest freely within one another.
#[derive(Debug, Clone, PartialEq)]
pub enum InlineNode {
    Text(String),
    Strong(Vec<InlineNode>),
    Emphasis(Vec<InlineNode>),
    Strikethrough(Vec<InlineNode>),
    CodeSpan(String),
    Link {
        dest: String,
        title: String,
        content: Vec<InlineNode>,
    },
    Image {
        dest: String,
        title: String,
        alt: Vec<InlineNode>,
    },
    SoftBreak,
    HardBreak,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum ColumnAlignment {
    None,
    Left,
    Center,
    Right,
}

impl fmt::Display for Document {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for node in &self.nodes {
            write!(f, "{}", node)?;
        }
        Ok(())
    }
}

impl fmt::Display for DocumentNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DocumentNode::Paragraph(inlines) => {
                for inline in inlines {
                    write!(f, "{}", inline)?;
                }
                writeln!(f)
            }
            DocumentNode::Heading { level, content } => {
                for _ in 0..*level {
                    write!(f, "#")?;
                }
                write!(f, " ")?;
                for inline in content {
                    write!(f, "{}", inline)?;
                }
                writeln!(f)
            }
            DocumentNode::CodeBlock { language, content } => {
                write!(f, "```")?;
                if let Some(lang) = language {
                    write!(f, "{}", lang)?;
                }
                writeln!(f)?;
                write!(f, "{}", content)?;
                writeln!(f, "```")
            }
            DocumentNode::Blockquote(doc) => {
                let text = format!("{}", doc);
                for line in text.lines() {
                    writeln!(f, "> {}", line)?;
                }
                Ok(())
            }
            DocumentNode::Table { headers, rows, .. } => {
                write!(f, "|")?;
                for header in headers {
                    write!(f, " ")?;
                    for inline in header {
                        write!(f, "{}", inline)?;
                    }
                    write!(f, " |")?;
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
                        write!(f, " ")?;
                        for inline in cell {
                            write!(f, "{}", inline)?;
                        }
                        write!(f, " |")?;
                    }
                    writeln!(f)?;
                }
                Ok(())
            }
            DocumentNode::OrderedList { start, items } => {
                for (i, item) in items.iter().enumerate() {
                    write!(f, "{}. {}", *start as usize + i, item)?;
                }
                Ok(())
            }
            DocumentNode::UnorderedList { items } => {
                for item in items {
                    write!(f, "- {}", item)?;
                }
                Ok(())
            }
            DocumentNode::HorizontalRule => writeln!(f, "---"),
        }
    }
}

impl fmt::Display for InlineNode {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            InlineNode::Text(s) => write!(f, "{}", s),
            InlineNode::Strong(children) => {
                write!(f, "**")?;
                for child in children {
                    write!(f, "{}", child)?;
                }
                write!(f, "**")
            }
            InlineNode::Emphasis(children) => {
                write!(f, "*")?;
                for child in children {
                    write!(f, "{}", child)?;
                }
                write!(f, "*")
            }
            InlineNode::Strikethrough(children) => {
                write!(f, "~~")?;
                for child in children {
                    write!(f, "{}", child)?;
                }
                write!(f, "~~")
            }
            InlineNode::CodeSpan(code) => write!(f, "`{}`", code),
            InlineNode::Link { dest, content, .. } => {
                write!(f, "[")?;
                for child in content {
                    write!(f, "{}", child)?;
                }
                write!(f, "]({})", dest)
            }
            InlineNode::Image { dest, alt, .. } => {
                write!(f, "![")?;
                for child in alt {
                    write!(f, "{}", child)?;
                }
                write!(f, "]({})", dest)
            }
            InlineNode::SoftBreak => writeln!(f),
            InlineNode::HardBreak => writeln!(f),
        }
    }
}
