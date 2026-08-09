use crate::config::{self, Config};
use anyhow::{Context, Result};
use ratatui::prelude::Color;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};

#[derive(Clone)]
pub struct Theme {
    pub name: String,
    pub accent: Color,
    pub accent_search: Color,
    pub accent_filter: Color,
    pub text: Color,
    pub text_muted: Color,
    pub text_on_match: Color,
    pub border: Color,
    pub surface_selected: Color,
    pub surface_focused: Color,
    pub background: Color,
    pub background_dark: Color,
    pub background_deep: Color,
    pub selection: Color,
    pub key: Color,
    pub text_bright: Color,
    pub text_dim: Color,
    pub error: Color,
    pub picker_border: Color,
    pub picker_accent: Color,
    pub picker_directory: Color,
    pub picker_matched: Color,
    pub picker_loading: Color,
    pub picker_recent: Color,
}

#[derive(Default, Deserialize)]
struct ThemeFile {
    #[serde(default)]
    colors: BTreeMap<String, String>,
    #[serde(default)]
    ui: Ui,
}

#[derive(Default, Deserialize)]
struct Ui {
    background: Option<String>,
    background_dark: Option<String>,
    background_deep: Option<String>,
    border: Option<String>,
    accent: Option<String>,
    selection: Option<String>,
    key: Option<String>,
    text: Option<String>,
    text_bright: Option<String>,
    text_dim: Option<String>,
    text_muted: Option<String>,
    heading: Option<String>,
    error: Option<String>,
    cursor_bg: Option<String>,
    picker_border: Option<String>,
    picker_accent: Option<String>,
    picker_directory: Option<String>,
    picker_matched: Option<String>,
    picker_loading: Option<String>,
    picker_recent: Option<String>,
}

#[derive(Default, Deserialize)]
struct Catalog {
    #[serde(default)]
    themes: Vec<String>,
}

pub fn built_in_themes() -> Vec<Theme> {
    vec![
        moon_theme("Tokyo Night Moon".into()),
        built_in(
            "Catppuccin",
            (137, 180, 250),
            (249, 226, 175),
            (166, 227, 161),
            (205, 214, 244),
            (108, 112, 134),
            (30, 30, 46),
            (69, 71, 90),
            (49, 50, 68),
            (59, 60, 78),
        ),
        built_in(
            "Solarized Dark",
            (38, 139, 210),
            (181, 137, 0),
            (133, 153, 0),
            (147, 161, 161),
            (88, 110, 117),
            (0, 43, 54),
            (58, 80, 87),
            (7, 54, 66),
            (18, 65, 77),
        ),
        built_in(
            "Nord",
            (136, 192, 208),
            (235, 203, 139),
            (163, 190, 140),
            (216, 222, 233),
            (107, 112, 137),
            (46, 52, 64),
            (67, 76, 94),
            (59, 66, 82),
            (67, 76, 94),
        ),
        built_in(
            "Dracula",
            (189, 147, 249),
            (241, 250, 140),
            (80, 250, 123),
            (248, 248, 242),
            (98, 114, 164),
            (40, 42, 54),
            (68, 71, 90),
            (55, 57, 73),
            (65, 67, 83),
        ),
    ]
}

pub fn configured_themes(config: &Config) -> (Vec<Theme>, usize) {
    if config.theme.is_none() && config.theme_catalog.is_none() {
        return (built_in_themes(), 0);
    }

    let selected_path = config.theme.as_deref().map(config::expand_path);
    let mut paths = selected_path.iter().cloned().collect::<Vec<_>>();
    if let Some(catalog_path) = config.theme_catalog.as_deref().map(config::expand_path) {
        match load_catalog(&catalog_path) {
            Ok(catalog_paths) => paths.extend(catalog_paths),
            Err(error) => eprintln!("pqview: {error:#}"),
        }
    }

    let mut seen = HashSet::new();
    let mut themes = Vec::new();
    let mut selected = 0;
    for path in paths {
        let normalized = path.clone();
        if !seen.insert(normalized.clone()) {
            continue;
        }
        match load_theme(&path) {
            Ok(theme) => {
                if selected_path.as_ref() == Some(&normalized) {
                    selected = themes.len();
                }
                themes.push(theme);
            }
            Err(error) => eprintln!("pqview: {error:#}"),
        }
    }

    if themes.is_empty() {
        eprintln!("pqview: no configured themes could be loaded; using built-in themes");
        (built_in_themes(), 0)
    } else {
        (themes, selected)
    }
}

