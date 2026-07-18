use serde_json::Value;
use std::{
    fs,
    io::Write,
    path::PathBuf,
    process::{Command, Output, Stdio},
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

fn nimora_with_stdin(arguments: &[&str], input: &[u8]) -> Output {
    let mut child = nimora()
        .args(arguments)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn nimora");
    child
        .stdin
        .take()
        .expect("stdin")
        .write_all(input)
        .expect("write stdin");
    child.wait_with_output().expect("wait for nimora")
}

fn create_auto_mode_session(database_path: &str) -> (String, String) {
    let created = nimora_with_stdin(
        &[
            "ai",
            "goal",
            "create",
            "--database",
            database_path,
            "--input",
            "-",
        ],
        br#"{"title":"Supervise","objective":"Run bounded work","steps":["Inspect state"]}"#,
    );
    assert!(created.status.success());
    let created: Value = serde_json::from_slice(&created.stdout).expect("Goal output");
    let goal_id = created["goal"]["id"].as_str().expect("Goal ID");
    let started = nimora_with_stdin(
        &[
            "ai",
            "goal",
            "auto",
            "start",
            "--database",
            database_path,
            "--goal-id",
            goal_id,
            "--input",
            "-",
        ],
        br#"{"maxCycles":4,"maxConcurrency":1,"budget":{"maxSteps":4,"maxToolCalls":2,"maxElapsedMs":10000,"maxInputTokens":1000,"maxOutputTokens":500,"maxCostMicrounits":0},"maximumDataClassification":"personal","toolAllowlist":["pet.state.read"],"workspaceRevision":"git:abc"}"#,
    );
    assert!(started.status.success());
    let started: Value = serde_json::from_slice(&started.stdout).expect("start output");
    assert_eq!(started["session"]["status"], "running");
    (
        goal_id.to_owned(),
        started["session"]["id"]
            .as_str()
            .expect("session ID")
            .to_owned(),
    )
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

#[test]
fn goal_cli_persists_revises_and_requires_completion_evidence() {
    let database = temporary_database("goals");
    let database_path = database.to_str().expect("database path");
    let created = nimora_with_stdin(
        &[
            "ai",
            "goal",
            "create",
            "--database",
            database_path,
            "--input",
            "-",
        ],
        br#"{"title":"Ship Goal mode","objective":"Persist and resume a Goal","steps":["Implement store"]}"#,
    );
    assert!(created.status.success());
    let created: Value = serde_json::from_slice(&created.stdout).expect("create output");
    let goal_id = created["goal"]["id"].as_str().expect("Goal ID");
    let step_id = created["currentPlan"]["steps"][0]["id"]
        .as_str()
        .expect("step ID");

    let incomplete = nimora()
        .args([
            "ai",
            "goal",
            "status",
            "set",
            "--database",
            database_path,
            "--goal-id",
            goal_id,
            "--status",
            "completed",
        ])
        .output()
        .expect("attempt incomplete Goal");
    assert_eq!(incomplete.status.code(), Some(3));
    assert!(incomplete.stdout.is_empty());

    let plan = format!(
        r#"{{"rationale":"Implementation verified","steps":[{{"id":"{step_id}","text":"Implement store","status":"completed","evidence":["cargo test passed"]}}]}}"#
    );
    let revised = nimora_with_stdin(
        &[
            "ai",
            "goal",
            "plan",
            "replace",
            "--database",
            database_path,
            "--goal-id",
            goal_id,
            "--input",
            "-",
        ],
        plan.as_bytes(),
    );
    assert!(revised.status.success());
    let revised: Value = serde_json::from_slice(&revised.stdout).expect("revision output");
    assert_eq!(revised["currentPlan"]["revision"], 2);

    let completed = nimora()
        .args([
            "ai",
            "goal",
            "status",
            "set",
            "--database",
            database_path,
            "--goal-id",
            goal_id,
            "--status",
            "completed",
        ])
        .output()
        .expect("complete Goal");
    assert!(completed.status.success());
    let completed: Value = serde_json::from_slice(&completed.stdout).expect("completion output");
    assert_eq!(completed["goal"]["status"], "completed");

    let listed = nimora()
        .args(["ai", "goal", "list", "--database", database_path])
        .output()
        .expect("list Goals");
    assert!(listed.status.success());
    let listed: Value = serde_json::from_slice(&listed.stdout).expect("list output");
    assert_eq!(listed["goals"][0]["id"], goal_id);
    assert_eq!(listed["goals"][0]["currentPlanRevision"], 2);
    let _ = fs::remove_file(database);
}

#[test]
fn auto_mode_cli_persists_and_revalidates_resume_bindings() {
    let database = temporary_database("auto-mode");
    let database_path = database.to_str().expect("database path");
    let (_, session_id) = create_auto_mode_session(database_path);

    let paused = nimora()
        .args([
            "ai",
            "goal",
            "auto",
            "pause",
            "--database",
            database_path,
            "--session-id",
            &session_id,
        ])
        .output()
        .expect("pause session");
    assert!(paused.status.success());

    let rejected = nimora()
        .args([
            "ai",
            "goal",
            "auto",
            "resume",
            "--database",
            database_path,
            "--session-id",
            &session_id,
            "--workspace-revision",
            "git:changed",
        ])
        .output()
        .expect("reject changed workspace");
    assert_eq!(rejected.status.code(), Some(3));
    assert!(rejected.stdout.is_empty());
    let error: Value = serde_json::from_slice(&rejected.stderr).expect("JSON error");
    assert_eq!(error["error"], "auto-mode-input");

    let resumed = nimora()
        .args([
            "ai",
            "goal",
            "auto",
            "resume",
            "--database",
            database_path,
            "--session-id",
            &session_id,
            "--workspace-revision",
            "git:abc",
        ])
        .output()
        .expect("resume session");
    assert!(resumed.status.success());
    let resumed: Value = serde_json::from_slice(&resumed.stdout).expect("resume output");
    assert_eq!(resumed["session"]["status"], "running");

    let cancelled = nimora()
        .args([
            "ai",
            "goal",
            "auto",
            "cancel",
            "--database",
            database_path,
            "--session-id",
            &session_id,
        ])
        .output()
        .expect("cancel session");
    assert!(cancelled.status.success());
    let cancelled: Value = serde_json::from_slice(&cancelled.stdout).expect("cancel output");
    assert_eq!(cancelled["session"]["status"], "cancelled");
    let _ = fs::remove_file(database);
}
