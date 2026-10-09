//! Texts of the «Perfiles» section and of the tray submenu (DESIGN.md
//! «Perfiles», «Perfiles en la bandeja»). Pure functions; the controls live
//! in `mouse_settings.rs` with the rest of the page.

use super::light;
use super::text;
use crate::app::{Profile, Snapshot, Status};
use crate::profiles::MouseProfile;
use crate::protocol::onboard::Action;

/// Default mouse action of each button index (left, right, middle, back,
/// forward): changes from these are listed in the summary.
const STANDARD: [Action; 5] = [
    Action::Mouse(0x01),
    Action::Mouse(0x02),
    Action::Mouse(0x04),
    Action::Mouse(0x08),
    Action::Mouse(0x10),
];

/// "1600 DPI · Luz respiración, Rojo #FF0000, lenta · Botones: Lateral
/// trasero → Ctrl + C".
pub fn summary(p: &MouseProfile) -> String {
    let mut parts = Vec::new();
    if let Some(d) = p.dpi {
        parts.push(format!("{d} DPI"));
    }
    if let Some(l) = p.light() {
        parts.push(format!("Luz {}", light::describe(l)));
    }
    let changed: Vec<String> = p
        .buttons()
        .into_iter()
        .filter(|(i, a)| STANDARD.get(*i) != Some(a))
        .map(|(i, a)| {
            let act = match a {
                Action::Keyboard { modifiers, usage } => text::shortcut(modifiers, usage),
                other => text::action(other),
            };
            format!("{} → {act}", text::mouse_button(i))
        })
        .collect();
    parts.push(if changed.is_empty() {
        "Botones: estándar".into()
    } else {
        format!("Botones: {}", changed.join(", "))
    });
    parts.join(" · ")
}

/// Whether the mouse (as last read) holds `p`: the profile view must have
/// been read; `None` while it is unknown.
pub fn holds(snap: &Snapshot, p: &MouseProfile) -> Option<bool> {
    match &snap.profile {
        Profile::Read(v) => Some(p.matches(&v.buttons, v.light, v.dpi_cycle)),
        _ => None,
    }
}

/// "Activo · en el ratón", the mismatch wording, or, while a game rules,
/// "Activo · por juego" saying whether only the DPI and light are in use
/// (hybrid switch: the mouse's memory was not written).
pub fn active_label(holds: Option<bool>, game: bool) -> &'static str {
    match (game, holds) {
        (true, Some(false)) => "Activo · por juego · DPI y luz en uso, sin guardar en el ratón",
        (true, _) => "Activo · por juego",
        (false, Some(false)) => "Activo · el ratón no coincide",
        (false, _) => "Activo · en el ratón",
    }
}

/// "PEAK.exe está abierto: se usa el perfil «Juego». Al cerrarlo, el ratón
/// vuelve a «Trabajo»."
pub fn game_notice(snap: &Snapshot) -> Option<String> {
    let game = snap.game.as_deref()?;
    let active = snap.active_profile.as_deref().unwrap_or("");
    let mut t = format!("{game} está abierto: se usa el perfil «{active}».");
    if let Some(prev) = snap.game_previous.as_deref() {
        t.push_str(&format!(" Al cerrarlo, el ratón vuelve a «{prev}»."));
    }
    Some(t)
}

/// "El ratón tiene ajustes distintos de «Trabajo»: DPI al arrancar 3200 (en
/// el perfil: 1000)."
pub fn mismatch(snap: &Snapshot, p: &MouseProfile) -> Option<String> {
    let Profile::Read(v) = &snap.profile else {
        return None;
    };
    let e = p.edits(&v.buttons, v.light, v.dpi_cycle);
    // While a game rules, its DPI and light are in use live on purpose.
    if e == Default::default() || (snap.game.is_some() && e.buttons.is_empty()) {
        return None;
    }
    let mut what = Vec::new();
    if let Some(d) = e.dpi {
        let now = v
            .dpi_cycle
            .filter(|c| c.iter().all(|x| *x == c[0]))
            .map(|c| c[0].to_string())
            .unwrap_or_else(|| "variable".into());
        what.push(format!("DPI al arrancar {now} (en el perfil: {d})"));
    }
    if let Some(l) = e.light {
        let now = v
            .light
            .map(light::describe)
            .unwrap_or_else(|| "desconocida".into());
        what.push(format!("luz {now} (en el perfil: {})", light::describe(l)));
    }
    if !e.buttons.is_empty() {
        let names: Vec<String> = e
            .buttons
            .iter()
            .map(|(i, _)| text::mouse_button(*i))
            .collect();
        what.push(format!("botones {}", names.join(", ")));
    }
    Some(format!(
        "El ratón tiene ajustes distintos de «{}»: {}. Puede que otro programa los haya cambiado.",
        p.name,
        what.join("; ")
    ))
}

