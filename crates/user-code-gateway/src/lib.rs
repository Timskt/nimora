use nimora_user_code_policy::{ExecutionHandle, ExecutionPolicy, WorkerError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GatewayEnvelope {
    pub execution_id: String,
    pub trace_id: String,
    pub idempotency_key: Option<String>,
    pub request: CapabilityRequest,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CapabilityRequest {
    ReadPetState,
    InvokeCommand { command: String, arguments: Value },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CapabilityResponse {
    PetState { value: Value },
    CommandAccepted { value: Value },
}

pub trait CapabilityBackend: std::fmt::Debug + Send + Sync {
    /// Returns a serialized pet state without exposing the underlying object.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when state cannot be read.
    fn read_pet_state(&self) -> Result<Value, String>;

    /// Invokes one registered safe command.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when the command cannot be completed.
    fn invoke_command(
        &self,
        command: &str,
        arguments: Value,
        trace_id: &str,
        idempotency_key: Option<&str>,
    ) -> Result<Value, String>;
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum GatewayError {
    #[error("execution or trace identifier is invalid")]
    InvalidContext,
    #[error("execution identifier does not match the admitted worker")]
    ExecutionMismatch,
    #[error("capability was not granted")]
    CapabilityDenied,
    #[error("command was not declared by the program: {0}")]
    CommandNotDeclared(String),
    #[error("worker execution is no longer active: {0}")]
    Execution(#[from] WorkerError),
    #[error("capability backend failed: {0}")]
    Backend(String),
}

#[derive(Debug)]
pub struct CapabilityGateway<B> {
    backend: B,
}

impl<B: CapabilityBackend> CapabilityGateway<B> {
    #[must_use]
    pub const fn new(backend: B) -> Self {
        Self { backend }
    }

    /// Authorizes and dispatches one capability request.
    ///
    /// # Errors
    ///
    /// Returns an error when execution context is missing, the capability or
    /// command was not granted, the execution ended, or the backend fails.
    pub fn dispatch(
        &self,
        policy: &ExecutionPolicy,
        execution: &ExecutionHandle,
        envelope: GatewayEnvelope,
    ) -> Result<CapabilityResponse, GatewayError> {
        execution.checkpoint()?;
        if envelope.execution_id.parse::<Uuid>().is_err()
            || envelope.trace_id.parse::<Uuid>().is_err()
        {
            return Err(GatewayError::InvalidContext);
        }
        if envelope.execution_id != execution.execution_id().to_string() {
            return Err(GatewayError::ExecutionMismatch);
        }
        match envelope.request {
            CapabilityRequest::ReadPetState => {
                if !policy.can_read_pet_state {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_pet_state()
                    .map(|value| CapabilityResponse::PetState { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::InvokeCommand { command, arguments } => {
                if !policy.can_invoke_commands {
                    return Err(GatewayError::CapabilityDenied);
                }
                if !policy.manifest.commands.contains(&command) {
                    return Err(GatewayError::CommandNotDeclared(command));
                }
                self.backend
                    .invoke_command(
                        &command,
                        arguments,
                        &envelope.trace_id,
                        envelope.idempotency_key.as_deref(),
                    )
                    .map(|value| CapabilityResponse::CommandAccepted { value })
                    .map_err(GatewayError::Backend)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_user_code_policy::{
        Capability, EventConcurrencyPolicy, ExecutionController, ProgramManifest, evaluate,
    };
    use serde_json::json;

    #[derive(Debug)]
    struct Backend;

    impl CapabilityBackend for Backend {
        fn read_pet_state(&self) -> Result<Value, String> {
            Ok(json!({"state": "idle"}))
        }

        fn invoke_command(
            &self,
            command: &str,
            arguments: Value,
            trace_id: &str,
            idempotency_key: Option<&str>,
        ) -> Result<Value, String> {
            Ok(json!({
                "command": command,
                "arguments": arguments,
                "traceId": trace_id,
                "idempotencyKey": idempotency_key
            }))
        }
    }

    fn policy() -> ExecutionPolicy {
        evaluate(ProgramManifest {
            id: "studio.example.focus".into(),
            version: "1.0.0".into(),
            capabilities: vec![Capability::ReadPetState, Capability::InvokeSafeCommands],
            subscriptions: vec![],
            event_concurrency: EventConcurrencyPolicy::default(),
            event_queue_capacity: 16,
            commands: vec!["safe.pet.animate".into()],
            timeout_ms: 5_000,
            memory_bytes: 8 * 1024 * 1024,
        })
        .unwrap()
    }

    fn envelope(execution: &ExecutionHandle, request: CapabilityRequest) -> GatewayEnvelope {
        GatewayEnvelope {
            execution_id: execution.execution_id().to_string(),
            trace_id: Uuid::now_v7().to_string(),
            idempotency_key: Some("once-1".into()),
            request,
        }
    }

    #[test]
    fn dispatches_declared_commands_with_context() {
        let policy = policy();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        let response = CapabilityGateway::new(Backend)
            .dispatch(
                &policy,
                &execution,
                envelope(
                    &execution,
                    CapabilityRequest::InvokeCommand {
                        command: "safe.pet.animate".into(),
                        arguments: json!({"action": "idle"}),
                    },
                ),
            )
            .unwrap();
        assert!(matches!(
            response,
            CapabilityResponse::CommandAccepted { .. }
        ));
    }

    #[test]
    fn rejects_commands_missing_from_manifest() {
        let policy = policy();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        let result = CapabilityGateway::new(Backend).dispatch(
            &policy,
            &execution,
            envelope(
                &execution,
                CapabilityRequest::InvokeCommand {
                    command: "safe.pet.delete".into(),
                    arguments: json!({}),
                },
            ),
        );
        assert_eq!(
            result,
            Err(GatewayError::CommandNotDeclared("safe.pet.delete".into()))
        );
    }

    #[test]
    fn cancelled_execution_cannot_call_backend() {
        let policy = policy();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        execution.cancel();
        let result = CapabilityGateway::new(Backend).dispatch(
            &policy,
            &execution,
            envelope(&execution, CapabilityRequest::ReadPetState),
        );
        assert_eq!(result, Err(GatewayError::Execution(WorkerError::Cancelled)));
    }

    #[test]
    fn rejects_another_workers_execution_id() {
        let policy = policy();
        let controller = ExecutionController::default();
        let execution = controller.admit(&policy).unwrap();
        let another = controller.admit(&policy).unwrap();
        let result = CapabilityGateway::new(Backend).dispatch(
            &policy,
            &execution,
            envelope(&another, CapabilityRequest::ReadPetState),
        );
        assert_eq!(result, Err(GatewayError::ExecutionMismatch));
    }
}
