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
    pub const ONBOARD_PROFILES: u16 = 0x8100;
    pub const COLOR_LED_EFFECTS: u16 = 0x8070;
}

/// Every request this application is able to send. There is intentionally no
/// way to build an arbitrary request: anything not listed here cannot be sent.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Root fn1: protocol version with a ping byte echoed back.
    Ping {
        ping: u8,
    },
    /// Root fn0: look up the index of a feature id.
    GetFeature {
        feature: u16,
    },
    /// 0x0005 fn0.
    NameLength,
    /// 0x0005 fn1.
    NameChunk {
        offset: u8,
    },
    /// 0x0005 fn2 getDeviceType.
    DeviceType,
    /// 0x1000 fn0.
    BatteryStatus,
    /// 0x1004 fn1.
    UnifiedBatteryStatus,
    /// 0x2201 fn1.
    DpiList {
        sensor: u8,
    },
    /// 0x2201 fn2.
    GetDpi {
        sensor: u8,
    },
    /// 0x2201 fn3. Volatile: not stored in the device profile.
    SetDpi {
        sensor: u8,
        dpi: u16,
    },
    /// 0x8060 fn0.
    ReportRateList,
    /// 0x8060 fn1.
    GetReportRate,
    /// 0x8070 fn0: number of light zones. Read only.
    LedInfo,
    /// 0x8070 fn1: zone location and number of effects. Read only.
    LedZoneInfo {
        zone: u8,
    },
    /// 0x8070 fn2: id of the `index`-th effect of a zone. Read only.
    LedEffectInfo {
        zone: u8,
        index: u8,
    },
    /// 0x8070 fn3 setZoneEffect on zone 0. Volatile: the mouse returns to
    /// its profile light when switched off. The last (persistence) byte is
    /// always 0.
    SetLight {
        /// Position of the effect in the zone's effect list.
        effect_index: u8,
        light: super::onboard::Light,
    },
    /// 0x8100 fn0: memory layout (sector size, button count…).
    OnboardInfo,
    /// 0x8100 fn2 getOnboardMode: 1 onboard profiles, 2 host. Read only.
    GetOnboardMode,
    /// 0x8100 fn1 setOnboardMode(1): back to the onboard profiles. Never
    /// sets host mode. Verified on the G203 (2026-10-09): stays after
    /// reconnecting; the profile memory is not touched.
    ActivateOnboard,
    /// 0x8100 fn4: sector of the active profile.
    GetCurrentProfile,
    /// 0x8100 fn5: 16 bytes of a memory sector. Read only.
    MemoryRead {
        sector: u16,
        offset: u16,
    },
    /// 0x8100 fn6/fn7/fn8: write a memory sector. Only buildable through
    /// `onboard::ProfileImage::write_requests` (the `seal` field).
    MemoryWriteStart {
        sector: u16,
        offset: u16,
        len: u16,
        seal: super::onboard::Seal,
    },
    MemoryWrite {
        data: [u8; 16],
        seal: super::onboard::Seal,
    },
    MemoryWriteEnd {
        seal: super::onboard::Seal,
    },
}

impl Request {
    /// Feature id this request belongs to (needed to resolve its index).
    pub fn feature(&self) -> u16 {
        use Request::*;
        match self {
            Ping { .. } | GetFeature { .. } => feature::ROOT,
            NameLength | NameChunk { .. } | DeviceType => feature::DEVICE_NAME,
            BatteryStatus => feature::BATTERY_STATUS,
            UnifiedBatteryStatus => feature::UNIFIED_BATTERY,
            DpiList { .. } | GetDpi { .. } | SetDpi { .. } => feature::ADJUSTABLE_DPI,
            ReportRateList | GetReportRate => feature::REPORT_RATE,
            LedInfo | LedZoneInfo { .. } | LedEffectInfo { .. } | SetLight { .. } => {
                feature::COLOR_LED_EFFECTS
            }
            OnboardInfo
            | GetOnboardMode
            | ActivateOnboard
            | GetCurrentProfile
            | MemoryRead { .. }
            | MemoryWriteStart { .. }
            | MemoryWrite { .. }
            | MemoryWriteEnd { .. } => feature::ONBOARD_PROFILES,
        }
    }

