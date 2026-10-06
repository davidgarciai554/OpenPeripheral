//! hidapi-backed transport and device discovery.

use crate::devices::{self, Device, Error, Result, Transport};
use hidapi::{HidApi, HidDevice};
use std::ffi::CStr;

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

/// Re-enumerates HID interfaces and probes only whitelisted ones.
pub fn scan(api: &mut HidApi) -> Vec<Box<dyn Device>> {
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
            .map(|s| (s, d.path().to_owned()))
        })
        .collect();
    let mut found = Vec::new();
    for (s, path) in candidates {
        // Opening can fail if another program holds the interface exclusively.
        let Ok(t) = HidTransport::open(api, &path) else {
            continue;
        };
        found.extend(devices::probe(s, path, &t));
    }
    found
}
