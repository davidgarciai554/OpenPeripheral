//! HID++ 2.0 devices, directly attached or behind a receiver.

use super::{
    Change, Device, DeviceInfo, Dpi, Error, ReportRate, Result, State, Supported, Transport,
};
use crate::protocol::hidpp::{self, Reply, Request, feature};
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
        let name = String::from_utf8_lossy(&name)
            .trim_end_matches('\0')
            .trim()
            .to_string();
        Ok((!name.is_empty()).then_some(name))
    }
}

pub fn probe(s: &'static Supported, path: CString, t: &dyn Transport) -> Vec<HidppDevice> {
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
        };
        match dev.read_name(t) {
            Ok(Some(name)) => {
                dev.info = DeviceInfo::new(s, &name);
            }
            Ok(None) => {}
            // The name is the device identity (saved settings use it); a
            // device that cannot report it now is skipped until next scan.
            Err(_) => continue,
        }
        found.push(dev);
    }
    found
}

impl Device for HidppDevice {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn path(&self) -> &CString {
        &self.path
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
                state.dpi = Some(Dpi { current, allowed });
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
            Change::Color(_) => return Err(Error::Unsupported),
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{SUPPORTED, fake::FakeTransport};
    use crate::protocol::{Battery, ChargeState};

    /// Simulates a G305 at index 1: features resolved by id, fixed answers.
    fn g305() -> FakeTransport {
        FakeTransport::new(|req| {
            let (idx, feat, func) = (req[1], req[2], req[3]);
            if idx != 1 {
                return vec![]; // no device: timeout
            }
            let mut r = vec![0u8; 20];
            r[..4].copy_from_slice(&req[..4]);
            let f = func >> 4;
            let p: &[u8] = match (feat, f) {
                (0, 1) => &[4, 2, req[6]],
                (0, 0) => match u16::from_be_bytes([req[4], req[5]]) {
                    0x0005 => &[2],
                    0x1000 => &[4],
                    0x2201 => &[5],
                    0x8060 => &[6],
                    _ => &[0],
                },
                (2, 0) => &[4],
                (2, 1) => b"G305",
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
    fn probes_and_reads_g305() {
        let t = g305();
        let mut devs = probe(&SUPPORTED[0], CString::new("x").unwrap(), &t);
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
        assert_eq!(s.dpi.unwrap().current, 1600);
        assert_eq!(s.report_rate.unwrap().supported_ms, vec![1, 2, 4, 8]);
        assert!(!s.color);
    }

    #[test]
    fn rejects_out_of_range_before_writing() {
        let t = g305();
        let mut d = probe(&SUPPORTED[0], CString::new("x").unwrap(), &t).remove(0);
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
        assert!(probe(&SUPPORTED[0], CString::new("x").unwrap(), &t).is_empty());
    }
}
