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

use super::{
    display_width, CODE_BLOCK_BOTTOM_LEFT, CODE_BLOCK_BOTTOM_RIGHT, CODE_BLOCK_LEFT_BORDER,
    CODE_BLOCK_RIGHT_BORDER, CODE_BLOCK_SPACE_BEFORE_DASHES, CODE_BLOCK_TOP_LEFT,
    CODE_BLOCK_TOP_RIGHT, MAX_HL_CACHE,
};
use crate::theme::Theme;

// Syntax highlighting infrastructure
use std::sync::OnceLock;

static SYNTAX_SET: OnceLock<SyntaxSet> = OnceLock::new();
static THEME_SET: OnceLock<ThemeSet> = OnceLock::new();

use std::cell::RefCell;
use std::hash::DefaultHasher;
use std::hash::{Hash, Hasher};
use std::rc::Rc;

type HighlightEntry = (SyntectStyle, String);
type HighlightRegions = Vec<HighlightEntry>;
type CachedHighlightRegions = Rc<HighlightRegions>;
type CacheKey = (String, u64);

thread_local! {
    static HL_CACHE: RefCell<HighlightCache> = RefCell::new(HighlightCache::default());
}

#[derive(Default)]
struct HighlightCache {
    map: std::collections::HashMap<CacheKey, CachedHighlightRegions>,
    order: std::collections::VecDeque<CacheKey>,
}

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn get_cached_regions(cache_key: &CacheKey) -> Option<CachedHighlightRegions> {
    HL_CACHE.with(|cache| cache.borrow().map.get(cache_key).cloned())
}

fn cache_regions(cache_key: CacheKey, regions: CachedHighlightRegions) {
    HL_CACHE.with(|cache| {
        let mut cache = cache.borrow_mut();
        if cache.map.contains_key(&cache_key) {
            cache.order.retain(|k| k != &cache_key);
            cache.order.push_back(cache_key.clone());
        } else {
            cache.order.push_back(cache_key.clone());
            while cache.order.len() > MAX_HL_CACHE {
                if let Some(evicted) = cache.order.pop_front() {
                    cache.map.remove(&evicted);
                }
            }
        }
        cache.map.insert(cache_key, regions);
    });
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

    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    let cache_key = (lang_label.to_string(), hasher.finish());
    let cached_regions = get_cached_regions(&cache_key);

    let regions_to_render: CachedHighlightRegions = if let Some(cached) = cached_regions {
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

        let rc = Rc::new(all_regions);
        cache_regions(cache_key, Rc::clone(&rc));
        rc
    };

    let mut current_line_spans: Vec<Span<'static>> =
        vec![Span::styled(CODE_BLOCK_LEFT_BORDER, border_style)];
    let mut current_line_content_width: usize = 0;
    let max_content_width = width.saturating_sub(
        display_width(CODE_BLOCK_LEFT_BORDER) + display_width(CODE_BLOCK_RIGHT_BORDER),
    );

    for (style, text) in regions_to_render.iter() {
        let is_newline = text.ends_with('\n');
        let text = if is_newline {
            text.trim_end_matches('\n')
        } else {
            text
        };

        if !text.is_empty() {
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

            let mut current_chunk = String::new();

            for c in text.chars() {
                let c_width = crate::width::char_width(c);

                if current_line_content_width + c_width > max_content_width {
                    if !current_chunk.is_empty() {
                        current_line_spans.push(Span::styled(std::mem::take(&mut current_chunk), s));
                    }

                    let padding = max_content_width.saturating_sub(current_line_content_width);
                    if padding > 0 {
                        current_line_spans
                            .push(Span::styled(" ".repeat(padding), Style::default()));
                    }
                    current_line_spans.push(Span::styled(CODE_BLOCK_RIGHT_BORDER, border_style));
                    lines.push(Line::from(std::mem::take(&mut current_line_spans)));

                    current_line_spans = vec![Span::styled(CODE_BLOCK_LEFT_BORDER, border_style)];
                    current_line_content_width = 0;
                }

                current_chunk.push(c);
                current_line_content_width += c_width;
            }

            if !current_chunk.is_empty() {
                current_line_spans.push(Span::styled(current_chunk, s));
            }
        }

        if is_newline {
            let padding = max_content_width.saturating_sub(current_line_content_width);
            if padding > 0 {
                current_line_spans.push(Span::styled(" ".repeat(padding), Style::default()));
            }
            current_line_spans.push(Span::styled(CODE_BLOCK_RIGHT_BORDER, border_style));
            lines.push(Line::from(std::mem::take(&mut current_line_spans)));

            current_line_spans = vec![Span::styled(CODE_BLOCK_LEFT_BORDER, border_style)];
            current_line_content_width = 0;
        }
    }
    if current_line_spans.len() > 1 || current_line_content_width > 0 {
        let padding = max_content_width.saturating_sub(current_line_content_width);
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


