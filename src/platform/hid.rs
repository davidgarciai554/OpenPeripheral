//! hidapi-backed transport and device discovery.

use crate::devices::{self, Device, DeviceInfo, Error, Kind, Result, Transport};
use crate::protocol::hidpp;
use hidapi::{HidApi, HidDevice};
use std::collections::HashSet;
use std::ffi::{CStr, CString};

pub struct HidTransport(HidDevice);

fn io(e: hidapi::HidError) -> Error {
    Error::Io(e.to_string())
}

impl HidTransport {
    pub fn open(api: &HidApi, path: &CStr) -> Result<Self> {
        api.open_path(path).map(HidTransport).map_err(io)
    }
}

impl Transport for HidTransport {
    fn write(&self, data: &[u8]) -> Result<()> {
        self.0.write(data).map(|_| ()).map_err(io)
    }
    fn read_timeout(&self, buf: &mut [u8], timeout_ms: i32) -> Result<usize> {
        self.0.read_timeout(buf, timeout_ms).map_err(io)
    }
    fn send_feature(&self, data: &[u8]) -> Result<()> {
        self.0.send_feature_report(data).map_err(io)
    }
    fn get_feature(&self, buf: &mut [u8]) -> Result<usize> {
        self.0.get_feature_report(buf).map_err(io)
    }
}

/// Short HID++ collection (usage page 0xFF00, usage 0x0001) of every
/// whitelisted HID++ receiver: (vid, pid, path). Its device-connection
/// notifications arrive there. Uses the last enumeration of `api`.
pub fn receivers(api: &HidApi) -> Vec<(u16, u16, CString)> {
    let accepted: Vec<(u16, u16, i32)> = api
        .device_list()
        .filter(|d| {
            devices::lookup(
                d.vendor_id(),
                d.product_id(),
                d.usage_page(),
                d.usage(),
                d.interface_number(),
            )
            .is_some_and(|s| s.driver == devices::Driver::Hidpp)
        })
        .map(|d| (d.vendor_id(), d.product_id(), d.interface_number()))
        .collect();
    api.device_list()
        .filter(|d| {
            d.usage_page() == 0xff00
                && d.usage() == 0x0001
                && accepted.contains(&(d.vendor_id(), d.product_id(), d.interface_number()))
        })
        .map(|d| (d.vendor_id(), d.product_id(), d.path().to_owned()))
        .collect()
}

/// Re-enumerates HID interfaces and probes only whitelisted ones.
/// `skip` holds the [`devices::AUTO_LOGITECH`] paths that yielded nothing:
/// they are not probed again (a silent one costs 7 × 400 ms) until they
/// leave the enumeration or the caller clears the set.
pub fn scan(api: &mut HidApi, skip: &mut HashSet<CString>) -> Vec<Box<dyn Device>> {
    if api.refresh_devices().is_err() {
        return Vec::new();
    }
    let candidates: Vec<_> = api
        .device_list()
        .filter_map(|d| {
            devices::lookup(
                d.vendor_id(),
                d.product_id(),
                d.usage_page(),
                d.usage(),
                d.interface_number(),
            )
            .map(|s| {
                let name = listed_name(
                    d.manufacturer_string().unwrap_or(""),
                    d.product_string().unwrap_or(""),
                );
                (s, d.product_id(), name, d.path().to_owned())
            })
        })
        .collect();
    skip.retain(|p| candidates.iter().any(|(_, _, _, c)| c == p));
    let mut found: Vec<Box<dyn Device>> = Vec::new();
    for (s, pid, name, path) in candidates {
        if skip.contains(&path) {
            continue;
        }
        // Opening can fail if another program holds the interface exclusively.
        let Ok(t) = HidTransport::open(api, &path) else {
            continue;
        };
        let devs = devices::probe(s, pid, &name, path.clone(), &t);
        if devs.is_empty() && s.is_auto() {
            skip.insert(path);
        }
        // Several collections of one mouse can yield the same device
        // (SteelSeries vendor pages): the first one is kept.
        for d in devs {
            if !found.iter().any(|f| f.info().key == d.info().key) {
                found.push(d);
            }
        }
    }
    found
}

