//! "Ajustes del ratón" page (`docs/design/DESIGN.md`), shown in place of the
//! device grid inside the same window. Text is native STATIC controls and
//! the DPI value a native EDIT, so screen readers read them; the window
//! paints only the surfaces behind them (cards, field border, glyphs). The
//! STATICs are opaque in their surface colour, so nothing flickers.

use super::devices_view::{self, Geometry};
use super::glyph::{self, Canvas};
use super::light::{self, Pick};
use super::panel::{self, ButtonStyle, Ctx, FontRole, Fonts};
use super::profiles as texts;
use super::text::{self, Energy};
use super::theme::{Color, Palette};
use super::{send, wide, with_app};
use crate::app::{Command, Confirmed, Profile, ProfileView, Shared, Snapshot, Source, Status};
use crate::devices::{Change, DeviceKey, DpiRange};
use crate::protocol::onboard::{Action, Light, ProfileEdits};
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
const ID_PAGE_NOTE: i32 = 307;
const ID_DPI_TITLE: i32 = 310;
const ID_DPI_DESC: i32 = 311;
const ID_DPI_CURRENT: i32 = 312;
const ID_DPI_LABEL: i32 = 313;
const ID_DPI_EDIT: i32 = 314;
const ID_MINUS: i32 = 315;
const ID_PLUS: i32 = 316;
const ID_APPLY: i32 = 317;
const ID_PRESETS_LABEL: i32 = 318;
const ID_DPI_BADGE: i32 = 319;
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
// Light section.
const ID_LIGHT_TITLE: i32 = 430;
const ID_LIGHT_BADGE: i32 = 431;
const ID_LIGHT_DESC: i32 = 432;
const ID_EFFECT_LABEL: i32 = 433;
const ID_EFFECT0: i32 = 434; // ..=437
const ID_COLOR_LABEL: i32 = 438;
const ID_SWATCH0: i32 = 440; // ..=447
const ID_HEX_LABEL: i32 = 448;
const ID_HEX_EDIT: i32 = 449;
const ID_HEX_NAME: i32 = 450;
const ID_SPEED_LABEL: i32 = 451;
const ID_SPEED0: i32 = 452; // ..=454
const ID_LIGHT_NOTE: i32 = 455;
const ID_LIGHT_TRY: i32 = 456;
const ID_LIGHT_SAVED: i32 = 457;
const ID_LIGHT_STATUS: i32 = 458;
const ID_PROFILE_TITLE: i32 = 460;
const ID_RATE_TITLE: i32 = 330;
const ID_RATE_VALUE: i32 = 331;
const ID_RATE_RO: i32 = 332;
const ID_RATE_DESC: i32 = 333;
const ID_RST_DESC: i32 = 341;
const ID_RST_BTN: i32 = 342;
const ID_RST_STATUS: i32 = 343;
// Profiles section (plan `perfiles-raton`). Per row: name, summary and its
// buttons, in tab order.
const ID_PRF_TITLE: i32 = 470;
const ID_PRF_COUNT: i32 = 471;
const ID_PRF_MISMATCH: i32 = 472;
const ID_PRF_REAPPLY: i32 = 473;
const ID_PRF_SAVEAS: i32 = 474;
const ID_PRF_ACTIVE: i32 = 475;
const ID_PRF_FIRST: i32 = 476;
const ID_PRF_NEW: i32 = 477;
const ID_PRF_NOTE: i32 = 478;
const ID_PRF_STATUS: i32 = 479;
const ID_PRF_NAME0: i32 = 480; // ..=487
const ID_PRF_SUM0: i32 = 490; // ..=497
const ID_PRF_USE0: i32 = 500; // ..=507
const ID_PRF_DEL0: i32 = 510; // ..=517
const ID_PRF_REN0: i32 = 520; // ..=527
const MAX_ROWS: i32 = crate::profiles::MAX_PROFILES as i32;
const ID_PRF_GAME: i32 = 528;
const ID_PRF_PREV: i32 = 529;
const ID_PRF_PAGE: i32 = 530;
const ID_PRF_NEXT: i32 = 531;
// «Juegos y programas» (plan `perfiles-por-juego`).
const ID_GAM_TITLE: i32 = 600;
const ID_GAM_DESC: i32 = 601;
const ID_GAM_AUTO: i32 = 602;
const ID_GAM_ASK: i32 = 603;
const ID_GAM_ASK_HELP: i32 = 604;
const ID_GAM_NEW_TITLE: i32 = 605;
const ID_GAM_EMPTY: i32 = 606;
const ID_GAM_ADD: i32 = 607;
const ID_GAM_UNREG: i32 = 608;
const ID_GAM_STATUS: i32 = 609;
const ID_GAM_NEW0: i32 = 610; // ..=617 games not answered
const ID_GAM_CFG0: i32 = 620; // ..=627
const ID_GAM_NEVER0: i32 = 630; // ..=637
const MAX_NEW: i32 = 8;
const ID_GAM_NAME0: i32 = 700; // ..=763 programs
const ID_GAM_PATH0: i32 = 800;
const ID_GAM_SEL0: i32 = 900;
const ID_GAM_DEL0: i32 = 1000;
const MAX_PROGS: i32 = crate::games::MAX_PROGRAMS as i32;

/// Controls that live in the fixed bar at the bottom of the window.
const BAR: [i32; 4] = [ID_BAR_TEXT, ID_BAR_DISCARD, ID_BAR_SAVE, ID_SAVE_STATUS];
const CONTENT_CLASS: PCWSTR = w!("OpenPeripheral.MouseContent");
const BAR_CLASS: PCWSTR = w!("OpenPeripheral.MouseBar");

const N_EFFECTS: i32 = light::EFFECTS.len() as i32;
const N_SWATCHES: i32 = light::SWATCHES.len() as i32;
const N_SPEEDS: i32 = light::SPEEDS.len() as i32;

fn in_group(id: i32, first: i32, n: i32) -> Option<usize> {
    (first..first + n)
        .contains(&id)
        .then(|| (id - first) as usize)
}

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
    /// Native check box (classic look, so it takes the theme's colours).
    Check(Surface),
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
    /// "Probar" of a DPI value.
    Apply(u16),
    /// "Probar" of the picked light.
    TryLight,
    Restore,
    /// A write to the mouse's memory.
    Buttons(Write),
    /// A change to the profile list (no write to the mouse).
    Profiles,
    /// A change to the programs list or the two switches.
    Games,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Write {
    Save,
    Original,
    Factory,
    /// "Usar" a profile.
    Use,
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
    /// Light picked in the section; `None` until the profile is read.
    pick: Option<Pick>,
    /// Outcome of the last light "Probar".
    light_result: Option<bool>,
    /// What "Probar" sent and was not saved, to undo on discard or leave.
    tried_dpi: Option<u16>,
    tried_light: Option<Light>,
    /// The saved DPI/light changed (save or restore): refill the controls.
    resync: bool,
    hex_field: Option<RECT>,
    hex_invalid: bool,
    /// Scrolling child window holding the page, and the fixed bar below it.
    pub(crate) content: HWND,
    pub(crate) bar: HWND,
    /// Height of the bar in px (0 when hidden).
    pub(crate) bar_h: i32,
    preview: Option<RECT>,
    light_icon: Option<(i32, i32, bool)>,
    /// Profiles: painted rows (box, active), the check before «Activo», the
    /// status icon, the last result, the profile being used and the one it
    /// replaces, a profile to use once a save ends, and whether the focus
    /// goes to the new active row.
    rows: Vec<(RECT, bool)>,
    active_check: Option<(i32, i32)>,
    profile_icon: Option<(i32, i32, bool)>,
    profile_status: Option<(bool, String)>,
    using: Option<(String, String)>,
    then_use: Option<String>,
    focus_active: bool,
    /// Page of the profiles list chosen with the arrows (`None`: the page of
    /// the active profile) and the one shown.
    page: Option<usize>,
    page_shown: usize,
    /// «Juegos y programas»: last result, automatic results already seen,
    /// and whether the worker knows about unsaved button changes.
    games_status: Option<(bool, String)>,
    autos_seen: u64,
    page_edits_sent: bool,
}

// ---------------------------------------------------------------------------
// What is saved, what is pending.

fn profile_view(snap: &Snapshot) -> Option<&ProfileView> {
    match &snap.profile {
        Profile::Read(v) => Some(v),
        _ => None,
    }
}

/// DPI the mouse keeps: the one the app applies on connect, else the
/// profile's cycle when all its entries match, else the DPI read when the
/// mouse was first seen (never the one being tried).
pub fn base_dpi(
    saved: Option<u16>,
    cycle: Option<[u16; 4]>,
    original: Option<u16>,
    current: Option<u16>,
) -> Option<u16> {
    saved
        .or(cycle
            .filter(|c| c.iter().all(|&d| d == c[0]) && c[0] != 0)
            .map(|c| c[0]))
        .or(original)
        .or(current)
}

fn snap_base_dpi(snap: &Snapshot) -> Option<u16> {
    base_dpi(
        snap.saved_dpi,
        profile_view(snap).and_then(|v| v.dpi_cycle),
        snap.original_dpi,
        current_dpi(snap).map(|(c, _)| c),
    )
}

/// Light the mouse keeps: the one the app applies on connect, else the
/// profile's.
fn snap_base_light(snap: &Snapshot) -> Option<Light> {
    snap.saved_light
        .or(profile_view(snap).and_then(|v| v.light))
}

/// "Guardado: …" of the DPI section.
pub fn saved_dpi_text(saved: Option<u16>, cycle: Option<[u16; 4]>, base: Option<u16>) -> String {
    match (saved, cycle) {
        (None, Some(c)) if c.iter().any(|&d| d != c[0]) => {
            let list: Vec<String> = c.iter().filter(|&&d| d != 0).map(u16::to_string).collect();
            format!("Guardado: ciclo {}", list.join(" · "))
        }
        _ => base
            .map(|b| format!("Guardado: {b} DPI"))
            .unwrap_or_default(),
    }
}

/// Pending changes of the page, as a save would send them.
fn pending_edits(m: &MouseState, snap: Option<&Snapshot>, field: Option<u16>) -> ProfileEdits {
    let Some(snap) = snap else {
        return ProfileEdits::default();
    };
    let dpi = field
        .filter(|&v| current_dpi(snap).is_some_and(|(_, r)| r.contains(v)))
        .filter(|&v| Some(v) != snap_base_dpi(snap));
    let light = m
        .pick
        .as_ref()
        .and_then(Pick::light)
        .filter(|&l| Some(l) != snap_base_light(snap));
    ProfileEdits {
        dpi,
        light,
        buttons: m.edits.clone(),
    }
}

