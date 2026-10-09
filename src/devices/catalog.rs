//! Best-selling mice of several brands (research of 2026-10-09, plan
//! `banco-ratones-virtuales`) as test data: what the app offers each one
//! today. Logitech HID++ models run on the virtual mouse (`sim`); the rest
//! only through the OS enumeration, which is all the app does with them.
//!
//! Sources: Amazon US best-seller estimates (AsinSight, 2026), PcComponentes,
//! ProSettings, and USB ids from libratbag, OpenRazer, rivalcfg, ckb-next,
//! usb.ids and the Linux `hid-ids.h`. `ids_checked: false` marks an id taken
//! from one source or unknown (a made-up pid in the 0xFFxx range).

use super::sim::{self, Spec};
use super::{AUTO_LOGITECH, Change, Device, Error, Kind, SUPPORTED, Support, logitech, lookup};
use std::ffi::CString;

/// How the app could reach the mouse's settings.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    /// Logitech HID++ 2.0 by cable, or behind a receiver.
    Hidpp { wireless: bool },
    /// Logitech by Bluetooth: HID++ on page 0xFF43, not used by the app.
    HidppBluetooth,
    /// 90-byte feature reports. References: GPL only (facts usable).
    Razer,
    /// References: libratbag (MIT) and rivalcfg (WTFPL).
    SteelSeries,
    /// References: ckb-next (GPL only).
    Corsair,
    /// Sinowealth chip (Glorious, many unbranded): libratbag (MIT).
    Sinowealth,
    /// References: libratbag asus (MIT).
    Asus,
    /// Holtek chip (Redragon): mouse_m908 (GPL only), known to brick.
    Holtek,
    /// No public reference found (HyperX, Pulsar/VXE…).
    Unknown,
    /// Nothing to set from a PC (Zowie: buttons under the mouse; office
    /// mice without software).
    Nothing,
}

pub struct Model {
    pub brand: &'static str,
    pub name: &'static str,
    pub vid: u16,
    pub pid: u16,
    pub ids_checked: bool,
    pub buttons: u8,
    /// Where the buttons are, as sold.
    pub layout: &'static str,
    pub light: &'static str,
    pub max_dpi: Option<u16>,
    pub protocol: Protocol,
}

const fn m(
    brand: &'static str,
    name: &'static str,
    (vid, pid, ids_checked): (u16, u16, bool),
    (buttons, layout, light): (u8, &'static str, &'static str),
    max_dpi: Option<u16>,
    protocol: Protocol,
) -> Model {
    Model {
        brand,
        name,
        vid,
        pid,
        ids_checked,
        buttons,
        layout,
        light,
        max_dpi,
        protocol,
    }
}

use Protocol::*;
const WIRED: Protocol = Hidpp { wireless: false };
const RECEIVER: Protocol = Hidpp { wireless: true };
const STD5: &str = "izq., der., rueda, 2 laterales";
const STD6: &str = "izq., der., rueda, 2 laterales, DPI tras la rueda";

