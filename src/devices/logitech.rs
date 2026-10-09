//! HID++ 2.0 devices, directly attached or behind a receiver.

use super::{
    Change, Device, DeviceInfo, Dpi, Error, ProfileDump, ReportRate, Result, State, Supported,
    Transport,
};
use crate::protocol::hidpp::{self, Reply, Request, feature};
use crate::protocol::onboard;
use std::collections::HashMap;
use std::ffi::CString;
use std::time::{Duration, Instant};

/// Wired devices answer on index 0xFF; receivers host up to 6 devices.
const INDEXES: [u8; 7] = [0xff, 1, 2, 3, 4, 5, 6];
const REPLY_TIMEOUT: Duration = Duration::from_millis(400);

pub struct HidppDevice {
    info: DeviceInfo,
    path: CString,
    index: u8,
    features: HashMap<u16, Option<u8>>,
    onboard: bool,
}

/// Sends one request and waits for its reply, skipping unrelated traffic
/// (notifications and replies addressed to other host software).
fn call(t: &dyn Transport, index: u8, feature_index: u8, req: Request) -> Result<Vec<u8>> {
    let sent = req.encode(index, feature_index);
    t.write(&sent)?;
    let deadline = Instant::now() + REPLY_TIMEOUT;
    let mut buf = [0u8; 64];
    while Instant::now() < deadline {
        let n = t.read_timeout(&mut buf, 50)?;
        if n == 0 {
            continue;
        }
        match hidpp::match_reply(&sent, &buf[..n]) {
            Some(Reply::Ok(p)) => return Ok(p.to_vec()),
            // invalid argument, out of range, invalid function, unsupported
            Some(Reply::Error(0x02 | 0x03 | 0x07 | 0x09)) => return Err(Error::Unsupported),
            Some(Reply::Error(0x08)) => return Err(Error::Timeout), // busy
            Some(Reply::Error(_)) => return Err(Error::Protocol("error HID++ 2.0")),
            Some(Reply::Error10(_)) => return Err(Error::Timeout),
            None => {}
        }
    }
    Err(Error::Timeout)
}

impl HidppDevice {
    fn feature_index(&mut self, t: &dyn Transport, id: u16) -> Result<Option<u8>> {
        if id == feature::ROOT {
            return Ok(Some(0));
        }
        if let Some(&cached) = self.features.get(&id) {
            return Ok(cached);
        }
        let p = call(t, self.index, 0, Request::GetFeature { feature: id })?;
        let idx = p.first().copied().filter(|&i| i != 0);
        self.features.insert(id, idx);
        Ok(idx)
    }

    fn request(&mut self, t: &dyn Transport, req: Request) -> Result<Vec<u8>> {
        let idx = self
            .feature_index(t, req.feature())?
            .ok_or(Error::Unsupported)?;
        call(t, self.index, idx, req)
    }

    /// Like `request`, but a missing feature is `Ok(None)`.
    fn optional(&mut self, t: &dyn Transport, req: Request) -> Result<Option<Vec<u8>>> {
        match self.request(t, req) {
            Ok(p) => Ok(Some(p)),
            Err(Error::Unsupported) => Ok(None),
            Err(e) => Err(e),
        }
    }

    fn read_name(&mut self, t: &dyn Transport) -> Result<Option<String>> {
        let Some(p) = self.optional(t, Request::NameLength)? else {
            return Ok(None);
        };
        let len = p.first().copied().unwrap_or(0) as usize;
        let mut name = Vec::with_capacity(len);
        while name.len() < len {
            let chunk = self.request(
                t,
                Request::NameChunk {
                    offset: name.len() as u8,
                },
            )?;
            if chunk.is_empty() {
                break;
            }
            name.extend_from_slice(&chunk);
        }
        name.truncate(len);
        let name = hidpp::clean_name(&name);
        Ok((!name.is_empty()).then_some(name))
    }
}

