// src/ui.rs
// Draws the full TUI: layout, TOC panel, content panel, status bar, overlays.

use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::Style,
    text::{Line, Span, Text},
    widgets::{
        Block, BorderType, Borders, Clear, List, ListItem, ListState, Paragraph, Scrollbar,
        ScrollbarOrientation, ScrollbarState, Wrap,
    },
    Frame,
};
use ratatui_image::{Resize, StatefulImage};

use crate::app::{App, Focus, Mode};
use crate::config;
use crate::image_proto::ImageManager;

const BORDER_SIZE: u16 = 2;
use crate::renderer::{apply_search_highlight, IMAGE_RENDER_HEIGHT};
use crate::theme::Theme;
use crossterm::event::{KeyCode, KeyModifiers};

fn get_content_margin() -> u16 {
    config::get().content_margin
}

pub struct LayoutResult {
    pub toc_area: Option<Rect>,
    pub content_area: Rect,
    pub status_area: Rect,
}

pub fn calculate_layout(app: &mut App, area: Rect) -> LayoutResult {
    let [main_area, status_area] =
        Layout::vertical([Constraint::Min(0), Constraint::Length(1)]).areas(area);

    let show_toc = app.show_toc && (app.toc_len() > 0 || app.is_directory_mode());
    let (toc_area, content_area) = if show_toc {
        let toc_pct = app.toc_width_pct.min(50);
        let [l, r] = Layout::horizontal([
            Constraint::Percentage(toc_pct),
            Constraint::Percentage(100 - toc_pct),
        ])
        .areas(main_area);
        app.toc_height = l.height.saturating_sub(BORDER_SIZE);
        (Some(l), r)
    } else {
        app.toc_height = 0;
        (None, main_area)
    };

    let content_margin = get_content_margin();
    let effective_margin = (content_margin / 2) * 2;
    app.content_height = content_area.height.saturating_sub(BORDER_SIZE);
    app.content_width = content_area
        .width
        .saturating_sub(BORDER_SIZE)
        .saturating_sub(effective_margin);
    app.full_content_width = app.content_width;
    LayoutResult {
        toc_area,
        content_area,
        status_area,
    }
}

pub fn draw(frame: &mut Frame, app: &mut App, img_mgr: &mut ImageManager) {
    let area = frame.area();
    let layout = calculate_layout(app, area);
    // ── Draw panels ─────────────────────────────────────────────────────────
    if let Some(ta) = layout.toc_area {
        draw_toc(frame, app, ta);
    }
    draw_content(frame, app, img_mgr, layout.content_area);
    draw_status_bar(frame, app, layout.status_area);

    // ── Overlays ────────────────────────────────────────────────────────────
    match app.mode {
        Mode::Help => draw_help_overlay(frame, app, area),
        Mode::Search => draw_search_bar(frame, app, area),
        Mode::ThemePicker => draw_theme_overlay(frame, app, area),
        Mode::Normal => {}
    }

    // ── Toast notification ─────────────────────────────────────────────────
    if let Some(ref toast) = app.toast {
        draw_toast(frame, toast, area);
    }

    // ── First-run hint ─────────────────────────────────────────────────────
    if app.first_run {
        draw_first_run_hint(frame, area);
    }
}

// ---------------------------------------------------------------------------
// TOC panel
// ---------------------------------------------------------------------------

