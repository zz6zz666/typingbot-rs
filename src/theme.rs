//! Material 3 调色板。
//!
//! 浅色主题沿用 Python 版（也是标准 M3 紫色种子）；深色主题改为基于同一
//! 紫色种子的 **Material 3 深色方案**（低饱和的紫 / 薰衣草 / 柔粉），
//! 替换掉 Python 版偏 Material 2 的高饱和配色（亮青 `#03DAC6`、深蓝紫
//! `#3700B3`），避免深色模式显得刺眼。

use egui::Color32;
/// 将 `0xRRGGBB` 在编译期转成 [`Color32`]。
const fn rgb(hex: u32) -> Color32 {
    Color32::from_rgb(
        ((hex >> 16) & 0xFF) as u8,
        ((hex >> 8) & 0xFF) as u8,
        (hex & 0xFF) as u8,
    )
}

#[derive(Clone, Copy)]
pub struct Palette {
    pub primary: Color32,
    pub on_primary: Color32,
    pub primary_container: Color32,
    pub on_primary_container: Color32,
    pub secondary: Color32,
    pub on_secondary: Color32,
    pub tertiary: Color32,
    pub on_tertiary: Color32,
    pub surface: Color32,
    pub surface_variant: Color32,
    pub background: Color32,
    pub error: Color32,
    pub on_error: Color32,
    pub on_surface: Color32,
    pub outline: Color32,
    /// 保留自 Python 调色板，当前 UI 未直接使用。
    #[allow(dead_code)]
    pub success: Color32,
}

impl Palette {
    /// 预览用的固定强调色，两个主题共用。
    pub const PREV_GREY: Color32 = rgb(0x757575);
    pub const CURRENT_GREEN: Color32 = rgb(0x66BB6A);
    pub const NEXT_ORANGE: Color32 = rgb(0xFFA726);

    pub const fn light() -> Self {
        Self {
            primary: rgb(0x6750A4),
            on_primary: rgb(0xFFFFFF),
            primary_container: rgb(0xEADDFF),
            on_primary_container: rgb(0x21005E),
            secondary: rgb(0x625B71),
            on_secondary: rgb(0xFFFFFF),
            tertiary: rgb(0x7D5260),
            on_tertiary: rgb(0xFFFFFF),
            surface: rgb(0xFEF7FF),
            surface_variant: rgb(0xE7E0EC),
            background: rgb(0xFEF7FF),
            error: rgb(0xB3261E),
            on_error: rgb(0xFFFFFF),
            on_surface: rgb(0x1C1B1F),
            outline: rgb(0x79747E),
            success: rgb(0x2E7D32),
        }
    }

    pub const fn dark() -> Self {
        Self {
            primary: rgb(0xD0BCFF),
            on_primary: rgb(0x381E72),
            primary_container: rgb(0x4F378B),
            on_primary_container: rgb(0xEADDFF),
            secondary: rgb(0xCCC2DC),
            on_secondary: rgb(0x332D41),
            tertiary: rgb(0xEFB8C8),
            on_tertiary: rgb(0x492532),
            // surface 与 background 略有差异，卡片才有分层感。
            surface: rgb(0x1C1B1F),
            surface_variant: rgb(0x49454F),
            background: rgb(0x141218),
            error: rgb(0xF2B8B5),
            on_error: rgb(0x601410),
            on_surface: rgb(0xE6E1E5),
            outline: rgb(0x938F99),
            success: rgb(0xA5D6A7),
        }
    }

    pub fn for_dark(dark: bool) -> Self {
        if dark { Self::dark() } else { Self::light() }
    }
}
