use nimora_user_code_host::{WorkerConfig, WorkerMessage, WorkerProcess};
use serde_json::json;
use std::time::Duration;

fn worker_config(timeout: Duration) -> WorkerConfig {
    WorkerConfig {
        executable: env!("CARGO_BIN_EXE_nimora-user-code-worker").to_owned(),
        args: Vec::new(),
        execution_id: "integration-run".to_owned(),
        timeout,
        output_bytes: 1024 * 1024,
    }
}

#[test]
fn supervisor_runs_real_worker_process() {
    let request = WorkerMessage::Run {
        manifest: json!({"id": "integration.example.test"}),
        source: "({ value: 42 })".to_owned(),
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
    };
    let mut process =
        WorkerProcess::spawn(worker_config(Duration::from_millis(100)), &request).unwrap();
    assert_eq!(
        process.wait(),
        Err(nimora_user_code_host::HostError::TimedOut)
    );
}
