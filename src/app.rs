//! Background worker: owns all HID I/O so the UI thread never blocks.
//!
//! The UI sends [`Command`]s; the worker updates [`Shared`] and calls
//! `notify` so the UI can redraw. Between commands it only wakes up every
//! [`POLL`] to refresh battery levels.

use crate::config::{Config, parse_hex};
use crate::devices::{
    Change, Device, DeviceInfo, DeviceKey, Error, ProfileDump, State, Support, Transport,
};
use crate::games::{self, Assoc, AutoApply, Program};
use crate::platform::hid::{HidTransport, listed, receivers, scan};
use crate::profiles::{self, MouseProfile, ProfileApply};
use crate::protocol::Battery;
use crate::protocol::hidpp::{Link, parse_connection};
use crate::protocol::onboard::{self, Action, EditError, ProfileEdits, ProfileImage};
use crate::protocol::razer::Rgb;
use std::collections::{HashMap, HashSet};
use std::ffi::CString;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

pub const POLL: Duration = Duration::from_secs(60);
/// Device arrival fires several notifications; wait for them to settle.
const SETTLE: Duration = Duration::from_millis(800);

/// Write gate of plan `ajustes-raton` (increment D). While false, no path
/// sends a memory write to a real device. Switched on after the
/// identical-rewrite and minimal-change tests passed on a G305 (2026-10-07,
/// `docs/protocolos/logitech-hidpp.md`). Set it to false to make every
/// mouse's buttons read only again.
pub const WRITES_ENABLED: bool = true;

/// Proof that the user confirmed a write in the confirmation dialog. Only
/// the dialog creates it (`Confirmed::by_dialog`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Confirmed(());

impl Confirmed {
    pub fn by_dialog() -> Self {
        Confirmed(())
    }
}

/// What to write back to the mouse's buttons.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Source {
    /// The copy taken by this app before any change.
    Original,
    /// DPI cycle, light and button entries of the factory (ROM) profile.
    Factory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Rescan,
    Refresh,
    /// Sends a change without recording it ("Probar").
    Try(DeviceKey, Change),
    Restore(DeviceKey),
    /// Reads the onboard profile of a mouse (read only).
    ReadProfile(DeviceKey),
    /// Puts a mouse in host mode back on its onboard profiles.
    ActivateOnboard(DeviceKey, Confirmed),
    /// Saves DPI, light and button changes in the mouse's memory (one write).
    SaveProfile(DeviceKey, ProfileEdits, Confirmed),
    RestoreProfile(DeviceKey, Source, Confirmed),
    /// Writes a saved profile into the mouse ("Usar", no dialog). Only
    /// `profiles::use_command` creates it.
    UseProfile(DeviceKey, String, ProfileApply),
    /// Profile list edits; none of them writes to the mouse.
    NewProfile(DeviceKey, String),
    /// Saves what the mouse holds now as a new profile and makes it active.
    SaveAsProfile(DeviceKey, String),
    RenameProfile(DeviceKey, String, String),
    DeleteProfile(DeviceKey, String),
    /// A program came to the foreground for the first time (pid, path).
    ProcessOpened(u32, String),
    ProcessClosed(u32),
    /// Associates a program (full path) with a profile; `None` = «No cambiar
    /// de perfil» (also «No preguntar más»).
    SetProgram(DeviceKey, String, Option<String>),
    /// A new game or an added program gets its own profile (a copy of the
    /// active one, named by the user, by default after the program) and is
    /// associated with it (decision of the user, 2026-10-07).
    CreateGameProfile(DeviceKey, String, String),
    RemoveProgram(DeviceKey, String),
    /// The app-wide settings of «Ajustes» (saved at once).
    SetGeneral(crate::config::General),
    /// The settings page has button changes not saved yet: automatic
    /// switches must not write over them.
    PageEdits(DeviceKey, bool),
    /// A receiver reported that one of its devices is back (HID++ 0x41).
    Reconnected(Slot),
    /// A mouse photo was downloaded and saved (`photos`).
    PhotoReady,
    Quit,
}

/// A device behind a wireless receiver: receiver VID/PID and device index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Slot {
    pub vid: u16,
    pub pid: u16,
    pub index: u8,
}

/// Time the G305 needed after its connection notification to answer
/// (observed: 1.57 s); the saved settings are sent after this.
/// How often unlisted Logitech paths that answered nothing are probed again.
const SKIP_RETRY: Duration = Duration::from_secs(300);
const RECONNECT_SETTLE: Duration = Duration::from_millis(1500);
/// Pause before the single retry of a failed re-apply.
const RECONNECT_RETRY: Duration = Duration::from_secs(1);
/// Wait before an automatic switch writes the buttons (plan
/// `perfiles-por-juego`, D5): a program that closes at once costs nothing.
const AUTO_WRITE_DELAY: Duration = Duration::from_secs(5);

/// Reads a receiver's notifications from `t` (its short HID++ collection,
/// opened only for reading) and calls `up` once per device whose link came
/// up, `settle` after the notification; notifications arriving meanwhile are
/// merged. Returns when reading fails (receiver unplugged).
pub fn listen(t: &dyn Transport, settle: Duration, mut up: impl FnMut(u8)) {
    let mut buf = [0u8; 64];
    let up_index = |buf: &[u8]| match parse_connection(buf) {
        Some((index, Link::Up)) => Some(index),
        _ => None,
    };
    loop {
        let n = match t.read_timeout(&mut buf, 1000) {
            Ok(n) => n,
            Err(_) => return,
        };
        let Some(first) = up_index(&buf[..n]) else {
            continue;
        };
        sleep(settle);
        let mut indexes = vec![first];
        // Drain what arrived while waiting (bursts, other devices).
        let mut gone = false;
        loop {
            match t.read_timeout(&mut buf, 0) {
                Ok(0) => break,
                Ok(n) => {
                    if let Some(i) = up_index(&buf[..n]) {
                        if !indexes.contains(&i) {
                            indexes.push(i);
                        }
                    }
                }
                Err(_) => {
                    gone = true;
                    break;
                }
            }
        }
        indexes.into_iter().for_each(&mut up);
        if gone {
            return;
        }
    }
}

/// Entries a reconnection notification is about: present devices behind
/// that receiver at that index.
fn targets(entries: &[Entry], slot: Slot) -> Vec<usize> {
    let prefix = format!("{:04x}:{:04x}/", slot.vid, slot.pid);
    entries
        .iter()
        .enumerate()
        .filter(|(_, e)| {
            e.snapshot.info.key.0.starts_with(&prefix)
                && e.device
                    .as_ref()
                    .is_some_and(|d| d.receiver_index() == Some(slot.index))
        })
        .map(|(i, _)| i)
        .collect()
}

/// What the UI knows about a mouse's onboard profile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    NotRead,
    /// No onboard profiles, or a format this app has not verified.
    Unsupported,
    /// Another program put the mouse in host mode: its profile is not in use.
    HostMode,
    Failed(String),
    Read(ProfileView),
}

/// The decoded parts of a profile the UI shows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileView {
    pub buttons: Vec<Action>,
    /// Light stored in the profile; `None` if its effect is not known.
    pub light: Option<onboard::Light>,
    pub dpi_cycle: Option<[u16; 4]>,
    /// Light effect ids the mouse declares (0x8070); empty without light.
    pub effects: Vec<u16>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Status {
    Searching,
    Ok(State),
    /// Present but not answering (asleep, off, or busy).
    Failed(Error),
    /// Seen earlier in this session, not present now.
    Disconnected,
}

#[derive(Debug, Clone)]
pub struct Snapshot {
    pub info: DeviceInfo,
    pub status: Status,
    /// Last battery reading, kept when the device stops answering.
    pub last_battery: Option<Battery>,
    pub saved: bool,
    /// DPI and light the app applies when the mouse connects.
    pub saved_dpi: Option<u16>,
    pub saved_light: Option<onboard::Light>,
    /// Last colour this app applied (devices cannot report theirs).
    pub applied_color: Option<Rgb>,
    /// DPI read when the device was first seen ("Restaurar originales").
    pub original_dpi: Option<u16>,
    pub profile: Profile,
    /// Original profile copy on this PC: `Ok(date)` or the reason it failed.
    pub backup: Option<std::result::Result<String, String>>,
    /// Profiles saved on this PC for this model, and the active one.
    pub profiles: Vec<MouseProfile>,
    pub active_profile: Option<String>,
    /// Programs associated with a profile (the switches are in `general`).
    pub programs: Vec<Program>,
    /// The game whose profile is in use (display name) and the profile the
    /// mouse goes back to when it closes.
    pub game: Option<String>,
    pub game_previous: Option<String>,
    /// Games detected this session and not answered yet (full paths).
    pub unanswered: Vec<String>,
}

