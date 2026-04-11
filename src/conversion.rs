use crate::{clipboard, input, layouts, log};
use std::collections::HashMap;
use std::sync::LazyLock;

/// Conversion table: maps (from_layout, to_layout) -> char-to-char mapping.
/// Layout IDs: 0x0409 = EN, 0x0419 = RU, 0x0422 = UA
type ConversionMap = HashMap<(u16, u16), HashMap<char, char>>;

static TABLES: LazyLock<ConversionMap> = LazyLock::new(build_tables);

/// Converts text from one layout to another using built-in character tables.
pub fn convert(text: &str, from_layout: u16, to_layout: u16) -> Option<String> {
    let key = (from_layout, to_layout);
    let table = TABLES.get(&key)?;

    Some(
        text.chars()
            .map(|c| table.get(&c).copied().unwrap_or(c))
            .collect(),
    )
}

/// Returns all supported layout IDs.
pub fn supported_layouts() -> Vec<u16> {
    vec![0x0409, 0x0419, 0x0422]
}

/// Detects the most likely source layout based on text characters.
fn detect_text_layout(text: &str) -> Option<u16> {
    let mut has_latin = false;
    let mut has_cyrillic_ru_only = false; // ы, э, ъ, ё
    let mut has_cyrillic_ua_only = false; // і, є, ї, ґ
    let mut has_cyrillic_common = false;  // shared Cyrillic chars

    for c in text.chars() {
        match c {
            'a'..='z' | 'A'..='Z' => has_latin = true,
            'ы' | 'э' | 'ъ' | 'ё' | 'Ы' | 'Э' | 'Ъ' | 'Ё' => has_cyrillic_ru_only = true,
            'і' | 'є' | 'ї' | 'ґ' | 'І' | 'Є' | 'Ї' | 'Ґ' => has_cyrillic_ua_only = true,
            '\u{0400}'..='\u{04FF}' => has_cyrillic_common = true, // Cyrillic block
            _ => {}
        }
    }

    if has_latin && !has_cyrillic_common && !has_cyrillic_ru_only && !has_cyrillic_ua_only {
        Some(0x0409) // English
    } else if has_cyrillic_ua_only {
        Some(0x0422) // Ukrainian (has unique UA chars)
    } else if has_cyrillic_ru_only {
        Some(0x0419) // Russian (has unique RU chars)
    } else {
        None // Ambiguous (common Cyrillic or no identifiable chars) — let caller decide
    }
}

/// Attempts cyclic conversion: detects source layout from text,
/// falls back to current keyboard layout for ambiguous text.
/// Returns the converted text and the target layout ID.
pub fn convert_cyclic(text: &str, current_layout: u16, layout_order: &[u16]) -> (String, u16) {
    let detected = detect_text_layout(text);
    // Use detected layout if definitive, otherwise fall back to current keyboard layout.
    // This handles ambiguous text (e.g. common Cyrillic chars shared by RU and UA).
    let source = match detected {
        Some(lang) => lang,
        None => current_layout,
    };

    eprintln!("[convert] detected={:?}, using source=0x{:04X}", detected.map(|l| format!("0x{:04X}", l)), source);

    let current_idx = layout_order
        .iter()
        .position(|&id| id == source)
        .unwrap_or(0);

    let next_idx = (current_idx + 1) % layout_order.len();
    let target = layout_order[next_idx];

    // Always move to the target layout, even if text doesn't change
    // (e.g. ambiguous Cyrillic "Привет" — same in RU and UA).
    // This ensures the cycle progresses via layout switch.
    let converted = convert(text, source, target).unwrap_or_else(|| text.to_string());
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
    input::wait_for_modifiers_release();

    let saved_clipboard = clipboard::get_text();
    clipboard::set_text("");
    std::thread::sleep(std::time::Duration::from_millis(30));

    input::send_copy();
    std::thread::sleep(std::time::Duration::from_millis(80));

    let text = clipboard::get_text().unwrap_or_default();
    if text.is_empty() {
        restore_clipboard(saved_clipboard);
        return;
    }

    if !convert_and_paste(&text) {
        restore_clipboard(saved_clipboard);
        return;
    }

    std::thread::sleep(std::time::Duration::from_millis(50));
    restore_clipboard(saved_clipboard);
}