fn count(e: &ProfileEdits) -> usize {
    e.dpi.is_some() as usize + e.light.is_some() as usize + e.buttons.len()
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
        DpiRange::Range { min, max, step } => format!(
            "DPI con el que arranca el ratón. Admite de {min} a {max}, en pasos de {step}. Al guardarlo se escribe en los 4 niveles del botón DPI."
        ),
        DpiRange::Values(_) => format!(
            "DPI con el que arranca el ratón. Admite {}. Al guardarlo se escribe en los 4 niveles del botón DPI.",
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

fn register_classes() {
    unsafe {
        let instance = devices_view::instance();
        let cursor = LoadCursorW(None, IDC_ARROW).unwrap_or_default();
        let procs: [(PCWSTR, WNDPROC); 2] = [
            (CONTENT_CLASS, Some(content_proc)),
            (BAR_CLASS, Some(bar_proc)),
        ];
        for (class, proc) in procs {
            // Fails harmlessly once the class exists.
            RegisterClassW(&WNDCLASSW {
                lpfnWndProc: proc,
                hInstance: instance,
                lpszClassName: class,
                hCursor: cursor,
                ..Default::default()
            });
        }
    }
}

fn container(view: HWND, class: PCWSTR) -> HWND {
    unsafe {
        CreateWindowExW(
            WS_EX_CONTROLPARENT,
            class,
            w!(""),
            WINDOW_STYLE(WS_CHILD.0 | WS_VISIBLE.0 | WS_CLIPCHILDREN.0),
            0,
            0,
            0,
            0,
            Some(view),
            None,
            Some(devices_view::instance()),
            None,
        )
        .unwrap_or_default()
    }
}

/// Messages of the controls go to the view, which handles the page.
fn forward(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> Option<LRESULT> {
    match msg {
        WM_COMMAND | WM_DRAWITEM | WM_CTLCOLORSTATIC | WM_CTLCOLOREDIT | WM_CTLCOLORBTN
        | WM_CTLCOLORLISTBOX => unsafe {
            let parent = GetParent(hwnd).ok()?;
            Some(SendMessageW(parent, msg, Some(wparam), Some(lparam)))
        },
        _ => None,
    }
}

/// Paints into a memory bitmap of `hwnd`'s client area, then copies it.
fn paint_buffered(hwnd: HWND, draw: impl FnOnce(HDC, RECT, &Palette, f32)) {
    unsafe {
        let mut ps = PAINTSTRUCT::default();
        let hdc = BeginPaint(hwnd, &mut ps);
        let mut rc = RECT::default();
        let _ = GetClientRect(hwnd, &mut rc);
        if let Some(palette) = with_app(|a| a.view.palette) {
            let mem = CreateCompatibleDC(Some(hdc));
            let bmp = CreateCompatibleBitmap(hdc, rc.right.max(1), rc.bottom.max(1));
            let old = SelectObject(mem, bmp.into());
            draw(mem, rc, &palette, devices_view::scale_of(hwnd));
            let _ = BitBlt(hdc, 0, 0, rc.right, rc.bottom, Some(mem), 0, 0, SRCCOPY);
            SelectObject(mem, old);
            let _ = DeleteObject(bmp.into());
            let _ = DeleteDC(mem);
        }
        let _ = EndPaint(hwnd, &ps);
    }
}

extern "system" fn content_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(r) = forward(hwnd, msg, wparam, lparam) {
        return r;
    }
    match msg {
        WM_PAINT => {
            let scroll = with_app(|a| a.view.scroll).unwrap_or(0);
            paint_buffered(hwnd, |mem, rc, palette, scale| {
                unsafe { panel::fill(mem, rc, palette.bg_app) };
                paint(mem, palette, scale, scroll);
            });
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// The fixed bar: `surface.card` with a `border.control` top line
/// (DESIGN.md «Modelo de guardado»).
extern "system" fn bar_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if let Some(r) = forward(hwnd, msg, wparam, lparam) {
        return r;
    }
    match msg {
        WM_PAINT => {
            let icon = with_app(|a| a.view.mouse.as_ref().and_then(|m| m.save_icon)).flatten();
            paint_buffered(hwnd, |mem, rc, palette, scale| {
                let line = (scale.round() as i32).max(1);
                unsafe {
                    panel::fill(mem, rc, palette.surface_card);
                    panel::fill(mem, RECT { bottom: line, ..rc }, palette.border_control);
                }
                if let (Some((x, y, ok)), Some(mut c)) = (icon, Canvas::new(mem, scale)) {
                    c.origin(x as f32, y as f32 + scale);
                    if ok {
                        glyph::check(&c, 0.0, 0.0, palette.text_primary);
                    } else {
                        glyph::notice_icon(&c, 0.0, 0.0, true, palette.text_primary);
                    }
                }
            });
            LRESULT(0)
        }
        WM_ERASEBKGND => LRESULT(1),
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

/// Replaces the device grid with this page for `key`.
pub(crate) fn enter(view: HWND, key: DeviceKey) {
    devices_view::clear_devices_page(view);
    register_classes();
    // Creation order is tab order: the page, then the bar.
    let content = container(view, CONTENT_CLASS);
    let bar = container(view, BAR_CLASS);
    let st = SS_NOPREFIX.0 | SS_LEFT.0;
    let btn = WS_TABSTOP.0 | (BS_OWNERDRAW | BS_NOTIFY) as u32;
    let mut ctls = Vec::new();
    let mut add = |id: i32, kind: Kind, class: PCWSTR, text: &str, style: u32| {
        let parent = if BAR.contains(&id) { bar } else { content };
        let hwnd = create(parent, id, class, text, style);
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
        ID_PAGE_NOTE,
        Kind::Text(Secondary, App),
        s,
        "OpenPeripheral aplica tu DPI y tu luz al iniciarse o al conectar el ratón. «Guardar en el ratón…» los conserva también sin la app y en otros PC. «Probar» lo envía al ratón sin guardarlo.",
        st,
    );
    add(ID_PRF_TITLE, Kind::Text(Name, Card), s, "Perfiles", st);
    add(ID_PRF_COUNT, Kind::Text(Tertiary, Card), s, "", st);
    add(ID_PRF_GAME, Kind::Text(Notice, Card), s, "", st);
    add(ID_PRF_MISMATCH, Kind::Text(Notice, Card), s, "", st);
    add(
        ID_PRF_REAPPLY,
        Kind::Button(ButtonStyle::Primary, Card),
        b,
        "Volver a aplicar",
        btn,
    );
    add(
        ID_PRF_SAVEAS,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Guardar como perfil nuevo…",
        btn,
    );
    add(ID_PRF_ACTIVE, Kind::Text(Strong, Selected), s, "", st);
    for i in 0..MAX_ROWS {
        add(ID_PRF_NAME0 + i, Kind::Text(Strong, Card), s, "", st);
        add(ID_PRF_SUM0 + i, Kind::Text(Secondary, Card), s, "", st);
        add(
            ID_PRF_USE0 + i,
            Kind::Button(ButtonStyle::Primary, Card),
            b,
            "Usar",
            btn,
        );
        add(
            ID_PRF_DEL0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            "Eliminar…",
            btn,
        );
        add(
            ID_PRF_REN0 + i,
            Kind::Button(ButtonStyle::Normal, Selected),
            b,
            "Renombrar…",
            btn,
        );
    }
    add(
        ID_PRF_PREV,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Página anterior de perfiles",
        btn,
    );
    add(ID_PRF_PAGE, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_PRF_NEXT,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Página siguiente de perfiles",
        btn,
    );
    add(ID_PRF_FIRST, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_PRF_NEW,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Nuevo perfil…",
        btn,
    );
    add(ID_PRF_NOTE, Kind::Text(Tertiary, Card), s, "", st);
    add(ID_PRF_STATUS, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_DPI_TITLE,
        Kind::Text(Name, Card),
        s,
        "Sensibilidad (DPI)",
        st,
    );
    add(ID_DPI_BADGE, Kind::Text(Tertiary, Card), s, "", st);
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
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Probar",
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
    // Light. Each group (effects, swatches, speeds) starts with WS_GROUP on
    // its label and its first button: one Tab stop per group (the chosen
    // item) and the arrows move inside it.
    let grp = WS_GROUP.0;
    add(ID_LIGHT_TITLE, Kind::Text(Name, Card), s, "Iluminación", st);
    add(ID_LIGHT_BADGE, Kind::Text(Tertiary, Card), s, "", st);
    add(ID_LIGHT_DESC, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_EFFECT_LABEL,
        Kind::Text(Notice, Card),
        s,
        "Efecto",
        st | grp,
    );
    for i in 0..N_EFFECTS {
        let style = (BS_OWNERDRAW | BS_NOTIFY) as u32 | if i == 0 { grp } else { 0 };
        add(
            ID_EFFECT0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            light::EFFECTS[i as usize].1,
            style,
        );
    }
    add(
        ID_COLOR_LABEL,
        Kind::Text(Notice, Card),
        s,
        "Color",
        st | grp,
    );
    for i in 0..N_SWATCHES {
        let style = (BS_OWNERDRAW | BS_NOTIFY) as u32 | if i == 0 { grp } else { 0 };
        add(
            ID_SWATCH0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            "",
            style,
        );
    }
    // The label directly precedes the field: it becomes its accessible name.
    add(ID_HEX_LABEL, Kind::Text(Notice, Card), s, "Hex", st | grp);
    add(
        ID_HEX_EDIT,
        Kind::Edit,
        w!("EDIT"),
        "",
        WS_TABSTOP.0 | grp | ES_AUTOHSCROLL as u32,
    );
    add(ID_HEX_NAME, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_SPEED_LABEL,
        Kind::Text(Notice, Card),
        s,
        "Velocidad",
        st | grp,
    );
    for i in 0..N_SPEEDS {
        let style = (BS_OWNERDRAW | BS_NOTIFY) as u32 | if i == 0 { grp } else { 0 };
        add(
            ID_SPEED0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            light::SPEEDS[i as usize],
            style,
        );
    }
    add(ID_LIGHT_NOTE, Kind::Text(Secondary, Card), s, "", st | grp);
    add(
        ID_LIGHT_TRY,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Probar la luz",
        btn | grp,
    );
    add(ID_LIGHT_SAVED, Kind::Text(Secondary, Card), s, "", st);
    add(ID_LIGHT_STATUS, Kind::Text(Secondary, Card), s, "", st);
    add(ID_BTN_TITLE, Kind::Text(Name, Card), s, "Botones", st | grp);
    add(ID_BTN_DESC, Kind::Text(Secondary, Card), s, "", st);
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
    add(
        ID_GAM_TITLE,
        Kind::Text(Name, Card),
        s,
        "Juegos y programas",
        st,
    );
    add(
        ID_GAM_DESC,
        Kind::Text(Secondary, Card),
        s,
        "Al abrir uno de estos programas, OpenPeripheral usa su perfil: el DPI y la luz al momento, y escribe los botones en el ratón solo si cambian. Al cerrarlo, vuelve el perfil que había. Solo funciona con OpenPeripheral abierto.",
        st,
    );
    let chk = WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32;
    add(
        ID_GAM_AUTO,
        Kind::Check(Card),
        b,
        "Cambiar de perfil al abrir estos programas",
        chk,
    );
    add(
        ID_GAM_ASK,
        Kind::Check(Card),
        b,
        "Preguntar al detectar un juego nuevo",
        chk,
    );
    add(
        ID_GAM_ASK_HELP,
        Kind::Text(Secondary, Card),
        s,
        "Juegos de Steam, Xbox, Epic o GOG. Se pregunta con una notificación de Windows.",
        st,
    );
    add(
        ID_GAM_NEW_TITLE,
        Kind::Text(Notice, Card),
        s,
        "Juegos detectados sin responder",
        st,
    );
    for i in 0..MAX_NEW {
        add(ID_GAM_NEW0 + i, Kind::Text(Strong, Card), s, "", st);
        add(
            ID_GAM_CFG0 + i,
            Kind::Button(ButtonStyle::Primary, Card),
            b,
            "Configurar…",
            btn,
        );
        add(
            ID_GAM_NEVER0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            "No preguntar más",
            btn,
        );
    }
    for i in 0..MAX_PROGS {
        add(ID_GAM_NAME0 + i, Kind::Text(Strong, Card), s, "", st);
        add(ID_GAM_PATH0 + i, Kind::Text(Secondary, Card), s, "", st);
        add(
            ID_GAM_SEL0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            "",
            btn,
        );
        add(
            ID_GAM_DEL0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            "Quitar",
            btn,
        );
    }
    add(
        ID_GAM_EMPTY,
        Kind::Text(Secondary, Card),
        s,
        "Todavía no hay programas. Se añadirán al responder a la notificación de un juego nuevo, o con «Añadir programa…».",
        st,
    );
    add(
        ID_GAM_ADD,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Añadir programa…",
        btn,
    );
    add(
        ID_GAM_UNREG,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Quitar avisos de Windows",
        btn,
    );
    add(ID_GAM_STATUS, Kind::Text(Notice, Card), s, "", st);
    add(ID_PROFILE_TITLE, Kind::Text(Name, Card), s, "Restaurar", st);
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
    add(ID_RST_DESC, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_RST_BTN,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Restaurar originales",
        btn,
    );
    add(ID_RST_STATUS, Kind::Text(Secondary, Card), s, "", st);
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
    unsafe {
        let edit = ctls
            .iter()
            .find(|c| c.id == ID_DPI_EDIT)
            .map(|c| c.hwnd)
            .unwrap_or_default();
        SendMessageW(edit, EM_SETLIMITTEXT, Some(WPARAM(5)), None);
        if let Some(hex) = ctls.iter().find(|c| c.id == ID_HEX_EDIT) {
            SendMessageW(hex.hwnd, EM_SETLIMITTEXT, Some(WPARAM(7)), None);
        }
        if let Some(key) = ctls.iter().find(|c| c.id == ID_ED_KEY) {
            let _ = SetWindowSubclass(key.hwnd, Some(capture_proc), 1, 0);
        }
        // Classic check boxes draw their text with the colours of
        // WM_CTLCOLORSTATIC, so they follow the dark theme.
        for c in ctls.iter().filter(|c| matches!(c.kind, Kind::Check(_))) {
            let _ = SetWindowTheme(c.hwnd, w!(""), w!(""));
        }
        // Hidden until needed: hidden controls are skipped by Tab.
        for c in &ctls {
            let hidden = EDITOR.contains(&c.id)
                || (ID_BTN_CHANGE0..ID_BTN_CHANGE0 + MAX_BUTTONS).contains(&c.id)
                || [ID_BAR_DISCARD, ID_BAR_SAVE, ID_PROFILE_RESTORE, ID_FACTORY].contains(&c.id)
                || (ID_PRF_USE0..ID_PRF_REN0 + MAX_ROWS).contains(&c.id)
                || (ID_GAM_CFG0..ID_GAM_NEVER0 + MAX_NEW).contains(&c.id)
                || (ID_GAM_SEL0..ID_GAM_DEL0 + MAX_PROGS).contains(&c.id)
                || [
                    ID_PRF_PREV,
                    ID_PRF_NEXT,
                    ID_GAM_TITLE,
                    ID_GAM_AUTO,
                    ID_GAM_ASK,
                    ID_GAM_ADD,
                    ID_GAM_UNREG,
                ]
                .contains(&c.id)
                || [ID_PRF_TITLE, ID_PRF_REAPPLY, ID_PRF_SAVEAS, ID_PRF_NEW].contains(&c.id);
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
        pick: None,
        light_result: None,
        tried_dpi: None,
        tried_light: None,
        resync: false,
        hex_field: None,
        hex_invalid: false,
        content,
        bar,
        bar_h: 0,
        preview: None,
        light_icon: None,
        rows: Vec::new(),
        active_check: None,
        profile_icon: None,
        profile_status: None,
        using: None,
        then_use: None,
        focus_active: false,
        page: None,
        page_shown: 0,
        games_status: None,
        autos_seen: super::snapshot().map(|s| s.autos).unwrap_or(0),
        page_edits_sent: false,
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
    unsafe {
        // Destroys every control of the page with them.
        let _ = DestroyWindow(state.content);
        let _ = DestroyWindow(state.bar);
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

/// Fills the DPI field with the saved DPI (not the one being tried).
fn fill_field_from_device(view: HWND) {
    let _ = view;
    let Some(shared) = super::snapshot() else {
        return;
    };
    let Some(key) = with_app(|a| a.view.mouse.as_ref().map(|m| m.key.clone())).flatten() else {
        return;
    };
    if let Some(base) = snapshot(&shared, &key).and_then(snap_base_dpi) {
        set(ID_DPI_EDIT, &base.to_string());
    }
}

fn set_tabstop(id: i32, on: bool) {
    if let Some(h) = ctl(id) {
        unsafe {
            let style = GetWindowLongW(h, GWL_STYLE) as u32;
            let new = if on {
                style | WS_TABSTOP.0
            } else {
                style & !WS_TABSTOP.0
            };
            if new != style {
                SetWindowLongW(h, GWL_STYLE, new as i32);
            }
        }
    }
}

/// One option of a single-choice group: label, chosen look and state, and
/// the only Tab stop of the group when chosen.
fn option(id: i32, label: &str, chosen: bool, enabled: bool) {
    set(
        id,
        &if chosen {
            format!("{label}, elegido")
        } else {
            label.to_string()
        },
    );
    set_style(
        id,
        if chosen {
            ButtonStyle::Selected
        } else {
            ButtonStyle::Normal
        },
    );
    enable(id, enabled);
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
    let mut light_result = None;
    let mut profile_status = None;
    if let Some((mark, what)) = pending {
        let done = match what {
            Pending::Buttons(_) => shared.writes != mark,
            _ => shared.seq != mark,
        };
        if done {
            let ok = shared.message.as_ref().is_some_and(|m| m.ok);
            match what {
                Pending::Apply(v) => applied = Some((ok, v)),
                Pending::TryLight => light_result = Some(ok),
                Pending::Restore => {
                    restore_text = shared.message.as_ref().map(|m| (m.ok, m.text.clone()));
                    with_app(|a| a.view.mouse.as_mut().map(|m| m.resync = true));
                }
                Pending::Buttons(what) => finish_write(what, shared.write_result.as_ref()),
                Pending::Profiles => {
                    profile_status = shared.message.as_ref().map(|m| (m.ok, m.text.clone()))
                }
                Pending::Games => {
                    let r = shared.message.as_ref().map(|m| (m.ok, m.text.clone()));
                    with_app(|a| a.view.mouse.as_mut().map(|m| m.games_status = r));
                }
            }
            pending = None;
        }
    }
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.pending = pending;
            m.applied = applied;
            m.restore_text = restore_text.clone();
            if light_result.is_some() {
                m.light_result = light_result;
            }
            if profile_status.is_some() {
                m.profile_status = profile_status.clone();
            }
        }
    });

    // The saved DPI or light changed: the controls start again from them,
    // once the profile has been read again.
    let profile_read = snap.and_then(profile_view).is_some();
    let resync = with_app(|a| a.view.mouse.as_ref().is_some_and(|m| m.resync)) == Some(true);
    if resync && profile_read && pending.is_none() {
        with_app(|a| {
            if let Some(m) = a.view.mouse.as_mut() {
                m.resync = false;
                m.pick = None;
                m.tried_dpi = None;
                m.tried_light = None;
                m.light_result = None;
                m.applied = None;
            }
        });
        fill_field_from_device(view);
    }
    // First light pick from what the mouse keeps.
    let needs_pick = with_app(|a| a.view.mouse.as_ref().is_some_and(|m| m.pick.is_none()));
    if needs_pick == Some(true) {
        if let Some(base) = snap.and_then(snap_base_light) {
            let pick = Pick::from_light(base);
            set(ID_HEX_EDIT, &pick.hex);
            with_app(|a| a.view.mouse.as_mut().map(|m| m.pick = Some(pick)));
        }
    }

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
            "Conecta o enciende el ratón para cambiar sus ajustes. Los controles se activarán solos cuando vuelva; los cambios sin guardar se conservan."
        },
    );
    let notice = text::conflicts(&shared.conflicts);
    set(ID_NOTICE, &notice);
    devices_view::set_text_if_changed(view, &format!("OpenPeripheral — Ajustes de {name}"));

    let busy = pending.is_some();
    let typed = ctl(ID_DPI_EDIT).map(get_text).unwrap_or_default();
    let field = parse_field(&typed);
    let edits = with_app(|a| a.view.mouse.as_ref().map(|m| pending_edits(m, snap, field)))
        .flatten()
        .unwrap_or_default();

    // DPI section.
    let dpi = snap.and_then(|s| match &s.status {
        Status::Ok(st) => st.dpi.clone(),
        _ => None,
    });
    let base = snap.and_then(snap_base_dpi);
    let cycle = snap.and_then(profile_view).and_then(|v| v.dpi_cycle);
    set(
        ID_DPI_BADGE,
        &match (edits.dpi, base) {
            (Some(_), Some(b)) => format!("Cambiado · antes: {b}"),
            _ => String::new(),
        },
    );
    if let Some(d) = &dpi {
        set(ID_DPI_DESC, &range_sentence(&d.allowed));
        set(
            ID_DPI_CURRENT,
            &saved_dpi_text(snap.and_then(|s| s.saved_dpi), cycle, base),
        );
        let (lo, hi) = bounds(&d.allowed);
        let from = field.unwrap_or(d.current);
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
        enable(ID_MINUS, can && from > lo);
        enable(ID_PLUS, can && from < hi);
        set(
            ID_MINUS,
            if from > lo {
                "Bajar un paso de DPI"
            } else {
                "Bajar un paso de DPI (mínimo alcanzado)"
            },
        );
        set(
            ID_PLUS,
            if from < hi {
                "Subir un paso de DPI"
            } else {
                "Subir un paso de DPI (máximo alcanzado)"
            },
        );
        let failed = matches!(applied, Some((false, _)));
        set(ID_APPLY, if failed { "Reintentar" } else { "Probar" });
        enable(ID_APPLY, can);
        enable(ID_RST_BTN, can);
        let (status, icon) = if let Some((_, Pending::Apply(v))) = pending {
            (format!("Enviando {v} DPI al ratón para probarlo…"), None)
        } else if invalid {
            (invalid_message(&d.allowed, &typed), Some(false))
        } else if let Some((ok, v)) = applied.filter(|&(_, v)| field == Some(v)) {
            if !ok {
                (
                    format!(
                        "Error: el ratón no respondió; sigue en {} DPI. Muévelo para despertarlo y vuelve a intentarlo.",
                        d.current
                    ),
                    Some(false),
                )
            } else if edits.dpi == Some(v) {
                (
                    format!(
                        "Probando {v} DPI en el ratón. Guárdalo para que se mantenga al apagarlo."
                    ),
                    None,
                )
            } else {
                (String::new(), None)
            }
        } else if edits.dpi.is_some() {
            (
                "Pulsa «Probar» para notarlo en el ratón, o guárdalo directamente.".to_string(),
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

    // Light section: only for a mouse that declares light effects.
    refresh_light(snap, &shared, connected, busy, pending, &edits);

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
        Profile::Read(v) => (String::new(), v.buttons.clone()),
        Profile::NotRead if connected => ("Leyendo el perfil del ratón…".into(), Vec::new()),
        Profile::NotRead => (String::new(), Vec::new()),
        Profile::Unsupported => (
            "Este ratón todavía no permite cambiar sus botones desde OpenPeripheral.".into(),
            Vec::new(),
        ),
        Profile::Failed(e) => (
            format!("Error: no se pudo leer el perfil del ratón ({e})."),
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
    let (editing, result) = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| (m.editing.clone(), m.buttons_result.clone()))
    })
    .flatten()
    .unwrap_or_default();
    let writing = matches!(pending, Some((_, Pending::Buttons(_))));
    let edit_row = editing.as_ref().map(|e| e.row);
    for i in 0..MAX_BUTTONS {
        let row = i as usize;
        let device = buttons.get(row).copied();
        let new = edits
            .buttons
            .iter()
            .find(|(r, _)| *r == row)
            .map(|(_, a)| *a);
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

    // Pending changes bar: DPI, light and buttons, one save.
    let n = count(&edits);
    let hex_bad = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .and_then(|m| m.pick.as_ref())
            .is_some_and(|p| p.uses_color() && p.color().is_none())
    }) == Some(true);
    let failed = matches!(result, Some((false, _)));
    if n == 0 {
        set(ID_BAR_TEXT, "");
    } else {
        set(
            ID_BAR_TEXT,
            &format!(
                "{} sin guardar. Todavía no se han escrito en el ratón.",
                plural(n, "cambio", "cambios")
            ),
        );
    }
    show(ID_BAR_DISCARD, n > 0 || hex_bad);
    show(ID_BAR_SAVE, n > 0);
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
        connected && !busy && !ghub && editing.is_none() && editable && !hex_bad,
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

    refresh_profiles(snap, &shared, connected, pending, editing.is_some());
    refresh_games(snap, &shared, pending, !edits.buttons.is_empty());

    // Restore section.
    let backup = match snap.and_then(|s| s.backup.clone()) {
        Some(Ok(date)) => format!(
            "Copia del perfil original guardada en este PC el {}. Incluye el DPI, la luz y los botones.",
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
            "Cierra G HUB para guardar en el ratón."
        } else {
            ""
        },
    );
    for id in [ID_PROFILE_RESTORE, ID_FACTORY] {
        show(id, editable);
        enable(id, connected && !busy && !ghub && editing.is_none());
    }
    let original = snap.and_then(|s| s.original_dpi);
    set(
        ID_RST_DESC,
        &match original {
            Some(o) => format!(
                "«Restaurar originales» deja de aplicar el DPI y la luz de OpenPeripheral al conectar el ratón y vuelve al DPI que tenía al detectarlo ({o}) y a la luz de su perfil. No escribe en el ratón."
            ),
            None => "«Restaurar originales» deja de aplicar el DPI y la luz de OpenPeripheral al conectar el ratón. No escribe en el ratón.".into(),
        },
    );
    let rst = match (pending, &restore_text) {
        (Some((_, Pending::Restore)), _) => "Restaurando…".to_string(),
        (_, Some((true, t))) => t.clone(),
        (_, Some((false, t))) => format!("Error: {t}"),
        _ => String::new(),
    };
    set(ID_RST_STATUS, &rst);
    keep_focus_visible();

    // Report rate.
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

    devices_view::relayout(view);
    unsafe {
        let _ = RedrawWindow(
            Some(view),
            None,
            None,
            RDW_INVALIDATE | RDW_ALLCHILDREN | RDW_ERASE,
        );
    }
    // «Guardar y cambiar»: the save ended well, now the profile.
    let then = with_app(|a| {
        a.view
            .mouse
            .as_mut()
            .filter(|m| m.pending.is_none())
            .and_then(|m| m.then_use.take())
    })
    .flatten();
    if let Some(name) = then {
        use_profile(view, name);
    }
}

/// Texts, states and Tab stops of the «Perfiles» section.
fn refresh_profiles(
    snap: Option<&Snapshot>,
    shared: &Shared,
    connected: bool,
    pending: Option<(u64, Pending)>,
    editing: bool,
) {
    let profiles = snap.map(|s| s.profiles.clone()).unwrap_or_default();
    let active = snap.and_then(|s| s.active_profile.clone());
    let n = profiles.len();
    let has = n > 0;
    show(ID_PRF_TITLE, has);
    set(
        ID_PRF_COUNT,
        &if has { texts::count(n) } else { String::new() },
    );
    let busy = pending.is_some();
    let ghub = shared.conflicts.contains(&"Logitech G HUB");
    let can_use = connected && !busy && !ghub && !editing;
    let active_p = profiles.iter().find(|p| Some(&p.name) == active.as_ref());
    let holds = snap.zip(active_p).and_then(|(s, p)| texts::holds(s, p));
    let mismatch = if busy {
        None
    } else {
        snap.zip(active_p).and_then(|(s, p)| texts::mismatch(s, p))
    };
    set(ID_PRF_MISMATCH, mismatch.as_deref().unwrap_or(""));
    show(ID_PRF_REAPPLY, mismatch.is_some());
    show(ID_PRF_SAVEAS, mismatch.is_some());
    if let Some(p) = active_p {
        set(ID_PRF_REAPPLY, &format!("Volver a aplicar «{}»", p.name));
    }
    enable(ID_PRF_REAPPLY, can_use);
    enable(ID_PRF_SAVEAS, !busy && n < crate::profiles::MAX_PROFILES);
    let reason = snap.and_then(|s| texts::blocked(s, ghub));
    // Pages of 5 (decision of the user): the chosen one, else the active's.
    let pages = texts::page_count(n);
    let active_index = profiles
        .iter()
        .position(|p| Some(&p.name) == active.as_ref());
    let chosen = with_app(|a| a.view.mouse.as_ref().and_then(|m| m.page)).flatten();
    let page = chosen
        .or(active_index.map(texts::page_of))
        .unwrap_or(0)
        .min(pages - 1);
    with_app(|a| a.view.mouse.as_mut().map(|m| m.page_shown = page));
    let paged = n > texts::PAGE;
    show(ID_PRF_PREV, paged);
    show(ID_PRF_NEXT, paged);
    set(
        ID_PRF_PAGE,
        &if paged {
            texts::page_label(page, n)
        } else {
            String::new()
        },
    );
    enable(ID_PRF_PREV, page > 0);
    enable(ID_PRF_NEXT, page + 1 < pages);
    set(
        ID_PRF_GAME,
        &snap.and_then(texts::game_notice).unwrap_or_default(),
    );
    for i in 0..MAX_ROWS {
        let row = i as usize;
        let Some(p) = profiles.get(row).filter(|_| texts::page_of(row) == page) else {
            set(ID_PRF_NAME0 + i, "");
            set(ID_PRF_SUM0 + i, "");
            for id in [ID_PRF_USE0, ID_PRF_DEL0, ID_PRF_REN0] {
                show(id + i, false);
            }
            continue;
        };
        let is_active = Some(&p.name) == active.as_ref();
        let surface = if is_active {
            Surface::Selected
        } else {
            Surface::Card
        };
        set(ID_PRF_NAME0 + i, &p.name);
        set(ID_PRF_SUM0 + i, &texts::summary(p));
        set_kind(ID_PRF_NAME0 + i, Kind::Text(Role::Strong, surface));
        set_kind(ID_PRF_SUM0 + i, Kind::Text(Role::Secondary, surface));
        show(ID_PRF_USE0 + i, !is_active);
        show(ID_PRF_DEL0 + i, !is_active);
        show(ID_PRF_REN0 + i, is_active);
        // Accessible names carry the profile; the reason when blocked.
        let use_name = match (reason, can_use) {
            (Some(r), _) => format!("Usar el perfil {} ({})", p.name, r),
            (None, _) => format!("Usar el perfil {}", p.name),
        };
        set(ID_PRF_USE0 + i, &use_name);
        set(ID_PRF_DEL0 + i, &format!("Eliminar el perfil {}", p.name));
        set(ID_PRF_REN0 + i, &format!("Renombrar el perfil {}", p.name));
        enable(ID_PRF_USE0 + i, can_use);
        enable(ID_PRF_DEL0 + i, !busy);
        enable(ID_PRF_REN0 + i, !busy);
    }
    set(
        ID_PRF_ACTIVE,
        if active_p.is_some() {
            texts::active_label(holds, snap.is_some_and(|s| s.game.is_some()))
        } else {
            ""
        },
    );
    set(ID_PRF_FIRST, if has { texts::first_time(n) } else { "" });
    show(ID_PRF_NEW, has);
    let full = n >= crate::profiles::MAX_PROFILES;
    set(
        ID_PRF_NEW,
        if full {
            "Nuevo perfil (límite de 8 alcanzado)"
        } else {
            "Nuevo perfil…"
        },
    );
    enable(ID_PRF_NEW, !busy && !full && active_p.is_some());
    set(
        ID_PRF_NOTE,
        &if has {
            texts::new_note(n)
        } else {
            String::new()
        },
    );
    if has {
        set(
            ID_PAGE_NOTE,
            "Cada perfil guarda el DPI, la luz y los botones. «Usar» escribe el perfil en la memoria del ratón, así que funciona aunque cierres OpenPeripheral y en otros PC. «Probar» lo envía al ratón sin guardarlo.",
        );
    }
    // Status: switching, the last result, or why profiles cannot be used.
    let (using, result, focus) = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| (m.using.clone(), m.profile_status.clone(), m.focus_active))
    })
    .flatten()
    .unwrap_or_default();
    let (status, icon) = if let (Some((_, Pending::Buttons(Write::Use))), Some((to, _))) =
        (pending, &using)
    {
        (
            format!("Cambiando a «{to}»… No apagues ni desconectes el ratón hasta que termine."),
            None,
        )
    } else if let Some((ok, t)) = result {
        (t, Some(ok))
    } else if n >= 2 {
        (reason.unwrap_or("").to_string(), None)
    } else {
        (String::new(), None)
    };
    set(ID_PRF_STATUS, &status);
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.profile_icon = icon.map(|ok| (0, 0, ok));
            m.focus_active = false;
        }
    });
    // After "Usar", the focus goes to the row that is now active.
    if focus {
        if let Some(i) = profiles
            .iter()
            .position(|p| Some(&p.name) == active.as_ref())
        {
            if let Some(h) = ctl(ID_PRF_REN0 + i as i32) {
                unsafe {
                    let _ = SetFocus(Some(h));
                }
            }
        }
    }
}

