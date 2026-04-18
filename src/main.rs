// Use "windows" subsystem in release to hide console, "console" in debug for logging.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod clipboard;
mod colors;
mod config;
mod conversion;
mod hooks;
mod hotkeys;
mod icon;
mod input;
mod layouts;
mod logger;
mod tables;
mod ui;

use config::Config;
use std::sync::Mutex;
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_MESSAGE, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CW_USEDEFAULT, CreateWindowExW, DefWindowProcW, DestroyWindow,
    DispatchMessageW, GetMessageW, HICON, KillTimer, MSG, PostQuitMessage, RegisterClassW,
    SetTimer, TranslateMessage, WM_COMMAND, WM_DESTROY, WM_TIMER, WM_USER, WNDCLASSW,
    WS_OVERLAPPEDWINDOW,
};
use windows::core::w;

const WM_TRAY_ICON: u32 = WM_USER + 1;
const IDM_SETTINGS: u16 = 1001;
const IDM_EXIT: u16 = 1002;
const IDM_ABOUT: u16 = 1003;
const TIMER_LAYOUT_POLL: usize = 1;
const TIMER_LAYOUT_POLL_MS: u32 = 500;

/// Exported so the settings UI can post it when the user picks a new color,
/// forcing an immediate tray-icon rebuild.
pub const WM_APP_REFRESH_ICON: u32 = WM_USER + 2;

static CURRENT_LAYOUT: Mutex<layouts::HklId> = Mutex::new(0);
static CURRENT_ICON: Mutex<isize> = Mutex::new(0);
static CURRENT_ICON_COLOR: Mutex<colors::Color> = Mutex::new(0);

fn main() {
    logger::init();
    log!("LightSwitch starting");

    let installed = layouts::get_installed_layouts();
    log!("Detected {} keyboard layout(s)", installed.len());
    for layout in &installed {
        log!("  - {} (HKL=0x{:08X})", layout.name, layout.hkl_id);
    }

    // Build conversion tables for all installed layouts.
    let installed_ids: Vec<layouts::HklId> = installed.iter().map(|l| l.hkl_id).collect();
    tables::rebuild(&installed_ids);

    // Load config and apply hotkey bindings + color overrides
    let config = Config::load();
    let bindings = config.to_bindings();
    if bindings.is_empty() {
        log!("No hotkey bindings configured — open Settings to set them up");
    } else {
        log!("Loaded {} hotkey binding(s)", bindings.len());
    }
    hotkeys::set_bindings(bindings);
    colors::set_overrides(config.to_color_overrides());

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

/// Returns the effective color for the given layout, consulting user overrides
/// first (via `colors::get_color`) then falling back to the default palette.
fn current_color_for(hkl: layouts::HklId) -> colors::Color {
    let installed = layouts::get_installed_layouts();
    let idx = installed.iter().position(|l| l.hkl_id == hkl).unwrap_or(0);
    colors::get_color(hkl, idx)
}

/// Polls the foreground window's keyboard layout and updates the tray icon if
/// the layout OR its configured color has changed. Also detects installed-layout
/// changes and rebuilds conversion tables.
fn poll_and_update_layout(hwnd: HWND) {
    // Detect installed-layout changes (e.g. user added/removed a layout).
    let installed_ids = layouts::get_layout_order();
    if tables::needs_rebuild(&installed_ids) {
        log!("Installed layouts changed — rebuilding tables");
        tables::rebuild(&installed_ids);
    }

    let hkl_id = layouts::get_current_layout();
    let color = current_color_for(hkl_id);

    let mut current_layout = CURRENT_LAYOUT.lock().unwrap();
    let mut current_color = CURRENT_ICON_COLOR.lock().unwrap();
    if *current_layout == hkl_id && *current_color == color {
        return;
    }
    *current_layout = hkl_id;
    *current_color = color;
    drop(current_layout);
    drop(current_color);

    let label = icon::hkl_to_label(hkl_id);
    let new_icon = icon::create_text_icon(&label, color);

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
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    };

    let mut current_icon = CURRENT_ICON.lock().unwrap();
    if *current_icon != 0 {
        icon::destroy_icon(HICON(*current_icon as *mut _));
    }
    *current_icon = new_icon.0 as isize;
}

fn add_tray_icon(hwnd: HWND) {
    // Create initial icon based on current layout + its configured color.
    let hkl_id = layouts::get_current_layout();
    let color = current_color_for(hkl_id);
    *CURRENT_LAYOUT.lock().unwrap() = hkl_id;
    *CURRENT_ICON_COLOR.lock().unwrap() = color;
    let label = icon::hkl_to_label(hkl_id);
    let hicon = icon::create_text_icon(&label, color);
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

    let _ = unsafe { Shell_NotifyIconW(NIM_ADD, &nid) };
}

