// src/parser.rs
// Converts a Markdown string into a structured document tree + TOC

use pulldown_cmark::{Event, HeadingLevel, Options, Parser, Tag, TagEnd};

/// A single entry in the Table of Contents
#[derive(Debug, Clone)]
pub struct TocEntry {
    pub level: u8, // 1–6
    pub title: String,
    /// Node index in `Document::nodes` where this heading lives
    pub node_index: usize,
}

/// A renderable document node produced by the parser
#[derive(Debug, Clone)]
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

#[derive(Debug, Clone)]
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

#[derive(Debug, Default)]
pub struct Document {
    pub nodes: Vec<DocNode>,
    pub toc: Vec<TocEntry>,
}

pub fn parse(markdown: &str) -> Document {
    let mut opts = Options::empty();
    opts.insert(Options::ENABLE_TABLES);
    opts.insert(Options::ENABLE_FOOTNOTES);
    opts.insert(Options::ENABLE_STRIKETHROUGH);
    opts.insert(Options::ENABLE_TASKLISTS);

    let parser = Parser::new_ext(markdown, opts);
    let events: Vec<Event> = parser.collect();

    let mut doc = Document::default();
    let mut pos = 0usize;

    while pos < events.len() {
        let event = get_event(&events, pos).unwrap();
        match event {
            Event::Start(Tag::Heading { level, .. }) => {
                pos += 1;
                let mut text = String::new();
                while let Some(e) = get_event(&events, pos) {
                    match e {
                        Event::Text(t) => text.push_str(t),
                        Event::Code(t) => text.push_str(t),
                        Event::End(TagEnd::Heading(_)) => break,
                        _ => {}
                    }
                    pos += 1;
                }
                let lvl = heading_level_to_u8(*level);
                let node_index = doc.nodes.len();
                doc.toc.push(TocEntry {
                    level: lvl,
                    title: text.clone(),
                    node_index,
                });
                doc.nodes.push(DocNode::Heading { level: lvl, text });
                doc.nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::Paragraph) => {
                pos += 1;
                let mut spans = Vec::new();
                collect_inline_spans(&events, &mut pos, &mut spans, TagEnd::Paragraph);
                if spans.len() == 1 {
                    if let InlineSpan::Image { src, alt } = &spans[0] {
                        doc.nodes.push(DocNode::Image {
                            src: src.clone(),
                            alt: alt.clone(),
                        });
                        doc.nodes.push(DocNode::Blank);
                        continue;
                    }
                }
                if !spans.is_empty() {
                    doc.nodes.push(DocNode::Paragraph(spans));
                    doc.nodes.push(DocNode::Blank);
                }
            }

            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        let s = lang.to_string();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s)
                        }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                pos += 1;
                let mut code = String::new();
                while let Some(e) = get_event(&events, pos) {
                    match e {
                        Event::Text(t) => code.push_str(t),
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                    pos += 1;
                }
                doc.nodes.push(DocNode::CodeBlock { language, code });
                doc.nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::BlockQuote(_)) => {
                pos += 1;
                let mut children: Vec<DocNode> = Vec::new();
                collect_blockquote(&events, &mut pos, &mut children);
                doc.nodes.push(DocNode::BlockQuote(children));
                doc.nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::List(start_num)) => {
                let ordered = start_num.is_some();
                let mut counter = start_num.unwrap_or(1);
                pos += 1;
                collect_list_items(&events, &mut pos, &mut doc.nodes, 0, ordered, &mut counter);
                doc.nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::Table(_)) => {
                pos += 1;
                let mut headers: Vec<String> = Vec::new();
                let mut rows: Vec<Vec<String>> = Vec::new();
                collect_table(&events, &mut pos, &mut headers, &mut rows);
                doc.nodes.push(DocNode::Table { headers, rows });
                doc.nodes.push(DocNode::Blank);
            }

            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                let alt = collect_image_alt_text(&events, &mut pos);
                doc.nodes.push(DocNode::Image {
                    src: dest_url.to_string(),
                    alt: if alt.is_empty() {
                        title.to_string()
                    } else {
                        alt
                    },
                });
                doc.nodes.push(DocNode::Blank);
            }

            Event::Rule => {
                doc.nodes.push(DocNode::Rule);
                doc.nodes.push(DocNode::Blank);
            }

            Event::HardBreak | Event::SoftBreak => {}

            _ => {}
        }
        pos += 1;
    }

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

fn get_event<'a>(events: &'a [Event<'a>], pos: usize) -> Option<&'a Event<'a>> {
    events.get(pos)
}

