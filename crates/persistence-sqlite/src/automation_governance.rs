use crate::{SqlitePersistenceError, prepare_connection};
use rusqlite::{Connection, OptionalExtension, TransactionBehavior, params};
use std::{path::Path, sync::Mutex};
use uuid::Uuid;

const DAY_MS: u64 = 86_400_000;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationRunAdmission {
    pub run_id: Uuid,
    pub automation_id: String,
    pub max_concurrent_runs: u16,
    pub cooldown_ms: u64,
    pub daily_cost_budget_microunits: u64,
    pub now_ms: u64,
    pub lease_expires_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomationCostReservation {
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub reserved_cost_microunits: u64,
    pub now_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationCostStatus {
    Reserved,
    Settled,
    Indeterminate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationCostEntry {
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub automation_id: String,
    pub day_bucket: u64,
    pub reserved_cost_microunits: u64,
    pub actual_cost_microunits: Option<u64>,
    pub status: AutomationCostStatus,
    pub updated_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationCostReconciliationReason {
    ProviderStatement,
    BillingExport,
    OperatorConservativeEstimate,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReconcileAutomationCostRequest {
    pub decision_id: Uuid,
    pub task_id: Uuid,
    pub expected_updated_at_ms: u64,
    pub actual_cost_microunits: u64,
    pub reason: AutomationCostReconciliationReason,
    pub decided_at_ms: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationCostReconciliation {
    pub decision_id: Uuid,
    pub task_id: Uuid,
    pub run_id: Uuid,
    pub automation_id: String,
    pub reserved_cost_microunits: u64,
    pub actual_cost_microunits: u64,
    pub reason: AutomationCostReconciliationReason,
    pub decided_at_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AutomationGovernanceSnapshot {
    pub active_runs: u64,
    pub last_started_at_ms: Option<u64>,
    pub reserved_cost_microunits: u64,
    pub settled_cost_microunits: u64,
    pub indeterminate_cost_microunits: u64,
    pub indeterminate_cost_count: u64,
}

#[derive(Debug)]
pub struct SqliteAutomationGovernance {
    connection: Mutex<Connection>,
}

impl SqliteAutomationGovernance {
    /// Opens a governance store at the supplied `SQLite` path.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot open or initialize the store.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open(path)?)
    }

    /// Creates an in-memory governance store.
    ///
    /// # Errors
    ///
    /// Returns an error when `SQLite` cannot initialize the store.
    pub fn in_memory() -> Result<Self, SqlitePersistenceError> {
        Self::from_connection(Connection::open_in_memory()?)
    }

    fn from_connection(mut connection: Connection) -> Result<Self, SqlitePersistenceError> {
        prepare_connection(&mut connection)?;
        Ok(Self {
            connection: Mutex::new(connection),
        })
    }

    /// Atomically admits a run under its concurrency and cooldown policy.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, policy rejection, poisoned state, or
    /// a `SQLite` transaction failure.
    pub fn admit_run(
        &self,
        admission: &AutomationRunAdmission,
    ) -> Result<(), SqlitePersistenceError> {
        validate_run_admission(admission)?;
        let now_ms = to_i64(admission.now_ms)?;
        let expires_at_ms = to_i64(admission.lease_expires_at_ms)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "DELETE FROM automation_run_lease WHERE expires_at_ms <= ?1",
            [now_ms],
        )?;
        let active = transaction.query_row(
            "SELECT COUNT(*) FROM automation_run_lease WHERE automation_id = ?1",
            [&admission.automation_id],
            |row| row.get::<_, i64>(0),
        )?;
        let active = u64::try_from(active)
            .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?;
        if active >= u64::from(admission.max_concurrent_runs) {
            return Err(SqlitePersistenceError::AutomationConcurrencyExceeded);
        }
        let last_started_at_ms = transaction
            .query_row(
                "SELECT last_started_at_ms FROM automation_governance_state
                 WHERE automation_id = ?1",
                [&admission.automation_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        if let Some(last_started_at_ms) = last_started_at_ms {
            let cooldown_end = u64::try_from(last_started_at_ms)
                .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?
                .checked_add(admission.cooldown_ms)
                .ok_or(SqlitePersistenceError::InvalidAutomationGovernance)?;
            if admission.now_ms < cooldown_end {
                return Err(SqlitePersistenceError::AutomationCooldownActive);
            }
        }
        transaction.execute(
            "INSERT INTO automation_run_lease
                (run_id, automation_id, acquired_at_ms, expires_at_ms,
                 daily_cost_budget_microunits)
             VALUES (?1, ?2, ?3, ?4, ?5)",
            params![
                admission.run_id.to_string(),
                &admission.automation_id,
                now_ms,
                expires_at_ms,
                to_i64(admission.daily_cost_budget_microunits)?,
            ],
        )?;
        transaction.execute(
            "INSERT INTO automation_governance_state (automation_id, last_started_at_ms)
             VALUES (?1, ?2)
             ON CONFLICT(automation_id) DO UPDATE SET last_started_at_ms = excluded.last_started_at_ms",
            params![&admission.automation_id, now_ms],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Releases the active lease for a run.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid identifier, poisoned state, or `SQLite`
    /// failure.
    pub fn release_run(&self, run_id: Uuid) -> Result<bool, SqlitePersistenceError> {
        if run_id.is_nil() {
            return Err(SqlitePersistenceError::InvalidAutomationGovernance);
        }
        Ok(self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "DELETE FROM automation_run_lease WHERE run_id = ?1",
                [run_id.to_string()],
            )?
            == 1)
    }

    /// Recovers governance state after a host restart.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid timestamp, poisoned state, or `SQLite`
    /// transaction failure.
    pub fn recover(&self, now_ms: u64) -> Result<u64, SqlitePersistenceError> {
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "UPDATE automation_cost_ledger SET status = 'indeterminate', updated_at_ms = ?1
             WHERE status = 'reserved' AND updated_at_ms <= ?1",
            [to_i64(now_ms)?],
        )?;
        let changed = transaction.execute("DELETE FROM automation_run_lease", [])?;
        transaction.commit()?;
        u64::try_from(changed).map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)
    }

    /// Reserves an Automation run's daily budget before Provider execution.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, missing run admission, exhausted
    /// budget, a conflicting reservation, poisoned state, or `SQLite` failure.
    pub fn reserve_agent_cost(
        &self,
        reservation: AutomationCostReservation,
    ) -> Result<(), SqlitePersistenceError> {
        validate_cost_reservation(reservation)?;
        let day_bucket = reservation.now_ms / DAY_MS;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        if let Some(existing) = load_cost_entry(&transaction, reservation.task_id)? {
            if existing.run_id == reservation.run_id
                && existing.day_bucket == day_bucket
                && existing.reserved_cost_microunits == reservation.reserved_cost_microunits
            {
                return Ok(());
            }
            return Err(SqlitePersistenceError::AutomationCostReservationConflict);
        }
        let (automation_id, daily_budget_microunits) = transaction
            .query_row(
                "SELECT automation_id, daily_cost_budget_microunits
                 FROM automation_run_lease WHERE run_id = ?1",
                [reservation.run_id.to_string()],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)?)),
            )
            .optional()?
            .ok_or(SqlitePersistenceError::InvalidAutomationGovernance)?;
        let daily_budget_microunits = u64::try_from(daily_budget_microunits)
            .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?;
        let committed = transaction.query_row(
            "SELECT COALESCE(SUM(CASE
                WHEN status = 'settled' THEN actual_cost_microunits
                ELSE reserved_cost_microunits END), 0)
             FROM automation_cost_ledger
             WHERE automation_id = ?1 AND day_bucket = ?2",
            params![automation_id, to_i64(day_bucket)?],
            |row| row.get::<_, i64>(0),
        )?;
        let committed = u64::try_from(committed)
            .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?;
        let projected = committed
            .checked_add(reservation.reserved_cost_microunits)
            .ok_or(SqlitePersistenceError::InvalidAutomationGovernance)?;
        if projected > daily_budget_microunits {
            return Err(SqlitePersistenceError::AutomationDailyCostBudgetExceeded);
        }
        transaction.execute(
            "INSERT INTO automation_cost_ledger
                (task_id, run_id, automation_id, day_bucket, reserved_cost_microunits,
                 actual_cost_microunits, status, created_at_ms, updated_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, NULL, 'reserved', ?6, ?6)",
            params![
                reservation.task_id.to_string(),
                reservation.run_id.to_string(),
                automation_id,
                to_i64(day_bucket)?,
                to_i64(reservation.reserved_cost_microunits)?,
                to_i64(reservation.now_ms)?,
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    /// Replaces a cost reservation with verified Provider usage.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, usage above the reservation, a
    /// conflicting terminal state, poisoned state, or `SQLite` failure.
    pub fn settle_agent_cost(
        &self,
        task_id: Uuid,
        actual_cost_microunits: u64,
        now_ms: u64,
    ) -> Result<(), SqlitePersistenceError> {
        if task_id.is_nil() {
            return Err(SqlitePersistenceError::InvalidAutomationGovernance);
        }
        let changed = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "UPDATE automation_cost_ledger
                 SET actual_cost_microunits = ?2, status = 'settled', updated_at_ms = ?3
                 WHERE task_id = ?1 AND status = 'reserved'
                   AND reserved_cost_microunits >= ?2 AND updated_at_ms <= ?3",
                params![
                    task_id.to_string(),
                    to_i64(actual_cost_microunits)?,
                    to_i64(now_ms)?,
                ],
            )?;
        if changed != 1 {
            let existing = self.get_cost(task_id)?;
            if existing.is_some_and(|entry| {
                entry.status == AutomationCostStatus::Settled
                    && entry.actual_cost_microunits == Some(actual_cost_microunits)
            }) {
                return Ok(());
            }
            return Err(SqlitePersistenceError::AutomationCostReservationConflict);
        }
        Ok(())
    }

    /// Marks an unresolved Provider cost as indeterminate.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, a missing or terminal reservation,
    /// poisoned state, or `SQLite` failure.
    pub fn mark_agent_cost_indeterminate(
        &self,
        task_id: Uuid,
        now_ms: u64,
    ) -> Result<(), SqlitePersistenceError> {
        if task_id.is_nil() {
            return Err(SqlitePersistenceError::InvalidAutomationGovernance);
        }
        let changed = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?
            .execute(
                "UPDATE automation_cost_ledger
                 SET status = 'indeterminate', updated_at_ms = ?2
                 WHERE task_id = ?1 AND status = 'reserved' AND updated_at_ms <= ?2",
                params![task_id.to_string(), to_i64(now_ms)?],
            )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::AutomationCostReservationConflict);
        }
        Ok(())
    }

    /// Loads the cost ledger entry for a task.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid identifier, malformed persisted data,
    /// poisoned state, or `SQLite` failure.
    pub fn get_cost(
        &self,
        task_id: Uuid,
    ) -> Result<Option<AutomationCostEntry>, SqlitePersistenceError> {
        if task_id.is_nil() {
            return Err(SqlitePersistenceError::InvalidAutomationGovernance);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        load_cost_entry(&connection, task_id)
    }

    /// Lists unresolved cost entries in stable oldest-first order.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid limit, malformed persisted data, poisoned state, or
    /// `SQLite` failure.
    pub fn list_indeterminate_costs(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomationCostEntry>, SqlitePersistenceError> {
        if !(1..=200).contains(&limit) {
            return Err(SqlitePersistenceError::InvalidAutomationGovernance);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT task_id FROM automation_cost_ledger
             WHERE status = 'indeterminate'
             ORDER BY updated_at_ms ASC, task_id ASC LIMIT ?1",
        )?;
        let task_ids = statement
            .query_map(
                [i64::try_from(limit)
                    .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?],
                |row| row.get::<_, String>(0),
            )?
            .collect::<Result<Vec<_>, _>>()?;
        task_ids
            .into_iter()
            .map(|task_id| {
                let task_id = Uuid::parse_str(&task_id)
                    .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?;
                load_cost_entry(&connection, task_id)?
                    .ok_or(SqlitePersistenceError::InvalidAutomationGovernance)
            })
            .collect()
    }

    /// Atomically resolves one unknown cost and appends an immutable audit decision.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid input, stale state, a duplicate decision, malformed
    /// persisted data, poisoned state, or `SQLite` failure.
    pub fn reconcile_indeterminate_cost(
        &self,
        request: &ReconcileAutomationCostRequest,
    ) -> Result<AutomationCostReconciliation, SqlitePersistenceError> {
        validate_reconciliation(request)?;
        let mut connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let entry = load_cost_entry(&transaction, request.task_id)?
            .ok_or(SqlitePersistenceError::AutomationCostReconciliationConflict)?;
        if entry.status != AutomationCostStatus::Indeterminate
            || entry.updated_at_ms != request.expected_updated_at_ms
            || request.decided_at_ms < entry.updated_at_ms
        {
            return Err(SqlitePersistenceError::AutomationCostReconciliationConflict);
        }
        let reconciliation = AutomationCostReconciliation {
            decision_id: request.decision_id,
            task_id: request.task_id,
            run_id: entry.run_id,
            automation_id: entry.automation_id,
            reserved_cost_microunits: entry.reserved_cost_microunits,
            actual_cost_microunits: request.actual_cost_microunits,
            reason: request.reason,
            decided_at_ms: request.decided_at_ms,
        };
        transaction
            .execute(
                "INSERT INTO automation_cost_reconciliation
                (decision_id, task_id, run_id, automation_id, reserved_cost_microunits,
                 actual_cost_microunits, reason, decided_at_ms)
             VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    reconciliation.decision_id.to_string(),
                    reconciliation.task_id.to_string(),
                    reconciliation.run_id.to_string(),
                    reconciliation.automation_id,
                    to_i64(reconciliation.reserved_cost_microunits)?,
                    to_i64(reconciliation.actual_cost_microunits)?,
                    reconciliation_reason_name(reconciliation.reason),
                    to_i64(reconciliation.decided_at_ms)?,
                ],
            )
            .map_err(|_| SqlitePersistenceError::AutomationCostReconciliationConflict)?;
        let changed = transaction.execute(
            "UPDATE automation_cost_ledger
             SET actual_cost_microunits = ?2, status = 'settled', updated_at_ms = ?3
             WHERE task_id = ?1 AND status = 'indeterminate' AND updated_at_ms = ?4",
            params![
                request.task_id.to_string(),
                to_i64(request.actual_cost_microunits)?,
                to_i64(request.decided_at_ms)?,
                to_i64(request.expected_updated_at_ms)?,
            ],
        )?;
        if changed != 1 {
            return Err(SqlitePersistenceError::AutomationCostReconciliationConflict);
        }
        transaction.commit()?;
        Ok(reconciliation)
    }

    /// Lists immutable reconciliation decisions in stable newest-first order.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid limit, malformed persisted data, poisoned state, or
    /// `SQLite` failure.
    pub fn list_cost_reconciliations(
        &self,
        limit: usize,
    ) -> Result<Vec<AutomationCostReconciliation>, SqlitePersistenceError> {
        if !(1..=200).contains(&limit) {
            return Err(SqlitePersistenceError::InvalidAutomationGovernance);
        }
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let mut statement = connection.prepare(
            "SELECT decision_id, task_id, run_id, automation_id,
                    reserved_cost_microunits, actual_cost_microunits, reason, decided_at_ms
             FROM automation_cost_reconciliation
             ORDER BY decided_at_ms DESC, decision_id DESC LIMIT ?1",
        )?;
        statement
            .query_map(
                [i64::try_from(limit)
                    .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, i64>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, String>(6)?,
                        row.get::<_, i64>(7)?,
                    ))
                },
            )?
            .map(|row| {
                let (decision_id, task_id, run_id, automation_id, reserved, actual, reason, at) =
                    row?;
                Ok(AutomationCostReconciliation {
                    decision_id: parse_uuid(&decision_id)?,
                    task_id: parse_uuid(&task_id)?,
                    run_id: parse_uuid(&run_id)?,
                    automation_id,
                    reserved_cost_microunits: from_i64(reserved)?,
                    actual_cost_microunits: from_i64(actual)?,
                    reason: parse_reconciliation_reason(&reason)?,
                    decided_at_ms: from_i64(at)?,
                })
            })
            .collect()
    }

    /// Loads a privacy-safe runtime and daily cost aggregate for an Automation.
    ///
    /// # Errors
    ///
    /// Returns an error for an invalid Automation identifier or timestamp,
    /// malformed persisted data, poisoned state, or `SQLite` failure.
    pub fn snapshot(
        &self,
        automation_id: &str,
        now_ms: u64,
    ) -> Result<AutomationGovernanceSnapshot, SqlitePersistenceError> {
        validate_automation_id(automation_id)?;
        let now_ms_i64 = to_i64(now_ms)?;
        let day_bucket = to_i64(now_ms / DAY_MS)?;
        let connection = self
            .connection
            .lock()
            .map_err(|_| SqlitePersistenceError::StatePoisoned)?;
        let active_runs = connection.query_row(
            "SELECT COUNT(*) FROM automation_run_lease
             WHERE automation_id = ?1 AND expires_at_ms > ?2",
            params![automation_id, now_ms_i64],
            |row| row.get::<_, i64>(0),
        )?;
        let last_started_at_ms = connection
            .query_row(
                "SELECT last_started_at_ms FROM automation_governance_state
                 WHERE automation_id = ?1",
                [automation_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?;
        let costs = connection.query_row(
            "SELECT
                COALESCE(SUM(CASE WHEN status = 'reserved'
                    THEN reserved_cost_microunits ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'settled'
                    THEN actual_cost_microunits ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'indeterminate'
                    THEN reserved_cost_microunits ELSE 0 END), 0),
                COALESCE(SUM(CASE WHEN status = 'indeterminate' THEN 1 ELSE 0 END), 0)
             FROM automation_cost_ledger
             WHERE automation_id = ?1 AND day_bucket = ?2",
            params![automation_id, day_bucket],
            |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, i64>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                ))
            },
        )?;
        Ok(AutomationGovernanceSnapshot {
            active_runs: from_i64(active_runs)?,
            last_started_at_ms: last_started_at_ms.map(from_i64).transpose()?,
            reserved_cost_microunits: from_i64(costs.0)?,
            settled_cost_microunits: from_i64(costs.1)?,
            indeterminate_cost_microunits: from_i64(costs.2)?,
            indeterminate_cost_count: from_i64(costs.3)?,
        })
    }
}

