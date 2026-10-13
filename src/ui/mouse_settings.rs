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
use std::rc::Rc;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::SystemServices::{SS_LEFT, SS_NOPREFIX};
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, EM_SETLIMITTEXT, EM_SETSEL, ODT_LISTBOX};
use windows::Win32::UI::Input::KeyboardAndMouse::{
    EnableWindow, GetFocus, GetKeyState, IsWindowEnabled, SetFocus, VIRTUAL_KEY, VK_CONTROL,
    VK_ESCAPE, VK_LBUTTON, VK_LWIN, VK_MENU, VK_RWIN, VK_SHIFT, VK_TAB,
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
const ID_DPI_BADGE: i32 = 319;
const ID_PRESET0: i32 = 320; // ..=323
const ID_DPI_STATUS: i32 = 324;
/// «ACTUAL»: the DPI the sensor uses now (it may be one being tried).
const ID_DPI_NOW_LABEL: i32 = 325;
const ID_DPI_NOW: i32 = 326;
// «Botones y DPI» (revisión 2026-10-08): reading the profile, a point per
// button on the drawing with its label (name and action), the list of
// buttons of a narrow window, and the hint under the drawing.
const ID_BTN_STATUS: i32 = 362;
const ID_BTN_HINT: i32 = 363;
const ID_BTN_ACTIVATE: i32 = 364;
/// Buttons shown: as many as an onboard profile can hold (16). Those
/// without a point on the drawing go to the list under it.
const MAX_BUTTONS: i32 = crate::protocol::onboard::MAX_BUTTONS as i32;
const ID_DOT0: i32 = 1100; // ..=1115
const ID_LBL_NAME0: i32 = 1120; // ..=1135
const ID_LBL_ACT0: i32 = 1140; // ..=1155
const ID_ROW0: i32 = 1160; // ..=1175
// «Asignaciones»: what the chosen button does, in three tabs.
const ID_AS_TITLE: i32 = 1180;
const ID_AS_FOR: i32 = 1181;
const ID_AS_TAB0: i32 = 1182; // ..=1184
const ID_AS_LOCK: i32 = 1185;
const ID_AS_SEARCH_LABEL: i32 = 1186;
const ID_AS_SEARCH: i32 = 1187;
const ID_AS_COUNT: i32 = 1188;
const ID_AS_LIST: i32 = 1189;
const ID_AS_KEY_LABEL: i32 = 1191;
const ID_AS_KEY: i32 = 1192;
const ID_AS_KEY_HELP: i32 = 1193;
const ID_AS_KEY_ASSIGN: i32 = 1194;
const ID_AS_KEY_CLEAR: i32 = 1195;
const ID_AS_MOUSE0: i32 = 1196; // ..=1200
const ID_BTN_LAST: i32 = 1200;

/// Tabs of «Asignaciones».
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
enum AsTab {
    #[default]
    Commands,
    Keys,
    Mouse,
}

const AS_TABS: [(AsTab, &str); 3] = [
    (AsTab::Commands, "Comandos"),
    (AsTab::Keys, "Teclas"),
    (AsTab::Mouse, "Ratón"),
];

/// «Ratón»: the clicks this app can write.
const MOUSE_ACTIONS: [u16; 5] = [0x01, 0x02, 0x04, 0x08, 0x10];

/// What the page shows of a button: its name, its action (pending or in
/// the mouse), the action it replaces and whether it cannot be changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
struct ButtonRow {
    name: String,
    action: String,
    before: Option<String>,
    locked: bool,
}
const ID_BAR_TEXT: i32 = 420;
const ID_BAR_DISCARD: i32 = 421;
const ID_BAR_SAVE: i32 = 422;
const ID_SAVE_STATUS: i32 = 423;
const ID_WRITE_LOCK: i32 = 424;
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
/// Result of «Restaurar originales» (in the «Perfil …» menu).
const ID_RST_STATUS: i32 = 343;
// Profiles (revisión 2026-10-08): «PERFILES», a pill per profile, «+ Nuevo»,
// «Perfil …» and the notices below them, in tab order.
const ID_PRF_LABEL: i32 = 461;
const ID_PRF_MENU: i32 = 462;
const ID_PRF_HINT: i32 = 463;
const ID_PRF_MISMATCH: i32 = 472;
const ID_PRF_REAPPLY: i32 = 473;
const ID_PRF_SAVEAS: i32 = 474;
const ID_PRF_FIRST: i32 = 476;
const ID_PRF_NEW: i32 = 477;
const ID_PRF_STATUS: i32 = 479;
const ID_PILL0: i32 = 480; // ..=487
const MAX_ROWS: i32 = crate::profiles::MAX_PROFILES as i32;
const ID_PRF_GAME: i32 = 528;
/// Tabs of the page.
const ID_TAB0: i32 = 490; // ..=492
// «Avanzado»: the programs linked to the active profile.
const ID_GAM_TITLE: i32 = 600;
const ID_GAM_DESC: i32 = 601;
const ID_GAM_FOOT: i32 = 604;
const ID_GAM_EMPTY: i32 = 606;
const ID_GAM_ADD: i32 = 607;
const ID_GAM_STATUS: i32 = 609;
// «Avanzado» › «Rendimiento del sensor»: the report rate (revisión
// 2026-10-13 (2)). Inside the «Avanzado» id range.
const ID_RATE_TITLE: i32 = 610;
const ID_RATE_NOW: i32 = 611;
const ID_RATE_HELP: i32 = 612;
const ID_RATE0: i32 = 620; // ..=627, one per interval of 1 to 8 ms
const MAX_RATES: i32 = 8;
const ID_GAM_NAME0: i32 = 700; // ..=763 programs
const ID_GAM_PATH0: i32 = 800;
const ID_GAM_DEL0: i32 = 1000;
const MAX_PROGS: i32 = crate::games::MAX_PROGRAMS as i32;
const ID_GAM_LAST: i32 = ID_GAM_DEL0 + MAX_PROGS - 1;

/// Tabs of the page (DESIGN.md, revisión 2026-10-08).
#[derive(Clone, Copy, PartialEq, Eq, Default, Debug)]
pub(crate) enum Tab {
    #[default]
    Buttons,
    Light,
    Advanced,
}

const TABS: [(Tab, &str); 3] = [
    (Tab::Buttons, "Botones y DPI"),
    (Tab::Light, "Iluminación"),
    (Tab::Advanced, "Avanzado"),
];

/// The tab a control belongs to; `None`: it shows in every tab.
fn tab_of(id: i32) -> Option<Tab> {
    match id {
        ID_DPI_TITLE..=ID_DPI_NOW | ID_BTN_STATUS..=ID_BTN_ACTIVATE | ID_DOT0..=ID_BTN_LAST => {
            Some(Tab::Buttons)
        }
        ID_LIGHT_TITLE..=ID_LIGHT_STATUS => Some(Tab::Light),
        ID_GAM_TITLE..=ID_GAM_LAST => Some(Tab::Advanced),
        _ => None,
    }
}

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
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text(Role, Surface),
    Button(ButtonStyle, Surface),
    Edit,
    /// The «Comandos» list box (owner-drawn rows).
    List,
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
    /// «Activar la memoria del ratón» (host mode).
    Activate,
    Original,
    Factory,
    /// "Usar" a profile.
    Use,
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
    /// Report interval (ms) chosen in «Avanzado», not saved yet.
    rate: Option<u8>,
    pending: Option<(u64, Pending)>,
    /// Last completed apply: (ok, value).
    applied: Option<(bool, u16)>,
    restore_text: Option<(bool, String)>,
    brushes: Option<(Palette, HBRUSH, HBRUSH, HBRUSH)>,
    /// Separators: (x1, x2, y) in content px.
    separators: Vec<(i32, i32, i32)>,
    profile_requested: bool,
    /// Button changes not yet written: (index, new action), by index.
    edits: Vec<(usize, Action)>,
    /// Outcome of the last write: (ok, text).
    buttons_result: Option<(bool, String)>,
    /// Painted outlines: the "Cambiado" labels.
    badges: Vec<RECT>,
    /// Fields painted behind their EDIT (search and shortcut): `bg.app`
    /// with a `border.control` outline.
    boxes: Vec<RECT>,
    save_icon: Option<(i32, i32, bool)>,
    /// «Botones y DPI»: the chosen button (profile order), the rows shown
    /// for the buttons, the tab of «Asignaciones», the shortcut captured in
    /// «Teclas», the «Comandos» list (rows and the search they come from),
    /// the leader lines (x1, y1, x2, y2) and the drawing (x, y, px per
    /// unit).
    sel: usize,
    /// Whether `sel` was chosen (by the user or once the buttons were read).
    sel_set: bool,
    button_rows: Vec<ButtonRow>,
    as_tab: AsTab,
    as_key: Option<(u8, u8)>,
    list_rows: Vec<Option<usize>>,
    list_query: Option<String>,
    leaders: Vec<(i32, i32, i32, i32)>,
    art: Option<(i32, i32, f32, &'static super::mouse_art::Art)>,
    /// The cached photo of this mouse (plan `fotos-de-ratones`): which file
    /// and cache change it was read for, its bytes, and the copy scaled to
    /// the drawing's box (painted instead of the drawing, also behind each
    /// point). Released before GDI+ shuts down (WM_DESTROY comes first).
    photo_key: Option<(&'static str, u64)>,
    photo: Option<(&'static super::mouse_art::PhotoArt, Rc<[u8]>)>,
    picture: Option<Rc<glyph::Picture>>,
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
    /// Painted rows of «Avanzado»; profiles: the status icon, the last
    /// result, the profile being used and the one it replaces, a profile to
    /// use once a save ends, and whether the focus goes to the new active
    /// pill.
    rows: Vec<(RECT, bool)>,
    profile_icon: Option<(i32, i32, bool)>,
    profile_status: Option<(bool, String)>,
    using: Option<(String, String)>,
    then_use: Option<String>,
    focus_active: bool,
    /// Profile pills, in order: (name, active).
    pills: Vec<(String, bool)>,
    /// The tab shown, and the controls hidden on purpose in any tab.
    tab: Tab,
    hidden: Vec<i32>,
    /// «Avanzado»: last result, automatic results already seen, and whether
    /// the worker knows about unsaved button changes.
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
        current_dpi(snap).and_then(|(c, _)| c),
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
    let rate = m.rate.filter(|&ms| {
        rate_of(snap)
            .is_some_and(|r| r.writable && r.current_ms != ms && r.supported_ms.contains(&ms))
    });
    ProfileEdits {
        dpi,
        light,
        buttons: m.edits.clone(),
        rate,
    }
}

fn rate_of(snap: &Snapshot) -> Option<&crate::devices::ReportRate> {
    match &snap.status {
        Status::Ok(st) => st.report_rate.as_ref(),
        _ => None,
    }
}

fn count(e: &ProfileEdits) -> usize {
    e.dpi.is_some() as usize
        + e.light.is_some() as usize
        + e.buttons.len()
        + e.rate.is_some() as usize
}

/// Only the report rate is pending: nothing goes to the mouse's memory.
fn only_rate(e: &ProfileEdits) -> bool {
    e.rate.is_some() && e.dpi.is_none() && e.light.is_none() && e.buttons.is_empty()
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
/// `auto`: a mouse found by automatic detection, saved in the app only.
pub fn range_sentence(range: &DpiRange, auto: bool) -> String {
    let admits = match range {
        DpiRange::Range { min, max, step } => format!("de {min} a {max}, en pasos de {step}"),
        DpiRange::Values(_) => text::dpi_range(range),
    };
    let save = if auto {
        "Al guardarlo, OpenPeripheral lo aplica cada vez que conectas el ratón."
    } else {
        "Al guardarlo se escribe en los 4 niveles del botón DPI."
    };
    format!("DPI con el que arranca el ratón. Admite {admits}. {save}")
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

/// A button that cannot be changed: the left one (a click must remain) and
/// a profile function this app cannot write, e.g. the DPI cycle of the
/// G305's DPI button.
pub fn locked(row: usize, device: Action) -> bool {
    row == 0 || matches!(device, Action::Raw(_))
}

/// The page's rows of buttons from the mouse's buttons and the pending
/// changes (pure; tested).
fn button_rows(buttons: &[Action], edits: &[(usize, Action)]) -> Vec<ButtonRow> {
    buttons
        .iter()
        .enumerate()
        .map(|(row, &device)| {
            let new = edits.iter().find(|(r, _)| *r == row).map(|(_, a)| *a);
            ButtonRow {
                name: text::mouse_button(row),
                action: text::action(new.unwrap_or(device)),
                before: new.map(|_| text::action(device)),
                locked: locked(row, device),
            }
        })
        .collect()
}

/// Rows of the «Comandos» list: a group header (`None`) before each group's
/// commands (`Some(index)`), for the commands matching `query`.
fn command_rows(query: &str) -> Vec<Option<usize>> {
    let mut rows = Vec::new();
    let mut group = "";
    for i in super::commands::filter(query) {
        let c = &super::commands::COMMANDS[i];
        if c.group != group {
            rows.push(None);
            group = c.group;
        }
        rows.push(Some(i));
    }
    rows
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
/// are never captured (they leave the field), nor anything with the Windows
/// key (decision of the user, 2026-10-08: Windows acts on it itself); lone
/// modifiers and keys without a name are ignored.
pub fn capture(vk: u16, ctrl: bool, shift: bool, alt: bool, win: bool) -> Option<(u8, u8)> {
    if vk == VK_TAB.0 || vk == VK_ESCAPE.0 || win {
        return None;
    }
    let usage = vk_usage(vk)?;
    let mods = ctrl as u8 | (shift as u8) << 1 | (alt as u8) << 2;
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
    add(ID_PRF_LABEL, Kind::Text(Tertiary, App), s, "PERFILES", st);
    for i in 0..MAX_ROWS {
        add(
            ID_PILL0 + i,
            Kind::Button(ButtonStyle::Normal, App),
            b,
            "",
            btn,
        );
    }
    add(
        ID_PRF_NEW,
        Kind::Button(ButtonStyle::Normal, App),
        b,
        "Nuevo perfil…",
        btn,
    );
    add(
        ID_PRF_MENU,
        Kind::Button(ButtonStyle::Normal, App),
        b,
        "Opciones del perfil",
        btn,
    );
    add(ID_PRF_HINT, Kind::Text(Secondary, App), s, "", st);
    add(ID_PRF_GAME, Kind::Text(Notice, App), s, "", st);
    add(ID_PRF_FIRST, Kind::Text(Secondary, App), s, "", st);
    add(ID_WRITE_LOCK, Kind::Text(Notice, App), s, "", st);
    add(ID_RST_STATUS, Kind::Text(Secondary, App), s, "", st);
    add(ID_PRF_MISMATCH, Kind::Text(Notice, App), s, "", st);
    add(
        ID_PRF_REAPPLY,
        Kind::Button(ButtonStyle::Primary, App),
        b,
        "Volver a aplicar",
        btn,
    );
    add(
        ID_PRF_SAVEAS,
        Kind::Button(ButtonStyle::Normal, App),
        b,
        "Guardar como perfil nuevo…",
        btn,
    );
    add(ID_PRF_STATUS, Kind::Text(Secondary, App), s, "", st);
    // Tabs: one Tab stop (the chosen one) and the arrows move inside; the
    // next control starts a new group.
    for (i, (_, label)) in TABS.iter().enumerate() {
        let style = (BS_OWNERDRAW | BS_NOTIFY) as u32 | if i == 0 { WS_GROUP.0 } else { 0 };
        add(
            ID_TAB0 + i as i32,
            Kind::Button(ButtonStyle::Normal, App),
            b,
            label,
            style,
        );
    }
    // «Botones y DPI», in tab order: «Asignaciones», the points of the
    // drawing (or the list of a narrow window), then the DPI.
    add(
        ID_AS_TITLE,
        Kind::Text(Name, Card),
        s,
        "Asignaciones",
        st | WS_GROUP.0,
    );
    add(ID_AS_FOR, Kind::Text(Secondary, Card), s, "", st);
    for (i, (_, label)) in AS_TABS.iter().enumerate() {
        let style = (BS_OWNERDRAW | BS_NOTIFY) as u32 | if i == 0 { WS_GROUP.0 } else { 0 };
        add(
            ID_AS_TAB0 + i as i32,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            label,
            style,
        );
    }
    add(ID_AS_LOCK, Kind::Text(Notice, Card), s, "", st | WS_GROUP.0);
    // A label directly before a field becomes its accessible name.
    add(
        ID_AS_SEARCH_LABEL,
        Kind::Text(Notice, Card),
        s,
        "Buscar un comando",
        st,
    );
    add(
        ID_AS_SEARCH,
        Kind::Edit,
        w!("EDIT"),
        "",
        WS_TABSTOP.0 | ES_AUTOHSCROLL as u32,
    );
    add(ID_AS_COUNT, Kind::Text(Tertiary, Card), s, "", st);
    add(
        ID_AS_LIST,
        Kind::List,
        w!("LISTBOX"),
        "",
        WS_TABSTOP.0
            | WS_VSCROLL.0
            | (LBS_OWNERDRAWFIXED | LBS_HASSTRINGS | LBS_NOTIFY | LBS_NOINTEGRALHEIGHT) as u32,
    );
    add(
        ID_AS_KEY_LABEL,
        Kind::Text(Notice, Card),
        s,
        "Combinación de teclas",
        st,
    );
    add(
        ID_AS_KEY,
        Kind::Edit,
        w!("EDIT"),
        "",
        WS_TABSTOP.0 | (ES_READONLY | ES_CENTER | ES_AUTOHSCROLL) as u32,
    );
    add(
        ID_AS_KEY_HELP,
        Kind::Text(Secondary, Card),
        s,
        "Con el campo seleccionado, pulsa la combinación. Tab y Esc no se capturan: sirven para salir del campo. Tampoco la tecla Windows.",
        st,
    );
    add(
        ID_AS_KEY_ASSIGN,
        Kind::Button(ButtonStyle::Primary, Card),
        b,
        "Asignar el atajo",
        btn,
    );
    add(
        ID_AS_KEY_CLEAR,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Borrar el atajo",
        btn,
    );
    for (i, &mask) in MOUSE_ACTIONS.iter().enumerate() {
        add(
            ID_AS_MOUSE0 + i as i32,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            &text::action(Action::Mouse(mask)),
            btn,
        );
    }
    add(
        ID_BTN_STATUS,
        Kind::Text(Notice, App),
        s,
        "",
        st | WS_GROUP.0,
    );
    add(
        ID_BTN_ACTIVATE,
        Kind::Button(ButtonStyle::Primary, App),
        b,
        "Activar la memoria del ratón",
        btn,
    );
    for i in 0..MAX_BUTTONS {
        add(
            ID_DOT0 + i,
            Kind::Button(ButtonStyle::Normal, App),
            b,
            "",
            btn,
        );
    }
    for i in 0..MAX_BUTTONS {
        add(
            ID_ROW0 + i,
            Kind::Button(ButtonStyle::Normal, App),
            b,
            "",
            btn,
        );
    }
    for i in 0..MAX_BUTTONS {
        add(ID_LBL_NAME0 + i, Kind::Text(Tertiary, App), s, "", st);
        add(ID_LBL_ACT0 + i, Kind::Text(Strong, App), s, "", st);
    }
    add(
        ID_BTN_HINT,
        Kind::Text(Secondary, App),
        s,
        "Pulsa un punto del ratón y elige su acción en «Asignaciones». Lo que cambies queda pendiente hasta que lo guardes.",
        st,
    );
    add(
        ID_DPI_TITLE,
        Kind::Text(Name, Card),
        s,
        "Sensibilidad (DPI)",
        st | WS_GROUP.0,
    );
    add(ID_DPI_BADGE, Kind::Text(Tertiary, Card), s, "", st);
    add(
        ID_DPI_NOW_LABEL,
        Kind::Text(Tertiary, Card),
        s,
        "ACTUAL",
        st,
    );
    add(ID_DPI_NOW, Kind::Text(Title, Card), s, "", st);
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
    add(
        ID_RATE_TITLE,
        Kind::Text(Name, Card),
        s,
        "Rendimiento del sensor",
        st | WS_GROUP.0,
    );
    add(ID_RATE_NOW, Kind::Text(Secondary, Card), s, "", st);
    // Choosing does not write: one Tab stop per button (DESIGN.md).
    for i in 0..MAX_RATES {
        add(
            ID_RATE0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            "",
            btn,
        );
    }
    add(ID_RATE_HELP, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_GAM_TITLE,
        Kind::Text(Name, Card),
        s,
        "Programas vinculados",
        st | WS_GROUP.0,
    );
    add(ID_GAM_DESC, Kind::Text(Secondary, Card), s, "", st);
    for i in 0..MAX_PROGS {
        add(ID_GAM_NAME0 + i, Kind::Text(Strong, Card), s, "", st);
        add(ID_GAM_PATH0 + i, Kind::Text(Secondary, Card), s, "", st);
        add(
            ID_GAM_DEL0 + i,
            Kind::Button(ButtonStyle::Normal, Card),
            b,
            "Desvincular",
            btn,
        );
    }
    add(ID_GAM_EMPTY, Kind::Text(Secondary, Card), s, "", st);
    add(
        ID_GAM_ADD,
        Kind::Button(ButtonStyle::Normal, Card),
        b,
        "Vincular programa…",
        btn,
    );
    add(ID_GAM_STATUS, Kind::Text(Notice, Card), s, "", st);
    add(ID_GAM_FOOT, Kind::Text(Secondary, Card), s, "", st);
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
        if let Some(key) = ctls.iter().find(|c| c.id == ID_AS_KEY) {
            let _ = SetWindowSubclass(key.hwnd, Some(capture_proc), 1, 0);
        }
    }
    // Hidden until needed: hidden controls are skipped by Tab.
    let hidden: Vec<i32> = ctls
        .iter()
        .map(|c| c.id)
        .filter(|&id| {
            (ID_DOT0..=ID_BTN_LAST).contains(&id)
                || [ID_BAR_DISCARD, ID_BAR_SAVE, ID_BTN_ACTIVATE].contains(&id)
                || (ID_PILL0..ID_PILL0 + MAX_ROWS).contains(&id)
                || (ID_GAM_DEL0..ID_GAM_DEL0 + MAX_PROGS).contains(&id)
                || (ID_RATE0..ID_RATE0 + MAX_RATES).contains(&id)
                || id == ID_RATE_TITLE
                || [
                    ID_PRF_LABEL,
                    ID_PRF_NEW,
                    ID_PRF_MENU,
                    ID_PRF_REAPPLY,
                    ID_PRF_SAVEAS,
                    ID_GAM_TITLE,
                    ID_GAM_ADD,
                ]
                .contains(&id)
        })
        .collect();
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
        rate: None,
        pending: None,
        applied: None,
        restore_text: None,
        brushes: None,
        separators: Vec::new(),
        profile_requested: false,
        edits: Vec::new(),
        buttons_result: None,
        badges: Vec::new(),
        boxes: Vec::new(),
        save_icon: None,
        sel: 0,
        sel_set: false,
        button_rows: Vec::new(),
        as_tab: AsTab::Commands,
        as_key: None,
        list_rows: Vec::new(),
        list_query: None,
        leaders: Vec::new(),
        art: None,
        photo_key: None,
        photo: None,
        picture: None,
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
        profile_icon: None,
        profile_status: None,
        using: None,
        then_use: None,
        focus_active: false,
        pills: Vec::new(),
        tab: Tab::Buttons,
        hidden,
        games_status: None,
        autos_seen: super::snapshot().map(|s| s.autos).unwrap_or(0),
        page_edits_sent: false,
    };
    with_app(|a| {
        a.view.mouse = Some(state);
        a.view.scroll = 0;
    });
    apply_tab_mask();
    fill_field_from_device(view);
    if let Some(p) = with_app(|a| a.view.palette) {
        super::theme::apply_scroll_bars(view, &p);
    }
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
    // The worker may write buttons again (no unsaved ones left here).
    if state.page_edits_sent {
        send(Command::PageEdits(state.key.clone(), false));
    }
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

/// Shows or hides a control on purpose; it also needs its tab to be shown.
fn show(id: i32, on: bool) {
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.hidden.retain(|&h| h != id);
            if !on {
                m.hidden.push(id);
            }
        }
    });
    apply_visibility(id);
}

/// Visible when wanted and on the current tab; hidden controls are
/// skipped by Tab.
fn apply_visibility(id: i32) {
    let Some((h, on)) = with_app(|a| {
        let m = a.view.mouse.as_ref()?;
        let c = m.ctls.iter().find(|c| c.id == id)?;
        let on = !m.hidden.contains(&id) && tab_of(id).is_none_or(|t| t == m.tab);
        Some((c.hwnd, on))
    })
    .flatten() else {
        return;
    };
    unsafe {
        let _ = ShowWindow(h, if on { SW_SHOWNA } else { SW_HIDE });
    }
}

fn apply_tab_mask() {
    let ids = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| m.ctls.iter().map(|c| c.id).collect::<Vec<_>>())
    })
    .flatten()
    .unwrap_or_default();
    for id in ids {
        apply_visibility(id);
    }
}

