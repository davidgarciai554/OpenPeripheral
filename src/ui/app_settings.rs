//! «Ajustes» page (`docs/design/DESIGN.md`, «Pantalla «Ajustes»»): settings
//! of the whole app, not of a device or a profile. Shown in place of the
//! device grid, like «Ajustes del ratón». Its controls are children of the
//! view, so they scroll with it; the view paints the cards behind them.

use super::devices_view::{self, Geometry};
use super::panel::{self, ButtonStyle, Ctx};
use super::theme::{self, Color, Palette};
use super::{send, wide, with_app};
use crate::app::{Command, Shared};
use crate::config::{General, ThemeChoice};
use crate::devices::DeviceKey;
use crate::platform::system;
use windows::Win32::Foundation::{COLORREF, HWND, LPARAM, LRESULT, RECT, WPARAM};
use windows::Win32::Graphics::Gdi::*;
use windows::Win32::System::SystemServices::{SS_LEFT, SS_NOPREFIX};
use windows::Win32::UI::Controls::{DRAWITEMSTRUCT, SetWindowTheme};
use windows::Win32::UI::Input::KeyboardAndMouse::{GetFocus, IsWindowEnabled, SetFocus};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

// Control ids (creation order is tab order). Kept apart from the device
// grid and the mouse page.
const ID_BACK: i32 = 1500;
const ID_TITLE: i32 = 1501;
const ID_INTRO: i32 = 1502;
const ID_GEN_TITLE: i32 = 1510;
const ID_STARTUP: i32 = 1511;
const ID_STARTUP_HELP: i32 = 1512;
const ID_PHOTOS: i32 = 1513;
const ID_PHOTOS_HELP: i32 = 1514;
const ID_SPEED: i32 = 1515;
const ID_SPEED_SLOWER: i32 = 1516;
const ID_SPEED_FASTER: i32 = 1517;
const ID_SPEED_RESET: i32 = 1518;
const ID_SPEED_HELP: i32 = 1519;
const ID_LOOK_TITLE: i32 = 1520;
const ID_THEME_LABEL: i32 = 1521;
const ID_THEME0: i32 = 1522; // ..=1524
const ID_THEME_HELP0: i32 = 1525; // ..=1527
const ID_GAMES_TITLE: i32 = 1530;
const ID_AUTO: i32 = 1531;
const ID_AUTO_HELP: i32 = 1532;
const ID_ASK: i32 = 1533;
const ID_ASK_HELP: i32 = 1534;
const ID_NEW_TITLE: i32 = 1535;
const ID_NEW_EMPTY: i32 = 1536;
const ID_IGN_TITLE: i32 = 1537;
const ID_IGN_HELP: i32 = 1538;
const ID_IGN_EMPTY: i32 = 1539;
const ID_GAMES_NOTE: i32 = 1540;
const ID_NOTE_TITLE: i32 = 1550;
const ID_NOTIFY: i32 = 1551;
const ID_NOTIFY_HELP: i32 = 1552;
const ID_CONFLICT: i32 = 1553;
const ID_CONFLICT_HELP: i32 = 1554;
/// «Quitar avisos de Windows»: removes the app's notification name from
/// the registry (plan `perfiles-por-juego`, D12); and its result.
const ID_UNREG: i32 = 1555;
const ID_UNREG_STATUS: i32 = 1556;
/// «Acerca de»: the legal notices the AGPL asks of an interactive program
/// (§5(d)) and the way to its source code.
const ID_ABOUT_TITLE: i32 = 1560;
const ID_ABOUT_NAME: i32 = 1561;
const ID_ABOUT_COPYRIGHT: i32 = 1562;
const ID_ABOUT_LICENSE: i32 = 1563;
const ID_ABOUT_URL: i32 = 1564;
const ID_SOURCE: i32 = 1565;
const ID_SOURCE_STATUS: i32 = 1566;
// Rows: games not answered (name, detail, «Configurar…», «No preguntar
// más») and games not asked about (name, detail, «Volver a preguntar»).
const MAX_NEW: i32 = 8;
const ID_NEW_NAME0: i32 = 1600;
const ID_NEW_DETAIL0: i32 = 1610;
const ID_NEW_CFG0: i32 = 1620;
const ID_NEW_NEVER0: i32 = 1630;
const MAX_IGNORED: i32 = 32;
const ID_IGN_NAME0: i32 = 1700;
const ID_IGN_DETAIL0: i32 = 1740;
const ID_IGN_AGAIN0: i32 = 1780;

const MAX_COLUMN: f32 = 760.0;
const SOURCE_URL: &str = "https://github.com/davidgarciai554/OpenPeripheral";
const ABOUT_NAME: &str = concat!("OpenPeripheral ", env!("CARGO_PKG_VERSION"));
/// Visible label of «Ver código fuente y licencia (se abre en el
/// navegador)», its accessible name.
const SOURCE_LABEL: &str = "Código fuente y licencia ↗";
const ABOUT_COPYRIGHT: &str = "Copyright (C) 2026 Chidev";
const ABOUT_LICENSE: &str = "Este programa se ofrece sin ninguna garantía. Es software libre bajo la licencia GNU AGPL versión 3 o posterior: puedes redistribuirlo y modificarlo según sus términos.";
const THEMES: [(ThemeChoice, &str); 3] = [
    (ThemeChoice::Sistema, "Sistema"),
    (ThemeChoice::Oscuro, "Oscuro"),
    (ThemeChoice::Claro, "Claro"),
];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Role {
    Title,
    Name,
    Strong,
    Secondary,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    /// Text on the app background or on a card.
    Text(Role, bool),
    Button(bool),
    /// Native check box or radio button (classic look: takes our colours).
    Choice,
}

#[derive(Clone, Copy)]
struct Ctl {
    id: i32,
    hwnd: HWND,
    kind: Kind,
}

pub(crate) struct SettingsState {
    ctls: Vec<Ctl>,
    /// Cards and row separators, in content px (not scrolled).
    cards: Vec<RECT>,
    separators: Vec<RECT>,
    brushes: Option<(Palette, HBRUSH, HBRUSH)>,
}

