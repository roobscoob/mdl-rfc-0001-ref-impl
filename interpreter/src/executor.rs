use std::collections::HashMap;
use std::io::Write;
use std::path::PathBuf;

use mdl::block::Block;
use mdl::block::reference::BlockReference;
use mdl::chain::Chain;
use mdl::instruction::Instruction;

use crate::environment::{Environment, Scope};
use crate::error::{DiagnosticError, RuntimeError};
use crate::evaluator::evaluate;
use crate::runtime_value::RuntimeValue;

/// Registry of all blocks in the program, indexed by name.
/// Supports loading blocks from imported files.
pub struct BlockRegistry {
    blocks: HashMap<String, Block>,
    /// Maps each block name to its lexical parent's name (None for top-level blocks).
    parent_map: HashMap<String, Option<String>>,
    /// Base directory for resolving relative imports.
    base_dir: PathBuf,
    /// Cache of imported file blocks, keyed by canonical path.
    import_cache: HashMap<PathBuf, HashMap<String, Block>>,
    /// Source file ID for codespan-reporting diagnostics.
    pub source_id: usize,
}

impl BlockRegistry {
    pub fn from_program(program: &mdl::Program) -> Self {
        Self::from_program_with_base(program, PathBuf::from("."))
    }

    pub fn from_program_with_base(program: &mdl::Program, base_dir: PathBuf) -> Self {
        let mut blocks = HashMap::new();
        let mut parent_map = HashMap::new();
        for block in &program.blocks {
            register_block(&mut blocks, &mut parent_map, block, None);
        }
        BlockRegistry {
            blocks,
            parent_map,
            base_dir,
            import_cache: HashMap::new(),
            source_id: program.source_id,
        }
    }

    pub fn get(&self, name: &str) -> Option<&Block> {
        self.blocks.get(name)
    }

    /// Case-insensitive block lookup. Tries exact match first, then case-insensitive.
    pub fn get_entry(&self, name: &str) -> Option<&Block> {
        self.blocks.get(name).or_else(|| {
            let lower = name.to_lowercase();
            self.blocks
                .iter()
                .find(|(k, _)| k.to_lowercase() == lower)
                .map(|(_, v)| v)
        })
    }

    /// Return all top-level block names (for --list-blocks).
    pub fn block_names(&self) -> Vec<&str> {
        self.blocks.keys().map(|s| s.as_str()).collect()
    }

    /// Load and cache blocks from a local import file.
    fn load_import(&mut self, path: &str) -> Result<(), RuntimeError> {
        let resolved = self.base_dir.join(path);
        // Try with .md extension if no extension present
        let resolved = if resolved.extension().is_none() {
            resolved.with_extension("md")
        } else {
            resolved
        };

        let canonical = resolved
            .canonicalize()
            .map_err(|e| RuntimeError::IoError(format!("cannot resolve '{}': {}", path, e)))?;

        if self.import_cache.contains_key(&canonical) {
            return Ok(());
        }

        let source = std::fs::read_to_string(&canonical)
            .map_err(|e| RuntimeError::IoError(format!("cannot read '{}': {}", path, e)))?;

        let parser = mdl::parser::Parser::new(source, 0);
        let program = parser.parse().map_err(|errs| {
            let messages: Vec<String> = errs.iter().map(|e| e.message.clone()).collect();
            RuntimeError::Custom(format!(
                "parse errors in '{}': {}",
                path,
                messages.join(", ")
            ))
        })?;

        let mut import_blocks = HashMap::new();
        for block in &program.blocks {
            register_block(&mut import_blocks, &mut self.parent_map, block, None);
        }

        self.import_cache.insert(canonical, import_blocks);
        Ok(())
    }

    /// Get a block from an imported file.
    fn get_imported(&mut self, path: &str, block_name: &str) -> Result<Block, RuntimeError> {
        self.load_import(path)?;

        let resolved = self.base_dir.join(path);
        let resolved = if resolved.extension().is_none() {
            resolved.with_extension("md")
        } else {
            resolved
        };
        let canonical = resolved
            .canonicalize()
            .map_err(|e| RuntimeError::IoError(format!("cannot resolve '{}': {}", path, e)))?;

        self.import_cache
            .get(&canonical)
            .and_then(|blocks| blocks.get(block_name))
            .cloned()
            .ok_or(RuntimeError::UndefinedBlock(format!(
                "{}#{}",
                path, block_name
            )))
    }

