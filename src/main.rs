// Use "windows" subsystem in release to hide console, "console" in debug for logging.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod config;
mod conversion;
mod hooks;
mod hotkeys;
mod icon;
mod input;
mod layouts;
mod logger;
mod ui;

use config::Config;
use std::sync::Mutex;
use windows::core::w;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY,
    NOTIFYICONDATAW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DestroyWindow, DispatchMessageW, GetMessageW, KillTimer,
    PostQuitMessage, RegisterClassW, SetTimer, TranslateMessage, CS_HREDRAW, CS_VREDRAW,
    CW_USEDEFAULT, HICON, MSG, WM_COMMAND, WM_DESTROY, WM_TIMER, WM_USER, WNDCLASSW,
    WS_OVERLAPPEDWINDOW,
};

const WM_TRAY_ICON: u32 = WM_USER + 1;
const IDM_SETTINGS: u16 = 1001;
const IDM_EXIT: u16 = 1002;
const TIMER_LAYOUT_POLL: usize = 1;
const TIMER_LAYOUT_POLL_MS: u32 = 500;

static CURRENT_LAYOUT: Mutex<u16> = Mutex::new(0);
static CURRENT_ICON: Mutex<isize> = Mutex::new(0);

fn main() {
    logger::init();
    log!("LightSwitch starting");

    let installed = layouts::get_installed_layouts();
    log!("Detected {} keyboard layout(s)", installed.len());
    for layout in &installed {
        log!("  - {} (0x{:04X})", layout.name, layout.lang_id);
    }

    // Load config and apply hotkey bindings
    let config = Config::load();
    let bindings = config.to_bindings();
    if bindings.is_empty() {
        log!("No hotkey bindings configured — open Settings to set them up");
    } else {
        log!("Loaded {} hotkey binding(s)", bindings.len());
    }
    hotkeys::set_bindings(bindings);

    match hooks::install_hook() {
        Ok(_) => log!("Keyboard hook installed"),
        Err(e) => log!("Failed to install hook: {}", e),
    }

    unsafe {
        let hinstance = GetModuleHandleW(None).unwrap();

        let class_name = w!("LightSwitchClass");
        let wc = WNDCLASSW {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(wnd_proc),
            hInstance: hinstance.into(),
            lpszClassName: class_name,
            ..Default::default()
        };
        RegisterClassW(&wc);

        let hwnd = CreateWindowExW(
            Default::default(),
            class_name,
            w!("LightSwitch"),
            WS_OVERLAPPEDWINDOW,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            None,
            None,
            Some(hinstance.into()),
            None,
        )
        .unwrap();

        hooks::set_main_hwnd(hwnd);
        add_tray_icon(hwnd);

        // Start polling layout for tray icon updates
        SetTimer(Some(hwnd), TIMER_LAYOUT_POLL, TIMER_LAYOUT_POLL_MS, None);

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = KillTimer(Some(hwnd), TIMER_LAYOUT_POLL);
        remove_tray_icon(hwnd);
        let icon_handle = *CURRENT_ICON.lock().unwrap();
        if icon_handle != 0 {
            icon::destroy_icon(HICON(icon_handle as *mut _));
        }
        hooks::uninstall_hook();
    }
}

/// Polls the foreground window's keyboard layout and updates the tray icon if changed.
fn poll_and_update_layout(hwnd: HWND) {
    let lang_id = layouts::get_current_layout();
    let mut current = CURRENT_LAYOUT.lock().unwrap();
    if *current == lang_id {
        return;
    }
    *current = lang_id;
    drop(current);

    let label = icon::lang_id_to_label(lang_id);
    let new_icon = icon::create_text_icon(&label);

    // Replace tray icon
    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_TIP,
        hIcon: new_icon,
        ..Default::default()
    };
    let tip = format!("LightSwitch ({})", label);
    for (i, ch) in tip.encode_utf16().enumerate() {
        if i >= nid.szTip.len() - 1 {
            break;
        }
        nid.szTip[i] = ch;
    }
    unsafe { let _ = Shell_NotifyIconW(NIM_MODIFY, &nid); };

    // Destroy previous icon and store new one
    let mut current_icon = CURRENT_ICON.lock().unwrap();
    if *current_icon != 0 {
        icon::destroy_icon(HICON(*current_icon as *mut _));
    }
    *current_icon = new_icon.0 as isize;
}

fn add_tray_icon(hwnd: HWND) {
    // Create initial icon based on current layout
    let lang_id = layouts::get_current_layout();
    *CURRENT_LAYOUT.lock().unwrap() = lang_id;
    let label = icon::lang_id_to_label(lang_id);
    let hicon = icon::create_text_icon(&label);
    *CURRENT_ICON.lock().unwrap() = hicon.0 as isize;

    let mut nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP,
        uCallbackMessage: WM_TRAY_ICON,
        hIcon: hicon,
        ..Default::default()
    };

    let tip = "LightSwitch";
    for (i, ch) in tip.encode_utf16().enumerate() {
        if i >= nid.szTip.len() - 1 {
            break;
        }
        nid.szTip[i] = ch;
    }

    unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
}

fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    };
    unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

fn show_tray_context_menu(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, GetCursorPos, SetForegroundWindow, TrackPopupMenu,
        MF_STRING, TPM_BOTTOMALIGN, TPM_LEFTALIGN,
    };

    unsafe {
        let menu = CreatePopupMenu().unwrap();
        AppendMenuW(menu, MF_STRING, IDM_SETTINGS as usize, w!("Settings...")).unwrap();
        AppendMenuW(menu, MF_STRING, IDM_EXIT as usize, w!("Exit")).unwrap();

        let mut pt = Default::default();
        GetCursorPos(&mut pt).unwrap();

        SetForegroundWindow(hwnd);
        TrackPopupMenu(menu, TPM_LEFTALIGN | TPM_BOTTOMALIGN, pt.x, pt.y, Some(0), hwnd, None);
    }
}

unsafe extern "system" fn wnd_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    // Catch any panic inside the message handler so it doesn't abort the process.
    // wnd_proc is called from Windows code which cannot unwind through Rust panics.
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        wnd_proc_inner(hwnd, msg, wparam, lparam)
    }))
    .unwrap_or_else(|_| {
        eprintln!("[wnd_proc] Caught panic in handler — continuing");
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    })
}

unsafe fn wnd_proc_inner(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wparam.0 == TIMER_LAYOUT_POLL {
                poll_and_update_layout(hwnd);
            }
            LRESULT(0)
        }
        hooks::WM_APP_HOTKEY => {
            match wparam.0 {
                hooks::ACTION_SWITCH_LAYOUT => {
                    let lang_id = lparam.0 as u16;
                    layouts::switch_layout(lang_id);
                }
                hooks::ACTION_CONVERT_TEXT => {
                    conversion::perform_conversion();
                }
                hooks::ACTION_CONVERT_WORD => {
                    conversion::perform_word_conversion();
                }
                _ => {}
            }
            LRESULT(0)
        }
        WM_TRAY_ICON => {
            let event = (lparam.0 & 0xFFFF) as u32;
            use windows::Win32::UI::WindowsAndMessaging::{WM_LBUTTONDBLCLK, WM_RBUTTONUP};
            if event == WM_RBUTTONUP {
                show_tray_context_menu(hwnd);
            } else if event == WM_LBUTTONDBLCLK {
                ui::show_settings();
            }
            LRESULT(0)
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as u16;
            match id {
                IDM_SETTINGS => {
                    ui::show_settings();
                }
                IDM_EXIT => {
                    unsafe { DestroyWindow(hwnd).unwrap() };
                }
                _ => {}
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
