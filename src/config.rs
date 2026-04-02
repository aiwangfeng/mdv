// src/config.rs

use anyhow::Result;
use crossterm::event::{KeyCode, KeyModifiers};
use directories::ProjectDirs;
use ratatui::style::Color;
use serde::{Deserialize, Deserializer};
use std::fs;
use std::path::PathBuf;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::LazyLock;
use std::sync::{Mutex, OnceLock};

use crate::themes::{ThemeColors, ThemeName};

thread_local! {
    static THEME_CONFIG: Mutex<ThemeConfig> = Mutex::new(ThemeConfig::default());
}

static CONFIG_LOADED: OnceLock<()> = OnceLock::new();
static CONFIG: OnceLock<Config> = OnceLock::new();
static KEYMAP: OnceLock<ResolvedKeybindings> = OnceLock::new();
static CURRENT_THEME_INDEX: AtomicUsize = AtomicUsize::new(0);

static DEFAULT_KEYMAP: LazyLock<ResolvedKeybindings> = LazyLock::new(ResolvedKeybindings::default);

pub const AVAILABLE_THEMES: &[ThemeName] = &[
    ThemeName::CatppuccinMocha,
    ThemeName::Nord,
    ThemeName::GruvboxDark,
    ThemeName::TokyoNight,
    ThemeName::OneDark,
];

pub fn apply_theme_by_index(index: usize) {
    let normalized = index % AVAILABLE_THEMES.len();
    let theme_name = AVAILABLE_THEMES[normalized];
    let theme_config = ThemeConfig::from(ThemeColors::from_theme(theme_name));

    CURRENT_THEME_INDEX.store(normalized, Ordering::Relaxed);
    THEME_CONFIG.with(|tc| {
        if let Ok(mut guard) = tc.lock() {
            *guard = theme_config;
        }
    });
}

pub fn current_theme_index() -> usize {
    CURRENT_THEME_INDEX.load(Ordering::Relaxed) % AVAILABLE_THEMES.len()
}

#[allow(dead_code)]
pub fn current_theme_name() -> &'static str {
    AVAILABLE_THEMES[current_theme_index()].display_name()
}

