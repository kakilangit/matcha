//! Breaks a pattern into readable parts, using the `regex-syntax` parser.

use std::ops::Range;

use regex_syntax::ast::{
    self, AssertionKind, Ast, ClassAsciiKind, ClassPerlKind, ClassSet, ClassSetItem,
    ClassUnicodeKind, Flag, FlagsItemKind, GroupKind, LiteralKind, RepetitionKind, RepetitionRange,
    SpecialLiteralKind,
};

use crate::engine::Flags;

/// One line of the explanation: a slice of the pattern and what it does.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Part {
    /// Nesting level, for indentation.
    pub depth: usize,
    /// Byte range of this part in the pattern.
    pub span: Range<usize>,
    pub description: String,
}

#[derive(Debug, PartialEq, Eq)]
pub enum ExplainError {
    /// Uses look-around or backreferences, which only `fancy-regex` supports.
    FancyOnly,
    Invalid,
}

pub fn explain(pattern: &str, flags: Flags) -> Result<Vec<Part>, ExplainError> {
    let ast = ast::parse::ParserBuilder::new()
        .ignore_whitespace(flags.ignore_whitespace)
        .build()
        .parse(pattern)
        .map_err(|err| match err.kind() {
            ast::ErrorKind::UnsupportedLookAround | ast::ErrorKind::UnsupportedBackreference => {
                ExplainError::FancyOnly
            }
            _ => ExplainError::Invalid,
        })?;
    let mut explainer = Explainer {
        pattern,
        flags,
        parts: Vec::new(),
    };
    explainer.visit(&ast, 0);
    Ok(explainer.parts)
}

struct Explainer<'a> {
    pattern: &'a str,
    flags: Flags,
    parts: Vec<Part>,
}

fn span(span: &ast::Span) -> Range<usize> {
    span.start.offset..span.end.offset
}

