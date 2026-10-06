//! Sony DualSense over USB.

use super::{Change, Device, DeviceInfo, Error, Result, State, Supported, Transport};
use crate::protocol::dualsense;
use std::ffi::CString;
use std::time::{Duration, Instant};

/// The controller streams input reports every few ms while connected.
const READ_WINDOW: Duration = Duration::from_millis(300);

pub struct DualSense {
    info: DeviceInfo,
    path: CString,
}

impl DualSense {
    pub fn new(s: &'static Supported, path: CString) -> Self {
        Self {
            info: DeviceInfo::new(s, s.model),
            path,
        }
    }
}

impl Device for DualSense {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn path(&self) -> &CString {
        &self.path
    }

    fn read_state(&mut self, t: &dyn Transport) -> Result<State> {
        let deadline = Instant::now() + READ_WINDOW;
        let mut buf = [0u8; 78];
        while Instant::now() < deadline {
            let n = t.read_timeout(&mut buf, 50)?;
            if let Some(battery) = dualsense::parse_battery(&buf[..n]) {
                return Ok(State {
                    battery: Some(battery),
                    color: true,
                    ..State::default()
                });
            }
        }
        Err(Error::Timeout)
    }

    fn apply(&mut self, t: &dyn Transport, change: Change) -> Result<()> {
        match change {
            Change::Color(c) => t.write(&dualsense::lightbar_report(c.r, c.g, c.b)),
            _ => Err(Error::Unsupported),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{SUPPORTED, fake::FakeTransport};
    use crate::protocol::{ChargeState, razer::Rgb};

    #[test]
    fn reads_battery_from_stream() {
        let t = FakeTransport::new(|_| vec![]);
        t.push(vec![0x31; 10]); // unrelated report first
        let mut r = vec![0u8; 64];
        r[0] = 0x01;
        r[53] = 0x18;
        t.push(r);
        let mut d = DualSense::new(&SUPPORTED[2], CString::new("x").unwrap());
        let s = d.read_state(&t).unwrap();
        let b = s.battery.unwrap();
        assert_eq!((b.percent, b.charging), (85, ChargeState::Charging));
        assert!(s.color);
    }

    #[test]
    fn writes_lightbar_only() {
        let t = FakeTransport::new(|_| vec![]);
        let mut d = DualSense::new(&SUPPORTED[2], CString::new("x").unwrap());
        d.apply(&t, Change::Color(Rgb { r: 9, g: 8, b: 7 }))
            .unwrap();
        let sent = t.sent.borrow();
        assert_eq!(sent[0].len(), 48);
        assert_eq!(&sent[0][45..], &[9, 8, 7]);
        assert_eq!(d.apply(&t, Change::Dpi(400)), Err(Error::Unsupported));
        assert_eq!(
            d.read_state(&FakeTransport::new(|_| vec![])),
            Err(Error::Timeout)
        );
    }
}