    /// Compute the list of lexical ancestor block names for a given block.
    pub fn lexical_ancestors(&self, block_name: &str) -> Vec<String> {
        let mut ancestors = Vec::new();
        let mut current = block_name.to_string();
        while let Some(Some(parent)) = self.parent_map.get(&current) {
            ancestors.push(parent.clone());
            current = parent.clone();
        }
        ancestors
    }
}

fn register_block(
    registry: &mut HashMap<String, Block>,
    parent_map: &mut HashMap<String, Option<String>>,
    block: &Block,
    parent_name: Option<&str>,
) {
    registry.insert(block.name.clone(), block.clone());
    parent_map.insert(block.name.clone(), parent_name.map(|s| s.to_string()));
    for child in &block.children {
        register_block(registry, parent_map, child, Some(&block.name));
    }
}

/// Execute a program: find the first top-level block and run it.
pub fn execute_program(
    program: &mdl::Program,
    output: &mut dyn Write,
) -> Result<(RuntimeValue, Vec<DiagnosticError>), DiagnosticError> {
    execute_program_with_base(program, output, PathBuf::from("."))
}

/// Execute a program with a specified base directory for imports.
pub fn execute_program_with_base(
    program: &mdl::Program,
    output: &mut dyn Write,
    base_dir: PathBuf,
) -> Result<(RuntimeValue, Vec<DiagnosticError>), DiagnosticError> {
    let mut registry = BlockRegistry::from_program_with_base(program, base_dir);
    let mut env = Environment::new();
    let mut diagnostics = Vec::new();

    let entry = program
        .blocks
        .first()
        .ok_or(DiagnosticError::from(RuntimeError::NoEntryPoint))?
        .clone();

    let result = execute_block(
        &entry,
        vec![],
        &mut env,
        &mut registry,
        output,
        0,
        &mut diagnostics,
    )?;
    Ok((result, diagnostics))
}

/// Execute a program with a named entrypoint block and arguments.
/// Block name matching is case-insensitive.
pub fn execute_program_entry(
    program: &mdl::Program,
    output: &mut dyn Write,
    base_dir: PathBuf,
    entry_name: &str,
    arguments: Vec<RuntimeValue>,
) -> Result<(RuntimeValue, Vec<DiagnosticError>), DiagnosticError> {
    let mut registry = BlockRegistry::from_program_with_base(program, base_dir.clone());
    let mut env = Environment::new();
    let mut diagnostics = Vec::new();

    if program.blocks.is_empty() {
        return Err(DiagnosticError::from(RuntimeError::NoEntryPoint));
    }

    let entry = registry
        .get_entry(entry_name)
        .ok_or_else(|| {
            let available: Vec<&str> = registry.block_names();
            DiagnosticError::from(RuntimeError::UndefinedBlock(format!(
                "'{}' (available blocks: {})",
                entry_name,
                if available.is_empty() {
                    "none".to_string()
                } else {
                    available.join(", ")
                }
            )))
        })?
        .clone();

    let result = execute_block(
        &entry,
        arguments,
        &mut env,
        &mut registry,
        output,
        0,
        &mut diagnostics,
    )?;
    Ok((result, diagnostics))
}

/// Execute a block with given arguments.
pub fn execute_block(
    block: &Block,
    arguments: Vec<RuntimeValue>,
    env: &mut Environment,
    registry: &mut BlockRegistry,
    output: &mut dyn Write,
    depth: usize,
    diagnostics: &mut Vec<DiagnosticError>,
) -> Result<RuntimeValue, DiagnosticError> {
    // If block has no chain (no ordered list), return its body as a Document.
    if block.chain.is_empty() {
        let doc = block.body.clone();
        // Single-element Document auto-unwrap
        return Ok(auto_unwrap_document(doc));
    }

    // Hoist variables: scan all instructions for assignment targets
    let hoisted = collect_hoisted_variables(&block.chain);
    let ancestors = registry.lexical_ancestors(&block.name);
    let scope = Scope::new(arguments, hoisted, block.name.clone(), ancestors);
    env.push_scope(scope);

    let source_id = registry.source_id;
    let mut last_value = RuntimeValue::Unit;

    // Execute fence groups in order
    for group in &block.chain.groups {
        env.push_fence_context();

        // Within a fence group, execute sequentially (valid under undefined order)
        for (instr_idx, instruction) in group.instructions.iter().enumerate() {
            env.set_fence_instruction(instr_idx);
            last_value = execute_instruction(
                instruction,
                env,
                registry,
                output,
                depth,
                diagnostics,
                source_id,
            )?;
        }

        // Check for same-fence UB: variable read and written by different instructions
        let violations = env.pop_fence_context();
        for (var_name, read_spans) in violations {
            for span in read_spans {
                diagnostics.push(DiagnosticError::warning(
                    format!(
                        "reading variable '{}' at the same fence as its assignment (undefined behavior)",
                        var_name
                    ),
                    span,
                    source_id,
                ));
            }
        }
    }

    env.pop_scope();
    Ok(last_value)
}