impl Snapshot {
    /// "Guardar" keeps DPI and light in the app, not in the mouse: the
    /// mouse is not verified, or its onboard profile is not in use.
    pub fn saves_in_app(&self) -> bool {
        self.info.support == Support::Auto || self.profile == Profile::HostMode
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    pub ok: bool,
    pub text: String,
}

#[derive(Debug, Clone, Default)]
pub struct Shared {
    pub devices: Vec<Snapshot>,
    pub message: Option<Message>,
    /// Incremented with every new message, so the UI knows a request ended.
    pub seq: u64,
    /// Incremented only when a profile write ends, with its result, so other
    /// messages (e.g. a rescan) are never taken for the write's outcome.
    pub writes: u64,
    pub write_result: Option<Message>,
    /// Names of other peripheral software currently running.
    pub conflicts: Vec<&'static str>,
    pub scanned: bool,
    /// False when the HID subsystem could not be opened at all.
    pub hid_available: bool,
    /// A new game to ask about: incremented with each, with (mouse, path).
    pub asks: u64,
    pub ask: Option<(DeviceKey, String)>,
    /// Outcome of the last automatic switch that failed or was not written,
    /// incremented with each (shown in the page and as a notification).
    pub autos: u64,
    pub auto_result: Option<Message>,
    /// «Ajustes» (app-wide), from the config.
    pub general: crate::config::General,
    /// Incremented when the cached photos change (one downloaded, all
    /// deleted), so the mouse page loads its photo again.
    pub photos: u64,
}

struct Entry {
    device: Option<Box<dyn Device>>,
    snapshot: Snapshot,
    /// DPI read the first time the device was seen, before re-applying.
    original_dpi: Option<u16>,
    /// The first read happened: `original_dpi` is final. A mouse that
    /// cannot report its DPI (SteelSeries) keeps `None`, never the one
    /// this app set later.
    original_read: bool,
    /// Last onboard-profile read (raw sectors stay inside the worker).
    profile: Option<std::result::Result<crate::devices::ProfileDump, Error>>,
    backup: Option<std::result::Result<String, String>>,
    /// A re-apply after a reconnection failed: the next rescan retries it.
    reapply_due: bool,
}

impl Entry {
    fn profile_view(&self) -> Profile {
        match &self.profile {
            None => Profile::NotRead,
            Some(Err(Error::Unsupported)) => Profile::Unsupported,
            Some(Err(Error::HostMode)) => Profile::HostMode,
            Some(Err(e)) => Profile::Failed(e.to_string()),
            Some(Ok(d)) => match onboard::parse_buttons(&d.user, d.info.button_count) {
                Some(buttons) => Profile::Read(ProfileView {
                    buttons,
                    light: profile_light(d),
                    dpi_cycle: onboard::parse_dpi_cycle(&d.user),
                    effects: d.effects.clone(),
                }),
                None => Profile::Failed("perfil incompleto".into()),
            },
        }
    }
}

type Build = fn(&ProfileDump, &ProfileDump) -> std::result::Result<ProfileImage, EditError>;

fn edit_error(e: EditError) -> String {
    match e {
        EditError::LeftButton => "el botón izquierdo no se puede cambiar.",
        EditError::NoSuchButton => "ese botón no existe en este ratón.",
        EditError::NotAssignable => "esa acción no se puede asignar.",
        EditError::BadDpi => "ese DPI no lo admite el ratón.",
        EditError::BadLight => "esa luz no la admite el ratón.",
        EditError::BadProfile => "el perfil del ratón no es el esperado; no se escribe nada.",
    }
    .into()
}

/// Builds the image from a fresh read (`build(current, saved)`) and writes
/// it. If the write or its read-back fails, writes back the bytes read
/// before, so a failed save leaves the mouse as it was.
fn write_checked(
    dev: &mut dyn Device,
    t: &dyn Transport,
    saved: &ProfileDump,
    build: impl FnOnce(&ProfileDump, &ProfileDump) -> std::result::Result<ProfileImage, EditError>,
) -> std::result::Result<(), String> {
    let before = dev
        .read_profile(t)
        .map_err(|e| format!("no se pudo leer el perfil ({e})."))?;
    let image = build(&before, saved).map_err(edit_error)?;
    let Err(err) = dev.write_profile(t, &image) else {
        return Ok(());
    };
    let undo = onboard::restore_image(
        &before.user,
        &before.user,
        before.sector,
        &before.directory,
        &before.limits(),
    )
    .map_err(edit_error)
    .and_then(|img| dev.write_profile(t, &img).map_err(|e| e.to_string()));
    Err(match undo {
        Ok(()) => format!("no se pudo guardar ({err}); el perfil se dejó como estaba."),
        Err(e2) => format!(
            "no se pudo guardar ({err}) ni volver al perfil anterior ({e2}). Usa «Restaurar perfil original»."
        ),
    })
}

/// Drops queued `Rescan`s (one rescan covers them all) and returns the first
/// other command, which must not be lost.
fn skip_rescans(rx: &Receiver<Command>) -> Option<Command> {
    loop {
        match rx.try_recv() {
            Ok(Command::Rescan) => continue,
            Ok(other) => return Some(other),
            Err(_) => return None,
        }
    }
}

/// Drains the queue: a later `UseProfile` for the same mouse replaces `name`;
/// every other command is returned in order.
fn latest_use(rx: &Receiver<Command>, key: &DeviceKey, mut name: String) -> (String, Vec<Command>) {
    let mut others = Vec::new();
    while let Ok(c) = rx.try_recv() {
        match c {
            Command::UseProfile(k, n, _) if &k == key => name = n,
            other => others.push(other),
        }
    }
    (name, others)
}

/// Light stored in the profile, only where the app can edit it (the mouse
/// declares effects): elsewhere its light block is neither shown nor written.
fn profile_light(d: &ProfileDump) -> Option<onboard::Light> {
    if d.effects.is_empty() {
        None
    } else {
        onboard::parse_light(&d.user)
    }
}

/// What has to change in the mouse (read as `d`) to hold the profile.
fn edits_for(p: &MouseProfile, d: &ProfileDump) -> Option<ProfileEdits> {
    let buttons = onboard::parse_buttons(&d.user, d.info.button_count)?;
    let mut edits = p.edits(
        &buttons,
        profile_light(d),
        onboard::parse_dpi_cycle(&d.user),
    );
    if d.effects.is_empty() {
        edits.light = None;
    }
    Some(edits)
}

/// Whether the mouse (read as `d`) already holds the profile.
fn holds(p: &MouseProfile, d: &ProfileDump) -> bool {
    edits_for(p, d).is_some_and(|e| e == ProfileEdits::default())
}

/// Hybrid switch: the mouse's memory is written only when the profile's
/// buttons differ from the mouse's (DPI and light go live).
fn buttons_differ(p: &MouseProfile, d: &ProfileDump) -> bool {
    edits_for(p, d).is_some_and(|e| !e.buttons.is_empty())
}

/// Image that makes the mouse (read as `d`) hold the profile; the profile
/// is checked again against what the mouse accepts (`apply_edits`).
fn profile_image(
    p: &MouseProfile,
    d: &ProfileDump,
) -> std::result::Result<ProfileImage, EditError> {
    let edits = edits_for(p, d).ok_or(EditError::BadProfile)?;
    onboard::apply_edits(&d.user, d.sector, &d.directory, &d.limits(), &edits)
}

/// Keys are "vvvv:pppp/name": same USB device.
fn same_vid_pid(a: &DeviceKey, b: &DeviceKey) -> bool {
    a.0.split('/').next() == b.0.split('/').next()
}

/// "Perfil 1": the buttons the mouse holds, with the DPI and light the app
/// already applies on connect (what the user has been using). Otherwise
/// using "Perfil 1" later would drop them (`DeviceSettings::activate`).
fn first_profile(mut read: MouseProfile, s: &crate::config::DeviceSettings) -> MouseProfile {
    read.dpi = s.dpi.or(read.dpi);
    read.light = s.light.clone().or(read.light);
    read
}

/// Adds `profile` under `name` (checked); returns the name used.
fn add_profile(
    s: &mut crate::config::DeviceSettings,
    name: &str,
    mut profile: MouseProfile,
) -> std::result::Result<String, String> {
    let name = name.trim().to_string();
    if s.profiles.len() >= profiles::MAX_PROFILES {
        return Err(format!(
            "ya hay {} perfiles. Elimina uno para crear otro.",
            profiles::MAX_PROFILES
        ));
    }
    if let Some(e) = profiles::name_error(&name, &s.profiles, None) {
        return Err(e);
    }
    profile.name = name.clone();
    s.profiles.push(profile);
    Ok(name)
}

pub struct Worker {
    api: Option<hidapi::HidApi>,
    entries: Vec<Entry>,
    config: Config,
    shared: Arc<Mutex<Shared>>,
    notify: Box<dyn Fn() + Send>,
    conflicts: fn() -> Vec<&'static str>,
    /// Invalid profiles dropped from the config at start, reported once.
    dropped: usize,
    /// Profiles per game: library folders, state per mouse key, programs
    /// running (pid, normalised path), games not answered this session,
    /// writes waiting 5 s (per mouse key: when, profile, program) and mice
    /// whose page has unsaved button changes.
    libraries: games::Libraries,
    games: HashMap<String, games::State>,
    running: Vec<(u32, String)>,
    unanswered: Vec<(DeviceKey, String)>,
    pending: HashMap<String, (std::time::Instant, String, Option<u32>)>,
    page_edits: Vec<DeviceKey>,
    /// Sends `Reconnected` back to this worker from the listener threads.
    tx: Sender<Command>,
    /// One listener per receiver path, alive while its flag is set.
    listeners: HashMap<CString, Arc<AtomicBool>>,
    /// Unlisted Logitech paths that answered nothing; skipped by the periodic
    /// rescan, retried on any device change, refresh, reconnection or every
    /// `SKIP_RETRY` (a mouse asleep behind a receiver sends no notification).
    skip: HashSet<CString>,
    skip_since: std::time::Instant,
    /// «Descargar fotos de los ratones», read by the download threads right
    /// before they save, and the photos already handled this session (one
    /// download attempt or one «seen» per photo and session).
    photos_on: Arc<AtomicBool>,
    photos_done: std::collections::HashSet<&'static str>,
}

impl Worker {
    pub fn new(
        shared: Arc<Mutex<Shared>>,
        notify: Box<dyn Fn() + Send>,
        conflicts: fn() -> Vec<&'static str>,
        tx: Sender<Command>,
    ) -> Self {
        let mut config = Config::load();
        let dropped = config.sanitize();
        Self {
            api: hidapi::HidApi::new().ok(),
            entries: Vec::new(),
            shared,
            notify,
            conflicts,
            dropped,
            libraries: games::Libraries::default(),
            games: HashMap::new(),
            running: Vec::new(),
            unanswered: Vec::new(),
            pending: HashMap::new(),
            page_edits: Vec::new(),
            tx,
            listeners: HashMap::new(),
            skip: HashSet::new(),
            skip_since: std::time::Instant::now(),
            photos_on: Arc::new(AtomicBool::new(config.general.photos)),
            photos_done: Default::default(),
            config,
        }
    }

