//! Profiles per game (plan `perfiles-por-juego`, increment B). Pure code:
//! the program associations kept in the config, the game libraries and the
//! classifier, and the state machine that decides which profile a mouse
//! uses while programs open and close. No I/O here.
//!
//! Only programs that are installed in a game library (Steam, Xbox, Epic,
//! GOG) or that the user added by hand count; full-screen detection was
//! dropped by the user (2026-10-07). Program paths are only compared as
//! strings: nothing read here is ever run or opened.

use crate::profiles::MouseProfile;
use serde::{Deserialize, Serialize};

pub const MAX_PROGRAMS: usize = 64;
pub const MAX_PATH: usize = 520;
/// Library roots kept from external files (vdf, manifests, registry).
pub const MAX_ROOTS: usize = 32;
/// External files larger than this are not read.
pub const MAX_FILE: u64 = 1024 * 1024;

/// A program associated with a profile. `profile: None` = «No cambiar de
/// perfil» (also what «No preguntar más» saves).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Program {
    /// Full path, normalised with `normalize`.
    pub exe: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub profile: Option<String>,
}

/// Proof that a profile switch comes from a program the user associated
/// while «Cambiar de perfil al abrir estos programas» is on (CR-5b). Only
/// this module creates it, through `State`; the worker resolves the profile
/// from the validated config.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutoApply(());

/// Lower case, `\` separators, no `\\?\` prefix. `None` for anything that
/// is not an absolute local path (`C:\…`), has control characters, `..`
/// components or is too long. The key is the full path: moving the game
/// loses its association (decision of the user).
pub fn normalize(path: &str) -> Option<String> {
    let p = path.trim().replace('/', "\\");
    let p = p.strip_prefix("\\\\?\\").unwrap_or(&p);
    let b = p.as_bytes();
    let absolute = b.len() >= 3 && b[0].is_ascii_alphabetic() && b[1] == b':' && b[2] == b'\\';
    let bad = p.chars().any(char::is_control)
        || p.split('\\').any(|c| c == "..")
        || p.chars().count() > MAX_PATH;
    (absolute && !bad).then(|| p.to_lowercase())
}

/// "g:\…\peak\peak.exe" -> "PEAK.exe" style display name (file name only).
pub fn display_name(path: &str) -> String {
    path.rsplit(['\\', '/']).next().unwrap_or(path).to_string()
}

/// Keeps valid, unique programs (at most 64); an association to a profile
/// that does not exist becomes «No cambiar de perfil». Returns how many
/// entries changed or were dropped.
pub fn sanitize(programs: &mut Vec<Program>, profiles: &[MouseProfile]) -> usize {
    let mut changed = 0;
    let mut kept: Vec<Program> = Vec::new();
    for mut p in programs.drain(..) {
        let Some(exe) = normalize(&p.exe) else {
            changed += 1;
            continue;
        };
        if kept.len() >= MAX_PROGRAMS || kept.iter().any(|k| k.exe == exe) {
            changed += 1;
            continue;
        }
        if exe != p.exe {
            p.exe = exe;
        }
        if p.profile
            .as_deref()
            .is_some_and(|n| !profiles.iter().any(|q| q.name == n))
        {
            p.profile = None;
            changed += 1;
        }
        kept.push(p);
    }
    *programs = kept;
    changed
}

/// Folders whose programs count as games: each root ends with `\`, so
/// `steamapps\commonality` never matches `steamapps\common\`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Libraries {
    roots: Vec<String>,
}

impl Libraries {
    /// Library folders as found (Steam library folders, `X:\XboxGames`,
    /// Epic install locations, GOG paths); invalid ones are dropped.
    pub fn new(steam: &[String], other: &[String]) -> Self {
        let steam_roots = steam.iter().map(|s| format!("{s}\\steamapps\\common"));
        let mut roots: Vec<String> = Vec::new();
        for r in steam_roots.chain(other.iter().cloned()) {
            let Some(n) = normalize(&r) else { continue };
            let n = format!("{}\\", n.trim_end_matches('\\'));
            // A drive root would make every program a game.
            if n.len() > 3 && !roots.contains(&n) && roots.len() < MAX_ROOTS {
                roots.push(n);
            }
        }
        Self { roots }
    }

    /// `path` must be normalised.
    pub fn is_game(&self, path: &str) -> bool {
        self.roots.iter().any(|r| path.starts_with(r.as_str()))
    }
}

