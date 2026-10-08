//! Saved settings in `%APPDATA%\OpenPeripheral\config.toml`.
//!
//! Devices are identified by `vid:pid/model`; serial numbers, MAC addresses
//! and Bluetooth names are never stored.

use crate::devices::{Change, DeviceKey};
use crate::games::{self, Program};
use crate::profiles::{self, MouseProfile};
use crate::protocol::onboard::Light;
use crate::protocol::razer::Rgb;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Generous sanity bound; the device itself validates the exact range.
const MAX_DPI: u16 = 50_000;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
    /// Mouse light: "apagada", "fija #RRGGBB", "ciclo <ms>" or
    /// "respiracion #RRGGBB <ms>".
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<String>,
    /// Named mouse profiles (`profiles.rs`). With profiles, `dpi` and
    /// `light` mirror the active one, so an older version of the app still
    /// applies the right DPI and light.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub profiles: Vec<MouseProfile>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub active: Option<String>,
    /// Programs associated with a profile (`games.rs`), by full path.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub programs: Vec<Program>,
    /// «Cambiar de perfil al abrir estos programas» (on by default).
    #[serde(default = "on", skip_serializing_if = "is_on")]
    pub auto_switch: bool,
    /// «Preguntar al detectar un juego nuevo» (on by default).
    #[serde(default = "on", skip_serializing_if = "is_on")]
    pub ask_new: bool,
}

fn on() -> bool {
    true
}

fn is_on(v: &bool) -> bool {
    *v
}

impl Default for DeviceSettings {
    fn default() -> Self {
        Self {
            dpi: None,
            color: None,
            light: None,
            profiles: Vec::new(),
            active: None,
            programs: Vec::new(),
            auto_switch: true,
            ask_new: true,
        }
    }
}

impl DeviceSettings {
    /// Changes to re-apply, dropping any value that is out of bounds.
    pub fn changes(&self) -> Vec<Change> {
        let mut out = Vec::new();
        if let Some(dpi) = self.dpi.filter(|d| (1..=MAX_DPI).contains(d)) {
            out.push(Change::Dpi(dpi));
        }
        if let Some(c) = self.color.as_deref().and_then(parse_hex) {
            out.push(Change::Color(c));
        }
        if let Some(l) = self.light.as_deref().and_then(parse_light) {
            out.push(Change::Light(l));
        }
        out
    }

