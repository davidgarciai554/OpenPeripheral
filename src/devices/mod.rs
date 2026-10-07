//! Device drivers built on the pure codecs in `crate::protocol`.
//!
//! Drivers talk to hardware only through [`Transport`], which is crate-private
//! and only ever receives bytes produced by the typed request enums of the
//! codecs. Only devices in [`SUPPORTED`] are ever opened.

mod dualsense;
mod logitech;
mod razer;

pub use crate::protocol::DpiRange;
use crate::protocol::{Battery, razer::Rgb};
use std::ffi::CString;
use std::fmt;

pub use dualsense::DualSense;
pub use razer::RazerKeyboard;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// The OS refused or failed the I/O (unplugged, exclusive access...).
    Io(String),
    /// No answer in time (device asleep, out of range, or busy).
    Timeout,
    /// The device answered something this application does not accept.
    Protocol(&'static str),
    /// The device does not offer this function or value.
    Unsupported,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "error de E/S: {e}"),
            Error::Timeout => write!(f, "el dispositivo no responde"),
            Error::Protocol(e) => write!(f, "respuesta no válida: {e}"),
            Error::Unsupported => write!(f, "función no admitida por el dispositivo"),
        }
    }
}

pub type Result<T> = std::result::Result<T, Error>;

/// Minimal HID access needed by the drivers. Implemented over hidapi in
/// `crate::platform::hid` and by a scripted fake in tests.
pub(crate) trait Transport {
    fn write(&self, data: &[u8]) -> Result<()>;
    /// Returns 0 on timeout.
    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize>;
    /// `data[0]` is the report id.
    fn send_feature(&self, data: &[u8]) -> Result<()>;
    /// `buf[0]` must hold the report id; returns the length read.
    fn get_feature(&self, buf: &mut [u8]) -> Result<usize>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Driver {
    /// Logitech receiver or device speaking HID++ 2.0.
    Hidpp,
    RazerKeyboard,
    DualSenseUsb,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Mouse,
    Keyboard,
    Gamepad,
}

/// Whitelisted HID interface. Only verified hardware is listed here; see
/// CONTRIBUTING.md before adding entries.
#[derive(Debug, Clone, Copy)]
pub struct Supported {
    pub vid: u16,
    pub pid: u16,
    pub usage_page: u16,
    pub usage: u16,
    pub interface: Option<i32>,
    pub driver: Driver,
    pub vendor: &'static str,
    /// Name shown when the device does not report one itself.
    pub model: &'static str,
    pub kind: Kind,
    /// How it reaches the PC, as shown to the user.
    pub connection: &'static str,
    /// Powered by its cable (shown as "Con cable" when it reports no battery).
    pub wired: bool,
}

pub const SUPPORTED: &[Supported] = &[
    Supported {
        vid: 0x046d,
        pid: 0xc53f,
        usage_page: 0xff00,
        usage: 0x0002,
        interface: None,
        driver: Driver::Hidpp,
        vendor: "Logitech",
        model: "Receptor LIGHTSPEED",
        kind: Kind::Mouse,
        connection: "Receptor LIGHTSPEED",
        wired: false,
    },
    Supported {
        vid: 0x1532,
        pid: 0x0a24,
        usage_page: 0x0001,
        usage: 0x0002,
        interface: Some(2),
        driver: Driver::RazerKeyboard,
        vendor: "Razer",
        model: "BlackWidow V3 Tenkeyless",
        kind: Kind::Keyboard,
        connection: "USB",
        wired: true,
    },
    Supported {
        vid: 0x054c,
        pid: 0x0ce6,
        usage_page: 0x0001,
        usage: 0x0005,
        interface: None,
        driver: Driver::DualSenseUsb,
        vendor: "Sony",
        model: "DualSense",
        kind: Kind::Gamepad,
        connection: "USB",
        wired: true,
    },
];

pub fn lookup(
    vid: u16,
    pid: u16,
    usage_page: u16,
    usage: u16,
    interface: i32,
) -> Option<&'static Supported> {
    SUPPORTED.iter().find(|s| {
        s.vid == vid
            && s.pid == pid
            && s.usage_page == usage_page
            && s.usage == usage
            && s.interface.is_none_or(|i| i == interface)
    })
}

/// Stable identity used for saved settings. Contains no serial number.
#[derive(Debug, Clone, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct DeviceKey(pub String);

#[derive(Debug, Clone)]
pub struct DeviceInfo {
    pub key: DeviceKey,
    pub vendor: &'static str,
    pub name: String,
    pub kind: Kind,
    pub connection: &'static str,
    pub wired: bool,
}

