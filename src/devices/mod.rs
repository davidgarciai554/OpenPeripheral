//! Device drivers built on the pure codecs in `crate::protocol`.
//!
//! Drivers talk to hardware only through [`Transport`], which is crate-private
//! and only ever receives bytes produced by the typed request enums of the
//! codecs. Only devices in [`SUPPORTED`] and Logitech HID++ long collections
//! ([`AUTO_LOGITECH`]) are ever opened; other devices are only listed from
//! the OS enumeration, never opened.

mod asus;
#[cfg(test)]
mod catalog;
mod dualsense;
mod logitech;
mod razer;
#[cfg(test)]
pub(crate) mod sim;
mod steelseries;

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
    /// The onboard profile is not active: another program put the mouse
    /// in host mode (0x8100). This app never changes the mode.
    HostMode,
}

impl fmt::Display for Error {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Error::Io(e) => write!(f, "error de E/S: {e}"),
            Error::Timeout => write!(f, "el dispositivo no responde"),
            Error::Protocol(e) => write!(f, "respuesta no válida: {e}"),
            Error::Unsupported => write!(f, "función no admitida por el dispositivo"),
            Error::HostMode => write!(f, "otro programa controla el ratón"),
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
    /// Razer mouse found automatically (90-byte reports, volatile DPI).
    RazerMouse,
    /// SteelSeries mouse of the rivalcfg table (volatile DPI).
    SteelSeries,
    /// ASUS ROG / TUF mouse of the libratbag table (volatile DPI).
    Asus,
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
    /// Onboard-profile format verified on this hardware (CONTRIBUTING.md,
    /// rule 2). Without it the profile is never read nor written.
    pub onboard: bool,
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
        onboard: true,
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
        onboard: false,
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
        onboard: false,
    },
    // Verified 2026-10-09: profile format 4, same DPI and button layout as
    // the G305; its profile light is not verified and is never written.
    Supported {
        vid: 0x046d,
        pid: 0xc092,
        usage_page: 0xff00,
        usage: 0x0002,
        interface: None,
        driver: Driver::Hidpp,
        vendor: "Logitech",
        model: "G203 LIGHTSYNC",
        kind: Kind::Mouse,
        connection: "USB",
        wired: true,
        onboard: true,
    },
];

/// Any other Logitech HID++ long collection. Never verified on hardware:
/// volatile settings only (`onboard: false`). `pid` 0 marks it; the real one
/// comes from the enumeration.
pub static AUTO_LOGITECH: Supported = Supported {
    vid: 0x046d,
    pid: 0,
    usage_page: 0xff00,
    usage: 0x0002,
    interface: None,
    driver: Driver::Hidpp,
    vendor: "Logitech",
    model: "Logitech",
    kind: Kind::Mouse,
    connection: "USB",
    wired: true,
    onboard: false,
};

/// A Logitech mouse paired by Bluetooth: HID++ goes through the vendor
/// collection of page 0xFF43 (long reports only, device index 0xFF).
/// Volatile settings only, as `AUTO_LOGITECH`. Not verified on hardware:
/// no Bluetooth Logitech mouse was available (plan `cualquier-raton`).
pub static AUTO_LOGITECH_BT: Supported = Supported {
    vid: 0x046d,
    pid: 0,
    usage_page: 0xff43,
    usage: 0,
    interface: None,
    driver: Driver::Hidpp,
    vendor: "Logitech",
    model: "Logitech",
    kind: Kind::Mouse,
    connection: "Bluetooth",
    wired: false,
    onboard: false,
};

/// Any Razer mouse not in `SUPPORTED`: its standard collections are probed
/// with read-only commands (firmware and DPI) and only a device reporting a
/// DPI is kept. Volatile DPI only (plan `cualquier-raton`, phase 4). Not
/// verified on hardware: no Razer mouse was available.
pub static AUTO_RAZER: Supported = Supported {
    vid: 0x1532,
    pid: 0,
    usage_page: 0x0001,
    usage: 0x0002,
    interface: None,
    driver: Driver::RazerMouse,
    vendor: "Razer",
    model: "Ratón Razer",
    kind: Kind::Mouse,
    connection: "USB",
    wired: false,
    onboard: false,
};

/// A SteelSeries mouse whose DPI command is known (rivalcfg table,
/// `protocol::steelseries::MODELS`): any vendor collection (page 0xFF00 or
/// above) on that interface; with several, `scan` keeps the first. Which
/// one Windows exposes is not verified (no SteelSeries mouse available).
/// Volatile DPI only.
pub static AUTO_STEELSERIES: Supported = Supported {
    vid: 0x1038,
    pid: 0,
    usage_page: 0xff00,
    usage: 0,
    interface: None,
    driver: Driver::SteelSeries,
    vendor: "SteelSeries",
    model: "SteelSeries",
    kind: Kind::Mouse,
    connection: "USB",
    wired: true,
    onboard: false,
};

