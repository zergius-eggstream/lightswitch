use windows::Win32::UI::Input::KeyboardAndMouse::{GetKeyboardLayoutList, HKL};

/// Represents an installed keyboard layout.
#[derive(Debug, Clone)]
pub struct LayoutInfo {
    pub hkl: HKL,
    pub lang_id: u16,
    pub name: String,
}

/// Retrieves all keyboard layouts currently installed in the system.
pub fn get_installed_layouts() -> Vec<LayoutInfo> {
    let count = unsafe { GetKeyboardLayoutList(None) };
    if count == 0 {
        return Vec::new();
    }

    let mut hkls = vec![HKL::default(); count as usize];
    let actual = unsafe { GetKeyboardLayoutList(Some(&mut hkls)) };
    hkls.truncate(actual as usize);

    hkls.into_iter()
        .map(|hkl| {
            let lang_id = (hkl.0 as usize & 0xFFFF) as u16;
            let name = lang_id_to_name(lang_id);
            LayoutInfo { hkl, lang_id, name }
        })
        .collect()
}

/// Maps a language ID to a human-readable name.
fn lang_id_to_name(lang_id: u16) -> String {
    match lang_id {
        0x0409 => "English (US)".to_string(),
        0x0809 => "English (UK)".to_string(),
        0x0419 => "Русский".to_string(),
        0x0422 => "Українська".to_string(),
        0x0415 => "Polski".to_string(),
        0x0407 => "Deutsch".to_string(),
        0x040C => "Français".to_string(),
        _ => format!("Unknown (0x{:04X})", lang_id),
    }
}
