use std::collections::HashMap;
use std::path::PathBuf;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct SessionConfig {
    #[serde(default)]
    pub last_folders: HashMap<String, PathBuf>,
    #[serde(default)]
    pub last_source: Option<PathBuf>,
}
fn config_path() -> Option<PathBuf> {
    let dirs = directories::ProjectDirs::from("app", "TexelBox", "TexelBox")?;
    Some(dirs.config_dir().join("session.json"))
}
pub fn load() -> SessionConfig {
    let Some(path) = config_path() else { return SessionConfig::default() };
    let Ok(text) = std::fs::read_to_string(&path) else { return SessionConfig::default() };
    serde_json::from_str(&text).unwrap_or_default()
}
pub fn save(cfg: &SessionConfig) {
    let Some(path) = config_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(text) = serde_json::to_string_pretty(cfg) {
        let _ = std::fs::write(&path, text);
    }
}
pub fn last_folder(cfg: &SessionConfig, panel: &str) -> Option<PathBuf> {
    cfg.last_folders.get(panel).cloned().filter(|p| p.exists())
}
pub fn set_last_folder(cfg: &mut SessionConfig, panel: &str, path: PathBuf) {
    cfg.last_folders.insert(panel.to_string(), path);
    save(cfg);
}
