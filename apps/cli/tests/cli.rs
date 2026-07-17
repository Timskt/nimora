use serde_json::Value;
use std::{
    io::Write,
    process::{Command, Stdio},
};

fn nimora() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nimora"))
}

#[test]
fn provider_list_is_machine_readable_and_credential_free() {
    let output = nimora()
        .args(["ai", "provider", "list"])
        .output()
        .expect("run nimora");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let document: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(document["spec"], "nimora.ai-provider-list/1");
    assert_eq!(
        document["providers"][0]["id"],
        "provider:deterministic-local"
    );
    assert!(document.to_string().find("credential").is_none());
}

#[test]
fn offline_run_accepts_bounded_stdin_and_uses_agent_runtime() {
    let mut child = nimora()
        .args(["ai", "run", "--input", "-", "--output", "json", "--offline"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nimora");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(br#"{"prompt":"hello offline"}"#)
        .expect("write input");
    let output = child.wait_with_output().expect("wait for nimora");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let document: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(document["spec"], "nimora.ai-run-result/1");
    assert_eq!(document["content"], "hello offline");
    assert_eq!(document["task"]["origin"], "cli");
    assert_eq!(document["task"]["status"], "succeeded");
    assert_eq!(document["usage"]["costMicrounits"], 0);
}

#[test]
fn invalid_command_has_stable_exit_and_keeps_stdout_empty() {
    let output = nimora()
        .args(["ai", "unknown"])
        .output()
        .expect("run nimora");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let document: Value = serde_json::from_slice(&output.stderr).expect("json error");
    assert_eq!(document["spec"], "nimora.cli-error/1");
    assert_eq!(document["error"], "usage");
}

#[test]
fn run_requires_explicit_machine_output_mode() {
    let output = nimora()
        .args(["ai", "run", "--input", "missing.json"])
        .output()
        .expect("run nimora");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let document: Value = serde_json::from_slice(&output.stderr).expect("json error");
    assert_eq!(document["error"], "usage");
}

#[test]
fn provider_probe_executes_a_real_local_request() {
    let output = nimora()
        .args(["ai", "provider", "probe"])
        .output()
        .expect("run nimora");
    assert!(output.status.success());
    let document: Value = serde_json::from_slice(&output.stdout).expect("json output");
    assert_eq!(document["healthy"], true);
    assert_eq!(document["providerId"], "provider:deterministic-local");
}
