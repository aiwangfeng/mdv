// src/theme.rs
// Dynamic theme definition loaded from configuration.

use crate::config;
use ratatui::style::{Modifier, Style};
use std::sync::{LazyLock, RwLock, RwLockReadGuard};

pub struct Theme;

#[derive(Clone)]
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
}

static STYLE_CACHE: LazyLock<RwLock<CachedStyles>> =
    LazyLock::new(|| RwLock::new(build_cached_styles()));

fn cached_styles() -> RwLockReadGuard<'static, CachedStyles> {
    STYLE_CACHE.read().expect("theme style cache poisoned")
}

pub fn refresh_cached_styles() {
    *STYLE_CACHE.write().expect("theme style cache poisoned") = build_cached_styles();
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
    }
}

impl Theme {
    pub fn h1() -> Style {
        cached_styles().h1
    }
    pub fn h2() -> Style {
        cached_styles().h2
    }
    pub fn h3() -> Style {
        cached_styles().h3
    }
    pub fn h4() -> Style {
        cached_styles().h4
    }
    pub fn h5() -> Style {
        cached_styles().h5
    }
    pub fn h6() -> Style {
        cached_styles().h6
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
        cached_styles().text
    }
    pub fn subtext() -> Style {
        cached_styles().subtext
    }
    pub fn bold() -> Style {
        cached_styles().bold
    }
    pub fn italic() -> Style {
        cached_styles().italic
    }
    pub fn bold_italic() -> Style {
        cached_styles().bold_italic
    }
    pub fn strikethrough() -> Style {
        cached_styles().strikethrough
    }
    pub fn inline_code() -> Style {
        cached_styles().inline_code
    }
    pub fn link() -> Style {
        cached_styles().link
    }
    pub fn blockquote_bar() -> Style {
        cached_styles().blockquote_bar
    }
    pub fn rule() -> Style {
        cached_styles().rule
    }
    pub fn table_header() -> Style {
        cached_styles().table_header
    }
    pub fn table_border() -> Style {
        cached_styles().table_border
    }
    pub fn table_row_even() -> Style {
        cached_styles().table_row_even
    }
    pub fn table_row_odd() -> Style {
        cached_styles().table_row_odd
    }
    pub fn bullet(depth: usize) -> Style {
        let colors = [
            config::get_theme(|t| t.mauve),
            config::get_theme(|t| t.blue),
            config::get_theme(|t| t.teal),
            config::get_theme(|t| t.green),
            config::get_theme(|t| t.yellow),
            config::get_theme(|t| t.peach),
        ];
        Style::default().fg(colors[depth % colors.len()])
    }
    pub fn code_block_border() -> Style {
        cached_styles().code_block_border
    }
    pub fn code_block_lang() -> Style {
        cached_styles().code_block_lang
    }

    // TOC panel
    pub fn toc_border() -> Style {
        cached_styles().toc_border
    }
    pub fn toc_border_focused() -> Style {
        cached_styles().toc_border_focused
    }
    pub fn toc_title() -> Style {
        cached_styles().toc_title
    }
    pub fn toc_item(level: u8) -> Style {
        Self::heading(level).remove_modifier(Modifier::BOLD)
    }
    pub fn toc_selected() -> Style {
        cached_styles().toc_selected
    }
    pub fn toc_synced() -> Style {
        cached_styles().toc_synced
    }

    // Content panel
    pub fn content_border() -> Style {
        cached_styles().content_border
    }
    pub fn content_border_focused() -> Style {
        cached_styles().content_border_focused
    }
    pub fn content_title() -> Style {
        cached_styles().content_title
    }

    // Search
    pub fn search_match() -> Style {
        cached_styles().search_match
    }
    pub fn search_current() -> Style {
        cached_styles().search_current
    }

    // Status bar
    pub fn statusbar() -> Style {
        cached_styles().statusbar
    }
    pub fn statusbar_mode() -> Style {
        cached_styles().statusbar_mode
    }
    pub fn statusbar_key() -> Style {
        cached_styles().statusbar_key
    }
    pub fn statusbar_dim() -> Style {
        cached_styles().statusbar_dim
    }
}
