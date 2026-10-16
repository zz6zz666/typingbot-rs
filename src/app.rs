//! egui 界面层：无边框置顶可拖拽悬浮窗，功能对齐 Python 版。

use crate::{
    control::{Control, Reason},
    engine, hotkey,
    settings::{Settings, ThemeMode},
    theme::Palette,
};
use eframe::egui;
use std::sync::Arc;
use std::time::{Duration, Instant};

/// 版本号（对齐 Python 版 `version.json` / `APP_VERSION`）。
pub const APP_VERSION: &str = "3.0.0";
const APP_VERSION_NAME: &str = "Rust Edition";

/// 倒计时秒数。
const COUNTDOWN_SECS: f32 = 3.0;

/// 倒计时浮窗尺寸（对齐 Python 版 `create_countdown_window` 的 250×120）。
const COUNTDOWN_WINDOW: egui::Vec2 = egui::vec2(260.0, 120.0);

/// 倒计时浮窗距离屏幕底部的距离。
const COUNTDOWN_BOTTOM_MARGIN: f32 = 100.0;

/// 点了 Stop / 按了 ESC 之后，最多等输入线程多久。超过就认定它卡住了，
/// 界面直接回到空闲（并让该线程作废），避免整个窗口被一次卡死的输入锁死。
const STOP_GRACE: Duration = Duration::from_millis(1500);

// ---------------------------------------------------------------------------
// 统一的排版 / 间距尺度。所有界面元素都从这里取值，避免“忽大忽小”。
// ---------------------------------------------------------------------------

/// 头部标题。
const FS_TITLE: f32 = 24.0;
/// 分区标题（“Text to Type” / “Typing Speed”）。
const FS_SECTION: f32 = 16.0;
/// 正文：输入框、按钮、说明文字。
const FS_BODY: f32 = 15.0;
/// 次要标签：Slow/Fast/Ultra-fast、页脚。
const FS_SMALL: f32 = 13.0;
/// 速度读数：数值与单位同字号，避免大小不一。
const FS_BADGE: f32 = 16.0;
/// 倒计时数字（大号展示）。
const FS_DISPLAY: f32 = 56.0;

/// 输入框行高（正文 1.5 倍）。
const EDITOR_LINE_HEIGHT: f32 = 22.0;

/// 分区之间的间距。
const GAP_SECTION: f32 = 18.0;
/// 分区标题与其控件之间的间距。
const GAP_LABEL: f32 = 10.0;
/// 同一分区内相邻行之间的间距。
const GAP_ROW: f32 = 10.0;

/// 头部高度与按钮高度。
const HEADER_HEIGHT: f32 = 56.0;
const CONTROL_HEIGHT: f32 = 42.0;

/// 默认窗口宽度：比 400 略宽几个像素，保证底部应用信息（footer）单行完整显示。
const WINDOW_WIDTH: f32 = 420.0;
/// 空闲时窗口高度：比内容略高一点，底部不至于局促。
const IDLE_HEIGHT: f32 = 690.0;
/// 打字时的高度：多出实时预览卡片的空间；结束后收回。
const TYPING_HEIGHT: f32 = 826.0;

/// 输入框最小高度（约 4~5 行）。输入框本身会随窗口高度伸缩，
/// 这样窗口变高时多出来的空间全部给输入框，输入框以下的控件位置不受影响。
const MIN_EDITOR_HEIGHT: f32 = 100.0;
/// 输入框外框的固定开销：上下内边距 + 描边（计算可用高度时要扣掉）。
const EDITOR_CHROME: f32 = 14.0 * 2.0 + 2.0;
/// 卡片与页脚之间的间距（页脚自身高度另算）。
const FOOTER_GAP: f32 = 12.0;

// 图标：使用 Windows 自带的 “Segoe Fluent Icons” 真实矢量图标（而非 emoji
// 占位），码位来自 Private Use Area。字形与文字用空格分隔。
const ICON_KEYBOARD: &str = "\u{E765}";
const ICON_PLAY: &str = "\u{E768}";
const ICON_PAUSE: &str = "\u{E769}";
const ICON_STOP: &str = "\u{E71A}";
const ICON_DELETE: &str = "\u{E74D}";
const ICON_PASTE: &str = "\u{E77F}";
const ICON_INFO: &str = "\u{E946}";
/// 图标与文字之间的空隙。
const ICON_GAP: &str = "  ";

/// Segoe Fluent Icons：图钉（置顶开关）。
const ICON_PIN: &str = "\u{E718}";
/// Segoe Fluent Icons：警示 / 月亮 / 太阳。
const ICON_WARNING: &str = "\u{E7BA}";
const ICON_MOON: &str = "\u{E708}";
const ICON_SUNNY: &str = "\u{E706}";

/// 提示条文案统一写成「图标 + 空隙 + 文字」，图标一律用字体里的 UI 图标，
/// 不用 emoji 充数。
fn notice(icon: &str, text: &str) -> String {
    format!("{icon}{ICON_GAP}{text}")
}

/// 作者与项目地址（本 fork / 移植版的作者）。
const AUTHOR_NAME: &str = "zz6zz666";
const GITHUB_URL: &str = "https://github.com/zz6zz666/typingbot-rs";
const AUTHOR_URL: &str = "https://github.com/zz6zz666";

enum Phase {
    Idle,
    Countdown { started: Instant },
    Typing,
}

/// 过渡尺寸：倒计时浮窗（260×120）与打字临时高度（826）。
///
/// 这两种尺寸只是运行中的临时状态，不能当作“常规尺寸”的基准，
/// 也不能在上次退出时被 eframe 存下来，否则下次启动窗口会偏高或变成一条。
fn is_transitional_size(size: egui::Vec2) -> bool {
    (size.y - TYPING_HEIGHT).abs() < 2.0
        || (size.y - COUNTDOWN_WINDOW.y).abs() < 2.0
        || (size.x - COUNTDOWN_WINDOW.x).abs() < 2.0
}

pub struct TypingBotApp {
    text: String,
    speed: f32,
    dark: bool,
    palette: Palette,

    control: Arc<Control>,
    settings: Settings,
    dirty: bool,

    phase: Phase,
    /// (消息, 是否错误, 出现时间)
    notification: Option<(String, bool, Instant)>,

    applied_dark: Option<bool>,
    placed_window: bool,
    was_active: bool,
    /// 进入倒计时前主窗口的位置与尺寸，倒计时结束后原样恢复。
    window_restore: Option<(egui::Pos2, egui::Vec2)>,
    /// 为容纳实时预览而临时增高前的窗口尺寸，打字结束后原样恢复。
    height_before_typing: Option<egui::Vec2>,
    /// 主窗口的“常规”尺寸（内容区）。只在空闲状态、且不在过渡动画中时更新，
    /// 所有自适应尺寸都以此为准，避免把倒计时浮窗 / 打字临时尺寸当成基准。
    base_size: egui::Vec2,
    /// 卡片内“输入框以下”所有控件的高度（上一帧实测），用于计算输入框该有多高。
    below_h: f32,
    /// 当前生效的最小窗口高度（按内容实测，保证缩到最小时输入框仍不小于最小值）。
    min_window_h: f32,
    /// 彩色 emoji 渲染器（系统 emoji 字体不可用时为 `None`，界面自动退回单色）。
    emoji: Option<crate::emoji::EmojiRenderer>,
    /// 窗口是否置顶（标题栏图钉）。默认开启。
    pinned: bool,
    applied_pinned: Option<bool>,
    /// 主题模式（跟随系统 / 浅色 / 深色）。
    theme_mode: ThemeMode,
    /// 主窗口的 HWND（去标题栏/去最大化按钮等 Win32 调整用）。
    window_hwnd: Option<usize>,
    /// 请求停止后的宽限截止时间，见 [`STOP_GRACE`]。
    stop_deadline: Option<Instant>,
    /// 只在启动头几帧执行一次的“纠正上次遗留的过渡窗口高度”。
    size_repaired: bool,
    /// 开发/回归测试开关：`TB_AUTOSTART=1` 时启动即开始输入（跳过倒计时）。
    auto_start: bool,
}

