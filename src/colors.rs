//! Per-layout background colors for the tray icon.
//!
//! Default palette is Windows 11 Fluent colors (first 5 layouts). Beyond 5,
//! a deterministic pseudo-random color is derived from the HKL so each
//! layout gets a distinct-but-stable color across sessions.
//!
//! The user can override any layout's color via the settings window;
//! overrides are held in-memory (loaded from config at startup) and
//! consulted before the default palette.

use crate::layouts::HklId;
use std::collections::HashMap;
use std::sync::Mutex;

/// 0x00RRGGBB packed color (low 24 bits, high byte unused).
pub type Color = u32;

/// Default palette — Windows 11 Fluent colors chosen for strong white-text contrast
/// and distinctness at 16×16.
const DEFAULT_PALETTE: &[Color] = &[
    0x005FB8, // Microsoft blue
    0xC42B1C, // Windows red
    0x107C10, // Windows green
    0xCA5010, // burnt orange
    0x8764B8, // soft purple
];

/// Fallback color for layouts beyond the palette — stable per-HKL hash → HSL.
fn hash_color(hkl: HklId) -> Color {
    // xorshift-like mixer, then map to HSL (fixed S/L, variable hue).
    let mut h = hkl ^ (hkl >> 33);
    h = h.wrapping_mul(0xff51_afd7_ed55_8ccd);
    h ^= h >> 33;

    let hue = (h % 360) as f32;
    hsl_to_rgb(hue, 0.65, 0.40)
}

/// Returns the color for a layout. Checks user overrides first, then palette by
/// index, then hash-based fallback. `installed_index` is the layout's position
/// in the system's installed-layouts list.
pub fn get_color(hkl: HklId, installed_index: usize) -> Color {
    if let Ok(guard) = OVERRIDES.lock() {
        if let Some(ov) = guard.as_ref() {
            if let Some(&c) = ov.get(&hkl) {
                return c;
            }
        }
    }
    default_color(hkl, installed_index)
}

pub fn default_color(hkl: HklId, installed_index: usize) -> Color {
    if installed_index < DEFAULT_PALETTE.len() {
        DEFAULT_PALETTE[installed_index]
    } else {
        hash_color(hkl)
    }
}

/// Returns a contrasting text color (black or white) for the given background,
/// using the simplified WCAG relative-luminance formula.
pub fn text_color_for(bg: Color) -> Color {
    let r = ((bg >> 16) & 0xFF) as f32;
    let g = ((bg >> 8) & 0xFF) as f32;
    let b = (bg & 0xFF) as f32;
    let luminance = (0.299 * r + 0.587 * g + 0.114 * b) / 255.0;
    if luminance > 0.5 { 0x000000 } else { 0xFFFFFF }
}

/// Parses `"#RRGGBB"` (or `"RRGGBB"` with no hash). Returns None on malformed input.
pub fn parse_hex(s: &str) -> Option<Color> {
    let hex = s.trim().trim_start_matches('#');
    if hex.len() != 6 {
        return None;
    }
    u32::from_str_radix(hex, 16).ok()
}

/// Formats as `"#RRGGBB"` (uppercase).
pub fn format_hex(color: Color) -> String {
    format!("#{:06X}", color & 0xFFFFFF)
}

// ---- Shared override state (loaded from config, updated by settings UI) ----

static OVERRIDES: Mutex<Option<HashMap<HklId, Color>>> = Mutex::new(None);

/// Replaces the full override map. Called at startup and on full config save.
pub fn set_overrides(map: HashMap<HklId, Color>) {
    *OVERRIDES.lock().unwrap() = Some(map);
}

/// Adds or updates a single override (used for immediate preview when the user
/// picks a new color in settings).
pub fn set_override(hkl: HklId, color: Color) {
    let mut guard = OVERRIDES.lock().unwrap();
    let map = guard.get_or_insert_with(HashMap::new);
    map.insert(hkl, color);
}

/// Removes a single override (falls back to default palette).
pub fn clear_override(hkl: HklId) {
    if let Some(map) = OVERRIDES.lock().unwrap().as_mut() {
        map.remove(&hkl);
    }
}

// ---- HSL → RGB helper for the hash-based fallback ----

fn hsl_to_rgb(h: f32, s: f32, l: f32) -> Color {
    let c = (1.0 - (2.0 * l - 1.0).abs()) * s;
    let h_seg = h / 60.0;
    let x = c * (1.0 - (h_seg.rem_euclid(2.0) - 1.0).abs());
    let (r1, g1, b1) = match h as u32 {
        0..=59 => (c, x, 0.0),
        60..=119 => (x, c, 0.0),
        120..=179 => (0.0, c, x),
        180..=239 => (0.0, x, c),
        240..=299 => (x, 0.0, c),
        _ => (c, 0.0, x),
    };
    let m = l - c / 2.0;
    let to_byte = |v: f32| ((v + m).clamp(0.0, 1.0) * 255.0) as u32;
    (to_byte(r1) << 16) | (to_byte(g1) << 8) | to_byte(b1)
}
