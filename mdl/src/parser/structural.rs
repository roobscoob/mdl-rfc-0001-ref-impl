use std::ops::Range;

use pulldown_cmark::{Event, HeadingLevel, Options, Parser as CmarkParser, Tag, TagEnd};

use crate::block::Block;
use crate::chain::Chain;
use crate::chain::fence_group::FenceGroup;
use crate::document::{
    ColumnAlignment, Document, DocumentNode, InlineNode,
};
use crate::parser::error::ParseError;
use crate::parser::expression;

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse Markdown source text into a list of top-level blocks.
pub fn parse_blocks(
    source: &str,
    file_id: usize,
) -> Result<Vec<Block>, Vec<ParseError>> {
    let options = Options::ENABLE_STRIKETHROUGH | Options::ENABLE_TABLES;
    let parser = CmarkParser::new_ext(source, options);
    let events: Vec<(Event<'_>, Range<usize>)> = parser.into_offset_iter().collect();

    let mut state = ParseState::new(source, file_id);
    state.process_events(&events)?;
    state.finalize()
}

// ---------------------------------------------------------------------------
// Parse state
// ---------------------------------------------------------------------------

struct ParseState<'a> {
    source: &'a str,
    file_id: usize,
    /// Stack of blocks being built. Innermost = current scope.
    block_stack: Vec<BlockBuilder>,
    /// Completed top-level blocks.
    top_blocks: Vec<Block>,
    errors: Vec<ParseError>,
}

struct BlockBuilder {
    name: String,
    level: u8,
    chain_groups: Vec<FenceGroup>,
    children: Vec<Block>,
    body_nodes: Vec<DocumentNode>,
    span_start: usize,
}

impl BlockBuilder {
    fn into_block(self, span_end: usize) -> Block {
        Block {
            name: self.name,
            level: self.level,
            chain: if self.chain_groups.is_empty() {
                Chain::empty()
            } else {
                Chain {
                    groups: self.chain_groups,
                }
            },
            children: self.children,
            body: Document {
                nodes: self.body_nodes,
            },
            span: self.span_start..span_end,
        }
    }
}

impl<'a> ParseState<'a> {
    fn new(source: &'a str, file_id: usize) -> Self {
        ParseState {
            source,
            file_id,
            block_stack: Vec::new(),
            top_blocks: Vec::new(),
            errors: Vec::new(),
        }
    }

