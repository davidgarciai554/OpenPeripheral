//! Settings window. Created on demand and destroyed on close, so it costs
//! nothing while hidden. Built from standard controls with keyboard
//! mnemonics, a logical tab order and the system message font.

use super::{text, wide, with_app};
use crate::app::{Command, Shared, Status};
use crate::config::{parse_hex, to_hex};
use crate::devices::{Change, DeviceKey, State as DevState};
use crate::platform::system;
use crate::protocol::razer::Rgb;
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::{
    COLOR_BTNFACE, CreateFontIndirectW, CreateSolidBrush, DeleteObject, GetSysColorBrush, HBRUSH,
    HFONT, InvalidateRect,
};
use windows::Win32::System::SystemServices::SS_NOPREFIX;
use windows::Win32::UI::Controls::Dialogs::{CC_FULLOPEN, CC_RGBINIT, CHOOSECOLORW, ChooseColorW};
use windows::Win32::UI::Controls::{BST_CHECKED, BST_UNCHECKED, EM_SETLIMITTEXT};
use windows::Win32::UI::HiDpi::{
    AdjustWindowRectExForDpi, GetDpiForWindow, SystemParametersInfoForDpi,
};
use windows::Win32::UI::Input::KeyboardAndMouse::{EnableWindow, GetFocus, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

const CLASS: PCWSTR = w!("OpenPeripheral.Settings");

const ID_LIST: i32 = 101;
const ID_DETAIL: i32 = 102;
const ID_DPI: i32 = 103;
const ID_DPI_RANGE: i32 = 104;
const ID_DPI_APPLY: i32 = 105;
const ID_COLOR: i32 = 106;
const ID_COLOR_PICK: i32 = 107;
const ID_SWATCH: i32 = 108;
const ID_COLOR_APPLY: i32 = 109;
const ID_RESTORE: i32 = 110;
const ID_WARN: i32 = 111;
const ID_STATUS: i32 = 112;
const ID_AUTOSTART: i32 = 113;
const ID_REFRESH: i32 = 114;
const ID_CLOSE: i32 = IDCANCEL.0;
const ID_LABEL: i32 = 0xffff; // labels need no id

/// Client size in 96-dpi units.
const WIDTH: i32 = 480;
const HEIGHT: i32 = 506;

#[derive(Default)]
pub(crate) struct State {
    selected: Option<DeviceKey>,
    /// Device whose values were last copied into the edit boxes.
    filled: Option<(DeviceKey, Option<u16>)>,
    list: Vec<String>,
    font: Option<HFONT>,
    swatch: Option<(Rgb, HBRUSH)>,
    /// Worker message sequence when a request was sent, and the text shown
    /// until the worker answers.
    pending: Option<(u64, &'static str)>,
    /// Last worker message seen; a newer one replaces any local message.
    seen_seq: u64,
    local_message: Option<(bool, String)>,
    custom_colors: [COLORREF; 16],
}

struct Ctl {
    id: i32,
    class: PCWSTR,
    text: &'static str,
    style: u32,
    rect: (i32, i32, i32, i32),
}

const fn ctl(
    id: i32,
    class: PCWSTR,
    text: &'static str,
    style: u32,
    rect: (i32, i32, i32, i32),
) -> Ctl {
    Ctl {
        id,
        class,
        text,
        style,
        rect,
    }
}

const STATIC: PCWSTR = w!("STATIC");
const BUTTON: PCWSTR = w!("BUTTON");
const EDIT: PCWSTR = w!("EDIT");
const LISTBOX: PCWSTR = w!("LISTBOX");

const TAB: u32 = WS_TABSTOP.0;
const NOPREFIX: u32 = SS_NOPREFIX.0;
const PUSH: u32 = BS_PUSHBUTTON as u32 | TAB;
const EDIT_STYLE: u32 = WS_BORDER.0 | ES_AUTOHSCROLL as u32 | TAB;

/// Controls in tab order. Each label precedes the control it names, which
/// gives that control its accessible name and makes the `&` mnemonic work.
fn controls() -> [Ctl; 19] {
    [
        ctl(ID_LABEL, STATIC, "&Dispositivos:", 0, (12, 12, 456, 20)),
        ctl(
            ID_LIST,
            LISTBOX,
            "",
            WS_BORDER.0 | WS_VSCROLL.0 | LBS_NOTIFY as u32 | LBS_NOINTEGRALHEIGHT as u32 | TAB,
            (12, 34, 456, 110),
        ),
        ctl(ID_DETAIL, STATIC, "", NOPREFIX, (12, 152, 456, 64)),
        ctl(ID_LABEL, STATIC, "D&PI:", 0, (12, 228, 110, 20)),
        ctl(
            ID_DPI,
            EDIT,
            "",
            EDIT_STYLE | ES_NUMBER as u32,
            (126, 225, 90, 24),
        ),
        ctl(ID_DPI_RANGE, STATIC, "", NOPREFIX, (224, 228, 130, 20)),
        ctl(
            ID_DPI_APPLY,
            BUTTON,
            "&Aplicar DPI",
            PUSH,
            (360, 224, 108, 28),
        ),
        ctl(ID_LABEL, STATIC, "&Color (#RRGGBB):", 0, (12, 264, 110, 20)),
        ctl(ID_COLOR, EDIT, "", EDIT_STYLE, (126, 261, 90, 24)),
        ctl(ID_COLOR_PICK, BUTTON, "&Elegir…", PUSH, (224, 260, 84, 28)),
        ctl(ID_SWATCH, STATIC, "", WS_BORDER.0, (316, 262, 32, 24)),
        ctl(
            ID_COLOR_APPLY,
            BUTTON,
            "Aplicar c&olor",
            PUSH,
            (360, 260, 108, 28),
        ),
        ctl(
            ID_RESTORE,
            BUTTON,
            "&Restaurar originales",
            PUSH,
            (12, 300, 180, 28),
        ),
        ctl(
            ID_LABEL,
            STATIC,
            "Los cambios no se guardan en la memoria del dispositivo; la aplicación los vuelve a aplicar al reconectarlo.",
            NOPREFIX,
            (12, 336, 456, 36),
        ),
        ctl(ID_WARN, STATIC, "", NOPREFIX, (12, 378, 456, 36)),
        ctl(ID_STATUS, STATIC, "", NOPREFIX, (12, 420, 456, 36)),
        ctl(
            ID_AUTOSTART,
            BUTTON,
            "Iniciar con &Windows",
            BS_AUTOCHECKBOX as u32 | TAB,
            (12, 468, 200, 24),
        ),
        ctl(ID_REFRESH, BUTTON, "Ac&tualizar", PUSH, (256, 466, 100, 28)),
        ctl(ID_CLOSE, BUTTON, "Cerrar", PUSH, (368, 466, 100, 28)),
    ]
}

pub(crate) fn register(instance: HINSTANCE) {
    unsafe {
        let class = WNDCLASSW {
            lpfnWndProc: Some(proc),
            hInstance: instance,
            lpszClassName: CLASS,
            hCursor: LoadCursorW(None, IDC_ARROW).unwrap_or_default(),
            hbrBackground: GetSysColorBrush(COLOR_BTNFACE),
            hIcon: LoadIconW(None, IDI_APPLICATION).unwrap_or_default(),
            ..Default::default()
        };
        RegisterClassW(&class);
    }
}

fn item(hwnd: HWND, id: i32) -> HWND {
    unsafe { GetDlgItem(Some(hwnd), id).unwrap_or_default() }
}

fn set_text(hwnd: HWND, id: i32, s: &str) {
    let w = wide(s);
    unsafe {
        let _ = SetWindowTextW(item(hwnd, id), PCWSTR(w.as_ptr()));
    }
}

fn get_text(hwnd: HWND, id: i32) -> String {
    let mut buf = [0u16; 64];
    let n = unsafe { GetWindowTextW(item(hwnd, id), &mut buf) };
    String::from_utf16_lossy(&buf[..n.max(0) as usize])
}

fn enable(hwnd: HWND, id: i32, on: bool) {
    unsafe {
        let _ = EnableWindow(item(hwnd, id), on);
    }
}

fn scale(v: i32, dpi: u32) -> i32 {
    v * dpi as i32 / 96
}

fn message_font(dpi: u32) -> Option<HFONT> {
    let mut m = NONCLIENTMETRICSW {
        cbSize: size_of::<NONCLIENTMETRICSW>() as u32,
        ..Default::default()
    };
    unsafe {
        SystemParametersInfoForDpi(
            SPI_GETNONCLIENTMETRICS.0,
            m.cbSize,
            Some((&mut m as *mut NONCLIENTMETRICSW).cast()),
            0,
            dpi,
        )
        .ok()?;
        let f = CreateFontIndirectW(&m.lfMessageFont);
        (!f.is_invalid()).then_some(f)
    }
}

/// Sizes the window and its children for the window's current DPI.
fn layout(hwnd: HWND, keep_position: Option<&RECT>) {
    unsafe {
        let dpi = GetDpiForWindow(hwnd).max(96);
        let style = WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX;
        let mut r = RECT {
            left: 0,
            top: 0,
            right: scale(WIDTH, dpi),
            bottom: scale(HEIGHT, dpi),
        };
        let _ = AdjustWindowRectExForDpi(&mut r, style, false, WINDOW_EX_STYLE::default(), dpi);
        let (w, h) = (r.right - r.left, r.bottom - r.top);
        match keep_position {
            Some(s) => {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    s.left,
                    s.top,
                    w,
                    h,
                    SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
            None => {
                let _ = SetWindowPos(
                    hwnd,
                    None,
                    0,
                    0,
                    w,
                    h,
                    SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
                );
            }
        }
        let font = message_font(dpi);
        // Direct children in z-order, which is creation (= tab) order.
        let mut children = Vec::new();
        let mut next = GetWindow(hwnd, GW_CHILD).ok();
        while let Some(c) = next {
            children.push(c);
            next = GetWindow(c, GW_HWNDNEXT).ok();
        }
        for (child, c) in children.iter().zip(controls().iter()) {
            let (x, y, cw, ch) = c.rect;
            let _ = MoveWindow(
                *child,
                scale(x, dpi),
                scale(y, dpi),
                scale(cw, dpi),
                scale(ch, dpi),
                true,
            );
            if let Some(f) = font {
                SendMessageW(
                    *child,
                    WM_SETFONT,
                    Some(WPARAM(f.0 as usize)),
                    Some(LPARAM(1)),
                );
            }
        }
        // Keep the old font if a new one could not be created: controls use it.
        let old = font.and_then(|f| with_app(|a| a.ui.font.replace(f)).flatten());
        if let Some(old) = old {
            let _ = DeleteObject(old.into());
        }
    }
}

fn create(instance: HINSTANCE) -> Option<HWND> {
    unsafe {
        let hwnd = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            CLASS,
            w!("OpenPeripheral — Ajustes"),
            WS_OVERLAPPED | WS_CAPTION | WS_SYSMENU | WS_MINIMIZEBOX,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            100,
            100,
            None,
            None,
            Some(instance),
            None,
        )
        .ok()?;
        for c in controls().iter() {
            let t = wide(c.text);
            let _ = CreateWindowExW(
                WINDOW_EX_STYLE::default(),
                c.class,
                PCWSTR(t.as_ptr()),
                WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | c.style),
                0,
                0,
                0,
                0,
                Some(hwnd),
                Some(HMENU(c.id as isize as *mut _)),
                Some(instance),
                None,
            );
        }
        SendMessageW(item(hwnd, ID_DPI), EM_SETLIMITTEXT, Some(WPARAM(5)), None);
        SendMessageW(item(hwnd, ID_COLOR), EM_SETLIMITTEXT, Some(WPARAM(7)), None);
        Some(hwnd)
    }
}

/// Opens (or focuses) the window, optionally selecting a device by index.
pub(crate) fn open(select: Option<DeviceKey>) {
    let existing = with_app(|a| a.settings).flatten();
    let hwnd = match existing {
        Some(h) => h,
        None => {
            let instance: HINSTANCE =
                unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(None) }
                    .unwrap_or_default()
                    .into();
            let Some(h) = create(instance) else { return };
            with_app(|a| a.settings = Some(h));
            layout(h, None);
            h
        }
    };
    if select.is_some() {
        with_app(|a| a.ui.selected = select);
    }
    refresh();
    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOWNORMAL);
        let _ = SetForegroundWindow(hwnd);
        let _ = SetFocus(Some(item(hwnd, ID_LIST)));
    }
}