    pub fn record(&mut self, change: Change) {
        match change {
            Change::Dpi(d) => self.dpi = Some(d),
            Change::Color(c) => self.color = Some(to_hex(c)),
            Change::Light(l) => self.light = Some(light_text(l)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.dpi.is_none()
            && self.color.is_none()
            && self.light.is_none()
            && self.profiles.is_empty()
            && self.programs.is_empty()
            && self.auto_switch
            && self.ask_new
    }

    pub fn active_profile(&self) -> Option<&MouseProfile> {
        let name = self.active.as_deref()?;
        self.profiles.iter().find(|p| p.name == name)
    }

    pub fn active_profile_mut(&mut self) -> Option<&mut MouseProfile> {
        let name = self.active.clone()?;
        self.profiles.iter_mut().find(|p| p.name == name)
    }

    /// Makes `name` the active profile and mirrors its DPI and light (what
    /// the app applies when the mouse connects). False if there is none.
    pub fn activate(&mut self, name: &str) -> bool {
        let Some(p) = self.profiles.iter().find(|p| p.name == name) else {
            return false;
        };
        self.dpi = p.dpi;
        self.light = p.light.clone();
        self.active = Some(p.name.clone());
        true
    }

    /// Mirrors the active profile again after it changed.
    pub fn sync_active(&mut self) {
        if let Some(name) = self.active.clone() {
            self.activate(&name);
        }
    }
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Config {
    #[serde(default)]
    pub devices: BTreeMap<String, DeviceSettings>,
}

impl Config {
    pub fn get(&self, key: &DeviceKey) -> Option<&DeviceSettings> {
        self.devices.get(&key.0)
    }

    pub fn record(&mut self, key: &DeviceKey, change: Change) {
        self.devices
            .entry(key.0.clone())
            .or_default()
            .record(change);
    }

    pub fn forget(&mut self, key: &DeviceKey) {
        self.devices.remove(&key.0);
    }

    pub fn parse(text: &str) -> Option<Self> {
        toml::from_str(text).ok()
    }

    pub fn settings_mut(&mut self, key: &DeviceKey) -> &mut DeviceSettings {
        self.devices.entry(key.0.clone()).or_default()
    }

    /// Drops invalid profiles (see `profiles::sanitize`); returns how many.
    pub fn sanitize(&mut self) -> usize {
        self.devices
            .values_mut()
            .map(|s| {
                let n = profiles::sanitize(&mut s.profiles, &mut s.active);
                if n > 0 {
                    s.sync_active();
                }
                n + games::sanitize(&mut s.programs, &s.profiles)
            })
            .sum()
    }

    pub fn to_text(&self) -> String {
        let mut c = self.clone();
        c.devices.retain(|_, s| !s.is_empty());
        toml::to_string(&c).unwrap_or_default()
    }

    fn path() -> Option<PathBuf> {
        let base = std::env::var_os("APPDATA")?;
        Some(
            PathBuf::from(base)
                .join("OpenPeripheral")
                .join("config.toml"),
        )
    }

    /// Missing file means defaults. An unreadable file is moved aside to
    /// `config.toml.bad` so it is not silently overwritten.
    pub fn load() -> Self {
        let Some(path) = Self::path() else {
            return Self::default();
        };
        // A config this large was not written by the app: not read.
        if std::fs::metadata(&path).is_ok_and(|m| m.len() > games::MAX_FILE) {
            let _ = std::fs::rename(&path, path.with_extension("toml.bad"));
            return Self::default();
        }
        let parsed = match std::fs::read_to_string(&path) {
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Self::default(),
            Ok(text) => Self::parse(&text),
            // Invalid UTF-8, sharing violation...: treat like a corrupt file.
            Err(_) => None,
        };
        parsed.unwrap_or_else(|| {
            let _ = std::fs::rename(&path, path.with_extension("toml.bad"));
            Self::default()
        })
    }

    pub fn save(&self) -> std::io::Result<()> {
        let path = Self::path().ok_or_else(|| std::io::Error::other("APPDATA no definido"))?;
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Write then rename so a crash never leaves a half-written file.
        let tmp = path.with_extension("toml.tmp");
        std::fs::write(&tmp, self.to_text())?;
        std::fs::rename(tmp, path)
    }
}

pub fn parse_hex(s: &str) -> Option<Rgb> {
    let s = s.trim();
    let s = s.strip_prefix('#').unwrap_or(s);
    if s.len() != 6 || !s.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(s, 16).ok()?;
    Some(Rgb {
        r: (v >> 16) as u8,
        g: (v >> 8) as u8,
        b: v as u8,
    })
}

pub fn to_hex(c: Rgb) -> String {
    format!("#{:02X}{:02X}{:02X}", c.r, c.g, c.b)
}

pub fn light_text(l: Light) -> String {
    match l {
        Light::Off => "apagada".into(),
        Light::Fixed(c) => format!("fija {}", to_hex(c)),
        Light::Cycle { period_ms } => format!("ciclo {period_ms}"),
        Light::Breathing { color, period_ms } => {
            format!("respiracion {} {period_ms}", to_hex(color))
        }
    }
}

/// Only lights that could have been saved (speeds from the closed tables).
pub fn parse_light(s: &str) -> Option<Light> {
    let parts: Vec<&str> = s.split_whitespace().collect();
    let light = match parts.as_slice() {
        ["apagada"] => Light::Off,
        ["fija", c] => Light::Fixed(parse_hex(c)?),
        ["ciclo", ms] => Light::Cycle {
            period_ms: ms.parse().ok()?,
        },
        ["respiracion", c, ms] => Light::Breathing {
            color: parse_hex(c)?,
            period_ms: ms.parse().ok()?,
        },
        _ => return None,
    };
    light.writable().then_some(light)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_text_round_trip() {
        let red = Rgb { r: 255, g: 0, b: 0 };
        for l in [
            Light::Off,
            Light::Fixed(red),
            Light::Cycle { period_ms: 8000 },
            Light::Breathing {
                color: red,
                period_ms: 6000,
            },
        ] {
            assert_eq!(parse_light(&light_text(l)), Some(l));
        }
        for bad in [
            "",
            "fija",
            "fija #12345",
            "ciclo 5000",
            "respiracion #FF0000",
            "azul",
        ] {
            assert_eq!(parse_light(bad), None, "{bad}");
        }
        // An old config without `light` still parses, and a new one round-trips.
        let old = Config::parse("[devices.\"046d:c53f/G305\"]\ndpi = 900\n").unwrap();
        assert_eq!(old.devices.values().next().unwrap().light, None);
        let mut c = Config::default();
        let k = DeviceKey("046d:c53f/G305".into());
        c.record(&k, Change::Light(Light::Fixed(red)));
        let back = Config::parse(&c.to_text()).unwrap();
        assert_eq!(
            back.get(&k).unwrap().changes(),
            vec![Change::Light(Light::Fixed(red))]
        );
    }

    #[test]
    fn hex_round_trip() {
        assert_eq!(
            parse_hex("#ff8000"),
            Some(Rgb {
                r: 255,
                g: 128,
                b: 0
            })
        );
        assert_eq!(parse_hex(" 00FF00 "), Some(Rgb { r: 0, g: 255, b: 0 }));
        assert_eq!(to_hex(Rgb { r: 1, g: 2, b: 255 }), "#0102FF");
        for bad in ["#fff", "#gg0000", "#ff00ñ", "+12345", "", "#1234567"] {
            assert_eq!(parse_hex(bad), None, "{bad}");
        }
    }

    #[test]
    fn round_trip_and_validation() {
        let key = DeviceKey("046d:c53f/G305".into());
        let mut c = Config::default();
        c.record(&key, Change::Dpi(1600));
        c.record(&key, Change::Color(Rgb { r: 255, g: 0, b: 0 }));
        let text = c.to_text();
        assert!(text.contains("dpi = 1600"));
        let back = Config::parse(&text).unwrap();
        assert_eq!(back, c);
        assert_eq!(back.get(&key).unwrap().changes().len(), 2);

        // Out-of-range or malformed values are ignored, not applied.
        let bad = Config::parse("[devices.\"x\"]\ndpi = 0\ncolor = \"red\"\n").unwrap();
        assert!(bad.devices["x"].changes().is_empty());
        assert!(
            Config::parse("[devices.\"x\"]\ndpi = 70000\n")
                .is_none_or(|c| c.devices["x"].changes().is_empty())
        );

        assert_eq!(Config::parse("not toml ["), None);
        assert_eq!(Config::parse("").unwrap(), Config::default());

        c.forget(&key);
        assert_eq!(c.to_text().trim(), "[devices]");
    }

    fn profile(name: &str, dpi: u16, light: &str) -> MouseProfile {
        MouseProfile {
            name: name.into(),
            dpi: Some(dpi),
            light: Some(light.into()),
            buttons: vec!["3:80020106".into()],
        }
    }

    #[test]
    fn profiles_round_trip_and_mirror_the_active_one() {
        let key = DeviceKey("046d:c53f/G305".into());
        let mut c = Config::default();
        let s = c.settings_mut(&key);
        s.profiles = vec![
            profile("Trabajo", 1000, "fija #FFFFFF"),
            profile("Juego", 1600, "respiracion #FF0000 6000"),
        ];
        assert!(s.activate("Juego"));
        assert!(!s.activate("Nadie"));
        let back = Config::parse(&c.to_text()).unwrap();
        assert_eq!(back, c);
        let s = back.get(&key).unwrap();
        assert_eq!(s.active_profile().unwrap().name, "Juego");
        // The mirror is what an older version (and `reapply`) applies.
        assert_eq!(s.dpi, Some(1600));
        assert_eq!(
            s.changes(),
            vec![
                Change::Dpi(1600),
                Change::Light(parse_light("respiracion #FF0000 6000").unwrap())
            ]
        );
        // A device holding only profiles is not dropped when saving.
        let mut only = Config::default();
        only.settings_mut(&key).profiles = vec![profile("Trabajo", 1000, "apagada")];
        assert!(only.to_text().contains("Trabajo"));
    }

    #[test]
    fn programs_and_switches_round_trip() {
        let key = DeviceKey("k".into());
        let mut c = Config::default();
        let s = c.settings_mut(&key);
        s.profiles = vec![profile("Juego", 1600, "apagada")];
        s.programs = vec![Program {
            exe: r"g:\steamlibrary\steamapps\common\peak\peak.exe".into(),
            profile: Some("Juego".into()),
        }];
        s.ask_new = false;
        let text = c.to_text();
        assert!(text.contains("ask_new = false") && !text.contains("auto_switch"));
        assert_eq!(Config::parse(&text).unwrap(), c);
        // Defaults: both switches on; a device with only programs is kept.
        let d = Config::parse("[devices.k]\ndpi = 800\n").unwrap();
        assert!(d.devices["k"].auto_switch && d.devices["k"].ask_new);
        let mut only = Config::default();
        only.settings_mut(&key).programs = c.devices["k"].programs.clone();
        assert!(only.to_text().contains("peak.exe"));
        // A program of a profile that no longer exists: «No cambiar».
        let mut gone = c.clone();
        gone.settings_mut(&key).profiles.clear();
        gone.sanitize();
        assert_eq!(gone.devices["k"].programs[0].profile, None);
    }

    #[test]
    fn a_config_without_profiles_still_loads_and_bad_profiles_are_dropped() {
        let old = "[devices.\"046d:c53f/G305\"]
dpi = 900
light = \"fija #00C8FF\"
";
        let c = Config::parse(old).unwrap();
        let s = c.devices.values().next().unwrap();
        assert!(s.profiles.is_empty() && s.active.is_none());
        assert_eq!(s.dpi, Some(900));

        let edited = "[devices.k]
active = \"Malo\"
dpi = 300
            [[devices.k.profiles]]
name = \"Malo\"
dpi = 0
            [[devices.k.profiles]]
name = \"Bueno\"
dpi = 800
";
        let mut c = Config::parse(edited).unwrap();
        assert_eq!(c.sanitize(), 1);
        let s = &c.devices["k"];
        assert_eq!(s.active.as_deref(), Some("Bueno"));
        assert_eq!(s.dpi, Some(800));
    }
}