impl TypingBotApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        install_fonts(&cc.egui_ctx);

        let settings = cc
            .storage
            .and_then(|s| s.get_string("settings"))
            .and_then(|raw| serde_json::from_str(&raw).ok())
            .unwrap_or_else(Settings::load);

        let theme_mode = settings.theme_mode();
        let dark = match theme_mode {
            ThemeMode::Dark => true,
            ThemeMode::Light => false,
            // 第一帧可能还没拿到系统主题，先用上次记住的实际状态，避免闪一下。
            ThemeMode::System => cc
                .egui_ctx
                .system_theme()
                .map(|t| t == egui::Theme::Dark)
                .unwrap_or(settings.dark_mode),
        };
        let pinned = settings.pinned;
        let control = Arc::new(Control::new());
        // 全局 ESC 监听常驻，无论是否在打字。
        hotkey::spawn(Arc::clone(&control));

        Self {
            text: settings.last_text.clone(),
            speed: settings.speed,
            dark,
            palette: Palette::for_dark(dark),
            control,
            settings,
            dirty: false,
            phase: Phase::Idle,
            notification: None,
            applied_dark: None,
            // eframe 的 persistence 已还原窗口位置时无需再定位。
            placed_window: cc.storage.is_some(),
            was_active: false,
            window_restore: None,
            height_before_typing: None,
            base_size: egui::vec2(WINDOW_WIDTH, IDLE_HEIGHT),
            below_h: 380.0,
            min_window_h: 630.0,
            emoji: crate::emoji::EmojiRenderer::load(&[&format!("{}\\seguiemj.ttf", fonts_dir())]),
            pinned,
            applied_pinned: None,
            theme_mode,
            window_hwnd: window_handle(cc),
            stop_deadline: None,
            size_repaired: false,
            auto_start: std::env::var("TB_AUTOSTART").is_ok_and(|v| v != "0" && !v.is_empty()),
        }
    }

    fn is_busy(&self) -> bool {
        !matches!(self.phase, Phase::Idle)
    }

    /// 提示条样式对齐 web 版：普通消息用 surface 底 + on_surface 字，
    /// 错误消息用 error 底 + 白字。
    fn notify(&mut self, message: &str, is_error: bool) {
        self.notification = Some((message.to_string(), is_error, Instant::now()));
    }

    fn mark_dirty(&mut self) {
        self.dirty = true;
    }

    fn persist(&mut self) {
        if self.dirty {
            self.settings.speed = self.speed;
            self.settings.dark_mode = self.dark;
            self.settings.theme = Some(self.theme_mode);
            self.settings.last_text = self.text.clone();
            self.settings.pinned = self.pinned;
            self.settings.save();
            self.dirty = false;
        }
    }

    // ---------------------------------------------------------------- 动作

    /// 主题按钮：在 跟随系统 → 深色 → 浅色 → 跟随系统 之间循环。
    fn toggle_dark(&mut self) {
        self.theme_mode = match self.theme_mode {
            ThemeMode::System => ThemeMode::Dark,
            ThemeMode::Dark => ThemeMode::Light,
            ThemeMode::Light => ThemeMode::System,
        };
        self.applied_dark = None; // 强制重新求值
        let label = match self.theme_mode {
            ThemeMode::System => "Theme: follow system",
            ThemeMode::Dark => "Theme: dark",
            ThemeMode::Light => "Theme: light",
        };
        let icon = if self.dark { ICON_MOON } else { ICON_SUNNY };
        self.notify(&notice(icon, label), false);
        self.mark_dirty();
    }

    fn paste_from_clipboard(&mut self) {
        match arboard::Clipboard::new() {
            Ok(mut clipboard) => match clipboard.get_text() {
                Ok(text) if !text.trim().is_empty() => {
                    self.text = text;
                    self.notify(&notice(ICON_PASTE, "Pasted from clipboard"), false);
                    self.mark_dirty();
                }
                Ok(_) => self.notify(&notice(ICON_WARNING, "Clipboard is empty"), true),
                Err(_) => self.notify(&notice(ICON_WARNING, "Cannot access clipboard"), true),
            },
            Err(_) => self.notify(&notice(ICON_WARNING, "Cannot access clipboard"), true),
        }
    }

    fn clear_text(&mut self) {
        self.text.clear();
        self.notify(&notice(ICON_DELETE, "Text cleared"), false);
        self.mark_dirty();
    }

    fn start_countdown(&mut self, ctx: &egui::Context) {
        if self.text.trim().is_empty() {
            self.notify(&notice(ICON_WARNING, "Please enter some text"), true);
            return;
        }
        // 清掉可能残留的停止标志（例如空闲时按过 ESC）。
        self.control.reset();
        self.enter_countdown_window(ctx);
        self.phase = Phase::Countdown {
            started: Instant::now(),
        };
    }

    /// 收缩主窗口为屏幕底部居中的小倒计时浮窗。
    ///
    /// Python 版在倒计时期间 `withdraw()` 主窗口并单独弹出一个小窗，这里改为
    /// 直接复用主窗口，避免遮挡用户要点击的目标。
    fn enter_countdown_window(&mut self, ctx: &egui::Context) {
        let vp = ctx.input(|i| i.viewport().clone());
        // 位置取当前实际位置；尺寸用记录的“常规尺寸”，不用视口的临时值。
        let pos = vp.outer_rect.map(|r| r.min).unwrap_or(egui::pos2(0.0, 0.0));
        self.window_restore = Some((pos, self.base_size));
        // 倒计时浮窗不需要标题栏（egui 的 Decorations 命令在这里不生效，直接改窗口样式）。
        set_borderless(self.window_hwnd, true);
        if let Some(monitor) = vp.monitor_size {
            let x = ((monitor.x - COUNTDOWN_WINDOW.x) / 2.0).max(0.0);
            let y = (monitor.y - COUNTDOWN_WINDOW.y - COUNTDOWN_BOTTOM_MARGIN).max(0.0);
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
        }
        // 倒计时浮窗比常规窗口小得多：先放宽最小尺寸，否则会被窗口管理器挡住。
        ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(COUNTDOWN_WINDOW));
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(COUNTDOWN_WINDOW));
        ctx.request_repaint();
    }

    /// 恢复进入倒计时前的主窗口位置与尺寸。
    fn restore_window(&mut self, ctx: &egui::Context) {
        if let Some((pos, size)) = self.window_restore.take() {
            // 顺序很重要：先恢复最小尺寸与标题栏，再放大，最后摆位置。
            ctx.send_viewport_cmd(egui::ViewportCommand::MinInnerSize(egui::vec2(
                WINDOW_WIDTH,
                self.min_window_h,
            )));
            set_borderless(self.window_hwnd, false);
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(pos));
            self.base_size = size;
            ctx.request_repaint();
        }
    }

    /// 倒计时期间取消（Stop 按钮 / ESC）。
    fn cancel_countdown(&mut self, ctx: &egui::Context) {
        self.control.reset();
        self.restore_window(ctx);
        self.phase = Phase::Idle;
        self.notify(&notice(ICON_STOP, "Typing stopped"), false);
    }

    fn toggle_pause(&mut self) {
        if !self.control.is_active() {
            return;
        }
        self.control.toggle_pause();
        if self.control.is_paused() {
            self.notify(&notice(ICON_PAUSE, "Typing paused"), false);
        } else {
            self.notify(&notice(ICON_PLAY, "Typing resumed"), false);
        }
    }

    fn stop_typing(&mut self, ctx: &egui::Context) {
        if matches!(self.phase, Phase::Countdown { .. }) {
            self.cancel_countdown(ctx);
        } else {
            self.control.request_stop();
        }
    }

    /// 按给定尺寸调整窗口内容区。
    fn set_inner_size(&self, ctx: &egui::Context, size: egui::Vec2) {
        ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(size));
        ctx.request_repaint();
    }

    /// 打字开始时临时增高窗口以容纳实时预览卡片。
    ///
    /// 基准尺寸取自 `base_size`（空闲时记录的常规尺寸）。**不能**读取当前视口尺寸：
    /// 倒计时刚结束时视口还停留在浮窗的 260×120 上，用它做基准会把窗口缩成一条。
    fn grow_for_typing(&mut self, ctx: &egui::Context) {
        let base = self.base_size;
        if base.y + 1.0 < TYPING_HEIGHT {
            self.height_before_typing = Some(base);
            self.set_inner_size(ctx, egui::vec2(base.x, TYPING_HEIGHT));
        } else {
            self.height_before_typing = None;
        }
    }

    /// 打字结束：恢复到打字前的尺寸（宽高一起，避免残留临时宽度）。
    fn shrink_after_typing(&mut self, ctx: &egui::Context) {
        if let Some(size) = self.height_before_typing.take() {
            self.base_size = size;
            self.set_inner_size(ctx, size);
        }
    }

    /// 工作线程刚结束：复位界面状态并给出提示。
    fn on_typing_end(&mut self, ctx: &egui::Context) {
        let reason = self.control.take_reason();
        self.control.reset();
        self.stop_deadline = None;
        self.phase = Phase::Idle;
        self.shrink_after_typing(ctx);
        if reason == Some(Reason::Cancelled) {
            self.notify(&notice(ICON_STOP, "Typing stopped"), false);
        }
    }

    /// 强制回到空闲：输入线程卡在系统调用里出不来时用（见 [`STOP_GRACE`]）。
    ///
    /// `reset()` 会推进代数，卡住的那个线程一旦返回就会自行结束，不会再输入。
    fn force_idle(&mut self, ctx: &egui::Context) {
        crate::log_line(format_args!(
            "[typingbot] 输入线程在停止请求后仍未响应，已强制回到空闲"
        ));
        self.control.reset();
        self.stop_deadline = None;
        self.phase = Phase::Idle;
        self.shrink_after_typing(ctx);
        self.notify(&notice(ICON_STOP, "Typing stopped"), false);
    }

    /// 推进倒计时；到期则恢复主窗口并真正开打。
    fn advance_countdown(&mut self, ctx: &egui::Context) {
        let Phase::Countdown { started } = self.phase else {
            return;
        };
        if started.elapsed().as_secs_f32() < COUNTDOWN_SECS {
            return;
        }
        self.restore_window(ctx);
        let text = self.text.trim().to_string();
        if text.is_empty() {
            self.notify(&notice(ICON_WARNING, "Please enter some text"), true);
            self.phase = Phase::Idle;
            return;
        }
        engine::start(text, self.speed, Arc::clone(&self.control));
        self.grow_for_typing(ctx);
        self.phase = Phase::Typing;
    }
}