impl DeviceInfo {
    pub fn new(s: &Supported, name: &str) -> Self {
        Self {
            key: key_for(s, name),
            vendor: s.vendor,
            name: name.into(),
            kind: s.kind,
            connection: s.connection,
            wired: s.wired,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dpi {
    pub current: u16,
    pub allowed: DpiRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReportRate {
    /// Interval in ms (1 ms = 1000 Hz).
    pub current_ms: u8,
    pub supported_ms: Vec<u8>,
}

/// What the device currently reports. `None` means not offered or unreadable.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    pub battery: Option<Battery>,
    pub dpi: Option<Dpi>,
    pub report_rate: Option<ReportRate>,
    /// The device accepts a colour, which cannot be read back.
    pub color: bool,
}

/// Volatile changes this application can make.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Change {
    Dpi(u16),
    Color(Rgb),
}

/// Raw onboard-profile memory read from a mouse (HID++ 0x8100). Kept by the
/// worker for backups; never sent anywhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileDump {
    pub info: crate::protocol::onboard::Info,
    /// Sector of the active user profile.
    pub sector: u16,
    pub directory: Vec<u8>,
    pub user: Vec<u8>,
    /// Factory (ROM) copy of the profile.
    pub rom: Vec<u8>,
}

pub trait Device: Send {
    fn info(&self) -> &DeviceInfo;
    /// HID path to open for every operation.
    fn path(&self) -> &CString;
    fn read_state(&mut self, t: &dyn Transport) -> Result<State>;
    fn apply(&mut self, t: &dyn Transport, change: Change) -> Result<()>;
    /// Reads the onboard profile memory. Read only. `Unsupported` when the
    /// device has no onboard profiles or uses a format not yet verified.
    fn read_profile(&mut self, _t: &dyn Transport) -> Result<ProfileDump> {
        Err(Error::Unsupported)
    }
    /// Writes a validated profile image and reads the sector back. Returns
    /// `Ok` only if the device now holds exactly the image's bytes.
    fn write_profile(
        &mut self,
        _t: &dyn Transport,
        _image: &crate::protocol::onboard::ProfileImage,
    ) -> Result<()> {
        Err(Error::Unsupported)
    }
}

/// Builds the devices behind one whitelisted HID interface. A Logitech
/// receiver may yield several devices; a dead receiver yields none.
pub(crate) fn probe(
    s: &'static Supported,
    path: CString,
    t: &dyn Transport,
) -> Vec<Box<dyn Device>> {
    match s.driver {
        Driver::Hidpp => logitech::probe(s, path, t)
            .into_iter()
            .map(|d| Box::new(d) as Box<dyn Device>)
            .collect(),
        Driver::RazerKeyboard => vec![Box::new(RazerKeyboard::new(s, path))],
        Driver::DualSenseUsb => vec![Box::new(DualSense::new(s, path))],
    }
}

pub fn key_for(s: &Supported, model: &str) -> DeviceKey {
    DeviceKey(format!("{:04x}:{:04x}/{}", s.vid, s.pid, model))
}

#[cfg(test)]
pub(crate) mod fake {
    //! Scripted transport: each write or feature set is answered by the
    //! closure, and the answers are queued for subsequent reads.
    use super::{Result, Transport};
    use std::cell::RefCell;
    use std::collections::VecDeque;

    type Responder = Box<dyn Fn(&[u8]) -> Vec<Vec<u8>>>;

    pub struct FakeTransport {
        respond: Responder,
        queue: RefCell<VecDeque<Vec<u8>>>,
        pub sent: RefCell<Vec<Vec<u8>>>,
    }

    impl FakeTransport {
        pub fn new(respond: impl Fn(&[u8]) -> Vec<Vec<u8>> + 'static) -> Self {
            Self {
                respond: Box::new(respond),
                queue: Default::default(),
                sent: Default::default(),
            }
        }
        /// Queues an unsolicited input report.
        pub fn push(&self, report: Vec<u8>) {
            self.queue.borrow_mut().push_back(report);
        }
        fn take(&self, data: &[u8]) {
            self.sent.borrow_mut().push(data.to_vec());
            let replies = (self.respond)(data);
            self.queue.borrow_mut().extend(replies);
        }
        fn pop_into(&self, buf: &mut [u8]) -> usize {
            match self.queue.borrow_mut().pop_front() {
                Some(r) => {
                    let n = r.len().min(buf.len());
                    buf[..n].copy_from_slice(&r[..n]);
                    n
                }
                None => 0,
            }
        }
    }

    impl Transport for FakeTransport {
        fn write(&self, data: &[u8]) -> Result<()> {
            self.take(data);
            Ok(())
        }
        fn read_timeout(&self, buf: &mut [u8], _: i32) -> Result<usize> {
            Ok(self.pop_into(buf))
        }
        fn send_feature(&self, data: &[u8]) -> Result<()> {
            self.take(data);
            Ok(())
        }
        fn get_feature(&self, buf: &mut [u8]) -> Result<usize> {
            Ok(self.pop_into(buf))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn whitelist_lookup() {
        assert!(lookup(0x046d, 0xc53f, 0xff00, 0x0002, 2).is_some());
        // Same receiver, other collection: not opened.
        assert!(lookup(0x046d, 0xc53f, 0xff00, 0x0001, 2).is_none());
        // Razer keyboard only on its control interface.
        assert!(lookup(0x1532, 0x0a24, 0x0001, 0x0002, 2).is_some());
        assert!(lookup(0x1532, 0x0a24, 0x0001, 0x0002, 1).is_none());
        assert!(lookup(0x1234, 0x5678, 0x0001, 0x0002, 0).is_none());
    }

    #[test]
    fn keys_have_no_serial() {
        let k = key_for(&SUPPORTED[0], "G305");
        assert_eq!(k.0, "046d:c53f/G305");
    }
}
