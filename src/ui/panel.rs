//! Painted panels of the "Dispositivos" view: device cards, the conflict
//! notice, and the loading / empty / error states (`docs/design/DESIGN.md`).
//!
//! One `layout()` computes every element's position; it is used both to
//! measure a panel's height and to paint it, so the two cannot drift. The
//! only interactive element in a panel is a native BUTTON child, which
//! provides focus, keyboard and UI Automation; the panel itself exposes its
//! whole content as one accessible name (its window text).

use super::glyph::{self, Canvas};
use super::text::{self, Energy};
use super::theme::{Color, Palette};
use crate::app::{Shared, Snapshot, Status};
use crate::config::to_hex;
use crate::devices::{DeviceKey, Kind};
use crate::protocol::ChargeState;
use windows::Win32::Foundation::{COLORREF, RECT};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::UI::Controls::{
    DRAWITEMSTRUCT, ODS_DISABLED, ODS_FOCUS, ODS_NOFOCUSRECT, ODS_SELECTED,
};

/// Fonts for one DPI. Sizes and weights from DESIGN.md.
pub struct Fonts {
    pub title: HFONT,
    pub title_compact: HFONT,
    pub name: HFONT,
    pub percent: HFONT,
    pub strong: HFONT,
    pub body: HFONT,
    pub body_strong: HFONT,
    pub small: HFONT,
}

impl Fonts {
    pub fn new(dpi: u32) -> Self {
        let face = font_face();
        let f = |px: i32, weight: i32| unsafe {
            let mut lf = LOGFONTW {
                lfHeight: -(px * dpi as i32 / 96),
                lfWeight: weight,
                lfCharSet: DEFAULT_CHARSET,
                lfQuality: CLEARTYPE_QUALITY,
                ..Default::default()
            };
            lf.lfFaceName[..face.len()].copy_from_slice(&face);
            CreateFontIndirectW(&lf)
        };
        Fonts {
            title: f(26, 600),
            title_compact: f(22, 600),
            name: f(17, 600),
            percent: f(22, 600),
            strong: f(16, 600),
            body: f(14, 400),
            body_strong: f(14, 600),
            small: f(13, 400),
        }
    }
}

impl Drop for Fonts {
    fn drop(&mut self) {
        for f in [
            self.title,
            self.title_compact,
            self.name,
            self.percent,
            self.strong,
            self.body,
            self.body_strong,
            self.small,
        ] {
            unsafe {
                let _ = DeleteObject(f.into());
            }
        }
    }
}

/// "Segoe UI Variable Text" (Windows 11), else "Segoe UI" (Windows 10).
fn font_face() -> Vec<u16> {
    fn exists(name: &str) -> bool {
        let w: Vec<u16> = name.encode_utf16().chain([0]).collect();
        let mut lf = LOGFONTW {
            lfCharSet: DEFAULT_CHARSET,
            ..Default::default()
        };
        lf.lfFaceName[..w.len()].copy_from_slice(&w);
        extern "system" fn found(
            _: *const LOGFONTW,
            _: *const TEXTMETRICW,
            _: u32,
            l: windows::Win32::Foundation::LPARAM,
        ) -> i32 {
            unsafe { *(l.0 as *mut bool) = true };
            0
        }
        let mut hit = false;
        unsafe {
            let hdc = GetDC(None);
            EnumFontFamiliesExW(
                hdc,
                &lf,
                Some(found),
                windows::Win32::Foundation::LPARAM(&mut hit as *mut bool as isize),
                0,
            );
            ReleaseDC(None, hdc);
        }
        hit
    }
    let name = if exists("Segoe UI Variable Text") {
        "Segoe UI Variable Text"
    } else {
        "Segoe UI"
    };
    name.encode_utf16().chain([0]).collect()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PanelKind {
    Notice,
    Loading,
    Empty,
    Error,
    Device(DeviceKey),
}

#[derive(Clone, Copy)]
pub enum FontRole {
    Name,
    Percent,
    Strong,
    Body,
    Small,
}

enum Item {
    Text {
        rect: RECT,
        text: String,
        font: FontRole,
        color: Color,
        lines: i32,
    },
    KindIcon {
        x: f32,
        y: f32,
        size: f32,
        kind: Kind,
    },
    Battery {
        x: f32,
        y: f32,
        bars: u8,
        stale: bool,
    },
    Cable {
        x: f32,
        y: f32,
    },
    Swatch {
        x: f32,
        y: f32,
        color: Color,
    },
    Notice {
        x: f32,
        y: f32,
        error: bool,
    },
    Block {
        x: f32,
        y: f32,
        w: f32,
        h: f32,
    },
}

pub struct Layout {
    pub height: i32,
    /// Native button: control id and rectangle.
    pub button: Option<(i32, RECT)>,
    items: Vec<Item>,
    surface: Surface,
}

#[derive(Clone, Copy, PartialEq)]
enum Surface {
    Card,
    Disconnected,
    Notice,
    Dashed,
    Outline,
}

pub const ID_SETTINGS: i32 = 210;
pub const ID_EMPTY_REFRESH: i32 = 211;
pub const ID_RETRY: i32 = 212;

/// Text measurement and layout context.
pub struct Ctx<'a> {
    pub hdc: HDC,
    pub fonts: &'a Fonts,
    pub palette: &'a Palette,
    pub scale: f32,
    pub compact: bool,
}