// ------------------------------------------------------------------ 绘制

impl TypingBotApp {
    fn place_window_once(&mut self, ctx: &egui::Context) {
        if self.placed_window {
            return;
        }
        // 首次启动放到屏幕右上角（对齐 Python 版的 run()）。
        if let Some(monitor) = ctx.input(|i| i.viewport().monitor_size) {
            let (w, h) = (WINDOW_WIDTH, IDLE_HEIGHT);
            let x = (monitor.x - w - 20.0).max(0.0);
            let y = 20.0;
            ctx.send_viewport_cmd(egui::ViewportCommand::OuterPosition(egui::pos2(x, y)));
            ctx.send_viewport_cmd(egui::ViewportCommand::InnerSize(egui::vec2(w, h)));
            self.placed_window = true;
        } else {
            // 显示器信息还没就绪，下一帧再试。
            ctx.request_repaint();
        }
    }

    /// 当前生效的深浅色：跟随系统模式下取系统主题（拿不到时返回 `None`，沿用当前值）。
    ///
    /// 优先直接读 Windows 的 `AppsUseLightTheme`（注册表，系统主题的唯一真实来源），
    /// 这样不依赖 winit 是否把 `ThemeChanged` 事件送到 egui —— 用户改系统主题、
    /// 或应用启动时系统已是深色，都能正确跟随。
    fn effective_dark(&self, ctx: &egui::Context) -> Option<bool> {
        match self.theme_mode {
            ThemeMode::Light => Some(false),
            ThemeMode::Dark => Some(true),
            ThemeMode::System => {
                if let Some(dark) = os_prefers_dark() {
                    return Some(dark);
                }
                ctx.system_theme().map(|t| t == egui::Theme::Dark)
            }
        }
    }

    fn sync_theme(&mut self, ctx: &egui::Context) {
        // 跟随系统模式：系统主题变了要立刻跟上。
        if self.theme_mode == ThemeMode::System {
            ctx.request_repaint_after(Duration::from_millis(500));
        }
        let dark = self.effective_dark(ctx).unwrap_or(self.dark);
        if self.applied_dark == Some(dark) {
            return;
        }
        self.dark = dark;
        self.palette = Palette::for_dark(dark);
        ctx.set_visuals(if dark {
            egui::Visuals::dark()
        } else {
            egui::Visuals::light()
        });
        self.applied_dark = Some(dark);
        ctx.request_repaint();
    }

    /// 下发置顶状态（只在变化时发送命令，避免每帧都调系统接口）。
    fn sync_pinned(&mut self, ctx: &egui::Context) {
        if self.applied_pinned == Some(self.pinned) {
            return;
        }
        ctx.send_viewport_cmd(egui::ViewportCommand::WindowLevel(if self.pinned {
            egui::WindowLevel::AlwaysOnTop
        } else {
            egui::WindowLevel::Normal
        }));
        self.applied_pinned = Some(self.pinned);
    }

    fn header(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let width = ui.available_width();
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(width, HEADER_HEIGHT), egui::Sense::hover());
        // 只有上面两角是圆的，和下方卡片拼成一张卡（对齐 web 的 .header）。
        let radius = egui::CornerRadius {
            nw: 18,
            ne: 18,
            sw: 0,
            se: 0,
        };
        ui.painter().add(elevation().as_shape(rect, radius));
        ui.painter().rect_filled(rect, radius, p.primary);

        // 系统标题栏已可拖拽；这里也支持直接拖动 Material 头部。
        let drag = ui.interact(rect, ui.id().with("titlebar"), egui::Sense::drag());
        if drag.drag_started() {
            ui.ctx().send_viewport_cmd(egui::ViewportCommand::StartDrag);
        }

