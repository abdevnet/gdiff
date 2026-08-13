use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Config {
    #[serde(default)]
    pub theme: Option<String>,
    #[serde(default)]
    pub editor_command: Option<String>,
    #[serde(default)]
    pub side_by_side: Option<bool>,
}

pub fn config_path() -> PathBuf {
    dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".gdiff-viewer.json")
}

pub fn load() -> Config {
    let path = config_path();
    match fs::read_to_string(&path) {
        Ok(text) => serde_json::from_str(&text).unwrap_or_default(),
        Err(_) => Config::default(),
    }
}

pub fn save(cfg: &Config) -> Result<(), String> {
    let path = config_path();
    let text = serde_json::to_string_pretty(cfg).map_err(|e| e.to_string())?;
    fs::write(&path, text).map_err(|e| format!("write {}: {e}", path.display()))
}

pub fn set_theme(theme: &str) {
    let mut cfg = load();
    cfg.theme = Some(theme.to_string());
    let _ = save(&cfg);
}

pub fn set_diff_mode(side_by_side: bool) {
    let mut cfg = load();
    cfg.side_by_side = Some(side_by_side);
    let _ = save(&cfg);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_js_config_keys() {
        let raw = r#"{"theme":"absent","editorCommand":"code {file}"}"#;
        let cfg: Config = serde_json::from_str(raw).unwrap();
        assert_eq!(cfg.theme.as_deref(), Some("absent"));
        assert_eq!(cfg.editor_command.as_deref(), Some("code {file}"));
    }
}
