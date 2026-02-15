use std::io::Write;
use std::ops::Range;

use mdl::instruction::template::template_string::{TemplateString, TemplateStringPart};
use mdl::instruction::value::{BinaryOperator, UnaryOperator, Value};

use crate::environment::{Environment, VariableLookup};
use crate::error::{DiagnosticError, RuntimeError};
use crate::executor::BlockRegistry;
use crate::pattern;
use crate::runtime_value::RuntimeValue;

pub const MAX_DEPTH: usize = 128;

/// Evaluate a Value AST node to produce a RuntimeValue.
pub fn evaluate(
    value: &Value,
    env: &mut Environment,
    blocks: &mut BlockRegistry,
    output: &mut dyn Write,
    depth: usize,
    diagnostics: &mut Vec<DiagnosticError>,
    source_id: usize,
    instruction_span: &Range<usize>,
) -> Result<RuntimeValue, DiagnosticError> {
    if depth > MAX_DEPTH {
        return Err(RuntimeError::StackOverflow.into());
    }

    match value {
        // --- Literals ---
        Value::NumberLiteral(n) => Ok(RuntimeValue::Number(*n)),
        Value::StringLiteral(s) => Ok(RuntimeValue::String(s.clone())),
        Value::BooleanLiteral(b) => Ok(RuntimeValue::Boolean(*b)),
        Value::UnitLiteral => Ok(RuntimeValue::Unit),

        // --- References ---
        Value::VariableReference(name, value_span) => {
            let span = if value_span.is_empty() { instruction_span.clone() } else { value_span.clone() };
            match env.get_variable_info(name) {
                VariableLookup::Found { value, cross_scope, non_lexical_scope } => {
                    let value = value.clone();
                    if non_lexical_scope {
                        diagnostics.push(DiagnosticError::warning(
                            format!(
                                "reading variable '{}' from a non-lexical scope (undefined behavior)",
                                name
                            ),
                            span,
                            source_id,
                        ));
                    } else if !cross_scope {
                        // Track same-scope reads for same-fence UB detection
                        env.record_fence_read(name, span);
                    }
                    Ok(value)
                }
                VariableLookup::HoistedUnassigned => {
                    // Track for same-fence UB detection even when unassigned
                    env.record_fence_read(name, span.clone());
                    diagnostics.push(DiagnosticError::warning(
                        format!(
                            "reading variable '{}' before assignment (undefined behavior)",
                            name
                        ),
                        span,
                        source_id,
                    ));
                    Ok(RuntimeValue::Unit)
                }
                VariableLookup::NotFound => {
                    let mut err = DiagnosticError::from(RuntimeError::UndefinedVariable(name.clone()));
                    err.span = Some(span);
                    err.source_id = source_id;
                    Err(err)
                }
            }
        },

        Value::PositionalArgumentReference(idx, value_span) => {
            let span = if value_span.is_empty() { instruction_span.clone() } else { value_span.clone() };
            env.get_argument(*idx)
                .cloned()
                .ok_or_else(|| {
                    let mut err = DiagnosticError::from(RuntimeError::ArgumentOutOfBounds(*idx));
                    err.span = Some(span.clone());
                    err.source_id = source_id;
                    err
                })
        },

        Value::SpreadArgumentReference => {
            let args = env.get_all_arguments();
            Ok(RuntimeValue::String(format!(
                "[{}]",
                args.iter()
                    .map(|a| a.to_string())
                    .collect::<Vec<_>>()
                    .join(", ")
            )))
        }

        // --- Operations ---
        Value::UnaryOperation { operator, operand } => {
            let val = evaluate(
                operand,
                env,
                blocks,
                output,
                depth + 1,
                diagnostics,
                source_id,
                instruction_span,
            )?;
            // Demand-evaluate Strikethrough operands for operations that need concrete types
            let val = match operator {
                UnaryOperator::Negation => {
                    demand(val, env, blocks, output, depth + 1, diagnostics)?
                }
                UnaryOperator::LogicalNot => val, // is_falsy handles Strikethrough directly
            };
            match operator {
                UnaryOperator::Negation => {
                    let n = coerce_number(&val)?;
                    Ok(RuntimeValue::Number(-n))
                }
                UnaryOperator::LogicalNot => Ok(RuntimeValue::Boolean(val.is_falsy())),
            }
        }

        Value::BinaryOperation {
            operator,
            left,
            right,
        } => {
            let l = evaluate(
                left,
                env,
                blocks,
                output,
                depth + 1,
                diagnostics,
                source_id,
                instruction_span,
            )?;
            let r = evaluate(
                right,
                env,
                blocks,
                output,
                depth + 1,
                diagnostics,
                source_id,
                instruction_span,
            )?;
            // Demand-evaluate Strikethrough operands for arithmetic/comparison ops
            let needs_demand = !matches!(
                operator,
                BinaryOperator::Equality
                    | BinaryOperator::Inequality
                    | BinaryOperator::LogicalAnd
                    | BinaryOperator::LogicalOr
            );
            let l = if needs_demand {
                demand(l, env, blocks, output, depth + 1, diagnostics)?
            } else {
                l
            };
            let r = if needs_demand {
                demand(r, env, blocks, output, depth + 1, diagnostics)?
            } else {
                r
            };
            Ok(eval_binary_op(operator, &l, &r)?)
        }

        // --- Print ---
        Value::Print(template) => {
            let s = eval_template_string(
                template,
                env,
                blocks,
                output,
                depth + 1,
                diagnostics,
                source_id,
                instruction_span,
            )?;
            writeln!(output, "{}", s)
                .map_err(|e| DiagnosticError::from(RuntimeError::IoError(e.to_string())))?;
            Ok(RuntimeValue::Unit)
        }

        // --- Interpolation ---
        Value::Interpolation(template) => {
            let s = eval_template_string(
                template,
                env,
                blocks,
                output,
                depth + 1,
                diagnostics,
                source_id,
                instruction_span,
            )?;
            Ok(RuntimeValue::String(s))
        }

        // --- Strikethrough ---
        Value::Strikethrough(template) => {
            // Check if template contains invocations (side effects)
            let has_invocations = template.parts.iter().any(|p| matches!(p,
                TemplateStringPart::Expression(Value::BlockInvocation(..))
                | TemplateStringPart::Expression(Value::EvaluatedBlockInvocation(..))
            ));

            if has_invocations {
                // Store as unevaluated template to prevent side effects
                Ok(RuntimeValue::Strikethrough(
                    crate::runtime_value::StrikethroughPayload::Template(Box::new(template.clone())),
                ))
            } else {
                // No invocations: eagerly evaluate interpolations
                let inner = evaluate_template_to_value(
                    template, env, blocks, output, depth + 1, diagnostics, source_id, instruction_span,
                )?;
                Ok(RuntimeValue::Strikethrough(
                    crate::runtime_value::StrikethroughPayload::Eager(Box::new(inner)),
                ))
            }
        }

        // --- Conditional ---
        Value::Conditional {
            condition,
            true_branch,
            false_branch,
        } => {
            let cond_val = evaluate(
                condition,
                env,
                blocks,
                output,
                depth + 1,
                diagnostics,
                source_id,
                instruction_span,
            )?;
            if cond_val.is_truthy() {
                evaluate(
                    true_branch,
                    env,
                    blocks,
                    output,
                    depth + 1,
                    diagnostics,
                    source_id,
                    instruction_span,
                )
            } else {
                match false_branch {
                    Some(fb) => evaluate(
                        fb,
                        env,
                        blocks,
                        output,
                        depth + 1,
                        diagnostics,
                        source_id,
                        instruction_span,
                    ),
                    None => {
                        // Two-operand conditional: falsy → Strikethrough of unevaluated expression
                        Ok(RuntimeValue::Strikethrough(
                            crate::runtime_value::StrikethroughPayload::Lazy(
                                Box::new(true_branch.as_ref().clone()),
                            ),
                        ))
                    }
                }
            }
        }

        // --- Match ---
        Value::Match {
            value: scrutinee,
            arms,
            otherwise,
        } => {
            let val = evaluate(
                scrutinee,
                env,
                blocks,
                output,
                depth + 1,
                diagnostics,
                source_id,
                instruction_span,
            )?;

            for (template, result) in arms {
                if let Some(bindings) = pattern::match_pattern(template, &val) {
                    for (name, bound_val) in bindings {
                        env.set_variable(&name, bound_val);
                    }
                    return evaluate(
                        result,
                        env,
                        blocks,
                        output,
                        depth + 1,
                        diagnostics,
                        source_id,
                        instruction_span,
                    );
                }
            }

            if let Some((binding, result)) = otherwise {
                if let Some(name) = binding {
                    env.set_variable(name, val);
                }
                return evaluate(
                    result,
                    env,
                    blocks,
                    output,
                    depth + 1,
                    diagnostics,
                    source_id,
                    instruction_span,
                );
            }

            Err(RuntimeError::NonExhaustiveMatch.into())
        }

        // --- Block invocation ---
        Value::BlockInvocation(args, block_ref) => {
            let evaluated_args: Vec<RuntimeValue> = args
                .iter()
                .map(|a| {
                    evaluate(
                        a,
                        env,
                        blocks,
                        output,
                        depth + 1,
                        diagnostics,
                        source_id,
                        instruction_span,
                    )
                })
                .collect::<Result<_, _>>()?;

            crate::executor::invoke_block(
                block_ref,
                evaluated_args,
                env,
                blocks,
                output,
                false,
                depth + 1,
                diagnostics,
            )
        }

        Value::EvaluatedBlockInvocation(args, block_ref) => {
            let evaluated_args: Vec<RuntimeValue> = args
                .iter()
                .map(|a| {
                    evaluate(
                        a,
                        env,
                        blocks,
                        output,
                        depth + 1,
                        diagnostics,
                        source_id,
                        instruction_span,
                    )
                })
                .collect::<Result<_, _>>()?;

            crate::executor::invoke_block(
                block_ref,
                evaluated_args,
                env,
                blocks,
                output,
                true,
                depth + 1,
                diagnostics,
            )
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

/// Demand-evaluate a Strikethrough value.
/// - Eager: return the already-evaluated inner value.
/// - Lazy: evaluate the stored AST expression now.
/// - Template: evaluate the stored template now (including invocations).
/// If the value is not a Strikethrough, returns it unchanged.
fn demand(
    val: RuntimeValue,
    env: &mut Environment,
    blocks: &mut BlockRegistry,
    output: &mut dyn Write,
    depth: usize,
    diagnostics: &mut Vec<DiagnosticError>,
) -> Result<RuntimeValue, DiagnosticError> {
    use crate::runtime_value::StrikethroughPayload;
    match val {
        RuntimeValue::Strikethrough(StrikethroughPayload::Eager(inner)) => Ok(*inner),
        RuntimeValue::Strikethrough(StrikethroughPayload::Lazy(ast)) => {
            let source_id = blocks.source_id;
            let span = 0..0;
            evaluate(&ast, env, blocks, output, depth, diagnostics, source_id, &span)
        }
        RuntimeValue::Strikethrough(StrikethroughPayload::Template(ts)) => {
            let source_id = blocks.source_id;
            let span = 0..0;
            evaluate_template_to_value(
                &ts, env, blocks, output, depth, diagnostics, source_id, &span,
            )
        }
        other => Ok(other),
    }
}

fn coerce_number(val: &RuntimeValue) -> Result<f64, RuntimeError> {
    match val {
        RuntimeValue::Number(n) => Ok(*n),
        other => Err(RuntimeError::TypeError {
            expected: "Number".to_string(),
            got: other.type_name().to_string(),
        }),
    }
}

fn eval_binary_op(
    op: &BinaryOperator,
    left: &RuntimeValue,
    right: &RuntimeValue,
) -> Result<RuntimeValue, RuntimeError> {
    match op {
        BinaryOperator::Addition => match (left, right) {
            (RuntimeValue::Number(a), RuntimeValue::Number(b)) => {
                Ok(RuntimeValue::Number(a + b))
            }
            (RuntimeValue::String(a), RuntimeValue::String(b)) => {
                Ok(RuntimeValue::String(format!("{}{}", a, b)))
            }
            _ => Err(RuntimeError::TypeError {
                expected: "matching numeric or string types".to_string(),
                got: format!("{} + {}", left.type_name(), right.type_name()),
            }),
        },
        BinaryOperator::Subtraction => numeric_binop(left, right, |a, b| a - b),
        BinaryOperator::Multiplication => numeric_binop(left, right, |a, b| a * b),
        BinaryOperator::Division => {
            let a = coerce_number(left)?;
            let b = coerce_number(right)?;
            if b == 0.0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Ok(RuntimeValue::Number(a / b))
        }
        BinaryOperator::Modulo => {
            let a = coerce_number(left)?;
            let b = coerce_number(right)?;
            if b == 0.0 {
                return Err(RuntimeError::DivisionByZero);
            }
            Ok(RuntimeValue::Number(a % b))
        }
        BinaryOperator::Equality => Ok(RuntimeValue::Boolean(left == right)),
        BinaryOperator::Inequality => Ok(RuntimeValue::Boolean(left != right)),
        BinaryOperator::GreaterThan => numeric_cmp(left, right, |a, b| a > b),
        BinaryOperator::LessThan => numeric_cmp(left, right, |a, b| a < b),
        BinaryOperator::GreaterThanOrEqual => numeric_cmp(left, right, |a, b| a >= b),
        BinaryOperator::LessThanOrEqual => numeric_cmp(left, right, |a, b| a <= b),
        BinaryOperator::LogicalAnd => {
            Ok(RuntimeValue::Boolean(left.is_truthy() && right.is_truthy()))
        }
        BinaryOperator::LogicalOr => {
            Ok(RuntimeValue::Boolean(left.is_truthy() || right.is_truthy()))
        }
    }
}

fn numeric_binop(
    left: &RuntimeValue,
    right: &RuntimeValue,
    f: impl Fn(f64, f64) -> f64,
) -> Result<RuntimeValue, RuntimeError> {
    let a = coerce_number(left)?;
    let b = coerce_number(right)?;
    Ok(RuntimeValue::Number(f(a, b)))
}

fn numeric_cmp(
    left: &RuntimeValue,
    right: &RuntimeValue,
    f: impl Fn(f64, f64) -> bool,
) -> Result<RuntimeValue, RuntimeError> {
    let a = coerce_number(left)?;
    let b = coerce_number(right)?;
    Ok(RuntimeValue::Boolean(f(a, b)))
}

pub fn eval_template_string(
    ts: &TemplateString,
    env: &mut Environment,
    blocks: &mut BlockRegistry,
    output: &mut dyn Write,
    depth: usize,
    diagnostics: &mut Vec<DiagnosticError>,
    source_id: usize,
    instruction_span: &Range<usize>,
) -> Result<String, DiagnosticError> {
    let mut result = String::new();
    for part in &ts.parts {
        match part {
            TemplateStringPart::Literal(s) => result.push_str(s),
            TemplateStringPart::Expression(expr) => {
                let val = evaluate(
                    expr,
                    env,
                    blocks,
                    output,
                    depth,
                    diagnostics,
                    source_id,
                    instruction_span,
                )?;
                result.push_str(&val.to_string());
            }
        }
    }
    Ok(result)
}

/// Evaluate a TemplateString to a RuntimeValue.
/// - If it has a single Expression part (no literals), returns the evaluated expression directly.
/// - Otherwise, concatenates all parts into a String.
fn evaluate_template_to_value(
    ts: &TemplateString,
    env: &mut Environment,
    blocks: &mut BlockRegistry,
    output: &mut dyn Write,
    depth: usize,
    diagnostics: &mut Vec<DiagnosticError>,
    source_id: usize,
    instruction_span: &Range<usize>,
) -> Result<RuntimeValue, DiagnosticError> {
    // Single expression with no surrounding text: return as its native type
    if ts.parts.len() == 1 {
        if let TemplateStringPart::Expression(expr) = &ts.parts[0] {
            return evaluate(
                expr, env, blocks, output, depth, diagnostics, source_id, instruction_span,
            );
        }
    }

    // Mixed parts or pure literal: concatenate to string
    let mut result = String::new();
    for part in &ts.parts {
        match part {
            TemplateStringPart::Literal(s) => result.push_str(s),
            TemplateStringPart::Expression(expr) => {
                let val = evaluate(
                    expr, env, blocks, output, depth, diagnostics, source_id, instruction_span,
                )?;
                result.push_str(&val.to_string());
            }
        }
    }
    Ok(RuntimeValue::String(result))
}

/// Render a Value AST node as a Markdown-like string for struck representation.
pub fn value_to_markdown_text(value: &Value) -> String {
    match value {
        Value::StringLiteral(s) => format!("\"{}\"", s),
        Value::NumberLiteral(n) => {
            if n.is_finite() && *n == n.floor() && n.abs() < 1e15 {
                format!("{}", *n as i64)
            } else {
                format!("{}", n)
            }
        }
        Value::BooleanLiteral(b) => format!("{}", b),
        Value::UnitLiteral => "()".to_string(),
        Value::VariableReference(name, _) => name.clone(),
        Value::PositionalArgumentReference(idx, _) => format!("#{}", idx),
        Value::SpreadArgumentReference => "#*".to_string(),
        Value::BlockInvocation(_, block_ref) => {
            format!("[](#{})", block_ref.block_name())
        }
        Value::EvaluatedBlockInvocation(_, block_ref) => {
            format!("![](#{})", block_ref.block_name())
        }
        Value::Print(ts) => {
            let inner = template_to_text(ts);
            format!("**{}**", inner)
        }
        _ => format!("{:?}", value),
    }
}

pub fn template_to_text(ts: &mdl::instruction::template::template_string::TemplateString) -> String {
    use mdl::instruction::template::template_string::TemplateStringPart;
    let mut result = String::new();
    for part in &ts.parts {
        match part {
            TemplateStringPart::Literal(s) => result.push_str(s),
            TemplateStringPart::Expression(v) => {
                result.push('{');
                result.push_str(&value_to_markdown_text(v));
                result.push('}');
            }
        }
    }
    result
}
