use nimora_user_code_policy::{ExecutionHandle, ExecutionPolicy, WorkerError};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeSet;
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentGatewayPolicy {
    pub task_id: Uuid,
    pub trace_id: Uuid,
    pub read_capabilities: BTreeSet<String>,
    pub commands: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ModuleGatewayPolicy {
    pub execution_id: Uuid,
    pub trace_id: Uuid,
    pub read_capabilities: BTreeSet<String>,
    pub commands: BTreeSet<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CapabilityRequest {
    ReadPetState,
    ReadPetActionCatalog,
    ReadProfileState,
    ReadCharacterState,
    ReadAssetCatalog,
    ReadProgramCatalog,
    ReadRuntimeHealth,
    ValidateAutomation {
        definition: Value,
        event_type: String,
        event_data: Value,
    },
    ReadLocalData {
        key: String,
    },
    WriteLocalData {
        key: String,
        value: Value,
    },
    DeleteLocalData {
        key: String,
    },
    InvokeCommand {
        command: String,
        arguments: Value,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum CapabilityResponse {
    PetState { value: Value },
    PetActionCatalog { value: Value },
    ProfileState { value: Value },
    CharacterState { value: Value },
    AssetCatalog { value: Value },
    ProgramCatalog { value: Value },
    RuntimeHealth { value: Value },
    AutomationValidation { value: Value },
    LocalData { value: Option<Value> },
    LocalDataWritten,
    LocalDataDeleted { deleted: bool },
    CommandAccepted { value: Value },
}

pub trait CapabilityBackend: std::fmt::Debug + Send + Sync {
    /// Returns a serialized pet state without exposing the underlying object.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when state cannot be read.
    fn read_pet_state(&self) -> Result<Value, String>;

    /// Returns the bounded action vocabulary accepted by the pet runtime.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when the action catalog cannot be read.
    fn read_pet_action_catalog(&self) -> Result<Value, String> {
        Err("pet action catalog capability is unavailable".to_owned())
    }

    /// Returns a serialized Profile snapshot without exposing its repository.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when Profile state cannot be read.
    fn read_profile_state(&self) -> Result<Value, String>;

    /// Returns a path-free active character and renderer summary.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when character state cannot be read.
    fn read_character_state(&self) -> Result<Value, String> {
        Err("character state capability is unavailable".to_owned())
    }

    /// Returns a serialized asset catalog without exposing its store.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when the catalog cannot be read.
    fn read_asset_catalog(&self) -> Result<Value, String> {
        Err("asset catalog capability is unavailable".to_owned())
    }

    /// Returns verified installed program identities without source or host paths.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when the catalog cannot be read.
    fn read_program_catalog(&self) -> Result<Value, String> {
        Err("program catalog capability is unavailable".to_owned())
    }

    /// Returns a bounded runtime health summary without logs or user content.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when health cannot be read.
    fn read_runtime_health(&self) -> Result<Value, String> {
        Err("runtime health capability is unavailable".to_owned())
    }

    /// Validates and dry-runs one bounded automation definition without side effects.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when the definition or test event is invalid.
    fn validate_automation(
        &self,
        _definition: &Value,
        _event_type: &str,
        _event_data: &Value,
    ) -> Result<Value, String> {
        Err("automation validation capability is unavailable".to_owned())
    }

    /// Reads a value from the program's isolated local-data namespace.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when the value cannot be read.
    fn read_local_data(&self, program_id: &str, key: &str) -> Result<Option<Value>, String>;

    /// Atomically writes a value to the program's isolated local-data namespace.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when validation, quota, or persistence fails.
    fn write_local_data(&self, program_id: &str, key: &str, value: &Value) -> Result<(), String>;

    /// Deletes a value from the program's isolated local-data namespace.
    ///
    /// # Errors
    ///
    /// Returns a backend-specific error when the value cannot be deleted.
    fn delete_local_data(&self, program_id: &str, key: &str) -> Result<bool, String>;

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
            CapabilityRequest::ReadProfileState => {
                if !policy.can_read_profile_state {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_profile_state()
                    .map(|value| CapabilityResponse::ProfileState { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::ReadPetActionCatalog
            | CapabilityRequest::ReadCharacterState
            | CapabilityRequest::ReadAssetCatalog
            | CapabilityRequest::ReadProgramCatalog
            | CapabilityRequest::ReadRuntimeHealth
            | CapabilityRequest::ValidateAutomation { .. } => Err(GatewayError::CapabilityDenied),
            CapabilityRequest::ReadLocalData { key } => {
                if !policy.can_store_local_data {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_local_data(&policy.manifest.id, &key)
                    .map(|value| CapabilityResponse::LocalData { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::WriteLocalData { key, value } => {
                if !policy.can_store_local_data {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .write_local_data(&policy.manifest.id, &key, &value)
                    .map(|()| CapabilityResponse::LocalDataWritten)
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::DeleteLocalData { key } => {
                if !policy.can_store_local_data {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .delete_local_data(&policy.manifest.id, &key)
                    .map(|deleted| CapabilityResponse::LocalDataDeleted { deleted })
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

    /// Authorizes and dispatches one Agent tool request through the shared capability backend.
    ///
    /// # Errors
    ///
    /// Returns an error for mismatched task correlation, missing capabilities, undeclared
    /// commands, unsupported Agent-local storage requests, or backend failures.
    pub fn dispatch_agent(
        &self,
        policy: &AgentGatewayPolicy,
        envelope: GatewayEnvelope,
    ) -> Result<CapabilityResponse, GatewayError> {
        self.dispatch_module(
            &ModuleGatewayPolicy {
                execution_id: policy.task_id,
                trace_id: policy.trace_id,
                read_capabilities: policy.read_capabilities.clone(),
                commands: policy.commands.clone(),
            },
            envelope,
        )
    }

    /// Authorizes one host module request through the shared capability boundary.
    ///
    /// # Errors
    ///
    /// Returns an error for correlation mismatch, undeclared capability or command,
    /// unsupported local storage, or backend failure.
    pub fn dispatch_module(
        &self,
        policy: &ModuleGatewayPolicy,
        envelope: GatewayEnvelope,
    ) -> Result<CapabilityResponse, GatewayError> {
        if envelope.execution_id != policy.execution_id.to_string()
            || envelope.trace_id != policy.trace_id.to_string()
        {
            return Err(GatewayError::ExecutionMismatch);
        }
        match envelope.request {
            CapabilityRequest::ReadPetState => {
                if !policy.read_capabilities.contains("pet.state") {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_pet_state()
                    .map(|value| CapabilityResponse::PetState { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::ReadPetActionCatalog => {
                if !policy.read_capabilities.contains("pet.action.catalog") {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_pet_action_catalog()
                    .map(|value| CapabilityResponse::PetActionCatalog { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::ReadProfileState => {
                if !policy.read_capabilities.contains("profile.state") {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_profile_state()
                    .map(|value| CapabilityResponse::ProfileState { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::ReadCharacterState => {
                if !policy.read_capabilities.contains("character.state") {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_character_state()
                    .map(|value| CapabilityResponse::CharacterState { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::ReadAssetCatalog => {
                if !policy.read_capabilities.contains("asset.catalog") {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_asset_catalog()
                    .map(|value| CapabilityResponse::AssetCatalog { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::ReadProgramCatalog => {
                if !policy.read_capabilities.contains("program.catalog") {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_program_catalog()
                    .map(|value| CapabilityResponse::ProgramCatalog { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::ReadRuntimeHealth => {
                if !policy.read_capabilities.contains("runtime.health") {
                    return Err(GatewayError::CapabilityDenied);
                }
                self.backend
                    .read_runtime_health()
                    .map(|value| CapabilityResponse::RuntimeHealth { value })
                    .map_err(GatewayError::Backend)
            }
            CapabilityRequest::ValidateAutomation {
                definition,
                event_type,
                event_data,
            } => self.dispatch_module_automation(policy, &definition, &event_type, &event_data),
            CapabilityRequest::InvokeCommand { command, arguments } => {
                if !policy.commands.contains(&command) {
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
            CapabilityRequest::ReadLocalData { .. }
            | CapabilityRequest::WriteLocalData { .. }
            | CapabilityRequest::DeleteLocalData { .. } => Err(GatewayError::CapabilityDenied),
        }
    }

    fn dispatch_module_automation(
        &self,
        policy: &ModuleGatewayPolicy,
        definition: &Value,
        event_type: &str,
        event_data: &Value,
    ) -> Result<CapabilityResponse, GatewayError> {
        if !policy
            .read_capabilities
            .contains("automation.definition.validate")
        {
            return Err(GatewayError::CapabilityDenied);
        }
        self.backend
            .validate_automation(definition, event_type, event_data)
            .map(|value| CapabilityResponse::AutomationValidation { value })
            .map_err(GatewayError::Backend)
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

        fn read_pet_action_catalog(&self) -> Result<Value, String> {
            Ok(
                json!({"actions": ["idle", "observe", "walk", "perch", "climb", "peek", "stretch", "sleep", "work", "celebrate"]}),
            )
        }

        fn read_profile_state(&self) -> Result<Value, String> {
            Ok(json!({"activeProfileId": "profile-1"}))
        }

        fn read_character_state(&self) -> Result<Value, String> {
            Ok(json!({"active": {"assetId": "builtin.aster"}}))
        }

        fn read_local_data(&self, _program_id: &str, key: &str) -> Result<Option<Value>, String> {
            Ok(Some(json!({"key": key})))
        }

        fn write_local_data(
            &self,
            _program_id: &str,
            _key: &str,
            _value: &Value,
        ) -> Result<(), String> {
            Ok(())
        }

        fn delete_local_data(&self, _program_id: &str, _key: &str) -> Result<bool, String> {
            Ok(true)
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
            capabilities: vec![
                Capability::ReadPetState,
                Capability::ReadProfileState,
                Capability::InvokeSafeCommands,
                Capability::StoreLocalData,
            ],
            subscriptions: vec![],
            event_concurrency: EventConcurrencyPolicy::Serial,
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

    #[test]
    fn dispatches_program_scoped_local_storage() {
        let policy = policy();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        let response = CapabilityGateway::new(Backend)
            .dispatch(
                &policy,
                &execution,
                envelope(
                    &execution,
                    CapabilityRequest::ReadLocalData {
                        key: "settings".into(),
                    },
                ),
            )
            .unwrap();
        assert_eq!(
            response,
            CapabilityResponse::LocalData {
                value: Some(json!({"key": "settings"}))
            }
        );
    }

    #[test]
    fn dispatches_authorized_profile_state() {
        let policy = policy();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        let response = CapabilityGateway::new(Backend)
            .dispatch(
                &policy,
                &execution,
                envelope(&execution, CapabilityRequest::ReadProfileState),
            )
            .unwrap();
        assert_eq!(
            response,
            CapabilityResponse::ProfileState {
                value: json!({"activeProfileId": "profile-1"})
            }
        );
    }

    #[test]
    fn rejects_profile_state_without_explicit_capability() {
        let mut manifest = policy().manifest;
        manifest
            .capabilities
            .retain(|capability| *capability != Capability::ReadProfileState);
        let policy = evaluate(manifest).unwrap();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        let result = CapabilityGateway::new(Backend).dispatch(
            &policy,
            &execution,
            envelope(&execution, CapabilityRequest::ReadProfileState),
        );
        assert_eq!(result, Err(GatewayError::CapabilityDenied));
    }

    #[test]
    fn user_programs_do_not_inherit_agent_character_state() {
        let policy = policy();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        let result = CapabilityGateway::new(Backend).dispatch(
            &policy,
            &execution,
            envelope(&execution, CapabilityRequest::ReadCharacterState),
        );
        assert_eq!(result, Err(GatewayError::CapabilityDenied));
    }

    #[test]
    fn user_programs_do_not_inherit_agent_action_catalog() {
        let policy = policy();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        let result = CapabilityGateway::new(Backend).dispatch(
            &policy,
            &execution,
            envelope(&execution, CapabilityRequest::ReadPetActionCatalog),
        );
        assert_eq!(result, Err(GatewayError::CapabilityDenied));
    }

    #[test]
    fn user_programs_do_not_inherit_agent_program_catalog() {
        let policy = policy();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        let result = CapabilityGateway::new(Backend).dispatch(
            &policy,
            &execution,
            envelope(&execution, CapabilityRequest::ReadProgramCatalog),
        );
        assert_eq!(result, Err(GatewayError::CapabilityDenied));
    }

    #[test]
    fn user_programs_do_not_inherit_agent_automation_validation() {
        let policy = policy();
        let execution = ExecutionController::default().admit(&policy).unwrap();
        let result = CapabilityGateway::new(Backend).dispatch(
            &policy,
            &execution,
            envelope(
                &execution,
                CapabilityRequest::ValidateAutomation {
                    definition: json!({}),
                    event_type: "dev.build.finished".to_owned(),
                    event_data: json!({}),
                },
            ),
        );
        assert_eq!(result, Err(GatewayError::CapabilityDenied));
    }

    #[test]
    fn agent_dispatch_requires_exact_task_trace_and_declared_command() {
        let task_id = Uuid::now_v7();
        let trace_id = Uuid::now_v7();
        let policy = AgentGatewayPolicy {
            task_id,
            trace_id,
            read_capabilities: BTreeSet::from(["pet.state".to_owned()]),
            commands: BTreeSet::from(["safe.pet.animate".to_owned()]),
        };
        let gateway = CapabilityGateway::new(Backend);
        let response = gateway
            .dispatch_agent(
                &policy,
                GatewayEnvelope {
                    execution_id: task_id.to_string(),
                    trace_id: trace_id.to_string(),
                    idempotency_key: Some("invocation:1".to_owned()),
                    request: CapabilityRequest::InvokeCommand {
                        command: "safe.pet.animate".to_owned(),
                        arguments: json!({"action": "wave"}),
                    },
                },
            )
            .expect("agent command");
        assert!(matches!(
            response,
            CapabilityResponse::CommandAccepted { .. }
        ));

        assert_eq!(
            gateway.dispatch_agent(
                &policy,
                GatewayEnvelope {
                    execution_id: Uuid::now_v7().to_string(),
                    trace_id: trace_id.to_string(),
                    idempotency_key: None,
                    request: CapabilityRequest::ReadPetState,
                },
            ),
            Err(GatewayError::ExecutionMismatch)
        );
        assert_eq!(
            gateway.dispatch_agent(
                &policy,
                GatewayEnvelope {
                    execution_id: task_id.to_string(),
                    trace_id: trace_id.to_string(),
                    idempotency_key: None,
                    request: CapabilityRequest::InvokeCommand {
                        command: "safe.pet.move".to_owned(),
                        arguments: json!({"x": 1, "y": 2}),
                    },
                },
            ),
            Err(GatewayError::CommandNotDeclared("safe.pet.move".to_owned()))
        );
    }
}
