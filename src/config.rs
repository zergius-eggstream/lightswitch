use crate::hotkeys::{Hotkey, HotkeyAction, HotkeyBinding, Modifiers};
use crate::layouts::HklId;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub layouts: HashMap<String, String>,
    #[serde(default)]
    pub conversion: ConversionConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default)]
    pub autostart: bool,
    #[serde(default = "default_autostart_scope")]
    pub autostart_scope: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ConversionConfig {
    #[serde(default = "default_conversion_hotkey")]
    pub hotkey: String,
    #[serde(default)]
    pub word_hotkey: String,
    #[serde(default = "default_conversion_mode")]
    pub mode: String,
}

fn default_autostart_scope() -> String {
    "user".to_string()
}

fn default_conversion_hotkey() -> String {
    "Pause".to_string()
}

fn default_conversion_mode() -> String {
    "auto".to_string()
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            autostart: false,
            autostart_scope: default_autostart_scope(),
        }
    }
}

impl Default for ConversionConfig {
    fn default() -> Self {
        Self {
            hotkey: default_conversion_hotkey(),
            word_hotkey: String::new(),
            mode: default_conversion_mode(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            general: GeneralConfig::default(),
            layouts: HashMap::new(),
            conversion: ConversionConfig::default(),
        }
    }
}

impl Config {
    pub fn path() -> PathBuf {
        let appdata = std::env::var("APPDATA").unwrap_or_else(|_| ".".to_string());
        PathBuf::from(appdata)
            .join("LightSwitch")
            .join("config.toml")
    }

    pub fn load() -> Self {
        let path = Self::path();
        match std::fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let content =
            toml::to_string_pretty(self).map_err(|e| std::io::Error::other(e.to_string()))?;
        std::fs::write(path, content)
    }