fn collect_inline_spans(
    events: &[Event],
    pos: &mut usize,
    spans: &mut Vec<InlineSpan>,
    end: TagEnd,
) {
    let mut bold = false;
    let mut italic = false;
    let mut strike = false;
    let mut link_url: Option<String> = None;
    let mut link_text = String::new();

    while *pos < events.len() {
        match &events[*pos] {
            Event::End(t) if t == &end => break,
            Event::Start(Tag::Strong) => bold = true,
            Event::End(TagEnd::Strong) => bold = false,
            Event::Start(Tag::Emphasis) => italic = true,
            Event::End(TagEnd::Emphasis) => italic = false,
            Event::Start(Tag::Strikethrough) => strike = true,
            Event::End(TagEnd::Strikethrough) => strike = false,
            Event::Start(Tag::Link { dest_url, .. }) => {
                link_url = Some(dest_url.to_string());
                link_text.clear();
            }
            Event::End(TagEnd::Link) => {
                if let Some(url) = link_url.take() {
                    spans.push(InlineSpan::Link {
                        text: link_text.clone(),
                        url,
                    });
                    link_text.clear();
                }
            }
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                let alt = collect_image_alt_text(events, pos);
                spans.push(InlineSpan::Image {
                    src: dest_url.to_string(),
                    alt: if alt.is_empty() {
                        title.to_string()
                    } else {
                        alt
                    },
                });
            }
            Event::Text(t) => {
                if link_url.is_some() {
                    link_text.push_str(t);
                } else if bold && italic {
                    spans.push(InlineSpan::BoldItalic(t.to_string()));
                } else if bold {
                    spans.push(InlineSpan::Bold(t.to_string()));
                } else if italic {
                    spans.push(InlineSpan::Italic(t.to_string()));
                } else if strike {
                    spans.push(InlineSpan::Strikethrough(t.to_string()));
                } else {
                    spans.push(InlineSpan::Text(t.to_string()));
                }
            }
            Event::Code(t) => {
                spans.push(InlineSpan::Code(t.to_string()));
            }
            Event::SoftBreak => spans.push(InlineSpan::SoftBreak),
            Event::HardBreak => spans.push(InlineSpan::HardBreak),
            _ => {}
        }
        *pos += 1;
    }
}

fn collect_image_alt_text(events: &[Event], pos: &mut usize) -> String {
    *pos += 1;
    let mut alt = String::new();

    while *pos < events.len() {
        match &events[*pos] {
            Event::End(TagEnd::Image) => break,
            Event::Text(t) | Event::Code(t) => alt.push_str(t),
            Event::SoftBreak | Event::HardBreak => alt.push(' '),
            _ => {}
        }
        *pos += 1;
    }

    alt
}

fn collect_blockquote(events: &[Event], pos: &mut usize, children: &mut Vec<DocNode>) {
    while *pos < events.len() {
        match &events[*pos] {
            Event::End(TagEnd::BlockQuote(_)) => break,
            Event::Start(Tag::Heading { level, .. }) => {
                *pos += 1;
                let mut text = String::new();
                while let Some(e) = get_event(events, *pos) {
                    match e {
                        Event::Text(t) => text.push_str(t),
                        Event::Code(t) => text.push_str(t),
                        Event::End(TagEnd::Heading(_)) => break,
                        _ => {}
                    }
                    *pos += 1;
                }
                children.push(DocNode::Heading {
                    level: heading_level_to_u8(*level),
                    text,
                });
                children.push(DocNode::Blank);
            }
            Event::Start(Tag::Paragraph) => {
                *pos += 1;
                let mut spans = Vec::new();
                collect_inline_spans(events, pos, &mut spans, TagEnd::Paragraph);
                if spans.len() == 1 {
                    if let InlineSpan::Image { src, alt } = &spans[0] {
                        children.push(DocNode::Image {
                            src: src.clone(),
                            alt: alt.clone(),
                        });
                        children.push(DocNode::Blank);
                        continue;
                    }
                }
                if !spans.is_empty() {
                    children.push(DocNode::Paragraph(spans));
                    children.push(DocNode::Blank);
                }
            }
            Event::Start(Tag::CodeBlock(kind)) => {
                let language = match kind {
                    pulldown_cmark::CodeBlockKind::Fenced(lang) => {
                        let s = lang.to_string();
                        if s.is_empty() {
                            None
                        } else {
                            Some(s)
                        }
                    }
                    pulldown_cmark::CodeBlockKind::Indented => None,
                };
                *pos += 1;
                let mut code = String::new();
                while let Some(e) = get_event(events, *pos) {
                    match e {
                        Event::Text(t) => code.push_str(t),
                        Event::End(TagEnd::CodeBlock) => break,
                        _ => {}
                    }
                    *pos += 1;
                }
                children.push(DocNode::CodeBlock { language, code });
                children.push(DocNode::Blank);
            }
            Event::Start(Tag::BlockQuote(_)) => {
                *pos += 1;
                let mut nested_children = Vec::new();
                collect_blockquote(events, pos, &mut nested_children);
                children.push(DocNode::BlockQuote(nested_children));
                children.push(DocNode::Blank);
            }
            Event::Start(Tag::List(start_num)) => {
                let ordered = start_num.is_some();
                let mut counter = start_num.unwrap_or(1);
                *pos += 1;
                collect_list_items(events, pos, children, 0, ordered, &mut counter);
                children.push(DocNode::Blank);
            }
            Event::Start(Tag::Table(_)) => {
                *pos += 1;
                let mut headers = Vec::new();
                let mut rows = Vec::new();
                collect_table(events, pos, &mut headers, &mut rows);
                children.push(DocNode::Table { headers, rows });
                children.push(DocNode::Blank);
            }
            Event::Start(Tag::Image {
                dest_url, title, ..
            }) => {
                let alt = collect_image_alt_text(events, pos);
                children.push(DocNode::Image {
                    src: dest_url.to_string(),
                    alt: if alt.is_empty() {
                        title.to_string()
                    } else {
                        alt
                    },
                });
                children.push(DocNode::Blank);
            }
            Event::Rule => {
                children.push(DocNode::Rule);
                children.push(DocNode::Blank);
            }
            Event::HardBreak | Event::SoftBreak => {}
            _ => {}
        }
        *pos += 1;
    }
}

