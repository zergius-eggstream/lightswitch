use crate::config::{self, Config};
use crate::hooks;
use crate::hotkeys::Modifiers;
use crate::layouts;
use std::sync::Mutex;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::Graphics::Gdi::{GetStockObject, DEFAULT_GUI_FONT};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Input::KeyboardAndMouse::*;
use windows::Win32::UI::WindowsAndMessaging::*;

const ID_SAVE: u16 = 2001;
const ID_CANCEL: u16 = 2002;
const ID_AUTOSTART: u16 = 2003;
const ID_CONVERSION_HOTKEY: u16 = 2100;
const ID_WORD_HOTKEY: u16 = 2101;
const ID_LAYOUT_HOTKEY_BASE: u16 = 3000;
const ID_CLEAR_BASE: u16 = 4000;         // 4000, 4001, 4002... clear buttons for layouts
const ID_CLEAR_CONVERSION: u16 = 4100;
const ID_CLEAR_WORD: u16 = 4101;

const BST_CHECKED_VAL: usize = 1;
const BM_SETCHECK_MSG: u32 = 0x00F1;
const BM_GETCHECK_MSG: u32 = 0x00F0;

struct SettingsState {
    config: Config,
    layout_ids: Vec<u16>,
    capturing_control: Option<u16>,
}

static STATE: Mutex<Option<SettingsState>> = Mutex::new(None);
static SETTINGS_HWND: Mutex<Option<isize>> = Mutex::new(None);

pub fn show_settings() {
    if let Some(hwnd_val) = *SETTINGS_HWND.lock().unwrap() {
        let hwnd = HWND(hwnd_val as *mut _);
        unsafe {
            SetForegroundWindow(hwnd);
        }
        return;
    }

    std::thread::spawn(|| {
        create_settings_window();
    });
}

fn create_settings_window() {
    let installed = layouts::get_installed_layouts();
    let config = Config::load();
    let layout_ids: Vec<u16> = installed.iter().map(|l| l.lang_id).collect();

    *STATE.lock().unwrap() = Some(SettingsState {
        config: config.clone(),
        layout_ids: layout_ids.clone(),
        capturing_control: None,
    });

    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        let class_name = w!("LightSwitchSettingsClass");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(settings_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            hbrBackground: windows::Win32::Graphics::Gdi::HBRUSH(
                (15 + 1) as *mut _, // COLOR_BTNFACE = 15
            ),
            ..Default::default()
        };
        RegisterClassW(&wc);

        let num_layouts = installed.len();
        let window_height = 190 + (num_layouts as i32 * 30) + 60;

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("LightSwitch \u{2014} Settings"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            420,
            window_height,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .unwrap();

        *SETTINGS_HWND.lock().unwrap() = Some(hwnd.0 as isize);

        let font = GetStockObject(DEFAULT_GUI_FONT);

        let mut y = 10;
        create_label(hwnd, "Keyboard Layouts:", 10, y, 390, 20, font.0);
        y += 25;

        for (i, layout) in installed.iter().enumerate() {
            let label = format!("{}:", layout.name);
            create_label(hwnd, &label, 20, y + 2, 150, 20, font.0);

            let control_id = ID_LAYOUT_HOTKEY_BASE + i as u16;
            let hotkey_str = config
                .layouts
                .get(&format!("0x{:04x}", layout.lang_id))
                .cloned()
                .unwrap_or_default();

            let display = if hotkey_str.is_empty() {
                "(click to set)".to_string()
            } else {
                hotkey_str
            };
            create_button(hwnd, &display, 170, y, 170, 24, control_id, font.0);
            create_button(hwnd, "X", 345, y, 26, 24, ID_CLEAR_BASE + i as u16, font.0);
            y += 30;
        }

        y += 10;
        create_label(hwnd, "Text Conversion:", 10, y, 390, 20, font.0);
        y += 25;
        create_label(hwnd, "All / Selection:", 20, y + 2, 150, 20, font.0);

        let conv_display = if config.conversion.hotkey.is_empty() {
            "(click to set)".to_string()
        } else {
            config.conversion.hotkey.clone()
        };
        create_button(hwnd, &conv_display, 170, y, 170, 24, ID_CONVERSION_HOTKEY, font.0);
        create_button(hwnd, "X", 345, y, 26, 24, ID_CLEAR_CONVERSION, font.0);
        y += 30;

        create_label(hwnd, "Last word:", 20, y + 2, 150, 20, font.0);
        let word_display = if config.conversion.word_hotkey.is_empty() {
            "(click to set)".to_string()
        } else {
            config.conversion.word_hotkey.clone()
        };
        create_button(hwnd, &word_display, 170, y, 170, 24, ID_WORD_HOTKEY, font.0);
        create_button(hwnd, "X", 345, y, 26, 24, ID_CLEAR_WORD, font.0);
        y += 40;

        create_checkbox(hwnd, "Start with Windows", 20, y, 200, 20, ID_AUTOSTART, config.general.autostart, font.0);
        y += 35;

        create_button(hwnd, "Save", 200, y, 90, 28, ID_SAVE, font.0);
        create_button(hwnd, "Cancel", 300, y, 90, 28, ID_CANCEL, font.0);

        // Validate existing config for conflicts
        {
            let state = STATE.lock().unwrap();
            if let Some(ref s) = *state {
                refresh_all_buttons(hwnd, s);
            }
        }

        let _ = ShowWindow(hwnd, SW_SHOW);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        *SETTINGS_HWND.lock().unwrap() = None;
        *STATE.lock().unwrap() = None;
    }
}