    /// Starts a listener for every receiver without a live one. Each is a
    /// thread blocked on a read (no CPU while idle) that only reads.
    fn start_listeners(&mut self) {
        let Some(api) = &self.api else { return };
        for (vid, pid, path) in receivers(api) {
            // Only receivers worth listening to: a verified one, or one with
            // a device found behind it (any Logitech HID++ collection is
            // accepted, keyboards and headsets too). Limit: an unknown
            // receiver whose mouse sleeps at start gets its listener once
            // the mouse is found (periodic retry, `SKIP_RETRY`).
            let verified = crate::devices::SUPPORTED
                .iter()
                .any(|s| (s.vid, s.pid) == (vid, pid) && !s.wired);
            let prefix = format!("{vid:04x}:{pid:04x}/");
            let behind = self.entries.iter().any(|e| {
                e.snapshot.info.key.0.starts_with(&prefix)
                    && e.device
                        .as_ref()
                        .is_some_and(|d| d.receiver_index().is_some())
            });
            if !verified && !behind {
                continue;
            }
            if self
                .listeners
                .get(&path)
                .is_some_and(|a| a.load(Ordering::Relaxed))
            {
                continue;
            }
            // Opening can fail if another program holds it exclusively; the
            // next rescan tries again.
            let Ok(t) = HidTransport::open(api, &path) else {
                continue;
            };
            let alive = Arc::new(AtomicBool::new(true));
            let flag = alive.clone();
            let tx = self.tx.clone();
            let spawned = std::thread::Builder::new()
                .name("receiver".into())
                .spawn(move || {
                    listen(&t, RECONNECT_SETTLE, |index| {
                        let _ = tx.send(Command::Reconnected(Slot { vid, pid, index }));
                    });
                    flag.store(false, Ordering::Relaxed);
                });
            if spawned.is_ok() {
                self.listeners.insert(path, alive);
            }
        }
    }

    /// Sends the saved DPI and light again to a device that came back,
    /// retrying once; a second failure is left for the next rescan.
    fn reconnected(&mut self, slot: Slot) {
        let found = targets(&self.entries, slot);
        if found.is_empty() {
            // A device not seen before: find it now, not at the next poll.
            self.skip.clear();
            return self.rescan(None);
        }
        let mut failures = Vec::new();
        for i in found {
            self.read(i);
            let mut result = self.reapply(i);
            if result.is_err() {
                sleep(RECONNECT_RETRY);
                result = self.reapply(i);
            }
            self.entries[i].reapply_due = result.is_err();
            if let Err(e) = result {
                failures.push(e);
            }
        }
        self.publish((!failures.is_empty()).then(|| Message {
            ok: false,
            text: failures.join(" "),
        }));
    }

    pub fn run(mut self, rx: Receiver<Command>) {
        let dropped = (self.dropped > 0).then(|| Message {
            ok: false,
            text: format!(
                "Se descartaron {} perfiles de la configuración que no eran válidos; no se escribieron en el ratón.",
                self.dropped
            ),
        });
        if let Some(dir) = crate::photos::dir() {
            crate::photos::sweep(&dir, std::time::SystemTime::now(), crate::photos::MAX_AGE);
            if !self.config.general.photos {
                crate::photos::delete_all(&dir);
            }
        }
        self.rescan(dropped);
        self.libraries = crate::platform::libraries::load();
        // Commands that arrived while coalescing.
        let mut queue: std::collections::VecDeque<Command> = Default::default();
        let mut last_poll = std::time::Instant::now();
        loop {
            self.run_due_writes();
            // Wake up for the next poll or the next delayed write.
            let now = std::time::Instant::now();
            let mut wait = POLL.saturating_sub(now - last_poll);
            if let Some(next) = self.pending.values().map(|(t, ..)| *t).min() {
                wait = wait.min(next.saturating_duration_since(now));
            }
            let cmd = match queue.pop_front() {
                Some(c) => Ok(c),
                None => rx.recv_timeout(wait),
            };
            if matches!(cmd, Err(RecvTimeoutError::Timeout)) && last_poll.elapsed() < POLL {
                continue;
            }
            if matches!(cmd, Err(RecvTimeoutError::Timeout)) {
                last_poll = std::time::Instant::now();
            }
            match cmd {
                Ok(Command::Rescan) => {
                    sleep(SETTLE);
                    queue.extend(skip_rescans(&rx));
                    self.skip.clear();
                    self.rescan(None);
                }
                // A full rescan also finds wireless devices that were switched
                // on behind an already present receiver (no OS notification).
                Err(RecvTimeoutError::Timeout) => {
                    if self.skip_since.elapsed() >= SKIP_RETRY {
                        self.skip.clear();
                        self.skip_since = std::time::Instant::now();
                    }
                    self.rescan(None)
                }
                Ok(Command::Refresh) => {
                    self.skip.clear();
                    self.rescan(Some(Message {
                        ok: true,
                        text: "Lista de dispositivos actualizada.".into(),
                    }))
                }
                Ok(Command::Try(key, change)) => {
                    if let Some(i) = self.index(&key) {
                        self.try_change(i, change)
                    }
                }
                Ok(Command::Restore(key)) => {
                    if let Some(i) = self.index(&key) {
                        self.restore(i)
                    }
                }
                Ok(Command::ReadProfile(key)) => {
                    if let Some(i) = self.index(&key) {
                        self.read_profile(i)
                    }
                }
                Ok(Command::ActivateOnboard(key, Confirmed(()))) => {
                    if let Some(i) = self.index(&key) {
                        self.activate_onboard(i)
                    }
                }
                Ok(Command::SaveProfile(key, edits, Confirmed(()))) => {
                    if let Some(i) = self.index(&key) {
                        // What was saved in the mouse is also what the app
                        // applies when the mouse connects.
                        let saved: Vec<Change> = edits
                            .dpi
                            .map(Change::Dpi)
                            .into_iter()
                            .chain(edits.light.map(Change::Light))
                            .collect();
                        let e = &self.entries[i];
                        if e.snapshot.info.support == Support::Auto
                            || matches!(e.profile, Some(Err(Error::HostMode)))
                        {
                            // Buttons only exist in the mouse's memory: they
                            // are never dropped silently.
                            if !edits.buttons.is_empty() {
                                let name = e.snapshot.info.name.clone();
                                self.finish_write(Message {
                                    ok: false,
                                    text: format!(
                                        "{name}: no se guardó nada; sus botones no se pueden cambiar ahora."
                                    ),
                                });
                                continue;
                            }
                            self.save_in_app(i, saved);
                            continue;
                        }
                        self.write_profile(
                            i,
                            "Guardado en el ratón.",
                            |d, _| {
                                onboard::apply_edits(
                                    &d.user,
                                    d.sector,
                                    &d.directory,
                                    &d.limits(),
                                    &edits,
                                )
                            },
                            |w| {
                                if w.has_profiles(&key) {
                                    w.update_active_from_mouse(i);
                                } else if !saved.is_empty() {
                                    saved.into_iter().for_each(|c| w.config.record(&key, c));
                                    w.save_config();
                                }
                            },
                        );
                    }
                }
                Ok(Command::RestoreProfile(key, source, Confirmed(()))) => {
                    if let Some(i) = self.index(&key) {
                        let (done, build): (&str, Build) = match source {
                            Source::Original => ("Perfil original restaurado.", |d, saved| {
                                if saved.sector != d.sector || saved.info != d.info {
                                    return Err(EditError::BadProfile);
                                }
                                onboard::restore_image(
                                    &saved.user,
                                    &d.user,
                                    d.sector,
                                    &d.directory,
                                    &d.limits(),
                                )
                            }),
                            Source::Factory => ("Valores de fábrica restaurados.", |d, _| {
                                onboard::factory_image(
                                    &d.user,
                                    &d.rom,
                                    d.sector,
                                    &d.directory,
                                    &d.limits(),
                                )
                            }),
                        };
                        // The restored profile rules: the app stops imposing
                        // its own DPI and light.
                        // With profiles, the active one takes what was restored.
                        self.write_profile(i, done, build, |w| {
                            if w.has_profiles(&key) {
                                w.update_active_from_mouse(i);
                            } else {
                                w.config.forget(&key);
                                w.save_config();
                            }
                        });
                    }
                }
                Ok(Command::UseProfile(key, name, _)) => {
                    // A later "Usar" for the same mouse replaces this one.
                    let (name, others) = latest_use(&rx, &key, name);
                    queue.extend(others);
                    // A choice by hand lasts until the open games close.
                    if let Some(g) = self.games.get_mut(&key.0) {
                        g.manual_use();
                    }
                    self.pending.remove(&key.0);
                    if let Some(i) = self.index(&key) {
                        self.use_profile(i, &name);
                    }
                }
                Ok(Command::NewProfile(key, name)) => {
                    self.edit_profiles(&key, |s| {
                        let base = s.active_profile().cloned().ok_or("no hay perfil activo.")?;
                        let n = add_profile(s, &name, base)?;
                        Ok(format!(
                            "Perfil «{n}» creado. Pulsa «Usar» para escribirlo en el ratón."
                        ))
                    });
                }
                Ok(Command::SaveAsProfile(key, name)) => {
                    let read = self.index(&key).and_then(|i| self.mouse_profile(i, &name));
                    self.edit_profiles(&key, |s| {
                        let p = read.ok_or("no se pudo leer el perfil del ratón.")?;
                        let n = add_profile(s, &name, p)?;
                        s.activate(&n);
                        Ok(format!(
                            "Perfil «{n}» creado con lo que tiene el ratón, y activo."
                        ))
                    });
                }
                Ok(Command::RenameProfile(key, old, new)) => {
                    let renamed = self.edit_profiles(&key, |s| {
                        let new = new.trim().to_string();
                        if let Some(e) = profiles::name_error(&new, &s.profiles, Some(&old)) {
                            return Err(e);
                        }
                        let p = s
                            .profiles
                            .iter_mut()
                            .find(|p| p.name == old)
                            .ok_or("ese perfil ya no existe.")?;
                        p.name = new.clone();
                        if s.active.as_deref() == Some(old.as_str()) {
                            s.active = Some(new.clone());
                        }
                        for prog in &mut s.programs {
                            if prog.profile.as_deref() == Some(old.as_str()) {
                                prog.profile = Some(new.clone());
                            }
                        }
                        Ok(format!("Perfil «{old}» renombrado a «{new}»."))
                    });
                    if renamed {
                        if let Some(g) = self.games.get_mut(&key.0) {
                            g.profile_renamed(&old, Some(new.trim()));
                        }
                    }
                }
                Ok(Command::DeleteProfile(key, name)) => {
                    let deleted = self.edit_profiles(&key, |s| {
                        if s.active.as_deref() == Some(name.as_str()) {
                            return Err("no se puede eliminar el perfil activo.".into());
                        }
                        let before = s.profiles.len();
                        s.profiles.retain(|p| p.name != name);
                        if s.profiles.len() == before {
                            return Err("ese perfil ya no existe.".into());
                        }
                        // Its programs: «No cambiar de perfil» (DESIGN.md).
                        for prog in &mut s.programs {
                            if prog.profile.as_deref() == Some(name.as_str()) {
                                prog.profile = None;
                            }
                        }
                        Ok(format!("Perfil «{name}» eliminado de este PC."))
                    });
                    if deleted {
                        if let Some(g) = self.games.get_mut(&key.0) {
                            g.profile_renamed(&name, None);
                        }
                    }
                }
                Ok(Command::ProcessOpened(pid, path)) => self.process_opened(pid, &path),
                Ok(Command::ProcessClosed(pid)) => self.process_closed(pid),
                Ok(Command::SetProgram(key, exe, profile)) => self.set_program(&key, &exe, profile),
                Ok(Command::CreateGameProfile(key, exe, name)) => {
                    let mut created = None;
                    self.edit_profiles(&key, |s| {
                        let base = s.active_profile().cloned().ok_or("no hay perfil activo.")?;
                        let n = add_profile(s, &name, base)?;
                        created = Some(n.clone());
                        Ok(format!("Perfil «{n}» creado."))
                    });
                    if let Some(n) = created {
                        self.set_program(&key, &exe, Some(n));
                    }
                }
                Ok(Command::RemoveProgram(key, exe)) => {
                    let exe = games::normalize(&exe).unwrap_or(exe);
                    self.edit_profiles(&key, |s| {
                        s.programs.retain(|p| p.exe != exe);
                        Ok(format!(
                            "{} quitado de la lista.",
                            games::display_name(&exe)
                        ))
                    });
                }
                Ok(Command::SetGeneral(g)) => {
                    let auto = g.auto_switch;
                    let photos = g.photos;
                    self.config.set_game_options(g.auto_switch, g.ask_new);
                    self.config.general = g;
                    if !auto {
                        self.pending.clear();
                    }
                    self.save_config();
                    if photos != self.photos_on.swap(photos, Ordering::Relaxed) {
                        self.photos_done.clear();
                        if !photos {
                            if let Some(dir) = crate::photos::dir() {
                                crate::photos::delete_all(&dir);
                            }
                            self.photos_changed();
                        }
                        self.check_photos();
                    }
                    self.publish(None);
                }
                Ok(Command::PhotoReady) => {
                    self.photos_changed();
                    self.publish(None);
                }
                Ok(Command::PageEdits(key, on)) => {
                    self.page_edits.retain(|k| k != &key);
                    if on {
                        self.page_edits.push(key);
                    }
                }
                Ok(Command::Reconnected(slot)) => self.reconnected(slot),
                Ok(Command::Quit) | Err(RecvTimeoutError::Disconnected) => break,
            }
        }
    }

