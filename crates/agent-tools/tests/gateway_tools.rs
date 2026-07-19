use nimora_agent_runtime::{
    AgentBudget, AgentCoordinator, AgentTask, AgentTaskOrigin, AgentTaskStatus, ProviderRegistry,
    ToolAdmission, ToolApproval, ToolInvocation, ToolStepOutcome,
};
use nimora_agent_tools::{GatewayToolBackend, production_tool_registry};
use nimora_runtime_core::CommandRisk;
use nimora_user_code_gateway::CapabilityBackend;
use serde_json::{Value, json};
use std::sync::{Arc, Mutex};
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq)]
struct CommandRecord {
    command: String,
    arguments: Value,
    trace_id: String,
    idempotency_key: Option<String>,
}

#[derive(Debug, Clone, Default)]
struct Backend {
    commands: Arc<Mutex<Vec<CommandRecord>>>,
}

impl CapabilityBackend for Backend {
    fn read_pet_state(&self) -> Result<Value, String> {
        Ok(json!({"state": "idle"}))
    }

    fn read_pet_action_catalog(&self) -> Result<Value, String> {
        Ok(json!({"actions": ["idle", "observe", "walk", "perch", "climb", "peek", "sleep", "work", "celebrate"]}))
    }

    fn read_profile_state(&self) -> Result<Value, String> {
        Ok(json!({"activeProfileId": "profile:default"}))
    }

    fn read_character_state(&self) -> Result<Value, String> {
        Ok(json!({"active": {"assetId": "builtin.aster"}, "renderer": {"backend": "built-in"}}))
    }

    fn read_asset_catalog(&self) -> Result<Value, String> {
        Ok(json!({"activeAssetId": "character:builtin.aster", "assets": []}))
    }

    fn read_program_catalog(&self) -> Result<Value, String> {
        Ok(json!({"programs": [], "rejected": 0}))
    }

    fn read_runtime_health(&self) -> Result<Value, String> {
        Ok(json!({"startup": {"mode": "normal"}, "safety": {"mode": "normal"}}))
    }

    fn validate_automation(
        &self,
        definition: &Value,
        event_type: &str,
        event_data: &Value,
    ) -> Result<Value, String> {
        Ok(json!({
            "mode": "dry_run",
            "status": "planned",
            "automationId": definition["id"],
            "eventType": event_type,
            "eventData": event_data
        }))
    }

    fn read_local_data(&self, _program_id: &str, _key: &str) -> Result<Option<Value>, String> {
        Err("Agent tools cannot access program storage".to_owned())
    }

    fn write_local_data(
        &self,
        _program_id: &str,
        _key: &str,
        _value: &Value,
    ) -> Result<(), String> {
        Err("Agent tools cannot access program storage".to_owned())
    }

    fn delete_local_data(&self, _program_id: &str, _key: &str) -> Result<bool, String> {
        Err("Agent tools cannot access program storage".to_owned())
    }

    fn invoke_command(
        &self,
        command: &str,
        arguments: Value,
        trace_id: &str,
        idempotency_key: Option<&str>,
    ) -> Result<Value, String> {
        self.commands
            .lock()
            .map_err(|_| "command log poisoned".to_owned())?
            .push(CommandRecord {
                command: command.to_owned(),
                arguments: arguments.clone(),
                trace_id: trace_id.to_owned(),
                idempotency_key: idempotency_key.map(ToOwned::to_owned),
            });
        Ok(json!({"accepted": true, "arguments": arguments}))
    }
}

fn task() -> AgentTask {
    let mut task = AgentTask::new(
        AgentTaskOrigin::Desktop,
        "desktop:user",
        "provider:local",
        AgentBudget::default(),
        1_000,
    )
    .expect("task");
    task.transition(AgentTaskStatus::Planning, 1_000)
        .expect("planning");
    task
}

#[test]
fn read_tool_executes_through_the_shared_gateway_without_confirmation() {
    let tools = production_tool_registry().expect("registry");
    let providers = ProviderRegistry::default();
    let coordinator = AgentCoordinator::new(&providers, &tools);
    let mut task = task();
    let backend = GatewayToolBackend::new(
        Backend::default(),
        GatewayToolBackend::<Backend>::standard_policy(task.id, task.trace_id),
    );
    let invocation = ToolInvocation::new(task.id, task.trace_id, "pet.state.read", json!({}))
        .expect("invocation");
    let outcome = coordinator
        .tool_step(&mut task, &backend, invocation, None, 1_001)
        .expect("tool step");
    assert!(matches!(
        outcome,
        ToolStepOutcome::Completed { output, .. } if output == json!({"state": "idle"})
    ));
    assert_eq!(task.usage.tool_calls, 1);
}