/// The `"path"` values of Steam's `libraryfolders.vdf`.
pub fn vdf_paths(text: &str) -> Vec<String> {
    text.lines()
        .filter_map(|l| {
            let rest = l.trim().strip_prefix("\"path\"")?;
            let v = rest.trim().strip_prefix('"')?.strip_suffix('"')?;
            Some(v.replace("\\\\", "\\"))
        })
        .take(MAX_ROOTS)
        .collect()
}

/// The `InstallLocation` of an Epic manifest (`*.item`, JSON), found by key
/// without a JSON parser: the value is a plain string with `\\` escapes.
pub fn epic_install(text: &str) -> Option<String> {
    let after = text.split_once("\"InstallLocation\"")?.1;
    let after = after.trim_start().strip_prefix(':')?.trim_start();
    let body = after.strip_prefix('"')?;
    let mut out = String::new();
    let mut chars = body.chars();
    while let Some(c) = chars.next() {
        match c {
            '"' => return Some(out),
            '\\' => match chars.next()? {
                '\\' => out.push('\\'),
                '/' => out.push('/'),
                '"' => out.push('"'),
                _ => return None,
            },
            c => out.push(c),
        }
    }
    None
}

/// What the program means for one mouse.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Assoc {
    Profile(String),
    /// «No cambiar de perfil».
    NoChange,
    /// Not associated: a game asks, anything else is ignored.
    Unknown {
        game: bool,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    None,
    /// Use the profile of the game that took over.
    SwitchTo(String, AutoApply),
    /// The last associated program closed: back to the profile before.
    Restore(String, AutoApply),
    /// A game without association: ask once for this opening.
    Ask(String),
}

/// Per mouse: associated programs open (pid, exe, profile), last opened
/// last, and the profile before the first one opened.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct State {
    open: Vec<(u32, String, String)>,
    previous: Option<String>,
    /// The user chose a profile by hand while a game was open: no more
    /// automatic switches until the games close (DESIGN.md).
    manual: bool,
    asked: Vec<u32>,
}

impl State {
    /// A program came to the foreground for the first time (`active` =
    /// profile in use now, `ask` and `auto` = the two switches).
    pub fn opened(
        &mut self,
        pid: u32,
        exe: &str,
        assoc: Assoc,
        active: Option<&str>,
        auto: bool,
        ask: bool,
    ) -> Action {
        match assoc {
            Assoc::Profile(name) if auto => {
                if self.open.iter().any(|(p, ..)| *p == pid) {
                    return Action::None;
                }
                if self.open.is_empty() {
                    self.previous = active.map(str::to_string);
                }
                self.open.push((pid, exe.to_string(), name.clone()));
                if self.manual || active == Some(name.as_str()) {
                    Action::None
                } else {
                    Action::SwitchTo(name, AutoApply(()))
                }
            }
            Assoc::Unknown { game: true } if ask && !self.asked.contains(&pid) => {
                self.asked.push(pid);
                Action::Ask(exe.to_string())
            }
            _ => Action::None,
        }
    }

    /// A program ended.
    pub fn closed(&mut self, pid: u32, active: Option<&str>) -> Action {
        self.asked.retain(|p| *p != pid);
        let Some(at) = self.open.iter().position(|(p, ..)| *p == pid) else {
            return Action::None;
        };
        let was_top = at + 1 == self.open.len();
        self.open.remove(at);
        if let Some((_, _, top)) = self.open.last() {
            // Another associated program is still open: it rules.
            if was_top && !self.manual && active != Some(top.as_str()) {
                return Action::SwitchTo(top.clone(), AutoApply(()));
            }
            return Action::None;
        }
        self.manual = false;
        match self.previous.take() {
            Some(p) if active != Some(p.as_str()) => Action::Restore(p, AutoApply(())),
            _ => Action::None,
        }
    }

    /// The user used a profile by hand: it stays until the games close.
    pub fn manual_use(&mut self) {
        if !self.open.is_empty() {
            self.manual = true;
        }
    }

    /// Display name of the game that rules now, if any.
    pub fn game(&self) -> Option<String> {
        self.open.last().map(|(_, exe, _)| display_name(exe))
    }

    /// Profile to go back to when the games close.
    pub fn previous(&self) -> Option<&str> {
        self.previous.as_deref()
    }

    /// Whether `pid` is an open associated program.
    pub fn is_open(&self, pid: u32) -> bool {
        self.open.iter().any(|(p, ..)| *p == pid)
    }

