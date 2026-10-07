//! "Ajustes del ratón" page (`docs/design/DESIGN.md`), shown in place of the
//! device grid inside the same window. Text is native STATIC controls and
//! the DPI value a native EDIT, so screen readers read them; the window
//! paints only the surfaces behind them (cards, field border, glyphs). The
//! STATICs are opaque in their surface colour, so nothing flickers.

use super::devices_view::{self, Geometry};
use super::glyph::{self, Canvas};
use super::panel::{self, ButtonStyle, Ctx, FontRole, Fonts};
use super::text::{self, Energy};
use super::theme::{Color, Palette};
use super::{send, wide, with_app};
use crate::app::{Command, Confirmed, Profile, Shared, Snapshot, Source, Status};
use crate::devices::{Change, DeviceKey, DpiRange};
use crate::protocol::onboard::Action;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::SystemServices::{SS_LEFT, SS_NOPREFIX};
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, EM_SETLIMITTEXT, EM_SETSEL, SetWindowTheme};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, IsWindowEnabled, SetFocus, VIRTUAL_KEY, VK_CONTROL,
    VK_ESCAPE, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_TAB,
};
use windows::Win32::UI::Shell::{DefSubclassProc, RemoveWindowSubclass, SetWindowSubclass};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

// Control ids. Creation order is tab order (DESIGN.md "Teclado y foco").
const ID_BACK: i32 = 300;
const ID_TITLE: i32 = 301;
const ID_SUB: i32 = 302;
const ID_PCT: i32 = 303;
const ID_PSTATE: i32 = 304;
const ID_DISC: i32 = 305;
const ID_NOTICE: i32 = 306;
const ID_DPI_TITLE: i32 = 310;
const ID_DPI_DESC: i32 = 311;
const ID_DPI_CURRENT: i32 = 312;
const ID_DPI_LABEL: i32 = 313;
const ID_DPI_EDIT: i32 = 314;
const ID_MINUS: i32 = 315;
const ID_PLUS: i32 = 316;
const ID_APPLY: i32 = 317;
const ID_PRESETS_LABEL: i32 = 318;
const ID_PRESET0: i32 = 320; // ..=323
const ID_DPI_STATUS: i32 = 324;
const ID_BTN_TITLE: i32 = 360;
const ID_BTN_DESC: i32 = 361;
const ID_BTN_STATUS: i32 = 362;
const ID_BTN_NAME0: i32 = 370; // ..=375
const ID_BTN_ACT0: i32 = 380; // ..=385
const ID_BTN_LOCK: i32 = 386;
const ID_BTN_BACKUP: i32 = 387;
/// Rows created for the buttons table (the G305 declares 6).
const MAX_BUTTONS: i32 = 6;
const ID_BTN_BADGE0: i32 = 390; // ..=395
const ID_BTN_CHANGE0: i32 = 400; // ..=405
// Row editor (moved in tab order right after the row being edited).
const ID_ED_LABEL: i32 = 410;
const ID_ED_COMBO: i32 = 411;
const ID_ED_KEY_LABEL: i32 = 412;
const ID_ED_KEY: i32 = 413;
const ID_ED_KEY_CLEAR: i32 = 414;
const ID_ED_KEY_HELP: i32 = 415;
const ID_ED_OK: i32 = 416;
const ID_ED_CANCEL: i32 = 417;
const EDITOR: [i32; 8] = [
    ID_ED_LABEL,
    ID_ED_COMBO,
    ID_ED_KEY_LABEL,
    ID_ED_KEY,
    ID_ED_KEY_CLEAR,
    ID_ED_KEY_HELP,
    ID_ED_OK,
    ID_ED_CANCEL,
];
const ID_BAR_TEXT: i32 = 420;
const ID_BAR_DISCARD: i32 = 421;
const ID_BAR_SAVE: i32 = 422;
const ID_SAVE_STATUS: i32 = 423;
const ID_WRITE_LOCK: i32 = 424;
const ID_PROFILE_RESTORE: i32 = 425;
const ID_FACTORY: i32 = 426;
const ID_RATE_TITLE: i32 = 330;
const ID_RATE_VALUE: i32 = 331;
const ID_RATE_RO: i32 = 332;
const ID_RATE_DESC: i32 = 333;
const ID_RST_TITLE: i32 = 340;
const ID_RST_DESC: i32 = 341;
const ID_RST_BTN: i32 = 342;
const ID_RST_STATUS: i32 = 343;
const ID_FOOTER: i32 = 350;

/// Proposed quick values (DESIGN.md); only those the mouse accepts are shown.
const PRESETS: [u16; 4] = [400, 800, 1600, 3200];
const MAX_COLUMN: f32 = 760.0;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Title,
    Name,
    Percent,
    Primary,
    Secondary,
    Tertiary,
    Notice,
    /// Body text in semibold (changed actions, pending bar).
    Strong,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Surface {
    App,
    Card,
    /// Row being edited (`surface.selected`).
    Selected,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text(Role, Surface),
    Button(ButtonStyle, Surface),
    Edit,
    Combo,
}

#[derive(Clone, Copy)]
struct Ctl {
    id: i32,
    hwnd: HWND,
    kind: Kind,
}

/// Outcome of the last request sent from this page.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Pending {
    Apply(u16),
    Restore,
    /// A write to the mouse's memory.
    Buttons(Write),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Write {
    Save,
    Original,
    Factory,
}

/// The row editor ("Cambiar").
#[derive(Clone)]
struct Editing {
    row: usize,
    choices: Vec<(String, Option<Action>)>,
    selected: usize,
    /// Captured shortcut (HID modifiers, usage).
    key: Option<(u8, u8)>,
}

pub(crate) struct MouseState {
    pub key: DeviceKey,
    ctls: Vec<Ctl>,
    cards: Vec<RECT>,
    notice: Option<RECT>,
    field: RECT,
    icon: (i32, i32, f32),
    battery: Option<(i32, i32)>,
    status_icon: Option<(i32, i32, bool)>,
    /// Field rejected on the last "Aplicar"; cleared when edited.
    invalid: bool,
    pending: Option<(u64, Pending)>,
    /// Last completed apply: (ok, value).
    applied: Option<(bool, u16)>,
    restore_text: Option<(bool, String)>,
    brushes: Option<(Palette, HBRUSH, HBRUSH, HBRUSH)>,
    /// Row separators of the buttons table: (x1, x2, y) in content px.
    separators: Vec<(i32, i32, i32)>,
    profile_requested: bool,
    /// Button changes not yet written: (index, new action), by index.
    edits: Vec<(usize, Action)>,
    editing: Option<Editing>,
    /// Outcome of the last write: (ok, text).
    buttons_result: Option<(bool, String)>,
    /// Painted outlines: "Cambiado" labels, the editor box, the key field.
    badges: Vec<RECT>,
    edit_box: Option<RECT>,
    key_field: Option<RECT>,
    save_icon: Option<(i32, i32, bool)>,
    /// Whether the combo box got the dark theme.
    dark_combo: Option<bool>,
}

// ---------------------------------------------------------------------------
// Pure logic of the DPI field (tested).

/// Parses the field; `None` for empty or non-numeric text.
pub fn parse_field(text: &str) -> Option<u16> {
    text.trim().parse().ok()
}

/// One step down (`-1`) or up (`+1`) from `from`, clamped to the range.
pub fn step(range: &DpiRange, from: u16, dir: i32) -> u16 {
    match range {
        DpiRange::Range { min, max, step } => {
            // Snap to the grid first, then move one step.
            let snapped = min + (from.clamp(*min, *max) - min) / step * step;
            let next = if dir > 0 {
                if snapped > from || from < *min {
                    snapped
                } else {
                    snapped.saturating_add(*step)
                }
            } else if snapped < from {
                snapped
            } else {
                snapped.saturating_sub(*step)
            };
            next.clamp(*min, *max)
        }
        DpiRange::Values(v) => {
            let pos = v
                .iter()
                .position(|&x| x >= from)
                .unwrap_or(v.len().saturating_sub(1));
            let i = if dir > 0 {
                if v.get(pos) == Some(&from) {
                    (pos + 1).min(v.len() - 1)
                } else {
                    pos
                }
            } else {
                pos.saturating_sub(1)
            };
            v.get(i).copied().unwrap_or(from)
        }
    }
}

pub fn bounds(range: &DpiRange) -> (u16, u16) {
    match range {
        DpiRange::Range { min, max, .. } => (*min, *max),
        DpiRange::Values(v) => (
            v.first().copied().unwrap_or(0),
            v.last().copied().unwrap_or(0),
        ),
    }
}

/// "Admite de 200 a 12000, en pasos de 50." / "Admite 400, 800 o 1600."
pub fn range_sentence(range: &DpiRange) -> String {
    match range {
        DpiRange::Range { min, max, step } => {
            format!("Puntos por pulgada del sensor. Admite de {min} a {max}, en pasos de {step}.")
        }
        DpiRange::Values(_) => format!(
            "Puntos por pulgada del sensor. Admite {}.",
            text::dpi_range(range)
        ),
    }
}

/// Error text for an invalid field, with the nearest valid values.
pub fn invalid_message(range: &DpiRange, typed: &str) -> String {
    let what = if typed.trim().is_empty() {
        "El campo está vacío".to_string()
    } else {
        format!("{} no es válido", typed.trim())
    };
    match (range, parse_field(typed)) {
        (DpiRange::Range { min, max, step: s }, Some(v)) if v >= *min && v <= *max => {
            let lo = min + (v - min) / s * s;
            format!(
                "Error: {what}. Usa un valor de {min} a {max} en pasos de {s} (por ejemplo, {lo} o {}).",
                (lo + s).min(*max)
            )
        }
        (DpiRange::Range { min, max, step: s }, _) => {
            format!("Error: {what}. Usa un valor de {min} a {max} en pasos de {s}.")
        }
        (DpiRange::Values(_), _) => format!(
            "Error: {what}. Valores admitidos: {}.",
            text::dpi_range(range)
        ),
    }
}

pub fn presets_for(range: &DpiRange) -> Vec<u16> {
    PRESETS.into_iter().filter(|&p| range.contains(p)).collect()
}

// ---------------------------------------------------------------------------
// Pure logic of the buttons editor (tested).

/// Button editing is shown once the write gate is open (`app::WRITES_ENABLED`)
/// and in debug builds, where the worker still refuses to write.
fn editing_available() -> bool {
    crate::app::WRITES_ENABLED || cfg!(debug_assertions)
}

/// Choices of the "Acción" selector. `None` is "Atajo de teclado…". Only
/// encodings observed on a real mouse are offered (no double click,
/// multimedia or "Desactivar" yet). A profile function the app cannot
/// write is kept as "Sin cambios".
pub fn choices(device: Action) -> Vec<(String, Option<Action>)> {
    let mut v: Vec<(String, Option<Action>)> = [0x01u16, 0x02, 0x04, 0x08, 0x10]
        .iter()
        .map(|&m| {
            let a = Action::Mouse(m);
            (format!("Clics y navegación: {}", text::action(a)), Some(a))
        })
        .collect();
    v.push(("Otras: Atajo de teclado…".into(), None));
    if !crate::protocol::onboard::assignable(device) && !matches!(device, Action::Keyboard { .. }) {
        v.push((
            format!("Sin cambios: {}", text::action(device)),
            Some(device),
        ));
    }
    v
}

/// Index of `current` in `choices` (shortcuts select "Atajo de teclado…").
pub fn choice_for(choices: &[(String, Option<Action>)], current: Action) -> usize {
    let wanted = match current {
        Action::Keyboard { .. } => None,
        a => Some(a),
    };
    choices.iter().position(|(_, a)| *a == wanted).unwrap_or(0)
}

/// Records `action` for `row`; choosing what the mouse already has
/// removes the pending change.
pub fn upsert(edits: &mut Vec<(usize, Action)>, row: usize, action: Action, device: Action) {
    edits.retain(|(r, _)| *r != row);
    if action != device {
        edits.push((row, action));
        edits.sort_by_key(|(r, _)| *r);
    }
}

/// HID usage of a Windows virtual key, for the keys `text::shortcut` names.
fn vk_usage(vk: u16) -> Option<u8> {
    Some(match vk {
        0x41..=0x5a => (vk - 0x41) as u8 + 0x04, // A-Z
        0x31..=0x39 => (vk - 0x31) as u8 + 0x1e, // 1-9
        0x30 => 0x27,
        0x0d => 0x28,                            // Enter
        0x08 => 0x2a,                            // Backspace
        0x20 => 0x2c,                            // Space
        0x70..=0x7b => (vk - 0x70) as u8 + 0x3a, // F1-F12
        0x2d => 0x49,                            // Insert
        0x24 => 0x4a,                            // Home
        0x21 => 0x4b,                            // Page up
        0x2e => 0x4c,                            // Delete
        0x23 => 0x4d,                            // End
        0x22 => 0x4e,                            // Page down
        0x27 => 0x4f,
        0x25 => 0x50,
        0x28 => 0x51,
        0x26 => 0x52,
        _ => return None,
    })
}

