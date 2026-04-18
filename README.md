<p align="center">
  <img src="docs/logo.png" alt="LightSwitch logo" width="128" height="128">
</p>

# LightSwitch

A lightweight, fast, and reliable keyboard layout switcher for Windows with text conversion between layouts.

Switch keyboard layouts with a single key tap (e.g. tap LCtrl for English, RCtrl for Ukrainian), and convert already-typed text from one layout to another with a hotkey — without losing the original clipboard.

## Why another switcher?

Existing alternatives are either bloated keyloggers, abandoned, or feature-incomplete. LightSwitch aims to do one thing well:

- **Lean** — single ~2 MB executable, no dependencies, no installer required
- **Fast** — written in Rust, native Win32, sub-millisecond hook latency
- **Reliable** — no conflicts with system shortcuts (Ctrl+C, Ctrl+Shift, Win+Space all work)
- **Privacy-respecting** — no telemetry, no network, no logging

## Features

- Switch keyboard layouts via standalone modifier keys (tap LCtrl/RCtrl/LAlt/etc.) or key combinations (Ctrl+1, Shift+F1, etc.)
- Convert selected text (or entire text field) between layouts with a hotkey
- Cyclic conversion across all installed layouts (EN → UA → PL → EN)
- Automatic detection of the source layout based on the text
- System layout switches to match converted text — keep typing without manual switching
- Tray icon with settings window
- Per-user autostart (no admin rights required)
- Conflict detection in settings: cannot assign the same hotkey twice

## Status

**Pre-release / MVP in development.** Core functionality works but the project is not yet ready for general use.

Supports any keyboard layout installed in Windows — conversion tables are built dynamically at startup via `ToUnicodeEx`, so English, Ukrainian, Polish, German, Arabic, etc. all work out of the box. Multiple variants of the same language (e.g. English QWERTY vs. English Dvorak) are distinguished by their full HKL.

See [docs/technical-specification.md](docs/technical-specification.md) for the full spec and roadmap.

## Known limitations

- **Apps without UI Automation support** fall back to a clipboard-based flow. This flow can't read the caret position without selecting something first, so in editors with "smart copy" (e.g. Notepad++) pressing the "convert selection" hotkey with nothing explicitly selected may behave unexpectedly. **Workaround:** select the text explicitly before triggering conversion, or keep "Use UI Automation" enabled in settings (default) — most modern editors (Notepad on Win11, Word, VS Code, Chrome, Edge, most Electron apps) expose UIA and aren't affected.
- **RAlt (AltGr), Win, and Fn keys** cannot be used as hotkeys.
- **No UAC elevation option** for capturing keys in admin-level windows.

## Building from source

Requirements:
- Windows 10+ (x86-64)
- [Rust toolchain](https://rustup.rs/) (1.90+ recommended)
- MSVC build tools (installed automatically by `rustup`)

```sh
git clone https://github.com/zergius-eggstream/lightswitch
cd lightswitch
cargo build --release
```

The resulting executable is at `target/release/lightswitch.exe`. It is fully standalone — copy it anywhere and run.

## Running

Just launch `lightswitch.exe`. The app starts hidden in the system tray.

- Right-click the tray icon → **Settings** to configure hotkeys
- Click each layout's hotkey field, then press the desired key (e.g. tap LCtrl)
- Configure the text conversion hotkey (e.g. Pause/Break)
- Optionally enable "Start with Windows"
- Save

Configuration is stored at `%APPDATA%\LightSwitch\config.toml`.

## Supported hotkey keys

| Key | Notes |
|-----|-------|
| LCtrl, RCtrl | Distinguished left/right |
| LShift, RShift | Distinguished left/right |
| LAlt | Works as standalone |
| RAlt | Not supported (AltGr conflict on many layouts) |
| CapsLock | Works as a regular key |
| Function keys, letters, digits, etc. | Combined with Ctrl/Shift/Alt as needed |
| Win key | Not supported yet (Start menu conflict) |
| Fn key | Not visible to Windows API |

**Notes when using modifiers as standalone hotkeys:**

- **LAlt** — Windows may briefly highlight the menu bar in some apps when Alt is pressed alone. LightSwitch does not suppress this side effect.
- **LShift / RShift** — Windows accessibility shortcuts remain active: 5 quick Shift presses trigger Sticky Keys, holding Right Shift for 8 seconds triggers Filter Keys. Disable these in Windows Accessibility settings if they get in the way.
- **Ctrl+Pause** — Windows generates `VK_CANCEL` instead of `VK_PAUSE` for this combo; LightSwitch normalizes this automatically.

## License

TBD (likely MIT or Apache 2.0)

## Contributing

The project is in active development. Issues and pull requests welcome once the first release lands.
