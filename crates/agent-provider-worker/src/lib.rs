use nimora_agent_runtime::{
    ProviderError, ProviderErrorKind, ProviderFinishReason, ProviderMessageRole, ProviderRequest,
    ProviderResponse, ProviderToolCall, ProviderUsage,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, Ipv6Addr, SocketAddr, TcpStream},
    str::FromStr,
    time::Duration,
};
use zeroize::Zeroizing;

mod host;
mod openai_compatible;
mod sidecar;

pub use host::{
    ProviderCredentialResolver, WorkerOllamaProvider, WorkerOpenAiCompatibleProvider,
    probe_ollama_worker, probe_openai_worker,
};
pub use openai_compatible::{OpenAiCompatibleEndpoint, OpenAiModel, OpenAiProbe};
pub use sidecar::{
    ProviderWorkerManifest, SidecarVerificationError, VerifiedProviderWorker,
    verify_provider_worker,
};

const MAX_PROTOCOL_BYTES: usize = 1024 * 1024;
const MAX_HTTP_HEADER_BYTES: usize = 16 * 1024;
const MAX_HTTP_BODY_BYTES: usize = 1024 * 1024;
const MAX_OLLAMA_MODELS: usize = 256;
const MAX_OLLAMA_MODEL_NAME_BYTES: usize = 128;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProviderWorkerRequest {
    Complete {
        request: ProviderRequest,
        endpoint: OllamaEndpoint,
        timeout_ms: u64,
    },
    Probe {
        endpoint: OllamaEndpoint,
        timeout_ms: u64,
    },
    OpenAiComplete {
        request: ProviderRequest,
        endpoint: OpenAiCompatibleEndpoint,
        credential: WorkerSecret,
        timeout_ms: u64,
    },
    OpenAiProbe {
        endpoint: OpenAiCompatibleEndpoint,
        credential: WorkerSecret,
        timeout_ms: u64,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase", deny_unknown_fields)]
pub enum ProviderWorkerResponse {
    Completed { response: ProviderResponse },
    Probed { probe: OllamaProbe },
    OpenAiProbed { probe: OpenAiProbe },
    Error { error: ProviderError },
}

#[derive(Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct WorkerSecret(Zeroizing<String>);

impl WorkerSecret {
    /// Creates a bounded, non-empty Worker-only secret payload.
    ///
    /// # Errors
    ///
    /// Returns an error when the credential is empty, oversized, or contains NUL.
    pub fn new(value: impl Into<String>) -> Result<Self, ProviderError> {
        Self::from_zeroizing(Zeroizing::new(value.into()))
    }

    /// Adopts an already zeroizing credential without creating another plaintext copy.
    ///
    /// # Errors
    ///
    /// Returns an error when the credential is empty, oversized, or contains NUL.
    pub fn from_zeroizing(value: Zeroizing<String>) -> Result<Self, ProviderError> {
        if value.is_empty() || value.len() > 64 * 1024 || value.contains('\0') {
            return Err(stable_error(
                ProviderErrorKind::InvalidRequest,
                "provider credential is invalid",
            ));
        }
        Ok(Self(value))
    }

    pub(crate) fn expose(&self) -> &str {
        &self.0
    }
}

impl std::fmt::Debug for WorkerSecret {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("WorkerSecret([REDACTED])")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OllamaProbe {
    pub models: Vec<OllamaModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OllamaModel {
    pub name: String,
    pub size: u64,
    pub modified_at: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OllamaEndpoint {
    pub address: IpAddr,
    pub port: u16,
}

impl OllamaEndpoint {
    /// Creates a loopback-only Ollama endpoint.
    ///
    /// # Errors
    ///
    /// Returns an error for non-loopback addresses or port zero.
    pub fn new(address: IpAddr, port: u16) -> Result<Self, ProviderError> {
        if !address.is_loopback() || port == 0 {
            return Err(stable_error(
                ProviderErrorKind::InvalidRequest,
                "Ollama endpoint must be loopback",
            ));
        }
        Ok(Self { address, port })
    }

    #[must_use]
    pub const fn default_ipv4() -> Self {
        Self {
            address: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 11_434,
        }
    }

    #[must_use]
    pub const fn default_ipv6() -> Self {
        Self {
            address: IpAddr::V6(Ipv6Addr::LOCALHOST),
            port: 11_434,
        }
    }
}

/// Executes one bounded Worker protocol request.
#[must_use]
pub fn execute(request: ProviderWorkerRequest) -> ProviderWorkerResponse {
    match request {
        ProviderWorkerRequest::Complete {
            request,
            endpoint,
            timeout_ms,
        } => match complete_ollama(&request, endpoint, timeout_ms) {
            Ok(response) => ProviderWorkerResponse::Completed { response },
            Err(error) => ProviderWorkerResponse::Error { error },
        },
        ProviderWorkerRequest::Probe {
            endpoint,
            timeout_ms,
        } => match probe_ollama(endpoint, timeout_ms) {
            Ok(probe) => ProviderWorkerResponse::Probed { probe },
            Err(error) => ProviderWorkerResponse::Error { error },
        },
        ProviderWorkerRequest::OpenAiComplete {
            request,
            endpoint,
            credential,
            timeout_ms,
        } => match openai_compatible::complete(&request, &endpoint, &credential, timeout_ms) {
            Ok(response) => ProviderWorkerResponse::Completed { response },
            Err(error) => ProviderWorkerResponse::Error { error },
        },
        ProviderWorkerRequest::OpenAiProbe {
            endpoint,
            credential,
            timeout_ms,
        } => match openai_compatible::probe(&endpoint, &credential, timeout_ms) {
            Ok(probe) => ProviderWorkerResponse::OpenAiProbed { probe },
            Err(error) => ProviderWorkerResponse::Error { error },
        },
    }
}

/// Parses one bounded protocol document and returns one bounded response document.
///
/// # Errors
///
/// Returns a stable protocol error when the document is oversized or malformed.
pub fn execute_json(input: &[u8]) -> Result<Vec<u8>, ProviderError> {
    if input.is_empty() || input.len() > MAX_PROTOCOL_BYTES {
        return Err(stable_error(
            ProviderErrorKind::InvalidRequest,
            "worker request is outside protocol limits",
        ));
    }
    let request = serde_json::from_slice(input).map_err(|_| {
        stable_error(
            ProviderErrorKind::InvalidRequest,
            "worker request is malformed",
        )
    })?;
    serde_json::to_vec(&execute(request)).map_err(|_| {
        stable_error(
            ProviderErrorKind::MalformedResponse,
            "worker response serialization failed",
        )
    })
}

fn complete_ollama(
    request: &ProviderRequest,
    endpoint: OllamaEndpoint,
    timeout_ms: u64,
) -> Result<ProviderResponse, ProviderError> {
    validate_execution_policy(endpoint, timeout_ms)?;
    let payload = ollama_payload(request);
    let body = serde_json::to_vec(&payload).map_err(|_| {
        stable_error(
            ProviderErrorKind::InvalidRequest,
            "provider payload is invalid",
        )
    })?;
    let timeout = Duration::from_millis(timeout_ms);
    let mut stream = connect_ollama(endpoint, timeout)?;
    let headers = format!(
        "POST /api/chat HTTP/1.1\r\nHost: localhost:{}\r\nContent-Type: application/json\r\nAccept: application/json\r\nConnection: close\r\nContent-Length: {}\r\n\r\n",
        endpoint.port,
        body.len()
    );
    stream
        .write_all(headers.as_bytes())
        .and_then(|()| stream.write_all(&body))
        .and_then(|()| stream.flush())
        .map_err(|_| stable_error(ProviderErrorKind::Unavailable, "provider request failed"))?;
    let response_body = read_http_response(&mut stream)?;
    parse_ollama_response(request, &response_body)
}

fn probe_ollama(endpoint: OllamaEndpoint, timeout_ms: u64) -> Result<OllamaProbe, ProviderError> {
    validate_execution_policy(endpoint, timeout_ms)?;
    let timeout = Duration::from_millis(timeout_ms);
    let mut stream = connect_ollama(endpoint, timeout)?;
    let request = format!(
        "GET /api/tags HTTP/1.1\r\nHost: localhost:{}\r\nAccept: application/json\r\nConnection: close\r\n\r\n",
        endpoint.port
    );
    stream
        .write_all(request.as_bytes())
        .and_then(|()| stream.flush())
        .map_err(|_| stable_error(ProviderErrorKind::Unavailable, "provider request failed"))?;
    let body = read_http_response(&mut stream)?;
    parse_ollama_probe(&body)
}

fn validate_execution_policy(
    endpoint: OllamaEndpoint,
    timeout_ms: u64,
) -> Result<(), ProviderError> {
    if !endpoint.address.is_loopback()
        || endpoint.port == 0
        || timeout_ms == 0
        || timeout_ms > 600_000
    {
        return Err(stable_error(
            ProviderErrorKind::InvalidRequest,
            "worker execution policy rejected request",
        ));
    }
    Ok(())
}

fn connect_ollama(endpoint: OllamaEndpoint, timeout: Duration) -> Result<TcpStream, ProviderError> {
    let stream =
        TcpStream::connect_timeout(&SocketAddr::new(endpoint.address, endpoint.port), timeout)
            .map_err(|_| {
                stable_error(
                    ProviderErrorKind::Unavailable,
                    "local Ollama is unavailable",
                )
            })?;
    stream.set_read_timeout(Some(timeout)).map_err(|_| {
        stable_error(
            ProviderErrorKind::Unavailable,
            "provider timeout setup failed",
        )
    })?;
    stream.set_write_timeout(Some(timeout)).map_err(|_| {
        stable_error(
            ProviderErrorKind::Unavailable,
            "provider timeout setup failed",
        )
    })?;
    Ok(stream)
}

fn parse_ollama_probe(body: &[u8]) -> Result<OllamaProbe, ProviderError> {
    let document: OllamaTagsResponse = serde_json::from_slice(body).map_err(|_| {
        stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider model catalog is malformed",
        )
    })?;
    if document.models.len() > MAX_OLLAMA_MODELS {
        return Err(stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider model catalog exceeded limits",
        ));
    }
    let mut models = document
        .models
        .into_iter()
        .map(|model| {
            if model.name.is_empty() || model.name.len() > MAX_OLLAMA_MODEL_NAME_BYTES {
                return Err(stable_error(
                    ProviderErrorKind::MalformedResponse,
                    "provider model name is invalid",
                ));
            }
            Ok(OllamaModel {
                name: model.name,
                size: model.size,
                modified_at: model.modified_at,
            })
        })
        .collect::<Result<Vec<_>, ProviderError>>()?;
    models.sort_by(|left, right| left.name.cmp(&right.name));
    models.dedup_by(|left, right| left.name == right.name);
    Ok(OllamaProbe { models })
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct OllamaTagsResponse {
    models: Vec<OllamaTagModel>,
}

#[derive(Debug, Deserialize)]
struct OllamaTagModel {
    name: String,
    size: u64,
    #[serde(default)]
    modified_at: Option<String>,
}

fn ollama_payload(request: &ProviderRequest) -> Value {
    let messages = request
        .messages
        .iter()
        .map(|message| {
            let mut document = json!({
                "role": match message.role {
                    ProviderMessageRole::System => "system",
                    ProviderMessageRole::User => "user",
                    ProviderMessageRole::Assistant => "assistant",
                    ProviderMessageRole::Tool => "tool",
                },
                "content": message.content,
            });
            if !message.tool_calls.is_empty() {
                document["tool_calls"] = json!(
                    message
                        .tool_calls
                        .iter()
                        .map(|call| json!({
                            "function": {
                                "name": call.tool_id.to_string(),
                                "arguments": call.arguments,
                            }
                        }))
                        .collect::<Vec<_>>()
                );
            }
            if let Some(tool_call_id) = &message.tool_call_id {
                document["tool_call_id"] = json!(tool_call_id);
            }
            if let Some(tool_name) = &message.tool_name {
                document["tool_name"] = json!(tool_name.to_string());
            }
            document
        })
        .collect::<Vec<_>>();
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.id.to_string(),
                    "description": tool.description,
                    "parameters": tool.input_schema,
                }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "model": request.model,
        "messages": messages,
        "tools": tools,
        "stream": false,
        "options": {"num_predict": request.max_output_tokens}
    })
}

fn read_http_response(stream: &mut TcpStream) -> Result<Vec<u8>, ProviderError> {
    let mut bytes = Vec::new();
    stream
        .take((MAX_HTTP_HEADER_BYTES + MAX_HTTP_BODY_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| stable_error(ProviderErrorKind::Unavailable, "provider response failed"))?;
    if bytes.len() > MAX_HTTP_HEADER_BYTES + MAX_HTTP_BODY_BYTES {
        return Err(stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider response exceeded limits",
        ));
    }
    let boundary = bytes
        .windows(4)
        .position(|window| window == b"\r\n\r\n")
        .ok_or_else(|| {
            stable_error(
                ProviderErrorKind::MalformedResponse,
                "provider HTTP response is malformed",
            )
        })?;
    if boundary > MAX_HTTP_HEADER_BYTES {
        return Err(stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider headers exceeded limits",
        ));
    }
    let headers = std::str::from_utf8(&bytes[..boundary]).map_err(|_| {
        stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider headers are malformed",
        )
    })?;
    let mut lines = headers.split("\r\n");
    let status = lines.next().unwrap_or_default();
    if !status.starts_with("HTTP/1.1 200 ") && !status.starts_with("HTTP/1.0 200 ") {
        return Err(stable_error(
            ProviderErrorKind::Unavailable,
            "provider returned a non-success status",
        ));
    }
    let mut content_length = None;
    for line in lines {
        if let Some(value) = line
            .strip_prefix("Content-Length:")
            .or_else(|| line.strip_prefix("content-length:"))
        {
            content_length = value.trim().parse::<usize>().ok();
        }
        if line.eq_ignore_ascii_case("Transfer-Encoding: chunked") {
            return Err(stable_error(
                ProviderErrorKind::MalformedResponse,
                "chunked provider response is unsupported",
            ));
        }
    }
    let body = &bytes[boundary + 4..];
    if content_length != Some(body.len()) || body.len() > MAX_HTTP_BODY_BYTES {
        return Err(stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider body length is invalid",
        ));
    }
    Ok(body.to_vec())
}

