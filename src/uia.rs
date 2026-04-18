//! UI Automation (UIA) integration — reads the user's selected text directly
//! from the focused control, bypassing the clipboard-based flow entirely.
//!
//! This module wraps just enough of the COM API surface to:
//!
//! - Get the currently focused `IUIAutomationElement`.
//! - Query its `IUIAutomationTextPattern`, if the element exposes one.
//! - Read the current selection as a `String`, or expand around the caret to
//!   the surrounding word and read that.
//!
//! Everything here is opt-in: if [`init`] hasn't been called, or if the
//! focused element doesn't implement `TextPattern`, the helpers return
//! `None` and the caller falls back to clipboard + `SendInput`.
//!
//! COM lifecycle note: we call `CoInitializeEx(COINIT_APARTMENTTHREADED)`
//! on the thread that first calls [`init`] (the main message loop thread
//! in practice) and leave it initialized for the process lifetime. The OS
//! tears down COM when the process exits, so an explicit `CoUninitialize`
//! isn't strictly necessary.

use std::cell::RefCell;

use windows::Win32::System::Com::{
    CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED, CoCreateInstance, CoInitializeEx,
};
use windows::Win32::UI::Accessibility::{
    CUIAutomation, IUIAutomation, IUIAutomationElement, IUIAutomationTextPattern,
    IUIAutomationTextRange, TextPatternRangeEndpoint_End, TextPatternRangeEndpoint_Start,
    TextUnit_Character, TextUnit_Line, UIA_TextPatternId,
};
use windows::core::Interface;

// UIA / COM objects are apartment-threaded — they must be accessed from the
// same thread that created them. The main message-loop thread is where we
// initialize COM, and it's also where hotkey-triggered conversions run
// (via `WM_APP_HOTKEY` → `wnd_proc`), so a thread-local fits naturally.
thread_local! {
    static AUTOMATION: RefCell<Option<IUIAutomation>> = const { RefCell::new(None) };
}

/// Runtime toggle — mirrors `config.general.use_uia`. Updated by main/settings.
/// When `false`, all helpers short-circuit and return `None`.
static USE_UIA: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(true);

