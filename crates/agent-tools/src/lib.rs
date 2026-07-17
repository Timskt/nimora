use nimora_agent_runtime::{
    AgentRuntimeError, ToolBackend, ToolDescriptor, ToolEffect, ToolInvocation, ToolRegistry,
};
use nimora_runtime_core::CommandRisk;
use nimora_user_code_gateway::{
    AgentGatewayPolicy, CapabilityBackend, CapabilityGateway, CapabilityRequest,
    CapabilityResponse, GatewayEnvelope,
};
use serde_json::{Value, json};
use std::{collections::BTreeSet, time::Duration};

const PET_STATE_READ: &str = "pet.state.read";
const PET_ACTION_CATALOG_READ: &str = "pet.action.catalog.read";
const PROFILE_STATE_READ: &str = "profile.state.read";
const PROFILE_ACTIVE_SWITCH: &str = "profile.active.switch";
const CHARACTER_STATE_READ: &str = "character.state.read";
const CHARACTER_ACTIVE_SWITCH: &str = "character.active.switch";
const ASSET_CATALOG_READ: &str = "asset.catalog.read";
const PROGRAM_CATALOG_READ: &str = "program.catalog.read";
const PROGRAM_EXECUTE: &str = "program.installed.execute";
const RUNTIME_HEALTH_READ: &str = "runtime.health.read";
const AUTOMATION_DEFINITION_VALIDATE: &str = "automation.definition.validate";
const PET_ANIMATION_PLAY: &str = "pet.animation.play";
const PET_POSITION_MOVE: &str = "pet.position.move";
const SAFE_PET_ANIMATE: &str = "safe.pet.animate";
const SAFE_PET_MOVE: &str = "safe.pet.move";
const SAFE_PROFILE_SWITCH: &str = "safe.profile.switch";
const SAFE_CHARACTER_SWITCH: &str = "safe.character.switch";
const SAFE_PROGRAM_EXECUTE: &str = "safe.program.execute";

/// Builds the bounded production Tool Registry exposed to Agent providers.
///
/// # Errors
///
/// Returns an error if a built-in descriptor violates the Agent tool contract.
pub fn production_tool_registry() -> Result<ToolRegistry, AgentRuntimeError> {
    let mut registry = ToolRegistry::default();
    for descriptor in production_tool_descriptors()? {
        registry.register(descriptor)?;
    }
    Ok(registry)
}

/// Returns the built-in module Tool descriptors without backend implementation details.
///
/// # Errors
///
/// Returns an error if a built-in descriptor violates the Agent tool contract.
pub fn production_tool_descriptors() -> Result<Vec<ToolDescriptor>, AgentRuntimeError> {
    Ok(vec![
        descriptor(
            PET_STATE_READ,
            "Read pet state",
            "Reads the current pet state through the Capability Gateway.",
            empty_object_schema(),
            CommandRisk::Safe,
            ToolEffect::ReadOnly,
        )?,
        descriptor(
            PET_ACTION_CATALOG_READ,
            "Read pet action catalog",
            "Reads the exact action vocabulary accepted by the pet runtime through the Capability Gateway.",
            empty_object_schema(),
            CommandRisk::Safe,
            ToolEffect::ReadOnly,
        )?,
        descriptor(
            PROFILE_STATE_READ,
            "Read profile state",
            "Reads the active profile collection through the Capability Gateway.",
            empty_object_schema(),
            CommandRisk::Safe,
            ToolEffect::ReadOnly,
        )?,
        descriptor(
            PROFILE_ACTIVE_SWITCH,
            "Switch active profile",
            "Switches to one existing Profile and applies its native window policy through the Capability Gateway.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["profileId"],
                "properties": {
                    "profileId": {"type": "string", "format": "uuid"}
                }
            }),
            CommandRisk::Low,
            ToolEffect::ReversibleWrite,
        )?,
        descriptor(
            CHARACTER_STATE_READ,
            "Read character state",
            "Reads the active character and a path-free renderer capability summary through the Capability Gateway.",
            empty_object_schema(),
            CommandRisk::Safe,
            ToolEffect::ReadOnly,
        )?,
        character_switch_descriptor()?,
        descriptor(
            ASSET_CATALOG_READ,
            "Read asset catalog",
            "Reads installed character assets and active selection through the Capability Gateway.",
            empty_object_schema(),
            CommandRisk::Safe,
            ToolEffect::ReadOnly,
        )?,
        program_catalog_descriptor()?,
        program_execute_descriptor()?,
        descriptor(
            RUNTIME_HEALTH_READ,
            "Read runtime health",
            "Reads safety, startup, event delivery, and backup health through the Capability Gateway.",
            empty_object_schema(),
            CommandRisk::Safe,
            ToolEffect::ReadOnly,
        )?,
        automation_validation_descriptor()?,
        descriptor(
            PET_ANIMATION_PLAY,
            "Play pet animation",
            "Plays one validated pet action through the Capability Gateway.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["action"],
                "properties": {"action": {"type": "string"}}
            }),
            CommandRisk::Low,
            ToolEffect::ReversibleWrite,
        )?,
        descriptor(
            PET_POSITION_MOVE,
            "Move pet",
            "Moves the pet to validated finite screen coordinates through the Capability Gateway.",
            json!({
                "type": "object",
                "additionalProperties": false,
                "required": ["x", "y"],
                "properties": {
                    "x": {"type": "number"},
                    "y": {"type": "number"}
                }
            }),
            CommandRisk::Low,
            ToolEffect::ReversibleWrite,
        )?,
    ])
}

