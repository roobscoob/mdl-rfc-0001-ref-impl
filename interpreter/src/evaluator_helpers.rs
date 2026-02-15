use mdl::block::reference::BlockReference;

/// Parse a destination string into a BlockReference at runtime.
pub fn parse_runtime_block_ref(dest: &str) -> BlockReference {
    if dest.starts_with('#') {
        BlockReference::Local(dest[1..].to_string())
    } else if let Some((path, block)) = dest.rsplit_once('#') {
        if path.starts_with("http://") || path.starts_with("https://") {
            BlockReference::RemoteImport {
                url: path.to_string(),
                block: block.to_string(),
            }
        } else {
            BlockReference::LocalImport {
                path: path.to_string(),
                block: block.to_string(),
            }
        }
    } else {
        BlockReference::Local(dest.to_string())
    }
}
