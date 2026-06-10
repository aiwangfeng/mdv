use std::cell::Cell;
use unicode_width::UnicodeWidthChar;

thread_local! {
    static CJK_WIDTH: Cell<bool> = const { Cell::new(false) };
}

pub fn init_cjk_width(enabled: Option<bool>) {
    let active = enabled.unwrap_or_else(|| {
        std::env::var("LANG")
            .or_else(|_| std::env::var("LC_ALL"))
            .or_else(|_| std::env::var("LC_CTYPE"))
            .map(|v| {
                let v = v.to_lowercase();
                v.contains("zh") || v.contains("ja") || v.contains("ko")
            })
            .unwrap_or(false)
    });
    set_cjk_width(active);
}

pub fn set_cjk_width(enabled: bool) {
    CJK_WIDTH.with(|cell| cell.set(enabled));
}

pub fn is_cjk() -> bool {
    CJK_WIDTH.with(|cell| cell.get())
}

pub fn char_width(c: char) -> usize {
    if is_cjk() {
        match c {
            // Common Neutral/Narrow symbols rendered as double-width in CJK terminals
            '✓' | '✗' | '✔' | '✘' |
            '★' | '☆' | '●' | '○' |
            '▲' | '▼' | '◀' | '▶' |
            '▪' | '▫' | '•' | '◦' | '▸' | '▹' |
            '⚠' => 2,
            _ => c.width_cjk().unwrap_or(0),
        }
    } else {
        c.width().unwrap_or(0)
    }
}

pub fn str_width(s: &str) -> usize {
    s.chars().map(char_width).sum()
}
