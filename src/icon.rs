use windows::core::w;
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    CreateCompatibleBitmap, CreateCompatibleDC, CreateFontW, CreateSolidBrush, DeleteDC,
    DeleteObject, DrawTextW, FillRect, GetDC, ReleaseDC, SelectObject, SetBkMode, SetTextColor,
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER,
    DT_SINGLELINE, DT_VCENTER, FF_DONTCARE, FW_BOLD, OUT_DEFAULT_PRECIS, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON, SM_CYSMICON,
};

/// Returns a short label for a layout language ID using Windows locale data.
/// Returns the 3-letter native abbreviation (e.g. "УКР", "РУС", "ENG").
pub fn lang_id_to_label(lang_id: u16) -> String {
    crate::layouts::lang_id_to_abbrev(lang_id)
}

/// Creates a tray icon with the given short text label.
/// Returns an HICON that the caller is responsible for destroying via `DestroyIcon`.
pub fn create_text_icon(text: &str) -> HICON {
    unsafe {
        let cx = GetSystemMetrics(SM_CXSMICON);
        let cy = GetSystemMetrics(SM_CYSMICON);

        let screen_dc = GetDC(None);
        let mem_dc = CreateCompatibleDC(Some(screen_dc));

        // Color bitmap for the icon
        let color_bmp = CreateCompatibleBitmap(screen_dc, cx, cy);
        let mask_bmp = CreateCompatibleBitmap(screen_dc, cx, cy);

        let old_bmp = SelectObject(mem_dc, color_bmp.into());

        // Background: dark blue
        let bg_brush = CreateSolidBrush(COLORREF(0x00802040)); // BBGGRR — dark purple/blue
        let mut rect = RECT { left: 0, top: 0, right: cx, bottom: cy };
        FillRect(mem_dc, &rect, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        // Font sized to fit 3 characters into the icon width.
        // Use ~60% of icon height for font height to leave room for 3 chars.
        let font_height = (cy * 60) / 100;
        let font = CreateFontW(
            font_height,
            0,
            0,
            0,
            FW_BOLD.0 as i32,
            0,
            0,
            0,
            DEFAULT_CHARSET,
            OUT_DEFAULT_PRECIS,
            CLIP_DEFAULT_PRECIS,
            CLEARTYPE_QUALITY,
            (DEFAULT_PITCH.0 | FF_DONTCARE.0) as u32,
            w!("Segoe UI"),
        );
        let old_font = SelectObject(mem_dc, font.into());

        SetBkMode(mem_dc, TRANSPARENT);
        SetTextColor(mem_dc, COLORREF(0x00FFFFFF)); // white

        let mut text_wide: Vec<u16> = text.encode_utf16().collect();
        DrawTextW(
            mem_dc,
            &mut text_wide,
            &mut rect,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE,
        );

        SelectObject(mem_dc, old_font);
        let _ = DeleteObject(font.into());

        // Mask bitmap (all zeros = fully opaque)
        SelectObject(mem_dc, mask_bmp.into());
        let black_brush = CreateSolidBrush(COLORREF(0x00000000));
        FillRect(mem_dc, &rect, black_brush);
        let _ = DeleteObject(black_brush.into());

        SelectObject(mem_dc, old_bmp);

        let icon_info = ICONINFO {
            fIcon: true.into(),
            xHotspot: 0,
            yHotspot: 0,
            hbmMask: mask_bmp,
            hbmColor: color_bmp,
        };

        let hicon = CreateIconIndirect(&icon_info).unwrap_or_default();

        let _ = DeleteObject(color_bmp.into());
        let _ = DeleteObject(mask_bmp.into());
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);

        hicon
    }
}

/// Destroys an icon previously created by `create_text_icon`.
pub fn destroy_icon(icon: HICON) {
    if !icon.is_invalid() {
        unsafe {
            let _ = DestroyIcon(icon);
        }
    }
}
