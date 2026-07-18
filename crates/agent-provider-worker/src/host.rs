use crate::{
    OllamaEndpoint, OllamaProbe, OpenAiCompatibleEndpoint, OpenAiProbe, ProviderWorkerRequest,
    ProviderWorkerResponse, WorkerSecret,
};
use nimora_agent_runtime::{
    ProviderAdapter, ProviderCapabilities, ProviderCapability, ProviderDescriptor, ProviderError,
    ProviderErrorKind, ProviderExecutionContext, ProviderLocality, ProviderReasoningCapabilities,
    ProviderRequest, ProviderResponse,
};
use std::{
    collections::BTreeSet,
    fmt,
    io::{Read, Write},
    process::{Child, Command, Stdio},
    sync::Arc,
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};
use zeroize::Zeroize;

const MAX_WORKER_OUTPUT_BYTES: u64 = 1024 * 1024;

type OutputReader = JoinHandle<Result<Vec<u8>, ProviderError>>;

pub trait ProviderCredentialResolver: Send + Sync {
    /// Resolves one exact secret reference without exposing storage internals to the Worker.
    ///
    /// # Errors
    ///
    /// Returns a stable error when the reference is absent or the secret store is unavailable.
    fn resolve(&self, reference: &str) -> Result<WorkerSecret, ProviderError>;
}

pub struct WorkerOpenAiCompatibleProvider {
    descriptor: ProviderDescriptor,
    executable: std::path::PathBuf,
    endpoint: OpenAiCompatibleEndpoint,
    credential_reference: String,
    credential_resolver: Arc<dyn ProviderCredentialResolver>,
}

impl fmt::Debug for WorkerOpenAiCompatibleProvider {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("WorkerOpenAiCompatibleProvider")
            .field("descriptor", &self.descriptor)
            .field("executable", &self.executable)
            .field("endpoint", &self.endpoint)
            .field("credential_reference", &self.credential_reference)
            .field("credential_resolver", &"[REDACTED]")
            .finish()
    }
}

impl WorkerOpenAiCompatibleProvider {
    /// Creates a network Provider backed by the isolated Worker protocol.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid identity, limits, executable, or credential binding.
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        id: impl Into<String>,
        display_name: impl Into<String>,
        executable: impl Into<std::path::PathBuf>,
        endpoint: OpenAiCompatibleEndpoint,
        credential_reference: impl Into<String>,
        context_window_tokens: u64,
        max_output_tokens: u64,
        reasoning: Option<ProviderReasoningCapabilities>,
        credential_resolver: Arc<dyn ProviderCredentialResolver>,
    ) -> Result<Self, ProviderError> {
        let executable = executable.into();
        let credential_reference = credential_reference.into();
        if executable.as_os_str().is_empty()
            || credential_reference.is_empty()
            || credential_reference.len() > 160
        {
            return Err(stable_error(
                ProviderErrorKind::InvalidRequest,
                "provider configuration is invalid",
            ));
        }
        Ok(Self {
            descriptor: ProviderDescriptor::new(
                id,
                display_name,
                ProviderLocality::Network,
                context_window_tokens,
                max_output_tokens,
                ProviderCapabilities {
                    supported: BTreeSet::from([
                        ProviderCapability::StructuredToolCalls,
                        ProviderCapability::Cancellation,
                        ProviderCapability::UsageReporting,
                    ]),
                    reasoning,
                },
            )?,
            executable,
            endpoint,
            credential_reference,
            credential_resolver,
        })
    }
}

impl ProviderAdapter for WorkerOpenAiCompatibleProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn complete(
        &self,
        request: &ProviderRequest,
        context: &ProviderExecutionContext,
    ) -> Result<ProviderResponse, ProviderError> {
        validate_openai_context(context, &self.credential_reference)?;
        let credential = self
            .credential_resolver
            .resolve(&self.credential_reference)?;
        let timeout_ms = u64::try_from(context.timeout.as_millis()).unwrap_or(u64::MAX);
        let mut payload = serde_json::to_vec(&ProviderWorkerRequest::OpenAiComplete {
            request: request.clone(),
            endpoint: self.endpoint.clone(),
            credential,
            timeout_ms,
        })
        .map_err(|_| {
            stable_error(
                ProviderErrorKind::InvalidRequest,
                "provider worker request serialization failed",
            )
        })?;
        let spawned = spawn_worker(&self.executable, &payload);
        payload.zeroize();
        let (mut child, reader) = spawned?;
        match supervise_worker(&mut child, reader, context.timeout, || {
            context.cancellation.is_cancelled()
        })? {
            ProviderWorkerResponse::Completed { response } => Ok(response),
            ProviderWorkerResponse::Error { error } => Err(error),
            ProviderWorkerResponse::Probed { .. } | ProviderWorkerResponse::OpenAiProbed { .. } => {
                Err(stable_error(
                    ProviderErrorKind::MalformedResponse,
                    "provider worker returned an unexpected response",
                ))
            }
        }
    }
}

