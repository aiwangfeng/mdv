// src/main.rs
// Entry point: parse CLI args, initialize terminal, run the event loop.

mod app;
mod config;
mod dir;
mod image_proto;
mod parser;
mod renderer;
mod theme;
mod themes;
mod ui;

use std::fs;
use std::io;
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};
use std::thread;
use std::time::{Duration, Instant};

use anyhow::{Context, Result};
use clap::Parser as ClapParser;
use crossterm::{
    event::{
        self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind, KeyModifiers,
    },
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::{backend::CrosstermBackend, Terminal};

use app::{measure_document, App, Focus, Mode};
use image_proto::ImageManager;

enum AppEvent {
    Key {
        code: KeyCode,
        modifiers: KeyModifiers,
    },
    Resize(u16, u16),
    InputClosed,
}

// ---------------------------------------------------------------------------
// CLI
// ---------------------------------------------------------------------------

const MAX_FILE_SIZE: u64 = 50 * 1024 * 1024; // 50 MB limit
const POLL_INTERVAL_MS: u64 = 16;
const RESIZE_DEBOUNCE_MS: u64 = 50;

#[derive(ClapParser, Debug)]
#[command(
    name = "mdv",
    version,
    about = "A TUI Markdown viewer with vim keybindings and inline image support",
    long_about = None,
)]
struct Cli {
    /// Markdown file or directory to open
    #[arg(value_name = "PATH")]
    path: Option<PathBuf>,

    /// Disable inline image rendering
    #[arg(long, default_value_t = false)]
    no_images: bool,
}

// ---------------------------------------------------------------------------
// Helper functions
// ---------------------------------------------------------------------------

fn get_base_dir(file_path: &Path) -> &Path {
    file_path.parent().unwrap_or_else(|| Path::new("."))
}