        let mut child = ui.new_child(egui::UiBuilder::new().max_rect(rect));
        child.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);
        child.horizontal_centered(|ui| {
            ui.add_space(24.0);
            ui.label(
                egui::RichText::new(ICON_KEYBOARD)
                    .size(28.0)
                    .color(p.on_primary),
            );
            ui.label(
                egui::RichText::new("TypingBot")
                    .size(FS_TITLE)
                    .color(p.on_primary),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_space(12.0);
                self.theme_button(ui);
                self.pin_button(ui);
            });
        });
    }

    /// 深色 / 浅色模式开关。
    ///
    /// 图形全部自绘：字体图标（Segoe Fluent Icons 的月亮）自带左右留白、在小尺寸下
    /// 看起来不居中也不对称，这里改为在 40×40 的透明按钮里画一个精确居中的太阳
    /// （深色模式）或月牙（浅色模式）。
    fn theme_button(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let color = if resp.hovered() {
            p.on_primary
        } else {
            p.on_primary.gamma_multiply(0.85)
        };
        let center = rect.center();
        let dark = self.dark;
        let system = self.theme_mode == ThemeMode::System;
        {
            let painter = ui.painter();
            if dark {
                // 当前是深色模式：画太阳（中心圆 + 8 道光芒），左右上下完全对称。
                painter.circle_filled(center, 4.4, color);
                for i in 0..8 {
                    let angle = std::f32::consts::TAU * (i as f32) / 8.0;
                    let (sin, cos) = angle.sin_cos();
                    painter.line_segment(
                        [
                            center + egui::vec2(cos, sin) * 7.0,
                            center + egui::vec2(cos, sin) * 9.8,
                        ],
                        egui::Stroke::new(1.8, color),
                    );
                }
            } else {
                // 当前是浅色模式：画月牙（实心圆 + 用头部底色“咬”出缺口）。
                painter.circle_filled(center, 6.6, color);
                painter.circle_filled(center + egui::vec2(-2.6, -2.8), 5.8, p.primary);
            }
        }
        if system {
            // 跟随系统主题时，图标下方点一个小点作为标识。
            ui.painter()
                .circle_filled(center + egui::vec2(0.0, 12.5), 2.0, color);
        }
        let tip = match self.theme_mode {
            ThemeMode::System => "Theme: follow system (click to change)",
            ThemeMode::Dark => "Theme: dark (click to change)",
            ThemeMode::Light => "Theme: light (click to change)",
        };
        if resp.on_hover_text(tip).clicked() {
            self.toggle_dark();
        }
    }

    /// 置顶开关（标题栏里的图钉）。默认开启。
    fn pin_button(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let (rect, resp) = ui.allocate_exact_size(egui::vec2(40.0, 40.0), egui::Sense::click());
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);
        let painter = ui.painter();
        // 开启时垫一层半透明圆底，状态一眼可见；关闭时只在悬停时给反馈。
        if self.pinned {
            painter.circle_filled(rect.center(), 15.0, p.on_primary.gamma_multiply(0.20));
        } else if resp.hovered() {
            painter.circle_filled(rect.center(), 15.0, p.on_primary.gamma_multiply(0.10));
        }
        let color = if self.pinned {
            p.on_primary
        } else {
            p.on_primary.gamma_multiply(0.65)
        };
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            ICON_PIN,
            egui::FontId::proportional(16.0),
            color,
        );
        let tip = if self.pinned {
            "Always on top: on"
        } else {
            "Always on top: off"
        };
        if resp.on_hover_text(tip).clicked() {
            self.toggle_pinned();
        }
    }

    fn toggle_pinned(&mut self) {
        self.pinned = !self.pinned;
        self.dirty = true;
        let msg = if self.pinned {
            notice(ICON_PIN, "Always on top enabled")
        } else {
            notice(ICON_PIN, "Always on top disabled")
        };
        self.notify(&msg, false);
    }

    /// 输入框：高度由外部给定（= 卡片可用高度扣掉下方所有控件之后的剩余）。
    /// 这样窗口变高时多出来的空间全部给输入框，输入框以下的控件位置不变。
    fn editor_section(&mut self, ui: &mut egui::Ui, editor_h: f32) {
        let p = self.palette;
        // 彩色 emoji 渲染器先取出来用，避免与下面 `&mut self.text` 的借用冲突。
        let mut emoji = self.emoji.take();
        let mut emoji_marks: Vec<(char, egui::Pos2)> = Vec::new();
        ui.label(
            egui::RichText::new("Text to Type")
                .size(FS_SECTION)
                .color(p.on_surface),
        );
        ui.add_space(GAP_LABEL);

        // 打字过程中对输入框做逐字高亮（对齐 Python 版 `update_text_highlighting`）。
        let highlight = self.control.is_active();
        let index = self.control.index();
        egui::Frame::new()
            .fill(p.surface_variant)
            .stroke(egui::Stroke::new(2.0, p.outline))
            .inner_margin(egui::Margin::same(14))
            .corner_radius(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                // 高度由外部给定；文字超出后在框内滚动，不再撑大窗口。
                egui::ScrollArea::vertical()
                    .id_salt("editor_scroll")
                    .max_height(editor_h)
                    .auto_shrink([false, false])
                    .scroll_bar_visibility(egui::scroll_area::ScrollBarVisibility::AlwaysHidden)
                    .show(ui, |ui| {
                        ui.set_width(ui.available_width());
                        // 自定义 layouter：按进度给每个字符上色，其余行为与默认一致。
                        let mut layouter =
                            |ui: &egui::Ui, buf: &dyn egui::TextBuffer, wrap_width: f32| {
                                let job = highlight_job(
                                    ui,
                                    buf.as_str(),
                                    highlight,
                                    index,
                                    p,
                                    wrap_width,
                                );
                                let mut galley =
                                    align_baselines(ui.fonts_mut(|f| f.layout_job(job)));
                                // 彩色 emoji：记下字形位置并隐掉自带的单色轮廓。
                                if let Some(renderer) = emoji.as_ref() {
                                    crate::emoji::collect_marks(
                                        std::sync::Arc::make_mut(&mut galley),
                                        buf.as_str(),
                                        renderer,
                                        &mut emoji_marks,
                                    );
                                }
                                galley
                            };
                        let resp = ui.add_enabled(
                            !self.is_busy(),
                            egui::TextEdit::multiline(&mut self.text)
                                // 外层 Frame 已提供底/边框，这里用无边框 Frame。
                                .frame(egui::Frame::new())
                                .desired_rows(4)
                                // 关键：不设置的话 egui 会用 `spacing.text_edit_width`
                                // （默认 280pt）限制宽度，中文就会提前换行、右侧留一大片空白。
                                .desired_width(f32::INFINITY)
                                .hint_text("Type or paste text here")
                                .text_color(p.on_surface)
                                .font(egui::FontId::proportional(FS_BODY))
                                .layouter(&mut layouter),
                        );
                        if resp.changed() {
                            self.mark_dirty();
                        }

                        // 把彩色贴图盖到对应字形上（同一 Ui 内、后画 = 在上层，
                        // 且自动跟随 ScrollArea 的裁剪与滚动偏移）。
                        if let Some(renderer) = emoji.as_mut() {
                            let ppp = ui.ctx().pixels_per_point();
                            let px = FS_BODY * ppp;
                            let unit = FS_BODY / renderer.upem();
                            let origin = resp.rect.min;
                            for (ch, at) in emoji_marks.drain(..) {
                                let Some((tex, bbox)) = renderer.texture(ui.ctx(), ch, px) else {
                                    continue;
                                };
                                let dst = egui::Rect::from_min_max(
                                    origin
                                        + egui::vec2(
                                            at.x + bbox.min.x * unit,
                                            at.y - bbox.max.y * unit,
                                        ),
                                    origin
                                        + egui::vec2(
                                            at.x + bbox.max.x * unit,
                                            at.y - bbox.min.y * unit,
                                        ),
                                );
                                ui.painter().image(
                                    tex.id(),
                                    dst,
                                    egui::Rect::from_min_max(
                                        egui::pos2(0.0, 0.0),
                                        egui::pos2(1.0, 1.0),
                                    ),
                                    egui::Color32::WHITE,
                                );
                            }
                        }
                    });
            });
        self.emoji = emoji;
    }

    /// Paste / Clear 一行。
    fn paste_row(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        ui.horizontal(|ui| {
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{ICON_PASTE}{ICON_GAP}Paste"))
                            .size(FS_BODY)
                            .color(p.on_secondary),
                    )
                    .fill(p.secondary)
                    .corner_radius(18)
                    .min_size(egui::vec2(120.0, CONTROL_HEIGHT)),
                )
                .clicked()
            {
                self.paste_from_clipboard();
            }
            // Clear 为文字按钮（对齐 web 的 .btn-text）。
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(format!("{ICON_DELETE}{ICON_GAP}Clear"))
                            .size(FS_BODY)
                            .color(p.primary),
                    )
                    .fill(egui::Color32::TRANSPARENT)
                    .corner_radius(18)
                    .min_size(egui::vec2(96.0, CONTROL_HEIGHT)),
                )
                .clicked()
            {
                self.clear_text();
            }
        });
    }

    fn speed_section(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        ui.label(
            egui::RichText::new("Typing Speed")
                .size(FS_SECTION)
                .color(p.on_surface),
        );
        ui.add_space(GAP_LABEL);
        ui.horizontal(|ui| {
            // 滑块撑满整行（对齐 web 的 `.slider { flex: 1 }`），右侧只留数值文字。
            let gap = ui.spacing().item_spacing.x;
            let value_w = 78.0;
            let slider_w = (ui.available_width() - value_w - gap).max(80.0);
            self.speed_slider(ui, slider_w);
            // 速度读数：不再用气泡底，直接右对齐显示“100 WPS”，更紧凑也更透气。
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{} WPS", self.speed.round() as i32))
                        .size(FS_BADGE)
                        .color(p.primary),
                );
            });
        });
        // Slow / Fast / Ultra-fast 三档标签（对齐 web 的 .speed-labels）：
        // 左对齐 / 居中 / 右对齐，直接在一条带上按位置绘制，避免列布局换行。
        ui.add_space(8.0);
        let (rect, _) =
            ui.allocate_exact_size(egui::vec2(ui.available_width(), 16.0), egui::Sense::hover());
        let painter = ui.painter();
        let font = egui::FontId::proportional(FS_SMALL);
        let left = painter.layout("Slow".into(), font.clone(), p.outline, f32::INFINITY);
        let center = painter.layout("Fast".into(), font.clone(), p.outline, f32::INFINITY);
        let right = painter.layout("Ultra-fast".into(), font, p.outline, f32::INFINITY);
        painter.galley(egui::pos2(rect.left(), rect.top()), left, p.outline);
        painter.galley(
            egui::pos2(rect.center().x - center.size().x / 2.0, rect.top()),
            center,
            p.outline,
        );
        painter.galley(
            egui::pos2(rect.right() - right.size().x, rect.top()),
            right,
            p.outline,
        );
    }

    /// 自绘速度滑块（对齐 web 的 range 外观）：surface_variant 轨道 + primary 圆形滑块，
    /// 无“已填充”轨道。支持点击/拖动。
    fn speed_slider(&mut self, ui: &mut egui::Ui, width: f32) {
        let p = self.palette;
        let (rect, resp) =
            ui.allocate_exact_size(egui::vec2(width, 24.0), egui::Sense::click_and_drag());
        let resp = resp.on_hover_cursor(egui::CursorIcon::PointingHand);

        let handle_r = 10.0_f32;
        let x0 = rect.left() + handle_r;
        let x1 = rect.right() - handle_r;

        if resp.dragged() || resp.clicked() {
            if let Some(pos) = resp.interact_pointer_pos() {
                let f = ((pos.x - x0) / (x1 - x0)).clamp(0.0, 1.0);
                // web 的 range 步进是 1，这里同样取整。
                self.speed = (1.0 + f * 499.0).round();
                self.mark_dirty();
            }
        }
        let frac = ((self.speed - 1.0) / 499.0).clamp(0.0, 1.0);
        let hx = x0 + frac * (x1 - x0);

        let painter = ui.painter();
        let track = egui::Rect::from_center_size(rect.center(), egui::vec2(rect.width(), 6.0));
        painter.rect_filled(track, egui::CornerRadius::same(3), p.surface_variant);
        // 滑块：primary 圆 + 轻微描边，保证与轨道有区分。
        painter.add(elevation().as_shape(
            egui::Rect::from_center_size(
                egui::pos2(hx, rect.center().y),
                egui::vec2(handle_r * 2.0, handle_r * 2.0),
            ),
            egui::CornerRadius::same(handle_r as u8),
        ));
        painter.circle(
            egui::pos2(hx, rect.center().y),
            handle_r,
            p.primary,
            egui::Stroke::NONE,
        );
    }

    fn control_section(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        let typing = matches!(self.phase, Phase::Typing);
        // 倒计时阶段也允许按 Stop / ESC 取消。
        let cancellable = typing || matches!(self.phase, Phase::Countdown { .. });
        let ctx = ui.ctx().clone();
        let start_enabled = !self.is_busy();
        // 窄窗口下三个按钮排一行会挤爆文字：改为两行——
        // 第一行 Start Typing 占满整行，第二行 Pause / Stop 各占一半。
        // 自绘按钮保证“图标 + 文字”整组左右留白完全对称。
        let spacing = ui.spacing().item_spacing.x;
        let full = ui.available_width();
        let half = ((full - spacing) / 2.0).max(60.0);
        let (pause_icon, pause_text) = if self.control.is_paused() {
            (ICON_PLAY, "Resume")
        } else {
            (ICON_PAUSE, "Pause")
        };
        let start_clicked = action_button(
            ui,
            full,
            ICON_PLAY,
            "Start Typing",
            p.primary,
            p.on_primary,
            start_enabled,
        );
        let mut pause_clicked = false;
        let mut stop_clicked = false;
        ui.add_space(GAP_ROW);
        ui.horizontal(|ui| {
            pause_clicked = action_button(
                ui,
                half,
                pause_icon,
                pause_text,
                p.tertiary,
                p.on_tertiary,
                typing,
            );
            stop_clicked = action_button(
                ui,
                half,
                ICON_STOP,
                "Stop",
                p.error,
                p.on_error,
                cancellable,
            );
        });
        if start_clicked {
            self.start_countdown(&ctx);
        }
        if pause_clicked {
            self.toggle_pause();
        }
        if stop_clicked {
            self.stop_typing(&ctx);
        }
    }

    /// 实时预览：前一字符 / 当前字符 / 后续 3 字符 + 进度条。
    fn preview_section(&mut self, ui: &mut egui::Ui) {
        let p = self.palette;
        if !self.control.is_active() {
            return;
        }
        ui.add_space(GAP_SECTION);
        let chars: Vec<char> = self.text.chars().collect();
        let idx = self.control.index();

        let pretty = |c: char| match c {
            '\n' => '↵',
            '\t' => '→',
            ' ' => '␣',
            other => other,
        };
        let past: String = if idx > 0 && idx <= chars.len() {
            pretty(chars[idx - 1]).to_string()
        } else {
            String::new()
        };
        let current: String = if idx < chars.len() {
            pretty(chars[idx]).to_string()
        } else {
            String::new()
        };
        let next: String = chars
            .iter()
            .skip(idx + 1)
            .take(3)
            .copied()
            .map(pretty)
            .collect();

        let total = self.control.total().max(1);
        let done = (idx + 1).min(total);
        let frac = done as f32 / total as f32;

        egui::Frame::new()
            .fill(p.surface_variant)
            .inner_margin(egui::Margin::same(20))
            .corner_radius(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.label(
                    egui::RichText::new("Typing...")
                        .size(FS_SMALL)
                        .color(p.outline),
                );
                ui.add_space(12.0);
                // 居中的“前一个 / 当前 / 后续”字符（对齐 web 的 .preview-text）。
                ui.vertical_centered(|ui| {
                    ui.horizontal(|ui| {
                        ui.spacing_mut().item_spacing.x = 6.0;
                        ui.label(
                            egui::RichText::new(past)
                                .font(egui::FontId::monospace(16.0))
                                .color(Palette::PREV_GREY),
                        );
                        ui.label(
                            egui::RichText::new(current)
                                .font(egui::FontId::monospace(20.0))
                                .strong()
                                .color(Palette::CURRENT_GREEN),
                        );
                        ui.label(
                            egui::RichText::new(next)
                                .font(egui::FontId::monospace(16.0))
                                .color(Palette::NEXT_ORANGE),
                        );
                    });
                });
                ui.add_space(16.0);
                // 进度条：surface 底 + primary 填充（对齐 web 的 .progress-bar）。
                let (rect, _) = ui.allocate_exact_size(
                    egui::vec2(ui.available_width(), 4.0),
                    egui::Sense::hover(),
                );
                ui.painter()
                    .rect_filled(rect, egui::CornerRadius::same(2), p.surface);
                if frac > 0.0 {
                    let fill = egui::Rect::from_min_size(
                        rect.min,
                        egui::vec2(rect.width() * frac, rect.height()),
                    );
                    ui.painter()
                        .rect_filled(fill, egui::CornerRadius::same(2), p.primary);
                }
            });
    }

    fn instructions(&self, ui: &mut egui::Ui) {
        let p = self.palette;
        egui::Frame::new()
            .fill(p.primary_container)
            .inner_margin(egui::Margin::same(16))
            .corner_radius(12)
            .show(ui, |ui| {
                ui.set_width(ui.available_width());
                ui.horizontal_top(|ui| {
                    ui.spacing_mut().item_spacing.x = 12.0;
                    ui.label(
                        egui::RichText::new(ICON_INFO)
                            .size(FS_SECTION)
                            .color(p.on_primary_container),
                    );
                    // 自动换行，避免文字溢出卡片右边缘。
                    ui.add(
                        egui::Label::new(
                            egui::RichText::new(
                                "Type/Paste text → Set speed → Press Start → Position cursor (Auto-detects language)",
                            )
                            .size(FS_BODY)
                            .color(p.on_primary_container),
                        )
                        .wrap(),
                    );
                });
            });
    }

    fn footer(&self, ui: &mut egui::Ui) {
        let p = self.palette;
        ui.vertical_centered(|ui| {
            ui.horizontal_wrapped(|ui| {
                ui.spacing_mut().item_spacing.x = 4.0;
                ui.label(
                    egui::RichText::new(format!(
                        "TypingBot v{APP_VERSION} {APP_VERSION_NAME} | Developed by "
                    ))
                    .size(FS_SMALL)
                    .color(p.outline),
                );
                if link(ui, AUTHOR_NAME, p.primary).clicked() {
                    let _ = open::that(AUTHOR_URL);
                }
                ui.label(egui::RichText::new("|").size(FS_SMALL).color(p.outline));
                if link(ui, "GitHub", p.primary).clicked() {
                    let _ = open::that(GITHUB_URL);
                }
            });
        });
    }

    /// 倒计时阶段独占整个（已收缩的）窗口，绘制底部居中的倒计时卡片
    /// （对齐 web 的 `#countdownModal`）。
    fn render_countdown(&mut self, ctx: &egui::Context, rect: egui::Rect) {
        let p = self.palette;
        let Phase::Countdown { started } = self.phase else {
            return;
        };
        let elapsed = started.elapsed().as_secs_f32();
        let remaining = (COUNTDOWN_SECS - elapsed).ceil().max(1.0) as i32;

        let painter = ctx.layer_painter(egui::LayerId::new(
            egui::Order::Background,
            "countdown".into(),
        ));
        // 整个客户区铺成卡片色：窗口本身由 DWM 画圆角和阴影（见 set_borderless），
        // 所以这里不需要再画内层圆角，避免出现深浅不一的边圈。
        painter.rect_filled(rect, egui::CornerRadius::ZERO, p.surface);

        let hint = painter.layout(
            "Position your cursor".into(),
            egui::FontId::proportional(FS_SMALL),
            p.on_surface,
            f32::INFINITY,
        );
        painter.galley(
            rect.center() - hint.size() / 2.0 - egui::vec2(0.0, 26.0),
            hint,
            p.on_surface,
        );

        let number = painter.layout(
            remaining.to_string().into(),
            egui::FontId::proportional(FS_DISPLAY),
            p.primary,
            f32::INFINITY,
        );
        painter.galley(
            rect.center() - number.size() / 2.0 + egui::vec2(0.0, 18.0),
            number,
            p.primary,
        );

        ctx.request_repaint_after(Duration::from_millis(50));
    }

    /// 通知 Toast（3 秒后自动消失）：普通消息表面色底，错误消息 error 底。
    fn overlays(&mut self, ctx: &egui::Context, window_rect: egui::Rect) {
        let p = self.palette;
        let toast = self
            .notification
            .as_ref()
            .filter(|(_, _, at)| at.elapsed() < Duration::from_secs(3))
            .map(|(msg, is_error, _)| (msg.clone(), *is_error));
        match toast {
            Some((msg, is_error)) => {
                let (bg, fg) = if is_error {
                    (p.error, p.on_error)
                } else {
                    (p.surface, p.on_surface)
                };
                let painter =
                    ctx.layer_painter(egui::LayerId::new(egui::Order::Foreground, "toast".into()));
                let galley = painter.layout(
                    msg.as_str().into(),
                    egui::FontId::proportional(FS_BODY),
                    fg,
                    f32::INFINITY,
                );
                let rect = egui::Rect::from_center_size(
                    egui::pos2(window_rect.center().x, window_rect.bottom() - 100.0),
                    egui::vec2(galley.size().x + 48.0, 46.0),
                );
                painter.add(elevation().as_shape(rect, egui::CornerRadius::same(12)));
                painter.rect_filled(rect, egui::CornerRadius::same(12), bg);
                painter.galley(rect.center() - galley.size() / 2.0, galley, fg);
                ctx.request_repaint_after(Duration::from_millis(100));
            }
            None => self.notification = None,
        }
    }
}

