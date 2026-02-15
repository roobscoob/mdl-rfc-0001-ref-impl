use std::ops::Range;

use codespan_reporting::diagnostic::{Diagnostic, Label, Severity};

/// Parse errors with source location information.
#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub span: Range<usize>,
    pub file_id: usize,
    pub severity: Severity,
    pub notes: Vec<String>,
}

impl ParseError {
    pub fn error(message: impl Into<String>, span: Range<usize>, file_id: usize) -> Self {
        ParseError {
            message: message.into(),
            span,
            file_id,
            severity: Severity::Error,
            notes: Vec::new(),
        }
    }

    pub fn warning(message: impl Into<String>, span: Range<usize>, file_id: usize) -> Self {
        ParseError {
            message: message.into(),
            span,
            file_id,
            severity: Severity::Warning,
            notes: Vec::new(),
        }
    }

    pub fn with_note(mut self, note: impl Into<String>) -> Self {
        self.notes.push(note.into());
        self
    }

    /// Convert to a codespan-reporting Diagnostic for display.
    pub fn to_diagnostic(&self) -> Diagnostic<usize> {
        Diagnostic::new(self.severity)
            .with_message(&self.message)
            .with_labels(vec![Label::primary(self.file_id, self.span.clone())])
            .with_notes(self.notes.clone())
    }
}