/// A game listed in the page: the mouse it belongs to, its path, and the
/// mouse's name when there is more than one mouse with profiles.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GameRow {
    pub key: DeviceKey,
    pub exe: String,
    pub mouse: Option<String>,
}

fn mice_with_programs(shared: &Shared) -> usize {
    shared
        .devices
        .iter()
        .filter(|d| !d.profiles.is_empty())
        .count()
}

fn row(shared: &Shared, key: &DeviceKey, exe: &str, name: &str) -> GameRow {
    GameRow {
        key: key.clone(),
        exe: exe.to_string(),
        mouse: (mice_with_programs(shared) > 1).then(|| super::profiles::short(name)),
    }
}

/// Games detected this session and not answered yet (pure; tested).
pub fn unanswered(shared: &Shared) -> Vec<GameRow> {
    shared
        .devices
        .iter()
        .flat_map(|d| {
            d.unanswered
                .iter()
                .map(|e| row(shared, &d.info.key, e, &d.info.name))
        })
        .take(MAX_NEW as usize)
        .collect()
}

/// Programs answered with «No preguntar más» (`profile: None`).
pub fn ignored(shared: &Shared) -> Vec<GameRow> {
    shared
        .devices
        .iter()
        .flat_map(|d| {
            d.programs
                .iter()
                .filter(|p| p.profile.is_none())
                .map(|p| row(shared, &d.info.key, &p.exe, &d.info.name))
        })
        .take(MAX_IGNORED as usize)
        .collect()
}

fn detail(r: &GameRow) -> String {
    match &r.mouse {
        Some(m) => format!("{} · {m}", r.exe),
        None => r.exe.clone(),
    }
}

/// «Sistema» says which mode Windows is in now.
pub fn system_help(light: bool) -> String {
    format!(
        "Sigue el modo de Windows (ahora {}).",
        if light { "claro" } else { "oscuro" }
    )
}

// ---------------------------------------------------------------------------
// Lifetime.

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

