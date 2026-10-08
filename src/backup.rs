//! Local copy of a mouse's onboard-profile memory, taken before this app
//! ever writes to it. Stored in `%APPDATA%\OpenPeripheral\backups\` as
//! readable TOML (hex sectors). Identifies the device only by
//! `vid:pid/model`; no serial numbers. The "original" copy is created once
//! and never overwritten.

use crate::devices::{DeviceKey, ProfileDump};
use crate::protocol::onboard::{self, Info};
use std::path::PathBuf;

const FORMAT: u32 = 1;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Backup {
    pub device: String,
    /// Local date when the copy was taken, `AAAA-MM-DD`.
    pub created: String,
    pub dump: ProfileDump,
}

fn hex(data: &[u8]) -> String {
    data.iter().map(|b| format!("{b:02x}")).collect()
}

fn unhex(s: &str) -> Option<Vec<u8>> {
    if s.len() % 2 != 0 {
        return None;
    }
    (0..s.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(s.get(i..i + 2)?, 16).ok())
        .collect()
}

pub fn to_text(b: &Backup) -> String {
    let d = &b.dump;
    format!(
        "# Copia del perfil del ratón hecha por OpenPeripheral antes de modificarlo.\n\
         # No contiene números de serie ni datos personales.\n\
         format = {FORMAT}\n\
         device = {device:?}\n\
         created = {created:?}\n\
         profile_format = {pf}\n\
         profile_count = {pc}\n\
         button_count = {bc}\n\
         sector_size = {ss}\n\
         sector = {sector}\n\
         directory = \"{dir}\"\n\
         user = \"{user}\"\n\
         rom = \"{rom}\"\n",
        device = b.device,
        created = b.created,
        pf = d.info.profile_format,
        pc = d.info.profile_count,
        bc = d.info.button_count,
        ss = d.info.sector_size,
        sector = d.sector,
        dir = hex(&d.directory),
        user = hex(&d.user),
        rom = hex(&d.rom),
    )
}

/// Parses and validates a backup: every sector has the declared size and
/// the directory and user profile CRCs match. A tampered or truncated file
/// is rejected, so it can never be written back to a device.
pub fn from_text(text: &str) -> Result<Backup, String> {
    #[derive(serde::Deserialize)]
    struct Raw {
        format: u32,
        device: String,
        created: String,
        profile_format: u8,
        profile_count: u8,
        button_count: u8,
        sector_size: u16,
        sector: u16,
        directory: String,
        user: String,
        rom: String,
    }
    let r: Raw = toml::from_str(text).map_err(|_| "formato no válido".to_string())?;
    if r.format != FORMAT {
        return Err(format!("versión de copia {} no admitida", r.format));
    }
    let size = r.sector_size as usize;
    let sector = |name: &str, s: &str| -> Result<Vec<u8>, String> {
        let v = unhex(s).ok_or(format!("{name}: hexadecimal no válido"))?;
        (v.len() == size)
            .then_some(v)
            .ok_or(format!("{name}: tamaño incorrecto"))
    };
    let dump = ProfileDump {
        info: Info {
            profile_format: r.profile_format,
            profile_count: r.profile_count,
            button_count: r.button_count,
            sector_size: r.sector_size,
        },
        sector: r.sector,
        directory: sector("directory", &r.directory)?,
        user: sector("user", &r.user)?,
        rom: sector("rom", &r.rom)?,
        // Limits are read from the device, never from a file.
        dpi: None,
        effects: Vec::new(),
    };
    if !onboard::crc_ok(&dump.directory) || !onboard::crc_ok(&dump.user) {
        return Err("la suma de comprobación no coincide".into());
    }
    Ok(Backup {
        device: r.device,
        created: r.created,
        dump,
    })
}

/// "046d:c53f/G305 Lightspeed …" -> "046d-c53f-g305-lightspeed-…".
fn file_stem(key: &DeviceKey) -> String {
    let mut out = String::new();
    for c in key.0.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
        } else if !out.ends_with('-') {
            out.push('-');
        }
    }
    out.trim_matches('-').chars().take(80).collect()
}

fn dir() -> Option<PathBuf> {
    Some(
        PathBuf::from(std::env::var_os("APPDATA")?)
            .join("OpenPeripheral")
            .join("backups"),
    )
}

fn original_path(key: &DeviceKey) -> Option<PathBuf> {
    Some(dir()?.join(format!("{}-original.toml", file_stem(key))))
}