/// Texts and states of «Juegos y programas».
fn refresh_games(
    snap: Option<&Snapshot>,
    shared: &Shared,
    pending: Option<(u64, Pending)>,
    button_edits: bool,
) {
    let has = snap.is_some_and(|s| !s.profiles.is_empty());
    for id in [
        ID_GAM_TITLE,
        ID_GAM_AUTO,
        ID_GAM_ASK,
        ID_GAM_ADD,
        ID_GAM_UNREG,
    ] {
        show(id, has);
    }
    let busy = pending.is_some();
    let Some(snap) = snap.filter(|_| has) else {
        for i in 0..MAX_NEW {
            set(ID_GAM_NEW0 + i, "");
            show(ID_GAM_CFG0 + i, false);
            show(ID_GAM_NEVER0 + i, false);
        }
        for i in 0..MAX_PROGS {
            set(ID_GAM_NAME0 + i, "");
            set(ID_GAM_PATH0 + i, "");
            show(ID_GAM_SEL0 + i, false);
            show(ID_GAM_DEL0 + i, false);
        }
        set(ID_GAM_EMPTY, "");
        set(ID_GAM_NEW_TITLE, "");
        set(ID_GAM_ASK_HELP, "");
        set(ID_GAM_DESC, "");
        return;
    };
    // The worker must not write buttons over unsaved ones in this page.
    let sent = with_app(|a| a.view.mouse.as_ref().map(|m| m.page_edits_sent)).flatten();
    if sent != Some(button_edits) {
        with_app(|a| {
            a.view
                .mouse
                .as_mut()
                .map(|m| m.page_edits_sent = button_edits)
        });
        send(Command::PageEdits(snap.info.key.clone(), button_edits));
    }
    for (id, on) in [(ID_GAM_AUTO, snap.auto_switch), (ID_GAM_ASK, snap.ask_new)] {
        if let Some(h) = ctl(id) {
            unsafe {
                SendMessageW(h, BM_SETCHECK, Some(WPARAM(on as usize)), None);
            }
            enable(id, !busy);
        }
    }
    set(
        ID_GAM_DESC,
        "Al abrir uno de estos programas, OpenPeripheral usa su perfil: el DPI y la luz al momento, y escribe los botones en el ratón solo si cambian. Al cerrarlo, vuelve el perfil que había. Solo funciona con OpenPeripheral abierto.",
    );
    set(
        ID_GAM_ASK_HELP,
        "Juegos de Steam, Xbox, Epic o GOG. Se pregunta con una notificación de Windows.",
    );
    let new = &snap.unanswered;
    set(
        ID_GAM_NEW_TITLE,
        if new.is_empty() {
            ""
        } else {
            "Juegos detectados sin responder"
        },
    );
    for i in 0..MAX_NEW {
        let name = new.get(i as usize).map(|e| crate::games::display_name(e));
        set(ID_GAM_NEW0 + i, name.as_deref().unwrap_or(""));
        show(ID_GAM_CFG0 + i, name.is_some());
        show(ID_GAM_NEVER0 + i, name.is_some());
        if let Some(n) = &name {
            set(ID_GAM_CFG0 + i, &format!("Configurar {n}"));
            set(ID_GAM_NEVER0 + i, &format!("No preguntar más por {n}"));
        }
        enable(ID_GAM_CFG0 + i, !busy);
        enable(ID_GAM_NEVER0 + i, !busy);
    }
    for i in 0..MAX_PROGS {
        let Some(p) = snap.programs.get(i as usize) else {
            set(ID_GAM_NAME0 + i, "");
            set(ID_GAM_PATH0 + i, "");
            show(ID_GAM_SEL0 + i, false);
            show(ID_GAM_DEL0 + i, false);
            continue;
        };
        let name = crate::games::display_name(&p.exe);
        let in_use = snap.game.as_deref() == Some(name.as_str());
        set(
            ID_GAM_NAME0 + i,
            &if in_use {
                format!("{name} · abierto")
            } else {
                name.clone()
            },
        );
        set(ID_GAM_PATH0 + i, &p.exe);
        show(ID_GAM_SEL0 + i, true);
        show(ID_GAM_DEL0 + i, true);
        // The accessible name carries the program; the label is drawn.
        set(
            ID_GAM_SEL0 + i,
            &format!("Perfil de {name}: {}", texts::program_profile(p)),
        );
        set(ID_GAM_DEL0 + i, &format!("Quitar {name}"));
        enable(ID_GAM_SEL0 + i, !busy);
        enable(ID_GAM_DEL0 + i, !busy);
    }
    set(
        ID_GAM_EMPTY,
        if snap.programs.is_empty() {
            "Todavía no hay programas. Se añadirán al responder a la notificación de un juego nuevo, o con «Añadir programa…»."
        } else {
            ""
        },
    );
    enable(
        ID_GAM_ADD,
        !busy && snap.programs.len() < crate::games::MAX_PROGRAMS,
    );
    show(ID_GAM_UNREG, crate::platform::toast::registered());
    // A failed automatic switch is shown here too, not only as a balloon.
    let seen = with_app(|a| a.view.mouse.as_ref().map(|m| m.autos_seen)).flatten();
    if seen.is_some_and(|s| s != shared.autos) {
        let r = shared.auto_result.as_ref().map(|m| (m.ok, m.text.clone()));
        with_app(|a| {
            a.view.mouse.as_mut().map(|m| {
                m.autos_seen = shared.autos;
                if r.is_some() {
                    m.games_status = r;
                }
            })
        });
    }
    let status = with_app(|a| a.view.mouse.as_ref().and_then(|m| m.games_status.clone())).flatten();
    set(
        ID_GAM_STATUS,
        &match status {
            Some((false, t)) if !t.starts_with("Error") => format!("Error: {t}"),
            Some((_, t)) => t,
            None => String::new(),
        },
    );
}

