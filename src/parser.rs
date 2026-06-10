// src/parser.rs
// Converts a Markdown string into a structured document tree + TOC

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};
use std::iter::Peekable;

/// A single entry in the Table of Contents
#[derive(Debug, Clone, PartialEq)]
pub struct TocEntry {
    pub level: u8, // 1–6
    pub title: String,
    /// Node index in `Document::nodes` where this heading lives
    pub node_index: usize,
}

/// A renderable document node produced by the parser
#[derive(Debug, Clone, PartialEq)]
pub enum DocNode {
    Heading {
        level: u8,
        text: String,
    },
    Paragraph(Vec<InlineSpan>),
    CodeBlock {
        language: Option<String>,
        code: String,
    },

    BlockQuote(Vec<DocNode>),
    ListItem {
        depth: usize,
        ordered: bool,
        number: Option<u64>,
        children: Vec<InlineSpan>,
    },
    Table {
        headers: Vec<String>,
        rows: Vec<Vec<String>>,
    },
    Rule,
    Image {
        src: String,
        alt: String,
    },
    /// Blank line / spacer
    Blank,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InlineSpan {
    Text(String),
    Bold(String),
    Italic(String),
    BoldItalic(String),
    Code(String),
    Strikethrough(String),
    Link { text: String, url: String },
    Image { src: String, alt: String },
    SoftBreak,
    HardBreak,
}

#[derive(Debug, Clone, Default)]
pub struct Document {
    pub nodes: Vec<DocNode>,
    pub toc: Vec<TocEntry>,
}

type EventIter<'a> = Peekable<Parser<'a>>;

pub fn parse(markdown: &str) -> Document {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let mut events = Parser::new_ext(markdown, opts).peekable();
    let mut doc = Document::default();

    parse_blocks(&mut events, &mut doc.nodes, Some(&mut doc.toc), false);

    doc
}

fn heading_level_to_u8(level: HeadingLevel) -> u8 {
    match level {
        HeadingLevel::H1 => 1,
        HeadingLevel::H2 => 2,
        HeadingLevel::H3 => 3,
        HeadingLevel::H4 => 4,
        HeadingLevel::H5 => 5,
        HeadingLevel::H6 => 6,
    }
}

fn extract_heading_text(events: &mut EventIter<'_>) -> String {
    let mut text = String::new();
    for e in events.by_ref() {
        match e {
            Event::Text(t) => text.push_str(&t),
            Event::Code(t) => text.push_str(&t),
            Event::End(TagEnd::Heading(_)) => break,
            _ => {}
        }
    }
    text
}

fn extract_code_content(events: &mut EventIter<'_>) -> String {
    let mut code = String::new();
    for e in events.by_ref() {
        match e {
            Event::Text(t) => code.push_str(&t),
            Event::End(TagEnd::CodeBlock) => break,
            _ => {}
        }
    }
    code
}

fn extract_language(kind: &pulldown_cmark::CodeBlockKind) -> Option<String> {
    match kind {
        pulldown_cmark::CodeBlockKind::Fenced(lang) => {
            if lang.is_empty() {
                None
            } else {
                Some(lang.to_string())
            }
        }
        pulldown_cmark::CodeBlockKind::Indented => None,
    }
}

#[derive(Default)]
struct InlineState {
    bold: bool,
    italic: bool,
    strike: bool,
    link_url: Option<String>,
    link_text: String,
}

fn handle_inline_event(
    event: Event<'_>,
    spans: &mut Vec<InlineSpan>,
    state: &mut InlineState,
    events: &mut EventIter<'_>,
) {
    match event {
        Event::Start(Tag::Strong) => state.bold = true,
        Event::End(TagEnd::Strong) => state.bold = false,
        Event::Start(Tag::Emphasis) => state.italic = true,
        Event::End(TagEnd::Emphasis) => state.italic = false,
        Event::Start(Tag::Strikethrough) => state.strike = true,
        Event::End(TagEnd::Strikethrough) => state.strike = false,
        Event::Start(Tag::Link { dest_url, .. }) => {
            state.link_url = Some(dest_url.into_string());
            state.link_text.clear();
        }
        Event::End(TagEnd::Link) => {
            if let Some(url) = state.link_url.take() {
                spans.push(InlineSpan::Link {
                    text: state.link_text.clone(),
                    url,
                });
                state.link_text.clear();
            }
        }
        Event::Start(Tag::Image {
            dest_url, title, ..
        }) => {
            let alt = collect_image_alt_text(events);
            spans.push(InlineSpan::Image {
                src: dest_url.into_string(),
                alt: if alt.is_empty() {
                    title.into_string()
                } else {
                    alt
                },
            });
        }
        Event::Text(t) => {
            if state.link_url.is_some() {
                state.link_text.push_str(&t);
            } else if state.bold && state.italic {
                spans.push(InlineSpan::BoldItalic(t.into_string()));
            } else if state.bold {
                spans.push(InlineSpan::Bold(t.into_string()));
            } else if state.italic {
                spans.push(InlineSpan::Italic(t.into_string()));
            } else if state.strike {
                spans.push(InlineSpan::Strikethrough(t.into_string()));
            } else {
                spans.push(InlineSpan::Text(t.into_string()));
            }
        }
        Event::Code(t) => {
            if state.link_url.is_some() {
                state.link_text.push_str(&format!("`{}`", t));
            } else {
                spans.push(InlineSpan::Code(t.into_string()));
            }
        }
        Event::Html(t) | Event::InlineHtml(t) => {
            if state.link_url.is_some() {
                state.link_text.push_str(&t);
            } else {
                spans.push(InlineSpan::Text(t.into_string()));
            }
        }
        Event::TaskListMarker(checked) => {
            let marker = if checked { "☑ " } else { "☐ " };
            spans.push(InlineSpan::Text(marker.to_string()));
        }
        Event::SoftBreak => spans.push(InlineSpan::SoftBreak),
        Event::HardBreak => spans.push(InlineSpan::HardBreak),
        _ => {}
    }
}

fn collect_inline_spans(events: &mut EventIter<'_>, spans: &mut Vec<InlineSpan>, end: TagEnd) {
    let mut state = InlineState::default();
    while let Some(event) = events.next() {
        if let Event::End(ref t) = event {
            if *t == end {
                break;
            }
        }
        handle_inline_event(event, spans, &mut state, events);
    }
}

fn collect_image_alt_text(events: &mut EventIter<'_>) -> String {
    let mut alt = String::new();
    for event in events.by_ref() {
        match event {
            Event::End(TagEnd::Image) => break,
            Event::Text(t) | Event::Code(t) => alt.push_str(&t),
            Event::SoftBreak | Event::HardBreak => alt.push(' '),
            _ => {}
        }
    }
    alt
}

fn parse_blocks(
    events: &mut EventIter<'_>,
    nodes: &mut Vec<DocNode>,
    mut toc: Option<&mut Vec<TocEntry>>,
    is_blockquote: bool,
) {
    while let Some(event) = events.next() {
        if is_blockquote {
            if let Event::End(TagEnd::BlockQuote(_)) = event {
                break;
            }
        }

        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                let text = extract_heading_text(events);
                let lvl = heading_level_to_u8(level);
                let node_index = nodes.len();
                if let Some(ref mut t) = toc {
                    t.push(TocEntry {
                        level: lvl,
                        title: text.clone(),
                        node_index,
                    });
                }
                nodes.push(DocNode::Heading { level: lvl, text });
                nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::Paragraph) => {
                let mut spans = Vec::new();
                collect_inline_spans(events, &mut spans, TagEnd::Paragraph);
                if spans.len() == 1 {
                    if let InlineSpan::Image { src, alt } = &spans[0] {
                        nodes.push(DocNode::Image {
                            src: src.clone(),
                            alt: alt.clone(),
                        });
                        nodes.push(DocNode::Blank);
                        continue;
                    }
                }
                if !spans.is_empty() {
                    nodes.push(DocNode::Paragraph(spans));
                    nodes.push(DocNode::Blank);
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                let language = extract_language(&kind);
                let code = extract_code_content(events);
                nodes.push(DocNode::CodeBlock { language, code });
                nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::BlockQuote(_)) => {
                let mut children: Vec<DocNode> = Vec::new();
                parse_blocks(events, &mut children, None, true);
                nodes.push(DocNode::BlockQuote(children));
                nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::List(start_num)) => {
                let ordered = start_num.is_some();
                let mut counter = start_num.unwrap_or(1);
                collect_list_items(events, nodes, 0, ordered, &mut counter);
                nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::Table(_)) => {
                let mut headers: Vec<String> = Vec::new();
                let mut rows: Vec<Vec<String>> = Vec::new();
                collect_table(events, &mut headers, &mut rows);
                nodes.push(DocNode::Table { headers, rows });
                nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                let alt = collect_image_alt_text(events);
                nodes.push(DocNode::Image {
                    src: dest_url.into_string(),
                    alt: if alt.is_empty() {
                        title.into_string()
                    } else {
                        alt
                    },
                });
                nodes.push(DocNode::Blank);
            }

            Event::Rule => {
                nodes.push(DocNode::Rule);
                nodes.push(DocNode::Blank);
            }

            Event::Html(t) => {
                nodes.push(DocNode::Paragraph(vec![InlineSpan::Text(t.into_string())]));
                nodes.push(DocNode::Blank);
            }

            Event::HardBreak | Event::SoftBreak => {}

            _ => {}
        }
    }
}