fn selected_index(shared: &Shared, key: &Option<DeviceKey>) -> Option<usize> {
    let key = key.as_ref()?;
    shared.devices.iter().position(|d| &d.info.key == key)
}

/// Re-reads the shared state into the controls. Never holds the app borrow
/// while calling Win32, since controls notify the window synchronously.
pub(crate) fn refresh() {
    let Some(Some(hwnd)) = with_app(|a| a.settings) else {
        return;
    };
    let Some(shared) = super::snapshot() else {
        return;
    };
    let Some((mut selected, filled, old_list, pending, local)) = with_app(|a| {
        if a.ui.seen_seq != shared.seq {
            a.ui.seen_seq = shared.seq;
            a.ui.local_message = None;
        }
        (
            a.ui.selected.clone(),
            a.ui.filled.clone(),
            a.ui.list.clone(),
            a.ui.pending,
            a.ui.local_message.clone(),
        )
    }) else {
        return;
    };

    if selected_index(&shared, &selected).is_none() {
        selected = shared.devices.first().map(|d| d.info.key.clone());
    }
    let list: Vec<String> = shared.devices.iter().map(text::device_line).collect();
    let lb = item(hwnd, ID_LIST);
    unsafe {
        if list != old_list {
            SendMessageW(lb, LB_RESETCONTENT, None, None);
            for line in &list {
                let w = wide(line);
                SendMessageW(lb, LB_ADDSTRING, None, Some(LPARAM(w.as_ptr() as isize)));
            }
            if list.is_empty() {
                let w = wide(if shared.scanned {
                    "Sin dispositivos compatibles conectados"
                } else {
                    "Buscando dispositivos…"
                });
                SendMessageW(lb, LB_ADDSTRING, None, Some(LPARAM(w.as_ptr() as isize)));
            }
        }
        let sel = selected_index(&shared, &selected)
            .map(|i| i as isize)
            .unwrap_or(-1);
        SendMessageW(lb, LB_SETCURSEL, Some(WPARAM(sel as usize)), None);
    }

    let snap = selected_index(&shared, &selected).map(|i| &shared.devices[i]);
    let state: Option<&DevState> = snap.and_then(|s| match &s.status {
        Status::Ok(st) => Some(st),
        _ => None,
    });
    let detail = match snap {
        None => "Conecta un dispositivo compatible. La lista se actualiza sola.".to_string(),
        Some(s) => {
            let mut lines = vec![format!("{} {}", s.info.vendor, s.info.name)];
            match state {
                Some(st) => lines.extend(text::state_lines(st)),
                None => lines.push(format!("Estado: {}", text::status_line(s))),
            }
            if s.saved {
                lines.push("Tiene ajustes guardados que se aplican al conectarlo.".into());
            }
            lines.join("\r\n")
        }
    };
    set_text(hwnd, ID_DETAIL, &detail);

    let busy = pending.is_some_and(|(p, _)| p == shared.seq);
    let dpi = state.and_then(|s| s.dpi.as_ref());
    let color = state.is_some_and(|s| s.color);
    enable(hwnd, ID_DPI, dpi.is_some());
    enable(hwnd, ID_DPI_APPLY, dpi.is_some() && !busy);
    set_text(
        hwnd,
        ID_DPI_RANGE,
        &dpi.map(|d| text::dpi_range(&d.allowed))
            .unwrap_or_else(|| "no disponible".into()),
    );
    for id in [ID_COLOR, ID_COLOR_PICK] {
        enable(hwnd, id, color);
    }
    enable(hwnd, ID_COLOR_APPLY, color && !busy);
    enable(
        hwnd,
        ID_RESTORE,
        snap.is_some_and(|s| s.saved || dpi.is_some()) && !busy,
    );

    // Copy the device value into the DPI box when the selection or the
    // device's actual DPI changes (e.g. after "Restaurar"), not on every poll.
    let fill = snap.map(|s| (s.info.key.clone(), dpi.map(|d| d.current)));
    if fill != filled && (state.is_some() || fill.is_none()) {
        set_text(
            hwnd,
            ID_DPI,
            &dpi.map(|d| d.current.to_string()).unwrap_or_default(),
        );
        with_app(|a| a.ui.filled = fill);
    }

    set_text(hwnd, ID_WARN, &text::conflicts(&shared.conflicts));
    let status = if let Some((_, label)) = pending.filter(|_| busy) {
        label.to_string()
    } else if let Some((ok, t)) = local {
        format!("{}{t}", if ok { "" } else { "Error: " })
    } else {
        match &shared.message {
            Some(m) if m.ok => m.text.clone(),
            Some(m) => format!("Error: {}", m.text),
            None => String::new(),
        }
    };
    set_text(hwnd, ID_STATUS, &status);
    unsafe {
        let state = if system::autostart_enabled() {
            BST_CHECKED
        } else {
            BST_UNCHECKED
        };
        SendMessageW(
            item(hwnd, ID_AUTOSTART),
            BM_SETCHECK,
            Some(WPARAM(state.0 as usize)),
            None,
        );
    }
    with_app(|a| {
        a.ui.selected = selected;
        a.ui.list = list;
        if !busy {
            a.ui.pending = None;
        }
    });
}

