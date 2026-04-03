// src/main.rs
// Entry point: parse CLI args, initialize terminal, run the event loop.

mod app;
mod config;
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

use app::{App, Focus, Mode};
use image_proto::ImageManager;
use renderer::RenderResult;

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

#[derive(ClapParser, Debug)]
#[command(
    name = "mdv",
    version,
    about = "A TUI Markdown viewer with vim keybindings and inline image support",
    long_about = None,
)]
struct Cli {
    /// Markdown file to open
    #[arg(value_name = "FILE")]
    file: PathBuf,

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

// ---------------------------------------------------------------------------
// Main
// ---------------------------------------------------------------------------

fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn")).init();
    config::load()?;
    let cli = Cli::parse();

    let metadata = fs::metadata(&cli.file)
        .with_context(|| format!("Cannot access '{}'", cli.file.display()))?;

    if metadata.len() > MAX_FILE_SIZE {
        anyhow::bail!(
            "File '{}' is too large ({} bytes). Maximum size is {} bytes.",
            cli.file.display(),
            metadata.len(),
            MAX_FILE_SIZE
        );
    }

    let markdown = fs::read_to_string(&cli.file)
        .with_context(|| format!("Cannot read '{}'", cli.file.display()))?;

    // Parse document
    let document = parser::parse(&markdown);

    // Pre-render lines at a nominal width (will re-render on first frame)
    let initial_width = 80u16;
    let RenderResult {
        lines: rendered_lines,
        image_positions,
        node_line_starts,
    } = renderer::render_nodes(&document.nodes, initial_width);
    let toc_line_indices = document
        .toc
        .iter()
        .map(|entry| node_line_starts.get(entry.node_index).copied().unwrap_or(0))
        .collect();

    // Image manager
    let mut img_mgr = ImageManager::new(cli.no_images);

    // Pre-load images (asynchronously)
    if img_mgr.is_enabled() {
        let base_dir = get_base_dir(&cli.file);
        for (_, src, _) in &image_positions {
            img_mgr.load_async(src, base_dir);
        }
    }

    let mut app = App::new(
        cli.file.clone(),
        document,
        rendered_lines,
        image_positions,
        toc_line_indices,
    );

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
    let _ = input_thread.join();

    result
}