pub static CATALOG: &[Model] = &[
    // Logitech gaming
    m(
        "Logitech",
        "G502 HERO",
        (0x046d, 0xc08b, true),
        (
            11,
            "izq., der., rueda, inclinación izq./der., 2 laterales, francotirador, DPI+ y DPI−",
            "RGB logo + indicadores DPI",
        ),
        Some(25600),
        WIRED,
    ),
    m(
        "Logitech",
        "G305 Lightspeed",
        (0x046d, 0xc53f, true),
        (6, STD6, "ninguna"),
        Some(12000),
        RECEIVER,
    ),
    m(
        "Logitech",
        "G203 LIGHTSYNC",
        (0x046d, 0xc092, true),
        (6, STD6, "RGB franja trasera"),
        Some(8000),
        WIRED,
    ),
    m(
        "Logitech",
        "G Pro X Superlight",
        (0x046d, 0xc094, true),
        (5, STD5, "ninguna"),
        Some(25600),
        WIRED,
    ),
    m(
        "Logitech",
        "G502 X Plus",
        (0x046d, 0xc095, true),
        (13, "disposición del G502", "RGB 8 zonas"),
        Some(25600),
        WIRED,
    ),
    m(
        "Logitech",
        "G502 HERO Lightspeed",
        (0x046d, 0xc08d, true),
        (11, "disposición del G502", "RGB logo"),
        Some(25600),
        WIRED,
    ),
    // Logitech office (behind their receiver; no adjustable DPI)
    m(
        "Logitech",
        "M185",
        (0x046d, 0xc534, false),
        (3, "izq., der., rueda", "ninguna"),
        None,
        RECEIVER,
    ),
    m(
        "Logitech",
        "M510",
        (0x046d, 0xc52b, true),
        (
            7,
            "izq., der., rueda, inclinación izq./der., atrás, adelante",
            "ninguna",
        ),
        None,
        RECEIVER,
    ),
    m(
        "Logitech",
        "M720 Triathlon",
        (0x046d, 0xc52b, true),
        (
            8,
            "izq., der., rueda, atrás, adelante, pulgar, cambio de equipo",
            "ninguna",
        ),
        None,
        RECEIVER,
    ),
    m(
        "Logitech",
        "MX Master 3S (Bluetooth)",
        (0x046d, 0xb034, true),
        (
            7,
            "izq., der., rueda, rueda de pulgar, atrás, adelante, gestos",
            "ninguna",
        ),
        Some(8000),
        HidppBluetooth,
    ),
    m(
        "Logitech",
        "MX Master 4 (Bluetooth)",
        (0x046d, 0xb042, true),
        (
            8,
            "izq., der., rueda, atrás, adelante, gestos, modo de rueda, Actions Ring",
            "ninguna",
        ),
        Some(8000),
        HidppBluetooth,
    ),
    // Razer
    m(
        "Razer",
        "DeathAdder Essential",
        (0x1532, 0x006e, true),
        (5, STD5, "verde, 1 zona"),
        Some(6400),
        Razer,
    ),
    m(
        "Razer",
        "Basilisk V3",
        (0x1532, 0x0099, true),
        (
            11,
            "izq., der., rueda, inclinación izq./der., 2 laterales, embrague, DPI, modo de rueda",
            "Chroma 11 zonas",
        ),
        Some(26000),
        Razer,
    ),
    m(
        "Razer",
        "Basilisk V3 X HyperSpeed",
        (0x1532, 0x00b9, true),
        (
            7,
            "izq., der., rueda, 2 laterales, embrague, DPI",
            "Chroma rueda",
        ),
        Some(18000),
        Razer,
    ),
    m(
        "Razer",
        "DeathAdder V3 Pro",
        (0x1532, 0x00b6, true),
        (5, STD5, "ninguna"),
        Some(30000),
        Razer,
    ),
    m(
        "Razer",
        "Viper V3 Pro",
        (0x1532, 0x00c0, true),
        (
            6,
            "izq., der., rueda, 2 laterales, DPI en la base",
            "ninguna",
        ),
        Some(35000),
        Razer,
    ),
    m(
        "Razer",
        "Naga X",
        (0x1532, 0x0096, true),
        (
            16,
            "izq., der., rueda, DPI, 12 en el pulgar",
            "rueda y panel lateral",
        ),
        Some(18000),
        Razer,
    ),
    // SteelSeries
    m(
        "SteelSeries",
        "Rival 3",
        (0x1038, 0x1824, true),
        (
            6,
            "izq., der., rueda, 2 laterales, CPI tras la rueda",
            "Prism 3 zonas",
        ),
        Some(8500),
        SteelSeries,
    ),
    m(
        "SteelSeries",
        "Aerox 3 Wireless",
        (0x1038, 0x183a, true),
        (6, "izq., der., rueda, 2 laterales, CPI", "3 zonas"),
        Some(18000),
        SteelSeries,
    ),
    // Corsair
    m(
        "Corsair",
        "Harpoon RGB Pro",
        (0x1b1c, 0x1b75, true),
        (6, "izq., der., rueda, 2 laterales, DPI", "RGB logo"),
        Some(12000),
        Corsair,
    ),
    m(
        "Corsair",
        "M65 RGB Elite",
        (0x1b1c, 0x1b5a, true),
        (
            8,
            "izq., der., rueda, 2 laterales, francotirador, DPI+ y DPI−",
            "RGB",
        ),
        Some(18000),
        Corsair,
    ),
    m(
        "Corsair",
        "Scimitar Elite",
        (0x1b1c, 0x1b8b, true),
        (17, "izq., der., rueda, DPI, 12 en el pulgar…", "RGB"),
        None,
        Corsair,
    ),
    // Others
    m(
        "HyperX",
        "Pulsefire Haste 2",
        (0x03f0, 0xff01, false),
        (6, "izq., der., rueda, 2 laterales, DPI", "RGB"),
        Some(26000),
        Unknown,
    ),
    m(
        "Glorious",
        "Model O",
        (0x258a, 0x0036, true),
        (
            6,
            "izq., der., rueda, 2 laterales, DPI bajo el ratón",
            "RGB perimetral",
        ),
        Some(12000),
        Sinowealth,
    ),
    m(
        "Redragon",
        "M612 Predator",
        (0x04d9, 0xff02, false),
        (
            11,
            "izq., der., rueda, disparo rápido, 2 laterales de macro, DPI…",
            "RGB 5 modos",
        ),
        Some(8000),
        Holtek,
    ),
    m(
        "Redragon",
        "M908 Impact",
        (0x04d9, 0xfc4d, true),
        (18, "7 principales y 12 laterales", "RGB"),
        Some(12400),
        Holtek,
    ),
    m(
        "Zowie",
        "EC2",
        (0x1af3, 0x0001, true),
        (
            5,
            "izq., der., rueda, 2 laterales; DPI bajo el ratón",
            "ninguna",
        ),
        Some(3200),
        Nothing,
    ),
    m(
        "ASUS",
        "TUF Gaming M3 Gen II",
        (0x0b05, 0x1a9b, true),
        (8, "izq., der., rueda, 2 laterales, DPI…", "1 LED"),
        Some(8000),
        Asus,
    ),
    m(
        "VXE",
        "Dragonfly R1",
        (0x3554, 0xff03, false),
        (5, STD5, "ninguna"),
        Some(26000),
        Unknown,
    ),
    // Office, no brand software
    m(
        "Microsoft",
        "Basic Optical Mouse",
        (0x045e, 0x0083, true),
        (3, "izq., der., rueda", "ninguna"),
        None,
        Nothing,
    ),
    m(
        "Amazon Basics",
        "Ratón con cable",
        (0xffff, 0xff04, false),
        (3, "izq., der., rueda", "ninguna"),
        None,
        Nothing,
    ),
    m(
        "TECKNET",
        "Inalámbrico 2.4G",
        (0xffff, 0xff05, false),
        (3, "izq., der., rueda", "ninguna"),
        None,
        Nothing,
    ),
];

