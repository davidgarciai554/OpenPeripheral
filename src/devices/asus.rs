//! ASUS ROG / TUF mice of the libratbag table: volatile DPI of the active
//! preset only (plan `cualquier-raton`, phase 4). The device is kept only if
//! it answers the read-only profile and settings requests. Not verified on
//! hardware: no ASUS mouse was available.

use super::{Change, Device, DeviceInfo, Dpi, Error, Result, State, Supported, Transport};
use crate::protocol::asus::{self, Model, PACKET};
use std::ffi::CString;
use std::time::{Duration, Instant};

const REPLY_TIMEOUT: Duration = Duration::from_millis(300);

pub struct AsusMouse {
    info: DeviceInfo,
    path: CString,
    model: &'static Model,
}

/// Sends a request and returns the next report (the device answers in
/// order; it has no request id).
fn query(t: &dyn Transport, request: &[u8; PACKET + 1]) -> Result<Vec<u8>> {
    // A late answer to an earlier request (or any report the mouse sent on
    // its own) must not be taken for this one.
    let mut stale = [0u8; PACKET + 1];
    while t.read_timeout(&mut stale, 0)? > 0 {}
    t.write(request)?;
    let deadline = Instant::now() + REPLY_TIMEOUT;
    let mut buf = [0u8; PACKET + 1];
    while Instant::now() < deadline {
        let n = t.read_timeout(&mut buf, 50)?;
        if n > 0 {
            return asus::checked(&buf[..n])
                .map(<[u8]>::to_vec)
                .ok_or(Error::Timeout);
        }
    }
    Err(Error::Timeout)
}

impl AsusMouse {
    /// Active preset and its DPI.
    fn current(&self, t: &dyn Transport) -> Result<(usize, u16)> {
        let preset = asus::active_preset(self.model, &query(t, &asus::get_profile_data())?)
            .ok_or(Error::Protocol("preset ASUS"))?;
        let dpi = self
            .model
            .preset_dpi(&query(t, &asus::get_settings())?, preset)
            .ok_or(Error::Protocol("DPI ASUS"))?;
        Ok((preset, dpi))
    }
}

/// The mouse for a whitelisted ASUS pid, if it answers the read-only
/// requests on this collection.
pub fn probe(
    s: &'static Supported,
    pid: u16,
    name: &str,
    path: CString,
    t: &dyn Transport,
) -> Option<AsusMouse> {
    let model = asus::model(pid)?;
    let name = if name.is_empty() { model.name } else { name };
    let mut info = DeviceInfo::new(s, name);
    info.key = super::key(s.vid, pid, name);
    let mouse = AsusMouse { info, path, model };
    mouse.current(t).ok()?;
    Some(mouse)
}

impl Device for AsusMouse {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn path(&self) -> &CString {
        &self.path
    }

    fn read_state(&mut self, t: &dyn Transport) -> Result<State> {
        let (_, dpi) = self.current(t)?;
        Ok(State {
            dpi: Some(Dpi {
                current: Some(dpi),
                allowed: self.model.range(),
            }),
            ..State::default()
        })
    }

    fn apply(&mut self, t: &dyn Transport, change: Change) -> Result<()> {
        let Change::Dpi(dpi) = change else {
            return Err(Error::Unsupported);
        };
        let (preset, _) = self.current(t)?;
        let request = self.model.set_dpi(preset, dpi).ok_or(Error::Unsupported)?;
        query(t, &request).map(|_| ())
    }
}