pub(crate) fn ensure_automation_governance_schema(
    connection: &Connection,
) -> Result<(), SqlitePersistenceError> {
    connection.execute_batch(
        "CREATE TABLE IF NOT EXISTS automation_governance_state (
            automation_id TEXT PRIMARY KEY,
            last_started_at_ms INTEGER NOT NULL CHECK (last_started_at_ms >= 0)
        );
        CREATE TABLE IF NOT EXISTS automation_run_lease (
            run_id TEXT PRIMARY KEY,
            automation_id TEXT NOT NULL,
            acquired_at_ms INTEGER NOT NULL CHECK (acquired_at_ms >= 0),
            expires_at_ms INTEGER NOT NULL CHECK (expires_at_ms > acquired_at_ms),
            daily_cost_budget_microunits INTEGER NOT NULL
                CHECK (daily_cost_budget_microunits >= 0)
        );
        CREATE INDEX IF NOT EXISTS automation_run_lease_active_idx
            ON automation_run_lease(automation_id, expires_at_ms);
        CREATE TABLE IF NOT EXISTS automation_cost_ledger (
            task_id TEXT PRIMARY KEY,
            run_id TEXT NOT NULL,
            automation_id TEXT NOT NULL,
            day_bucket INTEGER NOT NULL CHECK (day_bucket >= 0),
            reserved_cost_microunits INTEGER NOT NULL CHECK (reserved_cost_microunits >= 0),
            actual_cost_microunits INTEGER CHECK (actual_cost_microunits >= 0),
            status TEXT NOT NULL CHECK (status IN ('reserved', 'settled', 'indeterminate')),
            created_at_ms INTEGER NOT NULL CHECK (created_at_ms >= 0),
            updated_at_ms INTEGER NOT NULL CHECK (updated_at_ms >= created_at_ms),
            CHECK ((status = 'settled' AND actual_cost_microunits IS NOT NULL)
                OR (status != 'settled' AND actual_cost_microunits IS NULL))
        );
        CREATE INDEX IF NOT EXISTS automation_cost_daily_idx
            ON automation_cost_ledger(automation_id, day_bucket, status);
        CREATE TABLE IF NOT EXISTS automation_cost_reconciliation (
            decision_id TEXT PRIMARY KEY,
            task_id TEXT NOT NULL UNIQUE,
            run_id TEXT NOT NULL,
            automation_id TEXT NOT NULL,
            reserved_cost_microunits INTEGER NOT NULL CHECK (reserved_cost_microunits >= 0),
            actual_cost_microunits INTEGER NOT NULL CHECK (actual_cost_microunits >= 0),
            reason TEXT NOT NULL CHECK (reason IN
                ('provider_statement', 'billing_export', 'operator_conservative_estimate')),
            decided_at_ms INTEGER NOT NULL CHECK (decided_at_ms >= 0)
        );
        CREATE INDEX IF NOT EXISTS automation_cost_reconciliation_audit_idx
            ON automation_cost_reconciliation(automation_id, decided_at_ms, decision_id);",
    )?;
    Ok(())
}

