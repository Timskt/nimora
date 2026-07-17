//! `SQLite` persistence adapters for the `Nimora` runtime.

use nimora_runtime_app::{
    PetRepository, ProfileRepository, ProfileServiceError, ProfileSnapshot, RepositoryError,
};
use nimora_runtime_core::{Event, Pet};
use rusqlite::{Connection, OptionalExtension, backup::Backup, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex, time::Duration};
use thiserror::Error;

const DATABASE_VERSION: i64 = 1;
const PET_SNAPSHOT_VERSION: u32 = 1;
const PROFILE_SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug)]
pub struct SqlitePetRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug)]
pub struct SqliteProgramPermissionRepository {
    connection: Mutex<Connection>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramPermissionGrant {
    pub program_id: String,
    pub version: String,
    pub capabilities: Vec<String>,
}

impl SqliteProgramPermissionRepository {
    /// Opens the shared application database and initializes its schema.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot open, configure, or migrate the database.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated permission store for tests and ephemeral tools.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot configure or initialize the database.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Replaces the exact capability grant for one installed program version.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identities, duplicate capabilities, or persistence failures.
    pub fn grant(&self, grant: &ProgramPermissionGrant) -> Result<(), SqlitePersistenceError> {
        let capabilities = canonical_capabilities(grant)?;
        let payload = serde_json::to_string(&capabilities)?;
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "INSERT INTO user_program_permission_grant
                    (program_id, program_version, capabilities)
                 VALUES (?1, ?2, ?3)
                 ON CONFLICT(program_id, program_version) DO UPDATE SET
                    capabilities = excluded.capabilities,
                    granted_at = CURRENT_TIMESTAMP",
                params![grant.program_id, grant.version, payload],
            )?;
        Ok(())
    }

    /// Returns whether one version has an exact grant for its full capability set.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid requests, malformed stored data, or persistence failures.
    pub fn is_granted(
        &self,
        grant: &ProgramPermissionGrant,
    ) -> Result<bool, SqlitePersistenceError> {
        let requested = canonical_capabilities(grant)?;
        if requested.is_empty() {
            return Ok(true);
        }
        let payload = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .query_row(
                "SELECT capabilities FROM user_program_permission_grant
                 WHERE program_id = ?1 AND program_version = ?2",
                params![grant.program_id, grant.version],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        let Some(payload) = payload else {
            return Ok(false);
        };
        let mut stored = serde_json::from_str::<Vec<String>>(&payload)?;
        stored.sort_unstable();
        stored.dedup();
        Ok(stored == requested)
    }

    /// Revokes every persisted grant for a program identity.
    ///
    /// # Errors
    ///
    /// Returns an error when the identity is invalid or persistence fails.
    pub fn revoke_program(&self, program_id: &str) -> Result<(), SqlitePersistenceError> {
        if program_id.trim().is_empty() {
            return Err(SqlitePersistenceError::InvalidPermissionGrant);
        }
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "DELETE FROM user_program_permission_grant WHERE program_id = ?1",
                params![program_id],
            )?;
        Ok(())
    }
}

fn canonical_capabilities(
    grant: &ProgramPermissionGrant,
) -> Result<Vec<String>, SqlitePersistenceError> {
    if grant.program_id.trim().is_empty() || grant.version.trim().is_empty() {
        return Err(SqlitePersistenceError::InvalidPermissionGrant);
    }
    let mut capabilities = grant.capabilities.clone();
    if capabilities
        .iter()
        .any(|capability| capability.trim().is_empty())
    {
        return Err(SqlitePersistenceError::InvalidPermissionGrant);
    }
    capabilities.sort_unstable();
    let original_len = capabilities.len();
    capabilities.dedup();
    if capabilities.len() != original_len {
        return Err(SqlitePersistenceError::InvalidPermissionGrant);
    }
    Ok(capabilities)
}