/// Material elevation-2 阴影，用于头部 / 卡片 / 弹层。
fn elevation() -> egui::Shadow {
    egui::Shadow {
        offset: [0, 2],
        blur: 10,
        spread: 0,
        color: egui::Color32::from_black_alpha(70),
    }
}

/// 构造带打字进度的 [`egui::text::LayoutJob`]：已输入字符灰色、当前字符亮绿、
/// 待输入字符橙色，颜色与 Python 版 `update_text_highlighting` 的 tag 一致。
///
/// 未在打字时整段使用普通前景色，避免影响日常编辑。
fn highlight_job(
    ui: &egui::Ui,
    text: &str,
    active: bool,
    index: usize,
    p: Palette,
    wrap_width: f32,
) -> egui::text::LayoutJob {
    use egui::text::{LayoutJob, TextFormat};

    let _ = ui;
    let font = egui::FontId::proportional(FS_BODY);
    let mut job = LayoutJob::default();
    job.wrap.max_width = wrap_width;
    // 与默认 layouter 一致：显示尾随空格，换行符另起一行。
    job.keep_trailing_whitespace = true;
    job.break_on_newline = true;

    let append = |job: &mut LayoutJob, run: &str, color: egui::Color32| {
        job.append(
            run,
            0.0,
            TextFormat {
                font_id: font.clone(),
                color,
                line_height: Some(EDITOR_LINE_HEIGHT),
                ..Default::default()
            },
        );
    };

    if !active {
        append(&mut job, text, p.on_surface);
        return job;
    }

    // 引擎输入的是 `text.trim()`，把被裁掉的前导空白计入下标偏移。
    let leading_bytes = text.len() - text.trim_start().len();
    let current = text[..leading_bytes].chars().count() + index;

    let mut run = String::new();
    let mut run_color = Palette::PREV_GREY;
    for (pos, ch) in text.chars().enumerate() {
        let color = if pos < current {
            Palette::PREV_GREY
        } else if pos == current {
            Palette::CURRENT_GREEN
        } else {
            Palette::NEXT_ORANGE
        };
        if color != run_color && !run.is_empty() {
            append(&mut job, &run, run_color);
            run.clear();
        }
        run_color = color;
        run.push(ch);
    }
    append(&mut job, &run, run_color);

    job
}

