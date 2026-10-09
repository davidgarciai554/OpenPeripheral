//! The "Dispositivos" window (`docs/design/DESIGN.md`): header, conflict
//! notice and a responsive grid of painted panels with native buttons.
//! Created when opened and destroyed when closed; GDI+ and fonts live only
//! while it exists, so the idle cost of the app does not change.

use super::app_settings;
use super::glyph::GdiPlus;
use super::mouse_settings;
use super::panel::{self, Ctx, Fonts, ID_EMPTY_REFRESH, ID_RETRY, ID_SETTINGS, PanelKind};
use super::theme::{self, DARK, Palette};
use super::{send, text, wide, with_app};
use crate::app::{Command, Shared};
use std::rc::Rc;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Accessibility::NotifyWinEvent;
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, SetScrollInfo};
use windows::Win32::UI::HiDpi::{AdjustWindowRectExForDpi, GetDpiForWindow};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("OpenPeripheral.Devices");
const PANEL_CLASS: PCWSTR = w!("OpenPeripheral.Panel");
const ID_REFRESH: i32 = 201;
/// «Ajustes» in the header: the app-wide settings page.
const ID_APP_SETTINGS: i32 = 202;

/// Layout constants in design units (96 dpi).
const MAX_CONTENT: f32 = 1080.0;
const MIN_COLUMN: f32 = 300.0;
const GAP: f32 = 16.0;
const SECTION_GAP: f32 = 20.0;
const COMPACT_BELOW: f32 = 480.0;

pub(crate) struct State {
    hwnd: Option<HWND>,
    pub(super) palette: Palette,
    pub(super) fonts: Option<Rc<Fonts>>,
    gdiplus: Option<GdiPlus>,
    panels: Vec<(PanelKind, HWND)>,
    pub(super) shared: Shared,
    pub(super) scroll: i32,
    refresh: Option<HWND>,
    settings_button: Option<HWND>,
    /// The "Ajustes del ratón" page, when it replaces the device grid.
    pub(super) mouse: Option<mouse_settings::MouseState>,
    /// The «Ajustes» page, when it replaces the device grid.
    pub(super) settings: Option<app_settings::SettingsState>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            hwnd: None,
            palette: DARK,
            fonts: None,
            gdiplus: None,
            panels: Vec::new(),
            shared: Shared::default(),
            scroll: 0,
            refresh: None,
            settings_button: None,
            mouse: None,
            settings: None,
        }
    }
}

/// Number of columns and column width for `avail` px (pure; tested).
fn columns(avail: i32, gap: i32, min: i32) -> (usize, i32) {
    let cols = ((avail + gap) / (min + gap)).max(1) as usize;
    let w = (avail - gap * (cols as i32 - 1)) / cols as i32;
    (cols, w.max(1))
}

/// Grid placement: rows take the height of their tallest card (pure; tested).
fn place(top: i32, left: i32, cols: usize, col_w: i32, gap: i32, heights: &[i32]) -> Vec<RECT> {
    let mut out = Vec::with_capacity(heights.len());
    let mut y = top;
    for row in heights.chunks(cols) {
        let h = row.iter().copied().max().unwrap_or(0);
        for (i, _) in row.iter().enumerate() {
            let x = left + i as i32 * (col_w + gap);
            out.push(RECT {
                left: x,
                top: y,
                right: x + col_w,
                bottom: y + h,
            });
        }
        y += h + gap;
    }
    out
}

pub(crate) fn register(instance: HINSTANCE) {
    unsafe {
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        let view = WNDCLASSW {
            lpfnWndProc: Some(view_proc),
            hInstance: instance,
            lpszClassName: CLASS,
            hCursor: cursor,
            hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&view);
        let panel = WNDCLASSW {
            lpfnWndProc: Some(panel_proc),
            hInstance: instance,
            lpszClassName: PANEL_CLASS,
            hCursor: cursor,
            ..Default::default()
        };
        RegisterClassW(&panel);
    }
}

pub(super) fn instance() -> HINSTANCE {
    unsafe { GetModuleHandleW(None).unwrap_or_default().into() }
}

pub(super) fn scale_of(hwnd: HWND) -> f32 {
    unsafe { GetDpiForWindow(hwnd).max(96) as f32 / 96.0 }
}

