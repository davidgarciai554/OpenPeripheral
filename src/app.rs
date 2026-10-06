//! Background worker: owns all HID I/O so the UI thread never blocks.
//!
//! The UI sends [`Command`]s; the worker updates [`Shared`] and calls
//! `notify` so the UI can redraw. Between commands it only wakes up every
//! [`POLL`] to refresh battery levels.

use crate::config::{Config, parse_hex};
use crate::devices::{Change, Device, DeviceInfo, DeviceKey, Error, State};
use crate::platform::hid::{HidTransport, scan};
use crate::protocol::Battery;
use crate::protocol::razer::Rgb;
use std::collections::HashMap;
use std::sync::mpsc::{Receiver, RecvTimeoutError};
use std::sync::{Arc, Mutex};
use std::thread::sleep;
use std::time::Duration;

pub const POLL: Duration = Duration::from_secs(60);
/// Device arrival fires several notifications; wait for them to settle.
const SETTLE: Duration = Duration::from_millis(800);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Command {
    Rescan,
    Refresh,
    Apply(DeviceKey, Change),
    Restore(DeviceKey),
    Quit,
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
                },
                original_dpi: None,
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
