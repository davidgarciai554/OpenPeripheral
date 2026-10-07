//! Anti-aliased vector drawing with GDI+ (part of Windows, no extra
//! dependency): rounded surfaces, the 5-segment battery and the line icons
//! from `docs/design/DESIGN.md`. Coordinates are the design's units (CSS
//! px at 96 dpi); `Canvas` scales them to the window's DPI.

use super::theme::{Color, Palette, Theme};
use crate::devices::Kind;
use windows::Win32::Graphics::Gdi::HDC;
use windows::Win32::Graphics::GdiPlus::*;

/// Starts GDI+ while a window that draws with it exists.
pub struct GdiPlus(usize);

impl GdiPlus {
    pub fn start() -> Option<Self> {
        let input = GdiplusStartupInput {
            GdiplusVersion: 1,
            ..Default::default()
        };
        let mut token = 0usize;
        let status = unsafe { GdiplusStartup(&mut token, &input, std::ptr::null_mut()) };
        (status == Ok).then_some(GdiPlus(token))
    }
}

impl Drop for GdiPlus {
    fn drop(&mut self) {
        unsafe { GdiplusShutdown(self.0) }
    }
}

pub struct Canvas {
    g: *mut GpGraphics,
    scale: f32,
    ox: f32,
    oy: f32,
}

impl Canvas {
    /// `scale` = dpi / 96.
    pub fn new(hdc: HDC, scale: f32) -> Option<Self> {
        let mut g = std::ptr::null_mut();
        unsafe {
            if GdipCreateFromHDC(hdc, &mut g) != Ok {
                return None;
            }
            GdipSetSmoothingMode(g, SmoothingModeAntiAlias);
            GdipSetPixelOffsetMode(g, PixelOffsetModeHalf);
        }
        Some(Canvas {
            g,
            scale,
            ox: 0.0,
            oy: 0.0,
        })
    }

    /// Sets the origin (in physical px) for the following design-unit calls.
    pub fn origin(&mut self, x_px: f32, y_px: f32) {
        self.ox = x_px;
        self.oy = y_px;
    }

    fn x(&self, v: f32) -> f32 {
        self.ox + v * self.scale
    }
    fn y(&self, v: f32) -> f32 {
        self.oy + v * self.scale
    }
    fn s(&self, v: f32) -> f32 {
        v * self.scale
    }

    fn round_rect_path(&self, x: f32, y: f32, w: f32, h: f32, r: f32) -> *mut GpPath {
        let (x, y, w, h) = (self.x(x), self.y(y), self.s(w), self.s(h));
        let d = (self.s(r) * 2.0).min(w).min(h).max(0.01);
        let mut p = std::ptr::null_mut();
        unsafe {
            GdipCreatePath(FillModeAlternate, &mut p);
            GdipAddPathArc(p, x, y, d, d, 180.0, 90.0);
            GdipAddPathArc(p, x + w - d, y, d, d, 270.0, 90.0);
            GdipAddPathArc(p, x + w - d, y + h - d, d, d, 0.0, 90.0);
            GdipAddPathArc(p, x, y + h - d, d, d, 90.0, 90.0);
            GdipClosePathFigure(p);
        }
        p
    }

    fn with_pen(&self, color: Color, width: f32, dashed: bool, f: impl FnOnce(*mut GpPen)) {
        let mut pen = std::ptr::null_mut();
        unsafe {
            if GdipCreatePen1(color.argb(), self.s(width), UnitPixel, &mut pen) != Ok {
                return;
            }
            GdipSetPenLineCap197819(pen, LineCapRound, LineCapRound, DashCapRound);
            GdipSetPenLineJoin(pen, LineJoinRound);
            if dashed {
                GdipSetPenDashStyle(pen, DashStyleDash);
            }
            f(pen);
            GdipDeletePen(pen);
        }
    }

    fn with_brush(&self, color: Color, f: impl FnOnce(*mut GpBrush)) {
        let mut brush = std::ptr::null_mut();
        unsafe {
            if GdipCreateSolidFill(color.argb(), &mut brush) != Ok {
                return;
            }
            f(brush.cast());
            GdipDeleteBrush(brush.cast());
        }
    }

    pub fn fill_round_rect(&self, x: f32, y: f32, w: f32, h: f32, r: f32, color: Color) {
        let p = self.round_rect_path(x, y, w, h, r);
        self.with_brush(color, |b| unsafe {
            GdipFillPath(self.g, b, p);
        });
        unsafe { GdipDeletePath(p) };
    }