/// Opens the window, or brings it to the front if already open.
pub(crate) fn open() {
    if let Some(Some(h)) = with_app(|a| a.view.hwnd) {
        unsafe {
            let _ = ShowWindow(h, SW_SHOWNORMAL);
            let _ = SetForegroundWindow(h);
        }
        return;
    }
    let style = WS_OVERLAPPEDWINDOW | WS_VSCROLL | WS_CLIPCHILDREN;
    let Ok(hwnd) = (unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT,
            CLASS,
            w!("OpenPeripheral — Dispositivos"),
            style,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            960,
            720,
            None,
            None,
            Some(instance()),
            None,
        )
    }) else {
        return;
    };
    let dpi = unsafe { GetDpiForWindow(hwnd).max(96) };
    let palette = theme::current();
    with_app(|a| {
        a.view.hwnd = Some(hwnd);
        a.view.palette = palette;
        a.view.fonts = Some(Rc::new(Fonts::new(dpi)));
        a.view.gdiplus = GdiPlus::start();
        a.view.scroll = 0;
    });
    theme::apply_title_bar(hwnd, &palette);
    theme::apply_scroll_bars(hwnd, &palette);
    unsafe {
        // Client area of the design (1440×900, so «Ajustes del ratón» shows
        // its three panels), clamped to the work area of the monitor.
        let s = dpi as i32;
        let mut r = RECT {
            left: 0,
            top: 0,
            right: 1440 * s / 96,
            bottom: 900 * s / 96,
        };
        let _ = AdjustWindowRectExForDpi(&mut r, style, false, WS_EX_CONTROLPARENT, dpi);
        let mut mi = MONITORINFO {
            cbSize: size_of::<MONITORINFO>() as u32,
            ..Default::default()
        };
        let (mut w, mut h) = (r.right - r.left, r.bottom - r.top);
        if GetMonitorInfoW(MonitorFromWindow(hwnd, MONITOR_DEFAULTTONEAREST), &mut mi).as_bool() {
            let work = mi.rcWork;
            w = w.min((work.right - work.left) * 9 / 10);
            h = h.min((work.bottom - work.top) * 9 / 10);
        }
        let _ = SetWindowPos(hwnd, None, 0, 0, w, h, SWP_NOMOVE | SWP_NOZORDER);
        let refresh = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Actualizar"),
            WINDOW_STYLE(
                WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | (BS_OWNERDRAW | BS_NOTIFY) as u32,
            ),
            0,
            0,
            0,
            0,
            Some(hwnd),
            Some(HMENU(ID_REFRESH as isize as *mut _)),
            Some(instance()),
            None,
        )
        .ok();
        let settings = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            w!("BUTTON"),
            w!("Ajustes de OpenPeripheral"),
            WINDOW_STYLE(
                WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | (BS_OWNERDRAW | BS_NOTIFY) as u32,
            ),
            0,
            0,
            0,
            0,
            Some(hwnd),
            Some(HMENU(ID_APP_SETTINGS as isize as *mut _)),
            Some(instance()),
            None,
        )
        .ok();
        with_app(|a| {
            a.view.refresh = refresh;
            a.view.settings_button = settings;
        });
    }
    refresh();
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        let _ = SetForegroundWindow(hwnd);
        if let Some(Some(b)) = with_app(|a| a.view.refresh) {
            let _ = SetFocus(Some(b));
        }
    }
}

pub(crate) fn hwnd() -> Option<HWND> {
    with_app(|a| a.view.hwnd).flatten()
}

/// For callers that already hold the app state.
pub(crate) fn hwnd_of(state: &State) -> Option<HWND> {
    state.hwnd
}

fn is_descendant(parent: HWND, mut h: HWND) -> bool {
    while !h.is_invalid() {
        if h == parent {
            return true;
        }
        h = unsafe { GetParent(h) }.unwrap_or_default();
    }
    false
}

fn button_id(kind: &PanelKind) -> Option<i32> {
    match kind {
        PanelKind::Device(_) => Some(ID_SETTINGS),
        PanelKind::Empty => Some(ID_EMPTY_REFRESH),
        PanelKind::Error => Some(ID_RETRY),
        PanelKind::Notice | PanelKind::Loading => None,
    }
}

fn create_panel(view: HWND, kind: &PanelKind) -> Option<HWND> {
    unsafe {
        let p = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            PANEL_CLASS,
            w!(""),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_CLIPCHILDREN.0),
            0,
            0,
            0,
            0,
            Some(view),
            None,
            Some(instance()),
            None,
        )
        .ok()?;
        if let Some(id) = button_id(kind) {
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                w!("BUTTON"),
                w!(""),
                WINDOW_STYLE(
                    WS_CHILD.0 | WS_VISIBLE.0 | WS_TABSTOP.0 | (BS_OWNERDRAW | BS_NOTIFY) as u32,
                ),
                0,
                0,
                0,
                0,
                Some(p),
                Some(HMENU(id as isize as *mut _)),
                Some(instance()),
                None,
            );
        }
        Some(p)
    }
}

pub(super) fn set_text_if_changed(h: HWND, text: &str) -> bool {
    let len = unsafe { GetWindowTextLengthW(h) }.max(0) as usize;
    let mut buf = vec![0u16; len + 1];
    let n = unsafe { GetWindowTextW(h, &mut buf) }.max(0) as usize;
    if String::from_utf16_lossy(&buf[..n]) == text {
        return false;
    }
    let w = wide(text);
    unsafe {
        let _ = SetWindowTextW(h, PCWSTR(w.as_ptr()));
    }
    true
}