fn collect_list_items(
    events: &mut EventIter<'_>,
    nodes: &mut Vec<DocNode>,
    depth: usize,
    ordered: bool,
    counter: &mut u64,
) {
    while let Some(event) = events.next() {
        match event {
            Event::End(TagEnd::List(_)) => break,
            Event::Start(Tag::Item) => {
                let mut spans = Vec::new();
                let number = if ordered {
                    let n = *counter;
                    *counter += 1;
                    Some(n)
                } else {
                    None
                };
                let mut state = InlineState::default();
                while let Some(item_event) = events.next() {
                    match item_event {
                        Event::End(TagEnd::Item) => break,
                        Event::Start(Tag::Paragraph) => {
                            collect_inline_spans(events, &mut spans, TagEnd::Paragraph);
                        }
                        Event::Start(Tag::List(start)) => {
                            if !spans.is_empty() {
                                nodes.push(DocNode::ListItem {
                                    depth,
                                    ordered,
                                    number,
                                    children: std::mem::take(&mut spans),
                                });
                            }
                            let child_ordered = start.is_some();
                            let mut child_counter = start.unwrap_or(1);
                            collect_list_items(
                                events,
                                nodes,
                                depth + 1,
                                child_ordered,
                                &mut child_counter,
                            );
                        }
                        other => {
                            handle_inline_event(other, &mut spans, &mut state, events);
                        }
                    }
                }
                if !spans.is_empty() {
                    nodes.push(DocNode::ListItem {
                        depth,
                        ordered,
                        number,
                        children: spans,
                    });
                }
            }
            _ => {}
        }
    }
}