/// What the app offers a model today.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Offer {
    pub card: &'static str,
    pub dpi: bool,
    pub light: bool,
    pub buttons: bool,
}

/// Runs a model through the app's code: Logitech HID++ on the virtual
/// mouse, any other through `lookup` and the enumeration rules.
pub fn offer(model: &Model) -> Offer {
    // (behind a receiver, HID++ collection) of the Logitech models.
    let hidpp = match model.protocol {
        Hidpp { wireless } => Some((wireless, 0xff00, 0x0002)),
        HidppBluetooth => Some((false, 0xff43, 0x0202)),
        _ => None,
    };
    if let Some((wireless, usage_page, usage)) = hidpp {
        let mut s = Spec::new(model.name, model.pid, model.buttons);
        s.wireless = wireless;
        s.dpi = model.max_dpi.map(|max| (100, max, 50));
        s.effects = if model.light == "ninguna" {
            &[]
        } else {
            &[0x00, 0x01, 0x03, 0x0a]
        };
        // The enumerated collection decides the entry, as in `scan`.
        let entry = lookup(
            model.vid,
            model.pid,
            usage_page,
            usage,
            if wireless { 2 } else { 1 },
        )
        .expect("a Logitech HID++ collection is always accepted");
        let (t, mem) = sim::mouse(&s);
        let mut found = logitech::probe(entry, model.pid, CString::new("x").unwrap(), &t);
        let mut d: Box<dyn Device> = Box::new(found.remove(0));
        let state = d.read_state(&t).unwrap();
        let dpi = match state.dpi {
            Some(dpi) => d.apply(&t, Change::Dpi(dpi.current.unwrap())).is_ok(),
            None => false,
        };
        let profile = d.read_profile(&t);
        assert_eq!(
            mem.borrow().writes,
            0,
            "{}: reading never writes",
            model.name
        );
        let buttons = profile.is_ok();
        assert!(buttons || profile == Err(Error::Unsupported));
        let light = !s.effects.is_empty();
        return Offer {
            card: match d.info().support {
                Support::Verified => "verificado",
                _ => "autodetectado",
            },
            dpi,
            light,
            buttons,
        };
    }
    if model.protocol == SteelSeries {
        if let Some(m) = crate::protocol::steelseries::model(model.pid) {
            let entry = lookup(model.vid, model.pid, 0xffc0, 0x0001, m.interface)
                .expect("its vendor collection is taken");
            let t = super::fake::FakeTransport::new(|_| vec![]);
            let mut d = super::steelseries::probe(entry, model.pid, "", CString::new("x").unwrap())
                .expect("a model of the table");
            let dpi = d.apply(&t, Change::Dpi(1600)).is_ok();
            return Offer {
                card: "autodetectado",
                dpi,
                light: false,
                buttons: false,
            };
        }
    }
    if model.protocol == Asus && crate::protocol::asus::model(model.pid).is_some() {
        let entry = lookup(model.vid, model.pid, 0xff01, 0x0001, 0).expect("its vendor collection");
        let (t, mem) = sim::asus(4);
        let mut d = super::asus::probe(entry, model.pid, "", CString::new("x").unwrap(), &t)
            .expect("an ASUS mouse answers");
        let dpi = d.apply(&t, Change::Dpi(1600)).is_ok();
        assert!(!mem.borrow().commands.contains(&0x0350));
        return Offer {
            card: "autodetectado",
            dpi,
            light: false,
            buttons: false,
        };
    }
    if model.protocol == Razer {
        // Probed on its mouse collection with read-only commands.
        let entry = lookup(model.vid, model.pid, 0x0001, 0x0002, 0)
            .expect("a Razer mouse collection is always probed");
        let (t, mem) = sim::razer(0x1f, true);
        let name = format!("{} {}", model.brand, model.name);
        let mut d =
            super::razer::probe_mouse(entry, model.pid, &name, CString::new("x").unwrap(), &t)
                .expect("a Razer mouse answers");
        let dpi = d.apply(&t, Change::Dpi(1600)).is_ok();
        assert_eq!(mem.borrow().stores, [0], "{}: never stored", model.name);
        return Offer {
            card: "autodetectado",
            dpi,
            light: false,
            buttons: false,
        };
    }
    // Any other brand: only its standard mouse collection is looked at,
    // and no driver may claim it (the device is never opened).
    assert!(
        lookup(model.vid, model.pid, 0x0001, 0x0002, 0).is_none(),
        "{} must not be opened",
        model.name
    );
    assert_eq!(
        crate::platform::hid::listed_kind(0x0001, 0x0002),
        Some(Kind::Mouse)
    );
    Offer {
        card: "sin ajustes",
        dpi: false,
        light: false,
        buttons: false,
    }
}

