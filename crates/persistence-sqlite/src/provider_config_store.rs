use crate::{SqlitePersistenceError, prepare_connection};
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};
use url::Url;

const SCHEMA_VERSION: u32 = 1;
const MAX_CONFIGS: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProviderConfig {
    pub spec: String,
    pub id: String,
    pub display_name: String,
    pub base_url: String,
    pub credential_reference: String,
    pub default_model: Option<String>,
    pub context_window_tokens: u64,
    pub max_output_tokens: u64,
    pub enabled: bool,
    pub revision: u64,
}

impl ProviderConfig {
    /// Creates a canonical OpenAI-compatible Provider configuration without secret material.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, endpoint, reference, model, or token limits.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        base_url: impl Into<String>,
        credential_reference: impl Into<String>,
        default_model: Option<String>,
        context_window_tokens: u64,
        max_output_tokens: u64,
        enabled: bool,
    ) -> Result<Self, SqlitePersistenceError> {
        let mut config = Self {
            spec: "nimora.provider-config/1".to_owned(),
            id: id.into(),
            display_name: display_name.into(),
            base_url: base_url.into(),
            credential_reference: credential_reference.into(),
            default_model,
            context_window_tokens,
            max_output_tokens,
            enabled,
            revision: 0,
        };
        config.base_url = canonical_endpoint(&config.base_url)?;
        validate(&config)?;
        Ok(config)
    }
}

#[derive(Debug)]
pub struct SqliteProviderConfigRepository {
    connection: Mutex<Connection>,
}

impl SqliteProviderConfigRepository {
    /// Opens the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot initialize the Provider schema.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated repository for tests and recovery mode.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot initialize the Provider schema.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        connection.execute_batch(
            "CREATE TABLE IF NOT EXISTS provider_config (
                provider_id TEXT PRIMARY KEY NOT NULL,
                revision INTEGER NOT NULL,
                schema_version INTEGER NOT NULL,
                payload TEXT NOT NULL
             ) STRICT;",
        )?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Creates or replaces a configuration using optimistic concurrency.
    ///
    /// New records require revision zero. Existing records require the exact current revision.
    /// The returned record contains the committed, incremented revision.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid data, capacity exhaustion, or stale revision.
    pub fn save(&self, config: &ProviderConfig) -> Result<ProviderConfig, SqlitePersistenceError> {
        validate(config)?;
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let current = transaction
            .query_row(
                "SELECT revision FROM provider_config WHERE provider_id = ?1",
                [&config.id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .map(|value| {
                u64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidProviderConfig)
            })
            .transpose()?;
        match current {
            Some(revision) if revision != config.revision => {
                return Err(SqlitePersistenceError::ProviderConfigConflict);
            }
            None if config.revision != 0 => {
                return Err(SqlitePersistenceError::ProviderConfigConflict);
            }
            None => {
                let count =
                    transaction.query_row("SELECT COUNT(*) FROM provider_config", [], |row| {
                        row.get::<_, i64>(0)
                    })?;
                let count = usize::try_from(count)
                    .map_err(|_| SqlitePersistenceError::InvalidProviderConfig)?;
                if count >= MAX_CONFIGS {
                    return Err(SqlitePersistenceError::InvalidProviderConfig);
                }
            }
            Some(_) => {}
        }
        let mut committed = config.clone();
        committed.revision = config.revision.saturating_add(1);
        if committed.revision == config.revision {
            return Err(SqlitePersistenceError::InvalidProviderConfig);
        }
        let payload = serde_json::to_string(&committed)?;
        transaction.execute(
            "INSERT INTO provider_config (provider_id, revision, schema_version, payload)
             VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(provider_id) DO UPDATE SET
               revision = excluded.revision,
               schema_version = excluded.schema_version,
               payload = excluded.payload",
            params![
                committed.id,
                i64::try_from(committed.revision)
                    .map_err(|_| SqlitePersistenceError::InvalidProviderConfig)?,
                SCHEMA_VERSION,
                payload,
            ],
        )?;
        transaction.commit()?;
        Ok(committed)
    }

    /// Lists all configurations in stable ID order and revalidates persisted metadata.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt, unsupported, or inconsistent records.
    pub fn list(&self) -> Result<Vec<ProviderConfig>, SqlitePersistenceError> {
        let connection = self.lock()?;
        let mut statement = connection.prepare(
            "SELECT provider_id, revision, schema_version, payload
             FROM provider_config ORDER BY provider_id",
        )?;
        let rows = statement.query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, i64>(1)?,
                row.get::<_, u32>(2)?,
                row.get::<_, String>(3)?,
            ))
        })?;
        rows.map(|row| {
            let (id, revision, version, payload) = row?;
            if version != SCHEMA_VERSION {
                return Err(SqlitePersistenceError::InvalidProviderConfig);
            }
            let config: ProviderConfig = serde_json::from_str(&payload)?;
            validate(&config)?;
            if config.id != id
                || i64::try_from(config.revision)
                    .map_err(|_| SqlitePersistenceError::InvalidProviderConfig)?
                    != revision
            {
                return Err(SqlitePersistenceError::InvalidProviderConfig);
            }
            Ok(config)
        })
        .collect()
    }

    /// Loads one configuration by exact ID.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity or corrupt storage.
    pub fn get(&self, id: &str) -> Result<Option<ProviderConfig>, SqlitePersistenceError> {
        if !valid_provider_id(id) {
            return Err(SqlitePersistenceError::InvalidProviderConfig);
        }
        Ok(self.list()?.into_iter().find(|config| config.id == id))
    }

    /// Deletes one exact revision and returns whether a row was removed.
    ///
    /// # Errors
    ///
    /// Returns an error when the record changed or storage is unavailable.
    pub fn delete(&self, id: &str, revision: u64) -> Result<bool, SqlitePersistenceError> {
        if !valid_provider_id(id) || revision == 0 {
            return Err(SqlitePersistenceError::InvalidProviderConfig);
        }
        let changed = {
            let connection = self.lock()?;
            connection.execute(
                "DELETE FROM provider_config WHERE provider_id = ?1 AND revision = ?2",
                params![
                    id,
                    i64::try_from(revision)
                        .map_err(|_| SqlitePersistenceError::InvalidProviderConfig)?
                ],
            )?
        };
        if changed == 0 && self.get(id)?.is_some() {
            return Err(SqlitePersistenceError::ProviderConfigConflict);
        }
        Ok(changed == 1)
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, Connection>, SqlitePersistenceError> {
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)
    }
}

