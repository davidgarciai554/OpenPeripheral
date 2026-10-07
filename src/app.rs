//! Background worker: owns all HID I/O so the UI thread never blocks.
//!
//! The UI sends [`Command`]s; the worker updates [`Shared`] and calls
//! `notify` so the UI can redraw. Between commands it only wakes up every
//! [`POLL`] to refresh battery levels.

use crate::config::{Config, parse_hex};
use crate::devices::{Change, Device, DeviceInfo, DeviceKey, Error, ProfileDump, State, Transport};
use crate::platform::hid::{HidTransport, scan};
use crate::protocol::Battery;
use crate::protocol::onboard::{self, Action, EditError, ProfileImage};
use crate::protocol::razer::Rgb;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
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
    /// Button entries of the factory (ROM) profile.
    Factory,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Rescan,
    Refresh,
    Apply(DeviceKey, Change),
    Restore(DeviceKey),
    /// Reads the onboard profile of a mouse (read only).
    ReadProfile(DeviceKey),
    /// Reassigns buttons (index, action) in the mouse's memory.
    WriteButtons(DeviceKey, Vec<(usize, Action)>, Confirmed),
    RestoreProfile(DeviceKey, Source, Confirmed),
    Quit,
}

/// What the UI knows about a mouse's onboard profile.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Profile {
    #[default]
    NotRead,
    /// No onboard profiles, or a format this app has not verified.
    Unsupported,
    Failed(String),
    Buttons(Vec<Action>),
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
    /// Last colour this app applied (devices cannot report theirs).
    pub applied_color: Option<Rgb>,
    /// DPI read when the device was first seen ("Restaurar originales").
    pub original_dpi: Option<u16>,
    pub profile: Profile,
    /// Original profile copy on this PC: `Ok(date)` or the reason it failed.
    pub backup: Option<std::result::Result<String, String>>,
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
}

struct Entry {
    device: Option<Box<dyn Device>>,
    snapshot: Snapshot,
    /// DPI read the first time the device was seen, before re-applying.
    original_dpi: Option<u16>,
    /// Last onboard-profile read (raw sectors stay inside the worker).
    profile: Option<std::result::Result<crate::devices::ProfileDump, Error>>,
    backup: Option<std::result::Result<String, String>>,
}

impl Entry {
    fn profile_view(&self) -> Profile {
        match &self.profile {
            None => Profile::NotRead,
            Some(Err(Error::Unsupported)) => Profile::Unsupported,
            Some(Err(e)) => Profile::Failed(e.to_string()),
            Some(Ok(d)) => {
                match crate::protocol::onboard::parse_buttons(&d.user, d.info.button_count) {
                    Some(b) => Profile::Buttons(b),
                    None => Profile::Failed("perfil incompleto".into()),
                }
            }
        }
    }
}

type Build = fn(&ProfileDump, &ProfileDump) -> std::result::Result<ProfileImage, EditError>;