fn local_message(ok: bool, text: String) {
    with_app(|a| a.ui.local_message = Some((ok, text)));
    refresh();
}

fn current() -> Option<(DeviceKey, DevState)> {
    let shared = super::snapshot()?;
    let key = with_app(|a| a.ui.selected.clone()).flatten()?;
    let snap = shared.devices.iter().find(|d| d.info.key == key)?;
    match &snap.status {
        Status::Ok(s) => Some((key, s.clone())),
        _ => None,
    }
}

/// Sends a request to the worker and shows `label` until it answers.
fn start(cmd: Command, label: &'static str) {
    let seq = super::snapshot().map(|s| s.seq).unwrap_or(0);
    with_app(|a| {
        a.ui.pending = Some((seq, label));
        a.ui.local_message = None;
        let _ = a.tx.send(cmd);
    });
    refresh();
}

fn submit(key: DeviceKey, change: Change) {
    start(Command::Apply(key, change), "Aplicando…");
}

fn apply_dpi(hwnd: HWND) {
    let Some((key, state)) = current() else {
        return;
    };
    let Some(dpi) = state.dpi else { return };
    let typed = get_text(hwnd, ID_DPI);
    let valid = typed
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|&v| dpi.allowed.contains(v));
    match valid {
        Some(v) => submit(key, Change::Dpi(v)),
        None => {
            local_message(
                false,
                format!(
                    "DPI no válido. Valores admitidos: {}.",
                    text::dpi_range(&dpi.allowed)
                ),
            );
            unsafe {
                let _ = SetFocus(Some(item(hwnd, ID_DPI)));
            }
        }
    }
}

