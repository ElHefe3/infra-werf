//! The owner's InfraWerf PIN (F27/T065): stored as a salted hash in
//! `pin.json` beside the workspace database, never in the database itself.
//! The count of wrong PINs is kept beside it in `pin-attempts.json`, so
//! restarting InfraWerf does not give back free tries.
use super::*;
use atlas_core::pin::{PinGate, PinRecord, PinStatus};
use std::time::Instant;

const FILE: &str = "pin.json";
const ATTEMPTS_FILE: &str = "pin-attempts.json";

#[derive(serde::Serialize, serde::Deserialize)]
struct Attempts {
    failures: u32,
}

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
    /// file never silently removes the lock. Wrong PINs counted before the
    /// last restart still count.
    pub(super) fn load_pin(path: &std::path::Path) -> PinGate {
        let record = match std::fs::read(path) {
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
            Err(_) => Some(unusable()),
            Ok(bytes) => {
                Some(serde_json::from_slice::<PinRecord>(&bytes).unwrap_or_else(|_| unusable()))
            }
        };
        let failures = std::fs::read(path.with_file_name(ATTEMPTS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice::<Attempts>(&bytes).ok())
            .map_or(0, |attempts| attempts.failures);
        PinGate::with_failures(record, failures, Instant::now())
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
            Some(record) => write_json(&path, record),
        }
    }

    /// Keeps the wrong-PIN count when it changed. Best effort: a failed write
    /// leaves the count in memory, which still applies until a restart.
    fn save_attempts(&self, before: u32, gate: &PinGate) {
        let failures = gate.failures();
        if failures == before {
            return;
        }
        let path = self.pin_path().with_file_name(ATTEMPTS_FILE);
        if failures == 0 {
            let _ = std::fs::remove_file(&path);
        } else {
            let _ = write_json(&path, &Attempts { failures });
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
        let mut gate = self.lock_pin()?;
        let before = gate.failures();
        let result = gate.unlock(pin, Instant::now());
        self.save_attempts(before, &gate);
        result
    }

    /// Locks now and, while a PIN is set, closes every open Files, Terminal
    /// and workload-details session, so nothing protected stays usable.
    /// Application windows and their tunnels are closed by the caller.
    pub fn pin_lock(&self) -> Result<PinStatus, AtlasError> {
        let status = {
            let mut gate = self.lock_pin()?;
            gate.lock();
            gate.status(Instant::now())
        };
        if status.set {
            self.files.close_all();
        }
        Ok(status)
    }

    pub fn pin_set(&self, current: Option<&str>, pin: &str) -> Result<PinStatus, AtlasError> {
        let mut gate = self.lock_pin()?;
        let previous = gate.record().cloned();
        let before = gate.failures();
        let result = gate.set(current, pin, Instant::now());
        self.save_attempts(before, &gate);
        let record = result?;
        if let Err(error) = self.save_pin(Some(&record)) {
            gate.restore(previous);
            return Err(error);
        }
        Ok(gate.status(Instant::now()))
    }

    pub fn pin_clear(&self, current: &str) -> Result<PinStatus, AtlasError> {
        let mut gate = self.lock_pin()?;
        let previous = gate.record().cloned();
        let before = gate.failures();
        let result = gate.clear(current, Instant::now());
        self.save_attempts(before, &gate);
        result?;
        if let Err(error) = self.save_pin(None) {
            gate.restore(previous);
            return Err(error);
        }
        Ok(gate.status(Instant::now()))
    }
}

/// Writes JSON through a temporary file, so a crash never leaves half a file.
fn write_json(path: &std::path::Path, value: &impl serde::Serialize) -> Result<(), AtlasError> {
    let directory = path.parent().ok_or_else(failure)?;
    let mut file = tempfile::NamedTempFile::new_in(directory).map_err(|_| failure())?;
    serde_json::to_writer(&mut file, value).map_err(|_| failure())?;
    file.persist(path).map_err(|_| failure())?;
    Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wrong_pins_are_kept_across_a_restart() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join(FILE);
        let record = atlas_core::pin::new_record("2468").unwrap();
        std::fs::write(&path, serde_json::to_vec(&record).unwrap()).unwrap();
        std::fs::write(directory.path().join(ATTEMPTS_FILE), br#"{"failures":5}"#).unwrap();
        let gate = Workspace::load_pin(&path);
        assert_eq!(gate.failures(), 5);
        assert!(gate.status(Instant::now()).retry_after_seconds.is_some());
        // No PIN: a leftover count is ignored.
        std::fs::remove_file(&path).unwrap();
        assert_eq!(Workspace::load_pin(&path).failures(), 0);
    }
}
