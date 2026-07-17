use crate::{OllamaEndpoint, ProviderWorkerRequest, ProviderWorkerResponse};
use nimora_agent_runtime::{
    ProviderAdapter, ProviderCapabilities, ProviderCapability, ProviderDescriptor, ProviderError,
    ProviderErrorKind, ProviderExecutionContext, ProviderLocality, ProviderRequest,
    ProviderResponse,
};
use std::{
    collections::BTreeSet,
    io::{Read, Write},
    process::{Child, Command, Stdio},
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

const MAX_WORKER_OUTPUT_BYTES: u64 = 1024 * 1024;

type OutputReader = JoinHandle<Result<Vec<u8>, ProviderError>>;

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
        supervise_worker(&mut child, reader, context)
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
    context: &ProviderExecutionContext,
) -> Result<ProviderResponse, ProviderError> {
    let started = Instant::now();
    loop {
        if context.cancellation.is_cancelled() {
            terminate(child);
            return Err(stable_error(
                ProviderErrorKind::Cancelled,
                "provider request was cancelled",
            ));
        }
        if started.elapsed() >= context.timeout {
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

fn decode_worker_output(reader: OutputReader) -> Result<ProviderResponse, ProviderError> {
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
    match serde_json::from_slice(&output) {
        Ok(ProviderWorkerResponse::Completed { response }) => Ok(response),
        Ok(ProviderWorkerResponse::Error { error }) => Err(error),
        Err(_) => Err(stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider worker response is malformed",
        )),
    }
}

fn terminate(child: &mut Child) {
    let _ = child.kill();
    let _ = child.wait();
}

fn stable_error(kind: ProviderErrorKind, message: &'static str) -> ProviderError {
    ProviderError::new(kind, message)
}
