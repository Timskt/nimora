pub use nimora_user_code_policy::ExecutionCancellation;
use serde::{Deserialize, Serialize};
use std::io::{BufRead, BufReader, Write};
use std::process::{Child, Command, Stdio};
use std::sync::mpsc::{self, Receiver, RecvTimeoutError};
use std::thread;
use std::time::{Duration, Instant};
use thiserror::Error;

const PROTOCOL_VERSION: u16 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum WorkerMessage {
    Hello {
        protocol_version: u16,
        execution_id: String,
    },
    Run {
        manifest: serde_json::Value,
        source: String,
        #[serde(default)]
        input: serde_json::Value,
    },
    Validate {
        source: String,
    },
    Cancel,
    Result {
        value: serde_json::Value,
    },
    Validated,
    Error {
        code: String,
        message: String,
    },
    Log {
        level: String,
        message: String,
    },
}

#[derive(Debug, Clone)]
pub struct WorkerConfig {
    pub executable: String,
    pub args: Vec<String>,
    pub execution_id: String,
    pub timeout: Duration,
    pub output_bytes: u64,
    pub cancellation: Option<ExecutionCancellation>,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum HostError {
    #[error("failed to start worker: {0}")]
    Start(String),
    #[error("worker protocol error: {0}")]
    Protocol(String),
    #[error("worker timed out")]
    TimedOut,
    #[error("worker was cancelled")]
    Cancelled,
    #[error("worker output exceeded its budget")]
    OutputLimit,
    #[error("worker exited without a result")]
    Crashed,
    #[error("worker I/O error: {0}")]
    Io(String),
}

pub struct WorkerProcess {
    child: Child,
    lines: Receiver<Result<Vec<u8>, String>>,
    started: Instant,
    config: WorkerConfig,
    output_seen: u64,
    cancelled: bool,
}

impl std::fmt::Debug for WorkerProcess {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("WorkerProcess")
            .field("execution_id", &self.config.execution_id)
            .field("output_seen", &self.output_seen)
            .field("cancelled", &self.cancelled)
            .finish_non_exhaustive()
    }
}

impl WorkerProcess {
    /// Starts a worker and sends its first protocol message.
    ///
    /// # Errors
    ///
    /// Returns an error when the process cannot start or the initial message
    /// cannot be encoded or written.
    pub fn spawn(config: WorkerConfig, request: &WorkerMessage) -> Result<Self, HostError> {
        let mut command = Command::new(&config.executable);
        command
            .args(&config.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::null());
        let mut child = command
            .spawn()
            .map_err(|error| HostError::Start(error.to_string()))?;
        let mut stdin = child
            .stdin
            .take()
            .ok_or_else(|| HostError::Start("worker stdin unavailable".to_owned()))?;
        let payload =
            serde_json::to_vec(request).map_err(|error| HostError::Protocol(error.to_string()))?;
        stdin
            .write_all(&payload)
            .and_then(|()| stdin.write_all(b"\n"))
            .and_then(|()| stdin.flush())
            .map_err(|error| HostError::Io(error.to_string()))?;
        drop(stdin);
        let stdout = child
            .stdout
            .take()
            .ok_or_else(|| HostError::Start("worker stdout unavailable".to_owned()))?;
        let (sender, lines) = mpsc::channel();
        thread::spawn(move || {
            for line in BufReader::new(stdout).split(b'\n') {
                let result = line.map_err(|error| error.to_string());
                if sender.send(result).is_err() {
                    break;
                }
            }
        });
        Ok(Self {
            child,
            lines,
            started: Instant::now(),
            config,
            output_seen: 0,
            cancelled: false,
        })
    }

    /// Terminates the worker immediately.
    ///
    /// # Errors
    ///
    /// Returns an error when the operating system rejects termination.
    pub fn cancel(&mut self) -> Result<(), HostError> {
        self.cancelled = true;
        self.child
            .kill()
            .map_err(|error| HostError::Io(error.to_string()))
    }

    /// Waits for a terminal worker message, enforcing the configured limits.
    ///
    /// # Errors
    ///
    /// Returns an error when the worker times out, is cancelled, crashes, or
    /// violates the protocol or output budget.
    pub fn wait(&mut self) -> Result<WorkerMessage, HostError> {
        loop {
            if self
                .config
                .cancellation
                .as_ref()
                .is_some_and(ExecutionCancellation::is_cancelled)
            {
                self.cancelled = true;
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(HostError::Cancelled);
            }
            if self.started.elapsed() >= self.config.timeout {
                let _ = self.child.kill();
                let _ = self.child.wait();
                return Err(if self.cancelled {
                    HostError::Cancelled
                } else {
                    HostError::TimedOut
                });
            }
            match self.lines.recv_timeout(Duration::from_millis(10)) {
                Ok(Ok(line)) => {
                    self.output_seen = self.output_seen.saturating_add(line.len() as u64 + 1);
                    if self.output_seen > self.config.output_bytes {
                        let _ = self.child.kill();
                        let _ = self.child.wait();
                        return Err(HostError::OutputLimit);
                    }
                    if line.is_empty() {
                        continue;
                    }
                    let message = serde_json::from_slice::<WorkerMessage>(&line)
                        .map_err(|error| HostError::Protocol(error.to_string()))?;
                    if matches!(
                        message,
                        WorkerMessage::Result { .. }
                            | WorkerMessage::Validated
                            | WorkerMessage::Error { .. }
                    ) {
                        let _ = self.child.wait();
                        return Ok(message);
                    }
                }
                Ok(Err(error)) => return Err(HostError::Io(error)),
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => {
                    let _ = self.child.wait();
                    return Err(HostError::Crashed);
                }
            }
        }
    }
}

impl Drop for WorkerProcess {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

#[must_use]
pub fn protocol_version() -> u16 {
    PROTOCOL_VERSION
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn protocol_messages_round_trip_as_jsonl() {
        for message in [
            WorkerMessage::Hello {
                protocol_version: PROTOCOL_VERSION,
                execution_id: "run-1".to_owned(),
            },
            WorkerMessage::Validate {
                source: "throw new Error('must not run')".to_owned(),
            },
            WorkerMessage::Validated,
        ] {
            let encoded = serde_json::to_string(&message).unwrap();
            assert_eq!(
                serde_json::from_str::<WorkerMessage>(&encoded).unwrap(),
                message
            );
        }
    }

    #[test]
    fn protocol_version_is_explicit() {
        assert_eq!(protocol_version(), 1);
    }
}