impl SqlitePetRepository {
    /// Opens or creates an `Nimora` database and initializes its schema.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot open, configure, or initialize the
    /// database. A database from a newer application version is rejected.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated in-memory database for tests and ephemeral tools.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot configure or initialize the database.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    /// Creates a consistent online backup, including WAL-backed pages.
    ///
    /// # Errors
    ///
    /// Returns an error when the destination cannot be created or `SQLite`
    /// cannot complete the online backup.
    pub fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), SqlitePersistenceError> {
        backup_connection(&self.connection, destination)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    fn load_snapshot(&self) -> Result<Option<Pet>, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let row = connection
            .query_row(
                "SELECT schema_version, payload FROM pet_snapshot WHERE singleton = 1",
                [],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((schema_version, payload)) = row else {
            return Ok(None);
        };
        if schema_version != PET_SNAPSHOT_VERSION {
            return Err(SqlitePersistenceError::UnsupportedPetSnapshotVersion(
                schema_version,
            ));
        }
        let snapshot: StoredPetSnapshot = serde_json::from_str(&payload)?;
        if snapshot.schema_version != schema_version {
            return Err(SqlitePersistenceError::SnapshotVersionMismatch);
        }
        snapshot.pet.validate()?;
        Ok(Some(snapshot.pet))
    }

    fn save_snapshot(&self, pet: &Pet) -> Result<(), SqlitePersistenceError> {
        pet.validate()?;
        let payload = serde_json::to_string(&StoredPetSnapshot {
            schema_version: PET_SNAPSHOT_VERSION,
            pet: pet.clone(),
        })?;
        save_singleton_snapshot(
            &self.connection,
            "pet_snapshot",
            PET_SNAPSHOT_VERSION,
            &payload,
            None,
        )
    }

    fn save_snapshot_with_event(
        &self,
        pet: &Pet,
        event: &Event,
    ) -> Result<(), SqlitePersistenceError> {
        pet.validate()?;
        let payload = serde_json::to_string(&StoredPetSnapshot {
            schema_version: PET_SNAPSHOT_VERSION,
            pet: pet.clone(),
        })?;
        save_singleton_snapshot(
            &self.connection,
            "pet_snapshot",
            PET_SNAPSHOT_VERSION,
            &payload,
            Some(event),
        )
    }
}

fn prepare_connection(connection: &mut Connection) -> Result<(), SqlitePersistenceError> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    let version = connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
    if version != 0 && version != DATABASE_VERSION {
        return Err(SqlitePersistenceError::UnsupportedDatabaseVersion(version));
    }
    if version == 0 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE pet_snapshot (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    schema_version INTEGER NOT NULL,
                    payload TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE profile_snapshot (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    schema_version INTEGER NOT NULL,
                    payload TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE TABLE event_outbox (
                    event_id TEXT PRIMARY KEY,
                    event_type TEXT NOT NULL,
                    trace_id TEXT NOT NULL,
                    payload TEXT NOT NULL,
                    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                CREATE INDEX event_outbox_created_at_idx
                    ON event_outbox(created_at, event_id);
                CREATE TABLE user_program_permission_grant (
                    program_id TEXT NOT NULL,
                    program_version TEXT NOT NULL,
                    capabilities TEXT NOT NULL,
                    granted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                    PRIMARY KEY (program_id, program_version)
                );
                PRAGMA user_version = 1;",
        )?;
        transaction.commit()?;
    }
    Ok(())
}

fn save_singleton_snapshot(
    connection: &Mutex<Connection>,
    table: &str,
    schema_version: u32,
    payload: &str,
    event: Option<&Event>,
) -> Result<(), SqlitePersistenceError> {
    let mut connection = connection
        .lock()
        .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
    let transaction = connection.transaction()?;
    let statement = format!(
        "INSERT INTO {table} (singleton, schema_version, payload)
         VALUES (1, ?1, ?2)
         ON CONFLICT(singleton) DO UPDATE SET
           schema_version = excluded.schema_version,
           payload = excluded.payload,
           updated_at = CURRENT_TIMESTAMP"
    );
    transaction.execute(&statement, params![schema_version, payload])?;
    if let Some(event) = event {
        insert_outbox_event(&transaction, event)?;
    }
    transaction.commit()?;
    Ok(())
}

fn insert_outbox_event(
    transaction: &rusqlite::Transaction<'_>,
    event: &Event,
) -> Result<(), SqlitePersistenceError> {
    transaction.execute(
        "INSERT INTO event_outbox (event_id, event_type, trace_id, payload)
         VALUES (?1, ?2, ?3, ?4)",
        params![
            event.id.to_string(),
            event.event_type,
            event.trace_id.to_string(),
            serde_json::to_string(event)?,
        ],
    )?;
    Ok(())
}

fn backup_connection(
    source: &Mutex<Connection>,
    destination: impl AsRef<Path>,
) -> Result<(), SqlitePersistenceError> {
    let source = source
        .lock()
        .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
    let mut destination = Connection::open(destination)?;
    let backup = Backup::new(&source, &mut destination)?;
    backup.run_to_completion(128, Duration::from_millis(10), None)?;
    Ok(())
}

