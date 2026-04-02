// src/theme.rs
// Dynamic theme definition loaded from configuration.

use crate::config;
use ratatui::style::{Modifier, Style};

pub struct Theme;

impl Theme {
    // --- Semantic styles ---

    pub fn h1() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD)
    }
    pub fn h2() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.lavender))
            .add_modifier(Modifier::BOLD)
    }
    pub fn h3() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.sapphire))
            .add_modifier(Modifier::BOLD)
    }
    pub fn h4() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.teal))
            .add_modifier(Modifier::BOLD)
    }
    pub fn h5() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.green))
            .add_modifier(Modifier::BOLD)
    }
    pub fn h6() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.yellow))
            .add_modifier(Modifier::BOLD)
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
        Style::default().fg(config::get_theme(|t| t.text))
    }
    pub fn subtext() -> Style {
        Style::default().fg(config::get_theme(|t| t.subtext))
    }
    pub fn bold() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.text))
            .add_modifier(Modifier::BOLD)
    }
    pub fn italic() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.subtext))
            .add_modifier(Modifier::ITALIC)
    }
    pub fn bold_italic() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.text))
            .add_modifier(Modifier::BOLD | Modifier::ITALIC)
    }
    pub fn strikethrough() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.overlay0))
            .add_modifier(Modifier::CROSSED_OUT)
    }
    pub fn inline_code() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.peach))
            .bg(config::get_theme(|t| t.surface0))
    }
    pub fn link() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.blue))
            .add_modifier(Modifier::UNDERLINED)
    }
    pub fn blockquote_bar() -> Style {
        Style::default().fg(config::get_theme(|t| t.teal))
    }
    pub fn rule() -> Style {
        Style::default().fg(config::get_theme(|t| t.surface1))
    }
    pub fn table_header() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD)
    }
    pub fn table_border() -> Style {
        Style::default().fg(config::get_theme(|t| t.surface1))
    }
    pub fn table_row_even() -> Style {
        Style::default().fg(config::get_theme(|t| t.text))
    }
    pub fn table_row_odd() -> Style {
        Style::default().fg(config::get_theme(|t| t.subtext))
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
        Style::default().fg(config::get_theme(|t| t.surface1))
    }
    pub fn code_block_lang() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.peach))
            .add_modifier(Modifier::ITALIC)
    }

    // TOC panel
    pub fn toc_border() -> Style {
        Style::default().fg(config::get_theme(|t| t.surface1))
    }
    pub fn toc_border_focused() -> Style {
        Style::default().fg(config::get_theme(|t| t.mauve))
    }
    pub fn toc_title() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD)
    }
    pub fn toc_item(level: u8) -> Style {
        Self::heading(level).remove_modifier(Modifier::BOLD)
    }
    pub fn toc_selected() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD)
    }
    pub fn toc_synced() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.lavender))
    }

    // Content panel
    pub fn content_border() -> Style {
        Style::default().fg(config::get_theme(|t| t.surface1))
    }
    pub fn content_border_focused() -> Style {
        Style::default().fg(config::get_theme(|t| t.blue))
    }
    pub fn content_title() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.blue))
            .add_modifier(Modifier::BOLD)
    }

    // Search
    pub fn search_match() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.yellow))
            .add_modifier(Modifier::BOLD)
    }
    pub fn search_current() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.peach))
            .add_modifier(Modifier::BOLD)
    }

    // Status bar
    pub fn statusbar() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.text))
            .bg(config::get_theme(|t| t.surface0))
    }
    pub fn statusbar_mode() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.crust))
            .bg(config::get_theme(|t| t.mauve))
            .add_modifier(Modifier::BOLD)
    }
    pub fn statusbar_key() -> Style {
        Style::default()
            .fg(config::get_theme(|t| t.peach))
            .add_modifier(Modifier::BOLD)
    }
    pub fn statusbar_dim() -> Style {
        Style::default().fg(config::get_theme(|t| t.overlay0))
    }
}
