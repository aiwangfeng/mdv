// src/ui.rs
// Draws the full TUI: layout, TOC panel, content panel, status bar, overlays.

pub const CONTENT_MARGIN: u16 = 4;

use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame,
};
use ratatui_image::{Resize, StatefulImage};
use unicode_width::UnicodeWidthStr;

use crate::app::{App, Focus, Mode};
use crate::config;
use crate::image_proto::ImageManager;
use crate::renderer::{apply_search_highlight, IMAGE_RENDER_HEIGHT};
use crate::theme::Theme;

pub fn draw(frame: &mut Frame, app: &mut App, img_mgr: &mut ImageManager) {
    let area = frame.area();

    // ── Overall vertical split: content area + status bar ──────────────────
    let [main_area, status_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    // ── Horizontal split: TOC (optional) | Content ─────────────────────────
    let (toc_area, content_area) = if app.show_toc && app.toc_len() > 0 {
        let toc_pct = app.toc_width_pct;
        let [l, r] = Layout::horizontal([
            Constraint::Percentage(toc_pct),
            Constraint::Percentage(100 - toc_pct),
        ])
        .areas(main_area);
        (Some(l), r)
    } else {
        (None, main_area)
    };

    let inner_content_height = content_area.height.saturating_sub(2);
    let inner_content_width = content_area
        .width
        .saturating_sub(2)
        .saturating_sub(CONTENT_MARGIN);
    app.content_height = inner_content_height;
    app.content_width = inner_content_width;
    if let Some(ta) = toc_area {
        app.toc_height = ta.height.saturating_sub(2);
    }

    // ── Draw panels ─────────────────────────────────────────────────────────
    if let Some(ta) = toc_area {
        draw_toc(frame, app, ta);
    }
    draw_content(frame, app, img_mgr, content_area);
    draw_status_bar(frame, app, status_area);

    // ── Overlays ────────────────────────────────────────────────────────────
    match app.mode {
        Mode::Help => draw_help_overlay(frame, area),
        Mode::Search => draw_search_bar(frame, app, area),
        Mode::ThemePicker => draw_theme_overlay(frame, app, area),
        Mode::Normal => {}
    }
}

// ---------------------------------------------------------------------------
// TOC panel
// ---------------------------------------------------------------------------

fn draw_toc(frame: &mut Frame, app: &App, area: Rect) {
    let focused = app.focus == Focus::Toc;
    let border_style = if focused {
        Theme::toc_border_focused()
    } else {
        Theme::toc_border()
    };
    let title_style = Theme::toc_title();
    let synced = app.synced_toc_index();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(" 󰉻 TOC ", title_style));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let items: Vec<ListItem> = app
        .document
        .toc
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let prefix = match entry.level {
                1 => "▌ ",
                2 => "  ▎ ",
                _ => "    · ",
            };
            let style = if i == app.toc_cursor && focused {
                Theme::toc_selected()
            } else if Some(i) == synced {
                Theme::toc_synced()
            } else {
                Theme::toc_item(entry.level)
            };
            let text = format!("{}{}", prefix, entry.title);
            ListItem::new(Line::from(Span::styled(text, style)))
        })
        .collect();

    let mut list_state = ListState::default();
    list_state.select(Some(app.toc_cursor.saturating_sub(app.toc_scroll)));

    // Render visible slice
    let visible_start = app.toc_scroll;
    let visible_end = (app.toc_scroll + inner.height as usize).min(items.len());
    let visible_items: Vec<ListItem> = items
        .into_iter()
        .skip(visible_start)
        .take(visible_end - visible_start)
        .collect();

    let list = List::new(visible_items).highlight_style(Theme::toc_selected());
    frame.render_stateful_widget(list, inner, &mut list_state);
}

// ---------------------------------------------------------------------------
// Content panel
// ---------------------------------------------------------------------------

