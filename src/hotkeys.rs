use std::fmt;
use std::sync::Mutex;

/// Modifier flags for hotkey combinations.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Modifiers {
    pub ctrl: bool,
    pub shift: bool,
    pub alt: bool,
}

impl Modifiers {
    pub const NONE: Self = Self { ctrl: false, shift: false, alt: false };
    pub const CTRL: Self = Self { ctrl: true, shift: false, alt: false };
}

/// A hotkey binding: a virtual key code + modifier state.
/// For standalone modifier keys (e.g., LCtrl alone), set vk to the specific
/// modifier VK code (VK_LCONTROL) and modifiers to NONE.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Hotkey {
    pub vk: u16,
    pub modifiers: Modifiers,
}

impl Hotkey {
    /// Returns true if this hotkey is a standalone modifier key press
    /// (e.g., just LCtrl with no other modifiers or keys).
    pub fn is_standalone_modifier(&self) -> bool {
        use windows::Win32::UI::Input::KeyboardAndMouse::*;
        self.modifiers == Modifiers::NONE
            && matches!(
                VIRTUAL_KEY(self.vk),
                VK_LCONTROL | VK_RCONTROL | VK_LSHIFT | VK_RSHIFT | VK_LMENU | VK_RMENU
            )
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.ctrl {
            write!(f, "Ctrl+")?;
        }
        if self.modifiers.shift {
            write!(f, "Shift+")?;
        }
        if self.modifiers.alt {
            write!(f, "Alt+")?;
        }
        let name = match self.vk {
            0xA2 => "LCtrl",
            0xA3 => "RCtrl",
            0xA0 => "LShift",
            0xA1 => "RShift",
            0xA4 => "LAlt",
            0xA5 => "RAlt",
            _ => return write!(f, "0x{:02X}", self.vk),
        };
        write!(f, "{}", name)
    }
}

/// An action triggered by a hotkey.
#[derive(Debug, Clone)]
pub enum HotkeyAction {
    /// Switch to a specific keyboard layout (by lang_id).
    SwitchLayout(u16),
    /// Convert selected/all text cyclically.
    ConvertText,
    /// Convert the word to the left of the cursor (or selection) cyclically.
    ConvertWord,
}

/// A registered hotkey binding.
#[derive(Debug, Clone)]
pub struct HotkeyBinding {
    pub hotkey: Hotkey,
    pub action: HotkeyAction,
}

/// Global list of active hotkey bindings.
static BINDINGS: Mutex<Vec<HotkeyBinding>> = Mutex::new(Vec::new());

/// Registers a set of hotkey bindings (replaces all previous).
pub fn set_bindings(bindings: Vec<HotkeyBinding>) {
    *BINDINGS.lock().unwrap() = bindings;
}

/// Checks if the given key press matches any registered hotkey.
pub fn match_hotkey(vk: u16, modifiers: Modifiers) -> Option<HotkeyAction> {
    let bindings = BINDINGS.lock().unwrap();
    for binding in bindings.iter() {
        if binding.hotkey.vk == vk && binding.hotkey.modifiers == modifiers {
            return Some(binding.action.clone());
        }
    }
    None
}

/// Returns all bindings that are standalone modifier hotkeys.
pub fn get_standalone_modifier_bindings() -> Vec<HotkeyBinding> {
    let bindings = BINDINGS.lock().unwrap();
    bindings
        .iter()
        .filter(|b| b.hotkey.is_standalone_modifier())
        .cloned()
        .collect()
}
