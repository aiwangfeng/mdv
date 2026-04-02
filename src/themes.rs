// src/themes.rs
// Built-in color themes for mdv

use ratatui::style::Color;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    CatppuccinMocha,
    Nord,
    GruvboxDark,
    TokyoNight,
    OneDark,
}

#[derive(Debug, Clone, Copy)]
pub struct ThemeColors {
    pub base: Color,
    pub surface0: Color,
    pub surface1: Color,
    pub overlay0: Color,
    pub text: Color,
    pub subtext: Color,
    pub lavender: Color,
    pub blue: Color,
    pub sapphire: Color,
    pub teal: Color,
    pub green: Color,
    pub yellow: Color,
    pub peach: Color,
    pub mauve: Color,
    pub crust: Color,
    pub red: Color,
    pub pink: Color,
    pub flamingo: Color,
    pub maroon: Color,
    pub sky: Color,
}

impl ThemeColors {
    pub fn from_theme(name: ThemeName) -> Self {
        match name {
            ThemeName::CatppuccinMocha => Self::catppuccin_mocha(),
            ThemeName::Nord => Self::nord(),
            ThemeName::GruvboxDark => Self::gruvbox_dark(),
            ThemeName::TokyoNight => Self::tokyo_night(),
            ThemeName::OneDark => Self::one_dark(),
        }
    }
}