fn load_catalog(path: &Path) -> Result<Vec<PathBuf>> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("could not read theme catalog {}", path.display()))?;
    let catalog: Catalog = toml::from_str(&contents)
        .with_context(|| format!("invalid theme catalog {}", path.display()))?;
    Ok(catalog
        .themes
        .iter()
        .map(|entry| {
            let expanded = config::expand_path(entry);
            if expanded.is_relative() {
                path.parent()
                    .unwrap_or_else(|| Path::new("."))
                    .join(expanded)
            } else {
                expanded
            }
        })
        .collect())
}

fn load_theme(path: &Path) -> Result<Theme> {
    let contents = std::fs::read_to_string(path)
        .with_context(|| format!("could not read theme {}", path.display()))?;
    let source: ThemeFile =
        toml::from_str(&contents).with_context(|| format!("invalid theme {}", path.display()))?;
    Ok(source.resolve(theme_name(path)))
}

impl ThemeFile {
    fn resolve(&self, name: String) -> Theme {
        let fallback = moon_theme(name.clone());
        let color = |value: &Option<String>, default: Color| {
            value
                .as_deref()
                .and_then(|name| self.colors.get(name).map(String::as_str).or(Some(name)))
                .and_then(parse_color)
                .unwrap_or(default)
        };
        let background = color(&self.ui.background, fallback.background);
        let selection = color(&self.ui.selection, fallback.selection);
        let heading = color(&self.ui.heading, fallback.accent_search);
        let cursor_bg = color(&self.ui.cursor_bg, fallback.surface_focused);
        Theme {
            name,
            accent: color(&self.ui.accent, fallback.accent),
            accent_search: heading,
            accent_filter: self
                .colors
                .get("green")
                .and_then(|value| parse_color(value))
                .unwrap_or(fallback.accent_filter),
            text: color(&self.ui.text, fallback.text),
            text_muted: color(&self.ui.text_muted, fallback.text_muted),
            text_on_match: background,
            border: color(&self.ui.border, fallback.border),
            surface_selected: cursor_bg,
            surface_focused: cursor_bg,
            background,
            background_dark: color(&self.ui.background_dark, fallback.background_dark),
            background_deep: color(&self.ui.background_deep, fallback.background_deep),
            selection,
            key: color(&self.ui.key, fallback.key),
            text_bright: color(&self.ui.text_bright, fallback.text_bright),
            text_dim: color(&self.ui.text_dim, fallback.text_dim),
            error: color(&self.ui.error, fallback.error),
            picker_border: color(&self.ui.picker_border, fallback.picker_border),
            picker_accent: color(&self.ui.picker_accent, fallback.picker_accent),
            picker_directory: color(&self.ui.picker_directory, fallback.picker_directory),
            picker_matched: color(&self.ui.picker_matched, fallback.picker_matched),
            picker_loading: color(&self.ui.picker_loading, fallback.picker_loading),
            picker_recent: color(&self.ui.picker_recent, fallback.picker_recent),
        }
    }
}

fn theme_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("Custom")
        .replace('-', " ")
}

fn parse_color(value: &str) -> Option<Color> {
    let hex = value.strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    Some(Color::Rgb(
        u8::from_str_radix(&hex[0..2], 16).ok()?,
        u8::from_str_radix(&hex[2..4], 16).ok()?,
        u8::from_str_radix(&hex[4..6], 16).ok()?,
    ))
}

#[allow(clippy::too_many_arguments)]
fn built_in(
    name: &str,
    accent: (u8, u8, u8),
    accent_search: (u8, u8, u8),
    accent_filter: (u8, u8, u8),
    text: (u8, u8, u8),
    text_muted: (u8, u8, u8),
    background: (u8, u8, u8),
    border: (u8, u8, u8),
    selected: (u8, u8, u8),
    focused: (u8, u8, u8),
) -> Theme {
    let rgb = |(r, g, b)| Color::Rgb(r, g, b);
    Theme {
        name: name.into(),
        accent: rgb(accent),
        accent_search: rgb(accent_search),
        accent_filter: rgb(accent_filter),
        text: rgb(text),
        text_muted: rgb(text_muted),
        text_on_match: rgb(background),
        border: rgb(border),
        surface_selected: rgb(selected),
        surface_focused: rgb(focused),
        background: rgb(background),
        background_dark: rgb(selected),
        background_deep: rgb(background),
        selection: rgb(accent),
        key: rgb(accent),
        text_bright: rgb(text),
        text_dim: rgb(text_muted),
        error: Color::Rgb(255, 117, 127),
        picker_border: rgb(border),
        picker_accent: rgb(accent),
        picker_directory: rgb(accent),
        picker_matched: rgb(accent_search),
        picker_loading: rgb(accent_filter),
        picker_recent: rgb(accent_search),
    }
}

