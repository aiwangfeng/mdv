# mdv

`mdv` is a terminal Markdown viewer written in Rust. It is built for fast keyboard-driven reading with a table of contents sidebar, incremental search, multiple color themes, syntax-highlighted code blocks, UTF-8-aware rendering, and inline image support when the terminal supports it.

## Features

- Vim-style navigation in a TUI
- Table of contents sidebar with synced heading tracking
- Incremental search with next/previous match navigation
- Built-in theme picker and configurable default theme
- Syntax highlighting for fenced code blocks
- Inline image rendering for supported terminals
- UTF-8-aware wrapping and truncation
- Configurable keybindings

## Build

Rust and Cargo are required.

```bash
cargo build --release
```

The release binary will be available at:

```bash
./target/release/mdv
```

## Usage

Open a Markdown file:

```bash
cargo run -- path/to/file.md
```

Disable inline image rendering:

```bash
cargo run -- path/to/file.md --no-images
```

If you built the release binary:

```bash
./target/release/mdv path/to/file.md
```

Notes:

- Files larger than 50 MB are rejected.
- `mdv` is a viewer, not an editor.

## Default Controls

- `j` / `k`: scroll down / up
- `d` / `u`: half-page down / up
- `g` / `G`: jump to top / bottom
- `h` / `l`: move focus between TOC and content
- `s`: toggle the TOC sidebar
- `/`: start search
- `n` / `N`: next / previous search match
- `t`: open the theme picker
- `?`: show help
- `q`: quit

Keybindings can be overridden in the config file.

## Configuration

`mdv` loads configuration from the platform config directory using the `directories` crate.

Typical config locations:

- macOS: `~/Library/Application Support/mdv/config.toml`
- Linux: `~/.config/mdv/config.toml`
- Windows: `%APPDATA%\\mdv\\config.toml`

You can set a default built-in theme with `theme_name` and override individual keybindings under `keys`.

Example:

```toml
theme_name = "tokyo-night"

[keys]
quit = "q"
down = "j"
up = "k"
next_theme = "t"
```

Built-in theme names currently include:

- `catppuccin-mocha`
- `nord`
- `gruvbox-dark`
- `tokyo-night`
- `one-dark`

## Terminal Notes

- Inline image rendering depends on terminal support and the image protocol detected at runtime.
- When image rendering is unavailable, `mdv` still shows image placeholders in the document flow.
- UTF-8 content such as CJK text is handled with display-width-aware wrapping and truncation.
