pub mod block;
pub mod chain;
pub mod document;
pub mod instruction;
pub mod parser;

use crate::block::Block;

/// A parsed markdownlang program.
#[derive(Debug, Clone)]
pub struct Program {
    /// Top-level blocks (heading level 1).
    pub blocks: Vec<Block>,
    /// The source file ID (for error reporting with codespan-reporting).
    pub source_id: usize,
}