/// Texts, states and Tab stops of the light section.
fn refresh_light(
    snap: Option<&Snapshot>,
    shared: &Shared,
    connected: bool,
    busy: bool,
    pending: Option<(u64, Pending)>,
    edits: &ProfileEdits,
) {
    let _ = shared;
    let effects = snap
        .and_then(profile_view)
        .map(|v| v.effects.clone())
        .unwrap_or_default();
    let (pick, tried, result) = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| (m.pick.clone(), m.tried_light, m.light_result))
    })
    .flatten()
    .unwrap_or_default();
    let shown = !effects.is_empty() && pick.is_some();
    let all: Vec<i32> = [
        ID_LIGHT_TITLE,
        ID_LIGHT_BADGE,
        ID_LIGHT_DESC,
        ID_EFFECT_LABEL,
        ID_COLOR_LABEL,
        ID_HEX_LABEL,
        ID_HEX_EDIT,
        ID_HEX_NAME,
        ID_SPEED_LABEL,
        ID_LIGHT_NOTE,
        ID_LIGHT_TRY,
        ID_LIGHT_SAVED,
        ID_LIGHT_STATUS,
    ]
    .into_iter()
    .chain(ID_EFFECT0..ID_EFFECT0 + N_EFFECTS)
    .chain(ID_SWATCH0..ID_SWATCH0 + N_SWATCHES)
    .chain(ID_SPEED0..ID_SPEED0 + N_SPEEDS)
    .collect();
    let Some(pick) = pick.filter(|_| shown) else {
        for id in all {
            show(id, false);
        }
        with_app(|a| {
            if let Some(m) = a.view.mouse.as_mut() {
                m.hex_invalid = false;
                m.light_icon = None;
            }
        });
        return;
    };
    let can = connected && !busy;
    let names: Vec<&str> = light::EFFECTS
        .iter()
        .filter(|(id, _)| effects.contains(id))
        .map(|(_, n)| *n)
        .collect();
    let list = match names.as_slice() {
        [] => String::new(),
        [one] => one.to_lowercase(),
        [rest @ .., last] => format!(
            "{} y {}",
            rest.iter()
                .map(|n| n.to_lowercase())
                .collect::<Vec<_>>()
                .join(", "),
            last.to_lowercase()
        ),
    };
    set(
        ID_LIGHT_DESC,
        &format!("Una zona de luz. Este ratón admite: {list}."),
    );
    let base = snap.and_then(snap_base_light);
    set(
        ID_LIGHT_BADGE,
        &match (edits.light, base) {
            (Some(_), Some(b)) => format!("Cambiado · antes: {}", light::describe(b)),
            _ => String::new(),
        },
    );
    for id in [
        ID_LIGHT_TITLE,
        ID_LIGHT_BADGE,
        ID_LIGHT_DESC,
        ID_EFFECT_LABEL,
    ] {
        show(id, true);
    }
    // Effects the mouse declares; the chosen one is the group's Tab stop.
    for (i, (eid, label)) in light::EFFECTS.iter().enumerate() {
        let id = ID_EFFECT0 + i as i32;
        let offered = effects.contains(eid);
        show(id, offered);
        let chosen = pick.effect == *eid;
        option(id, label, chosen, can);
        set_tabstop(id, offered && chosen);
    }
    // Color.
    let color = pick.color();
    let uses_color = pick.uses_color();
    for id in [ID_COLOR_LABEL, ID_HEX_LABEL, ID_HEX_EDIT, ID_HEX_NAME] {
        show(id, uses_color);
    }
    let chosen_swatch =
        color.and_then(|c| (0..light::SWATCHES.len()).find(|&i| light::swatch(i) == c));
    for i in 0..light::SWATCHES.len() {
        let id = ID_SWATCH0 + i as i32;
        show(id, uses_color);
        let (name, value) = light::SWATCHES[i];
        let chosen = chosen_swatch == Some(i);
        option(
            id,
            &format!("{name}, {}", crate::config::to_hex(light::swatch(i))),
            chosen,
            can,
        );
        set_tabstop(id, uses_color && chosen_swatch.map_or(i == 0, |c| c == i));
        let _ = value;
    }
    enable(ID_HEX_EDIT, can);
    set(
        ID_HEX_NAME,
        &color
            .map(light::color_name)
            .unwrap_or_else(|| "Sin vista previa".into()),
    );
    // Speed.
    let uses_speed = pick.uses_speed();
    show(ID_SPEED_LABEL, uses_speed);
    for i in 0..light::SPEEDS.len() {
        let id = ID_SPEED0 + i as i32;
        show(id, uses_speed);
        let chosen = pick.speed == i;
        option(id, light::SPEEDS[i], chosen, can);
        set_tabstop(id, uses_speed && chosen);
    }
    let note = match pick.effect {
        0x00 => "Con la luz apagada no hay color ni velocidad que elegir.",
        0x03 => "El ciclo recorre todos los colores; solo se elige la velocidad.",
        _ => "",
    };
    set(ID_LIGHT_NOTE, note);
    show(ID_LIGHT_NOTE, !note.is_empty());
    // Probar and the saved light.
    let hex_invalid = uses_color && color.is_none();
    let failed = result == Some(false);
    show(ID_LIGHT_TRY, true);
    set(
        ID_LIGHT_TRY,
        if failed {
            "Reintentar"
        } else {
            "Probar la luz"
        },
    );
    enable(ID_LIGHT_TRY, can && !hex_invalid);
    show(ID_LIGHT_SAVED, true);
    set(
        ID_LIGHT_SAVED,
        &base
            .map(|b| format!("Guardado: {}", light::describe_cap(b)))
            .unwrap_or_default(),
    );
    let now = pick.light();
    let (status, icon) = if matches!(pending, Some((_, Pending::TryLight))) {
        (
            format!(
                "Enviando la luz {} al ratón para probarla…",
                now.map(light::describe).unwrap_or_default()
            ),
            None,
        )
    } else if hex_invalid {
        (light::invalid_hex(&pick.hex), Some(false))
    } else if failed {
        (
            "Error: el ratón no respondió; la luz del ratón no ha cambiado. Muévelo para despertarlo y vuelve a intentarlo.".to_string(),
            Some(false),
        )
    } else if let Some(l) = edits.light {
        if tried == Some(l) {
            (
                format!(
                    "Probando en el ratón: {}. Guárdalo para que se mantenga al apagarlo.",
                    light::describe(l)
                ),
                None,
            )
        } else {
            (
                "Pulsa «Probar la luz» para verla en el ratón, o guárdala directamente."
                    .to_string(),
                None,
            )
        }
    } else {
        (String::new(), None)
    };
    show(ID_LIGHT_STATUS, true);
    set(ID_LIGHT_STATUS, &status);
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.hex_invalid = hex_invalid;
            m.light_icon = icon.map(|ok| (0, 0, ok));
        }
    });
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
        .chain([ID_PROFILE_RESTORE, ID_APPLY, ID_BACK])
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
            if what == Write::Use {
                let (to, from) = m.using.take().unwrap_or_default();
                let text = if !ok {
                    format!(
                        "Error: no se pudo cambiar a «{to}». {cause} El ratón sigue con «{from}»."
                    )
                } else if cause.contains("ya lo tenía") {
                    format!("Perfil «{to}» activo en el ratón (ya lo tenía).")
                } else {
                    format!(
                        "Perfil «{to}» activo en el ratón. Se comprobó la escritura leyendo de nuevo el perfil."
                    )
                };
                if ok {
                    m.resync = true;
                }
                m.focus_active = ok;
                m.profile_status = Some((ok, text));
                return;
            }
            if what == Write::Save && !ok {
                // «Guardar y cambiar»: nothing is used after a failed save.
                m.then_use = None;
            }
            let text = match (what, ok) {
                (Write::Save, true) => "Guardado en el ratón. Se comprobó la escritura leyendo de nuevo el perfil.".to_string(),
                (Write::Original, true) => "Perfil original restaurado en el ratón. Se comprobó leyendo de nuevo el perfil.".to_string(),
                (Write::Factory, true) => "Valores de fábrica restaurados en el ratón. Se comprobó leyendo de nuevo el perfil.".to_string(),
                (Write::Use, true) => String::new(),
                (Write::Save, false) => format!(
                    "Error: no se pudo guardar en el ratón. {cause} Tus cambios siguen pendientes en esta pantalla."
                ),
                (_, false) => format!("Error: no se pudo restaurar el perfil. {cause}"),
            };
            if ok && what == Write::Save {
                m.edits.clear();
            }
            if ok {
                // The saved DPI and light changed: start again from them.
                m.resync = true;
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

fn inner_of(col: i32, pad: i32) -> i32 {
    col - 2 * pad
}

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
    let (_, nth) = measure(ID_PAGE_NOTE, Role::Secondary, col);
    put(ID_PAGE_NOTE, left, y, col, nth);
    y += nth + px(20.0);

    let pad = px(if g.compact { 16.0 } else { 20.0 });
    let mut badges = Vec::new();
    let mut separators = Vec::new();
    // The control's own flag: a control of the hidden bar is still "shown".
    let visible =
        |id: i32| unsafe { GetWindowLongW(h_of(id), GWL_STYLE) as u32 & WS_VISIBLE.0 != 0 };
    // Section title with the "Cambiado · antes: …" outline at its right
    // (below it in a narrow window).
    let title_row = |title: i32,
                     badge: i32,
                     cy: i32,
                     put: &mut dyn FnMut(i32, i32, i32, i32, i32),
                     badges: &mut Vec<RECT>|
     -> i32 {
        let (tw, th) = measure(title, Role::Name, inner_of(col, pad));
        let (bw, bdh) = measure(badge, Role::Tertiary, inner_of(col, pad) - px(12.0));
        let ix = left + pad;
        let inner = inner_of(col, pad);
        put(title, ix, cy, tw + 2, th);
        if bdh == 0 {
            put(badge, 0, 0, 0, 0);
            return th;
        }
        let fits = !g.compact && tw + px(16.0) + bw + px(12.0) <= inner;
        let (bx, by) = if fits {
            (ix + inner - bw - px(12.0), cy + (th - bdh - px(4.0)) / 2)
        } else {
            (ix, cy + th + px(6.0))
        };
        let r = RECT {
            left: bx,
            top: by,
            right: bx + bw + px(12.0),
            bottom: by + bdh + px(4.0),
        };
        put(badge, r.left + px(6.0), r.top + px(2.0), bw + 2, bdh);
        badges.push(r);
        if fits { th } else { r.bottom - cy }
    };
    let inner = col - 2 * pad;
    let ix = left + pad;

    // Profiles section.
    let mut rows = Vec::new();
    let mut active_check = None;
    let mut profile_status_xy = None;
    let row_ids = |i: i32| {
        [
            ID_PRF_NAME0 + i,
            ID_PRF_SUM0 + i,
            ID_PRF_USE0 + i,
            ID_PRF_DEL0 + i,
            ID_PRF_REN0 + i,
        ]
    };
    if visible(ID_PRF_TITLE) {
        let top = y;
        let mut cy = y + pad;
        let (tw, th) = measure(ID_PRF_TITLE, Role::Name, inner);
        put(ID_PRF_TITLE, ix, cy, tw + 2, th);
        let (cw, ch) = measure(ID_PRF_COUNT, Role::Tertiary, inner);
        put(
            ID_PRF_COUNT,
            ix + inner - cw - 2,
            cy + (th - ch) / 2,
            cw + 2,
            ch,
        );
        cy += th + px(12.0);
        let (_, gh) = measure(ID_PRF_GAME, Role::Notice, inner);
        if gh > 0 {
            put(ID_PRF_GAME, ix, cy, inner, gh);
            cy += gh + px(12.0);
        } else {
            put(ID_PRF_GAME, 0, 0, 0, 0);
        }
        // The mouse does not hold the active profile.
        let (_, mh) = measure(ID_PRF_MISMATCH, Role::Notice, inner);
        if mh > 0 && visible(ID_PRF_REAPPLY) {
            put(ID_PRF_MISMATCH, ix, cy, inner, mh);
            cy += mh + px(10.0);
            // Drawn in semibold (primary style): wider than measured.
            let rw = btn_w(&txt(ID_PRF_REAPPLY)) + px(16.0);
            let sw = btn_w("Guardar como perfil nuevo…");
            if g.compact {
                put(
                    ID_PRF_REAPPLY,
                    ix - ring,
                    cy - ring,
                    inner + 2 * ring,
                    bh + 2 * ring,
                );
                cy += bh + px(10.0);
                put(
                    ID_PRF_SAVEAS,
                    ix - ring,
                    cy - ring,
                    inner + 2 * ring,
                    bh + 2 * ring,
                );
            } else {
                put(
                    ID_PRF_REAPPLY,
                    ix - ring,
                    cy - ring,
                    rw + 2 * ring,
                    bh + 2 * ring,
                );
                let sx = ix + rw + px(12.0);
                put(
                    ID_PRF_SAVEAS,
                    sx - ring,
                    cy - ring,
                    sw + 2 * ring,
                    bh + 2 * ring,
                );
            }
            cy += bh + px(14.0);
        } else {
            for id in [ID_PRF_MISMATCH, ID_PRF_REAPPLY, ID_PRF_SAVEAS] {
                put(id, 0, 0, 0, 0);
            }
        }
        // One row per profile.
        let rpad = px(if g.compact { 12.0 } else { 14.0 });
        let rin = inner - 2 * rpad;
        let rx = ix + rpad;
        let gap = px(8.0);
        let mut active_seen = false;
        for i in 0..MAX_ROWS {
            if txt(ID_PRF_NAME0 + i).is_empty() {
                for id in row_ids(i) {
                    put(id, 0, 0, 0, 0);
                }
                continue;
            }
            let is_active = visible(ID_PRF_REN0 + i);
            let buttons: Vec<(i32, i32)> = if is_active {
                vec![(ID_PRF_REN0 + i, btn_w("Renombrar…"))]
            } else {
                vec![
                    (ID_PRF_USE0 + i, btn_w("Usar")),
                    (ID_PRF_DEL0 + i, btn_w("Eliminar…")),
                ]
            };
            let buttons_w = buttons.iter().map(|(_, w)| w + gap).sum::<i32>() - gap;
            let text_w = if g.compact {
                rin
            } else {
                rin - buttons_w - px(16.0)
            };
            let row_top = cy;
            let mut ry = cy + rpad;
            let (nw, nh) = measure(ID_PRF_NAME0 + i, Role::Strong, text_w);
            put(ID_PRF_NAME0 + i, rx, ry, nw + 2, nh);
            let mut name_h = nh;
            if is_active {
                // Check and «Activo · en el ratón» after the name, or below.
                let check = px(22.0);
                let (aw, ah) = measure(ID_PRF_ACTIVE, Role::Strong, text_w - check);
                let fits = nw + px(10.0) + check + aw <= text_w;
                let (ax, ay) = if fits {
                    (rx + nw + px(10.0), ry)
                } else {
                    (rx, ry + nh + px(2.0))
                };
                active_check = Some((ax, ay + (ah - px(16.0)) / 2));
                put(ID_PRF_ACTIVE, ax + check, ay, aw + 2, ah);
                if !fits {
                    name_h += ah + px(2.0);
                }
                active_seen = true;
            }
            ry += name_h + px(2.0);
            let (_, sh) = measure(ID_PRF_SUM0 + i, Role::Secondary, text_w);
            put(ID_PRF_SUM0 + i, rx, ry, text_w, sh);
            ry += sh;
            let row_bottom = if g.compact {
                ry += px(10.0);
                let n = buttons.len() as i32;
                let w = (rin - gap * (n - 1)) / n;
                for (k, (id, _)) in buttons.iter().enumerate() {
                    let bx = rx + (w + gap) * k as i32;
                    put(*id, bx - ring, ry - ring, w + 2 * ring, bh + 2 * ring);
                }
                ry + bh + rpad
            } else {
                let content_h = (ry - (cy + rpad)).max(bh);
                let by = cy + rpad + (content_h - bh) / 2;
                let mut bx = rx + rin - buttons_w;
                for (id, w) in &buttons {
                    put(*id, bx - ring, by - ring, w + 2 * ring, bh + 2 * ring);
                    bx += w + gap;
                }
                cy + rpad + content_h + rpad
            };
            for id in [ID_PRF_USE0 + i, ID_PRF_DEL0 + i, ID_PRF_REN0 + i] {
                if !buttons.iter().any(|(b, _)| *b == id) {
                    put(id, 0, 0, 0, 0);
                }
            }
            rows.push((
                RECT {
                    left: ix,
                    top: row_top,
                    right: ix + inner,
                    bottom: row_bottom,
                },
                is_active,
            ));
            cy = row_bottom + gap;
        }
        if !active_seen {
            put(ID_PRF_ACTIVE, 0, 0, 0, 0);
        }
        if visible(ID_PRF_PREV) {
            let pw = btn_w("‹ Anterior");
            let nw = btn_w("Siguiente ›");
            let (lw, lh) = measure(ID_PRF_PAGE, Role::Secondary, inner);
            put(
                ID_PRF_PREV,
                ix - ring,
                cy - ring,
                pw + 2 * ring,
                bh + 2 * ring,
            );
            put(
                ID_PRF_PAGE,
                ix + pw + px(16.0),
                cy + (bh - lh) / 2,
                lw + 2,
                lh,
            );
            let nx = ix + pw + px(32.0) + lw;
            put(
                ID_PRF_NEXT,
                nx - ring,
                cy - ring,
                nw + 2 * ring,
                bh + 2 * ring,
            );
            cy += bh + px(8.0);
        } else {
            for id in [ID_PRF_PREV, ID_PRF_PAGE, ID_PRF_NEXT] {
                put(id, 0, 0, 0, 0);
            }
        }
        cy += px(6.0);
        let (_, fh) = measure(ID_PRF_FIRST, Role::Secondary, inner);
        if fh > 0 {
            put(ID_PRF_FIRST, ix, cy, inner, fh);
            cy += fh + px(10.0);
        } else {
            put(ID_PRF_FIRST, 0, 0, 0, 0);
        }
        let nw = btn_w(&txt(ID_PRF_NEW)).max(btn_w("Nuevo perfil…"));
        if g.compact {
            put(
                ID_PRF_NEW,
                ix - ring,
                cy - ring,
                inner + 2 * ring,
                bh + 2 * ring,
            );
            cy += bh + px(8.0);
            let (_, h) = measure(ID_PRF_NOTE, Role::Tertiary, inner);
            put(ID_PRF_NOTE, ix, cy, inner, h);
            cy += h;
        } else {
            put(
                ID_PRF_NEW,
                ix - ring,
                cy - ring,
                nw + 2 * ring,
                bh + 2 * ring,
            );
            let note_w = inner - nw - px(16.0);
            let (_, h) = measure(ID_PRF_NOTE, Role::Tertiary, note_w);
            put(
                ID_PRF_NOTE,
                ix + nw + px(16.0),
                cy + (bh - h).max(0) / 2,
                note_w,
                h,
            );
            cy += bh.max(h);
        }
        let (_, sth) = measure(ID_PRF_STATUS, Role::Secondary, inner - px(24.0));
        if sth > 0 {
            cy += px(12.0);
            put(ID_PRF_STATUS, ix + px(24.0), cy, inner - px(24.0), sth);
            profile_status_xy = Some((ix, cy));
            cy += sth;
        } else {
            put(ID_PRF_STATUS, 0, 0, 0, 0);
        }
        cy += pad;
        cards.push(RECT {
            left,
            top,
            right: left + col,
            bottom: cy,
        });
        y = cy + px(20.0);
    } else {
        for id in [
            ID_PRF_TITLE,
            ID_PRF_COUNT,
            ID_PRF_GAME,
            ID_PRF_PREV,
            ID_PRF_PAGE,
            ID_PRF_NEXT,
            ID_PRF_MISMATCH,
            ID_PRF_REAPPLY,
            ID_PRF_SAVEAS,
            ID_PRF_ACTIVE,
            ID_PRF_FIRST,
            ID_PRF_NEW,
            ID_PRF_NOTE,
            ID_PRF_STATUS,
        ]
        .into_iter()
        .chain((0..MAX_ROWS).flat_map(row_ids))
        {
            put(id, 0, 0, 0, 0);
        }
    }

    // DPI section.
    let top = y;
    let mut cy = y + pad;
    let h = title_row(ID_DPI_TITLE, ID_DPI_BADGE, cy, &mut put, &mut badges);
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
    let apply_w = btn_w("Reintentar").max(btn_w("Probar"));
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
        .filter(|&id| unsafe { GetWindowLongW(h_of(id), GWL_STYLE) as u32 & WS_VISIBLE.0 != 0 })
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

    // Light section.
    let mut hex_field = None;
    let mut preview = None;
    let mut light_status_xy = None;
    if visible(ID_LIGHT_TITLE) {
        let top = y;
        let mut cy = y + pad;
        let h = title_row(ID_LIGHT_TITLE, ID_LIGHT_BADGE, cy, &mut put, &mut badges);
        cy += h + px(4.0);
        let (_, h) = measure(ID_LIGHT_DESC, Role::Secondary, inner);
        put(ID_LIGHT_DESC, ix, cy, inner, h);
        cy += h + px(18.0);
        // A row of option buttons that wraps; a 2-column grid when narrow.
        let options = |ids: &[i32], cy: &mut i32, put: &mut dyn FnMut(i32, i32, i32, i32, i32)| {
            let ids: Vec<i32> = ids.iter().copied().filter(|&id| visible(id)).collect();
            if g.compact {
                let gw = (inner - px(8.0)) / 2;
                for (k, id) in ids.iter().enumerate() {
                    let x = ix + (k as i32 % 2) * (gw + px(8.0));
                    let yy = *cy + (k as i32 / 2) * (bh + px(8.0));
                    put(*id, x - ring, yy - ring, gw + 2 * ring, bh + 2 * ring);
                }
                let rows = (ids.len() as i32 + 1) / 2;
                *cy += rows * bh + (rows - 1).max(0) * px(8.0);
            } else {
                let mut x = ix;
                for id in &ids {
                    let label = get_text(h_of(*id));
                    let label = label.trim_end_matches(", elegido");
                    let w = ctx.measure(label, FontRole::Body, 1000, 1).0 + px(32.0);
                    if x > ix && x + w > ix + inner {
                        x = ix;
                        *cy += bh + px(8.0);
                    }
                    put(*id, x - ring, *cy - ring, w + 2 * ring, bh + 2 * ring);
                    x += w + px(8.0);
                }
                *cy += bh;
            }
        };
        let (lw, lh) = measure(ID_EFFECT_LABEL, Role::Notice, inner);
        put(ID_EFFECT_LABEL, ix, cy, lw + 2, lh);
        cy += lh + px(8.0);
        let effects: Vec<i32> = (0..N_EFFECTS).map(|i| ID_EFFECT0 + i).collect();
        options(&effects, &mut cy, &mut put);
        cy += px(18.0);
        if visible(ID_COLOR_LABEL) {
            let (lw, lh) = measure(ID_COLOR_LABEL, Role::Notice, inner);
            put(ID_COLOR_LABEL, ix, cy, lw + 2, lh);
            cy += lh + px(8.0);
            let sw = px(if g.compact { 44.0 } else { 32.0 });
            let gap = px(10.0);
            let per_row = if g.compact {
                4
            } else {
                ((inner + gap) / (sw + gap)).max(1)
            };
            let cell = if g.compact {
                (inner - gap * (per_row - 1)) / per_row
            } else {
                sw
            };
            for i in 0..N_SWATCHES {
                let (r, c) = (i / per_row, i % per_row);
                let x = ix + c * (cell + gap);
                let yy = cy + r * (sw + gap);
                put(
                    ID_SWATCH0 + i,
                    x - ring,
                    yy - ring,
                    cell + 2 * ring,
                    sw + 2 * ring,
                );
            }
            let rows = (N_SWATCHES + per_row - 1) / per_row;
            cy += rows * sw + (rows - 1) * gap + px(16.0);
            // Hex field, preview and name.
            let (hlw, hlh) = measure(ID_HEX_LABEL, Role::Notice, inner);
            // The preview is as tall (and wide) as a button.
            let fw = if g.compact {
                inner - bh - px(10.0)
            } else {
                px(112.0)
            };
            let (nw, nh) = measure(ID_HEX_NAME, Role::Secondary, inner);
            if g.compact {
                put(ID_HEX_LABEL, ix, cy, hlw + 2, hlh);
                cy += hlh + px(6.0);
            } else {
                put(ID_HEX_LABEL, ix, cy + (bh - hlh) / 2, hlw + 2, hlh);
            }
            let fx = if g.compact { ix } else { ix + hlw + px(12.0) };
            let f = RECT {
                left: fx,
                top: cy,
                right: fx + fw,
                bottom: cy + bh,
            };
            let edit_h = (bh - px(10.0)).max(px(20.0));
            put(
                ID_HEX_EDIT,
                f.left + px(8.0),
                cy + (bh - edit_h) / 2,
                fw - px(16.0),
                edit_h,
            );
            hex_field = Some(f);
            let pv = RECT {
                left: f.right + px(10.0),
                top: cy,
                right: f.right + px(10.0) + bh,
                bottom: cy + bh,
            };
            preview = Some(pv);
            if g.compact {
                cy += bh + px(6.0);
                put(ID_HEX_NAME, ix, cy, nw + 2, nh);
                cy += nh;
            } else {
                put(
                    ID_HEX_NAME,
                    pv.right + px(16.0),
                    cy + (bh - nh) / 2,
                    nw + 2,
                    nh,
                );
                cy += bh;
            }
            cy += px(18.0);
        } else {
            for id in [ID_COLOR_LABEL, ID_HEX_LABEL, ID_HEX_EDIT, ID_HEX_NAME] {
                put(id, 0, 0, 0, 0);
            }
        }
        if visible(ID_SPEED_LABEL) {
            let (lw, lh) = measure(ID_SPEED_LABEL, Role::Notice, inner);
            put(ID_SPEED_LABEL, ix, cy, lw + 2, lh);
            cy += lh + px(8.0);
            let speeds: Vec<i32> = (0..N_SPEEDS).map(|i| ID_SPEED0 + i).collect();
            options(&speeds, &mut cy, &mut put);
            cy += px(18.0);
        } else {
            put(ID_SPEED_LABEL, 0, 0, 0, 0);
        }
        let (_, h) = measure(ID_LIGHT_NOTE, Role::Secondary, inner);
        if h > 0 {
            put(ID_LIGHT_NOTE, ix, cy, inner, h);
            cy += h + px(18.0);
        } else {
            put(ID_LIGHT_NOTE, 0, 0, 0, 0);
        }
        // Probar and the saved light, under a separator.
        separators.push((ix, ix + inner, cy - px(2.0)));
        cy += px(14.0);
        let tw = btn_w("Probar la luz").max(btn_w("Reintentar"));
        let (sw2, sh2) = measure(ID_LIGHT_SAVED, Role::Secondary, inner);
        if g.compact {
            put(
                ID_LIGHT_TRY,
                ix - ring,
                cy - ring,
                inner + 2 * ring,
                bh + 2 * ring,
            );
            cy += bh + px(10.0);
            put(ID_LIGHT_SAVED, ix, cy, inner, sh2);
            cy += sh2;
        } else {
            put(
                ID_LIGHT_TRY,
                ix - ring,
                cy - ring,
                tw + 2 * ring,
                bh + 2 * ring,
            );
            let sx = ix + tw + px(16.0);
            let (_, sh3) = measure(ID_LIGHT_SAVED, Role::Secondary, inner - tw - px(16.0));
            put(
                ID_LIGHT_SAVED,
                sx,
                cy + (bh - sh3) / 2,
                (inner - tw - px(16.0)).min(sw2 + 2),
                sh3,
            );
            cy += bh.max(sh3);
        }
        let (_, sth) = measure(ID_LIGHT_STATUS, Role::Secondary, inner - px(24.0));
        if sth > 0 {
            cy += px(12.0);
            put(ID_LIGHT_STATUS, ix + px(24.0), cy, inner - px(24.0), sth);
            light_status_xy = Some((ix, cy));
            cy += sth;
        } else {
            put(ID_LIGHT_STATUS, 0, 0, 0, 0);
        }
        cy += pad;
        cards.push(RECT {
            left,
            top,
            right: left + col,
            bottom: cy,
        });
        y = cy + px(20.0);
    }

    // Buttons.
    let top = y;
    let mut cy = y + pad;
    let (_, h) = measure(ID_BTN_TITLE, Role::Name, inner);
    put(ID_BTN_TITLE, ix, cy, inner, h);
    cy += h + px(12.0);
    put(ID_BTN_DESC, 0, 0, 0, 0);
    let (_, h) = measure(ID_BTN_STATUS, Role::Notice, inner);
    if h > 0 {
        put(ID_BTN_STATUS, ix, cy, inner, h);
        cy += h + px(12.0);
    } else {
        put(ID_BTN_STATUS, 0, 0, 0, 0);
    }
    let mut edit_box = None;
    let mut key_field = None;
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
    cy += pad - px(10.0);
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

    // «Juegos y programas».
    let gam_ids = [
        ID_GAM_TITLE,
        ID_GAM_DESC,
        ID_GAM_AUTO,
        ID_GAM_ASK,
        ID_GAM_ASK_HELP,
        ID_GAM_NEW_TITLE,
        ID_GAM_EMPTY,
        ID_GAM_ADD,
        ID_GAM_UNREG,
        ID_GAM_STATUS,
    ];
    let new_ids = |i: i32| [ID_GAM_NEW0 + i, ID_GAM_CFG0 + i, ID_GAM_NEVER0 + i];
    let prog_ids = |i: i32| {
        [
            ID_GAM_NAME0 + i,
            ID_GAM_PATH0 + i,
            ID_GAM_SEL0 + i,
            ID_GAM_DEL0 + i,
        ]
    };
    if visible(ID_GAM_TITLE) {
        let top = y;
        let mut cy = y + pad;
        let (_, h) = measure(ID_GAM_TITLE, Role::Name, inner);
        put(ID_GAM_TITLE, ix, cy, inner, h);
        cy += h + px(4.0);
        let (_, h) = measure(ID_GAM_DESC, Role::Secondary, inner);
        put(ID_GAM_DESC, ix, cy, inner, h);
        cy += h + px(14.0);
        for id in [ID_GAM_AUTO, ID_GAM_ASK] {
            put(id, ix, cy, inner, bh);
            cy += bh + px(2.0);
        }
        let (_, h) = measure(ID_GAM_ASK_HELP, Role::Secondary, inner);
        put(ID_GAM_ASK_HELP, ix, cy, inner, h);
        cy += h + px(14.0);
        // Games not answered: name, «Configurar…», «No preguntar más».
        let (_, h) = measure(ID_GAM_NEW_TITLE, Role::Notice, inner);
        if h > 0 {
            put(ID_GAM_NEW_TITLE, ix, cy, inner, h);
            cy += h + px(8.0);
        } else {
            put(ID_GAM_NEW_TITLE, 0, 0, 0, 0);
        }
        let cw = btn_w("Configurar…");
        let vw = btn_w("No preguntar más");
        for i in 0..MAX_NEW {
            if txt(ID_GAM_NEW0 + i).is_empty() {
                for id in new_ids(i) {
                    put(id, 0, 0, 0, 0);
                }
                continue;
            }
            let text_w = if g.compact {
                inner
            } else {
                inner - cw - vw - px(28.0)
            };
            let (_, nh) = measure(ID_GAM_NEW0 + i, Role::Strong, text_w);
            if g.compact {
                put(ID_GAM_NEW0 + i, ix, cy, inner, nh);
                cy += nh + px(6.0);
                let w = (inner - px(8.0)) / 2;
                put(
                    ID_GAM_CFG0 + i,
                    ix - ring,
                    cy - ring,
                    w + 2 * ring,
                    bh + 2 * ring,
                );
                let x2 = ix + w + px(8.0);
                put(
                    ID_GAM_NEVER0 + i,
                    x2 - ring,
                    cy - ring,
                    w + 2 * ring,
                    bh + 2 * ring,
                );
                cy += bh + px(10.0);
            } else {
                let row = nh.max(bh);
                put(ID_GAM_NEW0 + i, ix, cy + (row - nh) / 2, text_w, nh);
                let bx = ix + inner - cw - vw - px(8.0);
                put(
                    ID_GAM_CFG0 + i,
                    bx - ring,
                    cy + (row - bh) / 2 - ring,
                    cw + 2 * ring,
                    bh + 2 * ring,
                );
                let vx = bx + cw + px(8.0);
                put(
                    ID_GAM_NEVER0 + i,
                    vx - ring,
                    cy + (row - bh) / 2 - ring,
                    vw + 2 * ring,
                    bh + 2 * ring,
                );
                cy += row + px(8.0);
            }
        }
        // Programs: name and path, then «Perfil: X ▾» and «Quitar»
        // (stacked in a narrow window).
        let rpad = px(if g.compact { 12.0 } else { 14.0 });
        let rin = inner - 2 * rpad;
        let rx = ix + rpad;
        let dw = btn_w("Quitar");
        for i in 0..MAX_PROGS {
            if txt(ID_GAM_NAME0 + i).is_empty() {
                for id in prog_ids(i) {
                    put(id, 0, 0, 0, 0);
                }
                continue;
            }
            let label = sel_label(&txt(ID_GAM_SEL0 + i));
            let sw = btn_w(&label);
            let buttons_w = sw + px(8.0) + dw;
            let text_w = if g.compact {
                rin
            } else {
                rin - buttons_w - px(16.0)
            };
            let row_top = cy;
            let mut ry = cy + rpad;
            let (_, nh) = measure(ID_GAM_NAME0 + i, Role::Strong, text_w);
            put(ID_GAM_NAME0 + i, rx, ry, text_w, nh);
            ry += nh + px(2.0);
            let (_, ph) = measure(ID_GAM_PATH0 + i, Role::Secondary, text_w);
            put(ID_GAM_PATH0 + i, rx, ry, text_w, ph);
            ry += ph;
            let bottom = if g.compact {
                ry += px(10.0);
                let w = rin - dw - px(8.0);
                put(
                    ID_GAM_SEL0 + i,
                    rx - ring,
                    ry - ring,
                    w + 2 * ring,
                    bh + 2 * ring,
                );
                let dx = rx + w + px(8.0);
                put(
                    ID_GAM_DEL0 + i,
                    dx - ring,
                    ry - ring,
                    dw + 2 * ring,
                    bh + 2 * ring,
                );
                ry + bh + rpad
            } else {
                let content = (ry - (cy + rpad)).max(bh);
                let by = cy + rpad + (content - bh) / 2;
                let bx = rx + rin - buttons_w;
                put(
                    ID_GAM_SEL0 + i,
                    bx - ring,
                    by - ring,
                    sw + 2 * ring,
                    bh + 2 * ring,
                );
                let dx = bx + sw + px(8.0);
                put(
                    ID_GAM_DEL0 + i,
                    dx - ring,
                    by - ring,
                    dw + 2 * ring,
                    bh + 2 * ring,
                );
                cy + rpad + content + rpad
            };
            rows.push((
                RECT {
                    left: ix,
                    top: row_top,
                    right: ix + inner,
                    bottom,
                },
                false,
            ));
            cy = bottom + px(8.0);
        }
        let (_, h) = measure(ID_GAM_EMPTY, Role::Secondary, inner);
        if h > 0 {
            put(ID_GAM_EMPTY, ix, cy, inner, h);
            cy += h + px(12.0);
        } else {
            put(ID_GAM_EMPTY, 0, 0, 0, 0);
        }
        let aw = btn_w("Añadir programa…");
        put(
            ID_GAM_ADD,
            ix - ring,
            cy - ring,
            aw + 2 * ring,
            bh + 2 * ring,
        );
        if visible(ID_GAM_UNREG) {
            let uw = btn_w("Quitar avisos de Windows");
            if g.compact {
                cy += bh + px(10.0);
                put(
                    ID_GAM_UNREG,
                    ix - ring,
                    cy - ring,
                    uw + 2 * ring,
                    bh + 2 * ring,
                );
            } else {
                let ux = ix + aw + px(12.0);
                put(
                    ID_GAM_UNREG,
                    ux - ring,
                    cy - ring,
                    uw + 2 * ring,
                    bh + 2 * ring,
                );
            }
        } else {
            put(ID_GAM_UNREG, 0, 0, 0, 0);
        }
        cy += bh;
        let (_, h) = measure(ID_GAM_STATUS, Role::Notice, inner);
        if h > 0 {
            cy += px(12.0);
            put(ID_GAM_STATUS, ix, cy, inner, h);
            cy += h;
        } else {
            put(ID_GAM_STATUS, 0, 0, 0, 0);
        }
        cy += pad;
        cards.push(RECT {
            left,
            top,
            right: left + col,
            bottom: cy,
        });
        y = cy + px(20.0);
    } else {
        for id in gam_ids
            .into_iter()
            .chain((0..MAX_NEW).flat_map(new_ids))
            .chain((0..MAX_PROGS).flat_map(prog_ids))
        {
            put(id, 0, 0, 0, 0);
        }
    }

    // Profile: original copy, restores, and forgetting the app's settings.
    let top = y;
    let mut cy = y + pad;
    let (_, h) = measure(ID_PROFILE_TITLE, Role::Name, inner);
    put(ID_PROFILE_TITLE, ix, cy, inner, h);
    cy += h + px(4.0);
    for (id, role) in [
        (ID_BTN_BACKUP, Role::Secondary),
        (ID_WRITE_LOCK, Role::Notice),
    ] {
        let (_, h) = measure(id, role, inner);
        if h > 0 {
            put(id, ix, cy, inner, h);
            cy += h + px(8.0);
        } else {
            put(id, 0, 0, 0, 0);
        }
    }
    if visible(ID_PROFILE_RESTORE) {
        cy += px(4.0);
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
        cy += bh + px(16.0);
    } else {
        put(ID_PROFILE_RESTORE, 0, 0, 0, 0);
        put(ID_FACTORY, 0, 0, 0, 0);
    }
    separators.push((ix, ix + inner, cy - px(2.0)));
    cy += px(14.0);
    let rb_w = btn_w("Restaurar originales");
    let desc_w = if g.compact {
        inner
    } else {
        inner - rb_w - px(24.0)
    };
    let (_, h2) = measure(ID_RST_DESC, Role::Secondary, desc_w);
    put(ID_RST_DESC, ix, cy, desc_w, h2);
    if g.compact {
        cy += h2 + px(10.0);
        put(
            ID_RST_BTN,
            ix - ring,
            cy - ring,
            inner + 2 * ring,
            bh + 2 * ring,
        );
        cy += bh;
    } else {
        let by = cy + (h2 - bh) / 2;
        put(
            ID_RST_BTN,
            left + col - pad - rb_w - ring,
            by.max(cy) - ring,
            rb_w + 2 * ring,
            bh + 2 * ring,
        );
        cy += h2.max(bh);
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

    // Fixed bar: pending changes and the save result, in bar coordinates.
    let mut save_xy = None;
    let mut bar_height = 0;
    let (_, text_h) = measure(ID_BAR_TEXT, Role::Strong, inner);
    let (_, sth) = measure(ID_SAVE_STATUS, Role::Secondary, inner - px(24.0));
    let pending_bar = text_h > 0 && visible(ID_BAR_SAVE);
    let buttons_row = pending_bar || visible(ID_BAR_DISCARD);
    let vpad = px(if g.compact { 12.0 } else { 14.0 });
    if buttons_row || sth > 0 {
        let mut cy = vpad;
        if buttons_row {
            let save_w = btn_w("Guardar en el ratón…").max(btn_w("Reintentar…"));
            let discard_w = btn_w("Descartar");
            let text_w = if g.compact {
                col
            } else {
                col - save_w - discard_w - px(24.0)
            };
            let (_, th) = measure(ID_BAR_TEXT, Role::Strong, text_w);
            if g.compact {
                put(ID_BAR_TEXT, left, cy, col, th);
                cy += if th > 0 { th + px(8.0) } else { 0 };
                put(
                    ID_BAR_DISCARD,
                    left - ring,
                    cy - ring,
                    discard_w + 2 * ring,
                    bh + 2 * ring,
                );
                let sx = left + discard_w + px(12.0);
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
                put(ID_BAR_TEXT, left, cy + (row_h - th) / 2, text_w, th);
                let sx = left + col - save_w;
                put(
                    ID_BAR_SAVE,
                    sx - ring,
                    cy + (row_h - bh) / 2 - ring,
                    save_w + 2 * ring,
                    bh + 2 * ring,
                );
                let dx = if visible(ID_BAR_SAVE) {
                    sx - px(12.0) - discard_w
                } else {
                    left + col - discard_w
                };
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
        if sth > 0 {
            if buttons_row {
                cy += px(10.0);
            }
            put(ID_SAVE_STATUS, left + px(24.0), cy, col - px(24.0), sth);
            save_xy = Some((left, cy));
            cy += sth;
        } else {
            put(ID_SAVE_STATUS, 0, 0, 0, 0);
        }
        bar_height = cy + vpad;
    } else {
        for id in BAR {
            put(id, 0, 0, 0, 0);
        }
    }
    y += px(20.0);

    unsafe {
        ReleaseDC(Some(view), hdc);
    }

    // Apply positions (the page scrolls, the bar does not), fonts.
    for (id, r) in &pos {
        let h = h_of(*id);
        let dy = if BAR.contains(id) { 0 } else { scroll };
        unsafe {
            let _ = SetWindowPos(
                h,
                None,
                r.left,
                r.top - dy,
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
            Kind::Button(..) | Kind::Combo | Kind::Check(_) => fonts.body,
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
            m.hex_field = hex_field;
            m.rows = rows;
            m.active_check = active_check;
            m.profile_icon = match (m.profile_icon, profile_status_xy) {
                (Some((_, _, ok)), Some((x, y))) => Some((x, y, ok)),
                _ => None,
            };
            m.bar_h = bar_height;
            m.preview = preview;
            m.light_icon = match (m.light_icon, light_status_xy) {
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
    // Profile rows: the active one on `surface.selected` with a
    // `text.primary` border, never only by colour (it also says «Activo»).
    let (rows, active_check, profile_icon) = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| (m.rows.clone(), m.active_check, m.profile_icon))
    })
    .flatten()
    .unwrap_or_default();
    for (r, active) in &rows {
        c.origin(r.left as f32, (r.top - scroll) as f32);
        let (w, h) = (u(r.right - r.left), u(r.bottom - r.top));
        if *active {
            c.fill_round_rect(0.0, 0.0, w, h, 8.0, palette.surface_selected);
            c.stroke_round_rect(0.0, 0.0, w, h, 8.0, 1.0, palette.text_primary, false);
        } else {
            c.stroke_round_rect(0.0, 0.0, w, h, 8.0, 1.0, palette.border_card, false);
        }
    }
    if let Some((x, y)) = active_check {
        c.origin(x as f32, (y - scroll) as f32);
        glyph::check(&c, 0.0, 0.0, palette.text_primary);
    }
    if let Some((sx, sy, ok)) = profile_icon {
        c.origin(sx as f32, (sy - scroll) as f32 + scale);
        if ok {
            glyph::check(&c, 0.0, 0.0, palette.text_primary);
        } else {
            glyph::notice_icon(&c, 0.0, 0.0, true, palette.text_primary);
        }
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
    let _ = save_icon;
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
    let (hex_field, hex_invalid, preview, light_icon, pick) = with_app(|a| {
        a.view.mouse.as_ref().map(|m| {
            (
                m.hex_field,
                m.hex_invalid,
                m.preview,
                m.light_icon,
                m.pick.clone(),
            )
        })
    })
    .flatten()
    .unwrap_or_default();
    if let Some(f) = hex_field {
        c.origin(f.left as f32, (f.top - scroll) as f32);
        let (w, h) = (u(f.right - f.left), u(f.bottom - f.top));
        c.fill_round_rect(0.0, 0.0, w, h, 6.0, palette.bg_app);
        if hex_invalid {
            c.stroke_round_rect(0.0, 0.0, w, h, 6.0, 2.0, palette.text_primary, false);
        } else {
            c.stroke_round_rect(0.0, 0.0, w, h, 6.0, 1.0, palette.border_control, false);
        }
    }
    if let Some(r) = preview {
        // The light's colour is data, not a token; without a valid colour
        // the box is empty with a dashed outline.
        c.origin(r.left as f32, (r.top - scroll) as f32);
        let (w, h) = (u(r.right - r.left), u(r.bottom - r.top));
        match pick.as_ref().and_then(Pick::color) {
            Some(rgb) => {
                c.fill_round_rect(0.0, 0.0, w, h, 6.0, rgb_color(rgb));
                c.stroke_round_rect(0.0, 0.0, w, h, 6.0, 1.0, palette.border_control, false);
            }
            None => c.stroke_round_rect(0.0, 0.0, w, h, 6.0, 1.0, palette.border_control, true),
        }
    }
    if let Some((sx, sy, ok)) = light_icon {
        c.origin(sx as f32, (sy - scroll) as f32 + scale);
        if ok {
            glyph::check(&c, 0.0, 0.0, palette.text_primary);
        } else {
            glyph::notice_icon(&c, 0.0, 0.0, true, palette.text_primary);
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

fn rgb_color(c: crate::protocol::razer::Rgb) -> Color {
    Color((c.r as u32) << 16 | (c.g as u32) << 8 | c.b as u32)
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
    if let Some(i) = in_group(id, ID_SWATCH0, N_SWATCHES) {
        panel::draw_swatch(
            dis,
            rgb_color(light::swatch(i)),
            bg,
            &palette,
            devices_view::scale_of(view),
            style == ButtonStyle::Selected,
        );
        return;
    }
    let label = match id {
        c if (ID_BTN_CHANGE0..ID_BTN_CHANGE0 + MAX_BUTTONS).contains(&c) => "Cambiar".into(),
        e if in_group(e, ID_EFFECT0, N_EFFECTS).is_some() => {
            light::EFFECTS[(e - ID_EFFECT0) as usize].1.to_string()
        }
        v if in_group(v, ID_SPEED0, N_SPEEDS).is_some() => {
            light::SPEEDS[(v - ID_SPEED0) as usize].to_string()
        }
        ID_LIGHT_TRY => get_text(dis.hwndItem),
        ID_ED_KEY_CLEAR => "Borrar".into(),
        u if in_group(u, ID_PRF_USE0, MAX_ROWS).is_some() => "Usar".into(),
        d if in_group(d, ID_PRF_DEL0, MAX_ROWS).is_some() => "Eliminar…".into(),
        r if in_group(r, ID_PRF_REN0, MAX_ROWS).is_some() => "Renombrar…".into(),
        ID_PRF_NEW => "Nuevo perfil…".into(),
        ID_PRF_PREV => "‹ Anterior".into(),
        ID_PRF_NEXT => "Siguiente ›".into(),
        c if in_group(c, ID_GAM_CFG0, MAX_NEW).is_some() => "Configurar…".into(),
        v if in_group(v, ID_GAM_NEVER0, MAX_NEW).is_some() => "No preguntar más".into(),
        q if in_group(q, ID_GAM_SEL0, MAX_PROGS).is_some() => sel_label(&get_text(dis.hwndItem)),
        d if in_group(d, ID_GAM_DEL0, MAX_PROGS).is_some() => "Quitar".into(),
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
        Kind::Check(surface) => {
            let fg = if unsafe { IsWindowEnabled(child).as_bool() } {
                palette.text_primary
            } else {
                palette.text_disabled
            };
            match surface {
                Surface::App => (fg, palette.bg_app, app),
                Surface::Card => (fg, palette.surface_card, card),
                Surface::Selected => (fg, palette.surface_selected, selected),
            }
        }
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
        Some(Profile::Read(v)) => v.buttons.clone(),
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
            m.pending = Some((shared.writes, Pending::Buttons(what)));
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

/// Lines of the save dialog: "DPI al arrancar: 1000 → 1600", "Luz: …",
/// "Lateral trasero: Atrás → Ctrl + C".
pub fn change_lines(
    edits: &ProfileEdits,
    base_dpi: Option<u16>,
    base_light: Option<Light>,
    buttons: &[Action],
) -> Vec<String> {
    let mut lines = Vec::new();
    if let Some(d) = edits.dpi {
        lines.push(format!(
            "DPI al arrancar: {} → {d} (en los 4 niveles del botón DPI)",
            base_dpi
                .map(|b| b.to_string())
                .unwrap_or_else(|| "—".into())
        ));
    }
    if let Some(l) = edits.light {
        lines.push(format!(
            "Luz: {} → {}",
            base_light
                .map(light::describe)
                .unwrap_or_else(|| "—".into()),
            light::describe(l)
        ));
    }
    for &(r, a) in &edits.buttons {
        let before = buttons.get(r).map(|&b| text::action(b)).unwrap_or_default();
        lines.push(format!(
            "{}: {before} → {}",
            text::mouse_button(r),
            text::action(a)
        ));
    }
    lines
}

/// The page's pending changes and what they replace: (edits, saved DPI,
/// saved light, the mouse's buttons).
type Current = (ProfileEdits, Option<u16>, Option<Light>, Vec<Action>);

fn current_edits() -> Option<Current> {
    let shared = super::snapshot()?;
    let typed = ctl(ID_DPI_EDIT).map(get_text).unwrap_or_default();
    with_app(|a| {
        let m = a.view.mouse.as_ref()?;
        let snap = snapshot(&shared, &m.key)?;
        Some((
            pending_edits(m, Some(snap), parse_field(&typed)),
            snap_base_dpi(snap),
            snap_base_light(snap),
            profile_view(snap)
                .map(|v| v.buttons.clone())
                .unwrap_or_default(),
        ))
    })
    .flatten()
}

fn confirm_save(view: HWND) {
    let Some((edits, base_dpi, base_light, buttons)) = current_edits() else {
        return;
    };
    let n = count(&edits);
    if n == 0 {
        return;
    }
    let lines = change_lines(&edits, base_dpi, base_light, &buttons);
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
                Command::SaveProfile(key, edits, Confirmed::by_dialog())
            })
        }),
    );
}

/// Drops the page's pending changes. Whatever "Probar" sent goes back to
/// what is saved.
fn discard(view: HWND) {
    let Some(shared) = super::snapshot() else {
        return;
    };
    let undo = with_app(|a| {
        let m = a.view.mouse.as_mut()?;
        m.edits.clear();
        m.buttons_result = None;
        m.invalid = false;
        m.applied = None;
        m.light_result = None;
        let snap = snapshot(&shared, &m.key)?;
        let mut undo = Vec::new();
        if m.tried_dpi.take().is_some() {
            undo.extend(snap_base_dpi(snap).map(Change::Dpi));
        }
        if m.tried_light.take().is_some() {
            undo.extend(snap_base_light(snap).map(Change::Light));
        }
        m.pick = None;
        Some((m.key.clone(), undo))
    })
    .flatten();
    if let Some((key, undo)) = undo {
        for c in undo {
            send(Command::Try(key.clone(), c));
        }
    }
    fill_field_from_device(view);
    refresh(view);
    // Never onto a disabled control (e.g. the mouse is asleep).
    let next = [ID_APPLY, ID_LIGHT_TRY, ID_BACK]
        .into_iter()
        .filter_map(ctl)
        .find(|&h| unsafe { IsWindowVisible(h).as_bool() && IsWindowEnabled(h).as_bool() });
    if let Some(h) = next {
        unsafe {
            let _ = SetFocus(Some(h));
        }
    }
}

// ---------------------------------------------------------------------------
// Profiles: «Usar», new, rename, delete.

/// The mouse's key, profiles and active profile.
fn profile_list() -> Option<(
    DeviceKey,
    Vec<crate::profiles::MouseProfile>,
    Option<String>,
)> {
    let shared = super::snapshot()?;
    let key = with_app(|a| a.view.mouse.as_ref().map(|m| m.key.clone())).flatten()?;
    let s = snapshot(&shared, &key)?;
    Some((key, s.profiles.clone(), s.active_profile.clone()))
}

/// A change to the profile list; its result is shown in the section.
fn send_profiles(view: HWND, command: Command) {
    let Some(shared) = super::snapshot() else {
        return;
    };
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.pending = Some((shared.seq, Pending::Profiles));
            m.profile_status = None;
        }
    });
    send(command);
    refresh(view);
}

/// «Usar»: writes the profile into the mouse (no dialog, see
/// `profiles::ProfileApply`).
fn use_profile(view: HWND, name: String) {
    let from = profile_list().and_then(|(.., a)| a).unwrap_or_default();
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.using = Some((name.clone(), from));
            m.profile_status = None;
            m.page = None;
        }
    });
    send_write(view, Write::Use, |key| {
        crate::profiles::use_command(key, name)
    });
}

