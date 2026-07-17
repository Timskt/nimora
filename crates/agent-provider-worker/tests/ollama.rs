use nimora_agent_provider_worker::{
    OllamaEndpoint, ProviderWorkerRequest, ProviderWorkerResponse, WorkerOllamaProvider, execute,
    probe_ollama_worker,
};
use nimora_agent_runtime::{
    CancellationFlag, DataClassification, ProviderExecutionContext, ProviderFinishReason,
    ProviderMessage, ProviderMessageRole, ProviderRegistry, ProviderRequest, ProviderResponse,
    ProviderToolCall, ProviderUsage, ToolDescriptor, ToolEffect,
};
use nimora_runtime_core::CommandRisk;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::{IpAddr, Ipv4Addr, TcpListener},
    thread,
    time::Duration,
};
use uuid::Uuid;

fn request(tools: Vec<ToolDescriptor>) -> ProviderRequest {
    ProviderRequest::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        "provider:ollama-loopback",
        "qwen3:8b",
        vec![ProviderMessage::text(
            ProviderMessageRole::User,
            "inspect my profile",
            DataClassification::Personal,
            true,
        )],
        tools,
        128,
    )
    .expect("request")
}

fn mock_ollama_tags(response: serde_json::Value) -> (OllamaEndpoint, thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind mock Ollama");
    let endpoint = OllamaEndpoint::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        listener.local_addr().expect("address").port(),
    )
    .expect("endpoint");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request_bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        loop {
            let read = stream.read(&mut buffer).expect("read request");
            assert_ne!(read, 0);
            request_bytes.extend_from_slice(&buffer[..read]);
            if request_bytes.windows(4).any(|part| part == b"\r\n\r\n") {
                break;
            }
        }
        assert!(request_bytes.starts_with(b"GET /api/tags HTTP/1.1\r\n"));
        let response_body = serde_json::to_vec(&response).expect("response JSON");
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response_body.len()
        )
        .expect("write headers");
        stream.write_all(&response_body).expect("write body");
        request_bytes
    });
    (endpoint, handle)
}

fn mock_ollama(response: serde_json::Value) -> (OllamaEndpoint, thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind mock Ollama");
    let endpoint = OllamaEndpoint::new(
        IpAddr::V4(Ipv4Addr::LOCALHOST),
        listener.local_addr().expect("address").port(),
    )
    .expect("endpoint");
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut request_bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        let header_end = loop {
            let read = stream.read(&mut buffer).expect("read request");
            assert_ne!(read, 0);
            request_bytes.extend_from_slice(&buffer[..read]);
            if let Some(position) = request_bytes
                .windows(4)
                .position(|part| part == b"\r\n\r\n")
            {
                break position;
            }
        };
        let headers = std::str::from_utf8(&request_bytes[..header_end]).expect("headers");
        let content_length = headers
            .lines()
            .find_map(|line| line.strip_prefix("Content-Length: "))
            .expect("content length")
            .parse::<usize>()
            .expect("content length number");
        while request_bytes.len() < header_end + 4 + content_length {
            let read = stream.read(&mut buffer).expect("read body");
            assert_ne!(read, 0);
            request_bytes.extend_from_slice(&buffer[..read]);
        }
        let body = &request_bytes[header_end + 4..header_end + 4 + content_length];
        let document: serde_json::Value = serde_json::from_slice(body).expect("request JSON");
        assert_eq!(document["stream"], false);
        assert_eq!(document["messages"][0]["content"], "inspect my profile");
        let response_body = serde_json::to_vec(&response).expect("response JSON");
        write!(
            stream,
            "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            response_body.len()
        )
        .expect("write headers");
        stream.write_all(&response_body).expect("write body");
        request_bytes
    });
    (endpoint, handle)
}

fn request_document(request_bytes: &[u8]) -> serde_json::Value {
    let header_end = request_bytes
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .expect("header boundary");
    serde_json::from_slice(&request_bytes[header_end + 4..]).expect("request JSON")
}

#[test]
fn completes_against_real_loopback_http_and_accepts_ollama_extensions() {
    let (endpoint, server) = mock_ollama(json!({
        "model": "qwen3:8b",
        "created_at": "2026-07-17T00:00:00Z",
        "message": {"role": "assistant", "content": "profile inspected"},
        "done": true,
        "done_reason": "stop",
        "prompt_eval_count": 11,
        "eval_count": 7,
        "total_duration": 1234
    }));
    let provider_request = request(Vec::new());
    let response = execute(ProviderWorkerRequest::Complete {
        request: provider_request.clone(),
        endpoint,
        timeout_ms: 2_000,
    });
    let ProviderWorkerResponse::Completed { response } = response else {
        panic!("expected completion");
    };
    assert_eq!(response.request_id, provider_request.request_id);
    assert_eq!(response.content, "profile inspected");
    assert_eq!(response.usage.input_tokens, 11);
    assert_eq!(response.usage.output_tokens, 7);
    server.join().expect("mock server");
}

#[test]
fn converts_ollama_function_calls_to_runtime_tool_calls() {
    let tool = ToolDescriptor::new(
        "profile.appearance.inspect",
        "Inspect appearance",
        "Reads the current appearance through the capability gateway.",
        json!({"type": "object"}),
        json!({"type": "object"}),
        CommandRisk::Safe,
        ToolEffect::ReadOnly,
    )
    .expect("tool");
    let (endpoint, server) = mock_ollama(json!({
        "message": {
            "role": "assistant",
            "content": "",
            "tool_calls": [{"function": {
                "name": "profile.appearance.inspect",
                "arguments": {"profileRef": "profile:active"}
            }}]
        },
        "done": true,
        "prompt_eval_count": 5,
        "eval_count": 3
    }));
    let response = execute(ProviderWorkerRequest::Complete {
        request: request(vec![tool]),
        endpoint,
        timeout_ms: 2_000,
    });
    let ProviderWorkerResponse::Completed { response } = response else {
        panic!("expected completion");
    };
    assert_eq!(response.tool_calls.len(), 1);
    assert_eq!(
        response.tool_calls[0].tool_id.to_string(),
        "profile.appearance.inspect"
    );
    assert_eq!(
        response.tool_calls[0].arguments["profileRef"],
        "profile:active"
    );
    server.join().expect("mock server");
}