pub fn get_theme<F, R>(f: F) -> R
where
    F: FnOnce(&ThemeConfig) -> R,
{
    THEME_CONFIG.with(|tc| {
        let guard = tc.lock().unwrap();
        f(&guard)
    })
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct Config {
    pub theme: ThemeConfig,
    pub keys: KeybindingsConfig,
    #[serde(default)]
    pub theme_name: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct KeybindingsConfig {
    pub quit: String,
    pub up: String,
    pub down: String,
    pub page_up: String,
    pub page_down: String,
    pub top: String,
    pub bottom: String,
    pub toggle_toc: String,
    pub focus_prev: String,
    pub focus_next: String,
    pub search: String,
    pub search_next: String,
    pub search_prev: String,
    pub help: String,
    pub next_theme: String,
}

impl Default for KeybindingsConfig {
    fn default() -> Self {
        Self {
            quit: "q".to_string(),
            up: "k".to_string(),
            down: "j".to_string(),
            page_up: "u".to_string(),
            page_down: "d".to_string(),
            top: "g".to_string(),
            bottom: "G".to_string(),
            toggle_toc: "s".to_string(),
            focus_prev: "h".to_string(),
            focus_next: "l".to_string(),
            search: "/".to_string(),
            search_next: "n".to_string(),
            search_prev: "N".to_string(),
            help: "?".to_string(),
            next_theme: "t".to_string(),
        }
    }
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ThemeConfig {
    #[serde(deserialize_with = "deserialize_color")]
    pub base: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub surface0: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub surface1: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub overlay0: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub text: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub subtext: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub lavender: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub blue: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub sapphire: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub teal: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub green: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub yellow: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub peach: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub mauve: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub crust: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub red: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub pink: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub flamingo: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub maroon: Color,
    #[serde(deserialize_with = "deserialize_color")]
    pub sky: Color,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KeyBinding {
    pub code: KeyCode,
    pub modifiers: KeyModifiers,
}

impl KeyBinding {
    pub const fn new(code: KeyCode, modifiers: KeyModifiers) -> Self {
        Self { code, modifiers }
    }

    pub fn matches(self, code: KeyCode, modifiers: KeyModifiers) -> bool {
        if self.code != code {
            return false;
        }

        let relevant_modifiers = modifiers & !KeyModifiers::SHIFT;
        relevant_modifiers == self.modifiers
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResolvedKeybindings {
    pub quit: KeyBinding,
    pub up: KeyBinding,
    pub down: KeyBinding,
    pub page_up: KeyBinding,
    pub page_down: KeyBinding,
    pub top: KeyBinding,
    pub bottom: KeyBinding,
    pub toggle_toc: KeyBinding,
    pub focus_prev: KeyBinding,
    pub focus_next: KeyBinding,
    pub search: KeyBinding,
    pub search_next: KeyBinding,
    pub search_prev: KeyBinding,
    pub help: KeyBinding,
    pub next_theme: KeyBinding,
}

impl Default for ThemeConfig {
    fn default() -> Self {
        Self {
            base: Color::Rgb(30, 30, 46),        // #1e1e2e
            surface0: Color::Rgb(49, 50, 68),    // #313244
            surface1: Color::Rgb(69, 71, 90),    // #45475a
            overlay0: Color::Rgb(108, 112, 134), // #6c7086
            text: Color::Rgb(205, 214, 244),     // #cdd6f4
            subtext: Color::Rgb(166, 173, 200),  // #a6adc8
            lavender: Color::Rgb(180, 190, 254), // #b4befe
            blue: Color::Rgb(137, 180, 250),     // #89b4fa
            sapphire: Color::Rgb(116, 199, 236), // #74c7ec
            teal: Color::Rgb(148, 226, 213),     // #94e2d5
            green: Color::Rgb(166, 227, 161),    // #a6e3a1
            yellow: Color::Rgb(249, 226, 175),   // #f9e2af
            peach: Color::Rgb(250, 179, 135),    // #fab387
            mauve: Color::Rgb(203, 166, 247),    // #cba6f7
            crust: Color::Rgb(17, 17, 27),       // #11111b
            red: Color::Rgb(243, 139, 168),      // #f38ba8
            pink: Color::Rgb(245, 194, 231),     // #f5c2e7
            flamingo: Color::Rgb(242, 205, 205), // #f2cdcd
            maroon: Color::Rgb(235, 160, 172),   // #eba0ac
            sky: Color::Rgb(137, 220, 235),      // #89dceb
        }
    }
}

impl From<ThemeColors> for ThemeConfig {
    fn from(theme_colors: ThemeColors) -> Self {
        Self {
            base: theme_colors.base,
            surface0: theme_colors.surface0,
            surface1: theme_colors.surface1,
            overlay0: theme_colors.overlay0,
            text: theme_colors.text,
            subtext: theme_colors.subtext,
            lavender: theme_colors.lavender,
            blue: theme_colors.blue,
            sapphire: theme_colors.sapphire,
            teal: theme_colors.teal,
            green: theme_colors.green,
            yellow: theme_colors.yellow,
            peach: theme_colors.peach,
            mauve: theme_colors.mauve,
            crust: theme_colors.crust,
            red: theme_colors.red,
            pink: theme_colors.pink,
            flamingo: theme_colors.flamingo,
            maroon: theme_colors.maroon,
            sky: theme_colors.sky,
        }
    }
}

#[allow(dead_code)]
pub fn get() -> &'static Config {
    if CONFIG.get().is_none() {
        let _ = load();
    }
    CONFIG.get().expect("config should be loaded")
}

pub fn keymap() -> &'static ResolvedKeybindings {
    if KEYMAP.get().is_none() {
        let _ = load();
    }
    KEYMAP.get().unwrap_or_else(|| &DEFAULT_KEYMAP)
}

pub fn load() -> Result<()> {
    if CONFIG_LOADED.get().is_some() {
        return Ok(());
    }

    let config_path = config_file_path();
    let mut errors = Vec::new();

    if let Some(path) = config_path.as_ref().filter(|p| p.exists()) {
        match fs::read_to_string(path) {
            Ok(content) => match toml::from_str::<Config>(&content) {
                Ok(c) => {
                    let keymap = ResolvedKeybindings::from_config(&c);
                    let mut config = c;
                    if let Some(theme_name) = ThemeName::from_str(&config.theme_name) {
                        config.theme = ThemeConfig::from(ThemeColors::from_theme(theme_name));
                        if let Some(index) = AVAILABLE_THEMES.iter().position(|&t| t == theme_name)
                        {
                            CURRENT_THEME_INDEX.store(index, Ordering::Relaxed);
                        }
                    }
                    let _ = CONFIG.set(config.clone());
                    let _ = KEYMAP.set(keymap);
                    let _ = CONFIG_LOADED.set(());

                    THEME_CONFIG.with(|tc| {
                        if let Ok(mut guard) = tc.lock() {
                            *guard = config.theme.clone();
                        }
                    });

                    return Ok(());
                }
                Err(e) => errors.push(format!(
                    "Failed to parse config file: {}: {}",
                    path.display(),
                    e
                )),
            },
            Err(e) => errors.push(format!(
                "Failed to read config file: {}: {}",
                path.display(),
                e
            )),
        }
    }

    let keymap = ResolvedKeybindings::default();
    let _ = CONFIG.set(Config::default());
    let _ = KEYMAP.set(keymap);
    let _ = CONFIG_LOADED.set(());

    THEME_CONFIG.with(|tc| {
        if let Ok(mut guard) = tc.lock() {
            *guard = ThemeConfig::default();
        }
    });

    for error in &errors {
        eprintln!("Warning: {}", error);
    }

    Ok(())
}

fn config_file_path() -> Option<PathBuf> {
    ProjectDirs::from("", "", "mdv").map(|p| p.config_dir().join("config.toml"))
}

// Helper to deserialize hex color strings into ratatui Color::Rgb
fn deserialize_color<'de, D>(deserializer: D) -> std::result::Result<Color, D::Error>
where
    D: Deserializer<'de>,
{
    let s = String::deserialize(deserializer)?;
    let s = s.trim_start_matches('#');
    if s.len() != 6 {
        return Err(serde::de::Error::custom(
            "Color must be a 6-character hex string (e.g. #ff0000)",
        ));
    }
    let r = u8::from_str_radix(&s[0..2], 16).map_err(serde::de::Error::custom)?;
    let g = u8::from_str_radix(&s[2..4], 16).map_err(serde::de::Error::custom)?;
    let b = u8::from_str_radix(&s[4..6], 16).map_err(serde::de::Error::custom)?;
    Ok(Color::Rgb(r, g, b))
}

fn parse_key_code(s: &str) -> KeyCode {
    let trimmed = s.trim();
    if trimmed.chars().count() == 1 {
        return KeyCode::Char(trimmed.chars().next().unwrap());
    }

    match trimmed.to_lowercase().as_str() {
        "esc" => KeyCode::Esc,
        "enter" => KeyCode::Enter,
        "up" => KeyCode::Up,
        "down" => KeyCode::Down,
        "left" => KeyCode::Left,
        "right" => KeyCode::Right,
        "tab" => KeyCode::Tab,
        "backspace" => KeyCode::Backspace,
        "ctrl-c" | "c" => KeyCode::Char('c'),
        "pageup" | "pgup" => KeyCode::PageUp,
        "pagedown" | "pgdown" => KeyCode::PageDown,
        _ => {
            log::warn!("Unknown key binding: '{}', using default", s);
            KeyCode::Null
        }
    }
}

pub fn parse_key(s: &str) -> KeyBinding {
    let trimmed = s.trim();
    if trimmed.is_empty() {
        return KeyBinding::new(KeyCode::Null, KeyModifiers::empty());
    }

    let parts: Vec<&str> = trimmed.split('-').collect();
    let (modifier_parts, key_part) = if parts.len() > 1 {
        (&parts[..parts.len() - 1], parts[parts.len() - 1])
    } else {
        (&[][..], trimmed)
    };

    let mut modifiers = KeyModifiers::empty();
    for part in modifier_parts {
        match part.to_lowercase().as_str() {
            "ctrl" | "control" => modifiers |= KeyModifiers::CONTROL,
            "alt" | "option" => modifiers |= KeyModifiers::ALT,
            "shift" => modifiers |= KeyModifiers::SHIFT,
            _ => {
                log::warn!("Unknown key modifier: '{}', using null binding", part);
                return KeyBinding::new(KeyCode::Null, KeyModifiers::empty());
            }
        }
    }

    KeyBinding::new(parse_key_code(key_part), modifiers)
}

impl Default for ResolvedKeybindings {
    fn default() -> Self {
        Self {
            quit: KeyBinding::new(KeyCode::Char('q'), KeyModifiers::empty()),
            up: KeyBinding::new(KeyCode::Char('k'), KeyModifiers::empty()),
            down: KeyBinding::new(KeyCode::Char('j'), KeyModifiers::empty()),
            page_up: KeyBinding::new(KeyCode::Char('u'), KeyModifiers::empty()),
            page_down: KeyBinding::new(KeyCode::Char('d'), KeyModifiers::empty()),
            top: KeyBinding::new(KeyCode::Char('g'), KeyModifiers::empty()),
            bottom: KeyBinding::new(KeyCode::Char('G'), KeyModifiers::empty()),
            toggle_toc: KeyBinding::new(KeyCode::Char('s'), KeyModifiers::empty()),
            focus_prev: KeyBinding::new(KeyCode::Char('h'), KeyModifiers::empty()),
            focus_next: KeyBinding::new(KeyCode::Char('l'), KeyModifiers::empty()),
            search: KeyBinding::new(KeyCode::Char('/'), KeyModifiers::empty()),
            search_next: KeyBinding::new(KeyCode::Char('n'), KeyModifiers::empty()),
            search_prev: KeyBinding::new(KeyCode::Char('N'), KeyModifiers::empty()),
            help: KeyBinding::new(KeyCode::Char('?'), KeyModifiers::empty()),
            next_theme: KeyBinding::new(KeyCode::Char('t'), KeyModifiers::empty()),
        }
    }
}

impl ResolvedKeybindings {
    fn from_config(config: &Config) -> Self {
        let keys = &config.keys;
        Self {
            quit: parse_key(&keys.quit),
            up: parse_key(&keys.up),
            down: parse_key(&keys.down),
            page_up: parse_key(&keys.page_up),
            page_down: parse_key(&keys.page_down),
            top: parse_key(&keys.top),
            bottom: parse_key(&keys.bottom),
            toggle_toc: parse_key(&keys.toggle_toc),
            focus_prev: parse_key(&keys.focus_prev),
            focus_next: parse_key(&keys.focus_next),
            search: parse_key(&keys.search),
            search_next: parse_key(&keys.search_next),
            search_prev: parse_key(&keys.search_prev),
            help: parse_key(&keys.help),
            next_theme: parse_key(&keys.next_theme),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{parse_key, Config, KeyBinding, ResolvedKeybindings};
    use crossterm::event::{KeyCode, KeyModifiers};

    #[test]
    fn preserves_case_for_single_char_keys() {
        assert_eq!(
            parse_key("G"),
            KeyBinding::new(KeyCode::Char('G'), KeyModifiers::empty())
        );
        assert_eq!(
            parse_key("q"),
            KeyBinding::new(KeyCode::Char('q'), KeyModifiers::empty())
        );
    }

    #[test]
    fn parses_control_bindings_without_matching_plain_key() {
        let binding = parse_key("ctrl-c");

        assert!(binding.matches(KeyCode::Char('c'), KeyModifiers::CONTROL));
        assert!(!binding.matches(KeyCode::Char('c'), KeyModifiers::empty()));
    }

    #[test]
    fn resolves_keybindings_once() {
        let config = Config::default();
        let keymap = ResolvedKeybindings::from_config(&config);

        assert_eq!(
            keymap.quit,
            KeyBinding::new(KeyCode::Char('q'), KeyModifiers::empty())
        );
        assert_eq!(
            keymap.bottom,
            KeyBinding::new(KeyCode::Char('G'), KeyModifiers::empty())
        );
        assert_eq!(
            keymap.search,
            KeyBinding::new(KeyCode::Char('/'), KeyModifiers::empty())
        );
    }
}
