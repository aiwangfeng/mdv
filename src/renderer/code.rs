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
type CacheKey = (String, usize, u64);

thread_local! {
    static HL_CACHE: RefCell<HighlightCache> = RefCell::new(HighlightCache::default());
}

#[derive(Default)]
struct HighlightCache {
    map: std::collections::HashMap<CacheKey, (CachedHighlightRegions, u64)>,
    access_counter: u64,
}

fn get_syntax_set() -> &'static SyntaxSet {
    SYNTAX_SET.get_or_init(SyntaxSet::load_defaults_newlines)
}

fn get_theme_set() -> &'static ThemeSet {
    THEME_SET.get_or_init(ThemeSet::load_defaults)
}

fn get_cached_regions(cache_key: &CacheKey) -> Option<CachedHighlightRegions> {
    HL_CACHE.with(|cache| {
        let mut c = cache.borrow_mut();
        c.access_counter += 1;
        let counter = c.access_counter;
        if let Some((regions, access)) = c.map.get_mut(cache_key) {
            *access = counter;
            Some(Rc::clone(regions))
        } else {
            None
        }
    })
}

fn cache_regions(cache_key: CacheKey, regions: CachedHighlightRegions) {
    HL_CACHE.with(|cache| {
        let mut c = cache.borrow_mut();
        c.access_counter += 1;
        let counter = c.access_counter;
        c.map.insert(cache_key, (regions, counter));
        if c.map.len() > MAX_HL_CACHE {
            if let Some(oldest_key) = c
                .map
                .iter()
                .min_by_key(|(_, (_, access))| *access)
                .map(|(k, _)| k.clone())
            {
                c.map.remove(&oldest_key);
            }
        }
    });
}

pub(crate) fn expand_tabs(s: &str, tab_size: usize) -> String {
    if !s.contains(['\t', '\r']) {
        return s.to_string();
    }
    let tab_size = tab_size.max(1);
    let mut result = String::with_capacity(s.len() + 16);
    let mut col = 0;
    for c in s.chars() {
        match c {
            '\t' => {
                let spaces = tab_size - (col % tab_size);
                for _ in 0..spaces {
                    result.push(' ');
                }
                col += spaces;
            }
            '\n' => {
                result.push('\n');
                col = 0;
            }
            '\r' => {}
            c => {
                let w = crate::width::char_width(c);
                result.push(c);
                col += w;
            }
        }
    }
    result
}

pub(super) fn render_code_block(
    lines: &mut Vec<Line<'static>>,
    language: Option<&str>,
    code: &str,
    width: usize,
) {
    let expanded_code_storage;
    let code = if code.contains(['\t', '\r']) {
        expanded_code_storage = expand_tabs(code, 4);
        &expanded_code_storage
    } else {
        code
    };

    let border_style = Theme::code_block_border();
    let raw_lang = language.unwrap_or("text");
    let fixed_top_overhead = display_width(CODE_BLOCK_TOP_LEFT)
        + CODE_BLOCK_SPACE_BEFORE_DASHES
        + display_width(CODE_BLOCK_TOP_RIGHT);
    let max_lang_width = width.saturating_sub(fixed_top_overhead);
    let lang_label = if display_width(raw_lang) > max_lang_width {
        super::table::truncate_text(raw_lang, max_lang_width)
    } else {
        raw_lang.to_string()
    };
    let lang_label_width = display_width(&lang_label);

    let top_border_width = fixed_top_overhead + lang_label_width;
    let top_right_len = width.saturating_sub(top_border_width);
    let top_right = "─".repeat(top_right_len);
    let top_right_with_space = format!(" {top_right}{CODE_BLOCK_TOP_RIGHT}");
    lines.push(Line::from(vec![
        Span::styled(CODE_BLOCK_TOP_LEFT, border_style),
        Span::styled(lang_label.clone(), Theme::code_block_lang()),
        Span::styled(top_right_with_space, border_style),
    ]));

    let theme_idx = crate::config::current_theme_index();
    let theme_name =
        crate::config::AVAILABLE_THEMES[theme_idx % crate::config::AVAILABLE_THEMES.len()];
    let syntect_name = theme_name.syntect_theme_name();

    let mut hasher = DefaultHasher::new();
    code.hash(&mut hasher);
    let lookup_lang = language.unwrap_or("text");
    let cache_key = (lookup_lang.to_string(), theme_idx, hasher.finish());
    let cached_regions = get_cached_regions(&cache_key);

    let regions_to_render: CachedHighlightRegions = if let Some(cached) = cached_regions {
        cached
    } else {
        let ss = get_syntax_set();
        let ts = get_theme_set();
        let syntax = ss
            .find_syntax_by_token(lookup_lang)
            .unwrap_or_else(|| ss.find_syntax_plain_text());
        let theme = ts
            .themes
            .get(syntect_name)
            .unwrap_or(&ts.themes["base16-ocean.dark"]);
        let mut highlighter = HighlightLines::new(syntax, theme);

        let mut all_regions = Vec::new();
        for line_str in LinesWithEndings::from(code) {
            match highlighter.highlight_line(line_str, ss) {
                Ok(regions) => {
                    all_regions.extend(regions.into_iter().map(|(s, t)| (s, t.to_string())));
                }
                Err(_) => {
                    all_regions.push((SyntectStyle::default(), line_str.to_string()));
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
    let max_content_width = width
        .saturating_sub(
            display_width(CODE_BLOCK_LEFT_BORDER) + display_width(CODE_BLOCK_RIGHT_BORDER),
        )
        .max(1);

    for (style, text) in regions_to_render.iter() {
        let is_newline = text.ends_with('\n');
        let text = if is_newline {
            text.trim_end_matches(['\r', '\n'])
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

                if current_line_content_width > 0
                    && current_line_content_width + c_width > max_content_width
                {
                    if !current_chunk.is_empty() {
                        current_line_spans
                            .push(Span::styled(std::mem::take(&mut current_chunk), s));
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
        format!("{CODE_BLOCK_BOTTOM_LEFT}{bot}{CODE_BLOCK_BOTTOM_RIGHT}"),
        border_style,
    )]));
}

pub(super) fn syntect_color_to_ratatui(c: syntect::highlighting::Color) -> ratatui::style::Color {
    ratatui::style::Color::Rgb(c.r, c.g, c.b)
}