#[test]
fn sends_correlated_assistant_calls_and_tool_results_on_continuation() {
    let tool = ToolDescriptor::new(
        "profile.appearance.inspect",
        "Inspect appearance",
        "Reads the current appearance through the capability gateway.",
        json!({"type": "object"}),
        json!({"type": "object"}),
        CommandRisk::Safe,
        ToolEffect::ReadOnly,
    )
    .expect("tool");
    let call = ProviderToolCall {
        id: "ollama:0".to_owned(),
        tool_id: tool.id.clone(),
        arguments: json!({"profileRef": "profile:active"}),
    };
    let first_response = ProviderResponse {
        spec: "nimora.agent-provider-response/1".to_owned(),
        request_id: Uuid::now_v7(),
        content: String::new(),
        tool_calls: vec![call.clone()],
        finish_reason: ProviderFinishReason::ToolCalls,
        usage: ProviderUsage {
            input_tokens: 5,
            output_tokens: 3,
            cost_microunits: 0,
        },
    };
    let messages = vec![
        ProviderMessage::text(
            ProviderMessageRole::User,
            "inspect my profile",
            DataClassification::Personal,
            true,
        ),
        ProviderMessage::assistant_tool_calls(&first_response),
        ProviderMessage::tool_result(&call, &json!({"theme": "violet"})).expect("tool result"),
    ];
    let provider_request = ProviderRequest::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        "provider:ollama-loopback",
        "qwen3:8b",
        messages,
        vec![tool],
        128,
    )
    .expect("continuation request");
    let (endpoint, server) = mock_ollama(json!({
        "message": {"role": "assistant", "content": "profile inspected"},
        "done": true,
        "done_reason": "stop"
    }));
    let response = execute(ProviderWorkerRequest::Complete {
        request: provider_request,
        endpoint,
        timeout_ms: 2_000,
    });
    assert!(matches!(response, ProviderWorkerResponse::Completed { .. }));
    let document = request_document(&server.join().expect("mock server"));
    assert_eq!(
        document["messages"][1]["tool_calls"][0]["function"]["name"],
        "profile.appearance.inspect"
    );
    assert_eq!(document["messages"][2]["tool_call_id"], "ollama:0");
    assert_eq!(
        document["messages"][2]["tool_name"],
        "profile.appearance.inspect"
    );
}

#[test]
fn rejects_non_loopback_target_even_when_protocol_is_deserialized_directly() {
    let response = execute(ProviderWorkerRequest::Complete {
        request: request(Vec::new()),
        endpoint: OllamaEndpoint {
            address: "192.0.2.1".parse().expect("address"),
            port: 11_434,
        },
        timeout_ms: 100,
    });
    let ProviderWorkerResponse::Error { error } = response else {
        panic!("expected policy error");
    };
    assert_eq!(
        error.kind,
        nimora_agent_runtime::ProviderErrorKind::InvalidRequest
    );
}

#[test]
fn registry_completes_through_the_real_worker_process() {
    let (endpoint, server) = mock_ollama(json!({
        "message": {"role": "assistant", "content": "sidecar verified"},
        "done": true,
        "prompt_eval_count": 9,
        "eval_count": 4
    }));
    let mut registry = ProviderRegistry::default();
    registry
        .register(
            WorkerOllamaProvider::new(env!("CARGO_BIN_EXE_nimora-agent-provider-worker"), endpoint)
                .expect("worker provider"),
        )
        .expect("register provider");
    let provider_request = request(Vec::new());
    let response = registry
        .complete(
            &provider_request,
            ProviderExecutionContext {
                timeout: Duration::from_secs(2),
                cancellation: CancellationFlag::default(),
                credential_reference: None,
            },
            true,
        )
        .expect("worker completion");
    assert_eq!(response.content, "sidecar verified");
    assert_eq!(response.request_id, provider_request.request_id);
    server.join().expect("mock server");
}

#[test]
fn probes_sorted_deduplicated_models_through_real_worker_process() {
    let (endpoint, server) = mock_ollama_tags(json!({
        "models": [
            {"name": "zeta:latest", "size": 20, "modified_at": "2026-07-18T00:00:00Z"},
            {"name": "alpha:latest", "size": 10},
            {"name": "alpha:latest", "size": 11}
        ]
    }));
    let probe = probe_ollama_worker(
        std::path::Path::new(env!("CARGO_BIN_EXE_nimora-agent-provider-worker")),
        endpoint,
        Duration::from_secs(2),
    )
    .expect("worker probe");
    assert_eq!(probe.models.len(), 2);
    assert_eq!(probe.models[0].name, "alpha:latest");
    assert_eq!(probe.models[1].name, "zeta:latest");
    server.join().expect("mock server");
}

#[test]
fn probe_rejects_non_loopback_target() {
    let response = execute(ProviderWorkerRequest::Probe {
        endpoint: OllamaEndpoint {
            address: "192.0.2.1".parse().expect("address"),
            port: 11_434,
        },
        timeout_ms: 100,
    });
    assert!(matches!(response, ProviderWorkerResponse::Error { .. }));
}
