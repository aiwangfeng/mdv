//! Search highlighting: apply search result highlights to rendered lines.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use crate::theme::Theme;

pub fn apply_search_highlight(
    lines: Vec<Line<'static>>,
    query: &str,
    current_match_line: Option<usize>,
    start_idx: usize,
    lowercased_texts: Option<&[&str]>,
) -> Vec<Line<'static>> {
    if query.is_empty() {
        return lines;
    }
    let query_lower = query.to_lowercase();

    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let line_idx = start_idx + i;

            let has_match = if let Some(texts) = lowercased_texts {
                texts[i].contains(&query_lower)
            } else {
                let full_text: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                full_text.to_lowercase().contains(&query_lower)
            };

            if !has_match {
                return line;
            }

            // This line has a match
            let is_current = current_match_line == Some(line_idx);

            // Re-build spans with highlights
            let hl_style = if is_current {
                Theme::search_current()
            } else {
                Theme::search_match()
            };
            let mut new_spans: Vec<Span<'static>> = Vec::new();

            for span in line.spans {
                highlight_span(&mut new_spans, span, query, &query_lower, hl_style);
            }

            Line::from(new_spans)
        })
        .collect()
}

fn highlight_span(
    new_spans: &mut Vec<Span<'static>>,
    span: Span<'static>,
    query: &str,
    query_lower: &str,
    hl_style: Style,
) {
    let text = span.content.to_string();
    let text_lower = text.to_lowercase();
    let base_style = span.style;
    let query_len = query.chars().count();
    let mut offset = 0usize;

    while let Some(idx) = text_lower[offset..].find(query_lower) {
        let abs_idx = offset + idx;
        if offset < abs_idx {
            new_spans.push(Span::styled(text[offset..abs_idx].to_string(), base_style));
        }
        let end = nth_char_boundary(&text[abs_idx..], query_len);
        new_spans.push(Span::styled(
            text[abs_idx..abs_idx + end].to_string(),
            hl_style,
        ));
        offset = abs_idx + end;
    }

    if offset < text.len() {
        new_spans.push(Span::styled(text[offset..].to_string(), base_style));
    }
}

fn nth_char_boundary(s: &str, char_count: usize) -> usize {
    s.char_indices()
        .nth(char_count)
        .map(|(idx, _)| idx)
        .unwrap_or(s.len())
}