impl Ctx<'_> {
    pub fn px(&self, units: f32) -> i32 {
        (units * self.scale).round() as i32
    }
    pub fn font(&self, role: FontRole) -> HFONT {
        match role {
            FontRole::Name => self.fonts.name,
            FontRole::Percent => self.fonts.percent,
            FontRole::Strong => self.fonts.strong,
            FontRole::Body => self.fonts.body,
            FontRole::Small => self.fonts.small,
        }
    }
    /// Size of `text` wrapped to `width` px, at most `lines` lines (0 = any).
    pub fn measure(&self, text: &str, role: FontRole, width: i32, lines: i32) -> (i32, i32) {
        let w: Vec<u16> = text.encode_utf16().collect();
        let mut r = RECT {
            left: 0,
            top: 0,
            right: width.max(1),
            bottom: 0,
        };
        unsafe {
            let old = SelectObject(self.hdc, self.font(role).into());
            let mut tm = TEXTMETRICW::default();
            let _ = GetTextMetricsW(self.hdc, &mut tm);
            let mut wcopy = w.clone();
            DrawTextW(
                self.hdc,
                &mut wcopy,
                &mut r,
                DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
            );
            SelectObject(self.hdc, old);
            let line = tm.tmHeight;
            let h = if lines > 0 {
                r.bottom.min(line * lines)
            } else {
                r.bottom
            };
            (r.right, h.max(line))
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn text_item(
    ctx: &Ctx,
    x: i32,
    y: i32,
    width: i32,
    text: String,
    font: FontRole,
    color: Color,
    lines: i32,
) -> (Item, i32, i32) {
    let (w, h) = ctx.measure(&text, font, width, lines);
    let rect = RECT {
        left: x,
        top: y,
        right: x + width,
        bottom: y + h,
    };
    (
        Item::Text {
            rect,
            text,
            font,
            color,
            lines,
        },
        w,
        h,
    )
}

/// Lays out one panel `width` px wide. `height` is the panel's actual
/// height when known (rows equalise card heights; the button stays at the
/// bottom), or 0 when measuring.
pub fn layout(ctx: &Ctx, kind: &PanelKind, shared: &Shared, width: i32, height: i32) -> Layout {
    let p = ctx.palette;
    match kind {
        PanelKind::Device(key) => match shared.devices.iter().find(|d| &d.info.key == key) {
            Some(snap) => device(ctx, snap, width, height),
            None => Layout {
                height: 0,
                button: None,
                items: Vec::new(),
                surface: Surface::Card,
            },
        },
        PanelKind::Notice => {
            let (pad_x, pad_y) = (ctx.px(14.0), ctx.px(10.0));
            let tx = pad_x + ctx.px(28.0);
            let (item, _, h) = text_item(
                ctx,
                tx,
                pad_y,
                width - tx - pad_x,
                text::conflicts(&shared.conflicts),
                FontRole::Body,
                p.text_notice,
                0,
            );
            let items = vec![
                Item::Notice {
                    x: pad_x as f32,
                    y: (pad_y + ctx.px(1.0)) as f32,
                    error: false,
                },
                item,
            ];
            Layout {
                height: h.max(ctx.px(18.0)) + 2 * pad_y,
                button: None,
                items,
                surface: Surface::Notice,
            }
        }
        PanelKind::Loading => {
            let pad = ctx.px(20.0);
            let s = |v: f32| v * ctx.scale;
            let inner = (width - 2 * pad) as f32;
            let mut items = vec![
                Item::Block {
                    x: pad as f32,
                    y: pad as f32,
                    w: s(32.0),
                    h: s(32.0),
                },
                Item::Block {
                    x: pad as f32,
                    y: pad as f32 + s(46.0),
                    w: inner * 0.8,
                    h: s(14.0),
                },
                Item::Block {
                    x: pad as f32,
                    y: pad as f32 + s(74.0),
                    w: inner * 0.55,
                    h: s(12.0),
                },
                Item::Block {
                    x: pad as f32,
                    y: pad as f32 + s(100.0),
                    w: s(40.0),
                    h: s(18.0),
                },
            ];
            let y = pad + ctx.px(140.0);
            let (item, _, h) = text_item(
                ctx,
                pad,
                y,
                width - 2 * pad,
                "Buscando dispositivos…".into(),
                FontRole::Body,
                p.text_secondary,
                0,
            );
            items.push(item);
            Layout {
                height: (y + h + pad).max(ctx.px(236.0)),
                button: None,
                items,
                surface: Surface::Card,
            }
        }
        PanelKind::Empty | PanelKind::Error => {
            let error = *kind == PanelKind::Error;
            let pad = ctx.px(20.0);
            let mut items = Vec::new();
            let mut tx = pad;
            if error {
                items.push(Item::Notice {
                    x: pad as f32,
                    y: (pad + ctx.px(2.0)) as f32,
                    error: true,
                });
                tx += ctx.px(26.0);
            }
            let title = if error {
                "Error: no se pudo acceder a los dispositivos"
            } else {
                "No hay dispositivos compatibles conectados"
            };
            let (item, _, h) = text_item(
                ctx,
                tx,
                pad,
                width - tx - pad,
                title.into(),
                FontRole::Strong,
                p.text_primary,
                0,
            );
            items.push(item);
            let mut y = pad + h + ctx.px(10.0);
            let body = if error {
                "Windows no permitió abrir la interfaz HID. Cierra otros programas que controlen los periféricos y vuelve a intentarlo."
            } else {
                "Conecta un ratón, teclado o mando compatible; la lista se actualiza sola. La lista de dispositivos compatibles está en el README del proyecto."
            };
            let (item, _, h) = text_item(
                ctx,
                pad,
                y,
                width - 2 * pad,
                body.into(),
                FontRole::Body,
                p.text_secondary,
                0,
            );
            items.push(item);
            y += h + ctx.px(20.0);
            let (id, label) = if error {
                (ID_RETRY, "Reintentar")
            } else {
                (ID_EMPTY_REFRESH, "Actualizar")
            };
            let bw = button_width(ctx, label);
            let bh = ctx.px(if ctx.compact { 44.0 } else { 36.0 });
            let button = RECT {
                left: pad,
                top: y,
                right: pad + bw,
                bottom: y + bh,
            };
            Layout {
                height: (y + bh + pad).max(ctx.px(200.0)),
                button: Some((id, button)),
                items,
                surface: if error {
                    Surface::Outline
                } else {
                    Surface::Dashed
                },
            }
        }
    }
}

pub fn button_width(ctx: &Ctx, label: &str) -> i32 {
    ctx.measure(label, FontRole::Body, 1000, 1).0 + ctx.px(32.0)
}

fn device(ctx: &Ctx, snap: &Snapshot, width: i32, height: i32) -> Layout {
    let p = ctx.palette;
    let disconnected = matches!(snap.status, Status::Disconnected);
    let pad = ctx.px(if ctx.compact { 16.0 } else { 20.0 });
    let gap = ctx.px(if ctx.compact { 12.0 } else { 16.0 });
    let inner = width - 2 * pad;
    let secondary = if disconnected {
        p.text_tertiary
    } else {
        p.text_secondary
    };
    let mut items = Vec::new();
    let mut y = pad;

    // Type, name and status.
    let status = if ctx.compact {
        format!(
            "{} · {}",
            text::kind(snap.info.kind),
            text::card_status(snap)
        )
    } else {
        text::card_status(snap)
    };
    let name_color = if disconnected {
        p.text_secondary
    } else {
        p.text_primary
    };
    if ctx.compact {
        let icon = ctx.px(28.0);
        items.push(Item::KindIcon {
            x: pad as f32,
            y: y as f32,
            size: 28.0,
            kind: snap.info.kind,
        });
        let tx = pad + icon + ctx.px(12.0);
        let (item, _, nh) = text_item(
            ctx,
            tx,
            y,
            width - tx - pad,
            snap.info.name.clone(),
            FontRole::Name,
            name_color,
            2,
        );
        items.push(item);
        let (item, _, sh) = text_item(
            ctx,
            tx,
            y + nh + ctx.px(2.0),
            width - tx - pad,
            status,
            FontRole::Body,
            secondary,
            0,
        );
        items.push(item);
        y = (y + icon).max(y + nh + ctx.px(2.0) + sh) + gap;
    } else {
        items.push(Item::KindIcon {
            x: pad as f32,
            y: y as f32,
            size: 32.0,
            kind: snap.info.kind,
        });
        let (kw, kh) = ctx.measure(text::kind(snap.info.kind), FontRole::Small, inner, 1);
        let ky = y + (ctx.px(32.0) - kh) / 2;
        let rect = RECT {
            left: pad + ctx.px(44.0),
            top: ky,
            right: pad + ctx.px(44.0) + kw + 2,
            bottom: ky + kh,
        };
        items.push(Item::Text {
            rect,
            text: text::kind(snap.info.kind).into(),
            font: FontRole::Small,
            color: secondary,
            lines: 1,
        });
        y += ctx.px(32.0) + gap;
        let (item, _, nh) = text_item(
            ctx,
            pad,
            y,
            inner,
            snap.info.name.clone(),
            FontRole::Name,
            name_color,
            2,
        );
        items.push(item);
        y += nh + ctx.px(4.0);
        let (item, _, sh) = text_item(ctx, pad, y, inner, status, FontRole::Body, secondary, 0);
        items.push(item);
        y += sh + gap;
    }

    // Energy: battery, cable or nothing.
    let row = ctx.px(28.0);
    let gx = pad as f32;
    let gy = (y + (row - ctx.px(18.0)) / 2) as f32;
    let after = |items: &mut Vec<Item>, x: &mut i32, text: String, font: FontRole, color: Color| {
        let (w, h) = ctx.measure(&text, font, width, 1);
        let ty = y + (row - h) / 2;
        items.push(Item::Text {
            rect: RECT {
                left: *x,
                top: ty,
                right: *x + w + 2,
                bottom: ty + h,
            },
            text,
            font,
            color,
            lines: 1,
        });
        *x += w + ctx.px(8.0);
    };
    match text::energy(snap) {
        Energy::Battery {
            percent,
            bars,
            charge,
            stale,
        } => {
            items.push(Item::Battery {
                x: gx,
                y: gy,
                bars,
                stale,
            });
            let mut x = pad + ctx.px(50.0);
            let pct_color = if stale {
                p.text_secondary
            } else {
                p.text_primary
            };
            after(
                &mut items,
                &mut x,
                format!("{percent} %"),
                FontRole::Percent,
                pct_color,
            );
            if stale {
                after(
                    &mut items,
                    &mut x,
                    "último valor conocido".into(),
                    FontRole::Body,
                    p.text_secondary,
                );
            } else {
                if text::is_low(percent) && charge == ChargeState::Discharging {
                    after(
                        &mut items,
                        &mut x,
                        "Batería baja".into(),
                        FontRole::Strong,
                        p.text_primary,
                    );
                }
                after(
                    &mut items,
                    &mut x,
                    text::charging(charge).into(),
                    FontRole::Body,
                    p.text_secondary,
                );
            }
            y += row + gap;
        }
        Energy::Wired => {
            items.push(Item::Cable { x: gx, y: gy });
            let mut x = pad + ctx.px(50.0);
            after(
                &mut items,
                &mut x,
                "Con cable".into(),
                FontRole::Strong,
                p.text_primary,
            );
            after(
                &mut items,
                &mut x,
                "sin batería".into(),
                FontRole::Body,
                p.text_secondary,
            );
            y += row + gap;
        }
        Energy::BatteryError => {
            let mut x = pad;
            after(
                &mut items,
                &mut x,
                "Error de batería".into(),
                FontRole::Strong,
                p.text_primary,
            );
            y += row + gap;
        }
        Energy::Unknown => {}
    }

    // Capabilities, applied colour, reconnection note. In compact mode the
    // button shares the last of these rows, so text stops short of it.
    let bh = ctx.px(if ctx.compact { 44.0 } else { 36.0 });
    let bw = button_width(ctx, "Ajustes");
    let text_w = if ctx.compact { inner - bw - gap } else { inner };
    // (y, height, width) of the rows the button may share in compact mode.
    let mut extra: Vec<(i32, i32, i32)> = Vec::new();
    if let Some(caps) = text::capabilities(snap) {
        let (item, w, h) = text_item(ctx, pad, y, text_w, caps, FontRole::Body, p.text_notice, 0);
        items.push(item);
        extra.push((y, h, w));
        y += h + gap;
    }
    if let Some(c) = snap.applied_color {
        let label = text::color_label(snap);
        let (lw, lh) = ctx.measure(label, FontRole::Body, inner, 1);
        items.push(Item::Text {
            rect: RECT {
                left: pad,
                top: y,
                right: pad + lw + 2,
                bottom: y + lh,
            },
            text: label.into(),
            font: FontRole::Body,
            color: p.text_notice,
            lines: 1,
        });
        let sx = pad + lw + ctx.px(8.0);
        let sw = ctx.px(14.0);
        items.push(Item::Swatch {
            x: sx as f32,
            y: (y + (lh - sw) / 2) as f32,
            color: Color((c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32),
        });
        let hx = sx + sw + ctx.px(8.0);
        let hex = to_hex(c);
        let (hw, _) = ctx.measure(&hex, FontRole::Body, inner, 1);
        items.push(Item::Text {
            rect: RECT {
                left: hx,
                top: y,
                right: hx + hw + 2,
                bottom: y + lh,
            },
            text: hex,
            font: FontRole::Body,
            color: p.text_secondary,
            lines: 1,
        });
        extra.push((y, lh, hx + hw - pad));
        y += lh + gap;
    }
    if disconnected && snap.saved {
        let (item, _, h) = text_item(
            ctx,
            pad,
            y,
            inner,
            "Los ajustes guardados se aplicarán al volver a conectarlo.".into(),
            FontRole::Body,
            p.text_tertiary,
            0,
        );
        items.push(item);
        y += h + gap;
    }

    // "Ajustes" button, bottom right.
    let by = if ctx.compact {
        // Shares the last row only when that row fits beside the button.
        match extra.last() {
            Some(&(ry, rh, rw)) if rw <= text_w => ry - ((bh - rh) / 2).max(0),
            _ => y,
        }
    } else {
        y.max(ctx.px(280.0).max(height) - pad - bh)
    };
    let button = RECT {
        left: width - pad - bw,
        top: by,
        right: width - pad,
        bottom: by + bh,
    };
    let height = (by + bh + pad).max(y);
    Layout {
        height,
        button: Some((ID_SETTINGS, button)),
        items,
        surface: if disconnected {
            Surface::Disconnected
        } else {
            Surface::Card
        },
    }
}

/// Paints a panel into `hdc` (a memory DC the size of the panel).
pub fn paint(ctx: &Ctx, layout: &Layout, width: i32, height: i32) {
    let p = ctx.palette;
    unsafe {
        fill(
            ctx.hdc,
            RECT {
                left: 0,
                top: 0,
                right: width,
                bottom: height,
            },
            p.bg_app,
        );
    }
    let units = |px: i32| px as f32 / ctx.scale;
    let (w, h) = (units(width), units(height));
    if let Some(mut c) = Canvas::new(ctx.hdc, ctx.scale) {
        match layout.surface {
            Surface::Card | Surface::Disconnected => {
                let fillc = if layout.surface == Surface::Card {
                    p.surface_card
                } else {
                    p.surface_card_disconnected
                };
                c.fill_round_rect(0.0, 0.0, w, h, 10.0, fillc);
                c.stroke_round_rect(0.0, 0.0, w, h, 10.0, 1.0, p.border_card, false);
            }
            Surface::Notice => {
                c.stroke_round_rect(0.0, 0.0, w, h, 8.0, 1.0, p.border_notice, false)
            }
            Surface::Dashed => {
                c.stroke_round_rect(0.0, 0.0, w, h, 10.0, 1.0, p.border_control, true)
            }
            Surface::Outline => {
                c.stroke_round_rect(0.0, 0.0, w, h, 10.0, 1.0, p.border_control, false)
            }
        }
        for item in &layout.items {
            match *item {
                Item::KindIcon { x, y, size, kind } => {
                    c.origin(x, y);
                    let color = if layout.surface == Surface::Disconnected {
                        p.text_tertiary
                    } else {
                        p.text_primary
                    };
                    glyph::kind_icon(&mut c, 0.0, 0.0, size, kind, color);
                }
                Item::Battery { x, y, bars, stale } => {
                    c.origin(x, y);
                    glyph::battery(&c, 0.0, 0.0, bars, stale, p);
                }
                Item::Cable { x, y } => {
                    c.origin(x, y);
                    glyph::cable(&c, 0.0, 0.0, p.battery_outline);
                }
                Item::Swatch { x, y, color } => {
                    c.origin(x, y);
                    c.fill_round_rect(0.0, 0.0, 14.0, 14.0, 3.0, color);
                    c.stroke_round_rect(0.0, 0.0, 14.0, 14.0, 3.0, 1.0, p.border_control, false);
                }
                Item::Notice { x, y, error } => {
                    c.origin(x, y);
                    glyph::notice_icon(&c, 0.0, 0.0, error, p.text_notice);
                }
                Item::Block { x, y, w, h } => {
                    c.origin(x, y);
                    c.fill_round_rect(0.0, 0.0, w / ctx.scale, h / ctx.scale, 3.0, p.battery_empty);
                }
                Item::Text { .. } => {}
            }
        }
    }
    unsafe {
        SetBkMode(ctx.hdc, TRANSPARENT);
        for item in &layout.items {
            if let Item::Text {
                rect,
                ref text,
                font,
                color,
                lines,
            } = *item
            {
                let old = SelectObject(ctx.hdc, ctx.font(font).into());
                SetTextColor(ctx.hdc, COLORREF(color.colorref()));
                let mut w: Vec<u16> = text.encode_utf16().collect();
                let mut r = rect;
                let mut flags = DT_WORDBREAK | DT_NOPREFIX;
                if lines > 0 {
                    flags |= DT_EDITCONTROL | DT_END_ELLIPSIS;
                }
                DrawTextW(ctx.hdc, &mut w, &mut r, flags);
                SelectObject(ctx.hdc, old);
            }
        }
    }
}

pub unsafe fn fill(hdc: HDC, r: RECT, color: Color) {
    unsafe {
        let b = CreateSolidBrush(COLORREF(color.colorref()));
        FillRect(hdc, &r, b);
        let _ = DeleteObject(b.into());
    }
}

/// Owner-drawn button: outline radius 6, text, 2 px focus ring.
/// Visual role of an owner-drawn button (DESIGN.md: emphasis by contrast
/// and weight, never by an accent colour).
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum ButtonStyle {
    Normal,
    /// Main action of a section: `text.primary` border, semibold label.
    Primary,
    /// Chosen quick value: `surface.selected` fill, `text.primary` border.
    Selected,
}

pub fn draw_button(
    dis: &DRAWITEMSTRUCT,
    label: &str,
    bg: Color,
    palette: &Palette,
    fonts: &Fonts,
    scale: f32,
    style: ButtonStyle,
) {
    let r = dis.rcItem;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    unsafe {
        let hdc = dis.hDC;
        let mem = CreateCompatibleDC(Some(hdc));
        let bmp = CreateCompatibleBitmap(hdc, w, h);
        let old_bmp = SelectObject(mem, bmp.into());
        fill(
            mem,
            RECT {
                left: 0,
                top: 0,
                right: w,
                bottom: h,
            },
            bg,
        );
        let has = |f: windows::Win32::UI::Controls::ODS_FLAGS| dis.itemState.0 & f.0 != 0;
        let disabled = has(ODS_DISABLED);
        let focused = has(ODS_FOCUS) && !has(ODS_NOFOCUSRECT);
        let pressed = has(ODS_SELECTED);
        if let Some(c) = Canvas::new(mem, scale) {
            let (uw, uh) = (w as f32 / scale, h as f32 / scale);
            // Leave room for the focus ring (2 px, 2 px offset) inside the control.
            let (x, y, bw, bh) = (4.0, 4.0, uw - 8.0, uh - 8.0);
            let face =
                palette
                    .button_face()
                    .unwrap_or(if pressed || style == ButtonStyle::Selected {
                        palette.surface_selected
                    } else {
                        bg
                    });
            c.fill_round_rect(x, y, bw, bh, 6.0, face);
            let border = if disabled {
                palette.border_card
            } else if style == ButtonStyle::Normal {
                palette.border_control
            } else {
                palette.text_primary
            };
            c.stroke_round_rect(x, y, bw, bh, 6.0, 1.0, border, false);
            if focused {
                c.stroke_round_rect(
                    x - 4.0,
                    y - 4.0,
                    bw + 8.0,
                    bh + 8.0,
                    9.0,
                    2.0,
                    palette.focus_ring,
                    false,
                );
            }
        }
        SetBkMode(mem, TRANSPARENT);
        let color = if disabled {
            palette.text_disabled
        } else {
            palette.text_primary
        };
        SetTextColor(mem, COLORREF(color.colorref()));
        let font = if style == ButtonStyle::Normal || disabled {
            fonts.body
        } else {
            fonts.body_strong
        };
        let old_font = SelectObject(mem, font.into());
        let mut t: Vec<u16> = label.encode_utf16().collect();
        let mut tr = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        DrawTextW(
            mem,
            &mut t,
            &mut tr,
            DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
        );
        SelectObject(mem, old_font);
        let _ = BitBlt(hdc, r.left, r.top, w, h, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, old_bmp);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
    }
}

/// Accessible name of a panel (its window text).
pub fn accessible_name(kind: &PanelKind, shared: &Shared) -> String {
    match kind {
        PanelKind::Device(key) => shared
            .devices
            .iter()
            .find(|d| &d.info.key == key)
            .map(text::card_summary)
            .unwrap_or_default(),
        PanelKind::Notice => text::conflicts(&shared.conflicts),
        PanelKind::Loading => "Buscando dispositivos…".into(),
        PanelKind::Empty => "No hay dispositivos compatibles conectados".into(),
        PanelKind::Error => "Error: no se pudo acceder a los dispositivos".into(),
    }
}

/// Accessible name of a panel's button ("Ajustes de G305 …").
pub fn button_name(kind: &PanelKind, shared: &Shared) -> String {
    match kind {
        PanelKind::Device(key) => {
            let snap = shared.devices.iter().find(|d| &d.info.key == key);
            match snap {
                Some(s) if matches!(s.status, Status::Disconnected) => {
                    format!("Ajustes de {} (no disponible: desconectado)", s.info.name)
                }
                Some(s) if s.info.kind == crate::devices::Kind::Mouse => {
                    format!("Ajustes de {}", s.info.name)
                }
                // Placeholder button for now: say so instead of looking broken.
                Some(s) => format!("Ajustes de {}, próximamente", s.info.name),
                None => "Ajustes".into(),
            }
        }
        PanelKind::Error => "Reintentar".into(),
        _ => "Actualizar".into(),
    }
}

pub fn button_enabled(kind: &PanelKind, shared: &Shared) -> bool {
    match kind {
        PanelKind::Device(key) => shared
            .devices
            .iter()
            .find(|d| &d.info.key == key)
            .is_some_and(|d| !matches!(d.status, Status::Disconnected)),
        _ => true,
    }
}

/// Which panels the view shows, in order (pure; tested).
pub fn panels_for(shared: &Shared) -> Vec<PanelKind> {
    let mut v = Vec::new();
    if !shared.conflicts.is_empty() {
        v.push(PanelKind::Notice);
    }
    if !shared.scanned {
        v.push(PanelKind::Loading);
    } else if !shared.hid_available {
        v.push(PanelKind::Error);
    } else if shared.devices.is_empty() {
        v.push(PanelKind::Empty);
    } else {
        v.extend(
            shared
                .devices
                .iter()
                .map(|d| PanelKind::Device(d.info.key.clone())),
        );
    }
    v
}

/// Whether a panel spans the full content width (not a grid cell).
pub fn full_width(kind: &PanelKind) -> bool {
    matches!(
        kind,
        PanelKind::Notice | PanelKind::Empty | PanelKind::Error
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::Snapshot;
    use crate::devices::{DeviceInfo, SUPPORTED, State};

    fn snap(name: &str) -> Snapshot {
        Snapshot {
            info: DeviceInfo::new(&SUPPORTED[0], name),
            status: Status::Ok(State::default()),
            last_battery: None,
            saved: false,
            applied_color: None,
            original_dpi: None,
            profile: crate::app::Profile::NotRead,
            backup: None,
        }
    }

    #[test]
    fn panel_selection() {
        let mut s = Shared::default();
        assert_eq!(panels_for(&s), vec![PanelKind::Loading]);
        s.scanned = true;
        assert_eq!(panels_for(&s), vec![PanelKind::Error]);
        s.hid_available = true;
        assert_eq!(panels_for(&s), vec![PanelKind::Empty]);
        s.devices.push(snap("G305"));
        s.conflicts.push("Logitech G HUB");
        let key = s.devices[0].info.key.clone();
        assert_eq!(
            panels_for(&s),
            vec![PanelKind::Notice, PanelKind::Device(key)]
        );
    }

    #[test]
    fn button_names_and_state() {
        let mut s = Shared::default();
        s.devices.push(snap("G305"));
        let k = PanelKind::Device(s.devices[0].info.key.clone());
        // Mice open their settings page.
        assert_eq!(button_name(&k, &s), "Ajustes de G305");
        assert!(button_enabled(&k, &s));
        // Other device types are still placeholders.
        let mut keyboard = snap("BlackWidow");
        keyboard.info = DeviceInfo::new(&SUPPORTED[1], "BlackWidow");
        let kb = PanelKind::Device(keyboard.info.key.clone());
        s.devices.push(keyboard);
        assert_eq!(button_name(&kb, &s), "Ajustes de BlackWidow, próximamente");
        s.devices[0].status = Status::Disconnected;
        assert_eq!(
            button_name(&k, &s),
            "Ajustes de G305 (no disponible: desconectado)"
        );
        assert!(!button_enabled(&k, &s));
        assert_eq!(button_name(&PanelKind::Error, &s), "Reintentar");
    }
}