#[derive(Debug)]
pub struct SqliteProfileRepository {
    connection: Mutex<Connection>,
}

impl SqliteProfileRepository {
    /// Opens or creates the shared runtime database for profile storage.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` configuration or initialization fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open(path)?;
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Creates a consistent online backup, including WAL-backed pages.
    ///
    /// # Errors
    ///
    /// Returns an error when the destination cannot be created or `SQLite`
    /// cannot complete the online backup.
    pub fn backup_to(&self, destination: impl AsRef<Path>) -> Result<(), SqlitePersistenceError> {
        backup_connection(&self.connection, destination)
    }

    fn load_snapshot(&self) -> Result<Option<ProfileSnapshot>, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let row = connection
            .query_row(
                "SELECT schema_version, payload FROM profile_snapshot WHERE singleton = 1",
                [],
                |row| Ok((row.get::<_, u32>(0)?, row.get::<_, String>(1)?)),
            )
            .optional()?;
        let Some((schema_version, payload)) = row else {
            return Ok(None);
        };
        if schema_version != PROFILE_SNAPSHOT_VERSION {
            return Err(SqlitePersistenceError::UnsupportedProfileSnapshotVersion(
                schema_version,
            ));
        }
        let snapshot: ProfileSnapshot = serde_json::from_str(&payload)?;
        if snapshot.schema_version != schema_version {
            return Err(SqlitePersistenceError::SnapshotVersionMismatch);
        }
        snapshot.validate()?;
        Ok(Some(snapshot))
    }

    fn save_snapshot(&self, snapshot: &ProfileSnapshot) -> Result<(), SqlitePersistenceError> {
        snapshot.validate()?;
        let payload = serde_json::to_string(snapshot)?;
        save_singleton_snapshot(
            &self.connection,
            "profile_snapshot",
            PROFILE_SNAPSHOT_VERSION,
            &payload,
            None,
        )
    }

    fn save_snapshot_with_event(
        &self,
        snapshot: &ProfileSnapshot,
        event: &Event,
    ) -> Result<(), SqlitePersistenceError> {
        snapshot.validate()?;
        let payload = serde_json::to_string(snapshot)?;
        save_singleton_snapshot(
            &self.connection,
            "profile_snapshot",
            PROFILE_SNAPSHOT_VERSION,
            &payload,
            Some(event),
        )
    }
}

impl PetRepository for SqlitePetRepository {
    fn load(&self) -> Result<Option<Pet>, RepositoryError> {
        self.load_snapshot()
            .map_err(|error| RepositoryError::new(error.to_string()))
    }

    fn save(&self, pet: &Pet) -> Result<(), RepositoryError> {
        self.save_snapshot(pet)
            .map_err(|error| RepositoryError::new(error.to_string()))
    }

    fn save_with_event(&self, pet: &Pet, event: &Event) -> Result<(), RepositoryError> {
        self.save_snapshot_with_event(pet, event)
            .map_err(|error| RepositoryError::new(error.to_string()))
    }
}

impl ProfileRepository for SqliteProfileRepository {
    fn load(&self) -> Result<Option<ProfileSnapshot>, RepositoryError> {
        self.load_snapshot()
            .map_err(|error| RepositoryError::new(error.to_string()))
    }

    fn save(&self, snapshot: &ProfileSnapshot) -> Result<(), RepositoryError> {
        self.save_snapshot(snapshot)
            .map_err(|error| RepositoryError::new(error.to_string()))
    }