/// Replaces the device grid with this page.
pub(crate) fn enter(view: HWND) {
    devices_view::clear_devices_page(view);
    let st = SS_NOPREFIX.0 | SS_LEFT.0;
    let btn = WS_TABSTOP.0 | (BS_OWNERDRAW | BS_NOTIFY) as u32;
    let chk = WS_TABSTOP.0 | BS_AUTOCHECKBOX as u32;
    let mut ctls = Vec::new();
    let mut add = |id: i32, kind: Kind, class: PCWSTR, text: &str, style: u32| {
        let hwnd = create(view, id, class, text, style);
        ctls.push(Ctl { id, hwnd, kind });
    };
    let (s, b) = (w!("STATIC"), w!("BUTTON"));
    use Role::*;
    add(
        ID_BACK,
        Kind::Button(false),
        b,
        "Volver a Dispositivos",
        btn,
    );
    add(ID_TITLE, Kind::Text(Title, false), s, "Ajustes", st);
    add(
        ID_INTRO,
        Kind::Text(Secondary, false),
        s,
        "Ajustes de OpenPeripheral para todos tus dispositivos. Lo de cada ratón (DPI, luz, botones y perfiles) está en su página.",
        st,
    );
    add(ID_GEN_TITLE, Kind::Text(Name, true), s, "General", st);
    add(ID_STARTUP, Kind::Choice, b, "Iniciar con Windows", chk);
    add(
        ID_STARTUP_HELP,
        Kind::Text(Secondary, true),
        s,
        "OpenPeripheral se abre en la bandeja al iniciar sesión. También está en el menú de la bandeja.",
        st,
    );
    add(
        ID_PHOTOS,
        Kind::Choice,
        b,
        "Descargar fotos de los ratones",
        chk,
    );
    add(
        ID_PHOTOS_HELP,
        Kind::Text(Secondary, true),
        s,
        "Se conecta a img.openmouse.app (Cloudflare), un servidor externo, que ve tu IP y qué modelo pides; no se envía nada más. Si lo desactivas o falla, se usa el dibujo y se borran las fotos guardadas. Las fotos pertenecen a sus fabricantes.",
        st,
    );
    add(ID_SPEED, Kind::Text(Strong, true), s, "", st);
    // Names that make sense alone: a screen reader reads only the button.
    add(
        ID_SPEED_SLOWER,
        Kind::Button(true),
        b,
        "Puntero más lento",
        btn,
    );
    add(
        ID_SPEED_FASTER,
        Kind::Button(true),
        b,
        "Puntero más rápido",
        btn,
    );
    add(
        ID_SPEED_RESET,
        Kind::Button(true),
        b,
        &format!("Velocidad de Windows ({})", system::POINTER_SPEED_DEFAULT),
        btn,
    );
    add(
        ID_SPEED_HELP,
        Kind::Text(Secondary, true),
        s,
        "Vale para todos los ratones a la vez, también los que OpenPeripheral no puede configurar. No cambia el DPI del ratón: Windows multiplica su movimiento, así que algunos juegos no la usan. Es el mismo ajuste que Configuración › Mouse.",
        st,
    );
    add(ID_LOOK_TITLE, Kind::Text(Name, true), s, "Apariencia", st);
    add(
        ID_THEME_LABEL,
        Kind::Text(Strong, true),
        s,
        "Tema",
        st | WS_GROUP.0,
    );
    for (i, (_, label)) in THEMES.iter().enumerate() {
        // One Tab stop for the group; the arrows move inside it.
        let style = BS_AUTORADIOBUTTON as u32 | if i == 0 { WS_GROUP.0 | WS_TABSTOP.0 } else { 0 };
        add(ID_THEME0 + i as i32, Kind::Choice, b, label, style);
        let help = match i {
            0 => system_help(theme::system_is_light()),
            1 => "Siempre oscuro.".into(),
            _ => "Siempre claro.".into(),
        };
        add(
            ID_THEME_HELP0 + i as i32,
            Kind::Text(Secondary, true),
            s,
            &help,
            st,
        );
    }
    add(
        ID_GAMES_TITLE,
        Kind::Text(Name, true),
        s,
        "Juegos y programas",
        st | WS_GROUP.0,
    );
    add(
        ID_AUTO,
        Kind::Choice,
        b,
        "Cambiar de perfil al abrir los programas asociados",
        chk | WS_GROUP.0,
    );
    add(
        ID_AUTO_HELP,
        Kind::Text(Secondary, true),
        s,
        "Al abrir un programa con perfil, OpenPeripheral lo escribe en el ratón; al cerrarlo, vuelve el que había. Solo funciona con OpenPeripheral abierto.",
        st,
    );
    add(
        ID_ASK,
        Kind::Choice,
        b,
        "Preguntar al detectar un juego nuevo",
        chk,
    );
    add(
        ID_ASK_HELP,
        Kind::Text(Secondary, true),
        s,
        "Juegos de Steam, Xbox, Epic o GOG. Se pregunta con una notificación de Windows.",
        st,
    );
    add(
        ID_NEW_TITLE,
        Kind::Text(Strong, true),
        s,
        "Juegos detectados sin responder",
        st,
    );
    add(
        ID_NEW_EMPTY,
        Kind::Text(Secondary, true),
        s,
        "Ninguno en esta sesión.",
        st,
    );
    for i in 0..MAX_NEW {
        add(ID_NEW_NAME0 + i, Kind::Text(Strong, true), s, "", st);
        add(ID_NEW_DETAIL0 + i, Kind::Text(Secondary, true), s, "", st);
        add(ID_NEW_CFG0 + i, Kind::Button(true), b, "Configurar…", btn);
        add(
            ID_NEW_NEVER0 + i,
            Kind::Button(true),
            b,
            "No preguntar más",
            btn,
        );
    }
    add(
        ID_IGN_TITLE,
        Kind::Text(Strong, true),
        s,
        "Juegos por los que no se pregunta",
        st,
    );
    add(
        ID_IGN_HELP,
        Kind::Text(Secondary, true),
        s,
        "Los que respondiste con «No preguntar más», y los programas cuyo perfil se eliminó.",
        st,
    );
    add(ID_IGN_EMPTY, Kind::Text(Secondary, true), s, "Ninguno.", st);
    for i in 0..MAX_IGNORED {
        add(ID_IGN_NAME0 + i, Kind::Text(Strong, true), s, "", st);
        add(ID_IGN_DETAIL0 + i, Kind::Text(Secondary, true), s, "", st);
        add(
            ID_IGN_AGAIN0 + i,
            Kind::Button(true),
            b,
            "Volver a preguntar",
            btn,
        );
    }
    add(
        ID_GAMES_NOTE,
        Kind::Text(Secondary, true),
        s,
        "Se aplica a todos los ratones. Los programas se vinculan a cada perfil en la pestaña «Avanzado» de cada ratón.",
        st,
    );
    add(
        ID_NOTE_TITLE,
        Kind::Text(Name, true),
        s,
        "Notificaciones y avisos",
        st,
    );
    add(ID_NOTIFY, Kind::Choice, b, "Notificaciones de Windows", chk);
    add(
        ID_NOTIFY_HELP,
        Kind::Text(Secondary, true),
        s,
        "Juego nuevo y cambios de perfil que fallan. Si las desactivas, los fallos se siguen mostrando en la página del ratón.",
        st,
    );
    add(
        ID_CONFLICT,
        Kind::Choice,
        b,
        "Avisar de programas que pueden sobrescribir tus ajustes",
        chk,
    );
    add(
        ID_CONFLICT_HELP,
        Kind::Text(Secondary, true),
        s,
        "Por ejemplo G HUB o Razer Synapse. Con G HUB abierto, guardar en el ratón sigue bloqueado aunque desactives el aviso.",
        st,
    );
    add(
        ID_UNREG,
        Kind::Button(true),
        b,
        "Quitar avisos de Windows",
        btn,
    );
    add(ID_UNREG_STATUS, Kind::Text(Secondary, true), s, "", st);
    add(ID_ABOUT_TITLE, Kind::Text(Name, true), s, "Acerca de", st);
    add(ID_ABOUT_NAME, Kind::Text(Strong, true), s, ABOUT_NAME, st);
    add(
        ID_ABOUT_COPYRIGHT,
        Kind::Text(Secondary, true),
        s,
        ABOUT_COPYRIGHT,
        st,
    );
    add(
        ID_ABOUT_LICENSE,
        Kind::Text(Secondary, true),
        s,
        ABOUT_LICENSE,
        st,
    );
    add(ID_ABOUT_URL, Kind::Text(Secondary, true), s, SOURCE_URL, st);
    add(
        ID_SOURCE,
        Kind::Button(true),
        b,
        "Ver código fuente y licencia (se abre en el navegador)",
        btn,
    );
    add(ID_SOURCE_STATUS, Kind::Text(Secondary, true), s, "", st);
    for c in ctls.iter().filter(|c| c.kind == Kind::Choice) {
        // Classic look: the text takes the colours of WM_CTLCOLORSTATIC.
        unsafe {
            let _ = SetWindowTheme(c.hwnd, w!(""), w!(""));
        }
    }
    let first = ctls[0].hwnd;
    with_app(|a| {
        a.view.scroll = 0;
        a.view.settings = Some(SettingsState {
            ctls,
            cards: Vec::new(),
            separators: Vec::new(),
            brushes: None,
        });
    });
    devices_view::set_text_if_changed(view, "OpenPeripheral — Ajustes");
    refresh(view);
    unsafe {
        let _ = SetFocus(Some(first));
    }
}

/// Back to the device grid, with the focus on its «Ajustes» button.
pub(crate) fn leave(view: HWND) {
    let Some(state) = with_app(|a| a.view.settings.take()).flatten() else {
        return;
    };
    for c in &state.ctls {
        unsafe {
            let _ = DestroyWindow(c.hwnd);
        }
    }
    delete_brushes(&state);
    with_app(|a| a.view.scroll = 0);
    devices_view::restore_devices_page(view, None);
    devices_view::focus_settings_button();
}

