//! Renderer module: converts DocNode trees into ratatui Lines for display.
//! Submodules are organized by functional domain.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::parser::DocNode;
use crate::theme::Theme;

pub mod code;
pub mod inline;
pub mod measure;
pub mod search;
pub mod table;
#[cfg(test)]
mod tests;

pub const IMAGE_RENDER_HEIGHT: usize = 10;

const MAX_HL_CACHE: usize = 64;

const CODE_BLOCK_LEFT_BORDER: &str = "│ ";
const CODE_BLOCK_RIGHT_BORDER: &str = "│";
const CODE_BLOCK_TOP_LEFT: &str = "╭─ ";
const CODE_BLOCK_TOP_RIGHT: &str = "╮";
const CODE_BLOCK_BOTTOM_LEFT: &str = "╰─";
const CODE_BLOCK_BOTTOM_RIGHT: &str = "┘";
const CODE_BLOCK_SPACE_BEFORE_DASHES: usize = 1;

const BLOCKQUOTE_LEFT_BORDER: &str = "│ ";
const BLOCKQUOTE_RIGHT_BORDER: &str = "│";
const BLOCKQUOTE_TOP_LEFT: &str = "╭─";
const BLOCKQUOTE_TOP_RIGHT: &str = "─╮";
const BLOCKQUOTE_BOTTOM_LEFT: &str = "╰─";
const BLOCKQUOTE_BOTTOM_RIGHT: &str = "─╯";
const BLOCKQUOTE_HORIZONTAL: &str = "─";

const RULE_CHAR: &str = "─";

const TABLE_VERTICAL_BORDER: &str = "│";
const TABLE_TOP_LEFT: &str = "╭";
const TABLE_TOP_MID: &str = "┬";
const TABLE_TOP_RIGHT: &str = "╮";
const TABLE_MID_LEFT: &str = "├";
const TABLE_MID_MID: &str = "┼";
const TABLE_MID_RIGHT: &str = "┤";
const TABLE_BOTTOM_LEFT: &str = "╰";
const TABLE_BOTTOM_MID: &str = "┴";
const TABLE_BOTTOM_RIGHT: &str = "╯";
const TABLE_CELL_PADDING: usize = 1;
const TABLE_CELL_PADDING_TOTAL: usize = TABLE_CELL_PADDING * 2;
const TABLE_MIN_COL_WIDTH: usize = 3;

const LIST_INDENT_PER_DEPTH: usize = 2;
const LIST_BULLETS: [&str; 4] = ["•", "◦", "▸", "▹"];
const LIST_MIN_AVAILABLE_WIDTH: usize = 10;

const INLINE_CODE_PADDING: usize = 1;

#[derive(Debug)]

pub struct RenderResult {
    pub lines: Vec<Line<'static>>,
    pub image_positions: Vec<(usize, String, String)>,
    pub node_line_starts: Vec<usize>,
}

pub(super) fn byte_index_for_width(text: &str, max_width: usize) -> usize {
    let mut width = 0usize;
    let mut last_end = 0usize;

    for (idx, ch) in text.char_indices() {
        let ch_width = crate::width::char_width(ch);
        let ch_end = idx + ch.len_utf8();

        if last_end > 0 && width + ch_width > max_width {
            break;
        }

        if last_end == 0 && ch_width > max_width {
            return ch_end;
        }

        width += ch_width;
        last_end = ch_end;
    }

    if last_end == 0 {
        text.len()
    } else {
        last_end
    }
}

#[inline]
pub(super) fn display_width(text: &str) -> usize {
    crate::width::str_width(text)
}

pub use measure::{compute_line_starts, measure_nodes};
pub use search::apply_search_highlight;