    fn function_and_params(&self) -> (u8, [u8; 16]) {
        use Request::*;
        let p4 = |b: [u8; 4]| {
            let mut p = [0u8; 16];
            p[..4].copy_from_slice(&b);
            p
        };
        match *self {
            Ping { ping } => (1, p4([0, 0, ping, 0])),
            GetFeature { feature } => (0, p4([(feature >> 8) as u8, feature as u8, 0, 0])),
            NameLength => (0, p4([0; 4])),
            NameChunk { offset } => (1, p4([offset, 0, 0, 0])),
            DeviceType => (2, p4([0; 4])),
            BatteryStatus => (0, p4([0; 4])),
            UnifiedBatteryStatus => (1, p4([0; 4])),
            DpiList { sensor } => (1, p4([sensor, 0, 0, 0])),
            GetDpi { sensor } => (2, p4([sensor, 0, 0, 0])),
            SetDpi { sensor, dpi } => (3, p4([sensor, (dpi >> 8) as u8, dpi as u8, 0])),
            ReportRateList => (0, p4([0; 4])),
            GetReportRate => (1, p4([0; 4])),
            LedInfo => (0, p4([0; 4])),
            LedZoneInfo { zone } => (1, p4([zone, 0, 0, 0])),
            LedEffectInfo { zone, index } => (2, p4([zone, index, 0, 0])),
            SetLight {
                effect_index,
                light,
            } => {
                let mut p = [0u8; 16];
                p[1] = effect_index;
                p[2..12].copy_from_slice(&light.params());
                (3, p)
            }
            OnboardInfo => (0, p4([0; 4])),
            GetOnboardMode => (2, p4([0; 4])),
            ActivateOnboard => (1, p4([1, 0, 0, 0])),
            GetCurrentProfile => (4, p4([0; 4])),
            MemoryRead { sector, offset } => {
                let ([s1, s0], [o1, o0]) = (sector.to_be_bytes(), offset.to_be_bytes());
                (5, p4([s1, s0, o1, o0]))
            }
            MemoryWriteStart {
                sector,
                offset,
                len,
                ..
            } => {
                let ([s1, s0], [o1, o0], [l1, l0]) = (
                    sector.to_be_bytes(),
                    offset.to_be_bytes(),
                    len.to_be_bytes(),
                );
                let mut p = [0u8; 16];
                p[..6].copy_from_slice(&[s1, s0, o1, o0, l1, l0]);
                (6, p)
            }
            MemoryWrite { data, .. } => (7, data),
            MemoryWriteEnd { .. } => (8, [0; 16]),
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
        out[4..20].copy_from_slice(&params);
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

/// Link state reported by the receiver for one of its devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Link {
    Up,
    Down,
}

/// 0x0005 fn2 reply: whether the device is a pointing device (mouse 3,
/// trackpad 4 or trackball 5 in the public deviceType table).
pub fn is_pointer(payload: &[u8]) -> Option<bool> {
    payload.first().map(|t| matches!(t, 3..=5))
}

/// Longest device name kept, in characters.
pub const NAME_MAX: usize = 64;

/// Name reported by the device (0x0005), made safe to show and to use in a
/// settings key: no control or bidirectional-override characters, trimmed,
/// at most [`NAME_MAX`] characters.
pub fn clean_name(raw: &[u8]) -> String {
    String::from_utf8_lossy(raw)
        .chars()
        .filter(|c| {
            !c.is_control()
                && !matches!(c, '\u{200e}' | '\u{200f}' | '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')
        })
        .take(NAME_MAX)
        .collect::<String>()
        .trim()
        .to_string()
}

/// Receiver notification 0x41 (HID++ 1.0 short report): device `index`
/// (1-6) connected or disconnected. Observed on a LIGHTSPEED receiver when
/// the G305 is switched off (`10 01 41 0D 62 74 40`, bit 0x40 of byte 4 set:
/// no link) and on (`10 01 41 0D A2 74 40`).
pub fn parse_connection(r: &[u8]) -> Option<(u8, Link)> {
    match r {
        [REPORT_SHORT, index @ 1..=6, 0x41, _, flags, ..] => Some((
            *index,
            if flags & 0x40 == 0 {
                Link::Up
            } else {
                Link::Down
            },
        )),
        _ => None,
    }
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

/// 0x8070 getInfo: number of light zones.
pub fn parse_led_zone_count(p: &[u8]) -> Option<u8> {
    p.first().copied()
}

/// 0x8070 getZoneInfo: number of effects of the zone.
pub fn parse_led_effect_count(p: &[u8]) -> Option<u8> {
    p.get(3).copied()
}

/// 0x8070 getZoneEffectInfo: effect id (u16 big-endian at bytes 2..4).
pub fn parse_led_effect_id(p: &[u8]) -> Option<u16> {
    Some(u16::from_be_bytes([*p.get(2)?, *p.get(3)?]))
}

#[cfg(test)]
mod tests {
    #[test]
    fn device_type_and_name() {
        assert_eq!(super::is_pointer(&[3]), Some(true));
        assert_eq!(super::is_pointer(&[5, 0]), Some(true));
        assert_eq!(super::is_pointer(&[0]), Some(false)); // keyboard
        assert_eq!(super::is_pointer(&[8]), Some(false)); // headset
        assert_eq!(super::is_pointer(&[]), None);
        assert_eq!(super::clean_name(b"G203 LIGHTSYNC\0\0"), "G203 LIGHTSYNC");
        assert_eq!(super::clean_name("a\u{202e}b\x07c\n".as_bytes()), "abc");
        assert_eq!(
            super::clean_name(&[b'x'; 200]).chars().count(),
            super::NAME_MAX
        );
        assert_eq!(super::clean_name(b"\0\x01 "), "");
    }

    use super::*;

    #[test]
    fn encodes_get_feature() {
        let r = Request::GetFeature { feature: 0x1000 }.encode(1, 0);
        assert_eq!(&r[..7], &[0x11, 0x01, 0x00, 0x0b, 0x10, 0x00, 0x00]);
        assert!(r[7..].iter().all(|&b| b == 0));
    }

    #[test]
    fn connection_notifications_as_captured() {
        // Captured on the G305's receiver when switching the mouse off and on.
        assert_eq!(
            parse_connection(&[0x10, 0x01, 0x41, 0x0d, 0x62, 0x74, 0x40]),
            Some((1, Link::Down))
        );
        assert_eq!(
            parse_connection(&[0x10, 0x01, 0x41, 0x0d, 0xa2, 0x74, 0x40]),
            Some((1, Link::Up))
        );
        // Replies to pings of absent devices, other reports, truncated ones.
        for r in [
            &[0x10, 0x02, 0x8f, 0x00, 0x1b, 0x08, 0x00][..],
            &[0x11, 0x01, 0x04, 0x00, 0x01, 0x01, 0x01],
            &[0x10, 0x00, 0x41, 0x0d, 0xa2, 0x74, 0x40],
            &[0x10, 0x07, 0x41, 0x0d, 0xa2, 0x74, 0x40],
            &[0x10, 0x01, 0x41, 0x0d],
            &[],
        ] {
            assert_eq!(parse_connection(r), None, "{r:02x?}");
        }
    }

    #[test]
    fn encodes_light_reads() {
        assert_eq!(
            &Request::LedInfo.encode(1, 6)[..5],
            &[0x11, 0x01, 0x06, 0x0b, 0x00]
        );
        assert_eq!(
            &Request::LedZoneInfo { zone: 0 }.encode(1, 6)[..5],
            &[0x11, 0x01, 0x06, 0x1b, 0x00]
        );
        assert_eq!(
            &Request::LedEffectInfo { zone: 0, index: 3 }.encode(1, 6)[..6],
            &[0x11, 0x01, 0x06, 0x2b, 0x00, 0x03]
        );
        assert_eq!(Request::LedInfo.feature(), feature::COLOR_LED_EFFECTS);
        // Payloads read from the G305.
        assert_eq!(parse_led_zone_count(&[0x01, 0, 0, 0, 0x14]), Some(1));
        assert_eq!(parse_led_effect_count(&[0, 0, 0x01, 0x04]), Some(4));
        assert_eq!(
            parse_led_effect_id(&[0x00, 0x03, 0x00, 0x0a, 0xc1, 0x05]),
            Some(0x0a)
        );
        assert_eq!(parse_led_effect_id(&[0x00]), None);
    }

    #[test]
    fn encodes_live_light_as_sent_in_hardware_test_h2() {
        use super::super::onboard::Light;
        use super::super::razer::Rgb;
        let red = Rgb {
            r: 0xff,
            g: 0,
            b: 0,
        };
        // Exact parameters verified on the G305 (2026-10-07).
        let cases = [
            (1, Light::Fixed(red), "0001ff00000000000000000000000000"),
            (
                3,
                Light::Breathing {
                    color: red,
                    period_ms: 2000,
                },
                "0003ff000007d0000000000000000000",
            ),
            (
                2,
                Light::Cycle { period_ms: 4000 },
                "000200000000000fa000000000000000",
            ),
        ];
        for (effect_index, light, hex) in cases {
            let r = Request::SetLight {
                effect_index,
                light,
            }
            .encode(1, 6);
            assert_eq!(&r[..4], &[0x11, 0x01, 0x06, 0x3b]);
            let params: String = r[4..].iter().map(|b| format!("{b:02x}")).collect();
            assert_eq!(params, hex);
        }
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
    fn encodes_memory_read() {
        let r = Request::MemoryRead {
            sector: 0x0101,
            offset: 0x00ef,
        }
        .encode(1, 7);
        assert_eq!(&r[..8], &[0x11, 0x01, 0x07, 0x5b, 0x01, 0x01, 0x00, 0xef]);
        assert_eq!(Request::GetCurrentProfile.encode(1, 7)[3], 0x4b);
        // No request of this application targets 0x8100 write functions (6, 7, 8).
        assert_eq!(Request::GetOnboardMode.encode(1, 7)[3], 0x2b);
        assert_eq!(
            &Request::ActivateOnboard.encode(0xff, 7)[..5],
            &[0x11, 0xff, 7, 0x1b, 1]
        );
        for req in [
            Request::OnboardInfo,
            Request::GetOnboardMode,
            Request::GetCurrentProfile,
            Request::MemoryRead {
                sector: 1,
                offset: 0,
            },
        ] {
            assert!(req.encode(1, 7)[3] >> 4 <= 5);
        }
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
