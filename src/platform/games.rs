//! Programs opening and closing, without polling (plan `perfiles-por-juego`,
//! D6). Windows reports each window that comes to the foreground
//! (`SetWinEventHook`, on the UI thread, out of context: nothing is injected
//! into other processes); the first time a process is seen, its path is read
//! with the least access (`PROCESS_QUERY_LIMITED_INFORMATION | SYNCHRONIZE`)
//! and Windows reports its end (`RegisterWaitForSingleObject`). Elevated or
//! protected processes cannot be opened and are ignored without notice.
//! Limits: a program that never comes to the foreground is not seen, nor one
//! already in the foreground when the app starts.

use crate::app::Command;
use std::ffi::c_void;
use std::sync::Mutex;
use std::sync::mpsc::Sender;
use windows::Win32::Foundation::{CloseHandle, HANDLE, HWND};
use windows::Win32::System::Threading::{
    INFINITE, OpenProcess, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
    PROCESS_SYNCHRONIZE, QueryFullProcessImageNameW, RegisterWaitForSingleObject, UnregisterWait,
    WT_EXECUTEONLYONCE,
};
use windows::Win32::UI::Accessibility::{HWINEVENTHOOK, SetWinEventHook};
use windows::Win32::UI::WindowsAndMessaging::{
    EVENT_SYSTEM_FOREGROUND, GetWindowThreadProcessId, WINEVENT_OUTOFCONTEXT,
    WINEVENT_SKIPOWNPROCESS,
};
use windows::core::PWSTR;

static TX: Mutex<Option<Sender<Command>>> = Mutex::new(None);
/// Processes being watched (one wait each).
static SEEN: Mutex<Vec<u32>> = Mutex::new(Vec::new());

struct Watch {
    pid: u32,
    process: HANDLE,
    wait: Mutex<HANDLE>,
}

fn send(c: Command) {
    if let Some(tx) = TX.lock().ok().and_then(|t| t.clone()) {
        let _ = tx.send(c);
    }
}

/// Starts watching. Call once on the UI thread (it needs its message loop).
pub fn start(tx: Sender<Command>) -> HWINEVENTHOOK {
    if let Ok(mut t) = TX.lock() {
        *t = Some(tx);
    }
    unsafe {
        SetWinEventHook(
            EVENT_SYSTEM_FOREGROUND,
            EVENT_SYSTEM_FOREGROUND,
            None,
            Some(on_foreground),
            0,
            0,
            WINEVENT_OUTOFCONTEXT | WINEVENT_SKIPOWNPROCESS,
        )
    }
}

fn path_of(process: HANDLE) -> Option<String> {
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    unsafe {
        QueryFullProcessImageNameW(
            process,
            PROCESS_NAME_WIN32,
            PWSTR(buf.as_mut_ptr()),
            &mut len,
        )
        .ok()?;
    }
    Some(String::from_utf16_lossy(&buf[..len as usize]))
}

unsafe extern "system" fn on_foreground(
    _hook: HWINEVENTHOOK,
    _event: u32,
    hwnd: HWND,
    _object: i32,
    _child: i32,
    _thread: u32,
    _time: u32,
) {
    let mut pid = 0u32;
    unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid)) };
    if pid == 0 {
        return;
    }
    {
        let Ok(mut seen) = SEEN.lock() else { return };
        if seen.contains(&pid) {
            return;
        }
        seen.push(pid);
    }
    let Ok(process) = (unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            false,
            pid,
        )
    }) else {
        // Elevated or protected: ignored without notice (it stays in SEEN,
        // so it is not tried on every focus change).
        return;
    };
    let Some(path) = path_of(process) else {
        unsafe {
            let _ = CloseHandle(process);
        }
        return;
    };
    let watch = Box::into_raw(Box::new(Watch {
        pid,
        process,
        wait: Mutex::new(HANDLE::default()),
    }));
    // Held while registering: `on_exit` (another thread) takes this lock
    // before freeing `watch`, so it cannot run ahead of the line below.
    let Ok(mut slot) = unsafe { &*watch }.wait.lock() else {
        return;
    };
    let mut wait = HANDLE::default();
    let registered = unsafe {
        RegisterWaitForSingleObject(
            &mut wait,
            process,
            Some(on_exit),
            Some(watch as *const c_void),
            INFINITE,
            WT_EXECUTEONLYONCE,
        )
    };
    if registered.is_err() {
        drop(slot);
        unsafe {
            drop(Box::from_raw(watch));
            let _ = CloseHandle(process);
        }
        if let Ok(mut seen) = SEEN.lock() {
            seen.retain(|p| *p != pid);
        }
        return;
    }
    *slot = wait;
    drop(slot);
    send(Command::ProcessOpened(pid, path));
}

/// Runs on a thread-pool thread: only reports and frees.
unsafe extern "system" fn on_exit(context: *mut c_void, _timed_out: bool) {
    let watch = unsafe { Box::from_raw(context as *mut Watch) };
    // Waits until `on_foreground` stored the wait handle.
    drop(watch.wait.lock());
    send(Command::ProcessClosed(watch.pid));
    if let Ok(mut seen) = SEEN.lock() {
        seen.retain(|p| *p != watch.pid);
    }
    unsafe {
        if let Ok(w) = watch.wait.lock() {
            if !w.is_invalid() {
                let _ = UnregisterWait(*w);
            }
        }
        let _ = CloseHandle(watch.process);
    }
}