/// The original copy for `key`, if one exists and is valid.
pub fn load_original(key: &DeviceKey) -> Result<Option<Backup>, String> {
    let Some(path) = original_path(key) else {
        return Ok(None);
    };
    match std::fs::read_to_string(&path) {
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e.to_string()),
        Ok(text) => from_text(&text).map(Some),
    }
}

/// Stores the original copy once. If it already exists it is returned
/// unchanged: the first copy is never overwritten.
pub fn save_original(key: &DeviceKey, dump: &ProfileDump, today: &str) -> Result<Backup, String> {
    if let Some(existing) = load_original(key)? {
        return Ok(existing);
    }
    let path = original_path(key).ok_or("APPDATA no definido")?;
    let backup = Backup {
        device: key.0.clone(),
        created: today.into(),
        dump: dump.clone(),
    };
    let text = to_text(&backup);
    // Parse what will be written before writing it.
    if from_text(&text).as_ref() != Ok(&backup) {
        return Err("la copia no se pudo validar".into());
    }
    let dir = path.parent().ok_or("ruta no válida")?;
    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    let tmp = path.with_extension("toml.tmp");
    std::fs::write(&tmp, &text).map_err(|e| e.to_string())?;
    // hard_link fails if the target exists: creation is atomic and exclusive.
    let linked = std::fs::hard_link(&tmp, &path);
    let _ = std::fs::remove_file(&tmp);
    linked.map_err(|e| e.to_string())?;
    // Read back from disk and compare.
    match load_original(key)? {
        Some(b) if b == backup => Ok(b),
        _ => Err("la copia guardada no coincide con lo leído".into()),
    }
}

/// Today's local date as `AAAA-MM-DD`.
pub fn today() -> String {
    let t = unsafe { windows::Win32::System::SystemInformation::GetLocalTime() };
    format!("{:04}-{:02}-{:02}", t.wYear, t.wMonth, t.wDay)
}

/// "2026-10-06" -> "6/10/2026" for display.
pub fn display_date(iso: &str) -> String {
    let parts: Vec<&str> = iso.split('-').collect();
    match parts[..] {
        [y, m, d] => format!(
            "{}/{}/{y}",
            d.trim_start_matches('0'),
            m.trim_start_matches('0')
        ),
        _ => iso.into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::onboard::samples;

    fn dump() -> ProfileDump {
        ProfileDump {
            info: Info {
                profile_format: 3,
                profile_count: 1,
                button_count: 6,
                sector_size: 255,
            },
            sector: 1,
            directory: samples::directory(),
            user: samples::user_profile(),
            rom: samples::rom_profile(),
            dpi: None,
            effects: Vec::new(),
        }
    }

    fn backup() -> Backup {
        Backup {
            device: "046d:c53f/G305".into(),
            created: "2026-10-06".into(),
            dump: dump(),
        }
    }

    #[test]
    fn text_round_trip_is_exact() {
        let b = backup();
        let text = to_text(&b);
        assert!(text.contains("device = \"046d:c53f/G305\""));
        assert!(!text.to_lowercase().contains("serial"));
        assert_eq!(from_text(&text).unwrap(), b);
    }

    #[test]
    fn tampered_or_truncated_copies_are_rejected() {
        let text = to_text(&backup());
        // One changed hex digit in the user profile breaks its CRC.
        let user_at = text.find("user = \"").unwrap() + 8 + 0x2f * 2;
        let mut bad = text.clone();
        let flipped = if &bad[user_at..user_at + 1] == "0" {
            "1"
        } else {
            "0"
        };
        bad.replace_range(user_at..user_at + 1, flipped);
        assert!(from_text(&bad).is_err());
        // A shorter sector.
        let short = text.replacen("user = \"", "user = \"00", 1);
        assert!(from_text(&short).is_err());
        assert!(from_text("format = 2\n").is_err());
        assert!(from_text("not toml [").is_err());
    }

    #[test]
    fn file_names_have_no_spaces_or_slashes() {
        assert_eq!(
            file_stem(&DeviceKey(
                "046d:c53f/G305 Lightspeed Wireless Gaming Mouse".into()
            )),
            "046d-c53f-g305-lightspeed-wireless-gaming-mouse"
        );
        assert_eq!(display_date("2026-10-06"), "6/10/2026");
    }
}