fn run(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    app: &mut App,
    img_mgr: &mut ImageManager,
    rx: mpsc::Receiver<AppEvent>,
) -> Result<()> {
    let mut current_width = terminal.size()?.width;
    let mut last_width = 0u16;
    let mut needs_redraw = true;
    let mut pending_resize: Option<u16> = None;
    let mut resize_deadline: Option<Instant> = None;
    let mut initial_render_done = false;
    const RESIZE_DEBOUNCE_MS: u64 = 50;

    loop {
        if let (Some(w), Some(deadline)) = (pending_resize, resize_deadline) {
            if Instant::now() >= deadline {
                pending_resize = None;
                resize_deadline = None;
                current_width = w;
                initial_render_done = true;

                let content_width = if app.show_toc && app.toc_len() > 0 {
                    let toc_cols = current_width * app.toc_width_pct / 100;
                    current_width.saturating_sub(toc_cols).saturating_sub(2)
                } else {
                    current_width.saturating_sub(2)
                };

                if content_width != last_width && content_width > 0 {
                    let RenderResult {
                        lines,
                        image_positions: img_pos,
                        node_line_starts,
                    } = renderer::render_nodes(&app.document.nodes, content_width);
                    app.update_render(lines, img_pos, &node_line_starts);

                    last_width = content_width;
                    needs_redraw = true;
                }
            }
        }

        if !initial_render_done && current_width > 0 {
            let content_width = if app.show_toc && app.toc_len() > 0 {
                let toc_cols = current_width * app.toc_width_pct / 100;
                current_width.saturating_sub(toc_cols).saturating_sub(2)
            } else {
                current_width.saturating_sub(2)
            };

            if content_width != last_width && content_width > 0 {
                let RenderResult {
                    lines,
                    image_positions: img_pos,
                    node_line_starts,
                } = renderer::render_nodes(&app.document.nodes, content_width);
                app.update_render(lines, img_pos, &node_line_starts);

                if img_mgr.is_enabled() {
                    let base_dir = get_base_dir(&app.file_path);
                    for (_, src, _) in &app.image_positions {
                        img_mgr.load_async(src, base_dir);
                    }
                }
                last_width = content_width;
                needs_redraw = true;
            }
            initial_render_done = true;
        }

        if needs_redraw {
            terminal.draw(|frame| {
                ui::draw(frame, app, img_mgr);
            })?;
            needs_redraw = false;
        }

        if img_mgr.process_incoming() {
            needs_redraw = true;
        }

        match rx.recv_timeout(Duration::from_millis(16)) {
            Ok(event) => {
                if let AppEvent::Resize(w, _h) = event {
                    pending_resize = Some(w);
                    resize_deadline =
                        Some(Instant::now() + Duration::from_millis(RESIZE_DEBOUNCE_MS));
                }
                needs_redraw |= handle_app_event(app, event);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => return Ok(()),
        }

        while let Ok(message) = rx.try_recv() {
            if let AppEvent::Resize(w, _h) = message {
                pending_resize = Some(w);
                resize_deadline = Some(Instant::now() + Duration::from_millis(RESIZE_DEBOUNCE_MS));
            }
            needs_redraw |= handle_app_event(app, message);
        }

        if app.quit {
            return Ok(());
        }
    }
}

fn spawn_input_thread(tx: mpsc::Sender<AppEvent>, stop: Arc<AtomicBool>) -> thread::JoinHandle<()> {
    thread::spawn(move || {
        while !stop.load(Ordering::Relaxed) {
            match event::poll(Duration::from_millis(16)) {
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

fn handle_app_event(app: &mut App, event: AppEvent) -> bool {
    match event {
        AppEvent::Key { code, modifiers } => {
            handle_key(app, code, modifiers);
            true
        }
        AppEvent::Resize(_, _) => true,
        AppEvent::InputClosed => {
            app.quit = true;
            false
        }
    }
}

// ---------------------------------------------------------------------------
// Key handling
// ---------------------------------------------------------------------------

fn handle_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match app.mode {
        Mode::Search => handle_search_key(app, code, modifiers),
        Mode::Help => handle_help_key(app, code, modifiers),
        Mode::ThemePicker => handle_theme_picker_key(app, code, modifiers),
        Mode::Normal => handle_normal_key(app, code, modifiers),
    }
}

fn handle_normal_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    if code == KeyCode::Null {
        return;
    }

    let keys = config::keymap();

    match code {
        c if keys.quit.matches(c, modifiers) => app.quit = true,
        c if keys.help.matches(c, modifiers) => app.toggle_help(),

        c if keys.focus_prev.matches(c, modifiers) => {
            if app.focus == Focus::Content && app.show_toc {
                app.focus = Focus::Toc;
            }
        }
        c if keys.focus_next.matches(c, modifiers) => {
            if app.focus == Focus::Toc {
                app.focus = Focus::Content;
            }
        }
        KeyCode::Tab if modifiers.is_empty() => app.toggle_focus(),

        c if keys.toggle_toc.matches(c, modifiers) => app.toggle_toc(),
        c if keys.next_theme.matches(c, modifiers) => {
            app.open_theme_picker(config::current_theme_index())
        }
        KeyCode::Char('<') if modifiers.is_empty() => app.narrow_toc(),
        KeyCode::Char('>') if modifiers.is_empty() => app.widen_toc(),

        c if keys.down.matches(c, modifiers) => {
            if app.focus == Focus::Content {
                app.scroll_down(1);
            } else {
                app.toc_down();
            }
        }
        c if keys.up.matches(c, modifiers) => {
            if app.focus == Focus::Content {
                app.scroll_up(1);
            } else {
                app.toc_up();
            }
        }
        c if keys.toc_down.matches(c, modifiers) => app.toc_down(),
        c if keys.toc_up.matches(c, modifiers) => app.toc_up(),

        c if keys.page_down.matches(c, modifiers) => app.scroll_down(app.half_page()),
        c if keys.page_up.matches(c, modifiers) => app.scroll_up(app.half_page()),
        c if keys.top.matches(c, modifiers) => app.scroll_top(),
        c if keys.bottom.matches(c, modifiers) => app.scroll_bottom(),

        KeyCode::Enter if modifiers.is_empty() => {
            if app.focus == Focus::Toc {
                app.toc_jump_to_cursor();
                app.focus = Focus::Content;
            }
        }

        KeyCode::Char(']') if modifiers.is_empty() => app.next_heading(),
        KeyCode::Char('[') if modifiers.is_empty() => app.prev_heading(),

        c if keys.search.matches(c, modifiers) => app.start_search(),
        c if keys.search_next.matches(c, modifiers) => app.search_next(),
        c if keys.search_prev.matches(c, modifiers) => app.search_prev(),

        _ => {}
    }
}

fn handle_search_key(app: &mut App, code: KeyCode, _modifiers: KeyModifiers) {
    match code {
        KeyCode::Esc => app.search_cancel(),
        KeyCode::Enter => app.search_confirm(),
        KeyCode::Backspace => app.search_pop(),
        KeyCode::Char(c) => app.search_push(c),
        _ => {}
    }
}

fn handle_help_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    match code {
        c if config::keymap().help.matches(c, modifiers) => app.toggle_help(),
        KeyCode::Esc if modifiers.is_empty() => app.toggle_help(),
        c if config::keymap().quit.matches(c, modifiers) => app.toggle_help(),
        _ => {}
    }
}

fn handle_theme_picker_key(app: &mut App, code: KeyCode, modifiers: KeyModifiers) {
    let keys = config::keymap();
    let preview = |app: &mut App, delta| {
        if let Some(index) = app.move_theme_picker(delta, config::AVAILABLE_THEMES.len()) {
            config::apply_theme_by_index(index);
        }
    };

    match code {
        KeyCode::Esc if modifiers.is_empty() => {
            if let Some(index) = app.cancel_theme_picker() {
                config::apply_theme_by_index(index);
            }
        }
        KeyCode::Enter if modifiers.is_empty() => app.confirm_theme_picker(),
        c if keys.quit.matches(c, modifiers) || keys.next_theme.matches(c, modifiers) => {
            if let Some(index) = app.cancel_theme_picker() {
                config::apply_theme_by_index(index);
            }
        }
        c if keys.up.matches(c, modifiers) => preview(app, -1),
        c if keys.down.matches(c, modifiers) => preview(app, 1),
        _ => {}
    }
}
