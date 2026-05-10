//! Code block rendering with syntax highlighting.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use syntect::{
    easy::HighlightLines,
    highlighting::{Style as SyntectStyle, ThemeSet},
    parsing::SyntaxSet,
    util::LinesWithEndings,
};

use super::{display_width, CODE_BLOCK_LEFT_BORDER, CODE_BLOCK_RIGHT_BORDER, CODE_BLOCK_TOP_LEFT, CODE_BLOCK_TOP_RIGHT, CODE_BLOCK_BOTTOM_LEFT, CODE_BLOCK_BOTTOM_RIGHT, CODE_BLOCK_SPACE_BEFORE_DASHES, MAX_HL_CACHE};
use crate::theme::Theme;

// Syntax highlighting infrastructure
static SYNTAX_SET: std::sync::OnceLock<SyntaxSet> = std::sync::OnceLock::new();
static THEME_SET: std::sync::OnceLock<ThemeSet> = std::sync::OnceLock::new();
static HL_CACHE: std::sync::OnceLock<std::sync::Mutex<HighlightCache>> = std::sync::OnceLock::new();

#[derive(Default)]
struct HighlightCache {
    map: std::collections::HashMap<(String, String), Vec<(SyntectStyle, String)>>,
    order: std::collections::VecDeque<(String, String)>,
}


fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn get_hl_cache() -> &'static std::sync::Mutex<HighlightCache> {
    HL_CACHE.get_or_init(|| std::sync::Mutex::new(HighlightCache::default()))
}

fn get_cached_regions(cache_key: &(String, String)) -> Option<Vec<(SyntectStyle, String)>> {
    get_hl_cache()
        .lock()
        .ok()
        .and_then(|cache| cache.map.get(cache_key).cloned())
}

fn cache_regions(cache_key: (String, String), regions: Vec<(SyntectStyle, String)>) {
    if let Ok(mut cache) = get_hl_cache().lock() {
        if !cache.map.contains_key(&cache_key) {
            cache.order.push_back(cache_key.clone());
            while cache.order.len() > MAX_HL_CACHE {
                if let Some(evicted) = cache.order.pop_front() {
                    cache.map.remove(&evicted);
                }
            }
        }
        cache.map.insert(cache_key, regions);
    }
}

pub(super) fn render_code_block(
    lines: &mut Vec<Line<'static>>,
    language: Option<&str>,
    code: &str,
    width: usize,
) {
    let border_style = Theme::code_block_border();
    let lang_label = language.unwrap_or("text");
    let lang_label_width = display_width(lang_label);

    let top_border_width = display_width(CODE_BLOCK_TOP_LEFT)
        + lang_label_width
        + CODE_BLOCK_SPACE_BEFORE_DASHES
        + display_width(CODE_BLOCK_TOP_RIGHT);
    let top_right_len = width.saturating_sub(top_border_width);
    let top_right = "─".repeat(top_right_len);
    let top_right_with_space = format!(" {}{}", top_right, CODE_BLOCK_TOP_RIGHT);
    lines.push(Line::from(vec![
        Span::styled(CODE_BLOCK_TOP_LEFT, border_style),
        Span::styled(lang_label.to_string(), Theme::code_block_lang()),
        Span::styled(top_right_with_space, border_style),
    ]));

    let cache_key = (lang_label.to_string(), code.to_string());
    let cached_regions = get_cached_regions(&cache_key);

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

        cache_regions(cache_key, all_regions.clone());
        all_regions
    };

    let mut current_line_spans: Vec<Span<'static>> =
        vec![Span::styled(CODE_BLOCK_LEFT_BORDER, border_style)];
    let mut current_line_content_width: usize = display_width(CODE_BLOCK_LEFT_BORDER);
    let right_border_width = display_width(CODE_BLOCK_RIGHT_BORDER);
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
                    current_line_content_width += display_width(trimmed);
                }
            }
            let padding = width.saturating_sub(current_line_content_width + right_border_width);
            if padding > 0 {
                current_line_spans.push(Span::styled(" ".repeat(padding), Style::default()));
            }
            current_line_spans.push(Span::styled(CODE_BLOCK_RIGHT_BORDER, border_style));
            lines.push(Line::from(std::mem::take(&mut current_line_spans)));
            current_line_spans = vec![Span::styled(CODE_BLOCK_LEFT_BORDER, border_style)];
            current_line_content_width = display_width(CODE_BLOCK_LEFT_BORDER);
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
            let text_width = display_width(&text);
            current_line_spans.push(Span::styled(text, s));
            current_line_content_width += text_width;
        }
    }
    if current_line_spans.len() > 1 {
        let padding = width.saturating_sub(current_line_content_width + right_border_width);
        if padding > 0 {
            current_line_spans.push(Span::styled(" ".repeat(padding), Style::default()));
        }
        current_line_spans.push(Span::styled(CODE_BLOCK_RIGHT_BORDER, border_style));
        lines.push(Line::from(current_line_spans));
    }

    let bot = "─".repeat(width.saturating_sub(
        display_width(CODE_BLOCK_BOTTOM_LEFT) + display_width(CODE_BLOCK_BOTTOM_RIGHT),
    ));
    lines.push(Line::from(vec![Span::styled(
        format!(
            "{}{}{}",
            CODE_BLOCK_BOTTOM_LEFT, bot, CODE_BLOCK_BOTTOM_RIGHT
        ),
        border_style,
    )]));
}

pub(super) fn syntect_color_to_ratatui(c: syntect::highlighting::Color) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(c.r, c.g, c.b)
}

// ---------------------------------------------------------------------------
// Tables
// ---------------------------------------------------------------------------
