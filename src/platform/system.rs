//! Small Windows helpers: running vendor software, autostart, single instance.

use windows::Win32::Foundation::{CloseHandle, ERROR_ALREADY_EXISTS, GetLastError, HANDLE};
use windows::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, PROCESSENTRY32W, Process32FirstW, Process32NextW, TH32CS_SNAPPROCESS,
};
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, KEY_QUERY_VALUE, KEY_SET_VALUE, REG_SZ, RegCloseKey, RegDeleteValueW,
    RegOpenKeyExW, RegQueryValueExW, RegSetValueExW,
};
use windows::Win32::System::Threading::CreateMutexW;
use windows::core::{HSTRING, PCWSTR, w};

/// Process name prefixes (lower case) of software known to drive the same
/// devices. Only used to warn the user; nothing is ever stopped.
const KNOWN: &[(&str, &str)] = &[
    ("lghub", "Logitech G HUB"),
    ("logioptionsplus", "Logi Options+"),
    ("razer", "Razer Synapse"),
    ("openlogi", "OpenLogi"),
    ("steam.exe", "Steam (Steam Input)"),
    ("ds4windows", "DS4Windows"),
    ("openrgb", "OpenRGB"),
    ("signalrgb", "SignalRGB"),
];

pub fn match_known(process: &str) -> Option<&'static str> {
    let p = process.to_ascii_lowercase();
    KNOWN
        .iter()
        .find(|(prefix, _)| p.starts_with(prefix))
        .map(|&(_, name)| name)
}

pub fn running_conflicts() -> Vec<&'static str> {
    let mut found = Vec::new();
    unsafe {
        let Ok(snap) = CreateToolhelp32Snapshot(TH32CS_SNAPPROCESS, 0) else {
            return found;
        };
        let mut e = PROCESSENTRY32W {
            dwSize: size_of::<PROCESSENTRY32W>() as u32,
            ..Default::default()
        };
        let mut more = Process32FirstW(snap, &mut e).is_ok();
        while more {
            let len = e
                .szExeFile
                .iter()
                .position(|&c| c == 0)
                .unwrap_or(e.szExeFile.len());
            let name = String::from_utf16_lossy(&e.szExeFile[..len]);
            if let Some(n) = match_known(&name) {
                if !found.contains(&n) {
                    found.push(n);
                }
            }
            more = Process32NextW(snap, &mut e).is_ok();
        }
        let _ = CloseHandle(snap);
    }
    found.sort_unstable();
    found
}

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const RUN_VALUE: PCWSTR = w!("OpenPeripheral");

fn run_key(access: windows::Win32::System::Registry::REG_SAM_FLAGS) -> Option<HKEY> {
    let mut key = HKEY::default();
    unsafe {
        RegOpenKeyExW(HKEY_CURRENT_USER, RUN_KEY, None, access, &mut key)
            .is_ok()
            .then_some(key)
    }
}

pub fn autostart_enabled() -> bool {
    let Some(key) = run_key(KEY_QUERY_VALUE) else {
        return false;
    };
    let found = unsafe { RegQueryValueExW(key, RUN_VALUE, None, None, None, None).is_ok() };
    unsafe {
        let _ = RegCloseKey(key);
    }
    found
}

/// Adds or removes the per-user autostart entry (no administrator needed).
pub fn set_autostart(enable: bool) -> std::io::Result<()> {
    let key = run_key(KEY_SET_VALUE)
        .ok_or_else(|| std::io::Error::other("no se pudo abrir la clave Run"))?;
    let result = if enable {
        let exe = std::env::current_exe()?;
        let cmd: Vec<u16> = format!("\"{}\"", exe.display())
            .encode_utf16()
            .chain([0])
            .collect();
        let bytes = unsafe { std::slice::from_raw_parts(cmd.as_ptr().cast::<u8>(), cmd.len() * 2) };
        unsafe { RegSetValueExW(key, RUN_VALUE, None, REG_SZ, Some(bytes)) }
    } else {
        unsafe { RegDeleteValueW(key, RUN_VALUE) }
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    result.ok().map_err(|e| std::io::Error::other(e.message()))
}

/// Returns `None` if another instance already holds the mutex. The handle is
/// kept for the life of the process.
pub fn single_instance() -> Option<HANDLE> {
    unsafe {
        let h = CreateMutexW(None, true, &HSTRING::from("Local\\OpenPeripheral.Instance")).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(h);
            return None;
        }
        Some(h)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn known_process_names() {
        assert_eq!(match_known("lghub_agent.exe"), Some("Logitech G HUB"));
        assert_eq!(match_known("RazerAppEngine.exe"), Some("Razer Synapse"));
        assert_eq!(match_known("steam.exe"), Some("Steam (Steam Input)"));
        assert_eq!(match_known("steamwebhelper.exe"), None);
        assert_eq!(match_known("explorer.exe"), None);
    }
}
