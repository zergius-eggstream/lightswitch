use crate::layouts::{self, HklId};
use crate::{clipboard, input, log, tables, uia};

/// Attempts cyclic conversion: detects source layout from text,
/// falls back to current keyboard layout for ambiguous text.
/// Returns the converted text and the target HklId.
pub fn convert_cyclic(
    text: &str,
    current_layout: HklId,
    layout_order: &[HklId],
) -> (String, HklId) {
    let source = tables::detect_source_layout(text).unwrap_or(current_layout);

    let current_idx = layout_order
        .iter()
        .position(|&id| id == source)
        .unwrap_or(0);
    let next_idx = (current_idx + 1) % layout_order.len();
    let target = layout_order[next_idx];

    let converted = match tables::get_conversion(source, target) {
        Some(table) => text
            .chars()
            .map(|c| table.get(&c).copied().unwrap_or(c))
            .collect(),
        None => text.to_string(),
    };
    (converted, target)
}

/// Converts the currently selected text via clipboard.
/// If nothing is selected — does nothing.
///
/// **Known limitation:** in editors with "smart copy" (Notepad on Windows 11,
/// VS Code, many Electron apps, etc.) Ctrl+C without an explicit selection
/// copies the current line. We can't distinguish this from a real one-line
/// selection without higher-level APIs (UI Automation), so pressing this
/// hotkey with no selection in such an editor will append a converted copy
/// of the current line. Workaround: always make a real selection. Proper
/// fix planned via UI Automation (Stage 8).
pub fn perform_conversion() {
    // 1. Try UIA first — it reports the real selection without touching the
    //    clipboard, which avoids the "smart copy" bug in Notepad Win11, VS
    //    Code, and other modern editors.
    let uia_text = uia::get_selected_text();
    if let Some(text) = &uia_text {
        log!("[uia] read selection: {} chars", text.len());
    }

    let saved_clipboard = clipboard::get_text();

    let text = match uia_text {
        Some(t) => t,
        None => {
            // 2. Fallback: clipboard + Ctrl+C. This hits the smart-copy
            //    issue in editors that copy the current line on empty
            //    selection — documented limitation.
            clipboard::set_text("");
            std::thread::sleep(std::time::Duration::from_millis(30));
            input::send_copy();
            std::thread::sleep(std::time::Duration::from_millis(80));
            let t = clipboard::get_text().unwrap_or_default();
            if t.is_empty() {
                restore_clipboard(saved_clipboard);
                return;
            }
            t
        }
    };

    let Some(converted_len) = convert_and_paste(&text) else {
        restore_clipboard(saved_clipboard);
        return;
    };

    // Re-select the pasted text so the user can cycle through layouts
    // with repeated hotkey presses.
    input::send_select_n_left(converted_len);
    std::thread::sleep(std::time::Duration::from_millis(30));

    std::thread::sleep(std::time::Duration::from_millis(50));
    restore_clipboard(saved_clipboard);
}

/// Performs single-word conversion: selects the word to the left of the cursor
/// (Ctrl+Shift+Left), converts it, pastes back.
pub fn perform_word_conversion() {
    // UIA path: expand around the caret to a Word unit, Select() it.
    // After this the selection is the word; the rest of the flow is the
    // same as the selection-based conversion — we just don't re-select
    // at the end, so the cursor lands after the pasted text.
    let uia_text = uia::select_word_at_caret();
    if let Some(text) = &uia_text {
        log!("[uia] selected word: {} chars", text.len());
    }

    let saved_clipboard = clipboard::get_text();

    let text = match uia_text {
        Some(t) => t,
        None => {
            // Fallback: send Ctrl+Shift+Left, then copy.
            clipboard::set_text("");
            std::thread::sleep(std::time::Duration::from_millis(30));
            input::send_select_word_left();
            std::thread::sleep(std::time::Duration::from_millis(50));
            input::send_copy();
            std::thread::sleep(std::time::Duration::from_millis(80));
            let t = clipboard::get_text().unwrap_or_default();
            if t.is_empty() {
                restore_clipboard(saved_clipboard);
                return;
            }
            t
        }
    };

    // Word conversion doesn't re-select the pasted text — cursor stays at
    // the end, matching the pre-conversion state.
    if convert_and_paste(&text).is_none() {
        restore_clipboard(saved_clipboard);
        return;
    }

    std::thread::sleep(std::time::Duration::from_millis(50));
    restore_clipboard(saved_clipboard);
}

/// Shared logic: takes text, performs cyclic conversion, pastes result, switches layout.
/// Returns `Some(pasted_char_count)` on success (usable for re-selection by
/// the caller), or `None` if no conversion was possible.
fn convert_and_paste(text: &str) -> Option<usize> {
    if text.is_empty() {
        return None;
    }

    let current_layout = layouts::get_current_layout();
    let layout_order = layouts::get_layout_order();

    if layout_order.len() < 2 {
        log!(
            "[convert] need at least 2 installed layouts, found {}",
            layout_order.len()
        );
        return None;
    }

    let source = if layout_order.contains(&current_layout) {
        current_layout
    } else {
        layout_order[0]
    };

    let (converted, target) = convert_cyclic(text, source, &layout_order);
    log!(
        "[convert] '{}' -> '{}' (0x{:08X} → 0x{:08X})",
        truncate(text, 40),
        truncate(&converted, 40),
        source,
        target
    );

    clipboard::set_text(&converted);
    std::thread::sleep(std::time::Duration::from_millis(50));
    input::send_paste();
    std::thread::sleep(std::time::Duration::from_millis(80));

    layouts::switch_layout(target);

    // Return pasted char count (skip '\r' since editors normalize \r\n → \n).
    // Caller decides whether to re-select based on use case.
    Some(converted.chars().filter(|&c| c != '\r').count())
}

fn restore_clipboard(saved: Option<String>) {
    if let Some(text) = saved {
        clipboard::set_text(&text);
    }
}

/// Truncates a string to at most `max` characters (not bytes), appending "..." if cut.
/// Safe for UTF-8 text including multi-byte characters.
fn truncate(s: &str, max: usize) -> String {
    let char_count = s.chars().count();
    if char_count <= max {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max).collect();
        format!("{}...", truncated)
    }
}