#[derive(Debug)]
pub struct WorkerOllamaProvider {
    descriptor: ProviderDescriptor,
    executable: std::path::PathBuf,
    endpoint: OllamaEndpoint,
}

impl WorkerOllamaProvider {
    /// Creates an Ollama Provider whose network transport runs only in the configured sidecar.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty executable or invalid Provider descriptor.
    pub fn new(
        executable: impl Into<std::path::PathBuf>,
        endpoint: OllamaEndpoint,
    ) -> Result<Self, ProviderError> {
        let executable = executable.into();
        if executable.as_os_str().is_empty() {
            return Err(stable_error(
                ProviderErrorKind::InvalidRequest,
                "provider worker executable is invalid",
            ));
        }
        Ok(Self {
            descriptor: ProviderDescriptor::new(
                "provider:ollama-loopback",
                "Ollama Loopback Worker",
                ProviderLocality::Local,
                128_000,
                32_768,
                ProviderCapabilities {
                    supported: BTreeSet::from([
                        ProviderCapability::StructuredToolCalls,
                        ProviderCapability::Cancellation,
                        ProviderCapability::UsageReporting,
                    ]),
                    reasoning: None,
                },
            )?,
            executable,
            endpoint,
        })
    }
}

impl ProviderAdapter for WorkerOllamaProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn complete(
        &self,
        request: &ProviderRequest,
        context: &ProviderExecutionContext,
    ) -> Result<ProviderResponse, ProviderError> {
        validate_context(context)?;
        let timeout_ms = u64::try_from(context.timeout.as_millis()).unwrap_or(u64::MAX);
        let payload = serde_json::to_vec(&ProviderWorkerRequest::Complete {
            request: request.clone(),
            endpoint: self.endpoint,
            timeout_ms,
        })
        .map_err(|_| {
            stable_error(
                ProviderErrorKind::InvalidRequest,
                "provider worker request serialization failed",
            )
        })?;
        let (mut child, reader) = spawn_worker(&self.executable, &payload)?;
        match supervise_worker(&mut child, reader, context.timeout, || {
            context.cancellation.is_cancelled()
        })? {
            ProviderWorkerResponse::Completed { response } => Ok(response),
            ProviderWorkerResponse::Error { error } => Err(error),
            ProviderWorkerResponse::Probed { .. } | ProviderWorkerResponse::OpenAiProbed { .. } => {
                Err(stable_error(
                    ProviderErrorKind::MalformedResponse,
                    "provider worker returned an unexpected response",
                ))
            }
        }
    }
}

/// Probes the local Ollama service through the isolated Provider Worker.
///
/// # Errors
///
/// Returns a stable Provider error when the Worker or loopback service fails.
pub fn probe_ollama_worker(
    executable: &std::path::Path,
    endpoint: OllamaEndpoint,
    timeout: Duration,
) -> Result<OllamaProbe, ProviderError> {
    let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
    let payload = serde_json::to_vec(&ProviderWorkerRequest::Probe {
        endpoint,
        timeout_ms,
    })
    .map_err(|_| {
        stable_error(
            ProviderErrorKind::InvalidRequest,
            "provider worker request serialization failed",
        )
    })?;
    let (mut child, reader) = spawn_worker(executable, &payload)?;
    match supervise_worker(&mut child, reader, timeout, || false)? {
        ProviderWorkerResponse::Probed { probe } => Ok(probe),
        ProviderWorkerResponse::Error { error } => Err(error),
        ProviderWorkerResponse::Completed { .. } | ProviderWorkerResponse::OpenAiProbed { .. } => {
            Err(stable_error(
                ProviderErrorKind::MalformedResponse,
                "provider worker returned an unexpected response",
            ))
        }
    }
}

