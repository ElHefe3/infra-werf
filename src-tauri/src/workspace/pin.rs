//! The owner's InfraWerf PIN (F27/T065): stored as a salted hash in
//! `pin.json` beside the workspace database, never in the database itself.
use super::*;
use atlas_core::pin::{PinGate, PinRecord, PinStatus};
use std::time::Instant;

const FILE: &str = "pin.json";

impl Workspace {
    fn pin_path(&self) -> PathBuf {
        self.paths
            .database
            .parent()
            .unwrap_or_else(|| std::path::Path::new("."))
            .join(FILE)
    }

    /// Reads the stored record. A missing file means no PIN; an unreadable
    /// or malformed one is treated as a set but unusable PIN, so a damaged
    /// file never silently removes the lock.
    pub(super) fn load_pin(path: &std::path::Path) -> PinGate {
        match std::fs::read(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => PinGate::new(None),
            Err(_) => PinGate::new(Some(unusable())),
            Ok(bytes) => PinGate::new(Some(
                serde_json::from_slice::<PinRecord>(&bytes).unwrap_or_else(|_| unusable()),
            )),
        }
    }

    fn lock_pin(&self) -> Result<std::sync::MutexGuard<'_, PinGate>, AtlasError> {
        self.pin.lock().map_err(|_| failure())
    }

    fn save_pin(&self, record: Option<&PinRecord>) -> Result<(), AtlasError> {
        let path = self.pin_path();
        match record {
            None => match std::fs::remove_file(&path) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(failure()),
                _ => Ok(()),
            },
            Some(record) => {
                let directory = path.parent().ok_or_else(failure)?;
                let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| failure())?;
                serde_json::to_writer(&mut file, record).map_err(|_| failure())?;
                file.persist(&path).map_err(|_| failure())?;
                Ok(())
            }
        }
    }

    /// Ok while no PIN is set or it was entered in the last few minutes.
    pub fn require_pin(&self) -> Result<(), AtlasError> {
        self.lock_pin()?.require(Instant::now())
    }

    pub fn pin_status(&self) -> Result<PinStatus, AtlasError> {
        Ok(self.lock_pin()?.status(Instant::now()))
    }

    pub fn pin_unlock(&self, pin: &str) -> Result<PinStatus, AtlasError> {
        self.lock_pin()?.unlock(pin, Instant::now())
    }

    pub fn pin_lock(&self) -> Result<PinStatus, AtlasError> {
        let mut gate = self.lock_pin()?;
        gate.lock();
        Ok(gate.status(Instant::now()))
    }

    pub fn pin_set(&self, current: Option<&str>, pin: &str) -> Result<PinStatus, AtlasError> {
        let mut gate = self.lock_pin()?;
        let previous = gate.record().cloned();
        let record = gate.set(current, pin, Instant::now())?;
        if let Err(error) = self.save_pin(Some(&record)) {
            gate.restore(previous);
            return Err(error);
        }
        Ok(gate.status(Instant::now()))
    }

    pub fn pin_clear(&self, current: &str) -> Result<PinStatus, AtlasError> {
        let mut gate = self.lock_pin()?;
        let previous = gate.record().cloned();
        gate.clear(current, Instant::now())?;
        if let Err(error) = self.save_pin(None) {
            gate.restore(previous);
            return Err(error);
        }
        Ok(gate.status(Instant::now()))
    }
}

/// A record no PIN matches (the owner can remove `pin.json` to reset).
fn unusable() -> PinRecord {
    PinRecord {
        version: 0,
        iterations: 0,
        salt: String::new(),
        hash: String::new(),
    }
}