    fn has_profiles(&self, key: &DeviceKey) -> bool {
        self.config.get(key).is_some_and(|s| !s.profiles.is_empty())
    }

    /// What the mouse holds (last read), as a profile named `name`.
    fn mouse_profile(&self, i: usize, name: &str) -> Option<MouseProfile> {
        match &self.entries[i].profile {
            Some(Ok(d)) => Some(MouseProfile::from_profile(
                name,
                &onboard::parse_buttons(&d.user, d.info.button_count)?,
                profile_light(d),
                onboard::parse_dpi_cycle(&d.user),
            )),
            _ => None,
        }
    }

    /// After a save or a restore, the active profile takes what the mouse
    /// now holds (read back by `write_profile`).
    fn update_active_from_mouse(&mut self, i: usize) {
        let key = self.entries[i].snapshot.info.key.clone();
        let Some(read) = self.mouse_profile(i, "") else {
            return;
        };
        let s = self.config.settings_mut(&key);
        if let Some(p) = s.active_profile_mut() {
            p.update_from(read);
            s.sync_active();
            self.save_config();
        }
    }

    /// First successful read of a mouse without profiles: what it holds
    /// becomes "Perfil 1" (`first_profile`). Nothing is written to the mouse.
    fn migrate(&mut self, i: usize) {
        let key = self.entries[i].snapshot.info.key.clone();
        if self.has_profiles(&key) {
            return;
        }
        let Some(read) = self.mouse_profile(i, profiles::FIRST_NAME) else {
            return;
        };
        let s = self.config.settings_mut(&key);
        let p = first_profile(read, s);
        s.profiles.push(p);
        s.active = Some(profiles::FIRST_NAME.into());
        self.save_config();
    }

    /// Applies a profile list edit, saves the config and reports it; true
    /// when it was applied.
    fn edit_profiles(
        &mut self,
        key: &DeviceKey,
        edit: impl FnOnce(&mut crate::config::DeviceSettings) -> std::result::Result<String, String>,
    ) -> bool {
        let result = match self.config.devices.get_mut(&key.0) {
            Some(s) => edit(s),
            None => Err("este ratón no tiene perfiles.".into()),
        };
        let ok = result.is_ok();
        let message = match result {
            Ok(text) => {
                self.save_config();
                Message { ok: true, text }
            }
            Err(text) => Message {
                ok: false,
                text: format!("Error: {text}"),
            },
        };
        self.publish(Some(message));
        ok
    }

    // -----------------------------------------------------------------------
    // Profiles per game (plan `perfiles-por-juego`).

    /// Mice whose config has profiles (the ones profiles per game apply to).
    fn profile_mice(&self) -> Vec<DeviceKey> {
        self.config
            .devices
            .iter()
            .filter(|(_, s)| !s.profiles.is_empty())
            .map(|(k, _)| DeviceKey(k.clone()))
            .collect()
    }

    fn process_opened(&mut self, pid: u32, path: &str) {
        let Some(exe) = games::normalize(path) else {
            return;
        };
        self.running.retain(|(p, _)| *p != pid);
        self.running.push((pid, exe.clone()));
        let is_game = self.libraries.is_game(&exe);
        let mut asked = false;
        for key in self.profile_mice() {
            let Some(s) = self.config.get(&key) else {
                continue;
            };
            let assoc = match s.programs.iter().find(|p| p.exe == exe) {
                Some(Program {
                    profile: Some(n), ..
                }) => Assoc::Profile(n.clone()),
                Some(_) => Assoc::NoChange,
                // One question per game, for the first mouse that asks.
                None => Assoc::Unknown {
                    game: is_game && !asked,
                },
            };
            let g = &self.config.general;
            let (active, auto, ask) = (s.active.clone(), g.auto_switch, g.ask_new);
            let action = self.games.entry(key.0.clone()).or_default().opened(
                pid,
                &exe,
                assoc,
                active.as_deref(),
                auto,
                ask,
            );
            if matches!(action, games::Action::Ask(_)) {
                asked = true;
            }
            self.act(&key, action, Some(pid));
        }
    }

    fn process_closed(&mut self, pid: u32) {
        self.running.retain(|(p, _)| *p != pid);
        for key in self.profile_mice() {
            let active = self.config.get(&key).and_then(|s| s.active.clone());
            let action = match self.games.get_mut(&key.0) {
                Some(g) => g.closed(pid, active.as_deref()),
                None => continue,
            };
            // A write still waiting for this program is not needed anymore.
            if self
                .pending
                .get(&key.0)
                .is_some_and(|(_, _, p)| *p == Some(pid))
            {
                self.pending.remove(&key.0);
            }
            self.act(&key, action, None);
        }
        self.publish(None);
    }

    fn act(&mut self, key: &DeviceKey, action: games::Action, pid: Option<u32>) {
        match action {
            games::Action::None => {}
            games::Action::SwitchTo(name, auth) | games::Action::Restore(name, auth) => {
                self.auto_switch(key, &name, auth, pid)
            }
            games::Action::Ask(exe) => {
                if !self.unanswered.iter().any(|(k, e)| k == key && *e == exe) {
                    self.unanswered.push((key.clone(), exe.clone()));
                }
                let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
                s.asks += 1;
                s.ask = Some((key.clone(), exe));
                drop(s);
                self.publish(None);
            }
        }
    }

    /// Hybrid switch (decision of the user): the profile becomes the active
    /// one and its DPI and light are sent live at once (no memory write);
    /// the buttons are written 5 s later only if they differ from the
    /// mouse's (`run_due_writes`).
    fn auto_switch(&mut self, key: &DeviceKey, name: &str, _auth: AutoApply, pid: Option<u32>) {
        if !self.config.settings_mut(key).activate(name) {
            return;
        }
        self.save_config();
        if let Some(i) = self.index(key) {
            let _ = self.reapply(i);
        }
        self.pending.insert(
            key.0.clone(),
            (
                std::time::Instant::now() + AUTO_WRITE_DELAY,
                name.to_string(),
                pid,
            ),
        );
        self.publish(None);
    }