/// Why the profiles cannot be used now, if they cannot.
pub fn blocked(snap: &Snapshot, ghub: bool) -> Option<&'static str> {
    if !matches!(snap.status, Status::Ok(_)) {
        Some(
            "Conecta o enciende el ratón para cambiar de perfil. Los cambios sin guardar se conservan.",
        )
    } else if ghub {
        Some(
            "Cierra G HUB para cambiar de perfil. Puedes crear, renombrar y eliminar perfiles igualmente.",
        )
    } else {
        None
    }
}

pub fn first_time(n: usize) -> &'static str {
    if n == 1 {
        "Crea otro perfil para cambiar rápido entre configuraciones, por ejemplo una para trabajar y otra para jugar."
    } else {
        ""
    }
}

/// Tray: «&» shows as an access key in a menu, so it is doubled.
fn menu_text(s: &str) -> String {
    s.replace('&', "&&")
}

/// One submenu per mouse with at least 2 profiles: its title, the reason
/// it is blocked (first line, greyed) and the items (name, active).
pub struct TrayMenu {
    pub title: String,
    pub blocked: Option<&'static str>,
    pub items: Vec<(String, bool)>,
}

pub fn tray_menu(snap: &Snapshot, ghub: bool) -> Option<TrayMenu> {
    if snap.profiles.len() < 2 {
        return None;
    }
    let active = snap.active_profile.as_deref();
    Some(TrayMenu {
        title: menu_text(&format!(
            "Perfil de {}: {}",
            short(&snap.info.name),
            active.unwrap_or("ninguno")
        )),
        blocked: if !matches!(snap.status, Status::Ok(_)) {
            Some("Conecta o enciende el ratón para cambiar de perfil")
        } else if ghub {
            Some("Cierra G HUB para cambiar de perfil")
        } else {
            None
        },
        items: snap
            .profiles
            .iter()
            .map(|p| (menu_text(&p.name), Some(p.name.as_str()) == active))
            .collect(),
    })
}

/// "G305 Lightspeed Wireless Gaming Mouse" -> "G305".
pub fn short(name: &str) -> String {
    name.split_whitespace().next().unwrap_or(name).to_string()
}