    /// Strokes inside the box (inset by half the width, like a CSS border).
    #[allow(clippy::too_many_arguments)]
    pub fn stroke_round_rect(
        &self,
        x: f32,
        y: f32,
        w: f32,
        h: f32,
        r: f32,
        width: f32,
        color: Color,
        dashed: bool,
    ) {
        let i = width / 2.0;
        let p = self.round_rect_path(x + i, y + i, w - width, h - width, (r - i).max(0.0));
        self.with_pen(color, width, dashed, |pen| unsafe {
            GdipDrawPath(self.g, pen, p);
        });
        unsafe { GdipDeletePath(p) };
    }

    fn line(&self, pen: *mut GpPen, x1: f32, y1: f32, x2: f32, y2: f32) {
        unsafe {
            GdipDrawLine(self.g, pen, self.x(x1), self.y(y1), self.x(x2), self.y(y2));
        }
    }

    fn circle(&self, pen: *mut GpPen, cx: f32, cy: f32, r: f32) {
        unsafe {
            GdipDrawEllipse(
                self.g,
                pen,
                self.x(cx - r),
                self.y(cy - r),
                self.s(2.0 * r),
                self.s(2.0 * r),
            );
        }
    }

    /// Strokes a path given as `(start, [(c1, c2, end)…, line_to…])`.
    fn stroke_path(&self, color: Color, width: f32, build: impl FnOnce(&Self, *mut GpPath)) {
        let mut p = std::ptr::null_mut();
        unsafe { GdipCreatePath(FillModeAlternate, &mut p) };
        build(self, p);
        self.with_pen(color, width, false, |pen| unsafe {
            GdipDrawPath(self.g, pen, p);
        });
        unsafe { GdipDeletePath(p) };
    }

    fn bez(&self, p: *mut GpPath, a: (f32, f32), b: (f32, f32), c: (f32, f32), d: (f32, f32)) {
        unsafe {
            GdipAddPathBezier(
                p,
                self.x(a.0),
                self.y(a.1),
                self.x(b.0),
                self.y(b.1),
                self.x(c.0),
                self.y(c.1),
                self.x(d.0),
                self.y(d.1),
            );
        }
    }

    fn seg(&self, p: *mut GpPath, a: (f32, f32), b: (f32, f32)) {
        unsafe {
            GdipAddPathLine(p, self.x(a.0), self.y(a.1), self.x(b.0), self.y(b.1));
        }
    }
}

impl Drop for Canvas {
    fn drop(&mut self) {
        unsafe {
            GdipDeleteGraphics(self.g);
        }
    }
}

/// Device type icon, 32×32 units at (x, y).
pub fn kind_icon(c: &mut Canvas, x: f32, y: f32, size: f32, kind: Kind, color: Color) {
    let (ox, oy, sc) = (c.ox, c.oy, c.scale);
    c.origin(c.x(x), c.y(y));
    c.scale = sc * size / 32.0;
    match kind {
        Kind::Mouse => {
            c.stroke_round_rect(
                9.0 - 0.8,
                4.0 - 0.8,
                14.0 + 1.6,
                24.0 + 1.6,
                7.8,
                1.6,
                color,
                false,
            );
            c.with_pen(color, 1.6, false, |pen| {
                c.line(pen, 16.0, 4.0, 16.0, 12.0);
                c.line(pen, 9.0, 12.0, 23.0, 12.0);
            });
        }
        Kind::Keyboard => {
            c.stroke_round_rect(
                3.0 - 0.8,
                9.0 - 0.8,
                26.0 + 1.6,
                14.0 + 1.6,
                3.3,
                1.6,
                color,
                false,
            );
            c.with_pen(color, 1.6, false, |pen| {
                for kx in [7.0, 11.0, 15.0, 19.0, 23.0] {
                    c.line(pen, kx, 13.0, kx + 2.0, 13.0);
                }
                c.line(pen, 7.0, 17.0, 9.0, 17.0);
                c.line(pen, 23.0, 17.0, 25.0, 17.0);
                c.line(pen, 11.0, 19.0, 21.0, 19.0);
            });
        }
        Kind::Gamepad => {
            c.stroke_path(color, 1.6, |c, p| {
                c.seg(p, (9.0, 10.0), (23.0, 10.0));
                c.bez(p, (23.0, 10.0), (26.0, 10.0), (27.5, 12.0), (28.0, 15.0));
                c.seg(p, (28.0, 15.0), (29.0, 21.0));
                c.bez(p, (29.0, 21.0), (29.4, 23.6), (26.6, 25.0), (24.8, 23.2));
                c.seg(p, (24.8, 23.2), (22.0, 20.0));
                c.seg(p, (22.0, 20.0), (10.0, 20.0));
                c.seg(p, (10.0, 20.0), (7.2, 23.2));
                c.bez(p, (7.2, 23.2), (5.4, 25.0), (2.6, 23.6), (3.0, 21.0));
                c.seg(p, (3.0, 21.0), (4.0, 15.0));
                c.bez(p, (4.0, 15.0), (4.5, 12.0), (6.0, 10.0), (9.0, 10.0));
                unsafe { GdipClosePathFigure(p) };
            });
            c.with_pen(color, 1.6, false, |pen| {
                c.line(pen, 9.0, 13.5, 9.0, 18.5);
                c.line(pen, 6.5, 16.0, 11.5, 16.0);
                for (bx, by) in [(23.0, 13.6), (25.4, 16.0), (23.0, 18.4), (20.6, 16.0)] {
                    c.circle(pen, bx, by, 1.1);
                }
            });
        }
    }
    c.origin(ox, oy);
    c.scale = sc;
}

