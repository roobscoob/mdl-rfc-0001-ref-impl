use std::collections::HashMap;

use mdl::instruction::template::Template;

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
            RuntimeValue::Strikethrough(doc) => {
                if let Some(inner_pattern) = inner {
                    match_inner(
                        inner_pattern,
                        &RuntimeValue::Document(doc.clone()),
                        bindings,
                    )
                } else {
                    true
                }
            }
            _ => false,
        },

        Template::DocumentPattern(_doc_pattern) => {
            // TODO: full document structure matching
            matches!(value, RuntimeValue::Document(_))
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
