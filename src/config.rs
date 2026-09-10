//! Конфигурация приложения (~/.config/taynik/config.toml).

use std::path::PathBuf;

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Последняя открытая база.
    pub last_database: Option<String>,
    /// Автоблокировка через N минут бездействия (0 — отключить).
    pub auto_lock_minutes: u32,
    /// Блокировать при сворачивании.
    pub lock_on_minimize: bool,
    /// Очистка буфера обмена через N секунд.
    pub clipboard_clear_seconds: u64,
    /// Тема оформления (ключ из ui::theme::THEMES).
    pub theme: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            last_database: None,
            auto_lock_minutes: 5,
            lock_on_minimize: false,
            clipboard_clear_seconds: 45,
            theme: "system".to_string(),
        }
    }
}

fn config_path() -> PathBuf {
    let base = std::env::var("XDG_CONFIG_HOME")
        .map(PathBuf::from)
        .unwrap_or_else(|_| {
            dirs_home().join(".config")
        });
    base.join("taynik").join("config.toml")
}

fn dirs_home() -> PathBuf {
    std::env::var("HOME").map(PathBuf::from).unwrap_or_else(|_| PathBuf::from("/tmp"))
}

impl Config {
    pub fn load() -> Self {
        std::fs::read_to_string(config_path())
            .ok()
            .and_then(|s| toml::from_str(&s).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) {
        let path = config_path();
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(s) = toml::to_string_pretty(self) {
            let _ = std::fs::write(path, s);
        }
    }
}
