use crate::{SqlitePersistenceError, prepare_connection};
use nimora_automation_runtime::{AutomationDefinition, AutomationEngine};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use serde::{Deserialize, Serialize};
use std::{path::Path, sync::Mutex};

const AUTOMATION_CATALOG_VERSION: u32 = 1;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationCatalogEntry {
    pub spec: String,
    pub definition: AutomationDefinition,
    pub enabled: bool,
    pub installed_at_ms: u64,
    pub updated_at_ms: u64,
    pub previous_version: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AutomationInstallReceipt {
    pub spec: String,
    pub automation_id: String,
    pub version: String,
    pub replaced_version: Option<String>,
    pub enabled: bool,
}

#[derive(Debug)]
pub struct SqliteAutomationCatalog {
    connection: Mutex<Connection>,
}

impl SqliteAutomationCatalog {
    /// Opens the Automation catalog in the shared application database.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot open or initialize the catalog.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an isolated Automation catalog for tests or recovery mode.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot initialize the in-memory catalog.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Atomically installs a strictly newer definition and retains one previous version.
    ///
    /// # Errors
    ///
    /// Rejects invalid definitions, duplicate or non-increasing versions, and storage failures.
    pub fn install(
        &self,
        definition: &AutomationDefinition,
        now_ms: u64,
    ) -> Result<AutomationInstallReceipt, SqlitePersistenceError> {
        AutomationEngine::validate(definition)
            .map_err(|_| SqlitePersistenceError::InvalidAutomationCatalog)?;
        let mut definition = definition.clone();
        definition.enabled = false;
        let payload = serde_json::to_string(&definition)?;
        let now_ms_i64 =
            i64::try_from(now_ms).map_err(|_| SqlitePersistenceError::InvalidAutomationCatalog)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let previous = transaction
            .query_row(
                "SELECT current_version, current_payload, installed_at_ms
                 FROM automation_catalog WHERE automation_id = ?1",
                params![definition.id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, i64>(2)?,
                    ))
                },
            )
            .optional()?;
        if previous
            .as_ref()
            .is_some_and(|(version, _, _)| version == &definition.version)
        {
            return Err(SqlitePersistenceError::AutomationVersionAlreadyInstalled);
        }
        if previous.as_ref().is_some_and(|(version, _, _)| {
            version_tuple(&definition.version) <= version_tuple(version)
        }) {
            return Err(SqlitePersistenceError::AutomationVersionNotNewer);
        }
        let replaced_version = previous.as_ref().map(|(version, _, _)| version.clone());
        let installed_at_ms = previous
            .as_ref()
            .map_or(now_ms_i64, |(_, _, installed)| *installed);
        let previous_payload = previous.as_ref().map(|(_, payload, _)| payload.as_str());
        transaction.execute(
            "INSERT INTO automation_catalog
                (automation_id, current_version, current_payload, previous_version,
                 previous_payload, enabled, installed_at_ms, updated_at_ms, schema_version)
             VALUES (?1, ?2, ?3, ?4, ?5, 0, ?6, ?7, ?8)
             ON CONFLICT(automation_id) DO UPDATE SET
                previous_version = automation_catalog.current_version,
                previous_payload = automation_catalog.current_payload,
                current_version = excluded.current_version,
                current_payload = excluded.current_payload,
                enabled = 0,
                updated_at_ms = excluded.updated_at_ms,
                schema_version = excluded.schema_version",
            params![
                definition.id,
                definition.version,
                payload,
                replaced_version,
                previous_payload,
                installed_at_ms,
                now_ms_i64,
                AUTOMATION_CATALOG_VERSION,
            ],
        )?;
        transaction.commit()?;
        Ok(AutomationInstallReceipt {
            spec: "nimora.automation-install-receipt/1".to_owned(),
            automation_id: definition.id,
            version: definition.version,
            replaced_version,
            enabled: false,
        })
    }

    /// Loads one installed Automation after revalidating its stored definition.
    ///
    /// # Errors
    ///
    /// Returns an error for storage, schema, or stored-contract corruption.
    pub fn get(
        &self,
        automation_id: &str,
    ) -> Result<Option<AutomationCatalogEntry>, SqlitePersistenceError> {
        self.connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .query_row(
                "SELECT current_payload, enabled, installed_at_ms, updated_at_ms,
                        previous_version, schema_version
                 FROM automation_catalog WHERE automation_id = ?1",
                params![automation_id],
                decode_entry,
            )
            .optional()
            .map_err(Into::into)
    }

    /// Lists all installed Automations in stable identifier order.
    ///
    /// # Errors
    ///
    /// Returns an error for storage, schema, or stored-contract corruption.
    pub fn list(&self) -> Result<Vec<AutomationCatalogEntry>, SqlitePersistenceError> {
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT current_payload, enabled, installed_at_ms, updated_at_ms,
                    previous_version, schema_version
             FROM automation_catalog ORDER BY automation_id",
        )?;
        statement
            .query_map([], decode_entry)?
            .collect::<Result<Vec<_>, _>>()
            .map_err(Into::into)
    }

    /// Changes only the explicit enabled state of an installed Automation.
    ///
    /// # Errors
    ///
    /// Rejects unknown identities, invalid timestamps, and storage failures.
    pub fn set_enabled(
        &self,
        automation_id: &str,
        enabled: bool,
        now_ms: u64,
    ) -> Result<(), SqlitePersistenceError> {
        let changed = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "UPDATE automation_catalog SET enabled = ?2, updated_at_ms = ?3
                 WHERE automation_id = ?1",
                params![
                    automation_id,
                    enabled,
                    i64::try_from(now_ms)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationCatalog)?,
                ],
            )?;
        if changed == 1 {
            Ok(())
        } else {
            Err(SqlitePersistenceError::AutomationNotInstalled)
        }
    }

    /// Atomically swaps current and previous versions and leaves the result disabled.
    ///
    /// # Errors
    ///
    /// Rejects unknown identities, missing previous versions, invalid timestamps, and storage failures.
    pub fn rollback(
        &self,
        automation_id: &str,
        now_ms: u64,
    ) -> Result<AutomationInstallReceipt, SqlitePersistenceError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let versions = transaction
            .query_row(
                "SELECT current_version, current_payload, previous_version, previous_payload
                 FROM automation_catalog WHERE automation_id = ?1",
                params![automation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or(SqlitePersistenceError::AutomationNotInstalled)?;
        let previous_version = versions
            .2
            .ok_or(SqlitePersistenceError::AutomationPreviousVersionUnavailable)?;
        let previous_payload = versions
            .3
            .ok_or(SqlitePersistenceError::AutomationPreviousVersionUnavailable)?;
        transaction.execute(
            "UPDATE automation_catalog SET current_version = ?2, current_payload = ?3,
                previous_version = ?4, previous_payload = ?5, enabled = 0, updated_at_ms = ?6
             WHERE automation_id = ?1",
            params![
                automation_id,
                previous_version,
                previous_payload,
                versions.0,
                versions.1,
                i64::try_from(now_ms)
                    .map_err(|_| SqlitePersistenceError::InvalidAutomationCatalog)?,
            ],
        )?;
        transaction.commit()?;
        Ok(AutomationInstallReceipt {
            spec: "nimora.automation-install-receipt/1".to_owned(),
            automation_id: automation_id.to_owned(),
            version: previous_version,
            replaced_version: Some(versions.0),
            enabled: false,
        })
    }
}