fn execute_instruction(
    instruction: &Instruction,
    env: &mut Environment,
    registry: &mut BlockRegistry,
    output: &mut dyn Write,
    depth: usize,
    diagnostics: &mut Vec<DiagnosticError>,
    source_id: usize,
) -> Result<RuntimeValue, DiagnosticError> {
    let span = instruction.span().clone();

    let result = match instruction {
        Instruction::Assignment {
            variable, value, ..
        } => {
            let val = evaluate(
                value,
                env,
                registry,
                output,
                depth,
                diagnostics,
                source_id,
                &span,
            )?;
            env.set_variable(variable, val.clone());
            env.record_fence_write(variable);
            Ok(val)
        }
        Instruction::Expression { value, .. } => evaluate(
            value,
            env,
            registry,
            output,
            depth,
            diagnostics,
            source_id,
            &span,
        ),
    };

    // Attach instruction span to errors that don't already have one
    result.map_err(|mut e| {
        if e.span.is_none() {
            e.span = Some(span.clone());
            e.source_id = source_id;
        }
        e
    })
}

/// Invoke a block by reference.
pub fn invoke_block(
    block_ref: &BlockReference,
    arguments: Vec<RuntimeValue>,
    env: &mut Environment,
    registry: &mut BlockRegistry,
    output: &mut dyn Write,
    evaluate_result: bool,
    depth: usize,
    diagnostics: &mut Vec<DiagnosticError>,
) -> Result<RuntimeValue, DiagnosticError> {
    let block_name = block_ref.block_name();

    if depth > crate::evaluator::MAX_DEPTH {
        return Err(RuntimeError::StackOverflow.into());
    }

    match block_ref {
        BlockReference::Local(_) => {
            let block = registry
                .get(block_name)
                .ok_or(RuntimeError::UndefinedBlock(block_name.to_string()))?
                .clone();

            let result =
                execute_block(&block, arguments, env, registry, output, depth + 1, diagnostics)?;

            if evaluate_result {
                // ![args](#block): evaluate the Document result
                match result {
                    RuntimeValue::Document(doc) => {
                        evaluate_document(&doc, env, registry, output, depth + 1, diagnostics)
                    }
                    other => Ok(other),
                }
            } else {
                Ok(result)
            }
        }
        BlockReference::LocalImport { path, .. } => {
            let block = registry.get_imported(path, block_name)?;
            let result =
                execute_block(&block, arguments, env, registry, output, depth + 1, diagnostics)?;

            if evaluate_result {
                match result {
                    RuntimeValue::Document(doc) => {
                        evaluate_document(&doc, env, registry, output, depth + 1, diagnostics)
                    }
                    other => Ok(other),
                }
            } else {
                Ok(result)
            }
        }
        BlockReference::RemoteImport { url, .. } => {
            Err(RuntimeError::ImportNotImplemented(url.clone()).into())
        }
    }
}

/// Evaluate a Document by interpreting its Markdown content as expressions.
pub(crate) fn evaluate_document(
    doc: &mdl::document::Document,
    env: &mut Environment,
    registry: &mut BlockRegistry,
    output: &mut dyn Write,
    depth: usize,
    diagnostics: &mut Vec<DiagnosticError>,
) -> Result<RuntimeValue, DiagnosticError> {
    use mdl::document::DocumentNode;

    let mut last = RuntimeValue::Unit;

    for node in &doc.nodes {
        match node {
            DocumentNode::Paragraph(inlines) => {
                for inline in inlines {
                    last = evaluate_inline(inline, env, registry, output, depth, diagnostics)?;
                }
            }
            _ => {
                // Other block-level nodes: return as document
                last = RuntimeValue::Document(mdl::document::Document {
                    nodes: vec![node.clone()],
                });
            }
        }
    }

    Ok(last)
}