/// Performs single-word conversion: selects the word to the left of the cursor
/// (Ctrl+Shift+Left), converts it, pastes back.
pub fn perform_word_conversion() {
    input::wait_for_modifiers_release();

    let saved_clipboard = clipboard::get_text();
    clipboard::set_text("");
    std::thread::sleep(std::time::Duration::from_millis(30));

    input::send_select_word_left();
    std::thread::sleep(std::time::Duration::from_millis(50));

    input::send_copy();
    std::thread::sleep(std::time::Duration::from_millis(80));

    let text = clipboard::get_text().unwrap_or_default();
    if text.is_empty() {
        restore_clipboard(saved_clipboard);
        return;
    }

    if !convert_and_paste(&text) {
        restore_clipboard(saved_clipboard);
        return;
    }

    std::thread::sleep(std::time::Duration::from_millis(50));
    restore_clipboard(saved_clipboard);
}

/// Shared logic: takes text, performs cyclic conversion, pastes result, switches layout.
/// Returns true on success, false if no conversion was possible.
fn convert_and_paste(text: &str) -> bool {
    if text.is_empty() {
        return false;
    }

    let current_layout = layouts::get_current_layout();
    let layout_order = layouts::get_layout_order();

    let supported = supported_layouts();
    let active_order: Vec<u16> = layout_order
        .iter()
        .filter(|id| supported.contains(id))
        .copied()
        .collect();

    if active_order.len() < 2 {
        log!("[convert] need at least 2 supported layouts, found {}", active_order.len());
        return false;
    }

    let source = if active_order.contains(&current_layout) {
        current_layout
    } else {
        active_order[0]
    };

    let (converted, target) = convert_cyclic(text, source, &active_order);
    log!("[convert] '{}' -> '{}' (0x{:04X} → 0x{:04X})",
        truncate(text, 40), truncate(&converted, 40), source, target);

    clipboard::set_text(&converted);
    std::thread::sleep(std::time::Duration::from_millis(50));
    input::send_paste();
    std::thread::sleep(std::time::Duration::from_millis(80));

    // Re-select the just-pasted text so the user can press the hotkey again
    // to cycle through layouts without manually re-selecting.
    let select_count = converted.chars().filter(|&c| c != '\r').count();
    input::send_select_n_left(select_count);
    std::thread::sleep(std::time::Duration::from_millis(30));

    layouts::switch_layout(target);
    true
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

fn build_tables() -> ConversionMap {
    let mut map = ConversionMap::new();

    // EN <-> RU
    let en_ru_pairs: &[(char, char)] = &[
        ('q', 'й'), ('w', 'ц'), ('e', 'у'), ('r', 'к'), ('t', 'е'),
        ('y', 'н'), ('u', 'г'), ('i', 'ш'), ('o', 'щ'), ('p', 'з'),
        ('[', 'х'), (']', 'ъ'), ('a', 'ф'), ('s', 'ы'), ('d', 'в'),
        ('f', 'а'), ('g', 'п'), ('h', 'р'), ('j', 'о'), ('k', 'л'),
        ('l', 'д'), (';', 'ж'), ('\'', 'э'), ('z', 'я'), ('x', 'ч'),
        ('c', 'с'), ('v', 'м'), ('b', 'и'), ('n', 'т'), ('m', 'ь'),
        (',', 'б'), ('.', 'ю'), ('/', '.'),
        ('`', 'ё'), ('Q', 'Й'), ('W', 'Ц'), ('E', 'У'), ('R', 'К'),
        ('T', 'Е'), ('Y', 'Н'), ('U', 'Г'), ('I', 'Ш'), ('O', 'Щ'),
        ('P', 'З'), ('{', 'Х'), ('}', 'Ъ'), ('A', 'Ф'), ('S', 'Ы'),
        ('D', 'В'), ('F', 'А'), ('G', 'П'), ('H', 'Р'), ('J', 'О'),
        ('K', 'Л'), ('L', 'Д'), (':', 'Ж'), ('"', 'Э'), ('Z', 'Я'),
        ('X', 'Ч'), ('C', 'С'), ('V', 'М'), ('B', 'И'), ('N', 'Т'),
        ('M', 'Ь'), ('<', 'Б'), ('>', 'Ю'), ('?', ','),
        ('~', 'Ё'), ('@', '"'), ('#', '№'), ('$', ';'), ('^', ':'),
        ('&', '?'),
    ];
    insert_bidirectional(&mut map, 0x0409, 0x0419, en_ru_pairs);

    // EN <-> UA
    let en_ua_pairs: &[(char, char)] = &[
        ('q', 'й'), ('w', 'ц'), ('e', 'у'), ('r', 'к'), ('t', 'е'),
        ('y', 'н'), ('u', 'г'), ('i', 'ш'), ('o', 'щ'), ('p', 'з'),
        ('[', 'х'), (']', 'ї'), ('a', 'ф'), ('s', 'і'), ('d', 'в'),
        ('f', 'а'), ('g', 'п'), ('h', 'р'), ('j', 'о'), ('k', 'л'),
        ('l', 'д'), (';', 'ж'), ('\'', 'є'), ('z', 'я'), ('x', 'ч'),
        ('c', 'с'), ('v', 'м'), ('b', 'и'), ('n', 'т'), ('m', 'ь'),
        (',', 'б'), ('.', 'ю'), ('/', '.'),
        ('`', 'ґ'), ('Q', 'Й'), ('W', 'Ц'), ('E', 'У'), ('R', 'К'),
        ('T', 'Е'), ('Y', 'Н'), ('U', 'Г'), ('I', 'Ш'), ('O', 'Щ'),
        ('P', 'З'), ('{', 'Х'), ('}', 'Ї'), ('A', 'Ф'), ('S', 'І'),
        ('D', 'В'), ('F', 'А'), ('G', 'П'), ('H', 'Р'), ('J', 'О'),
        ('K', 'Л'), ('L', 'Д'), (':', 'Ж'), ('"', 'Є'), ('Z', 'Я'),
        ('X', 'Ч'), ('C', 'С'), ('V', 'М'), ('B', 'И'), ('N', 'Т'),
        ('M', 'Ь'), ('<', 'Б'), ('>', 'Ю'), ('?', ','),
        ('~', 'Ґ'), ('@', '"'), ('#', '№'), ('$', ';'), ('^', ':'),
        ('&', '?'),
    ];
    insert_bidirectional(&mut map, 0x0409, 0x0422, en_ua_pairs);

    // RU <-> UA
    let ru_ua_pairs: &[(char, char)] = &[
        ('ы', 'і'), ('э', 'є'), ('ъ', 'ї'), ('ё', 'ґ'),
        ('Ы', 'І'), ('Э', 'Є'), ('Ъ', 'Ї'), ('Ё', 'Ґ'),
        // Shared letters map to themselves — no entry needed.
        // Only letters that differ between RU and UA are listed.
    ];
    insert_bidirectional(&mut map, 0x0419, 0x0422, ru_ua_pairs);

    map
}

fn insert_bidirectional(
    map: &mut ConversionMap,
    from: u16,
    to: u16,
    pairs: &[(char, char)],
) {
    let forward = map.entry((from, to)).or_default();
    for &(a, b) in pairs {
        forward.insert(a, b);
    }

    let reverse = map.entry((to, from)).or_default();
    for &(a, b) in pairs {
        reverse.insert(b, a);
    }
}
