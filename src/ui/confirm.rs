//! Modal confirmation dialog of the design (DESIGN.md «Confirmación»): a
//! panel over a 55 % black scrim that covers the owner's client area. The
//! owner is disabled while it is open, so focus stays inside; it starts on
//! «Cancelar», Esc cancels and focus returns to the control that opened it.

use super::devices_view;
use super::panel::{self, ButtonStyle, Ctx, FontRole, Fonts};
use super::theme::Palette;
use super::{wide, with_app};
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::SystemServices::{SS_LEFT, SS_NOPREFIX};
use windows::Win32::UI::Controls::DRAWITEMSTRUCT;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, SetActiveWindow, SetFocus,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("OpenPeripheral.Confirm");
const SCRIM_CLASS: PCWSTR = w!("OpenPeripheral.Scrim");
const ID_TITLE: i32 = 1;
const ID_BODY: i32 = 2;
const ID_CANCEL: i32 = 3;
const ID_CONFIRM: i32 = 4;
/// `overlay.scrim`: black at 55 %.
const SCRIM_ALPHA: u8 = 140;

struct Dialog {
    hwnd: HWND,
    scrim: HWND,
    owner: HWND,
    return_focus: HWND,
    confirm_label: String,
    palette: Palette,
    fonts: Rc<Fonts>,
    brush: HBRUSH,
    on_confirm: Option<Box<dyn FnOnce()>>,
}

thread_local! {
    // Separate from the app state: the dialog's procedure runs while the
    // app state may be borrowed by the caller that opened it.
    static DIALOG: RefCell<Option<Dialog>> = const { RefCell::new(None) };
}

/// The open dialog, for the message loop's `IsDialogMessageW`.
pub(crate) fn hwnd() -> Option<HWND> {
    DIALOG.with(|d| d.try_borrow().ok().and_then(|d| d.as_ref().map(|d| d.hwnd)))
}

fn register() {
    unsafe {
        let instance = devices_view::instance();
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        let dialog = WNDCLASSW {
            lpfnWndProc: Some(dialog_proc),
            hInstance: instance,
            lpszClassName: CLASS,
            hCursor: cursor,
            ..Default::default()
        };
        RegisterClassW(&dialog);
        let scrim = WNDCLASSW {
            lpfnWndProc: Some(scrim_proc),
            hInstance: instance,
            lpszClassName: SCRIM_CLASS,
            hCursor: cursor,
            hbrBackground: HBRUSH(GetStockObject(BLACK_BRUSH).0),
            ..Default::default()
        };
        RegisterClassW(&scrim);
    }
}

fn child(parent: HWND, id: i32, class: PCWSTR, text: &str, style: u32) -> HWND {
    let t = wide(text);
    unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            class,
            PCWSTR(t.as_ptr()),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | style),
            0,
            0,
            0,
            0,
            Some(parent),
            Some(HMENU(id as isize as *mut _)),
            Some(devices_view::instance()),
            None,
        )
        .unwrap_or_default()
    }
}