pub fn set_enabled(enabled: bool) {
    USE_UIA.store(enabled, std::sync::atomic::Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    USE_UIA.load(std::sync::atomic::Ordering::Relaxed)
}

/// Initializes COM (apartment-threaded) and creates the `IUIAutomation` root
/// on the *current* thread. Must be called from the same thread that will
/// later call the read helpers (in our case, the main message-loop thread).
/// Safe to call more than once — subsequent calls are no-ops on the same
/// thread. Returns `false` if init fails; helpers then always return `None`.
pub fn init() -> bool {
    AUTOMATION.with(|cell| {
        if cell.borrow().is_some() {
            return true;
        }
        unsafe {
            // COINIT_APARTMENTTHREADED is what UIA expects for a GUI thread.
            // Returns S_FALSE if COM was already initialized on this thread,
            // which is fine — `.is_err()` only matches actual failure.
            let hr = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
            if hr.is_err() {
                crate::logger::log(&format!("[uia] CoInitializeEx failed: {hr:?}"));
                return false;
            }
            match CoCreateInstance::<_, IUIAutomation>(&CUIAutomation, None, CLSCTX_INPROC_SERVER) {
                Ok(a) => {
                    *cell.borrow_mut() = Some(a);
                    crate::logger::log("[uia] initialized");
                    true
                }
                Err(e) => {
                    crate::logger::log(&format!("[uia] CoCreateInstance failed: {e:?}"));
                    false
                }
            }
        }
    })
}

/// Returns the current focused UIA element, or `None` if UIA is disabled /
/// uninitialized / the system has no focused element.
fn focused_element() -> Option<IUIAutomationElement> {
    if !is_enabled() {
        return None;
    }
    AUTOMATION.with(|cell| {
        let borrow = cell.borrow();
        let automation = borrow.as_ref()?;
        unsafe { automation.GetFocusedElement().ok() }
    })
}

/// Tries to get the `TextPattern` from the given element.
fn text_pattern(element: &IUIAutomationElement) -> Option<IUIAutomationTextPattern> {
    unsafe {
        let pattern = element.GetCurrentPattern(UIA_TextPatternId).ok()?;
        pattern.cast::<IUIAutomationTextPattern>().ok()
    }
}

/// Reads the text of a UIA text range, up to a generous cap.
fn range_text(range: &IUIAutomationTextRange) -> Option<String> {
    unsafe {
        // -1 means "no limit". UIA clamps internally.
        let bstr = range.GetText(-1).ok()?;
        let s = bstr.to_string();
        Some(s)
    }
}

/// Reads the currently selected text from the focused element, if any.
/// Returns `None` in any of these cases:
/// - UIA is disabled (config or init failed)
/// - No focused element / no TextPattern support
/// - Nothing is selected
/// - The selection is empty (`""`)
pub fn get_selected_text() -> Option<String> {
    let element = focused_element()?;
    let pattern = text_pattern(&element)?;

    unsafe {
        let selection = pattern.GetSelection().ok()?;
        let count = selection.Length().ok()?;
        if count == 0 {
            return None;
        }
        // Real multi-range selections are rare; first range is enough for our needs.
        let range = selection.GetElement(0).ok()?;
        let text = range_text(&range)?;
        if text.is_empty() { None } else { Some(text) }
    }
}

/// Selects and returns the "word" surrounding the caret, where "word" is
/// defined as a maximal run of **non-whitespace** characters. This is
/// intentionally different from UIA's `TextUnit_Word`, because UIA's
/// definition:
///
/// 1. Splits on punctuation — so `ghb,jh` is two UIA-words (`ghb` and
///    `jh`), which breaks cyclic conversion: the second hotkey press
///    only re-converts one half.
/// 2. In some apps (Notepad Win11, Word) includes the **trailing
///    whitespace** in the word range, so after pasting the replacement
///    the caret lands past the space — the next cycle then selects the
///    next word, not the one we just converted.
///
/// Our definition is whitespace-delimited: we grab the enclosing line
/// via UIA, locate the caret column by reading the prefix text from
/// line-start to caret, and in Rust walk outward from that column until
/// we hit whitespace on each side. Then we move the range endpoints to
/// exactly that span. Result: `ghb,jh` is one unit, trailing spaces are
/// excluded, cycling works.
///
/// Returns `None` if UIA isn't available, the focused element doesn't
/// implement `TextPattern`, or the caret is on whitespace.
pub fn select_word_at_caret() -> Option<String> {
    let element = focused_element()?;
    let pattern = text_pattern(&element)?;

    unsafe {
        let selection = pattern.GetSelection().ok()?;
        if selection.Length().ok()? == 0 {
            return None;
        }
        let caret_range = selection.GetElement(0).ok()?;

        // Enclosing line: gives us the text surrounding the caret.
        let line_range = caret_range.Clone().ok()?;
        line_range.ExpandToEnclosingUnit(TextUnit_Line).ok()?;
        let line_text = range_text(&line_range)?;

        // Caret column (in characters) within the line: build a range from
        // line-start to caret-start, read its text, count the chars.
        let prefix_range = caret_range.Clone().ok()?;
        prefix_range
            .MoveEndpointByRange(
                TextPatternRangeEndpoint_Start,
                &line_range,
                TextPatternRangeEndpoint_Start,
            )
            .ok()?;
        let caret_col = range_text(&prefix_range)?.chars().count();

        // Walk outward until whitespace / line boundary.
        let chars: Vec<char> = line_text.chars().collect();
        let mut start_col = caret_col;
        let mut end_col = caret_col;
        while start_col > 0 && !chars[start_col - 1].is_whitespace() {
            start_col -= 1;
        }
        while end_col < chars.len() && !chars[end_col].is_whitespace() {
            end_col += 1;
        }
        if start_col == end_col {
            // Caret sits on whitespace — nothing to convert.
            return None;
        }

        // Contract the line range to [start_col, end_col) in the line.
        let word_range = line_range.Clone().ok()?;
        if start_col > 0 {
            word_range
                .MoveEndpointByUnit(
                    TextPatternRangeEndpoint_Start,
                    TextUnit_Character,
                    start_col as i32,
                )
                .ok()?;
        }
        let tail = chars.len() - end_col;
        if tail > 0 {
            word_range
                .MoveEndpointByUnit(
                    TextPatternRangeEndpoint_End,
                    TextUnit_Character,
                    -(tail as i32),
                )
                .ok()?;
        }

        word_range.Select().ok()?;
        let word_text = range_text(&word_range)?;
        if word_text.trim().is_empty() {
            None
        } else {
            Some(word_text)
        }
    }
}
