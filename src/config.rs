use serde::Deserialize;
use std::path::{Path, PathBuf};

#[derive(Default, Deserialize)]
pub struct Config {
    pub theme: Option<String>,
    pub theme_catalog: Option<String>,
}

impl Config {
    pub fn load() -> Self {
        let path = config_path();
        match std::fs::read_to_string(&path) {
            Ok(contents) => match toml::from_str(&contents) {
                Ok(config) => config,
                Err(error) => {
                    eprintln!("pqview: ignoring {}: {error}", path.display());
                    Self::default()
                }
            },
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => Self::default(),
            Err(error) => {
                eprintln!("pqview: could not read {}: {error}", path.display());
                Self::default()
            }
        }
    }
}

pub fn expand_path(path: &str) -> PathBuf {
    if path == "~" {
        return home_dir().unwrap_or_else(|| PathBuf::from(path));
    }
    if let Some(suffix) = path.strip_prefix("~/")
        && let Some(home) = home_dir()
    {
        return home.join(suffix);
    }
    PathBuf::from(path)
}

fn config_path() -> PathBuf {
    if let Some(directory) = std::env::var_os("XDG_CONFIG_HOME") {
        return Path::new(&directory).join("pqview/config.toml");
    }
    home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".config/pqview/config.toml")
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_theme_paths() {
        let config: Config = toml::from_str(
            r#"
theme = "~/.config/themes/moon.toml"
theme_catalog = "~/.config/themes/catalog.toml"
"#,
        )
        .unwrap();

        assert_eq!(config.theme.as_deref(), Some("~/.config/themes/moon.toml"));
        assert_eq!(
            config.theme_catalog.as_deref(),
            Some("~/.config/themes/catalog.toml")
        );
    }
}
