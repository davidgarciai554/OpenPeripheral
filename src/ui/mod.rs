//! Native Win32 user interface: a tray icon plus a settings window that only
//! exists while it is open. Everything uses system controls, fonts and
//! colours so keyboard navigation, high contrast and screen readers work as
//! in any standard Windows dialog. See `docs/design/DESIGN.md`.

pub mod text;
pub mod theme;

mod app_settings;
mod commands;
mod confirm;
mod devices_view;
mod glyph;
mod icon;
mod light;
mod mouse_art;
mod mouse_settings;
mod panel;
mod profiles;

use crate::app::{Command, Shared, Worker};
use crate::platform::system;
use std::cell::RefCell;
use std::sync::mpsc::{Sender, channel};
use std::sync::{Arc, Mutex};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::HiDpi::{
    DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2, SetProcessDpiAwarenessContext,
};
use windows::Win32::UI::Shell::{
    NIF_ICON, NIF_INFO, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIIF_WARNING, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW,
    Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

pub(crate) const WM_APP_TRAY: u32 = WM_APP + 1;
pub(crate) const WM_APP_UPDATE: u32 = WM_APP + 2;
pub(crate) const WM_APP_SHOW: u32 = WM_APP + 3;
/// A button of a toast was clicked (queued in `TOAST_CLICKS`).
pub(crate) const WM_APP_TOAST: u32 = WM_APP + 4;

/// Toast clicks arrive on a thread-pool thread: they wait here for the UI.
static TOAST_CLICKS: Mutex<Vec<String>> = Mutex::new(Vec::new());
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

const MAIN_CLASS: PCWSTR = w!("OpenPeripheral.Main");

const ID_OPEN: usize = 1;
const ID_REFRESH: usize = 2;
const ID_AUTOSTART: usize = 3;
const ID_EXIT: usize = 4;
const ID_SETTINGS: usize = 5;
const ID_DEVICE_BASE: usize = 1000;
/// Tray profile entries: `ID_PROFILE_BASE + device * 8 + profile`.
const ID_PROFILE_BASE: usize = 2000;
/// «Editar perfiles…» of each device: `ID_EDIT_PROFILES_BASE + device`.
const ID_EDIT_PROFILES_BASE: usize = 3000;

/// A profile chosen in the tray menu, until its write ends:
/// (`Shared::writes` when sent, profile, profile it replaces).
type TrayUse = (u64, String, String);

pub(crate) struct App {
    pub shared: Arc<Mutex<Shared>>,
    pub tx: Sender<Command>,
    pub view: devices_view::State,
    taskbar_created: u32,
    tray_use: Option<TrayUse>,
    /// New games asked and automatic results already shown.
    asks_seen: u64,
    autos_seen: u64,
}

thread_local! {
    static APP: RefCell<Option<App>> = const { RefCell::new(None) };
}

/// Runs `f` with the app state unless it is already borrowed. Win32 calls
/// can re-enter the window procedure synchronously; those re-entrant
/// messages are simply skipped instead of panicking.
pub(crate) fn with_app<R>(f: impl FnOnce(&mut App) -> R) -> Option<R> {
    APP.with(|a| a.try_borrow_mut().ok().and_then(|mut a| a.as_mut().map(f)))
}

pub(crate) fn snapshot() -> Option<Shared> {
    with_app(|a| a.shared.lock().unwrap_or_else(|p| p.into_inner()).clone())
}

pub(crate) fn send(cmd: Command) {
    with_app(|a| {
        let _ = a.tx.send(cmd);
    });
}

pub(crate) fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain([0]).collect()
}

fn tray_data(hwnd: HWND) -> NOTIFYICONDATAW {
    NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: 1,
        ..Default::default()
    }
}

fn tray_add(hwnd: HWND) {
    let mut nid = tray_data(hwnd);
    nid.uFlags = NIF_MESSAGE | NIF_ICON | NIF_TIP | NIF_SHOWTIP;
    nid.uCallbackMessage = WM_APP_TRAY;
    nid.hIcon = icon::tray_icon(hwnd);
    set_tip(&mut nid, "OpenPeripheral\nBuscando dispositivos…");
    unsafe {
        let _ = Shell_NotifyIconW(NIM_ADD, &nid);
        nid.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        let _ = Shell_NotifyIconW(NIM_SETVERSION, &nid);
    }
}