/// Shortcut captured from a key press: (HID modifiers, usage). Tab and Esc
/// are never captured (they leave the field); lone modifiers and keys
/// without a name are ignored.
pub fn capture(vk: u16, ctrl: bool, shift: bool, alt: bool, win: bool) -> Option<(u8, u8)> {
    if vk == VK_TAB.0 || vk == VK_ESCAPE.0 {
        return None;
    }
    let usage = vk_usage(vk)?;
    let mods = ctrl as u8 | (shift as u8) << 1 | (alt as u8) << 2 | (win as u8) << 3;
    Some((mods, usage))
}

fn plural(n: usize, one: &str, many: &str) -> String {
    format!("{n} {}", if n == 1 { one } else { many })
}

// ---------------------------------------------------------------------------
// Page lifetime.

fn create(view: HWND, id: i32, class: PCWSTR, text: &str, style: u32) -> HWND {
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
            Some(view),
            Some(HMENU(id as isize as *mut _)),
            Some(devices_view::instance()),
            None,
        )
        .unwrap_or_default()
    }
}

/// Replaces the device grid with this page for `key`.
pub(crate) fn enter(view: HWND, key: DeviceKey) {
    devices_view::clear_devices_page(view);
    let st = SS_NOPREFIX.0 | SS_LEFT.0;
    let btn = WS_TABSTOP.0 | (BS_OWNERDRAW | BS_NOTIFY) as u32;
    let mut ctls = Vec::new();
    let mut add = |id: i32, kind: Kind, class: PCWSTR, text: &str, style: u32| {
        let hwnd = create(view, id, class, text, style);
        ctls.push(Ctl { id, hwnd, kind });
    };
    let s = w!("STATIC");
    let b = w!("BUTTON");
    use Role::*;
    use Surface::*;
    add(
        ID_BACK,
        Kind::Button(ButtonStyle::Normal, App),
        b,
        "Volver a Dispositivos",
        btn,
    );
    add(ID_TITLE, Kind::Text(Title, App), s, "", st);
    add(ID_SUB, Kind::Text(Secondary, App), s, "", st);
    add(ID_PCT, Kind::Text(Percent, App), s, "", st);
    add(ID_PSTATE, Kind::Text(Secondary, App), s, "", st);
    add(ID_DISC, Kind::Text(Notice, App), s, "", st);
    add(ID_NOTICE, Kind::Text(Notice, App), s, "", st);
    add(
        ID_DPI_TITLE,
        Kind::Text(Name, Card),
        s,
        "Sensibilidad (DPI)",
        st,
    );
    add(ID_DPI_DESC, Kind::Text(Secondary, Card), s, "", st);
    add(ID_DPI_CURRENT, Kind::Text(Secondary, Card), s, "", st);
    // The label directly precedes the field: it becomes its accessible name.
    add(
        ID_DPI_LABEL,
        Kind::Text(Notice, Card),
        s,
        "Nuevo valor de DPI",
        st,
    );
    add(
        ID_DPI_EDIT,
        Kind::Edit,
        w!("EDIT"),
        "",
        WS_TABSTOP.0 | (ES_NUMBER | ES_CENTER | ES_AUTOHSCROLL) as u32,
    );
    add(
        ID_MINUS,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Bajar 50 DPI",
        btn,
    );
    add(
        ID_PLUS,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Subir 50 DPI",
        btn,
    );
    add(
        ID_APPLY,
        Kind::Button(ButtonStyle::Primary, Card),
        b,
        "Aplicar",
        btn,
    );
    add(
        ID_PRESETS_LABEL,
        Kind::Text(Secondary, Card),
        s,
        "Valores rápidos",
        st,
    );
    for i in 0..PRESETS.len() as i32 {
        add(
            ID_PRESET0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            "",
            btn,
        );
    }
    add(ID_DPI_STATUS, Kind::Text(Secondary, Card), s, "", st);
    add(ID_BTN_TITLE, Kind::Text(Name, Card), s, "Botones", st);
    add(
        ID_BTN_DESC,
        Kind::Text(Secondary, Card),
        s,
        "Se guardan en la memoria del ratón: funcionan aunque cierres OpenPeripheral y en otros PC.",
        st,
    );
    add(ID_BTN_STATUS, Kind::Text(Notice, Card), s, "", st);
    for i in 0..MAX_BUTTONS {
        add(ID_BTN_NAME0 + i, Kind::Text(Name, Card), s, "", st);
        add(ID_BTN_ACT0 + i, Kind::Text(Primary, Card), s, "", st);
        if i == 0 {
            add(
                ID_BTN_LOCK,
                Kind::Text(Tertiary, Card),
                s,
                "No se cambia",
                st,
            );
        }
        add(ID_BTN_BADGE0 + i, Kind::Text(Tertiary, Card), s, "", st);
        add(
            ID_BTN_CHANGE0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            "Cambiar",
            btn,
        );
    }
    // The label directly precedes the selector and the key field: it
    // becomes their accessible name.
    add(ID_ED_LABEL, Kind::Text(Notice, Selected), s, "Acción", st);
    add(
        ID_ED_COMBO,
        Kind::Combo,
        w!("COMBOBOX"),
        "",
        WS_TABSTOP.0 | WS_VSCROLL.0 | CBS_DROPDOWNLIST as u32,
    );
    add(
        ID_ED_KEY_LABEL,
        Kind::Text(Notice, Selected),
        s,
        "Atajo de teclado",
        st,
    );
    add(
        ID_ED_KEY,
        Kind::Edit,
        w!("EDIT"),
        "",
        WS_TABSTOP.0 | (ES_READONLY | ES_CENTER | ES_AUTOHSCROLL) as u32,
    );
    add(
        ID_ED_KEY_CLEAR,
        Kind::Button(ButtonStyle::Normal, Selected),
        b,
        "Borrar el atajo",
        btn,
    );
    add(
        ID_ED_KEY_HELP,
        Kind::Text(Secondary, Selected),
        s,
        "Con el campo seleccionado, pulsa la combinación. Tab y Esc no se capturan: sirven para salir del campo.",
        st,
    );
    add(
        ID_ED_OK,
        Kind::Button(ButtonStyle::Primary, Selected),
        b,
        "Aceptar",
        btn,
    );
    add(
        ID_ED_CANCEL,
        Kind::Button(ButtonStyle::Normal, Selected),
        b,
        "Cancelar",
        btn,
    );
    add(ID_BAR_TEXT, Kind::Text(Strong, Card), s, "", st);
    add(
        ID_BAR_DISCARD,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Descartar",
        btn,
    );
    add(
        ID_BAR_SAVE,
        Kind::Button(ButtonStyle::Primary, Card),
        b,
        "Guardar en el ratón…",
        btn,
    );
    add(ID_SAVE_STATUS, Kind::Text(Secondary, Card), s, "", st);
    add(ID_BTN_BACKUP, Kind::Text(Secondary, Card), s, "", st);
    add(ID_WRITE_LOCK, Kind::Text(Notice, Card), s, "", st);
    add(
        ID_PROFILE_RESTORE,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Restaurar perfil original…",
        btn,
    );
    add(
        ID_FACTORY,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Valores de fábrica…",
        btn,
    );
    add(
        ID_RATE_TITLE,
        Kind::Text(Name, Card),
        s,
        "Tasa de sondeo",
        st,
    );
    add(ID_RATE_VALUE, Kind::Text(Percent, Card), s, "", st);
    add(
        ID_RATE_RO,
        Kind::Text(Secondary, Card),
        s,
        "· solo lectura",
        st,
    );
    add(
        ID_RATE_DESC,
        Kind::Text(Secondary, Card),
        s,
        "No se puede cambiar mientras el ratón usa sus perfiles integrados.",
        st,
    );
    add(ID_RST_TITLE, Kind::Text(Name, Card), s, "Restaurar", st);
    add(ID_RST_DESC, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_RST_BTN,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Restaurar originales",
        btn,
    );
    add(ID_RST_STATUS, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_FOOTER,
        Kind::Text(Tertiary, App),
        s,
        "Los cambios de DPI no se guardan en la memoria del ratón: OpenPeripheral los vuelve a aplicar al reconectarlo.",
        st,
    );
    unsafe {
        let edit = ctls
            .iter()
            .find(|c| c.id == ID_DPI_EDIT)
            .map(|c| c.hwnd)
            .unwrap_or_default();
        SendMessageW(edit, EM_SETLIMITTEXT, Some(WPARAM(5)), None);
        if let Some(key) = ctls.iter().find(|c| c.id == ID_ED_KEY) {
            let _ = SetWindowSubclass(key.hwnd, Some(capture_proc), 1, 0);
        }
        // Hidden until needed: hidden controls are skipped by Tab.
        for c in &ctls {
            let hidden = EDITOR.contains(&c.id)
                || (ID_BTN_CHANGE0..ID_BTN_CHANGE0 + MAX_BUTTONS).contains(&c.id)
                || [ID_BAR_DISCARD, ID_BAR_SAVE, ID_PROFILE_RESTORE, ID_FACTORY].contains(&c.id);
            if hidden {
                let _ = ShowWindow(c.hwnd, SW_HIDE);
            }
        }
    }
    let state = MouseState {
        key,
        ctls,
        cards: Vec::new(),
        notice: None,
        field: RECT::default(),
        icon: (0, 0, 40.0),
        battery: None,
        status_icon: None,
        invalid: false,
        pending: None,
        applied: None,
        restore_text: None,
        brushes: None,
        separators: Vec::new(),
        profile_requested: false,
        edits: Vec::new(),
        editing: None,
        buttons_result: None,
        badges: Vec::new(),
        edit_box: None,
        key_field: None,
        save_icon: None,
        dark_combo: None,
    };
    with_app(|a| {
        a.view.mouse = Some(state);
        a.view.scroll = 0;
    });
    fill_field_from_device(view);
    refresh(view);
    if let Some(b) = ctl(ID_BACK) {
        unsafe {
            let _ = SetFocus(Some(b));
        }
    }
}

/// Back to the device grid, focus on this mouse's "Ajustes" button.
pub(crate) fn leave(view: HWND) {
    let Some(state) = with_app(|a| a.view.mouse.take()).flatten() else {
        return;
    };
    for c in &state.ctls {
        unsafe {
            let _ = DestroyWindow(c.hwnd);
        }
    }
    delete_brushes(&state);
    with_app(|a| a.view.scroll = 0);
    devices_view::restore_devices_page(view, Some(&state.key));
}

fn delete_brushes(state: &MouseState) {
    if let Some((_, a, b, c)) = state.brushes {
        unsafe {
            let _ = DeleteObject(a.into());
            let _ = DeleteObject(b.into());
            let _ = DeleteObject(c.into());
        }
    }
}

/// Releases GDI objects when the window is destroyed with this page open.
pub(crate) fn destroyed() {
    if let Some(Some(state)) = with_app(|a| a.view.mouse.take()) {
        delete_brushes(&state);
    }
}

fn ctl(id: i32) -> Option<HWND> {
    with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .and_then(|m| m.ctls.iter().find(|c| c.id == id).map(|c| c.hwnd))
    })
    .flatten()
}

fn kind_of(h: HWND) -> Option<(i32, Kind)> {
    with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .and_then(|m| m.ctls.iter().find(|c| c.hwnd == h).map(|c| (c.id, c.kind)))
    })
    .flatten()
}

fn get_text(h: HWND) -> String {
    let len = unsafe { GetWindowTextLengthW(h) }.max(0) as usize;
    let mut buf = vec![0u16; len + 1];
    let n = unsafe { GetWindowTextW(h, &mut buf) }.max(0) as usize;
    String::from_utf16_lossy(&buf[..n])
}

fn set(id: i32, text: &str) {
    if let Some(h) = ctl(id) {
        devices_view::set_text_if_changed(h, text);
    }
}

fn show(id: i32, on: bool) {
    if let Some(h) = ctl(id) {
        unsafe {
            let _ = ShowWindow(h, if on { SW_SHOWNA } else { SW_HIDE });
        }
    }
}

fn enable(id: i32, on: bool) {
    if let Some(h) = ctl(id) {
        unsafe {
            let _ = EnableWindow(h, on);
        }
    }
}

fn snapshot<'a>(shared: &'a Shared, key: &DeviceKey) -> Option<&'a Snapshot> {
    shared.devices.iter().find(|d| &d.info.key == key)
}

