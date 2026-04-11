use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_RCONTROL,
    VK_RMENU, VK_RSHIFT, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, PostMessageW, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK,
    KBDLLHOOKSTRUCT, LLKHF_INJECTED, WH_KEYBOARD_LL, WM_KEYDOWN, WM_KEYUP, WM_SYSKEYDOWN,
    WM_SYSKEYUP,
};

use crate::hotkeys::{self, Modifiers};
use std::collections::HashSet;
use std::sync::Mutex;

static HOOK_HANDLE: Mutex<Option<isize>> = Mutex::new(None);
static MAIN_HWND: Mutex<Option<isize>> = Mutex::new(None);
static HOOK_SUSPENDED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// Set of VK codes for which we have suppressed a keydown.
/// We swallow auto-repeat keydowns and the matching keyup for these keys,
/// so the application sees neither the press nor any phantom events.
static SUPPRESSED_KEYS: Mutex<Option<HashSet<u16>>> = Mutex::new(None);

fn suppressed_keys() -> std::sync::MutexGuard<'static, Option<HashSet<u16>>> {
    let mut guard = SUPPRESSED_KEYS.lock().unwrap();
    if guard.is_none() {
        *guard = Some(HashSet::new());
    }
    guard
}

/// Suspends hook processing (e.g. while settings window captures a hotkey).
pub fn set_suspended(suspended: bool) {
    HOOK_SUSPENDED.store(suspended, std::sync::atomic::Ordering::Relaxed);
}

/// Tracks state for standalone modifier detection.
/// When a modifier key is pressed, we record its VK.
/// If any other key is pressed before the modifier is released, we clear it.
/// If the modifier is released cleanly, it's a standalone press.
static PENDING_MODIFIER: Mutex<Option<u16>> = Mutex::new(None);

pub const WM_APP_HOTKEY: u32 = 0x8001;
pub const ACTION_SWITCH_LAYOUT: usize = 0;
pub const ACTION_CONVERT_TEXT: usize = 1;
pub const ACTION_CONVERT_WORD: usize = 2;

pub fn set_main_hwnd(hwnd: HWND) {
    *MAIN_HWND.lock().unwrap() = Some(hwnd.0 as isize);
}

pub fn install_hook() -> windows::core::Result<()> {
    unsafe {
        let hook = SetWindowsHookExW(WH_KEYBOARD_LL, Some(keyboard_proc), None, 0)?;
        *HOOK_HANDLE.lock().unwrap() = Some(hook.0 as isize);
        Ok(())
    }
}

pub fn uninstall_hook() {
    let handle = HOOK_HANDLE.lock().unwrap().take();
    if let Some(h) = handle {
        unsafe {
            let _ = UnhookWindowsHookEx(HHOOK(h as *mut _));
        }
    }
}

fn get_modifiers() -> Modifiers {
    unsafe {
        Modifiers {
            ctrl: GetKeyState(VK_CONTROL.0 as i32) < 0,
            shift: GetKeyState(VK_SHIFT.0 as i32) < 0,
            alt: GetKeyState(VK_MENU.0 as i32) < 0,
        }
    }
}

fn is_modifier_key(vk: u16) -> bool {
    matches!(
        VIRTUAL_KEY(vk),
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_LSHIFT | VK_RSHIFT | VK_LCONTROL | VK_RCONTROL
            | VK_LMENU | VK_RMENU
    )
}

