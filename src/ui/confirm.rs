//! Modal confirmation dialog of the design (DESIGN.md «Confirmación»): a
//! panel over a 55 % black scrim that covers the owner's client area. The
//! owner is disabled while it is open, so focus stays inside; it starts on
//! «Cancelar» (or on the name field when there is one), Esc cancels and
//! focus returns to the control that opened it. Besides «Cancelar» it has
//! one or more choices (the last one is the emphasised action) and, for the
//! profile names, an optional text field checked as the user types.

use super::devices_view;
use super::panel::{self, ButtonStyle, Ctx, FontRole, Fonts};
use super::theme::Palette;
use super::{wide, with_app};
use std::cell::RefCell;
use std::rc::Rc;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, POINT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::SystemServices::{SS_LEFT, SS_NOPREFIX};
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, EM_SETLIMITTEXT, EM_SETSEL};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, IsWindowEnabled, SetActiveWindow, SetFocus,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("OpenPeripheral.Confirm");
const SCRIM_CLASS: PCWSTR = w!("OpenPeripheral.Scrim");
const ID_TITLE: i32 = 1;
const ID_BODY: i32 = 2;
const ID_CANCEL: i32 = 3;
const ID_FIELD_LABEL: i32 = 5;
const ID_FIELD: i32 = 6;
const ID_ERROR: i32 = 7;
/// Choice buttons: `ID_CHOICE0 + i`.
const ID_CHOICE0: i32 = 10;
/// `overlay.scrim`: black at 55 %.
const SCRIM_ALPHA: u8 = 140;

/// Check of the field's text: the error to show, if any.
pub(crate) type Check = Box<dyn Fn(&str) -> Option<String>>;

/// A text field in the dialog: label, initial text, maximum length and a
/// check that returns the error to show (the choices stay disabled then).
pub(crate) struct Field {
    pub label: String,
    pub text: String,
    pub max: usize,
    pub check: Check,
}

struct Dialog {
    hwnd: HWND,
    scrim: HWND,
    owner: HWND,
    return_focus: HWND,
    choices: Vec<String>,
    palette: Palette,
    fonts: Rc<Fonts>,
    brush: HBRUSH,
    field_brush: HBRUSH,
    field: Option<(HWND, RECT, Check)>,
    invalid: bool,
    on_choice: Option<Box<dyn FnOnce(usize, String)>>,
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
    open_choices(
        owner,
        title,
        body,
        &[confirm],
        None,
        Box::new(move |_, _| on_confirm()),
    );
}