/// «Usar» with changes not saved yet asks first (DESIGN.md: Cancelar ·
/// Descartar y cambiar · Guardar y cambiar).
fn request_use(view: HWND, row: usize) {
    let busy = with_app(|a| a.view.mouse.as_ref().is_some_and(|m| m.pending.is_some()));
    if busy != Some(false) {
        return;
    }
    let Some((_, list, active)) = profile_list() else {
        return;
    };
    let Some(name) = list.get(row).map(|p| p.name.clone()) else {
        return;
    };
    let Some((edits, base_dpi, base_light, buttons)) = current_edits() else {
        return;
    };
    let n = count(&edits);
    if n == 0 {
        use_profile(view, name);
        return;
    }
    let lines = change_lines(&edits, base_dpi, base_light, &buttons);
    let title = format!(
        "«{}» tiene {} sin guardar",
        active.unwrap_or_default(),
        plural(n, "cambio", "cambios")
    );
    let body = format!(
        "{}\n\nAntes de cambiar a «{name}», elige qué hacer con {}. Si {} descartas, se pierden.",
        lines.join("\n"),
        if n == 1 { "él" } else { "ellos" },
        if n == 1 { "lo" } else { "los" },
    );
    super::confirm::open_choices(
        view,
        &title,
        &body,
        &["Descartar y cambiar", "Guardar y cambiar"],
        None,
        Box::new(move |choice, _| {
            if choice == 0 {
                discard(view);
                use_profile(view, name);
            } else {
                with_app(|a| a.view.mouse.as_mut().map(|m| m.then_use = Some(name)));
                send_write(view, Write::Save, |key| {
                    Command::SaveProfile(key, edits, Confirmed::by_dialog())
                });
            }
        }),
    );
}

