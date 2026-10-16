// 不带控制台子系统：双击 exe 不会再弹出黑窗口。
// 日志只在调试构建里写文件，正式构建默认不碰用户目录（排查时可用 TB_LOG=1 打开）。
#![cfg_attr(windows, windows_subsystem = "windows")]

mod app;
mod control;
mod emoji;
mod engine;
mod hotkey;
mod settings;
mod theme;

use egui::ViewportBuilder;

/// 是否允许往磁盘写任何东西（日志、设置）。
///
/// 这是一个**单 exe 便携应用**：正式构建默认一个文件都不写 —— 不写日志、
/// 不写设置、不写窗口几何，整个程序不落任何文件（也不读任何历史状态）。
/// 只有开发时（`debug_assertions`）或显式设置环境变量 `TB_STATE=1`
/// （旧名 `TB_LOG=1` 仍可用）才会写，方便排查问题。
pub fn writes_enabled() -> bool {
    static ENABLED: std::sync::OnceLock<bool> = std::sync::OnceLock::new();
    *ENABLED.get_or_init(|| cfg!(debug_assertions) || env_flag("TB_STATE") || env_flag("TB_LOG"))
}

fn env_flag(name: &str) -> bool {
    std::env::var(name).is_ok_and(|v| v != "0" && !v.is_empty())
}

/// 写一行日志（仅在 [`writes_enabled`] 为真时）：stderr（有控制台时可见）
/// + `%APPDATA%/TypingBot/typingbot.log`。GUI 子系统下多数情况没有控制台，
/// 所以文件那份才是关键；写失败一律静默丢弃
/// （不能直接用 `eprintln!`：写失败会 panic）。
pub fn log_line(args: std::fmt::Arguments<'_>) {
    use std::io::Write as _;

    if !writes_enabled() {
        return;
    }

    let _ = writeln!(std::io::stderr(), "{args}");

    let Some(dir) = dirs::config_dir().map(|d| d.join("TypingBot")) else {
        return;
    };
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("typingbot.log"))
    {
        let _ = writeln!(file, "{args}");
    }
}

/// 窗口图标（与 exe 资源里的图标同一份设计，64×64 预乘前的原始 RGBA）。
fn window_icon() -> egui::IconData {
    egui::IconData {
        rgba: include_bytes!("../assets/icon_64.rgba").to_vec(),
        width: 64,
        height: 64,
    }
}

fn main() -> eframe::Result {
    let options = eframe::NativeOptions {
        viewport: ViewportBuilder::default()
            .with_title("TypingBot")
            // 空闲时刚好放下内容；打字时会自动增高以容纳实时预览卡片。
            .with_inner_size([420.0, 690.0])
            .with_min_inner_size([420.0, 630.0])
            .with_resizable(true)
            // 使用系统标题栏（可原生拖拽/最小化/关闭），与 Python 版一致；
            // 窗口内容里仍保留一层 Material 头部。
            .with_decorations(true)
            .with_icon(std::sync::Arc::new(window_icon()))
            // 默认置顶；标题栏的图钉按钮可随时切换（设置里也会记住）。
            .with_always_on_top(),
        ..Default::default()
    };

    eframe::run_native(
        "TypingBot",
        options,
        Box::new(|cc| Ok(Box::new(app::TypingBotApp::new(cc)))),
    )
}
