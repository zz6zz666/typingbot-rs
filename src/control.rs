//! UI 线程与打字工作线程之间的共享控制状态。
//!
//! 全部使用原子变量，避免锁竞争；打字循环每轮都读取这些标志，
//! 语义与 Python 版 `typing_state = {is_typing, is_paused}` 一致。

use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};

/// 工作线程每轮都会核对自己拿到的代数是否仍然有效。
///
/// 用途：万一某个工作线程卡在系统调用里（例如 `SendInput` 被别的程序挂住），
/// 界面可以先用 [`Control::reset`] 把代数推进一格回到空闲，卡住的线程一旦
/// 从系统调用里出来就会发现自己已经过时，立刻退出，不会把余下的字符继续打出去。
fn next_generation(current: usize) -> usize {
    current.wrapping_add(1).max(1)
}

/// 工作线程退出原因。
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Reason {
    /// 正常打完。
    Completed = 1,
    /// 被用户/ESC 取消。
    Cancelled = 2,
}

#[derive(Default)]
pub struct Control {
    pub(crate) active: AtomicBool,
    pub(crate) paused: AtomicBool,
    pub(crate) stop: AtomicBool,
    /// 0 = 仍在运行，否则取 [`Reason`] 的值。
    pub(crate) reason: AtomicUsize,
    pub(crate) index: AtomicUsize,
    pub(crate) total: AtomicUsize,
    /// 当前这一次运行的编号，见 [`next_generation`]。
    pub(crate) generation: AtomicUsize,
}

impl Control {
    pub fn new() -> Self {
        Self::default()
    }

    /// 是否正在打字（倒计时阶段不算）。
    pub fn is_active(&self) -> bool {
        self.active.load(Ordering::SeqCst)
    }

    pub fn is_paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    pub fn should_stop(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }

    /// 当前已输入的字符下标（0 基）。
    pub fn index(&self) -> usize {
        self.index.load(Ordering::SeqCst)
    }

    pub fn total(&self) -> usize {
        self.total.load(Ordering::SeqCst)
    }

    /// 读取退出原因；仅在工作线程结束后调用。
    pub fn take_reason(&self) -> Option<Reason> {
        match self.reason.load(Ordering::SeqCst) {
            1 => Some(Reason::Completed),
            2 => Some(Reason::Cancelled),
            _ => None,
        }
    }

    /// 请求停止；同时解除暂停，保证工作线程能立刻看到。
    pub fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
    }

    pub fn toggle_pause(&self) {
        let next = !self.paused.load(Ordering::SeqCst);
        self.paused.store(next, Ordering::SeqCst);
    }

    /// 归零所有标志，准备下一次运行。
    ///
    /// 同时把代数推进一格：任何还在跑（或卡住）的旧线程都会因此作废。
    pub fn reset(&self) {
        self.active.store(false, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        self.stop.store(false, Ordering::SeqCst);
        self.reason.store(0, Ordering::SeqCst);
        self.index.store(0, Ordering::SeqCst);
        self.total.store(0, Ordering::SeqCst);
        self.generation
            .store(next_generation(self.generation()), Ordering::SeqCst);
    }

    /// 当前本次运行的编号。
    pub fn generation(&self) -> usize {
        self.generation.load(Ordering::SeqCst)
    }

    /// 工作线程在启动前调用，进入运行态；返回值是本次运行的编号。
    pub(crate) fn begin(&self) -> usize {
        self.stop.store(false, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        self.reason.store(0, Ordering::SeqCst);
        self.index.store(0, Ordering::SeqCst);
        self.total.store(0, Ordering::SeqCst);
        let generation = next_generation(self.generation());
        self.generation.store(generation, Ordering::SeqCst);
        self.active.store(true, Ordering::SeqCst);
        generation
    }

    /// 工作线程在结束时调用。
    pub(crate) fn finish(&self, reason: Reason) {
        self.reason.store(reason as usize, Ordering::SeqCst);
        self.paused.store(false, Ordering::SeqCst);
        self.active.store(false, Ordering::SeqCst);
    }
}