/// An ASUS ROG / TUF mouse of the libratbag table
/// (`protocol::asus::MODELS`): its vendor collections, kept only if one
/// answers the read-only requests. Volatile DPI only; not verified on
/// hardware.
pub static AUTO_ASUS: Supported = Supported {
    vid: 0x0b05,
    pid: 0,
    usage_page: 0xff01,
    usage: 0,
    interface: None,
    driver: Driver::Asus,
    vendor: "ASUS",
    model: "ASUS",
    kind: Kind::Mouse,
    connection: "USB",
    wired: false,
    onboard: false,
};

impl Supported {
    /// Detected automatically, not a verified entry.
    pub fn is_auto(&self) -> bool {
        self.pid == 0
    }
}

pub fn lookup(
    vid: u16,
    pid: u16,
    usage_page: u16,
    usage: u16,
    interface: i32,
) -> Option<&'static Supported> {
    SUPPORTED
        .iter()
        .find(|s| {
            s.vid == vid
                && s.pid == pid
                && s.usage_page == usage_page
                && s.usage == usage
                && s.interface.is_none_or(|i| i == interface)
        })
        .or_else(|| {
            (vid == AUTO_LOGITECH.vid
                && usage_page == AUTO_LOGITECH.usage_page
                && usage == AUTO_LOGITECH.usage)
                .then_some(&AUTO_LOGITECH)
        })
        // Any usage: the collection of that page is the HID++ one.
        .or_else(|| {
            (vid == AUTO_LOGITECH_BT.vid && usage_page == AUTO_LOGITECH_BT.usage_page)
                .then_some(&AUTO_LOGITECH_BT)
        })
        // Razer: the mouse and keyboard collections (the 90-byte report is
        // on one of them, which one depends on the model). Never a vid:pid
        // with its own entry (the BlackWidow keeps its driver).
        .or_else(|| {
            (vid == AUTO_RAZER.vid
                && usage_page == 0x0001
                && matches!(usage, 0x0002 | 0x0006)
                && !SUPPORTED.iter().any(|s| (s.vid, s.pid) == (vid, pid)))
            .then_some(&AUTO_RAZER)
        })
        // SteelSeries: a vendor collection of the interface that takes the
        // DPI report, for the models of the table only.
        .or_else(|| {
            (vid == AUTO_STEELSERIES.vid
                && usage_page >= 0xff00
                && crate::protocol::steelseries::model(pid)
                    .is_some_and(|m| m.interface == interface))
            .then_some(&AUTO_STEELSERIES)
        })
        // ASUS: the vendor collections of the models of the table.
        .or_else(|| {
            (vid == AUTO_ASUS.vid
                && usage_page >= 0xff00
                && crate::protocol::asus::model(pid).is_some())
            .then_some(&AUTO_ASUS)
        })
}

