//! `SQLite` persistence adapters for the `AsterPet` runtime.

use asterpet_runtime_app::{PetRepository, RepositoryError};
use asterpet_runtime_core::Pet;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex, time::Duration};
use thiserror::Error;

const DATABASE_VERSION: i64 = 1;
const PET_SNAPSHOT_VERSION: u32 = 1;

#[derive(Debug)]
pub struct SqlitePetRepository {
    connection: Mutex<Connection>,
}

impl SqlitePetRepository {
    /// Opens or creates an `AsterPet` database and applies pending migrations.
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
        connection.busy_timeout(Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", true)?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        let version =
            connection.query_row("PRAGMA user_version", [], |row| row.get::<_, i64>(0))?;
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
            return Err(SqlitePersistenceError::UnsupportedSnapshotVersion(
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
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction()?;
        transaction.execute(
            "INSERT INTO pet_snapshot (singleton, schema_version, payload)
             VALUES (1, ?1, ?2)
             ON CONFLICT(singleton) DO UPDATE SET
               schema_version = excluded.schema_version,
               payload = excluded.payload,
               updated_at = CURRENT_TIMESTAMP",
            params![PET_SNAPSHOT_VERSION, payload],
        )?;
        transaction.commit()?;
        Ok(())
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
    UnsupportedSnapshotVersion(u32),
    #[error("pet snapshot metadata and payload versions do not match")]
    SnapshotVersionMismatch,
    #[error(transparent)]
    Sqlite(#[from] rusqlite::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Pet(#[from] asterpet_runtime_core::PetError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use asterpet_runtime_app::RuntimeService;
    use asterpet_runtime_core::{PetAction, PetState};
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
            Err(SqlitePersistenceError::UnsupportedSnapshotVersion(99))
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
            "asterpet-persistence-{}-{unique}.sqlite3",
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
}