    /// Writes the buttons of automatic switches whose 5 s have passed.
    fn run_due_writes(&mut self) {
        let now = std::time::Instant::now();
        let due: Vec<String> = self
            .pending
            .iter()
            .filter(|(_, (t, ..))| *t <= now)
            .map(|(k, _)| k.clone())
            .collect();
        for k in due {
            let key = DeviceKey(k.clone());
            // Waits for a mouse that is not connected (applied on return).
            let Some(i) = self
                .index(&key)
                .filter(|&i| self.entries[i].device.is_some())
            else {
                continue;
            };
            let Some((_, name, pid)) = self.pending.remove(&k) else {
                continue;
            };
            let Some(s) = self.config.get(&key) else {
                continue;
            };
            // Something else happened meanwhile: nothing to write.
            if !self.config.general.auto_switch
                || s.active.as_deref() != Some(name.as_str())
                || pid.is_some_and(|p| !self.games.get(&k).is_some_and(|g| g.is_open(p)))
            {
                continue;
            }
            let Some(profile) = s.profiles.iter().find(|p| p.name == name).cloned() else {
                continue;
            };
            self.read_profile(i);
            let differ =
                matches!(&self.entries[i].profile, Some(Ok(d)) if buttons_differ(&profile, d));
            if !differ {
                continue;
            }
            if self.page_edits.contains(&key) {
                self.auto_report(Message {
                    ok: false,
                    text: format!(
                        "No se escribieron los botones de «{name}»: hay cambios de botones sin guardar en la página."
                    ),
                });
                continue;
            }
            let device = self.entries[i].snapshot.info.name.clone();
            match self.try_write(i, |d, _| profile_image(&profile, d)) {
                Ok(()) => {
                    self.read_profile(i);
                }
                Err(e) => {
                    let from = name.clone();
                    self.read_profile(i);
                    self.auto_report(Message {
                        ok: false,
                        text: format!(
                            "{device}: no se pudieron escribir los botones de «{from}» ({e}) DPI y luz sí están en uso."
                        ),
                    });
                }
            }
        }
    }

    /// Result of an automatic switch shown in the page and as a notification.
    fn auto_report(&mut self, message: Message) {
        {
            let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
            s.autos += 1;
            s.auto_result = Some(message.clone());
        }
        self.publish(Some(message));
    }

    fn set_program(&mut self, key: &DeviceKey, exe: &str, profile: Option<String>) {
        let Some(exe) = games::normalize(exe) else {
            self.publish(Some(Message {
                ok: false,
                text: "Error: esa ruta no es válida.".into(),
            }));
            return;
        };
        let ok = self.edit_profiles(key, |s| {
            if let Some(n) = &profile {
                if !s.profiles.iter().any(|p| &p.name == n) {
                    return Err("ese perfil ya no existe.".into());
                }
            }
            let full = s.programs.len() >= games::MAX_PROGRAMS;
            match s.programs.iter_mut().find(|p| p.exe == exe) {
                Some(p) => p.profile = profile.clone(),
                None if full => {
                    return Err(format!(
                        "ya hay {} programas. Quita alguno para añadir otro.",
                        games::MAX_PROGRAMS
                    ));
                }
                None => s.programs.push(Program {
                    exe: exe.clone(),
                    profile: profile.clone(),
                }),
            }
            Ok(match &profile {
                Some(n) => format!("{} usará el perfil «{n}».", games::display_name(&exe)),
                None => format!("{}: no cambiará de perfil.", games::display_name(&exe)),
            })
        });
        if !ok {
            return;
        }
        self.unanswered.retain(|(k, e)| !(k == key && *e == exe));
        // Already running: it takes effect now.
        let open: Vec<u32> = self
            .running
            .iter()
            .filter(|(_, e)| *e == exe)
            .map(|(p, _)| *p)
            .collect();
        for pid in open {
            let path = exe.clone();
            if let Some(g) = self.games.get_mut(&key.0) {
                g.closed(pid, None);
            }
            self.process_opened(pid, &path);
        }
        self.publish(None);
    }

    /// "Usar": writes the profile into the mouse unless it already holds
    /// it, then sends its DPI and light live (the mouse keeps running on
    /// the values it started with).
    fn use_profile(&mut self, i: usize, name: &str) {
        let key = self.entries[i].snapshot.info.key.clone();
        let Some(profile) = self
            .config
            .get(&key)
            .and_then(|s| s.profiles.iter().find(|p| p.name == name))
            .cloned()
        else {
            self.finish_write(Message {
                ok: false,
                text: format!("Error: el perfil «{name}» ya no existe."),
            });
            return;
        };
        // A fresh read: nothing is written when the mouse already holds it.
        self.read_profile(i);
        let holds = matches!(&self.entries[i].profile, Some(Ok(d)) if holds(&profile, d));
        let done = format!(
            "perfil «{name}» activo en el ratón. Se comprobó la escritura leyendo de nuevo el perfil."
        );
        // Active (and its DPI and light sent live) before the result is
        // published.
        let activate = |w: &mut Self| {
            w.config.settings_mut(&key).activate(name);
            w.save_config();
            let _ = w.reapply(i);
        };
        if holds {
            activate(self);
            let device = self.entries[i].snapshot.info.name.clone();
            self.finish_write(Message {
                ok: true,
                text: format!("{device}: perfil «{name}» activo en el ratón (ya lo tenía)."),
            });
        } else {
            self.write_profile(i, &done, |d, _| profile_image(&profile, d), activate);
        }
    }

    /// Ends a profile write request with `message` (see `Shared::writes`).
    fn finish_write(&mut self, message: Message) {
        {
            let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
            s.writes += 1;
            s.write_result = Some(message.clone());
        }
        self.publish(Some(message));
    }

    fn index(&self, key: &DeviceKey) -> Option<usize> {
        self.entries
            .iter()
            .position(|e| &e.snapshot.info.key == key)
    }

    fn publish(&self, message: Option<Message>) {
        let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
        // Saved settings come from the config, the single source of truth.
        s.devices = self
            .entries
            .iter()
            .map(|e| {
                let key = &e.snapshot.info.key;
                let saved = self.config.get(key);
                Snapshot {
                    saved: saved.is_some(),
                    saved_dpi: saved.and_then(|c| c.dpi),
                    saved_light: saved
                        .and_then(|c| c.light.as_deref())
                        .and_then(crate::config::parse_light),
                    applied_color: saved.and_then(|c| c.color.as_deref()).and_then(parse_hex),
                    original_dpi: e.original_dpi,
                    profile: e.profile_view(),
                    backup: e.backup.clone(),
                    profiles: saved.map(|c| c.profiles.clone()).unwrap_or_default(),
                    active_profile: saved.and_then(|c| c.active.clone()),
                    programs: saved.map(|c| c.programs.clone()).unwrap_or_default(),
                    game: self.games.get(&key.0).and_then(|g| g.game()),
                    game_previous: self
                        .games
                        .get(&key.0)
                        .and_then(|g| g.game().and(g.previous().map(str::to_string))),
                    unanswered: self
                        .unanswered
                        .iter()
                        .filter(|(k, _)| k == key)
                        .map(|(_, e)| e.clone())
                        .collect(),
                    ..e.snapshot.clone()
                }
            })
            .collect();
        s.conflicts = (self.conflicts)();
        s.general = self.config.general.clone();
        s.scanned = true;
        s.hid_available = self.api.is_some();
        if message.is_some() {
            s.message = message;
            s.seq += 1;
        }
        drop(s);
        (self.notify)();
    }

    fn read(&mut self, i: usize) {
        let Some(api) = &self.api else { return };
        let e = &mut self.entries[i];
        let Some(dev) = e.device.as_mut() else {
            e.snapshot.status = Status::Disconnected;
            return;
        };
        let result = HidTransport::open(api, dev.path()).and_then(|t| dev.read_state(&t));
        e.snapshot.status = match result {
            Ok(state) => {
                if state.battery.is_some() {
                    e.snapshot.last_battery = state.battery;
                }
                if !e.original_read {
                    e.original_read = true;
                    e.original_dpi = state.dpi.as_ref().and_then(|d| d.current);
                }
                Status::Ok(state)
            }
            Err(err) => Status::Failed(err),
        };
    }

