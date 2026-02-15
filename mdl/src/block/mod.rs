pub mod reference;

use std::ops::Range;

use crate::chain::Chain;
use crate::document::Document;

/// A named block defined by a Markdown heading.
/// Blocks are the fundamental unit of execution in markdownlang.
#[derive(Debug, Clone)]
pub struct Block {
    /// The block name (from heading text), case-sensitive, whitespace-normalized.
    pub name: String,
    /// Heading level: 1 = top-level (#), 2-6 = sub-blocks (##-######).
    pub level: u8,
    /// The execution chain (ordered instructions grouped by fence index).
    pub chain: Chain,
    /// Sub-blocks defined lexically within this block's scope.
    pub children: Vec<Block>,
    /// Non-instruction content (Markdown body).
    /// Returned as a Document when the block is invoked without a chain.
    pub body: Document,
    /// Byte span in source for error reporting.
    pub span: Range<usize>,
}