fn delete_brushes(state: &SettingsState) {
    if let Some((_, a, b)) = state.brushes {
        unsafe {
            let _ = DeleteObject(a.into());
            let _ = DeleteObject(b.into());
        }
    }
}

/// Releases GDI objects when the window is destroyed with this page open.
pub(crate) fn destroyed() {
    if let Some(Some(state)) = with_app(|a| a.view.settings.take()) {
        delete_brushes(&state);
    }
}

fn ctl(id: i32) -> Option<HWND> {
    with_app(|a| {
        a.view
            .settings
            .as_ref()
            .and_then(|s| s.ctls.iter().find(|c| c.id == id).map(|c| c.hwnd))
    })
    .flatten()
}

fn kind_of(h: HWND) -> Option<(i32, Kind)> {
    with_app(|a| {
        a.view
            .settings
            .as_ref()
            .and_then(|s| s.ctls.iter().find(|c| c.hwnd == h).map(|c| (c.id, c.kind)))
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

fn check(id: i32, on: bool) {
    if let Some(h) = ctl(id) {
        let v = usize::from(on);
        unsafe {
            SendMessageW(h, BM_SETCHECK, Some(WPARAM(v)), None);
        }
    }
}

fn checked(id: i32) -> bool {
    ctl(id).is_some_and(|h| unsafe { SendMessageW(h, BM_GETCHECK, None, None).0 } == 1)
}

/// The Windows pointer speed row, read from Windows each time (it can
/// also change in Configuración).
/// The last change of the pointer speed failed (kept until the next one).
static SPEED_FAILED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

fn refresh_speed() {
    // The buttons stay enabled at the ends (1 and 20): disabling the
    // focused one would lose the keyboard focus. There they do nothing.
    let mut text = speed_text(system::pointer_speed());
    if SPEED_FAILED.load(std::sync::atomic::Ordering::Relaxed) {
        text.push_str(". Error: no se pudo cambiar.");
    }
    set(ID_SPEED, &text);
}

/// "Velocidad del puntero de Windows: 12 de 20".
pub fn speed_text(speed: Option<u32>) -> String {
    match speed {
        Some(s) => format!(
            "Velocidad del puntero de Windows: {s} de {}",
            system::POINTER_SPEED.end()
        ),
        None => "Velocidad del puntero de Windows: no se pudo leer".into(),
    }
}

/// Fills the page from the shared state.
pub(crate) fn refresh(view: HWND) {
    let Some(shared) = with_app(|a| a.view.shared.clone()) else {
        return;
    };
    let g = &shared.general;
    check(ID_STARTUP, system::autostart_enabled());
    check(ID_PHOTOS, g.photos);
    refresh_speed();
    for (i, (choice, _)) in THEMES.iter().enumerate() {
        check(ID_THEME0 + i as i32, g.theme == *choice);
    }
    set(ID_THEME_HELP0, &system_help(theme::system_is_light()));
    check(ID_AUTO, g.auto_switch);
    check(ID_ASK, g.ask_new);
    check(ID_NOTIFY, g.notifications);
    check(ID_CONFLICT, g.conflict_notice);
    show(ID_UNREG, crate::platform::toast::registered());

    let new = unanswered(&shared);
    show(ID_NEW_EMPTY, new.is_empty());
    for i in 0..MAX_NEW {
        let r = new.get(i as usize);
        if let Some(r) = r {
            let name = crate::games::display_name(&r.exe);
            set(ID_NEW_NAME0 + i, &name);
            set(ID_NEW_DETAIL0 + i, &detail(r));
            set(
                ID_NEW_CFG0 + i,
                &format!("Configurar un perfil para {name}"),
            );
            set(ID_NEW_NEVER0 + i, &format!("No preguntar más por {name}"));
        }
        for id in [ID_NEW_NAME0, ID_NEW_DETAIL0, ID_NEW_CFG0, ID_NEW_NEVER0] {
            show(id + i, r.is_some());
        }
    }
    let ign = ignored(&shared);
    show(ID_IGN_EMPTY, ign.is_empty());
    for i in 0..MAX_IGNORED {
        let r = ign.get(i as usize);
        if let Some(r) = r {
            let name = crate::games::display_name(&r.exe);
            set(ID_IGN_NAME0 + i, &name);
            set(ID_IGN_DETAIL0 + i, &detail(r));
            set(ID_IGN_AGAIN0 + i, &format!("Volver a preguntar por {name}"));
        }
        for id in [ID_IGN_NAME0, ID_IGN_DETAIL0, ID_IGN_AGAIN0] {
            show(id + i, r.is_some());
        }
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
}

// ---------------------------------------------------------------------------
// Layout and painting.

fn font_for(fonts: &panel::Fonts, role: Role, compact: bool) -> HFONT {
    match role {
        Role::Title if compact => fonts.title_compact,
        Role::Title => fonts.title,
        Role::Name => fonts.name,
        Role::Strong => fonts.body_strong,
        Role::Secondary => fonts.body,
    }
}

/// Positions every control; returns the content height in px.
pub(crate) fn layout(view: HWND, g: &Geometry, scroll: i32) -> i32 {
    let Some((ctls, palette, fonts)) = with_app(|a| {
        (
            a.view.settings.as_ref().map(|s| s.ctls.clone()),
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
    let visible =
        |id: i32| unsafe { GetWindowLongW(h_of(id), GWL_STYLE) as u32 & WS_VISIBLE.0 != 0 };
    let measure = |id: i32, role: Role, width: i32| -> i32 {
        let t = get_text(h_of(id));
        if t.is_empty() {
            return 0;
        }
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
            r.bottom
        }
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
    let ring = px(4.0);
    let col = g.avail.min(px(MAX_COLUMN));
    let mut rc = RECT::default();
    unsafe {
        let _ = GetClientRect(view, &mut rc);
    }
    let left = (rc.right - col) / 2;
    let bh = px(if g.compact { 44.0 } else { 36.0 });
    let pad = px(if g.compact { 16.0 } else { 20.0 });
    let (ix, inner) = (left + pad, col - 2 * pad);
    let mut cards = Vec::new();
    let mut separators = Vec::new();
    let mut y = px(if g.compact { 16.0 } else { 24.0 });

    let back_w = panel::button_width(&ctx, "‹  Dispositivos");
    put(
        ID_BACK,
        left - ring,
        y - ring,
        back_w + 2 * ring,
        bh + 2 * ring,
    );
    y += bh + px(20.0);
    let th = measure(ID_TITLE, Role::Title, col);
    put(ID_TITLE, left, y, col, th);
    y += th + px(6.0);
    let ih = measure(ID_INTRO, Role::Secondary, col);
    put(ID_INTRO, left, y, col, ih);
    y += ih + px(20.0);

    // A choice (check box or radio) with its help text below, indented.
    let choice_h = px(24.0);
    let indent = px(24.0);
    let choice = |id: i32, help: i32, y: &mut i32, put: &mut dyn FnMut(i32, i32, i32, i32, i32)| {
        put(id, ix, *y, inner, choice_h);
        *y += choice_h + px(2.0);
        let hh = measure(help, Role::Secondary, inner - indent);
        put(help, ix + indent, *y, inner - indent, hh);
        *y += hh + px(14.0);
    };
    let section = |title: i32, y: &mut i32, put: &mut dyn FnMut(i32, i32, i32, i32, i32)| {
        let h = measure(title, Role::Name, inner);
        put(title, ix, *y, inner, h);
        *y += h + px(14.0);
    };
    let card_end = |top: i32, y: &mut i32, cards: &mut Vec<RECT>| {
        // The last item already added its 14 px gap; the card pads 20.
        *y += pad - px(14.0);
        cards.push(RECT {
            left,
            top,
            right: left + col,
            bottom: *y,
        });
        *y += px(20.0);
    };

    // General.
    let top = y;
    y += pad;
    section(ID_GEN_TITLE, &mut y, &mut put);
    choice(ID_STARTUP, ID_STARTUP_HELP, &mut y, &mut put);
    choice(ID_PHOTOS, ID_PHOTOS_HELP, &mut y, &mut put);
    // Windows pointer speed: value, its buttons (wrapped when narrow), help.
    let sh = measure(ID_SPEED, Role::Strong, inner);
    put(ID_SPEED, ix, y, inner, sh);
    y += sh + px(8.0);
    let (mut bx, mut by) = (ix, y);
    for id in [ID_SPEED_SLOWER, ID_SPEED_FASTER, ID_SPEED_RESET] {
        let bw = panel::button_width(&ctx, &get_text(h_of(id)));
        if bx > ix && bx + bw > ix + inner {
            bx = ix;
            by += bh + px(8.0);
        }
        put(id, bx - ring, by - ring, bw + 2 * ring, bh + 2 * ring);
        bx += bw + px(8.0);
    }
    y = by + bh + px(8.0);
    let hh = measure(ID_SPEED_HELP, Role::Secondary, inner);
    put(ID_SPEED_HELP, ix, y, inner, hh);
    y += hh + px(14.0);
    card_end(top, &mut y, &mut cards);

    // Apariencia.
    let top = y;
    y += pad;
    section(ID_LOOK_TITLE, &mut y, &mut put);
    let lh = measure(ID_THEME_LABEL, Role::Strong, inner);
    put(ID_THEME_LABEL, ix, y, inner, lh);
    y += lh + px(8.0);
    for i in 0..THEMES.len() as i32 {
        choice(ID_THEME0 + i, ID_THEME_HELP0 + i, &mut y, &mut put);
    }
    card_end(top, &mut y, &mut cards);

    // Juegos y programas.
    let top = y;
    y += pad;
    section(ID_GAMES_TITLE, &mut y, &mut put);
    choice(ID_AUTO, ID_AUTO_HELP, &mut y, &mut put);
    choice(ID_ASK, ID_ASK_HELP, &mut y, &mut put);
    let sep = |y: &mut i32, separators: &mut Vec<RECT>| {
        separators.push(RECT {
            left: ix,
            top: *y,
            right: ix + inner,
            bottom: *y + 1,
        });
        *y += px(14.0);
    };
    // Rows: name and detail at the left, buttons at the right (below them
    // in a narrow window).
    let game_rows = |names: i32,
                     details: i32,
                     buttons: &[(i32, &str)],
                     n: i32,
                     y: &mut i32,
                     put: &mut dyn FnMut(i32, i32, i32, i32, i32)| {
        for i in 0..n {
            if !visible(names + i) {
                for id in [names, details] {
                    put(id + i, 0, 0, 0, 0);
                }
                for (b, _) in buttons {
                    put(b + i, 0, 0, 0, 0);
                }
                continue;
            }
            let bws: Vec<i32> = buttons
                .iter()
                .map(|(_, l)| panel::button_width(&ctx, l))
                .collect();
            let all = bws.iter().sum::<i32>() + px(8.0) * (bws.len() as i32 - 1);
            let side = !g.compact && all + px(200.0) <= inner;
            let tw = if side { inner - all - px(16.0) } else { inner };
            let nh = measure(names + i, Role::Strong, tw);
            let dh = measure(details + i, Role::Secondary, tw);
            put(names + i, ix, *y, tw, nh);
            put(details + i, ix, *y + nh + px(2.0), tw, dh);
            let text_h = nh + px(2.0) + dh;
            let (mut bx, by) = if side {
                (ix + inner - all, *y + (text_h - bh) / 2)
            } else {
                (ix, *y + text_h + px(8.0))
            };
            for ((b, _), bw) in buttons.iter().zip(&bws) {
                put(b + i, bx - ring, by - ring, bw + 2 * ring, bh + 2 * ring);
                bx += bw + px(8.0);
            }
            *y += if side {
                text_h.max(bh)
            } else {
                text_h + px(8.0) + bh
            } + px(12.0);
        }
    };
    sep(&mut y, &mut separators);
    let nh = measure(ID_NEW_TITLE, Role::Strong, inner);
    put(ID_NEW_TITLE, ix, y, inner, nh);
    y += nh + px(8.0);
    if visible(ID_NEW_EMPTY) {
        let eh = measure(ID_NEW_EMPTY, Role::Secondary, inner);
        put(ID_NEW_EMPTY, ix, y, inner, eh);
        y += eh + px(14.0);
    } else {
        put(ID_NEW_EMPTY, 0, 0, 0, 0);
    }
    game_rows(
        ID_NEW_NAME0,
        ID_NEW_DETAIL0,
        &[
            (ID_NEW_CFG0, "Configurar…"),
            (ID_NEW_NEVER0, "No preguntar más"),
        ],
        MAX_NEW,
        &mut y,
        &mut put,
    );
    sep(&mut y, &mut separators);
    let ih = measure(ID_IGN_TITLE, Role::Strong, inner);
    put(ID_IGN_TITLE, ix, y, inner, ih);
    y += ih + px(2.0);
    let hh = measure(ID_IGN_HELP, Role::Secondary, inner);
    put(ID_IGN_HELP, ix, y, inner, hh);
    y += hh + px(8.0);
    if visible(ID_IGN_EMPTY) {
        let eh = measure(ID_IGN_EMPTY, Role::Secondary, inner);
        put(ID_IGN_EMPTY, ix, y, inner, eh);
        y += eh + px(14.0);
    } else {
        put(ID_IGN_EMPTY, 0, 0, 0, 0);
    }
    game_rows(
        ID_IGN_NAME0,
        ID_IGN_DETAIL0,
        &[(ID_IGN_AGAIN0, "Volver a preguntar")],
        MAX_IGNORED,
        &mut y,
        &mut put,
    );
    sep(&mut y, &mut separators);
    let gh = measure(ID_GAMES_NOTE, Role::Secondary, inner);
    put(ID_GAMES_NOTE, ix, y, inner, gh);
    y += gh + px(14.0);
    card_end(top, &mut y, &mut cards);

    // Notificaciones y avisos.
    let top = y;
    y += pad;
    section(ID_NOTE_TITLE, &mut y, &mut put);
    choice(ID_NOTIFY, ID_NOTIFY_HELP, &mut y, &mut put);
    choice(ID_CONFLICT, ID_CONFLICT_HELP, &mut y, &mut put);
    if visible(ID_UNREG) {
        let uw = panel::button_width(&ctx, "Quitar avisos de Windows");
        put(ID_UNREG, ix - ring, y - ring, uw + 2 * ring, bh + 2 * ring);
        y += bh + px(14.0);
    } else {
        put(ID_UNREG, 0, 0, 0, 0);
    }
    let sh = measure(ID_UNREG_STATUS, Role::Secondary, inner);
    if sh > 0 {
        put(ID_UNREG_STATUS, ix, y, inner, sh);
        y += sh + px(14.0);
    } else {
        put(ID_UNREG_STATUS, 0, 0, 0, 0);
    }
    card_end(top, &mut y, &mut cards);

    // Acerca de.
    let top = y;
    y += pad;
    section(ID_ABOUT_TITLE, &mut y, &mut put);
    for (id, role, gap) in [
        (ID_ABOUT_NAME, Role::Strong, 2.0),
        (ID_ABOUT_COPYRIGHT, Role::Secondary, 8.0),
        (ID_ABOUT_LICENSE, Role::Secondary, 8.0),
        (ID_ABOUT_URL, Role::Secondary, 12.0),
    ] {
        let h = measure(id, role, inner);
        put(id, ix, y, inner, h);
        y += h + px(gap);
    }
    let sw = panel::button_width(&ctx, SOURCE_LABEL).min(inner);
    put(ID_SOURCE, ix - ring, y - ring, sw + 2 * ring, bh + 2 * ring);
    y += bh + px(14.0);
    let sh = measure(ID_SOURCE_STATUS, Role::Secondary, inner);
    if sh > 0 {
        put(ID_SOURCE_STATUS, ix, y, inner, sh);
        y += sh + px(14.0);
    } else {
        put(ID_SOURCE_STATUS, 0, 0, 0, 0);
    }
    card_end(top, &mut y, &mut cards);
    y += px(20.0);

    unsafe {
        ReleaseDC(Some(view), hdc);
    }
    for (id, r) in &pos {
        unsafe {
            let _ = SetWindowPos(
                h_of(*id),
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
            Kind::Button(_) | Kind::Choice => fonts.body,
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
    with_app(|a| {
        if let Some(s) = a.view.settings.as_mut() {
            s.cards = cards;
            s.separators = separators;
        }
    });
    ensure_brushes();
    y
}

/// Cards and separators behind the controls (the view's WM_PAINT).
pub(crate) fn paint(mem: HDC, palette: &Palette, scale: f32, scroll: i32) {
    let Some((cards, separators)) = with_app(|a| {
        a.view
            .settings
            .as_ref()
            .map(|s| (s.cards.clone(), s.separators.clone()))
    })
    .flatten() else {
        return;
    };
    let Some(mut c) = super::glyph::Canvas::new(mem, scale) else {
        return;
    };
    let u = |px: i32| px as f32 / scale;
    for r in &cards {
        c.origin(r.left as f32, (r.top - scroll) as f32);
        let (w, h) = (u(r.right - r.left), u(r.bottom - r.top));
        c.fill_round_rect(0.0, 0.0, w, h, 10.0, palette.surface_card);
        c.stroke_round_rect(0.0, 0.0, w, h, 10.0, 1.0, palette.border_card, false);
    }
    for r in &separators {
        c.origin(r.left as f32, (r.top - scroll) as f32);
        c.fill_round_rect(0.0, 0.0, u(r.right - r.left), 1.0, 0.0, palette.border_card);
    }
}

fn ensure_brushes() {
    let Some(palette) = with_app(|a| a.view.palette) else {
        return;
    };
    let fresh = with_app(|a| {
        a.view
            .settings
            .as_ref()
            .and_then(|s| s.brushes)
            .is_some_and(|(p, ..)| p == palette)
    });
    if fresh != Some(false) {
        return;
    }
    let new = unsafe {
        (
            palette,
            CreateSolidBrush(COLORREF(palette.bg_app.colorref())),
            CreateSolidBrush(COLORREF(palette.surface_card.colorref())),
        )
    };
    let old = with_app(|a| {
        a.view
            .settings
            .as_mut()
            .and_then(|s| s.brushes.replace(new))
    })
    .flatten();
    if let Some((_, a, b)) = old {
        unsafe {
            let _ = DeleteObject(a.into());
            let _ = DeleteObject(b.into());
        }
    }
}

pub(crate) fn draw_item(view: HWND, dis: &DRAWITEMSTRUCT) {
    let Some((id, Kind::Button(on_card))) = kind_of(dis.hwndItem) else {
        return;
    };
    let Some((palette, Some(fonts))) = with_app(|a| (a.view.palette, a.view.fonts.clone())) else {
        return;
    };
    // Short visible labels of buttons whose accessible name is longer;
    // any other button shows its own text.
    let label = match id {
        ID_BACK => "‹  Dispositivos".to_string(),
        ID_SOURCE => SOURCE_LABEL.into(),
        c if (ID_NEW_CFG0..ID_NEW_CFG0 + MAX_NEW).contains(&c) => "Configurar…".into(),
        n if (ID_NEW_NEVER0..ID_NEW_NEVER0 + MAX_NEW).contains(&n) => "No preguntar más".into(),
        a if (ID_IGN_AGAIN0..ID_IGN_AGAIN0 + MAX_IGNORED).contains(&a) => {
            "Volver a preguntar".into()
        }
        _ => get_text(dis.hwndItem),
    };
    let bg = if on_card {
        palette.surface_card
    } else {
        palette.bg_app
    };
    panel::draw_button(
        dis,
        &label,
        bg,
        &palette,
        &fonts,
        devices_view::scale_of(view),
        ButtonStyle::Normal,
    );
}

/// WM_CTLCOLORSTATIC / WM_CTLCOLORBTN for this page's controls.
pub(crate) fn ctl_color(hdc: HDC, child: HWND) -> Option<LRESULT> {
    let (_, kind) = kind_of(child)?;
    let (palette, brushes) = with_app(|a| {
        (
            a.view.palette,
            a.view.settings.as_ref().and_then(|s| s.brushes),
        )
    })?;
    let (_, app, card) = brushes?;
    let (fg, on_card): (Color, bool) = match kind {
        Kind::Text(role, on_card) => (
            match role {
                Role::Secondary => palette.text_secondary,
                _ => palette.text_primary,
            },
            on_card,
        ),
        Kind::Choice => (
            if unsafe { IsWindowEnabled(child).as_bool() } {
                palette.text_primary
            } else {
                palette.text_disabled
            },
            true,
        ),
        Kind::Button(on_card) => {
            return Some(LRESULT(if on_card { card.0 } else { app.0 } as isize));
        }
    };
    let (bg, brush) = if on_card {
        (palette.surface_card, card)
    } else {
        (palette.bg_app, app)
    };
    unsafe {
        SetTextColor(hdc, COLORREF(fg.colorref()));
        SetBkColor(hdc, COLORREF(bg.colorref()));
    }
    Some(LRESULT(brush.0 as isize))
}

// ---------------------------------------------------------------------------
// Commands.

fn send_general(f: impl FnOnce(&mut General)) {
    let Some(mut g) = with_app(|a| a.view.shared.general.clone()) else {
        return;
    };
    f(&mut g);
    // The theme applies at once, before the worker answers.
    theme::set_choice(g.theme);
    with_app(|a| a.view.shared.general = g.clone());
    send(Command::SetGeneral(g));
}

/// Handles WM_COMMAND for this page; false when not one of its controls.
pub(crate) fn on_command(view: HWND, id: i32, code: u32) -> bool {
    let Some(shared) = with_app(|a| a.view.shared.clone()) else {
        return false;
    };
    match (id, code) {
        // Esc (IsDialogMessage sends IDCANCEL) and «‹ Dispositivos».
        (2, _) | (ID_BACK, BN_CLICKED) => leave(view),
        // Enter on an owner-drawn button: click it.
        (1, _) => unsafe {
            let f = GetFocus();
            if kind_of(f).is_some_and(|(_, k)| matches!(k, Kind::Button(_))) {
                SendMessageW(f, BM_CLICK, None, None);
            }
        },
        (ID_SPEED_SLOWER | ID_SPEED_FASTER | ID_SPEED_RESET, BN_CLICKED) => {
            let to = match (id, system::pointer_speed()) {
                (ID_SPEED_RESET, _) => Some(system::POINTER_SPEED_DEFAULT),
                (ID_SPEED_SLOWER, Some(s)) => Some(s.saturating_sub(1).max(1)),
                (_, Some(s)) => Some((s + 1).min(*system::POINTER_SPEED.end())),
                (_, None) => None,
            };
            let ok = to.is_some_and(|s| system::set_pointer_speed(s).is_ok());
            SPEED_FAILED.store(!ok, std::sync::atomic::Ordering::Relaxed);
            refresh_speed();
        }
        (ID_STARTUP, BN_CLICKED) => {
            let _ = system::set_autostart(checked(ID_STARTUP));
            check(ID_STARTUP, system::autostart_enabled());
        }
        (t, BN_CLICKED) if (ID_THEME0..ID_THEME0 + THEMES.len() as i32).contains(&t) => {
            let choice = THEMES[(t - ID_THEME0) as usize].0;
            // Applied when the worker publishes it (WM_APP_UPDATE).
            send_general(|g| g.theme = choice);
        }
        (ID_AUTO | ID_ASK | ID_NOTIFY | ID_CONFLICT | ID_PHOTOS, BN_CLICKED) => {
            let (auto, ask) = (checked(ID_AUTO), checked(ID_ASK));
            let (notify, conflict) = (checked(ID_NOTIFY), checked(ID_CONFLICT));
            let photos = checked(ID_PHOTOS);
            send_general(|g| {
                g.auto_switch = auto;
                g.ask_new = ask;
                g.notifications = notify;
                g.conflict_notice = conflict;
                g.photos = photos;
            });
        }
        (c, BN_CLICKED) if (ID_NEW_CFG0..ID_NEW_CFG0 + MAX_NEW).contains(&c) => {
            if let Some(r) = unanswered(&shared).get((c - ID_NEW_CFG0) as usize) {
                let (key, exe) = (r.key.clone(), r.exe.clone());
                leave(view);
                super::configure_game(key, &exe);
            }
        }
        (n, BN_CLICKED) if (ID_NEW_NEVER0..ID_NEW_NEVER0 + MAX_NEW).contains(&n) => {
            if let Some(r) = unanswered(&shared).get((n - ID_NEW_NEVER0) as usize) {
                send(Command::SetProgram(r.key.clone(), r.exe.clone(), None));
            }
        }
        (ID_UNREG, BN_CLICKED) => {
            let text = match crate::platform::toast::unregister() {
                Ok(()) => "Avisos de Windows quitados: se borró el registro de OpenPeripheral para las notificaciones. Se vuelve a crear con el próximo aviso.".to_string(),
                Err(e) => format!("Error: no se pudieron quitar los avisos ({e})."),
            };
            set(ID_UNREG_STATUS, &text);
            refresh(view);
        }
        (ID_SOURCE, BN_CLICKED) => {
            let text = match system::open_url(SOURCE_URL) {
                Ok(()) => String::new(),
                Err(e) => {
                    format!("No se pudo abrir el navegador ({e}). La dirección es la de arriba.")
                }
            };
            set(ID_SOURCE_STATUS, &text);
            refresh(view);
        }
        (a, BN_CLICKED) if (ID_IGN_AGAIN0..ID_IGN_AGAIN0 + MAX_IGNORED).contains(&a) => {
            if let Some(r) = ignored(&shared).get((a - ID_IGN_AGAIN0) as usize) {
                send(Command::RemoveProgram(r.key.clone(), r.exe.clone()));
            }
        }
        (_, BN_SETFOCUS) => {
            if let Some(h) = ctl(id) {
                devices_view::ensure_visible(view, h);
            }
        }
        _ => return kind_of(ctl(id).unwrap_or_default()).is_some(),
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::app::{Snapshot, Status};
    use crate::devices::{DeviceInfo, SUPPORTED, State};
    use crate::games::Program;

    fn mouse(
        key: &str,
        name: &str,
        unanswered: &[&str],
        programs: &[(&str, Option<&str>)],
    ) -> Snapshot {
        let mut info = DeviceInfo::new(&SUPPORTED[0], name);
        info.key = DeviceKey(key.into());
        let mut s = Snapshot {
            info,
            status: Status::Ok(State::default()),
            last_battery: None,
            saved: false,
            saved_dpi: None,
            saved_light: None,
            applied_color: None,
            original_dpi: None,
            profile: crate::app::Profile::NotRead,
            backup: None,
            profiles: Vec::new(),
            active_profile: None,
            programs: Vec::new(),
            game: None,
            game_previous: None,
            unanswered: Vec::new(),
        };
        s.unanswered = unanswered.iter().map(|e| e.to_string()).collect();
        s.programs = programs
            .iter()
            .map(|(exe, p)| Program {
                exe: exe.to_string(),
                profile: p.map(str::to_string),
            })
            .collect();
        s.profiles = vec![crate::profiles::MouseProfile {
            name: "Trabajo".into(),
            dpi: None,
            light: None,
            buttons: Vec::new(),
        }];
        s
    }

    #[test]
    fn games_lists_come_from_every_mouse() {
        let one = mouse(
            "a",
            "G305",
            &[r"c:\juegos\peak.exe"],
            &[
                (r"c:\juegos\elden.exe", None),
                (r"c:\juegos\cs2.exe", Some("Trabajo")),
            ],
        );
        let shared = Shared {
            devices: vec![one.clone()],
            ..Default::default()
        };
        let new = unanswered(&shared);
        assert_eq!(new.len(), 1);
        assert_eq!(new[0].mouse, None);
        assert_eq!(detail(&new[0]), r"c:\juegos\peak.exe");
        // Only «No preguntar más» (no profile) is listed as ignored.
        let ign = ignored(&shared);
        assert_eq!(ign.len(), 1);
        assert_eq!(ign[0].exe, r"c:\juegos\elden.exe");

        // With two mice, each row says which one.
        let two = mouse("b", "G502 X", &[], &[(r"c:\juegos\x.exe", None)]);
        let shared = Shared {
            devices: vec![one, two],
            ..Default::default()
        };
        let ign = ignored(&shared);
        assert_eq!(ign.len(), 2);
        assert!(ign.iter().all(|r| r.mouse.is_some()));
        assert_eq!(ign[1].key, DeviceKey("b".into()));
    }

    #[test]
    fn system_theme_help_says_the_current_mode() {
        assert_eq!(
            system_help(false),
            "Sigue el modo de Windows (ahora oscuro)."
        );
        assert_eq!(system_help(true), "Sigue el modo de Windows (ahora claro).");
    }

    #[test]
    fn about_shows_the_agpl_notices_and_the_source() {
        // AGPL §5(d): copyright, no warranty and how to get the licence.
        assert_eq!(env!("CARGO_PKG_LICENSE"), "AGPL-3.0-or-later");
        assert!(ABOUT_NAME.ends_with(env!("CARGO_PKG_VERSION")));
        assert!(ABOUT_COPYRIGHT.contains("Chidev"));
        assert!(ABOUT_LICENSE.contains("sin ninguna garantía"));
        assert!(ABOUT_LICENSE.contains("AGPL versión 3 o posterior"));
        assert_eq!(
            SOURCE_URL,
            "https://github.com/davidgarciai554/OpenPeripheral"
        );
        assert_eq!(env!("CARGO_PKG_REPOSITORY"), SOURCE_URL);
        assert!(system::is_safe_url(SOURCE_URL));
    }
}