/// Like `open` with several choices after «Cancelar» and an optional field.
/// `on_choice` gets the index of the choice and the field's text.
pub(crate) fn open_choices(
    owner: HWND,
    title: &str,
    body: &str,
    choices: &[&str],
    field: Option<Field>,
    on_choice: Box<dyn FnOnce(usize, String)>,
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
    let gap = px(12.0);
    let cancel_w = panel::button_width(&ctx, "Cancelar");
    let widths: Vec<i32> = choices
        .iter()
        .map(|c| panel::button_width(&ctx, c))
        .collect();
    // The buttons go in one row when they fit, else one per line.
    let row_w = cancel_w + widths.iter().map(|w| w + gap).sum::<i32>();
    let stacked = row_w > inner;
    let buttons_h = if stacked {
        (bh + gap) * (1 + choices.len() as i32) - gap
    } else {
        bh
    };
    // Field: label, edit box and room for a two-line error.
    let (label_h, error_h) = match &field {
        Some(f) => (
            ctx.measure(&f.label, FontRole::Body, inner, 0).1,
            ctx.measure(
                "Error: ya hay un perfil llamado «MMMMMMMMMMMMMMMMMMMMMMMMMMMMMMMM». Elige otro nombre.",
                FontRole::Body,
                inner,
                0,
            )
            .1,
        ),
        None => (0, 0),
    };
    let field_h = if field.is_some() {
        px(16.0) + label_h + px(6.0) + bh + px(8.0) + error_h
    } else {
        0
    };
    unsafe {
        ReleaseDC(Some(owner), hdc);
    }
    let height = pad + title_h + px(12.0) + body_h + field_h + px(24.0) + buttons_h + pad;

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
        let place = |h: HWND, x: i32, y: i32, w: i32, h2: i32| {
            let _ = SetWindowPos(h, None, x, y, w, h2, SWP_NOZORDER | SWP_NOACTIVATE);
        };
        place(h_title, pad, pad, inner, title_h);
        place(h_body, pad, pad + title_h + px(12.0), inner, body_h);
        let mut fonted = vec![(h_title, fonts.name), (h_body, fonts.body)];
        // The label directly precedes the field: it becomes its accessible
        // name. Creation order is tab order: field, Cancelar, choices.
        let mut field_box = None;
        let mut h_field = None;
        if let Some(f) = &field {
            let fy = pad + title_h + px(12.0) + body_h + px(16.0);
            let h_label = child(dialog, ID_FIELD_LABEL, w!("STATIC"), &f.label, st);
            place(h_label, pad, fy, inner, label_h);
            let ey = fy + label_h + px(6.0);
            let h = child(
                dialog,
                ID_FIELD,
                w!("EDIT"),
                &f.text,
                WS_TABSTOP.0 | ES_AUTOHSCROLL as u32,
            );
            let edit_h = (bh - px(10.0)).max(px(20.0));
            place(
                h,
                pad + px(8.0),
                ey + (bh - edit_h) / 2,
                inner - px(16.0),
                edit_h,
            );
            SendMessageW(h, EM_SETLIMITTEXT, Some(WPARAM(f.max)), None);
            let h_error = child(dialog, ID_ERROR, w!("STATIC"), "", st);
            place(h_error, pad, ey + bh + px(8.0), inner, error_h);
            fonted.extend([
                (h_label, fonts.body),
                (h, fonts.body),
                (h_error, fonts.body),
            ]);
            field_box = Some(RECT {
                left: pad,
                top: ey,
                right: pad + inner,
                bottom: ey + bh,
            });
            h_field = Some(h);
        }
        let h_cancel = child(dialog, ID_CANCEL, w!("BUTTON"), "Cancelar", btn);
        fonted.push((h_cancel, fonts.body));
        let by = height - pad - buttons_h;
        if stacked {
            place(
                h_cancel,
                pad - ring,
                by - ring,
                inner + 2 * ring,
                bh + 2 * ring,
            );
        }
        let mut x = width - pad - widths.iter().map(|w| w + gap).sum::<i32>() + gap;
        place_row_cancel(
            stacked,
            h_cancel,
            x - gap - cancel_w,
            by,
            cancel_w,
            bh,
            ring,
        );
        for (i, label) in choices.iter().enumerate() {
            let h = child(dialog, ID_CHOICE0 + i as i32, w!("BUTTON"), label, btn);
            fonted.push((h, fonts.body));
            if stacked {
                let y = by + (bh + gap) * (i as i32 + 1);
                place(h, pad - ring, y - ring, inner + 2 * ring, bh + 2 * ring);
            } else {
                place(h, x - ring, by - ring, widths[i] + 2 * ring, bh + 2 * ring);
                x += widths[i] + gap;
            }
        }
        for (h, font) in fonted {
            SendMessageW(
                h,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(0)),
            );
        }
        let brush = CreateSolidBrush(COLORREF(palette.surface_card.colorref()));
        let field_brush = CreateSolidBrush(COLORREF(palette.bg_app.colorref()));
        DIALOG.with(|d| {
            *d.borrow_mut() = Some(Dialog {
                hwnd: dialog,
                scrim,
                owner,
                return_focus,
                choices: choices.iter().map(|c| c.to_string()).collect(),
                palette,
                fonts,
                brush,
                field_brush,
                field: match (h_field, field_box, field) {
                    (Some(h), Some(r), Some(f)) => Some((h, r, f.check)),
                    _ => None,
                },
                invalid: false,
                on_choice: Some(on_choice),
            })
        });
        check_field();
        let _ = EnableWindow(owner, false);
        let _ = ShowWindow(scrim, SW_SHOWNOACTIVATE);
        let _ = ShowWindow(dialog, SW_SHOW);
        let _ = SetForegroundWindow(dialog);
        match h_field {
            Some(h) => {
                let _ = SetFocus(Some(h));
                SendMessageW(h, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
            }
            None => {
                let _ = SetFocus(Some(h_cancel));
            }
        }
    }
}