fn character_switch_descriptor() -> Result<ToolDescriptor, AgentRuntimeError> {
    descriptor(
        CHARACTER_ACTIVE_SWITCH,
        "Switch active character",
        "Switches to one installed character asset and refreshes the pet renderer through the Capability Gateway.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["assetId"],
            "properties": {
                "assetId": {"type": "string", "minLength": 1, "maxLength": 128}
            }
        }),
        CommandRisk::Low,
        ToolEffect::ReversibleWrite,
    )
}

fn automation_validation_descriptor() -> Result<ToolDescriptor, AgentRuntimeError> {
    descriptor(
        AUTOMATION_DEFINITION_VALIDATE,
        "Validate automation definition",
        "Validates and dry-runs one bounded automation definition through the Capability Gateway without executing module actions.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["definition", "eventType", "eventData"],
            "properties": {
                "definition": {"type": "object"},
                "eventType": {"type": "string"},
                "eventData": {"type": "object"}
            }
        }),
        CommandRisk::Safe,
        ToolEffect::ReadOnly,
    )
}

fn program_catalog_descriptor() -> Result<ToolDescriptor, AgentRuntimeError> {
    descriptor(
        PROGRAM_CATALOG_READ,
        "Read program catalog",
        "Reads verified installed user programs, exact versions, declared capabilities, and permission state without source or host paths.",
        empty_object_schema(),
        CommandRisk::Safe,
        ToolEffect::ReadOnly,
    )
}

fn program_execute_descriptor() -> Result<ToolDescriptor, AgentRuntimeError> {
    descriptor(
        PROGRAM_EXECUTE,
        "Execute installed program",
        "Executes one integrity-verified and permission-granted user program at an exact installed version.",
        json!({
            "type": "object",
            "additionalProperties": false,
            "required": ["programId", "version"],
            "properties": {
                "programId": {"type": "string", "minLength": 1, "maxLength": 128},
                "version": {"type": "string", "minLength": 5, "maxLength": 64}
            }
        }),
        CommandRisk::Medium,
        ToolEffect::ExternalSideEffect,
    )
}

#[derive(Debug)]
pub struct GatewayToolBackend<B> {
    gateway: CapabilityGateway<B>,
    policy: AgentGatewayPolicy,
}

impl<B: CapabilityBackend> GatewayToolBackend<B> {
    #[must_use]
    pub fn new(backend: B, policy: AgentGatewayPolicy) -> Self {
        Self {
            gateway: CapabilityGateway::new(backend),
            policy,
        }
    }

    #[must_use]
    pub fn standard_policy(task_id: uuid::Uuid, trace_id: uuid::Uuid) -> AgentGatewayPolicy {
        AgentGatewayPolicy {
            task_id,
            trace_id,
            read_capabilities: BTreeSet::from([
                "asset.catalog".to_owned(),
                "automation.definition.validate".to_owned(),
                "character.state".to_owned(),
                "pet.action.catalog".to_owned(),
                "pet.state".to_owned(),
                "profile.state".to_owned(),
                "program.catalog".to_owned(),
                "runtime.health".to_owned(),
            ]),
            commands: BTreeSet::from([
                SAFE_PET_ANIMATE.to_owned(),
                SAFE_PET_MOVE.to_owned(),
                SAFE_PROFILE_SWITCH.to_owned(),
                SAFE_CHARACTER_SWITCH.to_owned(),
                SAFE_PROGRAM_EXECUTE.to_owned(),
            ]),
        }
    }
}

