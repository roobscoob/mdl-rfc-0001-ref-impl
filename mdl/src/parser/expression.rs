use std::ops::Range;

use pulldown_cmark::{CowStr, Event, Tag, TagEnd};

use crate::block::reference::BlockReference;
use crate::instruction::Instruction;
use crate::instruction::template::template_string::{TemplateString, TemplateStringPart};
use crate::instruction::value::{BinaryOperator, UnaryOperator, Value};
use crate::parser::error::ParseError;

// ---------------------------------------------------------------------------
// Token types
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
enum Token {
    // Literals
    Number(f64),
    StringLit(String),
    True,
    False,
    Unit,

    // Identifiers & references
    Ident(String, Range<usize>),
    ArgRef(usize, Range<usize>),   // #0, #1, ...
    SpreadRef,                     // #*
    Hash(usize),                   // bare # (carries byte offset for merge)

    // Keywords
    Match,

    // Operators
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,         // =
    EqEq,       // ==
    BangEq,     // !=
    Gt,
    Lt,
    GtEq,
    LtEq,
    Amp,        // &
    AmpAmp,     // &&
    Pipe,       // |
    PipePipe,   // ||
    Bang,       // !
    Question,   // ?
    Colon,      // :
    Comma,
    Underscore, // _

    // Grouping
    LParen,
    RParen,
    LBrace,    // {
    RBrace,    // }

    // Markdown-derived compound tokens
    Bold(TemplateString),
    Strike(TemplateString),
    Link { text_tokens: Vec<Token>, dest: String },
    Image { text_tokens: Vec<Token>, dest: String },

    // Nested unordered list (for match arms), stored as raw events
    MatchArms(Vec<MatchArm>),
}

#[derive(Debug, Clone)]
struct MatchArm {
    /// Tokens for the pattern portion (before the colon).
    pattern: (Vec<Token>, Range<usize>),
    /// Tokens for the result portion (after the colon).
    result: (Vec<Token>, Range<usize>),
    /// Whether this arm's pattern text starts with "otherwise".
    is_otherwise: bool,
}

#[derive(Debug, Clone)]
#[allow(dead_code)]
enum OwnedEvent {
    Text(String),
    Code(String),
    StartStrong,
    EndStrong,
    StartStrikethrough,
    EndStrikethrough,
    StartLink { dest: String },
    EndLink,
    StartImage { dest: String },
    EndImage,
    SoftBreak,
}

// ---------------------------------------------------------------------------
// Public API
// ---------------------------------------------------------------------------