/// «Cancelar» to the left of the choices when they share a row.
fn place_row_cancel(stacked: bool, h: HWND, x: i32, y: i32, w: i32, bh: i32, ring: i32) {
    if !stacked {
        unsafe {
            let _ = SetWindowPos(
                h,
                None,
                x - ring,
                y - ring,
                w + 2 * ring,
                bh + 2 * ring,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
}

fn text_of(h: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(h) }.max(0) as usize;
    let mut buf = vec![0u16; len + 1];
    let n = unsafe { GetWindowTextW(h, &mut buf) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

/// Checks the field: shows the error, marks the field invalid and enables
/// the choices only when the text is valid.
fn check_field() {
    let Some((dialog, error, n)) = DIALOG.with(|d| {
        let mut d = d.try_borrow_mut().ok()?;
        let d = d.as_mut()?;
        let (h, _, check) = d.field.as_ref()?;
        let error = check(&text_of(*h));
        d.invalid = error.is_some();
        Some((d.hwnd, error, d.choices.len()))
    }) else {
        return;
    };
    unsafe {
        if let Ok(h) = GetDlgItem(Some(dialog), ID_ERROR) {
            let t = wide(
                &error
                    .as_deref()
                    .map(|e| format!("Error: {e}"))
                    .unwrap_or_default(),
            );
            let _ = SetWindowTextW(h, PCWSTR(t.as_ptr()));
        }
        for i in 0..n as i32 {
            if let Ok(h) = GetDlgItem(Some(dialog), ID_CHOICE0 + i) {
                let _ = EnableWindow(h, error.is_none());
            }
        }
        let _ = InvalidateRect(Some(dialog), None, true);
    }
}

/// Closes the dialog; runs the action of the choice, if any.
fn close(choice: Option<usize>) {
    let text = DIALOG
        .with(|d| {
            d.try_borrow().ok().and_then(|d| {
                d.as_ref()
                    .and_then(|d| d.field.as_ref().map(|f| text_of(f.0)))
            })
        })
        .unwrap_or_default();
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
        let _ = DeleteObject(d.field_brush.into());
        if !d.return_focus.is_invalid() && IsWindow(Some(d.return_focus)).as_bool() {
            let _ = SetFocus(Some(d.return_focus));
        }
    }
    if let (Some(i), Some(f)) = (choice, d.on_choice.take()) {
        f(i, text);
    }
}

/// The owner is being destroyed: drop the dialog without running anything.
pub(crate) fn owner_destroyed() {
    if let Some(d) = DIALOG.with(|d| d.borrow_mut().take()) {
        unsafe {
            let _ = DestroyWindow(d.hwnd);
            let _ = DestroyWindow(d.scrim);
            let _ = DeleteObject(d.brush.into());
            let _ = DeleteObject(d.field_brush.into());
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
                d.as_ref().map(|d| {
                    (
                        d.palette,
                        d.fonts.clone(),
                        d.brush,
                        d.choices.clone(),
                        d.field_brush,
                        d.field.as_ref().map(|f| f.1),
                        d.invalid,
                    )
                })
            })
        })
    };
    match msg {
        WM_PAINT => unsafe {
            let mut ps = PAINTSTRUCT::default();
            let hdc = BeginPaint(hwnd, &mut ps);
            if let Some((palette, _, _, _, _, field, invalid)) = style() {
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
                // Field box: `border.control`, 2 px `text.primary` when invalid.
                if let Some(f) = field {
                    let b = if invalid { 2 * s } else { s };
                    let border = if invalid {
                        palette.text_primary
                    } else {
                        palette.border_control
                    };
                    panel::fill(hdc, f, border);
                    let inside = RECT {
                        left: f.left + b,
                        top: f.top + b,
                        right: f.right - b,
                        bottom: f.bottom - b,
                    };
                    panel::fill(hdc, inside, palette.bg_app);
                }
            }
            let _ = EndPaint(hwnd, &ps);
            LRESULT(0)
        },
        WM_ERASEBKGND => LRESULT(1),
        WM_CTLCOLORSTATIC | WM_CTLCOLORBTN | WM_CTLCOLOREDIT => {
            let Some((palette, _, brush, _, field_brush, ..)) = style() else {
                return unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) };
            };
            let hdc = HDC(wparam.0 as *mut _);
            let id = unsafe { GetDlgCtrlID(HWND(lparam.0 as *mut _)) };
            let fg = match id {
                ID_BODY => palette.text_secondary,
                ID_FIELD_LABEL => palette.text_notice,
                _ => palette.text_primary,
            };
            let (bg, brush) = if id == ID_FIELD {
                (palette.bg_app, field_brush)
            } else {
                (palette.surface_card, brush)
            };
            unsafe {
                SetTextColor(hdc, COLORREF(fg.colorref()));
                SetBkColor(hdc, COLORREF(bg.colorref()));
            }
            LRESULT(brush.0 as isize)
        }
        WM_DRAWITEM => {
            let dis = unsafe { &*(lparam.0 as *const DRAWITEMSTRUCT) };
            if let Some((palette, fonts, _, choices, ..)) = style() {
                let id = dis.CtlID as i32;
                let (label, style) = match usize::try_from(id - ID_CHOICE0)
                    .ok()
                    .and_then(|i| choices.get(i).map(|c| (i, c)))
                {
                    // The last choice is the emphasised action.
                    Some((i, c)) if i + 1 == choices.len() => (c.clone(), ButtonStyle::Primary),
                    Some((_, c)) => (c.clone(), ButtonStyle::Normal),
                    None => ("Cancelar".to_string(), ButtonStyle::Normal),
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
                (ID_FIELD, EN_CHANGE) => check_field(),
                (ID_CANCEL, BN_CLICKED) | (2, _) => close(None),
                (id, BN_CLICKED) if id >= ID_CHOICE0 => close(Some((id - ID_CHOICE0) as usize)),
                // Enter: owner-drawn buttons get IDOK; click the focused one.
                // In the field, Enter takes the emphasised (last) choice.
                (1, _) => unsafe {
                    let f = GetFocus();
                    if GetDlgCtrlID(f) == ID_FIELD {
                        let last = style().map(|s| s.3.len()).unwrap_or(0) as i32 - 1;
                        if let Ok(h) = GetDlgItem(Some(hwnd), ID_CHOICE0 + last) {
                            if IsWindowEnabled(h).as_bool() {
                                SendMessageW(h, BM_CLICK, None, None);
                            }
                        }
                    } else if GetParent(f).ok() == Some(hwnd) {
                        SendMessageW(f, BM_CLICK, None, None);
                    }
                },
                _ => {}
            }
            LRESULT(0)
        }
        WM_CLOSE => {
            close(None);
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