/// 把同一行内的所有字形对齐到同一条基线上。
///
/// epaint 是按“每个字体自身的 ascent”来摆放字形的，混排时（例如中文字体 + 系统
/// emoji 字体）基线会不一致，emoji 会明显偏高或偏低。这里在布局完成后统一基线，
/// 并同步平移已经生成好的字形网格，保证绘制结果与逻辑位置一致。
fn align_baselines(galley: Arc<egui::Galley>) -> Arc<egui::Galley> {
    let mut galley = galley;
    let inner = Arc::make_mut(&mut galley);
    for placed in &mut inner.rows {
        let row = Arc::make_mut(&mut placed.row);
        if row.glyphs.is_empty() {
            continue;
        }
        let baseline = row
            .glyphs
            .iter()
            .map(|glyph| glyph.font_face_ascent.max(glyph.font_ascent))
            .fold(f32::NEG_INFINITY, f32::max);
        if !baseline.is_finite() {
            continue;
        }
        let count = row.glyphs.len();
        let vertex_end = row.visuals.glyph_vertex_range.end;
        let vertex_len = row.visuals.mesh.vertices.len();
        for i in 0..count {
            let dy = baseline - row.glyphs[i].pos.y;
            row.glyphs[i].pos.y = baseline;
            if dy == 0.0 {
                continue;
            }
            // 每个字形在网格里占据 [first_vertex, 下一个字形的 first_vertex) 一段顶点。
            let start = row.glyphs[i].first_vertex as usize;
            let stop = if i + 1 < count {
                row.glyphs[i + 1].first_vertex as usize
            } else {
                vertex_end
            }
            .min(vertex_len);
            if start < stop {
                for vertex in &mut row.visuals.mesh.vertices[start..stop] {
                    vertex.pos.y += dy;
                }
            }
        }
        row.visuals.mesh_bounds = row.visuals.mesh.calc_bounds();
    }
    galley
}

/// 自绘动作按钮：整组“图标 + 文字”按实测宽度居中，左右留白完全对称。
/// 返回是否被点击（禁用时不响应）。
fn action_button(
    ui: &mut egui::Ui,
    width: f32,
    icon: &str,
    label: &str,
    fill: egui::Color32,
    fg: egui::Color32,
    enabled: bool,
) -> bool {
    let (rect, resp) =
        ui.allocate_exact_size(egui::vec2(width, CONTROL_HEIGHT), egui::Sense::click());
    let hovered = enabled && resp.hovered();
    let resp = if enabled {
        resp.on_hover_cursor(egui::CursorIcon::PointingHand)
    } else {
        resp
    };

    let fg = if enabled { fg } else { fg.gamma_multiply(0.45) };
    let fill = if !enabled {
        fill.gamma_multiply(0.45)
    } else if hovered {
        fill.gamma_multiply(1.08)
    } else {
        fill
    };

    let painter = ui.painter();
    painter.rect_filled(rect, egui::CornerRadius::same(18), fill);

    let font = egui::FontId::proportional(FS_BODY);
    let icon_galley = painter.layout(icon.to_owned(), font.clone(), fg, f32::INFINITY);
    let text_galley = painter.layout(label.to_owned(), font, fg, f32::INFINITY);
    let gap = 6.0;
    let total = icon_galley.size().x + gap + text_galley.size().x;
    let x = rect.center().x - total / 2.0;
    painter.galley(
        egui::pos2(x, rect.center().y - icon_galley.size().y / 2.0),
        icon_galley.clone(),
        fg,
    );
    painter.galley(
        egui::pos2(
            x + icon_galley.size().x + gap,
            rect.center().y - text_galley.size().y / 2.0,
        ),
        text_galley,
        fg,
    );

    enabled && resp.clicked()
}

fn link(ui: &mut egui::Ui, text: &str, color: egui::Color32) -> egui::Response {
    ui.add(
        egui::Label::new(
            egui::RichText::new(text)
                .size(FS_SMALL)
                .strong()
                .color(color)
                .underline(),
        )
        .sense(egui::Sense::click()),
    )
}

/// 默认的字体微调（hinting 策略）。
///
/// egui 默认的 Smooth hinting 是按拉丁字母调的：横竖笔画都会被吸附到像素网格。
/// 中文在 15pt 上下字号时横画非常密集（例如“管”“能”的下半部分），横画被吸附后
/// 相邻笔画会糊在一起，看起来又粗又模糊；实测关闭 hinting 后笔画更干净、更接近
/// 浏览器 / DirectWrite 的现代渲染，所以默认关闭。
///
/// 可用环境变量 `TB_FONT_HINT` 覆盖以便对比：`off`（默认）/ `light` / `normal`。
fn font_tweak() -> egui::FontTweak {
    use egui::epaint::text::{HintingTarget, SmoothHinting};

    match std::env::var("TB_FONT_HINT").as_deref() {
        Ok("light") => egui::FontTweak {
            hinting_target: HintingTarget::Smooth(SmoothHinting {
                light: true,
                symmetric_rendering: true,
                preserve_linear_metrics: true,
            }),
            ..Default::default()
        },
        Ok("normal") => egui::FontTweak::default(),
        _ => egui::FontTweak {
            hinting: Some(false),
            ..Default::default()
        },
    }
}

/// 读取第一个存在的字体文件并注册。
fn load_font(fonts: &mut egui::FontDefinitions, name: &str, paths: &[&str]) -> bool {
    for path in paths {
        if let Ok(bytes) = std::fs::read(path) {
            let mut data = egui::FontData::from_owned(bytes);
            data.tweak = font_tweak();
            fonts.font_data.insert(name.to_owned(), Arc::new(data));
            return true;
        }
    }
    false
}

/// 注册字体与回退链。
///
/// egui 不会像系统控件那样自动回退，缺字形就画成豆腐块（□）。这里以
/// **微软雅黑** 为默认正文字体（拉丁与中文风格统一），并按脚本补齐回退：
/// - 图标：Segoe Fluent Icons（PUA 图标）；
/// - 拉丁/符号：Segoe UI；
/// - 孟加拉文：Nirmala UI；中文：SimSun、SimsunExtG（CJK 扩展区）；
/// - 日文：Meiryo / Yu Gothic；韩文：Malgun Gothic。
/// 系统字体目录：`%WINDIR%\Fonts`（取不到 `WINDIR` 时回落 `C:\Windows\Fonts`）。
/// 不写死 `C:\Windows`，Windows 装在别的盘符上也能正常工作；非 Windows 平台
/// 这些路径不存在，`load_font` 会安静跳过，界面回落到 egui 自带字体。
fn fonts_dir() -> String {
    let windir = std::env::var("WINDIR").unwrap_or_else(|_| r"C:\Windows".to_owned());
    format!("{windir}\\Fonts")
}

