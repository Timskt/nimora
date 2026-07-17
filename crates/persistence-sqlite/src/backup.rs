use crate::{SqlitePersistenceError, SqlitePetRepository, verify_database_file};
use serde::{Deserialize, Serialize};
use std::{
    fs,
    path::{Path, PathBuf},
    time::{Duration, SystemTime, UNIX_EPOCH},
};

const BACKUP_PREFIX: &str = "runtime-";
const BACKUP_SUFFIX: &str = ".sqlite3";
const PENDING_RESTORE_FILE: &str = "restore-pending.json";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BackupPolicy {
    pub interval: Duration,
    pub retain: usize,
}

impl Default for BackupPolicy {
    fn default() -> Self {
        Self {
            interval: Duration::from_hours(6),
            retain: 12,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRecord {
    pub id: String,
    pub created_at_ms: u64,
    pub bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupHealth {
    pub due: bool,
    pub latest: Option<BackupRecord>,
    pub available: Vec<BackupRecord>,
    pub pending_restore: Option<String>,
    pub last_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PendingRestore {
    pub spec: String,
    pub backup_id: String,
}

#[derive(Debug, Clone)]
pub struct BackupCoordinator {
    database_path: PathBuf,
    backup_directory: PathBuf,
    policy: BackupPolicy,
}

impl BackupCoordinator {
    #[must_use]
    pub fn new(
        database_path: impl Into<PathBuf>,
        backup_directory: impl Into<PathBuf>,
        policy: BackupPolicy,
    ) -> Self {
        Self {
            database_path: database_path.into(),
            backup_directory: backup_directory.into(),
            policy,
        }
    }

    /// Creates, verifies, atomically publishes, and prunes a consistent backup.
    ///
    /// # Errors
    ///
    /// Returns an error when the source cannot be backed up, verification fails,
    /// or the backup directory cannot be updated safely.
    pub fn create_now(&self) -> Result<BackupRecord, SqlitePersistenceError> {
        fs::create_dir_all(&self.backup_directory)?;
        let (created_at_ms, id) = self.next_backup_id(epoch_millis(SystemTime::now())?);
        let temporary = self.backup_directory.join(format!(".{id}.partial"));
        let destination = self.backup_directory.join(&id);
        let _ = fs::remove_file(&temporary);
        let result = (|| {
            SqlitePetRepository::open(&self.database_path)?.backup_to(&temporary)?;
            verify_database(&temporary)?;
            fs::File::open(&temporary)?.sync_all()?;
            fs::rename(&temporary, &destination)?;
            sync_directory(&self.backup_directory)?;
            self.prune()?;
            record_for_path(&destination, id, created_at_ms)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&temporary);
        }
        result
    }

    /// Returns backup availability and whether the configured interval elapsed.
    ///
    /// # Errors
    ///
    /// Returns an error when backup metadata cannot be read safely.
    pub fn health(&self) -> Result<BackupHealth, SqlitePersistenceError> {
        let available = self.list()?;
        let latest = available.first().cloned();
        let now = epoch_millis(SystemTime::now())?;
        let due = latest.as_ref().is_none_or(|record| {
            now.saturating_sub(record.created_at_ms)
                >= u64::try_from(self.policy.interval.as_millis()).unwrap_or(u64::MAX)
        });
        let pending_restore =
            read_pending_restore(&self.backup_directory)?.map(|pending| pending.backup_id);
        Ok(BackupHealth {
            due,
            latest,
            available,
            pending_restore,
            last_error: None,
        })
    }

    /// Creates a backup only when the configured interval has elapsed.
    ///
    /// # Errors
    ///
    /// Returns an error when health inspection or backup creation fails.
    pub fn create_if_due(&self) -> Result<Option<BackupRecord>, SqlitePersistenceError> {
        self.health()?.due.then(|| self.create_now()).transpose()
    }

    /// Records a verified backup to restore before the next database open.
    ///
    /// # Errors
    ///
    /// Returns an error for unknown identifiers, corrupt backups, or failed writes.
    pub fn request_restore(&self, backup_id: &str) -> Result<(), SqlitePersistenceError> {
        let backup = safe_backup_path(&self.backup_directory, backup_id)?;
        verify_database(&backup)?;
        let pending = serde_json::to_vec_pretty(&PendingRestore {
            spec: "nimora.restore-request/1".to_owned(),
            backup_id: backup_id.to_owned(),
        })?;
        let temporary = self.backup_directory.join(".restore-pending.partial");
        fs::write(&temporary, pending)?;
        fs::File::open(&temporary)?.sync_all()?;
        replace_file(
            &temporary,
            &self.backup_directory.join(PENDING_RESTORE_FILE),
        )?;
        sync_directory(&self.backup_directory)
    }

    fn list(&self) -> Result<Vec<BackupRecord>, SqlitePersistenceError> {
        if !self.backup_directory.exists() {
            return Ok(Vec::new());
        }
        let mut records = Vec::new();
        for entry in fs::read_dir(&self.backup_directory)? {
            let entry = entry?;
            let name = entry.file_name();
            let Some(id) = name.to_str() else { continue };
            let Some(created_at_ms) = parse_backup_id(id) else {
                continue;
            };
            if entry.file_type()?.is_file() {
                records.push(record_for_path(
                    &entry.path(),
                    id.to_owned(),
                    created_at_ms,
                )?);
            }
        }
        records.sort_by_key(|record| std::cmp::Reverse(record.created_at_ms));
        Ok(records)
    }

    fn next_backup_id(&self, mut created_at_ms: u64) -> (u64, String) {
        loop {
            let id = format!("{BACKUP_PREFIX}{created_at_ms}{BACKUP_SUFFIX}");
            if !self.backup_directory.join(&id).exists() {
                return (created_at_ms, id);
            }
            created_at_ms = created_at_ms.saturating_add(1);
        }
    }

    fn prune(&self) -> Result<(), SqlitePersistenceError> {
        let pending =
            read_pending_restore(&self.backup_directory)?.map(|request| request.backup_id);
        for record in self.list()?.into_iter().skip(self.policy.retain.max(1)) {
            if pending.as_deref() == Some(record.id.as_str()) {
                continue;
            }
            fs::remove_file(self.backup_directory.join(record.id))?;
        }
        sync_directory(&self.backup_directory)
    }
}

/// Applies a previously requested restore before any database connection opens.
///
/// # Errors
///
/// Returns an error without deleting the current database when the request or
/// backup is invalid, or restores the previous database if activation fails.
pub fn apply_pending_restore(
    database_path: &Path,
    backup_directory: &Path,
) -> Result<Option<String>, SqlitePersistenceError> {
    let Some(pending) = read_pending_restore(backup_directory)? else {
        return Ok(None);
    };
    if pending.spec != "nimora.restore-request/1" {
        return Err(SqlitePersistenceError::InvalidBackupRequest);
    }
    let backup = safe_backup_path(backup_directory, &pending.backup_id)?;
    verify_database(&backup)?;
    let staged = database_path.with_extension("sqlite3.restore-staged");
    let rollback = database_path.with_extension("sqlite3.restore-rollback");
    let _ = fs::remove_file(&staged);
    let _ = fs::remove_file(&rollback);
    fs::copy(&backup, &staged)?;
    fs::File::open(&staged)?.sync_all()?;
    verify_database(&staged)?;
    if database_path.exists() {
        fs::rename(database_path, &rollback)?;
    }
    if let Err(error) = fs::rename(&staged, database_path) {
        if rollback.exists() {
            let _ = fs::rename(&rollback, database_path);
        }
        return Err(error.into());
    }
    let _ = fs::remove_file(&rollback);
    for suffix in ["-wal", "-shm"] {
        let _ = fs::remove_file(path_with_suffix(database_path, suffix));
    }
    fs::remove_file(backup_directory.join(PENDING_RESTORE_FILE))?;
    if let Some(parent) = database_path.parent() {
        sync_directory(parent)?;
    }
    Ok(Some(pending.backup_id))
}

fn verify_database(path: &Path) -> Result<(), SqlitePersistenceError> {
    verify_database_file(path)
}

fn read_pending_restore(
    backup_directory: &Path,
) -> Result<Option<PendingRestore>, SqlitePersistenceError> {
    let path = backup_directory.join(PENDING_RESTORE_FILE);
    if !path.exists() {
        return Ok(None);
    }
    let pending: PendingRestore = serde_json::from_slice(&fs::read(path)?)?;
    Ok(Some(pending))
}

fn safe_backup_path(directory: &Path, id: &str) -> Result<PathBuf, SqlitePersistenceError> {
    parse_backup_id(id).ok_or(SqlitePersistenceError::InvalidBackupRequest)?;
    let path = directory.join(id);
    if !path.is_file() {
        return Err(SqlitePersistenceError::InvalidBackupRequest);
    }
    Ok(path)
}

fn parse_backup_id(id: &str) -> Option<u64> {
    id.strip_prefix(BACKUP_PREFIX)?
        .strip_suffix(BACKUP_SUFFIX)?
        .parse()
        .ok()
}

fn record_for_path(
    path: &Path,
    id: String,
    created_at_ms: u64,
) -> Result<BackupRecord, SqlitePersistenceError> {
    Ok(BackupRecord {
        id,
        created_at_ms,
        bytes: fs::metadata(path)?.len(),
    })
}

fn epoch_millis(time: SystemTime) -> Result<u64, SqlitePersistenceError> {
    u64::try_from(time.duration_since(UNIX_EPOCH)?.as_millis())
        .map_err(|_| SqlitePersistenceError::InvalidBackupRequest)
}

fn sync_directory(path: &Path) -> Result<(), SqlitePersistenceError> {
    #[cfg(unix)]
    fs::File::open(path)?.sync_all()?;
    #[cfg(not(unix))]
    let _ = path;
    Ok(())
}

fn replace_file(source: &Path, destination: &Path) -> Result<(), SqlitePersistenceError> {
    let previous = destination.with_extension("json.previous");
    let _ = fs::remove_file(&previous);
    if destination.exists() {
        fs::rename(destination, &previous)?;
    }
    if let Err(error) = fs::rename(source, destination) {
        if previous.exists() {
            let _ = fs::rename(&previous, destination);
        }
        return Err(error.into());
    }
    let _ = fs::remove_file(previous);
    Ok(())
}

fn path_with_suffix(path: &Path, suffix: &str) -> PathBuf {
    let mut value = path.as_os_str().to_owned();
    value.push(suffix);
    PathBuf::from(value)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::SqlitePetRepository;
    use nimora_runtime_app::{RuntimeEventBus, RuntimeService};
    use nimora_runtime_core::{PetAction, PetState};

    fn test_root(name: &str) -> PathBuf {
        std::env::temp_dir().join(format!(
            "nimora-backup-{name}-{}",
            epoch_millis(SystemTime::now()).unwrap()
        ))
    }

    #[test]
    fn creates_verified_backups_and_enforces_retention() {
        let root = test_root("retention");
        let database = root.join("runtime.sqlite3");
        let backups = root.join("backups");
        fs::create_dir_all(&root).unwrap();
        RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        let coordinator = BackupCoordinator::new(
            &database,
            &backups,
            BackupPolicy {
                interval: Duration::ZERO,
                retain: 2,
            },
        );
        coordinator.create_now().unwrap();
        coordinator.create_now().unwrap();
        coordinator.create_now().unwrap();
        let health = coordinator.health().unwrap();
        assert_eq!(health.available.len(), 2);
        assert!(health.latest.is_some());
        assert!(health.due);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn requested_restore_is_applied_before_reopening_database() {
        let root = test_root("restore");
        let database = root.join("runtime.sqlite3");
        let backups = root.join("backups");
        fs::create_dir_all(&root).unwrap();
        let service = RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        service.play_action(PetAction::Sleep).unwrap();
        drop(service);
        let coordinator = BackupCoordinator::new(&database, &backups, BackupPolicy::default());
        let backup = coordinator.create_now().unwrap();
        let service = RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        service.play_action(PetAction::Work).unwrap();
        drop(service);
        coordinator.request_restore(&backup.id).unwrap();
        assert_eq!(
            apply_pending_restore(&database, &backups).unwrap(),
            Some(backup.id)
        );
        let restored = RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        assert_eq!(restored.snapshot().unwrap().state, PetState::Sleeping);
        assert!(coordinator.health().unwrap().pending_restore.is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn newly_created_backup_is_not_due_again() {
        let root = test_root("not-due");
        let database = root.join("runtime.sqlite3");
        let backups = root.join("backups");
        fs::create_dir_all(&root).unwrap();
        RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        let coordinator = BackupCoordinator::new(&database, &backups, BackupPolicy::default());
        coordinator.create_now().unwrap();
        assert_eq!(coordinator.create_if_due().unwrap(), None);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn retention_preserves_backup_selected_for_restore() {
        let root = test_root("pending-retention");
        let database = root.join("runtime.sqlite3");
        let backups = root.join("backups");
        fs::create_dir_all(&root).unwrap();
        RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        let coordinator = BackupCoordinator::new(
            &database,
            &backups,
            BackupPolicy {
                interval: Duration::ZERO,
                retain: 1,
            },
        );
        let selected = coordinator.create_now().unwrap();
        coordinator.request_restore(&selected.id).unwrap();
        coordinator.create_now().unwrap();
        coordinator.create_now().unwrap();
        let health = coordinator.health().unwrap();
        assert_eq!(health.pending_restore, Some(selected.id.clone()));
        assert!(
            health
                .available
                .iter()
                .any(|record| record.id == selected.id)
        );
        assert_eq!(health.available.len(), 2);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_pending_request_is_rejected_without_changing_database() {
        let root = test_root("corrupt-request");
        let database = root.join("runtime.sqlite3");
        let backups = root.join("backups");
        fs::create_dir_all(&backups).unwrap();
        let service = RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        service.play_action(PetAction::Work).unwrap();
        drop(service);
        fs::write(backups.join(PENDING_RESTORE_FILE), b"not-json").unwrap();
        assert!(apply_pending_restore(&database, &backups).is_err());
        let reopened = RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        assert_eq!(reopened.snapshot().unwrap().state, PetState::Working);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn corrupt_backup_is_rejected_without_changing_database() {
        let root = test_root("corrupt-backup");
        let database = root.join("runtime.sqlite3");
        let backups = root.join("backups");
        fs::create_dir_all(&root).unwrap();
        let service = RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        service.play_action(PetAction::Work).unwrap();
        drop(service);
        let coordinator = BackupCoordinator::new(&database, &backups, BackupPolicy::default());
        let backup = coordinator.create_now().unwrap();
        coordinator.request_restore(&backup.id).unwrap();
        fs::write(backups.join(&backup.id), b"corrupt").unwrap();
        assert!(apply_pending_restore(&database, &backups).is_err());
        let reopened = RuntimeService::initialize_with_event_bus(
            SqlitePetRepository::open(&database).unwrap(),
            "Aster",
            RuntimeEventBus::default(),
        )
        .unwrap();
        assert_eq!(reopened.snapshot().unwrap().state, PetState::Working);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restore_rejects_unknown_and_traversal_identifiers() {
        let root = test_root("invalid");
        let coordinator = BackupCoordinator::new(
            root.join("runtime.sqlite3"),
            root.join("backups"),
            BackupPolicy::default(),
        );
        for id in ["../runtime.sqlite3", "runtime-invalid.sqlite3"] {
            assert!(matches!(
                coordinator.request_restore(id),
                Err(SqlitePersistenceError::InvalidBackupRequest)
            ));
        }
    }
}
