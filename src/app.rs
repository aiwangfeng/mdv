// src/app.rs
// Central application state + event dispatch

use ratatui::text::Line;
use std::collections::{HashMap, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, Instant};

use crate::config;
use crate::parser::Document;
use crate::renderer;

const SEARCH_DEBOUNCE_MS: u64 = 75;
const MAX_SEARCH_HIGHLIGHT_CACHE: usize = 256;
const MAX_FILE_CACHE: usize = 32;

/// Number of extra lines rendered above & below the viewport for smooth scrolling.
const RENDER_BUFFER_LINES: usize = 50;

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

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DirView {
    FileList,
    FileView,
}

#[derive(Debug, Default)]
pub struct CachedDocument {
    pub document: Arc<Document>,
    pub node_heights: Vec<usize>,
    pub node_line_starts: Vec<usize>,
    pub total_lines: usize,
    pub toc_line_indices: Vec<usize>,
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

    pub document: Arc<Document>,
    pub node_heights: Vec<usize>,
    pub node_line_starts: Vec<usize>,
    pub total_content_lines: usize,
    pub raw_lines: Vec<String>,
    pub image_positions: Vec<(usize, String, String)>,
    pub toc_line_indices: Vec<usize>,

    /// Cache of the currently visible viewport lines.
    viewport_scroll: usize,
    viewport_lines: Vec<Line<'static>>,
    viewport_dirty: bool,

    /// Full-document rendered line texts (lowercased) for search.
    full_rendered_texts: Vec<String>,
    full_render_width: u16,

    pub content_height: u16,
    pub content_width: u16,
    pub full_content_width: u16,

    pub toc_width_pct: u16,
    pub show_toc: bool,

    pub focus: Focus,

    pub scroll: usize,

    pub toc_cursor: usize,
    pub toc_scroll: usize,
    pub toc_height: u16,

    pub mode: Mode,
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub search_current: usize,
    search_highlight_cache: HashMap<usize, Line<'static>>,
    search_highlight_order: VecDeque<usize>,
    cached_search_query: Option<String>,
    search_dirty: bool,
    search_deadline: Option<Instant>,
    pub theme_picker_index: usize,
    theme_picker_origin: Option<usize>,

    pub toast: Option<Toast>,
    pub first_run: bool,

    pub quit: bool,

    // Directory mode
    pub directory_mode: bool,
    pub dir_view: DirView,
    pub dir_files: Vec<crate::dir::DirEntry>,
    pub dir_base: PathBuf,
    pub dir_cursor: usize,
    pub dir_scroll: usize,
    pub current_file_index: Option<usize>,
    pub file_cache: HashMap<usize, CachedDocument>,
    file_cache_order: VecDeque<usize>,
    pub scroll_positions: HashMap<usize, usize>,
}

impl App {
    pub fn new(
        file_path: PathBuf,
        document: Document,
        raw_lines: Vec<String>,
        node_heights: Vec<usize>,
        node_line_starts: Vec<usize>,
        total_content_lines: usize,
        image_positions: Vec<(usize, String, String)>,
        toc_line_indices: Vec<usize>,
    ) -> Self {
        let file_name = file_path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "mdv".to_string());

