//! Dynamic character conversion tables built via `ToUnicodeEx`.
//!
//! For each installed keyboard layout we query which character every physical
//! key produces (both plain and with Shift). From those we derive:
//!
//! - **Per-layout char sets** — every character a layout can produce.
//! - **Per-layout exclusive char sets** — characters unique to one layout
//!   (used to detect the source layout of text).
//! - **Per-pair conversion tables** — `HashMap<char, char>` mapping the
//!   character at each physical key position from one layout to another.
//!
//! Tables are built once at startup and rebuilt automatically when the list
//! of installed layouts changes (detected by the existing 500 ms poller).
//!
//! This replaces the previous hardcoded EN/RU/UA tables and gives us
//! automatic support for any installed layout, including multiple variants
//! of the same language (e.g. Russian standard vs Russian Typewriter).

use crate::layouts::{id_to_hkl, HklId};
use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    MapVirtualKeyExW, ToUnicodeEx, MAPVK_VK_TO_VSC, VK_SHIFT, VK_SPACE,
};

/// Set of virtual key codes we probe to build each layout's character set.
/// Covers the printable keys of a standard ANSI 102 layout.
const PROBED_VKS: &[u16] = &[
    // Row 1: ` 1 2 3 4 5 6 7 8 9 0 - =
    0xC0, 0x31, 0x32, 0x33, 0x34, 0x35, 0x36, 0x37, 0x38, 0x39, 0x30, 0xBD, 0xBB,
    // Row 2: Q W E R T Y U I O P [ ] \
    0x51, 0x57, 0x45, 0x52, 0x54, 0x59, 0x55, 0x49, 0x4F, 0x50, 0xDB, 0xDD, 0xDC,
    // Row 3: A S D F G H J K L ; '
    0x41, 0x53, 0x44, 0x46, 0x47, 0x48, 0x4A, 0x4B, 0x4C, 0xBA, 0xDE,
    // Row 4: Z X C V B N M , . /
    0x5A, 0x58, 0x43, 0x56, 0x42, 0x4E, 0x4D, 0xBC, 0xBE, 0xBF,
];

/// Position of a key stroke: virtual key + shift state.
type KeyPos = (u16, bool);

/// A single layout's keyboard mapping: every probed key position → produced char.
type LayoutMap = HashMap<KeyPos, char>;

struct TableSet {
    /// HKL → set of characters that layout can produce.
    char_sets: HashMap<HklId, HashSet<char>>,
    /// HKL → chars that ONLY this layout produces among installed layouts.
    exclusive_chars: HashMap<HklId, HashSet<char>>,
    /// (from_hkl, to_hkl) → char-to-char conversion.
    conversions: HashMap<(HklId, HklId), Arc<HashMap<char, char>>>,
    /// Snapshot of installed HKLs the tables were built for (for change detection).
    built_for: Vec<HklId>,
}

static TABLES: Mutex<Option<Arc<TableSet>>> = Mutex::new(None);

/// Builds all conversion tables for the given installed layouts. Called at
/// startup and whenever the installed-layout list changes.
pub fn rebuild(installed: &[HklId]) {
    let start = std::time::Instant::now();

    // Phase 1: probe each layout
    let mut layout_maps: HashMap<HklId, LayoutMap> = HashMap::new();
    for &hkl in installed {
        let map = probe_layout(hkl);
        crate::logger::log(&format!(
            "[tables] probed HKL 0x{:08X}: {} key positions",
            hkl,
            map.len()
        ));
        layout_maps.insert(hkl, map);
    }

    // Phase 2: char sets
    let mut char_sets: HashMap<HklId, HashSet<char>> = HashMap::new();
    for (&hkl, map) in &layout_maps {
        let set: HashSet<char> = map.values().copied().collect();
        char_sets.insert(hkl, set);
    }

    // Phase 3: exclusive char sets (chars only this layout produces)
    let mut exclusive_chars: HashMap<HklId, HashSet<char>> = HashMap::new();
    for (&hkl, chars) in &char_sets {
        let exclusive: HashSet<char> = chars
            .iter()
            .filter(|c| {
                !char_sets
                    .iter()
                    .any(|(&other, s)| other != hkl && s.contains(c))
            })
            .copied()
            .collect();
        crate::logger::log(&format!(
            "[tables] HKL 0x{:08X}: {} total chars, {} exclusive",
            hkl,
            chars.len(),
            exclusive.len()
        ));
        exclusive_chars.insert(hkl, exclusive);
    }

    // Phase 4: per-pair conversion tables
    let mut conversions: HashMap<(HklId, HklId), Arc<HashMap<char, char>>> = HashMap::new();
    for &from in installed {
        for &to in installed {
            if from == to {
                continue;
            }
            let from_map = &layout_maps[&from];
            let to_map = &layout_maps[&to];
            let mut table: HashMap<char, char> = HashMap::new();
            for (pos, &from_char) in from_map {
                if let Some(&to_char) = to_map.get(pos) {
                    if from_char != to_char {
                        table.insert(from_char, to_char);
                    }
                }
            }
            conversions.insert((from, to), Arc::new(table));
        }
    }

    let elapsed = start.elapsed();
    crate::logger::log(&format!(
        "[tables] rebuilt for {} layouts ({} pairs) in {}ms",
        installed.len(),
        conversions.len(),
        elapsed.as_millis()
    ));

    *TABLES.lock().unwrap() = Some(Arc::new(TableSet {
        char_sets,
        exclusive_chars,
        conversions,
        built_for: installed.to_vec(),
    }));
}

