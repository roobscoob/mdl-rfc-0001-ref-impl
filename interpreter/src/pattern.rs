use std::collections::HashMap;

use mdl::document::{Document, DocumentNode, InlineNode};
use mdl::instruction::template::{DocumentPattern, InlinePattern, Template};

use crate::runtime_value::RuntimeValue;

/// Attempt to match a RuntimeValue against a Template pattern.
/// Returns Some(bindings) if the match succeeds, None otherwise.
pub fn match_pattern(
    template: &Template,
    value: &RuntimeValue,
) -> Option<HashMap<String, RuntimeValue>> {
    let mut bindings = HashMap::new();
    if match_inner(template, value, &mut bindings) {
        Some(bindings)
    } else {
        None
    }
}

fn match_inner(
    template: &Template,
    value: &RuntimeValue,
    bindings: &mut HashMap<String, RuntimeValue>,
) -> bool {
    match template {
        Template::NumberLiteral(n) => {
            matches!(value, RuntimeValue::Number(v) if (*v - n).abs() < f64::EPSILON)
        }
        Template::StringLiteral(s) => {
            matches!(value, RuntimeValue::String(v) if v == s)
        }
        Template::BooleanLiteral(b) => {
            matches!(value, RuntimeValue::Boolean(v) if v == b)
        }
        Template::UnitLiteral => matches!(value, RuntimeValue::Unit),
        Template::Wildcard => true,

        Template::Binding(name) => {
            bindings.insert(name.clone(), value.clone());
            true
        }

        Template::Strikethrough(inner) => match value {
            RuntimeValue::Strikethrough(payload) => {
                if let Some(inner_pattern) = inner {
                    use crate::runtime_value::StrikethroughPayload;
                    let inner_val = match payload {
                        StrikethroughPayload::Eager(v) => v.as_ref().clone(),
                        StrikethroughPayload::Lazy(_) | StrikethroughPayload::Template(_) => {
                            // Can't evaluate lazy AST/template in pattern matching (no env);
                            // treat as opaque
                            RuntimeValue::Unit
                        }
                    };
                    match_inner(inner_pattern, &inner_val, bindings)
                } else {
                    true
                }
            }
            _ => false,
        },

        Template::DocumentPattern(doc_pattern) => {
            match value {
                RuntimeValue::Document(doc) => match_document_pattern(doc_pattern, doc, bindings),
                _ => false,
            }
        }

        Template::Alternation(alternatives) => {
            alternatives.iter().any(|alt| {
                let mut alt_bindings = HashMap::new();
                if match_inner(alt, value, &mut alt_bindings) {
                    bindings.extend(alt_bindings);
                    true
                } else {
                    false
                }
            })
        }

        Template::Compound(sub_patterns) => {
            // Compound patterns match multiple values positionally
            // For now, this is a simplified implementation
            match value {
                RuntimeValue::Document(doc) if doc.nodes.len() == sub_patterns.len() => {
                    for (pattern, node) in sub_patterns.iter().zip(&doc.nodes) {
                        let node_val = RuntimeValue::Document(mdl::document::Document {
                            nodes: vec![node.clone()],
                        });
                        if !match_inner(pattern, &node_val, bindings) {
                            return false;
                        }
                    }
                    true
                }
                _ => false,
            }
        }
    }
}

/// Match a DocumentPattern against a Document.
fn match_document_pattern(
    pattern: &DocumentPattern,
    doc: &Document,
    bindings: &mut HashMap<String, RuntimeValue>,
) -> bool {
    match pattern {
        DocumentPattern::Inline(inline_pat) => {
            // Try to find a matching inline in the document's paragraphs
            for node in &doc.nodes {
                if let DocumentNode::Paragraph(inlines) = node {
                    // Single inline in paragraph: match directly
                    if inlines.len() == 1 {
                        if match_inline_pattern(inline_pat, &inlines[0], bindings) {
                            return true;
                        }
                    }
                }
            }
            false
        }
        DocumentPattern::Block(_block_pat) => {
            // Block-level pattern matching — not yet needed
            false
        }
    }
}

/// Match an InlinePattern against an InlineNode.
fn match_inline_pattern(
    pattern: &InlinePattern,
    inline: &InlineNode,
    bindings: &mut HashMap<String, RuntimeValue>,
) -> bool {
    match (pattern, inline) {
        (InlinePattern::Text(expected), InlineNode::Text(actual)) => expected == actual,
        (InlinePattern::Capture(name), node) => {
            let val = inline_node_to_value(node);
            bindings.insert(name.clone(), val);
            true
        }
        (InlinePattern::Strong(sub_patterns), InlineNode::Strong(children)) => {
            match_inline_children(sub_patterns, children, bindings)
        }
        (InlinePattern::Emphasis(sub_patterns), InlineNode::Emphasis(children)) => {
            match_inline_children(sub_patterns, children, bindings)
        }
        (InlinePattern::Strikethrough(sub_patterns), InlineNode::Strikethrough(children)) => {
            match_inline_children(sub_patterns, children, bindings)
        }
        (InlinePattern::CodeSpan(expected), InlineNode::CodeSpan(actual)) => expected == actual,
        _ => false,
    }
}

/// Match a list of InlinePatterns against a list of InlineNodes.
fn match_inline_children(
    patterns: &[InlinePattern],
    children: &[InlineNode],
    bindings: &mut HashMap<String, RuntimeValue>,
) -> bool {
    // Special case: single Capture pattern matches all children as concatenated text
    if patterns.len() == 1 {
        if let InlinePattern::Capture(name) = &patterns[0] {
            let text: String = children.iter().map(|c| inline_node_to_string(c)).collect();
            bindings.insert(name.clone(), RuntimeValue::String(text));
            return true;
        }
    }

    // Otherwise, match positionally
    if patterns.len() != children.len() {
        return false;
    }
    for (pat, child) in patterns.iter().zip(children) {
        if !match_inline_pattern(pat, child, bindings) {
            return false;
        }
    }
    true
}

/// Convert an InlineNode to a RuntimeValue.
fn inline_node_to_value(node: &InlineNode) -> RuntimeValue {
    match node {
        InlineNode::Text(s) => RuntimeValue::String(s.clone()),
        _ => RuntimeValue::String(inline_node_to_string(node)),
    }
}

/// Convert an InlineNode to its text content.
fn inline_node_to_string(node: &InlineNode) -> String {
    match node {
        InlineNode::Text(s) => s.clone(),
        InlineNode::Strong(children) | InlineNode::Emphasis(children) | InlineNode::Strikethrough(children) => {
            children.iter().map(|c| inline_node_to_string(c)).collect()
        }
        InlineNode::CodeSpan(s) => s.clone(),
        InlineNode::SoftBreak => " ".to_string(),
        InlineNode::HardBreak => "\n".to_string(),
        _ => String::new(),
    }
}