/// Kind shown for a top-level HID collection (generic desktop page).
pub(crate) fn listed_kind(usage_page: u16, usage: u16) -> Option<Kind> {
    match (usage_page, usage) {
        (0x0001, 0x0002) => Some(Kind::Mouse),
        (0x0001, 0x0006) => Some(Kind::Keyboard),
        (0x0001, 0x0004 | 0x0005) => Some(Kind::Gamepad),
        _ => None,
    }
}

/// Mice, keyboards and gamepads in the last enumeration of `api` that no
/// driver handles, one per vid:pid. Only enumeration data is used: these
/// devices are never opened. Skips vid:pids of [`devices::SUPPORTED`] and
/// every vid:pid with a collection a driver accepts, even when it found
/// nothing (asleep, or not a mouse): it would otherwise show twice. A Razer
/// device is the exception: most do not answer the mouse probe (keyboards,
/// headsets), so it stays listed and the worker drops the listing when the
/// probe found it (`app::rescan`).
pub fn listed(api: &HidApi) -> Vec<DeviceInfo> {
    let known: Vec<(u16, u16)> = api
        .device_list()
        .filter(|d| {
            devices::lookup(
                d.vendor_id(),
                d.product_id(),
                d.usage_page(),
                d.usage(),
                d.interface_number(),
            )
            .is_some_and(|s| s.driver != devices::Driver::RazerMouse)
        })
        .map(|d| (d.vendor_id(), d.product_id()))
        .collect();
    let mut out: Vec<(u16, u16, DeviceInfo)> = Vec::new();
    for d in api.device_list() {
        let (vid, pid) = (d.vendor_id(), d.product_id());
        let Some(kind) = listed_kind(d.usage_page(), d.usage()) else {
            continue;
        };
        if known.contains(&(vid, pid))
            || devices::SUPPORTED
                .iter()
                .any(|s| (s.vid, s.pid) == (vid, pid))
            || out.iter().any(|(v, p, _)| (*v, *p) == (vid, pid))
        {
            continue;
        }
        let name = listed_name(
            d.manufacturer_string().unwrap_or(""),
            d.product_string().unwrap_or(""),
        );
        if name.is_empty() {
            continue;
        }
        out.push((vid, pid, DeviceInfo::listed(vid, pid, &name, kind)));
    }
    out.into_iter().map(|(_, _, i)| i).collect()
}

/// "Razer" + "Razer Basilisk" -> "Razer Basilisk"; "MSI" + "Mouse" -> "MSI Mouse".
fn listed_name(manufacturer: &str, product: &str) -> String {
    let (m, p) = (
        hidpp::clean_name(manufacturer.as_bytes()),
        hidpp::clean_name(product.as_bytes()),
    );
    if p.is_empty() || m.is_empty() || p.to_lowercase().starts_with(&m.to_lowercase()) {
        p
    } else {
        hidpp::clean_name(format!("{m} {p}").as_bytes())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn listed_collections_and_names() {
        assert_eq!(listed_kind(0x0001, 0x0002), Some(Kind::Mouse));
        assert_eq!(listed_kind(0x0001, 0x0006), Some(Kind::Keyboard));
        assert_eq!(listed_kind(0x0001, 0x0005), Some(Kind::Gamepad));
        // Vendor, consumer and system-control collections are not devices.
        assert_eq!(listed_kind(0xff00, 0x0002), None);
        assert_eq!(listed_kind(0x000c, 0x0001), None);
        assert_eq!(listed_kind(0x0001, 0x0080), None);
        assert_eq!(
            listed_name("Razer", "Razer Basilisk V3"),
            "Razer Basilisk V3"
        );
        assert_eq!(
            listed_name("mvsilicon", "USB MIC Device"),
            "mvsilicon USB MIC Device"
        );
        assert_eq!(listed_name("", "Mouse"), "Mouse");
        assert_eq!(listed_name("Acme", ""), "");
    }
}
