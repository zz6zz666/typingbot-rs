//! 打字引擎：在独立线程里逐字符注入键盘事件。
//!
//! Windows 下 `enigo` 走 `SendInput` + `KEYEVENTF_UNICODE`，可直接发送任意
//! Unicode 码点（孟加拉语、emoji 等），不依赖当前键盘布局、不需要剪贴板，
//! 与 Python 版 `pynput.keyboard.Controller.type()` 的行为一致。

use crate::control::{Control, Reason};
use enigo::{Direction, Enigo, Key, Keyboard, Settings as EnigoSettings};
use std::sync::Arc;
use std::sync::atomic::Ordering;
use std::time::Duration;

/// WPS → 每字符延迟，公式与 Python 版一致：`delay = 1 / (wps * 6)`。
/// 1 word ≈ 6 字符（5 字母 + 1 空格）；WPS >= 500 时视为瞬时。
pub fn delay_for_wps(wps: f32) -> Duration {
    let wps = wps.max(1.0) as f64;
    if wps >= 500.0 {
        Duration::from_nanos(1)
    } else {
        Duration::from_secs_f64(1.0 / (wps * 6.0))
    }
}

/// 启动一次输入任务，立即返回；UI 通过 [`Control`] 观察进度。
pub fn start(text: String, speed: f32, control: Arc<Control>) {
    control.begin();

    std::thread::spawn(move || {
        if let Err(err) = run(&text, speed, &control) {
            crate::log_line(format_args!("[typingbot] 输入引擎异常退出: {err}"));
        }
    });
}

fn run(text: &str, speed: f32, control: &Control) -> Result<(), String> {
    let mut enigo =
        Enigo::new(&EnigoSettings::default()).map_err(|e| format!("无法初始化输入引擎: {e}"))?;

    let delay = delay_for_wps(speed);
    let chars: Vec<char> = text.chars().collect();
    control.total.store(chars.len(), Ordering::SeqCst);

    for (i, ch) in chars.iter().enumerate() {
        if control.should_stop() {
            control.finish(Reason::Cancelled);
            return Ok(());
        }

        // 暂停等待：每 30ms 轮询一次，同时持续检查停止请求。
        while control.is_paused() && !control.should_stop() {
            std::thread::sleep(Duration::from_millis(30));
        }
        if control.should_stop() {
            control.finish(Reason::Cancelled);
            return Ok(());
        }

        // 先更新下标，让 UI 高亮“当前字符”。
        control.index.store(i, Ordering::SeqCst);

        let ch = *ch;
        // 与 Python 版相同的特判分支。
        let res = match ch {
            '\n' => enigo.key(Key::Return, Direction::Click),
            '\t' => enigo.key(Key::Tab, Direction::Click),
            ' ' => enigo.key(Key::Space, Direction::Click),
            _ => enigo.text(&ch.to_string()),
        };
        if let Err(err) = res {
            crate::log_line(format_args!("[typingbot] 无法输入字符 {ch:?}: {err}"));
        }

        if !delay.is_zero() {
            std::thread::sleep(delay);
        }
    }

    control.finish(Reason::Completed);
    Ok(())
}
