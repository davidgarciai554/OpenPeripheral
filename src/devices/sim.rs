//! Virtual HID++ 2.0 mice for tests (plan `banco-ratones-virtuales`): any
//! number of buttons, profile format, DPI range, cable or receiver, light
//! and onboard mode, answering like the G305 and G203 measured on hardware.
//!
//! Limit: the memory layout is the one observed on those two mice (formats
//! 3 and 4). It proves the code handles any button count with that layout;
//! it says nothing about the layout of a real model never read.

use super::fake::FakeTransport;
use super::{Driver, Kind, Supported};
use crate::protocol::onboard;
use std::cell::RefCell;
use std::rc::Rc;

/// A virtual mouse.
#[derive(Debug, Clone)]
pub struct Spec {
    pub name: &'static str,
    pub pid: u16,
    pub buttons: u8,
    /// 0x8100 profile format (getInfo byte 1).
    pub format: u8,
    /// 0x8100 onboard mode: 1 onboard profiles, 2 host.
    pub mode: u8,
    /// 0x2201 range (min, max, step); `None` = no 0x2201 (office mice).
    pub dpi: Option<(u16, u16, u16)>,
    /// Behind a receiver (index 1) instead of a cable (index 0xFF).
    pub wireless: bool,
    /// 0x8070 effect ids; empty = no 0x8070.
    pub effects: &'static [u16],
}

impl Spec {
    pub const fn new(name: &'static str, pid: u16, buttons: u8) -> Self {
        Spec {
            name,
            pid,
            buttons,
            format: 3,
            mode: 1,
            dpi: Some((100, 16000, 50)),
            wireless: false,
            effects: &[0x00, 0x01, 0x03, 0x0a],
        }
    }

    pub fn index(&self) -> u8 {
        if self.wireless { 1 } else { 0xff }
    }

    /// A whitelist entry for this mouse with its onboard profile enabled,
    /// as if it had been verified on hardware.
    pub fn verified(&self) -> &'static Supported {
        Box::leak(Box::new(Supported {
            vid: 0x046d,
            pid: self.pid,
            usage_page: 0xff00,
            usage: 0x0002,
            interface: None,
            driver: Driver::Hidpp,
            vendor: "Logitech",
            model: self.name,
            kind: Kind::Mouse,
            connection: "USB",
            wired: !self.wireless,
            onboard: true,
        }))
    }
}

/// Factory assignment of button `i`: the five mouse buttons, the DPI
/// button (`90 05`), then mouse buttons 6, 7… (masks 0x20, 0x40…).
pub fn factory_button(i: usize) -> [u8; 4] {
    match i {
        0..=4 => [0x80, 0x01, 0x00, 1 << i],
        5 => [0x90, 0x05, 0xff, 0xff],
        _ => {
            let [hi, lo] = (1u16 << (i - 1).min(15)).to_be_bytes();
            [0x80, 0x01, hi, lo]
        }
    }
}

/// Buttons start at 0x20; the G-shift copy at 0x60 (observed on the G203),
/// so a 255-byte sector of these formats holds at most 16 buttons.
pub const MAX_LAYOUT_BUTTONS: u8 = 16;

fn with_crc(mut v: Vec<u8>) -> Vec<u8> {
    let n = v.len() - 2;
    let crc = onboard::crc_ccitt(&v[..n]).to_be_bytes();
    v[n..].copy_from_slice(&crc);
    v
}