fn set_tip(nid: &mut NOTIFYICONDATAW, text: &str) {
    let w: Vec<u16> = text.encode_utf16().take(nid.szTip.len() - 1).collect();
    nid.szTip = [0; 128];
    nid.szTip[..w.len()].copy_from_slice(&w);
}

fn tray_update(hwnd: HWND, shared: &Shared) {
    let mut nid = tray_data(hwnd);
    nid.uFlags = NIF_TIP | NIF_SHOWTIP;
    set_tip(&mut nid, &text::tooltip(shared));
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
    // A profile chosen in the tray failed: a balloon of the icon says so
    // (DESIGN.md «Perfiles en la bandeja»; nothing is shown when it works).
    let failed = with_app(|a| {
        let mark = a.tray_use.as_ref()?.0;
        if shared.writes == mark {
            return None;
        }
        let (_, to, from) = a.tray_use.take()?;
        let failed = shared.write_result.as_ref().is_some_and(|m| !m.ok);
        failed.then_some((to, from))
    })
    .flatten();
    if let (Some((to, from)), true) = (failed, shared.general.notifications) {
        balloon(
            hwnd,
            &format!("No se pudo cambiar a «{to}»"),
            &format!("El ratón no respondió y sigue con «{from}». Muévelo y vuelve a intentarlo."),
        );
    }
}

/// A balloon notification of the tray icon (no buttons).
fn balloon(hwnd: HWND, title: &str, body: &str) {
    let mut nid = tray_data(hwnd);
    nid.uFlags = NIF_INFO;
    nid.dwInfoFlags = NIIF_WARNING;
    let copy = |dst: &mut [u16], text: &str| {
        let w: Vec<u16> = text.encode_utf16().take(dst.len() - 1).collect();
        dst.fill(0);
        dst[..w.len()].copy_from_slice(&w);
    };
    copy(&mut nid.szInfoTitle, title);
    copy(&mut nid.szInfo, body);
    unsafe {
        let _ = Shell_NotifyIconW(NIM_MODIFY, &nid);
    }
}

/// New games and failed automatic switches, from the worker.
fn game_notices(hwnd: HWND, shared: &Shared) {
    let Some((asks, autos)) = with_app(|a| {
        let r = (a.asks_seen != shared.asks, a.autos_seen != shared.autos);
        a.asks_seen = shared.asks;
        a.autos_seen = shared.autos;
        r
    }) else {
        return;
    };
    // With notifications off, new games still appear in «Ajustes» and
    // failures in the mouse page.
    if !shared.general.notifications {
        return;
    }
    if let (true, Some((key, exe))) = (asks, &shared.ask) {
        ask_toast(hwnd, shared, key, exe);
    }
    if let (true, Some(m)) = (autos, &shared.auto_result) {
        if !m.ok {
            balloon(hwnd, "Cambio de perfil por juego", &m.text);
        }
    }
}

/// «Juego nuevo: PEAK.exe»: a toast with «Configurar… / Ahora no / No
/// preguntar más». The app's notification name is registered in HKCU the
/// first time (plan `perfiles-por-juego`, D12). If the toast cannot be
/// shown, the game still appears in «Juegos detectados sin responder».
fn ask_toast(hwnd: HWND, shared: &Shared, key: &crate::devices::DeviceKey, exe: &str) {
    if !crate::platform::toast::registered() && crate::platform::toast::register().is_err() {
        return;
    }
    let mut name = crate::games::display_name(exe);
    if name.chars().count() > 60 {
        name = name.chars().take(59).collect::<String>() + "…";
    }
    let mouse = shared
        .devices
        .iter()
        .find(|d| &d.info.key == key)
        .map(|d| profiles::short(&d.info.name))
        .unwrap_or_else(|| "ratón".into());
    // Arguments: action|mouse key|path ('|' cannot appear in a path).
    let arg = |action: &str| format!("{action}|{}|{exe}", key.0);
    let (configure, later, never) = (arg("configurar"), arg("ahora-no"), arg("no-preguntar"));
    let main = hwnd.0 as usize;
    let _ = crate::platform::toast::show(
        &format!("Juego nuevo: {name}"),
        &format!("¿Quieres un perfil del {mouse} para cuando lo abras?"),
        &configure,
        &[
            ("Configurar…", configure.as_str()),
            ("Ahora no", later.as_str()),
            ("No preguntar más", never.as_str()),
        ],
        move |a| {
            if let Ok(mut c) = TOAST_CLICKS.lock() {
                c.push(a);
            }
            unsafe {
                let _ = PostMessageW(
                    Some(HWND(main as *mut _)),
                    WM_APP_TOAST,
                    WPARAM(0),
                    LPARAM(0),
                );
            }
        },
    );
}

