use boa_engine::{Context, Script, Source};
use nimora_user_code_host::WorkerMessage;

const MAX_SOURCE_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EngineError {
    SourceTooLarge,
    JavaScript(String),
    ResultSerialization(String),
}

impl std::fmt::Display for EngineError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::SourceTooLarge => formatter.write_str("source exceeds the 256 KiB limit"),
            Self::JavaScript(message) => write!(formatter, "JavaScript error: {message}"),
            Self::ResultSerialization(message) => {
                write!(formatter, "result serialization failed: {message}")
            }
        }
    }
}

/// Evaluates one source unit in a fresh ECMAScript context.
///
/// The context has no Node.js globals, filesystem, network, process, Tauri, or
/// native module loader. Platform abilities are introduced separately through
/// the versioned Worker/Gateway protocol.
///
/// # Errors
///
/// Returns an error for oversized source, JavaScript failures, or values that
/// cannot be converted to JSON.
pub fn evaluate(source: &str) -> Result<serde_json::Value, EngineError> {
    evaluate_with_input(source, &serde_json::Value::Null)
}

/// Parses one source unit without executing top-level code.
///
/// # Errors
///
/// Returns an error when the source exceeds its budget or is not valid JavaScript.
pub fn validate(source: &str) -> Result<(), EngineError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(EngineError::SourceTooLarge);
    }
    let mut context = Context::default();
    Script::parse(Source::from_bytes(source.as_bytes()), None, &mut context)
        .map(|_| ())
        .map_err(|error| EngineError::JavaScript(error.to_string()))
}

/// Executes one source unit with an immutable input snapshot while discarding its return value.
///
/// # Errors
///
/// Returns an error for oversized source or JavaScript failures.
pub fn sandbox(source: &str, input: &serde_json::Value) -> Result<(), EngineError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(EngineError::SourceTooLarge);
    }
    let input = serde_json::to_string(input)
        .map_err(|error| EngineError::ResultSerialization(error.to_string()))?;
    let wrapped_source = format!(
        "const __nimoraDeepFreeze = (value) => {{\n  if (value && typeof value === 'object' && !Object.isFrozen(value)) {{\n    Object.freeze(value);\n    Object.values(value).forEach(__nimoraDeepFreeze);\n  }}\n  return value;\n}};\nconst nimora = Object.freeze({{ input: __nimoraDeepFreeze({input}) }});\n{source}"
    );
    Context::default()
        .eval(Source::from_bytes(wrapped_source.as_bytes()))
        .map(|_| ())
        .map_err(|error| EngineError::JavaScript(error.to_string()))
}

/// Evaluates one source unit with an immutable JSON input snapshot.
///
/// # Errors
///
/// Returns an error for oversized source, JavaScript failures, or values that
/// cannot be converted to JSON.
pub fn evaluate_with_input(
    source: &str,
    input: &serde_json::Value,
) -> Result<serde_json::Value, EngineError> {
    if source.len() > MAX_SOURCE_BYTES {
        return Err(EngineError::SourceTooLarge);
    }
    let input = serde_json::to_string(input)
        .map_err(|error| EngineError::ResultSerialization(error.to_string()))?;
    let wrapped_source = format!(
        "const __nimoraDeepFreeze = (value) => {{\n  if (value && typeof value === 'object' && !Object.isFrozen(value)) {{\n    Object.freeze(value);\n    Object.values(value).forEach(__nimoraDeepFreeze);\n  }}\n  return value;\n}};\nconst nimora = Object.freeze({{ input: __nimoraDeepFreeze({input}) }});\n{source}"
    );
    let mut context = Context::default();
    let result = context
        .eval(Source::from_bytes(wrapped_source.as_bytes()))
        .map_err(|error| EngineError::JavaScript(error.to_string()))?;
    result
        .to_json(&mut context)
        .map_err(|error| EngineError::ResultSerialization(error.to_string()))?
        .ok_or_else(|| EngineError::ResultSerialization("value is not JSON compatible".to_owned()))
}

#[must_use]
pub fn execute(message: WorkerMessage) -> WorkerMessage {
    match message {
        WorkerMessage::Validate { source } => match validate(&source) {
            Ok(()) => WorkerMessage::Validated,
            Err(error) => WorkerMessage::Error {
                code: "validation-error".to_owned(),
                message: error.to_string(),
            },
        },
        WorkerMessage::Sandbox { source, input, .. } => match sandbox(&source, &input) {
            Ok(()) => WorkerMessage::Sandboxed,
            Err(error) => WorkerMessage::Error {
                code: "sandbox-error".to_owned(),
                message: error.to_string(),
            },
        },
        WorkerMessage::Run { source, input, .. } => match evaluate_with_input(&source, &input) {
            Ok(value) => WorkerMessage::Result { value },
            Err(error) => WorkerMessage::Error {
                code: "engine-error".to_owned(),
                message: error.to_string(),
            },
        },
        _ => WorkerMessage::Error {
            code: "protocol-error".to_owned(),
            message: "worker expects a run message".to_owned(),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn evaluates_json_compatible_javascript() {
        assert_eq!(
            evaluate("({ answer: 6 * 7 })").unwrap(),
            json!({"answer": 42})
        );
    }

    #[test]
    fn validates_without_executing_top_level_code() {
        assert_eq!(
            execute(WorkerMessage::Validate {
                source: "throw new Error('must not run')".to_owned()
            }),
            WorkerMessage::Validated
        );
        assert!(matches!(
            execute(WorkerMessage::Validate { source: "const =".to_owned() }),
            WorkerMessage::Error { code, .. } if code == "validation-error"
        ));
    }

    #[test]
    fn sandboxes_top_level_code_without_requiring_a_json_result() {
        assert_eq!(
            execute(WorkerMessage::Sandbox {
                manifest: json!({"id": "example"}),
                source: "const answer = 42;".to_owned(),
                input: json!({"event": "test"}),
            }),
            WorkerMessage::Sandboxed
        );
        assert!(matches!(
            execute(WorkerMessage::Sandbox {
                manifest: json!({"id": "example"}),
                source: "throw new Error('boom')".to_owned(),
                input: serde_json::Value::Null,
            }),
            WorkerMessage::Error { code, .. } if code == "sandbox-error"
        ));
    }

    #[test]
    fn node_and_tauri_globals_are_not_available() {
        assert_eq!(
            evaluate("typeof process + ':' + typeof require + ':' + typeof __TAURI_INTERNALS__")
                .unwrap(),
            json!("undefined:undefined:undefined")
        );
    }

    #[test]
    fn reports_javascript_failures_as_protocol_errors() {
        let response = execute(WorkerMessage::Run {
            manifest: json!({}),
            source: "throw new Error('boom')".to_owned(),
            input: json!(null),
        });
        assert!(matches!(response, WorkerMessage::Error { code, .. } if code == "engine-error"));
    }

    #[test]
    fn exposes_only_the_read_only_input_snapshot() {
        assert_eq!(
            evaluate_with_input(
                "({ name: nimora.input.name, process: typeof process })",
                &json!({"name": "Aster"})
            )
            .unwrap(),
            json!({"name": "Aster", "process": "undefined"})
        );
    }

    #[test]
    fn deeply_freezes_the_input_snapshot() {
        assert_eq!(
            evaluate_with_input(
                "({ root: Object.isFrozen(nimora.input), nested: Object.isFrozen(nimora.input.pet) })",
                &json!({"pet": {"name": "Aster"}})
            )
            .unwrap(),
            json!({"root": true, "nested": true})
        );
    }
}