fn validate_run_admission(
    admission: &AutomationRunAdmission,
) -> Result<(), SqlitePersistenceError> {
    if admission.run_id.is_nil()
        || admission.automation_id.is_empty()
        || admission.automation_id.len() > 128
        || admission.automation_id.chars().any(char::is_control)
        || admission.max_concurrent_runs == 0
        || admission.lease_expires_at_ms <= admission.now_ms
    {
        return Err(SqlitePersistenceError::InvalidAutomationGovernance);
    }
    Ok(())
}

fn validate_automation_id(automation_id: &str) -> Result<(), SqlitePersistenceError> {
    if automation_id.is_empty()
        || automation_id.len() > 128
        || automation_id.chars().any(char::is_control)
    {
        return Err(SqlitePersistenceError::InvalidAutomationGovernance);
    }
    Ok(())
}

fn from_i64(value: i64) -> Result<u64, SqlitePersistenceError> {
    u64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)
}

fn validate_cost_reservation(
    reservation: AutomationCostReservation,
) -> Result<(), SqlitePersistenceError> {
    if reservation.task_id.is_nil() || reservation.run_id.is_nil() {
        return Err(SqlitePersistenceError::InvalidAutomationGovernance);
    }
    Ok(())
}

fn validate_reconciliation(
    request: &ReconcileAutomationCostRequest,
) -> Result<(), SqlitePersistenceError> {
    if request.decision_id.is_nil() || request.task_id.is_nil() {
        return Err(SqlitePersistenceError::InvalidAutomationGovernance);
    }
    Ok(())
}