pub(crate) fn ensure_automation_catalog_schema(
    connection: &Connection,
) -> Result<(), SqlitePersistenceError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS automation_catalog (
            automation_id TEXT PRIMARY KEY,
            current_version TEXT NOT NULL,
            current_payload TEXT NOT NULL,
            previous_version TEXT,
            previous_payload TEXT,
            enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
            installed_at_ms INTEGER NOT NULL CHECK (installed_at_ms >= 0),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= installed_at_ms),
            schema_version INTEGER NOT NULL
        );",
    )?;
    Ok(())
}

fn decode_entry(row: &rusqlite::Row<'_>) -> rusqlite::Result<AutomationCatalogEntry> {
    let payload = row.get::<_, String>(0)?;
    let schema_version = row.get::<_, u32>(5)?;
    if schema_version != AUTOMATION_CATALOG_VERSION {
        return Err(rusqlite::Error::InvalidQuery);
    }
    let definition = serde_json::from_str::<AutomationDefinition>(&payload).map_err(|error| {
        rusqlite::Error::FromSqlConversionFailure(0, rusqlite::types::Type::Text, Box::new(error))
    })?;
    AutomationEngine::validate(&definition).map_err(|_| rusqlite::Error::InvalidQuery)?;
    Ok(AutomationCatalogEntry {
        spec: "nimora.automation-catalog-entry/1".to_owned(),
        definition,
        enabled: row.get(1)?,
        installed_at_ms: u64::try_from(row.get::<_, i64>(2)?)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(2, i64::MIN))?,
        updated_at_ms: u64::try_from(row.get::<_, i64>(3)?)
            .map_err(|_| rusqlite::Error::IntegralValueOutOfRange(3, i64::MIN))?,
        previous_version: row.get(4)?,
    })
}

