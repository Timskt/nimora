use crate::{SqlitePersistenceError, prepare_connection};
use nimora_agent_runtime::{CompactedContext, DataClassification};
use rusqlite::{Connection, OptionalExtension, Transaction, params};
use serde::{Deserialize, Serialize};
use std::{
    path::Path,
    sync::{Mutex, MutexGuard},
};

const SCHEMA_VERSION: u32 = 1;
const MAX_ENTRIES: usize = 4_096;
const MAX_BYTES: usize = 256 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ContextCachePolicy {
    pub max_entries: usize,
    pub max_bytes: usize,
}

impl ContextCachePolicy {
    /// Creates bounded persistent cache limits.
    ///
    /// # Errors
    ///
    /// Returns an error for zero or excessive limits.
    pub fn new(max_entries: usize, max_bytes: usize) -> Result<Self, SqlitePersistenceError> {
        if max_entries == 0
            || max_entries > MAX_ENTRIES
            || !(1_024..=MAX_BYTES).contains(&max_bytes)
        {
            return Err(SqlitePersistenceError::InvalidContextCache);
        }
        Ok(Self {
            max_entries,
            max_bytes,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct StoredContextCacheEntry {
    pub spec: String,
    pub context: CompactedContext,
    pub data_classification: DataClassification,
    pub expires_at_ms: u64,
}

impl StoredContextCacheEntry {
    /// Creates one immutable, expiring cache record.
    ///
    /// # Errors
    ///
    /// Returns an error when the context or expiry is invalid.
    pub fn new(
        context: CompactedContext,
        data_classification: DataClassification,
        expires_at_ms: u64,
    ) -> Result<Self, SqlitePersistenceError> {
        let entry = Self {
            spec: "nimora.stored-context-cache/1".to_owned(),
            context,
            data_classification,
            expires_at_ms,
        };
        validate(&entry)?;
        Ok(entry)
    }
}

#[derive(Debug)]
pub struct SqliteContextCacheRepository {
    connection: Mutex<Connection>,
    policy: ContextCachePolicy,
}

impl SqliteContextCacheRepository {
    /// Opens the shared application database with bounded cache limits.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` or the policy is invalid.
    pub fn open(
        path: impl AsRef<Path>,
        policy: ContextCachePolicy,
    ) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?, policy)
    }

    /// Creates an isolated cache for tests.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot initialize.
    pub fn in_memory(policy: ContextCachePolicy) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?, policy)
    }