    fn process_events(
        &mut self,
        events: &[(Event<'_>, Range<usize>)],
    ) -> Result<(), Vec<ParseError>> {
        let mut i = 0;

        while i < events.len() {
            let (ref ev, ref range) = events[i];

            match ev {
                Event::Start(Tag::Heading { level, .. }) => {
                    let heading_level = heading_level_to_u8(level);

                    // Collect heading text
                    i += 1;
                    let name = collect_heading_text(events, &mut i);

                    // Normalize: strip leading/trailing whitespace, collapse interior whitespace
                    let name = normalize_block_name(&name);

                    // Close blocks that are at the same or deeper level
                    self.close_blocks_to_level(heading_level, range.start);

                    // Push new block
                    self.block_stack.push(BlockBuilder {
                        name,
                        level: heading_level,
                        chain_groups: Vec::new(),
                        children: Vec::new(),
                        body_nodes: Vec::new(),
                        span_start: range.start,
                    });
                }

                // Ordered list = instruction chain
                Event::Start(Tag::List(Some(_start_num))) => {
                    i += 1;
                    self.process_ordered_list(events, &mut i)?;
                }

                // Unordered list outside instruction context = body content
                Event::Start(Tag::List(None)) => {
                    i += 1;
                    let doc = self.collect_unordered_list_as_document(events, &mut i);
                    if let Some(builder) = self.block_stack.last_mut() {
                        builder.body_nodes.push(doc);
                    }
                }

                // Paragraph = body content
                Event::Start(Tag::Paragraph) => {
                    i += 1;
                    let inlines = self.collect_inlines(events, &mut i, &|e| {
                        matches!(e, TagEnd::Paragraph)
                    });
                    if let Some(builder) = self.block_stack.last_mut() {
                        builder.body_nodes.push(DocumentNode::Paragraph(inlines));
                    }
                }

                // Code block = body content
                Event::Start(Tag::CodeBlock(kind)) => {
                    let language = match kind {
                        pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                            let lang = lang.to_string();
                            if lang.is_empty() { None } else { Some(lang) }
                        }
                        pulldown_cmark::CodeBlockKind::Indented => None,
                    };
                    i += 1;
                    let content = collect_text_until(events, &mut i, |e| {
                        matches!(e, TagEnd::CodeBlock)
                    });
                    if let Some(builder) = self.block_stack.last_mut() {
                        builder.body_nodes.push(DocumentNode::CodeBlock { language, content });
                    }
                }

                // Table = body content
                Event::Start(Tag::Table(alignments)) => {
                    let aligns: Vec<ColumnAlignment> = alignments
                        .iter()
                        .map(|a| match a {
                            pulldown_cmark::Alignment::None => ColumnAlignment::None,
                            pulldown_cmark::Alignment::Left => ColumnAlignment::Left,
                            pulldown_cmark::Alignment::Center => ColumnAlignment::Center,
                            pulldown_cmark::Alignment::Right => ColumnAlignment::Right,
                        })
                        .collect();
                    i += 1;
                    let (headers, rows) = self.collect_table(events, &mut i);
                    if let Some(builder) = self.block_stack.last_mut() {
                        builder.body_nodes.push(DocumentNode::Table {
                            alignments: aligns,
                            headers,
                            rows,
                        });
                    }
                }

                // Blockquote = body content
                Event::Start(Tag::BlockQuote(_)) => {
                    i += 1;
                    let inner = self.collect_blockquote(events, &mut i);
                    if let Some(builder) = self.block_stack.last_mut() {
                        builder.body_nodes.push(DocumentNode::Blockquote(inner));
                    }
                }

                // Horizontal rule = body content
                Event::Rule => {
                    if let Some(builder) = self.block_stack.last_mut() {
                        builder.body_nodes.push(DocumentNode::HorizontalRule);
                    }
                    i += 1;
                }

                _ => {
                    i += 1;
                }
            }
        }

        Ok(())
    }

    /// Process an ordered list: extract fence indices and instructions.
    fn process_ordered_list(
        &mut self,
        events: &[(Event<'_>, Range<usize>)],
        i: &mut usize,
    ) -> Result<(), Vec<ParseError>> {
        let mut items: Vec<(u64, Vec<(Event<'_>, Range<usize>)>)> = Vec::new();

        while *i < events.len() {
            let (ref ev, ref range) = events[*i];
            match ev {
                Event::End(TagEnd::List(true)) => {
                    *i += 1;
                    break;
                }
                Event::Start(Tag::Item) => {
                    // Extract the actual list item number from source text
                    let fence_index = extract_item_number(self.source, range.start);
                    *i += 1;

                    // Collect all events for this item
                    let item_events = self.collect_item_events(events, i);
                    items.push((fence_index, item_events));
                }
                _ => {
                    *i += 1;
                }
            }
        }

        // Group items by fence index into FenceGroups
        let Some(builder) = self.block_stack.last_mut() else {
            return Ok(());
        };

        for (fence_index, item_events) in items {
            // Parse the item events into an Instruction
            let span = if let Some((_, r)) = item_events.first() {
                r.clone()
            } else {
                0..0
            };

            match expression::parse_instruction(&item_events, self.source, span.clone(), self.file_id) {
                Ok(instruction) => {
                    // Find or create the FenceGroup for this index
                    if let Some(group) = builder
                        .chain_groups
                        .last_mut()
                        .filter(|g| g.index == fence_index)
                    {
                        group.instructions.push(instruction);
                    } else {
                        builder.chain_groups.push(FenceGroup {
                            index: fence_index,
                            instructions: vec![instruction],
                        });
                    }
                }
                Err(err) => {
                    self.errors.push(err);
                }
            }
        }

        Ok(())
    }

    /// Collect all events for a single list item until End(Item).
    fn collect_item_events<'b>(
        &self,
        events: &'b [(Event<'b>, Range<usize>)],
        i: &mut usize,
    ) -> Vec<(Event<'b>, Range<usize>)> {
        let mut item_events = Vec::new();
        let mut depth = 1u32;

        while *i < events.len() {
            let (ref ev, ref range) = events[*i];
            match ev {
                Event::End(TagEnd::Item) if depth == 1 => {
                    *i += 1;
                    break;
                }
                Event::Start(Tag::Item) => {
                    depth += 1;
                    item_events.push((ev.clone(), range.clone()));
                    *i += 1;
                }
                Event::End(TagEnd::Item) => {
                    depth -= 1;
                    item_events.push((ev.clone(), range.clone()));
                    *i += 1;
                }
                _ => {
                    item_events.push((ev.clone(), range.clone()));
                    *i += 1;
                }
            }
        }

        item_events
    }

    /// Collect inline nodes until a matching End tag.
    fn collect_inlines(
        &self,
        events: &[(Event<'_>, Range<usize>)],
        i: &mut usize,
        is_end: &dyn Fn(&TagEnd) -> bool,
    ) -> Vec<InlineNode> {
        let mut inlines = Vec::new();

        while *i < events.len() {
            let (ref ev, ref _range) = events[*i];
            match ev {
                Event::End(tag_end) if is_end(tag_end) => {
                    *i += 1;
                    break;
                }
                Event::Text(s) => {
                    inlines.push(InlineNode::Text(s.to_string()));
                    *i += 1;
                }
                Event::Code(s) => {
                    inlines.push(InlineNode::CodeSpan(s.to_string()));
                    *i += 1;
                }
                Event::SoftBreak => {
                    inlines.push(InlineNode::SoftBreak);
                    *i += 1;
                }
                Event::HardBreak => {
                    inlines.push(InlineNode::HardBreak);
                    *i += 1;
                }
                Event::Start(Tag::Strong) => {
                    *i += 1;
                    let children = self.collect_inlines(events, i, &|e| matches!(e, TagEnd::Strong));
                    inlines.push(InlineNode::Strong(children));
                }
                Event::Start(Tag::Emphasis) => {
                    *i += 1;
                    let children = self.collect_inlines(events, i, &|e| matches!(e, TagEnd::Emphasis));
                    inlines.push(InlineNode::Emphasis(children));
                }
                Event::Start(Tag::Strikethrough) => {
                    *i += 1;
                    let children = self.collect_inlines(events, i, &|e| matches!(e, TagEnd::Strikethrough));
                    inlines.push(InlineNode::Strikethrough(children));
                }
                Event::Start(Tag::Link { dest_url, title, .. }) => {
                    let dest = dest_url.to_string();
                    let title = title.to_string();
                    *i += 1;
                    let content = self.collect_inlines(events, i, &|e| matches!(e, TagEnd::Link));
                    inlines.push(InlineNode::Link { dest, title, content });
                }
                Event::Start(Tag::Image { dest_url, title, .. }) => {
                    let dest = dest_url.to_string();
                    let title = title.to_string();
                    *i += 1;
                    let alt = self.collect_inlines(events, i, &|e| matches!(e, TagEnd::Image));
                    inlines.push(InlineNode::Image { dest, title, alt });
                }
                _ => {
                    *i += 1;
                }
            }
        }

        inlines
    }

    /// Collect table headers and rows.
    fn collect_table(
        &self,
        events: &[(Event<'_>, Range<usize>)],
        i: &mut usize,
    ) -> (Vec<Vec<InlineNode>>, Vec<Vec<Vec<InlineNode>>>) {
        let mut headers: Vec<Vec<InlineNode>> = Vec::new();
        let mut rows: Vec<Vec<Vec<InlineNode>>> = Vec::new();
        let mut in_head = false;
        let mut current_row: Vec<Vec<InlineNode>> = Vec::new();

        while *i < events.len() {
            let (ref ev, _) = events[*i];
            match ev {
                Event::End(TagEnd::Table) => {
                    *i += 1;
                    break;
                }
                Event::Start(Tag::TableHead) => {
                    in_head = true;
                    *i += 1;
                }
                Event::End(TagEnd::TableHead) => {
                    in_head = false;
                    headers = std::mem::take(&mut current_row);
                    *i += 1;
                }
                Event::Start(Tag::TableRow) => {
                    current_row = Vec::new();
                    *i += 1;
                }
                Event::End(TagEnd::TableRow) => {
                    if !in_head {
                        rows.push(std::mem::take(&mut current_row));
                    }
                    *i += 1;
                }
                Event::Start(Tag::TableCell) => {
                    *i += 1;
                    let cell = self.collect_inlines(events, i, &|e| matches!(e, TagEnd::TableCell));
                    current_row.push(cell);
                }
                _ => {
                    *i += 1;
                }
            }
        }

        (headers, rows)
    }

    /// Collect a blockquote's content as a Document.
    fn collect_blockquote(
        &self,
        events: &[(Event<'_>, Range<usize>)],
        i: &mut usize,
    ) -> Document {
        let mut nodes = Vec::new();

        while *i < events.len() {
            let (ref ev, _) = events[*i];
            match ev {
                Event::End(TagEnd::BlockQuote(_)) => {
                    *i += 1;
                    break;
                }
                Event::Start(Tag::Paragraph) => {
                    *i += 1;
                    let inlines = self.collect_inlines(events, i, &|e| matches!(e, TagEnd::Paragraph));
                    nodes.push(DocumentNode::Paragraph(inlines));
                }
                _ => {
                    *i += 1;
                }
            }
        }

        Document { nodes }
    }

    /// Collect an unordered list as a Document node (body content, not match arms).
    fn collect_unordered_list_as_document(
        &self,
        events: &[(Event<'_>, Range<usize>)],
        i: &mut usize,
    ) -> DocumentNode {
        let mut items = Vec::new();

        while *i < events.len() {
            let (ref ev, _) = events[*i];
            match ev {
                Event::End(TagEnd::List(false)) => {
                    *i += 1;
                    break;
                }
                Event::Start(Tag::Item) => {
                    *i += 1;
                    let mut item_nodes = Vec::new();
                    while *i < events.len() {
                        let (ref ev2, _) = events[*i];
                        match ev2 {
                            Event::End(TagEnd::Item) => {
                                *i += 1;
                                break;
                            }
                            Event::Start(Tag::Paragraph) => {
                                *i += 1;
                                let inlines = self.collect_inlines(events, i, &|e| {
                                    matches!(e, TagEnd::Paragraph)
                                });
                                item_nodes.push(DocumentNode::Paragraph(inlines));
                            }
                            Event::Text(s) => {
                                item_nodes.push(DocumentNode::Paragraph(vec![
                                    InlineNode::Text(s.to_string()),
                                ]));
                                *i += 1;
                            }
                            _ => {
                                *i += 1;
                            }
                        }
                    }
                    items.push(Document { nodes: item_nodes });
                }
                _ => {
                    *i += 1;
                }
            }
        }

        DocumentNode::UnorderedList { items }
    }

    /// Close blocks from the stack down to the given heading level.
    fn close_blocks_to_level(&mut self, new_level: u8, span_end: usize) {
        // Pop blocks that are at the same or deeper level than the new heading
        while let Some(top) = self.block_stack.last() {
            if top.level >= new_level {
                let builder = self.block_stack.pop().unwrap();
                let block = builder.into_block(span_end);

                if let Some(parent) = self.block_stack.last_mut() {
                    // This block becomes a child of the parent
                    parent.children.push(block);
                } else {
                    // No parent — this is a top-level block
                    self.top_blocks.push(block);
                }
            } else {
                break;
            }
        }
    }

    fn finalize(mut self) -> Result<Vec<Block>, Vec<ParseError>> {
        let end = self.source.len();

        // Close all remaining blocks
        while let Some(builder) = self.block_stack.pop() {
            let block = builder.into_block(end);
            if let Some(parent) = self.block_stack.last_mut() {
                parent.children.push(block);
            } else {
                self.top_blocks.push(block);
            }
        }

        if self.errors.is_empty() {
            Ok(self.top_blocks)
        } else {
            Err(self.errors)
        }
    }
}

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn heading_level_to_u8(level: &HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

/// Collect heading text (all Text events until End(Heading)).
fn collect_heading_text(events: &[(Event<'_>, Range<usize>)], i: &mut usize) -> String {
    let mut name = String::new();
    while *i < events.len() {
        let (ref ev, _) = events[*i];
        match ev {
            Event::End(TagEnd::Heading(_)) => {
                *i += 1;
                break;
            }
            Event::Text(s) => {
                name.push_str(s);
                *i += 1;
            }
            Event::Code(s) => {
                name.push_str(s);
                *i += 1;
            }
            _ => {
                *i += 1;
            }
        }
    }
    name
}

/// Normalize block name: strip leading/trailing whitespace, collapse interior whitespace.
fn normalize_block_name(name: &str) -> String {
    name.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Collect all text content until a matching End tag.
fn collect_text_until(
    events: &[(Event<'_>, Range<usize>)],
    i: &mut usize,
    is_end: impl Fn(&TagEnd) -> bool,
) -> String {
    let mut text = String::new();
    while *i < events.len() {
        let (ref ev, _) = events[*i];
        match ev {
            Event::End(tag_end) if is_end(tag_end) => {
                *i += 1;
                break;
            }
            Event::Text(s) => {
                text.push_str(s);
                *i += 1;
            }
            _ => {
                *i += 1;
            }
        }
    }
    text
}

/// Extract the actual list item number from source text.
/// pulldown-cmark normalizes item numbers, so we look at the raw source.
///
/// pulldown-cmark's Item event range may start at either:
///   (a) the content after the marker (e.g. position of `x` in `1. x = 1`), or
///   (b) the list marker itself (e.g. position of `1` in `1. x = 1`).
/// We try a backward scan first (handles case a), then a forward scan (handles case b).
fn extract_item_number(source: &str, item_start: usize) -> u64 {
    // Backward scan: look at text between line start and item_start.
    // Works when item_start points to content (after the marker).
    let before = &source[..item_start];
    let line_start = before.rfind('\n').map(|p| p + 1).unwrap_or(0);
    let prefix = &source[line_start..item_start];
    if let Some(n) = parse_list_marker(prefix) {
        return n;
    }

    // Forward scan: item_start is at the list marker itself.
    // Read from item_start to end of line and parse the number there.
    let rest = &source[item_start..];
    let line_end = rest.find('\n').unwrap_or(rest.len());
    let line = &rest[..line_end];
    if let Some(n) = parse_list_marker(line) {
        return n;
    }

    // Fallback
    1
}

/// Try to parse a list item number from text like "1. " or "  2) ".
fn parse_list_marker(text: &str) -> Option<u64> {
    let trimmed = text.trim();
    for sep in ['.', ')'] {
        if let Some(pos) = trimmed.find(sep) {
            if let Ok(n) = trimmed[..pos].parse::<u64>() {
                return Some(n);
            }
        }
    }
    None
}
