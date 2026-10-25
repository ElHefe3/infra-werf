use atlas_core::*;
use std::{
    path::PathBuf,
    sync::{Arc, Mutex},
};

fn failure() -> AtlasError {
    AtlasError::new(ErrorCode::IoFailure)
}

#[derive(Clone)]
pub struct Paths {
    pub database: PathBuf,
    pub legacy: Option<PathBuf>,
    pub reports: Option<PathBuf>,
}

impl Paths {
    pub fn from_directory(directory: PathBuf) -> Self {
        Self {
            database: std::env::var_os("ATLAS_NATIVE_DB")
                .map(PathBuf::from)
                .unwrap_or_else(|| directory.join("atlas.sqlite3")),
            legacy: std::env::var_os("ATLAS_LEGACY_DB").map(PathBuf::from),
            reports: std::env::var_os("ATLAS_REPORTS_DIR").map(PathBuf::from),
        }
    }

    fn open(&self) -> Result<Store, AtlasError> {
        if self.database.exists() {
            return Store::open(&self.database);
        }
        if let Some(source) = &self.legacy {
            // Read-only migration creates a separate native file and retains
            // a private recovery copy. Never fall back to empty on failure.
            let recovery = self.database.with_file_name(format!(
                "legacy-recovery-{}.sqlite3",
                ServerId::generate().as_str()
            ));
            migrate_legacy(source, &self.database, &recovery)?;
            Store::open(&self.database)
        } else {
            Store::create(&self.database)
        }
    }
}

#[derive(Default)]
struct Lifecycle {
    active: u32,
    closing: bool,
}

pub struct Workspace {
    paths: Paths,
    store: Mutex<Result<Store, AtlasError>>,
    lifecycle: Mutex<Lifecycle>,
}

impl Workspace {
    pub fn new(paths: Paths) -> Arc<Self> {
        let store = paths.open();
        Arc::new(Self {
            paths,
            store: Mutex::new(store),
            lifecycle: Mutex::new(Lifecycle::default()),
        })
    }

    pub fn status(&self) -> Result<WorkspaceStatus, AtlasError> {
        Ok(WorkspaceStatus {
            contract_version: CONTRACT_VERSION,
            storage_ready: self.store.lock().map_err(|_| failure())?.is_ok(),
            report_import_available: self.paths.reports.is_some(),
            coordinator_available: false, // F05 owns the authenticated listener.
            pending_operations: self.lifecycle.lock().map_err(|_| failure())?.active,
        })
    }

    pub fn begin(self: &Arc<Self>) -> Result<Operation, AtlasError> {
        let mut lifecycle = self.lifecycle.lock().map_err(|_| failure())?;
        if lifecycle.closing {
            return Err(AtlasError::new(ErrorCode::Conflict));
        }
        lifecycle.active += 1;
        Ok(Operation(self.clone()))
    }

    /// The same lock admits operations and closes the app, avoiding a race
    /// between the close event and a queued persistence operation.
    pub fn request_close(&self) -> bool {
        let Ok(mut lifecycle) = self.lifecycle.lock() else {
            return false;
        };
        if lifecycle.active != 0 {
            return false;
        }
        lifecycle.closing = true;
        true
    }

    pub fn with_store<T>(
        &self,
        action: impl FnOnce(&mut Store) -> Result<T, AtlasError>,
    ) -> Result<T, AtlasError> {
        let mut lock = self.store.lock().map_err(|_| failure())?;
        match &mut *lock {
            Ok(store) => action(store),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn retry(&self) -> Result<MapState, AtlasError> {
        let mut lock = self.store.lock().map_err(|_| failure())?;
        if lock.is_err() {
            *lock = self.paths.open();
        }
        match &*lock {
            Ok(store) => store.snapshot(),
            Err(error) => Err(error.clone()),
        }
    }

    pub fn import(&self) -> Result<MapState, AtlasError> {
        let path = self
            .paths
            .reports
            .as_ref()
            .ok_or_else(|| AtlasError::new(ErrorCode::Denied))?;
        self.with_store(|store| {
            store.import_reports(path)?;
            store.snapshot()
        })
    }
}

pub struct Operation(Arc<Workspace>);
impl Drop for Operation {
    fn drop(&mut self) {
        if let Ok(mut lifecycle) = self.0.lifecycle.lock() {
            lifecycle.active -= 1;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn persistence_and_close_admission_share_one_boundary() {
        let directory =
            std::env::temp_dir().join(format!("atlas-shell-{}", ServerId::generate().as_str()));
        let paths = Paths {
            database: directory.join("native.sqlite3"),
            legacy: None,
            reports: None,
        };
        let workspace = Workspace::new(paths.clone());
        let operation = workspace.begin().unwrap();
        assert!(!workspace.request_close());
        let id = workspace
            .with_store(|store| {
                store.add_server(AddServer {
                    name: "Desktop proof".into(),
                    host: "proof.invalid".into(),
                })
            })
            .unwrap();
        drop(operation);
        assert!(workspace.request_close());
        assert!(workspace.begin().is_err());
        drop(workspace);
        let reopened = Workspace::new(paths);
        assert_eq!(
            reopened
                .with_store(|store| store.snapshot())
                .unwrap()
                .servers[0]
                .id,
            id
        );
        drop(reopened);
        std::fs::remove_file(directory.join("native.sqlite3")).unwrap();
        std::fs::remove_dir(directory).unwrap();
    }

    #[test]
    fn failed_startup_stays_visible_without_replacing_input() {
        let paths = Paths {
            database: PathBuf::from("missing-parent-for-shell-proof/native.sqlite3"),
            legacy: Some(PathBuf::from("missing-legacy-proof.sqlite3")),
            reports: None,
        };
        let workspace = Workspace::new(paths.clone());
        assert!(!workspace.status().unwrap().storage_ready);
        assert!(workspace.retry().is_err());
        assert!(!paths.database.exists());
        assert!(workspace.import().is_err());
    }
}