fn apply_color(hwnd: HWND) {
    let Some((key, _)) = current() else { return };
    match parse_hex(&get_text(hwnd, ID_COLOR)) {
        Some(c) => submit(key, Change::Color(c)),
        None => {
            local_message(
                false,
                "Color no válido. Escribe seis cifras hexadecimales, por ejemplo #FF8000.".into(),
            );
            unsafe {
                let _ = SetFocus(Some(item(hwnd, ID_COLOR)));
            }
        }
    }
}

fn colorref(c: Rgb) -> COLORREF {
    COLORREF(c.r as u32 | (c.g as u32) << 8 | (c.b as u32) << 16)
}

fn pick_color(hwnd: HWND) {
    let initial = parse_hex(&get_text(hwnd, ID_COLOR)).unwrap_or(Rgb {
        r: 255,
        g: 255,
        b: 255,
    });
    let mut custom = with_app(|a| a.ui.custom_colors).unwrap_or_default();
    let mut cc = CHOOSECOLORW {
        lStructSize: size_of::<CHOOSECOLORW>() as u32,
        hwndOwner: hwnd,
        rgbResult: colorref(initial),
        lpCustColors: custom.as_mut_ptr(),
        Flags: CC_RGBINIT | CC_FULLOPEN,
        ..Default::default()
    };
    if unsafe { ChooseColorW(&mut cc) }.as_bool() {
        let v = cc.rgbResult.0;
        let c = Rgb {
            r: v as u8,
            g: (v >> 8) as u8,
            b: (v >> 16) as u8,
        };
        set_text(hwnd, ID_COLOR, &to_hex(c)); // EN_CHANGE updates the swatch
        unsafe {
            let _ = SetFocus(Some(item(hwnd, ID_COLOR_APPLY)));
        }
    }
    with_app(|a| a.ui.custom_colors = custom);
}