fn current_dpi(snap: &Snapshot) -> Option<(u16, DpiRange)> {
    match &snap.status {
        Status::Ok(s) => s.dpi.as_ref().map(|d| (d.current, d.allowed.clone())),
        _ => None,
    }
}

fn fill_field_from_device(view: HWND) {
    let _ = view;
    let Some(shared) = super::snapshot() else {
        return;
    };
    let Some(key) = with_app(|a| a.view.mouse.as_ref().map(|m| m.key.clone())).flatten() else {
        return;
    };
    if let Some((cur, _)) = snapshot(&shared, &key).and_then(current_dpi) {
        set(ID_DPI_EDIT, &cur.to_string());
    }
}

// ---------------------------------------------------------------------------
// Refresh: texts, enabled states and statuses from the shared state.

pub(crate) fn refresh(view: HWND) {
    let Some(shared) = super::snapshot() else {
        return;
    };
    let Some((key, invalid, pending, applied, restore_text)) = with_app(|a| {
        a.view.mouse.as_ref().map(|m| {
            (
                m.key.clone(),
                m.invalid,
                m.pending,
                m.applied,
                m.restore_text.clone(),
            )
        })
    })
    .flatten() else {
        return;
    };
    ensure_brushes();
    let snap = snapshot(&shared, &key);
    let connected = snap.is_some_and(|s| matches!(s.status, Status::Ok(_)));

    // A request finished: the worker published a newer message.
    let (mut pending, mut applied, mut restore_text) = (pending, applied, restore_text);
    if let Some((seq, what)) = pending {
        if shared.seq != seq {
            let ok = shared.message.as_ref().is_some_and(|m| m.ok);
            match what {
                Pending::Apply(v) => applied = Some((ok, v)),
                Pending::Restore => {
                    restore_text = shared.message.as_ref().map(|m| (m.ok, m.text.clone()));
                    fill_field_from_device(view);
                }
                Pending::Buttons(what) => finish_write(what, shared.message.as_ref()),
            }
            pending = None;
        }
    }
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.pending = pending;
            m.applied = applied;
            m.restore_text = restore_text.clone();
        }
    });

    // Header.
    let name = snap.map(|s| s.info.name.clone()).unwrap_or_default();
    set(ID_TITLE, &name);
    let sub = snap
        .map(|s| format!("{} · {}", text::kind(s.info.kind), text::card_status(s)))
        .unwrap_or_default();
    set(ID_SUB, &sub);
    let energy = snap.map(text::energy).unwrap_or(Energy::Unknown);
    let (pct, pstate) = match energy {
        Energy::Battery {
            percent,
            charge,
            stale,
            ..
        } => {
            let state = if stale {
                "último valor conocido".to_string()
            } else if text::is_low(percent) {
                format!("batería baja · {}", text::charging(charge))
            } else {
                text::charging(charge).to_string()
            };
            (format!("{percent} %"), state)
        }
        Energy::Wired => ("Con cable".into(), "sin batería".into()),
        Energy::BatteryError => (String::new(), "Error de batería".into()),
        Energy::Unknown => (String::new(), String::new()),
    };
    set(ID_PCT, &pct);
    set(ID_PSTATE, &pstate);
    set(
        ID_DISC,
        if connected {
            ""
        } else {
            "Conecta o enciende el ratón para cambiar sus ajustes. Los controles se activarán solos cuando vuelva."
        },
    );
    let notice = text::conflicts(&shared.conflicts);
    set(ID_NOTICE, &notice);
    devices_view::set_text_if_changed(view, &format!("OpenPeripheral — Ajustes de {name}"));

    // DPI section.
    let dpi = snap.and_then(|s| match &s.status {
        Status::Ok(st) => st.dpi.clone(),
        _ => None,
    });
    let typed = ctl(ID_DPI_EDIT).map(get_text).unwrap_or_default();
    let field = parse_field(&typed);
    let busy = pending.is_some();
    if let Some(d) = &dpi {
        set(ID_DPI_DESC, &range_sentence(&d.allowed));
        set(ID_DPI_CURRENT, &format!("Actual: {} DPI", d.current));
        let (lo, hi) = bounds(&d.allowed);
        let base = field.unwrap_or(d.current);
        let presets = presets_for(&d.allowed);
        for i in 0..PRESETS.len() {
            let id = ID_PRESET0 + i as i32;
            match presets.get(i) {
                Some(p) => {
                    let chosen = field == Some(*p);
                    set(
                        id,
                        &if chosen {
                            format!("{p} DPI, elegido")
                        } else {
                            format!("{p} DPI")
                        },
                    );
                    show(id, true);
                    set_style(
                        id,
                        if chosen {
                            ButtonStyle::Selected
                        } else {
                            ButtonStyle::Normal
                        },
                    );
                    enable(id, connected && !busy);
                }
                None => show(id, false),
            }
        }
        let can = connected && !busy;
        enable(ID_DPI_EDIT, can);
        enable(ID_MINUS, can && base > lo);
        enable(ID_PLUS, can && base < hi);
        set(
            ID_MINUS,
            if base > lo {
                "Bajar un paso de DPI"
            } else {
                "Bajar un paso de DPI (mínimo alcanzado)"
            },
        );
        set(
            ID_PLUS,
            if base < hi {
                "Subir un paso de DPI"
            } else {
                "Subir un paso de DPI (máximo alcanzado)"
            },
        );
        let failed = matches!(applied, Some((false, _)));
        set(ID_APPLY, if failed { "Reintentar" } else { "Aplicar" });
        enable(ID_APPLY, can);
        enable(ID_RST_BTN, can);
        let (status, icon) = if let Some((_, Pending::Apply(v))) = pending {
            (format!("Aplicando {v} DPI…"), None)
        } else if invalid {
            (invalid_message(&d.allowed, &typed), Some(false))
        } else if let Some((ok, v)) = applied.filter(|&(_, v)| field == Some(v)) {
            if ok {
                (
                    format!(
                        "DPI {v} aplicado y guardado. Se volverá a aplicar al reconectar el ratón."
                    ),
                    Some(true),
                )
            } else {
                (
                    format!(
                        "Error: el ratón no respondió; sigue en {} DPI. Muévelo para despertarlo y vuelve a intentarlo.",
                        d.current
                    ),
                    Some(false),
                )
            }
        } else if field.is_some_and(|v| v != d.current) {
            (
                format!(
                    "Pulsa «Aplicar» para enviar {} DPI al ratón.",
                    field.unwrap_or_default()
                ),
                None,
            )
        } else {
            (String::new(), None)
        };
        set(ID_DPI_STATUS, &status);
        with_app(|a| {
            if let Some(m) = a.view.mouse.as_mut() {
                m.status_icon = icon.map(|ok| (0, 0, ok));
            }
        });
    } else {
        for id in [ID_DPI_EDIT, ID_MINUS, ID_PLUS, ID_APPLY, ID_RST_BTN] {
            enable(id, false);
        }
        for i in 0..PRESETS.len() as i32 {
            enable(ID_PRESET0 + i, false);
        }
        set(ID_DPI_STATUS, "");
    }

    // Buttons.
    let profile = snap.map(|s| s.profile.clone()).unwrap_or_default();
    let requested = with_app(|a| a.view.mouse.as_ref().map(|m| m.profile_requested))
        .flatten()
        .unwrap_or(true);
    if connected && profile == Profile::NotRead && !requested {
        with_app(|a| {
            if let Some(m) = a.view.mouse.as_mut() {
                m.profile_requested = true;
            }
        });
        send(Command::ReadProfile(key.clone()));
    }
    let (status, buttons): (String, Vec<Action>) = match &profile {
        Profile::Buttons(b) => (String::new(), b.clone()),
        Profile::NotRead if connected => ("Leyendo los botones del ratón…".into(), Vec::new()),
        Profile::NotRead => (String::new(), Vec::new()),
        Profile::Unsupported => (
            "Este ratón todavía no permite cambiar sus botones desde OpenPeripheral.".into(),
            Vec::new(),
        ),
        Profile::Failed(e) => (
            format!("Error: no se pudieron leer los botones ({e})."),
            Vec::new(),
        ),
    };
    set(ID_BTN_STATUS, &status);
    let backup_date = match snap.and_then(|s| s.backup.clone()) {
        Some(Ok(date)) => Some(date),
        _ => None,
    };
    let editable = editing_available() && !buttons.is_empty() && backup_date.is_some();
    let ghub = shared.conflicts.contains(&"Logitech G HUB");
    let (edits, editing, result) = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| (m.edits.clone(), m.editing.clone(), m.buttons_result.clone()))
    })
    .flatten()
    .unwrap_or_default();
    let writing = matches!(pending, Some((_, Pending::Buttons(_))));
    let edit_row = editing.as_ref().map(|e| e.row);
    for i in 0..MAX_BUTTONS {
        let row = i as usize;
        let device = buttons.get(row).copied();
        let new = edits.iter().find(|(r, _)| *r == row).map(|(_, a)| *a);
        let name = device.map(|_| text::mouse_button(row)).unwrap_or_default();
        set(ID_BTN_NAME0 + i, &name);
        set(
            ID_BTN_ACT0 + i,
            &new.or(device).map(text::action).unwrap_or_default(),
        );
        set(
            ID_BTN_BADGE0 + i,
            &match (new, device) {
                (Some(_), Some(d)) => format!("Cambiado · antes: {}", text::action(d)),
                _ => String::new(),
            },
        );
        let surface = if edit_row == Some(row) {
            Surface::Selected
        } else {
            Surface::Card
        };
        set_kind(ID_BTN_NAME0 + i, Kind::Text(Role::Name, surface));
        set_kind(
            ID_BTN_ACT0 + i,
            Kind::Text(
                if new.is_some() {
                    Role::Strong
                } else {
                    Role::Primary
                },
                surface,
            ),
        );
        set_kind(ID_BTN_BADGE0 + i, Kind::Text(Role::Tertiary, surface));
        set(ID_BTN_CHANGE0 + i, &format!("Cambiar {name}"));
        show(
            ID_BTN_CHANGE0 + i,
            editable && row > 0 && device.is_some() && edit_row != Some(row),
        );
        enable(ID_BTN_CHANGE0 + i, connected && !busy && editing.is_none());
    }
    if edit_row == Some(0) {
        set_kind(ID_BTN_LOCK, Kind::Text(Role::Tertiary, Surface::Selected));
    }
    set(
        ID_BTN_LOCK,
        if buttons.is_empty() {
            ""
        } else {
            "No se cambia"
        },
    );

    // Row editor.
    let shortcut = editing
        .as_ref()
        .is_some_and(|e| e.choices.get(e.selected).is_some_and(|(_, a)| a.is_none()));
    for id in [ID_ED_LABEL, ID_ED_COMBO, ID_ED_OK, ID_ED_CANCEL] {
        show(id, editing.is_some());
    }
    for id in [ID_ED_KEY_LABEL, ID_ED_KEY, ID_ED_KEY_CLEAR, ID_ED_KEY_HELP] {
        show(id, shortcut);
    }
    if let Some(e) = &editing {
        set(
            ID_ED_KEY,
            &e.key.map(|(m, u)| text::shortcut(m, u)).unwrap_or_default(),
        );
        enable(ID_ED_KEY_CLEAR, e.key.is_some());
        enable(ID_ED_OK, !shortcut || e.key.is_some());
    }
    let dark = with_app(|a| a.view.palette == super::theme::DARK).unwrap_or(false);
    let themed = with_app(|a| a.view.mouse.as_ref().and_then(|m| m.dark_combo)).flatten();
    if themed != Some(dark) {
        if let Some(h) = ctl(ID_ED_COMBO) {
            unsafe {
                let _ = if dark {
                    SetWindowTheme(h, w!("DarkMode_CFD"), PCWSTR::null())
                } else {
                    SetWindowTheme(h, w!("Explorer"), PCWSTR::null())
                };
            }
        }
        with_app(|a| {
            if let Some(m) = a.view.mouse.as_mut() {
                m.dark_combo = Some(dark);
            }
        });
    }

    // Pending changes bar.
    let failed = matches!(result, Some((false, _)));
    if edits.is_empty() {
        set(ID_BAR_TEXT, "");
    } else {
        set(
            ID_BAR_TEXT,
            &format!(
                "{} sin guardar. Todavía no se han escrito en el ratón.",
                plural(edits.len(), "cambio", "cambios")
            ),
        );
    }
    show(ID_BAR_DISCARD, !edits.is_empty());
    show(ID_BAR_SAVE, !edits.is_empty());
    set(
        ID_BAR_SAVE,
        if failed {
            "Reintentar…"
        } else {
            "Guardar en el ratón…"
        },
    );
    enable(ID_BAR_DISCARD, !writing);
    enable(
        ID_BAR_SAVE,
        connected && !busy && !ghub && editing.is_none(),
    );
    let (save_text, save_icon) = if writing {
        (
            "Guardando en el ratón… No apagues ni desconectes el ratón hasta que termine."
                .to_string(),
            None,
        )
    } else {
        match result {
            Some((ok, t)) => (t, Some(ok)),
            None => (String::new(), None),
        }
    };
    set(ID_SAVE_STATUS, &save_text);
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.save_icon = save_icon.map(|ok| (0, 0, ok));
        }
    });

    let backup = match snap.and_then(|s| s.backup.clone()) {
        Some(Ok(date)) => format!(
            "Copia del perfil original guardada en este PC el {}.",
            crate::backup::display_date(&date)
        ),
        Some(Err(e)) => format!(
            "Error: no se pudo guardar la copia del perfil original ({e}). No se escribirá nada en el ratón."
        ),
        None => String::new(),
    };
    set(ID_BTN_BACKUP, &backup);
    set(
        ID_WRITE_LOCK,
        if editable && ghub {
            "Cierra G HUB para guardar los botones."
        } else {
            ""
        },
    );
    for id in [ID_PROFILE_RESTORE, ID_FACTORY] {
        show(id, editable);
        enable(id, connected && !busy && !ghub && editing.is_none());
    }
    keep_focus_visible();

    // Report rate and restore.
    let rate = snap.and_then(|s| match &s.status {
        Status::Ok(st) => st.report_rate.as_ref().map(|r| text::hz(r.current_ms)),
        _ => None,
    });
    set(
        ID_RATE_VALUE,
        &rate
            .map(|hz| format!("{hz} Hz"))
            .unwrap_or_else(|| "—".into()),
    );
    let original = snap.and_then(|s| s.original_dpi);
    set(
        ID_RST_DESC,
        &match original {
            Some(o) => format!(
                "Vuelve al DPI que tenía el ratón al detectarlo ({o}) y olvida los ajustes guardados."
            ),
            None => {
                "Vuelve al DPI que tenía el ratón al detectarlo y olvida los ajustes guardados."
                    .into()
            }
        },
    );
    let rst = match (pending, &restore_text) {
        (Some((_, Pending::Restore)), _) => "Restaurando…".to_string(),
        (_, Some((true, t))) => t.clone(),
        (_, Some((false, t))) => format!("Error: {t}"),
        _ => String::new(),
    };
    set(ID_RST_STATUS, &rst);

    devices_view::relayout(view);
    unsafe {
        let _ = RedrawWindow(
            Some(view),
            None,
            None,
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_ERASE,
        );
    }
}