fn load_visible_images(app: &App, img_mgr: &mut ImageManager) {
    if !img_mgr.is_enabled() || app.directory_mode {
        return;
    }

    let base_dir = get_base_dir(&app.file_path);
    let overscan = app.content_height as usize + renderer::IMAGE_RENDER_HEIGHT;
    let start = app.scroll.saturating_sub(overscan);
    let end = app.scroll + app.content_height as usize + overscan;
    for (line_idx, src, _) in &app.image_positions {
        if *line_idx >= start && *line_idx <= end {
            img_mgr.load_async(src, base_dir);
        }
    }
}

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    config::load()?;
    let cli = Cli::parse();

    let mut app = if let Some(path) = &cli.path {
        let metadata = match fs::metadata(path) {
            Ok(m) => m,
            Err(e) => {
                anyhow::bail!("Cannot access '{}': {}", path.display(), e);
            }
        };

        if metadata.is_dir() {
            // Directory mode
            let dir_files = dir::scan_markdown_files(path);
            if dir_files.is_empty() {
                anyhow::bail!("No markdown files found in '{}'", path.display());
            }
            App::new_directory_mode(path.clone(), dir_files)
        } else {
            // Single file mode
            if metadata.len() > MAX_FILE_SIZE {
                anyhow::bail!(
                    "File '{}' is too large ({} bytes). Maximum size is {} bytes.",
                    path.display(),
                    metadata.len(),
                    MAX_FILE_SIZE
                );
            }
            let markdown = fs::read_to_string(path)
                .with_context(|| format!("Cannot read '{}'", path.display()))?;
            let document = parser::parse(&markdown);
            let result = measure_document(&document, 80u16);
            App::new(
                path.clone(),
                document,
                result.node_heights,
                result.node_line_starts,
                result.total_content_lines,
                vec![],
                result.toc_line_indices,
            )
        }
    } else {
        // No path specified, use current directory
        let cwd = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let dir_files = dir::scan_markdown_files(&cwd);
        if dir_files.is_empty() {
            anyhow::bail!("No markdown files found in current directory");
        }
        App::new_directory_mode(cwd, dir_files)
    };

    // Image manager
    let mut img_mgr = ImageManager::new(cli.no_images);

    // ── Terminal setup ───────────────────────────────────────────────────────
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;
    let (tx, rx) = mpsc::channel();
    let stop_input = Arc::new(AtomicBool::new(false));
    let input_thread = spawn_input_thread(tx, Arc::clone(&stop_input));

    // ── Event loop ───────────────────────────────────────────────────────────
    let result = run(&mut terminal, &mut app, &mut img_mgr, rx);

    // ── Cleanup ──────────────────────────────────────────────────────────────
    if let Err(e) = disable_raw_mode() {
        eprintln!("Warning: Failed to disable raw mode: {}", e);
    }
    if let Err(e) = execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    ) {
        eprintln!("Warning: Failed to restore terminal: {}", e);
    }
    if let Err(e) = terminal.show_cursor() {
        eprintln!("Warning: Failed to show cursor: {}", e);
    }
    stop_input.store(true, Ordering::Relaxed);
    if let Err(e) = input_thread.join() {
        eprintln!("Warning: Input thread panicked: {:?}", e);
    }

    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    img_mgr: &mut ImageManager,
    rx: mpsc::Receiver<AppEvent>,
) -> Result<()> {
    let mut needs_redraw = true;
    let mut resize_deadline: Option<Instant> = None;

    // Initial layout (only re-run on resize events)
    {
        let size = terminal.size()?;
        ui::calculate_layout(
            app,
            ratatui::layout::Rect::new(0, 0, size.width, size.height),
        );
    }
    let mut last_render_width = app.content_width;

    loop {
        if let Some(deadline) = resize_deadline {
            if Instant::now() >= deadline {
                resize_deadline = None;
                let size = terminal.size()?;
                ui::calculate_layout(
                    app,
                    ratatui::layout::Rect::new(0, 0, size.width, size.height),
                );
                let cw = app.content_width;
                if cw > 0 && cw != last_render_width {
                    app.remeasure(cw);
                    last_render_width = cw;
                    app.mark_viewport_dirty();
                }
                needs_redraw = true;
            }
        }

        let cw = app.content_width;
        let fw = app.full_content_width;

        if app.ensure_viewport_rendered(cw, fw) {
            needs_redraw = true;
        }

        if app.run_pending_search() {
            needs_redraw = true;
        }

        if app.has_active_toast() && app.tick_toast() {
            needs_redraw = true;
        }

        if img_mgr.process_incoming() {
            needs_redraw = true;
        }

        if needs_redraw {
            load_visible_images(app, img_mgr);
            terminal.draw(|frame| {
                ui::draw(frame, app, img_mgr);
            })?;
            needs_redraw = false;
        }

        match rx.recv_timeout(Duration::from_millis(POLL_INTERVAL_MS)) {
            Ok(event) => {
                if let AppEvent::Resize(w, h) = event {
                    let _ = (w, h);
                    resize_deadline =
                        Some(Instant::now() + Duration::from_millis(RESIZE_DEBOUNCE_MS));
                } else {
                    needs_redraw |= handle_app_event(app, img_mgr, event);
                }
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }

        while let Ok(message) = rx.try_recv() {
            if let AppEvent::Resize(w, h) = message {
                let _ = (w, h);
                resize_deadline = Some(Instant::now() + Duration::from_millis(RESIZE_DEBOUNCE_MS));
            } else {
                needs_redraw |= handle_app_event(app, img_mgr, message);
            }
        }

        if app.quit {
            return Ok(());
        }
    }
}

fn spawn_input_thread(tx: mpsc::Sender<AppEvent>, stop: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match event::poll(Duration::from_millis(POLL_INTERVAL_MS)) {
                Ok(true) => match event::read() {
                    Ok(Event::Key(key))
                        if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                    {
                        if tx
                            .send(AppEvent::Key {
                                code: key.code,
                                modifiers: key.modifiers,
                            })
                            .is_err()
                        {
                            break;
                        }
                    }
                    Ok(Event::Resize(w, h)) => {
                        if tx.send(AppEvent::Resize(w, h)).is_err() {
                            break;
                        }
                    }
                    Ok(_) => {}
                    Err(_) => {
                        let _ = tx.send(AppEvent::InputClosed);
                        break;
                    }
                },
                Ok(false) => {}
                Err(_) => {
                    let _ = tx.send(AppEvent::InputClosed);
                    break;
                }
            }
        }
    })
}