fn parse_ollama_response(
    request: &ProviderRequest,
    body: &[u8],
) -> Result<ProviderResponse, ProviderError> {
    let document: OllamaResponse = serde_json::from_slice(body).map_err(|_| {
        stable_error(
            ProviderErrorKind::MalformedResponse,
            "provider JSON response is malformed",
        )
    })?;
    let tool_calls = document
        .message
        .tool_calls
        .into_iter()
        .enumerate()
        .map(|(index, call)| {
            let tool_id =
                nimora_agent_runtime::ToolId::from_str(&call.function.name).map_err(|_| {
                    stable_error(
                        ProviderErrorKind::MalformedResponse,
                        "provider returned invalid tool ID",
                    )
                })?;
            if !call.function.arguments.is_object() {
                return Err(stable_error(
                    ProviderErrorKind::MalformedResponse,
                    "provider returned invalid tool arguments",
                ));
            }
            Ok(ProviderToolCall {
                id: format!("ollama:{index}"),
                tool_id,
                arguments: call.function.arguments,
            })
        })
        .collect::<Result<Vec<_>, ProviderError>>()?;
    let finish_reason = if !tool_calls.is_empty() {
        ProviderFinishReason::ToolCalls
    } else if document.done_reason.as_deref() == Some("length") {
        ProviderFinishReason::Length
    } else {
        ProviderFinishReason::Completed
    };
    Ok(ProviderResponse {
        spec: "nimora.agent-provider-response/1".to_owned(),
        request_id: request.request_id,
        content: document.message.content,
        tool_calls,
        finish_reason,
        usage: ProviderUsage {
            input_tokens: document.prompt_eval_count.unwrap_or(0),
            output_tokens: document.eval_count.unwrap_or(0),
            cost_microunits: 0,
        },
    })
}

#[derive(Debug, Deserialize)]
struct OllamaResponse {
    message: OllamaMessage,
    #[serde(default)]
    done_reason: Option<String>,
    #[serde(default)]
    prompt_eval_count: Option<u64>,
    #[serde(default)]
    eval_count: Option<u64>,
}

#[derive(Debug, Deserialize)]
struct OllamaMessage {
    content: String,
    #[serde(default)]
    tool_calls: Vec<OllamaToolCall>,
}

#[derive(Debug, Deserialize)]
struct OllamaToolCall {
    function: OllamaFunction,
}

#[derive(Debug, Deserialize)]
struct OllamaFunction {
    name: String,
    arguments: Value,
}

fn stable_error(kind: ProviderErrorKind, message: &'static str) -> ProviderError {
    ProviderError::new(kind, message)
}