    fn from_connection(
        mut connection: Connection,
        policy: ContextCachePolicy,
    ) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
            policy,
        })
    }

    /// Upserts one entry and enforces TTL, LRU, entry and byte limits transactionally.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid entries or persistence failures.
    pub fn put(
        &self,
        entry: &StoredContextCacheEntry,
        now_ms: u64,
    ) -> Result<(), SqlitePersistenceError> {
        validate(entry)?;
        if entry.expires_at_ms <= now_ms {
            return Err(SqlitePersistenceError::InvalidContextCache);
        }
        let payload = serde_json::to_string(entry)?;
        let payload_bytes = payload.len();
        if payload_bytes > self.policy.max_bytes {
            return Err(SqlitePersistenceError::InvalidContextCache);
        }
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        purge_expired(&transaction, now_ms)?;
        transaction.execute(
            "INSERT INTO agent_context_cache (
                cache_key, provider_id, model, workspace_fingerprint, plan_revision,
                data_classification, created_at_ms, expires_at_ms, last_accessed_at_ms,
                payload_bytes, schema_version, payload
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
             ON CONFLICT(cache_key) DO UPDATE SET
                provider_id = excluded.provider_id,
                model = excluded.model,
                workspace_fingerprint = excluded.workspace_fingerprint,
                plan_revision = excluded.plan_revision,
                data_classification = excluded.data_classification,
                created_at_ms = excluded.created_at_ms,
                expires_at_ms = excluded.expires_at_ms,
                last_accessed_at_ms = excluded.last_accessed_at_ms,
                payload_bytes = excluded.payload_bytes,
                schema_version = excluded.schema_version,
                payload = excluded.payload",
            params![
                entry.context.cache_key,
                entry.context.provider_id,
                entry.context.model,
                entry.context.workspace_fingerprint,
                to_i64(entry.context.plan_revision)?,
                classification_name(entry.data_classification),
                to_i64(entry.context.created_at_ms)?,
                to_i64(entry.expires_at_ms)?,
                to_i64(now_ms.max(entry.context.created_at_ms))?,
                i64::try_from(payload_bytes)
                    .map_err(|_| SqlitePersistenceError::InvalidContextCache)?,
                SCHEMA_VERSION,
                payload,
            ],
        )?;
        enforce_limits(&transaction, self.policy)?;
        transaction.commit()?;
        Ok(())
    }

    /// Loads a cache hit only when identity, TTL and data policy still match.
    ///
    /// # Errors
    ///
    /// Returns an error for corrupt metadata or persistence failures.
    pub fn get(
        &self,
        cache_key: &str,
        workspace_fingerprint: &str,
        maximum_data_classification: DataClassification,
        now_ms: u64,
    ) -> Result<Option<CompactedContext>, SqlitePersistenceError> {
        if cache_key.trim().is_empty() || workspace_fingerprint.trim().is_empty() {
            return Err(SqlitePersistenceError::InvalidContextCache);
        }
        let mut connection = self.lock()?;
        let transaction = connection.transaction()?;
        let row = transaction
            .query_row(
                "SELECT schema_version, payload, provider_id, model, workspace_fingerprint,
                    plan_revision, data_classification, created_at_ms, expires_at_ms, payload_bytes
             FROM agent_context_cache WHERE cache_key = ?1",
                [cache_key],
                |row| {
                    Ok((
                        row.get::<_, u32>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                        row.get::<_, i64>(8)?,
                        row.get::<_, i64>(9)?,
                    ))
                },
            )
            .optional()?;
        let Some(row) = row else {
            transaction.commit()?;
            return Ok(None);
        };
        if row.0 != SCHEMA_VERSION {
            return Err(SqlitePersistenceError::UnsupportedContextCacheVersion(
                row.0,
            ));
        }
        let entry: StoredContextCacheEntry = serde_json::from_str(&row.1)?;
        validate(&entry)?;
        let payload_bytes =
            i64::try_from(row.1.len()).map_err(|_| SqlitePersistenceError::InvalidContextCache)?;
        if entry.context.cache_key != cache_key
            || entry.context.provider_id != row.2
            || entry.context.model != row.3
            || entry.context.workspace_fingerprint != row.4
            || entry.context.plan_revision != from_i64(row.5)?
            || classification_name(entry.data_classification) != row.6
            || entry.context.created_at_ms != from_i64(row.7)?
            || entry.expires_at_ms != from_i64(row.8)?
            || payload_bytes != row.9
        {
            return Err(SqlitePersistenceError::InvalidContextCache);
        }
        if entry.expires_at_ms <= now_ms {
            transaction.execute(
                "DELETE FROM agent_context_cache WHERE cache_key = ?1",
                [cache_key],
            )?;
            transaction.commit()?;
            return Ok(None);
        }
        if entry.context.workspace_fingerprint != workspace_fingerprint
            || entry.data_classification > maximum_data_classification
        {
            transaction.commit()?;
            return Ok(None);
        }
        transaction.execute(
            "UPDATE agent_context_cache SET last_accessed_at_ms = ?2 WHERE cache_key = ?1",
            params![cache_key, to_i64(now_ms.max(entry.context.created_at_ms))?],
        )?;
        transaction.commit()?;
        Ok(Some(entry.context))
    }

    /// Removes every entry bound to one obsolete Workspace fingerprint.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty fingerprint or persistence failure.
    pub fn invalidate_workspace(
        &self,
        workspace_fingerprint: &str,
    ) -> Result<usize, SqlitePersistenceError> {
        if workspace_fingerprint.trim().is_empty() {
            return Err(SqlitePersistenceError::InvalidContextCache);
        }
        let deleted = self.lock()?.execute(
            "DELETE FROM agent_context_cache WHERE workspace_fingerprint = ?1",
            [workspace_fingerprint],
        )?;
        Ok(deleted)
    }

    /// Removes all entries whose TTL has elapsed.
    ///
    /// # Errors
    ///
    /// Returns an error when the timestamp cannot be represented or persistence fails.
    pub fn purge_expired(&self, now_ms: u64) -> Result<usize, SqlitePersistenceError> {
        let connection = self.lock()?;
        purge_expired(&connection, now_ms)
    }

    fn lock(&self) -> Result<MutexGuard<'_, Connection>, SqlitePersistenceError> {
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)
    }
}

fn validate(entry: &StoredContextCacheEntry) -> Result<(), SqlitePersistenceError> {
    entry
        .context
        .validate()
        .map_err(|_| SqlitePersistenceError::InvalidContextCache)?;
    if entry.spec != "nimora.stored-context-cache/1"
        || entry.expires_at_ms <= entry.context.created_at_ms
    {
        return Err(SqlitePersistenceError::InvalidContextCache);
    }
    Ok(())
}

fn purge_expired(connection: &Connection, now_ms: u64) -> Result<usize, SqlitePersistenceError> {
    Ok(connection.execute(
        "DELETE FROM agent_context_cache WHERE expires_at_ms <= ?1",
        [to_i64(now_ms)?],
    )?)
}

fn enforce_limits(
    transaction: &Transaction<'_>,
    policy: ContextCachePolicy,
) -> Result<(), SqlitePersistenceError> {
    loop {
        let (entries, bytes): (i64, i64) = transaction.query_row(
            "SELECT COUNT(*), COALESCE(SUM(payload_bytes), 0) FROM agent_context_cache",
            [],
            |row| Ok((row.get(0)?, row.get(1)?)),
        )?;
        if entries
            <= i64::try_from(policy.max_entries)
                .map_err(|_| SqlitePersistenceError::InvalidContextCache)?
            && bytes
                <= i64::try_from(policy.max_bytes)
                    .map_err(|_| SqlitePersistenceError::InvalidContextCache)?
        {
            return Ok(());
        }
        let deleted = transaction.execute(
            "DELETE FROM agent_context_cache WHERE cache_key = (
                SELECT cache_key FROM agent_context_cache
                ORDER BY last_accessed_at_ms ASC, cache_key ASC LIMIT 1
             )",
            [],
        )?;
        if deleted != 1 {
            return Err(SqlitePersistenceError::InvalidContextCache);
        }
    }
}