fn reconciliation_reason_name(reason: AutomationCostReconciliationReason) -> &'static str {
    match reason {
        AutomationCostReconciliationReason::ProviderStatement => "provider_statement",
        AutomationCostReconciliationReason::BillingExport => "billing_export",
        AutomationCostReconciliationReason::OperatorConservativeEstimate => {
            "operator_conservative_estimate"
        }
    }
}

fn parse_reconciliation_reason(
    reason: &str,
) -> Result<AutomationCostReconciliationReason, SqlitePersistenceError> {
    match reason {
        "provider_statement" => Ok(AutomationCostReconciliationReason::ProviderStatement),
        "billing_export" => Ok(AutomationCostReconciliationReason::BillingExport),
        "operator_conservative_estimate" => {
            Ok(AutomationCostReconciliationReason::OperatorConservativeEstimate)
        }
        _ => Err(SqlitePersistenceError::InvalidAutomationGovernance),
    }
}

fn parse_uuid(value: &str) -> Result<Uuid, SqlitePersistenceError> {
    Uuid::parse_str(value).map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)
}

fn load_cost_entry(
    connection: &Connection,
    task_id: Uuid,
) -> Result<Option<AutomationCostEntry>, SqlitePersistenceError> {
    connection
        .query_row(
            "SELECT run_id, automation_id, day_bucket, reserved_cost_microunits,
                    actual_cost_microunits, status, updated_at_ms
             FROM automation_cost_ledger WHERE task_id = ?1",
            [task_id.to_string()],
            |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, Option<i64>>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, i64>(6)?,
                ))
            },
        )
        .optional()?
        .map(
            |(run_id, automation_id, day_bucket, reserved, actual, status, updated_at_ms)| {
                Ok(AutomationCostEntry {
                    task_id,
                    run_id: Uuid::parse_str(&run_id)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?,
                    automation_id,
                    day_bucket: u64::try_from(day_bucket)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?,
                    reserved_cost_microunits: u64::try_from(reserved)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?,
                    actual_cost_microunits: actual
                        .map(u64::try_from)
                        .transpose()
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?,
                    status: match status.as_str() {
                        "reserved" => AutomationCostStatus::Reserved,
                        "settled" => AutomationCostStatus::Settled,
                        "indeterminate" => AutomationCostStatus::Indeterminate,
                        _ => return Err(SqlitePersistenceError::InvalidAutomationGovernance),
                    },
                    updated_at_ms: u64::try_from(updated_at_ms)
                        .map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)?,
                })
            },
        )
        .transpose()
}