pub fn probe(
    s: &'static Supported,
    pid: u16,
    path: CString,
    t: &dyn Transport,
) -> Vec<HidppDevice> {
    let mut found = Vec::new();
    for index in INDEXES {
        let ping = 0x5a;
        let Ok(p) = call(t, index, 0, Request::Ping { ping }) else {
            continue;
        };
        // Only HID++ 2.0+ devices echo the ping in the third byte.
        if p.get(2) != Some(&ping) || p.first().is_none_or(|&major| major < 2) {
            continue;
        }
        let mut dev = HidppDevice {
            info: DeviceInfo::new(s, s.model),
            path: path.clone(),
            index,
            features: HashMap::new(),
            onboard: s.onboard,
        };
        match dev.read_name(t) {
            Ok(Some(name)) => {
                dev.info = DeviceInfo::new(s, &name);
            }
            // An unverified device needs its own name as identity.
            Ok(None) if s.is_auto() => continue,
            Ok(None) => {}
            // The name is the device identity (saved settings use it); a
            // device that cannot report it now is skipped until next scan.
            Err(_) => continue,
        }
        if s.is_auto() {
            // Only pointing devices: this driver offers mouse settings.
            let pointer = dev
                .request(t, Request::DeviceType)
                .ok()
                .and_then(|p| hidpp::is_pointer(&p));
            if pointer != Some(true) {
                continue;
            }
            // Index 0xFF: the device itself (cable, or Bluetooth for the
            // 0xFF43 entry); 1..6: behind a receiver.
            let (wired, connection) = match index {
                0xff if s.wired => (true, "USB"),
                0xff => (false, s.connection),
                _ => (false, "Receptor Logitech"),
            };
            dev.info.key = super::key(s.vid, pid, &dev.info.name);
            dev.info.wired = wired;
            dev.info.connection = connection;
        }
        found.push(dev);
    }
    found
}

impl HidppDevice {
    /// Effect ids of light zone 0 (0x8070); empty without that feature.
    /// Only the first zone is used: the G305 declares one.
    fn light_effects(&mut self, t: &dyn Transport) -> Result<Vec<u16>> {
        let Some(p) = self.optional(t, Request::LedInfo)? else {
            return Ok(Vec::new());
        };
        if hidpp::parse_led_zone_count(&p).unwrap_or(0) == 0 {
            return Ok(Vec::new());
        }
        let p = self.request(t, Request::LedZoneInfo { zone: 0 })?;
        let count = hidpp::parse_led_effect_count(&p).ok_or(Error::Protocol("zona de luz"))?;
        (0..count)
            .map(|index| {
                let p = self.request(t, Request::LedEffectInfo { zone: 0, index })?;
                hidpp::parse_led_effect_id(&p).ok_or(Error::Protocol("efecto de luz"))
            })
            .collect()
    }

    /// Reads a whole sector, 16 bytes at a time (the last read is aligned to
    /// the end: reading past it is rejected by the device).
    fn read_sector(&mut self, t: &dyn Transport, sector: u16, size: u16) -> Result<Vec<u8>> {
        let size = size as usize;
        let mut data = Vec::with_capacity(size);
        while data.len() < size {
            let offset = data.len().min(size.saturating_sub(16));
            let chunk = self.request(
                t,
                Request::MemoryRead {
                    sector,
                    offset: offset as u16,
                },
            )?;
            let chunk = chunk
                .get(..16)
                .ok_or(Error::Protocol("lectura de memoria"))?;
            data.truncate(offset);
            data.extend_from_slice(chunk);
        }
        data.truncate(size);
        Ok(data)
    }
}