    /// A profile was renamed or deleted while games are open.
    pub fn profile_renamed(&mut self, old: &str, new: Option<&str>) {
        for (.., n) in &mut self.open {
            if n == old {
                *n = new.unwrap_or_default().to_string();
            }
        }
        self.open.retain(|(.., n)| !n.is_empty());
        if self.previous.as_deref() == Some(old) {
            self.previous = new.map(str::to_string);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn steam() -> Libraries {
        Libraries::new(
            &[
                "C:\\Program Files (x86)\\Steam".into(),
                "G:\\SteamLibrary".into(),
            ],
            &[
                "E:\\XboxGames".into(),
                "D:\\Juegos\\Epic\\Fortnite".into(),
                "C:\\".into(),
            ],
        )
    }

    #[test]
    fn normalize_paths() {
        assert_eq!(
            normalize("G:/SteamLibrary/steamapps/common/PEAK/PEAK.exe").as_deref(),
            Some("g:\\steamlibrary\\steamapps\\common\\peak\\peak.exe")
        );
        assert_eq!(
            normalize("\\\\?\\C:\\Juegos\\x.exe").as_deref(),
            Some("c:\\juegos\\x.exe")
        );
        for bad in [
            "",
            "x.exe",
            "\\\\server\\share\\x.exe",
            "C:\\a\\..\\x.exe",
            "C:\\a\u{7}.exe",
            &format!("C:\\{}", "a".repeat(600)),
        ] {
            assert_eq!(normalize(bad), None, "{bad}");
        }
    }

    #[test]
    fn classifier_compares_whole_folders() {
        let l = steam();
        let n = |p: &str| normalize(p).unwrap();
        assert!(l.is_game(&n("G:\\SteamLibrary\\steamapps\\common\\PEAK\\PEAK.exe")));
        assert!(l.is_game(&n("c:/program files (x86)/steam/STEAMAPPS/common/x/x.exe")));
        assert!(l.is_game(&n("E:\\XboxGames\\Halo\\Content\\halo.exe")));
        assert!(l.is_game(&n("D:\\Juegos\\Epic\\Fortnite\\bin\\f.exe")));
        assert!(!l.is_game(&n("G:\\SteamLibrary\\steamapps\\commonality\\x.exe")));
        assert!(!l.is_game(&n("C:\\Program Files (x86)\\Steam\\steam.exe")));
        assert!(!l.is_game(&n("C:\\Windows\\notepad.exe")));
        // A drive root is never a library.
        assert!(!l.is_game(&n("C:\\Juegos\\x.exe")));
    }

    #[test]
    fn steam_and_epic_files() {
        let vdf = "\"libraryfolders\"\n{\n\t\"0\"\n\t{\n\t\t\"path\"\t\t\"C:\\\\Program Files (x86)\\\\Steam\"\n\t\t\"label\"\t\t\"\"\n\t}\n\t\"1\"\n\t{\n\t\t\"path\"\t\t\"G:\\\\SteamLibrary\"\n";
        assert_eq!(
            vdf_paths(vdf),
            vec!["C:\\Program Files (x86)\\Steam", "G:\\SteamLibrary"]
        );
        assert!(vdf_paths("").is_empty());
        assert!(vdf_paths("\"path\" \"sin cierre").is_empty());

        let item = r#"{ "DisplayName": "Fortnite", "InstallLocation": "D:\\Juegos\\Epic\\Fortnite", "x": 1 }"#;
        assert_eq!(
            epic_install(item).as_deref(),
            Some("D:\\Juegos\\Epic\\Fortnite")
        );
        assert_eq!(epic_install(r#"{"InstallLocation": "C:\\a"#), None);
        assert_eq!(epic_install(r#"{"Other": 1}"#), None);
        assert_eq!(epic_install(""), None);
    }

    fn profile(name: &str) -> MouseProfile {
        MouseProfile {
            name: name.into(),
            dpi: None,
            light: None,
            buttons: vec![],
        }
    }

    #[test]
    fn sanitize_programs() {
        let profiles = vec![profile("Juego")];
        let p = |exe: &str, profile: Option<&str>| Program {
            exe: exe.into(),
            profile: profile.map(str::to_string),
        };
        let mut list = vec![
            p(
                "G:/SteamLibrary/steamapps/common/PEAK/PEAK.exe",
                Some("Juego"),
            ),
            p("g:\\steamlibrary\\steamapps\\common\\peak\\peak.exe", None),
            p("x.exe", None),
            p("C:\\a\\b.exe", Some("Borrado")),
        ];
        assert_eq!(sanitize(&mut list, &profiles), 3);
        assert_eq!(
            list,
            vec![
                p(
                    "g:\\steamlibrary\\steamapps\\common\\peak\\peak.exe",
                    Some("Juego")
                ),
                p("c:\\a\\b.exe", None),
            ]
        );
        let mut many: Vec<Program> = (0..70)
            .map(|i| p(&format!("C:\\p\\{i}.exe"), None))
            .collect();
        sanitize(&mut many, &profiles);
        assert_eq!(many.len(), MAX_PROGRAMS);
    }

    fn prof(n: &str) -> Assoc {
        Assoc::Profile(n.into())
    }

    fn switched(a: &Action) -> Option<&str> {
        match a {
            Action::SwitchTo(n, _) => Some(n),
            _ => None,
        }
    }

    #[test]
    fn open_and_close_one_game() {
        let mut s = State::default();
        let a = s.opened(
            10,
            "g:\\peak.exe",
            prof("Juego"),
            Some("Trabajo"),
            true,
            true,
        );
        assert_eq!(switched(&a), Some("Juego"));
        assert_eq!(s.game().as_deref(), Some("peak.exe"));
        // The same process taking the focus again does nothing.
        assert_eq!(
            s.opened(10, "g:\\peak.exe", prof("Juego"), Some("Juego"), true, true),
            Action::None
        );
        assert!(matches!(s.closed(10, Some("Juego")), Action::Restore(ref p, _) if p == "Trabajo"));
        assert_eq!(s.game(), None);
        // Unknown pids close without effect.
        assert_eq!(s.closed(99, Some("Trabajo")), Action::None);
    }

    #[test]
    fn the_last_game_opened_rules() {
        let mut s = State::default();
        s.opened(1, "a.exe", prof("A"), Some("Trabajo"), true, true);
        let b = s.opened(2, "b.exe", prof("B"), Some("A"), true, true);
        assert_eq!(switched(&b), Some("B"));
        // Closing the first one leaves B.
        assert_eq!(s.closed(1, Some("B")), Action::None);
        s.opened(1, "a.exe", prof("A"), Some("B"), true, true);
        // Closing the top one goes back to the other open game.
        assert_eq!(switched(&s.closed(1, Some("A"))), Some("B"));
        assert!(matches!(s.closed(2, Some("B")), Action::Restore(ref p, _) if p == "Trabajo"));
    }

    #[test]
    fn a_manual_change_lasts_until_the_games_close() {
        let mut s = State::default();
        s.opened(1, "a.exe", prof("A"), Some("Trabajo"), true, true);
        s.manual_use();
        assert_eq!(
            s.opened(2, "b.exe", prof("B"), Some("Mío"), true, true),
            Action::None
        );
        assert_eq!(s.closed(2, Some("Mío")), Action::None);
        assert!(matches!(s.closed(1, Some("Mío")), Action::Restore(ref p, _) if p == "Trabajo"));
        // A manual change with no game open is not remembered.
        s.manual_use();
        let a = s.opened(1, "a.exe", prof("A"), Some("Trabajo"), true, true);
        assert_eq!(switched(&a), Some("A"));
    }

    #[test]
    fn asking_and_switches() {
        let mut s = State::default();
        let game = Assoc::Unknown { game: true };
        assert_eq!(
            s.opened(5, "peak.exe", game.clone(), None, true, true),
            Action::Ask("peak.exe".into())
        );
        // Once per opening, even if the focus comes back.
        assert_eq!(
            s.opened(5, "peak.exe", game.clone(), None, true, true),
            Action::None
        );
        s.closed(5, None);
        assert!(matches!(
            s.opened(5, "peak.exe", game.clone(), None, true, true),
            Action::Ask(_)
        ));
        // Not a game, «No cambiar», asking off, switching off: nothing.
        assert_eq!(
            s.opened(6, "x.exe", Assoc::Unknown { game: false }, None, true, true),
            Action::None
        );
        assert_eq!(
            s.opened(7, "y.exe", Assoc::NoChange, None, true, true),
            Action::None
        );
        assert_eq!(s.opened(8, "z.exe", game, None, true, false), Action::None);
        assert_eq!(
            s.opened(9, "a.exe", prof("A"), Some("T"), false, true),
            Action::None
        );
        // Already using the game's profile: nothing to switch.
        assert_eq!(
            s.opened(10, "a.exe", prof("A"), Some("A"), true, true),
            Action::None
        );
    }

    #[test]
    fn deleted_profiles_while_playing() {
        let mut s = State::default();
        s.opened(1, "a.exe", prof("A"), Some("Trabajo"), true, true);
        s.profile_renamed("Trabajo", Some("Oficina"));
        assert_eq!(s.previous(), Some("Oficina"));
        s.profile_renamed("A", None);
        assert!(!s.is_open(1));
    }
}