/// Re-reads the shared state. Panels are recreated only when the set of
/// panels changes; otherwise their texts are updated and they repaint.
pub(crate) fn refresh() {
    let Some(view) = hwnd() else { return };
    let Some(shared) = super::snapshot() else {
        return;
    };
    // The theme of «Ajustes» (or of Windows) may have changed.
    let palette = theme::current();
    let rethemed = with_app(|a| a.view.palette) != Some(palette);
    if rethemed {
        with_app(|a| a.view.palette = palette);
        theme::apply_title_bar(view, &palette);
        theme::apply_scroll_bars(view, &palette);
    }
    let redraw_all = || unsafe {
        let _ = RedrawWindow(
            Some(view),
            None,
            None,
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_ERASE,
        );
    };
    if with_app(|a| a.view.mouse.is_some()) == Some(true) {
        with_app(|a| a.view.shared = shared.clone());
        mouse_settings::refresh(view);
        if rethemed {
            redraw_all();
        }
        return;
    }
    if with_app(|a| a.view.settings.is_some()) == Some(true) {
        with_app(|a| a.view.shared = shared.clone());
        app_settings::refresh(view);
        return;
    }
    let kinds = panel::panels_for(&shared);
    let old: Vec<(PanelKind, HWND)> = with_app(|a| a.view.panels.clone()).unwrap_or_default();
    let same = old.len() == kinds.len() && old.iter().zip(&kinds).all(|((k, _), n)| k == n);
    let panels = if same {
        old
    } else {
        // Keep keyboard focus: if it was inside a panel being removed, move
        // it to "Actualizar" instead of losing it.
        let focus = unsafe { GetFocus() };
        let lost = old.iter().any(|(_, h)| is_descendant(*h, focus));
        for (_, h) in &old {
            unsafe {
                let _ = DestroyWindow(*h);
            }
        }
        let new: Vec<(PanelKind, HWND)> = kinds
            .iter()
            .filter_map(|k| create_panel(view, k).map(|h| (k.clone(), h)))
            .collect();
        if lost {
            if let Some(Some(b)) = with_app(|a| a.view.refresh) {
                unsafe {
                    let _ = SetFocus(Some(b));
                }
            }
        }
        new
    };
    with_app(|a| {
        a.view.panels = panels.clone();
        a.view.shared = shared.clone();
    });
    for (kind, h) in &panels {
        if set_text_if_changed(*h, &panel::accessible_name(kind, &shared)) {
            // Keep screen readers' view of the card current.
            unsafe {
                NotifyWinEvent(
                    EVENT_OBJECT_NAMECHANGE,
                    *h,
                    OBJID_WINDOW.0,
                    CHILDID_SELF as i32,
                )
            };
        }
        if let Some(id) = button_id(kind) {
            if let Ok(b) = unsafe { GetDlgItem(Some(*h), id) } {
                set_text_if_changed(b, &panel::button_name(kind, &shared));
                unsafe {
                    let _ = EnableWindow(b, panel::button_enabled(kind, &shared));
                }
            }
        }
    }
    let title = format!(
        "OpenPeripheral — Dispositivos ({})",
        text::connected_count(&shared)
    );
    set_text_if_changed(view, &title);
    relayout(view);
    unsafe {
        let _ = RedrawWindow(
            Some(view),
            None,
            None,
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_ERASE,
        );
    }
}

pub(super) struct Geometry {
    pub scale: f32,
    pub compact: bool,
    pub top: i32,
    pub left: i32,
    pub avail: i32,
}

fn geometry(view: HWND) -> Geometry {
    let scale = scale_of(view);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(view, &mut rc);
    }
    let width = rc.right;
    let compact = (width as f32 / scale) < COMPACT_BELOW;
    let margin = ((if compact { 16.0 } else { 32.0 }) * scale) as i32;
    let avail = (width - 2 * margin)
        .min((MAX_CONTENT * scale) as i32)
        .max(1);
    Geometry {
        scale,
        compact,
        top: ((if compact { 20.0 } else { 28.0 }) * scale) as i32,
        left: (width - avail) / 2,
        avail,
    }
}

/// Header block height (title + counter) in px.
fn header_height(g: &Geometry) -> i32 {
    (((if g.compact { 30.0 } else { 35.0 }) + 2.0 + 20.0) * g.scale) as i32
}