/// Opens the dialog over `owner`. `on_confirm` runs after it closes, only
/// if the user chose `confirm`. Does nothing if a dialog is already open.
pub(crate) fn open(
    owner: HWND,
    title: &str,
    body: &str,
    confirm: &str,
    on_confirm: Box<dyn FnOnce()>,
) {
    if hwnd().is_some() {
        return;
    }
    let Some((palette, Some(fonts))) = with_app(|a| (a.view.palette, a.view.fonts.clone())) else {
        return;
    };
    register();
    let return_focus = unsafe { GetFocus() };
    let scale = devices_view::scale_of(owner);
    let px = |u: f32| (u * scale).round() as i32;

    // Owner client area in screen coordinates.
    let mut rc = RECT::default();
    let mut origin = POINT::default();
    unsafe {
        let _ = GetClientRect(owner, &mut rc);
        let _ = ClientToScreen(owner, &mut origin);
    }

    // Size of the panel from its texts.
    let width = (rc.right - px(32.0)).min(px(480.0)).max(px(240.0));
    let pad = px(24.0);
    let inner = width - 2 * pad;
    let hdc = unsafe { GetDC(Some(owner)) };
    let ctx = Ctx {
        hdc,
        fonts: &fonts,
        palette: &palette,
        scale,
        compact: false,
    };
    let (_, title_h) = ctx.measure(title, FontRole::Name, inner, 0);
    let (_, body_h) = ctx.measure(body, FontRole::Body, inner, 0);
    let bh = px(36.0);
    let ring = px(4.0);
    let cancel_w = panel::button_width(&ctx, "Cancelar");
    let confirm_w = panel::button_width(&ctx, confirm);
    unsafe {
        ReleaseDC(Some(owner), hdc);
    }
    let height = pad + title_h + px(12.0) + body_h + px(24.0) + bh + pad;

    unsafe {
        let Ok(scrim) = CreateWindowExW(
            WS_EX_LAYERED | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
            SCRIM_CLASS,
            w!(""),
            WS_POPUP,
            origin.x,
            origin.y,
            rc.right,
            rc.bottom,
            Some(owner),
            None,
            Some(devices_view::instance()),
            None,
        ) else {
            return;
        };
        let _ = SetLayeredWindowAttributes(scrim, COLORREF(0), SCRIM_ALPHA, LWA_ALPHA);
        let t = wide(title);
        let Ok(dialog) = CreateWindowExW(
            WS_EX_CONTROLPARENT,
            CLASS,
            PCWSTR(t.as_ptr()),
            WS_POPUP,
            origin.x + (rc.right - width) / 2,
            origin.y + ((rc.bottom - height) / 2).max(0),
            width,
            height,
            Some(scrim),
            None,
            Some(devices_view::instance()),
            None,
        ) else {
            let _ = DestroyWindow(scrim);
            return;
        };
        let st = SS_NOPREFIX.0 | SS_LEFT.0;
        let btn = WS_TABSTOP.0 | (BS_OWNERDRAW | BS_NOTIFY) as u32;
        let h_title = child(dialog, ID_TITLE, w!("STATIC"), title, st);
        let h_body = child(dialog, ID_BODY, w!("STATIC"), body, st);
        let h_cancel = child(dialog, ID_CANCEL, w!("BUTTON"), "Cancelar", btn);
        let h_confirm = child(dialog, ID_CONFIRM, w!("BUTTON"), confirm, btn);
        let place = |h: HWND, x: i32, y: i32, w: i32, h2: i32| {
            let _ = SetWindowPos(h, None, x, y, w, h2, SWP_NOZORDER | SWP_NOACTIVATE);
        };
        place(h_title, pad, pad, inner, title_h);
        place(h_body, pad, pad + title_h + px(12.0), inner, body_h);
        let by = height - pad - bh;
        let cx = width - pad - confirm_w;
        place(
            h_confirm,
            cx - ring,
            by - ring,
            confirm_w + 2 * ring,
            bh + 2 * ring,
        );
        let kx = cx - px(12.0) - cancel_w;
        place(
            h_cancel,
            kx - ring,
            by - ring,
            cancel_w + 2 * ring,
            bh + 2 * ring,
        );
        for (h, font) in [
            (h_title, fonts.name),
            (h_body, fonts.body),
            (h_cancel, fonts.body),
            (h_confirm, fonts.body),
        ] {
            SendMessageW(
                h,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(0)),
            );
        }
        let brush = CreateSolidBrush(COLORREF(palette.surface_card.colorref()));
        DIALOG.with(|d| {
            *d.borrow_mut() = Some(Dialog {
                hwnd: dialog,
                scrim,
                owner,
                return_focus,
                confirm_label: confirm.to_string(),
                palette,
                fonts,
                brush,
                on_confirm: Some(on_confirm),
            })
        });
        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(scrim, SW_SHOWNOACTIVATE);
        let _ = ShowWindow(dialog, SW_SHOW);
        let _ = SetForegroundWindow(dialog);
        let _ = SetFocus(Some(h_cancel));
    }
}

