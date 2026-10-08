//! HID++ 2.0 onboard profiles (feature 0x8100) memory codec (pure, no I/O).
//!
//! Layout learned from read-only dumps of a G305 (profile format 3) and the
//! public HID++ 2.0 feature description; see `docs/protocolos/logitech-hidpp.md`.
//! Only what was observed is interpreted; any other bytes are kept verbatim.

use super::DpiRange;
use super::razer::Rgb;

/// CRC-CCITT (poly 0x1021, init 0xFFFF) as stored big-endian in the last two
/// bytes of a sector. Verified against the G305 directory and profile.
pub fn crc_ccitt(data: &[u8]) -> u16 {
    data.iter().fold(0xffff, |mut crc, &b| {
        crc ^= (b as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
        crc
    })
}

/// True when the sector's trailing CRC matches its content.
pub fn crc_ok(sector: &[u8]) -> bool {
    let Some(split) = sector.len().checked_sub(2) else {
        return false;
    };
    let stored = u16::from_be_bytes([sector[split], sector[split + 1]]);
    crc_ccitt(&sector[..split]) == stored
}

/// `getInfo` payload (fn0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Info {
    pub profile_format: u8,
    pub profile_count: u8,
    pub button_count: u8,
    pub sector_size: u16,
}

pub fn parse_info(p: &[u8]) -> Option<Info> {
    let info = Info {
        profile_format: *p.get(1)?,
        profile_count: *p.get(3)?,
        button_count: *p.get(5)?,
        sector_size: u16::from_be_bytes([*p.get(7)?, *p.get(8)?]),
    };
    // Sizes outside this range are not something this code has seen.
    (16..=4096).contains(&info.sector_size).then_some(info)
}

/// Offset of the first button entry in a format-3 profile.
const BUTTONS_OFFSET: usize = 0x20;
/// Highest profile format whose button layout has been observed.
pub const KNOWN_FORMAT: u8 = 3;

/// A button assignment. Unknown encodings stay `Raw` and are never altered.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Action {
    /// `80 01 hi lo`: mouse buttons bit mask (0x01 left … 0x10 forward).
    Mouse(u16),
    /// `80 02 mod usage`: key with HID modifier bits and HID usage id.
    Keyboard { modifiers: u8, usage: u8 },
    /// Anything else, e.g. `90 xx` profile functions (DPI cycling).
    Raw([u8; 4]),
}

impl Action {
    pub fn decode(b: [u8; 4]) -> Self {
        match b {
            [0x80, 0x01, hi, lo] => Action::Mouse(u16::from_be_bytes([hi, lo])),
            [0x80, 0x02, modifiers, usage] => Action::Keyboard { modifiers, usage },
            other => Action::Raw(other),
        }
    }

    pub fn encode(self) -> [u8; 4] {
        match self {
            Action::Mouse(mask) => {
                let [hi, lo] = mask.to_be_bytes();
                [0x80, 0x01, hi, lo]
            }
            Action::Keyboard { modifiers, usage } => [0x80, 0x02, modifiers, usage],
            Action::Raw(b) => b,
        }
    }
}

/// Button assignments of a profile sector, in device order.
pub fn parse_buttons(sector: &[u8], count: u8) -> Option<Vec<Action>> {
    (0..count as usize)
        .map(|i| {
            let at = BUTTONS_OFFSET + 4 * i;
            let b: [u8; 4] = sector.get(at..at + 4)?.try_into().ok()?;
            Some(Action::decode(b))
        })
        .collect()
}

/// Directory entries: (sector, enabled), terminated by `ff ff`.
pub fn parse_directory(sector: &[u8]) -> Vec<(u16, bool)> {
    sector
        .chunks_exact(4)
        .map_while(|e| {
            let s = u16::from_be_bytes([e[0], e[1]]);
            (s != 0xffff).then_some((s, e[2] != 0))
        })
        .collect()
}

/// Mouse-button masks this app can assign (left, right, middle, back, forward).
const MOUSE_MASKS: [u16; 5] = [0x01, 0x02, 0x04, 0x08, 0x10];