fn toast_clicked(arg: &str) {
    let mut parts = arg.splitn(3, '|');
    let (Some(action), Some(key), Some(exe)) = (parts.next(), parts.next(), parts.next()) else {
        return;
    };
    let key = crate::devices::DeviceKey(key.to_string());
    match action {
        "configurar" => configure_game(key, exe),
        "no-preguntar" => send(Command::SetProgram(key, exe.to_string(), None)),
        // «Ahora no»: asked again next time; it stays in the page's list.
        _ => {}
    }
}

/// «Configurar…» of a new game (notification or «Ajustes»): the mouse page
/// with a new profile for it.
pub(crate) fn configure_game(key: crate::devices::DeviceKey, exe: &str) {
    open_mouse_settings(key.clone());
    let page = with_app(|a| {
        (
            devices_view::hwnd_of(&a.view),
            a.view.mouse.as_ref().map(|m| m.key.clone()),
        )
    });
    if let Some((Some(view), Some(k))) = page {
        if k == key {
            mouse_settings::create_profile_for(view, exe.to_string());
        }
    }
}

/// Opens the settings page of a mouse (tray «Editar perfiles…»). If another
/// mouse's page is open, it stays: its unsaved changes are never dropped.
/// The «Ajustes» page has nothing unsaved: it closes.
fn open_mouse_settings(key: crate::devices::DeviceKey) {
    devices_view::open();
    let Some((Some(view), current, settings)) = with_app(|a| {
        (
            devices_view::hwnd_of(&a.view),
            a.view.mouse.as_ref().map(|m| m.key.clone()),
            a.view.settings.is_some(),
        )
    }) else {
        return;
    };
    if settings {
        app_settings::leave(view);
    }
    if current.is_none() {
        mouse_settings::enter(view, key);
    }
}

/// Tray «Ajustes…»: the «Ajustes» page, unless a mouse page is open (its
/// unsaved changes are never dropped).
fn open_app_settings() {
    devices_view::open();
    let Some((Some(view), mouse, settings)) = with_app(|a| {
        (
            devices_view::hwnd_of(&a.view),
            a.view.mouse.is_some(),
            a.view.settings.is_some(),
        )
    }) else {
        return;
    };
    if !mouse && !settings {
        app_settings::enter(view);
    }
}

