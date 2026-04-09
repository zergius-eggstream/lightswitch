use windows::Win32::Foundation::{HANDLE, HGLOBAL};
use windows::Win32::System::DataExchange::{
    CloseClipboard, EmptyClipboard, GetClipboardData, OpenClipboard, SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::CF_UNICODETEXT;

pub fn get_text() -> Option<String> {
    unsafe {
        OpenClipboard(None).ok()?;
        let result = read_clipboard_text();
        let _ = CloseClipboard();
        result
    }
}

pub fn set_text(text: &str) -> bool {
    unsafe {
        if OpenClipboard(None).is_err() {
            return false;
        }
        let _ = EmptyClipboard();
        let result = write_clipboard_text(text);
        let _ = CloseClipboard();
        result
    }
}

pub fn with_clipboard_restore<F, R>(f: F) -> R
where
    F: FnOnce() -> R,
{
    let saved = get_text();
    let result = f();
    if let Some(text) = saved {
        set_text(&text);
    }
    result
}

unsafe fn read_clipboard_text() -> Option<String> {
    let handle = unsafe { GetClipboardData(CF_UNICODETEXT.0 as u32).ok()? };
    let hmem = HGLOBAL(handle.0);
    let ptr = unsafe { GlobalLock(hmem) } as *const u16;
    if ptr.is_null() {
        return None;
    }

    let size = unsafe { GlobalSize(hmem) };
    let len = size / 2;
    let slice = unsafe { std::slice::from_raw_parts(ptr, len) };

    let text_len = slice.iter().position(|&c| c == 0).unwrap_or(len);
    let text = String::from_utf16_lossy(&slice[..text_len]);

    let _ = unsafe { GlobalUnlock(hmem) };
    Some(text)
}

unsafe fn write_clipboard_text(text: &str) -> bool {
    let wide: Vec<u16> = text.encode_utf16().chain(std::iter::once(0)).collect();
    let byte_len = wide.len() * 2;

    let hmem = unsafe { GlobalAlloc(GMEM_MOVEABLE, byte_len) };
    let Ok(hmem) = hmem else { return false };

    let ptr = unsafe { GlobalLock(hmem) } as *mut u16;
    if ptr.is_null() {
        return false;
    }
    unsafe { std::ptr::copy_nonoverlapping(wide.as_ptr(), ptr, wide.len()) };
    let _ = unsafe { GlobalUnlock(hmem) };

    unsafe {
        SetClipboardData(CF_UNICODETEXT.0 as u32, Some(HANDLE(hmem.0))).is_ok()
    }
}
