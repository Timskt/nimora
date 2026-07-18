use nimora_agent_runtime::CancellationFlag;
use serde::Serialize;
use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicU8, Ordering},
    },
};
use thiserror::Error;
use uuid::Uuid;

const CONTROL_CONTINUE: u8 = 0;
const CONTROL_PAUSE: u8 = 1;
const CONTROL_CANCEL: u8 = 2;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AutoModeJobStatus {
    Starting,
    Running,
    Pausing,
    Cancelling,
    Paused,
    Completed,
    Cancelled,
    Failed,
    Indeterminate,
}

impl AutoModeJobStatus {
    const fn is_terminal(self) -> bool {
        matches!(
            self,
            Self::Paused | Self::Completed | Self::Cancelled | Self::Failed | Self::Indeterminate
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AutoModeJobSnapshot {
    pub spec: &'static str,
    pub job_id: Uuid,
    pub session_id: Uuid,
    pub status: AutoModeJobStatus,
    pub turns_executed: u64,
    pub cache_hits: u64,
    pub checkpoint_sequence: u64,
    pub pause_reason: Option<String>,
    pub error_code: Option<String>,
    pub started_at_ms: u64,
    pub updated_at_ms: u64,
}

impl AutoModeJobSnapshot {
    fn starting(job_id: Uuid, session_id: Uuid, now_ms: u64) -> Self {
        Self {
            spec: "nimora.desktop-auto-mode-job/1",
            job_id,
            session_id,
            status: AutoModeJobStatus::Starting,
            turns_executed: 0,
            cache_hits: 0,
            checkpoint_sequence: 0,
            pause_reason: None,
            error_code: None,
            started_at_ms: now_ms,
            updated_at_ms: now_ms,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoModeJobControl {
    Continue,
    Pause,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct AutoModeJobControlHandle {
    control: Arc<AtomicU8>,
    cancellation: CancellationFlag,
}

impl AutoModeJobControlHandle {
    /// Returns the strongest control request currently visible to the runner.
    #[must_use]
    pub fn requested(&self) -> AutoModeJobControl {
        match self.control.load(Ordering::Acquire) {
            CONTROL_PAUSE => AutoModeJobControl::Pause,
            CONTROL_CANCEL => AutoModeJobControl::Cancel,
            _ => AutoModeJobControl::Continue,
        }
    }

    /// Returns the cancellation flag shared with the active Provider and Tool turn.
    #[must_use]
    pub fn cancellation(&self) -> CancellationFlag {
        self.cancellation.clone()
    }
}

#[derive(Debug)]
struct AutoModeJobRecord {
    snapshot: AutoModeJobSnapshot,
    control: Arc<AtomicU8>,
    cancellation: CancellationFlag,
}

#[derive(Debug, Default)]
struct AutoModeJobRegistryState {
    jobs: HashMap<Uuid, AutoModeJobRecord>,
    active_sessions: HashMap<Uuid, Uuid>,
}

#[derive(Debug, Default)]
pub struct AutoModeJobSupervisor {
    state: Mutex<AutoModeJobRegistryState>,
}

impl AutoModeJobSupervisor {
    /// Returns retained snapshots for all currently active jobs.
    ///
    /// # Errors
    ///
    /// Returns an error when the registry is unavailable.
    pub fn active_snapshots(&self) -> Result<Vec<AutoModeJobSnapshot>, AutoModeJobError> {
        let state = self.state.lock().map_err(|_| AutoModeJobError::Poisoned)?;
        let mut snapshots = state
            .active_sessions
            .values()
            .filter_map(|job_id| state.jobs.get(job_id))
            .map(|record| record.snapshot.clone())
            .collect::<Vec<_>>();
        snapshots.sort_unstable_by_key(|snapshot| snapshot.started_at_ms);
        Ok(snapshots)
    }

    /// Atomically reserves a new job for one persistent Auto Mode session.
    ///
    /// # Errors
    ///
    /// Returns an error when the session is already active or the registry is unavailable.
    pub fn start(
        &self,
        session_id: Uuid,
        now_ms: u64,
    ) -> Result<(AutoModeJobSnapshot, AutoModeJobControlHandle), AutoModeJobError> {
        let mut state = self.state.lock().map_err(|_| AutoModeJobError::Poisoned)?;
        if state.active_sessions.contains_key(&session_id) {
            return Err(AutoModeJobError::SessionAlreadyActive);
        }
        let job_id = Uuid::now_v7();
        let snapshot = AutoModeJobSnapshot::starting(job_id, session_id, now_ms);
        let control = Arc::new(AtomicU8::new(CONTROL_CONTINUE));
        let cancellation = CancellationFlag::default();
        state.active_sessions.insert(session_id, job_id);
        state.jobs.insert(
            job_id,
            AutoModeJobRecord {
                snapshot: snapshot.clone(),
                control: Arc::clone(&control),
                cancellation: cancellation.clone(),
            },
        );
        Ok((
            snapshot,
            AutoModeJobControlHandle {
                control,
                cancellation,
            },
        ))
    }

    /// Returns the latest immutable snapshot retained for a job.
    ///
    /// # Errors
    ///
    /// Returns an error when the job does not exist or the registry is unavailable.
    pub fn snapshot(&self, job_id: Uuid) -> Result<AutoModeJobSnapshot, AutoModeJobError> {
        self.state
            .lock()
            .map_err(|_| AutoModeJobError::Poisoned)?
            .jobs
            .get(&job_id)
            .map(|record| record.snapshot.clone())
            .ok_or(AutoModeJobError::NotFound)
    }

    /// Transitions a newly reserved job into active execution.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing job, invalid transition, or unavailable registry.
    pub fn mark_running(
        &self,
        job_id: Uuid,
        now_ms: u64,
    ) -> Result<AutoModeJobSnapshot, AutoModeJobError> {
        self.update(job_id, now_ms, |snapshot| {
            if snapshot.status != AutoModeJobStatus::Starting {
                return Err(AutoModeJobError::InvalidTransition);
            }
            snapshot.status = AutoModeJobStatus::Running;
            Ok(())
        })
    }

    /// Adds one bounded loop batch to the job's monotonic progress counters.
    ///
    /// # Errors
    ///
    /// Returns an error unless the job is running and the registry is available.
    pub fn record_batch(
        &self,
        job_id: Uuid,
        turns_executed: u16,
        cache_hits: u16,
        checkpoint_sequence: u64,
        now_ms: u64,
    ) -> Result<AutoModeJobSnapshot, AutoModeJobError> {
        self.update(job_id, now_ms, |snapshot| {
            if snapshot.status != AutoModeJobStatus::Running {
                return Err(AutoModeJobError::InvalidTransition);
            }
            snapshot.turns_executed = snapshot
                .turns_executed
                .saturating_add(u64::from(turns_executed));
            snapshot.cache_hits = snapshot.cache_hits.saturating_add(u64::from(cache_hits));
            snapshot.checkpoint_sequence = checkpoint_sequence;
            Ok(())
        })
    }

    /// Requests a cooperative pause without releasing session ownership prematurely.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing or terminal job, or an unavailable registry.
    pub fn request_pause(
        &self,
        job_id: Uuid,
        now_ms: u64,
    ) -> Result<AutoModeJobSnapshot, AutoModeJobError> {
        self.request_control(job_id, CONTROL_PAUSE, AutoModeJobStatus::Pausing, now_ms)
    }

    /// Requests cancellation, overriding an earlier cooperative pause request.
    ///
    /// # Errors
    ///
    /// Returns an error for a missing or terminal job, or an unavailable registry.
    pub fn request_cancel(
        &self,
        job_id: Uuid,
        now_ms: u64,
    ) -> Result<AutoModeJobSnapshot, AutoModeJobError> {
        self.request_control(
            job_id,
            CONTROL_CANCEL,
            AutoModeJobStatus::Cancelling,
            now_ms,
        )
    }

    /// Cooperatively cancels every active job without holding the registry during runner work.
    ///
    /// # Errors
    ///
    /// Returns an error when the registry is unavailable or time moves backwards.
    pub fn request_cancel_all(
        &self,
        now_ms: u64,
    ) -> Result<Vec<AutoModeJobSnapshot>, AutoModeJobError> {
        let mut state = self.state.lock().map_err(|_| AutoModeJobError::Poisoned)?;
        let active_job_ids = state.active_sessions.values().copied().collect::<Vec<_>>();
        let mut snapshots = Vec::with_capacity(active_job_ids.len());
        for job_id in active_job_ids {
            let record = state
                .jobs
                .get_mut(&job_id)
                .ok_or(AutoModeJobError::RegistryCorrupt)?;
            if now_ms < record.snapshot.updated_at_ms {
                return Err(AutoModeJobError::TimeMovedBackwards);
            }
            record.control.store(CONTROL_CANCEL, Ordering::Release);
            record.cancellation.cancel();
            record.snapshot.status = AutoModeJobStatus::Cancelling;
            record.snapshot.updated_at_ms = now_ms;
            snapshots.push(record.snapshot.clone());
        }
        snapshots.sort_unstable_by_key(|snapshot| snapshot.started_at_ms);
        Ok(snapshots)
    }

    /// Records a terminal outcome and releases the session for a later job.
    ///
    /// # Errors
    ///
    /// Returns an error for a non-terminal outcome, missing job, repeated finish, or unavailable
    /// registry.
    pub fn finish(
        &self,
        job_id: Uuid,
        status: AutoModeJobStatus,
        pause_reason: Option<String>,
        error_code: Option<String>,
        now_ms: u64,
    ) -> Result<AutoModeJobSnapshot, AutoModeJobError> {
        if !status.is_terminal() {
            return Err(AutoModeJobError::InvalidTransition);
        }
        let mut state = self.state.lock().map_err(|_| AutoModeJobError::Poisoned)?;
        let session_id = {
            let record = state
                .jobs
                .get_mut(&job_id)
                .ok_or(AutoModeJobError::NotFound)?;
            if record.snapshot.status.is_terminal() {
                return Err(AutoModeJobError::InvalidTransition);
            }
            if now_ms < record.snapshot.updated_at_ms {
                return Err(AutoModeJobError::TimeMovedBackwards);
            }
            record.snapshot.status = status;
            record.snapshot.pause_reason = pause_reason;
            record.snapshot.error_code = error_code;
            record.snapshot.updated_at_ms = now_ms;
            record.snapshot.session_id
        };
        state.active_sessions.remove(&session_id);
        Ok(state.jobs[&job_id].snapshot.clone())
    }

    fn request_control(
        &self,
        job_id: Uuid,
        control: u8,
        status: AutoModeJobStatus,
        now_ms: u64,
    ) -> Result<AutoModeJobSnapshot, AutoModeJobError> {
        let mut state = self.state.lock().map_err(|_| AutoModeJobError::Poisoned)?;
        let record = state
            .jobs
            .get_mut(&job_id)
            .ok_or(AutoModeJobError::NotFound)?;
        if !matches!(
            record.snapshot.status,
            AutoModeJobStatus::Starting | AutoModeJobStatus::Running | AutoModeJobStatus::Pausing
        ) {
            return Err(AutoModeJobError::InvalidTransition);
        }
        if now_ms < record.snapshot.updated_at_ms {
            return Err(AutoModeJobError::TimeMovedBackwards);
        }
        record.control.store(control, Ordering::Release);
        record.cancellation.cancel();
        record.snapshot.status = status;
        record.snapshot.updated_at_ms = now_ms;
        Ok(record.snapshot.clone())
    }

    fn update(
        &self,
        job_id: Uuid,
        now_ms: u64,
        update: impl FnOnce(&mut AutoModeJobSnapshot) -> Result<(), AutoModeJobError>,
    ) -> Result<AutoModeJobSnapshot, AutoModeJobError> {
        let mut state = self.state.lock().map_err(|_| AutoModeJobError::Poisoned)?;
        let record = state
            .jobs
            .get_mut(&job_id)
            .ok_or(AutoModeJobError::NotFound)?;
        if now_ms < record.snapshot.updated_at_ms {
            return Err(AutoModeJobError::TimeMovedBackwards);
        }
        update(&mut record.snapshot)?;
        record.snapshot.updated_at_ms = now_ms;
        Ok(record.snapshot.clone())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AutoModeJobError {
    #[error("Auto Mode session already has an active desktop job")]
    SessionAlreadyActive,
    #[error("Auto Mode desktop job was not found")]
    NotFound,
    #[error("Auto Mode desktop job transition is invalid")]
    InvalidTransition,
    #[error("Auto Mode desktop job time moved backwards")]
    TimeMovedBackwards,
    #[error("Auto Mode desktop job registry is inconsistent")]
    RegistryCorrupt,
    #[error("Auto Mode desktop job registry is unavailable")]
    Poisoned,
}

#[cfg(test)]
mod tests {
    use super::{AutoModeJobControl, AutoModeJobError, AutoModeJobStatus, AutoModeJobSupervisor};
    use uuid::Uuid;

    #[test]
    fn enforces_one_active_job_per_session() {
        let supervisor = AutoModeJobSupervisor::default();
        let session_id = Uuid::now_v7();
        supervisor.start(session_id, 100).expect("first job");

        assert!(matches!(
            supervisor.start(session_id, 101),
            Err(AutoModeJobError::SessionAlreadyActive)
        ));
    }

    #[test]
    fn publishes_pause_and_cancel_to_runner() {
        let supervisor = AutoModeJobSupervisor::default();
        let (job, control) = supervisor.start(Uuid::now_v7(), 100).expect("job");
        let cancellation = control.cancellation();
        supervisor.mark_running(job.job_id, 101).expect("running");
        supervisor.request_pause(job.job_id, 102).expect("pause");
        assert_eq!(control.requested(), AutoModeJobControl::Pause);
        assert!(cancellation.is_cancelled());
        supervisor.request_cancel(job.job_id, 103).expect("cancel");
        assert_eq!(control.requested(), AutoModeJobControl::Cancel);
    }

    #[test]
    fn terminal_job_releases_session_and_retains_snapshot() {
        let supervisor = AutoModeJobSupervisor::default();
        let session_id = Uuid::now_v7();
        let (job, _) = supervisor.start(session_id, 100).expect("job");
        supervisor.mark_running(job.job_id, 101).expect("running");
        supervisor
            .record_batch(job.job_id, 3, 2, 9, 102)
            .expect("batch");
        let completed = supervisor
            .finish(job.job_id, AutoModeJobStatus::Completed, None, None, 103)
            .expect("finish");

        assert_eq!(completed.turns_executed, 3);
        assert_eq!(completed.cache_hits, 2);
        assert_eq!(completed.checkpoint_sequence, 9);
        assert_eq!(
            supervisor.snapshot(job.job_id).expect("snapshot"),
            completed
        );
        supervisor.start(session_id, 104).expect("replacement job");
    }

    #[test]
    fn cancels_all_active_jobs_without_touching_history() {
        let supervisor = AutoModeJobSupervisor::default();
        let (first, first_control) = supervisor.start(Uuid::now_v7(), 100).expect("first");
        let (second, second_control) = supervisor.start(Uuid::now_v7(), 101).expect("second");
        supervisor.mark_running(first.job_id, 102).expect("running");
        supervisor
            .finish(second.job_id, AutoModeJobStatus::Completed, None, None, 102)
            .expect("completed");
        let (third, third_control) = supervisor.start(Uuid::now_v7(), 103).expect("third");

        let cancelled = supervisor.request_cancel_all(104).expect("cancel all");

        assert_eq!(cancelled.len(), 2);
        assert!(
            cancelled
                .iter()
                .all(|job| job.status == AutoModeJobStatus::Cancelling)
        );
        assert_eq!(first_control.requested(), AutoModeJobControl::Cancel);
        assert_eq!(third_control.requested(), AutoModeJobControl::Cancel);
        assert_eq!(second_control.requested(), AutoModeJobControl::Continue);
        assert_eq!(
            supervisor.snapshot(second.job_id).expect("history").status,
            AutoModeJobStatus::Completed
        );
        assert_eq!(supervisor.active_snapshots().expect("active").len(), 2);
        assert_eq!(third.job_id, cancelled[1].job_id);
    }

    #[test]
    fn rejects_time_regression() {
        let supervisor = AutoModeJobSupervisor::default();
        let (job, _) = supervisor.start(Uuid::now_v7(), 100).expect("job");

        assert_eq!(
            supervisor.mark_running(job.job_id, 99),
            Err(AutoModeJobError::TimeMovedBackwards)
        );
    }
}