fn set_style(id: i32, style: ButtonStyle) {
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            if let Some(c) = m.ctls.iter_mut().find(|c| c.id == id) {
                if let Kind::Button(_, s) = c.kind {
                    c.kind = Kind::Button(style, s);
                }
            }
        }
    });
}

fn set_kind(id: i32, kind: Kind) {
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            if let Some(c) = m.ctls.iter_mut().find(|c| c.id == id) {
                c.kind = kind;
            }
        }
    });
}

/// If the focused control was just hidden (e.g. the pending bar after a
/// save), moves the focus to the next sensible control.
fn keep_focus_visible() {
    let f = unsafe { GetFocus() };
    if kind_of(f).is_none() || unsafe { IsWindowVisible(f).as_bool() } {
        return;
    }
    let next = (1..MAX_BUTTONS)
        .map(|i| ID_BTN_CHANGE0 + i)
        .chain([ID_PROFILE_RESTORE, ID_BACK])
        .filter_map(ctl)
        .find(|&h| unsafe { IsWindowVisible(h).as_bool() && IsWindowEnabled(h).as_bool() });
    if let Some(h) = next {
        unsafe {
            let _ = SetFocus(Some(h));
        }
    }
}

/// Turns the worker's answer to a write into the section's status text.
fn finish_write(what: Write, message: Option<&crate::app::Message>) {
    let ok = message.is_some_and(|m| m.ok);
    let cause = message.map(|m| m.text.clone()).unwrap_or_default();
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            let text = match (what, ok) {
                (Write::Save, true) => "Botones guardados en el ratón. Se comprobó la escritura leyendo de nuevo el perfil.".to_string(),
                (Write::Original, true) => "Perfil original restaurado en el ratón. Se comprobó leyendo de nuevo el perfil.".to_string(),
                (Write::Factory, true) => "Botones de fábrica restaurados en el ratón. Se comprobó leyendo de nuevo el perfil.".to_string(),
                (Write::Save, false) => format!(
                    "Error: no se pudieron guardar los botones. {cause} Tus cambios siguen pendientes en esta pantalla."
                ),
                (_, false) => format!("Error: no se pudo restaurar el perfil. {cause}"),
            };
            if ok && what == Write::Save {
                m.edits.clear();
            }
            m.buttons_result = Some((ok, text));
        }
    });
}

fn ensure_brushes() {
    let Some(palette) = with_app(|a| a.view.palette) else {
        return;
    };
    let stale = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .and_then(|m| m.brushes)
            .map(|(p, ..)| p != palette)
    })
    .flatten();
    if stale == Some(false) {
        return;
    }
    let new = unsafe {
        (
            palette,
            CreateSolidBrush(COLORREF(palette.bg_app.colorref())),
            CreateSolidBrush(COLORREF(palette.surface_card.colorref())),
            CreateSolidBrush(COLORREF(palette.surface_selected.colorref())),
        )
    };
    let old = with_app(|a| a.view.mouse.as_mut().and_then(|m| m.brushes.replace(new))).flatten();
    if let Some((_, a, b, c)) = old {
        unsafe {
            let _ = DeleteObject(a.into());
            let _ = DeleteObject(b.into());
            let _ = DeleteObject(c.into());
        }
    }
}

// ---------------------------------------------------------------------------
// Layout.

fn font_for(fonts: &Fonts, role: Role, compact: bool) -> HFONT {
    match role {
        Role::Title if compact => fonts.title_compact,
        Role::Title => fonts.title,
        Role::Name => fonts.name,
        Role::Percent => fonts.percent,
        Role::Strong => fonts.body_strong,
        _ => fonts.body,
    }
}

