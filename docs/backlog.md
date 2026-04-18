# LightSwitch — Backlog

Free-form ideas and improvements that aren't tied to a specific roadmap stage.
For staged work, see [technical-specification.md](technical-specification.md).

Each item: title, status, problem statement, possible solutions, trade-offs.

---

## Smart cycling without leaving selection

**Status:** idea / not scheduled

**Problem:**
Currently, after `perform_conversion` pastes the converted text, it re-selects
that text via `Shift+Left × N` so the user can press the hotkey again to cycle
through layouts. This works perfectly for cycling, but if the user types
something immediately after the conversion, the selection gets replaced —
and recovering the original text requires **two** `Ctrl+Z` presses instead of
one (because most editors batch the "replace selection with first keystroke"
and "subsequent typing" into separate undo entries).

**Possible solution: state-based smart cycling**

Don't leave selection after paste. Instead track:
- Timestamp of the last conversion
- Length of the last pasted text (in cursor positions)
- Foreground HWND at conversion time

On the next hotkey press:
- If `now - last_conversion_time < ~3s` AND foreground HWND is the same →
  user is cycling. Send `Shift+Left × N` from the current cursor (which
  should still be at the end of the last paste), then convert.
- Otherwise → standard flow (`Ctrl+C` → convert).

**Pros:**
- Cycling still works
- Typing after conversion behaves naturally — no replacement, no undo quirk
- Foreground HWND check guards against window-switch confusion

**Cons:**
- Adds state and timing logic
- Edge case: user clicks somewhere else in the same window within 3 seconds,
  then presses the hotkey — `Shift+Left` selects the wrong text
- Doesn't work if user cycles slowly (>3s between presses)

**Extra mitigation:** before re-selecting, send `Ctrl+C` first. If clipboard
returns text → user has a real new selection, treat it as a fresh conversion.
If clipboard is empty → no selection, safe to assume cycling and re-select.
This nearly eliminates the edge case but adds an extra round-trip.

**Decision deferred until** UI Automation integration (Stage 8), which will
let us check selection state directly via `TextPattern.GetSelection()` —
making this whole timestamp dance unnecessary.

---

## Different background colors for first 5 layouts in tray icon

**Status:** idea / not scheduled

Originally noted in technical-specification.md Stage 7. For quick visual
identification of the active layout, give each of the first 5 installed
layouts a distinct background color (e.g. blue / red / yellow / green /
purple). Layout 6+ falls back to a default color.

---

## Investigate native Windows tray icon font for 3-letter abbreviations

**Status:** idea / not scheduled

The native Windows language switcher renders 3-letter abbreviations (УКР,
РУС, ENG) clearly even at 16×16. Our icon looks slightly cramped. Possible
causes: bitmap font, ClearType nuances, system DPI scaling, or a special
font choice. Investigate whether any Win API exposes the same font / sizing
the language bar uses.

**Key insight from user:** The native Windows language indicator is NOT a
tray icon — it's a **taskbar widget** with ~24px width, no background, and
no edge padding. That's why it looks clean with 3 letters. Our tray icon
is 16×16 with a background box, so we can't directly match it. Options:
- Accept 16px and optimize font rendering (smaller margins, tighter kerning)
- Use 2-letter codes on the 16px icon (EN, RU, UA) for readability
- Explore whether Windows allows custom taskbar widgets (probably not without shell extension)

---

## "About" menu item in tray icon

**Status:** idea / not scheduled

Add an **About** entry to the tray context menu (between Settings and Exit)
that opens a small dialog with:
- One-line description of the program ("Lightweight keyboard layout switcher with text conversion")
- Version (e.g. `0.1.0`)
- Release date
- Link to the repository (when hosted on GitHub)
- Link to the releases page (when available)
- Copyright / license line

Implementation notes:
- Version should come from `env!("CARGO_PKG_VERSION")` so it stays in sync with `Cargo.toml`
- Release date should be baked in at build time via a `build.rs` script or a build-time env var
- Simple Win32 `MessageBoxW` dialog might be enough for a first iteration; a custom window with clickable links later
- Links should open in the default browser via `ShellExecuteW` with the "open" verb

---

## Installer and standalone dual release

**Status:** idea / not scheduled

Two distribution formats:
1. **Standalone .exe** — for power users. Just download and run. Current format.
2. **Installer** — for regular users. Asks:
   - Install for all users (→ Program Files, needs UAC elevation) or
     current user only (→ user profile, no elevation)
   - Offers to configure all layout hotkeys during the setup wizard
   - Creates Start Menu shortcut and uninstaller

Installer tech options: WiX, NSIS, Inno Setup, or a custom Rust-based
installer. Consider `cargo-wix` for WiX integration in the Rust build.

---

## Application icon (not tray)

**Status:** idea / not scheduled

LightSwitch needs a proper application icon for:
- Task Manager process list
- Alt+Tab switcher
- Start Menu
- File Explorer (the .exe itself)
- Installer artwork

**Theme: light / switching.** Candidates:
- A **lightbulb** (on/off = switching) — simple, recognizable silhouette
- A **toggle switch** (up/down) — directly represents switching
- A **light switch plate** (the wall switch) — matches the app name literally
- A **lamp** — warm, friendly metaphor

The icon should be clean at 16×16, 32×32, 48×48, and 256×256. Should work
on both light and dark Windows themes. Consider generating multiple sizes
from an SVG source.

For the tray icon, the dynamic text abbreviation (УКР/ENG) stays — the app
icon is only for non-tray contexts.