fn handle_app_event(app: &mut App, img_mgr: &mut ImageManager, event: AppEvent) -> bool {
    match event {
        AppEvent::Key { code, modifiers } => {
            if app.first_run {
                app.first_run = false;
            }
            handle_key(app, img_mgr, code, modifiers)
        }
        AppEvent::Resize(_, _) => false,
        AppEvent::InputClosed => {
            app.quit = true;
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

fn handle_key(
    app: &mut App,
    img_mgr: &mut ImageManager,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    match app.mode {
        Mode::Search => handle_search_key(app, code, modifiers),
        Mode::Help => handle_help_key(app, code, modifiers),
        Mode::ThemePicker => handle_theme_picker_key(app, code, modifiers),
        Mode::Normal => handle_normal_key(app, img_mgr, code, modifiers),
    }
}

fn handle_normal_key(
    app: &mut App,
    img_mgr: &mut ImageManager,
    code: KeyCode,
    modifiers: KeyModifiers,
) -> bool {
    if code == KeyCode::Null {
        return false;
    }

    // Directory mode key handling
    if app.is_directory_mode() {
        let keys = config::keymap();
        match app.dir_view {
            crate::app::DirView::FileList => match code {
                c if keys.quit.matches(c, modifiers) => {
                    app.quit = true;
                    return true;
                }
                c if keys.down.matches(c, modifiers) => {
                    let before = (app.dir_cursor, app.dir_scroll);
                    app.dir_down();
                    return before != (app.dir_cursor, app.dir_scroll);
                }
                c if keys.up.matches(c, modifiers) => {
                    let before = (app.dir_cursor, app.dir_scroll);
                    app.dir_up();
                    return before != (app.dir_cursor, app.dir_scroll);
                }
                c if keys.top.matches(c, modifiers) => {
                    let before = (app.dir_cursor, app.dir_scroll);
                    app.dir_top();
                    return before != (app.dir_cursor, app.dir_scroll);
                }
                c if keys.bottom.matches(c, modifiers) => {
                    let before = (app.dir_cursor, app.dir_scroll);
                    app.dir_bottom();
                    return before != (app.dir_cursor, app.dir_scroll);
                }
                KeyCode::Enter if modifiers.is_empty() => {
                    let cursor = app.dir_cursor;
                    let base_dir = app.dir_base.clone();
                    return app.open_file_from_dir(cursor, &base_dir, img_mgr);
                }
                c if keys.search.matches(c, modifiers) => {
                    app.start_dir_search();
                    return true;
                }
                c if keys.search_next.matches(c, modifiers) => {
                    let before = (app.search_current, app.dir_cursor);
                    app.dir_search_next();
                    return before != (app.search_current, app.dir_cursor);
                }
                c if keys.search_prev.matches(c, modifiers) => {
                    let before = (app.search_current, app.dir_cursor);
                    app.dir_search_prev();
                    return before != (app.search_current, app.dir_cursor);
                }
                _ => return false,
            },
            crate::app::DirView::FileView => match code {
                KeyCode::Esc | KeyCode::Backspace => {
                    app.return_to_file_list();
                    return true;
                }
                _ => {}
            },
        }
    }

    let keys = config::keymap();

    match code {
        c if keys.quit.matches(c, modifiers) => {
            app.quit = true;
            true
        }
        c if keys.help.matches(c, modifiers) => {
            app.toggle_help();
            true
        }

        c if keys.focus_prev.matches(c, modifiers) => {
            if app.focus == Focus::Content && app.show_toc {
                app.focus = Focus::Toc;
                true
            } else {
                false
            }
        }
        c if keys.focus_next.matches(c, modifiers) => {
            if app.focus == Focus::Toc {
                app.focus = Focus::Content;
                true
            } else {
                false
            }
        }
        KeyCode::Tab if modifiers.is_empty() => {
            app.toggle_focus();
            true
        }

        c if keys.toggle_toc.matches(c, modifiers) => {
            app.toggle_toc();
            true
        }
        c if keys.next_theme.matches(c, modifiers) => {
            app.open_theme_picker(config::current_theme_index());
            true
        }
        KeyCode::Char('<') if modifiers.is_empty() => {
            app.narrow_toc();
            true
        }
        KeyCode::Char('>') if modifiers.is_empty() => {
            app.widen_toc();
            true
        }

        c if keys.down.matches(c, modifiers) => {
            let before = (app.scroll, app.toc_cursor, app.toc_scroll);
            if app.focus == Focus::Content {
                app.scroll_down(1);
            } else {
                app.toc_down();
            }
            before != (app.scroll, app.toc_cursor, app.toc_scroll)
        }
        c if keys.up.matches(c, modifiers) => {
            let before = (app.scroll, app.toc_cursor, app.toc_scroll);
            if app.focus == Focus::Content {
                app.scroll_up(1);
            } else {
                app.toc_up();
            }
            before != (app.scroll, app.toc_cursor, app.toc_scroll)
        }
        c if keys.toc_down.matches(c, modifiers) => {
            let before = (app.toc_cursor, app.toc_scroll);
            app.toc_down();
            before != (app.toc_cursor, app.toc_scroll)
        }
        c if keys.toc_up.matches(c, modifiers) => {
            let before = (app.toc_cursor, app.toc_scroll);
            app.toc_up();
            before != (app.toc_cursor, app.toc_scroll)
        }

        c if keys.page_down.matches(c, modifiers) => {
            let before = app.scroll;
            app.scroll_down(app.half_page());
            before != app.scroll
        }
        c if keys.page_up.matches(c, modifiers) => {
            let before = app.scroll;
            app.scroll_up(app.half_page());
            before != app.scroll
        }
        c if keys.top.matches(c, modifiers) => {
            let before = app.scroll;
            app.scroll_top();
            before != app.scroll
        }
        c if keys.bottom.matches(c, modifiers) => {
            let before = app.scroll;
            app.scroll_bottom();
            before != app.scroll
        }

        KeyCode::Enter if modifiers.is_empty() => {
            if app.focus == Focus::Toc {
                app.toc_jump_to_cursor();
                app.focus = Focus::Content;
                true
            } else {
                false
            }
        }

        KeyCode::Char(']') if modifiers.is_empty() => {
            let before = app.scroll;
            app.next_heading();
            before != app.scroll
        }
        KeyCode::Char('[') if modifiers.is_empty() => {
            let before = app.scroll;
            app.prev_heading();
            before != app.scroll
        }

        c if keys.search.matches(c, modifiers) => {
            app.start_search();
            true
        }
        c if keys.search_next.matches(c, modifiers) => {
            let before = (app.search_current, app.scroll);
            app.search_next();
            before != (app.search_current, app.scroll)
        }
        c if keys.search_prev.matches(c, modifiers) => {
            let before = (app.search_current, app.scroll);
            app.search_prev();
            before != (app.search_current, app.scroll)
        }

        _ => false,
    }
}

fn handle_search_key(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) -> bool {
    match code {
        KeyCode::Esc => {
            app.search_cancel();
            true
        }
        KeyCode::Enter => {
            // In directory mode FileList, Enter confirms the search filter
            // (vim-style: user then navigates with j/k/n/N and opens with Enter)
            if app.is_directory_mode() && app.dir_view == crate::app::DirView::FileList {
                app.dir_search_confirm();
            } else {
                app.search_confirm();
            }
            true
        }
        KeyCode::Up if app.is_directory_mode() && app.dir_view == crate::app::DirView::FileList => {
            app.dir_search_select(-1)
        }
        KeyCode::Down
            if app.is_directory_mode() && app.dir_view == crate::app::DirView::FileList =>
        {
            app.dir_search_select(1)
        }
        KeyCode::Backspace => {
            app.search_pop();
            true
        }
        KeyCode::Char(c) => {
            app.search_push(c);
            true
        }
        _ => false,
    }
}

fn handle_help_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> bool {
    match code {
        c if config::keymap().help.matches(c, modifiers) => {
            app.toggle_help();
            true
        }
        KeyCode::Esc if modifiers.is_empty() => {
            app.toggle_help();
            true
        }
        c if config::keymap().quit.matches(c, modifiers) => {
            app.toggle_help();
            true
        }
        _ => false,
    }
}

fn handle_theme_picker_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) -> bool {
    let keys = config::keymap();
    let preview = |app: &mut App, delta| {
        if let Some(index) = app.move_theme_picker(delta, config::AVAILABLE_THEMES.len()) {
            config::apply_theme_by_index(index);
            true
        } else {
            false
        }
    };

    match code {
        KeyCode::Esc if modifiers.is_empty() => {
            if let Some(index) = app.cancel_theme_picker() {
                config::apply_theme_by_index(index);
            }
            true
        }
        KeyCode::Enter if modifiers.is_empty() => {
            app.confirm_theme_picker();
            true
        }
        c if keys.quit.matches(c, modifiers) || keys.next_theme.matches(c, modifiers) => {
            if let Some(index) = app.cancel_theme_picker() {
                config::apply_theme_by_index(index);
            }
            true
        }
        c if keys.up.matches(c, modifiers) => preview(app, -1),
        c if keys.down.matches(c, modifiers) => preview(app, 1),
        _ => false,
    }
}