/// Positions every control and records the painted surfaces; returns the
/// content height in px.
pub(crate) fn layout(view: HWND, g: &Geometry, scroll: i32) -> i32 {
    let Some((ctls, palette, fonts)) = with_app(|a| {
        (
            a.view.mouse.as_ref().map(|m| m.ctls.clone()),
            a.view.palette,
            a.view.fonts.clone(),
        )
    }) else {
        return 0;
    };
    let (Some(ctls), Some(fonts)) = (ctls, fonts) else {
        return 0;
    };
    let hdc = unsafe { GetDC(Some(view)) };
    let ctx = Ctx {
        hdc,
        fonts: &fonts,
        palette: &palette,
        scale: g.scale,
        compact: g.compact,
    };
    let px = |u: f32| ctx.px(u);
    let h_of = |id: i32| {
        ctls.iter()
            .find(|c| c.id == id)
            .map(|c| c.hwnd)
            .unwrap_or_default()
    };
    let txt = |id: i32| get_text(h_of(id));
    let role_font = |role: Role| match role {
        Role::Title | Role::Name => FontRole::Name,
        Role::Percent => FontRole::Percent,
        _ => FontRole::Body,
    };
    let measure = |id: i32, role: Role, width: i32| -> (i32, i32) {
        let t = txt(id);
        if t.is_empty() {
            return (0, 0);
        }
        // Title and semibold text: measure with the real font.
        if role == Role::Title || role == Role::Strong {
            unsafe {
                let old = SelectObject(hdc, font_for(&fonts, role, g.compact).into());
                let mut w: Vec<u16> = t.encode_utf16().collect();
                let mut r = RECT {
                    left: 0,
                    top: 0,
                    right: width.max(1),
                    bottom: 0,
                };
                DrawTextW(
                    hdc,
                    &mut w,
                    &mut r,
                    DT_CALCRECT | DT_WORDBREAK | DT_NOPREFIX,
                );
                SelectObject(hdc, old);
                return (r.right, r.bottom);
            }
        }
        ctx.measure(&t, role_font(role), width, 0)
    };
    let mut pos: Vec<(i32, RECT)> = Vec::new();
    let mut put = |id: i32, x: i32, y: i32, w: i32, h: i32| {
        pos.push((
            id,
            RECT {
                left: x,
                top: y,
                right: x + w,
                bottom: y + h,
            },
        ))
    };
    // Owner-drawn buttons reserve 4 px around for the focus ring.
    let ring = px(4.0);
    let col = g.avail.min(px(MAX_COLUMN));
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(view, &mut rc);
    }
    let left = (rc.right - col) / 2;
    let bh = px(if g.compact { 44.0 } else { 36.0 });
    let btn_w = |label: &str| panel::button_width(&ctx, label);
    let mut cards = Vec::new();
    let mut y = px(if g.compact { 16.0 } else { 24.0 });

    // Back button.
    let back_w = btn_w("‹  Dispositivos");
    put(
        ID_BACK,
        left - ring,
        y - ring,
        back_w + 2 * ring,
        bh + 2 * ring,
    );
    y += bh + px(20.0);

    // Header.
    let icon = px(if g.compact { 32.0 } else { 40.0 });
    let icon_y = y;
    let (pw, ph) = measure(ID_PCT, Role::Percent, col);
    let (sw, sh) = measure(ID_PSTATE, Role::Secondary, col);
    let has_energy = pw > 0 || sw > 0;
    let energy_w = if has_energy {
        px(50.0) + pw + px(8.0) + sw
    } else {
        0
    };
    let tx = left + icon + px(if g.compact { 12.0 } else { 20.0 });
    let title_w = if g.compact || !has_energy {
        left + col - tx
    } else {
        left + col - tx - energy_w - px(20.0)
    };
    let (_, th) = measure(ID_TITLE, Role::Title, title_w);
    put(ID_TITLE, tx, y, title_w, th);
    let (_, subh) = measure(ID_SUB, Role::Secondary, title_w);
    put(ID_SUB, tx, y + th + px(2.0), title_w, subh);
    let header_h = icon.max(th + px(2.0) + subh);
    let mut battery = None;
    let energy_row =
        |bx: i32, by: i32, row_h: i32, put: &mut dyn FnMut(i32, i32, i32, i32, i32)| {
            let gy = by + (row_h - px(18.0)) / 2;
            put(ID_PCT, bx + px(50.0), by + (row_h - ph) / 2, pw + 2, ph);
            put(
                ID_PSTATE,
                bx + px(50.0) + pw + px(8.0),
                by + (row_h - sh) / 2,
                sw + 2,
                sh,
            );
            (bx, gy)
        };
    if g.compact {
        y += header_h + px(10.0);
        if has_energy {
            battery = Some(energy_row(left, y, px(28.0), &mut put));
            y += px(28.0);
        }
    } else {
        if has_energy {
            battery = Some(energy_row(left + col - energy_w, y, header_h, &mut put));
        }
        y += header_h;
    }
    if !has_energy {
        put(ID_PCT, 0, 0, 0, 0);
        put(ID_PSTATE, 0, 0, 0, 0);
    }
    y += px(20.0);

    // Disconnected text.
    let (_, dh) = measure(ID_DISC, Role::Notice, col);
    if dh > 0 {
        put(ID_DISC, left, y, col, dh);
        y += dh + px(20.0);
    } else {
        put(ID_DISC, 0, 0, 0, 0);
    }

    // Conflict notice.
    let mut notice = None;
    let (_, nh) = measure(ID_NOTICE, Role::Notice, col - px(56.0));
    if nh > 0 {
        let box_h = nh.max(px(18.0)) + px(20.0);
        put(ID_NOTICE, left + px(42.0), y + px(10.0), col - px(56.0), nh);
        notice = Some(RECT {
            left,
            top: y,
            right: left + col,
            bottom: y + box_h,
        });
        y += box_h + px(20.0);
    } else {
        put(ID_NOTICE, 0, 0, 0, 0);
    }

    let pad = px(if g.compact { 16.0 } else { 20.0 });
    let inner = col - 2 * pad;
    let ix = left + pad;

    // DPI section.
    let top = y;
    let mut cy = y + pad;
    let (_, h) = measure(ID_DPI_TITLE, Role::Name, inner);
    put(ID_DPI_TITLE, ix, cy, inner, h);
    cy += h + px(4.0);
    let (_, h) = measure(ID_DPI_DESC, Role::Secondary, inner);
    put(ID_DPI_DESC, ix, cy, inner, h);
    cy += h + px(16.0);
    let (lw, lh) = measure(ID_DPI_LABEL, Role::Notice, inner);
    put(ID_DPI_LABEL, ix, cy, lw + 2, lh);
    cy += lh + px(6.0);
    let step_w = px(if g.compact { 44.0 } else { 36.0 });
    let field_w = if g.compact {
        inner - 2 * step_w - px(12.0)
    } else {
        px(96.0)
    };
    let field = RECT {
        left: ix + step_w + px(6.0),
        top: cy,
        right: ix + step_w + px(6.0) + field_w,
        bottom: cy + bh,
    };
    put(
        ID_MINUS,
        ix - ring,
        cy - ring,
        step_w + 2 * ring,
        bh + 2 * ring,
    );
    put(
        ID_PLUS,
        field.right + px(6.0) - ring,
        cy - ring,
        step_w + 2 * ring,
        bh + 2 * ring,
    );
    let edit_h = (bh - px(10.0)).max(px(20.0));
    put(
        ID_DPI_EDIT,
        field.left + px(4.0),
        cy + (bh - edit_h) / 2,
        field_w - px(8.0),
        edit_h,
    );
    let apply_w = btn_w("Reintentar");
    let (cw, ch) = measure(ID_DPI_CURRENT, Role::Secondary, inner);
    if g.compact {
        cy += bh + px(14.0);
    } else {
        let ax = field.right + px(6.0) + step_w + px(16.0);
        put(
            ID_APPLY,
            ax - ring,
            cy - ring,
            apply_w + 2 * ring,
            bh + 2 * ring,
        );
        put(
            ID_DPI_CURRENT,
            ax + apply_w + px(16.0),
            cy + (bh - ch) / 2,
            cw + 2,
            ch,
        );
        cy += bh + px(16.0);
    }
    // Quick values.
    let presets: Vec<i32> = (0..PRESETS.len() as i32)
        .map(|i| ID_PRESET0 + i)
        .filter(|&id| unsafe { IsWindowVisible(h_of(id)).as_bool() })
        .collect();
    let (plw, plh) = measure(ID_PRESETS_LABEL, Role::Secondary, inner);
    if !presets.is_empty() {
        if g.compact {
            put(ID_PRESETS_LABEL, ix, cy, plw + 2, plh);
            cy += plh + px(6.0);
            let n = presets.len() as i32;
            let gw = (inner - px(6.0) * (n - 1)) / n;
            for (i, id) in presets.iter().enumerate() {
                let x = ix + i as i32 * (gw + px(6.0));
                put(*id, x - ring, cy - ring, gw + 2 * ring, bh + 2 * ring);
            }
            cy += bh + px(14.0);
            put(
                ID_APPLY,
                ix - ring,
                cy - ring,
                inner + 2 * ring,
                bh + 2 * ring,
            );
            cy += bh + px(14.0);
            put(ID_DPI_CURRENT, ix, cy, cw + 2, ch);
            cy += ch + px(10.0);
        } else {
            let ph2 = px(32.0);
            put(ID_PRESETS_LABEL, ix, cy + (ph2 - plh) / 2, plw + 2, plh);
            let mut x = ix + plw + px(12.0);
            for id in &presets {
                let label = PRESETS[(*id - ID_PRESET0) as usize].to_string();
                let w = ctx.measure(&label, FontRole::Body, 1000, 1).0 + px(28.0);
                put(*id, x - ring, cy - ring, w + 2 * ring, ph2 + 2 * ring);
                x += w + px(8.0);
            }
            cy += ph2 + px(16.0);
        }
    } else {
        put(ID_PRESETS_LABEL, 0, 0, 0, 0);
        if g.compact {
            put(
                ID_APPLY,
                ix - ring,
                cy - ring,
                inner + 2 * ring,
                bh + 2 * ring,
            );
            cy += bh + px(14.0);
            put(ID_DPI_CURRENT, ix, cy, cw + 2, ch);
            cy += ch + px(10.0);
        }
    }
    let (_, sth) = measure(ID_DPI_STATUS, Role::Secondary, inner - px(24.0));
    let status_xy = (ix, cy);
    put(ID_DPI_STATUS, ix + px(24.0), cy, inner - px(24.0), sth);
    if sth > 0 {
        cy += sth;
    } else {
        cy -= px(16.0);
    }
    cy += pad;
    cards.push(RECT {
        left,
        top,
        right: left + col,
        bottom: cy,
    });
    y = cy + px(20.0);

    // Buttons.
    let top = y;
    let mut cy = y + pad;
    let (_, h) = measure(ID_BTN_TITLE, Role::Name, inner);
    put(ID_BTN_TITLE, ix, cy, inner, h);
    cy += h + px(4.0);
    let (_, h) = measure(ID_BTN_DESC, Role::Secondary, inner);
    put(ID_BTN_DESC, ix, cy, inner, h);
    cy += h + px(12.0);
    let (_, h) = measure(ID_BTN_STATUS, Role::Notice, inner);
    if h > 0 {
        put(ID_BTN_STATUS, ix, cy, inner, h);
        cy += h + px(12.0);
    } else {
        put(ID_BTN_STATUS, 0, 0, 0, 0);
    }
    let mut separators = Vec::new();
    let mut badges = Vec::new();
    let mut edit_box = None;
    let mut key_field = None;
    let visible = |id: i32| unsafe { IsWindowVisible(h_of(id)).as_bool() };
    let editing_row = (0..MAX_BUTTONS).find(|&i| {
        // The row being edited is the one whose "Cambiar" is hidden while
        // the editor is shown.
        visible(ID_ED_COMBO)
            && !visible(ID_BTN_CHANGE0 + i)
            && measure(ID_BTN_NAME0 + i, Role::Name, inner).1 > 0
            && i > 0
    });
    let change_w = btn_w("Cambiar");
    let (lock_w, lock_h) = measure(ID_BTN_LOCK, Role::Tertiary, inner);
    let right_w = change_w.max(lock_w);
    let name_w = if g.compact {
        inner - right_w - px(12.0)
    } else {
        px(180.0)
    };
    for i in 0..MAX_BUTTONS {
        let (_, nh) = measure(ID_BTN_NAME0 + i, Role::Name, name_w);
        if nh == 0 {
            for id in [
                ID_BTN_NAME0 + i,
                ID_BTN_ACT0 + i,
                ID_BTN_BADGE0 + i,
                ID_BTN_CHANGE0 + i,
            ] {
                put(id, 0, 0, 0, 0);
            }
            continue;
        }
        let row_pad = px(10.0);
        let row_top = cy;
        let (act_x, act_w) = if g.compact {
            (ix, inner)
        } else {
            let x = ix + name_w + px(12.0);
            (x, inner - name_w - px(12.0) - right_w - px(12.0))
        };
        let act_role = if get_text(h_of(ID_BTN_BADGE0 + i)).is_empty() {
            Role::Primary
        } else {
            Role::Strong
        };
        let (_, ah) = measure(ID_BTN_ACT0 + i, act_role, act_w);
        let (bw, bdh) = measure(ID_BTN_BADGE0 + i, Role::Tertiary, act_w - px(12.0));
        let badge_h = if bdh > 0 { bdh + px(4.0) } else { 0 };
        let mut y0 = cy + row_pad;
        if g.compact {
            let top_h = nh.max(bh);
            put(ID_BTN_NAME0 + i, ix, y0 + (top_h - nh) / 2, name_w, nh);
            let right_y = y0 + (top_h - bh) / 2;
            let rx = ix + inner - change_w;
            put(
                ID_BTN_CHANGE0 + i,
                rx - ring,
                right_y - ring,
                change_w + 2 * ring,
                bh + 2 * ring,
            );
            if i == 0 {
                put(
                    ID_BTN_LOCK,
                    ix + inner - lock_w,
                    y0 + (top_h - lock_h) / 2,
                    lock_w + 2,
                    lock_h,
                );
            }
            y0 += top_h + px(2.0);
            put(ID_BTN_ACT0 + i, act_x, y0, act_w, ah);
            y0 += ah;
        } else {
            let row_h = nh
                .max(ah + if badge_h > 0 { px(4.0) + badge_h } else { 0 })
                .max(bh);
            put(ID_BTN_NAME0 + i, ix, y0 + (row_h - nh) / 2, name_w, nh);
            let block = ah + if badge_h > 0 { px(4.0) + badge_h } else { 0 };
            let ay = y0 + (row_h - block) / 2;
            put(ID_BTN_ACT0 + i, act_x, ay, act_w, ah);
            let rx = ix + inner - change_w;
            put(
                ID_BTN_CHANGE0 + i,
                rx - ring,
                y0 + (row_h - bh) / 2 - ring,
                change_w + 2 * ring,
                bh + 2 * ring,
            );
            if i == 0 {
                put(
                    ID_BTN_LOCK,
                    ix + inner - lock_w,
                    y0 + (row_h - lock_h) / 2,
                    lock_w + 2,
                    lock_h,
                );
            }
            y0 = ay + ah;
            if badge_h == 0 {
                y0 = cy + row_pad + row_h;
            }
        }
        if bdh > 0 {
            y0 += px(4.0);
            let r = RECT {
                left: act_x,
                top: y0,
                right: act_x + bw + px(12.0),
                bottom: y0 + bdh + px(4.0),
            };
            put(
                ID_BTN_BADGE0 + i,
                r.left + px(6.0),
                r.top + px(2.0),
                bw + 2,
                bdh,
            );
            badges.push(r);
            y0 = r.bottom;
        } else {
            put(ID_BTN_BADGE0 + i, 0, 0, 0, 0);
        }
        cy = y0.max(cy + row_pad + bh) + row_pad;

        if editing_row == Some(i) {
            // Editor box: the row plus the selector and its buttons.
            let bx = ix + px(12.0);
            let bwid = inner - px(24.0);
            let mut ey = cy;
            let (lw, lh) = measure(ID_ED_LABEL, Role::Notice, bwid);
            put(ID_ED_LABEL, bx, ey, lw + 2, lh);
            ey += lh + px(6.0);
            let combo_w = bwid.min(px(360.0));
            // A drop-down list's height includes its open list.
            put(ID_ED_COMBO, bx, ey, combo_w, bh + px(240.0));
            ey += bh + px(12.0);
            if visible(ID_ED_KEY) {
                let (klw, klh) = measure(ID_ED_KEY_LABEL, Role::Notice, bwid);
                put(ID_ED_KEY_LABEL, bx, ey, klw + 2, klh);
                ey += klh + px(6.0);
                let clear_w = btn_w("Borrar");
                let fw = (bwid - clear_w - px(12.0)).min(px(240.0));
                let f = RECT {
                    left: bx,
                    top: ey,
                    right: bx + fw,
                    bottom: ey + bh,
                };
                let edit_h = (bh - px(10.0)).max(px(20.0));
                put(
                    ID_ED_KEY,
                    f.left + px(4.0),
                    ey + (bh - edit_h) / 2,
                    fw - px(8.0),
                    edit_h,
                );
                put(
                    ID_ED_KEY_CLEAR,
                    f.right + px(12.0) - ring,
                    ey - ring,
                    clear_w + 2 * ring,
                    bh + 2 * ring,
                );
                key_field = Some(f);
                ey += bh + px(6.0);
                let (_, hh) = measure(ID_ED_KEY_HELP, Role::Secondary, bwid);
                put(ID_ED_KEY_HELP, bx, ey, bwid, hh);
                ey += hh + px(12.0);
            } else {
                for id in [ID_ED_KEY_LABEL, ID_ED_KEY, ID_ED_KEY_CLEAR, ID_ED_KEY_HELP] {
                    put(id, 0, 0, 0, 0);
                }
            }
            let ok_w = btn_w("Aceptar");
            let cancel_w = btn_w("Cancelar");
            put(
                ID_ED_OK,
                bx - ring,
                ey - ring,
                ok_w + 2 * ring,
                bh + 2 * ring,
            );
            put(
                ID_ED_CANCEL,
                bx + ok_w + px(12.0) - ring,
                ey - ring,
                cancel_w + 2 * ring,
                bh + 2 * ring,
            );
            ey += bh + px(12.0);
            edit_box = Some(RECT {
                left: ix - px(8.0),
                top: row_top + px(2.0),
                right: ix + inner + px(8.0),
                bottom: ey,
            });
            cy = ey + px(4.0);
        }
        if i + 1 < MAX_BUTTONS
            && measure(ID_BTN_NAME0 + i + 1, Role::Name, name_w).1 > 0
            && editing_row != Some(i)
            && editing_row != Some(i + 1)
        {
            separators.push((ix, ix + inner, cy));
        }
    }
    if editing_row.is_none() {
        for id in EDITOR {
            put(id, 0, 0, 0, 0);
        }
    }
    if lock_h == 0 {
        put(ID_BTN_LOCK, 0, 0, 0, 0);
    }
    // Pending changes bar.
    let (_, bar_h) = measure(ID_BAR_TEXT, Role::Strong, inner);
    if bar_h > 0 && visible(ID_BAR_SAVE) {
        separators.push((ix, ix + inner, cy));
        cy += px(14.0);
        let save_w = btn_w("Guardar en el ratón…").max(btn_w("Reintentar…"));
        let discard_w = btn_w("Descartar");
        let text_w = if g.compact {
            inner
        } else {
            inner - save_w - discard_w - px(24.0)
        };
        let (_, th) = measure(ID_BAR_TEXT, Role::Strong, text_w);
        if g.compact {
            put(ID_BAR_TEXT, ix, cy, inner, th);
            cy += th + px(10.0);
            put(
                ID_BAR_DISCARD,
                ix - ring,
                cy - ring,
                discard_w + 2 * ring,
                bh + 2 * ring,
            );
            let sx = ix + discard_w + px(12.0);
            put(
                ID_BAR_SAVE,
                sx - ring,
                cy - ring,
                save_w + 2 * ring,
                bh + 2 * ring,
            );
            cy += bh;
        } else {
            let row_h = th.max(bh);
            put(ID_BAR_TEXT, ix, cy + (row_h - th) / 2, text_w, th);
            let sx = ix + inner - save_w;
            put(
                ID_BAR_SAVE,
                sx - ring,
                cy + (row_h - bh) / 2 - ring,
                save_w + 2 * ring,
                bh + 2 * ring,
            );
            let dx = sx - px(12.0) - discard_w;
            put(
                ID_BAR_DISCARD,
                dx - ring,
                cy + (row_h - bh) / 2 - ring,
                discard_w + 2 * ring,
                bh + 2 * ring,
            );
            cy += row_h;
        }
    } else {
        for id in [ID_BAR_TEXT, ID_BAR_DISCARD, ID_BAR_SAVE] {
            put(id, 0, 0, 0, 0);
        }
    }
    let mut save_xy = None;
    let (_, sth) = measure(ID_SAVE_STATUS, Role::Secondary, inner - px(24.0));
    if sth > 0 {
        cy += px(12.0);
        put(ID_SAVE_STATUS, ix + px(24.0), cy, inner - px(24.0), sth);
        save_xy = Some((ix, cy));
        cy += sth;
    } else {
        put(ID_SAVE_STATUS, 0, 0, 0, 0);
    }
    let (_, bkh) = measure(ID_BTN_BACKUP, Role::Secondary, inner);
    if bkh > 0 {
        separators.push((ix, ix + inner, cy + px(10.0)));
        cy += px(24.0);
        put(ID_BTN_BACKUP, ix, cy, inner, bkh);
        cy += bkh;
    } else {
        put(ID_BTN_BACKUP, 0, 0, 0, 0);
    }
    let (_, lkh) = measure(ID_WRITE_LOCK, Role::Notice, inner);
    if lkh > 0 {
        cy += px(8.0);
        put(ID_WRITE_LOCK, ix, cy, inner, lkh);
        cy += lkh;
    } else {
        put(ID_WRITE_LOCK, 0, 0, 0, 0);
    }
    if visible(ID_PROFILE_RESTORE) {
        cy += px(12.0);
        let rw = btn_w("Restaurar perfil original…");
        let fw = btn_w("Valores de fábrica…");
        if g.compact {
            put(
                ID_PROFILE_RESTORE,
                ix - ring,
                cy - ring,
                inner + 2 * ring,
                bh + 2 * ring,
            );
            cy += bh + px(10.0);
            put(
                ID_FACTORY,
                ix - ring,
                cy - ring,
                inner + 2 * ring,
                bh + 2 * ring,
            );
        } else {
            put(
                ID_PROFILE_RESTORE,
                ix - ring,
                cy - ring,
                rw + 2 * ring,
                bh + 2 * ring,
            );
            let fx = ix + rw + px(12.0);
            put(
                ID_FACTORY,
                fx - ring,
                cy - ring,
                fw + 2 * ring,
                bh + 2 * ring,
            );
        }
        cy += bh;
    } else {
        put(ID_PROFILE_RESTORE, 0, 0, 0, 0);
        put(ID_FACTORY, 0, 0, 0, 0);
    }
    cy += pad;
    cards.push(RECT {
        left,
        top,
        right: left + col,
        bottom: cy,
    });
    y = cy + px(20.0);

    // Report rate.
    let top = y;
    let mut cy = y + pad;
    let (_, h) = measure(ID_RATE_TITLE, Role::Name, inner);
    put(ID_RATE_TITLE, ix, cy, inner, h);
    cy += h + px(8.0);
    let (vw, vh) = measure(ID_RATE_VALUE, Role::Percent, inner);
    let (rw, rh) = measure(ID_RATE_RO, Role::Secondary, inner);
    put(ID_RATE_VALUE, ix, cy, vw + 2, vh);
    put(
        ID_RATE_RO,
        ix + vw + px(8.0),
        cy + (vh - rh) / 2 + px(2.0),
        rw + 2,
        rh,
    );
    cy += vh + px(8.0);
    let (_, h) = measure(ID_RATE_DESC, Role::Secondary, inner);
    put(ID_RATE_DESC, ix, cy, inner, h);
    cy += h + pad;
    cards.push(RECT {
        left,
        top,
        right: left + col,
        bottom: cy,
    });
    y = cy + px(20.0);

    // Restore.
    let top = y;
    let mut cy = y + pad;
    let rb_w = btn_w("Restaurar originales");
    let desc_w = if g.compact {
        inner
    } else {
        inner - rb_w - px(24.0)
    };
    let (_, h1) = measure(ID_RST_TITLE, Role::Name, desc_w);
    put(ID_RST_TITLE, ix, cy, desc_w, h1);
    let (_, h2) = measure(ID_RST_DESC, Role::Secondary, desc_w);
    put(ID_RST_DESC, ix, cy + h1 + px(4.0), desc_w, h2);
    let block = h1 + px(4.0) + h2;
    if g.compact {
        cy += block + px(10.0);
        put(
            ID_RST_BTN,
            ix - ring,
            cy - ring,
            inner + 2 * ring,
            bh + 2 * ring,
        );
        cy += bh;
    } else {
        let by = cy + (block - bh) / 2;
        put(
            ID_RST_BTN,
            left + col - pad - rb_w - ring,
            by - ring,
            rb_w + 2 * ring,
            bh + 2 * ring,
        );
        cy += block.max(bh);
    }
    let (_, h3) = measure(ID_RST_STATUS, Role::Secondary, inner);
    if h3 > 0 {
        cy += px(10.0);
        put(ID_RST_STATUS, ix, cy, inner, h3);
        cy += h3;
    } else {
        put(ID_RST_STATUS, 0, 0, 0, 0);
    }
    cy += pad;
    cards.push(RECT {
        left,
        top,
        right: left + col,
        bottom: cy,
    });
    y = cy + px(20.0);

    // Footer.
    let (_, h) = measure(ID_FOOTER, Role::Tertiary, col);
    put(ID_FOOTER, left, y, col, h);
    y += h + px(40.0);

    unsafe {
        ReleaseDC(Some(view), hdc);
    }

    // Apply positions (scrolled), fonts.
    for (id, r) in &pos {
        let h = h_of(*id);
        unsafe {
            let _ = SetWindowPos(
                h,
                None,
                r.left,
                r.top - scroll,
                r.right - r.left,
                r.bottom - r.top,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
        }
    }
    for c in &ctls {
        let font = match c.kind {
            Kind::Text(role, _) => font_for(&fonts, role, g.compact),
            Kind::Edit => fonts.strong,
            Kind::Button(..) | Kind::Combo => fonts.body,
        };
        unsafe {
            SendMessageW(
                c.hwnd,
                WM_SETFONT,
                Some(WPARAM(font.0 as usize)),
                Some(LPARAM(0)),
            );
        }
    }
    if let Some(c) = ctls.iter().find(|c| c.id == ID_ED_COMBO) {
        // Height of the closed selector: the same as a button.
        unsafe {
            SendMessageW(
                c.hwnd,
                CB_SETITEMHEIGHT,
                Some(WPARAM(usize::MAX)),
                Some(LPARAM((bh - px(6.0)) as isize)),
            );
        }
    }
    let status_icon_xy = status_xy;
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.cards = cards;
            m.notice = notice;
            m.field = field;
            m.icon = (left, icon_y, if g.compact { 32.0 } else { 40.0 });
            m.battery = battery;
            m.separators = separators;
            m.badges = badges;
            m.edit_box = edit_box;
            m.key_field = key_field;
            m.save_icon = match (m.save_icon, save_xy) {
                (Some((_, _, ok)), Some((x, y))) => Some((x, y, ok)),
                _ => None,
            };
            if let Some((_, _, ok)) = m.status_icon {
                m.status_icon = Some((status_icon_xy.0, status_icon_xy.1, ok));
            }
        }
    });
    y
}

