//! Game library folders on this PC (Steam, Xbox, Epic, GOG), read once when
//! the worker starts: a library added later needs a restart (documented
//! limit). Everything read here is untrusted: files are size-capped and the
//! paths are only compared, never run or opened (`games.rs`).

use crate::games::{self, Libraries};
use std::path::Path;
use windows::Win32::System::Registry::{
    HKEY, HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE, KEY_READ, RRF_RT_REG_SZ, RegCloseKey,
    RegEnumKeyExW, RegGetValueW, RegOpenKeyExW,
};
use windows::core::{HSTRING, PCWSTR, PWSTR, w};

fn read_small(path: &Path) -> Option<String> {
    let m = std::fs::metadata(path).ok()?;
    (m.is_file() && m.len() <= games::MAX_FILE)
        .then(|| std::fs::read_to_string(path).ok())
        .flatten()
}

fn reg_string(root: HKEY, key: &str, value: PCWSTR) -> Option<String> {
    let mut buf = vec![0u16; 1024];
    let mut size = (buf.len() * 2) as u32;
    unsafe {
        RegGetValueW(
            root,
            &HSTRING::from(key),
            value,
            RRF_RT_REG_SZ,
            None,
            Some(buf.as_mut_ptr().cast()),
            Some(&mut size),
        )
        .ok()
        .ok()?;
    }
    let n = (size as usize / 2).saturating_sub(1).min(buf.len());
    Some(String::from_utf16_lossy(&buf[..n]))
}

/// Install folders of GOG games (`HKLM\SOFTWARE\WOW6432Node\GOG.com\Games`).
fn gog() -> Vec<String> {
    const GAMES: &str = "SOFTWARE\\WOW6432Node\\GOG.com\\Games";
    let mut out = Vec::new();
    unsafe {
        let mut key = HKEY::default();
        if RegOpenKeyExW(
            HKEY_LOCAL_MACHINE,
            &HSTRING::from(GAMES),
            None,
            KEY_READ,
            &mut key,
        )
        .is_err()
        {
            return out;
        }
        for i in 0..games::MAX_ROOTS as u32 {
            let mut name = [0u16; 256];
            let mut len = name.len() as u32;
            if RegEnumKeyExW(
                key,
                i,
                Some(PWSTR(name.as_mut_ptr())),
                &mut len,
                None,
                None,
                None,
                None,
            )
            .is_err()
            {
                break;
            }
            let sub = format!(
                "{GAMES}\\{}",
                String::from_utf16_lossy(&name[..len as usize])
            );
            if let Some(p) = reg_string(HKEY_LOCAL_MACHINE, &sub, w!("path")) {
                out.push(p);
            }
        }
        let _ = RegCloseKey(key);
    }
    out
}

/// Install folders of Epic games, from the launcher's manifests.
fn epic() -> Vec<String> {
    let Some(data) = std::env::var_os("ProgramData") else {
        return Vec::new();
    };
    let dir = Path::new(&data).join("Epic\\EpicGamesLauncher\\Data\\Manifests");
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .flatten()
        .map(|e| e.path())
        .filter(|p| {
            p.extension()
                .is_some_and(|x| x.eq_ignore_ascii_case("item"))
        })
        .take(games::MAX_ROOTS)
        .filter_map(|p| read_small(&p).and_then(|t| games::epic_install(&t)))
        .collect()
}

pub fn load() -> Libraries {
    let mut steam = Vec::new();
    if let Some(root) = reg_string(HKEY_CURRENT_USER, "Software\\Valve\\Steam", w!("SteamPath")) {
        let vdf = Path::new(&root).join("steamapps\\libraryfolders.vdf");
        steam.push(root);
        if let Some(text) = read_small(&vdf) {
            steam.extend(games::vdf_paths(&text));
        }
    }
    let mut other: Vec<String> = ('C'..='Z')
        .map(|d| format!("{d}:\\XboxGames"))
        .filter(|p| Path::new(p).is_dir())
        .collect();
    other.extend(epic());
    other.extend(gog());
    Libraries::new(&steam, &other)
}