impl ThemeColors {
    fn catppuccin_mocha() -> Self {
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

    fn nord() -> Self {
        Self {
            base: Color::Rgb(46, 52, 64),        // #2e3440
            surface0: Color::Rgb(59, 66, 82),    // #3b4252
            surface1: Color::Rgb(67, 76, 94),    // #434c5e
            overlay0: Color::Rgb(76, 86, 106),   // #4c566a
            text: Color::Rgb(216, 222, 233),     // #d8dee9
            subtext: Color::Rgb(189, 193, 202),  // #bdc1cc
            lavender: Color::Rgb(136, 192, 208), // #88c0d0
            blue: Color::Rgb(136, 192, 208),     // #88c0d0
            sapphire: Color::Rgb(129, 161, 193), // #81a1c1
            teal: Color::Rgb(163, 190, 140),     // #a3be8c
            green: Color::Rgb(163, 190, 140),    // #a3be8c
            yellow: Color::Rgb(235, 203, 139),   // #ebcb8b
            peach: Color::Rgb(229, 192, 112),    // #e5c07b
            mauve: Color::Rgb(180, 142, 173),    // #b48ead
            crust: Color::Rgb(35, 39, 50),       // #232830
            red: Color::Rgb(191, 97, 106),       // #bf616a
            pink: Color::Rgb(180, 142, 173),     // #b48ead
            flamingo: Color::Rgb(235, 203, 139), // #ebcb8b
            maroon: Color::Rgb(191, 97, 106),    // #bf616a
            sky: Color::Rgb(136, 192, 208),      // #88c0d0
        }
    }

    fn gruvbox_dark() -> Self {
        Self {
            base: Color::Rgb(40, 40, 40),        // #282828
            surface0: Color::Rgb(60, 56, 54),    // #3c3836
            surface1: Color::Rgb(80, 73, 69),    // #504945
            overlay0: Color::Rgb(102, 92, 86),   // #665c54
            text: Color::Rgb(235, 219, 178),     // #ebdbb2
            subtext: Color::Rgb(189, 174, 147),  // #bdae93
            lavender: Color::Rgb(219, 192, 131), // #d7c07f
            blue: Color::Rgb(184, 187, 38),      // #b8bb26
            sapphire: Color::Rgb(251, 73, 52),   // #fb4934
            teal: Color::Rgb(184, 187, 38),      // #b8bb26
            green: Color::Rgb(184, 187, 38),     // #b8bb26
            yellow: Color::Rgb(250, 189, 47),    // #fabd2f
            peach: Color::Rgb(254, 151, 32),     // #fe8019
            mauve: Color::Rgb(211, 134, 155),    // #d3869b
            crust: Color::Rgb(29, 32, 35),       // #1d2021
            red: Color::Rgb(251, 73, 52),        // #fb4934
            pink: Color::Rgb(249, 120, 133),     // #f97878
            flamingo: Color::Rgb(211, 134, 155), // #d3869b
            maroon: Color::Rgb(204, 105, 98),    // #cc6982
            sky: Color::Rgb(143, 151, 74),       // #8f974a
        }
    }

    fn tokyo_night() -> Self {
        Self {
            base: Color::Rgb(20, 21, 34),        // #16161e
            surface0: Color::Rgb(32, 33, 50),    // #20212f
            surface1: Color::Rgb(43, 44, 66),    // #2b2c40
            overlay0: Color::Rgb(100, 102, 126), // #646677
            text: Color::Rgb(192, 202, 245),     // #c0caf5
            subtext: Color::Rgb(152, 159, 192),  // #989fb8
            lavender: Color::Rgb(173, 186, 249), // #adbbff
            blue: Color::Rgb(122, 162, 247),     // #7aa2f7
            sapphire: Color::Rgb(136, 189, 244), // #88bdf4
            teal: Color::Rgb(38, 203, 180),      // #26cbb3
            green: Color::Rgb(158, 206, 106),    // #9ece6a
            yellow: Color::Rgb(227, 206, 130),   // #e3ce7d
            peach: Color::Rgb(255, 180, 132),    // #ffb484
            mauve: Color::Rgb(197, 156, 220),    // #c59cf4
            crust: Color::Rgb(14, 15, 26),       // #0e0f1a
            red: Color::Rgb(248, 113, 113),      // #f87171
            pink: Color::Rgb(243, 169, 177),     // #f3a9b1
            flamingo: Color::Rgb(249, 180, 204), // #f9b4c4
            maroon: Color::Rgb(238, 137, 152),   // #ee8998
            sky: Color::Rgb(125, 207, 240),      // #7dcff0
        }
    }

    fn one_dark() -> Self {
        Self {
            base: Color::Rgb(40, 44, 52),        // #282c34
            surface0: Color::Rgb(52, 56, 65),    // #343b44
            surface1: Color::Rgb(60, 66, 76),    // #3c424d
            overlay0: Color::Rgb(97, 103, 115),  // #616778
            text: Color::Rgb(220, 223, 228),     // #dcdfe4
            subtext: Color::Rgb(185, 190, 200),  // #b9bee8
            lavender: Color::Rgb(197, 177, 243), // #c5b1f3
            blue: Color::Rgb(97, 175, 239),      // #61afef
            sapphire: Color::Rgb(86, 182, 194),  // #56b6c2
            teal: Color::Rgb(86, 182, 194),      // #56b6c2
            green: Color::Rgb(152, 212, 106),    // #98d46a
            yellow: Color::Rgb(229, 205, 124),   // #e5cd7c
            peach: Color::Rgb(237, 169, 116),    // #eda974
            mauve: Color::Rgb(198, 120, 221),    // #c678dc
            crust: Color::Rgb(28, 31, 38),       // #1c1f24
            red: Color::Rgb(224, 108, 117),      // #e06c75
            pink: Color::Rgb(220, 135, 155),     // #dc879b
            flamingo: Color::Rgb(212, 162, 179), // #d4a2b3
            maroon: Color::Rgb(190, 111, 126),   // #be6f7e
            sky: Color::Rgb(97, 175, 239),       // #61afef
        }
    }
}

impl ThemeName {
    pub fn display_name(self) -> &'static str {
        match self {
            Self::CatppuccinMocha => "Catppuccin Mocha",
            Self::Nord => "Nord",
            Self::GruvboxDark => "Gruvbox Dark",
            Self::TokyoNight => "Tokyo Night",
            Self::OneDark => "One Dark",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "catppuccin-mocha" | "catppuccin_mocha" | "catppuccinmocha" => {
                Some(Self::CatppuccinMocha)
            }
            "nord" => Some(Self::Nord),
            "gruvbox" | "gruvbox-dark" | "gruvbox_dark" | "gruvboxdark" => Some(Self::GruvboxDark),
            "tokyonight" | "tokyo-night" | "tokyo_night" => Some(Self::TokyoNight),
            "one-dark" | "one_dark" | "onedark" => Some(Self::OneDark),
            _ => None,
        }
    }
}
