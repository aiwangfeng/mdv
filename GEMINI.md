# mdv - GEMINI.md

## Project Overview

`mdv` is a terminal-based Markdown viewer written in Rust. It provides a fast, keyboard-driven interface for reading Markdown documents with features like a Table of Contents (TOC) sidebar, syntax-highlighted code blocks, incremental search, and inline image support for compatible terminals.

### Core Technologies
- **TUI Framework:** [ratatui](https://github.com/ratatui-org/ratatui) with [crossterm](https://github.com/crossterm-rs/crossterm) backend.
- **Markdown Parsing:** [pulldown-cmark](https://github.com/raphlinus/pulldown-cmark).
- **Syntax Highlighting:** [syntect](https://github.com/trishume/syntect).
- **Image Rendering:** [ratatui-image](https://github.com/ratatui-org/ratatui-image).
- **CLI:** [clap](https://github.com/clap-rs/clap).
- **Configuration:** [serde](https://github.com/serde-rs/serde) and [toml](https://github.com/toml-rs/toml).

### Architecture
- `src/main.rs`: Entry point. Handles CLI parsing, terminal initialization, and the main event loop.
- `src/app.rs`: Manages application state, including navigation, search query, TOC sync, and mode switching (Normal, Search, Help, ThemePicker).
- `src/parser.rs`: Responsible for converting raw Markdown strings into an internal `Document` structure consisting of `DocNode` elements and a `TocEntry` list.
- `src/renderer/`: Contains logic for measuring and rendering `DocNode` elements into `ratatui::text::Line` objects. Implements wrapping, styling, and block layouts (tables, quotes, code).
- `src/ui.rs`: Defines the TUI layout (TOC sidebar vs. content) and coordinates the drawing of various components.
- `src/theme.rs` & `src/themes.rs`: Built-in theme definitions and styling logic.
- `src/config.rs`: Handles persistent user configuration and keybindings.

## Building and Running

### Commands
- **Build:** `cargo build --release`
- **Run:** `cargo run -- path/to/file.md`
- **Run with directory mode:** `cargo run -- path/to/directory/` (scans for .md files)
- **Test:** `cargo test`
- **Lint:** `cargo clippy`
- **Format:** `cargo fmt`

## Development Conventions

### Code Style
- **Error Handling:** Use `anyhow::Result` for application-level error handling.
- **Logging:** Use the `log` crate with `env_logger`. Set `RUST_LOG=debug` to see detailed logs.
- **Async/Threading:** The terminal input is handled in a separate thread to ensure a responsive UI. Image loading is also performed asynchronously to avoid blocking the main loop.
- **Formatting:** Adhere to standard Rust formatting (`cargo fmt`).

### Performance
- **Viewport Rendering:** Only the visible part of the document (plus a buffer) is rendered and cached to maintain high performance with large files.
- **Search Highlighting:** Rendered lines for search results are cached to ensure smooth scrolling while search is active.

### Testing
- Unit tests for the parser are located in `src/parser.rs`.
- Rendering tests are located in `src/renderer/tests.rs`.
- When adding new Markdown features, ensure both the parser and the renderer are updated and tested.
