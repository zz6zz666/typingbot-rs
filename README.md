# TypingBot - Bypass Paste, Bypass Text Pasting — Rust Edition

<div align="center">

![TypingBot](https://img.shields.io/badge/TypingBot-v3.0.0-6750A4?style=for-the-badge)
[![Release](https://img.shields.io/github/v/release/zz6zz666/typingbot-rs?style=for-the-badge&color=6750A4&label=Release)](https://github.com/zz6zz666/typingbot-rs/releases/latest)
[![Downloads](https://img.shields.io/github/downloads/zz6zz666/typingbot-rs/total?style=for-the-badge&color=6750A4&label=Downloads)](https://github.com/zz6zz666/typingbot-rs/releases)
![Rust](https://img.shields.io/badge/Rust-2024-000000?style=for-the-badge&logo=rust)
![egui](https://img.shields.io/badge/UI-egui%20%2F%20eframe-6750A4?style=for-the-badge)
![Platform](https://img.shields.io/badge/Platform-Windows-0078D4?style=for-the-badge&logo=windows)
![License](https://img.shields.io/badge/License-MIT-green?style=for-the-badge)

**Automate text typing with customizable speed - Perfect for bypassing paste restrictions**

A native **Rust rewrite** of the original Python TypingBot: one self-contained
`.exe`, no Python runtime, no embedded browser — same Material 3 interface,
same workflow.

[Download](#️-download) • [Features](#-features) • [Build](#-building-from-source) • [Usage](#-usage) • [Tech Stack](#-tech-stack) • [Credits](#-credits--acknowledgements)

</div>

---

## 🧭 About

TypingBot types a block of text for you, **character by character**, at a configurable
speed (1–500 WPS). That makes it work in places where pasting is blocked
(`Ctrl+V` disabled, `onpaste` handlers, paste-restricted form fields) and produces
natural, human-like input for forms, tests and automation demos.

The original TypingBot was written in Python (Flask + pywebview + pyautogui/pynput).
This repository is an **independent native port in Rust** using `eframe`/`egui` for
the UI: it keeps the Material 3 look, the speed model, the countdown workflow and the
settings format, while replacing the whole runtime (no Python, no local web server,
no browser engine).

Original project: [AprajitSarkar/typing-bot-bypass-paste](https://github.com/AprajitSarkar/typing-bot-bypass-paste)

## ⬇️ Download

**[Latest release: v3.0.0](https://github.com/zz6zz666/typingbot-rs/releases/latest)** — no installer,
no setup, nothing to configure:

| File | What it is |
|------|------------|
| [`TypingBot_v3.0.0.exe`](https://github.com/zz6zz666/typingbot-rs/releases/download/v3.0.0/TypingBot_v3.0.0.exe) | The app: a single ~5 MB portable executable (Windows 10/11, x64) |
| [`TypingBot_v3.0.0.exe.sha256`](https://github.com/zz6zz666/typingbot-rs/releases/download/v3.0.0/TypingBot_v3.0.0.exe.sha256) | Checksum for the download (`sha256sum -c` compatible) |

Download it, paste your text, press **Start Typing** — that's the whole setup. The exe is
unsigned, so Windows may show a SmartScreen prompt on first launch: see
[ANTIVIRUS.md](ANTIVIRUS.md). Prefer to compile it yourself?
See [Building from Source](#-building-from-source).

All versions, release notes and checksums: **[Releases →](https://github.com/zz6zz666/typingbot-rs/releases)**

## 🌟 Features

- **⚡ Variable Typing Speed**: 1 → 500 words per second (WPS), live readout
- **🔄 Auto-Type**: character-by-character synthetic keystrokes to bypass paste restrictions
- **⏱️ Countdown Timer**: 3-second countdown — the main window slides away and comes back
  as a small card at the bottom of the screen (so it does not cover your target), then
  typing starts
- **⏸ Start / Pause / Stop**: resume mid-way, stop instantly, or press **Esc** anywhere
  (global hotkey) to cancel
- **👁️ Live Preview**: while typing, the input box highlights the text and shows the
  current position
- **🌏 Unicode / Bangla / CJK / Emoji**: ASCII goes out as key events, everything else as
  `KEYEVENTF_UNICODE` input — any script works, regardless of keyboard layout, and your
  clipboard is never touched
- **🎨 Material 3 Design**: purple (`#6750A4`) Material 3 palette, rounded cards,
  elevation shadows — same visual language as the Python build
- **🌙 Dark / Light Mode**: self-drawn sun/moon toggle (starts by following the system theme)
- **📌 Always on Top**: **on by default**, with a pin button in the app bar
- **🎯 Draggable window**: drag the app bar or use the native title bar
- **🪟 Native window, no console**: system title bar and app icon (maximize button
  removed on purpose — the height is computed from content), runs as a GUI subsystem
  binary so no black console window appears
- **🖼️ Full-colour emoji**: a custom COLR v1 renderer (see below) so 😀 🎉 ❤️ render in
  real colours instead of monochrome outlines
- **🔤 Crisp CJK text**: font fallback chain starting at **Microsoft YaHei**, with font
  hinting disabled so dense characters stay sharp
- **📦 Fully portable, stateless**: one `.exe`, no installer, no runtime dependencies —
  and it **writes nothing anywhere**: no settings file, no logs, no registry keys. Copy
  it to another machine and it behaves exactly the same, starting from clean defaults
  every launch

## 🚀 Use Cases

- Bypass websites that disable paste functionality
- Automate form filling with natural typing speed
- Test typing-based applications
- Simulate human typing behaviour
- Educational demonstrations
- Content entry automation

## 🛠️ Building from Source

### Prerequisites

- **Rust** (stable toolchain, edition 2024 → Rust **1.85+**), MSVC target
- **Visual Studio Build Tools** with the *Desktop development with C++* workload
  (provides `link.exe`)
- **Windows SDK** (provides `rc.exe`, used to embed the app icon into the exe)

### Build

```bash
git clone https://github.com/zz6zz666/typingbot-rs.git
cd typingbot-rs

cargo build --release
```

Result: `target\release\typingbot-rs.exe` (single file, no runtime dependencies).

For development, `cargo run` / `cargo build` work the same way. The binary is a
GUI-subsystem app on Windows (no console window), so **debug builds** write runtime
messages to `%APPDATA%\TypingBot\typingbot.log` instead of stdout. **Release builds
write no log file at all** — set `TB_STATE=1` in the environment if you need one for
troubleshooting.

> The release profile uses `opt-level = "s"`, LTO and `strip = true`; `build.rs` embeds
> `assets/icon.ico` plus the version information (`TypingBot (Rust Edition)`, author
> `zz6zz666`) into the Windows resource section.

## 📖 Usage

1. **Enter Text**: type or paste the text you want to auto-type
2. **Set Speed**: drag the slider (1–500 WPS)
   - 1 WPS = slow, human-like typing
   - 100 WPS = fast typing (default)
   - 500 WPS = ultra-fast typing
3. **Start Typing**: click *Start Typing*
4. **Position Cursor**: during the 3-second countdown, click the field you want to fill
5. **Auto-Type**: the bot types your text at the selected speed
6. **Pause / Stop / Esc**: interrupt at any time; *Pause* resumes where it left off

## 🎯 Speed Guide

| WPS | Characters/Second | Description |
|-----|-------------------|-------------|
| 1 | 6 | Very slow, deliberate typing |
| 10 | 60 | Slow, human-like speed |
| 50 | 300 | Moderate typing speed |
| 100 | 600 | Fast typing (default) |
| 200 | 1200 | Very fast typing |
| 500 | 3000 | Ultra-fast, maximum speed |

*Note: 1 word ≈ 6 characters (5 letters + 1 space)*

## 🧱 Tech Stack

| Area | Crate / API | Notes |
|------|-------------|-------|
| Language | Rust 2024 (MSVC) | no runtime, single `.exe` |
| GUI | `eframe` / `egui` 0.36 (glow) | immediate-mode UI, custom Material 3 widgets |
| Colour emoji | `skrifa` (COLR v1 traversal) + `vello_cpu` | custom renderer, RGBA textures cached per glyph |
| Synthetic input | `enigo` | `SendInput`; ASCII as key events, any Unicode via `KEYEVENTF_UNICODE` |
| Clipboard | `arboard` | the **Paste** button only (typing does not use the clipboard) |
| Global hotkey | `SetWindowsHookExW(WH_KEYBOARD_LL)` (own hook) | Esc cancels the running job |
| Settings | `serde` / `serde_json` | dev builds only — a release build stores nothing |
| Window extras | `winresource`, `raw-window-handle` | exe icon/version, window style tweaks |
| Fonts | Microsoft YaHei → Segoe UI Emoji → … | PUA icon font: Segoe Fluent Icons |

## ⚙️ How It Works (Rust Edition)

1. **Speed calculation** — same formula as the Python version:

   ```text
   delay = 1.0 / (WPS × 6)
   ```

2. **Character typing** — a worker thread walks the text:
   - ASCII characters are sent as key events (`enigo`)
   - any other Unicode character (Bangla, Chinese, emoji, …) is sent as a
     `KEYEVENTF_UNICODE` input event, so it works regardless of keyboard layout and
     never touches your clipboard
   - newline / tab / space are handled explicitly
3. **Control** — the UI thread and the worker share an atomic control block
   (index / paused / stop reason), so Start, Pause, Stop and the global Esc hook all
   work without locks; the worker reports completion back to the UI
4. **Countdown** — the main window slides down out of view, and the same window comes
   back up from below the screen edge as a small bottom-centred card (borderless, with
   rounded corners). The title bar, the client size and the rendered content are all
   swapped while the window is off-screen, so the transition is a pure slide with no
   flicker. When the countdown ends the card slides away and the main window rises back
   into place before typing starts.
5. **Colour emoji** — `egui`'s text pipeline can only rasterise glyph *outlines* and
   tint them with a single colour (coverage mask × vertex colour), so COLR v1 emoji
   would show up monochrome. This port adds the missing piece: `skrifa` traverses the
   COLR v1 layer tree (solid / linear / radial / sweep gradients, transforms, extend
   modes), `vello_cpu` rasterises it into an RGBA texture at device resolution, the
   texture is cached per `(glyph, pixel size)` and painted over the input box while the
   monochrome outline underneath is made transparent.

## 📂 Project Layout

| File | Responsibility |
|------|----------------|
| `src/main.rs` | window setup (size, icon, always-on-top, GUI subsystem) |
| `src/app.rs` | all UI: header, editor, controls, overlays, fonts, layout maths |
| `src/theme.rs` | Material 3 light/dark palettes |
| `src/engine.rs` | typing worker (speed, character pipeline) |
| `src/control.rs` | shared atomic control/state between UI and worker |
| `src/hotkey.rs` | global Esc listener |
| `src/settings.rs` | settings load/save (dev builds only — a release build is stateless) |
| `src/emoji.rs` | COLR v1 colour-emoji renderer |
| `build.rs` | embeds the exe icon + version info |

## 🔧 Configuration

**There is no configuration file.** TypingBot is a stateless, portable tool: a release
build never creates or modifies any file — no settings, no logs, no registry entries —
and it does not read anything left behind by earlier runs, so every launch starts from
the same defaults (speed 100 WPS, theme follows the system, always-on-top on).

Defaults live in code and are easy to change:

| What | Where |
|------|-------|
| Window size, layout spacing, countdown length | `src/app.rs` constants |
| Default speed / theme / pin state | `src/settings.rs` |

For development, set `TB_STATE=1` (or just use a debug build) and the app will persist
`%APPDATA%\TypingBot\settings.json` between runs:

```json
{
  "speed": 100.0,
  "dark_mode": true,
  "last_text": "",
  "pinned": true,
  "theme": "system"
}
```

| Key | Meaning |
|-----|---------|
| `theme` | `"system"` (default — follows the Windows app theme), `"light"` or `"dark"` |
| `dark_mode` | Last effective theme (kept for the Python version's file format) |
| `pinned` | Always-on-top state (the pin button in the app bar) |

## 🐛 Troubleshooting

| Problem | Solution |
|---------|----------|
| Text does not appear | Make sure the cursor is in an editable field and that the window you target accepts simulated keystrokes |
| Typing is too fast/slow | Adjust the slider before starting |
| Nothing happens in games / elevated apps | Apps running **as administrator** ignore input from a non-elevated process — run TypingBot as administrator too |
| Countdown cancelled instantly | The global Esc hook also cancels the countdown; don't press Esc while positioning the cursor |
| Window not on top | Click the pin button in the app bar (it is on by default) |
| Antivirus / SmartScreen warning | See [ANTIVIRUS.md](ANTIVIRUS.md) |
| Need to see what went wrong | No console in a GUI build: debug builds log to `%APPDATA%\TypingBot\typingbot.log`; a release build does the same only when `TB_STATE=1` is set |

## 🧭 Environment Notes

- **Windows-only app, but the repo builds anywhere** that has a Rust toolchain:
  everything Windows-specific (`SetWindowsHookEx`, window style tweaks, registry
  theme lookup, DWM rounding, the exe icon resource) is behind `#[cfg(windows)]`
  or `cfg(windows)` build-dependencies, and missing system fonts (Microsoft YaHei,
  Segoe UI Emoji, …) are skipped gracefully, falling back to egui's bundled fonts.
- System font paths are derived from `%WINDIR%` (not hard-coded to `C:\Windows`).
- **Portable by design: the app stores nothing.** A release build writes no files at all
  (no settings, no logs, no window geometry — eframe's `persistence` feature is not even
  enabled) and reads nothing back, so it is safe to run from a USB stick or a read-only
  location. Debug builds (or `TB_STATE=1`) write `%APPDATA%\TypingBot\settings.json` and
  `typingbot.log` for development only.
- `Cargo.lock` is committed and points at the canonical crates.io index, so a clone
  resolves the same dependency versions anywhere.
- Optional dev knob: set `TB_FONT_HINT=light` (or `normal`) to re-enable font hinting;
  the default is `off`, which renders dense CJK strokes more cleanly.
- Optional dev knob: set `TB_AUTOSTART=1` to start typing right after launch (skipping
  the countdown) using the current text — handy for testing the typing flow without
  clicking.

## 🛡️ Antivirus

TypingBot is not malware, but automation tools that simulate keyboard input and install
a global keyboard hook are frequently flagged by heuristic engines. Details, verification
steps and false-positive reporting: **[ANTIVIRUS.md](ANTIVIRUS.md)**.

## 📝 License

This project is licensed under the **MIT License** — see [LICENSE](LICENSE).

Because this repository is a port of the original TypingBot, the license keeps the
original copyright notice and adds the port's author:

```text
Copyright (c) 2025 Jitu Sarkar (AprajitSarkar) — original TypingBot project
Copyright (c) 2026 zz6zz666 — Rust Edition (native port)
```

## 🙏 Credits & Acknowledgements

**Original project — all of the design work belongs to its author:**

- **TypingBot - Bypass Paste, Bypass Text Pasting** by
  **Aprajit Sarkar (Jitu Sarkar)**
  - GitHub: [@AprajitSarkar](https://github.com/AprajitSarkar)
  - Instagram: [@jitusarkar21](https://instagram.com/jitusarkar21)
  - Repository: [AprajitSarkar/typing-bot-bypass-paste](https://github.com/AprajitSarkar/typing-bot-bypass-paste)

This Rust Edition reuses the original's Material 3 visual design, interaction model,
speed formula, settings format and documentation structure. Many thanks to the original
author for publishing it under the MIT license.

**This port:**

- **zz6zz666** — [github.com/zz6zz666](https://github.com/zz6zz666)
- Repository: [zz6zz666/typingbot-rs](https://github.com/zz6zz666/typingbot-rs)

## 🤝 Contributing

Contributions are welcome — please open an issue or a pull request.

1. Fork the repository
2. Create your feature branch (`git checkout -b feature/AmazingFeature`)
3. Commit your changes (`git commit -m 'Add some AmazingFeature'`)
4. Push to the branch (`git push origin feature/AmazingFeature`)
5. Open a Pull Request

## ⭐ Show Your Support

If this project is useful to you, please give it a ⭐ — and consider starring the
[original repository](https://github.com/AprajitSarkar/typing-bot-bypass-paste) as well.

## 🔑 Keywords

typingbot, bypass paste, bypass text pasting, auto typer, text automation, typing
automation, paste bypass, form filler, keyboard automation, rust, rust-desktop-app,
egui, eframe, material-design-3, windows-automation, unicode-typing, bangla-typing,
multilingual-typing, colr-v1, color-emoji, data-entry-tool, typing-test-helper

---

<div align="center">

**Native Rust port made with ❤️ by zz6zz666 — based on the original work of Jitu Sarkar**

[⬆ Back to Top](#typingbot---bypass-paste-bypass-text-pasting--rust-edition)

</div>