// ---------------------------------------------------------------------------
// Painting, owner-drawn buttons and control colours.

/// Paints the page surfaces into `mem` (client-sized memory DC).
pub(crate) fn paint(mem: HDC, palette: &Palette, scale: f32, scroll: i32) {
    let Some((cards, notice, field, icon, invalid, status_icon, key, shared, kind)) =
        with_app(|a| {
            a.view.mouse.as_ref().map(|m| {
                let kind = a
                    .view
                    .shared
                    .devices
                    .iter()
                    .find(|d| d.info.key == m.key)
                    .map(|d| d.info.kind);
                (
                    m.cards.clone(),
                    m.notice,
                    m.field,
                    m.icon,
                    m.invalid,
                    m.status_icon,
                    m.key.clone(),
                    a.view.shared.clone(),
                    kind,
                )
            })
        })
        .flatten()
    else {
        return;
    };
    let Some(mut c) = Canvas::new(mem, scale) else {
        return;
    };
    let u = |px: i32| px as f32 / scale;
    for r in &cards {
        c.origin(r.left as f32, (r.top - scroll) as f32);
        let (w, h) = (u(r.right - r.left), u(r.bottom - r.top));
        c.fill_round_rect(0.0, 0.0, w, h, 10.0, palette.surface_card);
        c.stroke_round_rect(0.0, 0.0, w, h, 10.0, 1.0, palette.border_card, false);
    }
    if let Some(r) = notice {
        c.origin(r.left as f32, (r.top - scroll) as f32);
        c.stroke_round_rect(
            0.0,
            0.0,
            u(r.right - r.left),
            u(r.bottom - r.top),
            8.0,
            1.0,
            palette.border_notice,
            false,
        );
        glyph::notice_icon(&c, 14.0, 11.0, false, palette.text_notice);
    }
    let separators = with_app(|a| a.view.mouse.as_ref().map(|m| m.separators.clone()))
        .flatten()
        .unwrap_or_default();
    for (x1, x2, y) in separators {
        c.origin(x1 as f32, (y - scroll) as f32);
        c.fill_round_rect(0.0, 0.0, u(x2 - x1), 1.0, 0.0, palette.border_card);
    }
    let (badges, edit_box, key_field, save_icon) = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| (m.badges.clone(), m.edit_box, m.key_field, m.save_icon))
    })
    .flatten()
    .unwrap_or_default();
    if let Some(r) = edit_box {
        c.origin(r.left as f32, (r.top - scroll) as f32);
        let (w, h) = (u(r.right - r.left), u(r.bottom - r.top));
        c.fill_round_rect(0.0, 0.0, w, h, 8.0, palette.surface_selected);
        c.stroke_round_rect(0.0, 0.0, w, h, 8.0, 1.0, palette.border_control, false);
    }
    for r in badges.iter().chain(key_field.iter()) {
        c.origin(r.left as f32, (r.top - scroll) as f32);
        let (w, h) = (u(r.right - r.left), u(r.bottom - r.top));
        if Some(*r) == key_field {
            c.fill_round_rect(0.0, 0.0, w, h, 6.0, palette.bg_app);
        }
        c.stroke_round_rect(
            0.0,
            0.0,
            w,
            h,
            if Some(*r) == key_field { 6.0 } else { 4.0 },
            1.0,
            palette.border_control,
            false,
        );
    }
    if let Some((sx, sy, ok)) = save_icon {
        c.origin(sx as f32, (sy - scroll) as f32 + scale);
        if ok {
            glyph::check(&c, 0.0, 0.0, palette.text_primary);
        } else {
            glyph::notice_icon(&c, 0.0, 0.0, true, palette.text_primary);
        }
    }
    // Field: rounded box behind the EDIT; 2 px border when invalid.
    if field.right > field.left {
        c.origin(field.left as f32, (field.top - scroll) as f32);
        let (w, h) = (u(field.right - field.left), u(field.bottom - field.top));
        c.fill_round_rect(0.0, 0.0, w, h, 6.0, palette.bg_app);
        if invalid {
            c.stroke_round_rect(0.0, 0.0, w, h, 6.0, 2.0, palette.text_primary, false);
        } else {
            c.stroke_round_rect(0.0, 0.0, w, h, 6.0, 1.0, palette.border_control, false);
        }
    }
    let snap = shared.devices.iter().find(|d| d.info.key == key);
    let connected = snap.is_some_and(|s| matches!(s.status, Status::Ok(_)));
    if let Some(kind) = kind {
        c.origin(icon.0 as f32, (icon.1 - scroll) as f32);
        let color = if connected {
            palette.text_primary
        } else {
            palette.text_tertiary
        };
        glyph::kind_icon(&mut c, 0.0, 0.0, icon.2, kind, color);
    }
    if let Some((bx, by)) = with_app(|a| a.view.mouse.as_ref().and_then(|m| m.battery)).flatten() {
        c.origin(bx as f32, (by - scroll) as f32);
        match snap.map(text::energy) {
            Some(Energy::Battery { bars, stale, .. }) => {
                glyph::battery(&c, 0.0, 0.0, bars, stale, palette)
            }
            Some(Energy::Wired) => glyph::cable(&c, 0.0, 0.0, palette.battery_outline),
            _ => {}
        }
    }
    if let Some((sx, sy, ok)) = status_icon {
        c.origin(sx as f32, (sy - scroll) as f32 + scale);
        if ok {
            glyph::check(&c, 0.0, 0.0, palette.text_primary);
        } else {
            glyph::notice_icon(&c, 0.0, 0.0, true, palette.text_primary);
        }
    }
}