#[test]
fn module_catalog_and_health_reads_use_explicit_gateway_capabilities() {
    let tools = production_tool_registry().expect("registry");
    let providers = ProviderRegistry::default();
    let coordinator = AgentCoordinator::new(&providers, &tools);
    for (tool_id, expected_key) in [
        ("asset.catalog.read", "activeAssetId"),
        ("character.state.read", "renderer"),
        ("pet.action.catalog.read", "actions"),
        ("program.catalog.read", "programs"),
        ("runtime.health.read", "startup"),
    ] {
        let mut task = task();
        let backend = GatewayToolBackend::new(
            Backend::default(),
            GatewayToolBackend::<Backend>::standard_policy(task.id, task.trace_id),
        );
        let invocation =
            ToolInvocation::new(task.id, task.trace_id, tool_id, json!({})).expect("invocation");
        let outcome = coordinator
            .tool_step(&mut task, &backend, invocation, None, 1_001)
            .expect("tool step");
        assert!(matches!(
            outcome,
            ToolStepOutcome::Completed { output, .. } if output.get(expected_key).is_some()
        ));
    }
}

#[test]
fn automation_validation_is_a_side_effect_free_gateway_query() {
    let tools = production_tool_registry().expect("registry");
    let providers = ProviderRegistry::default();
    let coordinator = AgentCoordinator::new(&providers, &tools);
    let mut task = task();
    let capability_backend = Backend::default();
    let command_log = Arc::clone(&capability_backend.commands);
    let backend = GatewayToolBackend::new(
        capability_backend,
        GatewayToolBackend::<Backend>::standard_policy(task.id, task.trace_id),
    );
    let invocation = ToolInvocation::new(
        task.id,
        task.trace_id,
        "automation.definition.validate",
        json!({
            "definition": {"id": "local.focus.on-build"},
            "eventType": "dev.build.finished",
            "eventData": {"succeeded": true}
        }),
    )
    .expect("invocation");
    let outcome = coordinator
        .tool_step(&mut task, &backend, invocation, None, 1_001)
        .expect("tool step");
    assert!(matches!(
        outcome,
        ToolStepOutcome::Completed { output, .. }
            if output["status"] == "planned" && output["mode"] == "dry_run"
    ));
    assert!(command_log.lock().expect("command log").is_empty());
}

#[test]
fn write_tool_requires_bound_approval_before_fixed_gateway_command() {
    let tools = production_tool_registry().expect("registry");
    let providers = ProviderRegistry::default();
    let coordinator = AgentCoordinator::new(&providers, &tools);
    let mut task = task();
    let capability_backend = Backend::default();
    let command_log = Arc::clone(&capability_backend.commands);
    let backend = GatewayToolBackend::new(
        capability_backend,
        GatewayToolBackend::<Backend>::standard_policy(task.id, task.trace_id),
    );
    let invocation = ToolInvocation::new(
        task.id,
        task.trace_id,
        "pet.animation.play",
        json!({"action": "wave"}),
    )
    .expect("invocation");
    let waiting = coordinator
        .tool_step(&mut task, &backend, invocation.clone(), None, 1_001)
        .expect("admission");
    assert!(matches!(
        waiting,
        ToolStepOutcome::ConfirmationRequired { .. }
    ));
    assert_eq!(task.status, AgentTaskStatus::WaitingForConfirmation);
    assert_eq!(task.usage.tool_calls, 0);
    assert!(command_log.lock().expect("command log").is_empty());

    let ToolAdmission::ConfirmationRequired { effective_risk, .. } =
        tools.admit(&invocation).expect("admit")
    else {
        panic!("write tool must require confirmation");
    };
    assert_eq!(effective_risk, CommandRisk::Low);
    let approval = ToolApproval::bind(&invocation, effective_risk);
    let completed = coordinator
        .tool_step(
            &mut task,
            &backend,
            invocation.clone(),
            Some(&approval),
            1_002,
        )
        .expect("approved tool step");
    assert!(matches!(completed, ToolStepOutcome::Completed { .. }));
    let commands = command_log.lock().expect("command log");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "safe.pet.animate");
    assert_eq!(commands[0].trace_id, task.trace_id.to_string());
    assert_eq!(
        commands[0].idempotency_key.as_deref(),
        Some(invocation.invocation_id.to_string().as_str())
    );
}

