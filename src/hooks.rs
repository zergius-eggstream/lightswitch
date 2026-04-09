use windows::Win32::Foundation::{LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    GetKeyState, VIRTUAL_KEY, VK_CONTROL, VK_LCONTROL, VK_LMENU, VK_LSHIFT, VK_MENU, VK_RCONTROL,
    VK_RMENU, VK_RSHIFT, VK_SHIFT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CallNextHookEx, SetWindowsHookExW, UnhookWindowsHookEx, HHOOK, KBDLLHOOKSTRUCT,
    WH_KEYBOARD_LL, WM_KEYDOWN, WM_SYSKEYDOWN,
};

use std::sync::Mutex;

static HOOK_HANDLE: Mutex<Option<isize>> = Mutex::new(None);

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

fn is_modifier_held() -> bool {
    unsafe {
        let ctrl = GetKeyState(VK_CONTROL.0 as i32) < 0;
        let shift = GetKeyState(VK_SHIFT.0 as i32) < 0;
        let alt = GetKeyState(VK_MENU.0 as i32) < 0;
        ctrl || shift || alt
    }
}

fn is_modifier_key(vk: u16) -> bool {
    matches!(
        VIRTUAL_KEY(vk),
        VK_SHIFT | VK_CONTROL | VK_MENU | VK_LSHIFT | VK_RSHIFT | VK_LCONTROL | VK_RCONTROL
            | VK_LMENU | VK_RMENU
    )
}

unsafe extern "system" fn keyboard_proc(code: i32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if code >= 0 {
        let msg = wparam.0 as u32;
        if msg == WM_KEYDOWN || msg == WM_SYSKEYDOWN {
            let kb_struct = unsafe { &*(lparam.0 as *const KBDLLHOOKSTRUCT) };
            let vk = kb_struct.vkCode as u16;

            if !is_modifier_key(vk) {
                #[cfg(debug_assertions)]
                {
                    let modifiers = is_modifier_held();
                    eprintln!("[hook] vk=0x{:04X} modifiers_held={}", vk, modifiers);
                }
            }
        }
    }

    unsafe { CallNextHookEx(None, code, wparam, lparam) }
}
