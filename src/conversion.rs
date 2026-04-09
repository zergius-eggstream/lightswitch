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

/// Attempts cyclic conversion: tries each next layout in the chain.
/// Returns the converted text and the target layout ID.
pub fn convert_cyclic(text: &str, current_layout: u16, layout_order: &[u16]) -> (String, u16) {
    let current_idx = layout_order
        .iter()
        .position(|&id| id == current_layout)
        .unwrap_or(0);

    let next_idx = (current_idx + 1) % layout_order.len();
    let target = layout_order[next_idx];

    match convert(text, current_layout, target) {
        Some(converted) => (converted, target),
        None => (text.to_string(), current_layout),
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
