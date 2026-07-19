use std::sync::Mutex;

#[derive(Debug, Default)]
pub struct DesktopLifecycleGate {
    state: Mutex<DesktopLifecycleState>,
}

#[derive(Debug, Default)]
struct DesktopLifecycleState {
    shutting_down: bool,
}

impl DesktopLifecycleGate {
    pub fn run_if_active<T>(&self, operation: impl FnOnce() -> T) -> Option<T> {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if state.shutting_down {
            return None;
        }
        Some(operation())
    }

    pub fn begin_shutdown(&self) {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .shutting_down = true;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        sync::{Arc, mpsc},
        time::Duration,
    };

    #[test]
    fn shutdown_permanently_rejects_new_operations() {
        let gate = DesktopLifecycleGate::default();
        assert_eq!(gate.run_if_active(|| 42), Some(42));
        gate.begin_shutdown();
        assert_eq!(gate.run_if_active(|| 7), None);
    }

    #[test]
    fn shutdown_waits_for_an_admitted_operation_before_closing_the_gate() {
        let gate = Arc::new(DesktopLifecycleGate::default());
        let (operation_started_tx, operation_started_rx) = mpsc::channel();
        let (release_operation_tx, release_operation_rx) = mpsc::channel();
        let operation_gate = Arc::clone(&gate);
        let operation = std::thread::spawn(move || {
            operation_gate.run_if_active(|| {
                operation_started_tx.send(()).expect("signal operation");
                release_operation_rx.recv().expect("release operation");
            })
        });
        operation_started_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("operation starts");

        let (shutdown_finished_tx, shutdown_finished_rx) = mpsc::channel();
        let shutdown_gate = Arc::clone(&gate);
        let shutdown = std::thread::spawn(move || {
            shutdown_gate.begin_shutdown();
            shutdown_finished_tx.send(()).expect("signal shutdown");
        });
        assert!(
            shutdown_finished_rx
                .recv_timeout(Duration::from_millis(50))
                .is_err()
        );

        release_operation_tx.send(()).expect("release operation");
        assert_eq!(operation.join().expect("join operation"), Some(()));
        shutdown.join().expect("join shutdown");
        shutdown_finished_rx
            .recv_timeout(Duration::from_secs(1))
            .expect("shutdown finishes");
        assert_eq!(gate.run_if_active(|| ()), None);
    }
}