impl Explainer<'_> {
    fn push(&mut self, depth: usize, span: Range<usize>, description: impl Into<String>) {
        self.parts.push(Part {
            depth,
            span,
            description: description.into(),
        });
    }

    fn visit(&mut self, ast: &Ast, depth: usize) {
        match ast {
            Ast::Empty(_) => {}
            Ast::Concat(concat) => self.visit_sequence(&concat.asts, depth),
            Ast::Alternation(alternation) => {
                let count = alternation.asts.len();
                self.push(
                    depth,
                    span(&alternation.span),
                    format!("one of {count} alternatives"),
                );
                for (ix, alternative) in alternation.asts.iter().enumerate() {
                    let range = span(alternative.span());
                    if range.is_empty() {
                        self.push(depth + 1, range, "nothing (empty alternative)");
                        continue;
                    }
                    // Only label an alternative when it has several parts.
                    let first = self.parts.len();
                    self.visit(alternative, depth + 1);
                    if self.parts.len() - first > 1 {
                        for part in &mut self.parts[first..] {
                            part.depth += 1;
                        }
                        let label = Part {
                            depth: depth + 1,
                            span: range,
                            description: format!("alternative {}", ix + 1),
                        };
                        self.parts.insert(first, label);
                    }
                }
            }
            Ast::Group(group) => {
                let description = match &group.kind {
                    GroupKind::CaptureIndex(index) => format!("capture group #{index}"),
                    GroupKind::CaptureName { name, .. } => {
                        format!("named group “{}” (#{})", name.name, name.index)
                    }
                    GroupKind::NonCapturing(flags) if flags.items.is_empty() => {
                        "non-capturing group".to_owned()
                    }
                    GroupKind::NonCapturing(flags) => {
                        format!("non-capturing group that {}", describe_flags(&flags.items))
                    }
                };
                self.push(depth, span(&group.span), description);
                self.visit(&group.ast, depth + 1);
            }
            Ast::Repetition(repetition) => {
                let quantity = quantity(&repetition.op.kind, repetition.greedy);
                if is_leaf(&repetition.ast) {
                    let what = self.describe_leaf(&repetition.ast);
                    self.push(depth, span(&repetition.span), format!("{what}, {quantity}"));
                } else {
                    self.push(depth, span(&repetition.span), format!("repeat {quantity}:"));
                    self.visit(&repetition.ast, depth + 1);
                }
            }
            Ast::Flags(set) => {
                let description = describe_flags(&set.flags.items);
                self.push(
                    depth,
                    span(&set.span),
                    format!("from here on, {description}"),
                );
            }
            leaf => {
                let description = self.describe_leaf(leaf);
                self.push(depth, span(leaf.span()), description);
            }
        }
    }

    /// Visits a concatenation, merging runs of plain characters into one part.
    fn visit_sequence(&mut self, asts: &[Ast], depth: usize) {
        let mut ix = 0;
        while ix < asts.len() {
            let run_end = asts[ix..]
                .iter()
                .position(|ast| !matches!(ast, Ast::Literal(_)))
                .map_or(asts.len(), |len| ix + len);
            if run_end - ix > 1 {
                let range = span(asts[ix].span()).start..span(asts[run_end - 1].span()).end;
                let text: String = asts[ix..run_end]
                    .iter()
                    .filter_map(|ast| match ast {
                        Ast::Literal(literal) => Some(literal.c),
                        _ => None,
                    })
                    .collect();
                let description = format!("the text “{}”{}", visible(&text), self.case_note());
                self.push(depth, range, description);
                ix = run_end;
            } else {
                self.visit(&asts[ix], depth);
                ix += 1;
            }
        }
    }

    fn case_note(&self) -> &'static str {
        if self.flags.case_insensitive {
            " (ignoring case)"
        } else {
            ""
        }
    }

    fn describe_leaf(&self, ast: &Ast) -> String {
        match ast {
            Ast::Literal(literal) => {
                format!("{}{}", describe_literal(literal), self.case_note())
            }
            Ast::Dot(_) if self.flags.dot_matches_new_line => "any character".to_owned(),
            Ast::Dot(_) => "any character except a newline".to_owned(),
            Ast::Assertion(assertion) => self.describe_assertion(&assertion.kind).to_owned(),
            Ast::ClassPerl(class) => describe_perl(&class.kind, class.negated).to_owned(),
            Ast::ClassUnicode(class) => describe_unicode(class),
            Ast::ClassBracketed(class) => self.describe_bracketed(class),
            other => format!("“{}”", &self.pattern[span(other.span())]),
        }
    }

    fn describe_assertion(&self, kind: &AssertionKind) -> &'static str {
        match kind {
            AssertionKind::StartLine if self.flags.multi_line => "start of a line",
            AssertionKind::StartLine | AssertionKind::StartText => "start of the text",
            AssertionKind::EndLine if self.flags.multi_line => "end of a line",
            AssertionKind::EndLine | AssertionKind::EndText => "end of the text",
            AssertionKind::WordBoundary => "word boundary",
            AssertionKind::NotWordBoundary => "not a word boundary",
            AssertionKind::WordBoundaryStart | AssertionKind::WordBoundaryStartAngle => {
                "start of a word"
            }
            AssertionKind::WordBoundaryEnd | AssertionKind::WordBoundaryEndAngle => "end of a word",
            AssertionKind::WordBoundaryStartHalf => {
                "start of a word, or not after a word character"
            }
            AssertionKind::WordBoundaryEndHalf => "end of a word, or not before a word character",
        }
    }

    fn describe_bracketed(&self, class: &ast::ClassBracketed) -> String {
        let items = match &class.kind {
            ClassSet::Item(item) => self.describe_set_item(item),
            ClassSet::BinaryOp(op) => format!("“{}”", &self.pattern[span(&op.span)]),
        };
        if class.negated {
            format!("one character except {items}")
        } else {
            format!("one character from {items}")
        }
    }

    fn describe_set_item(&self, item: &ClassSetItem) -> String {
        match item {
            ClassSetItem::Empty(_) => "nothing".to_owned(),
            ClassSetItem::Literal(literal) => format!("“{}”", visible(&literal.c.to_string())),
            ClassSetItem::Range(range) => format!(
                "“{}” to “{}”",
                visible(&range.start.c.to_string()),
                visible(&range.end.c.to_string())
            ),
            ClassSetItem::Ascii(class) => {
                let name = ascii_name(&class.kind);
                if class.negated {
                    format!("not ASCII {name}")
                } else {
                    format!("ASCII {name}")
                }
            }
            ClassSetItem::Unicode(class) => describe_unicode(class),
            ClassSetItem::Perl(class) => describe_perl(&class.kind, class.negated).to_owned(),
            ClassSetItem::Bracketed(class) => format!("“{}”", &self.pattern[span(&class.span)]),
            ClassSetItem::Union(union) => {
                let items: Vec<String> = union
                    .items
                    .iter()
                    .map(|item| self.describe_set_item(item))
                    .collect();
                join_list(&items)
            }
        }
    }
}

