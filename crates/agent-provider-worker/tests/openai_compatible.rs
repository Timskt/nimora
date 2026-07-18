use nimora_agent_provider_worker::{
    OpenAiCompatibleEndpoint, ProviderWorkerRequest, ProviderWorkerResponse, WorkerSecret, execute,
    probe_openai_worker,
};
use nimora_agent_runtime::{
    DataClassification, ProviderErrorKind, ProviderFinishReason, ProviderMessage,
    ProviderMessageRole, ProviderRequest, ToolDescriptor, ToolEffect,
};
use nimora_runtime_core::CommandRisk;
use serde_json::json;
use std::{
    io::{Read, Write},
    net::{Ipv4Addr, TcpListener},
    path::Path,
    thread,
    time::Duration,
};
use uuid::Uuid;

fn provider_request(tools: Vec<ToolDescriptor>) -> ProviderRequest {
    ProviderRequest::new(
        Uuid::now_v7(),
        Uuid::now_v7(),
        "provider:openai-compatible:test",
        "model-test",
        vec![ProviderMessage::text(
            ProviderMessageRole::User,
            "inspect profile",
            DataClassification::Personal,
            true,
        )],
        tools,
        128,
    )
    .expect("request")
}

fn mock_response(
    status: &str,
    response: serde_json::Value,
) -> (OpenAiCompatibleEndpoint, thread::JoinHandle<Vec<u8>>) {
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).expect("bind mock provider");
    let endpoint = OpenAiCompatibleEndpoint::new(format!(
        "http://127.0.0.1:{}",
        listener.local_addr().expect("address").port()
    ))
    .expect("endpoint");
    let status = status.to_owned();
    let handle = thread::spawn(move || {
        let (mut stream, _) = listener.accept().expect("accept");
        let mut bytes = Vec::new();
        let mut buffer = [0_u8; 1024];
        let header_end = loop {
            let read = stream.read(&mut buffer).expect("read request");
            assert_ne!(read, 0);
            bytes.extend_from_slice(&buffer[..read]);
            if let Some(position) = bytes.windows(4).position(|part| part == b"\r\n\r\n") {
                break position;
            }
        };
        let headers = std::str::from_utf8(&bytes[..header_end]).expect("headers");
        let content_length = headers
            .lines()
            .find_map(|line| {
                let (name, value) = line.split_once(':')?;
                name.eq_ignore_ascii_case("content-length")
                    .then(|| value.trim().parse::<usize>().expect("content length"))
            })
            .unwrap_or(0);
        while bytes.len() < header_end + 4 + content_length {
            let read = stream.read(&mut buffer).expect("read body");
            assert_ne!(read, 0);
            bytes.extend_from_slice(&buffer[..read]);
        }
        let body = serde_json::to_vec(&response).expect("response JSON");
        write!(
            stream,
            "HTTP/1.1 {status}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
            body.len()
        )
        .expect("write headers");
        stream.write_all(&body).expect("write body");
        bytes
    });
    (endpoint, handle)
}

#[test]
fn probes_through_real_worker_with_bearer_auth() {
    let (endpoint, server) = mock_response(
        "200 OK",
        json!({"data":[{"id":"zeta"},{"id":"alpha"},{"id":"alpha"}]}),
    );
    let probe = probe_openai_worker(
        Path::new(env!("CARGO_BIN_EXE_nimora-agent-provider-worker")),
        endpoint,
        WorkerSecret::new("test-secret").expect("secret"),
        Duration::from_secs(2),
    )
    .expect("probe");
    assert_eq!(
        probe
            .models
            .iter()
            .map(|model| model.name.as_str())
            .collect::<Vec<_>>(),
        vec!["alpha", "zeta"]
    );
    let request = server.join().expect("server");
    let headers = String::from_utf8_lossy(&request);
    assert!(headers.starts_with("GET /v1/models HTTP/1.1\r\n"));
    assert!(
        headers
            .to_ascii_lowercase()
            .contains("authorization: bearer test-secret")
    );
}

#[test]
fn parses_text_usage_and_tool_calls() {
    let tool = ToolDescriptor::new(
        "profile.appearance.inspect",
        "Inspect appearance",
        "Inspect the active profile.",
        json!({"type":"object"}),
        json!({"type":"object"}),
        CommandRisk::Safe,
        ToolEffect::ReadOnly,
    )
    .expect("tool");
    let (endpoint, server) = mock_response(
        "200 OK",
        json!({
            "choices":[{"message":{"content":null,"tool_calls":[{
                "id":"call-safe-1","type":"function","function":{
                    "name":"profile.appearance.inspect","arguments":"{\"profileRef\":\"profile:active\"}"
                }
            }]},"finish_reason":"tool_calls"}],
            "usage":{"prompt_tokens":11,"completion_tokens":7}
        }),
    );
    let response = execute(ProviderWorkerRequest::OpenAiComplete {
        request: provider_request(vec![tool]),
        endpoint,
        credential: WorkerSecret::new("test-secret").expect("secret"),
        timeout_ms: 2_000,
    });
    let ProviderWorkerResponse::Completed { response } = response else {
        panic!("expected completion");
    };
    assert_eq!(response.finish_reason, ProviderFinishReason::ToolCalls);
    assert_eq!(response.tool_calls[0].id, "call-safe-1");
    assert_eq!(
        response.tool_calls[0].arguments["profileRef"],
        "profile:active"
    );
    assert_eq!(response.usage.input_tokens, 11);
    assert_eq!(response.usage.output_tokens, 7);
    let request = server.join().expect("server");
    let body_start = request
        .windows(4)
        .position(|part| part == b"\r\n\r\n")
        .expect("body")
        + 4;
    let document: serde_json::Value = serde_json::from_slice(&request[body_start..]).expect("JSON");
    assert_eq!(document["stream"], false);
    assert_eq!(document["messages"][0]["content"], "inspect profile");
}

#[test]
fn rejects_redirect_without_leaking_secret_or_body() {
    let (endpoint, server) = mock_response("302 Found", json!({"secret":"response-secret"}));
    let response = execute(ProviderWorkerRequest::OpenAiProbe {
        endpoint,
        credential: WorkerSecret::new("request-secret").expect("secret"),
        timeout_ms: 2_000,
    });
    let ProviderWorkerResponse::Error { error } = response else {
        panic!("expected error");
    };
    assert_eq!(error.kind, ProviderErrorKind::Unavailable);
    assert!(!error.message.contains("request-secret"));
    assert!(!error.message.contains("response-secret"));
    server.join().expect("server");
}
