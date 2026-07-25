use nimora_user_code_host::{
    ExecutionCancellation, HostError, WorkerConfig, WorkerMessage, WorkerProcess,
};
use serde_json::json;
use std::time::Duration;

fn worker_config(timeout: Duration) -> WorkerConfig {
    WorkerConfig {
        executable: env!("CARGO_BIN_EXE_nimora-user-code-worker").to_owned(),
        args: Vec::new(),
        execution_id: "integration-run".to_owned(),
        timeout,
        output_bytes: 1024 * 1024,
        cancellation: None,
    }
}

#[test]
fn supervisor_runs_real_worker_process() {
    let request = WorkerMessage::Run {
        manifest: json!({"id": "integration.example.test"}),
        source: "({ value: 42 })".to_owned(),
        input: json!(null),
    };
    let mut process =
        WorkerProcess::spawn(worker_config(Duration::from_secs(2)), &request).unwrap();
    assert_eq!(
        process.wait().unwrap(),
        WorkerMessage::Result {
            value: json!({"value": 42})
        }
    );
}

#[test]
fn supervisor_terminates_an_infinite_worker() {
    let request = WorkerMessage::Run {
        manifest: json!({"id": "integration.example.loop"}),
        source: "while (true) {}".to_owned(),
        input: json!(null),
    };
    let mut process =
        WorkerProcess::spawn(worker_config(Duration::from_millis(100)), &request).unwrap();
    assert_eq!(
        process.wait(),
        Err(nimora_user_code_host::HostError::TimedOut)
    );
}

#[test]
fn supervisor_honors_cross_thread_cancellation() {
    let cancellation = ExecutionCancellation::default();
    let mut config = worker_config(Duration::from_secs(5));
    config.cancellation = Some(cancellation.clone());
    let request = WorkerMessage::Run {
        manifest: json!({}),
        source: "while (true) {}".to_owned(),
        input: json!(null),
    };
    let mut worker = WorkerProcess::spawn(config, &request).expect("spawn worker");
    std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(50));
        cancellation.cancel();
    });
    assert_eq!(worker.wait(), Err(HostError::Cancelled));
}

#[test]
fn supervisor_enforces_the_output_budget() {
    let mut config = worker_config(Duration::from_secs(2));
    config.output_bytes = 4;
    let request = WorkerMessage::Run {
        manifest: json!({"id": "integration.example.output"}),
        source: "({ value: 'a very long result that exceeds the tiny output budget' })".to_owned(),
        input: json!(null),
    };
    let mut process = WorkerProcess::spawn(config, &request).expect("spawn worker");
    assert_eq!(process.wait(), Err(HostError::OutputLimit));
}

#[test]
fn supervisor_reports_a_worker_that_exits_without_a_result() {
    let request = WorkerMessage::Cancel;
    let mut process =
        WorkerProcess::spawn(worker_config(Duration::from_secs(2)), &request).expect("spawn worker");
    assert!(matches!(
        process.wait(),
        Ok(WorkerMessage::Error { .. }) | Err(HostError::Crashed)
    ));
}
