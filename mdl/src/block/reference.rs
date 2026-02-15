/// A reference to a block, used in invocations like [args](#block).
#[derive(Debug, Clone, PartialEq)]
pub enum BlockReference {
    /// Reference to a block by name within the current document: #block-name
    Local(String),
    /// Reference to a block in a local file: ./path#block
    LocalImport { path: String, block: String },
    /// Reference to a block at a remote URL: https://url#block
    RemoteImport { url: String, block: String },
}

impl BlockReference {
    pub fn block_name(&self) -> &str {
        match self {
            BlockReference::Local(name) => name,
            BlockReference::LocalImport { block, .. } => block,
            BlockReference::RemoteImport { block, .. } => block,
        }
    }
}
