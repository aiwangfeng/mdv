// src/theme.rs
// Dynamic theme definition loaded from configuration.

use crate::config;
use ratatui::style::{Modifier, Style};
use std::cell::RefCell;

pub struct Theme;

#[derive(Clone, Copy)]
pub struct CachedStyles {
    pub h1: Style,
    pub h2: Style,
    pub h3: Style,
    pub h4: Style,
    pub h5: Style,
    pub h6: Style,
    pub text: Style,
    pub subtext: Style,
    pub bold: Style,
    pub italic: Style,
    pub bold_italic: Style,
    pub strikethrough: Style,
    pub inline_code: Style,
    pub link: Style,
    pub blockquote_bar: Style,
    pub rule: Style,
    pub table_header: Style,
    pub table_border: Style,
    pub table_row_even: Style,
    pub table_row_odd: Style,
    pub code_block_border: Style,
    pub code_block_lang: Style,
    pub toc_border: Style,
    pub toc_border_focused: Style,
    pub toc_title: Style,
    pub toc_selected: Style,
    pub toc_synced: Style,
    pub content_border: Style,
    pub content_border_focused: Style,
    pub content_title: Style,
    pub search_match: Style,
    pub search_current: Style,
    pub statusbar: Style,
    pub statusbar_mode: Style,
    pub statusbar_key: Style,
    pub statusbar_dim: Style,
    pub bullets: [Style; 6],
}

thread_local! {
    static STYLE_CACHE: RefCell<CachedStyles> = RefCell::new(build_cached_styles());
}

fn get_style<F, R>(f: F) -> R
where
    F: FnOnce(&CachedStyles) -> R,
{
    STYLE_CACHE.with(|cell| f(&cell.borrow()))
}

pub fn refresh_cached_styles() {
    STYLE_CACHE.with(|cell| {
        *cell.borrow_mut() = build_cached_styles();
    });
}

fn build_cached_styles() -> CachedStyles {
    CachedStyles {
        h1: Style::default()
            .fg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD),
        h2: Style::default()
            .fg(config::get_theme(|t| t.lavender))
            .add_modifier(Modifier::BOLD),
        h3: Style::default()
            .fg(config::get_theme(|t| t.sapphire))
            .add_modifier(Modifier::BOLD),
        h4: Style::default()
            .fg(config::get_theme(|t| t.teal))
            .add_modifier(Modifier::BOLD),
        h5: Style::default()
            .fg(config::get_theme(|t| t.green))
            .add_modifier(Modifier::BOLD),
        h6: Style::default()
            .fg(config::get_theme(|t| t.yellow))
            .add_modifier(Modifier::BOLD),
        text: Style::default().fg(config::get_theme(|t| t.text)),
        subtext: Style::default().fg(config::get_theme(|t| t.subtext)),
        bold: Style::default()
            .fg(config::get_theme(|t| t.text))
            .add_modifier(Modifier::BOLD),
        italic: Style::default()
            .fg(config::get_theme(|t| t.subtext))
            .add_modifier(Modifier::ITALIC),
        bold_italic: Style::default()
            .fg(config::get_theme(|t| t.text))
            .add_modifier(Modifier::BOLD | Modifier::ITALIC),
        strikethrough: Style::default()
            .fg(config::get_theme(|t| t.overlay0))
            .add_modifier(Modifier::CROSSED_OUT),
        inline_code: Style::default()
            .fg(config::get_theme(|t| t.peach))
            .bg(config::get_theme(|t| t.surface0)),
        link: Style::default()
            .fg(config::get_theme(|t| t.blue))
            .add_modifier(Modifier::UNDERLINED),
        blockquote_bar: Style::default().fg(config::get_theme(|t| t.teal)),
        rule: Style::default().fg(config::get_theme(|t| t.surface1)),
        table_header: Style::default()
            .fg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD),
        table_border: Style::default().fg(config::get_theme(|t| t.surface1)),
        table_row_even: Style::default().fg(config::get_theme(|t| t.text)),
        table_row_odd: Style::default().fg(config::get_theme(|t| t.subtext)),
        code_block_border: Style::default().fg(config::get_theme(|t| t.surface1)),
        code_block_lang: Style::default()
            .fg(config::get_theme(|t| t.peach))
            .add_modifier(Modifier::ITALIC),
        toc_border: Style::default().fg(config::get_theme(|t| t.surface1)),
        toc_border_focused: Style::default().fg(config::get_theme(|t| t.mauve)),
        toc_title: Style::default()
            .fg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD),
        toc_selected: Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD),
        toc_synced: Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.lavender)),
        content_border: Style::default().fg(config::get_theme(|t| t.surface1)),
        content_border_focused: Style::default().fg(config::get_theme(|t| t.blue)),
        content_title: Style::default()
            .fg(config::get_theme(|t| t.blue))
            .add_modifier(Modifier::BOLD),
        search_match: Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.yellow))
            .add_modifier(Modifier::BOLD),
        search_current: Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.peach))
            .add_modifier(Modifier::BOLD),
        statusbar: Style::default()
            .fg(config::get_theme(|t| t.text))
            .bg(config::get_theme(|t| t.surface0)),
        statusbar_mode: Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD),
        statusbar_key: Style::default()
            .fg(config::get_theme(|t| t.peach))
            .add_modifier(Modifier::BOLD),
        statusbar_dim: Style::default().fg(config::get_theme(|t| t.overlay0)),
        bullets: [
            Style::default().fg(config::get_theme(|t| t.mauve)),
            Style::default().fg(config::get_theme(|t| t.blue)),
            Style::default().fg(config::get_theme(|t| t.teal)),
            Style::default().fg(config::get_theme(|t| t.green)),
            Style::default().fg(config::get_theme(|t| t.yellow)),
            Style::default().fg(config::get_theme(|t| t.peach)),
        ],
    }
}

