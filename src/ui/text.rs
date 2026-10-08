//! User-facing text, shared by the CLI, the tray tooltip and the window.
//! State is always spelled out in words, never conveyed by colour alone.

use crate::app::{Shared, Snapshot, Status};
use crate::devices::{DpiRange, Kind, State};
use crate::protocol::{Battery, ChargeState};

pub fn charging(c: ChargeState) -> &'static str {
    match c {
        ChargeState::Discharging => "descargando",
        ChargeState::Charging => "cargando",
        ChargeState::Full => "carga completa",
        ChargeState::Error => "error de batería",
    }
}

pub fn battery(b: Battery) -> String {
    match b.charging {
        ChargeState::Error => "error de batería".into(),
        ChargeState::Discharging if is_low(b.percent) => format!("{} % (baja)", b.percent),
        c => format!("{} % ({})", b.percent, charging(c)),
    }
}

pub fn dpi_range(r: &DpiRange) -> String {
    match r {
        DpiRange::Range { min, max, step } => format!("{min}–{max}, paso {step}"),
        DpiRange::Values(v) => v.iter().map(u16::to_string).collect::<Vec<_>>().join(", "),
    }
}

pub fn hz(ms: u8) -> u32 {
    1000 / ms.max(1) as u32
}

/// Detail lines for a working device.
pub fn state_lines(s: &State) -> Vec<String> {
    let mut lines = vec![match s.battery {
        Some(b) => format!("Batería: {}", battery(b)),
        None => "Batería: no disponible".into(),
    }];
    if let Some(d) = &s.dpi {
        lines.push(format!("DPI: {} ({})", d.current, dpi_range(&d.allowed)));
    }
    if let Some(r) = &s.report_rate {
        lines.push(format!(
            "Tasa de sondeo: {} Hz (solo lectura)",
            hz(r.current_ms)
        ));
    }
    if s.color {
        lines.push("Color: se puede cambiar (no se guarda en el dispositivo)".into());
    }
    lines
}

pub fn status_line(snap: &Snapshot) -> String {
    match &snap.status {
        Status::Searching => "Leyendo…".into(),
        Status::Ok(s) => match s.battery {
            Some(b) => battery(b),
            None => "conectado".into(),
        },
        Status::Failed(e) => match snap.last_battery {
            Some(b) => format!("sin respuesta, último valor {}", battery(b)),
            None => format!("sin respuesta ({e})"),
        },
        Status::Disconnected => "desconectado".into(),
    }
}

/// One line per device for lists and menus.
pub fn device_line(snap: &Snapshot) -> String {
    format!(
        "{} {}: {}",
        snap.info.vendor,
        snap.info.name,
        status_line(snap)
    )
}

/// Tray tooltip; Windows truncates it at 127 characters.
pub fn tooltip(shared: &Shared) -> String {
    let mut lines: Vec<String> = shared
        .devices
        .iter()
        .filter_map(|d| {
            let b = match &d.status {
                Status::Ok(s) => s.battery,
                Status::Failed(_) => d.last_battery,
                _ => None,
            }?;
            Some(format!(
                "{}: {}{}",
                short_name(&d.info.name),
                battery(b),
                super::profiles::tooltip_suffix(d)
            ))
        })
        .collect();
    if lines.is_empty() {
        lines.push(if shared.scanned {
            "Sin baterías que mostrar".into()
        } else {
            "Buscando dispositivos…".into()
        });
    }
    let mut text = format!("OpenPeripheral\n{}", lines.join("\n"));
    if text.chars().count() > 127 {
        text = text.chars().take(126).collect::<String>() + "…";
    }
    text
}

