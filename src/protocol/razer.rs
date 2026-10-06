//! Razer 90-byte feature report codec (pure, no I/O).
//!
//! Layout reimplemented from public protocol descriptions and verified against
//! a BlackWidow V3 Tenkeyless. See `docs/protocolos/razer.md`.

pub const REPORT_LEN: usize = 90;

/// Transaction id accepted by recent devices (verified on BlackWidow V3 TKL).
pub const TRANSACTION_ID: u8 = 0x1f;

const STATUS_SUCCESS: u8 = 0x02;

/// LED zone of a keyboard's per-key backlight.
const LED_BACKLIGHT: u8 = 0x05;
/// Do not persist the change in the device memory.
const NOSTORE: u8 = 0x00;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

/// Every command this application can send to a Razer device.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Request {
    /// Class 0x00, id 0x81.
    FirmwareVersion,
    /// Class 0x0F, id 0x02: static colour, never stored on the device.
    StaticColor(Rgb),
}

impl Request {
    fn class_id_args(&self) -> (u8, u8, Vec<u8>) {
        match *self {
            Request::FirmwareVersion => (0x00, 0x81, vec![0, 0]),
            Request::StaticColor(c) => (
                0x0f,
                0x02,
                // varstore, led, effect=static, 2 reserved, colour count, rgb
                vec![
                    NOSTORE,
                    LED_BACKLIGHT,
                    0x01,
                    0x00,
                    0x00,
                    0x01,
                    c.r,
                    c.g,
                    c.b,
                ],
            ),
        }
    }

    pub fn encode(&self) -> [u8; REPORT_LEN] {
        let (class, id, args) = self.class_id_args();
        let mut r = [0u8; REPORT_LEN];
        r[1] = TRANSACTION_ID;
        r[5] = args.len() as u8;
        r[6] = class;
        r[7] = id;
        r[8..8 + args.len()].copy_from_slice(&args);
        r[88] = crc(&r);
        r
    }
}

/// XOR of bytes 2..88.
pub fn crc(r: &[u8; REPORT_LEN]) -> u8 {
    r[2..88].iter().fold(0, |a, b| a ^ b)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReplyError {
    Malformed,
    /// Device status byte other than success (busy, failure, unsupported...).
    Status(u8),
    /// Reply is for a different command (e.g. stale or from other software).
    Mismatch,
}

/// Validates a reply to `sent` and returns its argument bytes.
pub fn parse_reply<'a>(sent: &[u8; REPORT_LEN], reply: &'a [u8]) -> Result<&'a [u8], ReplyError> {
    let r: &[u8; REPORT_LEN] = reply
        .get(..REPORT_LEN)
        .and_then(|s| s.try_into().ok())
        .ok_or(ReplyError::Malformed)?;
    if r[6] != sent[6] || r[7] != sent[7] {
        return Err(ReplyError::Mismatch);
    }
    if r[0] != STATUS_SUCCESS {
        return Err(ReplyError::Status(r[0]));
    }
    if r[88] != crc(r) {
        return Err(ReplyError::Malformed);
    }
    Ok(&r[8..88])
}

pub fn parse_firmware(args: &[u8]) -> Option<(u8, u8)> {
    Some((*args.first()?, *args.get(1)?))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn firmware_request_layout() {
        let r = Request::FirmwareVersion.encode();
        assert_eq!(&r[..8], &[0x00, 0x1f, 0, 0, 0, 0x02, 0x00, 0x81]);
        assert_eq!(r[88], 0x02 ^ 0x81);
    }

    #[test]
    fn static_color_never_stores() {
        let r = Request::StaticColor(Rgb {
            r: 0xff,
            g: 0x10,
            b: 0x00,
        })
        .encode();
        assert_eq!(
            &r[5..17],
            &[
                9, 0x0f, 0x02, NOSTORE, 0x05, 0x01, 0, 0, 1, 0xff, 0x10, 0x00
            ]
        );
        assert_eq!(r[88], crc(&r));
    }

    fn reply_to(sent: &[u8; REPORT_LEN], status: u8, args: &[u8]) -> [u8; REPORT_LEN] {
        let mut r = *sent;
        r[0] = status;
        r[8..8 + args.len()].copy_from_slice(args);
        r[88] = crc(&r);
        r
    }

    #[test]
    fn parses_firmware_reply() {
        let sent = Request::FirmwareVersion.encode();
        let r = reply_to(&sent, 0x02, &[1, 2]);
        let args = parse_reply(&sent, &r).unwrap();
        assert_eq!(parse_firmware(args), Some((1, 2)));
    }

    #[test]
    fn rejects_bad_replies() {
        let sent = Request::FirmwareVersion.encode();
        assert_eq!(parse_reply(&sent, &[0u8; 10]), Err(ReplyError::Malformed));

        let busy = reply_to(&sent, 0x01, &[]);
        assert_eq!(parse_reply(&sent, &busy), Err(ReplyError::Status(0x01)));

        let mut bad_crc = reply_to(&sent, 0x02, &[1, 2]);
        bad_crc[88] ^= 0xff;
        assert_eq!(parse_reply(&sent, &bad_crc), Err(ReplyError::Malformed));

        let other = reply_to(
            &Request::StaticColor(Rgb { r: 0, g: 0, b: 0 }).encode(),
            0x02,
            &[],
        );
        assert_eq!(parse_reply(&sent, &other), Err(ReplyError::Mismatch));
    }
}
