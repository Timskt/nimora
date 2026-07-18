use nimora_automation_runtime::{ActionFailure, AutomationBackend, AutomationExecutionContext};
use nimora_runtime_core::{Command, CommandId, CommandRisk};
use nimora_user_code_gateway::{
    CapabilityBackend, CapabilityGateway, CapabilityRequest, GatewayEnvelope, GatewayError,
    ModuleGatewayPolicy,
};
use std::collections::{BTreeMap, BTreeSet};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationCommandBinding {
    pub gateway_command: String,
    pub minimum_risk: CommandRisk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationCommandAdmission {
    pub gateway_command: String,
    pub effective_risk: CommandRisk,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AutomationCapabilityPolicy {
    bindings: BTreeMap<String, AutomationCommandBinding>,
}

impl AutomationCapabilityPolicy {
    /// Creates a host-controlled mapping from Automation actions to fixed Gateway commands.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid action IDs, non-safe Gateway commands, or an empty policy.
    pub fn new(
        bindings: impl IntoIterator<Item = (String, AutomationCommandBinding)>,
    ) -> Result<Self, String> {
        let bindings = bindings.into_iter().collect::<BTreeMap<_, _>>();
        if bindings.is_empty()
            || bindings.iter().any(|(action, binding)| {
                action.parse::<CommandId>().is_err()
                    || !binding.gateway_command.starts_with("safe.")
                    || binding.gateway_command.parse::<CommandId>().is_err()
            })
        {
            return Err("automation capability policy is invalid".to_owned());
        }
        Ok(Self { bindings })
    }

    #[must_use]
    pub fn pet_actions() -> Self {
        Self {
            bindings: BTreeMap::from([
                (
                    "pet.animation.play".to_owned(),
                    AutomationCommandBinding {
                        gateway_command: "safe.pet.animate".to_owned(),
                        minimum_risk: CommandRisk::Low,
                    },
                ),
                (
                    "pet.position.move".to_owned(),
                    AutomationCommandBinding {
                        gateway_command: "safe.pet.move".to_owned(),
                        minimum_risk: CommandRisk::Low,
                    },
                ),
            ]),
        }
    }

    /// Resolves one action through the same host policy used by execution.
    ///
    /// # Errors
    ///
    /// Rejects commands that are not registered by the host. A definition may
    /// overstate risk, but cannot lower the host minimum.
    pub fn admit(&self, command: &Command) -> Result<AutomationCommandAdmission, String> {
        let binding = self
            .bindings
            .get(&command.command_id.to_string())
            .ok_or_else(|| "automation command is not allowed by host policy".to_owned())?;
        Ok(AutomationCommandAdmission {
            gateway_command: binding.gateway_command.clone(),
            effective_risk: max_risk(command.risk, binding.minimum_risk),
        })
    }
}

#[derive(Debug)]
pub struct AutomationCapabilityBridge<B> {
    gateway: CapabilityGateway<B>,
    policy: AutomationCapabilityPolicy,
}

impl<B: CapabilityBackend> AutomationCapabilityBridge<B> {
    #[must_use]
    pub const fn new(backend: B, policy: AutomationCapabilityPolicy) -> Self {
        Self {
            gateway: CapabilityGateway::new(backend),
            policy,
        }
    }
}

impl<B: CapabilityBackend> AutomationBackend for AutomationCapabilityBridge<B> {
    fn execute(
        &self,
        context: &AutomationExecutionContext,
        command: Command,
    ) -> Result<(), ActionFailure> {
        let admission = self.policy.admit(&command).map_err(permanent)?;
        let policy = ModuleGatewayPolicy {
            execution_id: context.run_id,
            trace_id: context.trace_id,
            read_capabilities: BTreeSet::new(),
            commands: BTreeSet::from([admission.gateway_command.clone()]),
        };
        self.gateway
            .dispatch_module(
                &policy,
                GatewayEnvelope {
                    execution_id: context.run_id.to_string(),
                    trace_id: context.trace_id.to_string(),
                    idempotency_key: command.idempotency_key,
                    request: CapabilityRequest::InvokeCommand {
                        command: admission.gateway_command,
                        arguments: command.arguments,
                    },
                },
            )
            .map(|_| ())
            .map_err(|error| gateway_failure(&error))
    }
}

const fn risk_rank(risk: CommandRisk) -> u8 {
    match risk {
        CommandRisk::Safe => 0,
        CommandRisk::Low => 1,
        CommandRisk::Medium => 2,
        CommandRisk::High => 3,
        CommandRisk::Critical => 4,
    }
}

const fn max_risk(left: CommandRisk, right: CommandRisk) -> CommandRisk {
    if risk_rank(left) >= risk_rank(right) {
        left
    } else {
        right
    }
}

fn permanent(message: impl Into<String>) -> ActionFailure {
    ActionFailure {
        message: message.into(),
        transient: false,
    }
}

fn gateway_failure(error: &GatewayError) -> ActionFailure {
    ActionFailure {
        transient: matches!(error, GatewayError::Backend(_)),
        message: error.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_user_code_gateway::CapabilityBackend;
    use serde_json::{Value, json};
    use std::sync::{Arc, Mutex};
    use uuid::Uuid;

    type BackendCall = (String, Value, String, Option<String>);

    #[derive(Debug, Default)]
    struct Backend {
        calls: Arc<Mutex<Vec<BackendCall>>>,
    }

    impl CapabilityBackend for Backend {
        fn read_pet_state(&self) -> Result<Value, String> {
            Err("unused".to_owned())
        }

        fn read_profile_state(&self) -> Result<Value, String> {
            Err("unused".to_owned())
        }

        fn read_local_data(&self, _program_id: &str, _key: &str) -> Result<Option<Value>, String> {
            Err("unused".to_owned())
        }

        fn write_local_data(
            &self,
            _program_id: &str,
            _key: &str,
            _value: &Value,
        ) -> Result<(), String> {
            Err("unused".to_owned())
        }

        fn delete_local_data(&self, _program_id: &str, _key: &str) -> Result<bool, String> {
            Err("unused".to_owned())
        }

        fn invoke_command(
            &self,
            command: &str,
            arguments: Value,
            trace_id: &str,
            idempotency_key: Option<&str>,
        ) -> Result<Value, String> {
            self.calls.lock().expect("calls").push((
                command.to_owned(),
                arguments,
                trace_id.to_owned(),
                idempotency_key.map(ToOwned::to_owned),
            ));
            Ok(json!({"accepted": true}))
        }
    }

    fn context() -> AutomationExecutionContext {
        AutomationExecutionContext {
            run_id: Uuid::now_v7(),
            automation_id: "local.pet.greeter".to_owned(),
            action_id: "wave".to_owned(),
            event_id: "event:test".to_owned(),
            trace_id: Uuid::now_v7(),
        }
    }

    #[test]
    fn dispatches_fixed_command_with_run_trace_and_idempotency() {
        let backend = Backend::default();
        let calls = Arc::clone(&backend.calls);
        let bridge =
            AutomationCapabilityBridge::new(backend, AutomationCapabilityPolicy::pet_actions());
        let mut command = Command::new(
            "pet.animation.play",
            json!({"action": "wave"}),
            CommandRisk::Low,
        )
        .expect("command");
        command.idempotency_key = Some("wave-on-login".to_owned());
        let context = context();
        bridge.execute(&context, command).expect("execute");
        let calls = calls.lock().expect("calls");
        assert_eq!(calls[0].0, "safe.pet.animate");
        assert_eq!(calls[0].2, context.trace_id.to_string());
        assert_eq!(calls[0].3.as_deref(), Some("wave-on-login"));
    }

    #[test]
    fn rejects_unknown_actions_and_raises_understated_risk() {
        let backend = Backend::default();
        let calls = Arc::clone(&backend.calls);
        let bridge =
            AutomationCapabilityBridge::new(backend, AutomationCapabilityPolicy::pet_actions());
        let unknown = Command::new("profile.active.switch", json!({}), CommandRisk::Medium)
            .expect("unknown command");
        assert!(
            !bridge
                .execute(&context(), unknown)
                .expect_err("denied")
                .transient
        );
        let understated = Command::new(
            "pet.position.move",
            json!({"x": 1, "y": 2}),
            CommandRisk::Safe,
        )
        .expect("understated command");
        let admission = bridge.policy.admit(&understated).expect("admission");
        assert_eq!(admission.gateway_command, "safe.pet.move");
        assert_eq!(admission.effective_risk, CommandRisk::Low);
        bridge
            .execute(&context(), understated)
            .expect("execute with raised risk");
        let calls = calls.lock().expect("calls");
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].0, "safe.pet.move");
    }
}