    fn save_with_event(
        &self,
        snapshot: &ProfileSnapshot,
        event: &Event,
    ) -> Result<(), RepositoryError> {
        self.save_snapshot_with_event(snapshot, event)
            .map_err(|error| RepositoryError::new(error.to_string()))
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredPetSnapshot {
    schema_version: u32,
    pet: Pet,
}

#[derive(Debug, Error)]
pub enum SqlitePersistenceError {
    #[error("SQLite state lock is unavailable")]
    StatePoisoned,
    #[error("database version {0} is newer than this application supports")]
    UnsupportedDatabaseVersion(i64),
    #[error("pet snapshot version {0} is unsupported")]
    UnsupportedPetSnapshotVersion(u32),
    #[error("profile snapshot version {0} is unsupported")]
    UnsupportedProfileSnapshotVersion(u32),
    #[error("user program permission grant is invalid")]
    InvalidPermissionGrant,
    #[error("pet snapshot metadata and payload versions do not match")]
    SnapshotVersionMismatch,
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Pet(#[from] nimora_runtime_core::PetError),
    #[error(transparent)]
    Profile(#[from] ProfileServiceError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_runtime_app::{ProfileService, RuntimeEventBus, RuntimeService};
    use nimora_runtime_core::{Event, EventSource, PetAction, PetState};
    use std::time::{SystemTime, UNIX_EPOCH};

    #[test]
    fn round_trips_a_versioned_pet_snapshot() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        let mut pet = Pet::new("Aster").expect("pet");
        pet.apply_action(PetAction::Sleep);
        repository.save_snapshot(&pet).expect("save");
        let restored = repository.load_snapshot().expect("load").expect("snapshot");
        assert_eq!(restored.id, pet.id);
        assert_eq!(restored.state, PetState::Sleeping);
    }

    #[test]
    fn rejects_future_snapshot_versions() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        repository.connection.lock().expect("lock").execute(
            "INSERT INTO pet_snapshot (singleton, schema_version, payload) VALUES (1, 99, '{}')",
            [],
        ).expect("fixture");
        assert!(matches!(
            repository.load_snapshot(),
            Err(SqlitePersistenceError::UnsupportedPetSnapshotVersion(99))
        ));
    }

    #[test]
    fn initializes_schema_once() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        let version = repository
            .connection
            .lock()
            .expect("lock")
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("version");
        assert_eq!(version, DATABASE_VERSION);
    }

    #[test]
    fn permission_grants_are_bound_to_program_version_and_exact_capabilities() {
        let repository = SqliteProgramPermissionRepository::in_memory().expect("database");
        let grant = ProgramPermissionGrant {
            program_id: "studio.example.focus".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: vec![
                "invoke-safe-commands".to_owned(),
                "read-pet-state".to_owned(),
            ],
        };
        assert!(!repository.is_granted(&grant).expect("check"));
        repository.grant(&grant).expect("grant");
        assert!(repository.is_granted(&grant).expect("check"));
        assert!(
            !repository
                .is_granted(&ProgramPermissionGrant {
                    version: "2.0.0".to_owned(),
                    ..grant.clone()
                })
                .expect("version check")
        );
        assert!(
            !repository
                .is_granted(&ProgramPermissionGrant {
                    capabilities: vec!["read-pet-state".to_owned()],
                    ..grant
                })
                .expect("capability check")
        );
    }

    #[test]
    fn revoking_a_program_removes_all_version_grants() {
        let repository = SqliteProgramPermissionRepository::in_memory().expect("database");
        for version in ["1.0.0", "2.0.0"] {
            repository
                .grant(&ProgramPermissionGrant {
                    program_id: "studio.example.focus".to_owned(),
                    version: version.to_owned(),
                    capabilities: vec!["read-pet-state".to_owned()],
                })
                .expect("grant");
        }
        repository
            .revoke_program("studio.example.focus")
            .expect("revoke");
        assert!(
            !repository
                .is_granted(&ProgramPermissionGrant {
                    program_id: "studio.example.focus".to_owned(),
                    version: "1.0.0".to_owned(),
                    capabilities: vec!["read-pet-state".to_owned()],
                })
                .expect("check")
        );
    }

    #[test]
    fn atomically_persists_pet_state_and_event() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        let mut pet = Pet::new("Aster").expect("pet");
        pet.apply_action(PetAction::Sleep);
        let event = Event::new(
            "pet.state.changed",
            EventSource::Core,
            serde_json::json!({ "state": "sleeping" }),
        )
        .expect("event");

        repository.save_with_event(&pet, &event).expect("save");

        let connection = repository.connection.lock().expect("lock");
        let (event_type, trace_id, payload): (String, String, String) = connection
            .query_row(
                "SELECT event_type, trace_id, payload FROM event_outbox WHERE event_id = ?1",
                params![event.id.to_string()],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .expect("outbox row");
        let stored_event: Event = serde_json::from_str(&payload).expect("event payload");
        assert_eq!(event_type, event.event_type);
        assert_eq!(trace_id, event.trace_id.to_string());
        assert_eq!(stored_event, event);
    }

    #[test]
    fn duplicate_event_rolls_back_snapshot_update() {
        let repository = SqlitePetRepository::in_memory().expect("database");
        let original = Pet::new("Aster").expect("pet");
        let event = Event::new(
            "pet.state.changed",
            EventSource::Core,
            serde_json::json!({ "state": "idle" }),
        )
        .expect("event");
        repository
            .save_with_event(&original, &event)
            .expect("initial save");
        let mut candidate = original.clone();
        candidate.apply_action(PetAction::Sleep);

        assert!(repository.save_with_event(&candidate, &event).is_err());
        assert_eq!(
            repository
                .load_snapshot()
                .expect("load")
                .expect("snapshot")
                .state,
            PetState::Idle
        );
    }

    #[test]
    fn restores_state_after_runtime_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "nimora-persistence-{}-{unique}.sqlite3",
            std::process::id()
        ));
        {
            let repository = SqlitePetRepository::open(&path).expect("database");
            let service = RuntimeService::initialize(repository, "Aster").expect("runtime");
            service
                .play_action(PetAction::Sleep)
                .expect("persisted action");
        }
        {
            let repository = SqlitePetRepository::open(&path).expect("database");
            let service = RuntimeService::initialize(repository, "Ignored").expect("runtime");
            assert_eq!(
                service.snapshot().expect("snapshot").state,
                PetState::Sleeping
            );
        }
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn online_backup_restores_wal_backed_runtime_state() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let source = std::env::temp_dir().join(format!(
            "nimora-backup-source-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let destination = std::env::temp_dir().join(format!(
            "nimora-backup-destination-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let mut pet = Pet::new("Aster").expect("pet");
        pet.apply_action(PetAction::Sleep);
        {
            let repository = SqlitePetRepository::open(&source).expect("source");
            repository.save(&pet).expect("save");
            repository.backup_to(&destination).expect("backup");
        }
        let restored = SqlitePetRepository::open(&destination)
            .expect("destination")
            .load_snapshot()
            .expect("load")
            .expect("snapshot");
        assert_eq!(restored.state, PetState::Sleeping);
        std::fs::remove_file(source).expect("remove source");
        std::fs::remove_file(destination).expect("remove destination");
    }

    #[test]
    fn restores_profiles_after_runtime_restart() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "nimora-profiles-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let focus_id = {
            let repository = SqliteProfileRepository::open(&path).expect("database");
            let service = ProfileService::initialize(repository, RuntimeEventBus::default())
                .expect("profiles");
            service
                .create_profile("Focus", nimora_runtime_core::ProfilePolicy::standard())
                .expect("create");
            let snapshot = service.snapshot().expect("snapshot");
            let focus_id = snapshot
                .profiles
                .iter()
                .find(|profile| profile.name == "Focus")
                .expect("focus profile")
                .id;
            service.switch_active(focus_id).expect("activate");
            focus_id
        };
        {
            let repository = SqliteProfileRepository::open(&path).expect("database");
            let service = ProfileService::initialize(repository, RuntimeEventBus::default())
                .expect("profiles");
            let snapshot = service.snapshot().expect("snapshot");
            assert_eq!(snapshot.profiles.len(), 2);
            assert_eq!(snapshot.active_profile_id, focus_id);
        }
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn profile_changes_append_deserializable_outbox_events() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "nimora-profile-outbox-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let repository = SqliteProfileRepository::open(&path).expect("database");
        let service =
            ProfileService::initialize(repository, RuntimeEventBus::default()).expect("profiles");
        service
            .create_profile("Focus", nimora_runtime_core::ProfilePolicy::standard())
            .expect("create");
        let snapshot = service.snapshot().expect("snapshot");
        let focus_id = snapshot
            .profiles
            .iter()
            .find(|profile| profile.name == "Focus")
            .expect("focus profile")
            .id;
        service.switch_active(focus_id).expect("activate");
        drop(service);

        let connection = Connection::open(&path).expect("database");
        let mut statement = connection
            .prepare("SELECT payload FROM event_outbox ORDER BY created_at, event_id")
            .expect("statement");
        let events = statement
            .query_map([], |row| row.get::<_, String>(0))
            .expect("query")
            .map(|payload| {
                serde_json::from_str::<Event>(&payload.expect("payload")).expect("event")
            })
            .collect::<Vec<_>>();
        assert_eq!(events.len(), 2);
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "profile.collection.created")
        );
        assert!(
            events
                .iter()
                .any(|event| event.event_type == "profile.active.changed")
        );
        drop(statement);
        drop(connection);
        std::fs::remove_file(path).expect("remove fixture");
    }

    #[test]
    fn rejects_unpublished_database_versions() {
        let connection = Connection::open_in_memory().expect("database");
        connection
            .pragma_update(None, "user_version", 2)
            .expect("fixture version");
        assert!(matches!(
            SqlitePetRepository::from_connection(connection),
            Err(SqlitePersistenceError::UnsupportedDatabaseVersion(2))
        ));
    }
}
