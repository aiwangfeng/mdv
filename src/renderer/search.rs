//! Search highlighting: apply search result highlights to rendered lines.

use ratatui::text::{Line, Span};
use unicode_normalization::UnicodeNormalization;

use crate::theme::Theme;

fn align_strings(src: &str, dst: &str) -> Vec<usize> {
    if src == dst {
        return (0..=dst.len()).collect();
    }

    let src_chars: Vec<(usize, char)> = src.char_indices().collect();
    let dst_chars: Vec<(usize, char)> = dst.char_indices().collect();

    let n = src_chars.len();
    let m = dst_chars.len();

    if n == m {
        let mut byte_map = vec![0; dst.len() + 1];
        for char_idx in 0..m {
            let byte_offset = dst_chars[char_idx].0;
            let src_byte_offset = src_chars[char_idx].0;
            let next_dst_byte = if char_idx + 1 < m {
                dst_chars[char_idx + 1].0
            } else {
                dst.len()
            };
            byte_map[byte_offset..next_dst_byte].fill(src_byte_offset);
        }
        byte_map[dst.len()] = src.len();
        return byte_map;
    }

    let mut dp = vec![vec![0; m + 1]; n + 1];
    for (i, row) in dp.iter_mut().enumerate() {
        row[0] = i;
    }
    for (j, val) in dp[0].iter_mut().enumerate() {
        *val = j;
    }

    for i in 1..=n {
        for j in 1..=m {
            let c_src = src_chars[i - 1].1;
            let c_dst = dst_chars[j - 1].1;
            let cost = if c_src == c_dst { 0 } else { 1 };
            dp[i][j] = (dp[i - 1][j - 1] + cost)
                .min(dp[i - 1][j] + 1)
                .min(dp[i][j - 1] + 1);
        }
    }

    let mut dst_to_src_char = vec![0; m];
    let mut i = n;
    let mut j = m;
    while j > 0 {
        if i == 0 {
            dst_to_src_char[j - 1] = 0;
            j -= 1;
        } else {
            let c_src = src_chars[i - 1].1;
            let c_dst = dst_chars[j - 1].1;
            let cost = if c_src == c_dst { 0 } else { 1 };

            if dp[i][j] == dp[i - 1][j - 1] + cost {
                dst_to_src_char[j - 1] = i - 1;
                i -= 1;
                j -= 1;
            } else if dp[i][j] == dp[i - 1][j] + 1 {
                i -= 1;
            } else {
                dst_to_src_char[j - 1] = i.saturating_sub(1);
                j -= 1;
            }
        }
    }

    let mut byte_map = vec![0; dst.len() + 1];
    for char_idx in 0..m {
        let byte_offset = dst_chars[char_idx].0;
        let src_char_idx = dst_to_src_char[char_idx];
        let src_byte_offset = src_chars[src_char_idx].0;

        let next_dst_byte = if char_idx + 1 < m {
            dst_chars[char_idx + 1].0
        } else {
            dst.len()
        };
        byte_map[byte_offset..next_dst_byte].fill(src_byte_offset);
    }
    byte_map[dst.len()] = src.len();
    byte_map
}

/// Find all match ranges of `query_norm` in `line_text`, mapped back to original byte offsets.
fn find_mapped_matches(
    full: &str,
    line_text: &str,
    query_norm: &str,
) -> Vec<(usize, usize, usize, usize)> {
    // Build src_lower and src_map: lowercased chars → original byte offsets
    let mut src_lower = String::new();
    let mut src_map = Vec::new();
    for (byte_offset, c) in full.char_indices() {
        for lower_c in c.to_lowercase() {
            let len = lower_c.len_utf8();
            src_lower.push(lower_c);
            for _ in 0..len {
                src_map.push(byte_offset);
            }
        }
    }
    src_map.push(full.len());

    // Align line_text (lowercased/NFC normalized) with src_lower
    let byte_map = align_strings(&src_lower, line_text);

    // Find all match ranges in line_text, then map to original byte offsets
    let mut matches = Vec::new();
    let mut offset = 0;
    while let Some(idx) = line_text[offset..].find(query_norm) {
        let abs_start = offset + idx;
        let abs_end = abs_start + query_norm.len();
        matches.push((abs_start, abs_end));
        offset = abs_start + query_norm.len().max(1);
    }

    let mut mapped = Vec::new();
    for &(abs_start, abs_end) in &matches {
        let src_lower_start = byte_map[abs_start];
        let src_lower_end = byte_map[abs_end];
        let m_start = src_map[src_lower_start];
        let m_end = src_map[src_lower_end];
        if m_start < m_end {
            mapped.push((abs_start, abs_end, m_start, m_end));
        }
    }
    mapped
}