pub(crate) fn draw_item(view: HWND, dis: &DRAWITEMSTRUCT) {
    let Some((id, kind)) = kind_of(dis.hwndItem) else {
        return;
    };
    let Some((palette, fonts)) = with_app(|a| (a.view.palette, a.view.fonts.clone())) else {
        return;
    };
    let Some(fonts) = fonts else { return };
    let Kind::Button(style, surface) = kind else {
        return;
    };
    let bg = match surface {
        Surface::App => palette.bg_app,
        Surface::Card => palette.surface_card,
        Surface::Selected => palette.surface_selected,
    };
    let label = match id {
        c if (ID_BTN_CHANGE0..ID_BTN_CHANGE0 + MAX_BUTTONS).contains(&c) => "Cambiar".into(),
        ID_ED_KEY_CLEAR => "Borrar".into(),
        ID_BACK => "‹  Dispositivos".to_string(),
        ID_MINUS => "−".into(),
        ID_PLUS => "+".into(),
        ID_APPLY | ID_RST_BTN => get_text(dis.hwndItem),
        p if (ID_PRESET0..ID_PRESET0 + PRESETS.len() as i32).contains(&p) => {
            PRESETS[(p - ID_PRESET0) as usize].to_string()
        }
        _ => get_text(dis.hwndItem),
    };
    panel::draw_button(
        dis,
        &label,
        bg,
        &palette,
        &fonts,
        devices_view::scale_of(view),
        style,
    );
}

/// WM_CTLCOLORSTATIC / WM_CTLCOLOREDIT for this page's controls.
pub(crate) fn ctl_color(hdc: HDC, child: HWND) -> Option<LRESULT> {
    let (_, kind) = kind_of(child)?;
    let (palette, brushes) = with_app(|a| {
        (
            a.view.palette,
            a.view.mouse.as_ref().and_then(|m| m.brushes),
        )
    })?;
    let (_, app, card, selected) = brushes?;
    let (fg, bg, brush): (Color, Color, HBRUSH) = match kind {
        Kind::Text(role, surface) => {
            let fg = match role {
                Role::Secondary => palette.text_secondary,
                Role::Tertiary => palette.text_tertiary,
                Role::Notice => palette.text_notice,
                _ => palette.text_primary,
            };
            match surface {
                Surface::App => (fg, palette.bg_app, app),
                Surface::Card => (fg, palette.surface_card, card),
                Surface::Selected => (fg, palette.surface_selected, selected),
            }
        }
        Kind::Combo => (palette.text_primary, palette.bg_app, app),
        Kind::Edit => {
            let enabled = unsafe { IsWindowEnabled(child).as_bool() };
            (
                if enabled {
                    palette.text_primary
                } else {
                    palette.text_disabled
                },
                palette.bg_app,
                app,
            )
        }
        // Owner-drawn buttons: erase with their surface, never the default white.
        Kind::Button(_, surface) => {
            let brush = match surface {
                Surface::App => app,
                Surface::Card => card,
                Surface::Selected => selected,
            };
            return Some(LRESULT(brush.0 as isize));
        }
    };
    unsafe {
        SetTextColor(hdc, COLORREF(fg.colorref()));
        SetBkColor(hdc, COLORREF(bg.colorref()));
    }
    Some(LRESULT(brush.0 as isize))
}

/// WM_CTLCOLORLISTBOX: the open list of the "Acción" selector.
pub(crate) fn list_color(hdc: HDC) -> Option<LRESULT> {
    let (palette, brushes) = with_app(|a| {
        (
            a.view.palette,
            a.view.mouse.as_ref().and_then(|m| m.brushes),
        )
    })?;
    let (_, app, ..) = brushes?;
    unsafe {
        SetTextColor(hdc, COLORREF(palette.text_primary.colorref()));
        SetBkColor(hdc, COLORREF(palette.bg_app.colorref()));
    }
    Some(LRESULT(app.0 as isize))
}

// ---------------------------------------------------------------------------
// Shortcut capture field: a read-only EDIT that turns key presses into a
// shortcut. Tab and Esc keep their usual meaning.

extern "system" fn capture_proc(
    hwnd: HWND,
    msg: u32,
    wparam: WPARAM,
    lparam: LPARAM,
    _id: usize,
    _data: usize,
) -> LRESULT {
    let passes = |vk: usize| vk == VK_TAB.0 as usize || vk == VK_ESCAPE.0 as usize;
    unsafe {
        match msg {
            WM_GETDLGCODE => {
                let def = DefSubclassProc(hwnd, msg, wparam, lparam);
                let m = lparam.0 as *const MSG;
                if !m.is_null()
                    && matches!((*m).message, WM_KEYDOWN | WM_SYSKEYDOWN)
                    && !passes((*m).wParam.0)
                {
                    return LRESULT(def.0 | DLGC_WANTALLKEYS as isize);
                }
                def
            }
            WM_KEYDOWN | WM_SYSKEYDOWN if !passes(wparam.0) => {
                let down = |k: VIRTUAL_KEY| GetKeyState(k.0 as i32) < 0;
                if let Some((mods, usage)) = capture(
                    wparam.0 as u16,
                    down(VK_CONTROL),
                    down(VK_SHIFT),
                    down(VK_MENU),
                    down(VK_LWIN) || down(VK_RWIN),
                ) {
                    captured(mods, usage);
                }
                LRESULT(0)
            }
            WM_CHAR | WM_SYSCHAR | WM_KEYUP | WM_SYSKEYUP
                if !passes(wparam.0) && wparam.0 != 0x1b && wparam.0 != 0x09 =>
            {
                LRESULT(0)
            }
            WM_NCDESTROY => {
                let _ = RemoveWindowSubclass(hwnd, Some(capture_proc), 1);
                DefSubclassProc(hwnd, msg, wparam, lparam)
            }
            _ => DefSubclassProc(hwnd, msg, wparam, lparam),
        }
    }
}

fn captured(mods: u8, usage: u8) {
    with_app(|a| {
        if let Some(e) = a.view.mouse.as_mut().and_then(|m| m.editing.as_mut()) {
            e.key = Some((mods, usage));
        }
    });
    if let Some(view) = devices_view::hwnd() {
        refresh(view);
    }
}

// ---------------------------------------------------------------------------
// Button editing commands.

fn device_buttons() -> Vec<Action> {
    let Some(shared) = super::snapshot() else {
        return Vec::new();
    };
    let Some(key) = with_app(|a| a.view.mouse.as_ref().map(|m| m.key.clone())).flatten() else {
        return Vec::new();
    };
    match snapshot(&shared, &key).map(|s| &s.profile) {
        Some(Profile::Buttons(b)) => b.clone(),
        _ => Vec::new(),
    }
}

/// Opens the editor on `row`, right after it in tab order.
fn begin_edit(view: HWND, row: usize) {
    let buttons = device_buttons();
    let Some(&device) = buttons.get(row) else {
        return;
    };
    let current = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .and_then(|m| m.edits.iter().find(|(r, _)| *r == row).map(|(_, a)| *a))
    })
    .flatten()
    .unwrap_or(device);
    let list = choices(device);
    let selected = choice_for(&list, current);
    let key = match current {
        Action::Keyboard { modifiers, usage } => Some((modifiers, usage)),
        _ => None,
    };
    if let Some(combo) = ctl(ID_ED_COMBO) {
        unsafe {
            SendMessageW(combo, CB_RESETCONTENT, None, None);
            for (label, _) in &list {
                let w = wide(label);
                SendMessageW(combo, CB_ADDSTRING, None, Some(LPARAM(w.as_ptr() as isize)));
            }
            SendMessageW(combo, CB_SETCURSEL, Some(WPARAM(selected)), None);
        }
    }
    // Tab order follows z-order: put the editor right after the row.
    let mut after = ctl(ID_BTN_BADGE0 + row as i32);
    for id in EDITOR {
        if let (Some(prev), Some(h)) = (after, ctl(id)) {
            unsafe {
                let _ = SetWindowPos(
                    h,
                    Some(prev),
                    0,
                    0,
                    0,
                    0,
                    SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE,
                );
            }
            after = Some(h);
        }
    }
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.editing = Some(Editing {
                row,
                choices: list,
                selected,
                key,
            });
        }
    });
    refresh(view);
    if let Some(combo) = ctl(ID_ED_COMBO) {
        unsafe {
            let _ = SetFocus(Some(combo));
        }
    }
}

/// Closes the editor; with `accept`, records the chosen action.
fn end_edit(view: HWND, accept: bool) {
    let buttons = device_buttons();
    let row = with_app(|a| {
        let m = a.view.mouse.as_mut()?;
        let e = m.editing.take()?;
        if accept {
            let action = match e.choices.get(e.selected).map(|(_, a)| *a) {
                Some(Some(a)) => Some(a),
                Some(None) => e
                    .key
                    .map(|(modifiers, usage)| Action::Keyboard { modifiers, usage }),
                None => None,
            };
            if let (Some(action), Some(&device)) = (action, buttons.get(e.row)) {
                upsert(&mut m.edits, e.row, action, device);
                m.buttons_result = None;
            }
        }
        Some(e.row)
    })
    .flatten();
    refresh(view);
    if let Some(h) = row.and_then(|r| ctl(ID_BTN_CHANGE0 + r as i32)) {
        unsafe {
            let _ = SetFocus(Some(h));
        }
    }
}

fn send_write(view: HWND, what: Write, command: impl FnOnce(DeviceKey) -> Command) {
    let Some(shared) = super::snapshot() else {
        return;
    };
    let key = with_app(|a| {
        a.view.mouse.as_mut().map(|m| {
            m.pending = Some((shared.seq, Pending::Buttons(what)));
            m.buttons_result = None;
            if what != Write::Save {
                // "Se perderán los botones que hayas cambiado."
                m.edits.clear();
            }
            m.key.clone()
        })
    })
    .flatten();
    if let Some(key) = key {
        send(command(key));
    }
    refresh(view);
}

fn confirm_save(view: HWND) {
    let buttons = device_buttons();
    let edits = with_app(|a| a.view.mouse.as_ref().map(|m| m.edits.clone()))
        .flatten()
        .unwrap_or_default();
    if edits.is_empty() {
        return;
    }
    let lines: Vec<String> = edits
        .iter()
        .map(|&(r, a)| {
            let before = buttons.get(r).map(|&b| text::action(b)).unwrap_or_default();
            format!("{}: {before} → {}", text::mouse_button(r), text::action(a))
        })
        .collect();
    let n = edits.len();
    let title = format!(
        "¿Guardar {} en la memoria del ratón?",
        plural(n, "cambio", "cambios")
    );
    let body = format!(
        "{}\n\nEl ratón los conservará aunque cierres OpenPeripheral o lo uses en otro PC. Si la comprobación de la escritura falla, OpenPeripheral deja el perfil como estaba.",
        lines.join("\n")
    );
    super::confirm::open(
        view,
        &title,
        &body,
        "Guardar en el ratón",
        Box::new(move || {
            send_write(view, Write::Save, |key| {
                Command::WriteButtons(key, edits, Confirmed::by_dialog())
            })
        }),
    );
}