/// How far this app supports a device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Support {
    /// Listed in [`SUPPORTED`] and tested on hardware.
    Verified,
    /// Logitech HID++ found by [`AUTO_LOGITECH`]: volatile settings only.
    Auto,
    /// Seen in the OS enumeration only; never opened, no settings.
    Listed,
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
    pub support: Support,
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
            support: if s.is_auto() {
                Support::Auto
            } else {
                Support::Verified
            },
        }
    }

    /// A device seen only in the OS enumeration (never opened).
    pub fn listed(vid: u16, pid: u16, name: &str, kind: Kind) -> Self {
        Self {
            key: key(vid, pid, name),
            vendor: "",
            name: name.into(),
            kind,
            connection: "",
            wired: false,
            support: Support::Listed,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Dpi {
    /// `None`: the mouse cannot report it (SteelSeries) and the app has
    /// not set one since it was connected.
    pub current: Option<u16>,
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
    /// Mouse light (0x8070), not stored in the profile.
    Light(crate::protocol::onboard::Light),
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
    /// DPI accepted by the sensor (0x2201), read with the profile.
    pub dpi: Option<crate::protocol::DpiRange>,
    /// Light effect ids declared by the device (0x8070), zone 0.
    pub effects: Vec<u16>,
}

impl ProfileDump {
    pub fn limits(&self) -> crate::protocol::onboard::Limits<'_> {
        crate::protocol::onboard::Limits {
            button_count: self.info.button_count,
            dpi: self.dpi.as_ref(),
            effects: &self.effects,
        }
    }
}

pub trait Device: Send {
    fn info(&self) -> &DeviceInfo;
    /// HID path to open for every operation.
    fn path(&self) -> &CString;
    /// Device index behind a wireless receiver (HID++ 1-6), if any.
    fn receiver_index(&self) -> Option<u8> {
        None
    }
    fn read_state(&mut self, t: &dyn Transport) -> Result<State>;
    fn apply(&mut self, t: &dyn Transport, change: Change) -> Result<()>;
    /// Reads the onboard profile memory. Read only. `Unsupported` when the
    /// device has no onboard profiles or uses a format not yet verified.
    fn read_profile(&mut self, _t: &dyn Transport) -> Result<ProfileDump> {
        Err(Error::Unsupported)
    }
    /// Puts a mouse left in host mode back on its onboard profiles
    /// (HID++ 0x8100 setOnboardMode 1) and checks the mode. Only for
    /// verified models; the profile memory is not written.
    fn activate_onboard(&mut self, _t: &dyn Transport) -> Result<()> {
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
/// `pid` is the enumerated one (it differs from `s.pid` for [`AUTO_LOGITECH`]);
/// `name` is the one Windows reports, used by drivers whose protocol has
/// no name (Razer mice).
pub(crate) fn probe(
    s: &'static Supported,
    pid: u16,
    name: &str,
    path: CString,
    t: &dyn Transport,
) -> Vec<Box<dyn Device>> {
    match s.driver {
        Driver::Hidpp => logitech::probe(s, pid, path, t)
            .into_iter()
            .map(|d| Box::new(d) as Box<dyn Device>)
            .collect(),
        Driver::RazerKeyboard => vec![Box::new(RazerKeyboard::new(s, path))],
        Driver::SteelSeries => steelseries::probe(s, pid, name, path)
            .map(|d| Box::new(d) as Box<dyn Device>)
            .into_iter()
            .collect(),
        Driver::Asus => asus::probe(s, pid, name, path, t)
            .map(|d| Box::new(d) as Box<dyn Device>)
            .into_iter()
            .collect(),
        Driver::RazerMouse => razer::probe_mouse(s, pid, name, path, t)
            .map(|d| Box::new(d) as Box<dyn Device>)
            .into_iter()
            .collect(),
        Driver::DualSenseUsb => vec![Box::new(DualSense::new(s, path))],
    }
}

pub fn key_for(s: &Supported, model: &str) -> DeviceKey {
    key(s.vid, s.pid, model)
}

pub fn key(vid: u16, pid: u16, model: &str) -> DeviceKey {
    DeviceKey(format!("{vid:04x}:{pid:04x}/{model}"))
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
        // Unlisted Logitech: only its HID++ long collection, as AUTO_LOGITECH.
        assert!(lookup(0x046d, 0xc08b, 0xff00, 0x0002, 1).is_some_and(Supported::is_auto));
        assert!(lookup(0x046d, 0xc08b, 0xff00, 0x0001, 1).is_none());
        assert!(lookup(0x046d, 0xc08b, 0x0001, 0x0002, 0).is_none());
        // The verified G203 has its own entry, with the onboard profile.
        assert!(
            lookup(0x046d, 0xc092, 0xff00, 0x0002, 1).is_some_and(|s| s.onboard && !s.is_auto())
        );
        // Bluetooth: the 0xFF43 collection, as AUTO_LOGITECH_BT.
        let bt = lookup(0x046d, 0xb034, 0xff43, 0x0202, 0).unwrap();
        assert!(std::ptr::eq(bt, &AUTO_LOGITECH_BT) && bt.is_auto() && !bt.onboard);
        assert!(lookup(0x046d, 0xb034, 0x0001, 0x0002, 0).is_none());
        assert!(lookup(0x1532, 0x00b9, 0xff43, 0x0202, 0).is_none());
        // Razer mice: mouse and keyboard collections, never the BlackWidow.
        assert!(
            lookup(0x1532, 0x00b9, 0x0001, 0x0002, 0)
                .is_some_and(|s| s.driver == Driver::RazerMouse)
        );
        assert!(lookup(0x1532, 0x0099, 0x0001, 0x0006, 3).is_some_and(Supported::is_auto));
        assert!(lookup(0x1532, 0x0a24, 0x0001, 0x0002, 0).is_none());
        assert!(lookup(0x1532, 0x00b9, 0x000c, 0x0001, 0).is_none());
        // SteelSeries: the vendor collection of the model's interface only.
        assert!(
            lookup(0x1038, 0x1824, 0xffc0, 0x0001, 3)
                .is_some_and(|s| s.driver == Driver::SteelSeries)
        );
        assert!(lookup(0x1038, 0x1824, 0xffc0, 0x0001, 0).is_none());
        assert!(lookup(0x1038, 0x1824, 0x0001, 0x0002, 3).is_none());
        assert!(
            lookup(0x1038, 0x1702, 0xffc0, 0x0001, 0).is_none(),
            "not in the table"
        );
        // ASUS: vendor collections of the table's models only.
        assert!(
            lookup(0x0b05, 0x1a9b, 0xff01, 0x0001, 0).is_some_and(|s| s.driver == Driver::Asus)
        );
        assert!(lookup(0x0b05, 0x1a9b, 0x0001, 0x0002, 0).is_none());
        assert!(lookup(0x0b05, 0x1234, 0xff01, 0x0001, 0).is_none());
        // The verified receiver keeps its own entry.
        assert!(!lookup(0x046d, 0xc53f, 0xff00, 0x0002, 2).unwrap().is_auto());
        // Vendor collections of other brands are never opened.
        assert!(lookup(0x1532, 0x0528, 0xff00, 0x0002, 3).is_none());
        assert!(lookup(0xaa98, 0x1234, 0xff00, 0x55aa, 3).is_none());
    }

    #[test]
    fn keys_have_no_serial() {
        let k = key_for(&SUPPORTED[0], "G305");
        assert_eq!(k.0, "046d:c53f/G305");
    }
}
