//! Saved settings in `%APPDATA%\OpenPeripheral\config.toml`.
//!
//! Devices are identified by `vid:pid/model`; serial numbers, MAC addresses
//! and Bluetooth names are never stored.

use crate::devices::{Change, DeviceKey};
use crate::protocol::razer::Rgb;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::PathBuf;

/// Generous sanity bound; the device itself validates the exact range.
const MAX_DPI: u16 = 50_000;

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceSettings {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u16>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub color: Option<String>,
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
        out
    }

    pub fn record(&mut self, change: Change) {
        match change {
            Change::Dpi(d) => self.dpi = Some(d),
            Change::Color(c) => self.color = Some(to_hex(c)),
        }
    }

    pub fn is_empty(&self) -> bool {
        self.dpi.is_none() && self.color.is_none()
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