fn draw_content(frame: &mut Frame, app: &mut App, img_mgr: &mut ImageManager, area: Rect) {
    let focused = app.focus == Focus::Content;
    let border_style = if focused {
        Theme::content_border_focused()
    } else {
        Theme::content_border()
    };
    let title_style = Theme::content_title();

    let m = if app.search_matches.is_empty() {
        String::new()
    } else {
        format!(" {}/{} ", app.search_current + 1, app.search_matches.len())
    };
    let title = format!("  {} {}", app.file_name, m);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(title, title_style));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let [inner_with_margin] = Layout::horizontal([Constraint::Min(0)])
        .horizontal_margin(CONTENT_MARGIN / 2)
        .areas(inner);

    let scroll = app.scroll;
    let height = inner_with_margin.height as usize;

    let current_match_line = if app.search_matches.is_empty() {
        None
    } else {
        Some(app.search_matches[app.search_current])
    };

    let visible_lines = if app.search_query.is_empty() {
        app.rendered_lines
            .iter()
            .skip(scroll)
            .take(height)
            .cloned()
            .collect()
    } else {
        let mut result = Vec::with_capacity(height);
        for i in 0..height {
            let line_idx = scroll + i;
            if let Some(cached) = app.get_cached_highlight(line_idx) {
                result.push(cached.clone());
            } else if line_idx < app.rendered_lines.len() {
                let line = app.rendered_lines[line_idx].clone();
                let _is_current = current_match_line == Some(line_idx);
                let lowercased = app.line_lower_ref(line_idx).unwrap_or_else(|| {
                    line.spans
                        .iter()
                        .map(|s| s.content.as_ref())
                        .collect::<String>()
                        .to_lowercase()
                });
                let highlighted = apply_search_highlight(
                    vec![line],
                    &app.search_query,
                    current_match_line,
                    line_idx,
                    Some(std::slice::from_ref(&lowercased)),
                );
                if let Some(hl_line) = highlighted.into_iter().next() {
                    app.cache_highlight(line_idx, hl_line.clone());
                    result.push(hl_line);
                }
            }
        }
        result
    };

    let paragraph = Paragraph::new(Text::from(visible_lines)).wrap(Wrap { trim: false });
    frame.render_widget(paragraph, inner_with_margin);

    if img_mgr.is_enabled() {
        for (line_idx, src, _alt) in &app.image_positions {
            if *line_idx < scroll || *line_idx >= scroll + height {
                continue;
            }
            let row_in_view = (*line_idx - scroll) as u16;
            let img_height = (IMAGE_RENDER_HEIGHT as u16)
                .min(inner_with_margin.height.saturating_sub(row_in_view));
            if img_height == 0 {
                continue;
            }
            let img_area = Rect {
                x: inner_with_margin.x,
                y: inner_with_margin.y + row_in_view,
                width: inner_with_margin.width,
                height: img_height,
            };
            if let Some(protocol) = img_mgr.get_protocol_mut(src) {
                let widget = StatefulImage::new().resize(Resize::Fit(None));
                frame.render_stateful_widget(widget, img_area, protocol);
            }
        }
    }

    let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
    let mut scrollbar_state =
        ScrollbarState::new(app.total_lines().saturating_sub(height)).position(scroll);
    frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
}

// ---------------------------------------------------------------------------
// Status bar
// ---------------------------------------------------------------------------

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_text = match app.mode {
        Mode::Normal => " NORMAL ",
        Mode::Search => " SEARCH ",
        Mode::Help => " HELP   ",
        Mode::ThemePicker => " THEME  ",
    };

    let focus_text = match app.focus {
        Focus::Toc => "TOC",
        Focus::Content => "Content",
    };

    let scroll_pct = if app.total_lines() == 0 {
        100
    } else {
        (app.scroll * 100 / app.total_lines().max(1)).min(100)
    };

    let search_hint = if !app.search_query.is_empty() {
        format!(
            " /{} ({} matches) ",
            app.search_query,
            app.search_matches.len()
        )
    } else {
        String::new()
    };

    let left = Line::from(vec![
        Span::styled(mode_text, Theme::statusbar_mode()),
        Span::styled(format!("  {} ", focus_text), Theme::statusbar()),
        Span::styled(search_hint, Theme::statusbar_key()),
    ]);

    let theme_name = config::current_theme_name();
    let keys_text = format!(
        " q:quit  ?:help  /:search  s:toc  h/l:focus  j/k:scroll  t:theme({}) ",
        theme_name
    );
    let right_text = format!(" {}% ", scroll_pct);
    let right = format!("{}{}", keys_text, right_text);

    // Pad to fill width
    let left_plain: String = left.spans.iter().map(|s| s.content.as_ref()).collect();
    let pad = (area.width as usize).saturating_sub(left_plain.width() + right.as_str().width());

    // Re-build as styled line
    let status_line = Line::from(vec![
        Span::styled(left_plain, Theme::statusbar()),
        Span::styled(" ".repeat(pad), Theme::statusbar()),
        Span::styled(right, Theme::statusbar_dim()),
    ]);

    let paragraph = Paragraph::new(status_line).style(Theme::statusbar());
    frame.render_widget(paragraph, area);
}

// ---------------------------------------------------------------------------
// Search bar overlay
// ---------------------------------------------------------------------------

fn draw_search_bar(frame: &mut Frame, app: &App, area: Rect) {
    let search_area = Rect {
        x: area.x,
        y: area.height.saturating_sub(2),
        width: area.width,
        height: 1,
    };

    let line = Line::from(vec![
        Span::styled("/ ", Theme::statusbar_key()),
        Span::styled(app.search_query.clone(), Theme::search_match()),
        Span::styled("█", Theme::statusbar_key()),
    ]);
    let para = Paragraph::new(line).style(Theme::statusbar());
    frame.render_widget(Clear, search_area);
    frame.render_widget(para, search_area);
}