/// Positions the header button and every panel for the current size,
/// scroll position and DPI, and updates the scroll bar.
pub(super) fn relayout(view: HWND) {
    if with_app(|a| a.view.mouse.is_some()) == Some(true) {
        relayout_mouse(view);
        return;
    }
    if with_app(|a| a.view.settings.is_some()) == Some(true) {
        relayout_settings(view);
        return;
    }
    let Some((panels, shared, palette, fonts, scroll)) = with_app(|a| {
        (
            a.view.panels.clone(),
            a.view.shared.clone(),
            a.view.palette,
            a.view.fonts.clone(),
            a.view.scroll,
        )
    }) else {
        return;
    };
    let Some(fonts) = fonts else { return };
    let g = geometry(view);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(view, &mut rc);
    }
    let hdc = unsafe { GetDC(Some(view)) };
    let ctx = Ctx {
        hdc,
        fonts: &fonts,
        palette: &palette,
        scale: g.scale,
        compact: g.compact,
    };
    let gap = (GAP * g.scale) as i32;
    let section = (SECTION_GAP * g.scale) as i32;
    let mut y = g.top + header_height(&g) + section;
    let mut rects: Vec<RECT> = vec![RECT::default(); panels.len()];
    // Full-width panels first, then the device grid.
    let grid: Vec<usize> = (0..panels.len())
        .filter(|&i| !panel::full_width(&panels[i].0))
        .collect();
    for (i, (kind, _)) in panels.iter().enumerate() {
        if panel::full_width(kind) {
            let h = panel::layout(&ctx, kind, &shared, g.avail, 0).height;
            rects[i] = RECT {
                left: g.left,
                top: y,
                right: g.left + g.avail,
                bottom: y + h,
            };
            y += h + section;
        }
    }
    if !grid.is_empty() {
        let (cols, col_w) = if g.compact {
            (1, g.avail)
        } else {
            columns(g.avail, gap, (MIN_COLUMN * g.scale) as i32)
        };
        let heights: Vec<i32> = grid
            .iter()
            .map(|&i| panel::layout(&ctx, &panels[i].0, &shared, col_w, 0).height)
            .collect();
        for (slot, r) in grid
            .iter()
            .zip(place(y, g.left, cols, col_w, gap, &heights))
        {
            rects[*slot] = r;
            y = y.max(r.bottom + gap);
        }
    }
    let refresh_w = panel::button_width(&ctx, "Actualizar") + (8.0 * g.scale) as i32;
    let settings_w = panel::button_width(&ctx, "Ajustes") + (8.0 * g.scale) as i32;
    unsafe {
        ReleaseDC(Some(view), hdc);
    }
    let content = y + (40.0 * g.scale) as i32;
    let page = rc.bottom;
    let scroll = scroll.clamp(0, (content - page).max(0));
    with_app(|a| a.view.scroll = scroll);
    unsafe {
        let si = SCROLLINFO {
            cbSize: size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: content - 1,
            nPage: page.max(0) as u32,
            nPos: scroll,
            ..Default::default()
        };
        SetScrollInfo(view, SB_VERT, &si, true);
        // Showing or hiding the scroll bar resizes the client area and runs a
        // nested relayout with the new width; this one is then stale.
        let mut now = RECT::default();
        let _ = GetClientRect(view, &mut now);
        if now.right != rc.right {
            return;
        }
        // «Actualizar» and then «Ajustes», at the right of the title.
        let bh = ((if g.compact { 44.0 } else { 36.0 } + 8.0) * g.scale) as i32;
        let by = g.top + (header_height(&g) - bh) / 2 - scroll;
        let right = g.left + g.avail + (4.0 * g.scale) as i32;
        let sx = right - settings_w;
        if let Some(Some(b)) = with_app(|a| a.view.settings_button) {
            let _ = SetWindowPos(
                b,
                None,
                sx,
                by,
                settings_w,
                bh,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        if let Some(Some(b)) = with_app(|a| a.view.refresh) {
            let bx = sx - refresh_w;
            let _ = SetWindowPos(
                b,
                None,
                bx,
                by,
                refresh_w,
                bh,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
        for ((kind, h), r) in panels.iter().zip(&rects) {
            let (w, ht) = (r.right - r.left, r.bottom - r.top);
            let _ = SetWindowPos(
                *h,
                None,
                r.left,
                r.top - scroll,
                w,
                ht,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            // The button rectangle comes from the panel's own layout.
            if let Some(id) = button_id(kind) {
                if let Ok(b) = GetDlgItem(Some(*h), id) {
                    let hdc = GetDC(Some(*h));
                    let ctx = Ctx {
                        hdc,
                        fonts: &fonts,
                        palette: &palette,
                        scale: g.scale,
                        compact: g.compact,
                    };
                    let button = panel::layout(&ctx, kind, &shared, w, ht).button;
                    // Cards without settings have no button at all.
                    let _ = ShowWindow(b, if button.is_some() { SW_SHOWNA } else { SW_HIDE });
                    if let Some((_, br)) = button {
                        // Grow by the focus ring area drawn inside the control.
                        let o = (4.0 * g.scale) as i32;
                        let _ = SetWindowPos(
                            b,
                            None,
                            br.left - o,
                            br.top - o,
                            br.right - br.left + 2 * o,
                            br.bottom - br.top + 2 * o,
                            SWP_NOZORDER | SWP_NOACTIVATE,
                        );
                    }
                    ReleaseDC(Some(*h), hdc);
                }
            }
        }
    }
}

/// The mouse page's scrolling content window and fixed bar.
fn mouse_windows() -> Option<(HWND, HWND, i32)> {
    with_app(|a| a.view.mouse.as_ref().map(|m| (m.content, m.bar, m.bar_h))).flatten()
}

/// The «Ajustes» page: its controls are children of the view.
fn relayout_settings(view: HWND) {
    let g = geometry(view);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(view, &mut rc);
    }
    let scroll = with_app(|a| a.view.scroll).unwrap_or(0);
    let height = app_settings::layout(view, &g, scroll);
    let clamped = scroll.clamp(0, (height - rc.bottom).max(0));
    if clamped != scroll {
        with_app(|a| a.view.scroll = clamped);
        app_settings::layout(view, &g, clamped);
    }
    unsafe {
        let si = SCROLLINFO {
            cbSize: size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: height - 1,
            nPage: rc.bottom.max(0) as u32,
            nPos: clamped,
            ..Default::default()
        };
        SetScrollInfo(view, SB_VERT, &si, true);
    }
}

fn relayout_mouse(view: HWND) {
    let g = geometry(view);
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(view, &mut rc);
    }
    let scroll = with_app(|a| a.view.scroll).unwrap_or(0);
    let old_bar_h = mouse_windows().map(|w| w.2);
    let mut height = mouse_settings::layout(view, &g, scroll);
    let Some((content, bar, bar_h)) = mouse_windows() else {
        return;
    };
    // The page fits its cards to the space above the bar: lay out again
    // with the bar's new height.
    if old_bar_h != Some(bar_h) {
        height = mouse_settings::layout(view, &g, scroll);
    }
    // The page shows above the bar; the bar sits on the bottom edge.
    let page = (rc.bottom - bar_h).max(0);
    let clamped = scroll.clamp(0, (height - page).max(0));
    if clamped != scroll {
        with_app(|a| a.view.scroll = clamped);
        mouse_settings::layout(view, &g, clamped);
    }
    unsafe {
        let _ = SetWindowPos(
            content,
            None,
            0,
            0,
            rc.right,
            page,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let _ = SetWindowPos(
            bar,
            None,
            0,
            page,
            rc.right,
            bar_h,
            SWP_NOZORDER | SWP_NOACTIVATE,
        );
        let _ = ShowWindow(bar, if bar_h > 0 { SW_SHOWNA } else { SW_HIDE });
        let si = SCROLLINFO {
            cbSize: size_of::<SCROLLINFO>() as u32,
            fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
            nMin: 0,
            nMax: height - 1,
            nPage: page as u32,
            nPos: clamped,
            ..Default::default()
        };
        SetScrollInfo(view, SB_VERT, &si, true);
    }
}

/// Removes the device grid (used when the mouse settings page opens).
pub(super) fn clear_devices_page(view: HWND) {
    let panels = with_app(|a| std::mem::take(&mut a.view.panels)).unwrap_or_default();
    for (_, h) in panels {
        unsafe {
            let _ = DestroyWindow(h);
        }
    }
    for b in with_app(|a| [a.view.refresh, a.view.settings_button])
        .unwrap_or_default()
        .into_iter()
        .flatten()
    {
        unsafe {
            let _ = ShowWindow(b, SW_HIDE);
        }
    }
    let _ = view;
}

/// Focuses «Ajustes» of the header (back from the «Ajustes» page).
pub(super) fn focus_settings_button() {
    if let Some(Some(b)) = with_app(|a| a.view.settings_button) {
        unsafe {
            let _ = SetFocus(Some(b));
        }
    }
}

/// Shows the device grid again and focuses the "Ajustes" button of `focus`.
pub(super) fn restore_devices_page(view: HWND, focus: Option<&crate::devices::DeviceKey>) {
    for b in with_app(|a| [a.view.refresh, a.view.settings_button])
        .unwrap_or_default()
        .into_iter()
        .flatten()
    {
        unsafe {
            let _ = ShowWindow(b, SW_SHOWNA);
        }
    }
    set_text_if_changed(view, "OpenPeripheral — Dispositivos");
    refresh();
    let target = focus.and_then(|key| {
        with_app(|a| {
            a.view
                .panels
                .iter()
                .find(|(k, _)| matches!(k, PanelKind::Device(d) if d == key))
                .map(|(_, h)| *h)
        })
        .flatten()
    });
    let button = target.and_then(|p| unsafe { GetDlgItem(Some(p), ID_SETTINGS).ok() });
    let fallback = with_app(|a| a.view.refresh).flatten();
    if let Some(b) = button.or(fallback) {
        unsafe {
            let _ = SetFocus(Some(b));
        }
    }
}

/// Scrolls so that `child` (a descendant of the view) is fully visible.
pub(super) fn ensure_visible(view: HWND, child: HWND) {
    let mut r = RECT::default();
    let mut rc = RECT::default();
    unsafe {
        let _ = GetWindowRect(child, &mut r);
        let mut pts = [
            windows::Win32::Foundation::POINT {
                x: r.left,
                y: r.top,
            },
            windows::Win32::Foundation::POINT {
                x: r.right,
                y: r.bottom,
            },
        ];
        // On the mouse page only the content scrolls; the bar is always
        // visible, so its controls never need scrolling.
        let target = match mouse_windows() {
            Some((content, ..)) if IsChild(content, child).as_bool() => content,
            Some(_) => return,
            None => view,
        };
        MapWindowPoints(None, Some(target), &mut pts);
        let _ = GetClientRect(target, &mut rc);
        let margin = (16.0 * scale_of(view)) as i32;
        let delta = if pts[0].y < 0 {
            pts[0].y - margin
        } else if pts[1].y > rc.bottom {
            pts[1].y - rc.bottom + margin
        } else {
            0
        };
        if delta != 0 {
            scroll_by(view, delta);
        }
    }
}

/// Scrolls the content by `delta` px. ScrollWindowEx moves the child
/// controls and the already painted pixels in one step, so only the newly
/// exposed strip is repainted (moving every control and repainting the
/// whole window on each wheel notch left blank buttons behind).
fn scroll_by(view: HWND, delta: i32) {
    if delta == 0 {
        return;
    }
    let mut si = SCROLLINFO {
        cbSize: size_of::<SCROLLINFO>() as u32,
        fMask: SIF_RANGE | SIF_PAGE | SIF_POS,
        ..Default::default()
    };
    unsafe {
        let _ = GetScrollInfo(view, SB_VERT, &mut si);
    }
    let max = (si.nMax + 1 - si.nPage as i32).max(0);
    let old = with_app(|a| a.view.scroll).unwrap_or(0);
    let new = (old + delta).clamp(0, max);
    if new == old {
        return;
    }
    with_app(|a| a.view.scroll = new);
    unsafe {
        si.fMask = SIF_POS;
        si.nPos = new;
        SetScrollInfo(view, SB_VERT, &si, true);
        let target = mouse_windows().map_or(view, |(content, ..)| content);
        ScrollWindowEx(
            target,
            0,
            old - new,
            None,
            None,
            None,
            None,
            SW_SCROLLCHILDREN | SW_INVALIDATE | SW_ERASE,
        );
        let _ = UpdateWindow(target);
    }
}

/// Applies the system theme if it changed (WM_SETTINGCHANGE is broadcast
/// for many unrelated settings; those cost nothing).
fn retheme(view: HWND) {
    if with_app(|a| a.view.palette) == Some(theme::current()) {
        return;
    }
    let _ = view;
    // `refresh` applies the new palette and repaints.
    refresh();
}

fn paint_view(view: HWND) {
    let Some((palette, fonts, shared, scroll)) = with_app(|a| {
        (
            a.view.palette,
            a.view.fonts.clone(),
            a.view.shared.clone(),
            a.view.scroll,
        )
    }) else {
        // Never leave the region invalid, or WM_PAINT would repeat forever.
        unsafe {
            let _ = ValidateRect(Some(view), None);
        }
        return;
    };
    let g = geometry(view);
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(view, &mut ps);
        let mut rc = RECT::default();
        let _ = GetClientRect(view, &mut rc);
        let mem = CreateCompatibleDC(Some(hdc));
        let bmp = CreateCompatibleBitmap(hdc, rc.right.max(1), rc.bottom.max(1));
        let old = SelectObject(mem, bmp.into());
        panel::fill(mem, rc, palette.bg_app);
        let mouse_page = with_app(|a| a.view.mouse.is_some()) == Some(true);
        let settings_page = with_app(|a| a.view.settings.is_some()) == Some(true);
        // The mouse page paints itself in its content window.
        if mouse_page {
        } else if settings_page {
            app_settings::paint(mem, &palette, g.scale, scroll);
        } else if let Some(f) = fonts {
            SetBkMode(mem, TRANSPARENT);
            let draw = |text: &str, font: HFONT, color: theme::Color, y: i32| {
                let o = SelectObject(mem, font.into());
                SetTextColor(mem, COLORREF(color.colorref()));
                let mut w: Vec<u16> = text.encode_utf16().collect();
                let mut r = RECT {
                    left: g.left,
                    top: y - scroll,
                    right: g.left + g.avail,
                    bottom: y - scroll + (60.0 * g.scale) as i32,
                };
                DrawTextW(
                    mem,
                    &mut w,
                    &mut r,
                    DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
                );
                SelectObject(mem, o);
            };
            let title_font = if g.compact { f.title_compact } else { f.title };
            draw("Dispositivos", title_font, palette.text_primary, g.top);
            let title_h = ((if g.compact { 30.0 } else { 35.0 }) * g.scale) as i32;
            draw(
                &text::connected_count(&shared),
                f.body,
                palette.text_secondary,
                g.top + title_h + (2.0 * g.scale) as i32,
            );
        }
        let _ = BitBlt(hdc, 0, 0, rc.right, rc.bottom, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
        let _ = EndPaint(view, &ps);
    }
}

extern "system" fn view_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_view(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_SIZE => {
            relayout(hwnd);
            unsafe {
                let _ = RedrawWindow(
                    Some(hwnd),
                    None,
                    None,
                    RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_ERASE,
                );
            }
            LRESULT(0)
        }
        WM_DRAWITEM => {
            let dis = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
            if with_app(|a| a.view.mouse.is_some()) == Some(true) {
                mouse_settings::draw_item(hwnd, dis);
            } else if with_app(|a| a.view.settings.is_some()) == Some(true) {
                app_settings::draw_item(hwnd, dis);
            } else if dis.CtlID as i32 == ID_APP_SETTINGS {
                draw_button(hwnd, dis, "Ajustes", None);
            } else {
                draw_button(hwnd, dis, "Actualizar", None);
            }
            LRESULT(1)
        }
        WM_CTLCOLORLISTBOX => mouse_settings::list_color(HDC(wparam.0 as *mut _))
            .unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }),
        WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN => {
            let (hdc, child) = (HDC(wparam.0 as *mut _), HWND(lparam.0 as *mut _));
            app_settings::ctl_color(hdc, child)
                .or_else(|| mouse_settings::ctl_color(hdc, child))
                .unwrap_or_else(|| unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) })
        }
        WM_COMMAND => {
            let (id, code) = (
                (wparam.0 & 0xffff) as i32,
                ((wparam.0 >> 16) & 0xffff) as u32,
            );
            if with_app(|a| a.view.mouse.is_some()) == Some(true)
                && mouse_settings::on_command(hwnd, id, code, lparam)
            {
                return LRESULT(0);
            }
            if with_app(|a| a.view.settings.is_some()) == Some(true)
                && app_settings::on_command(hwnd, id, code)
            {
                return LRESULT(0);
            }
            match (id, code) {
                (ID_REFRESH, BN_CLICKED) => send(Command::Refresh),
                (ID_APP_SETTINGS, BN_CLICKED) => app_settings::enter(hwnd),
                (ID_REFRESH | ID_APP_SETTINGS, BN_SETFOCUS) => {
                    ensure_visible(hwnd, HWND(lparam.0 as *mut _))
                }
                // Enter: owner-drawn buttons are not "push buttons" for the
                // dialog manager, which sends IDOK instead; click the focused one.
                (1, _) => unsafe {
                    let f = GetFocus();
                    if f != hwnd && is_descendant(hwnd, f) {
                        SendMessageW(f, BM_CLICK, None, None);
                    }
                },
                // Esc (IsDialogMessage sends IDCANCEL).
                (2, _) => unsafe {
                    let _ = DestroyWindow(hwnd);
                },
                _ => {}
            }
            LRESULT(0)
        }
        WM_VSCROLL => {
            let Some(scroll) = with_app(|a| a.view.scroll) else {
                return LRESULT(0);
            };
            let line = (40.0 * scale_of(hwnd)) as i32;
            let mut rc = RECT::default();
            unsafe {
                let _ = GetClientRect(hwnd, &mut rc);
            }
            let req = (wparam.0 & 0xffff) as i32;
            let target = match SCROLLBAR_COMMAND(req) {
                SB_LINEUP => scroll - line,
                SB_LINEDOWN => scroll + line,
                SB_PAGEUP => scroll - rc.bottom,
                SB_PAGEDOWN => scroll + rc.bottom,
                SB_TOP => 0,
                SB_BOTTOM => i32::MAX / 2,
                SB_THUMBTRACK | SB_THUMBPOSITION => {
                    let mut si = SCROLLINFO {
                        cbSize: size_of::<SCROLLINFO>() as u32,
                        fMask: SIF_TRACKPOS,
                        ..Default::default()
                    };
                    unsafe {
                        let _ = GetScrollInfo(hwnd, SB_VERT, &mut si);
                    }
                    si.nTrackPos
                }
                _ => scroll,
            };
            scroll_by(hwnd, target - scroll);
            LRESULT(0)
        }
        WM_MOUSEWHEEL => {
            let delta = ((wparam.0 >> 16) & 0xffff) as i16 as i32;
            scroll_by(hwnd, -delta * (40.0 * scale_of(hwnd)) as i32 / 120);
            LRESULT(0)
        }
        WM_GETMINMAXINFO => {
            let mmi = unsafe { &mut *(lparam.0 as *mut MINMAXINFO) };
            let s = scale_of(hwnd);
            mmi.ptMinTrackSize.x = (360.0 * s) as i32;
            mmi.ptMinTrackSize.y = (400.0 * s) as i32;
            LRESULT(0)
        }
        WM_DPICHANGED => {
            let dpi = (wparam.0 & 0xffff) as u32;
            with_app(|a| a.view.fonts = Some(Rc::new(Fonts::new(dpi))));
            let r = unsafe { &*(lparam.0 as *const RECT) };
            unsafe {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    r.left,
                    r.top,
                    r.right - r.left,
                    r.bottom - r.top,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            refresh();
            LRESULT(0)
        }
        WM_SETTINGCHANGE | WM_SYSCOLORCHANGE | WM_THEMECHANGED => {
            retheme(hwnd);
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            super::confirm::owner_destroyed();
            mouse_settings::destroyed();
            app_settings::destroyed();
            with_app(|a| {
                a.view.hwnd = None;
                a.view.panels.clear();
                a.view.refresh = None;
                a.view.settings_button = None;
            });
            LRESULT(0)
        }
        WM_NCDESTROY => {
            // Children are gone now: release fonts and shut GDI+ down.
            let (fonts, gdiplus) =
                with_app(|a| (a.view.fonts.take(), a.view.gdiplus.take())).unwrap_or((None, None));
            drop(fonts);
            drop(gdiplus);
            unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Paints an owner-drawn button on the colour of whatever is behind it.
fn draw_button(parent: HWND, dis: &DRAWITEMSTRUCT, label: &str, kind: Option<&PanelKind>) {
    let Some((palette, fonts, shared)) =
        with_app(|a| (a.view.palette, a.view.fonts.clone(), a.view.shared.clone()))
    else {
        return;
    };
    let Some(fonts) = fonts else { return };
    let bg = match kind {
        Some(PanelKind::Device(key)) => {
            let disconnected = shared
                .devices
                .iter()
                .find(|d| &d.info.key == key)
                .is_some_and(|d| matches!(d.status, crate::app::Status::Disconnected));
            if disconnected {
                palette.surface_card_disconnected
            } else {
                palette.surface_card
            }
        }
        _ => palette.bg_app,
    };
    panel::draw_button(
        dis,
        label,
        bg,
        &palette,
        &fonts,
        scale_of(parent),
        panel::ButtonStyle::Normal,
    );
}

fn panel_kind(h: HWND) -> Option<PanelKind> {
    with_app(|a| {
        a.view
            .panels
            .iter()
            .find(|(_, p)| *p == h)
            .map(|(k, _)| k.clone())
    })
    .flatten()
}

fn paint_panel(h: HWND) {
    let Some((palette, fonts, shared, kind)) = with_app(|a| {
        (
            a.view.palette,
            a.view.fonts.clone(),
            a.view.shared.clone(),
            a.view
                .panels
                .iter()
                .find(|(_, p)| *p == h)
                .map(|(k, _)| k.clone()),
        )
    }) else {
        unsafe {
            let _ = ValidateRect(Some(h), None);
        }
        return;
    };
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(h, &mut ps);
        let mut rc = RECT::default();
        let _ = GetClientRect(h, &mut rc);
        if let (Some(fonts), Some(kind), Some(view)) = (fonts, kind, GetParent(h).ok()) {
            let mem = CreateCompatibleDC(Some(hdc));
            let bmp = CreateCompatibleBitmap(hdc, rc.right.max(1), rc.bottom.max(1));
            let old = SelectObject(mem, bmp.into());
            let g = geometry(view);
            let ctx = Ctx {
                hdc: mem,
                fonts: &fonts,
                palette: &palette,
                scale: g.scale,
                compact: g.compact,
            };
            let layout = panel::layout(&ctx, &kind, &shared, rc.right, rc.bottom);
            panel::paint(&ctx, &layout, rc.right, rc.bottom);
            let _ = BitBlt(hdc, 0, 0, rc.right, rc.bottom, Some(mem), 0, 0, SRCCOPY);
            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
        }
        let _ = EndPaint(h, &ps);
    }
}

extern "system" fn panel_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_PAINT => {
            paint_panel(hwnd);
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        WM_DRAWITEM => {
            let dis = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
            let kind = panel_kind(hwnd);
            let label = match dis.CtlID as i32 {
                ID_SETTINGS => "Ajustes",
                ID_RETRY => "Reintentar",
                _ => "Actualizar",
            };
            draw_button(hwnd, dis, label, kind.as_ref());
            LRESULT(1)
        }
        WM_COMMAND => {
            let (id, code) = (
                (wparam.0 & 0xffff) as i32,
                ((wparam.0 >> 16) & 0xffff) as u32,
            );
            match code {
                BN_CLICKED => match (id, panel_kind(hwnd)) {
                    (ID_SETTINGS, Some(PanelKind::Device(key))) => {
                        // Mice open their settings page; other device types
                        // are still placeholders (DESIGN.md, deviation 2).
                        let is_mouse = with_app(|a| {
                            a.view.shared.devices.iter().any(|d| {
                                d.info.key == key && d.info.kind == crate::devices::Kind::Mouse
                            })
                        });
                        if is_mouse == Some(true) {
                            if let Ok(view) = unsafe { GetParent(hwnd) } {
                                mouse_settings::enter(view, key);
                            }
                        }
                    }
                    (ID_EMPTY_REFRESH | ID_RETRY, _) => send(Command::Refresh),
                    _ => {}
                },
                BN_SETFOCUS => {
                    if let (Ok(view), Ok(b)) = (unsafe { GetParent(hwnd) }, unsafe {
                        GetDlgItem(Some(hwnd), id)
                    }) {
                        ensure_visible(view, b);
                    }
                }
                _ => {}
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn column_count_by_width() {
        // 300 px minimum, 16 px gap.
        assert_eq!(columns(300, 16, 300), (1, 300));
        assert_eq!(columns(615, 16, 300), (1, 615));
        assert_eq!(columns(616, 16, 300), (2, 300));
        assert_eq!(columns(1080, 16, 300), (3, 349));
        assert_eq!(columns(100, 16, 300), (1, 100));
        // 200 % DPI: everything doubles.
        assert_eq!(columns(2160, 32, 600), (3, 698));
    }

    #[test]
    fn grid_placement() {
        assert!(place(0, 0, 3, 100, 10, &[]).is_empty());
        let r = place(50, 20, 2, 100, 10, &[80, 120, 60]);
        assert_eq!(r.len(), 3);
        // Row 1: both cards take the tallest height.
        assert_eq!(
            (r[0].left, r[0].top, r[0].right, r[0].bottom),
            (20, 50, 120, 170)
        );
        assert_eq!((r[1].left, r[1].bottom), (130, 170));
        // Row 2 starts after row 1 plus the gap.
        assert_eq!((r[2].left, r[2].top, r[2].bottom), (20, 180, 240));
        // No overlaps and within the width (2 × 100 + 10).
        for a in &r {
            assert!(a.right <= 20 + 210);
            for b in &r {
                if a != b {
                    let overlap = a.left < b.right
                        && b.left < a.right
                        && a.top < b.bottom
                        && b.top < a.bottom;
                    assert!(!overlap);
                }
            }
        }
        // 7 cards in 3 columns -> 3 rows.
        let r = place(0, 0, 3, 100, 10, &[1; 7]);
        assert_eq!(
            r.iter().map(|r| r.top).collect::<Vec<_>>(),
            vec![0, 0, 0, 11, 11, 11, 22]
        );
    }
}
