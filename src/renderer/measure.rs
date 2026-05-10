//! Measuring: compute line heights for layout calculations.

use crate::parser::{DocNode, InlineSpan};
use unicode_width::UnicodeWidthStr;
use super::{display_width, BLOCKQUOTE_LEFT_BORDER, BLOCKQUOTE_RIGHT_BORDER, IMAGE_RENDER_HEIGHT, LIST_INDENT_PER_DEPTH, LIST_MIN_AVAILABLE_WIDTH, INLINE_CODE_PADDING};

pub fn measure_nodes(nodes: &[DocNode], width: u16) -> Vec<usize> {
    nodes.iter().map(|n| measure_node_height(n, width)).collect()
}

/// Compute node_line_starts from node_heights via prefix sum.
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
        DocNode::Heading { .. } => 1,
        DocNode::Paragraph(spans) => count_wrapped_inline_lines(spans, w),
        DocNode::CodeBlock { code, .. } => {
            // borders (2) + language label (1) + content lines
            let code_lines = code.lines().count();
            code_lines + 3
        }
        DocNode::BlockQuote(children) => {
            // estimate inner width for children
            let left_border = display_width(BLOCKQUOTE_LEFT_BORDER);
            let right_border = display_width(BLOCKQUOTE_RIGHT_BORDER);
            let inner = w.saturating_sub(left_border + right_border).max(1) as u16;
            let child_lines: usize = children.iter().map(|c| measure_node_height(c, inner)).sum();
            if child_lines > 0 { child_lines + 2 } else { 0 } // top + bottom borders
        }
        DocNode::ListItem { children, .. } => {
            // at least 1 line (bullet), more if wrapping
            let bullet_w = LIST_INDENT_PER_DEPTH + 2; // approx
            let avail = w.saturating_sub(bullet_w).max(LIST_MIN_AVAILABLE_WIDTH);
            count_wrapped_inline_lines(children, avail).max(1)
        }
        DocNode::Table { rows, .. } => {
            // top border + header + separator + data rows + bottom border
            2 + 1 + rows.len() + 1
        }
        DocNode::Rule | DocNode::Blank => 1,
        DocNode::Image { .. } => IMAGE_RENDER_HEIGHT,
    }
}

/// Count how many terminal lines a sequence of inline spans would produce after
/// soft-wrapping at `max_width`.  Uses a lightweight text-width simulation
/// without creating styled ratatui spans.
pub(super) fn count_wrapped_inline_lines(spans: &[InlineSpan], max_width: usize) -> usize {
    if max_width == 0 || spans.is_empty() {
        return 1;
    }

    // Flatten spans into text segments, simulating how render_inline_spans and
    // soft_wrap_spans process them.
    let mut lines = 1usize;
    let mut current_w = 0usize;
    let extra_pad = 2 * INLINE_CODE_PADDING;

    for span in spans {
        let (text, extra) = match span {
            InlineSpan::Text(t)
            | InlineSpan::Bold(t)
            | InlineSpan::Italic(t)
            | InlineSpan::BoldItalic(t)
            | InlineSpan::Strikethrough(t) => (t.as_str(), 0),
            InlineSpan::Code(t) => (t.as_str(), extra_pad),
            InlineSpan::Link { text, .. } => (text.as_str(), 0),
            InlineSpan::Image { alt, .. } => (alt.as_str(), 0),
            InlineSpan::SoftBreak => {
                lines += 1;
                current_w = 0;
                continue;
            }
            InlineSpan::HardBreak => {
                lines += 1;
                current_w = 0;
                continue;
            }
        };

        if text.is_empty() && extra == 0 {
            continue;
        }

        // Soft_wrap splits on '\n' first, then on spaces within each segment
        for segment in text.split('\n') {
            if segment.is_empty() && current_w > 0 {
                lines += 1;
                current_w = 0;
                continue;
            }
            // For the inline code padding, add before the start
            let seg_w = UnicodeWidthStr::width(segment) + extra;

            // We simulate split_inclusive(' ') wrapping:
            // If the segment fits on the current line, add it.
            if current_w + seg_w <= max_width {
                current_w += seg_w;
            } else if seg_w <= max_width {
                // Start new line
                lines += 1;
                current_w = seg_w;
            } else {
                // Segment itself is wider than max_width; force-break needed
                let forced = (seg_w + max_width - 1) / max_width;
                lines += forced - 1; // only extra lines beyond first
                current_w = seg_w % max_width;
            }
        }
    }

    lines.max(1)
}