fn is_leaf(ast: &Ast) -> bool {
    matches!(
        ast,
        Ast::Literal(_)
            | Ast::Dot(_)
            | Ast::Assertion(_)
            | Ast::ClassPerl(_)
            | Ast::ClassUnicode(_)
            | Ast::ClassBracketed(_)
    )
}

fn quantity(kind: &RepetitionKind, greedy: bool) -> String {
    let base = match kind {
        RepetitionKind::ZeroOrOne => "optional".to_owned(),
        RepetitionKind::ZeroOrMore => "zero or more times".to_owned(),
        RepetitionKind::OneOrMore => "one or more times".to_owned(),
        RepetitionKind::Range(RepetitionRange::Exactly(1)) => "exactly once".to_owned(),
        RepetitionKind::Range(RepetitionRange::Exactly(n)) => format!("exactly {n} times"),
        RepetitionKind::Range(RepetitionRange::AtLeast(n)) => format!("{n} or more times"),
        RepetitionKind::Range(RepetitionRange::Bounded(lo, hi)) => {
            format!("between {lo} and {hi} times")
        }
    };
    if greedy {
        base
    } else {
        format!("{base}, as few as possible")
    }
}

fn describe_literal(literal: &ast::Literal) -> String {
    let name = match &literal.kind {
        LiteralKind::Special(special) => Some(match special {
            SpecialLiteralKind::Bell => "bell character",
            SpecialLiteralKind::FormFeed => "form feed",
            SpecialLiteralKind::Tab => "tab",
            SpecialLiteralKind::LineFeed => "newline",
            SpecialLiteralKind::CarriageReturn => "carriage return",
            SpecialLiteralKind::VerticalTab => "vertical tab",
            SpecialLiteralKind::Space => "space",
        }),
        _ => match literal.c {
            ' ' => Some("space"),
            '\n' => Some("newline"),
            '\t' => Some("tab"),
            _ => None,
        },
    };
    match name {
        Some(name) => name.to_owned(),
        None => format!("the character “{}”", literal.c),
    }
}

fn describe_perl(kind: &ClassPerlKind, negated: bool) -> &'static str {
    match (kind, negated) {
        (ClassPerlKind::Digit, false) => "a digit",
        (ClassPerlKind::Digit, true) => "a non-digit",
        (ClassPerlKind::Space, false) => "a whitespace character",
        (ClassPerlKind::Space, true) => "a non-whitespace character",
        (ClassPerlKind::Word, false) => "a word character (letter, digit or underscore)",
        (ClassPerlKind::Word, true) => "a non-word character",
    }
}

fn describe_unicode(class: &ast::ClassUnicode) -> String {
    let name = match &class.kind {
        ClassUnicodeKind::OneLetter(letter) => unicode_letter_name(*letter)
            .map(str::to_owned)
            .unwrap_or_else(|| format!("Unicode class {letter}")),
        ClassUnicodeKind::Named(name) => format!("Unicode {name}"),
        ClassUnicodeKind::NamedValue { name, value, .. } => format!("Unicode {name}={value}"),
    };
    if class.is_negated() {
        format!("a character that is not {name}")
    } else {
        format!("a character in {name}")
    }
}

fn unicode_letter_name(letter: char) -> Option<&'static str> {
    Some(match letter {
        'L' => "Unicode letters",
        'N' => "Unicode numbers",
        'P' => "Unicode punctuation",
        'S' => "Unicode symbols",
        'Z' => "Unicode separators",
        'M' => "Unicode marks",
        'C' => "Unicode other characters",
        _ => return None,
    })
}

fn ascii_name(kind: &ClassAsciiKind) -> &'static str {
    match kind {
        ClassAsciiKind::Alnum => "letters and digits",
        ClassAsciiKind::Alpha => "letters",
        ClassAsciiKind::Ascii => "characters",
        ClassAsciiKind::Blank => "spaces and tabs",
        ClassAsciiKind::Cntrl => "control characters",
        ClassAsciiKind::Digit => "digits",
        ClassAsciiKind::Graph => "visible characters",
        ClassAsciiKind::Lower => "lowercase letters",
        ClassAsciiKind::Print => "printable characters",
        ClassAsciiKind::Punct => "punctuation",
        ClassAsciiKind::Space => "whitespace",
        ClassAsciiKind::Upper => "uppercase letters",
        ClassAsciiKind::Word => "word characters",
        ClassAsciiKind::Xdigit => "hex digits",
    }
}