fn edit_error(e: EditError) -> String {
    match e {
        EditError::LeftButton => "el botón izquierdo no se puede cambiar.",
        EditError::NoSuchButton => "ese botón no existe en este ratón.",
        EditError::NotAssignable => "esa acción no se puede asignar.",
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
    let undo = onboard::restore_image(&before.user, &before.user, before.sector, &before.directory)
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

pub struct Worker {
    api: Option<hidapi::HidApi>,
    entries: Vec<Entry>,
    config: Config,
    shared: Arc<Mutex<Shared>>,
    notify: Box<dyn Fn() + Send>,
    conflicts: fn() -> Vec<&'static str>,
}

impl Worker {
    pub fn new(
        shared: Arc<Mutex<Shared>>,
        notify: Box<dyn Fn() + Send>,
        conflicts: fn() -> Vec<&'static str>,
    ) -> Self {
        Self {
            api: hidapi::HidApi::new().ok(),
            entries: Vec::new(),
            config: Config::load(),
            shared,
            notify,
            conflicts,
        }
    }

    pub fn run(mut self, rx: Receiver<Command>) {
        self.rescan(None);
        // A command that arrived while coalescing device notifications.
        let mut next: Option<Command> = None;
        loop {
            let cmd = match next.take() {
                Some(c) => Ok(c),
                None => rx.recv_timeout(POLL),
            };
            match cmd {
                Ok(Command::Rescan) => {
                    sleep(SETTLE);
                    next = skip_rescans(&rx);
                    self.rescan(None);
                }
                // A full rescan also finds wireless devices that were switched
                // on behind an already present receiver (no OS notification).
                Err(RecvTimeoutError::Timeout) => self.rescan(None),
                Ok(Command::Refresh) => self.rescan(Some(Message {
                    ok: true,
                    text: "Lista de dispositivos actualizada.".into(),
                })),
                Ok(Command::Apply(key, change)) => {
                    if let Some(i) = self.index(&key) {
                        self.apply(i, change)
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
                Ok(Command::WriteButtons(key, edits, Confirmed(()))) => {
                    if let Some(i) = self.index(&key) {
                        self.write_profile(i, "Botones guardados en el ratón.", |d, _| {
                            onboard::apply_buttons(
                                &d.user,
                                d.sector,
                                &d.directory,
                                d.info.button_count,
                                &edits,
                            )
                        })
                    }
                }
                Ok(Command::RestoreProfile(key, source, Confirmed(()))) => {
                    if let Some(i) = self.index(&key) {
                        let (done, build): (&str, Build) = match source {
                            Source::Original => ("Perfil original restaurado.", |d, saved| {
                                if saved.sector != d.sector || saved.info != d.info {
                                    return Err(EditError::BadProfile);
                                }
                                onboard::restore_image(&saved.user, &d.user, d.sector, &d.directory)
                            }),
                            Source::Factory => ("Botones de fábrica restaurados.", |d, _| {
                                onboard::factory_buttons(
                                    &d.user,
                                    &d.rom,
                                    d.sector,
                                    &d.directory,
                                    d.info.button_count,
                                )
                            }),
                        };
                        self.write_profile(i, done, build)
                    }
                }
                Ok(Command::Quit) | Err(RecvTimeoutError::Disconnected) => break,
            }
        }
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
                let saved = self.config.get(&e.snapshot.info.key);
                Snapshot {
                    saved: saved.is_some(),
                    applied_color: saved.and_then(|c| c.color.as_deref()).and_then(parse_hex),
                    original_dpi: e.original_dpi,
                    profile: e.profile_view(),
                    backup: e.backup.clone(),
                    ..e.snapshot.clone()
                }
            })
            .collect();
        s.conflicts = (self.conflicts)();
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
                if e.original_dpi.is_none() {
                    e.original_dpi = state.dpi.as_ref().map(|d| d.current);
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
        let mut found: HashMap<DeviceKey, Box<dyn Device>> = scan(api)
            .into_iter()
            .map(|d| (d.info().key.clone(), d))
            .collect();
        for e in &mut self.entries {
            e.device = found.remove(&e.snapshot.info.key);
            if e.device.is_none() {
                e.snapshot.status = Status::Disconnected;
            }
        }
        for (_, d) in found {
            let info = d.info().clone();
            self.entries.push(Entry {
                device: Some(d),
                snapshot: Snapshot {
                    info,
                    status: Status::Searching,
                    last_battery: None,
                    saved: false,
                    applied_color: None,
                    original_dpi: None,
                    profile: Profile::NotRead,
                    backup: None,
                },
                original_dpi: None,
                profile: None,
                backup: None,
            });
        }
        self.entries.sort_by(|a, b| {
            (a.snapshot.info.vendor, &a.snapshot.info.name)
                .cmp(&(b.snapshot.info.vendor, &b.snapshot.info.name))
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
            if was_absent && matches!(self.entries[i].snapshot.status, Status::Ok(_)) {
                if let Err(e) = self.reapply(i) {
                    failures.push(e);
                }
            }
        }
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

    /// Re-applies saved settings; the error text names the device.
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
        e.profile = Some(read);
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
    fn write_profile(
        &mut self,
        i: usize,
        done: &str,
        build: impl FnOnce(&ProfileDump, &ProfileDump) -> std::result::Result<ProfileImage, EditError>,
    ) {
        let name = self.entries[i].snapshot.info.name.clone();
        let message = match self.try_write(i, build) {
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
        {
            let mut s = self.shared.lock().unwrap_or_else(|p| p.into_inner());
            s.writes += 1;
            s.write_result = Some(message.clone());
        }
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

    fn apply(&mut self, i: usize, change: Change) {
        let Some(api) = &self.api else { return };
        let Some(e) = self.entries.get_mut(i) else {
            return;
        };
        let name = e.snapshot.info.name.clone();
        let Some(dev) = e.device.as_mut() else {
            self.publish(Some(Message {
                ok: false,
                text: format!("{name} está desconectado."),
            }));
            return;
        };
        let result = HidTransport::open(api, dev.path()).and_then(|t| dev.apply(&t, change));
        let message = match result {
            Ok(()) => {
                let key = e.snapshot.info.key.clone();
                self.config.record(&key, change);
                let saved = self.config.save();
                let what = match change {
                    Change::Dpi(d) => format!("DPI {d}"),
                    Change::Color(c) => format!("color {}", crate::config::to_hex(c)),
                };
                match saved {
                    Ok(()) => Message {
                        ok: true,
                        text: format!("{name}: {what} aplicado y guardado."),
                    },
                    Err(err) => Message {
                        ok: false,
                        text: format!("{name}: {what} aplicado, pero no se pudo guardar ({err})."),
                    },
                }
            }
            Err(err) => Message {
                ok: false,
                text: format!("{name}: no se pudo aplicar ({err})."),
            },
        };
        self.read(i);
        self.publish(Some(message));
    }

    /// Restores the DPI read when the device was first seen and forgets the
    /// saved settings. Colours are volatile and return on reconnection.
    fn restore(&mut self, i: usize) {
        let Some(api) = &self.api else { return };
        let Some(e) = self.entries.get_mut(i) else {
            return;
        };
        let name = e.snapshot.info.name.clone();
        self.config.forget(&e.snapshot.info.key);
        let mut ok = self.config.save().is_ok();
        let mut text = format!("{name}: ajustes guardados eliminados.");
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
            },
            fail,
            writes: 0,
        }
    }

    fn edits(d: &ProfileDump, _: &ProfileDump) -> std::result::Result<ProfileImage, EditError> {
        onboard::apply_buttons(
            &d.user,
            d.sector,
            &d.directory,
            6,
            &[(3, Action::Mouse(0x08))],
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
            onboard::restore_image(&s.user, &d.user, d.sector, &d.directory)
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
            onboard::apply_buttons(
                &d.user,
                d.sector,
                &d.directory,
                6,
                &[(0, Action::Mouse(0x02))],
            )
        };
        assert_eq!(
            write_checked(&mut m, &t, &saved, left),
            Err("el botón izquierdo no se puede cambiar.".into())
        );
        assert_eq!(m.writes, 0);
    }

    #[test]
    fn coalescing_keeps_other_commands() {
        let (tx, rx) = channel();
        tx.send(Command::Rescan).unwrap();
        tx.send(Command::Apply(DeviceKey("k".into()), Change::Dpi(800)))
            .unwrap();
        tx.send(Command::Rescan).unwrap();
        assert_eq!(
            skip_rescans(&rx),
            Some(Command::Apply(DeviceKey("k".into()), Change::Dpi(800)))
        );
        // A later Rescan alone is absorbed: the caller rescans anyway.
        assert_eq!(skip_rescans(&rx), None);

        tx.send(Command::Quit).unwrap();
        assert_eq!(skip_rescans(&rx), Some(Command::Quit));
    }
}
