pub mod error;
pub mod expression;
mod structural;

pub use error::ParseError;

use crate::Program;

/// Parser entry point.
pub struct Parser {
    source: String,
    file_id: usize,
}

impl Parser {
    pub fn new(source: String, file_id: usize) -> Self {
        Parser { source, file_id }
    }

    /// Parse the source Markdown into a complete Program.
    pub fn parse(&self) -> Result<Program, Vec<ParseError>> {
        let blocks = structural::parse_blocks(&self.source, self.file_id)?;
        Ok(Program {
            blocks,
            source_id: self.file_id,
        })
    }
}