/// The profile submenu of a mouse (DESIGN.md «Perfiles en la bandeja").
unsafe fn profile_submenu(device: usize, m: &profiles::TrayMenu) -> Option<HMENU> {
    unsafe {
        let sub = CreatePopupMenu().ok()?;
        if let Some(reason) = m.blocked {
            let t = wide(reason);
            let _ = AppendMenuW(sub, MF_STRING | MF_GRAYED, 0, PCWSTR(t.as_ptr()));
            let _ = AppendMenuW(sub, MF_SEPARATOR, 0, None);
        }
        let first = ID_PROFILE_BASE + device * 8;
        let flags = if m.blocked.is_some() {
            MF_STRING | MF_GRAYED
        } else {
            MF_STRING
        };
        let mut active = None;
        for (i, (name, is_active)) in m.items.iter().enumerate() {
            let t = wide(name);
            let _ = AppendMenuW(sub, flags, first + i, PCWSTR(t.as_ptr()));
            if *is_active {
                active = Some(first + i);
            }
        }
        if let Some(a) = active {
            // Radio bullet on the active profile.
            let _ = CheckMenuRadioItem(
                sub,
                first as u32,
                (first + m.items.len() - 1) as u32,
                a as u32,
                MF_BYCOMMAND.0,
            );
        }
        let _ = AppendMenuW(sub, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(
            sub,
            MF_STRING,
            ID_EDIT_PROFILES_BASE + device,
            w!("&Editar perfiles…"),
        );
        Some(sub)
    }
}

fn show_menu(hwnd: HWND, x: i32, y: i32) {
    let Some(shared) = snapshot() else { return };
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        let ghub = shared.conflicts.contains(&"Logitech G HUB");
        for (i, d) in shared.devices.iter().enumerate() {
            let label = wide(&text::device_line(d).replace('&', "&&"));
            let _ = AppendMenuW(menu, MF_STRING, ID_DEVICE_BASE + i, PCWSTR(label.as_ptr()));
            if let Some(m) = profiles::tray_menu(d, ghub) {
                if let Some(sub) = profile_submenu(i, &m) {
                    let t = wide(&m.title);
                    let _ = AppendMenuW(
                        menu,
                        MF_STRING | MF_POPUP,
                        sub.0 as usize,
                        PCWSTR(t.as_ptr()),
                    );
                }
            }
        }
        if shared.devices.is_empty() {
            let label = wide(if shared.scanned {
                "Sin dispositivos compatibles"
            } else {
                "Buscando dispositivos…"
            });
            let _ = AppendMenuW(menu, MF_STRING | MF_GRAYED, 0, PCWSTR(label.as_ptr()));
        }
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, ID_OPEN, w!("&Abrir OpenPeripheral"));
        let _ = AppendMenuW(menu, MF_STRING, ID_REFRESH, w!("Ac&tualizar ahora"));
        let _ = AppendMenuW(menu, MF_STRING, ID_SETTINGS, w!("A&justes…"));
        let check = if system::autostart_enabled() {
            MF_CHECKED
        } else {
            MF_UNCHECKED
        };
        let _ = AppendMenuW(
            menu,
            MF_STRING | check,
            ID_AUTOSTART,
            w!("Iniciar con &Windows"),
        );
        let _ = AppendMenuW(menu, MF_SEPARATOR, 0, None);
        let _ = AppendMenuW(menu, MF_STRING, ID_EXIT, w!("&Salir"));
        let _ = SetMenuDefaultItem(menu, ID_OPEN as u32, 0);
        // Required so the menu closes when focus moves elsewhere.
        let _ = SetForegroundWindow(hwnd);
        // TPM_RETURNCMD: the choice is resolved against the same snapshot
        // the menu was built from, so a reordered list cannot misroute it.
        let chosen = TrackPopupMenu(
            menu,
            TPM_RIGHTBUTTON | TPM_BOTTOMALIGN | TPM_RETURNCMD | TPM_NONOTIFY,
            x,
            y,
            None,
            hwnd,
            None,
        )
        .0 as usize;
        let _ = PostMessageW(Some(hwnd), WM_NULL, WPARAM(0), LPARAM(0));
        let _ = DestroyMenu(menu);
        // Device entries open the view.
        if let Some(i) = chosen.checked_sub(ID_EDIT_PROFILES_BASE) {
            if let Some(d) = shared.devices.get(i) {
                open_mouse_settings(d.info.key.clone());
            }
        } else if let Some(n) = chosen.checked_sub(ID_PROFILE_BASE) {
            // Resolved against the snapshot the menu was built from.
            let (device, profile) = (n / 8, n % 8);
            if let Some(d) = shared.devices.get(device) {
                if let Some(p) = d.profiles.get(profile) {
                    let from = d.active_profile.clone().unwrap_or_default();
                    with_app(|a| a.tray_use = Some((shared.writes, p.name.clone(), from)));
                    send(crate::profiles::use_command(
                        d.info.key.clone(),
                        p.name.clone(),
                    ));
                }
            }
        } else if chosen >= ID_DEVICE_BASE {
            devices_view::open();
        } else {
            on_command(chosen);
        }
    }
}

fn on_command(id: usize) {
    match id {
        ID_OPEN => devices_view::open(),
        ID_REFRESH => send(Command::Refresh),
        ID_SETTINGS => open_app_settings(),
        ID_AUTOSTART => {
            let _ = system::set_autostart(!system::autostart_enabled());
        }
        ID_EXIT => unsafe { PostQuitMessage(0) },
        _ => {}
    }
}