fn update_swatch(hwnd: HWND) {
    let color = parse_hex(&get_text(hwnd, ID_COLOR));
    let old = with_app(|a| {
        let new = color.map(|c| (c, unsafe { CreateSolidBrush(colorref(c)) }));
        std::mem::replace(&mut a.ui.swatch, new)
    })
    .flatten();
    if let Some((_, b)) = old {
        unsafe {
            let _ = DeleteObject(b.into());
        }
    }
    unsafe {
        let _ = InvalidateRect(Some(item(hwnd, ID_SWATCH)), None, true);
    }
}

fn on_command(hwnd: HWND, id: i32, code: u32) {
    match (id, code) {
        (ID_LIST, LBN_SELCHANGE) => {
            let sel = unsafe { SendMessageW(item(hwnd, ID_LIST), LB_GETCURSEL, None, None) }.0;
            let key = super::snapshot().and_then(|s| {
                usize::try_from(sel)
                    .ok()
                    .and_then(|i| s.devices.get(i))
                    .map(|d| d.info.key.clone())
            });
            if key.is_some() {
                with_app(|a| {
                    a.ui.selected = key;
                    a.ui.local_message = None;
                });
                refresh();
            }
        }
        (ID_COLOR, EN_CHANGE) => update_swatch(hwnd),
        (ID_DPI_APPLY, _) => apply_dpi(hwnd),
        (ID_COLOR_PICK, _) => pick_color(hwnd),
        (ID_COLOR_APPLY, _) => apply_color(hwnd),
        (ID_RESTORE, _) => {
            if let Some(key) = with_app(|a| a.ui.selected.clone()).flatten() {
                start(Command::Restore(key), "Restaurando…");
            }
        }
        (ID_AUTOSTART, _) => {
            let checked =
                unsafe { SendMessageW(item(hwnd, ID_AUTOSTART), BM_GETCHECK, None, None) }.0
                    == BST_CHECKED.0 as isize;
            match system::set_autostart(checked) {
                Ok(()) => local_message(
                    true,
                    if checked {
                        "Se iniciará con Windows."
                    } else {
                        "Ya no se iniciará con Windows."
                    }
                    .into(),
                ),
                Err(e) => local_message(
                    false,
                    format!("No se pudo cambiar el inicio automático ({e})."),
                ),
            }
        }
        (ID_REFRESH, _) => {
            start(Command::Refresh, "Actualizando…");
        }
        // Enter: apply the field that has the focus.
        (1, _) => {
            let focus = unsafe { GetFocus() };
            if focus == item(hwnd, ID_DPI) {
                apply_dpi(hwnd);
            } else if focus == item(hwnd, ID_COLOR) {
                apply_color(hwnd);
            }
        }
        // Esc or "Cerrar".
        (ID_CLOSE, _) => unsafe {
            let _ = DestroyWindow(hwnd);
        },
        _ => {}
    }
}

