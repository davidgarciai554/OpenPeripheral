//! Semantic colour tokens from `docs/design/DESIGN.md`, per theme, plus
//! detection of the Windows app theme and high contrast mode.

use crate::config::ThemeChoice;
use std::sync::atomic::{AtomicU8, Ordering};
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Dwm::{DWMWA_USE_IMMERSIVE_DARK_MODE, DwmSetWindowAttribute};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, COLOR_BTNTEXT, COLOR_GRAYTEXT, COLOR_HIGHLIGHT, COLOR_WINDOW, COLOR_WINDOWTEXT,
    GetSysColor, SYS_COLOR_INDEX,
};
use windows::Win32::System::Registry::{HKEY_CURRENT_USER, RRF_RT_REG_DWORD, RegGetValueW};
use windows::Win32::UI::Accessibility::{HCF_HIGHCONTRASTON, HIGHCONTRASTW};
use windows::Win32::UI::WindowsAndMessaging::{
    SPI_GETHIGHCONTRAST, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
};
use windows::core::w;

/// sRGB colour as 0xRRGGBB.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Color(pub u32);

impl Color {
    pub fn r(self) -> u8 {
        (self.0 >> 16) as u8
    }
    pub fn g(self) -> u8 {
        (self.0 >> 8) as u8
    }
    pub fn b(self) -> u8 {
        self.0 as u8
    }
    /// GDI COLORREF (0x00BBGGRR).
    pub fn colorref(self) -> u32 {
        self.r() as u32 | (self.g() as u32) << 8 | (self.b() as u32) << 16
    }
    /// GDI+ ARGB, opaque.
    pub fn argb(self) -> u32 {
        0xff00_0000 | self.0
    }
    fn from_colorref(c: u32) -> Self {
        Color((c & 0xff) << 16 | (c & 0xff00) | (c >> 16) & 0xff)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Theme {
    Dark,
    Light,
    HighContrast,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub theme: Theme,
    pub bg_app: Color,
    pub surface_card: Color,
    pub surface_card_disconnected: Color,
    pub surface_selected: Color,
    pub border_card: Color,
    pub border_control: Color,
    pub border_notice: Color,
    pub text_primary: Color,
    pub text_secondary: Color,
    pub text_tertiary: Color,
    pub text_notice: Color,
    pub text_disabled: Color,
    pub focus_ring: Color,
    pub battery_outline: Color,
    pub battery_empty: Color,
    pub battery_high: Color,
    pub battery_mid: Color,
    pub battery_low: Color,
    pub battery_stale: Color,
}

pub const DARK: Palette = Palette {
    theme: Theme::Dark,
    bg_app: Color(0x0B0B0C),
    surface_card: Color(0x161618),
    surface_card_disconnected: Color(0x111113),
    surface_selected: Color(0x26262A),
    border_card: Color(0x26262A),
    border_control: Color(0x3A3A3F),
    border_notice: Color(0x2A2A2E),
    text_primary: Color(0xF2F2F3),
    text_secondary: Color(0xA1A1A6),
    text_tertiary: Color(0x8E8E94),
    text_notice: Color(0xC8C8CC),
    text_disabled: Color(0x6E6E74),
    focus_ring: Color(0xF2F2F3),
    battery_outline: Color(0xA1A1A6),
    battery_empty: Color(0x2C2C30),
    battery_high: Color(0x3FB950),
    battery_mid: Color(0xD29922),
    battery_low: Color(0xF85149),
    battery_stale: Color(0x6E6E74),
};

pub const LIGHT: Palette = Palette {
    theme: Theme::Light,
    bg_app: Color(0xF4F4F5),
    surface_card: Color(0xFFFFFF),
    surface_card_disconnected: Color(0xFAFAFB),
    surface_selected: Color(0xE1E1E5),
    border_card: Color(0xE1E1E5),
    border_control: Color(0xC9C9CE),
    border_notice: Color(0xDCDCE0),
    text_primary: Color(0x111113),
    text_secondary: Color(0x5C5C63),
    text_tertiary: Color(0x6B6B72),
    text_notice: Color(0x3A3A40),
    text_disabled: Color(0x8E8E94),
    focus_ring: Color(0x111113),
    battery_outline: Color(0x5C5C63),
    battery_empty: Color(0xE1E1E5),
    battery_high: Color(0x1A7F37),
    battery_mid: Color(0x9A6700),
    battery_low: Color(0xCF222E),
    battery_stale: Color(0x8E8E94),
};

impl Palette {
    /// High contrast uses only the user's system colours. The battery is
    /// told apart by filled vs. outlined segments (see `glyph`), not hue.
    pub fn high_contrast(sys: impl Fn(SYS_COLOR_INDEX) -> u32) -> Self {
        let c = |i| Color::from_colorref(sys(i));
        let (window, text, gray) = (c(COLOR_WINDOW), c(COLOR_WINDOWTEXT), c(COLOR_GRAYTEXT));
        Palette {
            theme: Theme::HighContrast,
            bg_app: window,
            surface_card: window,
            surface_card_disconnected: window,
            surface_selected: c(COLOR_HIGHLIGHT),
            border_card: text,
            border_control: c(COLOR_BTNTEXT),
            border_notice: text,
            text_primary: text,
            text_secondary: text,
            text_tertiary: gray,
            text_notice: text,
            text_disabled: gray,
            focus_ring: c(COLOR_HIGHLIGHT),
            battery_outline: text,
            battery_empty: window,
            battery_high: text,
            battery_mid: text,
            battery_low: text,
            battery_stale: gray,
        }
    }

    /// Button face in high contrast; elsewhere buttons are transparent.
    pub fn button_face(&self) -> Option<Color> {
        (self.theme == Theme::HighContrast)
            .then(|| Color::from_colorref(unsafe { GetSysColor(COLOR_BTNFACE) }))
    }

    /// Colour of the lit segments for a given number of lit segments.
    pub fn battery_fill(&self, bars: u8, stale: bool) -> Color {
        match (stale, bars) {
            (true, _) => self.battery_stale,
            (false, 0..=1) => self.battery_low,
            (false, 2) => self.battery_mid,
            _ => self.battery_high,
        }
    }
}

fn high_contrast_on() -> bool {
    let mut hc = HIGHCONTRASTW {
        cbSize: size_of::<HIGHCONTRASTW>() as u32,
        ..Default::default()
    };
    unsafe {
        SystemParametersInfoW(
            SPI_GETHIGHCONTRAST,
            hc.cbSize,
            Some((&mut hc as *mut HIGHCONTRASTW).cast()),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
        .is_ok()
            && hc.dwFlags.contains(HCF_HIGHCONTRASTON)
    }
}

/// Windows "app mode": 0 = dark, 1 = light; light when unknown.
fn apps_use_light_theme() -> bool {
    let mut value = 1u32;
    let mut size = size_of::<u32>() as u32;
    unsafe {
        let _ = RegGetValueW(
            HKEY_CURRENT_USER,
            w!("Software\\Microsoft\\Windows\\CurrentVersion\\Themes\\Personalize"),
            w!("AppsUseLightTheme"),
            RRF_RT_REG_DWORD,
            None,
            Some((&mut value as *mut u32).cast()),
            Some(&mut size),
        );
    }
    value != 0
}

/// «Tema» chosen in «Ajustes» (0 = Sistema, 1 = Oscuro, 2 = Claro).
static CHOICE: AtomicU8 = AtomicU8::new(0);

/// Records the «Tema» of «Ajustes»; true if it changed.
pub fn set_choice(choice: ThemeChoice) -> bool {
    let v = match choice {
        ThemeChoice::Sistema => 0,
        ThemeChoice::Oscuro => 1,
        ThemeChoice::Claro => 2,
    };
    CHOICE.swap(v, Ordering::Relaxed) != v
}

fn choice() -> ThemeChoice {
    match CHOICE.load(Ordering::Relaxed) {
        1 => ThemeChoice::Oscuro,
        2 => ThemeChoice::Claro,
        _ => ThemeChoice::Sistema,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Pick {
    HighContrast,
    Light,
    Dark,
}

/// High contrast always wins; then the choice; «Sistema» follows Windows.
fn pick(choice: ThemeChoice, high_contrast: bool, system_light: impl FnOnce() -> bool) -> Pick {
    match choice {
        _ if high_contrast => Pick::HighContrast,
        ThemeChoice::Oscuro => Pick::Dark,
        ThemeChoice::Claro => Pick::Light,
        ThemeChoice::Sistema if system_light() => Pick::Light,
        ThemeChoice::Sistema => Pick::Dark,
    }
}

/// Palette for the current settings.
pub fn current() -> Palette {
    match pick(choice(), high_contrast_on(), apps_use_light_theme) {
        Pick::HighContrast => Palette::high_contrast(|i| unsafe { GetSysColor(i) }),
        Pick::Light => LIGHT,
        Pick::Dark => DARK,
    }
}

/// Whether Windows is in light app mode («Sistema · ahora claro»).
pub fn system_is_light() -> bool {
    apps_use_light_theme()
}

/// Matches the title bar to the palette (documented DWM attribute).
pub fn apply_title_bar(hwnd: HWND, palette: &Palette) {
    let dark: i32 = (palette.theme == Theme::Dark) as i32;
    unsafe {
        let _ = DwmSetWindowAttribute(
            hwnd,
            DWMWA_USE_IMMERSIVE_DARK_MODE,
            (&dark as *const i32).cast(),
            size_of::<i32>() as u32,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn luminance(c: Color) -> f64 {
        let lin = |v: u8| {
            let v = v as f64 / 255.0;
            if v <= 0.03928 {
                v / 12.92
            } else {
                ((v + 0.055) / 1.055).powf(2.4)
            }
        };
        0.2126 * lin(c.r()) + 0.7152 * lin(c.g()) + 0.0722 * lin(c.b())
    }

    fn contrast(a: Color, b: Color) -> f64 {
        let (x, y) = (luminance(a), luminance(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    #[test]
    fn theme_choice_and_high_contrast() {
        use ThemeChoice::*;
        // High contrast always wins.
        for c in [Sistema, Oscuro, Claro] {
            assert_eq!(pick(c, true, || true), Pick::HighContrast);
        }
        // A forced theme ignores Windows (never even asks it).
        assert_eq!(pick(Oscuro, false, || unreachable!()), Pick::Dark);
        assert_eq!(pick(Claro, false, || unreachable!()), Pick::Light);
        // «Sistema» follows the app mode.
        assert_eq!(pick(Sistema, false, || true), Pick::Light);
        assert_eq!(pick(Sistema, false, || false), Pick::Dark);
    }

    #[test]
    fn text_and_graphics_meet_wcag() {
        for p in [DARK, LIGHT] {
            for (fg, bg, min, what) in [
                (
                    p.text_primary,
                    p.surface_card,
                    4.5,
                    "texto primario/tarjeta",
                ),
                (
                    p.text_secondary,
                    p.surface_card,
                    4.5,
                    "texto secundario/tarjeta",
                ),
                (p.text_secondary, p.bg_app, 4.5, "texto secundario/fondo"),
                (p.text_primary, p.bg_app, 4.5, "texto primario/fondo"),
                (p.text_tertiary, p.bg_app, 4.5, "texto terciario/fondo"),
                (
                    p.text_tertiary,
                    p.surface_card_disconnected,
                    4.5,
                    "desconectado",
                ),
                (p.text_notice, p.bg_app, 4.5, "aviso"),
                (p.battery_high, p.surface_card, 3.0, "pila verde"),
                (p.battery_mid, p.surface_card, 3.0, "pila ámbar"),
                (p.battery_low, p.surface_card, 3.0, "pila roja"),
                (p.battery_stale, p.surface_card, 3.0, "pila sin respuesta"),
                (p.battery_outline, p.surface_card, 3.0, "contorno de pila"),
                (p.focus_ring, p.surface_card, 3.0, "foco/tarjeta"),
                (p.focus_ring, p.bg_app, 3.0, "foco/fondo"),
            ] {
                let c = contrast(fg, bg);
                assert!(c >= min, "{:?} {what}: {c:.2}", p.theme);
            }
        }
    }

    #[test]
    fn battery_colour_by_level() {
        assert_eq!(DARK.battery_fill(1, false), DARK.battery_low);
        assert_eq!(DARK.battery_fill(2, false), DARK.battery_mid);
        assert_eq!(DARK.battery_fill(3, false), DARK.battery_high);
        assert_eq!(DARK.battery_fill(5, false), DARK.battery_high);
        assert_eq!(LIGHT.battery_fill(1, true), LIGHT.battery_stale);
    }

    #[test]
    fn high_contrast_uses_system_colours() {
        // Typical "High Contrast Black": white text on black, yellow highlight.
        let p = Palette::high_contrast(|i| match i {
            COLOR_WINDOW => 0x000000,
            COLOR_WINDOWTEXT | COLOR_BTNTEXT => 0xFFFFFF,
            COLOR_GRAYTEXT => 0x00FF3F,
            COLOR_HIGHLIGHT => 0xFFFF00, // COLORREF 0x00BBGGRR: cyan-ish
            _ => 0x808080,
        });
        assert_eq!(p.theme, Theme::HighContrast);
        assert_eq!(p.bg_app, Color(0x000000));
        assert_eq!(p.text_primary, Color(0xFFFFFF));
        assert_eq!(p.focus_ring, Color(0x00FFFF));
        assert_eq!(p.battery_high, p.text_primary);
        assert_eq!(p.battery_low, p.text_primary);
    }

    #[test]
    fn colorref_round_trip() {
        let c = Color(0x3FB950);
        assert_eq!(c.colorref(), 0x50B93F);
        assert_eq!(Color::from_colorref(c.colorref()), c);
        assert_eq!(c.argb(), 0xFF3FB950);
    }
}
