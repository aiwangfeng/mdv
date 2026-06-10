//! Measuring: compute line heights for layout calculations.

use super::{
    display_width, BLOCKQUOTE_LEFT_BORDER, BLOCKQUOTE_RIGHT_BORDER, IMAGE_RENDER_HEIGHT,
    INLINE_CODE_PADDING, LIST_INDENT_PER_DEPTH, LIST_MIN_AVAILABLE_WIDTH,
};
use crate::parser::{DocNode, InlineSpan};

pub fn measure_nodes(nodes: &[DocNode], width: u16) -> Vec<usize> {
    nodes
        .iter()
        .map(|n| measure_node_height(n, width))
        .collect()
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
        DocNode::Heading { level, text } => {
            let prefix = super::inline::heading_prefix(*level);
            let heading_spans = vec![InlineSpan::Text(format!("{} {}", prefix, text))];
            count_wrapped_inline_lines(&heading_spans, w)
        }
        DocNode::Paragraph(spans) => count_wrapped_inline_lines(spans, w),
        DocNode::CodeBlock { code, .. } => {
            // max_content_width is width minus the borders ("│ " and " │")
            let left_border = display_width(super::CODE_BLOCK_LEFT_BORDER);
            let right_border = display_width(super::CODE_BLOCK_RIGHT_BORDER);
            let max_content_width = w.saturating_sub(left_border + right_border);

            let mut line_count: usize = 0;
            for line in code.split('\n') {
                // If it's the last empty split (due to trailing newline), skip if code ended with \n
                let mut current_width = 0;
                for c in line.chars() {
                    let cw = crate::width::char_width(c);
                    if current_width + cw > max_content_width {
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
            let inner = w.saturating_sub(left_border + right_border).max(1) as u16;
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
                indent_len + 2
            };
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

fn split_width(text: &str, max_width: usize) -> (usize, &str) {
    let split_at = super::byte_index_for_width(text, max_width);
    let split_text = &text[..split_at];
    let remaining = &text[split_at..];
    (display_width(split_text), remaining)
}

fn simulate_push_word_with_extra(
    lines: &mut usize,
    current_width: &mut usize,
    word: &str,
    extra: usize,
    max_width: usize,
) {
    let mut remaining = word;
    let mut extra_to_apply = extra;

    while !remaining.is_empty() || extra_to_apply > 0 {
        // Trim leading spaces at line start to match push_wrapped_chunk behavior.
        if *current_width == 0 && extra_to_apply == 0 {
            remaining = remaining.trim_start_matches(' ');
            if remaining.is_empty() {
                break;
            }
        }

        let word_w = display_width(remaining) + extra_to_apply;
        if *current_width > 0 && *current_width + word_w > max_width {
            *lines += 1;
            *current_width = 0;
            continue;
        }

        if word_w <= max_width {
            *current_width += word_w;
            break;
        }

        // Must split.
        let (_, next_remaining) = if extra_to_apply > 0 {
            if extra_to_apply >= max_width {
                extra_to_apply -= max_width;
                (max_width, remaining)
            } else {
                let avail = max_width - extra_to_apply;
                let (w_width, rem) = split_width(remaining, avail);
                let total_w = extra_to_apply + w_width;
                extra_to_apply = 0;
                (total_w, rem)
            }
        } else {
            split_width(remaining, max_width)
        };

        *lines += 1;
        *current_width = 0;
        remaining = next_remaining;
    }
}

/// Count how many terminal lines a sequence of inline spans would produce after
/// soft-wrapping at `max_width`.  Uses a lightweight text-width simulation
/// without creating styled ratatui spans.
pub(super) fn count_wrapped_inline_lines(spans: &[InlineSpan], max_width: usize) -> usize {
    if max_width == 0 {
        return 1;
    }
    if spans.is_empty() {
        return 1;
    }

    let mut lines = 1usize;
    let mut current_width = 0usize;
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
                simulate_push_word_with_extra(&mut lines, &mut current_width, " ", 0, max_width);
                continue;
            }
            InlineSpan::HardBreak => {
                lines += 1;
                current_width = 0;
                continue;
            }
        };

        if text.is_empty() && extra == 0 {
            continue;
        }

        let mut segments = text.split('\n').peekable();
        while let Some(segment) = segments.next() {
            if segment.is_empty() && current_width == 0 {
                lines += 1;
                continue;
            }

            let words: Vec<&str> = segment.split_inclusive(' ').collect();
            let words_len = words.len();
            for (idx, word) in words.into_iter().enumerate() {
                let mut word_extra = 0;
                if extra > 0 {
                    if idx == 0 {
                        word_extra += INLINE_CODE_PADDING;
                    }
                    if idx + 1 == words_len {
                        word_extra += INLINE_CODE_PADDING;
                    }
                }
                simulate_push_word_with_extra(
                    &mut lines,
                    &mut current_width,
                    word,
                    word_extra,
                    max_width,
                );
            }

            if segments.peek().is_some() {
                lines += 1;
                current_width = 0;
            }
        }
    }

    lines.max(1)
}
