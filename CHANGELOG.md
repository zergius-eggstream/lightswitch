# Changelog

All notable changes to LightSwitch will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.0] — 2026-04-18

A significant rewrite of the core conversion engine plus two new user-visible
features: dynamic conversion tables (Stage 6) for arbitrary keyboard layouts,
and custom per-layout tray colors + an embedded application icon (Stage 7).

### Added
- **Stage 7 — custom per-layout tray-icon colors.** A color swatch next to
  each layout's hotkey field in settings opens the standard Windows color
  picker. Picks are applied immediately (tray icon updates without save)
  and persisted to `config.toml` under `[layout_colors]`. Defaults follow a
  Windows 11 Fluent palette for the first 5 layouts; further layouts get
  stable hash-based colors. Text color (white/black) is chosen
  automatically per WCAG luminance so the 3-letter label stays readable on
  any background.
- **Application icon.** An embedded, multi-resolution ICO (16/20/24/32/40/
  48/64/96/128/256) shows the app's lightbulb-on-its-side logo in Task
  Manager, Alt+Tab, Explorer, and the Start menu. Generated once via
  `cargo run --example gen_icon` and embedded by `build.rs` through
  `winresource`.

### Changed
- **Stage 6a — layout identification by full HKL.** Internal APIs and
  config keys migrated from 16-bit `lang_id` to the full 64-bit `HklId`
  (the Win32 HKL value). Distinguishes multiple layouts that share a
  language (e.g. Russian standard vs. Russian Typewriter). Config layout
  keys are now 8–16 hex chars (`0x04090409`, `0xfffffffff0a80422`).
  **Breaking config change:** 0.1.0 configs with `lang_id`-only keys
  are no longer auto-migrated — delete `config.toml` or reconfigure
  hotkeys in Settings.
- **Stage 6b — dynamic conversion tables via `ToUnicodeEx`.** Replaces the
  hardcoded EN/RU/UA character tables with tables probed at startup for
  every pair of installed layouts. Rebuilt automatically when the list of
  installed layouts changes. Source-language detection now uses
  exclusive-char scoring instead of hardcoded Cyrillic rules — so the app
  works out of the box with any installed layout (Polish, German, Arabic).
- **Word conversion preserves pre-conversion state** — if no selection was
  active before the hotkey, no selection is left after (cursor at end of
  pasted text). Selection-based conversion still re-selects for cycling.
- **Asynchronous file logging** via a background thread + mpsc channel.
  The keyboard hook callback no longer blocks on file I/O, fixing rare
  hotkey ignoring under disk pressure or antivirus interference.
- **Rapid-cycle hotkey fix.** SendInput wrappers (`send_copy`, `send_paste`,
  `send_select_word_left`, etc.) now check the user's physical modifier
  state (tracked from real, non-injected hook events) and avoid touching
  modifiers the user is already holding. Fixes the case where holding Ctrl
  and rapidly tapping Pause caused the second and subsequent taps to be
  misinterpreted as plain Pause because our own `Ctrl up` had released it.
- **Suppression tracking** normalizes `VK_CANCEL` → `VK_PAUSE` before
  inserting into the set, eliminating a short-circuit `||` bug that left
  stale entries on keyup and silently swallowed subsequent Ctrl+Pause
  presses.
- **Build timestamp** is baked into the binary via `build.rs` and printed
  on the first line of the log, making it easy to confirm which build is
  running during testing.

## [0.1.0] — 2026-04-13

First local release. Covers the primary use case: type text, select a
fragment that was typed in the wrong layout, press a hotkey, get it
converted in place. Cycling through layouts by pressing the hotkey
repeatedly also works.

### Added
- Initial project skeleton: tray icon, message loop, Win32 keyboard hook
- Detection of installed keyboard layouts via `GetKeyboardLayoutList`
- Layout switching via standalone modifier keys (tap LCtrl/RCtrl/etc.) without breaking system shortcuts like Ctrl+C, Ctrl+Shift
- Text conversion via clipboard with cyclic layout detection (EN ↔ RU ↔ UA)
- Source layout detection from text characters with fallback to current keyboard layout for ambiguous text
- Automatic system layout switch after conversion to match the converted text
- Settings window with per-layout hotkey assignment, conversion hotkey, and autostart toggle
- Interactive hotkey capture: click a field, press a key, the binding is saved
- Conflict detection: duplicate hotkey assignments are highlighted, Save is disabled until resolved
- Clear (X) button to remove individual hotkey bindings
- TOML-based configuration at `%APPDATA%\LightSwitch\config.toml`
- Per-user autostart via `HKCU\...\Run` registry key (no admin required)
- Hardcoded conversion tables for EN/RU/UA standard layouts
- Single-word conversion via `Ctrl+Shift+Left` selection of the word at the cursor
- Dynamic tray icon showing the current layout's 3-letter native abbreviation (УКР, РУС, ENG, etc.) via `GetLocaleInfoW`
- File logging to `%APPDATA%\LightSwitch\lightswitch.log` for diagnostics, cleared on each startup
- Suppression of auto-repeat keydown events for matched hotkeys (only one action per press)
- Force-release of held modifier keys before injecting our own SendInput sequences
- `KEYEVENTF_EXTENDEDKEY` flag for arrow/navigation keys so they aren't misinterpreted as numpad equivalents
- Normalization of `VK_CANCEL` (the code Windows produces for Ctrl+Pause) back to `VK_PAUSE`
- `catch_unwind` wrapper around the window proc so internal panics don't abort the process

### Known limitations
- Conversion tables are hardcoded and cover only EN/RU/UA
- Multiple keyboard layouts for the same language (e.g. Russian typewriter) are not distinguished — only the language ID is used
- RAlt (AltGr), Win, and Fn keys cannot be used as hotkeys
- No UAC elevation option for capturing keys in admin-level windows
- No icon yet (uses default Windows application icon)
- **Smart-copy editors (Notepad on Win11, VS Code, many Electron apps) copy the current line on `Ctrl+C` when nothing is selected.** LightSwitch can't tell this apart from a real one-line selection, so triggering the conversion hotkey without an explicit selection in such an editor will incorrectly append a converted copy of the current line. Workaround: always make a real selection before pressing the hotkey. Proper fix planned via UI Automation (Stage 8).