/// Posts an action to the main window.
fn post_action(action: &hotkeys::HotkeyAction) -> bool {
    let Some(hwnd_val) = *MAIN_HWND.lock().unwrap() else {
        return false;
    };
    let hwnd = HWND(hwnd_val as *mut _);
    let result = match action {
        hotkeys::HotkeyAction::SwitchLayout(lang_id) => unsafe {
            PostMessageW(
                Some(hwnd),
                WM_APP_HOTKEY,
                WPARAM(ACTION_SWITCH_LAYOUT),
                LPARAM(*lang_id as isize),
            )
        },
        hotkeys::HotkeyAction::ConvertText => unsafe {
            PostMessageW(
                Some(hwnd),
                WM_APP_HOTKEY,
                WPARAM(ACTION_CONVERT_TEXT),
                LPARAM(0),
            )
        },
        hotkeys::HotkeyAction::ConvertWord => unsafe {
            PostMessageW(
                Some(hwnd),
                WM_APP_HOTKEY,
                WPARAM(ACTION_CONVERT_WORD),
                LPARAM(0),
            )
        },
    };
    result.is_ok()
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        let kb_struct = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
        let vk = kb_struct.vkCode as u16;

        // Skip processing when hook is suspended (e.g. settings capturing a hotkey).
        if HOOK_SUSPENDED.load(std::sync::atomic::Ordering::Relaxed) {
            return unsafe { CallNextHookEx(None, code, wparam, lparam) };
        }

        // Skip injected input (our own SendInput calls) to avoid recursion.
        if (kb_struct.flags.0 & LLKHF_INJECTED.0) != 0 {
            return unsafe { CallNextHookEx(None, code, wparam, lparam) };
        }

        match msg {
            WM_KEYDOWN | WM_SYSKEYDOWN => {
                if is_modifier_key(vk) {
                    let mut pending = PENDING_MODIFIER.lock().unwrap();
                    if pending.is_some() {
                        // Another modifier pressed while one is pending — cancel standalone.
                        // This prevents conflict with Ctrl+Shift, Alt+Shift, etc.
                        *pending = None;
                    } else {
                        // Fresh modifier press. Check if it's a standalone hotkey candidate.
                        let standalone_bindings = hotkeys::get_standalone_modifier_bindings();
                        if standalone_bindings.iter().any(|b| b.hotkey.vk == vk) {
                            *pending = Some(vk);
                        }
                    }
                } else {
                    // A non-modifier key was pressed.
                    // Cancel any pending standalone modifier.
                    *PENDING_MODIFIER.lock().unwrap() = None;

                    // Windows quirk: Ctrl+Pause produces VK_CANCEL (0x03) instead of VK_PAUSE.
                    let normalized_vk = if vk == 0x03 { 0x13 } else { vk };

                    // If we already suppressed this key (auto-repeat), just swallow it.
                    {
                        let suppressed = suppressed_keys();
                        if let Some(set) = suppressed.as_ref() {
                            if set.contains(&normalized_vk) || set.contains(&vk) {
                                return LRESULT(1);
                            }
                        }
                    }

                    // Check regular hotkeys (key + modifiers).
                    let modifiers = get_modifiers();
                    if let Some(action) = hotkeys::match_hotkey(normalized_vk, modifiers) {
                        crate::logger::log(&format!("[hotkey] {:?}", action));
                        post_action(&action);
                        // Mark key as suppressed to swallow auto-repeats and matching keyup.
                        let mut suppressed = suppressed_keys();
                        if let Some(set) = suppressed.as_mut() {
                            set.insert(normalized_vk);
                            set.insert(vk); // also raw vk in case of normalization
                        }
                        return LRESULT(1); // Suppress
                    }
                }
            }
            WM_KEYUP | WM_SYSKEYUP => {
                // Normalize VK_CANCEL → VK_PAUSE for symmetry with keydown handling
                let normalized_vk = if vk == 0x03 { 0x13 } else { vk };

                // If we suppressed the keydown for this key, also suppress the keyup
                // and clear it from the suppressed set.
                {
                    let mut suppressed = suppressed_keys();
                    if let Some(set) = suppressed.as_mut() {
                        if set.remove(&normalized_vk) || set.remove(&vk) {
                            return LRESULT(1);
                        }
                    }
                }

                if is_modifier_key(vk) {
                    let pending = PENDING_MODIFIER.lock().unwrap().take();
                    if pending == Some(vk) {
                        // Modifier was pressed and released without any other key in between.
                        if let Some(action) = hotkeys::match_hotkey(vk, Modifiers::NONE) {
                            crate::logger::log(&format!("[hotkey] {:?} (standalone)", action));
                            post_action(&action);
                            // Don't suppress keyup — let it pass through.
                        }
                    }
                }
            }
            _ => {}
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