fn collect_list_items(
    events: &[Event],
    pos: &mut usize,
    nodes: &mut Vec<DocNode>,
    depth: usize,
    ordered: bool,
    counter: &mut u64,
) {
    while *pos < events.len() {
        match &events[*pos] {
            Event::End(TagEnd::List(_)) => break,
            Event::Start(Tag::Item) => {
                *pos += 1;
                let mut spans = Vec::new();
                let number = if ordered {
                    let n = *counter;
                    *counter += 1;
                    Some(n)
                } else {
                    None
                };
                // collect inline content of item until End(Item) or nested list
                while *pos < events.len() {
                    match &events[*pos] {
                        Event::End(TagEnd::Item) => break,
                        Event::Start(Tag::Paragraph) => {
                            *pos += 1;
                            collect_inline_spans(events, pos, &mut spans, TagEnd::Paragraph);
                        }
                        Event::Start(Tag::List(start)) => {
                            // flush current spans first
                            if !spans.is_empty() {
                                nodes.push(DocNode::ListItem {
                                    depth,
                                    ordered,
                                    number,
                                    children: spans.clone(),
                                });
                                spans.clear();
                            }
                            let child_ordered = start.is_some();
                            let mut child_counter = start.unwrap_or(1);
                            *pos += 1;
                            collect_list_items(
                                events,
                                pos,
                                nodes,
                                depth + 1,
                                child_ordered,
                                &mut child_counter,
                            );
                        }
                        Event::Text(t) => {
                            spans.push(InlineSpan::Text(t.to_string()));
                        }
                        Event::Code(t) => {
                            spans.push(InlineSpan::Code(t.to_string()));
                        }
                        Event::SoftBreak => spans.push(InlineSpan::SoftBreak),
                        _ => {}
                    }
                    *pos += 1;
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
        *pos += 1;
    }
}

fn collect_table(
    events: &[Event],
    pos: &mut usize,
    headers: &mut Vec<String>,
    rows: &mut Vec<Vec<String>>,
) {
    let mut in_head = false;
    let mut current_row: Vec<String> = Vec::new();
    let mut current_cell = String::new();

    while *pos < events.len() {
        match &events[*pos] {
            Event::End(TagEnd::Table) => break,
            Event::Start(Tag::TableHead) => in_head = true,
            Event::End(TagEnd::TableHead) => {
                if !current_cell.is_empty() {
                    current_row.push(current_cell.trim().to_string());
                    current_cell.clear();
                }
                if !current_row.is_empty() {
                    *headers = current_row.clone();
                    current_row.clear();
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
                    rows.push(current_row.clone());
                    current_row.clear();
                }
            }
            Event::Start(Tag::TableCell) => {
                current_cell.clear();
            }
            Event::End(TagEnd::TableCell) => {
                current_row.push(current_cell.trim().to_string());
                current_cell.clear();
            }
            Event::Text(t) => current_cell.push_str(t),
            Event::Code(t) => current_cell.push_str(t),
            _ => {}
        }
        *pos += 1;
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
}
