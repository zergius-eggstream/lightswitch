use crate::colors::{self, Color};
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::{
    CLEARTYPE_QUALITY, CLIP_DEFAULT_PRECIS, CreateCompatibleBitmap, CreateCompatibleDC,
    CreateFontW, CreateSolidBrush, DEFAULT_CHARSET, DEFAULT_PITCH, DT_CENTER, DT_SINGLELINE,
    DT_VCENTER, DeleteDC, DeleteObject, DrawTextW, FF_DONTCARE, FW_BOLD, FillRect, GetDC,
    OUT_DEFAULT_PRECIS, ReleaseDC, SelectObject, SetBkMode, SetTextColor, TRANSPARENT,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateIconIndirect, DestroyIcon, GetSystemMetrics, HICON, ICONINFO, SM_CXSMICON, SM_CYSMICON,
};
use windows::core::w;

/// Converts our `0x00RRGGBB` color to Win32 `COLORREF` (which is `0x00BBGGRR`).
fn color_to_colorref(color: Color) -> COLORREF {
    let r = (color >> 16) & 0xFF;
    let g = (color >> 8) & 0xFF;
    let b = color & 0xFF;
    COLORREF((b << 16) | (g << 8) | r)
}

/// Returns a short label for a layout using Windows locale data.
/// Returns the 3-letter native abbreviation (e.g. "УКР", "РУС", "ENG").
pub fn hkl_to_label(id: crate::layouts::HklId) -> String {
    crate::layouts::lang_id_to_abbrev(crate::layouts::hkl_lang_id(id))
}

/// Creates a tray icon with the given text label, drawn on a solid background
/// of the given color. Text color is chosen automatically (black or white)
/// based on background luminance.
/// The returned HICON must be freed via [`destroy_icon`].
pub fn create_text_icon(text: &str, bg_color: Color) -> HICON {
    let fg_color = colors::text_color_for(bg_color);

    unsafe {
        let cx = GetSystemMetrics(SM_CXSMICON);
        let cy = GetSystemMetrics(SM_CYSMICON);

        let screen_dc = GetDC(None);
        let mem_dc = CreateCompatibleDC(Some(screen_dc));

        let color_bmp = CreateCompatibleBitmap(screen_dc, cx, cy);
        let mask_bmp = CreateCompatibleBitmap(screen_dc, cx, cy);

        let old_bmp = SelectObject(mem_dc, color_bmp.into());

        let bg_brush = CreateSolidBrush(color_to_colorref(bg_color));
        let mut rect = RECT {
            left: 0,
            top: 0,
            right: cx,
            bottom: cy,
        };
        FillRect(mem_dc, &rect, bg_brush);
        let _ = DeleteObject(bg_brush.into());

        // Font sized to fit 3 characters into the icon width.
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
        SetTextColor(mem_dc, color_to_colorref(fg_color));

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