extern "system" fn main_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let taskbar_created = with_app(|a| a.taskbar_created).unwrap_or(0);
    match msg {
        WM_APP_TRAY => {
            // NOTIFYICON_VERSION_4: event in LOWORD(lParam), anchor in wParam.
            let event = (lparam.0 & 0xffff) as u32;
            let (x, y) = (
                (wparam.0 & 0xffff) as i16 as i32,
                ((wparam.0 >> 16) & 0xffff) as i16 as i32,
            );
            match event {
                // Click, or Enter/Space on the focused icon (Win+B).
                NIN_SELECT | NIN_KEYSELECT => devices_view::open(),
                WM_CONTEXTMENU => show_menu(hwnd, x, y),
                _ => {}
            }
            LRESULT(0)
        }
        WM_APP_UPDATE => {
            if let Some(shared) = snapshot() {
                theme::set_choice(shared.general.theme);
                tray_update(hwnd, &shared);
                game_notices(hwnd, &shared);
            }
            devices_view::refresh();
            LRESULT(0)
        }
        WM_APP_TOAST => {
            let clicks = TOAST_CLICKS
                .lock()
                .map(|mut c| std::mem::take(&mut *c))
                .unwrap_or_default();
            for c in clicks {
                toast_clicked(&c);
            }
            LRESULT(0)
        }
        WM_APP_SHOW => {
            devices_view::open();
            LRESULT(0)
        }
        WM_DEVICECHANGE => {
            send(Command::Rescan);
            LRESULT(1)
        }
        WM_COMMAND => {
            on_command(wparam.0 & 0xffff);
            LRESULT(0)
        }
        m if m == taskbar_created && m != 0 => {
            // Explorer restarted: the icon must be added again.
            tray_add(hwnd);
            if let Some(shared) = snapshot() {
                tray_update(hwnd, &shared);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) },
    }
}

pub fn run() {
    if system::single_instance().is_none() {
        // Already running: ask that instance to show its window.
        unsafe {
            if let Ok(h) = FindWindowW(MAIN_CLASS, None) {
                let _ = PostMessageW(Some(h), WM_APP_SHOW, WPARAM(0), LPARAM(0));
            }
        }
        return;
    }
    unsafe {
        let _ = SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
        let instance = GetModuleHandleW(None).unwrap_or_default();
        let class = WNDCLASSW {
            lpfnWndProc: Some(main_proc),
            hInstance: instance.into(),
            lpszClassName: MAIN_CLASS,
            ..Default::default()
        };
        RegisterClassW(&class);
        devices_view::register(instance.into());
        // A hidden top-level window (not message-only) so it receives the
        // TaskbarCreated and WM_DEVICECHANGE broadcasts.
        let Ok(main) = CreateWindowExW(
            WINDOW_EX_STYLE::default(),
            MAIN_CLASS,
            w!("OpenPeripheral"),
            WS_OVERLAPPED,
            0,
            0,
            0,
            0,
            None,
            None,
            Some(instance.into()),
            None,
        ) else {
            return;
        };

        let shared = Arc::new(Mutex::new(Shared::default()));
        let (tx, rx) = channel();
        let main_ptr = main.0 as usize;
        let notify = Box::new(move || {
            let _ = PostMessageW(
                Some(HWND(main_ptr as *mut _)),
                WM_APP_UPDATE,
                WPARAM(0),
                LPARAM(0),
            );
        });
        let worker = Worker::new(
            shared.clone(),
            notify,
            system::running_conflicts,
            tx.clone(),
        );
        let handle = std::thread::Builder::new()
            .name("devices".into())
            .spawn(move || worker.run(rx));

        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                shared,
                tx: tx.clone(),
                view: devices_view::State::default(),
                taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
                tray_use: None,
                asks_seen: 0,
                autos_seen: 0,
            })
        });
        tray_add(main);
        // Programs opening and closing (profiles per game); kept for the
        // life of the message loop.
        let _hook = crate::platform::games::start(tx.clone());

        let mut msg = MSG::default();
        while GetMessageW(&mut msg, None, 0, 0).as_bool() {
            // Keyboard navigation (Tab, Enter, Esc) for the view and its dialog.
            let dialogs =
                with_app(|a| [devices_view::hwnd_of(&a.view), confirm::hwnd()]).unwrap_or_default();
            if dialogs
                .iter()
                .flatten()
                .any(|&d| IsDialogMessageW(d, &msg).as_bool())
            {
                continue;
            }
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }

        let _ = Shell_NotifyIconW(NIM_DELETE, &tray_data(main));
        let _ = tx.send(Command::Quit);
        if let Ok(h) = handle {
            let _ = h.join();
        }
    }
}