    /// Creates hotkey bindings from the config.
    ///
    /// Layout keys in the config may be in one of two formats:
    /// - **New:** full HKL as hex (up to 16 chars, typically 8), e.g. `"0x04090409"`
    /// - **Old (pre-0.2):** language ID only (4 hex chars), e.g. `"0x0409"` —
    ///   migrated at load time to the first installed HKL with that lang_id
    pub fn to_bindings(&self) -> Vec<HotkeyBinding> {
        let mut bindings = Vec::new();

        for (layout_id_str, hotkey_str) in &self.layouts {
            if hotkey_str.is_empty() {
                continue;
            }
            let Some(hkl_id) = parse_layout_key(layout_id_str) else {
                crate::logger::log(&format!(
                    "[config] skipping binding: can't parse layout key '{}'",
                    layout_id_str
                ));
                continue;
            };
            if let Some(hotkey) = parse_hotkey(hotkey_str) {
                bindings.push(HotkeyBinding {
                    hotkey,
                    action: HotkeyAction::SwitchLayout(hkl_id),
                });
            }
        }

        if !self.conversion.hotkey.is_empty() {
            if let Some(hotkey) = parse_hotkey(&self.conversion.hotkey) {
                bindings.push(HotkeyBinding {
                    hotkey,
                    action: HotkeyAction::ConvertText,
                });
            }
        }

        if !self.conversion.word_hotkey.is_empty() {
            if let Some(hotkey) = parse_hotkey(&self.conversion.word_hotkey) {
                bindings.push(HotkeyBinding {
                    hotkey,
                    action: HotkeyAction::ConvertWord,
                });
            }
        }

        bindings
    }
}

/// Parses a layout key from config (full HKL as hex, e.g. `"0x04090409"`).
pub fn parse_layout_key(s: &str) -> Option<HklId> {
    let hex = s.trim_start_matches("0x").trim_start_matches("0X");
    u64::from_str_radix(hex, 16).ok()
}

/// Formats an HklId for use as a config key (e.g. `"0x04090409"`).
pub fn format_layout_key(id: HklId) -> String {
    format!("0x{:08x}", id)
}

/// Parses a hotkey string like "LCtrl", "RCtrl", "Pause", "Ctrl+1", "Shift+CapsLock".
pub fn parse_hotkey(s: &str) -> Option<Hotkey> {
    let s = s.trim();
    let parts: Vec<&str> = s.split('+').map(|p| p.trim()).collect();

    let mut modifiers = Modifiers::NONE;
    let mut key_part = "";

    for part in &parts {
        match part.to_lowercase().as_str() {
            "ctrl" => modifiers.ctrl = true,
            "shift" => modifiers.shift = true,
            "alt" => modifiers.alt = true,
            _ => key_part = part,
        }
    }

    // If no non-modifier key was found, the entire string might be a standalone modifier
    if key_part.is_empty() {
        key_part = s;
        modifiers = Modifiers::NONE;
    }

    let vk = key_name_to_vk(key_part)?;
    Some(Hotkey { vk, modifiers })
}

/// Converts a virtual key code to a display name.
pub fn vk_to_key_name(vk: u16, modifiers: Modifiers) -> String {
    let mut parts = Vec::new();
    if modifiers.ctrl {
        parts.push("Ctrl".to_string());
    }
    if modifiers.shift {
        parts.push("Shift".to_string());
    }
    if modifiers.alt {
        parts.push("Alt".to_string());
    }

    let key_name = match vk {
        0xA2 => "LCtrl",
        0xA3 => "RCtrl",
        0xA0 => "LShift",
        0xA1 => "RShift",
        0xA4 => "LAlt",
        0xA5 => "RAlt",
        0x14 => "CapsLock",
        0x13 => "Pause",
        0x2C => "PrintScreen",
        0x91 => "ScrollLock",
        0x90 => "NumLock",
        0x09 => "Tab",
        0x1B => "Esc",
        0x20 => "Space",
        0x0D => "Enter",
        0x08 => "Backspace",
        0x2D => "Insert",
        0x2E => "Delete",
        0x24 => "Home",
        0x23 => "End",
        0x21 => "PageUp",
        0x22 => "PageDown",
        0xC0 => "`",
        0x30..=0x39 => return format_parts(&parts, &(vk as u8 as char).to_string()),
        0x41..=0x5A => return format_parts(&parts, &((vk - 0x41 + b'A' as u16) as u8 as char).to_string()),
        0x70..=0x87 => {
            return format_parts(&parts, &format!("F{}", vk - 0x70 + 1));
        }
        _ => return format_parts(&parts, &format!("0x{:02X}", vk)),
    };
    format_parts(&parts, key_name)
}

fn format_parts(parts: &[String], key: &str) -> String {
    if parts.is_empty() {
        key.to_string()
    } else {
        format!("{}+{}", parts.join("+"), key)
    }
}

fn key_name_to_vk(name: &str) -> Option<u16> {
    match name.to_lowercase().as_str() {
        "lctrl" | "lcontrol" => Some(0xA2),
        "rctrl" | "rcontrol" => Some(0xA3),
        "lshift" => Some(0xA0),
        "rshift" => Some(0xA1),
        "lalt" | "lmenu" => Some(0xA4),
        "ralt" | "rmenu" => Some(0xA5),
        "capslock" | "caps" => Some(0x14),
        "pause" | "break" => Some(0x13),
        "printscreen" | "prtsc" => Some(0x2C),
        "scrolllock" => Some(0x91),
        "numlock" => Some(0x90),
        "tab" => Some(0x09),
        "esc" | "escape" => Some(0x1B),
        "space" => Some(0x20),
        "enter" | "return" => Some(0x0D),
        "backspace" | "back" => Some(0x08),
        "insert" | "ins" => Some(0x2D),
        "delete" | "del" => Some(0x2E),
        "home" => Some(0x24),
        "end" => Some(0x23),
        "pageup" | "pgup" => Some(0x21),
        "pagedown" | "pgdn" => Some(0x22),
        "`" | "~" | "oem3" => Some(0xC0),
        "f1" => Some(0x70),
        "f2" => Some(0x71),
        "f3" => Some(0x72),
        "f4" => Some(0x73),
        "f5" => Some(0x74),
        "f6" => Some(0x75),
        "f7" => Some(0x76),
        "f8" => Some(0x77),
        "f9" => Some(0x78),
        "f10" => Some(0x79),
        "f11" => Some(0x7A),
        "f12" => Some(0x7B),
        s if s.len() == 1 => {
            let c = s.chars().next()?;
            match c {
                '0'..='9' => Some(c as u16),
                'a'..='z' => Some(c as u16 - 0x20), // to uppercase VK
                _ => None,
            }
        }
        s if s.starts_with("0x") => u16::from_str_radix(&s[2..], 16).ok(),
        _ => None,
    }
}