fn name_field(
    text: String,
    list: Vec<crate::profiles::MouseProfile>,
    renaming: Option<String>,
) -> super::confirm::Field {
    super::confirm::Field {
        label: "Nombre".into(),
        text,
        max: crate::profiles::MAX_NAME,
        check: Box::new(move |t| crate::profiles::name_error(t, &list, renaming.as_deref())),
    }
}

/// «Nuevo perfil…» (a copy of the active one) or «Guardar como perfil
/// nuevo…» (what the mouse holds now).
fn new_profile(view: HWND, from_mouse: bool) {
    let Some((key, list, Some(active))) = profile_list() else {
        return;
    };
    let initial = crate::profiles::copy_name(&active, &list);
    // After creating it, the list shows the page of the new profile.
    let new_page = texts::page_of(list.len());
    with_app(|a| a.view.mouse.as_mut().map(|m| m.page = Some(new_page)));
    let (title, body, command): (&str, String, fn(DeviceKey, String) -> Command) = if from_mouse {
        (
            "Guardar como perfil nuevo",
            "Se crea un perfil con lo que tiene ahora el ratón y pasa a ser el activo. No se escribe en el ratón.".into(),
            Command::SaveAsProfile,
        )
    } else {
        let summary = list
            .iter()
            .find(|p| p.name == active)
            .map(texts::summary)
            .unwrap_or_default();
        (
            "Nuevo perfil",
            format!(
                "Empieza como una copia de «{active}»: {summary}. No se escribe en el ratón hasta que pulses «Usar»."
            ),
            Command::NewProfile,
        )
    };
    super::confirm::open_choices(
        view,
        title,
        &body,
        &["Crear perfil"],
        Some(name_field(initial, list, None)),
        Box::new(move |_, text| send_profiles(view, command(key, text.trim().to_string()))),
    );
}