/// The profile sector of `s` (user copy: `crc`; factory copy: no CRC).
pub fn profile(s: &Spec, crc: bool) -> Vec<u8> {
    let mut v = vec![0xffu8; 255];
    v[..3].copy_from_slice(&[0x01, 0x02, 0x01]);
    for (i, dpi) in [800u16, 1600, 3200, 6400].iter().enumerate() {
        let (min, max, _) = s.dpi.unwrap_or((800, 6400, 50));
        let dpi = (*dpi).clamp(min, max);
        v[3 + 2 * i..5 + 2 * i].copy_from_slice(&dpi.to_le_bytes());
    }
    v[0x0b..0x0d].copy_from_slice(&[0, 0]);
    let n = s.buttons.min(MAX_LAYOUT_BUTTONS) as usize;
    for i in 0..n {
        let b = factory_button(i);
        v[0x20 + 4 * i..0x24 + 4 * i].copy_from_slice(&b);
        v[0x60 + 4 * i..0x64 + 4 * i].copy_from_slice(&b);
    }
    for (i, c) in "PROFILE_NAME_DEFAULT".encode_utf16().enumerate() {
        v[0xa0 + 2 * i..0xa2 + 2 * i].copy_from_slice(&c.to_le_bytes());
    }
    // Light blocks: 0xD0 as on the G203, the active one (0xDB) fixed white.
    v[0xd0..0xdb].copy_from_slice(&[0x04, 0, 0, 0, 0, 0, 0, 0x40, 0x01, 0x64, 0x1f]);
    v[0xdb..0xe6].copy_from_slice(&[0x01, 0xff, 0xff, 0xff, 0, 0, 0, 0, 0, 0, 0]);
    if crc { with_crc(v) } else { v }
}

pub fn directory() -> Vec<u8> {
    let mut v = vec![0xffu8; 255];
    v[..4].copy_from_slice(&[0x00, 0x01, 0x01, 0x00]);
    with_crc(v)
}

/// The mouse's state that tests can inspect.
#[derive(Debug)]
pub struct Memory {
    pub user: Vec<u8>,
    pub dpi: u16,
    pub mode: u8,
    /// Number of 0x8100 fn6/7/8 requests received.
    pub writes: usize,
}