#[test]
fn care_tool_is_strict_and_dispatches_only_the_fixed_gateway_command() {
    let tools = production_tool_registry().expect("registry");
    let mut task = task();
    let capability_backend = Backend::default();
    let command_log = Arc::clone(&capability_backend.commands);
    let backend = GatewayToolBackend::new(
        capability_backend,
        GatewayToolBackend::<Backend>::standard_policy(task.id, task.trace_id),
    );
    for arguments in [
        json!({"action": "sleep"}),
        json!({"action": "feed", "nowMs": 1}),
    ] {
        let invalid = ToolInvocation::new(task.id, task.trace_id, "pet.care.perform", arguments)
            .expect("invocation shape");
        let ToolAdmission::ConfirmationRequired { effective_risk, .. } =
            tools.admit(&invalid).expect("descriptor admission")
        else {
            panic!("care remains a confirmed write");
        };
        let approval = ToolApproval::bind(&invalid, effective_risk);
        assert!(tools.dispatch(&backend, &invalid, Some(&approval)).is_err());
    }

    let invocation = ToolInvocation::new(
        task.id,
        task.trace_id,
        "pet.care.perform",
        json!({"action": "feed"}),
    )
    .expect("invocation");
    let ToolAdmission::ConfirmationRequired { effective_risk, .. } =
        tools.admit(&invocation).expect("admit")
    else {
        panic!("care must require confirmation");
    };
    assert_eq!(effective_risk, CommandRisk::Low);
    let approval = ToolApproval::bind(&invocation, effective_risk);
    let providers = ProviderRegistry::default();
    let coordinator = AgentCoordinator::new(&providers, &tools);
    coordinator
        .tool_step(&mut task, &backend, invocation, Some(&approval), 1_002)
        .expect("approved care");
    let commands = command_log.lock().expect("command log");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "safe.pet.care");
    assert_eq!(commands[0].arguments, json!({"action": "feed"}));
}

#[test]
fn profile_switch_requires_bound_approval_and_fixed_gateway_command() {
    let tools = production_tool_registry().expect("registry");
    let providers = ProviderRegistry::default();
    let coordinator = AgentCoordinator::new(&providers, &tools);
    let mut task = task();
    let capability_backend = Backend::default();
    let command_log = Arc::clone(&capability_backend.commands);
    let backend = GatewayToolBackend::new(
        capability_backend,
        GatewayToolBackend::<Backend>::standard_policy(task.id, task.trace_id),
    );
    let profile_id = Uuid::now_v7();
    let invocation = ToolInvocation::new(
        task.id,
        task.trace_id,
        "profile.active.switch",
        json!({"profileId": profile_id}),
    )
    .expect("invocation");
    let waiting = coordinator
        .tool_step(&mut task, &backend, invocation.clone(), None, 1_001)
        .expect("admission");
    assert!(matches!(
        waiting,
        ToolStepOutcome::ConfirmationRequired { .. }
    ));
    assert!(command_log.lock().expect("command log").is_empty());

    let ToolAdmission::ConfirmationRequired { effective_risk, .. } =
        tools.admit(&invocation).expect("admit")
    else {
        panic!("profile switch must require confirmation");
    };
    let approval = ToolApproval::bind(&invocation, effective_risk);
    let completed = coordinator
        .tool_step(&mut task, &backend, invocation, Some(&approval), 1_002)
        .expect("approved tool step");
    assert!(matches!(completed, ToolStepOutcome::Completed { .. }));
    let commands = command_log.lock().expect("command log");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "safe.profile.switch");
    assert_eq!(commands[0].arguments, json!({"profileId": profile_id}));
}