fn rename_profile(view: HWND, row: usize) {
    let Some((key, list, _)) = profile_list() else {
        return;
    };
    let Some(old) = list.get(row).map(|p| p.name.clone()) else {
        return;
    };
    super::confirm::open_choices(
        view,
        &format!("Renombrar «{old}»"),
        "",
        &["Renombrar"],
        Some(name_field(old.clone(), list, Some(old.clone()))),
        Box::new(move |_, text| {
            send_profiles(
                view,
                Command::RenameProfile(key, old, text.trim().to_string()),
            )
        }),
    );
}

fn confirm_delete(view: HWND, row: usize) {
    let Some((key, list, _)) = profile_list() else {
        return;
    };
    let Some(name) = list.get(row).map(|p| p.name.clone()) else {
        return;
    };
    super::confirm::open(
        view,
        &format!("¿Eliminar el perfil «{name}»?"),
        &format!(
            "Se borra de este PC. El ratón no cambia, porque «{name}» no es el perfil activo."
        ),
        "Eliminar perfil",
        Box::new(move || send_profiles(view, Command::DeleteProfile(key, name))),
    );
}

// ---------------------------------------------------------------------------
// «Juegos y programas».

/// "Perfil de PEAK.exe: Juego" (accessible name) -> "Perfil: Juego ▾".
fn sel_label(name: &str) -> String {
    let profile = name.rsplit_once(": ").map(|(_, p)| p).unwrap_or("");
    format!("Perfil: {profile} ▾")
}

fn unanswered(i: usize) -> Option<String> {
    let shared = super::snapshot()?;
    let key = with_app(|a| a.view.mouse.as_ref().map(|m| m.key.clone())).flatten()?;
    snapshot(&shared, &key)?.unanswered.get(i).cloned()
}

fn program(i: usize) -> Option<String> {
    let shared = super::snapshot()?;
    let key = with_app(|a| a.view.mouse.as_ref().map(|m| m.key.clone())).flatten()?;
    Some(snapshot(&shared, &key)?.programs.get(i)?.exe.clone())
}