/// What could be added, from the research (not implemented).
pub fn could(model: &Model) -> &'static str {
    match model.protocol {
        Hidpp { .. } if model.max_dpi.is_none() => "nada más: el ratón no tiene DPI ajustable",
        Hidpp { .. } => "botones al verificarlo con el ratón (formato 3/4)",
        HidppBluetooth => "botones: no, sin verificar un modelo por Bluetooth",
        Razer => {
            "luz y botones: solo con hechos del protocolo (código GPL); sin probar con un ratón real"
        }
        SteelSeries => "luz y botones portando rivalcfg (WTFPL); sin probar con un ratón real",
        Sinowealth => "DPI, botones y luz portando libratbag (MIT); riesgo medio",
        Asus => "luz y botones portando libratbag (MIT); sin probar con un ratón real",
        Corsair => "solo con sala limpia; referencias GPL",
        Holtek => "no recomendado: riesgo conocido de inutilizar el ratón",
        Unknown => "sin referencia pública",
        Nothing => "nada que ajustar por software",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_best_selling_mouse_is_shown_and_others_are_never_opened() {
        for model in CATALOG {
            let o = offer(model);
            match model.protocol {
                SteelSeries | Asus => {
                    assert_eq!(o.dpi, o.card == "autodetectado", "{}", model.name)
                }
                Hidpp { .. } | HidppBluetooth | Razer => {
                    assert_ne!(o.card, "sin ajustes", "{}", model.name);
                    assert_eq!(o.dpi, model.max_dpi.is_some(), "{}", model.name);
                }
                _ => assert_eq!(o.card, "sin ajustes", "{}", model.name),
            }
        }
        // The two verified models keep their onboard profile.
        for name in ["G305 Lightspeed", "G203 LIGHTSYNC"] {
            let model = CATALOG.iter().find(|m| m.name == name).unwrap();
            assert!(offer(model).buttons, "{name}");
        }
        assert!(
            SUPPORTED
                .iter()
                .all(|s| s.vid != 0x1532 || s.kind != Kind::Mouse)
        );
        assert!(AUTO_LOGITECH.is_auto());
    }

    /// The table for the plan: `cargo test catalog_report -- --ignored
    /// --nocapture`.
    #[test]
    #[ignore]
    fn catalog_report() {
        println!(
            "| Marca | Modelo | Botones | Dónde | Luz | Hoy en la app | DPI | Luz | Botones | Se podría |"
        );
        println!("|---|---|---|---|---|---|---|---|---|---|");
        let yes = |b: bool| if b { "sí" } else { "no" };
        for model in CATALOG {
            let o = offer(model);
            println!(
                "| {} | {}{} | {} | {} | {} | {} | {} | {} | {} | {} |",
                model.brand,
                model.name,
                if model.ids_checked { "" } else { " (id s/v)" },
                model.buttons,
                model.layout,
                model.light,
                o.card,
                yes(o.dpi),
                yes(o.light),
                yes(o.buttons),
                could(model)
            );
        }
    }
}