pub fn render_viewport(
    nodes: &[DocNode],
    node_line_starts: &[usize],
    viewport_first: usize,
    viewport_last: usize,
    width: u16,
    full_width: u16,
) -> RenderResult {
    if nodes.is_empty() || viewport_first >= viewport_last {
        return RenderResult {
            lines: Vec::new(),
            image_positions: Vec::new(),
            node_line_starts: Vec::new(),
        };
    }

    // binary-search the first node whose end line > viewport_first
    let first = match node_line_starts.binary_search(&viewport_first) {
        Ok(i) => i,
        Err(0) => 0,
        Err(i) => i - 1,
    };

    // linear scan from first to find the closing node (usually small range)
    let last = {
        let mut i = first;
        while i < nodes.len() {
            let node_end = node_line_starts[i] + measure::measure_node_height(&nodes[i], width);
            if node_end >= viewport_last {
                i += 1;
                break;
            }
            i += 1;
        }
        i.min(nodes.len())
    };

    if first >= last || first >= nodes.len() {
        return RenderResult {
            lines: Vec::new(),
            image_positions: Vec::new(),
            node_line_starts: Vec::new(),
        };
    }

    let mut r = render_nodes(&nodes[first..last], width, full_width);

    // Adjust node_line_starts to be relative to the full document,
    // then slice lines to the viewport.
    let base = node_line_starts[first];
    r.node_line_starts = r.node_line_starts.iter().map(|&s| s + base).collect();
    r.image_positions = r
        .image_positions
        .iter()
        .map(|(line, src, alt)| (line + base, src.clone(), alt.clone()))
        .collect();

    let start_idx = viewport_first.saturating_sub(base);
    if start_idx < r.lines.len() {
        r.lines.drain(0..start_idx);
        let keep_len = viewport_last.saturating_sub(viewport_first);
        r.lines.truncate(keep_len);
    } else {
        r.lines.clear();
    }
    r
}

