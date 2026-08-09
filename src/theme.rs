use crate::config::{self, Config};
use ratatui::prelude::Color;
use serde::Deserialize;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Theme {
    pub background: Color,
    pub background_dark: Color,
    pub background_deep: Color,
    pub border: Color,
    pub accent: Color,
    pub selection: Color,
    pub key: Color,
    pub text: Color,
    pub text_bright: Color,
    pub text_dim: Color,
    pub text_muted: Color,
    pub heading: Color,
    pub error: Color,
    pub cursor_bg: Color,
    pub picker_border: Color,
    pub picker_accent: Color,
    pub picker_directory: Color,
    pub picker_matched: Color,
    pub picker_loading: Color,
    pub picker_recent: Color,
    pub labels: CategoryLabels,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct CategoryLabels {
    pub bugs: Color,
    pub features: Color,
    pub improvements: Color,
    pub refactor: Color,
    pub docs: Color,
    pub chore: Color,
    pub data: Color,
    pub model: Color,
    pub experiment: Color,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ThemeConfig {
    #[serde(default)]
    pub colors: BTreeMap<String, String>,
    #[serde(default)]
    pub ui: Option<UiConfig>,
    #[serde(default)]
    pub labels: Option<LabelsConfig>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct UiConfig {
    pub background: Option<String>,
    pub background_dark: Option<String>,
    pub background_deep: Option<String>,
    pub border: Option<String>,
    pub accent: Option<String>,
    pub selection: Option<String>,
    pub key: Option<String>,
    pub text: Option<String>,
    pub text_bright: Option<String>,
    pub text_dim: Option<String>,
    pub text_muted: Option<String>,
    pub heading: Option<String>,
    pub error: Option<String>,
    pub cursor_bg: Option<String>,
    pub picker_border: Option<String>,
    pub picker_accent: Option<String>,
    pub picker_directory: Option<String>,
    pub picker_matched: Option<String>,
    pub picker_loading: Option<String>,
    pub picker_recent: Option<String>,
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct LabelsConfig {
    pub bugs: Option<String>,
    pub features: Option<String>,
    pub improvements: Option<String>,
    pub refactor: Option<String>,
    pub docs: Option<String>,
    pub chore: Option<String>,
    pub data: Option<String>,
    pub model: Option<String>,
    pub experiment: Option<String>,
}

pub fn default_theme() -> Theme {
    Theme {
        background: Color::Rgb(34, 36, 54),
        background_dark: Color::Rgb(30, 32, 48),
        background_deep: Color::Rgb(25, 27, 41),
        border: Color::Rgb(59, 66, 97),
        accent: Color::Rgb(192, 153, 255),
        selection: Color::Rgb(130, 170, 255),
        key: Color::Rgb(134, 225, 252),
        text: Color::Rgb(200, 211, 245),
        text_bright: Color::Rgb(213, 223, 245),
        text_dim: Color::Rgb(99, 109, 166),
        text_muted: Color::Rgb(59, 66, 97),
        heading: Color::Rgb(130, 170, 255),
        error: Color::Rgb(255, 117, 127),
        cursor_bg: Color::Rgb(47, 51, 77),
        picker_border: Color::Rgb(57, 75, 112),
        picker_accent: Color::Rgb(130, 170, 255),
        picker_directory: Color::Rgb(101, 188, 255),
        picker_matched: Color::Rgb(192, 153, 255),
        picker_loading: Color::Rgb(134, 225, 252),
        picker_recent: Color::Rgb(255, 199, 119),
        labels: CategoryLabels {
            bugs: Color::Rgb(255, 117, 127),
            features: Color::Rgb(195, 232, 141),
            improvements: Color::Rgb(192, 153, 255),
            refactor: Color::Rgb(255, 199, 119),
            docs: Color::Rgb(130, 170, 255),
            chore: Color::Rgb(99, 109, 166),
            data: Color::Rgb(79, 214, 190),
            model: Color::Rgb(252, 167, 234),
            experiment: Color::Rgb(255, 150, 108),
        },
    }
}

impl ThemeConfig {
    pub fn resolve(&self, base: &Theme) -> Theme {
        let palette = &self.colors;
        let ui = self.ui.as_ref();
        let labels = self.labels.as_ref();
        let role = |field: Option<&Option<String>>, fallback: Color| {
            field
                .and_then(Option::as_ref)
                .and_then(|name| resolve_color(name, palette))
                .unwrap_or(fallback)
        };
        let conventional =
            |name: &str, fallback: Color| resolve_color(name, palette).unwrap_or(fallback);
        let background = role(
            ui.map(|value| &value.background),
            conventional("bg", base.background),
        );
        let background_dark = role(
            ui.map(|value| &value.background_dark),
            conventional("bg_dark", background),
        );
        let background_deep = role(
            ui.map(|value| &value.background_deep),
            conventional("bg_dark1", background_dark),
        );
        let border = role(ui.map(|value| &value.border), base.border);
        let accent = role(ui.map(|value| &value.accent), base.accent);
        let heading = role(ui.map(|value| &value.heading), base.heading);
        let selection = role(
            ui.map(|value| &value.selection),
            conventional("blue", heading),
        );
        let key = role(ui.map(|value| &value.key), conventional("cyan", accent));

        Theme {
            background,
            background_dark,
            background_deep,
            border,
            accent,
            selection,
            key,
            text: role(ui.map(|value| &value.text), base.text),
            text_bright: role(ui.map(|value| &value.text_bright), base.text_bright),
            text_dim: role(ui.map(|value| &value.text_dim), base.text_dim),
            text_muted: role(ui.map(|value| &value.text_muted), base.text_muted),
            heading,
            error: role(ui.map(|value| &value.error), base.error),
            cursor_bg: role(ui.map(|value| &value.cursor_bg), base.cursor_bg),
            picker_border: role(ui.map(|value| &value.picker_border), border),
            picker_accent: role(ui.map(|value| &value.picker_accent), heading),
            picker_directory: role(ui.map(|value| &value.picker_directory), heading),
            picker_matched: role(ui.map(|value| &value.picker_matched), accent),
            picker_loading: role(ui.map(|value| &value.picker_loading), heading),
            picker_recent: role(
                ui.map(|value| &value.picker_recent),
                conventional("yellow", base.picker_recent),
            ),
            labels: CategoryLabels {
                bugs: role(labels.map(|value| &value.bugs), base.labels.bugs),
                features: role(labels.map(|value| &value.features), base.labels.features),
                improvements: role(
                    labels.map(|value| &value.improvements),
                    base.labels.improvements,
                ),
                refactor: role(labels.map(|value| &value.refactor), base.labels.refactor),
                docs: role(labels.map(|value| &value.docs), base.labels.docs),
                chore: role(labels.map(|value| &value.chore), base.labels.chore),
                data: role(labels.map(|value| &value.data), base.labels.data),
                model: role(labels.map(|value| &value.model), base.labels.model),
                experiment: role(
                    labels.map(|value| &value.experiment),
                    base.labels.experiment,
                ),
            },
        }
    }
}

pub fn configured_themes(config: &Config) -> (Vec<(String, Theme)>, usize) {
    let mut configs = BTreeMap::new();
    if let Some(catalog_path) = config.theme_catalog.as_deref() {
        for path in load_catalog_paths(&config::expand_path(catalog_path)) {
            load_theme_config(&mut configs, &path);
        }
    }
    if let Some(theme_path) = config.theme.as_deref() {
        load_theme_config(&mut configs, &config::expand_path(theme_path));
    }

    if configs.is_empty() {
        return (vec![("default".into(), default_theme())], 0);
    }

    let selected_name = config
        .theme
        .as_deref()
        .map(config::expand_path)
        .map(|path| theme_name(&path));
    let base = default_theme();
    let themes = configs
        .into_iter()
        .map(|(name, source)| (name, source.resolve(&base)))
        .collect::<Vec<_>>();
    let selected = selected_name
        .and_then(|name| themes.iter().position(|(candidate, _)| candidate == &name))
        .unwrap_or(0);
    (themes, selected)
}

fn load_catalog_paths(path: &Path) -> Vec<PathBuf> {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return Vec::new();
    };
    let Ok(catalog) = contents.parse::<toml::Value>() else {
        return Vec::new();
    };
    catalog
        .get("themes")
        .and_then(toml::Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(toml::Value::as_str)
        .map(config::expand_path)
        .collect()
}

fn load_theme_config(themes: &mut BTreeMap<String, ThemeConfig>, path: &Path) {
    let Ok(contents) = std::fs::read_to_string(path) else {
        return;
    };
    let Ok(config) = toml::from_str(&contents) else {
        return;
    };
    themes.insert(theme_name(path), config);
}

fn theme_name(path: &Path) -> String {
    path.file_stem()
        .and_then(|value| value.to_str())
        .unwrap_or("theme")
        .replace('-', " ")
}

fn resolve_color(name: &str, palette: &BTreeMap<String, String>) -> Option<Color> {
    palette.get(name).and_then(|value| parse_hex(value))
}

fn parse_hex(value: &str) -> Option<Color> {
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolves_roles_with_the_mdr_fallback_chain() {
        let config: ThemeConfig = toml::from_str(
            r##"
[colors]
bg = "#100000"
border = "#200000"
orange = "#300000"
yellow = "#400000"
cursor = "#500000"

[ui]
border = "border"
accent = "orange"
heading = "yellow"
cursor_bg = "cursor"
"##,
        )
        .unwrap();
        let theme = config.resolve(&default_theme());

        assert_eq!(theme.background, Color::Rgb(16, 0, 0));
        assert_eq!(theme.background_dark, theme.background);
        assert_eq!(theme.background_deep, theme.background);
        assert_eq!(theme.picker_border, Color::Rgb(32, 0, 0));
        assert_eq!(theme.picker_accent, Color::Rgb(64, 0, 0));
        assert_eq!(theme.picker_matched, Color::Rgb(48, 0, 0));
        assert_eq!(theme.cursor_bg, Color::Rgb(80, 0, 0));
    }

    #[test]
    fn missing_configuration_uses_one_default_theme() {
        let (themes, selected) = configured_themes(&Config::default());

        assert_eq!(themes, vec![("default".into(), default_theme())]);
        assert_eq!(selected, 0);
    }

    #[test]
    fn catalog_is_sorted_deduplicated_and_selects_configured_theme() {
        let directory = tempfile::tempdir().unwrap();
        let alpha = directory.path().join("alpha-theme.toml");
        let zeta = directory.path().join("zeta-theme.toml");
        let catalog = directory.path().join("catalog.toml");
        std::fs::write(&alpha, "[colors]\naccent = \"#112233\"\n").unwrap();
        std::fs::write(&zeta, "[colors]\naccent = \"#445566\"\n").unwrap();
        std::fs::write(
            &catalog,
            format!(
                "themes = [\"{}\", \"{}\", \"{}\"]\n",
                zeta.display(),
                alpha.display(),
                zeta.display()
            ),
        )
        .unwrap();
        let config = Config {
            theme: Some(zeta.display().to_string()),
            theme_catalog: Some(catalog.display().to_string()),
        };

        let (themes, selected) = configured_themes(&config);

        assert_eq!(
            themes
                .iter()
                .map(|(name, _)| name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha theme", "zeta theme"]
        );
        assert_eq!(selected, 1);
    }
}
