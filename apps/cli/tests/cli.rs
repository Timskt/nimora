use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Stdio},
    time::{SystemTime, UNIX_EPOCH},
};

fn nimora() -> Command {
    Command::new(env!("CARGO_BIN_EXE_nimora"))
}

fn temporary_database(name: &str) -> PathBuf {
    let unique = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("clock")
        .as_nanos();
    std::env::temp_dir().join(format!("nimora-cli-{name}-{unique}.sqlite3"))
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

#[test]
fn tool_catalog_exposes_gateway_backed_module_capabilities() {
    let output = nimora()
        .args(["ai", "tool", "list"])
        .output()
        .expect("run nimora");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let document: Value = serde_json::from_slice(&output.stdout).expect("json output");
    let tool_ids = document["tools"]
        .as_array()
        .expect("tools")
        .iter()
        .map(|tool| tool["id"].as_str().expect("tool id"))
        .collect::<Vec<_>>();
    assert_eq!(
        tool_ids,
        vec![
            "asset.catalog.read",
            "automation.definition.validate",
            "character.active.switch",
            "character.state.read",
            "pet.action.catalog.read",
            "pet.animation.play",
            "pet.position.move",
            "pet.state.read",
            "profile.active.switch",
            "profile.state.read",
            "program.catalog.read",
            "program.installed.execute",
            "runtime.health.read"
        ]
    );

    let described = nimora()
        .args(["ai", "tool", "describe", "pet.position.move"])
        .output()
        .expect("describe tool");
    assert!(described.status.success());
    let description: Value = serde_json::from_slice(&described.stdout).expect("json output");
    assert_eq!(description["tool"]["effect"], "reversible_write");
    assert_eq!(description["tool"]["baseRisk"], "low");
    assert_eq!(
        description["tool"]["inputSchema"]["additionalProperties"],
        false
    );
}

#[test]
fn ollama_run_requires_verified_sidecar() {
    let mut child = nimora()
        .args(["ai", "run", "--input", "-", "--output", "json"])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nimora");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(br#"{"prompt":"hello","providerId":"provider:ollama-loopback"}"#)
        .expect("write input");
    let output = child.wait_with_output().expect("wait for nimora");
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    let document: Value = serde_json::from_slice(&output.stderr).expect("json error");
    assert_eq!(document["error"], "sidecar-required");
}

#[test]
fn sidecar_arguments_must_be_provided_together() {
    let output = nimora()
        .args([
            "ai",
            "run",
            "--input",
            "-",
            "--output",
            "json",
            "--sidecar-root",
            ".",
        ])
        .output()
        .expect("run nimora");
    assert_eq!(output.status.code(), Some(2));
    assert!(output.stdout.is_empty());
    let document: Value = serde_json::from_slice(&output.stderr).expect("json error");
    assert_eq!(document["error"], "usage");
}

#[test]
fn invalid_trusted_sidecar_digest_fails_closed() {
    let mut child = nimora()
        .args([
            "ai",
            "run",
            "--input",
            "-",
            "--output",
            "json",
            "--sidecar-root",
            ".",
            "--sidecar-manifest-sha256",
            "invalid",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nimora");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(br#"{"prompt":"hello","providerId":"provider:ollama-loopback"}"#)
        .expect("write input");
    let output = child.wait_with_output().expect("wait for nimora");
    assert_eq!(output.status.code(), Some(4));
    assert!(output.stdout.is_empty());
    let document: Value = serde_json::from_slice(&output.stderr).expect("json error");
    assert_eq!(document["error"], "sidecar-integrity");
}

#[test]
fn completed_run_persists_and_exports_history() {
    let database = temporary_database("history-export");
    let database_path = database.to_str().expect("database path");
    let mut child = nimora()
        .args([
            "ai",
            "run",
            "--input",
            "-",
            "--output",
            "json",
            "--offline",
            "--history-database",
            database_path,
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .expect("spawn nimora");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(br#"{"prompt":"persist me"}"#)
        .expect("write input");
    let output = child.wait_with_output().expect("wait for nimora");
    assert!(output.status.success());
    let run: Value = serde_json::from_slice(&output.stdout).expect("run output");
    assert_eq!(run["history"]["persisted"], true);
    assert_eq!(run["history"]["degraded"], false);

    let output = nimora()
        .args([
            "ai",
            "history",
            "export",
            "--database",
            database_path,
            "--limit",
            "1",
        ])
        .output()
        .expect("export history");
    assert!(output.status.success());
    assert!(output.stderr.is_empty());
    let exported: Value = serde_json::from_slice(&output.stdout).expect("history output");
    assert_eq!(exported["spec"], "nimora.ai-history-export/1");
    assert_eq!(exported["records"][0]["task"]["id"], run["task"]["id"]);
    assert_eq!(exported["records"][0]["prompt"], "persist me");
    assert_eq!(exported["records"][0]["response"], "persist me");
    assert_eq!(
        exported["records"][0]["task"]["providerId"],
        "provider:deterministic-local"
    );

    let _ = fs::remove_file(database);
}

#[test]
fn history_delete_supports_task_and_all_without_touching_run_results() {
    let database = temporary_database("history-delete");
    let database_path = database.to_str().expect("database path");
    let mut task_ids = Vec::new();
    for prompt in ["first", "second"] {
        let input = format!(r#"{{"prompt":"{prompt}"}}"#);
        let mut child = nimora()
            .args([
                "ai",
                "run",
                "--input",
                "-",
                "--output",
                "json",
                "--offline",
                "--history-database",
                database_path,
            ])
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .expect("spawn nimora");
        child
            .stdin
            .take()
            .expect("stdin")
            .write_all(input.as_bytes())
            .expect("write input");
        let output = child.wait_with_output().expect("wait for nimora");
        assert!(output.status.success());
        let document: Value = serde_json::from_slice(&output.stdout).expect("run output");
        task_ids.push(document["task"]["id"].as_str().expect("task ID").to_owned());
    }

    let deleted = nimora()
        .args([
            "ai",
            "history",
            "delete",
            "--database",
            database_path,
            "--task-id",
            &task_ids[0],
        ])
        .output()
        .expect("delete task history");
    assert!(deleted.status.success());
    let document: Value = serde_json::from_slice(&deleted.stdout).expect("delete output");
    assert_eq!(document["deleted"], 1);

    let deleted = nimora()
        .args([
            "ai",
            "history",
            "delete",
            "--database",
            database_path,
            "--all",
        ])
        .output()
        .expect("delete all history");
    assert!(deleted.status.success());
    let document: Value = serde_json::from_slice(&deleted.stdout).expect("delete output");
    assert_eq!(document["deleted"], 1);
    let _ = fs::remove_file(database);
}

#[test]
fn history_cursor_must_be_paired_and_run_storage_failure_is_degraded() {
    let database = temporary_database("history-cursor");
    let output = nimora()
        .args([
            "ai",
            "history",
            "export",
            "--database",
            database.to_str().expect("database path"),
            "--before-created-at-ms",
            "42",
        ])
        .output()
        .expect("export history");
    assert_eq!(output.status.code(), Some(2));
    let error: Value = serde_json::from_slice(&output.stderr).expect("error output");
    assert_eq!(error["error"], "usage");

    let unavailable = database.join("missing-parent").join("history.sqlite3");
    let mut child = nimora()
        .args([
            "ai",
            "run",
            "--input",
            "-",
            "--output",
            "json",
            "--offline",
            "--history-database",
            unavailable.to_str().expect("database path"),
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn nimora");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(br#"{"prompt":"still succeeds"}"#)
        .expect("write input");
    let output = child.wait_with_output().expect("wait for nimora");
    assert!(output.status.success());
    let result: Value = serde_json::from_slice(&output.stdout).expect("run output");
    assert_eq!(result["task"]["status"], "succeeded");
    assert_eq!(result["history"]["persisted"], false);
    assert_eq!(result["history"]["degraded"], true);
    let _ = fs::remove_file(database);
}