/// A transport answering as the mouse `s`; its memory is shared.
pub fn mouse(s: &Spec) -> (FakeTransport, Rc<RefCell<Memory>>) {
    let mem = Rc::new(RefCell::new(Memory {
        user: profile(s, true),
        dpi: s.dpi.map_or(1000, |(min, max, _)| 800u16.clamp(min, max)),
        mode: s.mode,
        writes: 0,
    }));
    let s = s.clone();
    let shared = mem.clone();
    let rom = profile(&s, false);
    let pending: RefCell<Option<(u16, usize, Vec<u8>)>> = RefCell::new(None);
    let t = FakeTransport::new(move |req| {
        let (idx, feat, func) = (req[1], req[2], req[3]);
        if idx != s.index() {
            // Nothing at that index: HID++ 1.0 "unknown device", as a
            // receiver answers for an empty slot (no 400 ms wait).
            return vec![vec![0x10, idx, 0x8f, feat, func, 0x09, 0x00]];
        }
        let error = || {
            vec![vec![
                0x11, idx, 0xff, feat, func, 0x02, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0,
            ]]
        };
        let mut m = shared.borrow_mut();
        let p: Vec<u8> = match (feat, func >> 4) {
            (0, 1) => vec![4, 2, req[6]],
            (0, 0) => {
                let index = match u16::from_be_bytes([req[4], req[5]]) {
                    0x0005 => 2,
                    0x1000 if s.wireless => 4,
                    0x2201 if s.dpi.is_some() => 5,
                    0x8060 => 6,
                    0x8100 => 7,
                    0x8070 if !s.effects.is_empty() => 8,
                    _ => 0,
                };
                vec![index]
            }
            (2, 0) => vec![s.name.len() as u8],
            (2, 1) => s
                .name
                .as_bytes()
                .iter()
                .skip(req[4] as usize)
                .take(16)
                .copied()
                .collect(),
            (2, 2) => vec![3], // deviceType: mouse
            (4, 0) if s.wireless => vec![80, 50, 0],
            (5, 1) => {
                let Some((min, max, step)) = s.dpi else {
                    return error();
                };
                let mut p = vec![0];
                for w in [min, 0xe000 | step, max, 0] {
                    p.extend_from_slice(&w.to_be_bytes());
                }
                p
            }
            (5, 2) => {
                let [hi, lo] = m.dpi.to_be_bytes();
                vec![0, hi, lo, hi, lo]
            }
            (5, 3) => {
                m.dpi = u16::from_be_bytes([req[5], req[6]]);
                vec![0, req[5], req[6]]
            }
            (6, 0) => vec![0b1000_1011],
            (6, 1) => vec![1],
            (8, 0) if !s.effects.is_empty() => vec![0x01],
            (8, 1) if !s.effects.is_empty() => vec![0, 0, 0x01, s.effects.len() as u8],
            (8, 2) if !s.effects.is_empty() => {
                let [hi, lo] = s.effects[req[5] as usize % s.effects.len()].to_be_bytes();
                vec![0, req[5], hi, lo]
            }
            (8, 3) if !s.effects.is_empty() => vec![],
            (7, 0) => vec![
                0x01, s.format, 0x01, 0x01, 0x01, s.buttons, 0x10, 0x00, 0xff,
            ],
            (7, 1) => {
                m.mode = req[4];
                vec![]
            }
            (7, 2) => vec![m.mode],
            (7, 4) if m.mode == 1 => vec![0x00, 0x01],
            (7, 4) => vec![0x00, 0x00],
            (7, 5) => {
                let sector = match u16::from_be_bytes([req[4], req[5]]) {
                    0x0000 => directory(),
                    0x0001 => m.user.clone(),
                    0x0101 => rom.clone(),
                    _ => return error(),
                };
                let off = u16::from_be_bytes([req[6], req[7]]) as usize;
                match sector.get(off..off + 16) {
                    Some(c) => c.to_vec(),
                    None => return error(),
                }
            }
            (7, 6) => {
                m.writes += 1;
                let len = u16::from_be_bytes([req[8], req[9]]) as usize;
                *pending.borrow_mut() = Some((u16::from_be_bytes([req[4], req[5]]), len, vec![]));
                vec![]
            }
            (7, 7) => {
                m.writes += 1;
                if let Some((_, _, data)) = pending.borrow_mut().as_mut() {
                    data.extend_from_slice(&req[4..20]);
                }
                vec![]
            }
            (7, 8) => {
                m.writes += 1;
                if let Some((1, len, mut data)) = pending.borrow_mut().take() {
                    data.truncate(len);
                    m.user = data;
                }
                vec![]
            }
            _ => return error(),
        };
        let mut r = vec![0u8; 20];
        r[..4].copy_from_slice(&req[..4]);
        let n = p.len().min(16);
        r[4..4 + n].copy_from_slice(&p[..n]);
        vec![r]
    });
    (t, mem)
}

/// A virtual Razer device (90-byte feature reports): answers only with its
/// own transaction id; a mouse reports and takes a DPI, anything else
/// answers "not supported" to the DPI commands.
#[derive(Debug, Default)]
pub struct RazerMemory {
    pub dpi: u16,
    /// Storage byte of every DPI write (0 = not stored).
    pub stores: Vec<u8>,
}

pub fn razer(transaction_id: u8, mouse: bool) -> (FakeTransport, Rc<RefCell<RazerMemory>>) {
    razer_with(transaction_id, mouse, true)
}

/// `takes_dpi = false`: answers success to a DPI write but keeps its DPI
/// (a mouse that would ignore the "not stored" mode).
pub fn razer_with(
    transaction_id: u8,
    mouse: bool,
    takes_dpi: bool,
) -> (FakeTransport, Rc<RefCell<RazerMemory>>) {
    use crate::protocol::razer::{REPORT_LEN, crc};
    let mem = Rc::new(RefCell::new(RazerMemory {
        dpi: 800,
        stores: Vec::new(),
    }));
    let shared = mem.clone();
    let t = FakeTransport::new(move |req| {
        let mut r = [0u8; REPORT_LEN];
        r.copy_from_slice(&req[1..=REPORT_LEN]);
        let mut m = shared.borrow_mut();
        let (status, args): (u8, Vec<u8>) = match (r[1] == transaction_id, r[6], r[7]) {
            (false, _, _) => (0x03, vec![]),
            (_, 0x00, 0x81) => (0x02, vec![1, 2]),
            (_, 0x04, _) if !mouse => (0x05, vec![]),
            (_, 0x04, 0x85) => {
                let [h, l] = m.dpi.to_be_bytes();
                (0x02, vec![r[8], h, l, h, l])
            }
            (_, 0x04, 0x05) => {
                m.stores.push(r[8]);
                if takes_dpi {
                    m.dpi = u16::from_be_bytes([r[9], r[10]]);
                }
                (0x02, r[8..15].to_vec())
            }
            _ => (0x05, vec![]),
        };
        r[0] = status;
        r[8..8 + args.len()].copy_from_slice(&args);
        r[88] = crc(&r);
        let mut v = vec![0];
        v.extend_from_slice(&r);
        vec![v]
    });
    (t, mem)
}

