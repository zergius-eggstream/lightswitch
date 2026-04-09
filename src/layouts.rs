use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ActivateKeyboardLayout, GetKeyboardLayoutList, GetKeyboardLayout, HKL, KLF_SETFORPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
};

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

/// Finds the HKL for a given language ID.
pub fn find_hkl_by_lang_id(lang_id: u16) -> Option<HKL> {
    get_installed_layouts()
        .into_iter()
        .find(|l| l.lang_id == lang_id)
        .map(|l| l.hkl)
}

/// Switches the keyboard layout for the foreground window to the given language.
pub fn switch_layout(lang_id: u16) -> bool {
    let Some(hkl) = find_hkl_by_lang_id(lang_id) else {
        eprintln!("[layout] Language 0x{:04X} not found in installed layouts", lang_id);
        return false;
    };

    unsafe {
        // Post WM_INPUTLANGCHANGEREQUEST to the foreground window
        let fg = GetForegroundWindow();
        if fg == HWND::default() {
            return false;
        }

        let result = PostMessageW(
            Some(fg),
            WM_INPUTLANGCHANGEREQUEST,
            windows::Win32::Foundation::WPARAM(0),
            windows::Win32::Foundation::LPARAM(hkl.0 as isize),
        );

        if result.is_ok() {
            eprintln!("[layout] Switched to 0x{:04X} ({})", lang_id, lang_id_to_name(lang_id));
            true
        } else {
            // Fallback: ActivateKeyboardLayout (affects our process)
            let _ = ActivateKeyboardLayout(hkl, KLF_SETFORPROCESS);
            eprintln!("[layout] Fallback switch to 0x{:04X}", lang_id);
            true
        }
    }
}

/// Returns the current keyboard layout lang_id of the foreground window.
pub fn get_current_layout() -> u16 {
    unsafe {
        let fg = GetForegroundWindow();
        let thread_id = GetWindowThreadProcessId(fg, None);
        let hkl = GetKeyboardLayout(thread_id);
        (hkl.0 as usize & 0xFFFF) as u16
    }
}

/// Returns ordered list of installed layout lang_ids.
pub fn get_layout_order() -> Vec<u16> {
    get_installed_layouts().iter().map(|l| l.lang_id).collect()
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