/// Closes the dialog; runs the confirmation action if `confirmed`.
fn close(confirmed: bool) {
    let Some(mut d) = DIALOG.with(|d| d.borrow_mut().take()) else {
        return;
    };
    unsafe {
        // Re-enable the owner first so it gets the activation back.
        let _ = EnableWindow(d.owner, true);
        let _ = SetActiveWindow(d.owner);
        let _ = DestroyWindow(d.hwnd);
        let _ = DestroyWindow(d.scrim);
        let _ = DeleteObject(d.brush.into());
        if !d.return_focus.is_invalid() && IsWindow(Some(d.return_focus)).as_bool() {
            let _ = SetFocus(Some(d.return_focus));
        }
    }
    if confirmed {
        if let Some(f) = d.on_confirm.take() {
            f();
        }
    }
}

/// The owner is being destroyed: drop the dialog without running anything.
pub(crate) fn owner_destroyed() {
    if let Some(d) = DIALOG.with(|d| d.borrow_mut().take()) {
        unsafe {
            let _ = DestroyWindow(d.hwnd);
            let _ = DestroyWindow(d.scrim);
            let _ = DeleteObject(d.brush.into());
        }
    }
}

extern "system" fn scrim_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

extern "system" fn dialog_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let style = || {
        DIALOG.with(|d| {
            d.try_borrow().ok().and_then(|d| {
                d.as_ref()
                    .map(|d| (d.palette, d.fonts.clone(), d.brush, d.confirm_label.clone()))
            })
        })
    };
    match msg {
        WM_PAINT => unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if let Some((palette, ..)) = style() {
                let mut rc = RECT::default();
                let _ = GetClientRect(hwnd, &mut rc);
                panel::fill(hdc, rc, palette.border_control);
                let s = devices_view::scale_of(hwnd).round().max(1.0) as i32;
                let inner = RECT {
                    left: s,
                    top: s,
                    right: rc.right - s,
                    bottom: rc.bottom - s,
                };
                panel::fill(hdc, inner, palette.surface_card);
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        },
        WM_ERASEBKGND => LRESULT(1),
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN => {
            let Some((palette, _, brush, _)) = style() else {
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            };
            let hdc = HDC(wparam.0 as *mut _);
            let id = unsafe { GetDlgCtrlID(HWND(lparam.0 as *mut _)) };
            let fg = if id == ID_BODY {
                palette.text_secondary
            } else {
                palette.text_primary
            };
            unsafe {
                SetTextColor(hdc, COLORREF(fg.colorref()));
                SetBkColor(hdc, COLORREF(palette.surface_card.colorref()));
            }
            LRESULT(brush.0 as isize)
        }
        WM_DRAWITEM => {
            let dis = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
            if let Some((palette, fonts, _, confirm)) = style() {
                let (label, style) = if dis.CtlID as i32 == ID_CONFIRM {
                    (confirm, ButtonStyle::Primary)
                } else {
                    ("Cancelar".to_string(), ButtonStyle::Normal)
                };
                panel::draw_button(
                    dis,
                    &label,
                    palette.surface_card,
                    &palette,
                    &fonts,
                    devices_view::scale_of(hwnd),
                    style,
                );
            }
            LRESULT(1)
        }
        WM_COMMAND => {
            let (id, code) = (
                (wparam.0 & 0xffff) as i32,
                ((wparam.0 >> 16) & 0xffff) as u32,
            );
            match (id, code) {
                (ID_CONFIRM, BN_CLICKED) => close(true),
                (ID_CANCEL, BN_CLICKED) | (2, _) => close(false),
                // Enter: owner-drawn buttons get IDOK; click the focused one.
                (1, _) => unsafe {
                    let f = GetFocus();
                    if GetParent(f).ok() == Some(hwnd) {
                        SendMessageW(f, BM_CLICK, None, None);
                    }
                },
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            close(false);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