fn version_tuple(version: &str) -> (u64, u64, u64) {
    let mut segments = version
        .split('.')
        .map(|segment| segment.parse::<u64>().unwrap_or(u64::MAX));
    (
        segments.next().unwrap_or(u64::MAX),
        segments.next().unwrap_or(u64::MAX),
        segments.next().unwrap_or(u64::MAX),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_automation_runtime::{
        AutomationAction, AutomationPolicy, EventTrigger, FailurePolicy,
    };
    use nimora_runtime_core::CommandRisk;
    use serde_json::json;
    use uuid::Uuid;

    fn definition(version: &str) -> AutomationDefinition {
        AutomationDefinition {
            spec: "nimora.automation/1".to_owned(),
            id: "automation.local.catalog-test".to_owned(),
            version: version.to_owned(),
            name: "Catalog test".to_owned(),
            enabled: true,
            trigger: EventTrigger {
                event_type: "focus.session.finished".to_owned(),
            },
            conditions: vec![],
            actions: vec![AutomationAction {
                id: "celebrate".to_owned(),
                command: "pet.animation.play".to_owned(),
                arguments: json!({ "action": "celebrate" }),
                risk: CommandRisk::Low,
                retry_safe: false,
                idempotency_key: None,
                compensation: None,
            }],
            policy: AutomationPolicy {
                timeout_ms: 5_000,
                failure: FailurePolicy::Stop,
            },
        }
    }

    #[test]
    fn install_upgrade_enable_and_rollback_are_atomic() {
        let catalog = SqliteAutomationCatalog::in_memory().expect("catalog");
        let first = catalog.install(&definition("1.0.0"), 10).expect("install");
        assert!(!first.enabled);
        catalog
            .set_enabled(&first.automation_id, true, 11)
            .expect("enable");
        assert!(
            catalog
                .get(&first.automation_id)
                .expect("get")
                .expect("entry")
                .enabled
        );

        let upgraded = catalog.install(&definition("2.0.0"), 20).expect("upgrade");
        assert_eq!(upgraded.replaced_version.as_deref(), Some("1.0.0"));
        let entry = catalog
            .get(&first.automation_id)
            .expect("get")
            .expect("entry");
        assert_eq!(entry.definition.version, "2.0.0");
        assert!(!entry.enabled);

        let rolled_back = catalog
            .rollback(&first.automation_id, 30)
            .expect("rollback");
        assert_eq!(rolled_back.version, "1.0.0");
        assert!(!rolled_back.enabled);
        assert_eq!(catalog.list().expect("list").len(), 1);
    }

    #[test]
    fn same_version_and_missing_previous_fail_closed() {
        let catalog = SqliteAutomationCatalog::in_memory().expect("catalog");
        let installed_definition = definition("1.0.0");
        catalog.install(&installed_definition, 10).expect("install");
        assert!(matches!(
            catalog.install(&installed_definition, 20),
            Err(SqlitePersistenceError::AutomationVersionAlreadyInstalled)
        ));
        assert!(matches!(
            catalog.install(&definition("0.9.0"), 20),
            Err(SqlitePersistenceError::AutomationVersionNotNewer)
        ));
        assert!(matches!(
            catalog.rollback(&installed_definition.id, 30),
            Err(SqlitePersistenceError::AutomationPreviousVersionUnavailable)
        ));
    }

    #[test]
    fn catalog_survives_restart_across_upgrade_and_rollback() {
        let path = std::env::temp_dir().join(format!(
            "nimora-automation-catalog-{}.sqlite3",
            Uuid::now_v7()
        ));
        let automation_id = definition("1.0.0").id;

        {
            let catalog = SqliteAutomationCatalog::open(&path).expect("open catalog");
            catalog.install(&definition("1.0.0"), 10).expect("install");
        }
        {
            let catalog = SqliteAutomationCatalog::open(&path).expect("reopen catalog");
            let entry = catalog
                .get(&automation_id)
                .expect("get")
                .expect("persisted entry");
            assert_eq!(entry.definition.version, "1.0.0");
            assert!(!entry.enabled);
            catalog.install(&definition("2.0.0"), 20).expect("upgrade");
        }
        {
            let catalog = SqliteAutomationCatalog::open(&path).expect("reopen after upgrade");
            let entry = catalog
                .get(&automation_id)
                .expect("get")
                .expect("upgraded entry");
            assert_eq!(entry.definition.version, "2.0.0");
            assert_eq!(entry.previous_version.as_deref(), Some("1.0.0"));
            catalog.rollback(&automation_id, 30).expect("rollback");
        }
        {
            let catalog = SqliteAutomationCatalog::open(&path).expect("reopen after rollback");
            let entry = catalog
                .get(&automation_id)
                .expect("get")
                .expect("rolled back entry");
            assert_eq!(entry.definition.version, "1.0.0");
            assert_eq!(entry.previous_version.as_deref(), Some("2.0.0"));
            assert!(!entry.enabled);
        }

        std::fs::remove_file(path).expect("fixture cleanup");
    }
}
