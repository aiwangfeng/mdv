// src/app.rs
// Central application state + event dispatch

use ratatui::text::Line;
use std::collections::HashMap;
use std::path::PathBuf;
use std::time::{Duration, Instant};

use crate::parser::Document;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Focus {
    Toc,
    Content,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Mode {
    Normal,
    Search,
    Help,
    ThemePicker,
}

#[derive(Debug)]
pub struct Toast {
    pub message: String,
    pub created_at: Instant,
    pub duration: Duration,
}

impl Toast {
    pub fn new(message: &str, duration_ms: u64) -> Self {
        Self {
            message: message.to_string(),
            created_at: Instant::now(),
            duration: Duration::from_millis(duration_ms),
        }
    }

    pub fn is_expired(&self) -> bool {
        self.created_at.elapsed() > self.duration
    }
}

#[derive(Debug)]
pub struct App {
    pub file_path: PathBuf,
    pub file_name: String,

    pub document: Document,
    pub rendered_lines: Vec<Line<'static>>,
    rendered_texts_lower: HashMap<usize, String>,
    pub image_positions: Vec<(usize, String, String)>,
    pub toc_line_indices: Vec<usize>,

    pub toc_width_pct: u16,
    pub show_toc: bool,

    pub focus: Focus,

    pub scroll: usize,
    pub content_height: u16,
    pub content_width: u16,

    pub toc_cursor: usize,
    pub toc_scroll: usize,
    pub toc_height: u16,

    pub mode: Mode,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub search_current: usize,
    search_highlight_cache: HashMap<usize, Line<'static>>,
    cached_search_query: Option<String>,
    pub theme_picker_index: usize,
    theme_picker_origin: Option<usize>,

    pub toast: Option<Toast>,
    pub first_run: bool,

    pub quit: bool,
}

impl App {
    pub fn new(
        file_path: PathBuf,
        document: Document,
        rendered_lines: Vec<Line<'static>>,
        image_positions: Vec<(usize, String, String)>,
        toc_line_indices: Vec<usize>,
    ) -> Self {
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "mdv".to_string());

        Self {
            file_path,
            file_name,
            document,
            rendered_lines,
            rendered_texts_lower: HashMap::new(),
            image_positions,
            toc_line_indices,
            toc_width_pct: 25,
            show_toc: true,
            focus: Focus::Content,
            scroll: 0,
            content_height: 0,
            content_width: 0,
            toc_cursor: 0,
            toc_scroll: 0,
            toc_height: 0,
            mode: Mode::Normal,
            search_query: String::new(),
            search_matches: vec![],
            search_current: 0,
            search_highlight_cache: HashMap::new(),
            cached_search_query: None,
            theme_picker_index: 0,
            theme_picker_origin: None,
            toast: None,
            first_run: true,
            quit: false,
        }
    }

    fn line_lower(&mut self, idx: usize) -> String {
        if let Some(s) = self.rendered_texts_lower.get(&idx) {
            return s.clone();
        }
        let s = self.rendered_lines[idx]
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect::<String>()
            .to_lowercase();
        self.rendered_texts_lower.insert(idx, s.clone());
        s
    }

    pub fn line_lower_ref(&self, idx: usize) -> Option<String> {
        self.rendered_texts_lower.get(&idx).cloned()
    }

    // ---------------------------------------------------------------------------
    // Content scrolling
    // ---------------------------------------------------------------------------

    pub fn total_lines(&self) -> usize {
        self.rendered_lines.len()
    }

    pub fn max_scroll(&self) -> usize {
        self.total_lines()
            .saturating_sub(self.content_height as usize)
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll = (self.scroll + n).min(self.max_scroll());
        self.sync_toc_to_scroll();
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
        self.sync_toc_to_scroll();
    }

    pub fn scroll_to(&mut self, line: usize) {
        self.scroll = line.min(self.max_scroll());
        self.sync_toc_to_scroll();
    }

    pub fn half_page(&self) -> usize {
        (self.content_height as usize / 2).max(1)
    }

    pub fn scroll_top(&mut self) {
        self.scroll = 0;
        self.sync_toc_to_scroll();
    }

    pub fn scroll_bottom(&mut self) {
        self.scroll = self.max_scroll();
        self.sync_toc_to_scroll();
    }

    // ---------------------------------------------------------------------------
    // TOC navigation
    // ---------------------------------------------------------------------------

    pub fn toc_len(&self) -> usize {
        self.document.toc.len()
    }

    pub fn toc_down(&mut self) {
        if self.toc_cursor + 1 < self.toc_len() {
            self.toc_cursor += 1;
            self.ensure_toc_cursor_visible();
        }
    }

    pub fn toc_up(&mut self) {
        if self.toc_cursor > 0 {
            self.toc_cursor -= 1;
            self.ensure_toc_cursor_visible();
        }
    }

    pub fn toc_jump_to_cursor(&mut self) {
        if self.toc_line_indices.is_empty() || self.toc_cursor >= self.toc_line_indices.len() {
            return;
        }
        let line = self.toc_line_indices[self.toc_cursor];
        self.scroll_to(line);
    }

    fn ensure_toc_cursor_visible(&mut self) {
        let h = self.toc_height as usize;
        if h == 0 {
            return;
        }
        if self.toc_cursor < self.toc_scroll {
            self.toc_scroll = self.toc_cursor;
        } else if self.toc_cursor >= self.toc_scroll + h {
            self.toc_scroll = self.toc_cursor - h + 1;
        }
    }

    fn sync_toc_to_scroll(&mut self) {
        if self.toc_line_indices.is_empty() {
            return;
        }

        let idx = match self.toc_line_indices.binary_search(&self.scroll) {
            Ok(i) => i,
            Err(i) => i.saturating_sub(1),
        };

        if idx != self.toc_cursor {
            self.toc_cursor = idx;
            self.ensure_toc_cursor_visible();
        }
    }

    // ---------------------------------------------------------------------------
    // Heading navigation
    // ---------------------------------------------------------------------------

    /// Jump to the next heading below current scroll
    pub fn next_heading(&mut self) {
        if let Some(&line) = self
            .toc_line_indices
            .iter()
            .find(|&&line| line > self.scroll)
        {
            self.scroll_to(line);
        }
    }

    /// Jump to the previous heading above current scroll
    pub fn prev_heading(&mut self) {
        if let Some(&line) = self
            .toc_line_indices
            .iter()
            .rev()
            .find(|&&line| line < self.scroll)
        {
            self.scroll_to(line);
        }
    }

    // ---------------------------------------------------------------------------
    // Search
    // ---------------------------------------------------------------------------

    pub fn start_search(&mut self) {
        self.mode = Mode::Search;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_current = 0;
        self.invalidate_search_cache();
    }

    pub fn search_push(&mut self, ch: char) {
        self.search_query.push(ch);
        self.run_search();
    }

    pub fn search_pop(&mut self) {
        self.search_query.pop();
        self.run_search();
    }

    pub fn search_confirm(&mut self) {
        self.mode = Mode::Normal;
        self.jump_to_search_current();
    }

    pub fn search_cancel(&mut self) {
        self.mode = Mode::Normal;
        self.search_query.clear();
        self.search_matches.clear();
        self.invalidate_search_cache();
    }

    fn run_search(&mut self) {
        if self.search_query.is_empty() {
            self.search_matches.clear();
            return;
        }
        let q = self.search_query.to_lowercase();
        let total = self.rendered_lines.len();
        let mut matches = Vec::new();
        for i in 0..total {
            if self.line_lower(i).contains(&q) {
                matches.push(i);
            }
        }
        self.search_matches = matches;
        // Set current to the first match at or below current scroll
        self.search_current = self
            .search_matches
            .iter()
            .position(|&l| l >= self.scroll)
            .unwrap_or(0);
        self.jump_to_search_current();
    }

    pub fn search_next(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.search_current = (self.search_current + 1) % self.search_matches.len();
        self.jump_to_search_current();
    }

    pub fn search_prev(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if self.search_current == 0 {
            self.search_current = self.search_matches.len() - 1;
        } else {
            self.search_current -= 1;
        }
        self.jump_to_search_current();
    }

    fn jump_to_search_current(&mut self) {
        if let Some(&line) = self.search_matches.get(self.search_current) {
            self.scroll_to(line);
        }
    }

    pub fn get_cached_highlight(&self, line_idx: usize) -> Option<&Line<'static>> {
        if self.cached_search_query.as_ref() != Some(&self.search_query) {
            return None;
        }
        self.search_highlight_cache.get(&line_idx)
    }

    pub fn cache_highlight(&mut self, line_idx: usize, line: Line<'static>) {
        self.cached_search_query = Some(self.search_query.clone());
        self.search_highlight_cache.insert(line_idx, line);
    }

    pub fn invalidate_search_cache(&mut self) {
        self.search_highlight_cache.clear();
        self.cached_search_query = None;
    }

    // ---------------------------------------------------------------------------
    // Layout helpers
    // ---------------------------------------------------------------------------

    pub fn show_toast(&mut self, message: &str) {
        self.toast = Some(Toast::new(message, 1500));
    }

    pub fn tick_toast(&mut self) {
        if let Some(ref toast) = self.toast {
            if toast.is_expired() {
                self.toast = None;
            }
        }
    }

    pub fn toggle_toc(&mut self) {
        self.show_toc = !self.show_toc;
        let status = if self.show_toc {
            "TOC shown"
        } else {
            "TOC hidden"
        };
        self.show_toast(status);
    }

    pub fn widen_toc(&mut self) {
        self.toc_width_pct = (self.toc_width_pct + 2).min(50);
        self.show_toast(&format!("TOC width: {}%", self.toc_width_pct));
    }

    pub fn narrow_toc(&mut self) {
        self.toc_width_pct = (self.toc_width_pct.saturating_sub(2)).max(10);
        self.show_toast(&format!("TOC width: {}%", self.toc_width_pct));
    }

    pub fn toggle_focus(&mut self) {
        self.focus = match self.focus {
            Focus::Content => Focus::Toc,
            Focus::Toc => Focus::Content,
        };
    }

    pub fn toggle_help(&mut self) {
        self.mode = match self.mode {
            Mode::Help => Mode::Normal,
            _ => Mode::Help,
        };
    }

    pub fn open_theme_picker(&mut self, current_index: usize) {
        self.theme_picker_index = current_index;
        self.theme_picker_origin = Some(current_index);
        self.mode = Mode::ThemePicker;
    }

    pub fn move_theme_picker(&mut self, delta: isize, total_themes: usize) -> Option<usize> {
        if total_themes == 0 {
            return None;
        }

        let len = total_themes as isize;
        let next = (self.theme_picker_index as isize + delta).rem_euclid(len) as usize;
        self.theme_picker_index = next;
        Some(next)
    }

    pub fn confirm_theme_picker(&mut self) {
        self.theme_picker_origin = None;
        self.mode = Mode::Normal;
    }

    pub fn cancel_theme_picker(&mut self) -> Option<usize> {
        self.mode = Mode::Normal;
        self.theme_picker_origin.take()
    }

    // ---------------------------------------------------------------------------
    // Synced TOC entry for current scroll
    // ---------------------------------------------------------------------------
    pub fn synced_toc_index(&self) -> Option<usize> {
        self.toc_line_indices
            .iter()
            .enumerate()
            .rev()
            .find(|(_, line)| **line <= self.scroll)
            .map(|(i, _)| i)
    }

    pub fn update_render(
        &mut self,
        rendered_lines: Vec<Line<'static>>,
        image_positions: Vec<(usize, String, String)>,
        node_line_starts: &[usize],
    ) {
        self.rendered_texts_lower.clear();
        self.rendered_lines = rendered_lines;
        self.image_positions = image_positions;
        self.toc_line_indices = self
            .document
            .toc
            .iter()
            .map(|entry| node_line_starts.get(entry.node_index).copied().unwrap_or(0))
            .collect();
        self.scroll = self.scroll.min(self.max_scroll());
        self.invalidate_search_cache();
        self.sync_toc_to_scroll();
    }
}

#[cfg(test)]
mod tests {
    use super::App;
    use crate::parser::{DocNode, Document, TocEntry};
    use ratatui::text::Line;
    use std::path::PathBuf;

    #[test]
    fn update_render_maps_toc_entries_to_rendered_lines() {
        let document = Document {
            nodes: vec![
                DocNode::Heading {
                    level: 1,
                    text: "A".to_string(),
                },
                DocNode::Blank,
                DocNode::Heading {
                    level: 2,
                    text: "B".to_string(),
                },
            ],
            toc: vec![
                TocEntry {
                    level: 1,
                    title: "A".to_string(),
                    node_index: 0,
                },
                TocEntry {
                    level: 2,
                    title: "B".to_string(),
                    node_index: 2,
                },
            ],
        };

        let mut app = App::new(
            PathBuf::from("doc.md"),
            document,
            vec![Line::default()],
            vec![],
            vec![0, 0],
        );
        app.content_height = 3;
        app.update_render(vec![Line::default(); 8], vec![], &[0, 1, 5]);

        assert_eq!(app.toc_line_indices, vec![0, 5]);
        app.scroll_to(5);
        assert_eq!(app.synced_toc_index(), Some(1));
    }
}