fn confirm_restore(view: HWND, source: Source) {
    let date = super::snapshot()
        .and_then(|shared| {
            let key = with_app(|a| a.view.mouse.as_ref().map(|m| m.key.clone())).flatten()?;
            match snapshot(&shared, &key)?.backup.clone() {
                Some(Ok(d)) => Some(crate::backup::display_date(&d)),
                _ => None,
            }
        })
        .unwrap_or_default();
    let (title, body, label, what) = match source {
        Source::Original => (
            "¿Restaurar el perfil original?".to_string(),
            format!("Se escribirá en el ratón la copia guardada el {date}. Se perderán los botones que hayas cambiado."),
            "Restaurar",
            Write::Original,
        ),
        Source::Factory => (
            "¿Volver a los botones de fábrica?".to_string(),
            "Se escribirán en el ratón los botones de su perfil de fábrica; el resto del perfil no cambia. Se perderán los botones que hayas cambiado.".to_string(),
            "Restaurar botones de fábrica",
            Write::Factory,
        ),
    };
    super::confirm::open(
        view,
        &title,
        &body,
        label,
        Box::new(move || {
            send_write(view, what, |key| {
                Command::RestoreProfile(key, source, Confirmed::by_dialog())
            })
        }),
    );
}

/// Leaves the page; with button changes pending, asks first.
fn request_leave(view: HWND) {
    let n = with_app(|a| a.view.mouse.as_ref().map(|m| m.edits.len()))
        .flatten()
        .unwrap_or(0);
    if n == 0 {
        leave(view);
        return;
    }
    super::confirm::open(
        view,
        &format!("¿Descartar {} sin guardar?", plural(n, "cambio", "cambios")),
        "Los botones del ratón no han cambiado: estos cambios solo estaban en esta pantalla.",
        "Descartar cambios",
        Box::new(move || leave(view)),
    );
}

// ---------------------------------------------------------------------------
// Commands.

fn dpi_range(shared: &Shared) -> Option<(u16, DpiRange)> {
    let key = with_app(|a| a.view.mouse.as_ref().map(|m| m.key.clone())).flatten()?;
    snapshot(shared, &key).and_then(current_dpi)
}

fn apply(view: HWND) {
    let Some(shared) = super::snapshot() else {
        return;
    };
    let Some((_, range)) = dpi_range(&shared) else {
        return;
    };
    let typed = ctl(ID_DPI_EDIT).map(get_text).unwrap_or_default();
    match parse_field(&typed).filter(|&v| range.contains(v)) {
        Some(v) => {
            let key = with_app(|a| {
                a.view.mouse.as_mut().map(|m| {
                    m.invalid = false;
                    m.applied = None;
                    m.pending = Some((shared.seq, Pending::Apply(v)));
                    m.key.clone()
                })
            })
            .flatten();
            if let Some(key) = key {
                send(Command::Apply(key, Change::Dpi(v)));
            }
        }
        None => {
            with_app(|a| {
                if let Some(m) = a.view.mouse.as_mut() {
                    m.invalid = true;
                }
            });
            if let Some(e) = ctl(ID_DPI_EDIT) {
                unsafe {
                    let _ = SetFocus(Some(e));
                    SendMessageW(e, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
                }
            }
        }
    }
    refresh(view);
}

/// Handles WM_COMMAND while the page is open. Returns false if not handled.
pub(crate) fn on_command(view: HWND, id: i32, code: u32, lparam: LPARAM) -> bool {
    match (id, code) {
        // Esc: closes the row editor first, then leaves the page.
        (2, _) => {
            if with_app(|a| a.view.mouse.as_ref().is_some_and(|m| m.editing.is_some()))
                == Some(true)
            {
                end_edit(view, false);
            } else {
                request_leave(view);
            }
        }
        // Enter: apply the DPI when the field has the focus.
        (1, _) => {
            if Some(unsafe { GetFocus() }) == ctl(ID_DPI_EDIT) {
                apply(view);
            } else {
                let f = unsafe { GetFocus() };
                if kind_of(f).is_some_and(|(_, k)| matches!(k, Kind::Button(..))) {
                    unsafe { SendMessageW(f, BM_CLICK, None, None) };
                }
            }
        }
        (ID_BACK, BN_CLICKED) => {
            with_app(|a| a.view.mouse.as_mut().map(|m| m.editing = None));
            request_leave(view);
        }
        (c, BN_CLICKED) if (ID_BTN_CHANGE0 + 1..ID_BTN_CHANGE0 + MAX_BUTTONS).contains(&c) => {
            begin_edit(view, (c - ID_BTN_CHANGE0) as usize)
        }
        (ID_ED_COMBO, CBN_SELCHANGE) => {
            let sel = ctl(ID_ED_COMBO)
                .map(|h| unsafe { SendMessageW(h, CB_GETCURSEL, None, None).0 })
                .unwrap_or(-1);
            with_app(|a| {
                if let Some(e) = a.view.mouse.as_mut().and_then(|m| m.editing.as_mut()) {
                    if sel >= 0 && (sel as usize) < e.choices.len() {
                        e.selected = sel as usize;
                    }
                }
            });
            refresh(view);
        }
        (ID_ED_KEY_CLEAR, BN_CLICKED) => {
            with_app(|a| {
                if let Some(e) = a.view.mouse.as_mut().and_then(|m| m.editing.as_mut()) {
                    e.key = None;
                }
            });
            refresh(view);
            if let Some(h) = ctl(ID_ED_KEY) {
                unsafe {
                    let _ = SetFocus(Some(h));
                }
            }
        }
        (ID_ED_OK, BN_CLICKED) => end_edit(view, true),
        (ID_ED_CANCEL, BN_CLICKED) => end_edit(view, false),
        (ID_BAR_DISCARD, BN_CLICKED) => {
            with_app(|a| {
                if let Some(m) = a.view.mouse.as_mut() {
                    m.edits.clear();
                    m.buttons_result = None;
                }
            });
            refresh(view);
        }
        (ID_BAR_SAVE, BN_CLICKED) => confirm_save(view),
        (ID_PROFILE_RESTORE, BN_CLICKED) => confirm_restore(view, Source::Original),
        (ID_FACTORY, BN_CLICKED) => confirm_restore(view, Source::Factory),
        (ID_APPLY, BN_CLICKED) => apply(view),
        (ID_MINUS | ID_PLUS, BN_CLICKED) => {
            if let Some(shared) = super::snapshot() {
                if let Some((cur, range)) = dpi_range(&shared) {
                    let typed = ctl(ID_DPI_EDIT).map(get_text).unwrap_or_default();
                    let from = parse_field(&typed).unwrap_or(cur);
                    let v = step(&range, from, if id == ID_PLUS { 1 } else { -1 });
                    set(ID_DPI_EDIT, &v.to_string());
                }
            }
        }
        (p, BN_CLICKED) if (ID_PRESET0..ID_PRESET0 + PRESETS.len() as i32).contains(&p) => {
            set(ID_DPI_EDIT, &PRESETS[(p - ID_PRESET0) as usize].to_string());
        }
        (ID_RST_BTN, BN_CLICKED) => {
            let Some(shared) = super::snapshot() else {
                return true;
            };
            let key = with_app(|a| {
                a.view.mouse.as_mut().map(|m| {
                    m.pending = Some((shared.seq, Pending::Restore));
                    m.restore_text = None;
                    m.applied = None;
                    m.invalid = false;
                    m.key.clone()
                })
            })
            .flatten();
            if let Some(key) = key {
                send(Command::Restore(key));
            }
            refresh(view);
        }
        (ID_DPI_EDIT, EN_CHANGE) => {
            with_app(|a| {
                if let Some(m) = a.view.mouse.as_mut() {
                    m.invalid = false;
                }
            });
            refresh(view);
        }
        (ID_ED_COMBO, CBN_SETFOCUS) | (_, BN_SETFOCUS) | (_, EN_SETFOCUS) => {
            devices_view::ensure_visible(view, HWND(lparam.0 as *mut _))
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    const G305: DpiRange = DpiRange::Range {
        min: 200,
        max: 12000,
        step: 50,
    };

    #[test]
    fn action_choices() {
        let raw = Action::Raw([0x90, 0x0c, 0, 0]);
        let list = choices(raw);
        assert_eq!(list.len(), 7);
        assert_eq!(list[1].0, "Clics y navegación: Clic derecho");
        assert_eq!(list[5], ("Otras: Atajo de teclado…".to_string(), None));
        assert_eq!(list[6].1, Some(raw));
        assert_eq!(choice_for(&list, raw), 6);
        assert_eq!(choice_for(&list, Action::Mouse(0x08)), 3);
        let key = Action::Keyboard {
            modifiers: 1,
            usage: 0x19,
        };
        assert_eq!(choices(key).len(), 6);
        assert_eq!(choice_for(&choices(key), key), 5);
    }

    #[test]
    fn choosing_the_current_action_removes_the_change() {
        let mut edits = Vec::new();
        upsert(&mut edits, 4, Action::Mouse(0x10), Action::Mouse(0x08));
        upsert(&mut edits, 3, Action::Mouse(0x02), Action::Mouse(0x01));
        assert_eq!(
            edits,
            vec![(3, Action::Mouse(0x02)), (4, Action::Mouse(0x10))]
        );
        upsert(&mut edits, 4, Action::Mouse(0x08), Action::Mouse(0x08));
        assert_eq!(edits, vec![(3, Action::Mouse(0x02))]);
    }

    #[test]
    fn shortcut_capture_ignores_tab_esc_and_modifiers() {
        assert_eq!(capture(0x43, true, false, false, false), Some((0x01, 0x06))); // Ctrl+C
        assert_eq!(capture(0x70, false, true, true, false), Some((0x06, 0x3a))); // Mayús+Alt+F1
        assert_eq!(capture(VK_TAB.0, false, false, false, false), None);
        assert_eq!(capture(VK_ESCAPE.0, true, false, false, false), None);
        assert_eq!(capture(VK_CONTROL.0, true, false, false, false), None);
        assert_eq!(capture(0x0d, false, false, false, true), Some((0x08, 0x28)));
        let (m, u) = capture(0x56, true, true, false, false).unwrap();
        assert_eq!(text::shortcut(m, u), "Ctrl + Mayús + V");
        assert!(crate::protocol::onboard::assignable(Action::Keyboard {
            modifiers: m,
            usage: u
        }));
    }

    #[test]
    fn field_parsing() {
        assert_eq!(parse_field(" 1600 "), Some(1600));
        assert_eq!(parse_field(""), None);
        assert_eq!(parse_field("abc"), None);
        assert_eq!(parse_field("99999"), None);
    }

    #[test]
    fn steps_snap_and_clamp() {
        assert_eq!(step(&G305, 1000, 1), 1050);
        assert_eq!(step(&G305, 1000, -1), 950);
        // Off-grid values snap to the grid in the step's direction.
        assert_eq!(step(&G305, 1625, 1), 1650);
        assert_eq!(step(&G305, 1625, -1), 1600);
        assert_eq!(step(&G305, 12000, 1), 12000);
        assert_eq!(step(&G305, 200, -1), 200);
        assert_eq!(step(&G305, 50, 1), 200);
        assert_eq!(step(&G305, 60000, -1), 12000);
        let list = DpiRange::Values(vec![400, 800, 1600]);
        assert_eq!(step(&list, 800, 1), 1600);
        assert_eq!(step(&list, 800, -1), 400);
        assert_eq!(step(&list, 1600, 1), 1600);
        assert_eq!(step(&list, 400, -1), 400);
        assert_eq!(step(&list, 1000, 1), 1600);
    }

    #[test]
    fn presets_inside_range_only() {
        assert_eq!(presets_for(&G305), vec![400, 800, 1600, 3200]);
        let small = DpiRange::Range {
            min: 400,
            max: 1600,
            step: 400,
        };
        assert_eq!(presets_for(&small), vec![400, 800, 1600]);
        assert_eq!(bounds(&G305), (200, 12000));
    }

    #[test]
    fn invalid_messages_suggest_valid_values() {
        assert_eq!(
            invalid_message(&G305, "1625"),
            "Error: 1625 no es válido. Usa un valor de 200 a 12000 en pasos de 50 (por ejemplo, 1600 o 1650)."
        );
        assert_eq!(
            invalid_message(&G305, "13000"),
            "Error: 13000 no es válido. Usa un valor de 200 a 12000 en pasos de 50."
        );
        assert!(invalid_message(&G305, "").starts_with("Error: El campo está vacío."));
        assert_eq!(
            range_sentence(&G305),
            "Puntos por pulgada del sensor. Admite de 200 a 12000, en pasos de 50."
        );
    }
}