/// Probes an OpenAI-compatible service through the isolated Provider Worker.
///
/// # Errors
///
/// Returns a stable Provider error without exposing the endpoint, credential, or response body.
pub fn probe_openai_worker(
    executable: &std::path::Path,
    endpoint: OpenAiCompatibleEndpoint,
    credential: WorkerSecret,
    timeout: Duration,
) -> Result<OpenAiProbe, ProviderError> {
    let timeout_ms = u64::try_from(timeout.as_millis()).unwrap_or(u64::MAX);
    let mut payload = serde_json::to_vec(&ProviderWorkerRequest::OpenAiProbe {
        endpoint,
        credential,
        timeout_ms,
    })
    .map_err(|_| {
        stable_error(
            ProviderErrorKind::InvalidRequest,
            "provider worker request serialization failed",
        )
    })?;
    let spawned = spawn_worker(executable, &payload);
    payload.zeroize();
    let (mut child, reader) = spawned?;
    match supervise_worker(&mut child, reader, timeout, || false)? {
        ProviderWorkerResponse::OpenAiProbed { probe } => Ok(probe),
        ProviderWorkerResponse::Error { error } => Err(error),
        ProviderWorkerResponse::Completed { .. } | ProviderWorkerResponse::Probed { .. } => {
            Err(stable_error(
                ProviderErrorKind::MalformedResponse,
                "provider worker returned an unexpected response",
            ))
        }
    }
}

fn validate_context(context: &ProviderExecutionContext) -> Result<(), ProviderError> {
    if context.credential_reference.is_some() {
        return Err(stable_error(
            ProviderErrorKind::InvalidRequest,
            "Ollama loopback Provider does not accept credentials",
        ));
    }
    if context.cancellation.is_cancelled() {
        return Err(stable_error(
            ProviderErrorKind::Cancelled,
            "provider request was cancelled",
        ));
    }
    Ok(())
}

fn validate_openai_context(
    context: &ProviderExecutionContext,
    expected_reference: &str,
) -> Result<(), ProviderError> {
    if context.credential_reference.as_deref() != Some(expected_reference) {
        return Err(stable_error(
            ProviderErrorKind::InvalidRequest,
            "provider credential binding is invalid",
        ));
    }
    if context.cancellation.is_cancelled() {
        return Err(stable_error(
            ProviderErrorKind::Cancelled,
            "provider request was cancelled",
        ));
    }
    Ok(())
}

fn spawn_worker(
    executable: &std::path::Path,
    payload: &[u8],
) -> Result<(Child, OutputReader), ProviderError> {
    let mut child = Command::new(executable)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|_| {
            stable_error(
                ProviderErrorKind::Unavailable,
                "provider worker could not start",
            )
        })?;
    let write_result = child
        .stdin
        .take()
        .ok_or_else(|| {
            stable_error(
                ProviderErrorKind::Unavailable,
                "provider worker input is unavailable",
            )
        })?
        .write_all(payload);
    if write_result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
        return Err(stable_error(
            ProviderErrorKind::Unavailable,
            "provider worker input failed",
        ));
    }
    let stdout = child.stdout.take().ok_or_else(|| {
        stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider worker output is unavailable",
        )
    })?;
    let reader = thread::spawn(move || {
        let mut output = Vec::new();
        stdout
            .take(MAX_WORKER_OUTPUT_BYTES + 1)
            .read_to_end(&mut output)
            .map_err(|_| {
                stable_error(
                    ProviderErrorKind::MalformedResponse,
                    "provider worker output failed",
                )
            })?;
        Ok(output)
    });
    Ok((child, reader))
}

fn supervise_worker(
    child: &mut Child,
    reader: OutputReader,
    timeout: Duration,
    is_cancelled: impl Fn() -> bool,
) -> Result<ProviderWorkerResponse, ProviderError> {
    let started = Instant::now();
    loop {
        if is_cancelled() {
            terminate(child);
            return Err(stable_error(
                ProviderErrorKind::Cancelled,
                "provider request was cancelled",
            ));
        }
        if started.elapsed() >= timeout {
            terminate(child);
            return Err(stable_error(
                ProviderErrorKind::Timeout,
                "provider worker timed out",
            ));
        }
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return decode_worker_output(reader),
            Ok(Some(_)) => {
                return Err(stable_error(
                    ProviderErrorKind::Unavailable,
                    "provider worker exited unsuccessfully",
                ));
            }
            Ok(None) => thread::sleep(Duration::from_millis(5)),
            Err(_) => {
                terminate(child);
                return Err(stable_error(
                    ProviderErrorKind::Unavailable,
                    "provider worker status failed",
                ));
            }
        }
    }
}

fn decode_worker_output(reader: OutputReader) -> Result<ProviderWorkerResponse, ProviderError> {
    let output = reader.join().map_err(|_| {
        stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider worker output task failed",
        )
    })??;
    if output.len() as u64 > MAX_WORKER_OUTPUT_BYTES {
        return Err(stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider worker output exceeded limits",
        ));
    }
    serde_json::from_slice(&output).map_err(|_| {
        stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider worker response is malformed",
        )
    })
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn stable_error(kind: ProviderErrorKind, message: &'static str) -> ProviderError {
    ProviderError::new(kind, message)
}