fn create_label(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, font: *mut core::ffi::c_void) {
    let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { CreateWindowExW(
        Default::default(),
        w!("STATIC"),
        windows::core::PCWSTR(text_wide.as_ptr()),
        WS_CHILD | WS_VISIBLE,
        x, y, w, h,
        Some(parent), None, None, None,
    ).unwrap() };
    unsafe { SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(font as usize)), Some(LPARAM(1))) };
}

fn create_button(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, id: u16, font: *mut core::ffi::c_void) {
    let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { CreateWindowExW(
        Default::default(),
        w!("BUTTON"),
        windows::core::PCWSTR(text_wide.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_PUSHBUTTON as u32),
        x, y, w, h,
        Some(parent), Some(HMENU(id as *mut _)), None, None,
    ).unwrap() };
    unsafe { SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(font as usize)), Some(LPARAM(1))) };
}

fn create_checkbox(parent: HWND, text: &str, x: i32, y: i32, w: i32, h: i32, id: u16, checked: bool, font: *mut core::ffi::c_void) {
    let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let hwnd = unsafe { CreateWindowExW(
        Default::default(),
        w!("BUTTON"),
        windows::core::PCWSTR(text_wide.as_ptr()),
        WS_CHILD | WS_VISIBLE | WINDOW_STYLE(BS_AUTOCHECKBOX as u32),
        x, y, w, h,
        Some(parent), Some(HMENU(id as *mut _)), None, None,
    ).unwrap() };
    unsafe { SendMessageW(hwnd, WM_SETFONT, Some(WPARAM(font as usize)), Some(LPARAM(1))) };
    if checked {
        unsafe { SendMessageW(hwnd, BM_SETCHECK_MSG, Some(WPARAM(BST_CHECKED_VAL)), Some(LPARAM(0))) };
    }
}