impl Theme {
    pub fn h1() -> Style {
        get_style(|s| s.h1)
    }
    pub fn h2() -> Style {
        get_style(|s| s.h2)
    }
    pub fn h3() -> Style {
        get_style(|s| s.h3)
    }
    pub fn h4() -> Style {
        get_style(|s| s.h4)
    }
    pub fn h5() -> Style {
        get_style(|s| s.h5)
    }
    pub fn h6() -> Style {
        get_style(|s| s.h6)
    }

    pub fn heading(level: u8) -> Style {
        match level {
            1 => Self::h1(),
            2 => Self::h2(),
            3 => Self::h3(),
            4 => Self::h4(),
            5 => Self::h5(),
            _ => Self::h6(),
        }
    }

    pub fn text() -> Style {
        get_style(|s| s.text)
    }
    pub fn subtext() -> Style {
        get_style(|s| s.subtext)
    }
    pub fn bold() -> Style {
        get_style(|s| s.bold)
    }
    pub fn italic() -> Style {
        get_style(|s| s.italic)
    }
    pub fn bold_italic() -> Style {
        get_style(|s| s.bold_italic)
    }
    pub fn strikethrough() -> Style {
        get_style(|s| s.strikethrough)
    }
    pub fn inline_code() -> Style {
        get_style(|s| s.inline_code)
    }
    pub fn link() -> Style {
        get_style(|s| s.link)
    }
    pub fn blockquote_bar() -> Style {
        get_style(|s| s.blockquote_bar)
    }
    pub fn rule() -> Style {
        get_style(|s| s.rule)
    }
    pub fn table_header() -> Style {
        get_style(|s| s.table_header)
    }
    pub fn table_border() -> Style {
        get_style(|s| s.table_border)
    }
    pub fn table_row_even() -> Style {
        get_style(|s| s.table_row_even)
    }
    pub fn table_row_odd() -> Style {
        get_style(|s| s.table_row_odd)
    }
    pub fn bullet(depth: usize) -> Style {
        get_style(|s| s.bullets[depth % s.bullets.len()])
    }
    pub fn code_block_border() -> Style {
        get_style(|s| s.code_block_border)
    }
    pub fn code_block_lang() -> Style {
        get_style(|s| s.code_block_lang)
    }

    // TOC panel
    pub fn toc_border() -> Style {
        get_style(|s| s.toc_border)
    }
    pub fn toc_border_focused() -> Style {
        get_style(|s| s.toc_border_focused)
    }
    pub fn toc_title() -> Style {
        get_style(|s| s.toc_title)
    }
    pub fn toc_item(level: u8) -> Style {
        Self::heading(level).remove_modifier(Modifier::BOLD)
    }
    pub fn toc_selected() -> Style {
        get_style(|s| s.toc_selected)
    }
    pub fn toc_synced() -> Style {
        get_style(|s| s.toc_synced)
    }

    // Content panel
    pub fn content_border() -> Style {
        get_style(|s| s.content_border)
    }
    pub fn content_border_focused() -> Style {
        get_style(|s| s.content_border_focused)
    }
    pub fn content_title() -> Style {
        get_style(|s| s.content_title)
    }

    // Search
    pub fn search_match() -> Style {
        get_style(|s| s.search_match)
    }
    pub fn search_current() -> Style {
        get_style(|s| s.search_current)
    }

    // Status bar
    pub fn statusbar() -> Style {
        get_style(|s| s.statusbar)
    }
    pub fn statusbar_mode() -> Style {
        get_style(|s| s.statusbar_mode)
    }
    pub fn statusbar_key() -> Style {
        get_style(|s| s.statusbar_key)
    }
    pub fn statusbar_dim() -> Style {
        get_style(|s| s.statusbar_dim)
    }
}
