//! ASUS ROG / TUF mice: the DPI of the active preset (pure, no I/O).
//!
//! Ported from libratbag's ASUS driver (`src/asus.c`, `src/asus.h`), which
//! carries this notice:
//!
//! Copyright (C) 2021 Kyoken, kyoken@kyoken.ninja
//!
//! Permission is hereby granted, free of charge, to any person obtaining a
//! copy of this software and associated documentation files (the
//! "Software"), to deal in the Software without restriction, including
//! without limitation the rights to use, copy, modify, merge, publish,
//! distribute, sublicense, and/or sell copies of the Software, and to permit
//! persons to whom the Software is furnished to do so, subject to the
//! following conditions:
//!
//! The above copyright notice and this permission notice (including the
//! next paragraph) shall be included in all copies or substantial portions
//! of the Software.
//!
//! THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF ANY KIND, EXPRESS
//! OR IMPLIED, INCLUDING BUT NOT LIMITED TO THE WARRANTIES OF
//! MERCHANTABILITY, FITNESS FOR A PARTICULAR PURPOSE AND NONINFRINGEMENT. IN
//! NO EVENT SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY CLAIM,
//! DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION OF CONTRACT, TORT OR
//! OTHERWISE, ARISING FROM, OUT OF OR IN CONNECTION WITH THE SOFTWARE OR THE
//! USE OR OTHER DEALINGS IN THE SOFTWARE.
//!
//! Only the profile and settings reads and the DPI setting are used. The
//! "save" command (0x0350) is never sent: the DPI is lost when the mouse is
//! unplugged (volatile).

#[path = "asus_data.rs"]
mod data;
pub use data::MODELS;

/// 64-byte reports without report id (hidapi needs a leading 0).
pub const PACKET: usize = 64;
const GET_PROFILE_DATA: u16 = 0x0012;
const GET_SETTINGS: u16 = 0x0412;
const SET_SETTING: u16 = 0x3151;
/// Answer of a mouse asleep, disconnected or refusing the request.
const STATUS_ERROR: u16 = 0xaaff;

#[derive(Debug, PartialEq, Eq)]
pub struct Model {
    pub name: &'static str,
    pub pid: u16,
    /// DPI presets (2 or 4); the settings fields after them are the rate,
    /// response and snapping.
    pub presets: usize,
    /// Accepted DPI: (min, max, step).
    pub dpi: (u16, u16, u16),
    /// The sensor counts half the DPI (quirk DOUBLE_DPI).
    pub double_dpi: bool,
    /// Newer settings layout (quirk SETTINGS_V2): no DPI doubling.
    pub settings_v2: bool,
}

pub fn model(pid: u16) -> Option<&'static Model> {
    MODELS.iter().find(|m| m.pid == pid)
}

fn request(cmd: u16, params: &[u8]) -> [u8; PACKET + 1] {
    let mut r = [0u8; PACKET + 1];
    r[1..3].copy_from_slice(&cmd.to_le_bytes());
    r[3..3 + params.len()].copy_from_slice(params);
    r
}

/// Read only: the active profile and DPI preset.
pub fn get_profile_data() -> [u8; PACKET + 1] {
    request(GET_PROFILE_DATA, &[])
}

/// Read only: the DPI presets and the other settings.
pub fn get_settings() -> [u8; PACKET + 1] {
    request(GET_SETTINGS, &[0])
}

/// `None` for an error answer (asleep, refused) or a short one.
pub fn checked(reply: &[u8]) -> Option<&[u8]> {
    let code = u16::from_le_bytes([*reply.first()?, *reply.get(1)?]);
    (code != STATUS_ERROR && reply.len() >= 12).then_some(reply)
}

/// Active DPI preset (0-based) of a `get_profile_data` reply.
pub fn active_preset(m: &Model, reply: &[u8]) -> Option<usize> {
    let p = *checked(reply)?.get(11)? as usize;
    (1..=m.presets).contains(&p).then(|| p - 1)
}

impl Model {
    pub fn range(&self) -> super::DpiRange {
        let (min, max, step) = self.dpi;
        super::DpiRange::Range { min, max, step }
    }

    fn doubled(&self) -> bool {
        self.double_dpi && !self.settings_v2
    }

    /// DPI of `preset` in a `get_settings` reply.
    pub fn preset_dpi(&self, reply: &[u8], preset: usize) -> Option<u16> {
        let at = 4 + 2 * preset;
        let raw = checked(reply)?.get(at..at + 2)?;
        let dpi = u16::from_le_bytes([raw[0], raw[1]])
            .checked_mul(50)?
            .checked_add(50)?;
        let dpi = if self.doubled() {
            dpi.checked_mul(2)?
        } else {
            dpi
        };
        (preset < self.presets).then_some(dpi)
    }

    /// Sets `preset` to `dpi`; `None` for a DPI the model does not accept.
    pub fn set_dpi(&self, preset: usize, dpi: u16) -> Option<[u8; PACKET + 1]> {
        if preset >= self.presets || !self.range().contains(dpi) {
            return None;
        }
        let sensor = if self.doubled() { dpi / 2 } else { dpi };
        let [lo, hi] = ((sensor - 50) / 50).to_le_bytes();
        Some(request(SET_SETTING, &[preset as u8, 0, lo, hi]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_and_replies_follow_libratbag() {
        let tuf = model(0x1a9b).unwrap();
        assert_eq!(tuf.presets, 4);
        // Report id 0, command little-endian, preset, 0, DPI code.
        assert_eq!(
            &tuf.set_dpi(2, 1600).unwrap()[..7],
            &[0, 0x51, 0x31, 2, 0, 31, 0]
        );
        assert_eq!(tuf.set_dpi(4, 1600), None, "no such preset");
        assert_eq!(tuf.set_dpi(0, 1650), None, "not a step");
        assert_eq!(&get_profile_data()[..3], &[0, 0x12, 0x00]);
        assert_eq!(&get_settings()[..4], &[0, 0x12, 0x04, 0]);
        // DOUBLE_DPI: the sensor gets half.
        let chakram = model(0x18e3).unwrap();
        assert!(chakram.double_dpi);
        assert_eq!(&chakram.set_dpi(0, 1600).unwrap()[3..7], &[0, 0, 15, 0]);
        let mut reply = [0u8; PACKET];
        reply[4 + 2] = 15; // preset 1: 15 * 50 + 50 = 800, doubled
        assert_eq!(chakram.preset_dpi(&reply, 1), Some(1600));
        assert_eq!(tuf.preset_dpi(&reply, 1), Some(800));
        reply[11] = 2;
        assert_eq!(active_preset(tuf, &reply), Some(1));
        reply[11] = 0;
        assert_eq!(active_preset(tuf, &reply), None);
        reply[..2].copy_from_slice(&STATUS_ERROR.to_le_bytes());
        assert_eq!(tuf.preset_dpi(&reply, 0), None, "asleep");
        // No request of this module is the save command (0x0350).
        for r in [
            get_profile_data(),
            get_settings(),
            tuf.set_dpi(0, 800).unwrap(),
        ] {
            assert_ne!(&r[1..3], &[0x50, 0x03]);
        }
        // Every accepted DPI of every model encodes.
        for m in MODELS {
            let (min, max, step) = m.dpi;
            for dpi in (min..=max).step_by(step as usize) {
                assert!(m.set_dpi(0, dpi).is_some(), "{} {dpi}", m.name);
            }
        }
    }
}