#[test]
fn character_switch_requires_bound_approval_and_fixed_gateway_command() {
    let tools = production_tool_registry().expect("registry");
    let providers = ProviderRegistry::default();
    let coordinator = AgentCoordinator::new(&providers, &tools);
    let mut task = task();
    let capability_backend = Backend::default();
    let command_log = Arc::clone(&capability_backend.commands);
    let backend = GatewayToolBackend::new(
        capability_backend,
        GatewayToolBackend::<Backend>::standard_policy(task.id, task.trace_id),
    );
    let invocation = ToolInvocation::new(
        task.id,
        task.trace_id,
        "character.active.switch",
        json!({"assetId": "character.local.aurora"}),
    )
    .expect("invocation");
    let waiting = coordinator
        .tool_step(&mut task, &backend, invocation.clone(), None, 1_001)
        .expect("admission");
    assert!(matches!(
        waiting,
        ToolStepOutcome::ConfirmationRequired { .. }
    ));
    assert!(command_log.lock().expect("command log").is_empty());

    let ToolAdmission::ConfirmationRequired { effective_risk, .. } =
        tools.admit(&invocation).expect("admit")
    else {
        panic!("character switch must require confirmation");
    };
    let approval = ToolApproval::bind(&invocation, effective_risk);
    let completed = coordinator
        .tool_step(&mut task, &backend, invocation, Some(&approval), 1_002)
        .expect("approved tool step");
    assert!(matches!(completed, ToolStepOutcome::Completed { .. }));
    let commands = command_log.lock().expect("command log");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "safe.character.switch");
    assert_eq!(
        commands[0].arguments,
        json!({"assetId": "character.local.aurora"})
    );
}

#[test]
fn program_execute_requires_exact_version_approval_and_fixed_gateway_command() {
    let tools = production_tool_registry().expect("registry");
    let providers = ProviderRegistry::default();
    let coordinator = AgentCoordinator::new(&providers, &tools);
    let mut task = task();
    let capability_backend = Backend::default();
    let command_log = Arc::clone(&capability_backend.commands);
    let backend = GatewayToolBackend::new(
        capability_backend,
        GatewayToolBackend::<Backend>::standard_policy(task.id, task.trace_id),
    );
    let invocation = ToolInvocation::new(
        task.id,
        task.trace_id,
        "program.installed.execute",
        json!({"programId": "studio.example.focus", "version": "1.0.0"}),
    )
    .expect("invocation");
    let waiting = coordinator
        .tool_step(&mut task, &backend, invocation.clone(), None, 1_001)
        .expect("admission");
    assert!(matches!(
        waiting,
        ToolStepOutcome::ConfirmationRequired { .. }
    ));
    assert!(command_log.lock().expect("command log").is_empty());

    let ToolAdmission::ConfirmationRequired { effective_risk, .. } =
        tools.admit(&invocation).expect("admit")
    else {
        panic!("program execution must require confirmation");
    };
    assert_eq!(effective_risk, CommandRisk::Medium);
    let approval = ToolApproval::bind(&invocation, effective_risk);
    let completed = coordinator
        .tool_step(&mut task, &backend, invocation, Some(&approval), 1_002)
        .expect("approved tool step");
    assert!(matches!(completed, ToolStepOutcome::Completed { .. }));
    let commands = command_log.lock().expect("command log");
    assert_eq!(commands.len(), 1);
    assert_eq!(commands[0].command, "safe.program.execute");
    assert_eq!(
        commands[0].arguments,
        json!({"programId": "studio.example.focus", "version": "1.0.0"})
    );
}

#[test]
fn gateway_rejects_policy_correlation_mismatch_before_dispatch() {
    let tools = production_tool_registry().expect("registry");
    let task = task();
    let capability_backend = Backend::default();
    let command_log = Arc::clone(&capability_backend.commands);
    let backend = GatewayToolBackend::new(
        capability_backend,
        GatewayToolBackend::<Backend>::standard_policy(Uuid::now_v7(), task.trace_id),
    );
    let invocation = ToolInvocation::new(
        task.id,
        task.trace_id,
        "pet.animation.play",
        json!({"action": "wave"}),
    )
    .expect("invocation");
    let ToolAdmission::ConfirmationRequired { effective_risk, .. } =
        tools.admit(&invocation).expect("admit")
    else {
        panic!("write tool must require confirmation");
    };
    let approval = ToolApproval::bind(&invocation, effective_risk);
    assert!(
        tools
            .dispatch(&backend, &invocation, Some(&approval))
            .is_err()
    );
    assert!(command_log.lock().expect("command log").is_empty());
}