    fn rescan(&mut self, done: Option<Message>) {
        let Some(api) = self.api.as_mut() else {
            self.publish(Some(Message {
                ok: false,
                text: "No se pudo iniciar el acceso HID.".into(),
            }));
            return;
        };
        // Limit: two identical models share a key, so only one is listed.
        let mut found: HashMap<DeviceKey, Box<dyn Device>> = scan(api, &mut self.skip)
            .into_iter()
            .map(|d| (d.info().key.clone(), d))
            .collect();
        // Devices no driver handles: shown, never opened. One a driver found
        // (a Razer mouse is also listed until its probe answers) is not.
        // Nor one whose entry has a driver (asleep now: it is disconnected,
        // not a device without settings).
        let mut seen: HashMap<DeviceKey, DeviceInfo> = listed(api)
            .into_iter()
            .filter(|i| !found.keys().any(|k| same_vid_pid(k, &i.key)))
            .filter(|i| {
                !self.entries.iter().any(|e| {
                    e.snapshot.info.support != Support::Listed
                        && same_vid_pid(&e.snapshot.info.key, &i.key)
                })
            })
            .map(|i| (i.key.clone(), i))
            .collect();
        for e in &mut self.entries {
            e.device = found.remove(&e.snapshot.info.key);
            // A device listed before (its probe failed) that a driver found
            // now takes the driver's info: settings, vendor, connection.
            if let Some(d) = &e.device {
                if e.snapshot.info.support != d.info().support {
                    e.snapshot.info = d.info().clone();
                }
            }
            if seen.remove(&e.snapshot.info.key).is_some() {
                e.snapshot.status = Status::Ok(State::default());
            } else if e.device.is_none() {
                e.snapshot.status = Status::Disconnected;
            }
        }
        // A device without settings that left has nothing to keep.
        self.entries.retain(|e| {
            e.snapshot.info.support != Support::Listed
                || !matches!(e.snapshot.status, Status::Disconnected)
        });
        let new = found
            .into_values()
            .map(|d| (d.info().clone(), Some(d)))
            .chain(seen.into_values().map(|i| (i, None)));
        for (info, device) in new {
            let status = if device.is_some() {
                Status::Searching
            } else {
                Status::Ok(State::default())
            };
            self.entries.push(Entry {
                device,
                snapshot: Snapshot {
                    info,
                    status,
                    last_battery: None,
                    saved: false,
                    saved_dpi: None,
                    saved_light: None,
                    applied_color: None,
                    original_dpi: None,
                    profile: Profile::NotRead,
                    backup: None,
                    profiles: Vec::new(),
                    active_profile: None,
                    programs: Vec::new(),
                    game: None,
                    game_previous: None,
                    unanswered: Vec::new(),
                },
                original_dpi: None,
                original_read: false,
                profile: None,
                backup: None,
                reapply_due: false,
            });
        }
        // Devices without settings go last.
        self.entries.sort_by_key(|e| {
            let i = &e.snapshot.info;
            (i.support == Support::Listed, i.vendor, i.name.clone())
        });
        let mut failures = Vec::new();
        for i in 0..self.entries.len() {
            // Volatile settings are lost when a device powers off, so any
            // transition into a working state re-applies them.
            let was_absent = !matches!(self.entries[i].snapshot.status, Status::Ok(_));
            if self.entries[i].device.is_none() {
                continue;
            }
            self.read(i);
            let due = was_absent || self.entries[i].reapply_due;
            if due && matches!(self.entries[i].snapshot.status, Status::Ok(_)) {
                let result = self.reapply(i);
                self.entries[i].reapply_due = result.is_err();
                if let Err(e) = result {
                    failures.push(e);
                }
            }
        }
        self.start_listeners();
        self.check_photos();
        let message = if failures.is_empty() {
            done
        } else {
            Some(Message {
                ok: false,
                text: failures.join(" "),
            })
        };
        self.publish(message);
    }

    /// For each model with a photo seen for the first time this session:
    /// marks a cached photo as seen, or downloads it in a thread of its own
    /// (the app never waits for it, not even to quit).
    fn check_photos(&mut self) {
        let Some(dir) = crate::photos::dir() else {
            return;
        };
        if !self.config.general.photos {
            return;
        }
        for e in self.entries.iter().filter(|e| e.device.is_some()) {
            let Some(photo) = crate::photos::photo_for(&e.snapshot.info.name) else {
                continue;
            };
            if !self.photos_done.insert(photo.file) {
                continue;
            }
            if crate::photos::load(&dir, photo).is_some() {
                crate::photos::touch(&dir, photo);
                continue;
            }
            let (tx, on, dir) = (self.tx.clone(), self.photos_on.clone(), dir.clone());
            let _ = std::thread::Builder::new()
                .name("photo".into())
                .spawn(move || {
                    let keep = || on.load(Ordering::Relaxed);
                    let endpoint = crate::photos::Endpoint::openmouse();
                    if crate::photos::download(&endpoint, photo, &dir, &keep).is_ok() {
                        let _ = tx.send(Command::PhotoReady);
                    }
                });
        }
    }

    fn photos_changed(&self) {
        let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
        s.photos += 1;
    }

    /// «Activar la memoria del ratón»: back to the onboard profiles, then
    /// the profile is read again. Succeeds only if it can be read.
    fn activate_onboard(&mut self, i: usize) {
        let name = self.entries[i].snapshot.info.name.clone();
        let result = match (&self.api, self.entries[i].device.as_mut()) {
            (Some(api), Some(dev)) => {
                HidTransport::open(api, dev.path()).and_then(|t| dev.activate_onboard(&t))
            }
            _ => Err(Error::Timeout),
        };
        self.read_profile(i);
        let message = match (result, &self.entries[i].profile) {
            (Ok(()), Some(Ok(_))) => Message {
                ok: true,
                text: format!("{name}: memoria del ratón activada."),
            },
            (Ok(()), Some(Err(e))) => Message {
                ok: false,
                text: format!("{name}: se activó, pero no se pudo leer el perfil ({e})."),
            },
            (Ok(()), None) => Message {
                ok: false,
                text: format!("{name}: se activó, pero no se pudo leer el perfil."),
            },
            (Err(e), _) => Message {
                ok: false,
                text: format!("{name}: {e}."),
            },
        };
        self.finish_write(message);
    }

    /// Reads the onboard profile (read only); the first time it also takes
    /// the original copy, then publishes the result.
    fn read_profile(&mut self, i: usize) {
        let Some(api) = &self.api else { return };
        let e = &mut self.entries[i];
        let Some(dev) = e.device.as_mut() else { return };
        let read = HidTransport::open(api, dev.path()).and_then(|t| dev.read_profile(&t));
        if let Ok(dump) = &read {
            e.backup = Some(Self::ensure_backup(&e.snapshot.info.key, dump, || {
                HidTransport::open(api, dev.path()).and_then(|t| dev.read_profile(&t))
            }));
        }
        let ok = read.is_ok();
        e.profile = Some(read);
        if ok {
            self.migrate(i);
        }
        self.publish(None);
    }

    /// Takes the original copy the first time; it is verified with a second
    /// read from the device. An existing copy is only checked for validity
    /// (the profile may have changed since, by design).
    fn ensure_backup(
        key: &DeviceKey,
        dump: &crate::devices::ProfileDump,
        reread: impl FnOnce() -> crate::devices::Result<crate::devices::ProfileDump>,
    ) -> std::result::Result<String, String> {
        if let Some(existing) = crate::backup::load_original(key)? {
            return Ok(existing.created);
        }
        let second = reread().map_err(|e| format!("segunda lectura: {e}"))?;
        if &second != dump {
            return Err("dos lecturas del ratón no coinciden".into());
        }
        crate::backup::save_original(key, dump, &crate::backup::today()).map(|b| b.created)
    }

    /// Writes a new image of the active profile and reports the result.
    /// `on_ok` updates the config before the result is published, so the
    /// page never refills its controls from the values before the write.
    fn write_profile(
        &mut self,
        i: usize,
        done: &str,
        build: impl FnOnce(&ProfileDump, &ProfileDump) -> std::result::Result<ProfileImage, EditError>,
        on_ok: impl FnOnce(&mut Self),
    ) -> bool {
        let name = self.entries[i].snapshot.info.name.clone();
        let result = self.try_write(i, build);
        let ok = result.is_ok();
        let message = match result {
            Ok(()) => Message {
                ok: true,
                text: format!("{name}: {done}"),
            },
            Err(text) => Message {
                ok: false,
                text: format!("{name}: {text}"),
            },
        };
        self.read_profile(i);
        if ok {
            on_ok(self);
        }
        self.finish_write(message);
        ok
    }

    /// A failed config save only loses what is re-applied on connect; the
    /// mouse already holds the change.
    fn save_config(&mut self) {
        if let Err(err) = self.config.save() {
            self.publish(Some(Message {
                ok: false,
                text: format!("No se pudo guardar la configuración de la app ({err})."),
            }));
        }
    }

    /// "Guardar" on a mouse found by automatic detection: its onboard profile
    /// is never written, so the changes are applied now and recorded in the
    /// app, which re-applies them whenever the mouse connects.
    fn save_in_app(&mut self, i: usize, changes: Vec<Change>) {
        let name = self.entries[i].snapshot.info.name.clone();
        let key = self.entries[i].snapshot.info.key.clone();
        let result = match (&self.api, self.entries[i].device.as_mut()) {
            (Some(api), Some(dev)) => HidTransport::open(api, dev.path())
                .and_then(|t| changes.iter().try_for_each(|c| dev.apply(&t, *c))),
            _ => Err(Error::Timeout),
        };
        self.read(i);
        let message = match result {
            Ok(()) => {
                changes
                    .into_iter()
                    .for_each(|c| self.config.record(&key, c));
                self.save_config();
                Message {
                    ok: true,
                    text: format!("{name}: guardado en OpenPeripheral; se aplicará al conectarlo."),
                }
            }
            Err(err) => Message {
                ok: false,
                text: format!("{name}: no se pudo guardar ({err})."),
            },
        };
        self.finish_write(message);
    }

    /// "Probar": sends the change, records nothing.
    fn try_change(&mut self, i: usize, change: Change) {
        let Some(api) = &self.api else { return };
        let Some(e) = self.entries.get_mut(i) else {
            return;
        };
        let name = e.snapshot.info.name.clone();
        let Some(dev) = e.device.as_mut() else {
            return;
        };
        let result = HidTransport::open(api, dev.path()).and_then(|t| dev.apply(&t, change));
        self.read(i);
        let message = match result {
            Ok(()) => Message {
                ok: true,
                text: format!("{name}: probando, sin guardar."),
            },
            Err(err) => Message {
                ok: false,
                text: format!("{name}: no se pudo probar ({err})."),
            },
        };
        self.publish(Some(message));
    }

