use boa_engine::{Context, Source};
use nimora_skill_host::{SKILL_WORKER_PROTOCOL_VERSION, SkillExecutionOutput, SkillWorkerMessage};
use nimora_skill_runtime::{SkillCapability, SkillManifest, validate_manifest};

const MAX_SOURCE_BYTES: usize = 512 * 1024;
const MAX_INPUT_BYTES: usize = 256 * 1024;
const MAX_COMMANDS: usize = 128;
const MAX_AGENT_TASKS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillEngineError {
    InvalidManifest(String),
    ProtocolVersion,
    SourceTooLarge,
    InputTooLarge,
    UndeclaredActivation,
    JavaScript(String),
    ResultSerialization(String),
    OutputLimit,
}

impl std::fmt::Display for SkillEngineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InvalidManifest(message) => write!(formatter, "invalid manifest: {message}"),
            Self::ProtocolVersion => formatter.write_str("unsupported worker protocol version"),
            Self::SourceTooLarge => formatter.write_str("source exceeds the 512 KiB limit"),
            Self::InputTooLarge => formatter.write_str("input exceeds the 256 KiB limit"),
            Self::UndeclaredActivation => formatter.write_str("activation event is not declared"),
            Self::JavaScript(message) => write!(formatter, "JavaScript error: {message}"),
            Self::ResultSerialization(message) => {
                write!(formatter, "result serialization failed: {message}")
            }
            Self::OutputLimit => formatter.write_str("Skill output exceeds contribution limits"),
        }
    }
}

/// Executes one Skill activation in a fresh ECMAScript context.
///
/// The injected SDK records capability requests as data. It cannot execute
/// host commands, providers, filesystem, network, process, or Tauri APIs.
///
/// # Errors
///
/// Returns an error for invalid admission, resource limits, JavaScript failure,
/// or a result that violates the protocol schema.
pub fn evaluate_activation(
    manifest: &SkillManifest,
    source: &str,
    activation_event: &str,
    input: &serde_json::Value,
) -> Result<SkillExecutionOutput, SkillEngineError> {
    validate_manifest(manifest.clone())
        .map_err(|error| SkillEngineError::InvalidManifest(error.to_string()))?;
    if !manifest.activation_events.contains(activation_event) {
        return Err(SkillEngineError::UndeclaredActivation);
    }
    if source.len() > MAX_SOURCE_BYTES {
        return Err(SkillEngineError::SourceTooLarge);
    }
    let input_json = serde_json::to_string(input)
        .map_err(|error| SkillEngineError::ResultSerialization(error.to_string()))?;
    if input_json.len() > MAX_INPUT_BYTES {
        return Err(SkillEngineError::InputTooLarge);
    }
    let event_json = serde_json::to_string(activation_event)
        .map_err(|error| SkillEngineError::ResultSerialization(error.to_string()))?;
    let command_enabled = manifest
        .capabilities
        .contains(&SkillCapability::InvokeCommands);
    let agent_enabled = manifest
        .capabilities
        .contains(&SkillCapability::InvokeAgentTasks)
        && manifest.contributions.agent_tasks;
    let wrapped_source = format!(
        r"
const __deepFreeze = (value) => {{
  if (value && typeof value === 'object' && !Object.isFrozen(value)) {{
    Object.freeze(value);
    Object.values(value).forEach(__deepFreeze);
  }}
  return value;
}};
const __commands = [];
const __agentTasks = [];
const __commandEnabled = {command_enabled};
const __agentEnabled = {agent_enabled};
const nimora = Object.freeze({{
  activation: Object.freeze({{ event: {event_json}, input: __deepFreeze({input_json}) }}),
  commands: Object.freeze({{
    invoke(commandId, arguments = null) {{
      if (!__commandEnabled) throw new Error('invoke-commands capability is unavailable');
      __commands.push({{ commandId, arguments }});
    }}
  }}),
  agent: Object.freeze({{
    request(request) {{
      if (!__agentEnabled) throw new Error('invoke-agent-tasks capability is unavailable');
      __agentTasks.push(request);
    }}
  }})
}});
(() => {{
{source}
}})();
({{ commands: __commands, agentTasks: __agentTasks }});
"
    );
    let mut context = Context::default();
    let value = context
        .eval(Source::from_bytes(wrapped_source.as_bytes()))
        .map_err(|error| SkillEngineError::JavaScript(error.to_string()))?;
    let value = value
        .to_json(&mut context)
        .map_err(|error| SkillEngineError::ResultSerialization(error.to_string()))?
        .ok_or_else(|| {
            SkillEngineError::ResultSerialization("value is not JSON compatible".to_owned())
        })?;
    let output = serde_json::from_value::<SkillExecutionOutput>(value)
        .map_err(|error| SkillEngineError::ResultSerialization(error.to_string()))?;
    if output.commands.len() > MAX_COMMANDS || output.agent_tasks.len() > MAX_AGENT_TASKS {
        return Err(SkillEngineError::OutputLimit);
    }
    Ok(output)
}

