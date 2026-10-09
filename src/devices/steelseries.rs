//! SteelSeries mice with a DPI command known from rivalcfg: volatile DPI
//! only. They never report their DPI, so the current one is known only
//! after this app sets it (plan `cualquier-raton`, phase 4). Not verified
//! on hardware: no SteelSeries mouse was available.

use super::{Change, Device, DeviceInfo, Dpi, Error, Result, State, Supported, Transport};
use crate::protocol::steelseries::{self, Model};
use std::ffi::CString;

pub struct SteelSeriesMouse {
    info: DeviceInfo,
    path: CString,
    model: &'static Model,
    /// Last DPI this app set (the mouse cannot be asked).
    current: Option<u16>,
}

/// The mouse for a whitelisted SteelSeries pid; nothing is sent.
pub fn probe(
    s: &'static Supported,
    pid: u16,
    name: &str,
    path: CString,
) -> Option<SteelSeriesMouse> {
    let model = steelseries::model(pid)?;
    let name = if name.is_empty() { model.name } else { name };
    let mut info = DeviceInfo::new(s, name);
    info.key = super::key(s.vid, pid, name);
    Some(SteelSeriesMouse {
        info,
        path,
        model,
        current: None,
    })
}

impl Device for SteelSeriesMouse {
    fn info(&self) -> &DeviceInfo {
        &self.info
    }

    fn path(&self) -> &CString {
        &self.path
    }

    fn read_state(&mut self, _t: &dyn Transport) -> Result<State> {
        Ok(State {
            dpi: Some(Dpi {
                current: self.current,
                allowed: self.model.range(),
            }),
            ..State::default()
        })
    }

    fn apply(&mut self, t: &dyn Transport, change: Change) -> Result<()> {
        let Change::Dpi(dpi) = change else {
            return Err(Error::Unsupported);
        };
        let report = self.model.dpi_report(dpi).ok_or(Error::Unsupported)?;
        t.write(&report)?;
        self.current = Some(dpi);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{AUTO_STEELSERIES, Support, fake::FakeTransport};

    #[test]
    fn sets_a_volatile_dpi_and_remembers_it() {
        let t = FakeTransport::new(|_| vec![]);
        let mut d = probe(&AUTO_STEELSERIES, 0x1824, "", CString::new("x").unwrap()).unwrap();
        assert_eq!(d.info().name, "SteelSeries Rival 3");
        assert_eq!(d.info().support, Support::Auto);
        assert_eq!(d.read_state(&t).unwrap().dpi.unwrap().current, None);
        d.apply(&t, Change::Dpi(1600)).unwrap();
        assert_eq!(d.read_state(&t).unwrap().dpi.unwrap().current, Some(1600));
        assert_eq!(d.apply(&t, Change::Dpi(1650)), Err(Error::Unsupported));
        let sent = t.sent.borrow();
        assert_eq!(sent.len(), 1, "one report, never a save command");
        assert_eq!(&sent[0][..5], &[0, 0x0b, 0x00, 1, 1]);
        assert!(probe(&AUTO_STEELSERIES, 0x1702, "", CString::new("x").unwrap()).is_none());
    }
}
