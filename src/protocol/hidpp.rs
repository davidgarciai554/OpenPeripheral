//! Logitech HID++ 2.0 codec (pure, no I/O).
//!
//! Sources: Logitech's public HID++ 2.0 feature specifications and observation
//! of a G305 on a LIGHTSPEED receiver. See `docs/protocolos/logitech-hidpp.md`.

pub use super::{Battery, ChargeState, DpiRange};

pub const REPORT_SHORT: u8 = 0x10;
pub const REPORT_LONG: u8 = 0x11;
pub const LONG_LEN: usize = 20;

/// Software id placed in the low nibble of the function byte. Other host
/// software (e.g. G HUB) uses different ids, which lets replies be told apart.
pub const SOFTWARE_ID: u8 = 0x0b;

/// Feature ids used by this application.
pub mod feature {
    pub const ROOT: u16 = 0x0000;
    pub const DEVICE_NAME: u16 = 0x0005;
    pub const BATTERY_STATUS: u16 = 0x1000;
    pub const UNIFIED_BATTERY: u16 = 0x1004;
    pub const ADJUSTABLE_DPI: u16 = 0x2201;
    // Read only: changing it needs host mode (0x8100), excluded from v0.1.
    pub const REPORT_RATE: u16 = 0x8060;
}

/// Every request this application is able to send. There is intentionally no
/// way to build an arbitrary request: anything not listed here cannot be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Root fn1: protocol version with a ping byte echoed back.
    Ping { ping: u8 },
    /// Root fn0: look up the index of a feature id.
    GetFeature { feature: u16 },
    /// 0x0005 fn0.
    NameLength,
    /// 0x0005 fn1.
    NameChunk { offset: u8 },
    /// 0x1000 fn0.
    BatteryStatus,
    /// 0x1004 fn1.
    UnifiedBatteryStatus,
    /// 0x2201 fn1.
    DpiList { sensor: u8 },
    /// 0x2201 fn2.
    GetDpi { sensor: u8 },
    /// 0x2201 fn3. Volatile: not stored in the device profile.
    SetDpi { sensor: u8, dpi: u16 },
    /// 0x8060 fn0.
    ReportRateList,
    /// 0x8060 fn1.
    GetReportRate,
}

impl Request {
    /// Feature id this request belongs to (needed to resolve its index).
    pub fn feature(&self) -> u16 {
        use Request::*;
        match self {
            Ping { .. } | GetFeature { .. } => feature::ROOT,
            NameLength | NameChunk { .. } => feature::DEVICE_NAME,
            BatteryStatus => feature::BATTERY_STATUS,
            UnifiedBatteryStatus => feature::UNIFIED_BATTERY,
            DpiList { .. } | GetDpi { .. } | SetDpi { .. } => feature::ADJUSTABLE_DPI,
            ReportRateList | GetReportRate => feature::REPORT_RATE,
        }
    }

    fn function_and_params(&self) -> (u8, [u8; 3]) {
        use Request::*;
        match *self {
            Ping { ping } => (1, [0, 0, ping]),
            GetFeature { feature } => (0, [(feature >> 8) as u8, feature as u8, 0]),
            NameLength => (0, [0; 3]),
            NameChunk { offset } => (1, [offset, 0, 0]),
            BatteryStatus => (0, [0; 3]),
            UnifiedBatteryStatus => (1, [0; 3]),
            DpiList { sensor } => (1, [sensor, 0, 0]),
            GetDpi { sensor } => (2, [sensor, 0, 0]),
            SetDpi { sensor, dpi } => (3, [sensor, (dpi >> 8) as u8, dpi as u8]),
            ReportRateList => (0, [0; 3]),
            GetReportRate => (1, [0; 3]),
        }
    }