fn validate(config: &ProviderConfig) -> Result<(), SqlitePersistenceError> {
    if config.spec != "nimora.provider-config/1"
        || !valid_provider_id(&config.id)
        || config.display_name.trim().is_empty()
        || config.display_name.len() > 128
        || config.display_name.chars().any(char::is_control)
        || canonical_endpoint(&config.base_url)? != config.base_url
        || !valid_secret_reference(&config.credential_reference)
        || config.default_model.as_ref().is_some_and(|model| {
            model.trim().is_empty() || model.len() > 128 || model.chars().any(char::is_control)
        })
        || config.context_window_tokens == 0
        || config.max_output_tokens == 0
        || config.max_output_tokens > config.context_window_tokens
    {
        return Err(SqlitePersistenceError::InvalidProviderConfig);
    }
    Ok(())
}

fn valid_provider_id(value: &str) -> bool {
    value.starts_with("provider:openai-compatible:")
        && value.len() <= 128
        && value.split(':').all(|segment| {
            !segment.is_empty()
                && segment.len() <= 64
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'-' | b'_' | b'.')
                })
        })
}

fn valid_secret_reference(value: &str) -> bool {
    value.starts_with("secret:provider:")
        && value.len() <= 160
        && value.split(':').all(|segment| {
            !segment.is_empty()
                && segment.len() <= 64
                && segment.bytes().all(|byte| {
                    byte.is_ascii_lowercase()
                        || byte.is_ascii_digit()
                        || matches!(byte, b'-' | b'_' | b'.')
                })
        })
}

fn canonical_endpoint(value: &str) -> Result<String, SqlitePersistenceError> {
    let url = Url::parse(value).map_err(|_| SqlitePersistenceError::InvalidProviderConfig)?;
    if url.cannot_be_a_base()
        || url.query().is_some()
        || url.fragment().is_some()
        || !url.username().is_empty()
        || url.password().is_some()
        || (url.path() != "/" && !url.path().is_empty())
    {
        return Err(SqlitePersistenceError::InvalidProviderConfig);
    }
    let host = url
        .host_str()
        .ok_or(SqlitePersistenceError::InvalidProviderConfig)?;
    let loopback = host.eq_ignore_ascii_case("localhost")
        || host
            .parse::<std::net::IpAddr>()
            .is_ok_and(|address| address.is_loopback());
    if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
        return Err(SqlitePersistenceError::InvalidProviderConfig);
    }
    let mut normalized = url;
    normalized.set_path("");
    Ok(normalized.to_string().trim_end_matches('/').to_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn config(id: &str) -> ProviderConfig {
        ProviderConfig::new(
            id,
            "Work AI",
            "https://ai.example.com/",
            "secret:provider:work-ai",
            Some("model-a".to_owned()),
            128_000,
            16_384,
            true,
        )
        .expect("config")
    }

    #[test]
    fn persists_only_reference_and_uses_revision_cas() {
        let repository = SqliteProviderConfigRepository::in_memory().expect("repository");
        let saved = repository
            .save(&config("provider:openai-compatible:work"))
            .expect("save");
        assert_eq!(saved.revision, 1);
        assert_eq!(repository.get(&saved.id).expect("get"), Some(saved.clone()));
        assert!(matches!(
            repository.save(&config("provider:openai-compatible:work")),
            Err(SqlitePersistenceError::ProviderConfigConflict)
        ));
        let mut changed = saved.clone();
        changed.default_model = Some("model-b".to_owned());
        let changed = repository.save(&changed).expect("replace");
        assert_eq!(changed.revision, 2);
        assert!(
            repository
                .delete(&changed.id, changed.revision)
                .expect("delete")
        );
    }

    #[test]
    fn rejects_plaintext_fields_and_unsafe_endpoints() {
        let serialized =
            serde_json::to_string(&config("provider:openai-compatible:safe")).expect("serialize");
        assert!(!serialized.contains("apiKey"));
        assert!(!serialized.contains("token"));
        for endpoint in [
            "http://ai.example.com",
            "https://user@ai.example.com",
            "https://ai.example.com/v1",
        ] {
            assert!(
                ProviderConfig::new(
                    "provider:openai-compatible:unsafe",
                    "Unsafe",
                    endpoint,
                    "secret:provider:unsafe",
                    None,
                    1,
                    1,
                    true,
                )
                .is_err()
            );
        }
    }
}