fn remove_tray_icon(hwnd: HWND) {
    let nid = NOTIFYICONDATAW {
        cbSize: std::mem::size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    };
    let _ = unsafe { Shell_NotifyIconW(NIM_DELETE, &nid) };
}

fn show_tray_context_menu(hwnd: HWND) {
    use windows::Win32::UI::WindowsAndMessaging::{
        AppendMenuW, CreatePopupMenu, GetCursorPos, MF_SEPARATOR, MF_STRING, SetForegroundWindow,
        TPM_BOTTOMALIGN, TPM_LEFTALIGN, TrackPopupMenu,
    };

    unsafe {
        let menu = CreatePopupMenu().unwrap();
        AppendMenuW(menu, MF_STRING, IDM_SETTINGS as usize, w!("Settings...")).unwrap();
        AppendMenuW(menu, MF_STRING, IDM_ABOUT as usize, w!("About...")).unwrap();
        AppendMenuW(menu, MF_SEPARATOR, 0, None).unwrap();
        AppendMenuW(menu, MF_STRING, IDM_EXIT as usize, w!("Exit")).unwrap();

        let mut pt = Default::default();
        GetCursorPos(&mut pt).unwrap();

        let _ = SetForegroundWindow(hwnd);
        let _ = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_BOTTOMALIGN,
            pt.x,
            pt.y,
            Some(0),
            hwnd,
            None,
        );
    }
}

fn show_about_dialog(hwnd: HWND) {
    use windows::Win32::UI::Shell::ShellExecuteW;
    use windows::Win32::UI::WindowsAndMessaging::{
        IDYES, MB_ICONINFORMATION, MB_YESNO, MessageBoxW, SW_SHOWNORMAL,
    };

    const REPO_URL: &str = "https://github.com/zergius-eggstream/lightswitch";

    let text = format!(
        "LightSwitch v{}\n\n\
         Lightweight keyboard layout switcher with text conversion.\n\n\
         Built: {}\n\n\
         Licensed under MIT.\n\
         © 2026 zergius-eggstream and LightSwitch contributors.\n\n\
         {}\n\n\
         Open the project page in your browser?",
        env!("CARGO_PKG_VERSION"),
        env!("BUILD_TIMESTAMP"),
        REPO_URL,
    );
    let text_wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let title_wide: Vec<u16> = "About LightSwitch"
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();

    let result = unsafe {
        MessageBoxW(
            Some(hwnd),
            windows::core::PCWSTR(text_wide.as_ptr()),
            windows::core::PCWSTR(title_wide.as_ptr()),
            MB_YESNO | MB_ICONINFORMATION,
        )
    };

    if result == IDYES {
        let url_wide: Vec<u16> = REPO_URL.encode_utf16().chain(std::iter::once(0)).collect();
        let verb_wide: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
        unsafe {
            ShellExecuteW(
                Some(hwnd),
                windows::core::PCWSTR(verb_wide.as_ptr()),
                windows::core::PCWSTR(url_wide.as_ptr()),
                windows::core::PCWSTR::null(),
                windows::core::PCWSTR::null(),
                SW_SHOWNORMAL,
            );
        }
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
    std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| unsafe {
        wnd_proc_inner(hwnd, msg, wparam, lparam)
    }))
    .unwrap_or_else(|_| {
        eprintln!("[wnd_proc] Caught panic in handler — continuing");
        unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
    })
}

unsafe fn wnd_proc_inner(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_TIMER => {
            if wparam.0 == TIMER_LAYOUT_POLL {
                poll_and_update_layout(hwnd);
            }
            LRESULT(0)
        }
        WM_APP_REFRESH_ICON => {
            // Settings UI asked for an immediate tray-icon refresh (e.g. after
            // color change). Delegate to the polling routine, which will detect
            // the color change and rebuild.
            poll_and_update_layout(hwnd);
            LRESULT(0)
        }
        hooks::WM_APP_HOTKEY => {
            match wparam.0 {
                hooks::ACTION_SWITCH_LAYOUT => {
                    // LPARAM was packed from HklId (u64) via isize cast
                    let hkl_id = lparam.0 as u64;
                    layouts::switch_layout(hkl_id);
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
                IDM_ABOUT => {
                    show_about_dialog(hwnd);
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
