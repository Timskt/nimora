use nimora_skill_runtime::{SkillHost, SkillManifest, validate_manifest};
pub use nimora_user_code_policy::ExecutionCancellation;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

pub const SKILL_WORKER_PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillCommandRequest {
    pub command_id: String,
    #[serde(default)]
    pub arguments: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillContextSegment {
    pub source: String,
    pub content: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillAgentTaskRequest {
    pub provider_id: String,
    pub model: String,
    pub instruction: String,
    #[serde(default)]
    pub context: Vec<SkillContextSegment>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillExecutionOutput {
    #[serde(default)]
    pub commands: Vec<SkillCommandRequest>,
    #[serde(default)]
    pub agent_tasks: Vec<SkillAgentTaskRequest>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SkillWorkerMessage {
    Validate {
        protocol_version: u16,
        execution_id: String,
        manifest: Box<SkillManifest>,
        source: String,
    },
    Run {
        protocol_version: u16,
        execution_id: String,
        manifest: Box<SkillManifest>,
        source: String,
        activation_event: String,
        #[serde(default)]
        input: serde_json::Value,
    },
    Completed {
        execution_id: String,
        output: SkillExecutionOutput,
    },
    Validated {
        execution_id: String,
    },
    Error {
        execution_id: Option<String>,
        code: String,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct SkillWorkerConfig {
    pub executable: String,
    pub args: Vec<String>,
    pub execution_id: String,
    pub timeout: Duration,
    pub output_bytes: u64,
    pub cancellation: Option<ExecutionCancellation>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum SkillHostError {
    #[error("skill worker request was rejected: {0}")]
    Admission(String),
    #[error("failed to start skill worker: {0}")]
    Start(String),
    #[error("skill worker protocol error: {0}")]
    Protocol(String),
    #[error("skill worker timed out")]
    TimedOut,
    #[error("skill worker was cancelled")]
    Cancelled,
    #[error("skill worker output exceeded its budget")]
    OutputLimit,
    #[error("skill worker exited without a result")]
    Crashed,
    #[error("skill worker I/O error: {0}")]
    Io(String),
}

pub struct SkillWorkerProcess {
    child: Child,
    lines: Receiver<Result<Vec<u8>, String>>,
    started: Instant,
    config: SkillWorkerConfig,
    output_seen: u64,
    cancelled: bool,
}

impl std::fmt::Debug for SkillWorkerProcess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SkillWorkerProcess")
            .field("execution_id", &self.config.execution_id)
            .field("output_seen", &self.output_seen)
            .field("cancelled", &self.cancelled)
            .finish_non_exhaustive()
    }
}

impl SkillWorkerProcess {
    /// Starts an isolated Skill Worker after validating its request envelope.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid admission, process startup, encoding, or I/O.
    pub fn spawn(
        config: SkillWorkerConfig,
        request: &SkillWorkerMessage,
        lifecycle: &SkillHost,
    ) -> Result<Self, SkillHostError> {
        validate_run_request(&config, request, lifecycle)?;
        let mut command = Command::new(&config.executable);
        command
            .args(&config.args)
            .env_clear()
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|error| SkillHostError::Start(error.to_string()))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| SkillHostError::Start("worker stdin unavailable".to_owned()))?;
        let payload = serde_json::to_vec(request)
            .map_err(|error| SkillHostError::Protocol(error.to_string()))?;
        stdin
            .write_all(&payload)
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush())
            .map_err(|error| SkillHostError::Io(error.to_string()))?;
        drop(stdin);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| SkillHostError::Start("worker stdout unavailable".to_owned()))?;
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).split(b'\n') {
                if sender
                    .send(line.map_err(|error| error.to_string()))
                    .is_err()
                {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            lines,
            started: Instant::now(),
            config,
            output_seen: 0,
            cancelled: false,
        })
    }

    /// Terminates the worker immediately.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system rejects termination.
    pub fn cancel(&mut self) -> Result<(), SkillHostError> {
        self.cancelled = true;
        self.child
            .kill()
            .map_err(|error| SkillHostError::Io(error.to_string()))
    }

    /// Waits for one correlated terminal response while enforcing host limits.
    ///
    /// # Errors
    ///
    /// Returns an error for cancellation, timeout, crash, protocol, or output violations.
    pub fn wait(&mut self) -> Result<SkillWorkerMessage, SkillHostError> {
        loop {
            if self
                .config
                .cancellation
                .as_ref()
                .is_some_and(ExecutionCancellation::is_cancelled)
            {
                self.cancelled = true;
                self.terminate();
                return Err(SkillHostError::Cancelled);
            }
            if self.started.elapsed() >= self.config.timeout {
                self.terminate();
                return Err(if self.cancelled {
                    SkillHostError::Cancelled
                } else {
                    SkillHostError::TimedOut
                });
            }
            match self.lines.recv_timeout(Duration::from_millis(10)) {
                Ok(Ok(line)) => {
                    self.output_seen = self.output_seen.saturating_add(line.len() as u64 + 1);
                    if self.output_seen > self.config.output_bytes {
                        self.terminate();
                        return Err(SkillHostError::OutputLimit);
                    }
                    if line.is_empty() {
                        continue;
                    }
                    let message = serde_json::from_slice::<SkillWorkerMessage>(&line)
                        .map_err(|error| SkillHostError::Protocol(error.to_string()))?;
                    validate_response(&self.config.execution_id, &message)?;
                    self.terminate();
                    return Ok(message);
                }
                Ok(Err(error)) => return Err(SkillHostError::Io(error)),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    self.terminate();
                    return Err(SkillHostError::Crashed);
                }
            }
        }
    }

    /// Waits for completion and records isolation failures in the Skill lifecycle.
    ///
    /// Cancellation is an expected host action and does not increment the crash
    /// window. Engine-level errors are correlated terminal messages and remain
    /// execution failures rather than Worker crashes.
    ///
    /// # Errors
    ///
    /// Returns the original Worker error after best-effort lifecycle recording.
    pub fn wait_recording_failure(
        &mut self,
        lifecycle: &mut SkillHost,
        skill_id: &str,
        now_ms: u64,
    ) -> Result<SkillWorkerMessage, SkillHostError> {
        let result = self.wait();
        if result.as_ref().is_err_and(is_isolation_failure) {
            let _ = lifecycle.record_crash(skill_id, now_ms);
        }
        result
    }

    fn terminate(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn is_isolation_failure(error: &SkillHostError) -> bool {
    matches!(
        error,
        SkillHostError::Protocol(_)
            | SkillHostError::TimedOut
            | SkillHostError::OutputLimit
            | SkillHostError::Crashed
            | SkillHostError::Io(_)
    )
}

impl Drop for SkillWorkerProcess {
    fn drop(&mut self) {
        self.terminate();
    }
}

fn validate_run_request(
    config: &SkillWorkerConfig,
    request: &SkillWorkerMessage,
    lifecycle: &SkillHost,
) -> Result<(), SkillHostError> {
    let (protocol_version, execution_id, manifest, activation_event) = match request {
        SkillWorkerMessage::Run {
            protocol_version,
            execution_id,
            manifest,
            activation_event,
            ..
        } => (
            *protocol_version,
            execution_id,
            manifest,
            Some(activation_event),
        ),
        SkillWorkerMessage::Validate {
            protocol_version,
            execution_id,
            manifest,
            ..
        } => (*protocol_version, execution_id, manifest, None),
        _ => {
            return Err(SkillHostError::Admission(
                "expected run or validate request".to_owned(),
            ));
        }
    };
    if protocol_version != SKILL_WORKER_PROTOCOL_VERSION || execution_id != &config.execution_id {
        return Err(SkillHostError::Admission(
            "protocol version or execution identity mismatch".to_owned(),
        ));
    }
    validate_manifest((**manifest).clone())
        .map_err(|error| SkillHostError::Admission(error.to_string()))?;
    if let Some(activation_event) = activation_event {
        let active_manifest = lifecycle
            .active_manifest(&manifest.id)
            .map_err(|error| SkillHostError::Admission(error.to_string()))?;
        if active_manifest != manifest.as_ref() {
            return Err(SkillHostError::Admission(
                "request manifest does not match the active Skill lease".to_owned(),
            ));
        }
        if !manifest.activation_events.contains(activation_event) {
            return Err(SkillHostError::Admission(
                "activation event is not declared by the Skill".to_owned(),
            ));
        }
    }
    Ok(())
}

fn validate_response(
    execution_id: &str,
    response: &SkillWorkerMessage,
) -> Result<(), SkillHostError> {
    let (SkillWorkerMessage::Completed {
        execution_id: response_id,
        ..
    }
    | SkillWorkerMessage::Validated {
        execution_id: response_id,
    }
    | SkillWorkerMessage::Error {
        execution_id: Some(response_id),
        ..
    }) = response
    else {
        return Err(SkillHostError::Protocol(
            "worker returned a non-terminal or uncorrelated response".to_owned(),
        ));
    };
    if response_id != execution_id {
        return Err(SkillHostError::Protocol(
            "worker response execution identity mismatch".to_owned(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_skill_runtime::{SkillCapability, SkillContributions};
    use std::collections::BTreeSet;

    fn manifest() -> SkillManifest {
        SkillManifest {
            spec: "nimora.skill/1".to_owned(),
            id: "studio.example.validator".to_owned(),
            version: "1.0.0".to_owned(),
            publisher: "studio.example".to_owned(),
            entrypoint: "dist/main.js".to_owned(),
            capabilities: BTreeSet::from([SkillCapability::InvokeCommands]),
            activation_events: BTreeSet::from(["onStartup".to_owned()]),
            command_allowlist: BTreeSet::from(["safe.pet.animate".to_owned()]),
            contributions: SkillContributions::default(),
        }
    }

    #[test]
    fn protocol_version_is_explicit() {
        assert_eq!(SKILL_WORKER_PROTOCOL_VERSION, 1);
    }

    #[test]
    fn validation_is_admitted_without_an_active_skill_lease() {
        let request = SkillWorkerMessage::Validate {
            protocol_version: SKILL_WORKER_PROTOCOL_VERSION,
            execution_id: "validation-1".to_owned(),
            manifest: Box::new(manifest()),
            source: "export default {};".to_owned(),
        };
        let config = SkillWorkerConfig {
            executable: "unused".to_owned(),
            args: Vec::new(),
            execution_id: "validation-1".to_owned(),
            timeout: Duration::from_secs(1),
            output_bytes: 1024,
            cancellation: None,
        };

        assert_eq!(
            validate_run_request(&config, &request, &SkillHost::default()),
            Ok(())
        );
    }

    #[test]
    fn validated_response_must_match_execution_identity() {
        assert_eq!(
            validate_response(
                "validation-1",
                &SkillWorkerMessage::Validated {
                    execution_id: "validation-1".to_owned(),
                },
            ),
            Ok(())
        );
        assert!(matches!(
            validate_response(
                "validation-1",
                &SkillWorkerMessage::Validated {
                    execution_id: "validation-2".to_owned(),
                },
            ),
            Err(SkillHostError::Protocol(_))
        ));
    }

    fn config(execution_id: &str) -> SkillWorkerConfig {
        SkillWorkerConfig {
            executable: "unused".to_owned(),
            args: Vec::new(),
            execution_id: execution_id.to_owned(),
            timeout: Duration::from_secs(1),
            output_bytes: 1024,
            cancellation: None,
        }
    }

    fn activated_host() -> SkillHost {
        let mut host = SkillHost::default();
        let validated = validate_manifest(manifest()).expect("validate manifest");
        host.install(validated).expect("install");
        host.authorize(nimora_skill_runtime::SkillGrant {
            skill_id: "studio.example.validator".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: BTreeSet::from([SkillCapability::InvokeCommands]),
        })
        .expect("authorize");
        host.activate("studio.example.validator").expect("activate");
        host
    }

    fn run_request(execution_id: &str, activation_event: &str) -> SkillWorkerMessage {
        SkillWorkerMessage::Run {
            protocol_version: SKILL_WORKER_PROTOCOL_VERSION,
            execution_id: execution_id.to_owned(),
            manifest: Box::new(manifest()),
            source: "export default {};".to_owned(),
            activation_event: activation_event.to_owned(),
            input: serde_json::Value::Null,
        }
    }

    #[test]
    fn run_request_is_admitted_against_a_matching_active_lease() {
        let host = activated_host();
        assert_eq!(
            validate_run_request(&config("run-1"), &run_request("run-1", "onStartup"), &host),
            Ok(())
        );
    }

    #[test]
    fn run_request_rejects_undeclared_activation_event() {
        let host = activated_host();
        assert!(matches!(
            validate_run_request(&config("run-1"), &run_request("run-1", "onOther"), &host),
            Err(SkillHostError::Admission(_))
        ));
    }

    #[test]
    fn run_request_rejects_a_manifest_that_diverges_from_the_lease() {
        let host = activated_host();
        let SkillWorkerMessage::Run {
            manifest: mut divergent,
            ..
        } = run_request("run-1", "onStartup")
        else {
            unreachable!("constructed a run request")
        };
        divergent.version = "2.0.0".to_owned();
        let request = SkillWorkerMessage::Run {
            protocol_version: SKILL_WORKER_PROTOCOL_VERSION,
            execution_id: "run-1".to_owned(),
            manifest: divergent,
            source: "export default {};".to_owned(),
            activation_event: "onStartup".to_owned(),
            input: serde_json::Value::Null,
        };
        assert!(matches!(
            validate_run_request(&config("run-1"), &request, &host),
            Err(SkillHostError::Admission(_))
        ));
    }

    #[test]
    fn run_request_rejects_protocol_and_identity_and_shape_violations() {
        let host = SkillHost::default();
        let wrong_protocol = SkillWorkerMessage::Validate {
            protocol_version: SKILL_WORKER_PROTOCOL_VERSION + 7,
            execution_id: "validation-1".to_owned(),
            manifest: Box::new(manifest()),
            source: "export default {};".to_owned(),
        };
        assert!(matches!(
            validate_run_request(&config("validation-1"), &wrong_protocol, &host),
            Err(SkillHostError::Admission(_))
        ));

        let wrong_identity = SkillWorkerMessage::Validate {
            protocol_version: SKILL_WORKER_PROTOCOL_VERSION,
            execution_id: "validation-2".to_owned(),
            manifest: Box::new(manifest()),
            source: "export default {};".to_owned(),
        };
        assert!(matches!(
            validate_run_request(&config("validation-1"), &wrong_identity, &host),
            Err(SkillHostError::Admission(_))
        ));

        let terminal = SkillWorkerMessage::Validated {
            execution_id: "validation-1".to_owned(),
        };
        assert!(matches!(
            validate_run_request(&config("validation-1"), &terminal, &host),
            Err(SkillHostError::Admission(_))
        ));
    }

    #[test]
    fn validate_response_accepts_correlated_terminals_and_rejects_the_rest() {
        assert_eq!(
            validate_response(
                "run-1",
                &SkillWorkerMessage::Completed {
                    execution_id: "run-1".to_owned(),
                    output: SkillExecutionOutput::default(),
                },
            ),
            Ok(())
        );
        assert_eq!(
            validate_response(
                "run-1",
                &SkillWorkerMessage::Error {
                    execution_id: Some("run-1".to_owned()),
                    code: "engine".to_owned(),
                    message: "boom".to_owned(),
                },
            ),
            Ok(())
        );
        assert!(matches!(
            validate_response(
                "run-1",
                &SkillWorkerMessage::Error {
                    execution_id: None,
                    code: "startup".to_owned(),
                    message: "no id".to_owned(),
                },
            ),
            Err(SkillHostError::Protocol(_))
        ));
        assert!(matches!(
            validate_response(
                "run-1",
                &SkillWorkerMessage::Validate {
                    protocol_version: SKILL_WORKER_PROTOCOL_VERSION,
                    execution_id: "run-1".to_owned(),
                    manifest: Box::new(manifest()),
                    source: "export default {};".to_owned(),
                },
            ),
            Err(SkillHostError::Protocol(_))
        ));
    }

    #[test]
    fn isolation_failures_are_classified_apart_from_admission_faults() {
        for error in [
            SkillHostError::Protocol("p".to_owned()),
            SkillHostError::TimedOut,
            SkillHostError::OutputLimit,
            SkillHostError::Crashed,
            SkillHostError::Io("i".to_owned()),
        ] {
            assert!(is_isolation_failure(&error), "{error:?}");
        }
        for error in [
            SkillHostError::Admission("a".to_owned()),
            SkillHostError::Start("s".to_owned()),
            SkillHostError::Cancelled,
        ] {
            assert!(!is_isolation_failure(&error), "{error:?}");
        }
    }
}