fn describe_flags(items: &[ast::FlagsItem]) -> String {
    let mut on = Vec::new();
    let mut off = Vec::new();
    let mut negated = false;
    for item in items {
        match &item.kind {
            FlagsItemKind::Negation => negated = true,
            FlagsItemKind::Flag(flag) => {
                let name = match flag {
                    Flag::CaseInsensitive => "ignore case",
                    Flag::MultiLine => "multi-line",
                    Flag::DotMatchesNewLine => "dot matches newline",
                    Flag::SwapGreed => "swap greedy and lazy",
                    Flag::Unicode => "Unicode",
                    Flag::CRLF => "CRLF line endings",
                    Flag::IgnoreWhitespace => "ignore whitespace",
                };
                if negated {
                    off.push(name)
                } else {
                    on.push(name)
                }
            }
        }
    }
    let on = (!on.is_empty()).then(|| format!("turns on {}", join_list(&on)));
    let off = (!off.is_empty()).then(|| format!("turns off {}", join_list(&off)));
    match (on, off) {
        (Some(on), Some(off)) => format!("{on} and {off}"),
        (Some(text), None) | (None, Some(text)) => text,
        (None, None) => "changes no flags".to_owned(),
    }
}

fn join_list<S: AsRef<str>>(items: &[S]) -> String {
    match items {
        [] => String::new(),
        [only] => only.as_ref().to_owned(),
        [rest @ .., last] => {
            let rest: Vec<&str> = rest.iter().map(AsRef::as_ref).collect();
            format!("{} or {}", rest.join(", "), last.as_ref())
        }
    }
}

/// Makes whitespace visible in quoted text.
fn visible(text: &str) -> String {
    text.replace('\n', "⏎").replace('\t', "⇥")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn describe(pattern: &str) -> Vec<(usize, String, String)> {
        explain(pattern, Flags::default())
            .unwrap()
            .into_iter()
            .map(|part| (part.depth, pattern[part.span].to_owned(), part.description))
            .collect()
    }

    fn row(depth: usize, text: &str, description: &str) -> (usize, String, String) {
        (depth, text.to_owned(), description.to_owned())
    }

    #[test]
    fn explains_groups_and_repetition() {
        assert_eq!(
            describe(r"(?<year>\d{4})-\d+"),
            vec![
                row(0, r"(?<year>\d{4})", "named group “year” (#1)"),
                row(1, r"\d{4}", "a digit, exactly 4 times"),
                row(0, "-", "the character “-”"),
                row(0, r"\d+", "a digit, one or more times"),
            ]
        );
    }

    #[test]
    fn merges_plain_text_and_explains_alternatives() {
        assert_eq!(
            describe("^(cat|dog food)$"),
            vec![
                row(0, "^", "start of the text"),
                row(0, "(cat|dog food)", "capture group #1"),
                row(1, "cat|dog food", "one of 2 alternatives"),
                row(2, "cat", "the text “cat”"),
                row(2, "dog food", "the text “dog food”"),
                row(0, "$", "end of the text"),
            ]
        );
    }

    #[test]
    fn explains_classes() {
        assert_eq!(
            describe(r"[^a-z_\d]\pL.*?"),
            vec![
                row(
                    0,
                    r"[^a-z_\d]",
                    "one character except “a” to “z”, “_” or a digit"
                ),
                row(0, r"\pL", "a character in Unicode letters"),
                row(
                    0,
                    ".*?",
                    "any character except a newline, zero or more times, as few as possible"
                ),
            ]
        );
    }

    #[test]
    fn follows_flags() {
        let flags = Flags {
            multi_line: true,
            dot_matches_new_line: true,
            ..Flags::default()
        };
        let parts = explain("^.", flags).unwrap();
        assert_eq!(parts[0].description, "start of a line");
        assert_eq!(parts[1].description, "any character");

        assert_eq!(
            describe("(?i)a(?-s:b)")[0].2,
            "from here on, turns on ignore case"
        );
        assert_eq!(
            describe("(?i)a(?-s:b)")[2].2,
            "non-capturing group that turns off dot matches newline"
        );
    }

    #[test]
    fn reports_fancy_only_and_invalid_patterns() {
        assert_eq!(
            explain(r"a(?=b)", Flags::default()),
            Err(ExplainError::FancyOnly)
        );
        assert_eq!(
            explain(r"(a)\1", Flags::default()),
            Err(ExplainError::FancyOnly)
        );
        assert_eq!(explain("(", Flags::default()), Err(ExplainError::Invalid));
    }
}