    /// Refused while the gate is closed, while G HUB runs, or without a
    /// valid original copy on this PC.
    fn try_write(
        &mut self,
        i: usize,
        build: impl FnOnce(&ProfileDump, &ProfileDump) -> std::result::Result<ProfileImage, EditError>,
    ) -> std::result::Result<(), String> {
        if !WRITES_ENABLED {
            return Err(
                "el guardado de botones está desactivado hasta completar la prueba de escritura."
                    .into(),
            );
        }
        if (self.conflicts)().contains(&"Logitech G HUB") {
            return Err("cierra G HUB para guardar los botones.".into());
        }
        let e = &mut self.entries[i];
        let saved = crate::backup::load_original(&e.snapshot.info.key)?
            .ok_or("no hay copia del perfil original en este PC; no se escribe nada.")?;
        let api = self.api.as_ref().ok_or("no hay acceso HID.")?;
        let dev = e.device.as_mut().ok_or("está desconectado.")?;
        let t =
            HidTransport::open(api, dev.path()).map_err(|e| format!("no se pudo abrir ({e})."))?;
        write_checked(dev.as_mut(), &t, &saved.dump, build)
    }

    fn reapply(&mut self, i: usize) -> Result<(), String> {
        let key = self.entries[i].snapshot.info.key.clone();
        let Some(changes) = self.config.get(&key).map(|s| s.changes()) else {
            return Ok(());
        };
        let Some(api) = &self.api else { return Ok(()) };
        let e = &mut self.entries[i];
        let Some(dev) = e.device.as_mut() else {
            return Ok(());
        };
        let name = e.snapshot.info.name.clone();
        let result = HidTransport::open(api, dev.path())
            .and_then(|t| changes.into_iter().try_for_each(|c| dev.apply(&t, c)));
        self.read(i);
        result.map_err(|err| {
            format!("{name}: no se pudieron reaplicar los ajustes guardados ({err}).")
        })
    }