/// "G305 Lightspeed Wireless Gaming Mouse" -> "G305 Lightspeed".
fn short_name(name: &str) -> String {
    name.split_whitespace()
        .take(2)
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn conflicts(names: &[&str]) -> String {
    if names.is_empty() {
        String::new()
    } else {
        format!(
            "En ejecución: {}. Puede sobrescribir los ajustes; ciérralo si un cambio no se mantiene.",
            names.join(", ")
        )
    }
}

/// Names of a mouse's buttons in profile order (G305 layout, observed by
/// comparing the user and factory profiles).
pub fn mouse_button(index: usize) -> String {
    const NAMES: [&str; 6] = [
        "Izquierdo",
        "Derecho",
        "Central (rueda)",
        "Lateral trasero",
        "Lateral delantero",
        "Botón DPI",
    ];
    NAMES
        .get(index)
        .map(|n| n.to_string())
        .unwrap_or_else(|| format!("Botón {}", index + 1))
}

fn key_name(usage: u8) -> String {
    match usage {
        0x04..=0x1d => ((b'A' + usage - 0x04) as char).to_string(),
        0x1e..=0x26 => ((b'1' + usage - 0x1e) as char).to_string(),
        0x27 => "0".into(),
        0x28 => "Intro".into(),
        0x29 => "Esc".into(),
        0x2a => "Retroceso".into(),
        0x2b => "Tab".into(),
        0x2c => "Espacio".into(),
        0x37 => ".".into(),

        0x3a..=0x45 => format!("F{}", usage - 0x3a + 1),
        0x49 => "Insert".into(),
        0x4a => "Inicio".into(),
        0x4b => "RePág".into(),
        0x4c => "Supr".into(),
        0x4d => "Fin".into(),
        0x4e => "AvPág".into(),
        0x4f => "Flecha derecha".into(),
        0x50 => "Flecha izquierda".into(),
        0x51 => "Flecha abajo".into(),
        0x52 => "Flecha arriba".into(),
        // Numeric keypad: the same place on every keyboard layout.
        0x56 => "- (teclado numérico)".into(),
        0x57 => "+ (teclado numérico)".into(),
        0x62 => "0 (teclado numérico)".into(),
        u => format!("Tecla 0x{u:02X}"),
    }
}

/// "Ctrl + Mayús + V" from HID modifier bits and usage id.
pub fn shortcut(modifiers: u8, usage: u8) -> String {
    const MODS: [(u8, &str); 4] = [
        (0x11, "Ctrl"),
        (0x22, "Mayús"),
        (0x44, "Alt"),
        (0x88, "Win"),
    ];
    let mut parts: Vec<String> = MODS
        .iter()
        .filter(|(m, _)| modifiers & m != 0)
        .map(|(_, n)| n.to_string())
        .collect();
    parts.push(key_name(usage));
    parts.join(" + ")
}

/// What a button does, in words.
pub fn action(a: crate::protocol::onboard::Action) -> String {
    use crate::protocol::onboard::Action;
    match a {
        Action::Mouse(0x01) => "Clic izquierdo".into(),
        Action::Mouse(0x02) => "Clic derecho".into(),
        Action::Mouse(0x04) => "Clic central".into(),
        Action::Mouse(0x08) => "Atrás".into(),
        Action::Mouse(0x10) => "Adelante".into(),
        Action::Mouse(m) => format!("Botones de ratón 0x{m:04X}"),
        Action::Keyboard { modifiers, usage } => format!("Atajo {}", shortcut(modifiers, usage)),
        Action::Raw([0x90, ..]) => "Función del perfil".into(),
        Action::Raw(_) => "Acción no reconocida (no editable)".into(),
    }
}

/// DESIGN.md: 0–20 % is "Batería baja".
pub fn is_low(percent: u8) -> bool {
    percent <= 20
}

/// Lit segments of the 5-segment battery: one per started 20 %.
pub fn battery_bars(percent: u8) -> u8 {
    percent.min(100).div_ceil(20).max(1)
}

pub fn kind(k: Kind) -> &'static str {
    match k {
        Kind::Mouse => "Ratón",
        Kind::Keyboard => "Teclado",
        Kind::Gamepad => "Mando",
    }
}

/// What the energy block of a card shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Energy {
    Battery {
        percent: u8,
        bars: u8,
        charge: ChargeState,
        stale: bool,
    },
    BatteryError,
    Wired,
    Unknown,
}

pub fn energy(snap: &Snapshot) -> Energy {
    let (battery, stale) = match &snap.status {
        Status::Ok(s) => (s.battery, false),
        Status::Failed(_) => (snap.last_battery, true),
        _ => (None, false),
    };
    match battery {
        Some(b) if b.charging == ChargeState::Error => Energy::BatteryError,
        Some(b) => Energy::Battery {
            percent: b.percent,
            bars: battery_bars(b.percent),
            charge: b.charging,
            stale,
        },
        None if snap.info.wired && !matches!(snap.status, Status::Disconnected) => Energy::Wired,
        None => Energy::Unknown,
    }
}

