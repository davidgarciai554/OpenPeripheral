//! Mouse profiles kept on this PC (plan `perfiles-raton`, increment A).
//!
//! The G305 holds a single onboard profile, so the named profiles live in the
//! app's config and "Usar" writes one of them into the mouse. A profile only
//! carries what this app can write: the start DPI, the light and the
//! assignable button actions.
//!
//! Limit: profiles are stored per model key (`vid:pid/model`, no serial), so
//! two mice of the same model share them (decision of the user, 2026-10-07).

use crate::app::Command;
use crate::config::{light_text, parse_light};
use crate::devices::DeviceKey;
use crate::protocol::onboard::{self, Action, Light, ProfileEdits};
use serde::{Deserialize, Serialize};

pub const MAX_PROFILES: usize = 8;
pub const MAX_NAME: usize = 32;
/// Generous sanity bound; the sensor range is checked when writing.
const MAX_DPI: u16 = 50_000;
/// Name given to the profile created from what the mouse holds.
pub const FIRST_NAME: &str = "Perfil 1";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MouseProfile {
    pub name: String,
    /// DPI the mouse starts with (fills the 4 entries of the cycle). `None`
    /// leaves the DPI as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dpi: Option<u16>,
    /// Same text as `DeviceSettings::light`. `None` leaves the light as it is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub light: Option<String>,
    /// `"index:hex8"` per button, e.g. `"3:80020106"`. Only assignable
    /// actions are kept: profile functions (`90 xx`, the DPI button) cannot
    /// be written by this app, so a profile leaves them as they are.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub buttons: Vec<String>,
}

/// Proof that a profile write comes from a profile the user saved and chose
/// ("Usar"). Only this module creates it, through [`use_command`]; the worker
/// resolves the profile from the validated config, never from free edits.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ProfileApply(());

pub fn use_command(key: DeviceKey, name: String) -> Command {
    Command::UseProfile(key, name, ProfileApply(()))
}

fn button_text(index: usize, a: Action) -> String {
    let [b0, b1, b2, b3] = a.encode();
    format!("{index}:{b0:02X}{b1:02X}{b2:02X}{b3:02X}")
}

fn parse_button(s: &str) -> Option<(usize, Action)> {
    let (i, hex) = s.split_once(':')?;
    let index: usize = i.parse().ok()?;
    if hex.len() != 8 || !hex.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = u32::from_str_radix(hex, 16).ok()?;
    let a = Action::decode(v.to_be_bytes());
    (index >= 1 && onboard::assignable(a)).then_some((index, a))
}

impl MouseProfile {
    pub fn buttons(&self) -> Vec<(usize, Action)> {
        self.buttons
            .iter()
            .filter_map(|b| parse_button(b))
            .collect()
    }

    pub fn light(&self) -> Option<Light> {
        self.light.as_deref().and_then(parse_light)
    }

    /// The profile as read from the mouse: DPI only when the 4 entries of
    /// the cycle are equal (a factory cycle has no single start DPI).
    pub fn from_profile(
        name: &str,
        buttons: &[Action],
        light: Option<Light>,
        cycle: Option<[u16; 4]>,
    ) -> Self {
        Self {
            name: name.into(),
            dpi: cycle.filter(|c| c.iter().all(|d| *d == c[0])).map(|c| c[0]),
            light: light.filter(|l| l.writable()).map(light_text),
            buttons: buttons
                .iter()
                .enumerate()
                .skip(1)
                .filter(|(_, a)| onboard::assignable(**a))
                .map(|(i, a)| button_text(i, *a))
                .collect(),
        }
    }

    /// Same values, read from a fresh profile sector (light included).
    #[cfg(test)]
    pub fn from_sector(name: &str, user: &[u8], button_count: u8) -> Option<Self> {
        Some(Self::from_profile(
            name,
            &onboard::parse_buttons(user, button_count)?,
            onboard::parse_light(user),
            onboard::parse_dpi_cycle(user),
        ))
    }

    /// Takes the values the mouse holds now, keeping the name; a DPI that is
    /// not a single start DPI keeps the profile's.
    pub fn update_from(&mut self, read: Self) {
        self.dpi = read.dpi.or(self.dpi);
        self.light = read.light.or(self.light.take());
        self.buttons = read.buttons;
    }

