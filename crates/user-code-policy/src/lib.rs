use serde::{Deserialize, Serialize};
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

const fn default_event_queue_capacity() -> usize {
    16
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventConcurrencyPolicy {
    #[default]
    Serial,
    Drop,
    CancelPrevious,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    ReadPetState,
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
    #[serde(default)]
    pub event_concurrency: EventConcurrencyPolicy,
    #[serde(default = "default_event_queue_capacity")]
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
            cancelled: Arc::new(AtomicBool::new(false)),
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

impl ExecutionHandle {
    #[must_use]
    pub const fn execution_id(&self) -> Uuid {
        self.execution_id
    }

    pub fn cancel(&self) {
        self.cancelled.store(true, Ordering::Release);
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
    fn deserializes_backward_compatible_event_defaults() {
        let value = serde_json::json!({
            "id": "studio.example.focus",
            "version": "1.0.0",
            "capabilities": ["subscribe-events"],
            "subscriptions": ["pet.example.clicked"],
            "commands": [],
            "timeoutMs": 5000,
            "memoryBytes": 8_388_608
        });
        let manifest: ProgramManifest = serde_json::from_value(value).unwrap();
        assert_eq!(manifest.event_concurrency, EventConcurrencyPolicy::Serial);
        assert_eq!(manifest.event_queue_capacity, 16);
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