/// Render a list of DocNodes into a flat list of ratatui Lines.
///
/// `width` is the content column width for text wrapping.
pub fn render_nodes(nodes: &[DocNode], width: u16, full_width: u16) -> RenderResult {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut image_positions: Vec<(usize, String, String)> = Vec::new();
    let mut node_line_starts: Vec<usize> = Vec::with_capacity(nodes.len());
    let w = width as usize;
    let full_w = full_width as usize;

    for node in nodes {
        node_line_starts.push(lines.len());
        match node {
            DocNode::Heading { level, text } => {
                let prefix = inline::heading_prefix(*level);
                let style = Theme::heading(*level);
                let spans = vec![
                    Span::styled(prefix, style),
                    Span::styled(" ", style),
                    Span::styled(text.clone(), style),
                ];
                let wrapped = inline::soft_wrap_spans(spans, w);
                lines.extend(wrapped);
            }

            DocNode::Paragraph(spans) => {
                let rendered = inline::render_inline_spans(spans);
                let wrapped = inline::soft_wrap_spans(rendered, w);
                lines.extend(wrapped);
            }

            DocNode::CodeBlock { language, code } => {
                code::render_code_block(&mut lines, language.as_deref(), code, w);
            }

            DocNode::BlockQuote(children) => {
                let left_border_width = display_width(BLOCKQUOTE_LEFT_BORDER);
                let right_border_width = display_width(BLOCKQUOTE_RIGHT_BORDER);
                let inner_width = (w as u16)
                    .saturating_sub((left_border_width + right_border_width) as u16)
                    .max(1);
                let nested: RenderResult = render_nodes(children, inner_width, inner_width);
                let start_idx = lines.len();

                let border_style = Theme::blockquote_bar();

                if !nested.lines.is_empty() {
                    let top_bar_width = w.saturating_sub(
                        display_width(BLOCKQUOTE_TOP_LEFT) + display_width(BLOCKQUOTE_TOP_RIGHT),
                    );
                    let mut top_line = String::with_capacity(w);
                    top_line.push_str(BLOCKQUOTE_TOP_LEFT);
                    for _ in 0..top_bar_width {
                        top_line.push_str(BLOCKQUOTE_HORIZONTAL);
                    }
                    top_line.push_str(BLOCKQUOTE_TOP_RIGHT);
                    lines.push(Line::from(Span::styled(top_line, border_style)));
                }

                for line in nested.lines {
                    let content_width: usize =
                        line.spans.iter().map(|s| display_width(&s.content)).sum();
                    let padding = (inner_width as usize).saturating_sub(content_width);
                    let mut quoted_spans = Vec::with_capacity(line.spans.len() + 3);
                    quoted_spans.push(Span::styled(BLOCKQUOTE_LEFT_BORDER, border_style));
                    quoted_spans.extend(line.spans);
                    if padding > 0 {
                        quoted_spans.push(Span::styled(" ".repeat(padding), Style::default()));
                    }
                    quoted_spans.push(Span::styled(BLOCKQUOTE_RIGHT_BORDER, border_style));
                    lines.push(Line::from(quoted_spans));
                }

                if start_idx != lines.len() && start_idx + 1 < lines.len() {
                    let bot_bar_width = w.saturating_sub(
                        display_width(BLOCKQUOTE_BOTTOM_LEFT)
                            + display_width(BLOCKQUOTE_BOTTOM_RIGHT),
                    );
                    let mut bot_line = String::with_capacity(w);
                    bot_line.push_str(BLOCKQUOTE_BOTTOM_LEFT);
                    for _ in 0..bot_bar_width {
                        bot_line.push_str(BLOCKQUOTE_HORIZONTAL);
                    }
                    bot_line.push_str(BLOCKQUOTE_BOTTOM_RIGHT);
                    lines.push(Line::from(Span::styled(bot_line, border_style)));
                }

                // Nested content starts after the top border (if present).
                let content_offset =
                    if start_idx < lines.len() && !nested.image_positions.is_empty() {
                        1
                    } else {
                        0
                    };
                image_positions.extend(
                    nested
                        .image_positions
                        .into_iter()
                        .map(|(line_idx, src, alt)| {
                            (start_idx + content_offset + line_idx, src, alt)
                        }),
                );
            }

            DocNode::ListItem {
                depth,
                ordered,
                number,
                children,
            } => {
                let indent = " ".repeat(LIST_INDENT_PER_DEPTH * depth);
                let bullet = if *ordered {
                    format!("{}{}. ", indent, number.unwrap_or(1))
                } else {
                    format!("{}{} ", indent, LIST_BULLETS[depth % LIST_BULLETS.len()])
                };
                let bullet_style = Theme::bullet(*depth);
                let bullet_width = display_width(&bullet);
                let continuation_indent = " ".repeat(bullet_width);
                let avail = w.saturating_sub(bullet_width).max(LIST_MIN_AVAILABLE_WIDTH);

                let rendered = inline::render_inline_spans(children);
                let wrapped = inline::soft_wrap_spans(rendered, avail);
                for (i, wl) in wrapped.into_iter().enumerate() {
                    let mut new_spans = if i == 0 {
                        vec![Span::styled(bullet.clone(), bullet_style)]
                    } else {
                        vec![Span::raw(continuation_indent.clone())]
                    };
                    new_spans.extend(wl.spans);
                    lines.push(Line::from(new_spans));
                }
            }

            DocNode::Table { headers, rows } => {
                table::render_table(&mut lines, headers, rows, w);
            }

            DocNode::Rule => {
                let rule = RULE_CHAR.repeat(full_w);
                lines.push(Line::from(Span::styled(rule, Theme::rule())));
            }

            DocNode::Image { src, alt } => {
                // Reserve fixed vertical space so scrolling and following content stay aligned.
                let line_idx = lines.len();
                image_positions.push((line_idx, src.clone(), alt.clone()));
                let placeholder = format!(
                    "[image: {}]",
                    if alt.is_empty() {
                        src.as_str()
                    } else {
                        alt.as_str()
                    }
                );
                lines.push(Line::from(Span::styled(placeholder, Theme::subtext())));
                lines.extend((1..IMAGE_RENDER_HEIGHT).map(|_| Line::default()));
            }

            DocNode::Blank => {
                lines.push(Line::default());
            }
        }
    }

    RenderResult {
        lines,
        image_positions,
        node_line_starts,
    }
}