fn to_i64(value: u64) -> Result<i64, SqlitePersistenceError> {
    i64::try_from(value).map_err(|_| SqlitePersistenceError::InvalidAutomationGovernance)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::{Arc, Barrier};

    fn admission(run_id: Uuid, now_ms: u64) -> AutomationRunAdmission {
        AutomationRunAdmission {
            run_id,
            automation_id: "automation.focus".to_owned(),
            max_concurrent_runs: 1,
            cooldown_ms: 0,
            daily_cost_budget_microunits: 100,
            now_ms,
            lease_expires_at_ms: now_ms + 1_000,
        }
    }

    #[test]
    fn concurrency_and_cooldown_are_fail_closed() {
        let store = SqliteAutomationGovernance::in_memory().expect("store");
        store
            .admit_run(&admission(Uuid::now_v7(), 100))
            .expect("first");
        let second = Uuid::now_v7();
        assert!(matches!(
            store.admit_run(&admission(second, 101)),
            Err(SqlitePersistenceError::AutomationConcurrencyExceeded)
        ));
        store.recover(1_100).expect("recover");
        let mut cooled = admission(second, 1_100);
        cooled.cooldown_ms = 2_000;
        assert!(matches!(
            store.admit_run(&cooled),
            Err(SqlitePersistenceError::AutomationCooldownActive)
        ));
    }

    #[test]
    fn reservation_is_replaced_by_actual_cost() {
        let store = SqliteAutomationGovernance::in_memory().expect("store");
        let run_id = Uuid::now_v7();
        let task_id = Uuid::now_v7();
        store.admit_run(&admission(run_id, 100)).expect("run");
        store
            .reserve_agent_cost(AutomationCostReservation {
                task_id,
                run_id,
                reserved_cost_microunits: 80,
                now_ms: 101,
            })
            .expect("reserve");
        store.settle_agent_cost(task_id, 30, 102).expect("settle");
        let entry = store.get_cost(task_id).expect("load").expect("entry");
        assert_eq!(entry.status, AutomationCostStatus::Settled);
        assert_eq!(entry.actual_cost_microunits, Some(30));
    }

    #[test]
    fn snapshot_separates_active_reserved_settled_and_indeterminate_costs() {
        let store = SqliteAutomationGovernance::in_memory().expect("store");
        let run_id = Uuid::now_v7();
        store.admit_run(&admission(run_id, 100)).expect("run");
        let settled_task = Uuid::now_v7();
        store
            .reserve_agent_cost(AutomationCostReservation {
                task_id: settled_task,
                run_id,
                reserved_cost_microunits: 40,
                now_ms: 101,
            })
            .expect("reserve settled");
        store
            .settle_agent_cost(settled_task, 15, 102)
            .expect("settle");
        let unknown_task = Uuid::now_v7();
        store
            .reserve_agent_cost(AutomationCostReservation {
                task_id: unknown_task,
                run_id,
                reserved_cost_microunits: 30,
                now_ms: 103,
            })
            .expect("reserve unknown");
        store
            .mark_agent_cost_indeterminate(unknown_task, 104)
            .expect("unknown");
        let reserved_task = Uuid::now_v7();
        store
            .reserve_agent_cost(AutomationCostReservation {
                task_id: reserved_task,
                run_id,
                reserved_cost_microunits: 20,
                now_ms: 105,
            })
            .expect("reserve active");

        assert_eq!(
            store.snapshot("automation.focus", 106).expect("snapshot"),
            AutomationGovernanceSnapshot {
                active_runs: 1,
                last_started_at_ms: Some(100),
                reserved_cost_microunits: 20,
                settled_cost_microunits: 15,
                indeterminate_cost_microunits: 30,
                indeterminate_cost_count: 1,
            }
        );
        assert_eq!(
            store
                .snapshot("automation.focus", DAY_MS)
                .expect("next day"),
            AutomationGovernanceSnapshot {
                active_runs: 0,
                last_started_at_ms: Some(100),
                reserved_cost_microunits: 0,
                settled_cost_microunits: 0,
                indeterminate_cost_microunits: 0,
                indeterminate_cost_count: 0,
            }
        );
    }

    #[test]
    fn indeterminate_cost_keeps_the_full_reservation() {
        let store = SqliteAutomationGovernance::in_memory().expect("store");
        let first_run = Uuid::now_v7();
        let first_task = Uuid::now_v7();
        store.admit_run(&admission(first_run, 100)).expect("run");
        store
            .reserve_agent_cost(AutomationCostReservation {
                task_id: first_task,
                run_id: first_run,
                reserved_cost_microunits: 80,
                now_ms: 101,
            })
            .expect("reserve");
        store
            .mark_agent_cost_indeterminate(first_task, 102)
            .expect("indeterminate");
        store.release_run(first_run).expect("release");
        let second_run = Uuid::now_v7();
        store
            .admit_run(&admission(second_run, 103))
            .expect("next run");
        assert!(matches!(
            store.reserve_agent_cost(AutomationCostReservation {
                task_id: Uuid::now_v7(),
                run_id: second_run,
                reserved_cost_microunits: 30,
                now_ms: 104,
            }),
            Err(SqlitePersistenceError::AutomationDailyCostBudgetExceeded)
        ));
    }

    #[test]
    fn actual_cost_cannot_exceed_reservation() {
        let store = SqliteAutomationGovernance::in_memory().expect("store");
        let run_id = Uuid::now_v7();
        let task_id = Uuid::now_v7();
        store.admit_run(&admission(run_id, 100)).expect("run");
        store
            .reserve_agent_cost(AutomationCostReservation {
                task_id,
                run_id,
                reserved_cost_microunits: 10,
                now_ms: 101,
            })
            .expect("reserve");
        assert!(matches!(
            store.settle_agent_cost(task_id, 11, 102),
            Err(SqlitePersistenceError::AutomationCostReservationConflict)
        ));
        assert_eq!(
            store
                .get_cost(task_id)
                .expect("load")
                .expect("entry")
                .status,
            AutomationCostStatus::Reserved
        );
    }

    #[test]
    fn cooldown_boundary_and_state_survive_reopen() {
        let path = std::env::temp_dir().join(format!(
            "nimora-automation-governance-{}.sqlite3",
            Uuid::now_v7()
        ));
        let first_run = Uuid::now_v7();
        {
            let store = SqliteAutomationGovernance::open(&path).expect("store");
            let mut first = admission(first_run, 100);
            first.cooldown_ms = 1_000;
            store.admit_run(&first).expect("first");
            store.release_run(first_run).expect("release");
        }
        let store = SqliteAutomationGovernance::open(&path).expect("reopen");
        let mut too_early = admission(Uuid::now_v7(), 1_099);
        too_early.cooldown_ms = 1_000;
        assert!(matches!(
            store.admit_run(&too_early),
            Err(SqlitePersistenceError::AutomationCooldownActive)
        ));
        let mut boundary = admission(Uuid::now_v7(), 1_100);
        boundary.cooldown_ms = 1_000;
        store.admit_run(&boundary).expect("boundary");
        drop(store);
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
    }

    #[test]
    fn concurrent_connections_admit_only_one_run() {
        let path = std::env::temp_dir().join(format!(
            "nimora-automation-governance-race-{}.sqlite3",
            Uuid::now_v7()
        ));
        drop(SqliteAutomationGovernance::open(&path).expect("initialize schema"));
        let barrier = Arc::new(Barrier::new(2));
        let handles = [Uuid::now_v7(), Uuid::now_v7()].map(|run_id| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let store = SqliteAutomationGovernance::open(path).expect("store");
                barrier.wait();
                store.admit_run(&admission(run_id, 100))
            })
        });
        let results = handles.map(|handle| handle.join().expect("thread"));
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(SqlitePersistenceError::AutomationConcurrencyExceeded)
                ))
                .count(),
            1
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
    }

    #[test]
    fn reconciles_unknown_cost_once_with_immutable_audit() {
        let store = SqliteAutomationGovernance::in_memory().expect("store");
        let run_id = Uuid::now_v7();
        let task_id = Uuid::now_v7();
        store.admit_run(&admission(run_id, 100)).expect("run");
        store
            .reserve_agent_cost(AutomationCostReservation {
                task_id,
                run_id,
                reserved_cost_microunits: 80,
                now_ms: 101,
            })
            .expect("reserve");
        store
            .mark_agent_cost_indeterminate(task_id, 102)
            .expect("unknown");
        assert_eq!(
            store.list_indeterminate_costs(20).expect("pending")[0].task_id,
            task_id
        );
        let request = ReconcileAutomationCostRequest {
            decision_id: Uuid::now_v7(),
            task_id,
            expected_updated_at_ms: 102,
            actual_cost_microunits: 135,
            reason: AutomationCostReconciliationReason::ProviderStatement,
            decided_at_ms: 103,
        };
        let receipt = store
            .reconcile_indeterminate_cost(&request)
            .expect("reconcile");
        assert_eq!(receipt.actual_cost_microunits, 135);
        assert!(
            store
                .list_indeterminate_costs(20)
                .expect("pending")
                .is_empty()
        );
        let entry = store.get_cost(task_id).expect("load").expect("entry");
        assert_eq!(entry.status, AutomationCostStatus::Settled);
        assert_eq!(entry.actual_cost_microunits, Some(135));
        assert_eq!(
            store.list_cost_reconciliations(20).expect("audit"),
            vec![receipt]
        );
        assert!(matches!(
            store.reconcile_indeterminate_cost(&request),
            Err(SqlitePersistenceError::AutomationCostReconciliationConflict)
        ));
        let mut excessive = request;
        excessive.decision_id = Uuid::now_v7();
        excessive.actual_cost_microunits = 81;
        assert!(matches!(
            store.reconcile_indeterminate_cost(&excessive),
            Err(SqlitePersistenceError::AutomationCostReconciliationConflict)
        ));
    }

    #[test]
    fn restart_marks_unknown_cost_and_releases_all_process_leases() {
        let store = SqliteAutomationGovernance::in_memory().expect("store");
        let run_id = Uuid::now_v7();
        let task_id = Uuid::now_v7();
        store.admit_run(&admission(run_id, 100)).expect("run");
        store
            .reserve_agent_cost(AutomationCostReservation {
                task_id,
                run_id,
                reserved_cost_microunits: 80,
                now_ms: 101,
            })
            .expect("reserve");
        assert_eq!(store.recover(102).expect("recover"), 1);
        assert_eq!(
            store
                .get_cost(task_id)
                .expect("load")
                .expect("entry")
                .status,
            AutomationCostStatus::Indeterminate
        );
        store
            .admit_run(&admission(Uuid::now_v7(), 103))
            .expect("new process run");
    }

    #[test]
    fn concurrent_cost_reservations_have_one_budget_winner() {
        let path = std::env::temp_dir().join(format!(
            "nimora-automation-cost-race-{}.sqlite3",
            Uuid::now_v7()
        ));
        let first_run = Uuid::now_v7();
        let second_run = Uuid::now_v7();
        {
            let store = SqliteAutomationGovernance::open(&path).expect("store");
            let mut first = admission(first_run, 100);
            first.max_concurrent_runs = 2;
            store.admit_run(&first).expect("first run");
            let mut second = admission(second_run, 101);
            second.max_concurrent_runs = 2;
            store.admit_run(&second).expect("second run");
        }
        let barrier = Arc::new(Barrier::new(2));
        let handles = [first_run, second_run].map(|run_id| {
            let path = path.clone();
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                let store = SqliteAutomationGovernance::open(path).expect("store");
                barrier.wait();
                store.reserve_agent_cost(AutomationCostReservation {
                    task_id: Uuid::now_v7(),
                    run_id,
                    reserved_cost_microunits: 60,
                    now_ms: 102,
                })
            })
        });
        let results = handles.map(|handle| handle.join().expect("thread"));
        assert_eq!(results.iter().filter(|result| result.is_ok()).count(), 1);
        assert_eq!(
            results
                .iter()
                .filter(|result| matches!(
                    result,
                    Err(SqlitePersistenceError::AutomationDailyCostBudgetExceeded)
                ))
                .count(),
            1
        );
        let _ = std::fs::remove_file(&path);
        let _ = std::fs::remove_file(path.with_extension("sqlite3-wal"));
        let _ = std::fs::remove_file(path.with_extension("sqlite3-shm"));
    }
}