fn moon_theme(name: String) -> Theme {
    let mut theme = built_in(
        &name,
        (192, 153, 255),
        (130, 170, 255),
        (195, 232, 141),
        (200, 211, 245),
        (59, 66, 97),
        (34, 36, 54),
        (59, 66, 97),
        (130, 170, 255),
        (47, 51, 77),
    );
    theme.background_dark = Color::Rgb(30, 32, 48);
    theme.background_deep = Color::Rgb(25, 27, 41);
    theme.selection = Color::Rgb(130, 170, 255);
    theme.surface_selected = Color::Rgb(47, 51, 77);
    theme.surface_focused = Color::Rgb(47, 51, 77);
    theme.text_bright = Color::Rgb(213, 223, 245);
    theme.text_dim = Color::Rgb(99, 109, 166);
    theme.key = Color::Rgb(134, 225, 252);
    theme.picker_border = Color::Rgb(57, 75, 112);
    theme.picker_directory = Color::Rgb(101, 188, 255);
    theme.picker_loading = Color::Rgb(134, 225, 252);
    theme.picker_recent = Color::Rgb(255, 199, 119);
    theme
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_shared_semantic_theme_schema() {
        let source: ThemeFile = toml::from_str(
            r##"
[colors]
bg = "#010203"
blue = "#112233"
purple = "#aabbcc"
cursor = "#334455"

[ui]
background = "bg"
accent = "blue"
selection = "purple"
cursor_bg = "cursor"
picker_matched = "purple"
"##,
        )
        .unwrap();
        let theme = source.resolve("Synthetic".into());

        assert_eq!(theme.background, Color::Rgb(1, 2, 3));
        assert_eq!(theme.accent, Color::Rgb(17, 34, 51));
        assert_eq!(theme.selection, Color::Rgb(170, 187, 204));
        assert_eq!(theme.surface_selected, Color::Rgb(51, 68, 85));
        assert_eq!(theme.picker_matched, Color::Rgb(170, 187, 204));
        assert_eq!(theme.text, Color::Rgb(200, 211, 245));
    }

    #[test]
    fn built_in_fallback_has_moon_background_surfaces() {
        let theme = &built_in_themes()[0];

        assert_eq!(theme.name, "Tokyo Night Moon");
        assert_eq!(theme.background, Color::Rgb(34, 36, 54));
        assert_eq!(theme.background_dark, Color::Rgb(30, 32, 48));
        assert_eq!(theme.background_deep, Color::Rgb(25, 27, 41));
        assert_eq!(theme.surface_selected, Color::Rgb(47, 51, 77));
    }

    #[test]
    fn rejects_non_rgb_colors() {
        assert_eq!(parse_color("blue"), None);
        assert_eq!(parse_color("#123"), None);
    }

    #[test]
    fn catalog_uses_explicit_paths_relative_to_catalog() {
        let directory = tempfile::tempdir().unwrap();
        let catalog_path = directory.path().join("catalog.toml");
        std::fs::write(
            &catalog_path,
            "themes = [\"synthetic-one.toml\", \"nested/synthetic-two.toml\"]\n",
        )
        .unwrap();

        assert_eq!(
            load_catalog(&catalog_path).unwrap(),
            vec![
                directory.path().join("synthetic-one.toml"),
                directory.path().join("nested/synthetic-two.toml")
            ]
        );
    }

    #[test]
    fn configured_theme_is_selected_and_deduplicated_from_catalog() {
        let directory = tempfile::tempdir().unwrap();
        let selected_path = directory.path().join("synthetic-selected.toml");
        let alternate_path = directory.path().join("synthetic-alternate.toml");
        let catalog_path = directory.path().join("catalog.toml");
        std::fs::write(
            &selected_path,
            "[colors]\naccent = \"#112233\"\n[ui]\naccent = \"accent\"\n",
        )
        .unwrap();
        std::fs::write(&alternate_path, "[colors]\n[ui]\n").unwrap();
        std::fs::write(
            &catalog_path,
            format!(
                "themes = [\"{}\", \"{}\"]\n",
                alternate_path.display(),
                selected_path.display()
            ),
        )
        .unwrap();
        let config = Config {
            theme: Some(selected_path.display().to_string()),
            theme_catalog: Some(catalog_path.display().to_string()),
        };

        let (themes, selected) = configured_themes(&config);

        assert_eq!(themes.len(), 2);
        assert_eq!(selected, 0);
        assert_eq!(themes[0].name, "synthetic selected");
        assert_eq!(themes[0].accent, Color::Rgb(17, 34, 51));
        assert_eq!(themes[1].name, "synthetic alternate");
    }
}
