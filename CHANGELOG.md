# Changelog

All notable changes to LightSwitch will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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

### Known limitations
- Conversion tables are hardcoded and cover only EN/RU/UA
- Multiple keyboard layouts for the same language (e.g. Russian typewriter) are not distinguished — only the language ID is used
- RAlt (AltGr), Win, and Fn keys cannot be used as hotkeys
- No UAC elevation option for capturing keys in admin-level windows
- No icon yet (uses default Windows application icon)
