//! Tray icon drawn at runtime (no resource compiler needed): a neutral
//! battery outline, light with a dark border so it reads on both light and
//! dark taskbars. State is never encoded in the icon, only in text.

use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{CreateBitmap, DeleteObject};
use windows::Win32::UI::HiDpi::{GetDpiForWindow, GetSystemMetricsForDpi};
use windows::Win32::UI::WindowsAndMessaging::{CreateIconIndirect, HICON, ICONINFO, SM_CXSMICON};

/// BGRA pixels, row-major, `size` x `size`.
pub fn pixels(size: usize) -> Vec<u32> {
    const CLEAR: u32 = 0x0000_0000;
    const BORDER: u32 = 0xff20_2020;
    const FILL: u32 = 0xfff2_f2f2;
    let s = size as f32;
    let t = (s / 16.0).round().max(1.0) as usize; // border thickness
    let (top, bottom) = ((s * 0.25) as usize, (s * 0.78) as usize);
    let (left, right) = ((s * 0.06) as usize, (s * 0.82) as usize);
    let (nub_top, nub_bottom) = ((s * 0.40) as usize, (s * 0.63) as usize);
    let mut px = vec![CLEAR; size * size];
    for y in 0..size {
        for x in 0..size {
            let body = (left..=right).contains(&x) && (top..=bottom).contains(&y);
            let nub =
                (right + 1..=right + 2 * t).contains(&x) && (nub_top..=nub_bottom).contains(&y);
            px[y * size + x] = if body {
                let edge = x < left + t || x > right - t || y < top + t || y > bottom - t;
                if edge { BORDER } else { FILL }
            } else if nub {
                BORDER
            } else {
                CLEAR
            };
        }
    }
    px
}

thread_local! {
    /// Created once and reused (e.g. when Explorer restarts), never leaked.
    static ICON: std::cell::Cell<Option<HICON>> = const { std::cell::Cell::new(None) };
}

pub fn tray_icon(hwnd: HWND) -> HICON {
    if let Some(icon) = ICON.get() {
        return icon;
    }
    let icon = create(hwnd);
    ICON.set(Some(icon));
    icon
}

fn create(hwnd: HWND) -> HICON {
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let size = GetSystemMetricsForDpi(SM_CXSMICON, dpi).clamp(16, 64) as usize;
        let px = pixels(size);
        let color = CreateBitmap(size as i32, size as i32, 1, 32, Some(px.as_ptr().cast()));
        let mask_bits = vec![0u8; size * size.div_ceil(16) * 2];
        let mask = CreateBitmap(
            size as i32,
            size as i32,
            1,
            1,
            Some(mask_bits.as_ptr().cast()),
        );
        let info = ICONINFO {
            fIcon: true.into(),
            hbmMask: mask,
            hbmColor: color,
            ..Default::default()
        };
        let icon = CreateIconIndirect(&info).unwrap_or_default();
        let _ = DeleteObject(color.into());
        let _ = DeleteObject(mask.into());
        icon
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn icon_has_border_fill_and_transparency() {
        for size in [16, 24, 32] {
            let px = pixels(size);
            assert_eq!(px.len(), size * size);
            assert!(px.contains(&0xff20_2020));
            assert!(px.contains(&0xfff2_f2f2));
            assert_eq!(px[0], 0); // corner is transparent
        }
    }
}