/// Parse the pulldown-cmark events for a single ordered list item into an Instruction.
pub fn parse_instruction(
    events: &[(Event<'_>, Range<usize>)],
    _source: &str,
    span: Range<usize>,
    file_id: usize,
) -> Result<Instruction, ParseError> {
    let tokens = tokenize_events(events, file_id, span.clone())?;
    let mut parser = ExprParser::new(tokens, span.clone(), file_id);

    // Check for assignment: ident = expr
    if parser.is_assignment() {
        let name = parser.expect_ident()?;
        parser.expect_token_kind(TokenKind::Eq)?;
        let value = parser.parse_expr(0)?;
        if !parser.at_end() {
            return Err(parser.error("unexpected tokens after assignment"));
        }
        Ok(Instruction::Assignment {
            variable: name,
            value,
            span,
        })
    } else {
        let value = parser.parse_expr(0)?;
        if !parser.at_end() {
            return Err(parser.error("unexpected tokens after expression"));
        }
        Ok(Instruction::Expression { value, span })
    }
}

// ---------------------------------------------------------------------------
// Tokenizer: pulldown-cmark events → Token stream
// ---------------------------------------------------------------------------

fn tokenize_events(
    events: &[(Event<'_>, Range<usize>)],
    file_id: usize,
    span: Range<usize>,
) -> Result<Vec<Token>, ParseError> {
    let mut tokens = Vec::new();
    let mut i = 0;

    while i < events.len() {
        let (ref ev, ref range) = events[i];
        match ev {
            // Skip paragraph wrappers (tight vs loose lists)
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => {
                i += 1;
            }

            Event::Text(s) => {
                tokenize_text(s, &mut tokens, range.start);
                i += 1;
            }

            Event::Code(s) => {
                tokens.push(Token::StringLit(s.to_string()));
                i += 1;
            }

            Event::SoftBreak | Event::HardBreak => {
                i += 1;
            }

            Event::Start(Tag::Strong) => {
                i += 1;
                let ts = collect_template_string(events, &mut i, &|e| matches!(e, TagEnd::Strong), file_id, span.clone())?;
                tokens.push(Token::Bold(ts));
            }

            Event::Start(Tag::Strikethrough) => {
                i += 1;
                let ts = collect_template_string(events, &mut i, &|e| matches!(e, TagEnd::Strikethrough), file_id, span.clone())?;
                tokens.push(Token::Strike(ts));
            }

            Event::Start(Tag::Link { dest_url, .. }) => {
                let dest = dest_url.to_string();
                i += 1;
                let inner = collect_until_end(events, &mut i, |e| matches!(e, TagEnd::Link), file_id, span.clone())?;
                tokens.push(Token::Link {
                    text_tokens: inner,
                    dest,
                });
            }

            Event::Start(Tag::Image { dest_url, .. }) => {
                let dest = dest_url.to_string();
                i += 1;
                let inner = collect_until_end(events, &mut i, |e| matches!(e, TagEnd::Image), file_id, span.clone())?;
                tokens.push(Token::Image {
                    text_tokens: inner,
                    dest,
                });
            }

            // Unordered list inside an ordered list item = match arms
            Event::Start(Tag::List(None)) => {
                i += 1;
                let arms = collect_match_arms(events, &mut i, file_id, span.clone())?;
                tokens.push(Token::MatchArms(arms));
            }

            Event::Start(Tag::Emphasis) => {
                // Emphasis has no executable semantics; treat inner as plain tokens
                i += 1;
                let inner = collect_until_end(events, &mut i, |e| matches!(e, TagEnd::Emphasis), file_id, span.clone())?;
                tokens.extend(inner);
            }

            // Skip other events we don't handle in expression context
            _ => {
                i += 1;
            }
        }
    }

    // Post-process: merge Gt+Eq → GtEq and Lt+Eq → LtEq
    // (pulldown-cmark may split text around < and > producing separate tokens)
    merge_compound_operators(&mut tokens);

    Ok(tokens)
}

/// Merge adjacent compound tokens that may have been split across text events.
/// Handles: Gt+Eq → GtEq, Lt+Eq → LtEq, Hash+Star → SpreadRef, Hash+Number → ArgRef.
fn merge_compound_operators(tokens: &mut Vec<Token>) {
    // Merge adjacent tokens
    let mut i = 0;
    while i + 1 < tokens.len() {
        let merge = match (&tokens[i], &tokens[i + 1]) {
            (Token::Gt, Token::Eq) => Some(Token::GtEq),
            (Token::Lt, Token::Eq) => Some(Token::LtEq),
            (Token::Hash(_), Token::Star) => Some(Token::SpreadRef),
            (Token::Hash(offset), Token::Number(n)) => {
                // Approximate span: from # to end of number (exact end unknown, use offset+2 as estimate)
                let span = *offset..*offset + 2;
                Some(Token::ArgRef(*n as usize, span))
            }
            _ => None,
        };
        if let Some(merged) = merge {
            tokens[i] = merged;
            tokens.remove(i + 1);
        } else {
            i += 1;
        }
    }
}

/// Collect inner tokens until we hit a matching End tag.
/// Advances `i` past the End tag.
fn collect_until_end(
    events: &[(Event<'_>, Range<usize>)],
    i: &mut usize,
    is_end: impl Fn(&TagEnd) -> bool,
    file_id: usize,
    span: Range<usize>,
) -> Result<Vec<Token>, ParseError> {
    let mut inner_events = Vec::new();
    let mut depth = 1u32;

    while *i < events.len() {
        let (ref ev, ref range) = events[*i];
        match ev {
            Event::End(tag_end) if depth == 1 && is_end(tag_end) => {
                *i += 1;
                break;
            }
            Event::Start(_) => {
                depth += 1;
                inner_events.push((ev.clone(), range.clone()));
                *i += 1;
            }
            Event::End(_) => {
                depth -= 1;
                inner_events.push((ev.clone(), range.clone()));
                *i += 1;
            }
            _ => {
                inner_events.push((ev.clone(), range.clone()));
                *i += 1;
            }
        }
    }

    tokenize_events(&inner_events, file_id, span)
}

/// Collect the contents of a Bold/Strike tag and build a TemplateString directly.
/// Text is preserved literally (whitespace included) with `{expr}` interpolations.
/// Links and images become expression parts (block invocations).
fn collect_template_string(
    events: &[(Event<'_>, Range<usize>)],
    i: &mut usize,
    is_end: &dyn Fn(&TagEnd) -> bool,
    file_id: usize,
    span: Range<usize>,
) -> Result<TemplateString, ParseError> {
    let mut parts: Vec<TemplateStringPart> = Vec::new();

    while *i < events.len() {
        let (ref ev, ref _range) = events[*i];
        match ev {
            Event::End(tag_end) if is_end(tag_end) => {
                *i += 1;
                break;
            }
            Event::Start(Tag::Paragraph) | Event::End(TagEnd::Paragraph) => {
                *i += 1;
            }
            Event::Text(s) => {
                // Parse text as template: literal text with {expr} interpolations
                let text_parts = parse_template_parts(s, file_id, span.clone())?;
                parts.extend(text_parts);
                *i += 1;
            }
            Event::Code(s) => {
                parts.push(TemplateStringPart::Literal(s.to_string()));
                *i += 1;
            }
            Event::SoftBreak => {
                parts.push(TemplateStringPart::Literal(" ".to_string()));
                *i += 1;
            }
            Event::HardBreak => {
                parts.push(TemplateStringPart::Literal("\n".to_string()));
                *i += 1;
            }
            Event::Start(Tag::Link { dest_url, .. }) => {
                let dest = dest_url.to_string();
                *i += 1;
                let inner = collect_until_end(events, i, |e| matches!(e, TagEnd::Link), file_id, span.clone())?;
                let block_ref = parse_block_reference(&dest);
                let args = parse_argument_list(inner, file_id, span.clone())?;
                parts.push(TemplateStringPart::Expression(Value::BlockInvocation(args, block_ref)));
            }
            Event::Start(Tag::Image { dest_url, .. }) => {
                let dest = dest_url.to_string();
                *i += 1;
                let inner = collect_until_end(events, i, |e| matches!(e, TagEnd::Image), file_id, span.clone())?;
                let block_ref = parse_block_reference(&dest);
                let args = parse_argument_list(inner, file_id, span.clone())?;
                parts.push(TemplateStringPart::Expression(Value::EvaluatedBlockInvocation(args, block_ref)));
            }
            Event::Start(Tag::Emphasis) => {
                // Emphasis inside bold has no special meaning; pass through inner content
                *i += 1;
                let inner_ts = collect_template_string(events, i, &|e| matches!(e, TagEnd::Emphasis), file_id, span.clone())?;
                parts.extend(inner_ts.parts);
            }
            _ => {
                *i += 1;
            }
        }
    }

    if parts.is_empty() {
        parts.push(TemplateStringPart::Literal(String::new()));
    }

    Ok(TemplateString { parts })
}

/// Collect match arms from an unordered list.
/// Each list item is: `pattern: result` or `otherwise [binding]: result`.
fn collect_match_arms(
    events: &[(Event<'_>, Range<usize>)],
    i: &mut usize,
    file_id: usize,
    _span: Range<usize>,
) -> Result<Vec<MatchArm>, ParseError> {
    let mut arms = Vec::new();

    while *i < events.len() {
        let (ref ev, ref span) = events[*i];
        match ev {
            Event::End(TagEnd::List(false)) => {
                *i += 1;
                break;
            }
            Event::Start(Tag::Item) => {
                *i += 1;
                let arm = collect_single_match_arm(events, i, file_id, span.clone())?;
                arms.push(arm);
            }
            _ => {
                *i += 1;
            }
        }
    }

    Ok(arms)
}

fn collect_single_match_arm(
    events: &[(Event<'_>, Range<usize>)],
    i: &mut usize,
    file_id: usize,
    span: Range<usize>,
) -> Result<MatchArm, ParseError> {
    let mut pattern_span = 0..0;
    let mut result_span = 0..0;
    let mut pattern_events = Vec::new();
    let mut result_events = Vec::new();
    let mut current_span = &mut pattern_span;
    let mut current_events = &mut pattern_events;
    let mut writing_to_pattern = false;

    // Collect all events of this arm until End(Item)
    while *i < events.len() {
        let (ref ev, ref span) = events[*i];
        if current_span.start == 0 {
            *current_span = span.clone();
        }
        match ev {
            Event::End(TagEnd::Item) => {
                *i += 1;
                current_span.end = span.end;
                break;
            }
            Event::Text(text) if !writing_to_pattern && text.contains(":") => {
                *i += 1;
                let (before, after) = text.split_once(":").unwrap();
                current_events.push((Event::Text(before.into()), current_span.end..span.start));
                current_span.end = span.start;
                current_events = &mut result_events;
                current_span = &mut result_span;
                writing_to_pattern = true;
                *current_span = span.clone();
                current_events.push((Event::Text(after.into()), span.clone()));
            }
            _ => {
                *i += 1;
                current_span.end = span.end;
                current_events.push((ev.clone(), span.clone()));
            }
        }
    }

    let mut pattern = tokenize_events(&pattern_events, file_id, pattern_span.clone())?;
    let is_otherwise = if let Some(Token::Ident(otherwise, _)) = pattern.get(0)
        && otherwise == "otherwise" && pattern.len() >= 2 { true } else { false };
    let (result, is_otherwise) = if is_otherwise && result_events.len() == 0 {
        result_span = pattern_span.clone();
        if let Some(Token::Colon) = pattern.get(2) {
            pattern.remove(2);
            (pattern.split_off(2), true)
        } else {
            (pattern.split_off(2), true)
        }
    } else {
        (tokenize_events(&result_events, file_id, result_span.clone())?, is_otherwise)
    };

    Ok(MatchArm {
        pattern: (pattern, pattern_span),
        result: (result, result_span),
        is_otherwise,
    })
}

// ---------------------------------------------------------------------------
// Text tokenizer: raw text string → Token stream
// ---------------------------------------------------------------------------

fn tokenize_text(text: &str, tokens: &mut Vec<Token>, base_offset: usize) {
    let chars: Vec<char> = text.chars().collect();
    let len = chars.len();
    let mut i = 0;

    // Map character indices to byte offsets within the text
    let byte_pos: Vec<usize> = {
        let mut bp = Vec::with_capacity(len + 1);
        let mut offset = 0;
        for c in &chars {
            bp.push(offset);
            offset += c.len_utf8();
        }
        bp.push(offset);
        bp
    };

    while i < len {
        let c = chars[i];
        match c {
            ' ' | '\t' | '\n' | '\r' => {
                i += 1;
            }

            // String literal
            '"' => {
                i += 1;
                let start = i;
                while i < len && chars[i] != '"' {
                    i += 1;
                }
                let s: String = chars[start..i].iter().collect();
                if i < len {
                    i += 1; // skip closing quote
                }
                tokens.push(Token::StringLit(s));
            }

            // Numbers
            '0'..='9' => {
                let start = i;
                while i < len && (chars[i].is_ascii_digit() || chars[i] == '.') {
                    i += 1;
                }
                let num_str: String = chars[start..i].iter().collect();
                if let Ok(n) = num_str.parse::<f64>() {
                    tokens.push(Token::Number(n));
                }
            }

            // Identifiers and keywords
            'a'..='z' | 'A'..='Z' | '_' => {
                let start = i;
                while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
                    i += 1;
                }
                let ident: String = chars[start..i].iter().collect();
                let span = base_offset + byte_pos[start]..base_offset + byte_pos[i];
                match ident.as_str() {
                    "true" => tokens.push(Token::True),
                    "false" => tokens.push(Token::False),
                    "match" => tokens.push(Token::Match),
                    _ => tokens.push(Token::Ident(ident, span)),
                }
            }

            // Argument references: #0, #1, #*
            '#' => {
                let hash_start = i;
                i += 1;
                if i < len && chars[i] == '*' {
                    i += 1;
                    tokens.push(Token::SpreadRef);
                } else if i < len && chars[i].is_ascii_digit() {
                    let start = i;
                    while i < len && chars[i].is_ascii_digit() {
                        i += 1;
                    }
                    let num_str: String = chars[start..i].iter().collect();
                    let span = base_offset + byte_pos[hash_start]..base_offset + byte_pos[i];
                    if let Ok(n) = num_str.parse::<usize>() {
                        tokens.push(Token::ArgRef(n, span));
                    }
                } else {
                    // Bare # at end of text or before unknown char — emit Hash for merging
                    tokens.push(Token::Hash(base_offset + byte_pos[hash_start]));
                }
            }

            // Two-character operators
            '=' => {
                i += 1;
                if i < len && chars[i] == '=' {
                    i += 1;
                    tokens.push(Token::EqEq);
                } else {
                    tokens.push(Token::Eq);
                }
            }
            '!' => {
                i += 1;
                if i < len && chars[i] == '=' {
                    i += 1;
                    tokens.push(Token::BangEq);
                } else {
                    tokens.push(Token::Bang);
                }
            }
            '>' => {
                i += 1;
                if i < len && chars[i] == '=' {
                    i += 1;
                    tokens.push(Token::GtEq);
                } else {
                    tokens.push(Token::Gt);
                }
            }
            '<' => {
                i += 1;
                if i < len && chars[i] == '=' {
                    i += 1;
                    tokens.push(Token::LtEq);
                } else {
                    tokens.push(Token::Lt);
                }
            }
            '&' => {
                i += 1;
                if i < len && chars[i] == '&' {
                    i += 1;
                    tokens.push(Token::AmpAmp);
                } else {
                    tokens.push(Token::Amp);
                }
            }
            '|' => {
                i += 1;
                if i < len && chars[i] == '|' {
                    i += 1;
                    tokens.push(Token::PipePipe);
                } else {
                    tokens.push(Token::Pipe);
                }
            }

            // Single-character operators
            '+' => { i += 1; tokens.push(Token::Plus); }
            '-' => { i += 1; tokens.push(Token::Minus); }
            '*' => { i += 1; tokens.push(Token::Star); }
            '/' => { i += 1; tokens.push(Token::Slash); }
            '%' => { i += 1; tokens.push(Token::Percent); }
            '?' => { i += 1; tokens.push(Token::Question); }
            ':' => { i += 1; tokens.push(Token::Colon); }
            ',' => { i += 1; tokens.push(Token::Comma); }
            '(' => {
                i += 1;
                // Check for unit literal ()
                if i < len && chars[i] == ')' {
                    i += 1;
                    tokens.push(Token::Unit);
                } else {
                    tokens.push(Token::LParen);
                }
            }
            ')' => { i += 1; tokens.push(Token::RParen); }
            '{' => { i += 1; tokens.push(Token::LBrace); }
            '}' => { i += 1; tokens.push(Token::RBrace); }

            _ => {
                i += 1; // skip unknown chars
            }
        }
    }
}

// ---------------------------------------------------------------------------
// Token kind (for matching without payloads)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq)]
enum TokenKind {
    Number,
    StringLit,
    True,
    False,
    Unit,
    Ident,
    ArgRef,
    SpreadRef,
    Hash,
    Match,
    Plus,
    Minus,
    Star,
    Slash,
    Percent,
    Eq,
    EqEq,
    BangEq,
    Gt,
    Lt,
    GtEq,
    LtEq,
    Amp,
    AmpAmp,
    Pipe,
    PipePipe,
    Bang,
    Question,
    Colon,
    Comma,
    Underscore,
    LParen,
    RParen,
    LBrace,
    RBrace,
    Bold,
    Strike,
    Link,
    Image,
    MatchArms,
}

fn token_kind(t: &Token) -> TokenKind {
    match t {
        Token::Number(_) => TokenKind::Number,
        Token::StringLit(_) => TokenKind::StringLit,
        Token::True => TokenKind::True,
        Token::False => TokenKind::False,
        Token::Unit => TokenKind::Unit,
        Token::Ident(..) => TokenKind::Ident,
        Token::ArgRef(..) => TokenKind::ArgRef,
        Token::SpreadRef => TokenKind::SpreadRef,
        Token::Hash(_) => TokenKind::Hash,
        Token::Match => TokenKind::Match,
        Token::Plus => TokenKind::Plus,
        Token::Minus => TokenKind::Minus,
        Token::Star => TokenKind::Star,
        Token::Slash => TokenKind::Slash,
        Token::Percent => TokenKind::Percent,
        Token::Eq => TokenKind::Eq,
        Token::EqEq => TokenKind::EqEq,
        Token::BangEq => TokenKind::BangEq,
        Token::Gt => TokenKind::Gt,
        Token::Lt => TokenKind::Lt,
        Token::GtEq => TokenKind::GtEq,
        Token::LtEq => TokenKind::LtEq,
        Token::Amp => TokenKind::Amp,
        Token::AmpAmp => TokenKind::AmpAmp,
        Token::Pipe => TokenKind::Pipe,
        Token::PipePipe => TokenKind::PipePipe,
        Token::Bang => TokenKind::Bang,
        Token::Question => TokenKind::Question,
        Token::Colon => TokenKind::Colon,
        Token::Comma => TokenKind::Comma,
        Token::Underscore => TokenKind::Underscore,
        Token::LParen => TokenKind::LParen,
        Token::RParen => TokenKind::RParen,
        Token::LBrace => TokenKind::LBrace,
        Token::RBrace => TokenKind::RBrace,
        Token::Bold(_) => TokenKind::Bold,
        Token::Strike(_) => TokenKind::Strike,
        Token::Link { .. } => TokenKind::Link,
        Token::Image { .. } => TokenKind::Image,
        Token::MatchArms(_) => TokenKind::MatchArms,
    }
}

// ---------------------------------------------------------------------------
// Pratt parser
// ---------------------------------------------------------------------------

struct ExprParser {
    tokens: Vec<Token>,
    pos: usize,
    span: Range<usize>,
    file_id: usize,
}

// Binding powers (precedence). Higher = tighter binding.
// Left bp, right bp. For left-assoc: right = left + 1. For right-assoc: right = left.
const BP_CONDITIONAL: u8 = 2;   // ? :
const BP_OR: u8 = 4;            // ||
const BP_AND: u8 = 6;           // &&
const BP_EQUALITY: u8 = 8;      // == !=
const BP_COMPARISON: u8 = 10;   // < > <= >=
const BP_ADDITIVE: u8 = 12;     // + -
const BP_MULTIPLICATIVE: u8 = 14; // * / %
const BP_UNARY: u8 = 16;        // ! -

impl ExprParser {
    fn new(tokens: Vec<Token>, span: Range<usize>, file_id: usize) -> Self {
        ExprParser { tokens, pos: 0, span, file_id }
    }

    fn peek(&self) -> Option<&Token> {
        self.tokens.get(self.pos)
    }

    fn peek_kind(&self) -> Option<TokenKind> {
        self.peek().map(token_kind)
    }

    fn advance(&mut self) -> Option<Token> {
        if self.pos < self.tokens.len() {
            let t = self.tokens[self.pos].clone();
            self.pos += 1;
            Some(t)
        } else {
            None
        }
    }

    fn at_end(&self) -> bool {
        self.pos >= self.tokens.len()
    }

    fn error(&self, msg: impl Into<String>) -> ParseError {
        ParseError::error(msg, self.span.clone(), self.file_id)
    }

    fn expect_ident(&mut self) -> Result<String, ParseError> {
        match self.advance() {
            Some(Token::Ident(name, _)) => Ok(name),
            _ => Err(self.error("expected identifier")),
        }
    }

    fn expect_token_kind(&mut self, kind: TokenKind) -> Result<Token, ParseError> {
        match self.advance() {
            Some(t) if token_kind(&t) == kind => Ok(t),
            _ => Err(self.error(format!("expected {:?}", kind))),
        }
    }

    /// Check if the token stream is an assignment: ident = expr
    /// (ident followed by single `=`, not `==`)
    fn is_assignment(&self) -> bool {
        if self.tokens.len() < 3 {
            return false;
        }
        matches!(
            (&self.tokens[0], &self.tokens[1]),
            (Token::Ident(..), Token::Eq)
        )
    }

    // ------------------------------------------------------------------
    // Pratt parser core
    // ------------------------------------------------------------------

    fn parse_expr(&mut self, min_bp: u8) -> Result<Value, ParseError> {
        let mut left = self.parse_prefix()?;

        loop {
            if self.at_end() {
                break;
            }

            // Check for infix operators
            let Some(kind) = self.peek_kind() else { break };
            let Some((l_bp, r_bp)) = infix_bp(kind) else { break };

            if l_bp < min_bp {
                break;
            }

            // Special case: conditional operator (?)
            if kind == TokenKind::Question {
                self.advance();
                let true_branch = self.parse_expr(0)?;
                let false_branch = if self.peek_kind() == Some(TokenKind::Colon) {
                    self.advance();
                    Some(Box::new(self.parse_expr(0)?))
                } else {
                    None
                };
                left = Value::Conditional {
                    condition: Box::new(left),
                    true_branch: Box::new(true_branch),
                    false_branch,
                };
                continue;
            }

            let op = self.advance().unwrap();
            let right = self.parse_expr(r_bp)?;

            let operator = match token_kind(&op) {
                TokenKind::Plus => BinaryOperator::Addition,
                TokenKind::Minus => BinaryOperator::Subtraction,
                TokenKind::Star => BinaryOperator::Multiplication,
                TokenKind::Slash => BinaryOperator::Division,
                TokenKind::Percent => BinaryOperator::Modulo,
                TokenKind::EqEq => BinaryOperator::Equality,
                TokenKind::BangEq => BinaryOperator::Inequality,
                TokenKind::Gt => BinaryOperator::GreaterThan,
                TokenKind::Lt => BinaryOperator::LessThan,
                TokenKind::GtEq => BinaryOperator::GreaterThanOrEqual,
                TokenKind::LtEq => BinaryOperator::LessThanOrEqual,
                TokenKind::AmpAmp => BinaryOperator::LogicalAnd,
                TokenKind::PipePipe => BinaryOperator::LogicalOr,
                _ => return Err(self.error("unexpected infix operator")),
            };

            left = Value::BinaryOperation {
                operator,
                left: Box::new(left),
                right: Box::new(right),
            };
        }

        Ok(left)
    }

    fn parse_prefix(&mut self) -> Result<Value, ParseError> {
        let token = self.advance().ok_or_else(|| self.error("unexpected end of expression"))?;

        match token {
            // Literals
            Token::Number(n) => Ok(Value::NumberLiteral(n)),
            Token::StringLit(s) => self.parse_string_interpolation(s),
            Token::True => Ok(Value::BooleanLiteral(true)),
            Token::False => Ok(Value::BooleanLiteral(false)),
            Token::Unit => Ok(Value::UnitLiteral),

            // References
            Token::Ident(name, span) => Ok(Value::VariableReference(name, span)),
            Token::ArgRef(n, span) => Ok(Value::PositionalArgumentReference(n, span)),
            Token::SpreadRef => Ok(Value::SpreadArgumentReference),

            // Unary operators
            Token::Bang => {
                let operand = self.parse_expr(BP_UNARY)?;
                Ok(Value::UnaryOperation {
                    operator: UnaryOperator::LogicalNot,
                    operand: Box::new(operand),
                })
            }
            Token::Minus => {
                let operand = self.parse_expr(BP_UNARY)?;
                Ok(Value::UnaryOperation {
                    operator: UnaryOperator::Negation,
                    operand: Box::new(operand),
                })
            }

            // Parenthesized expression
            Token::LParen => {
                let expr = self.parse_expr(0)?;
                self.expect_token_kind(TokenKind::RParen)?;
                Ok(expr)
            }

            // Bold = Print
            Token::Bold(ts) => Ok(Value::Print(ts)),

            // Strikethrough = null / quotation
            Token::Strike(ts) => Ok(Value::Strikethrough(ts)),

            // Link = block invocation [args](#block)
            Token::Link { text_tokens, dest } => {
                let block_ref = parse_block_reference(&dest);
                let args = parse_argument_list(text_tokens, self.file_id, self.span.clone())?;
                Ok(Value::BlockInvocation(args, block_ref))
            }

            // Image = evaluated block invocation ![args](#block)
            Token::Image { text_tokens, dest } => {
                let block_ref = parse_block_reference(&dest);
                let args = parse_argument_list(text_tokens, self.file_id, self.span.clone())?;
                Ok(Value::EvaluatedBlockInvocation(args, block_ref))
            }

            // Match expression
            Token::Match => {
                let scrutinee = self.parse_expr(BP_UNARY)?;
                // The match arms should follow as a MatchArms token
                match self.advance() {
                    Some(Token::MatchArms(arms)) => {
                        self.build_match_expr(scrutinee, arms)
                    }
                    _ => Err(self.error("expected match arms (unordered list) after 'match'")),
                }
            }

            // Interpolation: {expr}
            Token::LBrace => {
                let expr = self.parse_expr(0)?;
                self.expect_token_kind(TokenKind::RBrace)?;
                Ok(expr)
            }

            _ => Err(self.error(format!("unexpected token: {:?}", token_kind(&token)))),
        }
    }

    /// Parse a string that may contain {expr} interpolations.
    fn parse_string_interpolation(&mut self, s: String) -> Result<Value, ParseError> {
        let parts = parse_template_parts(&s, self.file_id, self.span.clone())?;
        if parts.len() == 1 {
            if let TemplateStringPart::Literal(_) = &parts[0] {
                return Ok(Value::StringLiteral(s));
            }
        }
        if parts.iter().all(|p| matches!(p, TemplateStringPart::Literal(_))) {
            return Ok(Value::StringLiteral(s));
        }
        Ok(Value::Interpolation(TemplateString { parts }))
    }

    /// Build a match expression from parsed arms.
    fn build_match_expr(
        &self,
        scrutinee: Value,
        arms: Vec<MatchArm>,
    ) -> Result<Value, ParseError> {
        use crate::instruction::template::Template;

        let mut parsed_arms: Vec<(Template, Value)> = Vec::new();
        let mut otherwise: Option<(Option<String>, Box<Value>)> = None;

        for arm in arms {
            if arm.is_otherwise {
                let binding = match arm.pattern.0.get(0) {
                    Some(Token::Ident(ident, _)) => Some(ident.clone()),
                    Some(Token::Underscore) | None => None,
                    Some(_) => return Err(ParseError::error("expected binding", arm.pattern.1, self.file_id)),
                };
                let result_value = self.parse_arm_result(arm.result.0, arm.result.1)?;
                otherwise = Some((binding, Box::new(result_value)));
            } else {
                let template = parse_pattern(&arm.pattern.0, arm.pattern.1, self.file_id)?;
                let result_value = self.parse_arm_result(arm.result.0, arm.result.1)?;
                parsed_arms.push((template, result_value));
            }
        }

        Ok(Value::Match {
            value: Box::new(scrutinee),
            arms: parsed_arms,
            otherwise,
        })
    }

    fn parse_arm_result(&self, tokens: Vec<Token>, span: Range<usize>) -> Result<Value, ParseError> {
        let mut parser = ExprParser::new(tokens, span, self.file_id);
        parser.parse_expr(0)
    }
}

/// Infix binding powers: returns (left_bp, right_bp) or None if not infix.
fn infix_bp(kind: TokenKind) -> Option<(u8, u8)> {
    match kind {
        TokenKind::Question => Some((BP_CONDITIONAL, BP_CONDITIONAL)),
        TokenKind::PipePipe => Some((BP_OR, BP_OR + 1)),
        TokenKind::AmpAmp => Some((BP_AND, BP_AND + 1)),
        TokenKind::EqEq | TokenKind::BangEq => Some((BP_EQUALITY, BP_EQUALITY + 1)),
        TokenKind::Gt | TokenKind::Lt | TokenKind::GtEq | TokenKind::LtEq => {
            Some((BP_COMPARISON, BP_COMPARISON + 1))
        }
        TokenKind::Plus | TokenKind::Minus => Some((BP_ADDITIVE, BP_ADDITIVE + 1)),
        TokenKind::Star | TokenKind::Slash | TokenKind::Percent => {
            Some((BP_MULTIPLICATIVE, BP_MULTIPLICATIVE + 1))
        }
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Template string parsing
// ---------------------------------------------------------------------------

/// Parse a string's content for {expr} interpolations.
fn parse_template_parts(
    s: &str,
    file_id: usize,
    span: Range<usize>,
) -> Result<Vec<TemplateStringPart>, ParseError> {
    let mut parts = Vec::new();
    let mut current_literal = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;

    while i < chars.len() {
        if chars[i] == '{' {
            // Flush current literal
            if !current_literal.is_empty() {
                parts.push(TemplateStringPart::Literal(std::mem::take(&mut current_literal)));
            }
            // Find matching }
            i += 1;
            let mut depth = 1u32;
            let start = i;
            while i < chars.len() {
                if chars[i] == '{' {
                    depth += 1;
                } else if chars[i] == '}' {
                    depth -= 1;
                    if depth == 0 {
                        break;
                    }
                }
                i += 1;
            }
            let expr_str: String = chars[start..i].iter().collect();
            if i < chars.len() {
                i += 1; // skip closing }
            }
            // Parse the expression
            let mut tokens = Vec::new();
            tokenize_text(&expr_str, &mut tokens, 0);
            let mut parser = ExprParser::new(tokens, span.clone(), file_id);
            let expr = parser.parse_expr(0)?;
            parts.push(TemplateStringPart::Expression(expr));
        } else {
            current_literal.push(chars[i]);
            i += 1;
        }
    }

    if !current_literal.is_empty() {
        parts.push(TemplateStringPart::Literal(current_literal));
    }

    if parts.is_empty() {
        parts.push(TemplateStringPart::Literal(String::new()));
    }

    Ok(parts)
}

// ---------------------------------------------------------------------------
// Block reference parsing
// ---------------------------------------------------------------------------

fn parse_block_reference(dest: &str) -> BlockReference {
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

/// Parse comma-separated arguments from link text tokens.
fn parse_argument_list(
    tokens: Vec<Token>,
    file_id: usize,
    span: Range<usize>,
) -> Result<Vec<Value>, ParseError> {
    if tokens.is_empty() {
        return Ok(Vec::new());
    }

    // Split tokens on commas and parse each segment
    let mut args = Vec::new();
    let mut current = Vec::new();

    for token in tokens {
        if matches!(token, Token::Comma) {
            if !current.is_empty() {
                let mut parser = ExprParser::new(
                    std::mem::take(&mut current),
                    span.clone(),
                    file_id,
                );
                args.push(parser.parse_expr(0)?);
            }
        } else {
            current.push(token);
        }
    }

    if !current.is_empty() {
        let mut parser = ExprParser::new(current, span.clone(), file_id);
        args.push(parser.parse_expr(0)?);
    }

    Ok(args)
}

// ---------------------------------------------------------------------------
// Pattern parsing (for match arms)
// ---------------------------------------------------------------------------

fn parse_pattern(
    tokens: &[Token],
    span: Range<usize>,
    file_id: usize,
) -> Result<crate::instruction::template::Template, ParseError> {
    use crate::instruction::template::Template;
    
    let split = tokens.split(|x| matches!(x, Token::Pipe));
    let mut templates: Vec<Template> = Vec::new();

    for ele in split {
        templates.push(parse_single_pattern(ele, span.clone(), file_id)?);
    }

    if templates.len() == 1 {
        return Ok(templates.remove(0))
    }

    return Ok(Template::Alternation(templates))
}

fn parse_single_pattern(
    tokens: &[Token],
    span: Range<usize>,
    file_id: usize,
) -> Result<crate::instruction::template::Template, ParseError> {
    use crate::instruction::template::Template;

    match tokens {
        [Token::Number(value)] => Ok(Template::NumberLiteral(*value)),
        [Token::True] => Ok(Template::BooleanLiteral(true)),
        [Token::False] => Ok(Template::BooleanLiteral(false)),
        [Token::Unit] => Ok(Template::UnitLiteral),
        [Token::StringLit(string)] => Ok(Template::StringLiteral(string.clone())),
        [Token::Underscore] => Ok(Template::Wildcard),
        [Token::Ident(ident, _span)] => Ok(Template::Binding(ident.clone())),
        _ => Err(ParseError::error(
            "expected pattern", span, file_id
        ))
    }
}