impl Device for HidppDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn path(&self) -> &CString {
        &self.path
    }

    fn receiver_index(&self) -> Option<u8> {
        (1..=6).contains(&self.index).then_some(self.index)
    }

    fn read_profile(&mut self, t: &dyn Transport) -> Result<ProfileDump> {
        if !self.onboard {
            return Err(Error::Unsupported);
        }
        let p = self.request(t, Request::OnboardInfo)?;
        let info = onboard::parse_info(&p).ok_or(Error::Protocol("información del perfil"))?;
        if !onboard::KNOWN_FORMATS.contains(&info.profile_format)
            || info.button_count > onboard::MAX_BUTTONS
        {
            return Err(Error::Unsupported);
        }
        // In host mode the profile is not in use (and the active sector reads
        // as 0). Changing the mode is left to the program that set it.
        let mode = self.request(t, Request::GetOnboardMode)?;
        if mode.first() != Some(&1) {
            return Err(Error::HostMode);
        }
        let p = self.request(t, Request::GetCurrentProfile)?;
        let sector = u16::from_be_bytes([
            *p.first().ok_or(Error::Protocol("perfil activo"))?,
            *p.get(1).ok_or(Error::Protocol("perfil activo"))?,
        ]);
        // Only user profiles listed in the directory are ever considered.
        let directory = self.read_sector(t, 0x0000, info.sector_size)?;
        if !onboard::crc_ok(&directory)
            || !onboard::parse_directory(&directory)
                .iter()
                .any(|&(s, _)| s == sector)
        {
            return Err(Error::Protocol("directorio de perfiles"));
        }
        let user = self.read_sector(t, sector, info.sector_size)?;
        if !onboard::crc_ok(&user) {
            return Err(Error::Protocol("CRC del perfil"));
        }
        // Factory copies live at 0x01xx (observed: profile 0x0001 -> 0x0101).
        let rom = self.read_sector(t, 0x0100 | (sector & 0x00ff), info.sector_size)?;
        let dpi = self
            .optional(t, Request::DpiList { sensor: 0 })?
            .and_then(|p| hidpp::parse_dpi_list(&p));
        // Profile light verified only in format 3: elsewhere no light is
        // offered, so its block is never written.
        let effects = if info.profile_format == onboard::LIGHT_FORMAT {
            self.light_effects(t)?
        } else {
            Vec::new()
        };
        Ok(ProfileDump {
            info,
            sector,
            directory,
            user,
            rom,
            dpi,
            effects,
        })
    }

    fn activate_onboard(&mut self, t: &dyn Transport) -> Result<()> {
        if !self.onboard {
            return Err(Error::Unsupported);
        }
        self.request(t, Request::ActivateOnboard)?;
        let mode = self.request(t, Request::GetOnboardMode)?;
        if mode.first() != Some(&1) {
            return Err(Error::Protocol("el ratón sigue en modo host"));
        }
        Ok(())
    }

    fn write_profile(&mut self, t: &dyn Transport, image: &onboard::ProfileImage) -> Result<()> {
        if !self.onboard {
            return Err(Error::Unsupported);
        }
        for req in image.write_requests() {
            self.request(t, req)?;
        }
        let back = self.read_sector(t, image.sector(), image.bytes().len() as u16)?;
        if back != image.bytes() {
            return Err(Error::Protocol("la memoria releída no coincide"));
        }
        Ok(())
    }

    fn read_state(&mut self, t: &dyn Transport) -> Result<State> {
        let battery = match self.optional(t, Request::UnifiedBatteryStatus)? {
            Some(p) => hidpp::parse_unified_battery(&p),
            None => self
                .optional(t, Request::BatteryStatus)?
                .and_then(|p| hidpp::parse_battery_status(&p)),
        };
        let mut state = State {
            battery,
            ..State::default()
        };
        // A DPI or rate format this code does not understand only hides that
        // control; it must not hide the battery.
        if let Some(list) = self.optional(t, Request::DpiList { sensor: 0 })? {
            let current = self.request(t, Request::GetDpi { sensor: 0 })?;
            if let (Some(allowed), Some(current)) =
                (hidpp::parse_dpi_list(&list), hidpp::parse_dpi(&current))
            {
                state.dpi = Some(Dpi {
                    current: Some(current),
                    allowed,
                });
            }
        }
        if let Some(list) = self.optional(t, Request::ReportRateList)? {
            let current = self.request(t, Request::GetReportRate)?;
            if let (Some(supported_ms), Some(current_ms)) = (
                hidpp::parse_report_rate_list(&list),
                hidpp::parse_report_rate(&current),
            ) {
                state.report_rate = Some(ReportRate {
                    current_ms,
                    supported_ms,
                });
            }
        }
        Ok(state)
    }

    fn apply(&mut self, t: &dyn Transport, change: Change) -> Result<()> {
        // Validate against what the device itself declares before writing.
        match change {
            Change::Dpi(dpi) => {
                let list = self.request(t, Request::DpiList { sensor: 0 })?;
                let allowed =
                    hidpp::parse_dpi_list(&list).ok_or(Error::Protocol("lista de DPI"))?;
                if !allowed.contains(dpi) {
                    return Err(Error::Unsupported);
                }
                self.request(t, Request::SetDpi { sensor: 0, dpi })?;
            }
            Change::Light(light) => {
                let effects = self.light_effects(t)?;
                let effect_index = effects
                    .iter()
                    .position(|&id| id == light.effect_id())
                    .filter(|_| light.writable())
                    .ok_or(Error::Unsupported)?;
                self.request(
                    t,
                    Request::SetLight {
                        effect_index: effect_index as u8,
                        light,
                    },
                )?;
            }
            Change::Color(_) => return Err(Error::Unsupported),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{AUTO_LOGITECH, SUPPORTED, Support, fake::FakeTransport};
    use crate::protocol::{Battery, ChargeState};
    use std::cell::RefCell;
    use std::rc::Rc;

    /// Simulates a G305 at index 1: features resolved by id, fixed answers.
    fn g305() -> FakeTransport {
        g305_with(Rc::new(RefCell::new(onboard::samples::user_profile())))
    }

    /// Same, with the user profile (sector 1) held in `user`, writable
    /// through 0x8100 fn6/7/8.
    fn g305_with(user: Rc<RefCell<Vec<u8>>>) -> FakeTransport {
        g305_with_commit(user, true)
    }

    /// `commit = false` acknowledges writes without storing them.
    fn g305_with_commit(user: Rc<RefCell<Vec<u8>>>, commit: bool) -> FakeTransport {
        mouse(user, commit, 3, 1)
    }

    /// The G305 above reporting another profile `format` and onboard `mode`.
    fn mouse(user: Rc<RefCell<Vec<u8>>>, commit: bool, format: u8, mode: u8) -> FakeTransport {
        let pending: RefCell<Option<(u16, usize, Vec<u8>)>> = RefCell::new(None);
        FakeTransport::new(move |req| {
            let (idx, feat, func) = (req[1], req[2], req[3]);
            if idx != 1 {
                return vec![]; // no device: timeout
            }
            let mut r = vec![0u8; 20];
            r[..4].copy_from_slice(&req[..4]);
            let f = func >> 4;
            // 0x8100 memory reads are served from real G305 sectors.
            let memory;
            let p: &[u8] = match (feat, f) {
                (7, 0) => {
                    memory = vec![
                        0x01, format, 0x01, 0x01, 0x01, 0x06, 0x06, 0x00, 0xff, 0x0a, 0x02,
                    ];
                    &memory
                }
                (7, 2) => &[mode],
                (7, 4) => &[0x00, 0x01],
                (7, 5) => {
                    let sector = match u16::from_be_bytes([req[4], req[5]]) {
                        0x0000 => onboard::samples::directory(),
                        0x0001 => user.borrow().clone(),
                        0x0101 => onboard::samples::rom_profile(),
                        _ => return vec![],
                    };
                    let off = u16::from_be_bytes([req[6], req[7]]) as usize;
                    if off + 16 > sector.len() {
                        return vec![vec![
                            0x11, 1, 0xff, feat, func, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                            0,
                        ]];
                    }
                    memory = sector[off..off + 16].to_vec();
                    &memory
                }
                (7, 6) => {
                    let (sector, len) = (
                        u16::from_be_bytes([req[4], req[5]]),
                        u16::from_be_bytes([req[8], req[9]]) as usize,
                    );
                    *pending.borrow_mut() = Some((sector, len, Vec::new()));
                    &[]
                }
                (7, 7) => {
                    if let Some((_, _, data)) = pending.borrow_mut().as_mut() {
                        data.extend_from_slice(&req[4..20]);
                    }
                    &[]
                }
                (7, 8) => {
                    if let (true, Some((1, len, mut data))) = (commit, pending.borrow_mut().take())
                    {
                        data.truncate(len);
                        *user.borrow_mut() = data;
                    }
                    &[]
                }
                (0, 1) => &[4, 2, req[6]],
                (0, 0) => match u16::from_be_bytes([req[4], req[5]]) {
                    0x0005 => &[2],
                    0x1000 => &[4],
                    0x2201 => &[5],
                    0x8060 => &[6],
                    0x8100 => &[7],
                    0x8070 => &[8],
                    _ => &[0],
                },
                // 0x8070 replies observed on the G305 (one zone, 4 effects).
                (8, 0) => &[0x01, 0x00, 0x00, 0x00, 0x14],
                (8, 1) => &[0x00, 0x00, 0x01, 0x04],
                (8, 2) => {
                    let id = [0x00, 0x01, 0x03, 0x0a][req[5] as usize % 4];
                    memory = vec![0x00, req[5], 0x00, id];
                    &memory
                }
                (8, 3) => &[],
                (2, 0) => &[4],
                (2, 1) => b"G305",
                (2, 2) => &[3], // deviceType: mouse
                (4, 0) => &[90, 50, 0],
                (5, 1) => &[0, 0x00, 0xc8, 0xe0, 0x32, 0x2e, 0xe0, 0, 0],
                (5, 2) => &[0, 0x06, 0x40, 0x06, 0x40],
                (5, 3) => &[0, req[5], req[6]],
                (6, 0) => &[0b1000_1011],
                (6, 1) => &[1],
                _ => {
                    return vec![vec![
                        0x11, 1, 0xff, feat, func, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
                    ]];
                }
            };
            r[4..4 + p.len()].copy_from_slice(p);
            // Unrelated notification first, then the reply.
            vec![vec![0x11, 1, 0x09, 0x00, 1, 2, 3], r]
        })
    }

    #[test]
    fn unlisted_logitech_is_detected_with_volatile_settings_only() {
        let t = g305();
        let mut d = probe(&AUTO_LOGITECH, 0xc092, CString::new("x").unwrap(), &t).remove(0);
        assert_eq!(d.info.key.0, "046d:c092/G305");
        assert_eq!(d.info.support, Support::Auto);
        assert_eq!(d.info.connection, "Receptor Logitech");
        assert!(!d.info.wired);
        let before = t.sent.borrow().len();
        assert_eq!(d.read_profile(&t), Err(Error::Unsupported));
        assert_eq!(
            t.sent.borrow().len(),
            before,
            "nothing sent for the profile"
        );
        // Volatile settings still work.
        assert!(d.read_state(&t).unwrap().dpi.is_some());
    }

    #[test]
    fn host_mode_profile_is_not_read() {
        let t = mouse(
            Rc::new(RefCell::new(onboard::samples::user_profile())),
            true,
            3,
            2,
        );
        let mut d = probe(&SUPPORTED[0], 0xc53f, CString::new("x").unwrap(), &t).remove(0);
        let before = t.sent.borrow().len();
        assert_eq!(d.read_profile(&t), Err(Error::HostMode));
        // Only getInfo and getOnboardMode: no memory read, no mode change.
        let sent = t.sent.borrow();
        let fns: Vec<u8> = sent[before..]
            .iter()
            .filter(|r| r[2] == 7)
            .map(|r| r[3] >> 4)
            .collect();
        assert_eq!(fns, [0, 2]);
    }

    #[test]
    fn format_4_profile_is_read_without_its_light() {
        // The real G203 profile sector, served behind the G305's directory.
        let t = mouse(
            Rc::new(RefCell::new(onboard::samples::g203_user())),
            true,
            4,
            1,
        );
        let mut d = probe(&SUPPORTED[3], 0xc092, CString::new("x").unwrap(), &t).remove(0);
        assert_eq!(d.info.key.0, "046d:c092/G305");
        assert_eq!(d.info.support, Support::Verified);
        let dump = d.read_profile(&t).unwrap();
        assert_eq!(dump.info.profile_format, 4);
        assert!(dump.effects.is_empty(), "light not offered in format 4");
        assert_eq!(dump.user, onboard::samples::g203_user());
        // Unknown formats stay unsupported.
        let t = mouse(
            Rc::new(RefCell::new(onboard::samples::user_profile())),
            true,
            5,
            1,
        );
        let mut d = probe(&SUPPORTED[0], 0xc53f, CString::new("x").unwrap(), &t).remove(0);
        assert_eq!(d.read_profile(&t), Err(Error::Unsupported));
    }

    #[test]
    fn reads_the_onboard_profile_read_only() {
        let t = g305();
        let mut d = probe(&SUPPORTED[0], 0xc53f, CString::new("x").unwrap(), &t).remove(0);
        let before = t.sent.borrow().len();
        let dump = d.read_profile(&t).unwrap();
        assert_eq!(dump.sector, 0x0001);
        assert_eq!(dump.user, onboard::samples::user_profile());
        assert_eq!(dump.directory, onboard::samples::directory());
        assert_eq!(dump.rom, onboard::samples::rom_profile());
        assert_eq!(dump.user.len(), 255);
        assert_eq!(dump.effects, onboard::samples::EFFECTS);
        assert_eq!(dump.dpi, Some(onboard::samples::dpi_range()));
        // Only 0x8100 functions 0, 2, 4 and 5 (reads) were sent: never 6, 7, 8.
        let sent = t.sent.borrow();
        assert!(
            sent[before..]
                .iter()
                .filter(|r| r[2] == 7)
                .all(|r| matches!(r[3] >> 4, 0 | 2 | 4 | 5))
        );
    }

    #[test]
    fn writes_profile_and_verifies_by_reading_back() {
        let user = Rc::new(RefCell::new(onboard::samples::user_profile()));
        let t = g305_with(user.clone());
        let mut d = probe(&SUPPORTED[0], 0xc53f, CString::new("x").unwrap(), &t).remove(0);
        let dump = d.read_profile(&t).unwrap();
        let image = onboard::apply_edits(
            &dump.user,
            dump.sector,
            &dump.directory,
            &dump.limits(),
            &onboard::ProfileEdits {
                buttons: vec![(4, onboard::Action::Mouse(0x10))],
                ..Default::default()
            },
        )
        .unwrap();
        d.write_profile(&t, &image).unwrap();
        assert_eq!(user.borrow().as_slice(), image.bytes());
        // Writes only went to sector 1, as a 6/7.../8 sequence.
        let sent = t.sent.borrow();
        let writes: Vec<_> = sent
            .iter()
            .filter(|r| r[2] == 7 && matches!(r[3] >> 4, 6..=8))
            .collect();
        assert_eq!(writes.len(), 18);
        assert_eq!(&writes[0][4..10], &[0, 1, 0, 0, 0, 255]);
    }

    #[test]
    fn write_fails_when_read_back_differs() {
        // A device that acknowledges writes but keeps the old bytes.
        let t = g305_with_commit(
            Rc::new(RefCell::new(onboard::samples::user_profile())),
            false,
        );
        let mut d = probe(&SUPPORTED[0], 0xc53f, CString::new("x").unwrap(), &t).remove(0);
        let dump = d.read_profile(&t).unwrap();
        let image = onboard::apply_edits(
            &dump.user,
            dump.sector,
            &dump.directory,
            &dump.limits(),
            &onboard::ProfileEdits {
                buttons: vec![(4, onboard::Action::Mouse(0x10))],
                ..Default::default()
            },
        )
        .unwrap();
        assert_eq!(
            d.write_profile(&t, &image),
            Err(Error::Protocol("la memoria releída no coincide"))
        );
    }

    #[test]
    fn probes_and_reads_g305() {
        let t = g305();
        let mut devs = probe(&SUPPORTED[0], 0xc53f, CString::new("x").unwrap(), &t);
        assert_eq!(devs.len(), 1);
        let d = &mut devs[0];
        assert_eq!(d.info().name, "G305");
        assert_eq!(d.info().key.0, "046d:c53f/G305");
        let s = d.read_state(&t).unwrap();
        assert_eq!(
            s.battery,
            Some(Battery {
                percent: 90,
                charging: ChargeState::Discharging
            })
        );
        assert_eq!(s.dpi.unwrap().current, Some(1600));
        assert_eq!(s.report_rate.unwrap().supported_ms, vec![1, 2, 4, 8]);
        assert!(!s.color);
    }

    #[test]
    fn live_light_uses_the_effect_position_and_never_the_profile() {
        use crate::protocol::onboard::Light;
        let t = g305();
        let mut d = probe(&SUPPORTED[0], 0xc53f, CString::new("x").unwrap(), &t).remove(0);
        let before = t.sent.borrow().len();
        let light = Light::Breathing {
            color: crate::protocol::razer::Rgb {
                r: 0xff,
                g: 0,
                b: 0,
            },
            period_ms: 2000,
        };
        assert_eq!(d.apply(&t, Change::Light(light)), Ok(()));
        // Breathing (0x0A) is the 4th declared effect: index 3, as in H2.
        let sent = t.sent.borrow();
        let set: Vec<_> = sent[before..]
            .iter()
            .filter(|r| r[2] == 8 && r[3] >> 4 == 3)
            .collect();
        assert_eq!(set.len(), 1);
        assert_eq!(&set[0][4..9], &[0x00, 0x03, 0xff, 0x00, 0x00]);
        assert_eq!(&set[0][9..11], &[0x07, 0xd0]);
        // No onboard-profile function was used.
        assert!(sent[before..].iter().all(|r| r[2] != 7));
        drop(sent);
        // Speeds outside the closed tables are refused before sending.
        let before = t.sent.borrow().len();
        assert_eq!(
            d.apply(&t, Change::Light(Light::Cycle { period_ms: 5000 })),
            Err(Error::Unsupported)
        );
        assert!(t.sent.borrow()[before..].iter().all(|r| r[3] >> 4 != 3));
    }

    #[test]
    fn rejects_out_of_range_before_writing() {
        let t = g305();
        let mut d = probe(&SUPPORTED[0], 0xc53f, CString::new("x").unwrap(), &t).remove(0);
        let before = t.sent.borrow().len();
        assert_eq!(d.apply(&t, Change::Dpi(1625)), Err(Error::Unsupported));
        // Only list queries were sent, never a set request (function 3 / 2).
        let sent = t.sent.borrow();
        assert!(
            sent[before..]
                .iter()
                .all(|r| r[3] >> 4 == 1 || r[3] >> 4 == 0)
        );
        drop(sent);
        assert_eq!(d.apply(&t, Change::Dpi(800)), Ok(()));
        assert_eq!(
            d.apply(
                &t,
                Change::Color(crate::protocol::razer::Rgb { r: 1, g: 1, b: 1 })
            ),
            Err(Error::Unsupported)
        );
    }

    #[test]
    fn silent_receiver_yields_nothing() {
        let t = FakeTransport::new(|_| vec![]);
        assert!(probe(&SUPPORTED[0], 0xc53f, CString::new("x").unwrap(), &t).is_empty());
    }
}
