//! Inline span rendering: heading prefixes, inline spans, soft wrapping.

use ratatui::{
    style::Style,
    text::{Line, Span},
};
use unicode_width::UnicodeWidthChar;

use super::{display_width, INLINE_CODE_PADDING};
use crate::parser::InlineSpan;
use crate::theme::Theme;

pub(super) fn heading_prefix(level: u8) -> &'static str {
    match level {
        1 => "#",
        2 => "##",
        3 => "###",
        4 => "####",
        5 => "#####",
        6 => "######",
        _ => "######",
    }
}

// ---------------------------------------------------------------------------
// Inline span rendering
// ---------------------------------------------------------------------------

pub(super) fn render_inline_spans(spans: &[InlineSpan]) -> Vec<Span<'static>> {
    let mut result = Vec::with_capacity(spans.len());
    for span in spans {
        match span {
            InlineSpan::Text(t) => result.push(Span::styled(t.clone(), Theme::text())),
            InlineSpan::Bold(t) => result.push(Span::styled(t.clone(), Theme::bold())),
            InlineSpan::Italic(t) => result.push(Span::styled(t.clone(), Theme::italic())),
            InlineSpan::BoldItalic(t) => result.push(Span::styled(t.clone(), Theme::bold_italic())),
            InlineSpan::Code(t) => {
                let mut s = String::with_capacity(t.len() + INLINE_CODE_PADDING * 2);
                for _ in 0..INLINE_CODE_PADDING {
                    s.push(' ');
                }
                s.push_str(t);
                for _ in 0..INLINE_CODE_PADDING {
                    s.push(' ');
                }
                result.push(Span::styled(s, Theme::inline_code()));
            }
            InlineSpan::Strikethrough(t) => {
                result.push(Span::styled(t.clone(), Theme::strikethrough()))
            }
            InlineSpan::Link { text, url } => {
                let _ = url;
                result.push(Span::styled(text.clone(), Theme::link()));
            }
            InlineSpan::Image { src, alt } => {
                let label = if alt.is_empty() { src } else { alt };
                let mut s = String::with_capacity(label.len() + 10);
                s.push_str("[image: ");
                s.push_str(label);
                s.push(']');
                result.push(Span::styled(s, Theme::subtext()));
            }
            InlineSpan::SoftBreak => result.push(Span::raw(" ")),
            InlineSpan::HardBreak => result.push(Span::raw("\n")),
        }
    }
    result
}

// ---------------------------------------------------------------------------
// Soft-wrapping: break a list of Spans into wrapped Lines given column width.
// ---------------------------------------------------------------------------

pub(super) fn soft_wrap_spans(spans: Vec<Span<'static>>, max_width: usize) -> Vec<Line<'static>> {
    if max_width == 0 {
        return vec![Line::from(spans)];
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_line: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for span in spans {
        let style = span.style;
        let mut segments = span.content.as_ref().split('\n').peekable();
        while let Some(segment) = segments.next() {
            if segment.is_empty() && current_line.is_empty() {
                lines.push(Line::default());
                continue;
            }

            for word in segment.split_inclusive(' ') {
                push_wrapped_chunk(
                    &mut lines,
                    &mut current_line,
                    &mut current_width,
                    word,
                    style,
                    max_width,
                );
            }

            if segments.peek().is_some() {
                lines.push(Line::from(std::mem::take(&mut current_line)));
                current_width = 0;
            }
        }
    }

    if !current_line.is_empty() {
        lines.push(Line::from(current_line));
    }

    if lines.is_empty() {
        lines.push(Line::default());
    }

    lines
}

// ---------------------------------------------------------------------------
// Code blocks with syntect highlighting
// ---------------------------------------------------------------------------

pub(super) fn push_wrapped_chunk(
    lines: &mut Vec<Line<'static>>,
    current_line: &mut Vec<Span<'static>>,
    current_width: &mut usize,
    text: &str,
    style: Style,
    max_width: usize,
) {
    let mut remaining = text;

    while !remaining.is_empty() {
        let remaining_width = display_width(remaining);
        if *current_width > 0 && *current_width + remaining_width > max_width {
            lines.push(Line::from(std::mem::take(current_line)));
            *current_width = 0;
            continue;
        }

        if remaining_width <= max_width {
            current_line.push(Span::styled(remaining.to_string(), style));
            *current_width += remaining_width;
            break;
        }

        let split_at = byte_index_for_width(remaining, max_width);
        current_line.push(Span::styled(remaining[..split_at].to_string(), style));
        lines.push(Line::from(std::mem::take(current_line)));
        *current_width = 0;
        remaining = &remaining[split_at..];
    }
}

pub(super) fn byte_index_for_width(text: &str, max_width: usize) -> usize {
    let mut width = 0usize;
    let mut last_end = 0usize;

    for (idx, ch) in text.char_indices() {
        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(0);
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