/// 40×18 battery: outline, terminal and 5 segments, `bars` of them lit.
pub fn battery(c: &Canvas, x: f32, y: f32, bars: u8, stale: bool, p: &Palette) {
    let outline = if stale {
        p.battery_stale
    } else {
        p.battery_outline
    };
    c.stroke_round_rect(x, y, 36.0, 18.0, 3.75, 1.5, outline, stale);
    c.fill_round_rect(x + 36.0, y + 5.5, 3.0, 7.0, 1.0, outline);
    let lit = p.battery_fill(bars, stale);
    for i in 0..5u8 {
        let sx = x + 3.0 + 6.0 * i as f32;
        if i < bars {
            c.fill_round_rect(sx, y + 3.5, 5.0, 11.0, 1.0, lit);
        } else if p.theme == Theme::HighContrast {
            // High contrast: unlit segments are outlines, lit ones filled.
            c.stroke_round_rect(sx, y + 3.5, 5.0, 11.0, 1.0, 1.0, p.battery_outline, false);
        } else {
            c.fill_round_rect(sx, y + 3.5, 5.0, 11.0, 1.0, p.battery_empty);
        }
    }
}

/// 40×18 "wired" icon: cable entering the plug at mid height.
pub fn cable(c: &Canvas, x: f32, y: f32, color: Color) {
    c.stroke_path(color, 1.5, |c, p| {
        let t = |a: f32, b: f32| (x + a, y + b);
        c.bez(p, t(1.5, 9.0), t(4.5, 9.0), t(4.5, 4.0), t(7.5, 4.0));
        c.bez(p, t(7.5, 4.0), t(10.5, 4.0), t(10.5, 14.0), t(13.5, 14.0));
        c.bez(p, t(13.5, 14.0), t(16.5, 14.0), t(16.5, 9.0), t(20.5, 9.0));
    });
    c.stroke_round_rect(
        x + 20.5 - 0.75,
        y + 3.75 - 0.75,
        12.5,
        12.0,
        2.75,
        1.5,
        color,
        false,
    );
    c.with_pen(color, 1.5, false, |pen| {
        c.line(pen, x + 31.5, y + 6.5, x + 36.5, y + 6.5);
        c.line(pen, x + 31.5, y + 11.5, x + 36.5, y + 11.5);
    });
}

/// 18×18 notice icon: circle with "!" (or a triangle for errors).
pub fn notice_icon(c: &Canvas, x: f32, y: f32, error: bool, color: Color) {
    let k = 18.0 / 24.0;
    let t = |a: f32, b: f32| (x + a * k, y + b * k);
    c.with_pen(color, 1.6 * k, false, |pen| {
        if error {
            let (a, b, d) = (t(12.0, 3.0), t(21.5, 20.0), t(2.5, 20.0));
            c.line(pen, a.0, a.1, b.0, b.1);
            c.line(pen, b.0, b.1, d.0, d.1);
            c.line(pen, d.0, d.1, a.0, a.1);
            let (p, q) = (t(12.0, 10.0), t(12.0, 14.0));
            c.line(pen, p.0, p.1, q.0, q.1);
            let r = t(12.0, 17.0);
            c.line(pen, r.0, r.1, r.0, r.1 + 0.01);
        } else {
            let m = t(12.0, 12.0);
            c.circle(pen, m.0, m.1, 9.0 * k);
            let (p, q) = (t(12.0, 8.0), t(12.0, 13.0));
            c.line(pen, p.0, p.1, q.0, q.1);
            let r = t(12.0, 16.0);
            c.line(pen, r.0, r.1, r.0, r.1 + 0.01);
        }
    });
}

/// 16×16 check mark shown before success messages.
pub fn check(c: &Canvas, x: f32, y: f32, color: Color) {
    c.with_pen(color, 1.8, false, |pen| {
        c.line(pen, x + 3.0, y + 8.5, x + 6.0, y + 11.5);
        c.line(pen, x + 6.0, y + 11.5, x + 13.0, y + 4.5);
    });
}
