use std::collections::HashMap;
use std::ops::Range;

use crate::runtime_value::RuntimeValue;

/// Tracks variable reads and writes within a single fence group for UB detection.
#[derive(Debug, Default)]
struct FenceContext {
    /// Variables read during this fence group: name → [(instruction_index, span)].
    reads: HashMap<String, Vec<(usize, Range<usize>)>>,
    /// Variables written during this fence group: name → [instruction_index].
    writes: HashMap<String, Vec<usize>>,
    /// Index of the currently executing instruction within this fence group.
    current_instruction: usize,
}

/// Result of looking up a variable in the environment.
pub enum VariableLookup<'a> {
    /// Variable found with a value.
    Found {
        value: &'a RuntimeValue,
        /// True if found in a parent scope rather than the current one.
        cross_scope: bool,
        /// True if found in a scope that isn't a lexical ancestor of the
        /// current block — reading such a variable is undefined behavior.
        non_lexical_scope: bool,
    },
    /// Variable is hoisted (declared) but has not been assigned yet — UB.
    HoistedUnassigned,
    /// Variable does not exist in any scope.
    NotFound,
}

/// A single scope level, corresponding to a block invocation.
#[derive(Debug, Clone)]
pub struct Scope {
    /// Variables in this scope. Hoisted: keys exist from block entry,
    /// values start as None (reading before assignment = UB).
    variables: HashMap<String, Option<RuntimeValue>>,
    /// Positional arguments (#0, #1, ...).
    arguments: Vec<RuntimeValue>,
    /// Name of the block this scope belongs to.
    block_name: String,
    /// Names of all lexical ancestor blocks (parent, grandparent, ...).
    lexical_ancestors: Vec<String>,
}

impl Scope {
    pub fn new(
        arguments: Vec<RuntimeValue>,
        hoisted_vars: Vec<String>,
        block_name: String,
        lexical_ancestors: Vec<String>,
    ) -> Self {
        let mut variables = HashMap::new();
        for var in hoisted_vars {
            variables.insert(var, None);
        }
        Scope {
            variables,
            arguments,
            block_name,
            lexical_ancestors,
        }
    }

    pub fn get_variable(&self, name: &str) -> Option<&Option<RuntimeValue>> {
        self.variables.get(name)
    }

    pub fn set_variable(&mut self, name: &str, value: RuntimeValue) {
        self.variables.insert(name.to_string(), Some(value));
    }

    pub fn get_argument(&self, index: usize) -> Option<&RuntimeValue> {
        self.arguments.get(index)
    }

    pub fn get_all_arguments(&self) -> &[RuntimeValue] {
        &self.arguments
    }
}

/// The full environment is a stack of scopes.
/// Sub-blocks inherit parent scope (lexical scoping).
#[derive(Debug)]
pub struct Environment {
    scopes: Vec<Scope>,
    /// Stack of fence contexts for same-fence UB detection.
    fence_stack: Vec<FenceContext>,
}

impl Environment {
    pub fn new() -> Self {
        Environment {
            scopes: Vec::new(),
            fence_stack: Vec::new(),
        }
    }

    /// Begin tracking accesses for a new fence group.
    pub fn push_fence_context(&mut self) {
        self.fence_stack.push(FenceContext::default());
    }

    /// Set the index of the currently executing instruction within the fence group.
    pub fn set_fence_instruction(&mut self, idx: usize) {
        if let Some(ctx) = self.fence_stack.last_mut() {
            ctx.current_instruction = idx;
        }
    }

    /// Record a variable read in the current fence context.
    pub fn record_fence_read(&mut self, name: &str, span: Range<usize>) {
        if let Some(ctx) = self.fence_stack.last_mut() {
            let idx = ctx.current_instruction;
            ctx.reads
                .entry(name.to_string())
                .or_default()
                .push((idx, span));
        }
    }

    /// Record a variable write in the current fence context.
    pub fn record_fence_write(&mut self, name: &str) {
        if let Some(ctx) = self.fence_stack.last_mut() {
            let idx = ctx.current_instruction;
            ctx.writes.entry(name.to_string()).or_default().push(idx);
        }
    }

    /// End the current fence context and return UB violations:
    /// variables that were read by one instruction and written by a
    /// different instruction within the same fence group.
    /// Returns (variable_name, read_spans) pairs.
    pub fn pop_fence_context(&mut self) -> Vec<(String, Vec<Range<usize>>)> {
        let ctx = self.fence_stack.pop().expect("no fence context to pop");
        let mut violations = Vec::new();

        for (name, read_entries) in &ctx.reads {
            if let Some(write_indices) = ctx.writes.get(name) {
                // Flag reads from instructions different than any write instruction
                let ub_spans: Vec<Range<usize>> = read_entries
                    .iter()
                    .filter(|(read_idx, _)| {
                        write_indices.iter().any(|write_idx| read_idx != write_idx)
                    })
                    .map(|(_, span)| span.clone())
                    .collect();

                if !ub_spans.is_empty() {
                    violations.push((name.clone(), ub_spans));
                }
            }
        }

        violations
    }

    pub fn push_scope(&mut self, scope: Scope) {
        self.scopes.push(scope);
    }

    pub fn pop_scope(&mut self) -> Option<Scope> {
        self.scopes.pop()
    }

    pub fn current_scope_mut(&mut self) -> &mut Scope {
        self.scopes.last_mut().expect("no scope on stack")
    }

    /// Look up a variable, searching from innermost scope outward.
    pub fn get_variable(&self, name: &str) -> Option<&RuntimeValue> {
        for scope in self.scopes.iter().rev() {
            if let Some(val) = scope.get_variable(name) {
                return val.as_ref();
            }
        }
        None
    }

    /// Look up a variable with scope information.
    pub fn get_variable_info(&self, name: &str) -> VariableLookup<'_> {
        let current = self.scopes.last().expect("no scope on stack");
        for (depth, scope) in self.scopes.iter().rev().enumerate() {
            if let Some(val) = scope.get_variable(name) {
                let is_cross_scope = depth > 0;
                let is_non_lexical = is_cross_scope
                    && scope.block_name != current.block_name
                    && !current.lexical_ancestors.contains(&scope.block_name);
                return match val {
                    Some(v) => VariableLookup::Found {
                        value: v,
                        cross_scope: is_cross_scope,
                        non_lexical_scope: is_non_lexical,
                    },
                    None => VariableLookup::HoistedUnassigned,
                };
            }
        }
        VariableLookup::NotFound
    }

    /// Check if a variable name exists in any scope (hoisted).
    pub fn has_variable(&self, name: &str) -> bool {
        for scope in self.scopes.iter().rev() {
            if scope.get_variable(name).is_some() {
                return true;
            }
        }
        false
    }

    /// Set a variable in the current (innermost) scope.
    pub fn set_variable(&mut self, name: &str, value: RuntimeValue) {
        // First check if the variable is hoisted in the current scope
        let scope = self.scopes.last_mut().expect("no scope on stack");
        scope.set_variable(name, value);
    }

    /// Get a positional argument from the current scope.
    pub fn get_argument(&self, index: usize) -> Option<&RuntimeValue> {
        self.scopes.last()?.get_argument(index)
    }

    /// Get all arguments (for #*) from the current scope.
    pub fn get_all_arguments(&self) -> &[RuntimeValue] {
        self.scopes
            .last()
            .map(|s| s.get_all_arguments())
            .unwrap_or(&[])
    }
}
