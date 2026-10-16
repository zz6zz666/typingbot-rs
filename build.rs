//! 把图标与版本信息编进 Windows 可执行文件（资源段）。
//!
//! 没有这一步，`typingbot-rs.exe` 在资源管理器/任务栏里显示的是通用图标。
//! 依赖 Windows SDK 的 `rc.exe`（`winresource` 会自动定位）；找不到时只警告，
//! 不让整个构建失败。

fn main() {
    println!("cargo:rerun-if-changed=assets/icon.ico");
    println!("cargo:rerun-if-changed=build.rs");

    #[cfg(windows)]
    {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("assets/icon.ico");
        res.set("ProductName", "TypingBot");
        res.set("FileDescription", "TypingBot (Rust Edition)");
        res.set("CompanyName", "zz6zz666");
        res.set("LegalCopyright", "Copyright (C) 2026 zz6zz666");
        res.set("OriginalFilename", "typingbot-rs.exe");
        res.set("InternalName", "typingbot-rs");
        res.set("FileVersion", "3.0.0");
        res.set("ProductVersion", "3.0.0");
        // 与 app.rs 里的 APP_VERSION = "3.0.0" 保持一致。
        res.set_version_info(winresource::VersionInfo::FILEVERSION, 0x0003_0000_0000_0000);
        res.set_version_info(
            winresource::VersionInfo::PRODUCTVERSION,
            0x0003_0000_0000_0000,
        );
        if let Err(err) = res.compile() {
            println!("cargo:warning=Windows 资源编译失败（exe 将不带图标/版本信息）: {err}");
        }
    }
}
