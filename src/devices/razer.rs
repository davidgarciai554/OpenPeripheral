//! Razer keyboards using the 90-byte feature report protocol.

use super::{Change, Device, DeviceInfo, Error, Result, State, Supported, Transport};
use crate::protocol::razer::{self, REPORT_LEN, ReplyError, Request};
use std::ffi::CString;
use std::thread::sleep;
use std::time::Duration;

/// The firmware needs a short pause between the command and reading its
/// answer, and may answer "busy" a few times.
const SETTLE: Duration = Duration::from_millis(25);
const ATTEMPTS: usize = 4;

pub struct RazerKeyboard {
    info: DeviceInfo,
    path: CString,
}

impl RazerKeyboard {
    pub fn new(s: &'static Supported, path: CString) -> Self {
        Self {
            info: DeviceInfo::new(s, s.model),
            path,
        }
    }
}

fn call(t: &dyn Transport, req: Request) -> Result<Vec<u8>> {
    let sent = req.encode();
    let mut out = [0u8; REPORT_LEN + 1]; // report id 0 prefix
    out[1..].copy_from_slice(&sent);
    t.send_feature(&out)?;
    for _ in 0..ATTEMPTS {
        sleep(SETTLE);
        let mut buf = [0u8; REPORT_LEN + 1];
        let n = t.get_feature(&mut buf)?;
        match razer::parse_reply(&sent, buf.get(1..n).unwrap_or(&[])) {
            Ok(args) => return Ok(args.to_vec()),
            Err(ReplyError::Status(0x01)) => continue, // busy
            Err(ReplyError::Status(0x05)) => return Err(Error::Unsupported),
            Err(ReplyError::Status(_)) => return Err(Error::Protocol("estado Razer")),
            Err(ReplyError::Mismatch) => continue,
            Err(ReplyError::Malformed) => return Err(Error::Protocol("informe Razer")),
        }
    }
    Err(Error::Timeout)
}

impl Device for RazerKeyboard {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn path(&self) -> &CString {
        &self.path
    }

    fn read_state(&mut self, t: &dyn Transport) -> Result<State> {
        // A cheap read-only command proves the device is reachable.
        let fw = call(t, Request::FirmwareVersion)?;
        razer::parse_firmware(&fw).ok_or(Error::Protocol("firmware"))?;
        Ok(State {
            color: true,
            ..State::default()
        })
    }

    fn apply(&mut self, t: &dyn Transport, change: Change) -> Result<()> {
        match change {
            Change::Color(c) => call(t, Request::StaticColor(c)).map(|_| ()),
            _ => Err(Error::Unsupported),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{SUPPORTED, fake::FakeTransport};
    use crate::protocol::razer::Rgb;

    fn reply(sent: &[u8], status: u8, args: &[u8]) -> Vec<u8> {
        let mut r = [0u8; REPORT_LEN];
        r.copy_from_slice(&sent[1..]);
        r[0] = status;
        r[8..8 + args.len()].copy_from_slice(args);
        r[88] = razer::crc(&r);
        let mut v = vec![0];
        v.extend_from_slice(&r);
        v
    }

    #[test]
    fn reads_and_sets_color() {
        let t = FakeTransport::new(|req| vec![reply(req, 0x02, &[1, 2])]);
        let mut k = RazerKeyboard::new(&SUPPORTED[1], CString::new("x").unwrap());
        assert!(k.read_state(&t).unwrap().color);
        k.apply(&t, Change::Color(Rgb { r: 255, g: 0, b: 0 }))
            .unwrap();
        let sent = t.sent.borrow();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[1][0], 0); // report id
        assert_eq!(&sent[1][7..10], &[0x0f, 0x02, 0x00]); // static, NOSTORE
        assert_eq!(k.apply(&t, Change::Dpi(800)), Err(Error::Unsupported));
    }

    #[test]
    fn retries_while_busy() {
        let t = FakeTransport::new(|req| vec![reply(req, 0x01, &[]), reply(req, 0x02, &[1, 2])]);
        let mut k = RazerKeyboard::new(&SUPPORTED[1], CString::new("x").unwrap());
        assert!(k.read_state(&t).is_ok());
    }

    #[test]
    fn unsupported_and_silent() {
        let t = FakeTransport::new(|req| vec![reply(req, 0x05, &[])]);
        let mut k = RazerKeyboard::new(&SUPPORTED[1], CString::new("x").unwrap());
        assert_eq!(k.read_state(&t), Err(Error::Unsupported));
        let t = FakeTransport::new(|_| vec![]);
        assert!(matches!(k.read_state(&t), Err(Error::Protocol(_))));
    }
}
