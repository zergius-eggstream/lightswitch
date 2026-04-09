use crate::{clipboard, input, layouts};
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

/// Performs the full text conversion flow:
/// 1. Save clipboard
/// 2. Copy selected text (or select all + copy)
/// 3. Convert through layout chain
/// 4. Paste result
/// 5. Restore clipboard
pub fn perform_conversion() {
    // Save current clipboard content
    let saved_clipboard = clipboard::get_text();

    // Clear clipboard so we can detect if copy worked
    clipboard::set_text("");

    // Small delay to let clipboard settle
    std::thread::sleep(std::time::Duration::from_millis(30));

    // Try to copy selected text
    input::send_copy();
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Read what was copied
    let copied = clipboard::get_text().unwrap_or_default();
    let (text, did_select_all) = if copied.is_empty() {
        // Nothing was selected — select all and copy
        input::send_select_all();
        std::thread::sleep(std::time::Duration::from_millis(30));
        input::send_copy();
        std::thread::sleep(std::time::Duration::from_millis(50));
        let text = clipboard::get_text().unwrap_or_default();
        (text, true)
    } else {
        (copied, false)
    };

    if text.is_empty() {
        // Nothing to convert — restore clipboard and bail
        if let Some(saved) = saved_clipboard {
            clipboard::set_text(&saved);
        }
        return;
    }

    // Get current layout and layout order
    let current_layout = layouts::get_current_layout();
    let layout_order = layouts::get_layout_order();

    eprintln!("[convert] current_layout=0x{:04X}, layout_order={:04X?}", current_layout, layout_order);
    eprintln!("[convert] text to convert: '{}'", truncate(&text, 60));

    // Filter to supported layouts only
    let supported = supported_layouts();
    let active_order: Vec<u16> = layout_order
        .iter()
        .filter(|id| supported.contains(id))
        .copied()
        .collect();
    eprintln!("[convert] active_order (supported only): {:04X?}", active_order);

    if active_order.len() < 2 {
        eprintln!("[convert] Need at least 2 supported layouts, found {}", active_order.len());
        if let Some(saved) = saved_clipboard {
            clipboard::set_text(&saved);
        }
        return;
    }

    // Determine source layout: use current layout if it's supported,
    // otherwise try to guess from the text characters
    let source = if active_order.contains(&current_layout) {
        current_layout
    } else {
        // Default to first in order
        active_order[0]
    };

    let (converted, target) = convert_cyclic(&text, source, &active_order);
    eprintln!("[convert] '{}' -> '{}' (target layout: 0x{:04X})", truncate(&text, 40), truncate(&converted, 40), target);

    // Paste the converted text
    clipboard::set_text(&converted);
    std::thread::sleep(std::time::Duration::from_millis(30));
    input::send_paste();
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Switch system layout to match the converted text
    layouts::switch_layout(target);

    // If we selected all, move cursor to deselect (press End)
    if did_select_all {
        use windows::Win32::UI::Input::KeyboardAndMouse::VK_END;
        input::send_key(VK_END);
    }

    // Restore original clipboard
    std::thread::sleep(std::time::Duration::from_millis(30));
    if let Some(saved) = saved_clipboard {
        clipboard::set_text(&saved);
    }
}

fn truncate(s: &str, max: usize) -> String {
    if s.len() <= max {
        s.to_string()
    } else {
        format!("{}...", &s[..max])
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
