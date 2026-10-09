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
    ("steelseriesgg", "SteelSeries GG"),
    ("armourycrate", "Armoury Crate"),
    ("openlogi", "OpenLogi"),
    ("steam.exe", "Steam (Steam Input)"),
    ("ds4windows", "DS4Windows"),
    ("openrgb", "OpenRGB"),
    ("signalrgb", "SignalRGB"),
];

/// Background services of that software that never touch the devices; the
/// G HUB updater keeps running after G HUB is closed.
const IGNORED: &[&str] = &["lghub_updater"];

pub fn match_known(process: &str) -> Option<&'static str> {
    let p = process.to_ascii_lowercase();
    if IGNORED.iter().any(|i| p.starts_with(i)) {
        return None;
    }
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
        assert_eq!(match_known("lghub.exe"), Some("Logitech G HUB"));
        assert_eq!(match_known("lghub_updater.exe"), None);
        assert_eq!(match_known("RazerAppEngine.exe"), Some("Razer Synapse"));
        assert_eq!(match_known("steam.exe"), Some("Steam (Steam Input)"));
        assert_eq!(match_known("SteelSeriesGG.exe"), Some("SteelSeries GG"));
        assert_eq!(match_known("ArmouryCrate.exe"), Some("Armoury Crate"));
        assert_eq!(match_known("steamwebhelper.exe"), None);
        assert_eq!(match_known("explorer.exe"), None);
    }
}

/// Windows pointer speed (1–20; Windows' default is 10). It applies to
/// every mouse at once and scales the pointer movement, not the sensor's
/// DPI; games that read raw input do not use it.
pub const POINTER_SPEED: std::ops::RangeInclusive<u32> = 1..=20;
pub const POINTER_SPEED_DEFAULT: u32 = 10;

pub fn pointer_speed() -> Option<u32> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_GETMOUSESPEED, SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS, SystemParametersInfoW,
    };
    let mut speed = 0u32;
    unsafe {
        SystemParametersInfoW(
            SPI_GETMOUSESPEED,
            0,
            Some(&mut speed as *mut u32 as *mut _),
            SYSTEM_PARAMETERS_INFO_UPDATE_FLAGS(0),
        )
    }
    .ok()?;
    POINTER_SPEED.contains(&speed).then_some(speed)
}

/// Sets it as the Windows mouse settings do (saved for this user).
pub fn set_pointer_speed(speed: u32) -> windows::core::Result<()> {
    use windows::Win32::UI::WindowsAndMessaging::{
        SPI_SETMOUSESPEED, SPIF_SENDCHANGE, SPIF_UPDATEINIFILE, SystemParametersInfoW,
    };
    let speed = speed.clamp(*POINTER_SPEED.start(), *POINTER_SPEED.end());
    // The value goes in the pointer argument itself.
    unsafe {
        SystemParametersInfoW(
            SPI_SETMOUSESPEED,
            0,
            Some(speed as usize as *mut _),
            SPIF_UPDATEINIFILE | SPIF_SENDCHANGE,
        )
    }
}

#[cfg(test)]
mod pointer_tests {
    #[test]
    fn the_pointer_speed_is_read_in_range() {
        // Reading only: tests never change the user's Windows settings.
        if let Some(s) = super::pointer_speed() {
            assert!(super::POINTER_SPEED.contains(&s));
        }
    }
}
