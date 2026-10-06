//! Sony DualSense (USB) codec (pure, no I/O).
//!
//! Report layout from public documentation of the controller (e.g. the Linux
//! kernel's documented report structure, reimplemented here) and verified on
//! a DualSense over USB. See `docs/protocolos/dualsense.md`.

use super::{Battery, ChargeState};

pub const INPUT_REPORT_USB: u8 = 0x01;
pub const OUTPUT_REPORT_USB: u8 = 0x02;
pub const OUTPUT_LEN_USB: usize = 48;

/// Offset of the battery/status byte in the USB input report (report id at 0).
const STATUS_OFFSET: usize = 53;

const FLAG1_LIGHTBAR_CONTROL: u8 = 0x04;
const FLAG2_LIGHTBAR_SETUP_CONTROL: u8 = 0x02;
const FLAG2_OFFSET: usize = 39;
const LIGHTBAR_SETUP_OFFSET: usize = 42;
/// Ends the power-on lightbar animation so the requested colour shows.
const LIGHTBAR_SETUP_LIGHT_OUT: u8 = 0x02;
const LIGHTBAR_RGB_OFFSET: usize = 45;

/// Decodes battery from a USB input report 0x01.
pub fn parse_battery(report: &[u8]) -> Option<Battery> {
    if *report.first()? != INPUT_REPORT_USB {
        return None;
    }
    let status = *report.get(STATUS_OFFSET)?;
    let level = status & 0x0f;
    let estimate = (level as u16 * 10 + 5).min(100) as u8;
    match status >> 4 {
        0x0 => Some(Battery {
            percent: estimate,
            charging: ChargeState::Discharging,
        }),
        0x1 => Some(Battery {
            percent: estimate,
            charging: ChargeState::Charging,
        }),
        0x2 => Some(Battery {
            percent: 100,
            charging: ChargeState::Full,
        }),
        // voltage / temperature / charging errors: level is meaningless
        _ => Some(Battery {
            percent: 0,
            charging: ChargeState::Error,
        }),
    }
}

/// USB output report that only changes the lightbar colour.
pub fn lightbar_report(r: u8, g: u8, b: u8) -> [u8; OUTPUT_LEN_USB] {
    let mut out = [0u8; OUTPUT_LEN_USB];
    out[0] = OUTPUT_REPORT_USB;
    out[2] = FLAG1_LIGHTBAR_CONTROL;
    out[FLAG2_OFFSET] = FLAG2_LIGHTBAR_SETUP_CONTROL;
    out[LIGHTBAR_SETUP_OFFSET] = LIGHTBAR_SETUP_LIGHT_OUT;
    out[LIGHTBAR_RGB_OFFSET..LIGHTBAR_RGB_OFFSET + 3].copy_from_slice(&[r, g, b]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn input(status: u8) -> [u8; 64] {
        let mut r = [0u8; 64];
        r[0] = INPUT_REPORT_USB;
        r[STATUS_OFFSET] = status;
        r
    }

    #[test]
    fn battery_levels() {
        // Value observed on real hardware: level 8, charging -> 85 %.
        assert_eq!(
            parse_battery(&input(0x18)),
            Some(Battery {
                percent: 85,
                charging: ChargeState::Charging
            })
        );
        assert_eq!(parse_battery(&input(0x00)).unwrap().percent, 5);
        assert_eq!(parse_battery(&input(0x0a)).unwrap().percent, 100);
        assert_eq!(parse_battery(&input(0x0f)).unwrap().percent, 100);
        assert_eq!(
            parse_battery(&input(0x20)).unwrap().charging,
            ChargeState::Full
        );
        assert_eq!(
            parse_battery(&input(0xa3)).unwrap().charging,
            ChargeState::Error
        );
    }

    #[test]
    fn rejects_other_reports() {
        let mut r = input(0x18);
        r[0] = 0x31; // Bluetooth report id
        assert_eq!(parse_battery(&r), None);
        assert_eq!(parse_battery(&r[..10]), None);
        assert_eq!(parse_battery(&[]), None);
    }

    #[test]
    fn lightbar_layout() {
        let r = lightbar_report(1, 2, 3);
        assert_eq!(r.len(), 48);
        assert_eq!(r[0], 0x02);
        assert_eq!(r[1], 0);
        assert_eq!(r[2], 0x04);
        assert_eq!(&r[45..48], &[1, 2, 3]);
        assert_eq!((r[39], r[42]), (0x02, 0x02));
        // Nothing else (motors, triggers, audio, player LEDs) is touched.
        assert!(
            r[3..45]
                .iter()
                .enumerate()
                .all(|(i, &b)| b == 0 || i + 3 == 39 || i + 3 == 42)
        );
    }
}