/// Returns the cached conversion table for a given layout pair, if built.
pub fn get_conversion(from: HklId, to: HklId) -> Option<Arc<HashMap<char, char>>> {
    TABLES.lock().unwrap().as_ref()?.conversions.get(&(from, to)).cloned()
}

/// Returns true if the given HKL pair has a (non-empty) conversion table.
pub fn has_conversion(from: HklId, to: HklId) -> bool {
    TABLES
        .lock()
        .unwrap()
        .as_ref()
        .and_then(|t| t.conversions.get(&(from, to)))
        .map(|m| !m.is_empty())
        .unwrap_or(false)
}

/// Detects the source layout of the given text using exclusive-char scoring.
/// Returns None if no layout has any exclusive-char matches (ambiguous text).
pub fn detect_source_layout(text: &str) -> Option<HklId> {
    let tables = TABLES.lock().unwrap();
    let t = tables.as_ref()?;

    let mut scores: HashMap<HklId, u32> = HashMap::new();
    for c in text.chars() {
        for (&hkl, exclusive) in &t.exclusive_chars {
            if exclusive.contains(&c) {
                *scores.entry(hkl).or_insert(0) += 1;
            }
        }
    }

    crate::logger::log(&format!(
        "[tables] detect: scores={:?}",
        scores
            .iter()
            .map(|(h, s)| format!("0x{:08X}={}", h, s))
            .collect::<Vec<_>>()
    ));

    scores
        .into_iter()
        .max_by_key(|&(_, score)| score)
        .map(|(hkl, _)| hkl)
}

/// Returns true if the currently-cached tables were built for a different set
/// of installed layouts than `current`. Used by the poller to trigger rebuild.
pub fn needs_rebuild(current: &[HklId]) -> bool {
    let tables = TABLES.lock().unwrap();
    let Some(t) = tables.as_ref() else {
        return true;
    };
    if t.built_for.len() != current.len() {
        return true;
    }
    let mut a = t.built_for.clone();
    let mut b = current.to_vec();
    a.sort();
    b.sort();
    a != b
}

/// Probes one layout: for each key in PROBED_VKS, with and without Shift,
/// records what character it produces.
fn probe_layout(hkl: HklId) -> LayoutMap {
    let mut map = LayoutMap::new();
    let hkl_handle = id_to_hkl(hkl);

    for &vk in PROBED_VKS {
        for shift in [false, true] {
            let mut key_state = [0u8; 256];
            if shift {
                key_state[VK_SHIFT.0 as usize] = 0x80;
            }

            let scan = unsafe {
                MapVirtualKeyExW(vk as u32, MAPVK_VK_TO_VSC, Some(hkl_handle))
            };

            let mut buf = [0u16; 8];
            let ret = unsafe {
                ToUnicodeEx(vk as u32, scan, &key_state, &mut buf, 0, Some(hkl_handle))
            };

            if ret == 1 {
                if let Some(c) = char::from_u32(buf[0] as u32) {
                    if !c.is_control() {
                        map.insert((vk, shift), c);
                    }
                }
            } else if ret == -1 {
                // Dead key. Consume it with a neutral call so it doesn't leak
                // into the next probe.
                let mut throwaway = [0u16; 8];
                let zero_state = [0u8; 256];
                unsafe {
                    ToUnicodeEx(
                        VK_SPACE.0 as u32,
                        0,
                        &zero_state,
                        &mut throwaway,
                        0,
                        Some(hkl_handle),
                    );
                }
            }
            // ret == 0: no char produced (e.g. key not mapped in this layout).
            // ret >= 2: ligature (rare, ignore for now).
        }
    }
    map
}
