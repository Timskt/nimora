//! `SQLite` persistence adapters for the `Nimora` runtime.

use nimora_runtime_app::{
    PetRepository, ProfileRepository, ProfileServiceError, ProfileSnapshot, RepositoryError,
};
use nimora_runtime_core::Pet;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex, time::Duration};
use thiserror::Error;

const DATABASE_VERSION: i64 = 2;
const PET_SNAPSHOT_VERSION: u32 = 1;
const PROFILE_SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug)]
pub struct SqlitePetRepository {
    connection: Mutex<Connection>,
}

impl SqlitePetRepository {
    /// Opens or creates an `Nimora` database and applies pending migrations.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot open, configure, or migrate the
    /// database. A database from a newer application version is rejected.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated in-memory database for tests and ephemeral tools.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot configure or migrate the database.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
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
        )
    }
}

fn prepare_connection(connection: &mut Connection) -> Result<(), SqlitePersistenceError> {
    connection.busy_timeout(Duration::from_secs(5))?;
    connection.pragma_update(None, "foreign_keys", true)?;
    connection.pragma_update(None, "journal_mode", "WAL")?;
    connection.pragma_update(None, "synchronous", "NORMAL")?;
    let version = connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
    if version > DATABASE_VERSION {
        return Err(SqlitePersistenceError::UnsupportedDatabaseVersion(version));
    }
    if version < 1 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE pet_snapshot (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    schema_version INTEGER NOT NULL,
                    payload TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                PRAGMA user_version = 1;",
        )?;
        transaction.commit()?;
    }
    if version < 2 {
        let transaction = connection.transaction()?;
        transaction.execute_batch(
            "CREATE TABLE profile_snapshot (
                    singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                    schema_version INTEGER NOT NULL,
                    payload TEXT NOT NULL,
                    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                );
                PRAGMA user_version = 2;",
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
    transaction.commit()?;
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
    /// Returns an error when `SQLite` configuration or migration fails.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        let mut connection = Connection::open(path)?;
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
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
    use nimora_runtime_core::{PetAction, PetState};
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
    fn migrates_v1_without_losing_pet_state() {
        let unique = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("system time")
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "nimora-migration-{}-{unique}.sqlite3",
            std::process::id()
        ));
        let mut pet = Pet::new("Aster").expect("pet");
        pet.apply_action(PetAction::Sleep);
        let payload = serde_json::to_string(&StoredPetSnapshot {
            schema_version: PET_SNAPSHOT_VERSION,
            pet: pet.clone(),
        })
        .expect("payload");
        {
            let connection = Connection::open(&path).expect("database");
            connection
                .execute_batch(
                    "CREATE TABLE pet_snapshot (
                        singleton INTEGER PRIMARY KEY CHECK (singleton = 1),
                        schema_version INTEGER NOT NULL,
                        payload TEXT NOT NULL,
                        updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
                    );
                    PRAGMA user_version = 1;",
                )
                .expect("v1 schema");
            connection
                .execute(
                    "INSERT INTO pet_snapshot (singleton, schema_version, payload)
                     VALUES (1, ?1, ?2)",
                    params![PET_SNAPSHOT_VERSION, payload],
                )
                .expect("v1 state");
        }
        let repository = SqlitePetRepository::open(&path).expect("migration");
        assert_eq!(
            repository
                .load_snapshot()
                .expect("load")
                .expect("snapshot")
                .state,
            PetState::Sleeping
        );
        let version = repository
            .connection
            .lock()
            .expect("lock")
            .query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))
            .expect("version");
        assert_eq!(version, 2);
        drop(repository);
        std::fs::remove_file(path).expect("remove fixture");
    }
}
