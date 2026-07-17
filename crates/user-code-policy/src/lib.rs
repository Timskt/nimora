use serde::{Deserialize, Serialize};
use std::collections::VecDeque;
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, AtomicUsize, Ordering},
};
use std::time::{Duration, Instant};
use thiserror::Error;
use uuid::Uuid;

const MAX_SUBSCRIPTIONS: usize = 32;
const MAX_COMMANDS: usize = 32;
const MAX_RUNTIME_MS: u64 = 30_000;
const MAX_MEMORY_BYTES: u64 = 64 * 1024 * 1024;
const MAX_OUTPUT_BYTES: u64 = 1024 * 1024;
const MAX_CONCURRENT_EXECUTIONS: usize = 8;
const MAX_EVENT_QUEUE_CAPACITY: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventConcurrencyPolicy {
    Serial,
    Drop,
    CancelPrevious,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScheduledEvent<T> {
    pub execution_id: Uuid,
    pub event: T,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventAdmission<T> {
    Start(ScheduledEvent<T>),
    Queued,
    Dropped,
    CancelAndStart {
        cancelled_execution_id: Uuid,
        next: ScheduledEvent<T>,
    },
}

#[derive(Debug)]
pub struct EventTriggerScheduler<T> {
    policy: EventConcurrencyPolicy,
    capacity: usize,
    active_execution_id: Option<Uuid>,
    queue: VecDeque<T>,
    dropped: u64,
    generation: u64,
}

impl<T> EventTriggerScheduler<T> {
    #[must_use]
    pub fn new(policy: EventConcurrencyPolicy, capacity: usize) -> Self {
        Self {
            policy,
            capacity: capacity.clamp(1, MAX_EVENT_QUEUE_CAPACITY),
            active_execution_id: None,
            queue: VecDeque::new(),
            dropped: 0,
            generation: 0,
        }
    }

    #[must_use]
    pub fn admit(&mut self, event: T) -> EventAdmission<T> {
        let Some(active_execution_id) = self.active_execution_id else {
            let scheduled = self.start(event);
            return EventAdmission::Start(scheduled);
        };
        match self.policy {
            EventConcurrencyPolicy::Serial => {
                if self.queue.len() == self.capacity {
                    self.queue.pop_front();
                    self.dropped = self.dropped.saturating_add(1);
                }
                self.queue.push_back(event);
                EventAdmission::Queued
            }
            EventConcurrencyPolicy::Drop => {
                self.dropped = self.dropped.saturating_add(1);
                EventAdmission::Dropped
            }
            EventConcurrencyPolicy::CancelPrevious => {
                self.queue.clear();
                let next = self.start(event);
                EventAdmission::CancelAndStart {
                    cancelled_execution_id: active_execution_id,
                    next,
                }
            }
        }
    }

    #[must_use]
    pub fn finish(&mut self, execution_id: Uuid) -> Option<ScheduledEvent<T>> {
        if self.active_execution_id != Some(execution_id) {
            return None;
        }
        self.active_execution_id = None;
        self.queue.pop_front().map(|event| self.start(event))
    }

    pub fn cancel_all(&mut self) -> Option<Uuid> {
        self.queue.clear();
        self.generation = self.generation.saturating_add(1);
        self.active_execution_id.take()
    }

    #[must_use]
    pub const fn dropped(&self) -> u64 {
        self.dropped
    }

    #[must_use]
    pub fn queued(&self) -> usize {
        self.queue.len()
    }

    #[must_use]
    pub const fn generation(&self) -> u64 {
        self.generation
    }

    #[must_use]
    pub fn is_active(&self, execution_id: Uuid) -> bool {
        self.active_execution_id == Some(execution_id)
    }

    fn start(&mut self, event: T) -> ScheduledEvent<T> {
        let execution_id = Uuid::now_v7();
        self.active_execution_id = Some(execution_id);
        ScheduledEvent {
            execution_id,
            event,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    ReadPetState,
    ReadProfileState,
    SubscribeEvents,
    InvokeSafeCommands,
    StoreLocalData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProgramManifest {
    pub id: String,
    pub version: String,
    pub capabilities: Vec<Capability>,
    pub subscriptions: Vec<String>,
    pub event_concurrency: EventConcurrencyPolicy,
    pub event_queue_capacity: usize,
    pub commands: Vec<String>,
    pub timeout_ms: u64,
    pub memory_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
#[allow(clippy::struct_excessive_bools)]
pub struct ExecutionPolicy {
    pub manifest: ProgramManifest,
    pub can_read_pet_state: bool,
    pub can_read_profile_state: bool,
    pub can_subscribe_events: bool,
    pub can_invoke_commands: bool,
    pub can_store_local_data: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkerLimits {
    pub timeout: Duration,
    pub memory_bytes: u64,
    pub output_bytes: u64,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum WorkerError {
    #[error("maximum concurrent user programs reached")]
    ConcurrencyLimit,
    #[error("worker execution was cancelled")]
    Cancelled,
    #[error("worker execution exceeded its deadline")]
    TimedOut,
    #[error("worker output exceeded its budget")]
    OutputLimit,
}

#[derive(Debug)]
pub struct ExecutionController {
    active: Arc<AtomicUsize>,
    max_concurrent: usize,
}

impl Default for ExecutionController {
    fn default() -> Self {
        Self {
            active: Arc::new(AtomicUsize::new(0)),
            max_concurrent: MAX_CONCURRENT_EXECUTIONS,
        }
    }
}

impl ExecutionController {
    /// Reserves one bounded worker execution slot.
    ///
    /// # Errors
    ///
    /// Returns [`WorkerError::ConcurrencyLimit`] when all slots are occupied.
    pub fn admit(&self, policy: &ExecutionPolicy) -> Result<ExecutionHandle, WorkerError> {
        self.admit_with_cancellation(policy, ExecutionCancellation::default())
    }

    /// Reserves one bounded worker slot controlled by a supervisor-owned cancellation token.
    ///
    /// # Errors
    ///
    /// Returns [`WorkerError::ConcurrencyLimit`] when all slots are occupied.
    pub fn admit_with_cancellation(
        &self,
        policy: &ExecutionPolicy,
        cancellation: ExecutionCancellation,
    ) -> Result<ExecutionHandle, WorkerError> {
        let mut current = self.active.load(Ordering::Acquire);
        loop {
            if current >= self.max_concurrent {
                return Err(WorkerError::ConcurrencyLimit);
            }
            match self.active.compare_exchange_weak(
                current,
                current + 1,
                Ordering::AcqRel,
                Ordering::Acquire,
            ) {
                Ok(_) => break,
                Err(next) => current = next,
            }
        }
        Ok(ExecutionHandle {
            execution_id: Uuid::now_v7(),
            active: Arc::clone(&self.active),
            cancelled: cancellation.cancelled,
            deadline: Instant::now() + Duration::from_millis(policy.manifest.timeout_ms),
            output_bytes: Mutex::new(0),
            limits: WorkerLimits {
                timeout: Duration::from_millis(policy.manifest.timeout_ms),
                memory_bytes: policy.manifest.memory_bytes,
                output_bytes: MAX_OUTPUT_BYTES,
            },
        })
    }
}

#[derive(Debug)]
pub struct ExecutionHandle {
    execution_id: Uuid,
    active: Arc<AtomicUsize>,
    cancelled: Arc<AtomicBool>,
    deadline: Instant,
    output_bytes: Mutex<u64>,
    pub limits: WorkerLimits,
}

#[derive(Debug, Clone)]
pub struct ExecutionCancellation {
    cancelled: Arc<AtomicBool>,
}

impl Default for ExecutionCancellation {
    fn default() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
        }
    }
}

impl ExecutionCancellation {
    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Acquire)
    }
}

impl ExecutionHandle {
    #[must_use]
    pub const fn execution_id(&self) -> Uuid {
        self.execution_id
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
    }

    #[must_use]
    pub fn cancellation(&self) -> ExecutionCancellation {
        ExecutionCancellation {
            cancelled: Arc::clone(&self.cancelled),
        }
    }

    /// Checks whether the execution may continue.
    ///
    /// # Errors
    ///
    /// Returns an error after cancellation or deadline expiry.
    pub fn checkpoint(&self) -> Result<(), WorkerError> {
        if self.cancelled.load(Ordering::Acquire) {
            return Err(WorkerError::Cancelled);
        }
        if Instant::now() >= self.deadline {
            return Err(WorkerError::TimedOut);
        }
        Ok(())
    }

    /// Accounts bytes emitted by an isolated worker.
    ///
    /// # Errors
    ///
    /// Returns an error after cancellation, timeout, or output budget exhaustion.
    pub fn record_output(&self, bytes: u64) -> Result<(), WorkerError> {
        self.checkpoint()?;
        let mut total = self
            .output_bytes
            .lock()
            .map_err(|_| WorkerError::Cancelled)?;
        *total = total.checked_add(bytes).ok_or(WorkerError::OutputLimit)?;
        if *total > self.limits.output_bytes {
            return Err(WorkerError::OutputLimit);
        }
        Ok(())
    }
}

impl Drop for ExecutionHandle {
    fn drop(&mut self) {
        self.active.fetch_sub(1, Ordering::AcqRel);
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("program ID must be a lowercase namespaced identifier")]
    InvalidProgramId,
    #[error("program version must be a semantic version")]
    InvalidVersion,
    #[error("program declares too many subscriptions")]
    TooManySubscriptions,
    #[error("event queue capacity must be between 1 and 64")]
    InvalidEventQueueCapacity,
    #[error("program declares too many commands")]
    TooManyCommands,
    #[error("program timeout exceeds the 30 second limit")]
    TimeoutExceeded,
    #[error("program memory budget exceeds the 64 MiB limit")]
    MemoryExceeded,
    #[error("subscription is not a namespaced event type: {0}")]
    InvalidSubscription(String),
    #[error("command is not a safe namespaced command: {0}")]
    UnsafeCommand(String),
    #[error("manifest requires a capability that was not declared")]
    MissingCapability,
}

/// Validates a user-code manifest and produces the capabilities available to
/// its isolated runtime.
///
/// # Errors
///
/// Returns an error when identifiers, capabilities, requested commands, or
/// runtime budgets violate the policy.
pub fn evaluate(manifest: ProgramManifest) -> Result<ExecutionPolicy, PolicyError> {
    if !valid_namespaced_id(&manifest.id) {
        return Err(PolicyError::InvalidProgramId);
    }
    if !valid_semver(&manifest.version) {
        return Err(PolicyError::InvalidVersion);
    }
    if manifest.subscriptions.len() > MAX_SUBSCRIPTIONS {
        return Err(PolicyError::TooManySubscriptions);
    }
    if manifest.event_queue_capacity == 0
        || manifest.event_queue_capacity > MAX_EVENT_QUEUE_CAPACITY
    {
        return Err(PolicyError::InvalidEventQueueCapacity);
    }
    if manifest.commands.len() > MAX_COMMANDS {
        return Err(PolicyError::TooManyCommands);
    }
    if manifest.timeout_ms == 0 || manifest.timeout_ms > MAX_RUNTIME_MS {
        return Err(PolicyError::TimeoutExceeded);
    }
    if manifest.memory_bytes == 0 || manifest.memory_bytes > MAX_MEMORY_BYTES {
        return Err(PolicyError::MemoryExceeded);
    }
    let can_subscribe_events = manifest.capabilities.contains(&Capability::SubscribeEvents);
    if !manifest.subscriptions.is_empty() && !can_subscribe_events {
        return Err(PolicyError::MissingCapability);
    }
    for event_type in &manifest.subscriptions {
        if !valid_namespaced_id(event_type) {
            return Err(PolicyError::InvalidSubscription(event_type.clone()));
        }
    }
    let can_invoke_commands = manifest
        .capabilities
        .contains(&Capability::InvokeSafeCommands);
    if !manifest.commands.is_empty() && !can_invoke_commands {
        return Err(PolicyError::MissingCapability);
    }
    for command in &manifest.commands {
        if !command.starts_with("safe.") || !valid_namespaced_id(command) {
            return Err(PolicyError::UnsafeCommand(command.clone()));
        }
    }
    Ok(ExecutionPolicy {
        can_read_pet_state: manifest.capabilities.contains(&Capability::ReadPetState),
        can_read_profile_state: manifest
            .capabilities
            .contains(&Capability::ReadProfileState),
        can_subscribe_events,
        can_invoke_commands,
        can_store_local_data: manifest.capabilities.contains(&Capability::StoreLocalData),
        manifest,
    })
}

fn valid_namespaced_id(value: &str) -> bool {
    let segments = value.split('.').collect::<Vec<_>>();
    segments.len() >= 3
        && segments.iter().all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn valid_semver(value: &str) -> bool {
    let mut parts = value.split('.');
    parts.clone().count() == 3
        && parts.all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manifest() -> ProgramManifest {
        ProgramManifest {
            id: "studio.example.focus".into(),
            version: "1.0.0".into(),
            capabilities: vec![
                Capability::ReadPetState,
                Capability::SubscribeEvents,
                Capability::InvokeSafeCommands,
            ],
            subscriptions: vec!["pet.example.clicked".into()],
            event_concurrency: EventConcurrencyPolicy::Serial,
            event_queue_capacity: 16,
            commands: vec!["safe.example.notify".into()],
            timeout_ms: 5_000,
            memory_bytes: 8 * 1024 * 1024,
        }
    }

    #[test]
    fn evaluates_declared_safe_capabilities() {
        let policy = evaluate(manifest()).unwrap();
        assert!(policy.can_read_pet_state);
        assert!(!policy.can_read_profile_state);
        assert!(policy.can_subscribe_events);
        assert!(policy.can_invoke_commands);
    }

    #[test]
    fn rejects_command_without_safe_namespace() {
        let mut value = manifest();
        value.commands = vec!["system.example.shutdown".into()];
        assert!(matches!(
            evaluate(value),
            Err(PolicyError::UnsafeCommand(_))
        ));
    }

    #[test]
    fn rejects_subscription_without_capability() {
        let mut value = manifest();
        value
            .capabilities
            .retain(|capability| *capability != Capability::SubscribeEvents);
        assert_eq!(evaluate(value), Err(PolicyError::MissingCapability));
    }

    #[test]
    fn rejects_excessive_runtime_budget() {
        let mut value = manifest();
        value.timeout_ms = MAX_RUNTIME_MS + 1;
        assert_eq!(evaluate(value), Err(PolicyError::TimeoutExceeded));
    }

    #[test]
    fn rejects_invalid_event_queue_capacity() {
        let mut value = manifest();
        value.event_queue_capacity = MAX_EVENT_QUEUE_CAPACITY + 1;
        assert_eq!(evaluate(value), Err(PolicyError::InvalidEventQueueCapacity));
    }

    #[test]
    fn rejects_manifest_without_explicit_event_policy() {
        let value = serde_json::json!({
            "id": "studio.example.focus",
            "version": "1.0.0",
            "capabilities": ["subscribe-events"],
            "subscriptions": ["pet.example.clicked"],
            "commands": [],
            "timeoutMs": 5000,
            "memoryBytes": 8_388_608
        });
        let error = serde_json::from_value::<ProgramManifest>(value).unwrap_err();
        assert!(error.to_string().contains("eventConcurrency"));
    }

    #[test]
    fn serial_event_scheduler_bounds_queue_and_keeps_latest_events() {
        let mut scheduler = EventTriggerScheduler::new(EventConcurrencyPolicy::Serial, 2);
        let EventAdmission::Start(first) = scheduler.admit(1) else {
            panic!("first event should start");
        };
        assert_eq!(scheduler.admit(2), EventAdmission::Queued);
        assert_eq!(scheduler.admit(3), EventAdmission::Queued);
        assert_eq!(scheduler.admit(4), EventAdmission::Queued);
        assert_eq!(scheduler.queued(), 2);
        assert_eq!(scheduler.dropped(), 1);
        let second = scheduler.finish(first.execution_id).unwrap();
        assert_eq!(second.event, 3);
        let third = scheduler.finish(second.execution_id).unwrap();
        assert_eq!(third.event, 4);
        assert!(scheduler.finish(third.execution_id).is_none());
    }

    #[test]
    fn serial_event_scheduler_ignores_stale_completion_after_advancing() {
        let mut scheduler = EventTriggerScheduler::new(EventConcurrencyPolicy::Serial, 2);
        let EventAdmission::Start(first) = scheduler.admit("first") else {
            panic!("first event should start");
        };
        assert_eq!(scheduler.admit("second"), EventAdmission::Queued);
        let second = scheduler.finish(first.execution_id).unwrap();
        assert!(scheduler.is_active(second.execution_id));
        assert!(scheduler.finish(first.execution_id).is_none());
        assert!(scheduler.is_active(second.execution_id));
    }

    #[test]
    fn drop_event_scheduler_rejects_events_while_active() {
        let mut scheduler = EventTriggerScheduler::new(EventConcurrencyPolicy::Drop, 16);
        let EventAdmission::Start(first) = scheduler.admit("first") else {
            panic!("first event should start");
        };
        assert_eq!(scheduler.admit("second"), EventAdmission::Dropped);
        assert_eq!(scheduler.dropped(), 1);
        assert!(scheduler.is_active(first.execution_id));
        assert!(scheduler.finish(first.execution_id).is_none());
    }

    #[test]
    fn cancel_previous_scheduler_ignores_stale_completion() {
        let mut scheduler = EventTriggerScheduler::new(EventConcurrencyPolicy::CancelPrevious, 16);
        let EventAdmission::Start(first) = scheduler.admit("first") else {
            panic!("first event should start");
        };
        let EventAdmission::CancelAndStart {
            cancelled_execution_id,
            next,
        } = scheduler.admit("second")
        else {
            panic!("second event should replace active execution");
        };
        assert_eq!(cancelled_execution_id, first.execution_id);
        assert_eq!(next.event, "second");
        assert!(scheduler.finish(first.execution_id).is_none());
        assert!(scheduler.finish(next.execution_id).is_none());
    }

    #[test]
    fn cancel_previous_scheduler_tracks_only_latest_replacement() {
        let mut scheduler = EventTriggerScheduler::new(EventConcurrencyPolicy::CancelPrevious, 16);
        let EventAdmission::Start(first) = scheduler.admit(1) else {
            panic!("first event should start");
        };
        let EventAdmission::CancelAndStart { next: second, .. } = scheduler.admit(2) else {
            panic!("second event should replace first");
        };
        let EventAdmission::CancelAndStart {
            cancelled_execution_id,
            next: third,
        } = scheduler.admit(3)
        else {
            panic!("third event should replace second");
        };
        assert_eq!(cancelled_execution_id, second.execution_id);
        assert!(!scheduler.is_active(first.execution_id));
        assert!(!scheduler.is_active(second.execution_id));
        assert!(scheduler.is_active(third.execution_id));
        assert!(scheduler.finish(first.execution_id).is_none());
        assert!(scheduler.finish(second.execution_id).is_none());
        assert!(scheduler.is_active(third.execution_id));
    }

    #[test]
    fn cancelling_scheduler_clears_queue_and_advances_generation() {
        let mut scheduler = EventTriggerScheduler::new(EventConcurrencyPolicy::Serial, 16);
        let EventAdmission::Start(first) = scheduler.admit(1) else {
            panic!("first event should start");
        };
        assert_eq!(scheduler.admit(2), EventAdmission::Queued);
        assert_eq!(scheduler.cancel_all(), Some(first.execution_id));
        assert_eq!(scheduler.queued(), 0);
        assert_eq!(scheduler.generation(), 1);
        assert!(scheduler.finish(first.execution_id).is_none());
    }

    #[test]
    fn execution_handle_enforces_output_and_cancellation() {
        let policy = evaluate(manifest()).unwrap();
        let controller = ExecutionController::default();
        let handle = controller.admit(&policy).unwrap();
        assert!(handle.record_output(MAX_OUTPUT_BYTES).is_ok());
        assert_eq!(handle.record_output(1), Err(WorkerError::OutputLimit));
        handle.cancel();
        assert_eq!(handle.checkpoint(), Err(WorkerError::Cancelled));
    }

    #[test]
    fn supervisor_owned_cancellation_stops_an_admitted_execution() {
        let policy = evaluate(manifest()).unwrap();
        let controller = ExecutionController::default();
        let cancellation = ExecutionCancellation::default();
        let handle = controller
            .admit_with_cancellation(&policy, cancellation.clone())
            .unwrap();
        cancellation.cancel();
        assert_eq!(handle.checkpoint(), Err(WorkerError::Cancelled));
    }

    #[test]
    fn execution_slots_are_released_when_handles_drop() {
        let policy = evaluate(manifest()).unwrap();
        let controller = ExecutionController::default();
        let mut handles = Vec::new();
        for _ in 0..MAX_CONCURRENT_EXECUTIONS {
            handles.push(controller.admit(&policy).unwrap());
        }
        assert!(matches!(
            controller.admit(&policy),
            Err(WorkerError::ConcurrencyLimit)
        ));
        drop(handles);
        assert!(controller.admit(&policy).is_ok());
    }
}