    /// What has to change in the mouse (`buttons`, `light`, `cycle` as read
    /// from it) to hold this profile. Empty when it already does.
    pub fn edits(
        &self,
        buttons: &[Action],
        light: Option<Light>,
        cycle: Option<[u16; 4]>,
    ) -> ProfileEdits {
        ProfileEdits {
            dpi: self.dpi.filter(|d| cycle != Some([*d; 4])),
            light: self.light().filter(|l| light != Some(*l)),
            buttons: self
                .buttons()
                .into_iter()
                .filter(|(i, a)| buttons.get(*i) != Some(a))
                .collect(),
            rate: None,
        }
    }

    pub fn matches(
        &self,
        buttons: &[Action],
        light: Option<Light>,
        cycle: Option<[u16; 4]>,
    ) -> bool {
        self.edits(buttons, light, cycle) == ProfileEdits::default()
    }

    fn valid(&self) -> bool {
        name_ok(&self.name)
            && self.dpi.is_none_or(|d| (1..=MAX_DPI).contains(&d))
            && self
                .light
                .as_deref()
                .is_none_or(|l| parse_light(l).is_some())
            && self.buttons.iter().all(|b| parse_button(b).is_some())
    }
}

fn name_ok(name: &str) -> bool {
    let t = name.trim();
    !t.is_empty() && t == name && name.chars().count() <= MAX_NAME
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

/// Why a new name cannot be used, in the words of the dialog.
pub fn name_error(name: &str, others: &[MouseProfile], renaming: Option<&str>) -> Option<String> {
    let t = name.trim();
    if t.is_empty() {
        return Some("escribe un nombre.".into());
    }
    if t.chars().count() > MAX_NAME {
        return Some(format!("usa {MAX_NAME} caracteres como máximo."));
    }
    others
        .iter()
        .filter(|p| renaming.is_none_or(|r| p.name != r))
        .any(|p| same_name(&p.name, t))
        .then(|| format!("ya hay un perfil llamado «{t}». Elige otro nombre."))
}

/// A free name like "Juego 2" for a copy of `base`.
pub fn copy_name(base: &str, others: &[MouseProfile]) -> String {
    (2..)
        .map(|n| {
            let suffix = format!(" {n}");
            let keep = MAX_NAME - suffix.chars().count();
            let stem: String = base.chars().take(keep).collect();
            format!("{}{suffix}", stem.trim_end())
        })
        .find(|c| !others.iter().any(|p| same_name(&p.name, c)))
        .unwrap_or_else(|| FIRST_NAME.into())
}

/// Keeps valid, uniquely named profiles (at most 8) and an `active` that
/// names one of them. Returns how many were dropped. A hand-edited config can
/// hold anything; nothing dropped here is ever written to the mouse.
pub fn sanitize(profiles: &mut Vec<MouseProfile>, active: &mut Option<String>) -> usize {
    let before = profiles.len();
    let mut kept: Vec<MouseProfile> = Vec::new();
    for p in profiles.drain(..) {
        if kept.len() < MAX_PROFILES
            && p.valid()
            && !kept.iter().any(|k| same_name(&k.name, &p.name))
        {
            kept.push(p);
        }
    }
    let dropped = before - kept.len();
    *profiles = kept;
    if active
        .as_deref()
        .is_some_and(|a| !profiles.iter().any(|p| p.name == a))
    {
        *active = None;
    }
    if active.is_none() {
        *active = profiles.first().map(|p| p.name.clone());
    }
    dropped
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::onboard::samples;
    use crate::protocol::razer::Rgb;

    fn read() -> MouseProfile {
        MouseProfile::from_sector("Trabajo", &samples::user_profile(), 6).unwrap()
    }

    fn view() -> (Vec<Action>, Option<Light>, Option<[u16; 4]>) {
        let u = samples::user_profile();
        (
            onboard::parse_buttons(&u, 6).unwrap(),
            onboard::parse_light(&u),
            onboard::parse_dpi_cycle(&u),
        )
    }

    #[test]
    fn a_profile_read_from_the_mouse_matches_it() {
        let p = read();
        // The sample cycle has 4 different entries: no single start DPI.
        assert_eq!(p.dpi, None);
        // Index 0 (left) and the DPI button (90 0c) are not kept.
        assert_eq!(
            p.buttons,
            ["1:80010002", "2:80010004", "3:80020119", "4:80020106"]
        );
        let (b, l, c) = view();
        assert!(p.matches(&b, l, c));
        assert_eq!(p.edits(&b, l, c), ProfileEdits::default());
    }

    #[test]
    fn each_difference_is_an_edit() {
        let (b, l, c) = view();
        let red = Rgb { r: 255, g: 0, b: 0 };
        let mut p = read();
        p.dpi = Some(1600);
        p.light = Some("fija #FF0000".into());
        p.buttons[2] = "3:80010008".into();
        assert!(!p.matches(&b, l, c));
        assert_eq!(
            p.edits(&b, l, c),
            ProfileEdits {
                dpi: Some(1600),
                light: Some(Light::Fixed(red)),
                buttons: vec![(3, Action::Mouse(0x08))],
                rate: None,
            }
        );
        // Once the mouse holds it, nothing is left to write.
        assert!(!p.matches(&b, Some(Light::Fixed(red)), Some([1600; 4])));
        let mut b2 = b.clone();
        b2[3] = Action::Mouse(0x08);
        assert!(p.matches(&b2, Some(Light::Fixed(red)), Some([1600; 4])));
    }

    #[test]
    fn button_text_round_trip_and_rejects() {
        let a = Action::Keyboard {
            modifiers: 1,
            usage: 6,
        };
        assert_eq!(parse_button(&button_text(3, a)), Some((3, a)));
        for bad in [
            "0:80010001",
            "3:90050000",
            "3:8001",
            "x:80010002",
            "3:zz010002",
            "3",
        ] {
            assert_eq!(parse_button(bad), None, "{bad}");
        }
    }

    fn named(n: &str) -> MouseProfile {
        MouseProfile {
            name: n.into(),
            dpi: Some(800),
            light: None,
            buttons: vec![],
        }
    }

    #[test]
    fn sanitize_drops_what_could_not_be_written() {
        let mut bad_dpi = named("DPI");
        bad_dpi.dpi = Some(0);
        let mut bad_light = named("Luz");
        bad_light.light = Some("ciclo 5000".into());
        let mut bad_button = named("Botón");
        bad_button.buttons = vec!["0:80010002".into()];
        let mut list = vec![
            named("Trabajo"),
            named("trabajo"),
            named(""),
            named(" Juego"),
            named(&"x".repeat(33)),
            bad_dpi,
            bad_light,
            bad_button,
            named("Juego"),
        ];
        let mut active = Some("Nadie".to_string());
        assert_eq!(sanitize(&mut list, &mut active), 7);
        let names: Vec<_> = list.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(names, ["Trabajo", "Juego"]);
        assert_eq!(active.as_deref(), Some("Trabajo"));

        let mut many: Vec<_> = (1..=10).map(|i| named(&format!("P{i}"))).collect();
        let mut active = Some("P10".to_string());
        assert_eq!(sanitize(&mut many, &mut active), 2);
        assert_eq!(many.len(), MAX_PROFILES);
        assert_eq!(active.as_deref(), Some("P1"));
    }

    #[test]
    fn names() {
        let list = vec![named("Trabajo"), named("Juego")];
        assert_eq!(name_error("Diseño", &list, None), None);
        assert!(name_error("  ", &list, None).is_some());
        assert!(name_error(&"ñ".repeat(33), &list, None).is_some());
        assert_eq!(name_error(&"ñ".repeat(32), &list, None), None);
        assert!(
            name_error("JUEGO", &list, None)
                .unwrap()
                .contains("«JUEGO»")
        );
        // Renaming a profile to a new spelling of its own name is fine.
        assert_eq!(name_error("trabajo", &list, Some("Trabajo")), None);
        assert_eq!(copy_name("Juego", &list), "Juego 2");
        let long = "y".repeat(32);
        assert_eq!(copy_name(&long, &[]).chars().count(), 32);
    }

    #[test]
    fn update_keeps_the_start_dpi_when_the_cycle_has_none() {
        let mut p = named("Trabajo");
        p.update_from(read());
        assert_eq!(p.dpi, Some(800));
        assert_eq!(p.buttons, read().buttons);
    }
}
