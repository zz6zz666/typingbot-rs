# Antivirus / SmartScreen Notes

This file explains why TypingBot may trigger antivirus or SmartScreen warnings, and how
to verify the binary is clean.

## Why heuristics flag TypingBot

TypingBot is a **keyboard automation tool**, which is exactly the category that heuristic
engines watch closely:

1. **Synthetic keyboard input** — the app drives the keyboard API (`enigo`) to type text
   character by character
2. **Global keyboard hook** — a low-level hook (`rdev`) listens for **Esc** so the user
   can cancel a running job from anywhere
3. **Clipboard access** — non-ASCII characters are typed through the clipboard
   (`arboard` + `Ctrl+V`)
4. **Unsigned binary** — the Rust build is a single native `.exe` without a code-signing
   certificate (unlike the Python build, there is **no** PyInstaller
   self-extracting/packing behaviour)

None of these are malicious on their own, but together they look like a keystroke logger
to automated scanners.

## What TypingBot actually does (transparency)

✅ **DOES**

- Type the text you entered, character by character, only after you press *Start Typing*
- Listen for a single global hotkey (**Esc**) to cancel a job
- Read the clipboard when you press the *Paste* button (typing itself never uses it)
- Embed an icon and version resource in the exe

❌ **DOES NOT**

- Access the network — **the Rust Edition has no networking code at all**
  (the Python version only ran a local Flask server on localhost for its UI)
- Record, store or transmit keystrokes
- **Write anything to disk**: a release build creates no settings file, no log and no
  registry keys — it is completely stateless and portable (debug builds may write
  `%APPDATA%\TypingBot\` for development only)
- Steal credentials or personal data
- Modify system settings, services or startup entries
- Hide itself: the window is a normal, visible, always-on-top desktop window

## For end users

1. **Windows SmartScreen**: click *More info* → *Run anyway*.
2. **Add an exclusion** (PowerShell as Administrator):

   ```powershell
   Add-MpPreference -ExclusionPath "C:\path\to\TypingBot.exe"
   ```

3. **Report a false positive to Microsoft**:
   <https://www.microsoft.com/en-us/wdsi/filesubmission>

## For users building it themselves

1. Windows Defender scan of the built binary:

   ```powershell
   & "C:\Program Files\Windows Defender\MpCmdRun.exe" -Scan -ScanType 3 -File "target\release\typingbot-rs.exe"
   ```

2. Upload the binary to <https://www.virustotal.com> and check the detection count.
3. (Optional) Code-sign the binary with `signtool.exe` if you own a certificate.

## Source code transparency

All source code is in this repository — every capability listed above can be checked in
`src/engine.rs` (typing), `src/hotkey.rs` (Esc hook) and `src/settings.rs` (the dev-only
settings file; release builds never call it):

- **This repository**: <https://github.com/zz6zz666/typingbot-rs>
- **Original project (Python)**: <https://github.com/AprajitSarkar/typing-bot-bypass-paste>
