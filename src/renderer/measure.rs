//! Measuring: compute line heights for layout calculations.

use super::{
    display_width, BLOCKQUOTE_LEFT_BORDER, BLOCKQUOTE_RIGHT_BORDER, IMAGE_RENDER_HEIGHT,
    LIST_INDENT_PER_DEPTH, LIST_MIN_AVAILABLE_WIDTH,
};
use crate::parser::{DocNode, InlineSpan};

pub fn measure_nodes(nodes: &[DocNode], width: u16) -> Vec<usize> {
    nodes
        .iter()
        .map(|n| measure_node_height(n, width))
        .collect()
}

/// Compute `node_line_starts` from `node_heights` via prefix sum.
pub fn compute_line_starts(heights: &[usize]) -> Vec<usize> {
    let mut starts = Vec::with_capacity(heights.len());
    let mut offset = 0;
    for &h in heights {
        starts.push(offset);
        offset += h;
    }
    starts
}

pub(super) fn measure_node_height(node: &DocNode, width: u16) -> usize {
    let w = width as usize;
    match node {
        DocNode::Heading { level, text } => {
            let prefix = super::inline::heading_prefix(*level);
            let spans = vec![ratatui::text::Span::raw(format!("{prefix} {text}"))];
            super::inline::soft_wrap_spans(spans, w).len()
        }
        DocNode::Paragraph(spans) => count_wrapped_inline_lines(spans, w),
        DocNode::CodeBlock { code, .. } => {
            // max_content_width is width minus the borders ("│ " and "│")
            let left_border = display_width(super::CODE_BLOCK_LEFT_BORDER);
            let right_border = display_width(super::CODE_BLOCK_RIGHT_BORDER);
            let max_content_width = w.saturating_sub(left_border + right_border).max(1);

            let expanded_code_storage;
            let code = if code.contains(['\t', '\r']) {
                expanded_code_storage = super::code::expand_tabs(code, 4);
                &expanded_code_storage
            } else {
                code
            };

            let mut line_count: usize = 0;
            for line in code.split('\n') {
                let mut current_width = 0;
                for c in line.chars() {
                    let cw = crate::width::char_width(c);
                    if current_width > 0 && current_width + cw > max_content_width {
                        line_count += 1;
                        current_width = 0;
                    }
                    current_width += cw;
                }
                line_count += 1;
            }
            if code.ends_with('\n') {
                line_count = line_count.saturating_sub(1);
            }
            if code.is_empty() {
                line_count = 0;
            }

            line_count + 2
        }
        DocNode::BlockQuote(children) => {
            // estimate inner width for children
            let left_border = display_width(BLOCKQUOTE_LEFT_BORDER);
            let right_border = display_width(BLOCKQUOTE_RIGHT_BORDER);
            let inner = width
                .saturating_sub((left_border + right_border) as u16)
                .max(1);
            let child_lines: usize = children.iter().map(|c| measure_node_height(c, inner)).sum();
            if child_lines > 0 {
                child_lines + 2
            } else {
                0
            } // top + bottom borders
        }
        DocNode::ListItem {
            depth,
            ordered,
            number,
            children,
        } => {
            let indent_len = LIST_INDENT_PER_DEPTH * depth;
            let bullet_w = if *ordered {
                let num_len = number.unwrap_or(1).to_string().len();
                indent_len + num_len + 2
            } else {
                let bullet_char = super::LIST_BULLETS[depth % super::LIST_BULLETS.len()];
                indent_len + display_width(bullet_char) + 1
            };
            let avail = w.saturating_sub(bullet_w).max(LIST_MIN_AVAILABLE_WIDTH);
            count_wrapped_inline_lines(children, avail).max(1)
        }
        DocNode::Table { headers, rows } => {
            if headers.is_empty() {
                0
            } else {
                // top border + header + separator + data rows + bottom border
                2 + 1 + rows.len() + 1
            }
        }
        DocNode::Rule | DocNode::Blank => 1,
        DocNode::Image { .. } => IMAGE_RENDER_HEIGHT,
    }
}

/// Count how many terminal lines a sequence of inline spans produces after
/// soft-wrapping at `max_width`.
pub(super) fn count_wrapped_inline_lines(spans: &[InlineSpan], max_width: usize) -> usize {
    if max_width == 0 || spans.is_empty() {
        return 1;
    }
    let rendered = super::inline::render_inline_spans(spans);
    super::inline::soft_wrap_spans(rendered, max_width)
        .len()
        .max(1)
}