/// Whether `a` is something this app may write. Only encodings observed on a
/// real device are allowed; `Raw` values are never written as new choices.
pub fn assignable(a: Action) -> bool {
    match a {
        Action::Mouse(m) => MOUSE_MASKS.contains(&m),
        // HID keyboard usages from "A" (0x04) to "Application" (0x65).
        Action::Keyboard { usage, .. } => (0x04..=0x65).contains(&usage),
        Action::Raw(_) => false,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EditError {
    /// The left button can never be changed (the user must keep a click).
    LeftButton,
    NoSuchButton,
    NotAssignable,
    /// DPI not accepted by the sensor.
    BadDpi,
    /// Effect not declared by the device or speed outside the tables.
    BadLight,
    /// Profile not readable or sector not a listed user profile.
    BadProfile,
}

/// Sector of a user profile listed in the directory. Never the directory
/// (0x0000) or a factory copy (0x01xx). Only `ProfileImage` can hold one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct UserSector(u16);

impl UserSector {
    fn new(sector: u16, directory: &[u8]) -> Option<Self> {
        let listed = parse_directory(directory).iter().any(|&(s, _)| s == sector);
        (sector != 0x0000 && sector & 0xff00 == 0 && listed && crc_ok(directory))
            .then_some(UserSector(sector))
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

/// Seal carried by the HID++ memory-write requests: it can only be created
/// in this module, so the only way to build a write is `ProfileImage`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Seal(());

/// A complete, validated profile sector ready to be written: same size as
/// the original, valid CRC, only the editable ranges changed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileImage {
    sector: UserSector,
    bytes: Vec<u8>,
}

impl ProfileImage {
    pub fn sector(&self) -> u16 {
        self.sector.get()
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    /// The HID++ requests that write this image: start (sector, offset 0,
    /// length), 16-byte chunks (the last padded with 0xff), end.
    pub fn write_requests(&self) -> Vec<crate::protocol::hidpp::Request> {
        use crate::protocol::hidpp::Request;
        let mut out = vec![Request::MemoryWriteStart {
            sector: self.sector.get(),
            offset: 0,
            len: self.bytes.len() as u16,
            seal: Seal(()),
        }];
        for chunk in self.bytes.chunks(16) {
            let mut data = [0xffu8; 16];
            data[..chunk.len()].copy_from_slice(chunk);
            out.push(Request::MemoryWrite {
                data,
                seal: Seal(()),
            });
        }
        out.push(Request::MemoryWriteEnd { seal: Seal(()) });
        out
    }
}

fn with_crc(mut sector: Vec<u8>) -> Vec<u8> {
    let n = sector.len() - 2;
    let crc = crc_ccitt(&sector[..n]).to_be_bytes();
    sector[n..].copy_from_slice(&crc);
    sector
}

/// First DPI entry of the cycle: 4 little-endian u16 values, then `00 00`.
/// The entry the mouse starts with is not stored in the profile (observed:
/// byte 1 does not select it), so a saved DPI fills every entry.
const DPI_OFFSET: usize = 0x03;
const DPI_ENTRIES: usize = 4;
/// Active light block (11 bytes). The block at 0xD0 had no visible effect and
/// is never written.
const LIGHT_OFFSET: usize = 0xdb;
const LIGHT_LEN: usize = 11;

/// Closed speed tables (period in ms), slow to fast. Observed on the G305:
/// cycle 8000 ms = about 8 s per turn, breathing 2000 and 6000 ms.
pub const CYCLE_PERIODS: [u16; 3] = [12000, 8000, 4000];
pub const BREATHING_PERIODS: [u16; 3] = [6000, 4000, 2000];

/// Light effect of the active block. The effect ids match those declared by
/// feature 0x8070 (`00` off, `01` fixed, `03` cycle, `0A` breathing).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Light {
    Off,
    Fixed(Rgb),
    Cycle { period_ms: u16 },
    Breathing { color: Rgb, period_ms: u16 },
}

impl Light {
    pub fn effect_id(self) -> u16 {
        match self {
            Light::Off => 0x00,
            Light::Fixed(_) => 0x01,
            Light::Cycle { .. } => 0x03,
            Light::Breathing { .. } => 0x0a,
        }
    }

    fn decode(b: &[u8]) -> Option<Light> {
        let rgb = || Rgb {
            r: b[1],
            g: b[2],
            b: b[3],
        };
        let be = |i: usize| u16::from_be_bytes([b[i], b[i + 1]]);
        Some(match b[0] {
            0x00 => Light::Off,
            0x01 => Light::Fixed(rgb()),
            0x03 => Light::Cycle { period_ms: be(6) },
            0x0a => Light::Breathing {
                color: rgb(),
                period_ms: be(4),
            },
            _ => return None,
        })
    }

    /// Writes only the bytes this effect uses; the rest of the block keeps
    /// its bytes (as in the verified writes).
    fn write(self, b: &mut [u8]) {
        b[0] = self.effect_id() as u8;
        match self {
            Light::Off => {}
            Light::Fixed(c) => b[1..4].copy_from_slice(&[c.r, c.g, c.b]),
            Light::Cycle { period_ms } => b[6..8].copy_from_slice(&period_ms.to_be_bytes()),
            Light::Breathing { color, period_ms } => {
                b[1..4].copy_from_slice(&[color.r, color.g, color.b]);
                b[4..6].copy_from_slice(&period_ms.to_be_bytes());
                // Verified with the three following bytes at 0.
                b[6..9].fill(0);
            }
        }
    }

    /// The 10 parameter bytes of the block (`+1..+11`), which is also the
    /// parameter layout of 0x8070 setZoneEffect (verified on the G305).
    pub fn params(self) -> [u8; LIGHT_LEN - 1] {
        let mut block = [0u8; LIGHT_LEN];
        self.write(&mut block);
        block[1..].try_into().unwrap_or_default()
    }

    /// Speeds come from the closed tables only.
    pub fn writable(self) -> bool {
        match self {
            Light::Off | Light::Fixed(_) => true,
            Light::Cycle { period_ms } => CYCLE_PERIODS.contains(&period_ms),
            Light::Breathing { period_ms, .. } => BREATHING_PERIODS.contains(&period_ms),
        }
    }
}

/// Light stored in the active block, if its effect is one this code knows.
pub fn parse_light(sector: &[u8]) -> Option<Light> {
    Light::decode(sector.get(LIGHT_OFFSET..LIGHT_OFFSET + LIGHT_LEN)?)
}

/// The 4 DPI entries of the cycle.
pub fn parse_dpi_cycle(sector: &[u8]) -> Option<[u16; DPI_ENTRIES]> {
    let raw = sector.get(DPI_OFFSET..DPI_OFFSET + 2 * DPI_ENTRIES)?;
    let mut out = [0u16; DPI_ENTRIES];
    for (o, c) in out.iter_mut().zip(raw.chunks_exact(2)) {
        *o = u16::from_le_bytes([c[0], c[1]]);
    }
    Some(out)
}

/// What the device accepts, read from it (not from the profile).
pub struct Limits<'a> {
    pub button_count: u8,
    /// DPI declared by the sensor (0x2201); `None` forbids DPI edits.
    pub dpi: Option<&'a DpiRange>,
    /// Effect ids declared by 0x8070; empty forbids light edits.
    pub effects: &'a [u16],
}

/// Everything that can change in one save.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProfileEdits {
    /// Fills every DPI entry of the cycle.
    pub dpi: Option<u16>,
    pub light: Option<Light>,
    pub buttons: Vec<(usize, Action)>,
}

/// Checks the layout this code relies on: CRC, length, the DPI terminator
/// and a known light effect.
fn known_layout(user: &[u8], button_count: u8) -> bool {
    let buttons_end = BUTTONS_OFFSET + 4 * button_count as usize;
    let dpi_end = DPI_OFFSET + 2 * DPI_ENTRIES;
    crc_ok(user)
        && user.len() >= LIGHT_OFFSET + LIGHT_LEN + 2
        && buttons_end <= LIGHT_OFFSET
        && user[dpi_end..dpi_end + 2] == [0, 0]
        && parse_light(user).is_some()
}

/// Builds the image for one save from the profile read from the device
/// (`user`, at `sector`). Only the DPI entries (0x03..0x0B), the active light
/// block (0xDB..0xE6), the edited 4-byte button entries and the CRC change.
pub fn apply_edits(
    user: &[u8],
    sector: u16,
    directory: &[u8],
    limits: &Limits,
    edits: &ProfileEdits,
) -> Result<ProfileImage, EditError> {
    let sector = UserSector::new(sector, directory).ok_or(EditError::BadProfile)?;
    if !known_layout(user, limits.button_count) {
        return Err(EditError::BadProfile);
    }
    let mut bytes = user.to_vec();
    if let Some(dpi) = edits.dpi {
        if dpi == 0 || !limits.dpi.is_some_and(|r| r.contains(dpi)) {
            return Err(EditError::BadDpi);
        }
        for i in 0..DPI_ENTRIES {
            let at = DPI_OFFSET + 2 * i;
            bytes[at..at + 2].copy_from_slice(&dpi.to_le_bytes());
        }
    }
    if let Some(light) = edits.light {
        if !light.writable() || !limits.effects.contains(&light.effect_id()) {
            return Err(EditError::BadLight);
        }
        light.write(&mut bytes[LIGHT_OFFSET..LIGHT_OFFSET + LIGHT_LEN]);
    }
    for &(index, action) in &edits.buttons {
        if index == 0 {
            return Err(EditError::LeftButton);
        }
        if index >= limits.button_count as usize {
            return Err(EditError::NoSuchButton);
        }
        if !assignable(action) {
            return Err(EditError::NotAssignable);
        }
        let at = BUTTONS_OFFSET + 4 * index;
        bytes[at..at + 4].copy_from_slice(&action.encode());
    }
    Ok(ProfileImage {
        sector,
        bytes: with_crc(bytes),
    })
}

/// Image that writes back a previously saved copy of the same sector.
pub fn restore_image(
    saved: &[u8],
    current: &[u8],
    sector: u16,
    directory: &[u8],
) -> Result<ProfileImage, EditError> {
    let sector = UserSector::new(sector, directory).ok_or(EditError::BadProfile)?;
    if !crc_ok(saved) || saved.len() != current.len() || saved.len() < BUTTONS_OFFSET + 6 {
        return Err(EditError::BadProfile);
    }
    // A copy without a left click (e.g. taken after another program
    // remapped it) is never written back.
    let left: [u8; 4] = saved[BUTTONS_OFFSET..BUTTONS_OFFSET + 4]
        .try_into()
        .unwrap_or_default();
    if Action::decode(left) != Action::Mouse(0x01) {
        return Err(EditError::LeftButton);
    }
    Ok(ProfileImage {
        sector,
        bytes: saved.to_vec(),
    })
}

/// Image that copies the factory DPI cycle, active light block and button
/// entries (`rom`) into the current profile; every other byte is kept.
pub fn factory_image(
    current: &[u8],
    rom: &[u8],
    sector: u16,
    directory: &[u8],
    limits: &Limits,
) -> Result<ProfileImage, EditError> {
    let sector = UserSector::new(sector, directory).ok_or(EditError::BadProfile)?;
    if !known_layout(current, limits.button_count) || current.len() != rom.len() {
        return Err(EditError::BadProfile);
    }
    let buttons_end = BUTTONS_OFFSET + 4 * limits.button_count as usize;
    let ranges = [
        DPI_OFFSET..DPI_OFFSET + 2 * DPI_ENTRIES + 2,
        BUTTONS_OFFSET..buttons_end,
        LIGHT_OFFSET..LIGHT_OFFSET + LIGHT_LEN,
    ];
    let mut bytes = current.to_vec();
    for r in ranges {
        bytes[r.clone()].copy_from_slice(&rom[r]);
    }
    // The factory copy must pass the same checks as an edit.
    let dpi_ok = parse_dpi_cycle(&bytes).is_some_and(|c| {
        c[0] != 0
            && c.iter()
                .all(|&d| d == 0 || limits.dpi.is_some_and(|r| r.contains(d)))
    });
    let light_ok = parse_light(&bytes)
        .is_some_and(|l| l == Light::Off || limits.effects.contains(&l.effect_id()));
    if !dpi_ok || !light_ok {
        return Err(EditError::BadProfile);
    }
    if Action::decode(
        bytes[BUTTONS_OFFSET..BUTTONS_OFFSET + 4]
            .try_into()
            .unwrap_or_default(),
    ) != Action::Mouse(0x01)
    {
        return Err(EditError::LeftButton);
    }
    Ok(ProfileImage {
        sector,
        bytes: with_crc(bytes),
    })
}

#[cfg(test)]
pub(crate) mod samples {
    //! Real sectors read from a G305 (no serial numbers or personal data).

    fn sector(lines: &[&str]) -> Vec<u8> {
        let mut v: Vec<u8> = lines
            .iter()
            .flat_map(|l| {
                l.split_whitespace()
                    .map(|h| u8::from_str_radix(h, 16).unwrap())
            })
            .collect();
        v.resize(253, 0xff);
        v
    }

    fn with_crc(mut v: Vec<u8>, crc: [u8; 2]) -> Vec<u8> {
        v.extend_from_slice(&crc);
        v
    }

    fn profile(buttons: &str) -> Vec<u8> {
        let mut v = sector(&[
            "01 01 00 90 01 20 03 40 06 80 0c 00 00 ff ff ff",
            "ff 00 ff ff ff ff ff ff ff ff ff ff ff ff ff ff",
            buttons,
        ]);
        let tail = "00 00 00 00 00 00 1f 40 00 00 00 00 00 00 00 00 00 1f 40 00 00 00";
        for (i, h) in tail.split_whitespace().enumerate() {
            v[0xd0 + i] = u8::from_str_radix(h, 16).unwrap();
        }
        v
    }

    pub fn directory() -> Vec<u8> {
        with_crc(sector(&["00 01 01 00 ff ff 00 00"]), [0x58, 0x7c])
    }

    pub fn user_profile() -> Vec<u8> {
        with_crc(
            profile("80 01 00 01 80 01 00 02 80 01 00 04 80 02 01 19 80 02 01 06 90 0c 00 00"),
            [0xae, 0xbc],
        )
    }

    pub fn rom_profile() -> Vec<u8> {
        with_crc(
            profile("80 01 00 01 80 01 00 02 80 01 00 04 80 01 00 08 80 01 00 10 90 05 00 00"),
            [0xff, 0xff],
        )
    }

    /// Sector written and read back in hardware test H1-6 (2026-10-07):
    /// breathing red at 6000 ms and the second DPI entry set to 1200.
    pub fn lit_profile() -> Vec<u8> {
        let mut v = user_profile();
        v[0x05..0x07].copy_from_slice(&[0xb0, 0x04]);
        v[0xdb..0xe4].copy_from_slice(&[0x0a, 0xff, 0x00, 0x00, 0x17, 0x70, 0x00, 0x00, 0x00]);
        v[253..].copy_from_slice(&[0xc4, 0x34]);
        v
    }

    pub fn dpi_range() -> crate::protocol::DpiRange {
        crate::protocol::DpiRange::Range {
            min: 200,
            max: 12000,
            step: 50,
        }
    }

    /// Effect ids declared by the G305 (0x8070 getZoneEffectInfo).
    pub const EFFECTS: [u16; 4] = [0x00, 0x01, 0x03, 0x0a];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_matches_real_sectors() {
        assert!(crc_ok(&samples::directory()));
        assert!(crc_ok(&samples::user_profile()));
        // The ROM copy stores no CRC.
        assert!(!crc_ok(&samples::rom_profile()));
        let mut bad = samples::user_profile();
        bad[0x2f] ^= 1;
        assert!(!crc_ok(&bad));
        assert!(!crc_ok(&[0x12]));
        assert_eq!(crc_ccitt(b"123456789"), 0x29b1); // CRC-16/CCITT-FALSE check value
    }

    #[test]
    fn info_from_device() {
        let i = parse_info(&[
            0x01, 0x03, 0x01, 0x01, 0x01, 0x06, 0x06, 0x00, 0xff, 0x0a, 0x02,
        ])
        .unwrap();
        assert_eq!(
            i,
            Info {
                profile_format: 3,
                profile_count: 1,
                button_count: 6,
                sector_size: 255
            }
        );
        assert_eq!(parse_info(&[0; 4]), None);
        assert_eq!(parse_info(&[1, 3, 1, 1, 1, 6, 6, 0, 0]), None);
    }

    #[test]
    fn directory_points_to_user_profile() {
        assert_eq!(parse_directory(&samples::directory()), vec![(0x0001, true)]);
        assert!(parse_directory(&[0xff; 8]).is_empty());
    }

    #[test]
    fn buttons_of_user_and_factory_profiles() {
        let user = parse_buttons(&samples::user_profile(), 6).unwrap();
        assert_eq!(user[0], Action::Mouse(0x01));
        assert_eq!(user[2], Action::Mouse(0x04));
        assert_eq!(
            user[3],
            Action::Keyboard {
                modifiers: 0x01,
                usage: 0x19
            }
        ); // Ctrl+V
        assert_eq!(
            user[4],
            Action::Keyboard {
                modifiers: 0x01,
                usage: 0x06
            }
        ); // Ctrl+C
        assert_eq!(user[5], Action::Raw([0x90, 0x0c, 0x00, 0x00]));
        let rom = parse_buttons(&samples::rom_profile(), 6).unwrap();
        assert_eq!(rom[3], Action::Mouse(0x08));
        assert_eq!(rom[4], Action::Mouse(0x10));
        assert_eq!(parse_buttons(&[0u8; 0x30], 6), None);
    }

    #[test]
    fn encoding_round_trips_every_observed_entry() {
        for s in [samples::user_profile(), samples::rom_profile()] {
            for a in parse_buttons(&s, 6).unwrap() {
                assert_eq!(Action::decode(a.encode()), a);
            }
            let raw: Vec<u8> = parse_buttons(&s, 6)
                .unwrap()
                .into_iter()
                .flat_map(Action::encode)
                .collect();
            assert_eq!(raw, s[0x20..0x38]);
        }
    }

    #[test]
    fn random_bytes_never_panic() {
        // Cheap deterministic fuzz: parsers must reject or keep, never panic.
        let mut x: u32 = 0x1234_5678;
        for len in 0..300 {
            let data: Vec<u8> = (0..len)
                .map(|_| {
                    x ^= x << 13;
                    x ^= x >> 17;
                    x ^= x << 5;
                    x as u8
                })
                .collect();
            let _ = crc_ok(&data);
            let _ = parse_info(&data);
            let _ = parse_directory(&data);
            let _ = parse_buttons(&data, 6);
            let _ = parse_light(&data);
            let _ = parse_dpi_cycle(&data);
        }
    }

    fn limits(range: &DpiRange) -> Limits<'_> {
        Limits {
            button_count: 6,
            dpi: Some(range),
            effects: &samples::EFFECTS,
        }
    }

    fn edit_profile(user: &[u8], edits: &ProfileEdits) -> Result<ProfileImage, EditError> {
        let range = samples::dpi_range();
        apply_edits(user, 1, &samples::directory(), &limits(&range), edits)
    }

    fn image(buttons: &[(usize, Action)]) -> Result<ProfileImage, EditError> {
        edit_profile(
            &samples::user_profile(),
            &ProfileEdits {
                buttons: buttons.to_vec(),
                ..Default::default()
            },
        )
    }

    fn changed(before: &[u8], after: &[u8]) -> Vec<usize> {
        (0..before.len())
            .filter(|&i| before[i] != after[i])
            .collect()
    }

    const RED: Rgb = Rgb {
        r: 0xff,
        g: 0,
        b: 0,
    };

    #[test]
    fn no_edits_rewrites_identical_bytes() {
        let img = image(&[]).unwrap();
        assert_eq!(img.bytes(), samples::user_profile().as_slice());
        assert_eq!(img.sector(), 1);
    }

    #[test]
    fn one_edit_changes_only_its_entry_and_the_crc() {
        let img = image(&[(4, Action::Mouse(0x10))]).unwrap();
        let changed = changed(&samples::user_profile(), img.bytes());
        assert!(
            changed
                .iter()
                .all(|&i| (0x30..0x34).contains(&i) || i >= 253),
            "{changed:?}"
        );
        assert!(crc_ok(img.bytes()));
        assert_eq!(
            parse_buttons(img.bytes(), 6).unwrap()[4],
            Action::Mouse(0x10)
        );
    }

    #[test]
    fn forbidden_edits_are_rejected() {
        assert_eq!(
            image(&[(0, Action::Mouse(0x02))]),
            Err(EditError::LeftButton)
        );
        assert_eq!(
            image(&[(6, Action::Mouse(0x02))]),
            Err(EditError::NoSuchButton)
        );
        assert_eq!(
            image(&[(3, Action::Raw([0x90, 5, 0, 0]))]),
            Err(EditError::NotAssignable)
        );
        assert_eq!(
            image(&[(3, Action::Mouse(0x03))]),
            Err(EditError::NotAssignable)
        );
        assert_eq!(
            image(&[(
                3,
                Action::Keyboard {
                    modifiers: 1,
                    usage: 0
                }
            )]),
            Err(EditError::NotAssignable)
        );
        let mut bad = samples::user_profile();
        bad[0x40] ^= 1;
        assert_eq!(
            edit_profile(&bad, &ProfileEdits::default()),
            Err(EditError::BadProfile)
        );
    }

    #[test]
    fn writes_never_target_directory_or_factory_sectors() {
        let dir = samples::directory();
        let user = samples::user_profile();
        let range = samples::dpi_range();
        let none = ProfileEdits::default();
        for sector in [0x0000u16, 0x0101, 0x0100, 0x0002, 0xffff] {
            assert_eq!(
                apply_edits(&user, sector, &dir, &limits(&range), &none),
                Err(EditError::BadProfile),
                "{sector:#06x}"
            );
            assert!(
                factory_image(
                    &user,
                    &samples::rom_profile(),
                    sector,
                    &dir,
                    &limits(&range)
                )
                .is_err()
            );
            assert!(restore_image(&user, &user, sector, &dir).is_err());
        }
        // A directory with a bad CRC is not trusted either.
        let mut bad_dir = dir.clone();
        bad_dir[10] ^= 1;
        assert_eq!(
            apply_edits(&user, 1, &bad_dir, &limits(&range), &none),
            Err(EditError::BadProfile)
        );
    }

    #[test]
    fn write_request_sequence() {
        use crate::protocol::hidpp::Request;
        let reqs = image(&[]).unwrap().write_requests();
        assert_eq!(reqs.len(), 1 + 16 + 1); // 255 bytes = 16 chunks
        assert!(matches!(
            reqs[0],
            Request::MemoryWriteStart {
                sector: 1,
                offset: 0,
                len: 255,
                ..
            }
        ));
        let Request::MemoryWrite { data, .. } = reqs[16] else {
            panic!()
        };
        assert_eq!(&data[..15], &samples::user_profile()[240..255]);
        assert_eq!(data[15], 0xff);
        assert!(matches!(reqs[17], Request::MemoryWriteEnd { .. }));
    }

    #[test]
    fn factory_and_restore_images() {
        let user = samples::user_profile();
        let range = samples::dpi_range();
        let img = factory_image(
            &user,
            &samples::rom_profile(),
            1,
            &samples::directory(),
            &limits(&range),
        )
        .unwrap();
        let b = parse_buttons(img.bytes(), 6).unwrap();
        assert_eq!(
            (b[3], b[4], b[5]),
            (
                Action::Mouse(0x08),
                Action::Mouse(0x10),
                Action::Raw([0x90, 0x05, 0, 0])
            )
        );
        assert!(crc_ok(img.bytes()));
        // From a lit profile, factory copies the DPI cycle, light and
        // buttons back, and keeps every other byte (block 0xD0, bytes 0-2).
        let lit = samples::lit_profile();
        let rom = samples::rom_profile();
        let img = factory_image(&lit, &rom, 1, &samples::directory(), &limits(&range)).unwrap();
        let editable = |i: usize| {
            (0x03..0x0d).contains(&i) || (0x20..0x38).contains(&i) || (0xdb..0xe6).contains(&i)
        };
        for i in 0..253 {
            let want = if editable(i) { rom[i] } else { lit[i] };
            assert_eq!(img.bytes()[i], want, "byte {i:#04x}");
        }
        assert_eq!(parse_light(img.bytes()), Some(Light::Off));
        let edited = image(&[(3, Action::Mouse(0x08))]).unwrap();
        let back = restore_image(&user, edited.bytes(), 1, &samples::directory()).unwrap();
        assert_eq!(back.bytes(), user.as_slice());
        let mut tampered = user.clone();
        tampered[0x21] ^= 1;
        assert!(restore_image(&tampered, &user, 1, &samples::directory()).is_err());
        // A valid copy whose left button is not a left click is refused.
        let mut no_left = user.clone();
        no_left[0x23] = 0x02;
        let no_left = with_crc(no_left);
        assert_eq!(
            restore_image(&no_left, &user, 1, &samples::directory()),
            Err(EditError::LeftButton)
        );
    }

    #[test]
    fn real_lit_profile_decodes() {
        let lit = samples::lit_profile();
        assert!(crc_ok(&lit));
        assert_eq!(
            parse_light(&lit),
            Some(Light::Breathing {
                color: RED,
                period_ms: 6000
            })
        );
        assert_eq!(parse_dpi_cycle(&lit), Some([400, 1200, 1600, 3200]));
        assert_eq!(parse_light(&samples::user_profile()), Some(Light::Off));
        assert_eq!(
            parse_dpi_cycle(&samples::user_profile()),
            Some([400, 800, 1600, 3200])
        );
    }

    #[test]
    fn verified_writes_are_reproduced_byte_for_byte() {
        // H1-6 started from the original copy: same edits, same bytes and CRC
        // (except the DPI, which the app writes to every entry).
        let img = edit_profile(
            &samples::user_profile(),
            &ProfileEdits {
                light: Some(Light::Breathing {
                    color: RED,
                    period_ms: 6000,
                }),
                ..Default::default()
            },
        )
        .unwrap();
        let mut want = samples::lit_profile();
        want[0x05..0x07].copy_from_slice(&[0x20, 0x03]);
        assert_eq!(&img.bytes()[..253], &want[..253]);
    }

    #[test]
    fn each_field_changes_only_its_range_and_the_crc() {
        let user = samples::user_profile();
        type InRange = fn(usize) -> bool;
        let cases: [(ProfileEdits, InRange); 4] = [
            (
                ProfileEdits {
                    dpi: Some(1650),
                    ..Default::default()
                },
                |i| (0x03..0x0b).contains(&i),
            ),
            (
                ProfileEdits {
                    light: Some(Light::Fixed(Rgb {
                        r: 0x12,
                        g: 0xab,
                        b: 0x34,
                    })),
                    ..Default::default()
                },
                |i| (0xdb..0xe6).contains(&i),
            ),
            (
                ProfileEdits {
                    light: Some(Light::Cycle { period_ms: 4000 }),
                    ..Default::default()
                },
                |i| (0xdb..0xe6).contains(&i),
            ),
            (
                ProfileEdits {
                    dpi: Some(800),
                    light: Some(Light::Breathing {
                        color: RED,
                        period_ms: 2000,
                    }),
                    buttons: vec![(3, Action::Mouse(0x08))],
                },
                |i| {
                    (0x03..0x0b).contains(&i)
                        || (0xdb..0xe6).contains(&i)
                        || (0x2c..0x30).contains(&i)
                },
            ),
        ];
        for (edits, allowed) in cases {
            let img = edit_profile(&user, &edits).unwrap();
            let changed = changed(&user, img.bytes());
            assert!(
                changed.iter().all(|&i| allowed(i) || i >= 253),
                "{edits:?}: {changed:?}"
            );
            assert!(crc_ok(img.bytes()));
            if let Some(d) = edits.dpi {
                assert_eq!(parse_dpi_cycle(img.bytes()), Some([d; 4]));
            }
            if let Some(l) = edits.light {
                assert_eq!(parse_light(img.bytes()), Some(l));
            }
            // The other light block is never written.
            assert_eq!(&img.bytes()[0xd0..0xdb], &user[0xd0..0xdb]);
        }
    }

    #[test]
    fn dpi_and_light_outside_the_device_limits_are_rejected() {
        let user = samples::user_profile();
        for dpi in [0, 150, 1625, 12050] {
            let e = ProfileEdits {
                dpi: Some(dpi),
                ..Default::default()
            };
            assert_eq!(edit_profile(&user, &e), Err(EditError::BadDpi), "{dpi}");
        }
        for light in [
            Light::Cycle { period_ms: 5000 },
            Light::Breathing {
                color: RED,
                period_ms: 0,
            },
        ] {
            let e = ProfileEdits {
                light: Some(light),
                ..Default::default()
            };
            assert_eq!(edit_profile(&user, &e), Err(EditError::BadLight));
        }
        // No DPI list or no declared effects: nothing can be written.
        let none = Limits {
            button_count: 6,
            dpi: None,
            effects: &[0x00],
        };
        let dir = samples::directory();
        let e = ProfileEdits {
            dpi: Some(800),
            ..Default::default()
        };
        assert_eq!(
            apply_edits(&user, 1, &dir, &none, &e),
            Err(EditError::BadDpi)
        );
        let e = ProfileEdits {
            light: Some(Light::Fixed(RED)),
            ..Default::default()
        };
        assert_eq!(
            apply_edits(&user, 1, &dir, &none, &e),
            Err(EditError::BadLight)
        );
    }

    #[test]
    fn unknown_layouts_are_not_edited() {
        let range = samples::dpi_range();
        let mut no_end = samples::user_profile();
        no_end[0x0b] = 0x01;
        let mut odd_light = samples::user_profile();
        odd_light[0xdb] = 0x07;
        for bad in [no_end, odd_light] {
            let bad = with_crc(bad);
            assert_eq!(
                edit_profile(&bad, &ProfileEdits::default()),
                Err(EditError::BadProfile)
            );
            assert!(
                factory_image(
                    &bad,
                    &samples::rom_profile(),
                    1,
                    &samples::directory(),
                    &limits(&range)
                )
                .is_err()
            );
        }
    }

    #[test]
    fn random_profiles_only_change_editable_ranges() {
        // Deterministic pseudo-random sectors with a valid layout.
        let mut x: u32 = 0x9e37_79b9;
        let mut next = move || {
            x ^= x << 13;
            x ^= x >> 17;
            x ^= x << 5;
            x
        };
        let range = samples::dpi_range();
        for _ in 0..200 {
            let mut user: Vec<u8> = (0..255).map(|_| next() as u8).collect();
            user[0x0b..0x0d].fill(0);
            user[0xdb] = [0x00, 0x01, 0x03, 0x0a][next() as usize % 4];
            user[0x20..0x24].copy_from_slice(&[0x80, 0x01, 0x00, 0x01]);
            let user = with_crc(user);
            let edits = ProfileEdits {
                dpi: Some(200 + 50 * (next() % 237) as u16),
                light: Some(Light::Fixed(Rgb {
                    r: next() as u8,
                    g: next() as u8,
                    b: next() as u8,
                })),
                buttons: vec![(1 + next() as usize % 5, Action::Mouse(0x04))],
            };
            let img =
                apply_edits(&user, 1, &samples::directory(), &limits(&range), &edits).unwrap();
            for i in changed(&user, img.bytes()) {
                assert!(
                    (0x03..0x0b).contains(&i)
                        || (0x24..0x38).contains(&i)
                        || (0xdb..0xe6).contains(&i)
                        || i >= 253,
                    "byte {i:#04x}"
                );
            }
            assert!(crc_ok(img.bytes()));
            assert_eq!(parse_light(img.bytes()), edits.light);
        }
    }
}
