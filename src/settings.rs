//! 用户设置的 JSON 持久化（等价于 Python 版的 `user_settings.json`）。
//!
//! 写入位置：`%APPDATA%/TypingBot/settings.json`（`dirs::config_dir()`）。

use serde::{Deserialize, Serialize};
use std::path::PathBuf;

fn default_speed() -> f32 {
    100.0
}

/// v2.0.1 的默认行为是暗色模式（仅在从旧设置文件迁移时使用）。
fn default_dark_mode() -> bool {
    true
}

/// 默认置顶（对齐 Python 版的 always-on-top）。
fn default_pinned() -> bool {
    true
}

/// 主题模式：跟随系统 / 固定浅色 / 固定深色。
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum ThemeMode {
    /// 跟随系统主题（默认）。
    System,
    Light,
    Dark,
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Settings {
    #[serde(default = "default_speed")]
    pub speed: f32,
    #[serde(default = "default_dark_mode")]
    pub dark_mode: bool,
    #[serde(default)]
    pub last_text: String,
    /// 窗口是否置顶（标题栏的图钉按钮）。
    #[serde(default = "default_pinned")]
    pub pinned: bool,
    /// 主题模式。`None` 表示设置文件里没有这个字段（旧版本写的），
    /// 这时按 `dark_mode` 迁移成显式的浅色/深色。
    #[serde(default)]
    pub theme: Option<ThemeMode>,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            speed: default_speed(),
            dark_mode: default_dark_mode(),
            last_text: String::new(),
            pinned: default_pinned(),
            // 全新安装默认跟随系统主题。
            theme: Some(ThemeMode::System),
        }
    }
}

impl Settings {
    /// 生效的主题模式：设置文件里没有该字段（旧版本写的）时默认跟随系统。
    pub fn theme_mode(&self) -> ThemeMode {
        self.theme.unwrap_or(ThemeMode::System)
    }
}

fn settings_path() -> Option<PathBuf> {
    dirs::config_dir().map(|dir| dir.join("TypingBot").join("settings.json"))
}

impl Settings {
    /// 从磁盘读取；文件缺失或损坏时回落到默认值。
    pub fn load() -> Self {
        let Some(path) = settings_path() else {
            return Self::default();
        };
        match std::fs::read_to_string(&path) {
            Ok(raw) => serde_json::from_str(&raw).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    /// 原子写入磁盘（失败静默，与 Python 版的 try/except 语义一致）。
    pub fn save(&self) {
        let Some(path) = settings_path() else {
            return;
        };
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        if let Ok(raw) = serde_json::to_string_pretty(self) {
            let tmp = path.with_extension("json.tmp");
            if std::fs::write(&tmp, &raw).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            } else {
                let _ = std::fs::write(&path, &raw);
            }
        }
    }
}