impl<B: CapabilityBackend> ToolBackend for GatewayToolBackend<B> {
    fn invoke(
        &self,
        invocation: &ToolInvocation,
        _descriptor: &ToolDescriptor,
        _timeout: Duration,
    ) -> Result<Value, String> {
        let tool_id = invocation.tool_id.to_string();
        let request = match tool_id.as_str() {
            PET_STATE_READ => {
                require_empty_arguments(&invocation.arguments)?;
                CapabilityRequest::ReadPetState
            }
            PET_ACTION_CATALOG_READ => {
                require_empty_arguments(&invocation.arguments)?;
                CapabilityRequest::ReadPetActionCatalog
            }
            PROFILE_STATE_READ => {
                require_empty_arguments(&invocation.arguments)?;
                CapabilityRequest::ReadProfileState
            }
            PROFILE_ACTIVE_SWITCH => CapabilityRequest::InvokeCommand {
                command: SAFE_PROFILE_SWITCH.to_owned(),
                arguments: invocation.arguments.clone(),
            },
            CHARACTER_ACTIVE_SWITCH => CapabilityRequest::InvokeCommand {
                command: SAFE_CHARACTER_SWITCH.to_owned(),
                arguments: invocation.arguments.clone(),
            },
            CHARACTER_STATE_READ => {
                require_empty_arguments(&invocation.arguments)?;
                CapabilityRequest::ReadCharacterState
            }
            ASSET_CATALOG_READ => {
                require_empty_arguments(&invocation.arguments)?;
                CapabilityRequest::ReadAssetCatalog
            }
            PROGRAM_CATALOG_READ => {
                require_empty_arguments(&invocation.arguments)?;
                CapabilityRequest::ReadProgramCatalog
            }
            PROGRAM_EXECUTE => CapabilityRequest::InvokeCommand {
                command: SAFE_PROGRAM_EXECUTE.to_owned(),
                arguments: invocation.arguments.clone(),
            },
            RUNTIME_HEALTH_READ => {
                require_empty_arguments(&invocation.arguments)?;
                CapabilityRequest::ReadRuntimeHealth
            }
            AUTOMATION_DEFINITION_VALIDATE => CapabilityRequest::ValidateAutomation {
                definition: required_argument(&invocation.arguments, "definition")?.clone(),
                event_type: required_argument(&invocation.arguments, "eventType")?
                    .as_str()
                    .ok_or_else(|| "eventType must be a string".to_owned())?
                    .to_owned(),
                event_data: required_argument(&invocation.arguments, "eventData")?.clone(),
            },
            PET_ANIMATION_PLAY => CapabilityRequest::InvokeCommand {
                command: SAFE_PET_ANIMATE.to_owned(),
                arguments: invocation.arguments.clone(),
            },
            PET_POSITION_MOVE => CapabilityRequest::InvokeCommand {
                command: SAFE_PET_MOVE.to_owned(),
                arguments: invocation.arguments.clone(),
            },
            _ => return Err("tool has no registered Capability Gateway adapter".to_owned()),
        };
        let response = self
            .gateway
            .dispatch_agent(
                &self.policy,
                GatewayEnvelope {
                    execution_id: invocation.task_id.to_string(),
                    trace_id: invocation.trace_id.to_string(),
                    idempotency_key: Some(invocation.invocation_id.to_string()),
                    request,
                },
            )
            .map_err(|error| error.to_string())?;
        match response {
            CapabilityResponse::PetState { value }
            | CapabilityResponse::PetActionCatalog { value }
            | CapabilityResponse::ProfileState { value }
            | CapabilityResponse::CharacterState { value }
            | CapabilityResponse::AssetCatalog { value }
            | CapabilityResponse::ProgramCatalog { value }
            | CapabilityResponse::RuntimeHealth { value }
            | CapabilityResponse::AutomationValidation { value }
            | CapabilityResponse::CommandAccepted { value } => Ok(value),
            _ => Err("Capability Gateway returned an incompatible response".to_owned()),
        }
    }
}

fn descriptor(
    id: &str,
    name: &str,
    description: &str,
    input_schema: Value,
    risk: CommandRisk,
    effect: ToolEffect,
) -> Result<ToolDescriptor, AgentRuntimeError> {
    ToolDescriptor::new(
        id,
        name,
        description,
        input_schema,
        json!({"type": "object"}),
        risk,
        effect,
    )
}

fn empty_object_schema() -> Value {
    json!({"type": "object", "additionalProperties": false})
}

fn require_empty_arguments(arguments: &Value) -> Result<(), String> {
    if arguments.as_object().is_some_and(serde_json::Map::is_empty) {
        Ok(())
    } else {
        Err("read tool arguments must be an empty object".to_owned())
    }
}

fn required_argument<'a>(arguments: &'a Value, key: &str) -> Result<&'a Value, String> {
    arguments
        .as_object()
        .and_then(|object| object.get(key))
        .ok_or_else(|| format!("missing required argument: {key}"))
}