/// Rebuild a line's spans, applying highlight styles to matched regions.
fn rebuild_spans_with_highlights(
    spans: Vec<Span<'static>>,
    mapped_matches: &[(usize, usize, usize, usize)],
    line_idx: usize,
    current_match: Option<crate::app::SearchMatch>,
) -> Vec<Span<'static>> {
    let mut new_spans: Vec<Span<'static>> = Vec::new();
    let mut current_byte_offset = 0;

    for span in spans {
        let span_text = span.content.to_string();
        let span_len = span_text.len();
        let span_end = current_byte_offset + span_len;

        let mut last_processed = 0;

        for &(abs_start, abs_end, m_start, m_end) in mapped_matches {
            // Intersection of [current_byte_offset, span_end] and [m_start, m_end]
            let intersect_start = m_start.max(current_byte_offset);
            let intersect_end = m_end.min(span_end);

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
                    m.line_idx == line_idx && m.start_byte == abs_start && m.end_byte == abs_end
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

    new_spans
}

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
    let query_norm: String = if has_upper {
        query.nfc().collect()
    } else {
        query.to_lowercase().nfc().collect()
    };

    lines
        .into_iter()
        .enumerate()
        .map(|(i, line)| {
            let line_idx = start_idx + i;

            let full: String = line.spans.iter().map(|s| s.content.as_ref()).collect();

            let line_text: String = if let Some(texts) = lowercased_texts {
                texts
                    .get(text_offset + i)
                    .map(|s| s.to_string())
                    .unwrap_or_default()
            } else if has_upper {
                full.clone()
            } else {
                full.to_lowercase()
            };

            if !line_text.contains(&query_norm) {
                return line;
            }

            let mapped_matches = find_mapped_matches(&full, &line_text, &query_norm);
            let new_spans =
                rebuild_spans_with_highlights(line.spans, &mapped_matches, line_idx, current_match);
            Line::from(new_spans)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use ratatui::style::Style;
    use ratatui::text::Span;

    #[test]
    fn test_empty_query() {
        let line = Line::from(vec![Span::raw("Hello World")]);
        let highlighted = apply_search_highlight(vec![line.clone()], "", None, 0, None, 0);
        assert_eq!(highlighted[0], line);
    }

    #[test]
    fn test_no_matches() {
        let line = Line::from(vec![Span::raw("Hello World")]);
        let highlighted = apply_search_highlight(vec![line.clone()], "XYZ", None, 0, None, 0);
        assert_eq!(highlighted[0], line);
    }

    #[test]
    fn test_basic_match() {
        let line = Line::from(vec![Span::raw("Hello World")]);
        let highlighted = apply_search_highlight(vec![line], "World", None, 0, None, 0);
        let spans = &highlighted[0].spans;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "Hello ");
        assert_eq!(spans[1].content, "World");
        assert_eq!(spans[1].style, Theme::search_match());
    }

    #[test]
    fn test_case_insensitive_match() {
        let line = Line::from(vec![Span::raw("Hello World")]);
        let highlighted = apply_search_highlight(vec![line], "world", None, 0, None, 0);
        let spans = &highlighted[0].spans;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "Hello ");
        assert_eq!(spans[1].content, "World");
    }

    #[test]
    fn test_multiple_matches() {
        let line = Line::from(vec![Span::raw("aba")]);
        let highlighted = apply_search_highlight(vec![line], "a", None, 0, None, 0);
        let spans = &highlighted[0].spans;
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "a");
        assert_eq!(spans[0].style, Theme::search_match());
        assert_eq!(spans[1].content, "b");
        assert_eq!(spans[1].style, Style::default());
        assert_eq!(spans[2].content, "a");
        assert_eq!(spans[2].style, Theme::search_match());
    }

    #[test]
    fn test_kelvin_panic_prevention() {
        // Kelvin sign "K" (U+212A) is 3 bytes. Lowercases to ASCII "k" (1 byte).
        let line = Line::from(vec![Span::raw("AKB")]);
        let byte_map = align_strings("a k b", "a k b");
        println!("byte_map: {:?}", byte_map);
        // Search query "k" (case-insensitive) should match "K".
        let highlighted = apply_search_highlight(vec![line], "k", None, 0, None, 0);
        let spans = &highlighted[0].spans;
        println!("spans: {:?}", spans);
        // Should split "AKB" into "A", "K", "B".
        assert_eq!(spans.len(), 3);
        assert_eq!(spans[0].content, "A");
        assert_eq!(spans[1].content, "K");
        assert_eq!(spans[1].style, Theme::search_match());
        assert_eq!(spans[2].content, "B");
    }

    #[test]
    fn test_cjk_search_highlight() {
        // CJK characters are 3 bytes each.
        let line = Line::from(vec![Span::raw("车规")]);
        // Search query "规"
        let highlighted = apply_search_highlight(vec![line], "规", None, 0, None, 0);
        let spans = &highlighted[0].spans;
        assert_eq!(spans.len(), 2);
        assert_eq!(spans[0].content, "车");
        assert_eq!(spans[1].content, "规");
        assert_eq!(spans[1].style, Theme::search_match());
    }
}
