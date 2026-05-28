//! Search highlighting: apply search result highlights to rendered lines.

use ratatui::text::{Line, Span};

use crate::theme::Theme;

pub fn apply_search_highlight(
    lines: Vec<Line<'static>>,
    query: &str,
    current_match: Option<crate::app::SearchMatch>,
    start_idx: usize,
    lowercased_texts: Option<&[&str]>,
    text_offset: usize,
) -> Vec<Line<'static>> {
    if query.is_empty() {
        return lines;
    }

    let has_upper = query.chars().any(|c| c.is_uppercase());
    let query_norm = if has_upper {
        query.to_string()
    } else {
        query.to_lowercase()
    };

    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let line_idx = start_idx + i;

            let line_text: String = if let Some(texts) = lowercased_texts {
                texts[text_offset + i].to_string()
            } else {
                let full: String = line.spans.iter().map(|s| s.content.as_ref()).collect();
                if has_upper {
                    full
                } else {
                    full.to_lowercase()
                }
            };

            if !line_text.contains(&query_norm) {
                return line;
            }

            // Find all match ranges in this line
            let mut matches = Vec::new();
            let mut offset = 0;
            while let Some(idx) = line_text[offset..].find(&query_norm) {
                let abs_idx = offset + idx;
                matches.push((abs_idx, abs_idx + query_norm.len()));
                offset = abs_idx + query_norm.len().max(1);
            }

            // Re-build spans with highlights
            let mut new_spans: Vec<Span<'static>> = Vec::new();
            let mut current_byte_offset = 0;

            for span in line.spans {
                let span_text = span.content.to_string();
                let span_len = span_text.len();
                let span_end = current_byte_offset + span_len;

                let mut last_processed = 0;

                for (m_start, m_end) in &matches {
                    // Intersection of [current_byte_offset, span_end] and [m_start, m_end]
                    let intersect_start = (*m_start).max(current_byte_offset);
                    let intersect_end = (*m_end).min(span_end);

                    if intersect_start < intersect_end {
                        // There's a match segment in this span
                        let rel_start = intersect_start - current_byte_offset;
                        let rel_end = intersect_end - current_byte_offset;

                        if rel_start > last_processed {
                            new_spans.push(Span::styled(
                                span_text[last_processed..rel_start].to_string(),
                                span.style,
                            ));
                        }

                        let is_current = current_match.is_some_and(|m| {
                            m.line_idx == line_idx
                                && m.start_byte == *m_start
                                && m.end_byte == *m_end
                        });

                        let hl_style = if is_current {
                            Theme::search_current()
                        } else {
                            Theme::search_match()
                        };

                        new_spans.push(Span::styled(
                            span_text[rel_start..rel_end].to_string(),
                            hl_style,
                        ));
                        last_processed = rel_end;
                    }
                }

                if last_processed < span_len {
                    new_spans.push(Span::styled(
                        span_text[last_processed..].to_string(),
                        span.style,
                    ));
                }

                current_byte_offset = span_end;
            }

            Line::from(new_spans)
        })
        .collect()
}
