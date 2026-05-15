use ratatui::prelude::Color;

#[derive(Clone)]
pub struct Theme {
    pub name: &'static str,
    pub accent: Color,
    pub accent_search: Color,
    pub accent_filter: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_on_match: Color,
    pub border: Color,
    pub surface_selected: Color,
    pub surface_focused: Color,
}

pub const THEMES: &[Theme] = &[TOKYO_NIGHT, CATPPUCCIN, SOLARIZED, NORD, DRACULA];

pub const TOKYO_NIGHT: Theme = Theme {
    name: "Tokyo Night",
    accent: Color::Rgb(122, 162, 247),
    accent_search: Color::Rgb(224, 175, 104),
    accent_filter: Color::Rgb(158, 206, 106),
    text: Color::Rgb(255, 255, 255),
    text_muted: Color::Rgb(120, 120, 140),
    text_on_match: Color::Rgb(0, 0, 0),
    border: Color::Rgb(68, 68, 100),
    surface_selected: Color::Rgb(55, 58, 85),
    surface_focused: Color::Rgb(50, 52, 72),
};

pub const CATPPUCCIN: Theme = Theme {
    name: "Catppuccin",
    accent: Color::Rgb(137, 180, 250),
    accent_search: Color::Rgb(249, 226, 175),
    accent_filter: Color::Rgb(166, 227, 161),
    text: Color::Rgb(205, 214, 244),
    text_muted: Color::Rgb(108, 112, 134),
    text_on_match: Color::Rgb(30, 30, 46),
    border: Color::Rgb(69, 71, 90),
    surface_selected: Color::Rgb(49, 50, 68),
    surface_focused: Color::Rgb(59, 60, 78),
};

pub const SOLARIZED: Theme = Theme {
    name: "Solarized Dark",
    accent: Color::Rgb(38, 139, 210),
    accent_search: Color::Rgb(181, 137, 0),
    accent_filter: Color::Rgb(133, 153, 0),
    text: Color::Rgb(147, 161, 161),
    text_muted: Color::Rgb(88, 110, 117),
    text_on_match: Color::Rgb(0, 43, 54),
    border: Color::Rgb(58, 80, 87),
    surface_selected: Color::Rgb(7, 54, 66),
    surface_focused: Color::Rgb(18, 65, 77),
};

pub const NORD: Theme = Theme {
    name: "Nord",
    accent: Color::Rgb(136, 192, 208),
    accent_search: Color::Rgb(235, 203, 139),
    accent_filter: Color::Rgb(163, 190, 140),
    text: Color::Rgb(216, 222, 233),
    text_muted: Color::Rgb(107, 112, 137),
    text_on_match: Color::Rgb(46, 52, 64),
    border: Color::Rgb(67, 76, 94),
    surface_selected: Color::Rgb(59, 66, 82),
    surface_focused: Color::Rgb(67, 76, 94),
};

pub const DRACULA: Theme = Theme {
    name: "Dracula",
    accent: Color::Rgb(189, 147, 249),
    accent_search: Color::Rgb(241, 250, 140),
    accent_filter: Color::Rgb(80, 250, 123),
    text: Color::Rgb(248, 248, 242),
    text_muted: Color::Rgb(98, 114, 164),
    text_on_match: Color::Rgb(40, 42, 54),
    border: Color::Rgb(68, 71, 90),
    surface_selected: Color::Rgb(55, 57, 73),
    surface_focused: Color::Rgb(65, 67, 83),
};
