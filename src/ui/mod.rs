//! Native Win32 user interface: a tray icon plus a settings window that only
//! exists while it is open. Everything uses system controls, fonts and
//! colours so keyboard navigation, high contrast and screen readers work as
//! in any standard Windows dialog. See `docs/design/DESIGN.md`.

pub mod text;
pub mod theme;

mod confirm;
mod devices_view;
mod glyph;
mod icon;
mod mouse_settings;
mod panel;

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
    NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE, NIM_MODIFY, NIM_SETVERSION,
    NIN_SELECT, NINF_KEY, NOTIFYICON_VERSION_4, NOTIFYICONDATAW, Shell_NotifyIconW,
};
use windows::Win32::UI::WindowsAndMessaging::*;
use windows::core::{PCWSTR, w};

pub(crate) const WM_APP_TRAY: u32 = WM_APP + 1;
pub(crate) const WM_APP_UPDATE: u32 = WM_APP + 2;
pub(crate) const WM_APP_SHOW: u32 = WM_APP + 3;
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

const MAIN_CLASS: PCWSTR = w!("OpenPeripheral.Main");

const ID_OPEN: usize = 1;
const ID_REFRESH: usize = 2;
const ID_AUTOSTART: usize = 3;
const ID_EXIT: usize = 4;
const ID_DEVICE_BASE: usize = 1000;

pub(crate) struct App {
    pub shared: Arc<Mutex<Shared>>,
    pub tx: Sender<Command>,
    pub view: devices_view::State,
    taskbar_created: u32,
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
}

fn show_menu(hwnd: HWND, x: i32, y: i32) {
    let Some(shared) = snapshot() else { return };
    unsafe {
        let Ok(menu) = CreatePopupMenu() else { return };
        for (i, d) in shared.devices.iter().enumerate() {
            let label = wide(&text::device_line(d).replace('&', "&&"));
            let _ = AppendMenuW(menu, MF_STRING, ID_DEVICE_BASE + i, PCWSTR(label.as_ptr()));
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
        // Device entries open the view; their settings screen does not
        // exist yet.
        match chosen.checked_sub(ID_DEVICE_BASE) {
            Some(_) => devices_view::open(),
            None => on_command(chosen),
        }
    }
}

fn on_command(id: usize) {
    match id {
        ID_OPEN => devices_view::open(),
        ID_REFRESH => send(Command::Refresh),
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
                tray_update(hwnd, &shared);
            }
            devices_view::refresh();
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
        let worker = Worker::new(shared.clone(), notify, system::running_conflicts);
        let handle = std::thread::Builder::new()
            .name("devices".into())
            .spawn(move || worker.run(rx));

        APP.with(|a| {
            *a.borrow_mut() = Some(App {
                shared,
                tx: tx.clone(),
                view: devices_view::State::default(),
                taskbar_created: RegisterWindowMessageW(w!("TaskbarCreated")),
            })
        });
        tray_add(main);

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