unsafe extern "system" fn settings_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as u16;
            let notification = ((wparam.0 >> 16) & 0xFFFF) as u16;

            if notification == BN_CLICKED as u16 {
                match id {
                    ID_SAVE => {
                        save_settings(hwnd);
                        unsafe { DestroyWindow(hwnd).unwrap() };
                    }
                    ID_CANCEL => {
                        unsafe { DestroyWindow(hwnd).unwrap() };
                    }
                    _ if id == ID_CONVERSION_HOTKEY
                        || id == ID_WORD_HOTKEY
                        || (id >= ID_LAYOUT_HOTKEY_BASE && id < ID_LAYOUT_HOTKEY_BASE + 20) =>
                    {
                        start_hotkey_capture(hwnd, id);
                    }
                    _ if id == ID_CLEAR_CONVERSION => {
                        clear_hotkey(hwnd, ID_CONVERSION_HOTKEY);
                    }
                    _ if id == ID_CLEAR_WORD => {
                        clear_hotkey(hwnd, ID_WORD_HOTKEY);
                    }
                    _ if id >= ID_CLEAR_BASE && id < ID_CLEAR_BASE + 20 => {
                        let layout_control = ID_LAYOUT_HOTKEY_BASE + (id - ID_CLEAR_BASE);
                        clear_hotkey(hwnd, layout_control);
                    }
                    _ => {}
                }
            }
            LRESULT(0)
        }
        WM_KEYDOWN | WM_SYSKEYDOWN => {
            handle_hotkey_capture(hwnd, wparam, lparam, false);
            LRESULT(0)
        }
        WM_KEYUP | WM_SYSKEYUP => {
            let vk = wparam.0 as u16;
            if is_modifier_vk(vk) {
                handle_hotkey_capture(hwnd, wparam, lparam, true);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            unsafe { PostQuitMessage(0) };
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

fn is_modifier_vk(vk: u16) -> bool {
    matches!(
        VIRTUAL_KEY(vk),
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_LSHIFT | VK_RSHIFT | VK_LCONTROL | VK_RCONTROL
            | VK_LMENU | VK_RMENU
    )
}

fn start_hotkey_capture(hwnd: HWND, control_id: u16) {
    let mut state = STATE.lock().unwrap();
    if let Some(ref mut s) = *state {
        s.capturing_control = Some(control_id);
    }

    let button = unsafe { GetDlgItem(Some(hwnd), control_id as i32) }.unwrap();
    let text = w!("Press a key...");
    unsafe { SetWindowTextW(button, text).unwrap() };

    // Move focus to the parent window so WM_KEYDOWN reaches settings_proc
    unsafe { SetFocus(Some(hwnd)) };

    // Suspend the global hook so it doesn't intercept the key we're capturing
    hooks::set_suspended(true);
}

fn handle_hotkey_capture(hwnd: HWND, wparam: WPARAM, lparam: LPARAM, is_keyup: bool) {
    let mut state = STATE.lock().unwrap();
    let Some(ref mut s) = *state else { return };
    let Some(control_id) = s.capturing_control else {
        return;
    };

    let vk = wparam.0 as u16;
    // Bit 24 of lParam = extended key flag (right-side Ctrl, Alt, etc.)
    let is_extended = (lparam.0 >> 24) & 1 != 0;

    if is_keyup {
        if !is_modifier_vk(vk) {
            return;
        }
        let specific_vk = match VIRTUAL_KEY(vk) {
            VK_CONTROL => if is_extended { VK_RCONTROL.0 } else { VK_LCONTROL.0 },
            VK_SHIFT => {
                // Shift doesn't use extended flag — use scancode instead
                let scancode = ((lparam.0 >> 16) & 0xFF) as u32;
                let mapped = unsafe { MapVirtualKeyW(scancode, MAP_VIRTUAL_KEY_TYPE(3)) }; // MAPVK_VSC_TO_VK_EX
                if mapped == VK_RSHIFT.0 as u32 { VK_RSHIFT.0 } else { VK_LSHIFT.0 }
            }
            VK_MENU => if is_extended { VK_RMENU.0 } else { VK_LMENU.0 },
            _ => vk,
        };
        finish_capture(hwnd, s, control_id, specific_vk, Modifiers::NONE);
    } else {
        if is_modifier_vk(vk) {
            return;
        }
        let modifiers = Modifiers {
            ctrl: unsafe { GetKeyState(VK_CONTROL.0 as i32) } < 0,
            shift: unsafe { GetKeyState(VK_SHIFT.0 as i32) } < 0,
            alt: unsafe { GetKeyState(VK_MENU.0 as i32) } < 0,
        };
        // Windows quirk: Ctrl+Pause generates VK_CANCEL (0x03) instead of VK_PAUSE (0x13).
        // Normalize back to VK_PAUSE so the user sees what they actually pressed.
        let normalized_vk = if vk == 0x03 { 0x13 } else { vk };
        finish_capture(hwnd, s, control_id, normalized_vk, modifiers);
    }
}

fn finish_capture(hwnd: HWND, state: &mut SettingsState, control_id: u16, vk: u16, modifiers: Modifiers) {
    state.capturing_control = None;
    hooks::set_suspended(false);

    let key_name = config::vk_to_key_name(vk, modifiers);
    eprintln!("[settings] Captured hotkey: {} for control {}", key_name, control_id);

    // Store the new binding
    if control_id == ID_CONVERSION_HOTKEY {
        state.config.conversion.hotkey = key_name;
    } else if control_id == ID_WORD_HOTKEY {
        state.config.conversion.word_hotkey = key_name;
    } else if control_id >= ID_LAYOUT_HOTKEY_BASE {
        let idx = (control_id - ID_LAYOUT_HOTKEY_BASE) as usize;
        if let Some(&lang_id) = state.layout_ids.get(idx) {
            let key = format!("0x{:04x}", lang_id);
            state.config.layouts.insert(key, key_name);
        }
    }

    // Refresh all buttons to show/clear conflicts
    refresh_all_buttons(hwnd, state);
}

fn clear_hotkey(hwnd: HWND, control_id: u16) {
    let mut state = STATE.lock().unwrap();
    let Some(ref mut s) = *state else { return };

    if control_id == ID_CONVERSION_HOTKEY {
        s.config.conversion.hotkey.clear();
    } else if control_id == ID_WORD_HOTKEY {
        s.config.conversion.word_hotkey.clear();
    } else if control_id >= ID_LAYOUT_HOTKEY_BASE {
        let idx = (control_id - ID_LAYOUT_HOTKEY_BASE) as usize;
        if let Some(&lang_id) = s.layout_ids.get(idx) {
            let key = format!("0x{:04x}", lang_id);
            s.config.layouts.remove(&key);
        }
    }
    eprintln!("[settings] Cleared hotkey for control {}", control_id);

    // Refresh all buttons to update conflict status
    refresh_all_buttons(hwnd, s);
}

/// Refreshes all hotkey button labels, showing current values and any conflicts.
fn refresh_all_buttons(hwnd: HWND, state: &SettingsState) {
    // Collect all assignments: (control_id, key_name, display_name)
    let mut assignments: Vec<(u16, String, String)> = Vec::new();

    for (i, &lang_id) in state.layout_ids.iter().enumerate() {
        let layout_key = format!("0x{:04x}", lang_id);
        let hotkey_str = state.config.layouts.get(&layout_key).cloned().unwrap_or_default();
        let name = layouts::get_installed_layouts()
            .iter()
            .find(|l| l.lang_id == lang_id)
            .map(|l| l.name.clone())
            .unwrap_or_else(|| format!("0x{:04X}", lang_id));
        assignments.push((ID_LAYOUT_HOTKEY_BASE + i as u16, hotkey_str, name));
    }

    assignments.push((ID_CONVERSION_HOTKEY, state.config.conversion.hotkey.clone(), "Text Conversion".to_string()));
    assignments.push((ID_WORD_HOTKEY, state.config.conversion.word_hotkey.clone(), "Word Conversion".to_string()));

    // For each assignment, check if it conflicts with any other
    for i in 0..assignments.len() {
        let (control_id, ref key_name, _) = assignments[i];
        let button = unsafe { GetDlgItem(Some(hwnd), control_id as i32) };
        let Ok(button) = button else { continue };

        if key_name.is_empty() {
            let text = w!("(click to set)");
            unsafe { SetWindowTextW(button, text).unwrap() };
            continue;
        }

        // Find conflict
        let mut conflict_with: Option<&str> = None;
        for j in 0..assignments.len() {
            if i == j { continue; }
            if !assignments[j].1.is_empty() && assignments[j].1 == *key_name {
                conflict_with = Some(&assignments[j].2);
                break;
            }
        }

        let display = if let Some(cw) = conflict_with {
            format!("{} (conflict: {}!)", key_name, cw)
        } else {
            key_name.clone()
        };

        let text_wide: Vec<u16> = display.encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            SetWindowTextW(button, windows::core::PCWSTR(text_wide.as_ptr())).unwrap();
        }
    }

    // Enable/disable Save button based on conflicts
    if let Ok(save_btn) = unsafe { GetDlgItem(Some(hwnd), ID_SAVE as i32) } {
        let enable = !has_conflicts(state);
        unsafe { EnableWindow(save_btn, enable) };
    }
}

/// Returns true if there are any duplicate hotkey assignments.
fn has_conflicts(state: &SettingsState) -> bool {
    let mut seen: Vec<&str> = Vec::new();

    if !state.config.conversion.hotkey.is_empty() {
        seen.push(&state.config.conversion.hotkey);
    }

    if !state.config.conversion.word_hotkey.is_empty() {
        if seen.contains(&state.config.conversion.word_hotkey.as_str()) {
            return true;
        }
        seen.push(&state.config.conversion.word_hotkey);
    }

    for &lang_id in &state.layout_ids {
        let key = format!("0x{:04x}", lang_id);
        if let Some(hotkey_str) = state.config.layouts.get(&key) {
            if !hotkey_str.is_empty() {
                if seen.contains(&hotkey_str.as_str()) {
                    return true;
                }
                seen.push(hotkey_str);
            }
        }
    }

    false
}

/// Checks if the given hotkey is already assigned to another control.
/// Returns the name of the conflicting binding, or None.
fn find_conflict(state: &SettingsState, current_control: u16, key_name: &str) -> Option<String> {
    // Check conversion hotkey
    if current_control != ID_CONVERSION_HOTKEY && state.config.conversion.hotkey == key_name {
        return Some("Text Conversion".to_string());
    }

    // Check layout hotkeys
    for (i, &lang_id) in state.layout_ids.iter().enumerate() {
        let layout_control = ID_LAYOUT_HOTKEY_BASE + i as u16;
        if layout_control == current_control {
            continue;
        }
        let layout_key = format!("0x{:04x}", lang_id);
        if let Some(existing) = state.config.layouts.get(&layout_key) {
            if existing == key_name {
                let name = layouts::get_installed_layouts()
                    .iter()
                    .find(|l| l.lang_id == lang_id)
                    .map(|l| l.name.clone())
                    .unwrap_or_else(|| format!("0x{:04X}", lang_id));
                return Some(name);
            }
        }
    }

    None
}

fn save_settings(hwnd: HWND) {
    let mut state = STATE.lock().unwrap();
    let Some(ref mut s) = *state else { return };

    let autostart_hwnd = unsafe { GetDlgItem(Some(hwnd), ID_AUTOSTART as i32) }.unwrap();
    let checked = unsafe { SendMessageW(autostart_hwnd, BM_GETCHECK_MSG, None, None) };
    s.config.general.autostart = checked.0 == BST_CHECKED_VAL as isize;

    match s.config.save() {
        Ok(_) => {
            eprintln!("[settings] Config saved to {:?}", Config::path());
            let bindings = s.config.to_bindings();
            eprintln!("[settings] Applied {} hotkey bindings", bindings.len());
            crate::hotkeys::set_bindings(bindings);
        }
        Err(e) => {
            eprintln!("[settings] Failed to save config: {}", e);
        }
    }

    apply_autostart(s.config.general.autostart);
}

fn apply_autostart(enable: bool) {
    use windows::Win32::System::Registry::*;

    let exe_path = std::env::current_exe().unwrap_or_default();
    let exe_str = exe_path.to_string_lossy().to_string();

    unsafe {
        let mut key = HKEY::default();
        let result = RegOpenKeyExW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run"),
            Some(0),
            KEY_SET_VALUE,
            &mut key,
        );

        if result.is_err() {
            eprintln!("[autostart] Failed to open registry key");
            return;
        }

        if enable {
            let value_wide: Vec<u16> = exe_str.encode_utf16().chain(std::iter::once(0)).collect();
            let _ = RegSetValueExW(
                key,
                w!("LightSwitch"),
                Some(0),
                REG_SZ,
                Some(std::slice::from_raw_parts(
                    value_wide.as_ptr() as *const u8,
                    value_wide.len() * 2,
                )),
            );
            eprintln!("[autostart] Enabled for current user");
        } else {
            let _ = RegDeleteValueW(key, w!("LightSwitch"));
            eprintln!("[autostart] Disabled");
        }

        let _ = RegCloseKey(key);
    }
}
