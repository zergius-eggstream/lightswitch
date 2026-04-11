use windows::Win32::Foundation::HWND;
use windows::Win32::Globalization::{GetLocaleInfoW, LOCALE_SNATIVELANGUAGENAME};
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
        crate::logger::log(&format!("[layout] not installed: 0x{:04X}", lang_id));
        return false;
    };

    unsafe {
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

        if result.is_err() {
            // Fallback: ActivateKeyboardLayout (affects our process)
            let _ = ActivateKeyboardLayout(hkl, KLF_SETFORPROCESS);
        }
        true
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

/// Returns the native language name for a given lang_id (e.g. "українська", "русский", "English").
/// Falls back to a hex code if the lookup fails.
pub fn lang_id_to_name(lang_id: u16) -> String {
    let mut buffer = [0u16; 128];
    let len = unsafe { GetLocaleInfoW(lang_id as u32, LOCALE_SNATIVELANGUAGENAME, Some(&mut buffer)) };
    if len > 0 {
        let s = String::from_utf16_lossy(&buffer[..(len - 1) as usize]);
        // Capitalize first letter for display ("українська" → "Українська")
        let mut chars = s.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().chain(chars).collect(),
            None => format!("Unknown (0x{:04X})", lang_id),
        }
    } else {
        format!("Unknown (0x{:04X})", lang_id)
    }
}

/// Returns a 3-letter abbreviation in the language's native script (e.g. "УКР", "РУС", "ENG").
/// Built by taking the first 3 characters of the native language name and uppercasing them.
pub fn lang_id_to_abbrev(lang_id: u16) -> String {
    let mut buffer = [0u16; 128];
    let len = unsafe { GetLocaleInfoW(lang_id as u32, LOCALE_SNATIVELANGUAGENAME, Some(&mut buffer)) };
    if len > 0 {
        let s = String::from_utf16_lossy(&buffer[..(len - 1) as usize]);
        s.chars()
            .take(3)
            .flat_map(|c| c.to_uppercase())
            .collect()
    } else {
        "??".to_string()
    }
}