/// A virtual ASUS mouse (64-byte reports, libratbag layout): presets of
/// DPI codes, the active one, and every request it got.
#[derive(Debug, Default)]
pub struct AsusMemory {
    pub codes: [u16; 4],
    pub active: usize,
    pub commands: Vec<u16>,
}

pub fn asus(presets: usize) -> (FakeTransport, Rc<RefCell<AsusMemory>>) {
    let mem = Rc::new(RefCell::new(AsusMemory {
        codes: [15, 31, 63, 0],
        active: 1,
        commands: Vec::new(),
    }));
    let shared = mem.clone();
    let t = FakeTransport::new(move |req| {
        let mut m = shared.borrow_mut();
        let cmd = u16::from_le_bytes([req[1], req[2]]);
        m.commands.push(cmd);
        let mut r = vec![0u8; 64];
        match cmd {
            0x0012 => r[11] = m.active as u8 + 1,
            0x0412 => {
                for i in 0..presets {
                    r[4 + 2 * i..6 + 2 * i].copy_from_slice(&m.codes[i].to_le_bytes());
                }
            }
            0x3151 if (req[3] as usize) < presets => {
                m.codes[req[3] as usize] = u16::from_le_bytes([req[5], req[6]]);
            }
            _ => r[..2].copy_from_slice(&0xaaffu16.to_le_bytes()),
        }
        vec![r]
    });
    (t, mem)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{AUTO_LOGITECH, Change, Device, Error, Support, logitech};
    use crate::protocol::DpiRange;
    use crate::protocol::onboard::{Action, EditError, ProfileEdits};
    use std::ffi::CString;

    /// Synthetic mice: button counts from 2 to 16, both formats, cable and
    /// receiver, with and without light. Not real models.
    fn catalog() -> Vec<Spec> {
        let mut out = Vec::new();
        for (k, n) in [2u8, 3, 5, 6, 7, 8, 9, 11, 13, 16].into_iter().enumerate() {
            let mut s = Spec::new("Virtual Mouse", 0xd000 + n as u16, n);
            s.format = if k % 2 == 0 { 3 } else { 4 };
            s.wireless = k % 3 == 0;
            if k % 4 == 1 {
                s.effects = &[];
            }
            s.dpi = Some([(200, 12000, 50), (100, 25600, 50), (50, 8000, 50)][k % 3]);
            out.push(s);
        }
        out
    }

    fn open(s: &Spec, verified: bool) -> (FakeTransport, Rc<RefCell<Memory>>, Box<dyn Device>) {
        let (t, mem) = mouse(s);
        let entry = if verified {
            s.verified()
        } else {
            &AUTO_LOGITECH
        };
        let mut found = logitech::probe(entry, s.pid, CString::new("x").unwrap(), &t);
        assert_eq!(found.len(), 1, "{s:?}");
        let dev: Box<dyn Device> = Box::new(found.remove(0));
        (t, mem, dev)
    }

    /// The bytes that may change when button `i` is edited.
    fn editable(i: usize, at: usize) -> bool {
        (0x20 + 4 * i..0x24 + 4 * i).contains(&at) || at >= 253
    }

    #[test]
    fn every_button_of_every_virtual_mouse_can_be_assigned_and_restored() {
        for s in catalog() {
            let (t, mem, mut d) = open(&s, true);
            assert_eq!(d.info().name, s.name);
            assert_eq!(d.info().support, Support::Verified);
            let state = d.read_state(&t).unwrap();
            let (min, max, step) = s.dpi.unwrap();
            assert_eq!(
                state.dpi.unwrap().allowed,
                DpiRange::Range { min, max, step }
            );
            assert_eq!(state.battery.is_some(), s.wireless, "{s:?}");

            let first = d.read_profile(&t).unwrap();
            let n = s.buttons as usize;
            assert_eq!(first.info.button_count, s.buttons);
            assert_eq!(
                onboard::parse_buttons(&first.user, s.buttons)
                    .unwrap()
                    .len(),
                n
            );
            assert_eq!(
                first.effects.is_empty(),
                s.format != 3 || s.effects.is_empty()
            );

            for i in 1..n {
                for action in [
                    Action::Mouse(if i == 3 { 0x10 } else { 0x08 }),
                    Action::Keyboard {
                        modifiers: 0x01,
                        usage: 0x06,
                    },
                ] {
                    let before = d.read_profile(&t).unwrap();
                    let edits = ProfileEdits {
                        buttons: vec![(i, action)],
                        ..Default::default()
                    };
                    let image = onboard::apply_edits(
                        &before.user,
                        before.sector,
                        &before.directory,
                        &before.limits(),
                        &edits,
                    )
                    .unwrap_or_else(|e| panic!("{} buttons, button {i}: {e:?}", n));
                    d.write_profile(&t, &image).unwrap();
                    let after = d.read_profile(&t).unwrap();
                    let read = onboard::parse_buttons(&after.user, s.buttons).unwrap();
                    assert_eq!(read[i], action, "{n} buttons, button {i}");
                    for (at, (a, b)) in before.user.iter().zip(&after.user).enumerate() {
                        assert!(a == b || editable(i, at), "{n} buttons: byte {at:#x}");
                    }
                }
            }
            // The left click is never changed.
            let dump = d.read_profile(&t).unwrap();
            let edits = ProfileEdits {
                buttons: vec![(0, Action::Mouse(0x02))],
                ..Default::default()
            };
            assert_eq!(
                onboard::apply_edits(&dump.user, 1, &dump.directory, &dump.limits(), &edits),
                Err(EditError::LeftButton)
            );
            // Back to factory: every button as in the factory copy.
            let image = onboard::factory_image(
                &dump.user,
                &dump.rom,
                dump.sector,
                &dump.directory,
                &dump.limits(),
            )
            .unwrap();
            d.write_profile(&t, &image).unwrap();
            let back = d.read_profile(&t).unwrap();
            assert_eq!(
                onboard::parse_buttons(&back.user, s.buttons),
                onboard::parse_buttons(&back.rom, s.buttons),
                "{n} buttons"
            );
            assert!(mem.borrow().writes > 0);
        }
    }

    #[test]
    fn unverified_virtual_mice_get_volatile_settings_only() {
        for s in catalog() {
            let (t, mem, mut d) = open(&s, false);
            assert_eq!(d.info().support, Support::Auto);
            assert_eq!(d.read_profile(&t), Err(Error::Unsupported));
            let (min, max, step) = s.dpi.unwrap();
            d.apply(&t, Change::Dpi(min + step)).unwrap();
            assert_eq!(mem.borrow().dpi, min + step);
            assert_eq!(
                d.apply(&t, Change::Dpi(max + step)),
                Err(Error::Unsupported)
            );
            assert_eq!(mem.borrow().writes, 0, "{s:?}");
        }
    }

    #[test]
    fn a_bluetooth_logitech_mouse_gets_volatile_settings() {
        let s = Spec::new("MX Master 3S", 0xb034, 7);
        let (t, mem) = mouse(&s);
        let entry = crate::devices::lookup(0x046d, 0xb034, 0xff43, 0x0202, 0).unwrap();
        let mut found = logitech::probe(entry, s.pid, CString::new("x").unwrap(), &t);
        let mut d = found.remove(0);
        assert_eq!(d.info().connection, "Bluetooth");
        assert!(!d.info().wired);
        assert_eq!(d.info().support, Support::Auto);
        assert_eq!(d.info().key.0, "046d:b034/MX Master 3S");
        d.apply(&t, Change::Dpi(1600)).unwrap();
        assert_eq!(mem.borrow().dpi, 1600);
        assert_eq!(d.read_profile(&t), Err(Error::Unsupported));
        assert_eq!(d.activate_onboard(&t), Err(Error::Unsupported));
        assert_eq!(mem.borrow().writes, 0);
    }

    #[test]
    fn razer_mice_of_every_generation_get_volatile_dpi() {
        use crate::devices::{AUTO_RAZER, Device, razer};
        for tid in crate::protocol::razer::MOUSE_TRANSACTION_IDS {
            let (t, mem) = super::razer(tid, true);
            let mut d = razer::probe_mouse(
                &AUTO_RAZER,
                0x00b9,
                "Razer Basilisk V3 X HyperSpeed",
                CString::new("x").unwrap(),
                &t,
            )
            .unwrap_or_else(|| panic!("tid {tid:#x}"));
            assert_eq!(d.info().key.0, "1532:00b9/Razer Basilisk V3 X HyperSpeed");
            assert_eq!(d.info().support, Support::Auto);
            assert_eq!(d.read_state(&t).unwrap().dpi.unwrap().current, Some(800));
            d.apply(&t, Change::Dpi(1600)).unwrap();
            assert_eq!(mem.borrow().dpi, 1600);
            assert_eq!(d.apply(&t, Change::Dpi(16050)), Err(Error::Unsupported));
            assert_eq!(d.apply(&t, Change::Dpi(1625)), Err(Error::Unsupported));
            assert_eq!(mem.borrow().stores, [0], "never stored in the mouse");
            assert_eq!(d.read_profile(&t), Err(Error::Unsupported));
        }
        // A Razer keyboard or headset (no DPI) is not taken for a mouse.
        let (t, mem) = super::razer(0x1f, false);
        assert!(
            razer::probe_mouse(&AUTO_RAZER, 0x0a24, "", CString::new("x").unwrap(), &t).is_none()
        );
        assert!(mem.borrow().stores.is_empty());
        // Without a name from Windows.
        let (t, _) = super::razer(0x3f, true);
        let d = razer::probe_mouse(&AUTO_RAZER, 0x0084, "", CString::new("x").unwrap(), &t);
        assert_eq!(d.unwrap().info().name, "Ratón Razer");
    }

    #[test]
    fn asus_mice_get_volatile_dpi_on_the_active_preset() {
        use crate::devices::{AUTO_ASUS, asus};
        let (t, mem) = super::asus(4);
        let mut d = asus::probe(&AUTO_ASUS, 0x1a9b, "", CString::new("x").unwrap(), &t).unwrap();
        assert_eq!(d.info().name, "ASUSTeK TUF GAMING M3 GEN II");
        assert_eq!(d.info().support, Support::Auto);
        // Active preset 1: code 31 -> 1600 DPI.
        assert_eq!(d.read_state(&t).unwrap().dpi.unwrap().current, Some(1600));
        d.apply(&t, Change::Dpi(3200)).unwrap();
        assert_eq!(mem.borrow().codes[1], 63);
        assert_eq!(mem.borrow().codes[0], 15, "other presets untouched");
        assert_eq!(d.apply(&t, Change::Dpi(8100)), Err(Error::Unsupported));
        assert!(!mem.borrow().commands.contains(&0x0350), "never saved");
        // A collection that does not answer is not a mouse.
        let silent = FakeTransport::new(|_| vec![]);
        assert!(asus::probe(&AUTO_ASUS, 0x1a9b, "", CString::new("x").unwrap(), &silent).is_none());
    }

    #[test]
    fn razer_checks_the_dpi_by_reading_it_back_and_reaches_its_own_top() {
        use crate::devices::{AUTO_RAZER, Device, razer};
        let (t, mem) = super::razer_with(0x1f, true, false);
        let mut d =
            razer::probe_mouse(&AUTO_RAZER, 0x00b9, "", CString::new("x").unwrap(), &t).unwrap();
        assert!(matches!(
            d.apply(&t, Change::Dpi(1600)),
            Err(Error::Protocol(_))
        ));
        assert_eq!(mem.borrow().dpi, 800);
        // A 26000 DPI mouse: offered up to its DPI, so it can be sent back.
        let (t, mem) = super::razer(0x1f, true);
        mem.borrow_mut().dpi = 26000;
        let mut d =
            razer::probe_mouse(&AUTO_RAZER, 0x0099, "", CString::new("x").unwrap(), &t).unwrap();
        let dpi = d.read_state(&t).unwrap().dpi.unwrap();
        assert!(dpi.allowed.contains(26000) && !dpi.allowed.contains(26050));
        d.apply(&t, Change::Dpi(1600)).unwrap();
        d.apply(&t, Change::Dpi(26000)).unwrap();
        assert_eq!(mem.borrow().dpi, 26000);
    }

    #[test]
    fn asus_ignores_a_stale_report() {
        use crate::devices::{AUTO_ASUS, asus};
        let (t, _) = super::asus(4);
        let mut d = asus::probe(&AUTO_ASUS, 0x1a9b, "", CString::new("x").unwrap(), &t).unwrap();
        // A late answer with another preset and DPI waits in the queue.
        let mut stale = vec![0u8; 64];
        stale[4..6].copy_from_slice(&99u16.to_le_bytes());
        stale[11] = 1;
        t.push(stale);
        assert_eq!(d.read_state(&t).unwrap().dpi.unwrap().current, Some(1600));
    }

    #[test]
    fn a_virtual_mouse_in_host_mode_is_never_written() {
        let mut s = Spec::new("Virtual Mouse", 0xd100, 8);
        s.mode = 2;
        let (t, mem, mut d) = open(&s, true);
        assert_eq!(d.read_profile(&t), Err(Error::HostMode));
        assert_eq!(mem.borrow().mode, 2, "reading never changes the mode");
        assert_eq!(mem.borrow().writes, 0);
        // «Activar la memoria del ratón»: mode 1, profile readable, memory
        // untouched.
        d.activate_onboard(&t).unwrap();
        assert_eq!(mem.borrow().mode, 1);
        assert!(d.read_profile(&t).is_ok());
        assert_eq!(mem.borrow().writes, 0);
        // An unverified mouse is never switched.
        let (t, mem, mut d) = open(&s, false);
        assert_eq!(d.activate_onboard(&t), Err(Error::Unsupported));
        assert_eq!(mem.borrow().mode, 2);
    }

    #[test]
    fn a_profile_with_more_buttons_than_the_layout_is_not_touched() {
        for n in [17u8, 20, 24] {
            let s = Spec::new("Virtual Mouse", 0xd200 + n as u16, n);
            let (t, mem, mut d) = open(&s, true);
            assert_eq!(d.read_profile(&t), Err(Error::Unsupported), "{n} buttons");
            assert_eq!(mem.borrow().writes, 0);
        }
        // The codec refuses it too, whatever the caller does.
        let s = Spec::new("Virtual Mouse", 0xd210, 16);
        let user = profile(&s, true);
        let range = DpiRange::Range {
            min: 100,
            max: 16000,
            step: 50,
        };
        let limits = |button_count| onboard::Limits {
            button_count,
            dpi: Some(&range),
            effects: &[],
        };
        let edits = ProfileEdits {
            buttons: vec![(15, Action::Mouse(0x08))],
            ..Default::default()
        };
        assert!(onboard::apply_edits(&user, 1, &directory(), &limits(16), &edits).is_ok());
        let edits = ProfileEdits {
            buttons: vec![(16, Action::Mouse(0x08))],
            ..Default::default()
        };
        assert_eq!(
            onboard::apply_edits(&user, 1, &directory(), &limits(17), &edits),
            Err(EditError::BadProfile)
        );
    }
}
