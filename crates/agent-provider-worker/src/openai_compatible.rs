use crate::{WorkerSecret, stable_error};
use nimora_agent_runtime::{
    ProviderError, ProviderErrorKind, ProviderFinishReason, ProviderMessageRole, ProviderRequest,
    ProviderResponse, ProviderToolCall, ProviderUsage, ToolId,
};
use reqwest::{
    StatusCode, Url,
    blocking::{Client, Response},
    redirect::Policy,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{io::Read, str::FromStr, time::Duration};

const MAX_RESPONSE_BYTES: u64 = 1024 * 1024;
const MAX_MODELS: usize = 256;
const MAX_MODEL_NAME_BYTES: usize = 128;
const MAX_TOOL_CALLS: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct OpenAiCompatibleEndpoint(String);

impl OpenAiCompatibleEndpoint {
    /// Creates a normalized OpenAI-compatible base URL.
    ///
    /// Public endpoints require HTTPS. Plain HTTP is accepted only for literal loopback hosts.
    /// Query strings, fragments, user information, and non-root paths are rejected.
    ///
    /// # Errors
    ///
    /// Returns an error when the URL violates the Worker network policy.
    pub fn new(value: impl AsRef<str>) -> Result<Self, ProviderError> {
        let url = Url::parse(value.as_ref()).map_err(|_| invalid_endpoint())?;
        if url.cannot_be_a_base()
            || url.query().is_some()
            || url.fragment().is_some()
            || !url.username().is_empty()
            || url.password().is_some()
            || (url.path() != "/" && !url.path().is_empty())
        {
            return Err(invalid_endpoint());
        }
        let host = url.host_str().ok_or_else(invalid_endpoint)?;
        let loopback = host.eq_ignore_ascii_case("localhost")
            || host
                .parse::<std::net::IpAddr>()
                .is_ok_and(|address| address.is_loopback());
        if url.scheme() != "https" && !(url.scheme() == "http" && loopback) {
            return Err(invalid_endpoint());
        }
        let mut normalized = url;
        normalized.set_path("");
        Ok(Self(
            normalized.to_string().trim_end_matches('/').to_owned(),
        ))
    }

    fn join(&self, path: &str) -> Result<Url, ProviderError> {
        Url::parse(&format!("{}{path}", self.0)).map_err(|_| invalid_endpoint())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenAiProbe {
    pub models: Vec<OpenAiModel>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpenAiModel {
    pub name: String,
}

pub(crate) fn complete(
    request: &ProviderRequest,
    endpoint: &OpenAiCompatibleEndpoint,
    credential: &WorkerSecret,
    timeout_ms: u64,
) -> Result<ProviderResponse, ProviderError> {
    let client = client(timeout_ms)?;
    let response = client
        .post(endpoint.join("/v1/chat/completions")?)
        .bearer_auth(credential.expose())
        .json(&completion_payload(request))
        .send()
        .map_err(|_| unavailable())?;
    let body = success_body(response)?;
    parse_completion(request, &body)
}

pub(crate) fn probe(
    endpoint: &OpenAiCompatibleEndpoint,
    credential: &WorkerSecret,
    timeout_ms: u64,
) -> Result<OpenAiProbe, ProviderError> {
    let response = client(timeout_ms)?
        .get(endpoint.join("/v1/models")?)
        .bearer_auth(credential.expose())
        .send()
        .map_err(|_| unavailable())?;
    parse_probe(&success_body(response)?)
}

fn client(timeout_ms: u64) -> Result<Client, ProviderError> {
    if timeout_ms == 0 || timeout_ms > 600_000 {
        return Err(stable_error(
            ProviderErrorKind::InvalidRequest,
            "worker execution policy rejected request",
        ));
    }
    Client::builder()
        .timeout(Duration::from_millis(timeout_ms))
        .redirect(Policy::none())
        .user_agent("Nimora-Provider-Worker/1")
        .build()
        .map_err(|_| unavailable())
}

fn success_body(mut response: Response) -> Result<Vec<u8>, ProviderError> {
    if !response.status().is_success() {
        return Err(status_error(response.status()));
    }
    if response
        .content_length()
        .is_some_and(|length| length > MAX_RESPONSE_BYTES)
    {
        return Err(malformed("provider response exceeded limits"));
    }
    let mut body = Vec::new();
    response
        .by_ref()
        .take(MAX_RESPONSE_BYTES + 1)
        .read_to_end(&mut body)
        .map_err(|_| unavailable())?;
    if body.len() as u64 > MAX_RESPONSE_BYTES {
        return Err(malformed("provider response exceeded limits"));
    }
    Ok(body)
}

fn completion_payload(request: &ProviderRequest) -> Value {
    let messages = request.messages.iter().map(|message| {
        let mut value = json!({
            "role": match message.role {
                ProviderMessageRole::System => "system",
                ProviderMessageRole::User => "user",
                ProviderMessageRole::Assistant => "assistant",
                ProviderMessageRole::Tool => "tool",
            },
            "content": message.content,
        });
        if !message.tool_calls.is_empty() {
            value["tool_calls"] = json!(message.tool_calls.iter().map(|call| json!({
                "id": call.id,
                "type": "function",
                "function": {"name": call.tool_id.to_string(), "arguments": call.arguments.to_string()}
            })).collect::<Vec<_>>());
        }
        if let Some(id) = &message.tool_call_id {
            value["tool_call_id"] = json!(id);
        }
        value
    }).collect::<Vec<_>>();
    let tools = request
        .tools
        .iter()
        .map(|tool| {
            json!({
                "type": "function",
                "function": {
                    "name": tool.id.to_string(),
                    "description": tool.description,
                    "parameters": tool.input_schema
                }
            })
        })
        .collect::<Vec<_>>();
    json!({
        "model": request.model,
        "messages": messages,
        "tools": tools,
        "max_tokens": request.max_output_tokens,
        "stream": false
    })
}

#[derive(Deserialize)]
struct ModelCatalog {
    data: Vec<ModelEntry>,
}
#[derive(Deserialize)]
struct ModelEntry {
    id: String,
}

fn parse_probe(body: &[u8]) -> Result<OpenAiProbe, ProviderError> {
    let document: ModelCatalog = serde_json::from_slice(body)
        .map_err(|_| malformed("provider model catalog is malformed"))?;
    if document.data.len() > MAX_MODELS {
        return Err(malformed("provider model catalog exceeded limits"));
    }
    let mut models = document
        .data
        .into_iter()
        .map(|model| {
            if model.id.is_empty()
                || model.id.len() > MAX_MODEL_NAME_BYTES
                || model.id.chars().any(char::is_control)
            {
                return Err(malformed("provider model name is invalid"));
            }
            Ok(OpenAiModel { name: model.id })
        })
        .collect::<Result<Vec<_>, ProviderError>>()?;
    models.sort_by(|left, right| left.name.cmp(&right.name));
    models.dedup_by(|left, right| left.name == right.name);
    Ok(OpenAiProbe { models })
}

#[derive(Deserialize)]
struct CompletionDocument {
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}
#[derive(Deserialize)]
struct Choice {
    message: CompletionMessage,
    finish_reason: String,
}
#[derive(Deserialize)]
struct CompletionMessage {
    #[serde(default)]
    content: Option<String>,
    #[serde(default)]
    tool_calls: Vec<ToolCall>,
}
#[derive(Deserialize)]
struct ToolCall {
    id: String,
    #[serde(rename = "type")]
    kind: String,
    function: FunctionCall,
}
#[derive(Deserialize)]
struct FunctionCall {
    name: String,
    arguments: String,
}
#[derive(Deserialize)]
struct Usage {
    prompt_tokens: u64,
    completion_tokens: u64,
}

fn parse_completion(
    request: &ProviderRequest,
    body: &[u8],
) -> Result<ProviderResponse, ProviderError> {
    let mut document: CompletionDocument = serde_json::from_slice(body)
        .map_err(|_| malformed("provider JSON response is malformed"))?;
    if document.choices.len() != 1 {
        return Err(malformed("provider returned an invalid choice count"));
    }
    let choice = document
        .choices
        .pop()
        .ok_or_else(|| malformed("provider returned no choice"))?;
    if choice.message.tool_calls.len() > MAX_TOOL_CALLS {
        return Err(malformed("provider returned too many tool calls"));
    }
    let tool_calls = choice
        .message
        .tool_calls
        .into_iter()
        .map(|call| {
            if call.kind != "function"
                || call.id.is_empty()
                || call.id.len() > 128
                || call.id.chars().any(char::is_control)
            {
                return Err(malformed("provider returned invalid tool call"));
            }
            let tool_id = ToolId::from_str(&call.function.name)
                .map_err(|_| malformed("provider returned invalid tool ID"))?;
            let arguments: Value = serde_json::from_str(&call.function.arguments)
                .map_err(|_| malformed("provider returned invalid tool arguments"))?;
            if !arguments.is_object() {
                return Err(malformed("provider returned invalid tool arguments"));
            }
            Ok(ProviderToolCall {
                id: call.id,
                tool_id,
                arguments,
            })
        })
        .collect::<Result<Vec<_>, ProviderError>>()?;
    let finish_reason = match choice.finish_reason.as_str() {
        "stop" => ProviderFinishReason::Completed,
        "length" => ProviderFinishReason::Length,
        "tool_calls" if !tool_calls.is_empty() => ProviderFinishReason::ToolCalls,
        "content_filter" => ProviderFinishReason::ContentFiltered,
        _ => return Err(malformed("provider returned invalid finish reason")),
    };
    let usage = document.usage.unwrap_or(Usage {
        prompt_tokens: 0,
        completion_tokens: 0,
    });
    Ok(ProviderResponse {
        spec: "nimora.agent-provider-response/1".to_owned(),
        request_id: request.request_id,
        content: choice.message.content.unwrap_or_default(),
        tool_calls,
        finish_reason,
        usage: ProviderUsage {
            input_tokens: usage.prompt_tokens,
            output_tokens: usage.completion_tokens,
            cost_microunits: 0,
        },
    })
}

fn invalid_endpoint() -> ProviderError {
    stable_error(
        ProviderErrorKind::InvalidRequest,
        "provider endpoint is invalid",
    )
}

fn unavailable() -> ProviderError {
    stable_error(ProviderErrorKind::Unavailable, "provider request failed")
}

fn status_error(status: StatusCode) -> ProviderError {
    let (kind, message) = match status {
        StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN => (
            ProviderErrorKind::Authentication,
            "provider authentication failed",
        ),
        StatusCode::TOO_MANY_REQUESTS => (
            ProviderErrorKind::RateLimited,
            "provider rate limit was reached",
        ),
        StatusCode::REQUEST_TIMEOUT | StatusCode::GATEWAY_TIMEOUT => {
            (ProviderErrorKind::Timeout, "provider request timed out")
        }
        _ => (
            ProviderErrorKind::Unavailable,
            "provider returned a non-success status",
        ),
    };
    stable_error(kind, message)
}

fn malformed(message: &'static str) -> ProviderError {
    stable_error(ProviderErrorKind::MalformedResponse, message)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn endpoint_policy_accepts_https_and_loopback_http() {
        assert!(OpenAiCompatibleEndpoint::new("https://api.example.com").is_ok());
        assert!(OpenAiCompatibleEndpoint::new("http://127.0.0.1:8080").is_ok());
        assert!(OpenAiCompatibleEndpoint::new("http://localhost:8080").is_ok());
    }

    #[test]
    fn endpoint_policy_rejects_unsafe_urls() {
        for value in [
            "http://api.example.com",
            "https://user@example.com",
            "https://api.example.com/path",
            "https://api.example.com?key=value",
            "https://api.example.com#fragment",
        ] {
            assert!(OpenAiCompatibleEndpoint::new(value).is_err(), "{value}");
        }
    }

    #[test]
    fn model_catalog_is_sorted_and_deduplicated() {
        let probe = parse_probe(br#"{"data":[{"id":"zeta"},{"id":"alpha"},{"id":"alpha"}]}"#)
            .expect("catalog");
        assert_eq!(
            probe
                .models
                .iter()
                .map(|model| model.name.as_str())
                .collect::<Vec<_>>(),
            vec!["alpha", "zeta"]
        );
    }

    #[test]
    fn worker_secret_debug_is_redacted() {
        let secret = WorkerSecret::new("do-not-log").expect("secret");
        let debug = format!("{secret:?}");
        assert!(!debug.contains("do-not-log"));
        assert!(debug.contains("REDACTED"));
    }
}