/// Second line of a card, e.g. "Conectado · Receptor LIGHTSPEED".
pub fn card_status(snap: &Snapshot) -> String {
    match &snap.status {
        Status::Searching => "Leyendo…".into(),
        Status::Ok(_) => format!("Conectado · {}", snap.info.connection),
        Status::Failed(_) => "Sin respuesta · ¿apagado o en reposo?".into(),
        Status::Disconnected => "Desconectado".into(),
    }
}

/// Capabilities line, e.g. "1000 DPI · 1000 Hz". Colour is drawn apart.
pub fn capabilities(snap: &Snapshot) -> Option<String> {
    let Status::Ok(s) = &snap.status else {
        return None;
    };
    let mut parts = Vec::new();
    if let Some(d) = &s.dpi {
        parts.push(format!("{} DPI", d.current));
    }
    if let Some(r) = &s.report_rate {
        parts.push(format!("{} Hz", hz(r.current_ms)));
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

pub fn color_label(snap: &Snapshot) -> &'static str {
    match snap.info.kind {
        Kind::Gamepad => "Barra de luz aplicada",
        _ => "Color aplicado",
    }
}

/// Colour row of a card: the colour this app applied (keyboards, pads) or
/// a mouse's saved light, e.g. ("Luz fija", Some(#00C8FF)) or
/// ("Luz en ciclo de colores", None).
pub fn color_row(snap: &Snapshot) -> Option<(&'static str, Option<crate::protocol::razer::Rgb>)> {
    use crate::protocol::onboard::Light;
    if let Some(c) = snap.applied_color {
        return Some((color_label(snap), Some(c)));
    }
    let profile = match &snap.profile {
        crate::app::Profile::Read(v) => v.light,
        _ => None,
    };
    Some(match snap.saved_light.or(profile)? {
        Light::Off => ("Luz apagada", None),
        Light::Fixed(c) => ("Luz fija", Some(c)),
        Light::Cycle { .. } => ("Luz en ciclo de colores", None),
        Light::Breathing { color, .. } => ("Luz en respiración", Some(color)),
    })
}

/// Full text a screen reader announces for a card (one accessible name).
pub fn card_summary(snap: &Snapshot) -> String {
    let mut parts = vec![
        snap.info.name.clone(),
        kind(snap.info.kind).to_string(),
        card_status(snap),
    ];
    match energy(snap) {
        Energy::Battery {
            percent,
            bars,
            charge,
            stale,
        } => {
            let mut b = format!("Nivel de batería: {bars} de 5, {percent} %");
            if stale {
                b.push_str(", último valor conocido");
            } else {
                b.push_str(&format!(", {}", charging(charge)));
            }
            if is_low(percent) {
                b.push_str(", batería baja");
            }
            parts.push(b);
        }
        Energy::BatteryError => parts.push("Error de batería".into()),
        Energy::Wired => parts.push("Con cable, sin batería".into()),
        Energy::Unknown => {}
    }
    if let Some(c) = capabilities(snap) {
        parts.push(c);
    }
    if let Some((label, c)) = color_row(snap) {
        parts.push(match c {
            Some(c) => format!("{label} {}", crate::config::to_hex(c)),
            None => label.to_string(),
        });
    }
    if matches!(snap.status, Status::Disconnected) && snap.saved {
        parts.push("Los ajustes guardados se aplicarán al volver a conectarlo".into());
    }
    parts.join(". ")
}

/// Header counter: devices currently present (answering or not).
pub fn connected_count(shared: &Shared) -> String {
    let n = shared
        .devices
        .iter()
        .filter(|d| !matches!(d.status, Status::Disconnected))
        .count();
    match n {
        1 => "1 conectado".into(),
        n => format!("{n} conectados"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{DeviceInfo, DeviceKey, Dpi, Error, Kind};

    fn snap(name: &str, status: Status) -> Snapshot {
        Snapshot {
            info: DeviceInfo {
                key: DeviceKey(name.into()),
                vendor: "V",
                name: name.into(),
                kind: Kind::Mouse,
                connection: "USB",
                wired: false,
            },
            status,
            last_battery: None,
            saved: false,
            saved_dpi: None,
            saved_light: None,
            applied_color: None,
            original_dpi: None,
            profile: crate::app::Profile::NotRead,
            backup: None,
            profiles: Vec::new(),
            active_profile: None,
            programs: Vec::new(),
            game: None,
            game_previous: None,
            unanswered: Vec::new(),
        }
    }

    #[test]
    fn battery_text_never_relies_on_colour() {
        assert_eq!(
            battery(Battery {
                percent: 90,
                charging: ChargeState::Discharging
            }),
            "90 % (descargando)"
        );
        assert_eq!(
            battery(Battery {
                percent: 10,
                charging: ChargeState::Discharging
            }),
            "10 % (baja)"
        );
        assert_eq!(
            battery(Battery {
                percent: 10,
                charging: ChargeState::Charging
            }),
            "10 % (cargando)"
        );
        assert_eq!(
            battery(Battery {
                percent: 0,
                charging: ChargeState::Error
            }),
            "error de batería"
        );
    }

    #[test]
    fn detail_lines() {
        let s = State {
            battery: None,
            dpi: Some(Dpi {
                current: 800,
                allowed: DpiRange::Range {
                    min: 200,
                    max: 12000,
                    step: 50,
                },
            }),
            report_rate: None,
            color: true,
        };
        let l = state_lines(&s);
        assert_eq!(l[0], "Batería: no disponible");
        assert_eq!(l[1], "DPI: 800 (200–12000, paso 50)");
        assert_eq!(l.len(), 3);
    }

    #[test]
    fn tooltip_content() {
        let mut shared = Shared::default();
        assert_eq!(tooltip(&shared), "OpenPeripheral\nBuscando dispositivos…");
        shared.scanned = true;
        assert_eq!(tooltip(&shared), "OpenPeripheral\nSin baterías que mostrar");
        let b = Battery {
            percent: 90,
            charging: ChargeState::Discharging,
        };
        shared.devices.push(snap(
            "G305 Lightspeed Wireless Gaming Mouse",
            Status::Ok(State {
                battery: Some(b),
                ..State::default()
            }),
        ));
        let mut asleep = snap("DualSense (USB)", Status::Failed(Error::Timeout));
        asleep.last_battery = Some(b);
        shared.devices.push(asleep);
        shared
            .devices
            .push(snap("Teclado", Status::Ok(State::default())));
        assert_eq!(
            tooltip(&shared),
            "OpenPeripheral\nG305 Lightspeed: 90 % (descargando)\nDualSense (USB): 90 % (descargando)"
        );

        for i in 0..20 {
            shared.devices.push(snap(
                &format!("Mando {i}"),
                Status::Ok(State {
                    battery: Some(b),
                    ..State::default()
                }),
            ));
        }
        assert!(tooltip(&shared).chars().count() <= 127);
    }

    #[test]
    fn status_lines() {
        assert_eq!(
            status_line(&snap("x", Status::Disconnected)),
            "desconectado"
        );
        assert_eq!(
            status_line(&snap("x", Status::Failed(Error::Timeout))),
            "sin respuesta (el dispositivo no responde)"
        );
        assert_eq!(conflicts(&[]), "");
        assert!(conflicts(&["Logitech G HUB"]).starts_with("En ejecución: Logitech G HUB."));
    }

    #[test]
    fn battery_bar_thresholds() {
        let cases = [
            (0, 1),
            (1, 1),
            (20, 1),
            (21, 2),
            (40, 2),
            (41, 3),
            (60, 3),
            (61, 4),
            (80, 4),
            (81, 5),
            (100, 5),
            (255, 5),
        ];
        for (percent, bars) in cases {
            assert_eq!(battery_bars(percent), bars, "{percent} %");
        }
        assert!(is_low(20) && !is_low(21));
    }

    fn with_battery(percent: u8, charging: ChargeState) -> Snapshot {
        let b = Battery { percent, charging };
        snap(
            "G305",
            Status::Ok(State {
                battery: Some(b),
                ..State::default()
            }),
        )
    }

    #[test]
    fn energy_block() {
        assert_eq!(
            energy(&with_battery(90, ChargeState::Discharging)),
            Energy::Battery {
                percent: 90,
                bars: 5,
                charge: ChargeState::Discharging,
                stale: false
            }
        );
        assert_eq!(
            energy(&with_battery(0, ChargeState::Error)),
            Energy::BatteryError
        );

        let mut asleep = snap("G305", Status::Failed(Error::Timeout));
        asleep.last_battery = Some(Battery {
            percent: 30,
            charging: ChargeState::Discharging,
        });
        assert!(matches!(
            energy(&asleep),
            Energy::Battery {
                bars: 2,
                stale: true,
                ..
            }
        ));

        let mut keyboard = snap("BlackWidow", Status::Ok(State::default()));
        keyboard.info.wired = true;
        assert_eq!(energy(&keyboard), Energy::Wired);
        keyboard.status = Status::Disconnected;
        assert_eq!(energy(&keyboard), Energy::Unknown);
        assert_eq!(
            energy(&snap("x", Status::Ok(State::default()))),
            Energy::Unknown
        );
    }

    #[test]
    fn card_texts() {
        let mut g = with_battery(90, ChargeState::Discharging);
        g.info.connection = "Receptor LIGHTSPEED";
        if let Status::Ok(s) = &mut g.status {
            s.dpi = Some(Dpi {
                current: 1000,
                allowed: DpiRange::Values(vec![1000]),
            });
            s.report_rate = Some(crate::devices::ReportRate {
                current_ms: 1,
                supported_ms: vec![1],
            });
        }
        assert_eq!(card_status(&g), "Conectado · Receptor LIGHTSPEED");
        assert_eq!(capabilities(&g).as_deref(), Some("1000 DPI · 1000 Hz"));
        assert_eq!(
            card_summary(&g),
            "G305. Ratón. Conectado · Receptor LIGHTSPEED. Nivel de batería: 5 de 5, 90 %, descargando. 1000 DPI · 1000 Hz"
        );

        let low = with_battery(12, ChargeState::Discharging);
        assert!(
            card_summary(&low)
                .contains("Nivel de batería: 1 de 5, 12 %, descargando, batería baja")
        );

        let mut mouse = snap("G305", Status::Ok(State::default()));
        mouse.saved_light = Some(crate::protocol::onboard::Light::Breathing {
            color: crate::protocol::razer::Rgb {
                r: 0,
                g: 0xc8,
                b: 0xff,
            },
            period_ms: 6000,
        });
        assert!(card_summary(&mouse).ends_with("Luz en respiración #00C8FF"));
        mouse.saved_light = Some(crate::protocol::onboard::Light::Cycle { period_ms: 8000 });
        assert!(card_summary(&mouse).ends_with("Luz en ciclo de colores"));

        let mut pad = snap("DualSense", Status::Disconnected);
        pad.info.kind = Kind::Gamepad;
        pad.saved = true;
        pad.applied_color = Some(crate::protocol::razer::Rgb { r: 0, g: 0, b: 255 });
        assert_eq!(card_status(&pad), "Desconectado");
        assert_eq!(capabilities(&pad), None);
        assert_eq!(
            card_summary(&pad),
            "DualSense. Mando. Desconectado. Barra de luz aplicada #0000FF. Los ajustes guardados se aplicarán al volver a conectarlo"
        );
        assert_eq!(
            card_status(&snap("x", Status::Failed(Error::Timeout))),
            "Sin respuesta · ¿apagado o en reposo?"
        );
    }

    #[test]
    fn counter() {
        let mut shared = Shared::default();
        assert_eq!(connected_count(&shared), "0 conectados");
        shared.devices.push(snap("a", Status::Ok(State::default())));
        assert_eq!(connected_count(&shared), "1 conectado");
        shared
            .devices
            .push(snap("b", Status::Failed(Error::Timeout)));
        shared.devices.push(snap("c", Status::Disconnected));
        assert_eq!(connected_count(&shared), "2 conectados");
    }

    #[test]
    fn button_and_action_names() {
        use crate::protocol::onboard::Action;
        assert_eq!(mouse_button(0), "Izquierdo");
        assert_eq!(mouse_button(5), "Botón DPI");
        assert_eq!(mouse_button(7), "Botón 8");
        assert_eq!(action(Action::Mouse(0x08)), "Atrás");
        assert_eq!(
            action(Action::Keyboard {
                modifiers: 0x01,
                usage: 0x19
            }),
            "Atajo Ctrl + V"
        );
        assert_eq!(
            action(Action::Keyboard {
                modifiers: 0x03,
                usage: 0x06
            }),
            "Atajo Ctrl + Mayús + C"
        );
        assert_eq!(
            action(Action::Keyboard {
                modifiers: 0x00,
                usage: 0x3b
            }),
            "Atajo F2"
        );
        assert_eq!(
            action(Action::Raw([0x90, 0x0c, 0, 0])),
            "Función del perfil"
        );
        assert_eq!(
            action(Action::Raw([0x20, 0, 0, 0])),
            "Acción no reconocida (no editable)"
        );
        assert_eq!(shortcut(0x40, 0x1f), "Alt + 2");
    }
}