        let cfg = config::get();
        Self {
            file_path,
            file_name,
            document: Arc::new(document),
            node_heights,
            node_line_starts,
            total_content_lines,
            raw_lines,
            image_positions,
            toc_line_indices,
            viewport_scroll: 0,
            viewport_lines: Vec::new(),
            viewport_dirty: true,
            full_rendered_texts: Vec::new(),
            full_render_width: 0,
            content_height: 0,
            content_width: 0,
            full_content_width: 0,
            toc_width_pct: cfg.toc_width_pct,
            show_toc: true,
            focus: Focus::Content,
            scroll: 0,
            toc_cursor: 0,
            toc_scroll: 0,
            toc_height: 0,
            mode: Mode::Normal,
            search_query: String::new(),
            search_matches: vec![],
            search_current: 0,
            search_highlight_cache: HashMap::new(),
            search_highlight_order: VecDeque::new(),
            cached_search_query: None,
            search_dirty: false,
            search_deadline: None,
            theme_picker_index: 0,
            theme_picker_origin: None,
            toast: None,
            first_run: true,
            quit: false,
            directory_mode: false,
            dir_view: DirView::FileView,
            dir_files: vec![],
            dir_base: PathBuf::new(),
            dir_cursor: 0,
            dir_scroll: 0,
            current_file_index: None,
            file_cache: HashMap::new(),
            file_cache_order: VecDeque::new(),
            scroll_positions: HashMap::new(),
        }
    }

    pub fn new_directory_mode(dir_base: PathBuf, dir_files: Vec<crate::dir::DirEntry>) -> Self {
        let cfg = config::get();
        Self {
            file_path: PathBuf::new(),
            file_name: dir_base
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "mdv".to_string()),
            document: Arc::new(Document::default()),
            node_heights: vec![],
            node_line_starts: vec![],
            total_content_lines: 0,
            raw_lines: vec![],
            image_positions: vec![],
            toc_line_indices: vec![],
            viewport_scroll: 0,
            viewport_lines: Vec::new(),
            viewport_dirty: true,
            full_rendered_texts: Vec::new(),
            full_render_width: 0,
            toc_width_pct: cfg.toc_width_pct,
            show_toc: true,
            focus: Focus::Toc,
            scroll: 0,
            content_height: 0,
            content_width: 0,
            full_content_width: 0,
            toc_cursor: 0,
            toc_scroll: 0,
            toc_height: 0,
            mode: Mode::Normal,
            search_query: String::new(),
            search_matches: vec![],
            search_current: 0,
            search_highlight_cache: HashMap::new(),
            search_highlight_order: VecDeque::new(),
            cached_search_query: None,
            search_dirty: false,
            search_deadline: None,
            theme_picker_index: 0,
            theme_picker_origin: None,
            toast: None,
            first_run: false,
            quit: false,
            directory_mode: true,
            dir_view: DirView::FileList,
            dir_files,
            dir_base,
            dir_cursor: 0,
            dir_scroll: 0,
            current_file_index: None,
            file_cache: HashMap::new(),
            file_cache_order: VecDeque::new(),
            scroll_positions: HashMap::new(),
        }
    }

    /// Ensure the viewport around the current scroll position is rendered and cached.
    /// Returns true if the viewport was re-rendered.
    pub fn ensure_viewport_rendered(&mut self, width: u16, full_width: u16) -> bool {
        if !self.viewport_dirty && self.viewport_scroll == self.scroll && !self.viewport_lines.is_empty()
        {
            return false;
        }

        let viewport_end = self.scroll + self.content_height as usize + RENDER_BUFFER_LINES;
        let scroll_start = self.scroll.saturating_sub(RENDER_BUFFER_LINES);

        let result = renderer::render_viewport(
            &self.document.nodes,
            &self.node_line_starts,
            scroll_start,
            viewport_end,
            width,
            full_width,
        );

        self.viewport_lines = result.lines;
        self.image_positions = result.image_positions;
        self.viewport_scroll = scroll_start;
        self.viewport_dirty = false;
        true
    }

    pub fn get_viewport_lines(&self) -> &[Line<'static>] {
        &self.viewport_lines
    }

    pub fn get_viewport_scroll(&self) -> usize {
        self.viewport_scroll
    }

    pub fn mark_viewport_dirty(&mut self) {
        self.viewport_dirty = true;
        self.full_rendered_texts.clear();
    }

    /// Ensure full rendered line texts are available for search.
    fn ensure_full_rendered_texts(&mut self) {
        if !self.full_rendered_texts.is_empty()
            && self.full_render_width == self.content_width
        {
            return;
        }
        let result = renderer::render_nodes(
            &self.document.nodes,
            self.content_width,
            self.full_content_width,
        );
        self.full_rendered_texts = result
            .lines
            .iter()
            .map(|line| {
                line.spans
                    .iter()
                    .map(|s| s.content.as_ref())
                    .collect::<String>()
                    .to_lowercase()
            })
            .collect();
        self.full_render_width = self.content_width;
    }

    /// Check if a rendered line at `line_idx` contains the query (case-insensitive).
    pub fn rendered_line_matches(&mut self, line_idx: usize, query_lower: &str) -> bool {
        self.ensure_full_rendered_texts();
        self.full_rendered_texts
            .get(line_idx)
            .is_some_and(|text| text.contains(query_lower))
    }

    /// Get the lowercased rendered text for a line, for search highlighting.
    pub fn rendered_line_text_lower(&mut self, line_idx: usize) -> Option<&str> {
        self.ensure_full_rendered_texts();
        self.full_rendered_texts.get(line_idx).map(|s| s.as_str())
    }

    // ---------------------------------------------------------------------------
    // Content scrolling
    // ---------------------------------------------------------------------------

    pub fn total_lines(&self) -> usize {
        self.total_content_lines
    }

    pub fn max_scroll(&self) -> usize {
        self.total_lines()
            .saturating_sub(self.content_height as usize)
    }

    pub fn scroll_down(&mut self, n: usize) {
        self.scroll = (self.scroll + n).min(self.max_scroll());
        self.mark_viewport_dirty();
        self.sync_toc_to_scroll();
    }

    pub fn scroll_up(&mut self, n: usize) {
        self.scroll = self.scroll.saturating_sub(n);
        self.mark_viewport_dirty();
        self.sync_toc_to_scroll();
    }

    pub fn scroll_to(&mut self, line: usize) {
        self.scroll = line.min(self.max_scroll());
        self.mark_viewport_dirty();
        self.sync_toc_to_scroll();
    }

    pub fn half_page(&self) -> usize {
        (self.content_height as usize / 2).max(1)
    }

    pub fn scroll_top(&mut self) {
        self.scroll = 0;
        self.mark_viewport_dirty();
        self.sync_toc_to_scroll();
    }

    pub fn scroll_bottom(&mut self) {
        self.scroll = self.max_scroll();
        self.mark_viewport_dirty();
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
        let idx = self
            .toc_line_indices
            .partition_point(|&line| line <= self.scroll);
        if let Some(&line) = self.toc_line_indices.get(idx) {
            self.scroll_to(line);
        }
    }

    /// Jump to the previous heading above current scroll
    pub fn prev_heading(&mut self) {
        let idx = self
            .toc_line_indices
            .partition_point(|&line| line < self.scroll);
        if let Some(&line) = idx
            .checked_sub(1)
            .and_then(|i| self.toc_line_indices.get(i))
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
        self.search_dirty = false;
        self.search_deadline = None;
        self.invalidate_search_cache();
    }

    pub fn search_push(&mut self, ch: char) {
        self.search_query.push(ch);
        self.schedule_search();
    }

    pub fn search_pop(&mut self) {
        self.search_query.pop();
        self.schedule_search();
    }

    pub fn search_confirm(&mut self) {
        self.flush_search();
        self.mode = Mode::Normal;
        self.jump_to_search_current();
    }

    pub fn search_cancel(&mut self) {
        self.mode = Mode::Normal;
        self.search_query.clear();
        self.search_matches.clear();
        self.search_dirty = false;
        self.search_deadline = None;
        self.invalidate_search_cache();
    }

    fn schedule_search(&mut self) {
        self.search_dirty = true;
        self.search_deadline = Some(Instant::now() + Duration::from_millis(SEARCH_DEBOUNCE_MS));
        self.invalidate_search_cache();
    }

    fn flush_search(&mut self) {
        if self.search_dirty {
            self.run_search();
        }
    }

    pub fn run_pending_search(&mut self) -> bool {
        if self.search_dirty
            && self
                .search_deadline
                .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.run_search();
            return true;
        }
        false
    }

    fn run_search(&mut self) {
        self.search_dirty = false;
        self.search_deadline = None;
        if self.search_query.is_empty() {
            self.search_matches.clear();
            self.search_current = 0;
            return;
        }
        self.ensure_full_rendered_texts();
        let q = self.search_query.to_lowercase();
        let mut matches = Vec::new();
        for (i, text) in self.full_rendered_texts.iter().enumerate() {
            if text.contains(&q) {
                matches.push(i);
            }
        }
        self.search_matches = matches;
        self.search_current = self
            .search_matches
            .iter()
            .position(|&l| l >= self.scroll)
            .unwrap_or(0);
        self.jump_to_search_current();
    }

    pub fn search_next(&mut self) {
        self.flush_search();
        if self.search_matches.is_empty() {
            return;
        }
        self.search_current = (self.search_current + 1) % self.search_matches.len();
        self.jump_to_search_current();
    }

    pub fn search_prev(&mut self) {
        self.flush_search();
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
        if !self.search_highlight_cache.contains_key(&line_idx) {
            self.search_highlight_order.push_back(line_idx);
            while self.search_highlight_order.len() > MAX_SEARCH_HIGHLIGHT_CACHE {
                if let Some(evicted) = self.search_highlight_order.pop_front() {
                    self.search_highlight_cache.remove(&evicted);
                }
            }
        }
        self.search_highlight_cache.insert(line_idx, line);
    }

    pub fn invalidate_search_cache(&mut self) {
        self.search_highlight_cache.clear();
        self.search_highlight_order.clear();
        self.cached_search_query = None;
    }

    // ---------------------------------------------------------------------------
    // Layout helpers
    // ---------------------------------------------------------------------------

    pub fn show_toast(&mut self, message: &str) {
        self.toast = Some(Toast::new(message, 1500));
    }

    pub fn has_active_toast(&self) -> bool {
        self.toast.is_some()
    }

    pub fn tick_toast(&mut self) -> bool {
        if let Some(ref toast) = self.toast {
            if toast.is_expired() {
                self.toast = None;
                return true;
            }
        }
        false
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
        if self.toc_line_indices.is_empty() {
            return None;
        }
        let idx = self
            .toc_line_indices
            .partition_point(|&line| line <= self.scroll);
        idx.checked_sub(1)
    }

    #[allow(dead_code)]
    pub fn update_render(
        &mut self,
        rendered_lines: Vec<Line<'static>>,
        image_positions: Vec<(usize, String, String)>,
        node_line_starts: &[usize],
    ) {
        self.viewport_lines = rendered_lines;
        self.image_positions = image_positions;
        self.viewport_scroll = self.scroll;
        self.viewport_dirty = false;
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

    fn touch_file_cache(&mut self, file_index: usize) {
        self.file_cache_order.retain(|&idx| idx != file_index);
        self.file_cache_order.push_back(file_index);
    }

    fn insert_file_cache(&mut self, file_index: usize, cached: CachedDocument) {
        self.file_cache.insert(file_index, cached);
        self.touch_file_cache(file_index);
        while self.file_cache.len() > MAX_FILE_CACHE {
            if let Some(evicted) = self.file_cache_order.pop_front() {
                if self.current_file_index == Some(evicted) {
                    self.file_cache_order.push_back(evicted);
                    continue;
                }
                self.file_cache.remove(&evicted);
                self.scroll_positions.remove(&evicted);
                break;
            }
        }
    }

    #[allow(dead_code)]
    fn sync_current_file_cache(&mut self) {
        if !self.directory_mode {
            return;
        }
        if let Some(file_index) = self.current_file_index {
            let cached = CachedDocument {
                document: Arc::clone(&self.document),
                node_heights: self.node_heights.clone(),
                node_line_starts: self.node_line_starts.clone(),
                total_lines: self.total_content_lines,
                toc_line_indices: self.toc_line_indices.clone(),
            };
            self.insert_file_cache(file_index, cached);
        }
    }

    // ---------------------------------------------------------------------------
    // Directory mode methods
    // ---------------------------------------------------------------------------

    pub fn is_directory_mode(&self) -> bool {
        self.directory_mode
    }

    pub fn dir_up(&mut self) {
        if !self.directory_mode || self.dir_files.is_empty() {
            return;
        }
        self.dir_cursor = self.dir_cursor.saturating_sub(1);
        if self.dir_cursor < self.dir_scroll {
            self.dir_scroll = self.dir_cursor;
        }
    }

    pub fn dir_down(&mut self) {
        if !self.directory_mode || self.dir_files.is_empty() {
            return;
        }
        self.dir_cursor = (self.dir_cursor + 1).min(self.dir_files.len() - 1);
        let visible = self.toc_height as usize;
        if self.dir_cursor >= self.dir_scroll + visible {
            self.dir_scroll = self.dir_cursor.saturating_sub(visible - 1);
        }
    }

    pub fn dir_top(&mut self) {
        self.dir_cursor = 0;
        self.dir_scroll = 0;
    }

    pub fn dir_bottom(&mut self) {
        if self.dir_files.is_empty() {
            return;
        }
        self.dir_cursor = self.dir_files.len() - 1;
        let visible = self.toc_height as usize;
        self.dir_scroll = self.dir_cursor.saturating_sub(visible - 1);
    }

    pub fn open_file_from_dir(
        &mut self,
        file_index: usize,
        _base_dir: &Path,
        img_mgr: &mut crate::image_proto::ImageManager,
    ) -> bool {
        if file_index >= self.dir_files.len() {
            return false;
        }

        let path = self.dir_files[file_index].path.clone();
        let display_name = self.dir_files[file_index].display_name.clone();

        if let Some(cached) = self.file_cache.get(&file_index) {
            self.document = Arc::clone(&cached.document);
            self.node_heights = cached.node_heights.clone();
            self.node_line_starts = cached.node_line_starts.clone();
            self.total_content_lines = cached.total_lines;
            self.toc_line_indices = cached.toc_line_indices.clone();
            self.touch_file_cache(file_index);
        } else {
            let markdown = match std::fs::read_to_string(&path) {
                Ok(m) => m,
                Err(e) => {
                    log::warn!("Failed to read {}: {}", path.display(), e);
                    return false;
                }
            };

            let document = crate::parser::parse(&markdown);
            let raw_lines: Vec<String> = markdown.lines().map(|l| l.to_string()).collect();
            let node_heights = renderer::measure_nodes(&document.nodes, self.content_width);
            let node_line_starts = renderer::compute_line_starts(&node_heights);
            let total_content_lines = node_line_starts
                .last()
                .and_then(|&last| {
                    node_heights.last().map(|&h| last + h)
                })
                .unwrap_or(0);
            let toc_line_indices: Vec<usize> = document
                .toc
                .iter()
                .map(|e| {
                    node_line_starts
                        .get(e.node_index)
                        .copied()
                        .unwrap_or(0)
                })
                .collect();
            self.document = Arc::new(document);
            self.node_heights = node_heights;
            self.node_line_starts = node_line_starts;
            self.total_content_lines = total_content_lines;
            self.raw_lines = raw_lines;
            self.toc_line_indices = toc_line_indices.clone();

            self.insert_file_cache(
                file_index,
                CachedDocument {
                    document: Arc::clone(&self.document),
                    node_heights: self.node_heights.clone(),
                    node_line_starts: self.node_line_starts.clone(),
                    total_lines: self.total_content_lines,
                    toc_line_indices,
                },
            );
        }

        self.image_positions = vec![];
        self.search_dirty = false;
        self.search_deadline = None;
        self.mark_viewport_dirty();

        self.file_path = path;
        self.file_name = display_name;
        let _ = img_mgr;

        if let Some(&scroll) = self.scroll_positions.get(&file_index) {
            self.scroll = scroll.min(self.max_scroll());
        } else {
            self.scroll = 0;
        }

        self.current_file_index = Some(file_index);
        self.dir_view = DirView::FileView;
        self.focus = Focus::Content;
        self.invalidate_search_cache();
        self.sync_toc_to_scroll();
        true
    }

    pub fn return_to_file_list(&mut self) {
        if !self.directory_mode {
            return;
        }

        if let Some(idx) = self.current_file_index {
            self.scroll_positions.insert(idx, self.scroll);
        }

        self.dir_view = DirView::FileList;
        self.focus = Focus::Toc;
        self.toc_cursor = 0;
        self.toc_scroll = 0;
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

        // node_heights: heading(1) + blank(1) + heading(3) = 5 total
        let node_heights = vec![1usize, 1, 3];
        let node_line_starts = vec![0usize, 1, 2];
        let total_lines = 5;

        let mut app = App::new(
            PathBuf::from("doc.md"),
            document,
            vec!["# A".to_string(), "".to_string(), "## B".to_string()],
            node_heights,
            node_line_starts,
            total_lines,
            vec![],
            vec![0, 2],
        );
        app.content_height = 2;

        // Simulate a viewport update
        app.update_render(vec![Line::default(); 5], vec![], &[0, 1, 2]);

        assert_eq!(app.toc_line_indices, vec![0, 2]);
        app.scroll_to(2);
        assert_eq!(app.synced_toc_index(), Some(1));
    }
}