    fn restore(&mut self, i: usize) {
        if self.api.is_none() {
            return;
        }
        let Some(e) = self.entries.get(i) else {
            return;
        };
        let name = e.snapshot.info.name.clone();
        let key = e.snapshot.info.key.clone();
        let mut ok = true;
        let mut text = if self.has_profiles(&key) {
            // The active profile takes what the mouse's own profile holds.
            self.update_active_from_mouse(i);
            format!("{name}: el perfil activo vuelve a lo que guarda el ratón.")
        } else {
            self.config.forget(&key);
            ok = self.config.save().is_ok();
            format!("{name}: ajustes guardados eliminados.")
        };
        let Some(api) = &self.api else { return };
        let e = &mut self.entries[i];
        if let (Some(dpi), Some(dev)) = (e.original_dpi, e.device.as_mut()) {
            match HidTransport::open(api, dev.path()).and_then(|t| dev.apply(&t, Change::Dpi(dpi)))
            {
                Ok(()) => text.push_str(&format!(" DPI restaurado a {dpi}.")),
                Err(err) => {
                    ok = false;
                    text.push_str(&format!(" No se pudo restaurar el DPI ({err})."));
                }
            }
        }
        // The profile's own light, if the app had replaced it.
        let profile_light = match &e.profile {
            Some(Ok(d)) => profile_light(d),
            _ => None,
        };
        if let (Some(light), Some(dev)) = (profile_light, e.device.as_mut()) {
            match HidTransport::open(api, dev.path())
                .and_then(|t| dev.apply(&t, Change::Light(light)))
            {
                Ok(()) => text.push_str(" Luz devuelta a la de su perfil."),
                Err(err) => {
                    ok = false;
                    text.push_str(&format!(" No se pudo devolver la luz ({err})."));
                }
            }
        }
        if let Status::Ok(s) = &e.snapshot.status {
            if s.color {
                text.push_str(" El color vuelve al del dispositivo al desconectarlo y conectarlo.");
            }
        }
        self.read(i);
        self.publish(Some(Message { ok, text }));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::devices::DeviceKey;
    use std::sync::mpsc::channel;

    /// A mouse whose profile lives in memory; `fail` makes the next N
    /// writes corrupt memory and report a read-back mismatch.
    struct MemMouse {
        info: DeviceInfo,
        path: std::ffi::CString,
        dump: ProfileDump,
        fail: usize,
        writes: usize,
    }

    impl Device for MemMouse {
        fn info(&self) -> &DeviceInfo {
            &self.info
        }
        fn path(&self) -> &std::ffi::CString {
            &self.path
        }
        fn receiver_index(&self) -> Option<u8> {
            Some(1)
        }
        fn read_state(&mut self, _: &dyn Transport) -> crate::devices::Result<State> {
            Ok(State::default())
        }
        fn apply(&mut self, _: &dyn Transport, _: Change) -> crate::devices::Result<()> {
            Ok(())
        }
        fn read_profile(&mut self, _: &dyn Transport) -> crate::devices::Result<ProfileDump> {
            Ok(self.dump.clone())
        }
        fn write_profile(
            &mut self,
            _: &dyn Transport,
            image: &ProfileImage,
        ) -> crate::devices::Result<()> {
            self.writes += 1;
            if self.fail > 0 {
                self.fail -= 1;
                // Half-written: the device holds garbage until rewritten.
                self.dump.user[0x30] ^= 0xff;
                return Err(Error::Protocol("la memoria releída no coincide"));
            }
            self.dump.user = image.bytes().to_vec();
            Ok(())
        }
    }

    #[test]
    fn same_usb_device_ignores_the_name() {
        let k = |s: &str| DeviceKey(s.into());
        assert!(same_vid_pid(
            &k("1532:00b9/Razer Basilisk"),
            &k("1532:00b9/Ratón Razer")
        ));
        assert!(!same_vid_pid(&k("1532:00b9/A"), &k("1532:0099/A")));
    }

    #[test]
    fn the_first_profile_keeps_the_dpi_and_light_the_app_applies() {
        let m = mouse(0);
        let buttons = onboard::parse_buttons(&m.dump.user, 6).unwrap();
        let read = MouseProfile::from_profile(
            profiles::FIRST_NAME,
            &buttons,
            None,
            Some([800, 850, 1600, 3200]),
        );
        assert_eq!(read.dpi, None, "a cycle is not a start DPI");
        let mut s = crate::config::DeviceSettings {
            dpi: Some(950),
            light: Some("apagada".into()),
            ..Default::default()
        };
        let mut p = first_profile(read.clone(), &s);
        assert_eq!((p.dpi, p.light.as_deref()), (Some(950), Some("apagada")));
        assert_eq!(p.buttons, read.buttons);
        // Using it later keeps them.
        p.name = profiles::FIRST_NAME.into();
        s.profiles.push(p);
        assert!(s.activate(profiles::FIRST_NAME));
        assert_eq!(s.dpi, Some(950));
        // Nothing saved in the app: what the mouse holds.
        let p = first_profile(read.clone(), &crate::config::DeviceSettings::default());
        assert_eq!(p, read);
    }

    #[test]
    fn a_mouse_without_profile_light_ignores_the_profiles_light() {
        let mut m = mouse(0);
        m.dump.effects.clear();
        let d = &m.dump;
        let red = Some(onboard::Light::Fixed(crate::protocol::razer::Rgb {
            r: 255,
            g: 0,
            b: 0,
        }));
        let buttons = onboard::parse_buttons(&d.user, 6).unwrap();
        let p =
            MouseProfile::from_profile("Trabajo", &buttons, red, onboard::parse_dpi_cycle(&d.user));
        // Same buttons and DPI: already held, and an image never carries light.
        assert!(holds(&p, d));
        let img = profile_image(&p, d).unwrap();
        assert_eq!(img.bytes(), d.user.as_slice());
        // With light effects the same profile asks for its light.
        m.dump.effects = onboard::samples::EFFECTS.to_vec();
        assert!(!holds(&p, &m.dump));
    }

    fn mouse(fail: usize) -> MemMouse {
        use onboard::samples;
        MemMouse {
            info: DeviceInfo::new(&crate::devices::SUPPORTED[0], "G305"),
            path: std::ffi::CString::new("x").unwrap(),
            dump: ProfileDump {
                info: onboard::Info {
                    profile_format: 3,
                    profile_count: 1,
                    button_count: 6,
                    sector_size: 255,
                },
                sector: 1,
                directory: samples::directory(),
                user: samples::user_profile(),
                rom: samples::rom_profile(),
                dpi: Some(samples::dpi_range()),
                effects: samples::EFFECTS.to_vec(),
            },
            fail,
            writes: 0,
        }
    }

    /// Transport fed with captured reports; fails once they run out, like
    /// an unplugged receiver.
    struct Reports(std::cell::RefCell<std::collections::VecDeque<Vec<u8>>>);

    impl Transport for Reports {
        fn write(&self, _: &[u8]) -> crate::devices::Result<()> {
            panic!("the listener never writes")
        }
        fn read_timeout(&self, buf: &mut [u8], _: i32) -> crate::devices::Result<usize> {
            let r = self.0.borrow_mut().pop_front().ok_or(Error::Timeout)?;
            buf[..r.len()].copy_from_slice(&r);
            Ok(r.len())
        }
        fn send_feature(&self, _: &[u8]) -> crate::devices::Result<()> {
            panic!("the listener never writes")
        }
        fn get_feature(&self, _: &mut [u8]) -> crate::devices::Result<usize> {
            unreachable!()
        }
    }

    fn heard(reports: &[&[u8]]) -> Vec<u8> {
        let t = Reports(std::cell::RefCell::new(
            reports.iter().map(|r| r.to_vec()).collect(),
        ));
        let mut out = Vec::new();
        listen(&t, Duration::ZERO, |i| out.push(i));
        out
    }

    const OFF: &[u8] = &[0x10, 0x01, 0x41, 0x0d, 0x62, 0x74, 0x40];
    const ON: &[u8] = &[0x10, 0x01, 0x41, 0x0d, 0xa2, 0x74, 0x40];
    const WIRELESS_STATUS: &[u8] = &[0x11, 0x01, 0x04, 0x00, 0x01, 0x01, 0x01];

    #[test]
    fn listener_reports_each_reconnection_once() {
        // The sequence captured when switching the G305 off and on.
        assert_eq!(heard(&[OFF, ON, WIRELESS_STATUS]), vec![1]);
        // Switching off only, or unrelated traffic: nothing.
        assert_eq!(
            heard(&[OFF, WIRELESS_STATUS, &[0x10, 0x02, 0x8f, 0, 0x1b, 8, 0]]),
            vec![]
        );
        // A burst is merged; another device is reported too.
        let other: &[u8] = &[0x10, 0x02, 0x41, 0x0d, 0xa2, 0x74, 0x40];
        assert_eq!(heard(&[ON, ON, other, ON]), vec![1, 2]);
        // Empty reads (timeouts) do not stop it; a later reconnection counts.
        assert_eq!(heard(&[&[], ON, &[], ON]), vec![1, 1]);
    }

    #[test]
    fn reconnection_targets_the_device_behind_that_receiver() {
        let entry = |m: MemMouse| Entry {
            snapshot: Snapshot {
                info: m.info.clone(),
                status: Status::Ok(State::default()),
                last_battery: None,
                saved: true,
                saved_dpi: None,
                saved_light: None,
                applied_color: None,
                original_dpi: None,
                profile: Profile::NotRead,
                backup: None,
                profiles: Vec::new(),
                active_profile: None,
                programs: Vec::new(),
                game: None,
                game_previous: None,
                unanswered: Vec::new(),
            },
            device: Some(Box::new(m)),
            original_dpi: None,
            original_read: false,
            profile: None,
            backup: None,
            reapply_due: false,
        };
        let mut gone = entry(mouse(0));
        gone.device = None;
        let entries = vec![entry(mouse(0)), gone];
        let slot = |vid, pid, index| Slot { vid, pid, index };
        assert_eq!(targets(&entries, slot(0x046d, 0xc53f, 1)), vec![0]);
        assert!(targets(&entries, slot(0x046d, 0xc53f, 2)).is_empty());
        assert!(targets(&entries, slot(0x046d, 0xc547, 1)).is_empty());
    }

    fn buttons(edits: Vec<(usize, Action)>) -> ProfileEdits {
        ProfileEdits {
            buttons: edits,
            ..Default::default()
        }
    }

    fn edits(d: &ProfileDump, _: &ProfileDump) -> std::result::Result<ProfileImage, EditError> {
        onboard::apply_edits(
            &d.user,
            d.sector,
            &d.directory,
            &d.limits(),
            &buttons(vec![(3, Action::Mouse(0x08))]),
        )
    }

    #[test]
    fn write_then_restore_original() {
        let t = crate::devices::fake::FakeTransport::new(|_| vec![]);
        let mut m = mouse(0);
        let saved = m.dump.clone();
        write_checked(&mut m, &t, &saved, edits).unwrap();
        assert_eq!(
            onboard::parse_buttons(&m.dump.user, 6).unwrap()[3],
            Action::Mouse(0x08)
        );
        write_checked(&mut m, &t, &saved, |d, s| {
            onboard::restore_image(&s.user, &d.user, d.sector, &d.directory, &d.limits())
        })
        .unwrap();
        assert_eq!(m.dump.user, saved.user);
    }

    #[test]
    fn failed_write_puts_the_previous_bytes_back() {
        let t = crate::devices::fake::FakeTransport::new(|_| vec![]);
        let mut m = mouse(1);
        let saved = m.dump.clone();
        let err = write_checked(&mut m, &t, &saved, edits).unwrap_err();
        assert!(err.contains("se dejó como estaba"), "{err}");
        assert_eq!(m.dump.user, saved.user);
        assert_eq!(m.writes, 2);
        // When the undo also fails, the user is told how to recover.
        let mut m = mouse(2);
        let err = write_checked(&mut m, &t, &saved, edits).unwrap_err();
        assert!(err.contains("Restaurar perfil original"), "{err}");
    }

    #[test]
    fn rejected_images_write_nothing() {
        let t = crate::devices::fake::FakeTransport::new(|_| vec![]);
        let mut m = mouse(0);
        let saved = m.dump.clone();
        let left = |d: &ProfileDump, _: &ProfileDump| {
            onboard::apply_edits(
                &d.user,
                d.sector,
                &d.directory,
                &d.limits(),
                &buttons(vec![(0, Action::Mouse(0x02))]),
            )
        };
        assert_eq!(
            write_checked(&mut m, &t, &saved, left),
            Err("el botón izquierdo no se puede cambiar.".into())
        );
        assert_eq!(m.writes, 0);
    }

    fn juego() -> MouseProfile {
        MouseProfile {
            name: "Juego".into(),
            dpi: Some(1600),
            light: Some("respiracion #FF0000 6000".into()),
            buttons: vec!["3:80020106".into()],
        }
    }

    #[test]
    fn using_a_profile_writes_it_once_and_checks_it() {
        let t = crate::devices::fake::FakeTransport::new(|_| vec![]);
        let mut m = mouse(0);
        let saved = m.dump.clone();
        let p = juego();
        assert!(!holds(&p, &m.dump));
        write_checked(&mut m, &t, &saved, |d, _| profile_image(&p, d)).unwrap();
        assert_eq!(m.writes, 1);
        assert!(holds(&p, &m.dump));
        assert_eq!(onboard::parse_dpi_cycle(&m.dump.user), Some([1600; 4]));
        assert_eq!(
            onboard::parse_buttons(&m.dump.user, 6).unwrap()[3],
            Action::Keyboard {
                modifiers: 1,
                usage: 6
            }
        );
        // The profile read back from the mouse is the same profile.
        let back = MouseProfile::from_sector("Juego", &m.dump.user, 6).unwrap();
        assert_eq!(back.dpi, p.dpi);
        assert_eq!(back.light, p.light);
    }

    #[test]
    fn a_failed_profile_write_leaves_the_previous_profile() {
        let t = crate::devices::fake::FakeTransport::new(|_| vec![]);
        let mut m = mouse(1);
        let saved = m.dump.clone();
        let p = juego();
        let err = write_checked(&mut m, &t, &saved, |d, _| profile_image(&p, d)).unwrap_err();
        assert!(err.contains("se dejó como estaba"), "{err}");
        assert_eq!(m.dump.user, saved.user);
        assert!(!holds(&p, &m.dump));
    }

    #[test]
    fn a_profile_the_mouse_rejects_is_not_written() {
        let t = crate::devices::fake::FakeTransport::new(|_| vec![]);
        let mut m = mouse(0);
        let saved = m.dump.clone();
        let mut p = juego();
        p.dpi = Some(1625); // not a step of the sensor
        assert_eq!(
            write_checked(&mut m, &t, &saved, |d, _| profile_image(&p, d)),
            Err("ese DPI no lo admite el ratón.".into())
        );
        assert_eq!(m.writes, 0);
    }

    #[test]
    fn hybrid_switch_writes_only_when_buttons_change() {
        let t = crate::devices::fake::FakeTransport::new(|_| vec![]);
        let mut m = mouse(0);
        let saved = m.dump.clone();
        // Only DPI and light differ: nothing to write (they go live).
        let mut live = MouseProfile::from_sector("Juego", &m.dump.user, 6).unwrap();
        live.dpi = Some(1600);
        live.light = Some("fija #FF0000".into());
        assert!(!buttons_differ(&live, &m.dump));
        // A different button: one verified write, then nothing left.
        let p = juego();
        assert!(buttons_differ(&p, &m.dump));
        write_checked(&mut m, &t, &saved, |d, _| profile_image(&p, d)).unwrap();
        assert_eq!(m.writes, 1);
        assert!(!buttons_differ(&p, &m.dump));
    }

    #[test]
    fn later_use_replaces_a_pending_one() {
        let (tx, rx) = channel();
        let k = DeviceKey("k".into());
        let other = DeviceKey("o".into());
        tx.send(profiles::use_command(k.clone(), "B".into()))
            .unwrap();
        tx.send(Command::Refresh).unwrap();
        tx.send(profiles::use_command(other.clone(), "X".into()))
            .unwrap();
        tx.send(profiles::use_command(k.clone(), "C".into()))
            .unwrap();
        let (name, rest) = latest_use(&rx, &k, "A".into());
        assert_eq!(name, "C");
        assert_eq!(
            rest,
            vec![Command::Refresh, profiles::use_command(other, "X".into())]
        );
    }

    #[test]
    fn adding_profiles_checks_names_and_the_limit() {
        let mut s = crate::config::DeviceSettings::default();
        assert_eq!(add_profile(&mut s, " Juego ", juego()), Ok("Juego".into()));
        assert!(
            add_profile(&mut s, "juego", juego())
                .unwrap_err()
                .contains("ya hay un perfil")
        );
        for n in 2..=profiles::MAX_PROFILES {
            add_profile(&mut s, &format!("P{n}"), juego()).unwrap();
        }
        assert!(
            add_profile(&mut s, "Otro", juego())
                .unwrap_err()
                .contains("Elimina uno")
        );
        assert_eq!(s.profiles.len(), profiles::MAX_PROFILES);
    }

    #[test]
    fn coalescing_keeps_other_commands() {
        let (tx, rx) = channel();
        tx.send(Command::Rescan).unwrap();
        tx.send(Command::Try(DeviceKey("k".into()), Change::Dpi(800)))
            .unwrap();
        tx.send(Command::Rescan).unwrap();
        assert_eq!(
            skip_rescans(&rx),
            Some(Command::Try(DeviceKey("k".into()), Change::Dpi(800)))
        );
        // A later Rescan alone is absorbed: the caller rescans anyway.
        assert_eq!(skip_rescans(&rx), None);

        tx.send(Command::Quit).unwrap();
        assert_eq!(skip_rescans(&rx), Some(Command::Quit));
    }
}