#[must_use]
pub fn execute(message: SkillWorkerMessage) -> SkillWorkerMessage {
    let SkillWorkerMessage::Run {
        protocol_version,
        execution_id,
        manifest,
        source,
        activation_event,
        input,
    } = message
    else {
        return protocol_error(None, "worker expects a run message");
    };
    if protocol_version != SKILL_WORKER_PROTOCOL_VERSION {
        return protocol_error(Some(execution_id), "unsupported protocol version");
    }
    match evaluate_activation(&manifest, &source, &activation_event, &input) {
        Ok(output) => SkillWorkerMessage::Completed {
            execution_id,
            output,
        },
        Err(error) => SkillWorkerMessage::Error {
            execution_id: Some(execution_id),
            code: "engine-error".to_owned(),
            message: error.to_string(),
        },
    }
}

fn protocol_error(execution_id: Option<String>, message: &str) -> SkillWorkerMessage {
    SkillWorkerMessage::Error {
        execution_id,
        code: "protocol-error".to_owned(),
        message: message.to_owned(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_skill_runtime::{SkillContributions, SkillManifest};
    use std::collections::BTreeSet;

    fn manifest() -> SkillManifest {
        SkillManifest {
            spec: "nimora.skill/1".to_owned(),
            id: "studio.example.focus".to_owned(),
            version: "1.0.0".to_owned(),
            publisher: "studio.example".to_owned(),
            entrypoint: "dist/main.js".to_owned(),
            capabilities: BTreeSet::from([
                SkillCapability::InvokeAgentTasks,
                SkillCapability::InvokeCommands,
            ]),
            activation_events: BTreeSet::from(["onStartup".to_owned()]),
            command_allowlist: BTreeSet::from(["safe.pet.animate".to_owned()]),
            contributions: SkillContributions {
                commands: Vec::new(),
                agent_tasks: true,
            },
        }
    }

    #[test]
    fn records_requests_without_exposing_native_globals() {
        let output = evaluate_activation(
            &manifest(),
            "nimora.commands.invoke('runtime.pet.action', { action: 'wave' });\nnimora.agent.request({ providerId: 'provider:local', model: 'echo', instruction: 'Draft.', context: [] });",
            "onStartup",
            &serde_json::json!({"reason": "boot"}),
        )
        .unwrap();
        assert_eq!(output.commands.len(), 1);
        assert_eq!(output.agent_tasks.len(), 1);
        assert_eq!(
            evaluate_activation(
                &manifest(),
                "return { process: typeof process, require: typeof require, tauri: typeof __TAURI_INTERNALS__ };",
                "onStartup",
                &serde_json::Value::Null,
            )
            .unwrap(),
            SkillExecutionOutput::default()
        );
    }

    #[test]
    fn rejects_undeclared_capability_at_execution_time() {
        let mut restricted = manifest();
        restricted
            .capabilities
            .remove(&SkillCapability::InvokeCommands);
        restricted.command_allowlist.clear();
        assert!(matches!(
            evaluate_activation(
                &restricted,
                "nimora.commands.invoke('runtime.pet.action');",
                "onStartup",
                &serde_json::Value::Null,
            ),
            Err(SkillEngineError::JavaScript(_))
        ));
    }
}