/// Shows another tab of the page; pending changes stay.
fn select_tab(view: HWND, tab: Tab) {
    let changed = with_app(|a| {
        a.view.mouse.as_mut().map(|m| {
            let changed = m.tab != tab;
            m.tab = tab;
            changed
        })
    })
    .flatten();
    if changed == Some(true) {
        apply_tab_mask();
        refresh(view);
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

/// DPI the sensor uses now (`None`: the mouse cannot report it) and the
/// values it accepts.
fn current_dpi(snap: &Snapshot) -> Option<(Option<u16>, DpiRange)> {
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
    // The report rate is read-only: a fact of the header.
    let rate = snap.and_then(|s| match &s.status {
        Status::Ok(st) => st.report_rate.as_ref().map(|r| text::hz(r.current_ms)),
        _ => None,
    });
    let sub = snap
        .map(|s| {
            let mut t = format!("{} · {}", text::kind(s.info.kind), text::card_status(s));
            if let Some(hz) = rate {
                t.push_str(&format!(" · {hz} Hz"));
            }
            t
        })
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
    let mut notice = Vec::new();
    if snap.is_some_and(|s| s.info.support == crate::devices::Support::Auto) {
        notice.push(text::AUTO_NOTICE.to_string());
    }
    if shared.general.conflict_notice {
        notice.push(text::conflicts(&shared.conflicts));
    }
    notice.retain(|n| !n.is_empty());
    set(
        ID_NOTICE,
        &notice.join(
            "
",
        ),
    );
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
        let auto = snap.is_some_and(Snapshot::saves_in_app);
        set(ID_DPI_DESC, &range_sentence(&d.allowed, auto));
        set(
            ID_DPI_CURRENT,
            &saved_dpi_text(snap.and_then(|s| s.saved_dpi), cycle, base),
        );
        // What the sensor uses now: a DPI being tried shows here first.
        set(
            ID_DPI_NOW,
            &d.current.map_or("—".to_string(), |c| format!("{c} DPI")),
        );
        let (lo, hi) = bounds(&d.allowed);
        let from = field.or(d.current).unwrap_or(lo);
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
        let (status, icon) = if let Some((_, Pending::Apply(v))) = pending {
            (format!("Enviando {v} DPI al ratón para probarlo…"), None)
        } else if invalid {
            (invalid_message(&d.allowed, &typed), Some(false))
        } else if let Some((ok, v)) = applied.filter(|&(_, v)| field == Some(v)) {
            if !ok {
                (
                    match d.current {
                        Some(c) => format!(
                            "Error: el ratón no respondió; sigue en {c} DPI. Muévelo para despertarlo y vuelve a intentarlo."
                        ),
                        None => "Error: el ratón no respondió. Muévelo para despertarlo y vuelve a intentarlo.".to_string(),
                    },
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
        for id in [ID_DPI_EDIT, ID_MINUS, ID_PLUS, ID_APPLY] {
            enable(id, false);
        }
        for i in 0..PRESETS.len() as i32 {
            enable(ID_PRESET0 + i, false);
        }
        set(ID_DPI_STATUS, "");
        set(ID_DPI_NOW, "—");
    }

    // Light section: only for a mouse that declares light effects.
    refresh_light(snap, &shared, connected, busy, pending, &edits);

    // Buttons.
    let profile = snap.map(|s| s.profile.clone()).unwrap_or_default();
    let requested = with_app(|a| a.view.mouse.as_ref().map(|m| m.profile_requested))
        .flatten()
        .unwrap_or(true);
    // The profile is read again each time the page opens: another program
    // (G HUB) may have changed the mouse or its onboard mode since.
    if connected && !requested {
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
        Profile::HostMode => (text::HOST_MODE.into(), Vec::new()),
        Profile::Failed(e) => (
            format!("Error: no se pudo leer el perfil del ratón ({e})."),
            Vec::new(),
        ),
    };
    set(ID_BTN_STATUS, &status);
    show(ID_BTN_ACTIVATE, connected && profile == Profile::HostMode);
    enable(ID_BTN_ACTIVATE, pending.is_none());
    let backup_date = match snap.and_then(|s| s.backup.clone()) {
        Some(Ok(date)) => Some(date),
        _ => None,
    };
    let editable = editing_available() && !buttons.is_empty() && backup_date.is_some();
    let ghub = shared.conflicts.contains(&"Logitech G HUB");
    let result =
        with_app(|a| a.view.mouse.as_ref().and_then(|m| m.buttons_result.clone())).flatten();
    let writing = matches!(pending, Some((_, Pending::Buttons(_))));
    // The points of the drawing, their labels and the list of a narrow
    // window share the chosen button.
    let rows = button_rows(&buttons, &edits.buttons);
    let sel = with_app(|a| {
        let m = a.view.mouse.as_mut()?;
        // Once the buttons are known: the first one that can change.
        if !rows.is_empty() && (m.sel >= rows.len() || !m.sel_set) {
            m.sel = rows.iter().position(|r| !r.locked).unwrap_or(0);
            m.sel_set = true;
        }
        m.button_rows = rows.clone();
        Some(m.sel)
    })
    .flatten()
    .unwrap_or(0);
    show_button_views(view, rows.len(), drawn_points(view));
    for (i, r) in rows.iter().enumerate().take(MAX_BUTTONS as usize) {
        let id = |first: i32| first + i as i32;
        let note = if r.locked {
            " · no se cambia".to_string()
        } else {
            r.before
                .as_ref()
                .map(|b| format!(" · Cambiado · antes: {b}"))
                .unwrap_or_default()
        };
        set(id(ID_LBL_NAME0), &format!("{}{note}", r.name));
        set(id(ID_LBL_ACT0), &r.action);
        set_kind(
            id(ID_LBL_ACT0),
            Kind::Text(
                if r.locked {
                    Role::Secondary
                } else {
                    Role::Strong
                },
                Surface::App,
            ),
        );
        // The accessible name carries everything the label shows.
        let mut name = format!("{}: {}", r.name, r.action);
        if r.before.is_some() {
            name.push_str(", cambiado");
        }
        if r.locked {
            name.push_str(", no se cambia");
        }
        if i == sel {
            name.push_str(", elegido");
        }
        set(id(ID_DOT0), &name);
        set(id(ID_ROW0), &name);
    }

    // «Asignaciones», for the chosen button.
    let chosen = rows.get(sel);
    let as_tab = with_app(|a| a.view.mouse.as_ref().map(|m| m.as_tab))
        .flatten()
        .unwrap_or_default();
    let panel = !rows.is_empty();
    let reason = match chosen {
        _ if !panel => None,
        _ if !editing_available() => Some(
            "Este ratón todavía no permite cambiar sus botones desde OpenPeripheral.".to_string(),
        ),
        _ if backup_date.is_none() => {
            Some("Sin la copia del perfil original no se pueden cambiar los botones.".to_string())
        }
        Some(r) if r.locked && sel == 0 => {
            Some("El botón izquierdo no se cambia: así siempre puedes usar el ratón.".into())
        }
        Some(r) if r.locked => Some(format!(
            "«{}» tiene una función del perfil (en el G305, el ciclo de DPI) que todavía no se puede reasignar.",
            r.name
        )),
        _ => None,
    };
    let usable = panel && reason.is_none();
    let can = usable && connected && !busy;
    show(ID_AS_TITLE, panel);
    show(ID_AS_FOR, panel);
    set(
        ID_AS_FOR,
        &chosen
            .map(|r| format!("Para: {} · ahora {}", r.name, r.action))
            .unwrap_or_default(),
    );
    set(ID_AS_LOCK, reason.as_deref().unwrap_or(""));
    show(ID_AS_LOCK, panel && reason.is_some());
    for (i, (t, label)) in AS_TABS.iter().enumerate() {
        let id = ID_AS_TAB0 + i as i32;
        show(id, usable);
        set(
            id,
            &if *t == as_tab {
                format!("{label}, elegida")
            } else {
                label.to_string()
            },
        );
        set_tabstop(id, *t == as_tab);
        enable(id, can);
    }
    let on = |t: AsTab| usable && as_tab == t;
    for id in [ID_AS_SEARCH_LABEL, ID_AS_SEARCH, ID_AS_COUNT, ID_AS_LIST] {
        show(id, on(AsTab::Commands));
        enable(id, can);
    }
    for id in [
        ID_AS_KEY_LABEL,
        ID_AS_KEY,
        ID_AS_KEY_HELP,
        ID_AS_KEY_ASSIGN,
        ID_AS_KEY_CLEAR,
    ] {
        show(id, on(AsTab::Keys));
    }
    let current = buttons.get(sel).map(|&d| {
        edits
            .buttons
            .iter()
            .find(|(r, _)| *r == sel)
            .map_or(d, |(_, a)| *a)
    });
    for (i, &mask) in MOUSE_ACTIONS.iter().enumerate() {
        let id = ID_AS_MOUSE0 + i as i32;
        let a = Action::Mouse(mask);
        let is = current == Some(a);
        show(id, on(AsTab::Mouse));
        set(
            id,
            &if is {
                format!("{}, asignado", text::action(a))
            } else {
                text::action(a)
            },
        );
        set_style(
            id,
            if is {
                ButtonStyle::Selected
            } else {
                ButtonStyle::Normal
            },
        );
        enable(id, can);
    }
    let captured = with_app(|a| a.view.mouse.as_ref().and_then(|m| m.as_key)).flatten();
    set(
        ID_AS_KEY,
        &captured
            .map(|(m, u)| text::shortcut(m, u))
            .unwrap_or_default(),
    );
    enable(ID_AS_KEY, can);
    enable(ID_AS_KEY_ASSIGN, can && captured.is_some());
    enable(ID_AS_KEY_CLEAR, can && captured.is_some());
    refresh_commands();

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
    } else if only_rate(&edits) {
        set(ID_BAR_TEXT, "1 cambio sin guardar.");
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
        } else if only_rate(&edits) {
            "Guardar…"
        } else {
            "Guardar en el ratón…"
        },
    );
    enable(ID_BAR_DISCARD, !writing);
    enable(
        ID_BAR_SAVE,
        connected && !busy && !ghub && editable && !hex_bad,
    );
    let activating = matches!(pending, Some((_, Pending::Buttons(Write::Activate))));
    let (save_text, save_icon) = if activating {
        ("Activando la memoria del ratón…".to_string(), None)
    } else if writing {
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

    refresh_profiles(snap, &shared, connected, pending);
    refresh_rate(snap, connected, busy);
    refresh_games(snap, &shared, pending, !edits.buttons.is_empty());

    // Why nothing can be written, and «Restaurar originales» (the restores
    // are in the «Perfil …» menu).
    let lock = match snap.and_then(|s| s.backup.clone()) {
        Some(Err(e)) => format!(
            "Error: no se pudo guardar la copia del perfil original ({e}). No se escribirá nada en el ratón."
        ),
        _ if editable && ghub => "Cierra G HUB para guardar en el ratón.".into(),
        _ => String::new(),
    };
    set(ID_WRITE_LOCK, &lock);
    let rst = match (pending, &restore_text) {
        (Some((_, Pending::Restore)), _) => "Restaurando…".to_string(),
        (_, Some((true, t))) => t.clone(),
        (_, Some((false, t))) => format!("Error: {t}"),
        _ => String::new(),
    };
    set(ID_RST_STATUS, &rst);
    let tab = with_app(|a| a.view.mouse.as_ref().map(|m| m.tab))
        .flatten()
        .unwrap_or_default();
    for (i, (t, _)) in TABS.iter().enumerate() {
        let id = ID_TAB0 + i as i32;
        set(
            id,
            &if *t == tab {
                format!("{}, elegida", TABS[i].1)
            } else {
                TABS[i].1.to_string()
            },
        );
        set_tabstop(id, *t == tab);
    }
    keep_focus_visible();

    if let Some(s) = snap {
        refresh_photo(&shared, &s.info.name);
    }
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

/// Texts and states of the profile pills and the notices below them.
fn refresh_profiles(
    snap: Option<&Snapshot>,
    shared: &Shared,
    connected: bool,
    pending: Option<(u64, Pending)>,
) {
    let profiles = snap.map(|s| s.profiles.clone()).unwrap_or_default();
    let active = snap.and_then(|s| s.active_profile.clone());
    let n = profiles.len();
    let has = n > 0;
    for id in [ID_PRF_LABEL, ID_PRF_NEW, ID_PRF_MENU] {
        show(id, has);
    }
    let busy = pending.is_some();
    let ghub = shared.conflicts.contains(&"Logitech G HUB");
    let can_use = connected && !busy && !ghub;
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
    set(
        ID_PRF_GAME,
        &snap.and_then(texts::game_notice).unwrap_or_default(),
    );
    let game = snap.is_some_and(|s| s.game.is_some());
    // Known before any pill is shown (and drawn).
    let pills: Vec<(String, bool)> = profiles
        .iter()
        .map(|p| (p.name.clone(), Some(&p.name) == active.as_ref()))
        .collect();
    with_app(|a| a.view.mouse.as_mut().map(|m| m.pills = pills));
    for i in 0..MAX_ROWS {
        let id = ID_PILL0 + i;
        let Some(p) = profiles.get(i as usize) else {
            show(id, false);
            continue;
        };
        let is_active = Some(&p.name) == active.as_ref();
        show(id, true);
        // Accessible names carry the profile and its state, or the reason
        // it cannot be used; the pill draws only the name.
        let name = if is_active {
            format!(
                "Perfil {}, {}",
                p.name,
                texts::active_label(holds, game).to_lowercase()
            )
        } else {
            match reason {
                Some(r) => format!("Usar el perfil {} ({r})", p.name),
                None => format!("Usar el perfil {}", p.name),
            }
        };
        set(id, &name);
        enable(id, is_active || can_use);
    }
    set(
        ID_PRF_HINT,
        if n >= 2 {
            "Pulsar un perfil lo escribe en el ratón."
        } else {
            ""
        },
    );
    set(ID_PRF_FIRST, if has { texts::first_time(n) } else { "" });
    let full = n >= crate::profiles::MAX_PROFILES;
    set(
        ID_PRF_NEW,
        if full {
            "Nuevo perfil (límite de 8 alcanzado: elimina uno para crear otro)"
        } else {
            "Nuevo perfil…"
        },
    );
    enable(ID_PRF_NEW, !busy && !full && active_p.is_some());
    enable(ID_PRF_MENU, !busy && active_p.is_some());
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
    // After using a profile, the focus goes to its pill.
    if focus {
        if let Some(i) = profiles
            .iter()
            .position(|p| Some(&p.name) == active.as_ref())
        {
            if let Some(h) = ctl(ID_PILL0 + i as i32) {
                unsafe {
                    let _ = SetFocus(Some(h));
                }
            }
        }
    }
}

/// Paths of the programs linked to the active profile, in order (pure;
/// tested).
pub fn linked_programs(programs: &[crate::games::Program], active: Option<&str>) -> Vec<String> {
    let Some(active) = active else {
        return Vec::new();
    };
    programs
        .iter()
        .filter(|p| p.profile.as_deref() == Some(active))
        .map(|p| p.exe.clone())
        .collect()
}

/// «Rendimiento del sensor»: the rate, editable only where it needs no
/// host mode (`ReportRate::writable`).
fn refresh_rate(snap: Option<&Snapshot>, connected: bool, busy: bool) {
    let rate = snap.and_then(rate_of).cloned();
    show(ID_RATE_TITLE, rate.is_some());
    let picked = with_app(|a| a.view.mouse.as_ref().and_then(|m| m.rate)).flatten();
    let Some(r) = rate else {
        for id in [ID_RATE_NOW, ID_RATE_HELP] {
            set(id, "");
        }
        for i in 0..MAX_RATES {
            show(ID_RATE0 + i, false);
        }
        return;
    };
    set(
        ID_RATE_NOW,
        &format!("Tasa de sondeo (actual: {} Hz)", text::hz(r.current_ms)),
    );
    if !r.writable {
        set(
            ID_RATE_HELP,
            "No se puede cambiar sin pasar el ratón a modo host, que es permanente.",
        );
        for i in 0..MAX_RATES {
            show(ID_RATE0 + i, false);
        }
        return;
    }
    let verified = snap.is_some_and(|s| s.info.support == crate::devices::Support::Verified);
    set(
        ID_RATE_HELP,
        if verified {
            "Se pierde al apagar el ratón. OpenPeripheral la vuelve a aplicar al conectarlo."
        } else {
            "Se pierde al apagar el ratón."
        },
    );
    let list = rate_list(&r);
    let chosen = picked.unwrap_or(r.current_ms);
    for i in 0..MAX_RATES {
        let id = ID_RATE0 + i;
        match list.get(i as usize) {
            Some(&ms) => {
                let is = ms == chosen;
                let label = format!("{} Hz", text::hz(ms));
                set(
                    id,
                    &if is {
                        format!("{label}, elegida")
                    } else {
                        label
                    },
                );
                show(id, true);
                set_style(
                    id,
                    if is {
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
}

/// The intervals the mouse declares, from the lowest rate to the highest.
fn rate_list(r: &crate::devices::ReportRate) -> Vec<u8> {
    let mut list = r.supported_ms.clone();
    list.sort_unstable_by(|a, b| b.cmp(a));
    list.truncate(MAX_RATES as usize);
    list
}

/// Texts and states of «Avanzado»: the programs linked to the active
/// profile.
fn refresh_games(
    snap: Option<&Snapshot>,
    shared: &Shared,
    pending: Option<(u64, Pending)>,
    button_edits: bool,
) {
    let has = snap.is_some_and(|s| !s.profiles.is_empty());
    show(ID_GAM_TITLE, has);
    show(ID_GAM_ADD, has);
    let busy = pending.is_some();
    let Some(snap) = snap.filter(|_| has) else {
        for i in 0..MAX_PROGS {
            set(ID_GAM_NAME0 + i, "");
            set(ID_GAM_PATH0 + i, "");
            show(ID_GAM_DEL0 + i, false);
        }
        for id in [ID_GAM_EMPTY, ID_GAM_DESC, ID_GAM_FOOT] {
            set(id, "");
        }
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
    let active = snap.active_profile.clone().unwrap_or_default();
    set(ID_GAM_TITLE, &format!("Programas vinculados a «{active}»"));
    set(
        ID_GAM_DESC,
        "Al abrir uno de estos programas, OpenPeripheral usa este perfil: el DPI y la luz al momento, y escribe los botones en el ratón solo si cambian. Al cerrarlo, vuelve el perfil que había. Solo funciona con OpenPeripheral abierto.",
    );
    let linked = linked_programs(&snap.programs, snap.active_profile.as_deref());
    for i in 0..MAX_PROGS {
        let Some(exe) = linked.get(i as usize) else {
            set(ID_GAM_NAME0 + i, "");
            set(ID_GAM_PATH0 + i, "");
            show(ID_GAM_DEL0 + i, false);
            continue;
        };
        let name = crate::games::display_name(exe);
        let open = snap.game.as_deref() == Some(name.as_str());
        set(
            ID_GAM_NAME0 + i,
            &if open {
                format!("{name} · abierto")
            } else {
                name.clone()
            },
        );
        set(ID_GAM_PATH0 + i, exe);
        show(ID_GAM_DEL0 + i, true);
        set(ID_GAM_DEL0 + i, &format!("Desvincular {name}"));
        enable(ID_GAM_DEL0 + i, !busy);
    }
    set(
        ID_GAM_EMPTY,
        &if linked.is_empty() {
            format!(
                "Ningún programa vinculado a «{active}». Este perfil solo se usa cuando lo eliges."
            )
        } else {
            String::new()
        },
    );
    enable(
        ID_GAM_ADD,
        !busy && snap.programs.len() < crate::games::MAX_PROGRAMS,
    );
    set(
        ID_GAM_FOOT,
        if shared.general.auto_switch {
            "Cambio automático activado. Se cambia en «Ajustes», para todos los ratones."
        } else {
            "Cambio automático desactivado: abrir estos programas no cambia de perfil. Se activa en «Ajustes», para todos los ratones."
        },
    );
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
    let next = (0..MAX_BUTTONS)
        .map(|i| ID_DOT0 + i)
        .chain([ID_PRF_MENU, ID_APPLY, ID_BACK])
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
                (Write::Save, true) if cause.contains("se pierde al apagar") => {
                    "Aplicado. Se pierde al apagar el ratón.".to_string()
                }
                (Write::Save, true) if cause.contains("guardado en OpenPeripheral") => {
                    "Guardado en OpenPeripheral: se aplicará cada vez que conectes el ratón.".to_string()
                }
                (Write::Save, true) => "Guardado en el ratón. Se comprobó la escritura leyendo de nuevo el perfil.".to_string(),
                (Write::Activate, true) => "Memoria del ratón activada: ya puedes ver y cambiar sus botones.".to_string(),
                (Write::Activate, false) => format!("Error: no se pudo activar la memoria del ratón. {cause}"),
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
                m.rate = None;
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
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(view, &mut rc);
    }
    let tab = with_app(|a| a.view.mouse.as_ref().map(|m| m.tab))
        .flatten()
        .unwrap_or_default();
    // The header, the profiles, the tabs and the bar use the whole width
    // (`pl`, `pw`); «Iluminación» and «Avanzado» keep a 760 px column.
    let pl = px(if g.compact { 16.0 } else { 32.0 });
    let pw = (rc.right - 2 * pl).max(1);
    let col = pw.min(px(MAX_COLUMN));
    let left = (rc.right - col) / 2;
    let bh = px(if g.compact { 44.0 } else { 36.0 });
    let btn_w = |label: &str| panel::button_width(&ctx, label);
    let mut cards = Vec::new();
    let mut separators = Vec::new();
    let mut y = px(if g.compact { 16.0 } else { 24.0 });
    // The control's own flag: a control of the hidden bar is still "shown".
    let visible =
        |id: i32| unsafe { GetWindowLongW(h_of(id), GWL_STYLE) as u32 & WS_VISIBLE.0 != 0 };

    // Back button.
    let back_w = btn_w("‹  Dispositivos");
    put(
        ID_BACK,
        pl - ring,
        y - ring,
        back_w + 2 * ring,
        bh + 2 * ring,
    );
    y += bh + px(20.0);

    // Header.
    let icon = px(if g.compact { 32.0 } else { 40.0 });
    let icon_y = y;
    let (pct_w, ph) = measure(ID_PCT, Role::Percent, pw);
    let (sw, sh) = measure(ID_PSTATE, Role::Secondary, pw);
    let has_energy = pct_w > 0 || sw > 0;
    let energy_w = if has_energy {
        px(50.0) + pct_w + px(8.0) + sw
    } else {
        0
    };
    let tx = pl + icon + px(if g.compact { 12.0 } else { 20.0 });
    let title_w = if g.compact || !has_energy {
        pl + pw - tx
    } else {
        pl + pw - tx - energy_w - px(20.0)
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
            put(ID_PCT, bx + px(50.0), by + (row_h - ph) / 2, pct_w + 2, ph);
            put(
                ID_PSTATE,
                bx + px(50.0) + pct_w + px(8.0),
                by + (row_h - sh) / 2,
                sw + 2,
                sh,
            );
            (bx, gy)
        };
    if g.compact {
        y += header_h + px(10.0);
        if has_energy {
            battery = Some(energy_row(pl, y, px(28.0), &mut put));
            y += px(28.0);
        }
    } else {
        if has_energy {
            battery = Some(energy_row(pl + pw - energy_w, y, header_h, &mut put));
        }
        y += header_h;
    }
    if !has_energy {
        put(ID_PCT, 0, 0, 0, 0);
        put(ID_PSTATE, 0, 0, 0, 0);
    }
    y += px(20.0);

    // Disconnected text.
    let (_, dh) = measure(ID_DISC, Role::Notice, pw);
    if dh > 0 {
        put(ID_DISC, pl, y, pw, dh);
        y += dh + px(20.0);
    } else {
        put(ID_DISC, 0, 0, 0, 0);
    }

    // Conflict notice.
    let mut notice = None;
    let (_, nh) = measure(ID_NOTICE, Role::Notice, pw - px(56.0));
    if nh > 0 {
        let box_h = nh.max(px(18.0)) + px(20.0);
        put(ID_NOTICE, pl + px(42.0), y + px(10.0), pw - px(56.0), nh);
        notice = Some(RECT {
            left: pl,
            top: y,
            right: pl + pw,
            bottom: y + box_h,
        });
        y += box_h + px(20.0);
    } else {
        put(ID_NOTICE, 0, 0, 0, 0);
    }

    // Profiles: «PERFILES», a pill per profile, «+ Nuevo», «Perfil …» and
    // the hint, wrapping; then the notices about the profiles.
    let mut profile_status_xy = None;
    let pills = with_app(|a| a.view.mouse.as_ref().map(|m| m.pills.clone()))
        .flatten()
        .unwrap_or_default();
    let notes_w = pw.min(px(900.0));
    if visible(ID_PRF_LABEL) {
        let (lw, lh) = measure(ID_PRF_LABEL, Role::Tertiary, pw);
        let mut x = pl;
        let mut row_y = y;
        put(ID_PRF_LABEL, x, row_y + (bh - lh) / 2, lw + 2, lh);
        x += lw + px(14.0);
        // The active pill is semibold and has a check mark: wider.
        let mut items: Vec<(i32, i32)> = pills
            .iter()
            .enumerate()
            .map(|(i, (name, active))| {
                let extra = if *active { px(34.0) } else { 0 };
                (ID_PILL0 + i as i32, btn_w(name) + extra)
            })
            .collect();
        items.push((ID_PRF_NEW, btn_w("+ Nuevo")));
        items.push((ID_PRF_MENU, btn_w("Perfil …")));
        for (id, w) in items {
            if x > pl && x + w > pl + pw {
                x = pl;
                row_y += bh + px(8.0);
            }
            put(id, x - ring, row_y - ring, w + 2 * ring, bh + 2 * ring);
            x += w + px(8.0);
        }
        let (hw, hh) = measure(ID_PRF_HINT, Role::Secondary, pw);
        if hw > 0 {
            if x + px(8.0) + hw > pl + pw {
                x = pl;
                row_y += bh + px(8.0);
            } else {
                x += px(8.0);
            }
            put(ID_PRF_HINT, x, row_y + (bh - hh) / 2, hw + 2, hh);
        } else {
            put(ID_PRF_HINT, 0, 0, 0, 0);
        }
        y = row_y + bh + px(14.0);
    } else {
        for id in [ID_PRF_LABEL, ID_PRF_NEW, ID_PRF_MENU, ID_PRF_HINT] {
            put(id, 0, 0, 0, 0);
        }
    }
    for (id, role) in [
        (ID_PRF_GAME, Role::Notice),
        (ID_PRF_FIRST, Role::Secondary),
        (ID_WRITE_LOCK, Role::Notice),
        (ID_RST_STATUS, Role::Secondary),
    ] {
        let (_, h) = measure(id, role, notes_w);
        if h > 0 {
            put(id, pl, y, notes_w, h);
            y += h + px(10.0);
        } else {
            put(id, 0, 0, 0, 0);
        }
    }
    // The mouse does not hold the active profile.
    let (_, mh) = measure(ID_PRF_MISMATCH, Role::Notice, notes_w);
    if mh > 0 && visible(ID_PRF_REAPPLY) {
        put(ID_PRF_MISMATCH, pl, y, notes_w, mh);
        y += mh + px(10.0);
        // Drawn in semibold (primary style): wider than measured.
        let rw = btn_w(&txt(ID_PRF_REAPPLY)) + px(16.0);
        let sw = btn_w("Guardar como perfil nuevo…");
        put(
            ID_PRF_REAPPLY,
            pl - ring,
            y - ring,
            rw + 2 * ring,
            bh + 2 * ring,
        );
        if g.compact {
            y += bh + px(10.0);
            put(
                ID_PRF_SAVEAS,
                pl - ring,
                y - ring,
                sw + 2 * ring,
                bh + 2 * ring,
            );
        } else {
            let sx = pl + rw + px(12.0);
            put(
                ID_PRF_SAVEAS,
                sx - ring,
                y - ring,
                sw + 2 * ring,
                bh + 2 * ring,
            );
        }
        y += bh + px(14.0);
    } else {
        for id in [ID_PRF_MISMATCH, ID_PRF_REAPPLY, ID_PRF_SAVEAS] {
            put(id, 0, 0, 0, 0);
        }
    }
    let (_, sth) = measure(ID_PRF_STATUS, Role::Secondary, notes_w - px(24.0));
    if sth > 0 {
        put(ID_PRF_STATUS, pl + px(24.0), y, notes_w - px(24.0), sth);
        profile_status_xy = Some((pl, y));
        y += sth + px(14.0);
    } else {
        put(ID_PRF_STATUS, 0, 0, 0, 0);
    }
    y += px(6.0);

    // Tabs of the page: a row (3 equal columns in a narrow window) over a
    // `border.card` line; each tab draws its part of the line, so the first
    // one starts at the margin with its focus ring area.
    let mut x = pl + ring;
    for (i, (_, label)) in TABS.iter().enumerate() {
        let w = if g.compact {
            (pw - 2 * ring - px(8.0)) / 3
        } else {
            ctx.measure(label, FontRole::Strong, 1000, 1).0 + px(28.0)
        };
        put(
            ID_TAB0 + i as i32,
            x - ring,
            y - ring,
            w + 2 * ring,
            bh + 2 * ring,
        );
        x += w + px(4.0);
    }
    separators.push((pl, pl + pw, y + bh + ring - 1));
    y += bh + ring + px(20.0);

    let pad = px(if g.compact { 16.0 } else { 20.0 });
    let mut badges = Vec::new();
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
    let mut rows = Vec::new();
    let mut field = RECT::default();
    let mut status_xy = (0, 0);
    let mut boxes = Vec::new();
    let mut leaders = Vec::new();
    let mut art_at = None;
    let mut picture = None;

    // «Botones y DPI» (DESIGN.md, revisión 2026-10-08): «Asignaciones», the
    // drawing and the DPI side by side from 1290 px, the DPI under the
    // drawing from 760 px, everything stacked below.
    if tab == Tab::Buttons {
        use super::mouse_art::Side;
        let width_u = rc.right as f32 / g.scale;
        let stacked = width_u < 760.0;
        let three = width_u >= 1290.0;
        let gap = px(20.0);
        let (aw, dw) = (px(320.0), px(300.0));
        let (cx, cw) = if stacked {
            (pl, pw)
        } else if three {
            (pl + aw + gap, pw - aw - dw - 2 * gap)
        } else {
            (pl + aw + gap, pw - aw - gap)
        };
        let top = y;
        // Lowest content y that still fits above the bar: below the cards
        // come 20 px of gap and 20 px of page padding.
        let bar_h = with_app(|a| a.view.mouse.as_ref().map(|m| m.bar_h))
            .flatten()
            .unwrap_or(0);
        let fit_bottom = rc.bottom - bar_h - px(40.0);
        let (art, button_rows) = with_app(|a| {
            let m = a.view.mouse.as_ref()?;
            let art = a
                .view
                .shared
                .devices
                .iter()
                .find(|d| d.info.key == m.key)
                .map(|d| super::mouse_art::art_for(&d.info.name));
            Some((art, m.button_rows.clone()))
        })
        .flatten()
        .unwrap_or_default();
        let vector = art.unwrap_or(&super::mouse_art::GENERIC);
        // The photo replaces the drawing in the same box (not in the narrow
        // list); if GDI+ cannot scale it, the drawing stays.
        let photo = with_app(|a| a.view.mouse.as_ref().and_then(|m| m.photo.clone()))
            .flatten()
            .filter(|_| !stacked);
        let mut art = photo.as_ref().map_or(vector, |(pa, _)| &pa.art);
        // Buttons with a point on the drawing; the rest go to the list
        // under it (a narrow window lists them all).
        let n = button_rows
            .len()
            .min(art.points.len())
            .min(MAX_BUTTONS as usize);
        // The window may have crossed 760 px: labels or list.
        show_button_views(view, button_rows.len(), n);

        // The drawing, its points and their labels; the list when narrow.
        let mut cy = top;
        let (_, sh) = measure(ID_BTN_STATUS, Role::Notice, cw);
        if sh > 0 {
            put(ID_BTN_STATUS, cx, cy, cw, sh);
            cy += sh + px(12.0);
        } else {
            put(ID_BTN_STATUS, 0, 0, 0, 0);
        }
        if visible(ID_BTN_ACTIVATE) {
            // Drawn in semibold (primary style): wider than measured.
            let w = btn_w(&txt(ID_BTN_ACTIVATE)) + px(16.0);
            put(
                ID_BTN_ACTIVATE,
                cx - ring,
                cy - ring,
                w + 2 * ring,
                bh + 2 * ring,
            );
            cy += bh + px(12.0);
        }
        let name_h = ctx.measure("Ag", FontRole::Body, 1000, 1).1;
        let act_h = ctx.measure("Ag", FontRole::Strong, 1000, 1).1;
        if n > 0 {
            let side_w = if stacked { 0 } else { px(180.0) };
            let has_top = !stacked && art.points[..n].iter().any(|p| p.side == Side::Top);
            let ay0 = cy
                + if has_top {
                    name_h + act_h + px(18.0)
                } else {
                    px(8.0)
                };
            // With three columns the page fits the window: the drawing and
            // the hint under it shrink (down to 300 px) instead of scrolling.
            let max_h = if stacked {
                px(300.0)
            } else if three {
                let (_, hh) = measure(ID_BTN_HINT, Role::Secondary, cw);
                (fit_bottom - ay0 - px(32.0) - hh).clamp(px(300.0), px(470.0))
            } else {
                px(470.0)
            };
            let max_w = (cw - 2 * side_w - px(40.0)).max(px(120.0));
            let k = (max_h as f32 / art.size.1).min(max_w as f32 / art.size.0);
            let (art_w, art_h) = ((art.size.0 * k) as i32, (art.size.1 * k) as i32);
            if let Some((pa, bytes)) = &photo {
                picture = with_app(|a| a.view.mouse.as_ref().and_then(|m| m.picture.clone()))
                    .flatten()
                    .filter(|p| (p.w, p.h) == (art_w, art_h))
                    .or_else(|| glyph::Picture::scaled(bytes, pa.crop, art_w, art_h).map(Rc::new));
                if picture.is_none() && vector.size == art.size {
                    art = vector;
                }
            }
            let ax0 = cx + (cw - art_w) / 2;
            art_at = Some((ax0, ay0, k, art));
            let dot = px(28.0);
            // Bottom of the last label on the left and on the right: a label
            // that would overlap it goes under its line instead.
            let mut last = [i32::MIN; 2];
            let mut order: Vec<usize> = (0..n).collect();
            order.sort_by(|&a, &b| art.points[a].at.1.total_cmp(&art.points[b].at.1));
            for i in order {
                let p = art.points[i];
                let (dx, dy) = (ax0 + (p.at.0 * k) as i32, ay0 + (p.at.1 * k) as i32);
                let id = |first: i32| first + i as i32;
                put(id(ID_DOT0), dx - dot / 2, dy - dot / 2, dot, dot);
                if stacked {
                    put(id(ID_LBL_NAME0), 0, 0, 0, 0);
                    put(id(ID_LBL_ACT0), 0, 0, 0, 0);
                    continue;
                }
                // Labels never cover the drawing (they are opaque).
                let (lx, lw, align) = match p.side {
                    Side::Left => (cx, (dx - px(24.0)).min(ax0 - px(8.0)) - cx, 0),
                    Side::Right => {
                        let lx = (dx + px(24.0)).max(ax0 + art_w + px(8.0));
                        (lx, cx + cw - lx, 2)
                    }
                    Side::Top => (dx - px(150.0), px(300.0), 1),
                };
                let (_, nh) = measure(id(ID_LBL_NAME0), Role::Tertiary, lw);
                let (_, ah) = measure(id(ID_LBL_ACT0), Role::Strong, lw);
                let h = nh + px(2.0) + ah;
                let ly = match p.side {
                    Side::Top => cy,
                    side => {
                        let s = usize::from(side == Side::Right);
                        let above = dy - px(6.0) - h;
                        let ly = if above < last[s] + px(6.0) {
                            dy + px(6.0)
                        } else {
                            above
                        };
                        last[s] = ly + h;
                        ly
                    }
                };
                set_align(id(ID_LBL_NAME0), align);
                set_align(id(ID_LBL_ACT0), align);
                put(id(ID_LBL_NAME0), lx, ly, lw, nh);
                put(id(ID_LBL_ACT0), lx, ly + nh + px(2.0), lw, ah);
                leaders.push(match p.side {
                    Side::Left => (cx, dy, dx - dot / 2, dy),
                    Side::Right => (dx + dot / 2, dy, cx + cw, dy),
                    Side::Top => (dx, ly + h + px(4.0), dx, dy - dot / 2),
                });
            }
            for i in n as i32..MAX_BUTTONS {
                for first in [ID_DOT0, ID_LBL_NAME0, ID_LBL_ACT0] {
                    put(first + i, 0, 0, 0, 0);
                }
            }
            cy = ay0 + art_h + px(16.0);
            for (i, r) in button_rows.iter().enumerate().take(MAX_BUTTONS as usize) {
                let id = ID_ROW0 + i as i32;
                if !stacked && i < n {
                    put(id, 0, 0, 0, 0);
                    continue;
                }
                let rh = px(if r.before.is_some() { 60.0 } else { 44.0 });
                put(id, cx - ring, cy - ring, cw + 2 * ring, rh + 2 * ring);
                cy += rh + px(4.0);
            }
            let (_, hh) = measure(ID_BTN_HINT, Role::Secondary, cw);
            put(ID_BTN_HINT, cx, cy + px(4.0), cw, hh);
            cy += hh + px(12.0);
        } else {
            for i in 0..MAX_BUTTONS {
                for first in [ID_DOT0, ID_LBL_NAME0, ID_LBL_ACT0, ID_ROW0] {
                    put(first + i, 0, 0, 0, 0);
                }
            }
            put(ID_BTN_HINT, 0, 0, 0, 0);
        }
        let center_bottom = cy;

        // «Asignaciones».
        let (asx, asy, asw) = if stacked {
            (pl, center_bottom + gap, pw)
        } else {
            (pl, top, aw)
        };
        let mut as_bottom = asy;
        let commands = [ID_AS_SEARCH_LABEL, ID_AS_SEARCH, ID_AS_COUNT, ID_AS_LIST];
        let keys = [
            ID_AS_KEY_LABEL,
            ID_AS_KEY,
            ID_AS_KEY_HELP,
            ID_AS_KEY_ASSIGN,
            ID_AS_KEY_CLEAR,
        ];
        if visible(ID_AS_TITLE) {
            let ip = px(16.0);
            let (ax2, iw) = (asx + ip, asw - 2 * ip);
            let mut ay = asy + ip;
            let (_, h) = measure(ID_AS_TITLE, Role::Name, iw);
            put(ID_AS_TITLE, ax2, ay, iw, h);
            ay += h + px(4.0);
            let (_, h) = measure(ID_AS_FOR, Role::Secondary, iw);
            put(ID_AS_FOR, ax2, ay, iw, h);
            ay += h + px(12.0);
            if visible(ID_AS_TAB0) {
                let tw = (iw - px(8.0)) / 3;
                for i in 0..AS_TABS.len() as i32 {
                    put(
                        ID_AS_TAB0 + i,
                        ax2 + i * (tw + px(4.0)) - ring,
                        ay - ring,
                        tw + 2 * ring,
                        bh + 2 * ring,
                    );
                }
                ay += bh + ring + px(14.0);
            } else {
                for i in 0..AS_TABS.len() as i32 {
                    put(ID_AS_TAB0 + i, 0, 0, 0, 0);
                }
            }
            let (_, h) = measure(ID_AS_LOCK, Role::Notice, iw);
            if h > 0 {
                put(ID_AS_LOCK, ax2, ay, iw, h);
                ay += h + px(14.0);
            } else {
                put(ID_AS_LOCK, 0, 0, 0, 0);
            }
            // As tall as the window allows; the list scrolls inside.
            let card_h = if stacked {
                0
            } else {
                (fit_bottom - top).clamp(px(480.0), px(780.0))
            };
            let field_box = |ay: i32, boxes: &mut Vec<RECT>| {
                let f = RECT {
                    left: ax2,
                    top: ay,
                    right: ax2 + iw,
                    bottom: ay + bh,
                };
                boxes.push(f);
                let eh = (bh - px(10.0)).max(px(20.0));
                (f.left + px(8.0), ay + (bh - eh) / 2, iw - px(16.0), eh)
            };
            if visible(ID_AS_LIST) {
                let (lw, lh) = measure(ID_AS_SEARCH_LABEL, Role::Notice, iw);
                put(ID_AS_SEARCH_LABEL, ax2, ay, lw + 2, lh);
                ay += lh + px(6.0);
                let (ex, ey, ew, eh) = field_box(ay, &mut boxes);
                put(ID_AS_SEARCH, ex, ey, ew, eh);
                ay += bh + px(8.0);
                let (_, ch) = measure(ID_AS_COUNT, Role::Tertiary, iw);
                put(ID_AS_COUNT, ax2, ay, iw, ch);
                ay += ch + px(6.0);
                let list_h = if stacked {
                    px(320.0)
                } else {
                    (asy + card_h - ip - ay).max(px(200.0))
                };
                put(ID_AS_LIST, ax2, ay, iw, list_h);
                unsafe {
                    SendMessageW(
                        h_of(ID_AS_LIST),
                        LB_SETITEMHEIGHT,
                        Some(WPARAM(0)),
                        Some(LPARAM(px(34.0) as isize)),
                    );
                }
                ay += list_h;
            } else {
                for id in commands {
                    put(id, 0, 0, 0, 0);
                }
            }
            if visible(ID_AS_KEY) {
                let (lw, lh) = measure(ID_AS_KEY_LABEL, Role::Notice, iw);
                put(ID_AS_KEY_LABEL, ax2, ay, lw + 2, lh);
                ay += lh + px(6.0);
                let (ex, ey, ew, eh) = field_box(ay, &mut boxes);
                put(ID_AS_KEY, ex, ey, ew, eh);
                ay += bh + px(8.0);
                let (_, hh) = measure(ID_AS_KEY_HELP, Role::Secondary, iw);
                put(ID_AS_KEY_HELP, ax2, ay, iw, hh);
                ay += hh + px(12.0);
                // «Asignar» is semibold (primary style): wider than measured.
                let (sw, cw2) = (btn_w("Asignar") + px(16.0), btn_w("Borrar"));
                put(
                    ID_AS_KEY_ASSIGN,
                    ax2 - ring,
                    ay - ring,
                    sw + 2 * ring,
                    bh + 2 * ring,
                );
                put(
                    ID_AS_KEY_CLEAR,
                    ax2 + sw + px(8.0) - ring,
                    ay - ring,
                    cw2 + 2 * ring,
                    bh + 2 * ring,
                );
                ay += bh;
            } else {
                for id in keys {
                    put(id, 0, 0, 0, 0);
                }
            }
            if visible(ID_AS_MOUSE0) {
                for i in 0..MOUSE_ACTIONS.len() as i32 {
                    put(
                        ID_AS_MOUSE0 + i,
                        ax2 - ring,
                        ay - ring,
                        iw + 2 * ring,
                        bh + 2 * ring,
                    );
                    ay += bh + px(6.0);
                }
            } else {
                for i in 0..MOUSE_ACTIONS.len() as i32 {
                    put(ID_AS_MOUSE0 + i, 0, 0, 0, 0);
                }
            }
            as_bottom = (ay + ip).max(asy + card_h);
            cards.push(RECT {
                left: asx,
                top: asy,
                right: asx + asw,
                bottom: as_bottom,
            });
        } else {
            for id in [ID_AS_TITLE, ID_AS_FOR, ID_AS_LOCK]
                .into_iter()
                .chain(commands)
                .chain(keys)
                .chain((0..AS_TABS.len() as i32).map(|i| ID_AS_TAB0 + i))
                .chain((0..MOUSE_ACTIONS.len() as i32).map(|i| ID_AS_MOUSE0 + i))
            {
                put(id, 0, 0, 0, 0);
            }
        }

        // «Sensibilidad (DPI)».
        let (dx0, dy0, dw0) = if three {
            (pl + pw - dw, top, dw)
        } else if stacked {
            (pl, as_bottom + gap, pw)
        } else {
            (cx, center_bottom + gap, cw)
        };
        let ip = px(16.0);
        let (dxi, diw) = (dx0 + ip, dw0 - 2 * ip);
        let mut dy = dy0 + ip;
        let (tw, th) = measure(ID_DPI_TITLE, Role::Name, diw);
        put(ID_DPI_TITLE, dxi, dy, tw + 2, th);
        let (bw, bdh) = measure(ID_DPI_BADGE, Role::Tertiary, diw - px(12.0));
        if bdh > 0 {
            let r = RECT {
                left: dxi,
                top: dy + th + px(6.0),
                right: dxi + bw + px(12.0),
                bottom: dy + th + px(10.0) + bdh,
            };
            put(ID_DPI_BADGE, r.left + px(6.0), r.top + px(2.0), bw + 2, bdh);
            badges.push(r);
            dy = r.bottom + px(14.0);
        } else {
            put(ID_DPI_BADGE, 0, 0, 0, 0);
            dy += th + px(14.0);
        }
        let (lw, lh) = measure(ID_DPI_NOW_LABEL, Role::Tertiary, diw);
        put(ID_DPI_NOW_LABEL, dxi, dy, lw + 2, lh);
        dy += lh + px(2.0);
        let (_, nh) = measure(ID_DPI_NOW, Role::Title, diw);
        put(ID_DPI_NOW, dxi, dy, diw, nh);
        dy += nh + px(2.0);
        let (_, ch) = measure(ID_DPI_CURRENT, Role::Secondary, diw);
        put(ID_DPI_CURRENT, dxi, dy, diw, ch);
        dy += ch + px(16.0);
        let (lw, lh) = measure(ID_DPI_LABEL, Role::Notice, diw);
        put(ID_DPI_LABEL, dxi, dy, lw + 2, lh);
        dy += lh + px(6.0);
        let step_w = px(if g.compact { 44.0 } else { 36.0 });
        let apply_w = btn_w("Reintentar").max(btn_w("Probar"));
        let fw = (diw - 2 * step_w - apply_w - px(22.0)).max(px(60.0));
        field = RECT {
            left: dxi + step_w + px(6.0),
            top: dy,
            right: dxi + step_w + px(6.0) + fw,
            bottom: dy + bh,
        };
        put(
            ID_MINUS,
            dxi - ring,
            dy - ring,
            step_w + 2 * ring,
            bh + 2 * ring,
        );
        put(
            ID_PLUS,
            field.right + px(6.0) - ring,
            dy - ring,
            step_w + 2 * ring,
            bh + 2 * ring,
        );
        let edit_h = (bh - px(10.0)).max(px(20.0));
        put(
            ID_DPI_EDIT,
            field.left + px(4.0),
            dy + (bh - edit_h) / 2,
            fw - px(8.0),
            edit_h,
        );
        put(
            ID_APPLY,
            field.right + px(16.0) + step_w - ring,
            dy - ring,
            apply_w + 2 * ring,
            bh + 2 * ring,
        );
        dy += bh + px(10.0);
        let (_, dh) = measure(ID_DPI_DESC, Role::Secondary, diw);
        put(ID_DPI_DESC, dxi, dy, diw, dh);
        dy += dh + px(16.0);
        let presets: Vec<i32> = (0..PRESETS.len() as i32)
            .map(|i| ID_PRESET0 + i)
            .filter(|&id| visible(id))
            .collect();
        if presets.is_empty() {
            put(ID_PRESETS_LABEL, 0, 0, 0, 0);
        } else {
            let (lw, lh) = measure(ID_PRESETS_LABEL, Role::Secondary, diw);
            put(ID_PRESETS_LABEL, dxi, dy, lw + 2, lh);
            dy += lh + px(8.0);
            let cols = if stacked { 4 } else { 2 };
            let cell = (diw - px(8.0) * (cols - 1)) / cols;
            for (k, id) in presets.iter().enumerate() {
                let (r, c) = (k as i32 / cols, k as i32 % cols);
                put(
                    *id,
                    dxi + c * (cell + px(8.0)) - ring,
                    dy + r * (bh + px(8.0)) - ring,
                    cell + 2 * ring,
                    bh + 2 * ring,
                );
            }
            let rows_n = (presets.len() as i32 + cols - 1) / cols;
            dy += rows_n * bh + (rows_n - 1) * px(8.0) + px(14.0);
        }
        let (_, sth) = measure(ID_DPI_STATUS, Role::Secondary, diw - px(24.0));
        status_xy = (dxi, dy);
        if sth > 0 {
            put(ID_DPI_STATUS, dxi + px(24.0), dy, diw - px(24.0), sth);
            dy += sth + px(10.0);
        } else {
            put(ID_DPI_STATUS, 0, 0, 0, 0);
        }
        let dpi_bottom = dy + ip - px(10.0);
        cards.push(RECT {
            left: dx0,
            top: dy0,
            right: dx0 + dw0,
            bottom: dpi_bottom,
        });
        y = center_bottom.max(as_bottom).max(dpi_bottom) + px(20.0);
    }

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

    // «Avanzado» › «Rendimiento del sensor».
    let rate_ids: Vec<i32> = (0..MAX_RATES)
        .map(|i| ID_RATE0 + i)
        .filter(|&id| visible(id))
        .collect();
    if visible(ID_RATE_TITLE) {
        let top = y;
        let mut cy = y + pad;
        let (_, h) = measure(ID_RATE_TITLE, Role::Name, inner);
        put(ID_RATE_TITLE, ix, cy, inner, h);
        cy += h + px(4.0);
        let (_, h) = measure(ID_RATE_NOW, Role::Secondary, inner);
        put(ID_RATE_NOW, ix, cy, inner, h);
        cy += h + px(12.0);
        if !rate_ids.is_empty() {
            let cols = if g.compact { 2 } else { 4 };
            let cell = (inner - px(8.0) * (cols - 1)) / cols;
            for (k, id) in rate_ids.iter().enumerate() {
                let (r, c) = (k as i32 / cols, k as i32 % cols);
                put(
                    *id,
                    ix + c * (cell + px(8.0)) - ring,
                    cy + r * (bh + px(8.0)) - ring,
                    cell + 2 * ring,
                    bh + 2 * ring,
                );
            }
            let rows_n = (rate_ids.len() as i32 + cols - 1) / cols;
            cy += rows_n * bh + (rows_n - 1) * px(8.0) + px(12.0);
        }
        let (_, h) = measure(ID_RATE_HELP, Role::Secondary, inner);
        put(ID_RATE_HELP, ix, cy, inner, h);
        cy += h + pad;
        cards.push(RECT {
            left,
            top,
            right: left + col,
            bottom: cy,
        });
        y = cy + px(20.0);
    } else {
        for id in [ID_RATE_TITLE, ID_RATE_NOW, ID_RATE_HELP]
            .into_iter()
            .chain((0..MAX_RATES).map(|i| ID_RATE0 + i))
        {
            put(id, 0, 0, 0, 0);
        }
    }

    // «Avanzado»: the programs linked to the active profile.
    let prog_ids = |i: i32| [ID_GAM_NAME0 + i, ID_GAM_PATH0 + i, ID_GAM_DEL0 + i];
    if visible(ID_GAM_TITLE) {
        let top = y;
        let mut cy = y + pad;
        let (_, h) = measure(ID_GAM_TITLE, Role::Name, inner);
        put(ID_GAM_TITLE, ix, cy, inner, h);
        cy += h + px(4.0);
        let (_, h) = measure(ID_GAM_DESC, Role::Secondary, inner);
        put(ID_GAM_DESC, ix, cy, inner, h);
        cy += h + px(14.0);
        // A row per program: name and path, «Desvincular» at the right
        // (below them in a narrow window).
        let rpad = px(if g.compact { 12.0 } else { 14.0 });
        let rin = inner - 2 * rpad;
        let rx = ix + rpad;
        let dw = btn_w("Desvincular");
        for i in 0..MAX_PROGS {
            if txt(ID_GAM_NAME0 + i).is_empty() {
                for id in prog_ids(i) {
                    put(id, 0, 0, 0, 0);
                }
                continue;
            }
            let text_w = if g.compact { rin } else { rin - dw - px(16.0) };
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
                put(
                    ID_GAM_DEL0 + i,
                    rx - ring,
                    ry - ring,
                    dw + 2 * ring,
                    bh + 2 * ring,
                );
                ry + bh + rpad
            } else {
                let content = (ry - (cy + rpad)).max(bh);
                let by = cy + rpad + (content - bh) / 2;
                put(
                    ID_GAM_DEL0 + i,
                    rx + rin - dw - ring,
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
        let aw = btn_w("Vincular programa…");
        put(
            ID_GAM_ADD,
            ix - ring,
            cy - ring,
            aw + 2 * ring,
            bh + 2 * ring,
        );
        cy += bh;
        let (_, h) = measure(ID_GAM_STATUS, Role::Notice, inner);
        if h > 0 {
            cy += px(12.0);
            put(ID_GAM_STATUS, ix, cy, inner, h);
            cy += h;
        } else {
            put(ID_GAM_STATUS, 0, 0, 0, 0);
        }
        // The automatic switch is set in «Ajustes», under a separator.
        cy += px(16.0);
        separators.push((ix, ix + inner, cy));
        cy += px(14.0);
        let (_, h) = measure(ID_GAM_FOOT, Role::Secondary, inner);
        put(ID_GAM_FOOT, ix, cy, inner, h);
        cy += h + pad;
        cards.push(RECT {
            left,
            top,
            right: left + col,
            bottom: cy,
        });
        y = cy + px(20.0);
    } else {
        for id in [
            ID_GAM_TITLE,
            ID_GAM_DESC,
            ID_GAM_EMPTY,
            ID_GAM_ADD,
            ID_GAM_STATUS,
            ID_GAM_FOOT,
        ]
        .into_iter()
        .chain((0..MAX_PROGS).flat_map(prog_ids))
        {
            put(id, 0, 0, 0, 0);
        }
    }

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
            // Semibold (primary style): wider than measured.
            let save_w = btn_w("Guardar en el ratón…").max(btn_w("Reintentar…")) + px(16.0);
            let discard_w = btn_w("Descartar");
            let text_w = if g.compact {
                pw
            } else {
                pw - save_w - discard_w - px(24.0)
            };
            let (_, th) = measure(ID_BAR_TEXT, Role::Strong, text_w);
            if g.compact {
                put(ID_BAR_TEXT, pl, cy, pw, th);
                cy += if th > 0 { th + px(8.0) } else { 0 };
                put(
                    ID_BAR_DISCARD,
                    pl - ring,
                    cy - ring,
                    discard_w + 2 * ring,
                    bh + 2 * ring,
                );
                let sx = pl + discard_w + px(12.0);
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
                put(ID_BAR_TEXT, pl, cy + (row_h - th) / 2, text_w, th);
                let sx = pl + pw - save_w;
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
                    pl + pw - discard_w
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
            put(ID_SAVE_STATUS, pl + px(24.0), cy, pw - px(24.0), sth);
            save_xy = Some((pl, cy));
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
            Kind::Button(..) | Kind::List => fonts.body,
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
    let status_icon_xy = status_xy;
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.cards = cards;
            m.notice = notice;
            m.field = field;
            m.icon = (pl, icon_y, if g.compact { 32.0 } else { 40.0 });
            m.battery = battery;
            m.separators = separators;
            m.badges = badges;
            m.boxes = boxes;
            m.leaders = leaders;
            m.art = art_at;
            m.picture = picture;
            m.save_icon = match (m.save_icon, save_xy) {
                (Some((_, _, ok)), Some((x, y))) => Some((x, y, ok)),
                _ => None,
            };
            m.hex_field = hex_field;
            m.rows = rows;
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
    let (rows, profile_icon) = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| (m.rows.clone(), m.profile_icon))
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
    let (badges, boxes, leaders, art) = with_app(|a| {
        a.view.mouse.as_ref().map(|m| {
            (
                m.badges.clone(),
                m.boxes.clone(),
                m.leaders.clone(),
                m.art.map(|a| (a, m.picture.clone())),
            )
        })
    })
    .flatten()
    .unwrap_or_default();
    for r in &badges {
        c.origin(r.left as f32, (r.top - scroll) as f32);
        let (w, h) = (u(r.right - r.left), u(r.bottom - r.top));
        c.stroke_round_rect(0.0, 0.0, w, h, 4.0, 1.0, palette.border_control, false);
    }
    for r in &boxes {
        c.origin(r.left as f32, (r.top - scroll) as f32);
        let (w, h) = (u(r.right - r.left), u(r.bottom - r.top));
        c.fill_round_rect(0.0, 0.0, w, h, 6.0, palette.bg_app);
        c.stroke_round_rect(0.0, 0.0, w, h, 6.0, 1.0, palette.border_control, false);
    }
    // The drawing of the mouse, then the leader lines of its points (1 px
    // `border.control`; decorative, the points carry the names).
    if let Some(((x, y, k, art), pic)) = art {
        backdrop(
            &mut c,
            (x as f32, (y - scroll) as f32, k),
            art,
            pic.as_deref(),
            scale,
            palette,
        );
    }
    for (x1, y1, x2, y2) in leaders {
        c.origin(x1.min(x2) as f32, (y1.min(y2) - scroll) as f32);
        let (w, h) = (u((x2 - x1).abs()).max(u(1)), u((y2 - y1).abs()).max(u(1)));
        c.fill_round_rect(0.0, 0.0, w, h, 0.0, palette.border_control);
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
    if dis.CtlType == ODT_LISTBOX {
        draw_command(dis, &palette, &fonts, devices_view::scale_of(view));
        return;
    }
    let Kind::Button(style, surface) = kind else {
        return;
    };
    let bg = match surface {
        Surface::App => palette.bg_app,
        Surface::Card => palette.surface_card,
    };
    let scale = devices_view::scale_of(view);
    let row_of = |first: i32| {
        in_group(id, first, MAX_BUTTONS).and_then(|i| {
            with_app(|a| {
                let m = a.view.mouse.as_ref()?;
                Some((m.button_rows.get(i).cloned()?, m.sel == i))
            })
            .flatten()
        })
    };
    if let Some((row, chosen)) = row_of(ID_DOT0) {
        // The part of the drawing behind the point, so the point shows only
        // its circle: the drawing in content px, less the point's position.
        let behind = with_app(|a| {
            let m = a.view.mouse.as_ref()?;
            let (x, y, k, art) = m.art?;
            let mut at = [windows::Win32::Foundation::POINT::default()];
            unsafe { MapWindowPoints(Some(dis.hwndItem), Some(m.content), &mut at) };
            Some((
                x - at[0].x,
                y - a.view.scroll - at[0].y,
                k,
                art,
                m.picture.clone(),
            ))
        })
        .flatten();
        draw_dot(dis, chosen, row.locked, behind, &palette, scale);
        return;
    }
    if let Some((row, chosen)) = row_of(ID_ROW0) {
        draw_button_row(dis, &row, chosen, &palette, &fonts, scale);
        return;
    }
    if let Some(i) = in_group(id, ID_AS_TAB0, AS_TABS.len() as i32) {
        let chosen =
            with_app(|a| a.view.mouse.as_ref().map(|m| m.as_tab == AS_TABS[i].0)).flatten();
        let label = AS_TABS[i].1.to_uppercase();
        draw_tab(
            dis,
            &label,
            chosen == Some(true),
            bg,
            &palette,
            &fonts,
            scale,
        );
        return;
    }
    if let Some(i) = in_group(id, ID_TAB0, TABS.len() as i32) {
        let chosen = with_app(|a| a.view.mouse.as_ref().map(|m| m.tab == TABS[i].0)).flatten();
        draw_tab(
            dis,
            TABS[i].1,
            chosen == Some(true),
            bg,
            &palette,
            &fonts,
            scale,
        );
        return;
    }
    if let Some(i) = in_group(id, ID_PILL0, MAX_ROWS) {
        let (name, active) =
            with_app(|a| a.view.mouse.as_ref().and_then(|m| m.pills.get(i).cloned()))
                .flatten()
                .unwrap_or_default();
        let style = if active {
            ButtonStyle::Selected
        } else {
            ButtonStyle::Normal
        };
        let shape = panel::Shape {
            radius: 18.0,
            dashed: false,
            check: active,
        };
        panel::draw_shaped(dis, &name, bg, &palette, &fonts, scale, style, shape);
        return;
    }
    if id == ID_PRF_NEW || id == ID_PRF_MENU {
        let (label, dashed) = if id == ID_PRF_NEW {
            ("+ Nuevo", true)
        } else {
            ("Perfil …", false)
        };
        let shape = panel::Shape {
            radius: 18.0,
            dashed,
            check: false,
        };
        panel::draw_shaped(dis, label, bg, &palette, &fonts, scale, style, shape);
        return;
    }
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
        m if in_group(m, ID_AS_MOUSE0, MOUSE_ACTIONS.len() as i32).is_some() => {
            text::action(Action::Mouse(MOUSE_ACTIONS[(m - ID_AS_MOUSE0) as usize]))
        }
        ID_AS_KEY_ASSIGN => "Asignar".into(),
        ID_AS_KEY_CLEAR => "Borrar".into(),
        e if in_group(e, ID_EFFECT0, N_EFFECTS).is_some() => {
            light::EFFECTS[(e - ID_EFFECT0) as usize].1.to_string()
        }
        v if in_group(v, ID_SPEED0, N_SPEEDS).is_some() => {
            light::SPEEDS[(v - ID_SPEED0) as usize].to_string()
        }
        ID_LIGHT_TRY => get_text(dis.hwndItem),
        d if in_group(d, ID_GAM_DEL0, MAX_PROGS).is_some() => "Desvincular".into(),
        r if in_group(r, ID_RATE0, MAX_RATES).is_some() => get_text(dis.hwndItem)
            .trim_end_matches(", elegida")
            .to_string(),
        ID_BACK => "‹  Dispositivos".to_string(),
        ID_MINUS => "−".into(),
        ID_PLUS => "+".into(),
        ID_APPLY => get_text(dis.hwndItem),
        p if (ID_PRESET0..ID_PRESET0 + PRESETS.len() as i32).contains(&p) => {
            PRESETS[(p - ID_PRESET0) as usize].to_string()
        }
        _ => get_text(dis.hwndItem),
    };
    panel::draw_button(dis, &label, bg, &palette, &fonts, scale, style);
}

/// A tab of the page: its label (semibold when chosen), the `border.card`
/// line under the tabs and, on the chosen one, a 2 px `text.primary`
/// underline; never chosen by colour alone.
fn draw_tab(
    dis: &DRAWITEMSTRUCT,
    label: &str,
    chosen: bool,
    bg: Color,
    palette: &Palette,
    fonts: &Fonts,
    scale: f32,
) {
    use windows::Win32::UI::Controls::{ODS_DISABLED, ODS_FOCUS, ODS_NOFOCUSRECT};
    let r = dis.rcItem;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let has = |f: windows::Win32::UI::Controls::ODS_FLAGS| dis.itemState.0 & f.0 != 0;
    unsafe {
        let hdc = dis.hDC;
        let mem = CreateCompatibleDC(Some(hdc));
        let bmp = CreateCompatibleBitmap(hdc, w, h);
        let old_bmp = SelectObject(mem, bmp.into());
        let all = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        panel::fill(mem, all, bg);
        if let Some(c) = Canvas::new(mem, scale) {
            let (uw, uh) = (w as f32 / scale, h as f32 / scale);
            c.fill_round_rect(0.0, uh - 1.0, uw, 1.0, 0.0, palette.border_card);
            if chosen {
                c.fill_round_rect(4.0, uh - 2.0, uw - 8.0, 2.0, 0.0, palette.text_primary);
            }
            if has(ODS_FOCUS) && !has(ODS_NOFOCUSRECT) {
                c.stroke_round_rect(
                    1.0,
                    1.0,
                    uw - 2.0,
                    uh - 4.0,
                    6.0,
                    2.0,
                    palette.focus_ring,
                    false,
                );
            }
        }
        SetBkMode(mem, TRANSPARENT);
        let color = if has(ODS_DISABLED) {
            palette.text_disabled
        } else if chosen {
            palette.text_primary
        } else {
            palette.text_secondary
        };
        SetTextColor(mem, COLORREF(color.colorref()));
        let font = if chosen {
            fonts.body_strong
        } else {
            fonts.body
        };
        let old_font = SelectObject(mem, font.into());
        let mut t: Vec<u16> = label.encode_utf16().collect();
        let mut tr = all;
        if !t.is_empty() {
            DrawTextW(
                mem,
                &mut t,
                &mut tr,
                DT_CENTER | DT_VCENTER | DT_SINGLELINE | DT_NOPREFIX,
            );
        }
        SelectObject(mem, old_font);
        let _ = BitBlt(hdc, r.left, r.top, w, h, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, old_bmp);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
    }
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
    let (_, app, card, _) = brushes?;
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
            }
        }
        Kind::List => (palette.text_primary, palette.surface_card, card),
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
        a.view
            .mouse
            .as_mut()
            .map(|m| m.as_key = Some((mods, usage)))
    });
    if let Some(view) = devices_view::hwnd() {
        refresh(view);
    }
}

// ---------------------------------------------------------------------------
// «Botones y DPI»: choosing a button and assigning it an action.

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

/// «Botones y DPI» stacks everything below 760 px (DESIGN.md).
fn is_stacked(view: HWND) -> bool {
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(view, &mut rc);
    }
    (rc.right as f32 / devices_view::scale_of(view)) < 760.0
}

/// The labels of the points in a wide window, the list in a narrow one.
/// How button `i` of `n` shows: a point on the drawing (the first
/// `drawn`), its label beside it (wide window), and a row in the list
/// (narrow window, or no point).
#[derive(Debug, PartialEq, Eq)]
struct ButtonView {
    point: bool,
    label: bool,
    row: bool,
}

fn button_view(i: usize, n: usize, drawn: usize, stacked: bool) -> ButtonView {
    let has = i < n && i < MAX_BUTTONS as usize;
    let point = has && i < drawn;
    ButtonView {
        point,
        label: point && !stacked,
        row: has && (stacked || !point),
    }
}

/// Points of the drawing (or photo) of the open mouse, as `layout` picks it.
fn drawn_points(view: HWND) -> usize {
    let stacked = is_stacked(view);
    with_app(|a| {
        let m = a.view.mouse.as_ref()?;
        let photo = m.photo.as_ref().filter(|_| !stacked);
        let vector = a
            .view
            .shared
            .devices
            .iter()
            .find(|d| d.info.key == m.key)
            .map(|d| super::mouse_art::art_for(&d.info.name))
            .unwrap_or(&super::mouse_art::GENERIC);
        Some(photo.map_or(vector, |(pa, _)| &pa.art).points.len())
    })
    .flatten()
    .unwrap_or(0)
}

/// `n` buttons, the first `drawn` with a point on the drawing.
fn show_button_views(view: HWND, n: usize, drawn: usize) {
    let stacked = is_stacked(view);
    for i in 0..MAX_BUTTONS {
        let v = button_view(i as usize, n, drawn, stacked);
        show(ID_DOT0 + i, v.point);
        show(ID_ROW0 + i, v.row);
        show(ID_LBL_NAME0 + i, v.label);
        show(ID_LBL_ACT0 + i, v.label);
        // One way with Tab: the list when narrow, else the points (and
        // the rows of the buttons without one).
        set_tabstop(ID_DOT0 + i, !stacked);
    }
    show(ID_BTN_HINT, n > 0);
}

/// Left, centred or right text of a label (`SS_LEFT`, `SS_CENTER`,
/// `SS_RIGHT`).
fn set_align(id: i32, align: u32) {
    if let Some(h) = ctl(id) {
        unsafe {
            let style = GetWindowLongW(h, GWL_STYLE) as u32;
            let new = (style & !0x3) | align;
            if new != style {
                SetWindowLongW(h, GWL_STYLE, new as i32);
                let _ = InvalidateRect(Some(h), None, true);
            }
        }
    }
}

/// Fills the «Comandos» list when the search changed; repaints it so the
/// button of each command follows the pending changes.
fn refresh_commands() {
    let query = ctl(ID_AS_SEARCH).map(get_text).unwrap_or_default();
    let stale = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| m.list_query.as_deref() != Some(query.as_str()))
    })
    .flatten();
    let list = ctl(ID_AS_LIST);
    if stale == Some(true) {
        let rows = command_rows(&query);
        if let Some(list) = list {
            unsafe {
                SendMessageW(list, LB_RESETCONTENT, None, None);
                for (i, r) in rows.iter().enumerate() {
                    // The strings are what screen readers read.
                    let t = match r {
                        Some(c) => {
                            let c = &super::commands::COMMANDS[*c];
                            format!("{}, {}", c.name, c.keys())
                        }
                        None => rows
                            .get(i + 1)
                            .copied()
                            .flatten()
                            .map(|c| format!("Grupo {}", super::commands::COMMANDS[c].group))
                            .unwrap_or_default(),
                    };
                    let w = wide(&t);
                    SendMessageW(list, LB_ADDSTRING, None, Some(LPARAM(w.as_ptr() as isize)));
                }
            }
        }
        let n = rows.iter().filter(|r| r.is_some()).count();
        set(
            ID_AS_COUNT,
            &if n == 0 {
                format!("Sin resultados para «{}»", query.trim())
            } else {
                plural(n, "comando", "comandos")
            },
        );
        with_app(|a| {
            a.view.mouse.as_mut().map(|m| {
                m.list_rows = rows;
                m.list_query = Some(query);
            })
        });
    }
    if let Some(list) = list {
        unsafe {
            let _ = InvalidateRect(Some(list), None, false);
        }
    }
}

/// Chooses the button `row` (a point or a row of the list).
fn choose_button(view: HWND, row: usize) {
    with_app(|a| {
        a.view.mouse.as_mut().map(|m| {
            m.sel = row;
            m.sel_set = true;
        })
    });
    // No row selected: a click on any command (the same as before
    // included) changes the selection, so it assigns.
    if let Some(list) = ctl(ID_AS_LIST) {
        unsafe {
            SendMessageW(list, LB_SETCURSEL, Some(WPARAM(usize::MAX)), None);
        }
    }
    refresh(view);
}

/// Assigns `action` to the chosen button as a pending change: nothing is
/// written until «Guardar en el ratón…». The focus goes back to its point.
fn assign(view: HWND, action: Action) {
    let buttons = device_buttons();
    let row = with_app(|a| {
        let m = a.view.mouse.as_mut()?;
        let row = m.sel;
        let &device = buttons.get(row)?;
        if locked(row, device) || !crate::protocol::onboard::assignable(action) {
            return None;
        }
        upsert(&mut m.edits, row, action, device);
        m.buttons_result = None;
        Some(row)
    })
    .flatten();
    refresh(view);
    let point = if is_stacked(view) { ID_ROW0 } else { ID_DOT0 };
    if let Some(h) = row.and_then(|r| ctl(point + r as i32)) {
        unsafe {
            let _ = SetFocus(Some(h));
        }
    }
}

/// The command of the selected row of the list, if it is not a group.
fn selected_command() -> Option<Action> {
    let list = ctl(ID_AS_LIST)?;
    let i = unsafe { SendMessageW(list, LB_GETCURSEL, None, None).0 };
    let row = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .and_then(|m| m.list_rows.get(i as usize).copied())
    })
    .flatten()
    .flatten()?;
    Some(super::commands::COMMANDS[row].action())
}

/// The mouse behind the points at (x px, y px, px per unit): its photo when
/// there is one, the line drawing otherwise.
fn backdrop(
    c: &mut Canvas,
    (x, y, k): (f32, f32, f32),
    art: &super::mouse_art::Art,
    picture: Option<&glyph::Picture>,
    scale: f32,
    palette: &Palette,
) {
    match picture {
        Some(p) => glyph::picture(c, x, y, p),
        None => glyph::mouse(c, x, y, k, scale, art, palette),
    }
}

/// Reads this mouse's cached photo (`photos`) when «Descargar fotos de los
/// ratones» is on, the theme is not high contrast and its points were
/// measured; again only when the cache changes. Without it: the drawing.
fn refresh_photo(shared: &crate::app::Shared, name: &str) {
    let high_contrast =
        with_app(|a| a.view.palette.theme == super::theme::Theme::HighContrast).unwrap_or(true);
    let wanted = crate::photos::photo_for(name)
        .filter(|_| shared.general.photos && !high_contrast)
        .and_then(|p| Some((p, super::mouse_art::photo_art(p.file)?)));
    let key = wanted.map(|(p, _)| (p.file, shared.photos));
    if with_app(|a| a.view.mouse.as_ref().map(|m| m.photo_key)).flatten() == Some(key) {
        return;
    }
    let photo = wanted.and_then(|(p, pa)| {
        let bytes = crate::photos::load(&crate::photos::dir()?, p)?;
        Some((pa, Rc::<[u8]>::from(bytes)))
    });
    with_app(|a| {
        if let Some(m) = a.view.mouse.as_mut() {
            m.photo_key = key;
            m.photo = photo;
            m.picture = None;
        }
    });
}

/// The mouse behind a point: (x, y) relative to it, px per unit, drawing
/// and photo.
type Behind = (
    i32,
    i32,
    f32,
    &'static super::mouse_art::Art,
    Option<Rc<glyph::Picture>>,
);

/// A point of the drawing: a 12 px circle (hollow when the button cannot
/// change), a 2 px ring when chosen, the focus ring outside it.
fn draw_dot(
    dis: &DRAWITEMSTRUCT,
    chosen: bool,
    locked: bool,
    behind: Option<Behind>,
    palette: &Palette,
    scale: f32,
) {
    use windows::Win32::UI::Controls::{ODS_FOCUS, ODS_NOFOCUSRECT};
    let r = dis.rcItem;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let has = |f: windows::Win32::UI::Controls::ODS_FLAGS| dis.itemState.0 & f.0 != 0;
    unsafe {
        let hdc = dis.hDC;
        let mem = CreateCompatibleDC(Some(hdc));
        let bmp = CreateCompatibleBitmap(hdc, w, h);
        let old = SelectObject(mem, bmp.into());
        let all = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        panel::fill(mem, all, palette.bg_app);
        let mut on_photo = false;
        if let Some(mut c) = Canvas::new(mem, scale) {
            if let Some((x, y, k, art, pic)) = behind {
                on_photo = pic.is_some();
                backdrop(
                    &mut c,
                    (x as f32, y as f32, k),
                    art,
                    pic.as_deref(),
                    scale,
                    palette,
                );
                c.origin(0.0, 0.0);
            }
            // On a photo (a dark mouse on the page background) the point
            // keeps the dark theme's light circle, and its rings get a dark
            // halo so they show on the mouse and on a light background.
            let palette = if on_photo {
                &super::theme::DARK
            } else {
                palette
            };
            let (uw, uh) = (w as f32 / scale, h as f32 / scale);
            let (cx, cy) = (uw / 2.0, uh / 2.0);
            let circle = |r: f32| (cx - r, cy - r, 2.0 * r);
            let ring = |r: f32, width: f32, color: Color, dashed: bool| {
                let (x, y, d) = circle(r);
                if on_photo {
                    let (hx, hy, hd) = circle(r + 1.0);
                    c.stroke_round_rect(
                        hx,
                        hy,
                        hd,
                        hd,
                        r + 1.0,
                        width + 2.0,
                        palette.bg_app,
                        false,
                    );
                }
                c.stroke_round_rect(x, y, d, d, r, width, color, dashed);
            };
            if has(ODS_FOCUS) && !has(ODS_NOFOCUSRECT) {
                ring(13.0, 1.5, palette.focus_ring, true);
            }
            if chosen {
                ring(10.5, 2.0, palette.text_primary, false);
            }
            let (x, y, d) = circle(6.0);
            if locked {
                c.fill_round_rect(x, y, d, d, 6.0, palette.surface_card);
                c.stroke_round_rect(x, y, d, d, 6.0, 2.0, palette.text_secondary, false);
            } else {
                c.fill_round_rect(x, y, d, d, 6.0, palette.text_primary);
                c.stroke_round_rect(x, y, d, d, 6.0, 2.0, palette.bg_app, false);
            }
        }
        let _ = BitBlt(hdc, r.left, r.top, w, h, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
    }
}

/// Draws `text` in `rect` (single line, ellipsis), never with an empty
/// slice (see `panel::draw_shaped`).
unsafe fn text_at(
    hdc: HDC,
    text: &str,
    mut rect: RECT,
    font: HFONT,
    color: Color,
    flags: DRAW_TEXT_FORMAT,
) {
    let mut t: Vec<u16> = text.encode_utf16().collect();
    if t.is_empty() {
        return;
    }
    unsafe {
        let old = SelectObject(hdc, font.into());
        SetTextColor(hdc, COLORREF(color.colorref()));
        DrawTextW(
            hdc,
            &mut t,
            &mut rect,
            flags | DT_SINGLELINE | DT_NOPREFIX | DT_END_ELLIPSIS,
        );
        SelectObject(hdc, old);
    }
}

/// A row of the list of buttons (narrow window): the name at the left, the
/// action at the right and, when changed, what it replaces below; the
/// chosen row on `surface.selected` with a `text.primary` border.
fn draw_button_row(
    dis: &DRAWITEMSTRUCT,
    row: &ButtonRow,
    chosen: bool,
    palette: &Palette,
    fonts: &Fonts,
    scale: f32,
) {
    use windows::Win32::UI::Controls::{ODS_FOCUS, ODS_NOFOCUSRECT};
    let r = dis.rcItem;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let has = |f: windows::Win32::UI::Controls::ODS_FLAGS| dis.itemState.0 & f.0 != 0;
    let px = |v: f32| (v * scale) as i32;
    unsafe {
        let hdc = dis.hDC;
        let mem = CreateCompatibleDC(Some(hdc));
        let bmp = CreateCompatibleBitmap(hdc, w, h);
        let old = SelectObject(mem, bmp.into());
        let all = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        panel::fill(mem, all, palette.bg_app);
        if let Some(c) = Canvas::new(mem, scale) {
            let (uw, uh) = (w as f32 / scale, h as f32 / scale);
            if chosen {
                c.fill_round_rect(4.0, 4.0, uw - 8.0, uh - 8.0, 8.0, palette.surface_selected);
                c.stroke_round_rect(
                    4.0,
                    4.0,
                    uw - 8.0,
                    uh - 8.0,
                    8.0,
                    1.0,
                    palette.text_primary,
                    false,
                );
            } else {
                c.fill_round_rect(4.0, uh - 5.0, uw - 8.0, 1.0, 0.0, palette.border_card);
            }
            if has(ODS_FOCUS) && !has(ODS_NOFOCUSRECT) {
                c.stroke_round_rect(
                    1.0,
                    1.0,
                    uw - 2.0,
                    uh - 2.0,
                    9.0,
                    2.0,
                    palette.focus_ring,
                    false,
                );
            }
        }
        SetBkMode(mem, TRANSPARENT);
        let line = RECT {
            left: px(16.0),
            top: px(4.0),
            right: w - px(16.0),
            bottom: px(4.0) + px(36.0),
        };
        text_at(
            mem,
            &row.name,
            line,
            fonts.body,
            palette.text_secondary,
            DT_LEFT | DT_VCENTER,
        );
        let (font, color) = if row.locked {
            (fonts.body, palette.text_secondary)
        } else {
            (fonts.body_strong, palette.text_primary)
        };
        let action = if row.locked {
            format!("{} · no se cambia", row.action)
        } else {
            row.action.clone()
        };
        text_at(mem, &action, line, font, color, DT_RIGHT | DT_VCENTER);
        if let Some(b) = &row.before {
            let below = RECT {
                top: line.bottom - px(4.0),
                bottom: h - px(4.0),
                ..line
            };
            text_at(
                mem,
                &format!("Cambiado · antes: {b}"),
                below,
                fonts.body,
                palette.text_notice,
                DT_LEFT | DT_VCENTER,
            );
        }
        let _ = BitBlt(hdc, r.left, r.top, w, h, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
    }
}

/// Short name of a button for the tags of the «Comandos» list
/// ("Lateral trasero" -> "Trasero"), like «G4» in G HUB.
fn short_button(name: &str) -> &str {
    match name {
        "Lateral trasero" => "Trasero",
        "Lateral delantero" => "Delantero",
        "Central (rueda)" => "Central",
        "Botón DPI" => "DPI",
        other => other,
    }
}

/// A row of the «Comandos» list: a group header, or the keys (semibold),
/// the name and, at the right, the buttons that have it (like «G4» in
/// G HUB). The selected row on `surface.selected` with a `text.primary`
/// border.
fn draw_command(dis: &DRAWITEMSTRUCT, palette: &Palette, fonts: &Fonts, scale: f32) {
    use windows::Win32::UI::Controls::{ODS_FOCUS, ODS_SELECTED};
    if dis.itemID == u32::MAX {
        return;
    }
    let i = dis.itemID as usize;
    let Some((rows, buttons)) = with_app(|a| {
        a.view
            .mouse
            .as_ref()
            .map(|m| (m.list_rows.clone(), m.button_rows.clone()))
    })
    .flatten() else {
        return;
    };
    let Some(&row) = rows.get(i) else {
        return;
    };
    let r = dis.rcItem;
    let (w, h) = (r.right - r.left, r.bottom - r.top);
    let has = |f: windows::Win32::UI::Controls::ODS_FLAGS| dis.itemState.0 & f.0 != 0;
    let px = |v: f32| (v * scale) as i32;
    unsafe {
        let hdc = dis.hDC;
        let mem = CreateCompatibleDC(Some(hdc));
        let bmp = CreateCompatibleBitmap(hdc, w.max(1), h.max(1));
        let old = SelectObject(mem, bmp.into());
        let all = RECT {
            left: 0,
            top: 0,
            right: w,
            bottom: h,
        };
        panel::fill(mem, all, palette.surface_card);
        SetBkMode(mem, TRANSPARENT);
        match row {
            None => {
                let group = rows
                    .get(i + 1)
                    .copied()
                    .flatten()
                    .map(|c| super::commands::COMMANDS[c].group.to_uppercase())
                    .unwrap_or_default();
                let rect = RECT {
                    left: px(4.0),
                    top: px(8.0),
                    ..all
                };
                text_at(
                    mem,
                    &group,
                    rect,
                    fonts.small,
                    palette.text_tertiary,
                    DT_LEFT | DT_VCENTER,
                );
            }
            Some(ci) => {
                let c = &super::commands::COMMANDS[ci];
                if let Some(cv) = Canvas::new(mem, scale) {
                    let (uw, uh) = (w as f32 / scale, h as f32 / scale);
                    if has(ODS_SELECTED) {
                        cv.fill_round_rect(
                            1.0,
                            1.0,
                            uw - 2.0,
                            uh - 2.0,
                            6.0,
                            palette.surface_selected,
                        );
                        cv.stroke_round_rect(
                            1.0,
                            1.0,
                            uw - 2.0,
                            uh - 2.0,
                            6.0,
                            1.0,
                            palette.text_primary,
                            false,
                        );
                    } else {
                        cv.fill_round_rect(0.0, uh - 1.0, uw, 1.0, 0.0, palette.border_card);
                    }
                    if has(ODS_FOCUS) {
                        cv.stroke_round_rect(
                            2.0,
                            2.0,
                            uw - 4.0,
                            uh - 4.0,
                            5.0,
                            1.5,
                            palette.focus_ring,
                            true,
                        );
                    }
                }
                let keys = RECT {
                    left: px(8.0),
                    right: px(114.0),
                    ..all
                };
                // Arrows as symbols so the keys fit their column (the
                // accessible name keeps the words).
                let shown = c
                    .keys()
                    .replace("Flecha derecha", "→")
                    .replace("Flecha izquierda", "←");
                text_at(
                    mem,
                    &shown,
                    keys,
                    fonts.body_strong,
                    palette.text_primary,
                    DT_LEFT | DT_VCENTER,
                );
                // The buttons that send it now (pending changes included).
                let label = format!("Atajo {}", c.keys());
                let tag: Vec<&str> = buttons
                    .iter()
                    .filter(|b| b.action == label)
                    .map(|b| short_button(&b.name))
                    .collect();
                let tag = tag.join(", ");
                let tag_w = if tag.is_empty() { 0 } else { px(76.0) };
                let name = RECT {
                    left: px(118.0),
                    right: w - tag_w - px(8.0),
                    ..all
                };
                text_at(
                    mem,
                    c.name,
                    name,
                    fonts.body,
                    palette.text_secondary,
                    DT_LEFT | DT_VCENTER,
                );
                let tag_rect = RECT {
                    left: w - tag_w - px(6.0),
                    right: w - px(8.0),
                    ..all
                };
                text_at(
                    mem,
                    &tag,
                    tag_rect,
                    fonts.small,
                    palette.text_primary,
                    DT_RIGHT | DT_VCENTER,
                );
            }
        }
        let _ = BitBlt(hdc, r.left, r.top, w, h, Some(mem), 0, 0, SRCCOPY);
        SelectObject(mem, old);
        let _ = DeleteObject(bmp.into());
        let _ = DeleteDC(mem);
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
    if let Some(ms) = edits.rate {
        lines.push(format!(
            "Tasa de sondeo: {} Hz (no se escribe en la memoria del ratón)",
            text::hz(ms)
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

/// «Activar la memoria del ratón»: the mouse goes back to what it has saved.
fn confirm_activate(view: HWND) {
    super::confirm::open(
        view,
        "¿Activar la memoria del ratón?",
        "El ratón volverá a usar los botones y el DPI guardados en su memoria (los que le dejó G HUB u otro programa). Después podrás verlos y cambiarlos aquí.

No se escribe nada en su memoria. Si otro programa la vuelve a desactivar, tendrás que repetirlo.",
        "Activar",
        Box::new(move || {
            send_write(view, Write::Activate, |key| {
                Command::ActivateOnboard(key, Confirmed::by_dialog())
            })
        }),
    );
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
    // Saved in the app, never in the mouse (`Snapshot::saves_in_app`).
    let auto = super::snapshot().is_some_and(|shared| {
        with_app(|a| {
            let m = a.view.mouse.as_ref()?;
            snapshot(&shared, &m.key).map(|s| s.saves_in_app())
        })
        .flatten()
        .unwrap_or(false)
    });
    let changes = plural(n, "cambio", "cambios");
    let verified = super::snapshot().is_some_and(|shared| {
        with_app(|a| {
            let m = a.view.mouse.as_ref()?;
            snapshot(&shared, &m.key).map(|s| s.info.support == crate::devices::Support::Verified)
        })
        .flatten()
        .unwrap_or(false)
    });
    let (title, note, action) = if only_rate(&edits) {
        (
            format!("¿Guardar {changes}?"),
            if verified {
                "Se aplica ahora y OpenPeripheral la vuelve a aplicar al conectar el ratón. No se escribe en su memoria."
            } else {
                "Se aplica ahora y se pierde al apagar el ratón. No se escribe en su memoria."
            },
            "Guardar",
        )
    } else if auto {
        (
            format!("¿Guardar {changes}?"),
            "OpenPeripheral los aplicará cada vez que conectes el ratón. No se escriben en su memoria.",
            "Guardar",
        )
    } else {
        (
            format!("¿Guardar {changes} en la memoria del ratón?"),
            "El ratón los conservará aunque cierres OpenPeripheral o lo uses en otro PC. Si la comprobación de la escritura falla, OpenPeripheral deja el perfil como estaba.",
            "Guardar en el ratón",
        )
    };
    let body = format!("{}\n\n{note}", lines.join("\n"));
    super::confirm::open(
        view,
        &title,
        &body,
        action,
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
        m.rate = None;
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

/// «Perfil …»: a native menu with the actions on the active profile, the
/// restores and deleting the other profiles (DESIGN.md, revisión
/// 2026-10-08).
fn profile_menu(view: HWND) {
    const RENAME: usize = 1;
    const SAVE_AS: usize = 2;
    const REAPPLY: usize = 3;
    const ORIGINAL: usize = 4;
    const FACTORY: usize = 5;
    const ORIGINALS: usize = 6;
    const DELETE0: usize = 100;
    let Some((key, list, Some(active))) = profile_list() else {
        return;
    };
    let Some(shared) = super::snapshot() else {
        return;
    };
    let snap = snapshot(&shared, &key);
    let connected = snap.is_some_and(|s| matches!(s.status, Status::Ok(_)));
    let busy =
        with_app(|a| a.view.mouse.as_ref().is_some_and(|m| m.pending.is_some())) != Some(false);
    let ghub = shared.conflicts.contains(&"Logitech G HUB");
    let editable = editing_available()
        && snap
            .and_then(profile_view)
            .is_some_and(|v| !v.buttons.is_empty())
        && matches!(snap.and_then(|s| s.backup.clone()), Some(Ok(_)));
    let can_write = connected && !busy && !ghub;
    let row = list.iter().position(|p| p.name == active);
    let chosen = unsafe {
        let Ok(menu) = CreatePopupMenu() else {
            return;
        };
        let item = |id: usize, text: &str, on: bool| {
            let t = wide(&text.replace('&', "&&"));
            let flags = if on { MF_STRING } else { MF_STRING | MF_GRAYED };
            let _ = AppendMenuW(menu, flags, id, PCWSTR(t.as_ptr()));
        };
        item(RENAME, &format!("Renombrar «{active}»…"), !busy);
        item(
            SAVE_AS,
            "Guardar como perfil nuevo…",
            !busy && list.len() < crate::profiles::MAX_PROFILES,
        );
        item(REAPPLY, &format!("Volver a aplicar «{active}»"), can_write);
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        item(
            ORIGINAL,
            "Restaurar perfil original del ratón…",
            editable && can_write,
        );
        item(FACTORY, "Valores de fábrica…", editable && can_write);
        item(
            ORIGINALS,
            "Restaurar originales (deja de aplicar el DPI y la luz de la app)",
            connected && !busy,
        );
        if list.len() > 1 {
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
            item(0, "Eliminar un perfil (el activo no se puede)", false);
            for (i, p) in list.iter().enumerate().filter(|(_, p)| p.name != active) {
                item(DELETE0 + i, &format!("Eliminar «{}»…", p.name), !busy);
            }
        }
        // Below the button, from its left edge.
        let mut r = RECT::default();
        if let Some(b) = ctl(ID_PRF_MENU) {
            let _ = GetWindowRect(b, &mut r);
        }
        let chosen = TrackPopupMenu(
            menu,
            TPM_LEFTALIGN | TPM_TOPALIGN | TPM_RETURNCMD | TPM_NONOTIFY,
            r.left,
            r.bottom,
            None,
            view,
            None,
        )
        .0 as usize;
        let _ = DestroyMenu(menu);
        chosen
    };
    match chosen {
        RENAME => {
            if let Some(i) = row {
                rename_profile(view, i);
            }
        }
        SAVE_AS => new_profile(view, true),
        // With changes pending, the same question as using another profile.
        REAPPLY => {
            if let Some(i) = row {
                request_use(view, i);
            }
        }
        ORIGINAL => confirm_restore(view, Source::Original),
        FACTORY => confirm_restore(view, Source::Factory),
        ORIGINALS => restore_originals(view),
        d if d >= DELETE0 => confirm_delete(view, d - DELETE0),
        _ => {}
    }
}

/// «Restaurar originales»: the app stops applying its DPI and light when
/// the mouse connects; nothing is written to the mouse.
fn restore_originals(view: HWND) {
    let Some(shared) = super::snapshot() else {
        return;
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

// ---------------------------------------------------------------------------
// «Avanzado»: programs linked to the active profile.

fn program(i: usize) -> Option<String> {
    let shared = super::snapshot()?;
    let key = with_app(|a| a.view.mouse.as_ref().map(|m| m.key.clone())).flatten()?;
    let s = snapshot(&shared, &key)?;
    linked_programs(&s.programs, s.active_profile.as_deref())
        .get(i)
        .cloned()
}

/// «Vincular programa…»: links it to the active profile. A program has a
/// single profile: if another one had it, it moves here, and the dialog
/// says so first.
fn link_program(view: HWND, exe: String) {
    let Some((key, _, Some(active))) = profile_list() else {
        return;
    };
    let path = crate::games::normalize(&exe).unwrap_or_else(|| exe.clone());
    let current = super::snapshot().and_then(|s| {
        snapshot(&s, &key)?
            .programs
            .iter()
            .find(|p| p.exe == path)
            .map(|p| p.profile.clone())
    });
    let name = crate::games::display_name(&path);
    match current {
        Some(Some(p)) if p == active => {
            let text = format!("{name} ya está vinculado a «{active}».");
            with_app(|a| {
                a.view
                    .mouse
                    .as_mut()
                    .map(|m| m.games_status = Some((true, text)))
            });
            refresh(view);
        }
        Some(Some(other)) => super::confirm::open(
            view,
            &format!("¿Vincular {name} a «{active}»?"),
            &format!(
                "Ahora usa el perfil «{other}». Un programa solo puede tener un perfil: pasará a «{active}».\n\n{path}"
            ),
            "Vincular",
            Box::new(move || send_games(view, Command::SetProgram(key, exe, Some(active)))),
        ),
        _ => send_games(view, Command::SetProgram(key, exe, Some(active))),
    }
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

fn dpi_range(shared: &Shared) -> Option<(Option<u16>, DpiRange)> {
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
        // Esc leaves the page (asking first with changes pending).
        (2, _) => request_leave(view),
        // Enter: "Probar" of the DPI or the light when its field has the focus.
        (1, _) => {
            let f = Some(unsafe { GetFocus() });
            if f == ctl(ID_DPI_EDIT) {
                apply(view);
            } else if f == ctl(ID_AS_LIST) {
                if let Some(a) = selected_command() {
                    assign(view, a);
                }
            } else if f == ctl(ID_AS_KEY) {
                if let Some((modifiers, usage)) =
                    with_app(|a| a.view.mouse.as_ref().and_then(|m| m.as_key)).flatten()
                {
                    assign(view, Action::Keyboard { modifiers, usage });
                }
            } else if f == ctl(ID_HEX_EDIT) {
                try_light(view);
            } else {
                let f = unsafe { GetFocus() };
                if kind_of(f).is_some_and(|(_, k)| matches!(k, Kind::Button(..))) {
                    unsafe { SendMessageW(f, BM_CLICK, None, None) };
                }
            }
        }
        (ID_BACK, BN_CLICKED) => request_leave(view),
        (d, BN_CLICKED) if in_group(d, ID_DOT0, MAX_BUTTONS).is_some() => {
            choose_button(view, (d - ID_DOT0) as usize)
        }
        (r, BN_CLICKED) if in_group(r, ID_ROW0, MAX_BUTTONS).is_some() => {
            choose_button(view, (r - ID_ROW0) as usize)
        }
        // Tabs of «Asignaciones»: the arrows also choose (nothing is written).
        (t, BN_CLICKED | BN_SETFOCUS)
            if in_group(t, ID_AS_TAB0, AS_TABS.len() as i32).is_some() =>
        {
            let tab = AS_TABS[(t - ID_AS_TAB0) as usize].0;
            let changed = with_app(|a| {
                a.view.mouse.as_mut().map(|m| {
                    let changed = m.as_tab != tab;
                    m.as_tab = tab;
                    changed
                })
            })
            .flatten();
            if changed == Some(true) {
                refresh(view);
            }
            devices_view::ensure_visible(view, HWND(lparam.0 as *mut _));
        }
        (ID_AS_SEARCH, EN_CHANGE) => refresh_commands(),
        // A click on a command assigns it; the arrows only select (Enter or
        // a double click assigns).
        (ID_AS_LIST, LBN_SELCHANGE) => {
            if unsafe { GetKeyState(VK_LBUTTON.0 as i32) } < 0 {
                if let Some(a) = selected_command() {
                    assign(view, a);
                }
            }
        }
        (ID_AS_LIST, LBN_DBLCLK) => {
            if let Some(a) = selected_command() {
                assign(view, a);
            }
        }
        (ID_AS_LIST, LBN_SETFOCUS) => devices_view::ensure_visible(view, HWND(lparam.0 as *mut _)),
        (ID_AS_KEY_ASSIGN, BN_CLICKED) => {
            if let Some((modifiers, usage)) =
                with_app(|a| a.view.mouse.as_ref().and_then(|m| m.as_key)).flatten()
            {
                assign(view, Action::Keyboard { modifiers, usage });
            }
        }
        (ID_AS_KEY_CLEAR, BN_CLICKED) => {
            with_app(|a| a.view.mouse.as_mut().map(|m| m.as_key = None));
            refresh(view);
            if let Some(h) = ctl(ID_AS_KEY) {
                unsafe {
                    let _ = SetFocus(Some(h));
                }
            }
        }
        (m, BN_CLICKED) if in_group(m, ID_AS_MOUSE0, MOUSE_ACTIONS.len() as i32).is_some() => {
            assign(
                view,
                Action::Mouse(MOUSE_ACTIONS[(m - ID_AS_MOUSE0) as usize]),
            )
        }
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
        (p, BN_CLICKED) if in_group(p, ID_PILL0, MAX_ROWS).is_some() => {
            // The active pill does nothing: it is already in the mouse.
            let i = (p - ID_PILL0) as usize;
            let active = with_app(|a| {
                a.view
                    .mouse
                    .as_ref()
                    .and_then(|m| m.pills.get(i).map(|x| x.1))
            })
            .flatten();
            if active == Some(false) {
                request_use(view, i);
            }
        }
        (ID_PRF_NEW, BN_CLICKED) => new_profile(view, false),
        (ID_PRF_MENU, BN_CLICKED) => profile_menu(view),
        // Tabs: focus moved by the arrows also chooses (nothing is written).
        (t, BN_CLICKED | BN_SETFOCUS) if in_group(t, ID_TAB0, TABS.len() as i32).is_some() => {
            select_tab(view, TABS[(t - ID_TAB0) as usize].0);
            devices_view::ensure_visible(view, HWND(lparam.0 as *mut _));
        }
        (d, BN_CLICKED) if in_group(d, ID_GAM_DEL0, MAX_PROGS).is_some() => {
            if let (Some((key, ..)), Some(exe)) =
                (profile_list(), program((d - ID_GAM_DEL0) as usize))
            {
                send_games(view, Command::RemoveProgram(key, exe));
            }
        }
        (r, BN_CLICKED) if in_group(r, ID_RATE0, MAX_RATES).is_some() => {
            // Only marks the rate as pending; «Guardar» sends it.
            if let Some(shared) = super::snapshot() {
                with_app(|a| {
                    let m = a.view.mouse.as_mut()?;
                    let rate = snapshot(&shared, &m.key).and_then(rate_of)?;
                    let ms = *rate_list(rate).get((r - ID_RATE0) as usize)?;
                    m.rate = (ms != rate.current_ms).then_some(ms);
                    m.buttons_result = None;
                    Some(())
                });
            }
            refresh(view);
        }
        (ID_GAM_ADD, BN_CLICKED) => {
            if let Some(exe) = pick_exe(view) {
                link_program(view, exe);
            }
        }
        (ID_PRF_SAVEAS, BN_CLICKED) => new_profile(view, true),
        (ID_BTN_ACTIVATE, BN_CLICKED) => confirm_activate(view),
        (ID_PRF_REAPPLY, BN_CLICKED) => {
            if let Some((_, list, Some(active))) = profile_list() {
                if let Some(i) = list.iter().position(|p| p.name == active) {
                    request_use(view, i);
                }
            }
        }
        (ID_APPLY, BN_CLICKED) => apply(view),
        (ID_MINUS | ID_PLUS, BN_CLICKED) => {
            if let Some(shared) = super::snapshot() {
                if let Some((cur, range)) = dpi_range(&shared) {
                    let typed = ctl(ID_DPI_EDIT).map(get_text).unwrap_or_default();
                    let from = parse_field(&typed)
                        .or(cur)
                        .unwrap_or_else(|| bounds(&range).0);
                    let v = step(&range, from, if id == ID_PLUS { 1 } else { -1 });
                    set(ID_DPI_EDIT, &v.to_string());
                }
            }
        }
        (p, BN_CLICKED) if (ID_PRESET0..ID_PRESET0 + PRESETS.len() as i32).contains(&p) => {
            set(ID_DPI_EDIT, &PRESETS[(p - ID_PRESET0) as usize].to_string());
        }
        (ID_DPI_EDIT, EN_CHANGE) => {
            with_app(|a| {
                if let Some(m) = a.view.mouse.as_mut() {
                    m.invalid = false;
                }
            });
            refresh(view);
        }
        (_, BN_SETFOCUS) | (_, EN_SETFOCUS) => {
            devices_view::ensure_visible(view, HWND(lparam.0 as *mut _))
        }
        _ => return false,
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every button of a mouse with up to 16 is on the page, as a point
    /// or as a row (plan `banco-ratones-virtuales`, finding 1).
    #[test]
    fn every_button_up_to_sixteen_is_on_the_page() {
        use crate::devices::sim;
        for n in 1..=20usize {
            let buttons: Vec<Action> = (0..n)
                .map(|i| Action::decode(sim::factory_button(i)))
                .collect();
            assert_eq!(button_rows(&buttons, &[]).len(), n);
            for drawn in [6, 8] {
                for stacked in [false, true] {
                    let reachable = (0..MAX_BUTTONS as usize)
                        .filter(|&i| {
                            let v = button_view(i, n, drawn, stacked);
                            v.point || v.row
                        })
                        .count();
                    assert_eq!(reachable, n.min(16), "{n} buttons, {drawn} points");
                    for i in 0..n.min(16) {
                        let v = button_view(i, n, drawn, stacked);
                        assert!(!(v.label && v.row), "{n}/{i}: label and row");
                        assert_eq!(v.label, i < drawn && !stacked);
                    }
                }
            }
        }
    }

    #[test]
    fn advanced_lists_only_the_programs_of_the_active_profile() {
        use crate::games::Program;
        let p = |exe: &str, profile: Option<&str>| Program {
            exe: exe.into(),
            profile: profile.map(str::to_string),
        };
        let programs = [
            p(r"c:\juegos\a.exe", Some("Juego")),
            p(r"c:\juegos\b.exe", None),
            p(r"c:\c.exe", Some("Trabajo")),
            p(r"c:\d.exe", Some("Juego")),
        ];
        assert_eq!(
            linked_programs(&programs, Some("Juego")),
            vec![r"c:\juegos\a.exe".to_string(), r"c:\d.exe".to_string()]
        );
        // «No preguntar más» (no profile) is never linked; no active: none.
        assert!(linked_programs(&programs, None).is_empty());
        assert!(linked_programs(&programs, Some("Otro")).is_empty());
    }

    const G305: DpiRange = DpiRange::Range {
        min: 200,
        max: 12000,
        step: 50,
    };

    #[test]
    fn rows_of_the_buttons_show_pending_changes_and_locks() {
        let dpi = Action::Raw([0x90, 0x0c, 0, 0]);
        let buttons = [
            Action::Mouse(0x01),
            Action::Mouse(0x02),
            Action::Mouse(0x04),
            Action::Mouse(0x08),
            Action::Mouse(0x10),
            dpi,
        ];
        let ctrl_c = Action::Keyboard {
            modifiers: 1,
            usage: 0x06,
        };
        let rows = button_rows(&buttons, &[(3, ctrl_c)]);
        assert_eq!(rows.len(), 6);
        assert_eq!(rows[3].name, "Lateral trasero");
        assert_eq!(rows[3].action, "Atajo Ctrl + C");
        assert_eq!(rows[3].before.as_deref(), Some("Atrás"));
        assert!(rows[4].before.is_none());
        // The left button and the DPI cycle cannot change; a click can.
        assert!(rows[0].locked && rows[5].locked);
        assert!(!rows[1].locked && !rows[3].locked);
        assert!(locked(0, Action::Mouse(0x02)));
        assert!(!locked(5, ctrl_c));
    }

    #[test]
    fn command_list_has_a_header_per_group() {
        let all = command_rows("");
        let headers = all.iter().filter(|r| r.is_none()).count();
        assert_eq!(headers, 5);
        assert_eq!(all[0], None);
        assert_eq!(all.len(), super::super::commands::COMMANDS.len() + headers);
        // A search keeps only the groups with results.
        let pegar = command_rows("pegar");
        assert_eq!(pegar.len(), 2);
        assert_eq!(pegar[0], None);
        assert!(command_rows("xyzzy").is_empty());
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
        // Nothing with the Windows key.
        assert_eq!(capture(0x0d, false, false, false, true), None);
        assert_eq!(capture(0x44, true, false, false, true), None);
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
            rate: None,
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
    fn the_rate_alone_is_not_a_write_to_the_mouse() {
        let rate = ProfileEdits {
            rate: Some(2),
            ..Default::default()
        };
        assert!(only_rate(&rate));
        assert_eq!(count(&rate), 1);
        assert_eq!(
            change_lines(&rate, None, None, &[]),
            vec!["Tasa de sondeo: 500 Hz (no se escribe en la memoria del ratón)".to_string()]
        );
        let both = ProfileEdits {
            dpi: Some(800),
            ..rate
        };
        assert!(!only_rate(&both));
        assert_eq!(count(&both), 2);
        // Lowest rate first; never more buttons than the page has.
        let r = crate::devices::ReportRate {
            current_ms: 1,
            supported_ms: vec![1, 2, 4, 8],
            writable: true,
        };
        assert_eq!(rate_list(&r), vec![8, 4, 2, 1]);
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
            range_sentence(&G305, false),
            "DPI con el que arranca el ratón. Admite de 200 a 12000, en pasos de 50. Al guardarlo se escribe en los 4 niveles del botón DPI."
        );
    }
}