fn classification_name(value: DataClassification) -> &'static str {
    match value {
        DataClassification::Public => "public",
        DataClassification::Internal => "internal",
        DataClassification::Personal => "personal",
        DataClassification::Sensitive => "sensitive",
        DataClassification::Restricted => "restricted",
    }
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidContextCache)
}

fn from_i64(value: i64) -> Result<u64, SqlitePersistenceError> {
    u64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidContextCache)
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_agent_runtime::{
        ContextAnchor, ContextCompactionPolicy, ContextCompactor, ProviderMessage,
        ProviderMessageRole,
    };
    use uuid::Uuid;

    fn compacted(workspace: &str, content: &str, created_at_ms: u64) -> CompactedContext {
        ContextCompactor
            .compact(
                Uuid::now_v7(),
                Uuid::now_v7(),
                "provider:local",
                "model:local",
                &[
                    ProviderMessage::text(
                        ProviderMessageRole::System,
                        "system",
                        DataClassification::Internal,
                        true,
                    ),
                    ProviderMessage::text(
                        ProviderMessageRole::User,
                        content,
                        DataClassification::Personal,
                        true,
                    ),
                ],
                &[],
                &ContextAnchor {
                    goal: "finish safely".to_owned(),
                    constraints: vec!["preserve evidence".to_owned()],
                    pending_steps: vec!["run tests".to_owned()],
                    evidence: vec![],
                    workspace_fingerprint: workspace.to_owned(),
                    plan_revision: 1,
                },
                ContextCompactionPolicy {
                    max_messages: 4,
                    max_content_bytes: 16 * 1024,
                    retain_recent_units: 1,
                },
                created_at_ms,
            )
            .expect("compact context")
    }

    #[test]
    fn persists_hits_and_enforces_workspace_data_and_expiry() {
        let repository = SqliteContextCacheRepository::in_memory(
            ContextCachePolicy::new(4, 64 * 1024).expect("policy"),
        )
        .expect("repository");
        let context = compacted("sha256:workspace-one", "one", 1_000);
        let key = context.cache_key.clone();
        repository
            .put(
                &StoredContextCacheEntry::new(context.clone(), DataClassification::Personal, 2_000)
                    .expect("entry"),
                1_100,
            )
            .expect("put");

        assert!(
            repository
                .get(
                    &key,
                    "sha256:workspace-one",
                    DataClassification::Internal,
                    1_200,
                )
                .expect("classification miss")
                .is_none()
        );
        assert!(
            repository
                .get(
                    &key,
                    "sha256:workspace-two",
                    DataClassification::Personal,
                    1_200,
                )
                .expect("workspace miss")
                .is_none()
        );
        assert_eq!(
            repository
                .get(
                    &key,
                    "sha256:workspace-one",
                    DataClassification::Personal,
                    1_300,
                )
                .expect("hit"),
            Some(context)
        );
        assert!(
            repository
                .get(
                    &key,
                    "sha256:workspace-one",
                    DataClassification::Personal,
                    2_000,
                )
                .expect("expired")
                .is_none()
        );
    }

    #[test]
    fn evicts_lru_and_invalidates_workspace_partition() {
        let repository = SqliteContextCacheRepository::in_memory(
            ContextCachePolicy::new(2, 128 * 1024).expect("policy"),
        )
        .expect("repository");
        let first = compacted("sha256:workspace-one", "first", 1_000);
        let second = compacted("sha256:workspace-one", "second", 1_001);
        let third = compacted("sha256:workspace-two", "third", 1_002);
        for (context, access) in [(&first, 1_100), (&second, 1_101)] {
            repository
                .put(
                    &StoredContextCacheEntry::new(
                        context.clone(),
                        DataClassification::Internal,
                        3_000,
                    )
                    .expect("entry"),
                    access,
                )
                .expect("put");
        }
        repository
            .get(
                &first.cache_key,
                "sha256:workspace-one",
                DataClassification::Internal,
                1_200,
            )
            .expect("touch first");
        repository
            .put(
                &StoredContextCacheEntry::new(third.clone(), DataClassification::Internal, 3_000)
                    .expect("entry"),
                1_300,
            )
            .expect("put third");
        assert!(
            repository
                .get(
                    &second.cache_key,
                    "sha256:workspace-one",
                    DataClassification::Internal,
                    1_400,
                )
                .expect("evicted")
                .is_none()
        );
        assert_eq!(
            repository
                .invalidate_workspace("sha256:workspace-one")
                .expect("invalidate"),
            1
        );
        assert!(
            repository
                .get(
                    &third.cache_key,
                    "sha256:workspace-two",
                    DataClassification::Internal,
                    1_500,
                )
                .expect("other workspace")
                .is_some()
        );
    }
}
