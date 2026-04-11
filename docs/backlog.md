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