/// " · perfil Trabajo" for the tray tooltip (only with 2 or more profiles).
pub fn tooltip_suffix(snap: &Snapshot) -> String {
    match (&snap.active_profile, snap.profiles.len()) {
        (Some(a), _) if snap.game.is_some() => format!(" · perfil {a} (por juego)"),
        (Some(a), n) if n >= 2 => format!(" · perfil {a}"),
        _ => String::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::{DeviceInfo, DeviceKey, Kind, State};

    fn profile(name: &str) -> MouseProfile {
        MouseProfile {
            name: name.into(),
            dpi: Some(1600),
            light: Some("respiracion #FF0000 6000".into()),
            buttons: vec![
                "1:80010002".into(),
                "3:80020106".into(),
                "4:80010010".into(),
            ],
        }
    }

    fn snap(profiles: Vec<MouseProfile>, active: &str) -> Snapshot {
        Snapshot {
            info: DeviceInfo {
                key: DeviceKey("k".into()),
                vendor: "Logitech",
                name: "G305 Lightspeed Wireless Gaming Mouse".into(),
                kind: Kind::Mouse,
                connection: "Receptor",
                wired: false,
                support: crate::devices::Support::Verified,
            },
            status: Status::Ok(State::default()),
            last_battery: None,
            saved: true,
            saved_dpi: None,
            saved_light: None,
            applied_color: None,
            original_dpi: None,
            profile: Profile::NotRead,
            backup: None,
            profiles,
            active_profile: Some(active.into()),
            programs: Vec::new(),
            game: None,
            game_previous: None,
            unanswered: Vec::new(),
        }
    }

    #[test]
    fn summary_names_only_what_differs_from_the_standard_buttons() {
        assert_eq!(
            summary(&profile("Juego")),
            "1600 DPI · Luz respiración, Rojo #FF0000, lenta · Botones: Lateral trasero → Ctrl + C"
        );
        let mut plain = profile("Trabajo");
        plain.dpi = None;
        plain.light = None;
        plain.buttons = vec!["1:80010002".into()];
        assert_eq!(summary(&plain), "Botones: estándar");
    }

    #[test]
    fn tray_menu_needs_two_profiles_and_escapes_ampersands() {
        assert!(tray_menu(&snap(vec![profile("Solo")], "Solo"), false).is_none());
        let s = snap(vec![profile("Juego & más"), profile("Trabajo")], "Trabajo");
        let m = tray_menu(&s, false).unwrap();
        assert_eq!(m.title, "Perfil de G305: Trabajo");
        assert_eq!(
            m.items,
            vec![
                ("Juego && más".to_string(), false),
                ("Trabajo".to_string(), true)
            ]
        );
        assert_eq!(m.blocked, None);
        assert_eq!(
            tray_menu(&s, true).unwrap().blocked,
            Some("Cierra G HUB para cambiar de perfil")
        );
        let mut gone = s.clone();
        gone.status = Status::Disconnected;
        assert!(
            tray_menu(&gone, false)
                .unwrap()
                .blocked
                .unwrap()
                .starts_with("Conecta")
        );
        assert_eq!(tooltip_suffix(&s), " · perfil Trabajo");
        assert_eq!(tooltip_suffix(&snap(vec![profile("Solo")], "Solo")), "");
    }

    #[test]
    fn game_texts() {
        let mut s = snap(vec![profile("Juego"), profile("Trabajo")], "Juego");
        assert_eq!(game_notice(&s), None);
        s.game = Some("PEAK.exe".into());
        s.game_previous = Some("Trabajo".into());
        assert_eq!(
            game_notice(&s).unwrap(),
            "PEAK.exe está abierto: se usa el perfil «Juego». Al cerrarlo, el ratón vuelve a «Trabajo»."
        );
        assert_eq!(tooltip_suffix(&s), " · perfil Juego (por juego)");
    }

    #[test]
    fn mismatch_describes_what_another_program_changed() {
        use crate::app::ProfileView;
        let mut s = snap(vec![profile("Juego")], "Juego");
        let p = profile("Juego");
        assert_eq!(mismatch(&s, &p), None);
        assert_eq!(holds(&s, &p), None);
        s.profile = Profile::Read(ProfileView {
            buttons: vec![
                Action::Mouse(1),
                Action::Mouse(2),
                Action::Mouse(4),
                Action::Keyboard {
                    modifiers: 1,
                    usage: 6,
                },
                Action::Mouse(0x10),
            ],
            light: p.light(),
            dpi_cycle: Some([3200; 4]),
            effects: vec![],
        });
        assert_eq!(holds(&s, &p), Some(false));
        assert_eq!(
            mismatch(&s, &p).unwrap(),
            "El ratón tiene ajustes distintos de «Juego»: DPI al arrancar 3200 (en el perfil: 1600). Puede que otro programa los haya cambiado."
        );
        assert_eq!(
            active_label(Some(false), false),
            "Activo · el ratón no coincide"
        );
        // While a game rules, only buttons count as a mismatch.
        s.game = Some("PEAK.exe".into());
        assert_eq!(mismatch(&s, &p), None);
        assert_eq!(
            active_label(Some(false), true),
            "Activo · por juego · DPI y luz en uso, sin guardar en el ratón"
        );
        assert_eq!(active_label(Some(true), true), "Activo · por juego");
    }
}