    /// Encodes a long (20-byte) report addressed to `device` using the
    /// already resolved `feature_index`.
    pub fn encode(&self, device: u8, feature_index: u8) -> [u8; LONG_LEN] {
        let (func, params) = self.function_and_params();
        let mut out = [0u8; LONG_LEN];
        out[0] = REPORT_LONG;
        out[1] = device;
        out[2] = feature_index;
        out[3] = (func << 4) | SOFTWARE_ID;
        out[4..7].copy_from_slice(&params);
        out
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Reply<'a> {
    /// Successful answer; payload starts after the 4-byte header.
    Ok(&'a [u8]),
    /// HID++ 2.0 error (feature index 0xFF) with its error code.
    Error(u8),
    /// HID++ 1.0 error (sub id 0x8F), e.g. device not connected to receiver.
    Error10(u8),
}

/// Matches an incoming report against the request that produced `sent`.
/// Returns `None` if the report belongs to something else (notifications,
/// other software, other devices) and should be ignored.
pub fn match_reply<'a>(sent: &[u8; LONG_LEN], report: &'a [u8]) -> Option<Reply<'a>> {
    let (&id, rest) = report.split_first()?;
    let device = *rest.first()?;
    if device != sent[1] {
        return None;
    }
    match id {
        REPORT_LONG | REPORT_SHORT => {
            let sub = *rest.get(1)?;
            let func = *rest.get(2)?;
            if id == REPORT_LONG && sub == sent[2] && func == sent[3] {
                return Some(Reply::Ok(report.get(4..)?));
            }
            if sub == 0xff && func == sent[2] && *rest.get(3)? == sent[3] {
                return Some(Reply::Error(*rest.get(4)?));
            }
            if id == REPORT_SHORT && sub == 0x8f && func == sent[2] {
                return Some(Reply::Error10(*rest.get(4)?));
            }
            None
        }
        _ => None,
    }
}

/// 0x1000 GetBatteryLevelStatus payload: level, next level, status.
pub fn parse_battery_status(p: &[u8]) -> Option<Battery> {
    let percent = *p.first()?;
    if percent > 100 {
        return None;
    }
    let charging = match *p.get(2)? {
        0 => ChargeState::Discharging,
        // 2 = final charging stage, 4 = charging below optimal speed
        1 | 2 | 4 => ChargeState::Charging,
        3 => ChargeState::Full,
        _ => ChargeState::Error,
    };
    Some(Battery { percent, charging })
}

/// 0x1004 get_status payload: state of charge, level bitmap, charging status.
pub fn parse_unified_battery(p: &[u8]) -> Option<Battery> {
    let percent = *p.first()?;
    if percent > 100 {
        return None;
    }
    let charging = match *p.get(2)? {
        0 => ChargeState::Discharging,
        1 | 2 => ChargeState::Charging,
        3 => ChargeState::Full,
        _ => ChargeState::Error,
    };
    Some(Battery { percent, charging })
}

/// 0x2201 getSensorDpiList payload: sensor index then big-endian u16 values,
/// zero terminated. A value with the top three bits set encodes a step and
/// turns the surrounding values into a range.
pub fn parse_dpi_list(p: &[u8]) -> Option<DpiRange> {
    let words: Vec<u16> = p
        .get(1..)?
        .chunks_exact(2)
        .map(|c| u16::from_be_bytes([c[0], c[1]]))
        .take_while(|&w| w != 0)
        .collect();
    if let [min, step, max] = words[..] {
        if step & 0xe000 == 0xe000 {
            let step = step & 0x1fff;
            if step == 0 || min > max {
                return None;
            }
            return Some(DpiRange::Range { min, max, step });
        }
    }
    if words.is_empty() || words.iter().any(|w| w & 0xe000 == 0xe000) {
        return None;
    }
    Some(DpiRange::Values(words))
}

/// 0x2201 getSensorDpi payload: sensor, current dpi, default dpi.
pub fn parse_dpi(p: &[u8]) -> Option<u16> {
    let dpi = u16::from_be_bytes([*p.get(1)?, *p.get(2)?]);
    (dpi != 0).then_some(dpi)
}

/// 0x8060 getReportRateList payload: bit n set means a (n + 1) ms interval
/// is supported.
pub fn parse_report_rate_list(p: &[u8]) -> Option<Vec<u8>> {
    let flags = *p.first()?;
    let rates: Vec<u8> = (0..8)
        .filter(|b| flags & (1 << b) != 0)
        .map(|b| b + 1)
        .collect();
    (!rates.is_empty()).then_some(rates)
}