// Highlight matching substring in a file name for directory search.
// Uses char-level matching to avoid unicode byte-offset mismatches.
fn highlight_file_name(
    indent: &str,
    name: &str,
    query_lower: &str,
    base_style: Style,
) -> Vec<Span<'static>> {
    let mut spans: Vec<Span<'static>> = vec![Span::styled(indent.to_string(), base_style)];
    let query_chars: Vec<char> = query_lower.chars().collect();
    if query_chars.is_empty() {
        spans.push(Span::styled(name.to_string(), base_style));
        return spans;
    }

    let name_chars: Vec<char> = name.chars().collect();
    let name_lower_chars: Vec<char> = name.to_lowercase().chars().collect();

    // Find all match ranges in char-index space
    let mut char_matches: Vec<(usize, usize)> = Vec::new();
    let mut i = 0;
    while i + query_chars.len() <= name_lower_chars.len() {
        if name_lower_chars[i..i + query_chars.len()] == query_chars[..] {
            char_matches.push((i, i + query_chars.len()));
            i += query_chars.len();
        } else {
            i += 1;
        }
    }

    // Build a char-index → byte-offset table for the original name
    let mut char_to_byte = Vec::with_capacity(name_chars.len() + 1);
    let mut byte_pos = 0;
    char_to_byte.push(byte_pos);
    for ch in &name_chars {
        byte_pos += ch.len_utf8();
        char_to_byte.push(byte_pos);
    }

    // Emit spans: unmatched (base_style) then matched (search_match)
    let mut last_byte = 0;
    for (start_char, end_char) in &char_matches {
        let start_byte = char_to_byte[*start_char];
        let end_byte = char_to_byte[*end_char];
        if start_byte > last_byte {
            spans.push(Span::styled(
                name[last_byte..start_byte].to_string(),
                base_style,
            ));
        }
        spans.push(Span::styled(
            name[start_byte..end_byte].to_string(),
            Theme::search_match(),
        ));
        last_byte = end_byte;
    }
    if last_byte < name.len() {
        spans.push(Span::styled(name[last_byte..].to_string(), base_style));
    }

    spans
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

    let (title, visible_items): (String, Vec<ListItem>) =
        if app.is_directory_mode() && app.dir_view == crate::app::DirView::FileList {
            // File list mode (with optional search filter)
            let filtered = (app.dir_search_active || app.mode == crate::app::Mode::Search)
                && !app.search_matches.is_empty();
            let total = if filtered {
                app.search_matches.len()
            } else {
                app.dir_files.len()
            };
            let title = if filtered {
                format!(" {} files (/{}) ", "\u{1f4c1}", app.search_query)
            } else {
                format!(" {} files ", "\u{1f4c1}")
            };
            let visible_start = app.dir_scroll;
            let visible_end =
                (visible_start + area.height.saturating_sub(BORDER_SIZE) as usize).min(total);
            let query_lower = app.search_query.to_lowercase();
            let items: Vec<ListItem> = (visible_start..visible_end)
                .map(|pos| {
                    let file_idx = if filtered {
                        app.search_matches[pos].line_idx
                    } else {
                        pos
                    };
                    let entry = &app.dir_files[file_idx];
                    let indent = "  ".repeat(entry.depth);
                    let base_style = if file_idx == app.dir_cursor && focused {
                        Theme::toc_selected()
                    } else {
                        Theme::text()
                    };
                    // Highlight matching substring in file name when filter is active
                    let spans = if filtered && !query_lower.is_empty() {
                        highlight_file_name(&indent, &entry.display_name, &query_lower, base_style)
                    } else {
                        vec![Span::styled(
                            format!("{}{}", indent, entry.display_name),
                            base_style,
                        )]
                    };
                    ListItem::new(Line::from(spans))
                })
                .collect();
            (title, items)
        } else {
            // Normal TOC mode (headings)
            let title = format!(" {} TOC ", "📑");
            let synced = app.synced_toc_index();
            let visible_start = app.toc_scroll;
            let visible_end = (visible_start + area.height.saturating_sub(BORDER_SIZE) as usize)
                .min(app.document.toc.len());
            let items: Vec<ListItem> = app
                .document
                .toc
                .iter()
                .enumerate()
                .skip(visible_start)
                .take(visible_end.saturating_sub(visible_start))
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
            (title, items)
        };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(border_style)
        .title(Span::styled(title, title_style));

    let inner = block.inner(area);
    frame.render_widget(block, area);

    let (cursor, scroll) =
        if app.is_directory_mode() && app.dir_view == crate::app::DirView::FileList {
            if app.dir_search_active && !app.search_matches.is_empty() {
                (app.dir_search_cursor, app.dir_scroll)
            } else {
                (app.dir_cursor, app.dir_scroll)
            }
        } else {
            (app.toc_cursor, app.toc_scroll)
        };

    let mut list_state = ListState::default();
    list_state.select(Some(cursor.saturating_sub(scroll)));

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

    // File list welcome message for directory mode
    if app.is_directory_mode() && app.dir_view == crate::app::DirView::FileList {
        let title = format!("  {} ", app.file_name);
        let block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(border_style)
            .title(Span::styled(title, title_style));

        let inner = block.inner(area);
        frame.render_widget(block, area);

        let file_count = app.dir_files.len();
        let lines = vec![
            Line::from(vec![Span::styled(
                "  📁 Directory Mode  ".to_string(),
                Theme::toc_title(),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  Select a file from the list to view its content.  ",
                Theme::text(),
            )]),
            Line::from(""),
            Line::from(vec![Span::styled(
                "  ↑/↓  Navigate files  ",
                Theme::subtext(),
            )]),
            Line::from(vec![Span::styled("  Enter  Open file  ", Theme::subtext())]),
            Line::from(vec![Span::styled(
                "  Esc  Return to file list  ",
                Theme::subtext(),
            )]),
            Line::from(vec![Span::styled("  q  Quit  ", Theme::subtext())]),
            Line::from(""),
            Line::from(vec![Span::styled(
                format!("  {} markdown file(s) found  ", file_count),
                Theme::bold(),
            )]),
        ];

        let paragraph = Paragraph::new(Text::from(lines))
            .wrap(Wrap { trim: false })
            .alignment(ratatui::layout::Alignment::Center);
        frame.render_widget(paragraph, inner);

        let scrollbar = Scrollbar::new(ScrollbarOrientation::VerticalRight);
        let mut scrollbar_state = ScrollbarState::new(0).position(0);
        frame.render_stateful_widget(scrollbar, area, &mut scrollbar_state);
        return;
    }

    let m = if app.search_matches.is_empty() {
        "".to_string()
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
        .horizontal_margin(get_content_margin() / 2)
        .areas(inner);

    let scroll = app.scroll;
    let height = inner_with_margin.height as usize;

    let current_match = if app.search_matches.is_empty() {
        None
    } else {
        Some(app.search_matches[app.search_current])
    };

    fn borrow_line<'a>(line: &'a Line<'static>) -> Line<'a> {
        let spans = line
            .spans
            .iter()
            .map(|s| Span {
                content: std::borrow::Cow::Borrowed(s.content.as_ref()),
                style: s.style,
            })
            .collect::<Vec<_>>();
        let mut l = Line::from(spans);
        l.alignment = line.alignment;
        l.style = line.style;
        l
    }

    let viewport_lines = app.get_viewport_lines();
    let viewport_offset = app.get_viewport_scroll();

    if app.search_query.is_empty() || viewport_lines.is_empty() {
        let visible_lines: Vec<Line<'_>> = if viewport_offset <= scroll {
            let start = scroll - viewport_offset;
            let end = (start + height).min(viewport_lines.len());
            if start < end {
                viewport_lines[start..end].iter().map(borrow_line).collect()
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        let paragraph = Paragraph::new(Text::from(visible_lines)).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, inner_with_margin);
    } else {
        let visible_slice: Vec<Line<'static>> = if viewport_offset <= scroll {
            let start = scroll - viewport_offset;
            let end = (start + height).min(viewport_lines.len());
            if start < end {
                viewport_lines[start..end].to_vec()
            } else {
                vec![]
            }
        } else {
            vec![]
        };
        let mut result = Vec::with_capacity(height);
        let q = app.search_query_norm.clone();
        let has_upper = app.search_has_upper;

        for (i, line) in visible_slice.into_iter().enumerate() {
            let line_idx = scroll + i;
            if let Some(cached) = app.get_cached_highlight(line_idx) {
                result.push(cached);
                continue;
            }
            let line_matches = app.rendered_line_matches(line_idx, &q, has_upper);
            if line_matches {
                let line_text = if has_upper {
                    app.rendered_line_text(line_idx)
                } else {
                    app.rendered_line_text_lower(line_idx)
                }
                .map(|s| s.to_string())
                .unwrap_or_default();
                let highlighted = apply_search_highlight(
                    vec![line.clone()],
                    &app.search_query_norm,
                    current_match,
                    line_idx,
                    Some(&[line_text.as_str()]),
                    0,
                );
                if let Some(hl_line) = highlighted.into_iter().next() {
                    let hl = hl_line.clone();
                    app.cache_highlight(line_idx, hl);
                    result.push(hl_line);
                } else {
                    result.push(line);
                }
            } else {
                result.push(line);
            }
        }
        let paragraph = Paragraph::new(Text::from(result)).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, inner_with_margin);
    }

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
    let mode_text = if app.is_directory_mode() {
        " DIR "
    } else {
        match app.mode {
            Mode::Normal => " NORMAL ",
            Mode::Search => " SEARCH ",
            Mode::Help => " HELP   ",
            Mode::ThemePicker => " THEME  ",
        }
    };

    let focus_text = match app.focus {
        Focus::Toc => "TOC",
        Focus::Content => "Content",
    };

    let scroll_pct = if app.total_lines() == 0 {
        100
    } else {
        ((app.scroll as u64 * 100) / (app.total_lines().max(1) as u64)).min(100) as usize
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
    let keys = config::keymap();
    let key_label = |binding: config::KeyBinding| -> String {
        let modifier_prefix = if binding.modifiers.contains(KeyModifiers::CONTROL) {
            "C-"
        } else if binding.modifiers.contains(KeyModifiers::ALT) {
            "M-"
        } else {
            ""
        };
        let key_str = match binding.code {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            KeyCode::Backspace => "BS".to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            KeyCode::PageUp => "PgUp".to_string(),
            KeyCode::PageDown => "PgDn".to_string(),
            KeyCode::Home => "Home".to_string(),
            KeyCode::End => "End".to_string(),
            _ => "?".to_string(),
        };
        format!("{}{}", modifier_prefix, key_str)
    };
    let keys_text = format!(
        " {}:{}  {}:{}  {}:{}  {}:{}  {}/{}:focus  {}/{}:scroll  {}:theme({}) ",
        key_label(keys.quit),
        "quit",
        key_label(keys.help),
        "help",
        key_label(keys.search),
        "search",
        key_label(keys.toggle_toc),
        "toc",
        key_label(keys.focus_prev),
        key_label(keys.focus_next),
        key_label(keys.up),
        key_label(keys.down),
        key_label(keys.next_theme),
        theme_name,
    );
    let right_text = format!(" {}% ", scroll_pct);
    let right = format!("{}{}", keys_text, right_text);

    // Pad to fill width
    let left_width: usize = left
        .spans
        .iter()
        .map(|s| crate::width::str_width(s.content.as_ref()))
        .sum();
    let pad =
        (area.width as usize).saturating_sub(left_width + crate::width::str_width(right.as_str()));

    // Re-build as styled line preserving original left spans
    let mut status_spans = left.spans;
    status_spans.push(Span::styled(" ".repeat(pad), Theme::statusbar()));
    status_spans.push(Span::styled(right, Theme::statusbar_dim()));
    let status_line = Line::from(status_spans);

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

fn draw_help_overlay(frame: &mut Frame, _app: &App, area: Rect) {
    let keys = config::keymap();
    let fmt = |binding: config::KeyBinding| -> String {
        let mods = binding.modifiers;
        let mut parts = Vec::new();
        if mods.contains(KeyModifiers::CONTROL) {
            parts.push("C");
        }
        if mods.contains(KeyModifiers::ALT) {
            parts.push("A");
        }
        let key = match binding.code {
            KeyCode::Char(c) => c.to_string(),
            KeyCode::Up => "↑".to_string(),
            KeyCode::Down => "↓".to_string(),
            KeyCode::Left => "←".to_string(),
            KeyCode::Right => "→".to_string(),
            KeyCode::Esc => "Esc".to_string(),
            KeyCode::Enter => "Enter".to_string(),
            KeyCode::Backspace => "Bksp".to_string(),
            KeyCode::Tab => "Tab".to_string(),
            _ => "?".to_string(),
        };
        if parts.is_empty() {
            key
        } else {
            format!("{}-{}", parts.join("-"), key)
        }
    };

    let items: Vec<(String, &str)> = vec![
        (fmt(keys.down), "Scroll down"),
        (fmt(keys.up), "Scroll up"),
        (fmt(keys.page_down), "Half-page down"),
        (fmt(keys.page_up), "Half-page up"),
        (fmt(keys.top), "Go to top"),
        (fmt(keys.bottom), "Go to bottom"),
        (fmt(keys.focus_prev), "Focus TOC"),
        (fmt(keys.focus_next), "Focus Content"),
        (fmt(keys.toc_down), "TOC down"),
        (fmt(keys.toc_up), "TOC up"),
        // Note: The following keybindings are currently hardcoded in main.rs and not configurable
        ("Enter".to_string(), "Jump to TOC entry"),
        ("]".to_string(), "Next heading"),
        ("[".to_string(), "Prev heading"),
        (fmt(keys.toggle_toc), "Toggle TOC"),
        ("< / >".to_string(), "Narrow/Widen TOC"),
        (fmt(keys.search), "Start search"),
        (fmt(keys.search_next), "Next search match"),
        (fmt(keys.search_prev), "Prev search match"),
        (fmt(keys.next_theme), "Theme picker"),
        (fmt(keys.help), "Toggle help"),
        (fmt(keys.quit), "Quit"),
        ("Esc".to_string(), "Cancel / Close"),
    ];

    let max_key_width = items
        .iter()
        .map(|(k, _)| crate::width::str_width(k))
        .max()
        .unwrap_or(10);
    let col_width = max_key_width + 4;
    let half = items.len().div_ceil(2);
    let left_items = &items[..half];
    let right_items = &items[half..];

    let help_height = 4 + half.max(right_items.len()) as u16;
    let help_width = ((col_width * 2 + 12) as u16)
        .min(area.width.saturating_sub(4))
        .max(50);
    let x = area.width.saturating_sub(help_width) / 2;
    let y = area.height.saturating_sub(help_height) / 2;
    let help_area = Rect {
        x,
        y,
        width: help_width,
        height: help_height,
    };

    frame.render_widget(Clear, help_area);

    let build_list = |items: &[(String, &str)]| -> Vec<Line> {
        items
            .iter()
            .map(|(key, desc)| {
                Line::from(vec![
                    Span::styled(
                        format!("{:>width$}  ", key, width = max_key_width),
                        Theme::statusbar_key(),
                    ),
                    Span::styled(desc.to_string(), Theme::text()),
                ])
            })
            .collect()
    };

    let left_lines = build_list(left_items);
    let right_lines = build_list(right_items);

    let combined: Vec<Line> = std::iter::once(Line::from(Span::styled(
        " Keybindings ",
        Theme::toc_title(),
    )))
    .chain(std::iter::once(Line::default()))
    .chain(
        left_lines
            .into_iter()
            .zip(
                right_lines
                    .into_iter()
                    .chain(std::iter::repeat(Line::default())),
            )
            .map(|(left, right)| {
                let pad = " ".repeat(2);
                let mut spans = left.spans;
                spans.push(Span::raw(pad));
                spans.extend(right.spans);
                Line::from(spans)
            }),
    )
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

    let para = Paragraph::new(Text::from(combined))
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
    let min_width = crate::width::str_width(&helper_text) as u16 + 4;
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

// ---------------------------------------------------------------------------
// Toast notification
// ---------------------------------------------------------------------------

fn draw_toast(frame: &mut Frame, toast: &crate::app::Toast, area: Rect) {
    let width = (crate::width::str_width(&toast.message) as u16 + 6)
        .min(area.width.saturating_sub(4))
        .max(20);
    let height = 3;
    let x = (area.width.saturating_sub(width)) / 2;
    let y = area.height.saturating_sub(height + 3).max(area.y);
    let toast_area = Rect {
        x,
        y,
        width: width.min(area.width),
        height: height.min(area.height),
    };

    frame.render_widget(Clear, toast_area);

    let block = Block::default()
        .borders(Borders::ALL)
        .border_type(BorderType::Rounded)
        .border_style(Theme::toc_border_focused());

    let para = Paragraph::new(Text::from(vec![Line::from(Span::styled(
        &toast.message,
        Theme::toc_selected(),
    ))]))
    .block(block)
    .alignment(ratatui::layout::Alignment::Center);

    frame.render_widget(para, toast_area);
}

// ---------------------------------------------------------------------------
// First-run hint
// ---------------------------------------------------------------------------

fn draw_first_run_hint(frame: &mut Frame, area: Rect) {
    let hint = "? for help";
    let width = crate::width::str_width(hint) as u16 + 4;
    let height = 1;
    let x = area.width.saturating_sub(width + 1);
    let y = area.height.saturating_sub(height + 2);
    let hint_area = Rect {
        x,
        y,
        width,
        height,
    };

    let para = Paragraph::new(Text::from(Span::styled(hint, Theme::subtext())));
    frame.render_widget(para, hint_area);
}
