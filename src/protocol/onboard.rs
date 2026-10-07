//! HID++ 2.0 onboard profiles (feature 0x8100) memory codec (pure, no I/O).
//!
//! Layout learned from read-only dumps of a G305 (profile format 3) and the
//! public HID++ 2.0 feature description; see `docs/protocolos/logitech-hidpp.md`.
//! Only what was observed is interpreted; any other bytes are kept verbatim.

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
/// the original, valid CRC, only the button bytes changed.
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

/// Builds the image that changes some buttons of the profile read from the
/// device (`user`, at `sector`). Everything except the edited 4-byte entries
/// and the CRC stays byte-for-byte identical.
pub fn apply_buttons(
    user: &[u8],
    sector: u16,
    directory: &[u8],
    button_count: u8,
    edits: &[(usize, Action)],
) -> Result<ProfileImage, EditError> {
    let sector = UserSector::new(sector, directory).ok_or(EditError::BadProfile)?;
    if !crc_ok(user) || user.len() < BUTTONS_OFFSET + 4 * button_count as usize + 2 {
        return Err(EditError::BadProfile);
    }
    let mut bytes = user.to_vec();
    for &(index, action) in edits {
        if index == 0 {
            return Err(EditError::LeftButton);
        }
        if index >= button_count as usize {
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

/// Image that copies the factory button entries (`rom`) into the current
/// profile; every other byte of the current profile is kept.
pub fn factory_buttons(
    current: &[u8],
    rom: &[u8],
    sector: u16,
    directory: &[u8],
    button_count: u8,
) -> Result<ProfileImage, EditError> {
    let sector = UserSector::new(sector, directory).ok_or(EditError::BadProfile)?;
    let end = BUTTONS_OFFSET + 4 * button_count as usize;
    if !crc_ok(current) || current.len() != rom.len() || current.len() < end + 2 {
        return Err(EditError::BadProfile);
    }
    let mut bytes = current.to_vec();
    bytes[BUTTONS_OFFSET..end].copy_from_slice(&rom[BUTTONS_OFFSET..end]);
    // The factory left button must still be a left click.
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
        }
    }

    fn image(edits: &[(usize, Action)]) -> Result<ProfileImage, EditError> {
        apply_buttons(&samples::user_profile(), 1, &samples::directory(), 6, edits)
    }

    #[test]
    fn no_edits_rewrites_identical_bytes() {
        let img = image(&[]).unwrap();
        assert_eq!(img.bytes(), samples::user_profile().as_slice());
        assert_eq!(img.sector(), 1);
    }

    #[test]
    fn one_edit_changes_only_its_entry_and_the_crc() {
        let img = image(&[(4, Action::Mouse(0x10))]).unwrap();
        let before = samples::user_profile();
        let changed: Vec<usize> = (0..before.len())
            .filter(|&i| before[i] != img.bytes()[i])
            .collect();
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
            apply_buttons(&bad, 1, &samples::directory(), 6, &[]),
            Err(EditError::BadProfile)
        );
    }

    #[test]
    fn writes_never_target_directory_or_factory_sectors() {
        let dir = samples::directory();
        let user = samples::user_profile();
        for sector in [0x0000u16, 0x0101, 0x0100, 0x0002, 0xffff] {
            assert_eq!(
                apply_buttons(&user, sector, &dir, 6, &[]),
                Err(EditError::BadProfile),
                "{sector:#06x}"
            );
            assert!(restore_image(&user, &user, sector, &dir).is_err());
        }
        // A directory with a bad CRC is not trusted either.
        let mut bad_dir = dir.clone();
        bad_dir[10] ^= 1;
        assert_eq!(
            apply_buttons(&user, 1, &bad_dir, 6, &[]),
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
        let img =
            factory_buttons(&user, &samples::rom_profile(), 1, &samples::directory(), 6).unwrap();
        let b = parse_buttons(img.bytes(), 6).unwrap();
        assert_eq!(
            (b[3], b[4], b[5]),
            (
                Action::Mouse(0x08),
                Action::Mouse(0x10),
                Action::Raw([0x90, 0x05, 0, 0])
            )
        );
        assert_eq!(&img.bytes()[0xd0..0xe6], &user[0xd0..0xe6]);
        assert!(crc_ok(img.bytes()));
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
}