fn install_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let dir = fonts_dir();
    let f = |name: &str| format!("{dir}\\{name}");

    load_font(&mut fonts, "ui_font", &[&f("msyh.ttc"), &f("msyh.ttf")]);
    load_font(&mut fonts, "icons", &[&f("SegoeIcons.ttf")]);
    load_font(&mut fonts, "segoe", &[&f("segoeui.ttf")]);
    // 系统 emoji 字体（Windows 的 Segoe UI Emoji）。放在 Segoe UI 之后：拉丁与
    // 常见符号优先进 Segoe UI，emoji 才落到这里。
    load_font(&mut fonts, "emoji", &[&f("seguiemj.ttf")]);
    load_font(
        &mut fonts,
        "nirmala",
        &[&f("Nirmala.ttc"), &f("Nirmala.ttf")],
    );
    load_font(&mut fonts, "simsun", &[&f("simsun.ttc")]);
    load_font(&mut fonts, "simsun_extg", &[&f("SimsunExtG.ttf")]);
    load_font(
        &mut fonts,
        "meiryo",
        &[&f("meiryo.ttc"), &f("YuGothM.ttc"), &f("msgothic.ttc")],
    );
    load_font(&mut fonts, "malgun", &[&f("malgun.ttf")]);

    // 正文字体的回退顺序。
    const ORDER: [&str; 9] = [
        "ui_font",
        "icons",
        "segoe",
        "emoji",
        "nirmala",
        "simsun",
        "simsun_extg",
        "meiryo",
        "malgun",
    ];

    let build = |defaults: Vec<String>| -> Vec<String> {
        let mut list: Vec<String> = ORDER
            .iter()
            .filter(|n| fonts.font_data.contains_key(**n))
            .map(|s| s.to_string())
            .collect();
        for d in defaults {
            if !list.contains(&d) {
                list.push(d);
            }
        }
        list
    };

    let prop_defaults = fonts
        .families
        .get(&egui::FontFamily::Proportional)
        .cloned()
        .unwrap_or_default();
    let mono_defaults = fonts
        .families
        .get(&egui::FontFamily::Monospace)
        .cloned()
        .unwrap_or_default();

    fonts
        .families
        .insert(egui::FontFamily::Proportional, build(prop_defaults));

    // 等宽字体保留内置等宽字体在前，仅在其后追加图标与各脚本兜底。
    let mut mono: Vec<String> = vec!["icons".to_owned()];
    for n in [
        "emoji",
        "nirmala",
        "simsun",
        "simsun_extg",
        "meiryo",
        "malgun",
    ] {
        if fonts.font_data.contains_key(n) {
            mono.push(n.to_owned());
        }
    }
    for d in mono_defaults {
        if !mono.contains(&d) {
            mono.push(d);
        }
    }
    fonts.families.insert(egui::FontFamily::Monospace, mono);

    ctx.set_fonts(fonts);
}

impl eframe::App for TypingBotApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = ui.ctx().clone();
        // 根 Ui 没有边距，max_rect 即整个窗口。
        let window_rect = ui.max_rect();

        // 窗口不需要最大化：去掉标题栏上的最大化按钮（仍可自由拉伸）。
        // winit 会在启动阶段把样式位加回来，所以每帧都检查一次（只在需要时才改）。
        hide_maximize_button(self.window_hwnd);
        // 倒计时浮窗不要标题栏；winit 会不定时重写窗口样式，所以每帧校正一次
        // （只在需要时才真正改，不会每帧重画边框）。去掉边框会把外框尺寸并进
        // 客户区，所以改完再按浮窗尺寸压一次。
        let in_countdown = matches!(self.phase, Phase::Countdown { .. });
        if set_borderless(self.window_hwnd, in_countdown) && in_countdown {
            self.set_inner_size(&ctx, COUNTDOWN_WINDOW);
        }

        self.place_window_once(&ctx);
        // 上次退出时如果正处于打字 / 倒计时阶段，eframe 会把当时的过渡高度一起
        // 存下来，于是这次启动窗口会偏高（甚至缩成一条）。头一帧纠正回常规尺寸。
        if !self.size_repaired {
            if let Some(size) = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size())) {
                self.size_repaired = true;
                if is_transitional_size(size) {
                    crate::log_line(format_args!(
                        "[typingbot] 修正上次遗留的过渡窗口高度 {:.0} → {:.0}",
                        size.y, IDLE_HEIGHT
                    ));
                    self.base_size = egui::vec2(WINDOW_WIDTH, IDLE_HEIGHT);
                    self.set_inner_size(&ctx, self.base_size);
                }
            } else {
                // 视口尺寸还没就绪，下一帧再试。
                ctx.request_repaint();
            }
        }
        self.sync_theme(&ctx);
        self.sync_pinned(&ctx);

        // 记录“常规”窗口尺寸：只在空闲状态、且尺寸不是过渡值时更新。
        // 倒计时浮窗（260×120）和打字临时高度（826）都不能当作基准。
        if matches!(self.phase, Phase::Idle)
            && let Some(size) = ctx.input(|i| i.viewport().inner_rect.map(|r| r.size()))
        {
            if !is_transitional_size(size) && size.x > 200.0 && size.y > 200.0 {
                self.base_size = size;
            }
        }

        // 工作线程结束检测（含自然结束 / Stop / ESC）。
        //
        // 不能只看 `was_active` 的“由真变假”边沿：很短的文本（例如 6 个字符、
        // 100 WPS ≈ 10ms）会在同一帧里就跑完，边沿根本观察不到，界面会永久
        // 停在打字态（按钮全灰、Stop 点了也没反应）。因此只要“阶段是打字”
        // 而工作线程已经不活跃，就认为它结束了。
        let active = self.control.is_active();
        if !active && (self.was_active || matches!(self.phase, Phase::Typing)) {
            self.on_typing_end(&ctx);
        }
        self.was_active = active;
        if active {
            ctx.request_repaint_after(Duration::from_millis(16));
        }

        // 已经请求过停止、但输入线程迟迟不结束（卡在系统调用里）：
        // 宽限期一到就把界面拉回空闲，别让整个窗口被锁死。
        if self.stop_deadline.is_none()
            && active
            && self.control.should_stop()
            && matches!(self.phase, Phase::Typing)
        {
            self.stop_deadline = Some(Instant::now() + STOP_GRACE);
        }
        if let Some(deadline) = self.stop_deadline {
            if !active {
                self.stop_deadline = None;
            } else if Instant::now() >= deadline {
                self.force_idle(&ctx);
                ctx.request_repaint();
            }
        }

        // 倒计时期间按 Stop / ESC：global hotkey 会把 stop 置位。
        if matches!(self.phase, Phase::Countdown { .. }) && self.control.should_stop() {
            self.cancel_countdown(&ctx);
        }

        // 兜底通道：应用窗口自己有焦点时，egui 也能直接收到 ESC
        // （有些环境里全局键盘钩子收不到输入，两条路都能停）。
        if !matches!(self.phase, Phase::Idle) && ctx.input(|i| i.key_pressed(egui::Key::Escape)) {
            self.stop_typing(&ctx);
        }
        self.advance_countdown(&ctx);

        // 开发/回归测试用开关：启动即开始输入（跳过倒计时），方便自动化验证
        // “输入结束 → 界面回到空闲”这条路径。
        if self.auto_start && matches!(self.phase, Phase::Idle) && !self.text.trim().is_empty() {
            self.auto_start = false;
            let text = self.text.trim().to_string();
            engine::start(text, self.speed, Arc::clone(&self.control));
            self.grow_for_typing(&ctx);
            self.phase = Phase::Typing;
        }

        // 倒计时阶段：只画小浮窗，主界面暂不渲染（窗口此时已收缩）。
        if matches!(self.phase, Phase::Countdown { .. }) {
            self.render_countdown(&ctx, window_rect);
            self.persist();
            return;
        }

        let p = self.palette;
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(p.background)
                    .inner_margin(egui::Margin::same(10)),
            )
            .show(ui, |ui| {
                // 不滚动：窗口最小尺寸已保证内容放得下。
                // 头部与卡片紧贴成一张卡。
                ui.spacing_mut().item_spacing = egui::vec2(0.0, 0.0);

                self.header(ui);

                // 主卡片：只有下面两角是圆的，带 elevation-2 阴影。
                //
                // 卡片高度 = 头部以下、页脚以上的全部剩余高度（固定不变），内部
                // 只有输入框会伸缩：输入框高度 = 卡片内容高度 - 输入框上方/下方的
                // 其它控件高度。下方控件高度用上一帧实测，因此它们到窗口底边的
                // 距离始终不变。
                let footer_h = ui
                    .painter()
                    .layout_no_wrap(
                        "TypingBot".to_owned(),
                        egui::FontId::proportional(FS_SMALL),
                        p.outline,
                    )
                    .size()
                    .y;
                let card_outer_h = ui.available_height() - FOOTER_GAP - footer_h;
                let card_content_h = (card_outer_h - 16.0 * 2.0).max(0.0);
                let label_h = ui
                    .painter()
                    .layout_no_wrap(
                        "Text to Type".to_owned(),
                        egui::FontId::proportional(FS_SECTION),
                        p.on_surface,
                    )
                    .size()
                    .y;
                let editor_h =
                    (card_content_h - label_h - GAP_LABEL - EDITOR_CHROME - GAP_ROW - self.below_h)
                        .max(MIN_EDITOR_HEIGHT);
                let card_radius = egui::CornerRadius {
                    nw: 0,
                    ne: 0,
                    sw: 18,
                    se: 18,
                };
                egui::Frame::new()
                    .fill(p.surface)
                    .shadow(elevation())
                    .inner_margin(egui::Margin::same(16))
                    .corner_radius(card_radius)
                    .show(ui, |ui| {
                        ui.set_min_height(card_content_h);
                        ui.spacing_mut().item_spacing = egui::vec2(12.0, 0.0);
                        self.editor_section(ui, editor_h);
                        ui.add_space(GAP_ROW);
                        // 下面这些控件高度固定；量一遍存起来给下一帧算输入框高度。
                        let mark = ui.cursor().top();
                        self.paste_row(ui);
                        ui.add_space(GAP_SECTION);
                        self.speed_section(ui);
                        ui.add_space(GAP_SECTION);
                        self.control_section(ui);
                        self.preview_section(ui);
                        ui.add_space(GAP_SECTION);
                        self.instructions(ui);
                        let below = ui.cursor().top() - mark;
                        if (below - self.below_h).abs() > 0.5 {
                            self.below_h = below;
                            ui.ctx().request_repaint();
                        }
                        // 最小窗口高度 = 内容刚好放得下（输入框取最小值）所需的高度，
                        // 这样用户怎么缩都不会把下面的控件挤出去。
                        let min_h = 10.0 * 2.0
                            + HEADER_HEIGHT
                            + FOOTER_GAP
                            + footer_h
                            + label_h
                            + GAP_LABEL
                            + EDITOR_CHROME
                            + MIN_EDITOR_HEIGHT
                            + GAP_ROW
                            + below
                            + 16.0 * 2.0;
                        if (min_h - self.min_window_h).abs() > 1.0 {
                            self.min_window_h = min_h;
                            ui.ctx()
                                .send_viewport_cmd(egui::ViewportCommand::MinInnerSize(
                                    egui::vec2(WINDOW_WIDTH, min_h),
                                ));
                        }
                    });

                ui.add_space(FOOTER_GAP);
                self.footer(ui);
            });

        self.overlays(&ctx, window_rect);
        self.persist();
    }

    fn save(&mut self, storage: &mut dyn eframe::Storage) {
        // 窗口几何由 eframe 的 persistence 特性负责，这里只存业务设置。
        self.settings.speed = self.speed;
        self.settings.dark_mode = self.dark;
        self.settings.theme = Some(self.theme_mode);
        self.settings.last_text = self.text.clone();
        self.settings.pinned = self.pinned;
        if let Ok(raw) = serde_json::to_string(&self.settings) {
            storage.set_string("settings", raw);
        }
    }
}