fn collect_table(
    events: &mut EventIter<'_>,
    headers: &mut Vec<String>,
    rows: &mut Vec<Vec<String>>,
) {
    let mut in_head = false;
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();

    for event in events.by_ref() {
        match event {
            Event::End(TagEnd::Table) => break,
            Event::Start(Tag::TableHead) => in_head = true,
            Event::End(TagEnd::TableHead) => {
                if !current_cell.is_empty() {
                    current_row.push(current_cell.trim().to_string());
                    current_cell.clear();
                }
                if !current_row.is_empty() {
                    *headers = std::mem::take(&mut current_row);
                }
                in_head = false;
            }
            Event::Start(Tag::TableRow) => {
                current_row.clear();
            }
            Event::End(TagEnd::TableRow) => {
                if !current_cell.is_empty() {
                    current_row.push(current_cell.trim().to_string());
                    current_cell.clear();
                }
                if !in_head && !current_row.is_empty() {
                    rows.push(std::mem::take(&mut current_row));
                }
            }
            Event::Start(Tag::TableCell) => {
                current_cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                current_row.push(current_cell.trim().to_string());
                current_cell.clear();
            }
            Event::Text(t) => current_cell.push_str(&t),
            Event::Code(t) => current_cell.push_str(&t),
            _ => {}
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse, DocNode, InlineSpan};

    #[test]
    fn parses_link_text_and_url() {
        let document = parse("[docs](https://example.com)");

        match &document.nodes[0] {
            DocNode::Paragraph(spans) => match &spans[0] {
                InlineSpan::Link { text, url } => {
                    assert_eq!(text, "docs");
                    assert_eq!(url, "https://example.com");
                }
                other => panic!("expected link span, got {other:?}"),
            },
            other => panic!("expected paragraph, got {other:?}"),
        }
    }

    #[test]
    fn parses_image_alt_text_from_markdown() {
        let document = parse("![diagram](images/arch.png \"title\")");

        match &document.nodes[0] {
            DocNode::Image { src, alt } => {
                assert_eq!(src, "images/arch.png");
                assert_eq!(alt, "diagram");
            }
            other => panic!("expected image node, got {other:?}"),
        }
    }

    #[test]
    fn keeps_non_paragraph_content_inside_blockquotes() {
        let document = parse("> # Quoted heading\n>\n> ```rs\n> let x = 1;\n> ```");

        match &document.nodes[0] {
            DocNode::BlockQuote(children) => {
                assert!(matches!(children[0], DocNode::Heading { .. }));
                assert!(children
                    .iter()
                    .any(|node| matches!(node, DocNode::CodeBlock { .. })));
            }
            other => panic!("expected blockquote, got {other:?}"),
        }
    }

    #[test]
    fn parses_headings_across_levels() {
        let document = parse("# H1\n## H2\n### H3\n###### H6\n");
        // headings produce heading nodes; toc entries map each
        let heading_count = document
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::Heading { .. }))
            .count();
        assert_eq!(
            heading_count, 4,
            "should have 4 heading nodes, got {heading_count}"
        );
        assert_eq!(document.toc.len(), 4);
        // verify levels irrespective of order/extra nodes
        let levels: Vec<u8> = document
            .nodes
            .iter()
            .filter_map(|n| {
                if let DocNode::Heading { level, .. } = n {
                    Some(*level)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(levels, vec![1, 2, 3, 6]);
    }

    #[test]
    fn parses_code_block_with_language() {
        let document = parse("```rust\nfn main() {\n    println!(\"hi\");\n}\n```");
        let code_nodes: Vec<&DocNode> = document
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::CodeBlock { .. }))
            .collect();
        assert_eq!(
            code_nodes.len(),
            1,
            "should have 1 code block, got {}",
            code_nodes.len()
        );
        if let DocNode::CodeBlock { language, code } = &code_nodes[0] {
            assert_eq!(language.as_deref(), Some("rust"));
            assert!(code.contains("fn main()"));
            assert!(code.contains("println!"));
        }
    }

    #[test]
    fn parses_code_block_without_language() {
        let document = parse("```\nraw code\n```");
        match &document.nodes[0] {
            DocNode::CodeBlock { language, code } => {
                assert!(language.is_none());
                assert_eq!(code.trim(), "raw code");
            }
            other => panic!("expected CodeBlock, got {other:?}"),
        }
    }

    #[test]
    fn parses_unordered_list_items() {
        let document = parse("- one\n- two\n- three\n");
        let list_nodes: Vec<&DocNode> = document
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::ListItem { .. }))
            .collect();
        assert_eq!(
            list_nodes.len(),
            3,
            "should have 3 list items, got {}",
            list_nodes.len()
        );
        for node in &list_nodes {
            match node {
                DocNode::ListItem {
                    ordered,
                    number,
                    depth,
                    ..
                } => {
                    assert!(!*ordered, "should be unordered");
                    assert_eq!(*number, None);
                    assert_eq!(*depth, 0);
                }
                other => panic!("expected ListItem, got {other:?}"),
            }
        }
    }

    #[test]
    fn parses_ordered_list_items() {
        let document = parse("1. first\n2. second\n");
        let list_nodes: Vec<&DocNode> = document
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::ListItem { .. }))
            .collect();
        assert_eq!(list_nodes.len(), 2);
        for node in &list_nodes {
            if let DocNode::ListItem { ordered, .. } = node {
                assert!(*ordered);
            }
        }
    }

    #[test]
    fn parses_table() {
        let document = parse("| A | B |\n|---|---|\n| 1 | 2 |\n");
        match &document.nodes[0] {
            DocNode::Table { headers, rows } => {
                assert_eq!(headers, &["A", "B"]);
                assert_eq!(rows.len(), 1);
                assert_eq!(rows[0], ["1", "2"]);
            }
            other => panic!("expected Table, got {other:?}"),
        }
    }

    #[test]
    fn parses_thematic_break_as_rule() {
        let document = parse("---\n\n***\n");
        let rules: Vec<&DocNode> = document
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::Rule))
            .collect();
        assert_eq!(rules.len(), 2);
    }

    #[test]
    fn parses_inline_formatting() {
        let document = parse("**bold** *italic* `code` ~~strike~~");
        match &document.nodes[0] {
            DocNode::Paragraph(spans) => {
                assert!(spans
                    .iter()
                    .any(|s| matches!(s, InlineSpan::Bold(t) if t == "bold")));
                assert!(spans
                    .iter()
                    .any(|s| matches!(s, InlineSpan::Italic(t) if t == "italic")));
                assert!(spans
                    .iter()
                    .any(|s| matches!(s, InlineSpan::Code(t) if t == "code")));
                assert!(spans
                    .iter()
                    .any(|s| matches!(s, InlineSpan::Strikethrough(t) if t == "strike")));
            }
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }

    #[test]
    fn parses_hard_break() {
        let document = parse("line1  \nline2\n");
        match &document.nodes[0] {
            DocNode::Paragraph(spans) => {
                assert!(spans.iter().any(|s| matches!(s, InlineSpan::HardBreak)));
            }
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }

    #[test]
    fn table_toc_entries_have_empty_text() {
        let document = parse("| H1 | H2 |\n|----|----|\n| a  | b  |\n");
        assert!(document.toc.is_empty());
    }

    #[test]
    fn empty_document_has_no_nodes() {
        let document = parse("");
        assert!(document.nodes.is_empty());
        assert!(document.toc.is_empty());
    }

    #[test]
    fn parses_soft_break_within_paragraph() {
        let document = parse("hello\nworld\n");
        match &document.nodes[0] {
            DocNode::Paragraph(spans) => {
                assert!(spans.iter().any(|s| matches!(s, InlineSpan::SoftBreak)));
            }
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }

    #[test]
    fn parses_bold_italic_combined() {
        let document = parse("***bold italic***");
        match &document.nodes[0] {
            DocNode::Paragraph(spans) => {
                assert!(spans
                    .iter()
                    .any(|s| matches!(s, InlineSpan::BoldItalic(t) if t == "bold italic")));
            }
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }

    #[test]
    fn inline_code_with_backticks() {
        let document = parse("`` `code` ``");
        match &document.nodes[0] {
            DocNode::Paragraph(spans) => {
                assert!(spans
                    .iter()
                    .any(|s| matches!(s, InlineSpan::Code(t) if t == "`code`")));
            }
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }

    #[test]
    fn list_items_preserve_inline_formatting() {
        let document = parse("- **bold** *italic* `code` ~~strike~~ [link](url)\n");
        let list_nodes: Vec<&DocNode> = document
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::ListItem { .. }))
            .collect();
        assert_eq!(list_nodes.len(), 1);
        match list_nodes[0] {
            DocNode::ListItem { children, .. } => {
                let kinds: Vec<&str> = children
                    .iter()
                    .map(|s| match s {
                        InlineSpan::Bold(_) => "Bold",
                        InlineSpan::Italic(_) => "Italic",
                        InlineSpan::Code(_) => "Code",
                        InlineSpan::Strikethrough(_) => "Strikethrough",
                        InlineSpan::Link { .. } => "Link",
                        InlineSpan::Text(_) => "Text",
                        _ => "Other",
                    })
                    .collect();
                // Should contain formatted spans, not just plain Text
                assert!(kinds.contains(&"Bold"), "missing bold: {:?}", kinds);
                assert!(kinds.contains(&"Italic"), "missing italic: {:?}", kinds);
                assert!(kinds.contains(&"Code"), "missing code: {:?}", kinds);
                assert!(
                    kinds.contains(&"Strikethrough"),
                    "missing strikethrough: {:?}",
                    kinds
                );
                assert!(kinds.contains(&"Link"), "missing link: {:?}", kinds);
            }
            other => panic!("expected ListItem, got {other:?}"),
        }
    }

    #[test]
    fn tight_list_without_paragraph_still_formats_bold() {
        // Tight list: no blank lines, no Paragraph tag from pulldown-cmark
        let document = parse("- plain **bold**\n- item2\n");
        let list_nodes: Vec<&DocNode> = document
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::ListItem { .. }))
            .collect();
        assert_eq!(list_nodes.len(), 2);
        match list_nodes[0] {
            DocNode::ListItem { children, .. } => {
                assert!(
                    children
                        .iter()
                        .any(|s| matches!(s, InlineSpan::Bold(t) if t == "bold")),
                    "tight list should preserve bold: {:?}",
                    children
                );
            }
            other => panic!("expected ListItem, got {other:?}"),
        }
    }

    #[test]
    fn parses_task_list_items() {
        let document = parse("- [x] done\n- [ ] todo\n");
        let list_nodes: Vec<&DocNode> = document
            .nodes
            .iter()
            .filter(|n| matches!(n, DocNode::ListItem { .. }))
            .collect();
        assert_eq!(list_nodes.len(), 2);
        if let DocNode::ListItem { children, .. } = &list_nodes[0] {
            assert!(matches!(&children[0], InlineSpan::Text(s) if s == "☑ "));
        } else {
            panic!("expected ListItem");
        }
        if let DocNode::ListItem { children, .. } = &list_nodes[1] {
            assert!(matches!(&children[0], InlineSpan::Text(s) if s == "☐ "));
        } else {
            panic!("expected ListItem");
        }
    }

    #[test]
    fn parses_code_in_link() {
        let document = parse("[`code`](url)");
        match &document.nodes[0] {
            DocNode::Paragraph(spans) => match &spans[0] {
                InlineSpan::Link { text, url } => {
                    assert_eq!(text, "`code`");
                    assert_eq!(url, "url");
                }
                other => panic!("expected Link, got {other:?}"),
            },
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }

    #[test]
    fn parses_html_blocks_and_inline() {
        let document = parse("<div>test</div>\n\nhello <span>world</span>");
        assert_eq!(document.nodes.len(), 4); // Para1 + Blank1 + Para2 + Blank2
        match &document.nodes[0] {
            DocNode::Paragraph(spans) => {
                assert_eq!(spans[0], InlineSpan::Text("<div>test</div>\n".to_string()));
            }
            other => panic!("expected Paragraph, got {other:?}"),
        }
        match &document.nodes[2] {
            DocNode::Paragraph(spans) => {
                assert_eq!(spans[0], InlineSpan::Text("hello ".to_string()));
                assert_eq!(spans[1], InlineSpan::Text("<span>".to_string()));
                assert_eq!(spans[2], InlineSpan::Text("world".to_string()));
                assert_eq!(spans[3], InlineSpan::Text("</span>".to_string()));
            }
            other => panic!("expected Paragraph, got {other:?}"),
        }
    }
}
