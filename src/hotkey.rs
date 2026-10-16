//! 全局 ESC 停止：打字/倒计时期间焦点在目标程序，应用窗口本身收不到按键，
//! 所以装一个 `WH_KEYBOARD_LL` 被动钩子（只监听、不吞键）。
//!
//! 为什么不用 `rdev`：rdev 0.5.3 的 Windows 实现里 `listen()` 装完钩子后只调用了
//! **一次** `GetMessageA`（没有消息循环），函数随即返回、钩子线程结束，钩子也就失效了——
//! 表现就是一个键都收不到。这里自己实现，关键就是 `GetMessageW` 循环。
//!
//! 钩子无条件置位 `stop`：空闲时的误按会由下一次 `start_countdown` 的
//! `Control::reset()` 清掉，不会影响后续运行。

use crate::control::Control;
use std::sync::{Arc, OnceLock};

/// 钩子回调里要用的共享状态（钩子回调是 C 回调，只能走静态）。
static CONTROL: OnceLock<Arc<Control>> = OnceLock::new();

/// 在后台线程注册全局键盘监听。监听失败（权限/沙箱）时静默降级，
/// 用户仍可用界面上的 Stop 按钮。
pub fn spawn(control: Arc<Control>) {
    if CONTROL.set(control).is_err() {
        return; // 只装一次
    }
    std::thread::spawn(|| {
        if let Err(err) = run_hook() {
            crate::log_line(format_args!("[typingbot] 全局 ESC 监听不可用: {err}"));
        }
    });
}

#[cfg(windows)]
fn run_hook() -> Result<(), String> {
    windows_impl::run()
}

#[cfg(not(windows))]
fn run_hook() -> Result<(), String> {
    Ok(())
}

#[cfg(windows)]
mod windows_impl {
    use super::CONTROL;
    use std::ffi::c_void;

    const WH_KEYBOARD_LL: i32 = 13;
    const WM_KEYDOWN: usize = 0x0100;
    const WM_SYSKEYDOWN: usize = 0x0104;
    const VK_ESCAPE: u32 = 0x1B;

    #[repr(C)]
    struct KbdLlHookStruct {
        vk_code: u32,
        scan_code: u32,
        flags: u32,
        time: u32,
        extra_info: usize,
    }

    #[repr(C)]
    struct Point {
        x: i32,
        y: i32,
    }

    #[repr(C)]
    struct Msg {
        hwnd: *mut c_void,
        message: u32,
        w_param: usize,
        l_param: isize,
        time: u32,
        pt: Point,
        l_private: u32,
    }

    type HookProc = unsafe extern "system" fn(i32, usize, isize) -> isize;

    #[link(name = "user32")]
    unsafe extern "system" {
        fn SetWindowsHookExW(
            id_hook: i32,
            proc_: Option<HookProc>,
            module: *mut c_void,
            thread_id: u32,
        ) -> *mut c_void;
        fn UnhookWindowsHookEx(hook: *mut c_void) -> i32;
        fn CallNextHookEx(hook: *mut c_void, code: i32, w_param: usize, l_param: isize) -> isize;
        fn GetMessageW(msg: *mut Msg, hwnd: *mut c_void, min: u32, max: u32) -> i32;
        fn PeekMessageW(msg: *mut Msg, hwnd: *mut c_void, min: u32, max: u32, remove: u32) -> i32;
    }

    #[link(name = "kernel32")]
    unsafe extern "system" {
        fn GetModuleHandleW(name: *const u16) -> *mut c_void;
    }

    unsafe extern "system" fn keyboard_proc(code: i32, w_param: usize, l_param: isize) -> isize {
        if code >= 0
            && (w_param == WM_KEYDOWN || w_param == WM_SYSKEYDOWN)
            && !l_param_is_null(l_param)
        {
            // SAFETY: 系统保证 code >= 0 时 l_param 指向 KBDLLHOOKSTRUCT。
            let kb = unsafe { &*(l_param as *const KbdLlHookStruct) };
            if kb.vk_code == VK_ESCAPE {
                if let Some(control) = CONTROL.get() {
                    control.request_stop();
                    crate::log_line(format_args!("[typingbot] 收到全局 ESC，已请求停止"));
                }
            }
        }
        // 继续传递：不吞键，其他程序照常收到 ESC。
        unsafe { CallNextHookEx(std::ptr::null_mut(), code, w_param, l_param) }
    }

    fn l_param_is_null(l_param: isize) -> bool {
        l_param == 0
    }

    pub fn run() -> Result<(), String> {
        unsafe {
            // 先碰一次消息：确保本线程的消息队列已经建好，
            // LL 钩子的回调是挂在线程消息队列上投递的。
            let mut msg = std::mem::zeroed::<Msg>();
            PeekMessageW(&mut msg, std::ptr::null_mut(), 0, 0, 0);

            let hook = SetWindowsHookExW(
                WH_KEYBOARD_LL,
                Some(keyboard_proc),
                GetModuleHandleW(std::ptr::null()),
                0,
            );
            if hook.is_null() {
                return Err(format!(
                    "SetWindowsHookExW 失败: {}",
                    std::io::Error::last_os_error()
                ));
            }
            crate::log_line(format_args!("[typingbot] 全局 ESC 钩子已安装"));

            // 关键：必须一直取消息，系统才会把钩子回调投递到这个线程。
            // rdev 0.5.3 就是漏了这个循环，导致钩子装完立刻失效。
            while GetMessageW(&mut msg, std::ptr::null_mut(), 0, 0) > 0 {}
            UnhookWindowsHookEx(hook);
        }
        Ok(())
    }
}