// ---------------------------------------------------------------------------
// Help overlay
// ---------------------------------------------------------------------------

fn draw_help_overlay(frame: &mut Frame, area: Rect) {
    let width = (area.width / 2).max(50).min(area.width);
    let height = (area.height / 2).max(28).min(area.height);
    let x = (area.width - width) / 2;
    let y = (area.height - height) / 2;
    let help_area = Rect {
        x,
        y,
        width,
        height,
    };

    frame.render_widget(Clear, help_area);

    let items: Vec<(&str, &str)> = vec![
        ("j / k", "Scroll down / up"),
        ("d / u", "Half-page down / up"),
        ("g / G", "Top / bottom"),
        ("h / l", "Switch focus (TOC ↔ Content)"),
        ("J / K", "Move TOC cursor (in TOC)"),
        ("Enter", "Jump to TOC entry"),
        ("[ / ]", "Prev / next heading"),
        ("s", "Toggle TOC sidebar"),
        ("</>", "Narrow / widen TOC"),
        ("/", "Start search"),
        ("n / N", "Next / prev search match"),
        ("t", "Open theme picker"),
        ("Esc", "Cancel search / close overlay"),
        ("?", "Toggle this help"),
        ("q / Ctrl-C", "Quit"),
    ];

    let help_lines: Vec<Line> = std::iter::once(Line::from(Span::styled(
        " Keybindings ",
        Theme::toc_title(),
    )))
    .chain(std::iter::once(Line::default()))
    .chain(items.iter().map(|(key, desc)| {
        Line::from(vec![
            Span::styled(format!("  {:>12}  ", key), Theme::statusbar_key()),
            Span::styled(desc.to_string(), Theme::text()),
        ])
    }))
    .chain(std::iter::once(Line::default()))
    .chain(std::iter::once(Line::from(Span::styled(
        "  Press ? or Esc to close ",
        Theme::subtext(),
    ))))
    .collect();

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::toc_border_focused())
        .title(Span::styled(" mdv help ", Theme::toc_title()));

    let para = Paragraph::new(Text::from(help_lines))
        .block(block)
        .wrap(Wrap { trim: false });

    frame.render_widget(para, help_area);
}

fn draw_theme_overlay(frame: &mut Frame, app: &App, area: Rect) {
    let keys = &config::get().keys;
    let helper_text = format!(
        "{}/{} or ↑/↓ move   Enter confirm   Esc/{}/{} cancel",
        keys.down, keys.up, keys.next_theme, keys.quit
    );
    let min_width = helper_text.width() as u16 + 4;
    let width = area.width.min(min_width.max(30));
    let height = area.height.clamp(9, 12);
    let x = area.x + (area.width.saturating_sub(width)) / 2;
    let y = area.y + (area.height.saturating_sub(height)) / 2;
    let overlay_area = Rect {
        x,
        y,
        width,
        height,
    };

    frame.render_widget(Clear, overlay_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::toc_border_focused())
        .title(Span::styled(" theme picker ", Theme::toc_title()));
    let inner = block.inner(overlay_area);
    frame.render_widget(block, overlay_area);

    let [list_area, hint_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(2)]).areas(inner);

    let items: Vec<ListItem> = config::AVAILABLE_THEMES
        .iter()
        .enumerate()
        .map(|(index, theme)| {
            let prefix = if index == app.theme_picker_index {
                "› "
            } else {
                "  "
            };
            let style = if index == app.theme_picker_index {
                Theme::toc_selected()
            } else {
                Theme::text()
            };
            ListItem::new(Line::from(Span::styled(
                format!("{}{}", prefix, theme.display_name()),
                style,
            )))
        })
        .collect();

    let mut state = ListState::default();
    state.select(Some(app.theme_picker_index));
    let list = List::new(items).highlight_style(Theme::toc_selected());
    frame.render_stateful_widget(list, list_area, &mut state);

    let hint = Paragraph::new(Text::from(vec![
        Line::from(Span::styled(
            "Preview updates as you move",
            Theme::subtext(),
        )),
        Line::from(vec![
            Span::styled(format!("{}/{}", keys.down, keys.up), Theme::statusbar_key()),
            Span::styled(" or ", Theme::subtext()),
            Span::styled("↑/↓", Theme::statusbar_key()),
            Span::styled(" move   ", Theme::subtext()),
            Span::styled("Enter", Theme::statusbar_key()),
            Span::styled(" confirm   ", Theme::subtext()),
            Span::styled(
                format!("Esc/{}/{}", keys.next_theme, keys.quit),
                Theme::statusbar_key(),
            ),
            Span::styled(" cancel", Theme::subtext()),
        ]),
    ]));
    frame.render_widget(hint, hint_area);
}