extern "system" fn proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_COMMAND => {
            on_command(
                hwnd,
                (wparam.0 & 0xffff) as i32,
                ((wparam.0 >> 16) & 0xffff) as u32,
            );
            LRESULT(0)
        }
        WM_CTLCOLORSTATIC if HWND(lparam.0 as *mut _) == item(hwnd, ID_SWATCH) => {
            match with_app(|a| a.ui.swatch.map(|(_, b)| b)).flatten() {
                Some(b) => LRESULT(b.0 as isize),
                None => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
            }
        }
        WM_DPICHANGED => {
            let suggested = unsafe { &*(lparam.0 as *const RECT) };
            layout(hwnd, Some(suggested));
            LRESULT(0)
        }
        WM_CLOSE => {
            unsafe {
                let _ = DestroyWindow(hwnd);
            }
            LRESULT(0)
        }
        WM_DESTROY => {
            let (font, swatch) = with_app(|a| {
                a.settings = None;
                a.ui.filled = None;
                a.ui.list.clear();
                a.ui.local_message = None;
                (a.ui.font.take(), a.ui.swatch.take())
            })
            .unwrap_or((None, None));
            unsafe {
                if let Some(f) = font {
                    let _ = DeleteObject(f.into());
                }
                if let Some((_, b)) = swatch {
                    let _ = DeleteObject(b.into());
                }
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}
