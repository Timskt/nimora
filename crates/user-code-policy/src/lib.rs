use serde::{Deserialize, Serialize};
use thiserror::Error;

const MAX_SUBSCRIPTIONS: usize = 32;
const MAX_COMMANDS: usize = 32;
const MAX_RUNTIME_MS: u64 = 30_000;
const MAX_MEMORY_BYTES: u64 = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Capability {
    ReadPetState,
    SubscribeEvents,
    InvokeSafeCommands,
    StoreLocalData,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProgramManifest {
    pub id: String,
    pub version: String,
    pub capabilities: Vec<Capability>,
    pub subscriptions: Vec<String>,
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

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PolicyError {
    #[error("program ID must be a lowercase namespaced identifier")]
    InvalidProgramId,
    #[error("program version must be a semantic version")]
    InvalidVersion,
    #[error("program declares too many subscriptions")]
    TooManySubscriptions,
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
}
