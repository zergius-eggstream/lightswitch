# Contributing to LightSwitch

Thanks for your interest! LightSwitch aims to stay small, focused, and
reliable. Contributions that fit that spirit are very welcome.

## Development setup

Requirements:
- Windows 10 or 11 (x86-64)
- [Rust toolchain](https://rustup.rs/) (1.90+ recommended)
- MSVC build tools (installed automatically by `rustup`)

```sh
git clone https://github.com/zergius-eggstream/lightswitch
cd lightswitch
cargo build          # debug: shows a console window with logs
cargo build --release  # release: hidden window, smaller binary
cargo run            # run debug build
```

Configuration is stored at `%APPDATA%\LightSwitch\config.toml`.
Debug logs land at `%APPDATA%\LightSwitch\lightswitch.log` (cleared on
each startup).

## Project layout

- `src/main.rs` — entry point, tray icon, message loop, window proc
- `src/hooks.rs` — low-level keyboard hook, suppression tracking
- `src/hotkeys.rs` — hotkey binding type, modifier matching
- `src/input.rs` — `SendInput` wrappers (Ctrl+C, paste, select, etc.),
  modifier-aware so user-held modifiers are preserved
- `src/layouts.rs` — installed layout enumeration, HKL identifier
- `src/tables.rs` — dynamic conversion tables built via `ToUnicodeEx`
- `src/conversion.rs` — clipboard round-trip, cyclic conversion flow
- `src/icon.rs` — tray icon rendering (text-on-color via GDI)
- `src/colors.rs` — per-layout color palette, WCAG contrast
- `src/clipboard.rs` — clipboard read/write
- `src/config.rs` — TOML config load/save
- `src/ui.rs` — settings window
- `src/logger.rs` — async file logger (background thread + mpsc channel)
- `build.rs` — bakes build timestamp into the binary, embeds app icon
- `docs/technical-specification.md` — full spec and staged roadmap
- `docs/backlog.md` — free-form post-MVP ideas

## Before submitting a PR

- **Run `cargo build --release`** and make sure there are no warnings.
- **Run `cargo fmt`** to keep formatting consistent.
- **Run `cargo clippy -- -D warnings`** for lint checks.
- **Test the change manually.** This project doesn't have an automated
  test suite for UI/keyboard behavior — when you change something that
  affects the tray, hotkeys, or conversion flow, try it out in Notepad,
  VS Code, and a web browser's address bar at minimum.
- **Keep commits focused** — one logical change per commit. The commit
  message should explain *why* the change was made, not just *what*.
- **Update `CHANGELOG.md`** under the `[Unreleased]` section if your
  change is user-visible.

## Areas where contributions are especially welcome

- Additional keyboard layout scenarios (non-Latin, RTL, IME-based).
- UI polish for the settings window.
- UI Automation integration for smarter selection handling (Stage 8 in
  `docs/technical-specification.md`).
- Tests and CI improvements.
- Translations of UI strings.

See [`docs/backlog.md`](docs/backlog.md) for a fuller list.

## Reporting bugs

Please include:
- Windows version (e.g. "Windows 11 23H2")
- LightSwitch version (visible in the tray menu → About)
- Installed keyboard layouts
- Steps to reproduce
- Relevant section of `lightswitch.log` if available (it's cleared on
  each startup, so reproduce the bug, then grab the log before relaunch)

## Code style notes

- The project targets **Rust 2024 edition** — `unsafe` function bodies
  require explicit `unsafe {}` blocks inside.
- Win32 calls via the `windows` crate often return `Option<T>` or
  `Result<T>` where C code took raw handles — wrap accordingly.
- Low-level keyboard hook callbacks run with a **strict timeout**
  (~300 ms on modern Windows). Never do blocking I/O inside the hook;
  use the async `logger` module and `PostMessageW` for heavy work.
- Favor small, composable functions over monolithic ones. `conversion.rs`
  and `ui.rs` started large and got progressively factored.