// ------------------------------------------------------------------ 窗口样式

/// 读取 Windows 的系统主题设置（`False` = 系统使用深色）。
///
/// 数据源：`HKCU\Software\Microsoft\Windows\CurrentVersion\Themes\Personalize`
/// 下的 `AppsUseLightTheme`（0 = 深色，1 = 浅色）。读不到时返回 `None`。
#[cfg(windows)]
fn os_prefers_dark() -> Option<bool> {
    use std::ffi::c_void;

    #[link(name = "advapi32")]
    unsafe extern "system" {
        fn RegGetValueW(
            hkey: *mut c_void,
            sub_key: *const u16,
            value: *const u16,
            flags: u32,
            value_type: *mut u32,
            data: *mut c_void,
            data_size: *mut u32,
        ) -> i32;
    }

    /// `HKEY_CURRENT_USER` 的常量值。
    const HKEY_CURRENT_USER: usize = 0x8000_0001;
    /// 只接受 `REG_DWORD`。
    const RRF_RT_REG_DWORD: u32 = 0x0000_0010;

    fn wide(text: &str) -> Vec<u16> {
        text.encode_utf16().chain(std::iter::once(0)).collect()
    }

    let sub_key = wide(r"SOFTWARE\Microsoft\Windows\CurrentVersion\Themes\Personalize");
    let value = wide("AppsUseLightTheme");
    let mut apps_use_light: u32 = 1;
    let mut size = std::mem::size_of::<u32>() as u32;
    let status = unsafe {
        RegGetValueW(
            HKEY_CURRENT_USER as *mut c_void,
            sub_key.as_ptr(),
            value.as_ptr(),
            RRF_RT_REG_DWORD,
            std::ptr::null_mut(),
            (&raw mut apps_use_light).cast(),
            &mut size,
        )
    };
    // ERROR_SUCCESS == 0
    (status == 0).then_some(apps_use_light == 0)
}

#[cfg(not(windows))]
fn os_prefers_dark() -> Option<bool> {
    None
}

/// 取出主窗口的 HWND（用于 Win32 层的样式调整）。
#[cfg(all(windows, target_pointer_width = "64"))]
fn window_handle(cc: &eframe::CreationContext<'_>) -> Option<usize> {
    use raw_window_handle::{HasWindowHandle, RawWindowHandle};

    let handle = cc.window_handle().ok()?;
    match handle.as_raw() {
        RawWindowHandle::Win32(win32) => Some(win32.hwnd.get() as usize),
        _ => None,
    }
}

#[cfg(not(all(windows, target_pointer_width = "64")))]
fn window_handle(_cc: &eframe::CreationContext<'_>) -> Option<usize> {
    None
}

/// 去掉标题栏上的“最大化”按钮（本应用的高度按内容算，最大化没有意义）。
#[cfg(all(windows, target_pointer_width = "64"))]
fn hide_maximize_button(hwnd: Option<usize>) {
    if let Some(hwnd) = hwnd {
        win::strip_maximize_box(hwnd);
    }
}

#[cfg(not(all(windows, target_pointer_width = "64")))]
fn hide_maximize_button(_hwnd: Option<usize>) {}

/// 倒计时浮窗：去掉/恢复标题栏；返回是否真的改了样式。
#[cfg(all(windows, target_pointer_width = "64"))]
fn set_borderless(hwnd: Option<usize>, borderless: bool) -> bool {
    hwnd.is_some_and(|hwnd| win::set_borderless(hwnd, borderless))
}

#[cfg(not(all(windows, target_pointer_width = "64")))]
fn set_borderless(_hwnd: Option<usize>, _borderless: bool) -> bool {
    false
}

#[cfg(all(windows, target_pointer_width = "64"))]
mod win {
    use std::ffi::c_void;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn GetWindowLongPtrW(hwnd: *mut c_void, index: i32) -> isize;
        fn SetWindowLongPtrW(hwnd: *mut c_void, index: i32, value: isize) -> isize;
        fn SetWindowPos(
            hwnd: *mut c_void,
            insert_after: *mut c_void,
            x: i32,
            y: i32,
            cx: i32,
            cy: i32,
            flags: u32,
        ) -> i32;
    }

    const GWL_STYLE: i32 = -16;
    const WS_CAPTION: isize = 0x00C0_0000;
    const WS_SYSMENU: isize = 0x0008_0000;
    const WS_MINIMIZEBOX: isize = 0x0002_0000;
    const WS_MAXIMIZEBOX: isize = 0x0001_0000;
    const SWP_NOSIZE: u32 = 0x0001;
    const SWP_NOMOVE: u32 = 0x0002;
    const SWP_NOZORDER: u32 = 0x0004;
    const SWP_FRAMECHANGED: u32 = 0x0020;

    /// 普通窗口该有的（非最大化）标题栏样式位。
    /// 倒计时浮窗只去掉 `WS_CAPTION` 一组：保留 `WS_THICKFRAME` 才能让 DWM
    /// 继续给它画阴影和圆角（Win11）。
    const FRAME_BITS: isize = WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;

    /// `DwmSetWindowAttribute` 的圆角偏好。
    const DWMWA_WINDOW_CORNER_PREFERENCE: u32 = 33;
    const DWMWCP_ROUND: i32 = 2;

    #[link(name = "dwmapi")]
    unsafe extern "system" {
        fn DwmSetWindowAttribute(
            hwnd: *mut c_void,
            attribute: u32,
            value: *const c_void,
            size: u32,
        ) -> i32;
    }

    /// 请求窗口圆角（Win11；老系统调用失败也无所谓）。
    fn set_rounded(hwnd: *mut c_void, round: bool) {
        let preference: i32 = if round { DWMWCP_ROUND } else { 0 };
        unsafe {
            DwmSetWindowAttribute(
                hwnd,
                DWMWA_WINDOW_CORNER_PREFERENCE,
                (&raw const preference).cast(),
                std::mem::size_of::<i32>() as u32,
            );
        }
    }

    fn apply_style(hwnd: *mut c_void, style: isize) {
        unsafe {
            SetWindowLongPtrW(hwnd, GWL_STYLE, style);
            SetWindowPos(
                hwnd,
                std::ptr::null_mut(),
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOZORDER | SWP_FRAMECHANGED,
            );
        }
    }

    /// 清掉 `WS_MAXIMIZEBOX` 并让系统重画一次边框。
    ///
    /// 会被每帧调用：winit/eframe 在启动阶段（以及处理某些视口命令时）会把
    /// `WS_MAXIMIZEBOX` 加回来，这里只做一次读取，只有在被加回来时才重新清掉，
    /// 不会每帧都触发重画。
    pub fn strip_maximize_box(hwnd: usize) {
        let hwnd = hwnd as *mut c_void;
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            if style == 0 || style & WS_MAXIMIZEBOX == 0 {
                return;
            }
            apply_style(hwnd, style & !WS_MAXIMIZEBOX);
        }
    }

    /// 倒计时浮窗用：`borderless = true` 去掉标题栏，`false` 恢复。
    /// 返回是否真的改动了样式（调用方据此重新压一下客户区尺寸）。
    pub fn set_borderless(hwnd: usize, borderless: bool) -> bool {
        let hwnd = hwnd as *mut c_void;
        unsafe {
            let style = GetWindowLongPtrW(hwnd, GWL_STYLE);
            if style == 0 {
                return false;
            }
            let want = if borderless {
                style & !FRAME_BITS
            } else {
                style | FRAME_BITS
            };
            set_rounded(hwnd, borderless);
            if want == style {
                return false;
            }
            apply_style(hwnd, want);
            true
        }
    }
}
