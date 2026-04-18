use windows::Win32::Foundation::HWND;
use windows::Win32::Globalization::{GetLocaleInfoW, LOCALE_SNATIVELANGUAGENAME};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    ActivateKeyboardLayout, GetKeyboardLayout, GetKeyboardLayoutList, HKL, KLF_SETFORPROCESS,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetForegroundWindow, GetWindowThreadProcessId, PostMessageW, WM_INPUTLANGCHANGEREQUEST,
};

/// Stable numeric identifier for a keyboard layout (full HKL value as u64).
///
/// HKL is a Win32 handle whose low 16 bits hold the language ID and high 16 bits
/// hold the layout device ID. Two layouts for the same language (e.g. Russian
/// standard and Russian Typewriter) share the lang_id but have different HKL
/// device IDs. Using the full HKL as identifier distinguishes them.
pub type HklId = u64;

/// Converts a Win32 `HKL` handle to our stable `HklId`.
pub fn hkl_to_id(hkl: HKL) -> HklId {
    hkl.0 as usize as u64
}

/// Converts our `HklId` back to a Win32 `HKL` handle.
pub fn id_to_hkl(id: HklId) -> HKL {
    HKL(id as usize as *mut _)
}

/// Extracts the language ID (low 16 bits) from an HklId.
pub fn hkl_lang_id(id: HklId) -> u16 {
    (id & 0xFFFF) as u16
}

/// Represents an installed keyboard layout.
#[derive(Debug, Clone)]
pub struct LayoutInfo {
    /// Full HKL-derived stable identifier. The language ID is the low 16 bits,
    /// extractable via `hkl_lang_id`.
    pub hkl_id: HklId,
    /// Human-readable name in the layout's native language (e.g. "Українська").
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
            let hkl_id = hkl_to_id(hkl);
            let name = lang_id_to_name(hkl_lang_id(hkl_id));
            LayoutInfo { hkl_id, name }
        })
        .collect()
}

/// Returns true if the given HKL is currently installed in the system.
pub fn is_installed(id: HklId) -> bool {
    get_installed_layouts().iter().any(|l| l.hkl_id == id)
}

/// Switches the keyboard layout for the foreground window to the given HKL.
pub fn switch_layout(id: HklId) -> bool {
    if !is_installed(id) {
        crate::logger::log(&format!("[layout] not installed: 0x{:08X}", id));
        return false;
    }

    let hkl = id_to_hkl(id);

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

/// Returns the current keyboard layout (full HKL) of the foreground window.
pub fn get_current_layout() -> HklId {
    unsafe {
        let fg = GetForegroundWindow();
        let thread_id = GetWindowThreadProcessId(fg, None);
        let hkl = GetKeyboardLayout(thread_id);
        hkl_to_id(hkl)
    }
}

/// Returns the ordered list of installed layouts as HklIds.
pub fn get_layout_order() -> Vec<HklId> {
    get_installed_layouts().iter().map(|l| l.hkl_id).collect()
}

/// Returns the native language name for a given lang_id (e.g. "Українська").
/// Falls back to a hex code if the lookup fails.
pub fn lang_id_to_name(lang_id: u16) -> String {
    let mut buffer = [0u16; 128];
    let len = unsafe { GetLocaleInfoW(lang_id as u32, LOCALE_SNATIVELANGUAGENAME, Some(&mut buffer)) };
    if len > 0 {
        let s = String::from_utf16_lossy(&buffer[..(len - 1) as usize]);
        let mut chars = s.chars();
        match chars.next() {
            Some(c) => c.to_uppercase().chain(chars).collect(),
            None => format!("Unknown (0x{:04X})", lang_id),
        }
    } else {
        format!("Unknown (0x{:04X})", lang_id)
    }
}

/// Returns a 3-letter abbreviation in the language's native script (e.g. "УКР").
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
