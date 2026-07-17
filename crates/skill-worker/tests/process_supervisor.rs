use nimora_skill_host::{
    ExecutionCancellation, SKILL_WORKER_PROTOCOL_VERSION, SkillExecutionOutput, SkillHostError,
    SkillWorkerConfig, SkillWorkerMessage, SkillWorkerProcess,
};
use nimora_skill_runtime::{
    SkillCapability, SkillContributions, SkillGrant, SkillHost, SkillManifest, SkillStatus,
    validate_manifest,
};
use std::collections::BTreeSet;
use std::time::Duration;

fn manifest() -> SkillManifest {
    SkillManifest {
        spec: "nimora.skill/1".to_owned(),
        id: "studio.example.focus".to_owned(),
        version: "1.0.0".to_owned(),
        publisher: "studio.example".to_owned(),
        entrypoint: "dist/main.js".to_owned(),
        capabilities: BTreeSet::from([SkillCapability::InvokeCommands]),
        activation_events: BTreeSet::from(["onStartup".to_owned()]),
        contributions: SkillContributions::default(),
    }
}

fn config(timeout: Duration) -> SkillWorkerConfig {
    SkillWorkerConfig {
        executable: env!("CARGO_BIN_EXE_nimora-skill-worker").to_owned(),
        args: Vec::new(),
        execution_id: "skill-run-1".to_owned(),
        timeout,
        output_bytes: 1024 * 1024,
        cancellation: None,
    }
}

fn request(source: &str) -> SkillWorkerMessage {
    SkillWorkerMessage::Run {
        protocol_version: SKILL_WORKER_PROTOCOL_VERSION,
        execution_id: "skill-run-1".to_owned(),
        manifest: Box::new(manifest()),
        source: source.to_owned(),
        activation_event: "onStartup".to_owned(),
        input: serde_json::json!({"reason": "test"}),
    }
}

fn active_lifecycle() -> SkillHost {
    let skill_manifest = manifest();
    let mut lifecycle = SkillHost::default();
    lifecycle
        .install(validate_manifest(skill_manifest.clone()).unwrap())
        .unwrap();
    lifecycle
        .authorize(SkillGrant {
            skill_id: skill_manifest.id.clone(),
            version: skill_manifest.version.clone(),
            capabilities: skill_manifest.capabilities,
        })
        .unwrap();
    lifecycle.activate(&skill_manifest.id).unwrap();
    lifecycle
}

#[test]
fn supervisor_runs_a_real_isolated_skill_worker() {
    let lifecycle = active_lifecycle();
    let mut worker = SkillWorkerProcess::spawn(
        config(Duration::from_secs(2)),
        &request("nimora.commands.invoke('runtime.pet.action', { action: 'wave' });"),
        &lifecycle,
    )
    .unwrap();
    let SkillWorkerMessage::Completed { output, .. } = worker.wait().unwrap() else {
        panic!("expected completed response");
    };
    assert_eq!(output.commands.len(), 1);
}

#[test]
fn supervisor_terminates_an_infinite_skill() {
    let skill_manifest = manifest();
    let mut lifecycle = active_lifecycle();
    let mut worker = SkillWorkerProcess::spawn(
        config(Duration::from_millis(100)),
        &request("while (true) {}"),
        &lifecycle,
    )
    .unwrap();
    assert_eq!(
        worker.wait_recording_failure(&mut lifecycle, &skill_manifest.id, 1_000),
        Err(SkillHostError::TimedOut)
    );
    assert_eq!(
        lifecycle.status(&skill_manifest.id),
        Some(SkillStatus::Crashed)
    );
    assert!(lifecycle.active_contributions().is_empty());
}

#[test]
fn supervisor_honors_cross_thread_cancellation() {
    let lifecycle = active_lifecycle();
    let cancellation = ExecutionCancellation::default();
    let mut worker_config = config(Duration::from_secs(5));
    worker_config.cancellation = Some(cancellation.clone());
    let worker = SkillWorkerProcess::spawn(worker_config, &request("while (true) {}"), &lifecycle);
    let mut worker = worker.unwrap();
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        cancellation.cancel();
    });
    assert_eq!(worker.wait(), Err(SkillHostError::Cancelled));
}

#[test]
fn host_rejects_undeclared_activation_before_process_start() {
    let lifecycle = active_lifecycle();
    let mut invalid = request("");
    let SkillWorkerMessage::Run {
        activation_event, ..
    } = &mut invalid
    else {
        unreachable!();
    };
    *activation_event = "onEvent:runtime.pet.changed".to_owned();
    assert!(matches!(
        SkillWorkerProcess::spawn(config(Duration::from_secs(1)), &invalid, &lifecycle),
        Err(SkillHostError::Admission(_))
    ));
}

#[test]
fn empty_skill_returns_an_empty_plan() {
    let lifecycle = active_lifecycle();
    let mut worker =
        SkillWorkerProcess::spawn(config(Duration::from_secs(2)), &request(""), &lifecycle)
            .unwrap();
    assert_eq!(
        worker.wait().unwrap(),
        SkillWorkerMessage::Completed {
            execution_id: "skill-run-1".to_owned(),
            output: SkillExecutionOutput::default(),
        }
    );
}

#[test]
fn host_rejects_a_valid_manifest_without_an_active_lease() {
    let lifecycle = SkillHost::default();
    assert!(matches!(
        SkillWorkerProcess::spawn(config(Duration::from_secs(1)), &request(""), &lifecycle,),
        Err(SkillHostError::Admission(_))
    ));
}
