// src/renderer.rs
// Converts DocNode trees into ratatui Lines for display in the content panel.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use std::collections::HashMap;
use std::sync::OnceLock;
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::parser::{DocNode, InlineSpan};
use crate::theme::Theme;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();
static HL_CACHE: OnceLock<
    std::sync::Mutex<HashMap<(String, String), Vec<(SyntectStyle, String)>>>,
> = OnceLock::new();
pub const IMAGE_RENDER_HEIGHT: usize = 10;

#[derive(Debug)]
pub struct RenderResult {
    pub lines: Vec<Line<'static>>,
    pub image_positions: Vec<(usize, String, String)>,
    pub node_line_starts: Vec<usize>,
}

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn get_hl_cache(
) -> &'static std::sync::Mutex<HashMap<(String, String), Vec<(SyntectStyle, String)>>> {
    HL_CACHE.get_or_init(|| std::sync::Mutex::new(HashMap::new()))
}

/// Render a list of DocNodes into a flat list of ratatui Lines.
/// `width` is the content column width for wrapping / rule drawing.
/// Returns (lines, image_positions): image_positions maps line-index → (src, alt).
pub fn render_nodes(nodes: &[DocNode], width: u16) -> RenderResult {
    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut image_positions: Vec<(usize, String, String)> = Vec::new();
    let mut node_line_starts: Vec<usize> = Vec::with_capacity(nodes.len());
    let w = width as usize;

    for node in nodes {
        node_line_starts.push(lines.len());
        match node {
            DocNode::Heading { level, text } => {
                let prefix = heading_prefix(*level);
                let style = Theme::heading(*level);
                lines.push(Line::from(vec![
                    Span::styled(prefix, style),
                    Span::styled(" ", style),
                    Span::styled(text.clone(), style),
                ]));
            }

            DocNode::Paragraph(spans) => {
                let rendered = render_inline_spans(spans);
                let wrapped = soft_wrap_spans(rendered, w.saturating_sub(1));
                lines.extend(wrapped);
            }

            DocNode::CodeBlock { language, code } => {
                render_code_block(&mut lines, language.as_deref(), code, w);
            }

            DocNode::BlockQuote(children) => {
                let inner_width = width.saturating_sub(2).max(1);
                let nested = render_nodes(children, inner_width);
                let start_idx = lines.len();

                for line in nested.lines {
                    let mut quoted_spans = vec![Span::styled("│ ", Theme::blockquote_bar())];
                    quoted_spans.extend(line.spans);
                    lines.push(Line::from(quoted_spans));
                }

                image_positions.extend(
                    nested
                        .image_positions
                        .into_iter()
                        .map(|(line_idx, src, alt)| (start_idx + line_idx, src, alt)),
                );
            }

            DocNode::ListItem {
                depth,
                ordered,
                number,
                children,
            } => {
                let indent = "  ".repeat(*depth);
                let bullet = if *ordered {
                    format!("{}{}. ", indent, number.unwrap_or(1))
                } else {
                    let bullets = ["•", "◦", "▸", "▹"];
                    format!("{}{} ", indent, bullets[depth % bullets.len()])
                };
                let bullet_style = Theme::bullet(*depth);
                let bullet_width = display_width(&bullet);
                let continuation_indent = " ".repeat(bullet_width);
                let avail = w.saturating_sub(bullet_width).max(10);

                let rendered = render_inline_spans(children);
                let wrapped = soft_wrap_spans(rendered, avail);
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
                render_table(&mut lines, headers, rows, w);
            }

            DocNode::Rule => {
                let rule = "─".repeat(w.saturating_sub(2));
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

// ---------------------------------------------------------------------------
// Heading prefixes
// ---------------------------------------------------------------------------

fn heading_prefix(level: u8) -> String {
    "#".repeat(level as usize)
}

// ---------------------------------------------------------------------------
// Inline span rendering
// ---------------------------------------------------------------------------

fn render_inline_spans(spans: &[InlineSpan]) -> Vec<Span<'static>> {
    let mut result = Vec::new();
    for span in spans {
        match span {
            InlineSpan::Text(t) => result.push(Span::styled(t.clone(), Theme::text())),
            InlineSpan::Bold(t) => result.push(Span::styled(t.clone(), Theme::bold())),
            InlineSpan::Italic(t) => result.push(Span::styled(t.clone(), Theme::italic())),
            InlineSpan::BoldItalic(t) => result.push(Span::styled(t.clone(), Theme::bold_italic())),
            InlineSpan::Code(t) => {
                result.push(Span::styled(format!(" {} ", t), Theme::inline_code()))
            }
            InlineSpan::Strikethrough(t) => {
                result.push(Span::styled(t.clone(), Theme::strikethrough()))
            }
            InlineSpan::Link { text, url } => {
                // Note: URL is displayed as-is but not clickable in TUI.
                // Could add optional URL display via config in the future.
                let _ = url;
                result.push(Span::styled(text.clone(), Theme::link()));
            }
            InlineSpan::Image { src, alt } => {
                let label = if alt.is_empty() { src } else { alt };
                result.push(Span::styled(
                    format!("[image: {}]", label),
                    Theme::subtext(),
                ));
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

fn soft_wrap_spans(spans: Vec<Span<'static>>, max_width: usize) -> Vec<Line<'static>> {
    if max_width == 0 {
        return vec![Line::from(spans)];
    }

    let mut lines: Vec<Line<'static>> = Vec::new();
    let mut current_line: Vec<Span<'static>> = Vec::new();
    let mut current_width = 0usize;

    for span in spans {
        let style = span.style;
        let text = span.content.to_string();
        let segments: Vec<&str> = text.split('\n').collect();
        for (segment_idx, segment) in segments.iter().enumerate() {
            if segment.is_empty() && current_line.is_empty() {
                lines.push(Line::default());
                continue;
            }

            let words: Vec<&str> = segment.split_inclusive(' ').collect();
            for word in words {
                push_wrapped_chunk(
                    &mut lines,
                    &mut current_line,
                    &mut current_width,
                    word,
                    style,
                    max_width,
                );
            }

            if segment_idx + 1 < segments.len() {
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

fn render_code_block(
    lines: &mut Vec<Line<'static>>,
    language: Option<&str>,
    code: &str,
    width: usize,
) {
    let border_style = Theme::code_block_border();
    let lang_label = language.unwrap_or("text");

    // Top border
    let top_right = "─".repeat(width.saturating_sub(display_width(lang_label) + 5));
    lines.push(Line::from(vec![
        Span::styled("╭─ ", border_style),
        Span::styled(lang_label.to_string(), Theme::code_block_lang()),
        Span::styled(format!(" {}", top_right), border_style),
    ]));

    let cache_key = (lang_label.to_string(), code.to_string());
    let cached_regions = get_hl_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.get(&cache_key).cloned());

    let regions_to_render: Vec<(SyntectStyle, String)> = if let Some(cached) = cached_regions {
        cached
    } else {
        let ss = get_syntax_set();
        let ts = get_theme_set();
        let syntax = ss
            .find_syntax_by_token(lang_label)
            .unwrap_or_else(|| ss.find_syntax_plain_text());
        let theme = &ts.themes["base16-ocean.dark"];
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut all_regions = Vec::new();
        for line_str in LinesWithEndings::from(code) {
            match highlighter.highlight_line(line_str, ss) {
                Ok(regions) => {
                    all_regions.extend(regions.into_iter().map(|(s, t)| (s, t.to_string())));
                }
                Err(_) => {
                    all_regions.push((
                        SyntectStyle::default(),
                        line_str.trim_end_matches('\n').to_string(),
                    ));
                }
            }
        }

        if let Ok(mut cache) = get_hl_cache().lock() {
            cache.insert(cache_key, all_regions.clone());
        }
        all_regions
    };

    let mut current_line_spans: Vec<Span<'static>> = vec![Span::styled("│ ", border_style)];
    for (style, text) in regions_to_render {
        if text.ends_with('\n') {
            if !text.is_empty() {
                let trimmed = text.trim_end_matches('\n');
                if !trimmed.is_empty() {
                    let fg = syntect_color_to_ratatui(style.foreground);
                    let bold = style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::BOLD);
                    let italic = style
                        .font_style
                        .contains(syntect::highlighting::FontStyle::ITALIC);
                    let mut s = Style::default().fg(fg);
                    if bold {
                        s = s.add_modifier(Modifier::BOLD);
                    }
                    if italic {
                        s = s.add_modifier(Modifier::ITALIC);
                    }
                    current_line_spans.push(Span::styled(trimmed.to_string(), s));
                }
            }
            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
            current_line_spans = vec![Span::styled("│ ", border_style)];
        } else {
            if text.is_empty() {
                continue;
            }
            let fg = syntect_color_to_ratatui(style.foreground);
            let bold = style
                .font_style
                .contains(syntect::highlighting::FontStyle::BOLD);
            let italic = style
                .font_style
                .contains(syntect::highlighting::FontStyle::ITALIC);
            let mut s = Style::default().fg(fg);
            if bold {
                s = s.add_modifier(Modifier::BOLD);
            }
            if italic {
                s = s.add_modifier(Modifier::ITALIC);
            }
            current_line_spans.push(Span::styled(text, s));
        }
    }
    if current_line_spans.len() > 1 {
        lines.push(Line::from(current_line_spans));
    }

    // Bottom border
    let bot = "─".repeat(width.saturating_sub(2));
    lines.push(Line::from(vec![Span::styled(
        format!("╰{}", bot),
        border_style,
    )]));
}

fn syntect_color_to_ratatui(c: syntect::highlighting::Color) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(c.r, c.g, c.b)
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------

fn render_table(
    lines: &mut Vec<Line<'static>>,
    headers: &[String],
    rows: &[Vec<String>],
    width: usize,
) {
    if headers.is_empty() {
        return;
    }
    let ncols = headers.len();
    let col_widths = compute_col_widths(headers, rows, width, ncols);

    let border = Theme::table_border();
    let h_style = Theme::table_header();

    // Top border
    lines.push(table_top_border(&col_widths, border));

    // Header row
    let mut header_spans = vec![Span::styled("│", border)];
    for (i, h) in headers.iter().enumerate() {
        let w = col_widths[i];
        let trunc = truncate_text(h, w);
        header_spans.push(Span::styled(
            format!(" {:<width$} ", trunc, width = w),
            h_style,
        ));
        header_spans.push(Span::styled("│", border));
    }
    lines.push(Line::from(header_spans));

    // Header separator
    lines.push(table_separator(&col_widths, border));

    // Data rows
    for (ri, row) in rows.iter().enumerate() {
        let row_style = if ri % 2 == 0 {
            Theme::table_row_even()
        } else {
            Theme::table_row_odd()
        };
        let mut row_spans = vec![Span::styled("│", border)];
        for (i, cell) in row.iter().enumerate() {
            let w = col_widths.get(i).copied().unwrap_or(10);
            let trunc = truncate_text(cell, w);
            row_spans.push(Span::styled(
                format!(" {:<width$} ", trunc, width = w),
                row_style,
            ));
            row_spans.push(Span::styled("│", border));
        }
        // pad missing cells
        for w in col_widths.iter().take(ncols).skip(row.len()) {
            row_spans.push(Span::styled(" ".repeat(w + 2), row_style));
            row_spans.push(Span::styled("│", border));
        }
        lines.push(Line::from(row_spans));
    }

    // Bottom border
    lines.push(table_bottom_border(&col_widths, border));
}

fn compute_col_widths(
    headers: &[String],
    rows: &[Vec<String>],
    max_width: usize,
    ncols: usize,
) -> Vec<usize> {
    let mut widths: Vec<usize> = headers.iter().map(|h| display_width(h).max(3)).collect();
    for row in rows {
        for (i, cell) in row.iter().enumerate() {
            if i < widths.len() {
                widths[i] = widths[i].max(display_width(cell));
            }
        }
    }
    // Cap so total fits within max_width
    // borders: ncols+1 `│` + 2 spaces per col
    let overhead = ncols + 1 + ncols * 2;
    let avail = max_width.saturating_sub(overhead);
    let total: usize = widths.iter().sum();
    if total > avail && avail > 0 {
        // Give each column a fair minimum based on its content, then distribute
        // remaining space proportionally among columns that can shrink more.
        let min_per_col = 4;
        let guaranteed = ncols * min_per_col;
        if avail < guaranteed {
            // Not enough space for minimums — distribute equally
            let equal = (avail / ncols).max(3);
            widths = vec![equal; ncols];
        } else {
            let remaining = avail - guaranteed;
            let shrinkable: usize = widths.iter().map(|&w| w.saturating_sub(min_per_col)).sum();
            if shrinkable == 0 {
                widths = vec![min_per_col; ncols];
            } else {
                let scale = remaining as f64 / shrinkable as f64;
                widths = widths
                    .iter()
                    .map(|&w| {
                        let extra =
                            ((w.saturating_sub(min_per_col)) as f64 * scale).floor() as usize;
                        (min_per_col + extra).max(3)
                    })
                    .collect();
            }
        }
    }
    widths
}

fn truncate_text(s: &str, max_w: usize) -> String {
    if max_w == 0 {
        return String::new();
    }
    let mut result = String::new();
    let mut w = 0;
    for c in s.chars() {
        let cw = UnicodeWidthChar::width(c).unwrap_or(0);
        if w + cw > max_w {
            let ellipsis_width = UnicodeWidthChar::width('…').unwrap_or(1);
            while !result.is_empty() && w + ellipsis_width > max_w {
                if let Some(removed) = result.pop() {
                    w = w.saturating_sub(UnicodeWidthChar::width(removed).unwrap_or(0));
                }
            }
            if w + ellipsis_width <= max_w {
                result.push('…');
            }
            break;
        }
        result.push(c);
        w += cw;
    }
    result
}

fn display_width(text: &str) -> usize {
    UnicodeWidthStr::width(text)
}

fn push_wrapped_chunk(
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

fn byte_index_for_width(text: &str, max_width: usize) -> usize {
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

fn build_table_border(
    col_widths: &[usize],
    corners: (&str, &str, &str),
    style: Style,
) -> Line<'static> {
    let mut s = String::from(corners.0);
    for (i, &w) in col_widths.iter().enumerate() {
        s.push_str(&"─".repeat(w + 2));
        if i < col_widths.len() - 1 {
            s.push_str(corners.1);
        } else {
            s.push_str(corners.2);
        }
    }
    Line::from(Span::styled(s, style))
}

fn table_top_border(col_widths: &[usize], style: Style) -> Line<'static> {
    build_table_border(col_widths, ("╭", "┬", "╮"), style)
}

fn table_separator(col_widths: &[usize], style: Style) -> Line<'static> {
    build_table_border(col_widths, ("├", "┼", "┤"), style)
}

fn table_bottom_border(col_widths: &[usize], style: Style) -> Line<'static> {
    build_table_border(col_widths, ("╰", "┴", "╯"), style)
}

// ---------------------------------------------------------------------------
// Search highlight: post-process rendered lines to apply highlights
// ---------------------------------------------------------------------------

pub fn apply_search_highlight(
    lines: Vec<Line<'static>>,
    query: &str,
    current_match_line: Option<usize>,
    start_idx: usize,
    lowercased_texts: Option<&[String]>,
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

#[cfg(test)]
mod tests {
    use super::{render_nodes, truncate_text, IMAGE_RENDER_HEIGHT};
    use crate::config;
    use crate::parser::DocNode;

    #[test]
    fn reserves_space_for_images() {
        config::load().unwrap();
        let rendered = render_nodes(
            &[DocNode::Image {
                src: "img.png".to_string(),
                alt: "alt".to_string(),
            }],
            80,
        );

        assert_eq!(rendered.image_positions[0].0, 0);
        assert_eq!(rendered.lines.len(), IMAGE_RENDER_HEIGHT);
    }

    #[test]
    fn tracks_rendered_line_start_per_node() {
        config::load().unwrap();
        let rendered = render_nodes(
            &[
                DocNode::Heading {
                    level: 1,
                    text: "Heading".to_string(),
                },
                DocNode::Blank,
                DocNode::CodeBlock {
                    language: None,
                    code: "a\nb\n".to_string(),
                },
            ],
            40,
        );

        assert_eq!(rendered.node_line_starts, vec![0, 1, 2]);
    }

    #[test]
    fn keeps_blockquote_images_and_headings_rendered() {
        config::load().unwrap();
        let rendered = render_nodes(
            &[DocNode::BlockQuote(vec![
                DocNode::Heading {
                    level: 2,
                    text: "Quoted".to_string(),
                },
                DocNode::Blank,
                DocNode::Image {
                    src: "img.png".to_string(),
                    alt: "diagram".to_string(),
                },
            ])],
            40,
        );

        assert!(rendered.lines[0]
            .spans
            .iter()
            .any(|span| span.content.contains("Quoted")));
        assert_eq!(rendered.image_positions.len(), 1);
    }

    #[test]
    fn wraps_cjk_text_by_display_width() {
        config::load().unwrap();
        let rendered = render_nodes(
            &[DocNode::Paragraph(vec![crate::parser::InlineSpan::Text(
                "你好世界".to_string(),
            )])],
            4,
        );

        assert!(rendered.lines.len() >= 2);
    }

    #[test]
    fn truncates_utf8_text_without_breaking_encoding() {
        let truncated = truncate_text("你好世界", 3);

        assert!(truncated.is_char_boundary(truncated.len()));
        assert_eq!(unicode_width::UnicodeWidthStr::width(truncated.as_str()), 3);
        assert_eq!(truncated, "你…");
    }
}