/// Evaluate an inline node from a Document.
fn evaluate_inline(
    inline: &mdl::document::InlineNode,
    env: &mut Environment,
    registry: &mut BlockRegistry,
    output: &mut dyn Write,
    depth: usize,
    diagnostics: &mut Vec<DiagnosticError>,
) -> Result<RuntimeValue, DiagnosticError> {
    use mdl::document::InlineNode;

    match inline {
        InlineNode::Text(s) => Ok(RuntimeValue::String(s.clone())),
        InlineNode::Strong(children) => {
            // Bold = print. Parse {expr} templates in text children.
            let mut text = String::new();
            for child in children {
                match child {
                    InlineNode::Text(s) if s.contains('{') => {
                        // Parse as template and evaluate expressions
                        let source_id = registry.source_id;
                        let span = 0..0;
                        match mdl::parser::expression::parse_text_template(s, source_id) {
                            Ok(ts) => {
                                let val = crate::evaluator::eval_template_string(
                                    &ts, env, registry, output, depth, diagnostics, source_id, &span,
                                )?;
                                text.push_str(&val);
                            }
                            Err(_) => text.push_str(s),
                        }
                    }
                    _ => {
                        let val = evaluate_inline(child, env, registry, output, depth, diagnostics)?;
                        text.push_str(&val.to_string());
                    }
                }
            }
            writeln!(output, "{}", text)
                .map_err(|e| DiagnosticError::from(RuntimeError::IoError(e.to_string())))?;
            Ok(RuntimeValue::Unit)
        }
        InlineNode::Strikethrough(children) => {
            // Strikethrough = null; eagerly evaluate children to get the inner value
            let inner_doc = mdl::document::Document {
                nodes: vec![mdl::document::DocumentNode::Paragraph(children.clone())],
            };
            let inner = evaluate_document(&inner_doc, env, registry, output, depth, diagnostics)?;
            Ok(RuntimeValue::Strikethrough(
                crate::runtime_value::StrikethroughPayload::Eager(Box::new(inner)),
            ))
        }
        InlineNode::Link { dest, .. } => {
            // Link = block invocation
            let block_ref = crate::evaluator_helpers::parse_runtime_block_ref(dest);
            let args = Vec::new(); // TODO: parse link text as arguments
            invoke_block(
                &block_ref,
                args,
                env,
                registry,
                output,
                false,
                depth,
                diagnostics,
            )
        }
        InlineNode::Image { dest, .. } => {
            // Image = evaluated block invocation
            let block_ref = crate::evaluator_helpers::parse_runtime_block_ref(dest);
            let args = Vec::new();
            invoke_block(
                &block_ref,
                args,
                env,
                registry,
                output,
                true,
                depth,
                diagnostics,
            )
        }
        _ => Ok(RuntimeValue::Unit),
    }
}

/// Collect all variable names assigned within a chain (for hoisting).
fn collect_hoisted_variables(chain: &Chain) -> Vec<String> {
    let mut vars = Vec::new();
    for group in &chain.groups {
        for instruction in &group.instructions {
            if let Instruction::Assignment { variable, .. } = instruction {
                if !vars.contains(variable) {
                    vars.push(variable.clone());
                }
            }
        }
    }
    vars
}

/// Auto-unwrap a Document with a single element.
fn auto_unwrap_document(doc: mdl::document::Document) -> RuntimeValue {
    if doc.nodes.len() == 1 {
        match &doc.nodes[0] {
            mdl::document::DocumentNode::Paragraph(inlines) if inlines.len() == 1 => {
                match &inlines[0] {
                    mdl::document::InlineNode::Text(s) => RuntimeValue::String(s.clone()),
                    _ => RuntimeValue::Document(doc),
                }
            }
            mdl::document::DocumentNode::Table { headers, rows, .. } => {
                let header_strings: Vec<String> = headers
                    .iter()
                    .map(|h| {
                        h.iter()
                            .map(|n| format!("{}", n))
                            .collect::<Vec<_>>()
                            .join("")
                    })
                    .collect();

                let runtime_rows: Vec<Vec<RuntimeValue>> = rows
                    .iter()
                    .map(|row| {
                        row.iter()
                            .map(|cell| {
                                let text = cell
                                    .iter()
                                    .map(|n| format!("{}", n))
                                    .collect::<Vec<_>>()
                                    .join("");
                                // Try to parse as number
                                if let Ok(n) = text.trim().parse::<f64>() {
                                    RuntimeValue::Number(n)
                                } else {
                                    RuntimeValue::String(text)
                                }
                            })
                            .collect()
                    })
                    .collect();

                RuntimeValue::Table {
                    headers: header_strings,
                    rows: runtime_rows,
                }
            }
            _ => RuntimeValue::Document(doc),
        }
    } else {
        RuntimeValue::Document(doc)
    }
}