/// 0x8060 getReportRate payload: interval in ms.
pub fn parse_report_rate(p: &[u8]) -> Option<u8> {
    let ms = *p.first()?;
    (1..=8).contains(&ms).then_some(ms)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodes_get_feature() {
        let r = Request::GetFeature { feature: 0x1000 }.encode(1, 0);
        assert_eq!(&r[..7], &[0x11, 0x01, 0x00, 0x0b, 0x10, 0x00, 0x00]);
        assert!(r[7..].iter().all(|&b| b == 0));
    }

    #[test]
    fn encodes_set_dpi_big_endian() {
        let r = Request::SetDpi {
            sensor: 0,
            dpi: 1600,
        }
        .encode(1, 0x0b);
        assert_eq!(&r[..7], &[0x11, 0x01, 0x0b, 0x3b, 0x00, 0x06, 0x40]);
    }

    #[test]
    fn matches_own_reply_only() {
        let sent = Request::BatteryStatus.encode(1, 4);
        let mut ok = [0u8; 20];
        ok[..7].copy_from_slice(&[0x11, 0x01, 0x04, 0x0b, 90, 50, 0]);
        assert_eq!(match_reply(&sent, &ok), Some(Reply::Ok(&ok[4..])));

        let mut other_sw = ok;
        other_sw[3] = 0x0a; // same function, other software id
        assert_eq!(match_reply(&sent, &other_sw), None);

        let mut other_dev = ok;
        other_dev[1] = 2;
        assert_eq!(match_reply(&sent, &other_dev), None);
    }

    #[test]
    fn matches_errors() {
        let sent = Request::BatteryStatus.encode(1, 4);
        let e20 = [
            0x11, 0x01, 0xff, 0x04, 0x0b, 0x05, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
        ];
        assert_eq!(match_reply(&sent, &e20), Some(Reply::Error(5)));
        let e10 = [0x10, 0x01, 0x8f, 0x04, 0x0b, 0x09, 0x00];
        assert_eq!(match_reply(&sent, &e10), Some(Reply::Error10(9)));
    }

    #[test]
    fn truncated_reports_are_ignored() {
        let sent = Request::BatteryStatus.encode(1, 4);
        for len in 0..4 {
            let r = [0x11, 0x01, 0x04, 0x0b];
            assert_eq!(match_reply(&sent, &r[..len]), None);
        }
        assert_eq!(match_reply(&sent, &[0x11, 0x01, 0xff, 0x04, 0x0b]), None);
    }

    #[test]
    fn battery_status() {
        assert_eq!(
            parse_battery_status(&[90, 50, 0]),
            Some(Battery {
                percent: 90,
                charging: ChargeState::Discharging
            })
        );
        assert_eq!(
            parse_battery_status(&[100, 0, 3]).unwrap().charging,
            ChargeState::Full
        );
        assert_eq!(
            parse_battery_status(&[0, 0, 1]).unwrap().charging,
            ChargeState::Charging
        );
        assert_eq!(parse_battery_status(&[101, 0, 0]), None);
        assert_eq!(parse_battery_status(&[50, 0]), None);
    }

    #[test]
    fn unified_battery() {
        assert_eq!(
            parse_unified_battery(&[42, 0x04, 1, 1]),
            Some(Battery {
                percent: 42,
                charging: ChargeState::Charging
            })
        );
        assert_eq!(
            parse_unified_battery(&[42, 4, 9]).unwrap().charging,
            ChargeState::Error
        );
    }

    #[test]
    fn dpi_range_and_values() {
        // G-series style: 200..12000 step 50
        let p = [0, 0x00, 0xc8, 0xe0, 0x32, 0x2e, 0xe0, 0, 0];
        let l = parse_dpi_list(&p).unwrap();
        assert_eq!(
            l,
            DpiRange::Range {
                min: 200,
                max: 12000,
                step: 50
            }
        );
        assert!(l.contains(1600));
        assert!(!l.contains(1625));
        assert!(!l.contains(12050));

        let p = [0, 0x01, 0x90, 0x03, 0x20, 0, 0];
        assert_eq!(parse_dpi_list(&p), Some(DpiRange::Values(vec![400, 800])));
        assert_eq!(parse_dpi_list(&[0]), None);
        assert_eq!(
            parse_dpi_list(&[0, 0x00, 0xc8, 0xe0, 0x00, 0x2e, 0xe0]),
            None
        );
    }

    #[test]
    fn dpi_and_rates() {
        assert_eq!(parse_dpi(&[0, 0x06, 0x40, 0x03, 0x20]), Some(1600));
        assert_eq!(parse_dpi(&[0, 0, 0]), None);
        assert_eq!(
            parse_report_rate_list(&[0b1000_1011]),
            Some(vec![1, 2, 4, 8])
        );
        assert_eq!(parse_report_rate_list(&[0]), None);
        assert_eq!(parse_report_rate(&[1]), Some(1));
        assert_eq!(parse_report_rate(&[0]), None);
    }
}