/// A change to the programs list; its result is shown in the section.
fn send_games(view: HWND, command: Command) {
    let Some(shared) = super::snapshot() else {
        return;
    };
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.pending = Some((shared.seq, Pending::Games));
            m.games_status = None;
        }
    });
    send(command);
    refresh(view);
}

/// «Configurar…» of a new game and «Añadir programa…»: creates a profile
/// named after the program (editable), a copy of the active one, and
/// associates it (decision of the user, 2026-10-07: a new game gets its own
/// profile, not an existing one). «Cancelar» is «Ahora no».
pub(crate) fn create_profile_for(view: HWND, exe: String) {
    let Some((key, list, active)) = profile_list() else {
        return;
    };
    let file = crate::games::display_name(&exe);
    let stem = file
        .rsplit_once('.')
        .map(|(s, _)| s.to_string())
        .unwrap_or(file.clone());
    if list.len() >= crate::profiles::MAX_PROFILES {
        let text = format!(
            "Error: ya hay {} perfiles. Elimina uno para crear el de {stem}.",
            crate::profiles::MAX_PROFILES
        );
        with_app(|a| {
            a.view
                .mouse
                .as_mut()
                .map(|m| m.games_status = Some((false, text)))
        });
        refresh(view);
        return;
    }
    let mut initial: String = stem.chars().take(crate::profiles::MAX_NAME).collect();
    initial = initial.trim().to_string();
    if crate::profiles::name_error(&initial, &list, None).is_some() {
        initial = crate::profiles::copy_name(&initial, &list);
    }
    let base = active.unwrap_or_default();
    super::confirm::open_choices(
        view,
        &format!("Perfil para {file}"),
        &format!(
            "Se creará un perfil nuevo como copia de «{base}» y se usará al abrir {file}, mientras OpenPeripheral esté abierto. Después ajusta su DPI, su luz y sus botones en esta página.

{exe}"
        ),
        &["Crear perfil"],
        Some(name_field(initial, list, None)),
        Box::new(move |_, text| {
            send_games(
                view,
                Command::CreateGameProfile(key, exe, text.trim().to_string()),
            )
        }),
    );
}

/// «Perfil: X ▾» of a program: one button per profile and «No cambiar de
/// perfil», to change which profile it uses.
pub(crate) fn choose_profile_for(view: HWND, exe: String) {
    let Some((key, list, _)) = profile_list() else {
        return;
    };
    let names: Vec<String> = list.iter().map(|p| p.name.clone()).collect();
    let labels = names.clone();
    let mut choices: Vec<&str> = labels.iter().map(String::as_str).collect();
    choices.push("No cambiar de perfil");
    let name = crate::games::display_name(&exe);
    super::confirm::open_choices(
        view,
        &format!("Perfil para {name}"),
        &format!(
            "{exe}\n\nSe usará al abrirlo, mientras OpenPeripheral esté abierto. Si cambias de carpeta el programa, habrá que volver a asociarlo. «Cancelar» es «Ahora no»."
        ),
        &choices,
        None,
        Box::new(move |choice, _| {
            let profile = names.get(choice).cloned();
            send_games(view, Command::SetProgram(key, exe, profile));
        }),
    );
}

/// «Añadir programa…»: the system's open-file dialog, programs only. The
/// path is only stored and compared; the program is never run.
fn pick_exe(view: HWND) -> Option<String> {
    use windows::Win32::UI::Controls::Dialogs::{
        GetOpenFileNameW, OFN_DONTADDTORECENT, OFN_EXPLORER, OFN_FILEMUSTEXIST, OFN_NOCHANGEDIR,
        OFN_PATHMUSTEXIST, OPENFILENAMEW,
    };
    let mut buf = vec![0u16; 1024];
    let filter: Vec<u16> = "Programas (*.exe)\0*.exe\0\0".encode_utf16().collect();
    let title = wide("Añadir programa");
    let mut ofn = OPENFILENAMEW {
        lStructSize: std::mem::size_of::<OPENFILENAMEW>() as u32,
        hwndOwner: view,
        lpstrFilter: PCWSTR(filter.as_ptr()),
        lpstrFile: windows::core::PWSTR(buf.as_mut_ptr()),
        nMaxFile: buf.len() as u32,
        lpstrTitle: PCWSTR(title.as_ptr()),
        Flags: OFN_FILEMUSTEXIST
            | OFN_PATHMUSTEXIST
            | OFN_NOCHANGEDIR
            | OFN_DONTADDTORECENT
            | OFN_EXPLORER,
        ..Default::default()
    };
    if !unsafe { GetOpenFileNameW(&mut ofn) }.as_bool() {
        return None;
    }
    let len = buf.iter().position(|&c| c == 0).unwrap_or(buf.len());
    Some(String::from_utf16_lossy(&buf[..len]))
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
    // Stay until a write ends, so its result (or error) is always shown.
    let writing = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .is_some_and(|m| matches!(m.pending, Some((_, Pending::Buttons(_)))))
    });
    if writing == Some(true) {
        return;
    }
    let n = current_edits().map_or(0, |(e, ..)| count(&e));
    if n == 0 {
        discard_and_leave(view);
        return;
    }
    super::confirm::open(
        view,
        &format!("¿Descartar {} sin guardar?", plural(n, "cambio", "cambios")),
        "La memoria del ratón no ha cambiado: estos cambios solo estaban en esta pantalla. Lo que hayas probado vuelve a lo guardado.",
        "Descartar cambios",
        Box::new(move || discard_and_leave(view)),
    );
}

/// Leaving undoes what "Probar" sent, like "Descartar".
fn discard_and_leave(view: HWND) {
    let tried = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .is_some_and(|m| m.tried_dpi.is_some() || m.tried_light.is_some())
    });
    if tried == Some(true) {
        discard(view);
    }
    leave(view);
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
                    m.tried_dpi = Some(v);
                    m.pending = Some((shared.seq, Pending::Apply(v)));
                    m.key.clone()
                })
            })
            .flatten();
            if let Some(key) = key {
                send(Command::Try(key, Change::Dpi(v)));
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

/// "Probar la luz": sends the picked light without saving it.
fn try_light(view: HWND) {
    let Some(shared) = super::snapshot() else {
        return;
    };
    let key = with_app(|a| {
        let m = a.view.mouse.as_mut()?;
        let l = m.pick.as_ref().and_then(Pick::light)?;
        m.tried_light = Some(l);
        m.light_result = None;
        m.pending = Some((shared.seq, Pending::TryLight));
        Some((m.key.clone(), l))
    })
    .flatten();
    match key {
        Some((key, l)) => send(Command::Try(key, Change::Light(l))),
        // Invalid colour: back to the field, whose error is shown.
        None => {
            if let Some(h) = ctl(ID_HEX_EDIT) {
                unsafe {
                    let _ = SetFocus(Some(h));
                    SendMessageW(h, EM_SETSEL, Some(WPARAM(0)), Some(LPARAM(-1)));
                }
            }
        }
    }
    refresh(view);
}

/// Changes the light pick and redraws.
fn update_pick(view: HWND, f: impl FnOnce(&mut Pick)) {
    let changed = with_app(|a| {
        let p = a.view.mouse.as_mut()?.pick.as_mut()?;
        let before = p.clone();
        f(p);
        Some(*p != before)
    })
    .flatten();
    if changed == Some(true) {
        with_app(|a| a.view.mouse.as_mut().map(|m| m.light_result = None));
        refresh(view);
    }
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
        // Enter: "Probar" of the DPI or the light when its field has the focus.
        (1, _) => {
            let f = Some(unsafe { GetFocus() });
            if f == ctl(ID_DPI_EDIT) {
                apply(view);
            } else if f == ctl(ID_HEX_EDIT) {
                try_light(view);
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
        (ID_BAR_DISCARD, BN_CLICKED) => discard(view),
        // Single-choice groups: focus moved by the arrows also chooses.
        (e, BN_CLICKED | BN_SETFOCUS) if in_group(e, ID_EFFECT0, N_EFFECTS).is_some() => {
            let i = (e - ID_EFFECT0) as usize;
            update_pick(view, |p| p.effect = light::EFFECTS[i].0);
            devices_view::ensure_visible(view, HWND(lparam.0 as *mut _));
        }
        (v, BN_CLICKED | BN_SETFOCUS) if in_group(v, ID_SPEED0, N_SPEEDS).is_some() => {
            let i = (v - ID_SPEED0) as usize;
            update_pick(view, |p| p.speed = i);
            devices_view::ensure_visible(view, HWND(lparam.0 as *mut _));
        }
        (c, BN_CLICKED | BN_SETFOCUS) if in_group(c, ID_SWATCH0, N_SWATCHES).is_some() => {
            let hex = crate::config::to_hex(light::swatch((c - ID_SWATCH0) as usize));
            let same = with_app(|a| {
                a.view
                    .mouse
                    .as_ref()
                    .and_then(|m| m.pick.as_ref())
                    .is_some_and(|p| p.color() == crate::config::parse_hex(&hex))
            });
            // Tab arriving on the group must not replace a custom colour.
            if code == BN_CLICKED || same != Some(true) {
                set(ID_HEX_EDIT, &hex);
            }
            devices_view::ensure_visible(view, HWND(lparam.0 as *mut _));
        }
        (ID_HEX_EDIT, EN_CHANGE) => {
            let typed = ctl(ID_HEX_EDIT).map(get_text).unwrap_or_default();
            update_pick(view, |p| p.hex = typed);
        }
        (ID_LIGHT_TRY, BN_CLICKED) => try_light(view),
        (ID_BAR_SAVE, BN_CLICKED) => confirm_save(view),
        (u, BN_CLICKED) if in_group(u, ID_PRF_USE0, MAX_ROWS).is_some() => {
            request_use(view, (u - ID_PRF_USE0) as usize)
        }
        (d, BN_CLICKED) if in_group(d, ID_PRF_DEL0, MAX_ROWS).is_some() => {
            confirm_delete(view, (d - ID_PRF_DEL0) as usize)
        }
        (r, BN_CLICKED) if in_group(r, ID_PRF_REN0, MAX_ROWS).is_some() => {
            rename_profile(view, (r - ID_PRF_REN0) as usize)
        }
        (ID_PRF_NEW, BN_CLICKED) => new_profile(view, false),
        (ID_PRF_PREV | ID_PRF_NEXT, BN_CLICKED) => {
            with_app(|a| {
                a.view.mouse.as_mut().map(|m| {
                    m.page = Some(if id == ID_PRF_PREV {
                        m.page_shown.saturating_sub(1)
                    } else {
                        m.page_shown + 1
                    })
                })
            });
            refresh(view);
        }
        (ID_GAM_AUTO | ID_GAM_ASK, BN_CLICKED) => {
            let checked = |id: i32| {
                ctl(id).is_some_and(|h| unsafe { SendMessageW(h, BM_GETCHECK, None, None).0 == 1 })
            };
            if let Some((key, ..)) = profile_list() {
                send_games(
                    view,
                    Command::SetGameOptions(key, checked(ID_GAM_AUTO), checked(ID_GAM_ASK)),
                );
            }
        }
        (c, BN_CLICKED) if in_group(c, ID_GAM_CFG0, MAX_NEW).is_some() => {
            if let Some(exe) = unanswered((c - ID_GAM_CFG0) as usize) {
                create_profile_for(view, exe);
            }
        }
        (v, BN_CLICKED) if in_group(v, ID_GAM_NEVER0, MAX_NEW).is_some() => {
            if let (Some((key, ..)), Some(exe)) =
                (profile_list(), unanswered((v - ID_GAM_NEVER0) as usize))
            {
                send_games(view, Command::SetProgram(key, exe, None));
            }
        }
        (q, BN_CLICKED) if in_group(q, ID_GAM_SEL0, MAX_PROGS).is_some() => {
            if let Some(exe) = program((q - ID_GAM_SEL0) as usize) {
                choose_profile_for(view, exe);
            }
        }
        (d, BN_CLICKED) if in_group(d, ID_GAM_DEL0, MAX_PROGS).is_some() => {
            if let (Some((key, ..)), Some(exe)) =
                (profile_list(), program((d - ID_GAM_DEL0) as usize))
            {
                send_games(view, Command::RemoveProgram(key, exe));
            }
        }
        (ID_GAM_ADD, BN_CLICKED) => {
            if let Some(exe) = pick_exe(view) {
                create_profile_for(view, exe);
            }
        }
        (ID_GAM_UNREG, BN_CLICKED) => {
            let r = crate::platform::toast::unregister();
            let status = match r {
                Ok(()) => (true, "Avisos de Windows quitados: se borró el registro de OpenPeripheral para las notificaciones.".to_string()),
                Err(e) => (false, format!("Error: no se pudieron quitar los avisos ({e}).")),
            };
            with_app(|a| a.view.mouse.as_mut().map(|m| m.games_status = Some(status)));
            refresh(view);
        }
        (ID_PRF_SAVEAS, BN_CLICKED) => new_profile(view, true),
        (ID_PRF_REAPPLY, BN_CLICKED) => {
            if let Some((_, _, Some(active))) = profile_list() {
                use_profile(view, active);
            }
        }
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
    fn saved_dpi_never_follows_the_tried_one() {
        let mixed = Some([400, 800, 1600, 3200]);
        let same = Some([1200; 4]);
        // App setting first, then a uniform cycle, then the DPI at detection.
        assert_eq!(base_dpi(Some(900), same, Some(1600), Some(2000)), Some(900));
        assert_eq!(base_dpi(None, same, Some(1600), Some(2000)), Some(1200));
        assert_eq!(base_dpi(None, mixed, Some(1600), Some(2000)), Some(1600));
        assert_eq!(base_dpi(None, None, None, Some(2000)), Some(2000));
        assert_eq!(
            saved_dpi_text(None, mixed, Some(1600)),
            "Guardado: ciclo 400 · 800 · 1600 · 3200"
        );
        assert_eq!(saved_dpi_text(None, same, Some(1200)), "Guardado: 1200 DPI");
        assert_eq!(
            saved_dpi_text(Some(900), mixed, Some(900)),
            "Guardado: 900 DPI"
        );
    }

    #[test]
    fn save_dialog_lists_every_change() {
        let edits = ProfileEdits {
            dpi: Some(1600),
            light: Some(Light::Breathing {
                color: crate::protocol::razer::Rgb {
                    r: 0xff,
                    g: 0x80,
                    b: 0,
                },
                period_ms: 6000,
            }),
            buttons: vec![(
                3,
                Action::Keyboard {
                    modifiers: 1,
                    usage: 0x06,
                },
            )],
        };
        let lines = change_lines(
            &edits,
            Some(1000),
            Some(Light::Fixed(light::swatch(5))),
            &[
                Action::Mouse(1),
                Action::Mouse(2),
                Action::Mouse(4),
                Action::Mouse(8),
            ],
        );
        assert_eq!(
            lines,
            vec![
                "DPI al arrancar: 1000 → 1600 (en los 4 niveles del botón DPI)".to_string(),
                "Luz: fija, Cian #00C8FF → respiración, Naranja #FF8000, lenta".to_string(),
                "Lateral trasero: Atrás → Atajo Ctrl + C".to_string(),
            ]
        );
        assert_eq!(count(&edits), 3);
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
            "DPI con el que arranca el ratón. Admite de 200 a 12000, en pasos de 50. Al guardarlo se escribe en los 4 niveles del botón DPI."
        );
    }
}
