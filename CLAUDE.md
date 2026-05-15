# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Build/Run/Test

```bash
cargo build --release          # Build
cargo run -- <file.md|dir/>    # Run (file or directory mode)
cargo run -- file.md --no-images  # Run without image rendering
cargo test                     # Run all tests
cargo clippy                   # Lint
cargo fmt                      # Format
RUST_LOG=debug cargo run -- file.md  # Debug logging
```

Binary: `./target/release/mdv`. Config: `~/.config/mdv/config.toml` (Linux), loaded via `directories` crate.

## Architecture

mdv is a terminal Markdown **viewer** (not editor) — TUI, vim-keybindings, ratatui + crossterm.

### Data flow

```
Markdown file → parser::parse() → Document (Vec<DocNode> + Vec<TocEntry>)
  → renderer::measure_nodes() → node heights
  → renderer::compute_line_starts() → cumulative line offsets
  → App struct holds everything
  → renderer::render_viewport() → Vec<Line> (visible region + 50-line buffer)
  → ui::draw() → terminal
```

### Key architectural decisions

**Viewport rendering**: Only the visible region (±50 lines) is rendered and cached for each draw cycle. Width changes trigger re-measure (cheap) and re-render. Full-document rendering only happens for search (lowercased text cache).

**Threading model**: Three thread types:
1. Main thread — event loop + rendering (16ms poll interval)
2. Input thread — polls `crossterm::event::read()` and sends `AppEvent` over mpsc
3. Image threads — up to 4 concurrent for async image decode

**Config singletons** (`config.rs`): `OnceLock`-guarded `CONFIG` and `KEYMAP`, a `LazyLock` for `DEFAULT_KEYMAP`. Theme config is `thread_local!(RefCell<ThemeConfig>)`. Call `config::load()` once at startup, then use `config::get()`, `config::keymap()`, `config::get_theme(|t| ...)`.

**Search**: Debounced at 75ms. Builds full-document lowercased text cache, finds matching line indices, navigates through them. Highlighted lines are LRU-cached (max 256 entries).

**Resize**: Debounced at 50ms. Multiple rapid resize events are collapsed.

**Directory mode**: Scans `.md`/`.markdown` files (skips hidden dirs, `target/`), sorted by mtime. File content is cached in an LRU cache (max 32 files) with per-file scroll position memory.

### Module map

| Module | Role |
|---|---|
| `main.rs` | CLI (clap), terminal init, event loop, key dispatch |
| `app.rs` | `App` struct — all mutable state, navigation, search, directory mode, viewport cache |
| `parser.rs` | pulldown-cmark → `Document` (nodes + TOC entries) + `InlineSpan` |
| `renderer/` | Node measurement + viewport/full rendering → `ratatui::text::Line` |
| `renderer/code.rs` | syntect syntax highlighting for code blocks |
| `renderer/inline.rs` | Inline span rendering with soft-wrapping |
| `renderer/table.rs` | Proportional-width table rendering |
| `renderer/measure.rs` | Height estimation without full rendering |
| `renderer/search.rs` | Post-process search highlight on rendered lines |
| `ui.rs` | Layout (TOC sidebar + content + status bar), drawing, overlays (help, search bar, theme picker) |
| `config.rs` | Config loading, keybinding parsing (`ctrl-`/`alt-`/`shift-` prefixes), theme singletons |
| `themes.rs` | 5 built-in themes as raw `Color` values |
| `theme.rs` | `CachedStyles` (~40 named styles), rebuilt on theme change |
| `image_proto.rs` | ratatui-image protocol detection, async image loading pool, path safety checks |
| `dir.rs` | Recursive `.md` file scanner |

### Tests

- `src/renderer/tests.rs` — rendering integration tests
- `src/app.rs` — `#[cfg(test)] mod tests` for viewport/TOC sync
- `src/config.rs` — `#[cfg(test)] mod tests` for keybinding parsing

### Important constants

- `MAX_FILE_SIZE`: 50 MB (files larger are rejected)
- `RENDER_BUFFER_LINES`: 50 (overscan above/below viewport)
- `SEARCH_DEBOUNCE_MS`: 75ms
- `RESIZE_DEBOUNCE_MS`: 50ms
- `MAX_SEARCH_HIGHLIGHT_CACHE`: 256 lines (LRU)
- `MAX_FILE_CACHE`: 32 files (directory mode, LRU)
- `POLL_INTERVAL_MS`: 16ms (~60fps)
