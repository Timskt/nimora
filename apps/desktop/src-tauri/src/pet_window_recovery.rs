use std::{
    collections::VecDeque,
    sync::{
        Mutex,
        atomic::{AtomicBool, AtomicU64, Ordering},
    },
    time::Duration,
};

const RECOVERY_WINDOW_MS: u64 = 60_000;
const MAX_RECOVERY_ATTEMPTS: usize = 3;
const BASE_RETRY_DELAY: Duration = Duration::from_secs(1);
pub const HEARTBEAT_INTERVAL: Duration = Duration::from_secs(15);
pub const HEARTBEAT_TIMEOUT_MS: u64 = 90_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryDecision {
    RetryAfter(Duration),
    Exhausted,
}

#[derive(Debug, Default)]
pub struct PetWindowRecovery {
    attempts_ms: VecDeque<u64>,
}

#[derive(Debug, Default)]
pub struct PetWindowRecoveryHost {
    policy: Mutex<PetWindowRecovery>,
    active: AtomicBool,
    shutting_down: AtomicBool,
    last_heartbeat_ms: AtomicU64,
}

#[derive(Debug, Default)]
pub struct PetWindowWatchdog {
    was_visible: bool,
}

impl PetWindowWatchdog {
    pub fn should_recover(
        &mut self,
        host: &PetWindowRecoveryHost,
        visible: bool,
        now_ms: u64,
    ) -> bool {
        let became_visible = visible && !self.was_visible;
        self.was_visible = visible;
        if became_visible {
            host.record_heartbeat(now_ms);
            return false;
        }
        visible && host.heartbeat_is_stale(now_ms)
    }
}

impl PetWindowRecoveryHost {
    pub fn try_start(&self) -> bool {
        !self.shutting_down.load(Ordering::Acquire)
            && self
                .active
                .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
                .is_ok()
    }

    pub fn next_attempt(&self, now_ms: u64) -> RecoveryDecision {
        self.policy
            .lock()
            .ok()
            .map_or(RecoveryDecision::Exhausted, |mut policy| {
                policy.next_attempt(now_ms)
            })
    }

    pub fn finish(&self) {
        self.active.store(false, Ordering::Release);
    }

    pub fn begin_shutdown(&self) {
        self.shutting_down.store(true, Ordering::Release);
    }

    pub fn is_shutting_down(&self) -> bool {
        self.shutting_down.load(Ordering::Acquire)
    }

    pub fn record_heartbeat(&self, now_ms: u64) {
        self.last_heartbeat_ms.fetch_max(now_ms, Ordering::AcqRel);
    }

    pub fn heartbeat_is_stale(&self, now_ms: u64) -> bool {
        let last = self.last_heartbeat_ms.load(Ordering::Acquire);
        last > 0 && now_ms.saturating_sub(last) >= HEARTBEAT_TIMEOUT_MS
    }
}

impl PetWindowRecovery {
    pub fn next_attempt(&mut self, now_ms: u64) -> RecoveryDecision {
        while self
            .attempts_ms
            .front()
            .is_some_and(|attempt| now_ms.saturating_sub(*attempt) >= RECOVERY_WINDOW_MS)
        {
            self.attempts_ms.pop_front();
        }
        if self.attempts_ms.len() >= MAX_RECOVERY_ATTEMPTS {
            return RecoveryDecision::Exhausted;
        }
        let exponent = u32::try_from(self.attempts_ms.len()).unwrap_or(u32::MAX);
        let delay =
            BASE_RETRY_DELAY.saturating_mul(1_u32.checked_shl(exponent).unwrap_or(u32::MAX));
        self.attempts_ms.push_back(now_ms);
        RecoveryDecision::RetryAfter(delay)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retries_three_times_with_bounded_exponential_backoff() {
        let mut recovery = PetWindowRecovery::default();
        assert_eq!(
            recovery.next_attempt(0),
            RecoveryDecision::RetryAfter(Duration::from_secs(1))
        );
        assert_eq!(
            recovery.next_attempt(1_000),
            RecoveryDecision::RetryAfter(Duration::from_secs(2))
        );
        assert_eq!(
            recovery.next_attempt(3_000),
            RecoveryDecision::RetryAfter(Duration::from_secs(4))
        );
        assert_eq!(recovery.next_attempt(7_000), RecoveryDecision::Exhausted);
    }

    #[test]
    fn rolling_window_restores_the_budget_without_resetting_early() {
        let mut recovery = PetWindowRecovery::default();
        recovery.next_attempt(1_000);
        recovery.next_attempt(2_000);
        recovery.next_attempt(3_000);
        assert_eq!(recovery.next_attempt(60_999), RecoveryDecision::Exhausted);
        assert_eq!(
            recovery.next_attempt(61_000),
            RecoveryDecision::RetryAfter(Duration::from_secs(4))
        );
    }

    #[test]
    fn host_allows_only_one_worker_and_never_restarts_during_shutdown() {
        let host = PetWindowRecoveryHost::default();
        assert!(host.try_start());
        assert!(!host.try_start());
        host.finish();
        assert!(host.try_start());
        host.begin_shutdown();
        host.finish();
        assert!(!host.try_start());
    }

    #[test]
    fn heartbeat_ignores_clock_rollback_and_detects_bounded_staleness() {
        let host = PetWindowRecoveryHost::default();
        assert!(!host.heartbeat_is_stale(100_000));
        host.record_heartbeat(100_000);
        assert!(!host.heartbeat_is_stale(99_000));
        assert!(!host.heartbeat_is_stale(189_999));
        assert!(host.heartbeat_is_stale(190_000));
        host.record_heartbeat(200_000);
        host.record_heartbeat(150_000);
        assert!(!host.heartbeat_is_stale(289_999));
    }

    #[test]
    fn watchdog_pauses_hidden_windows_and_rearms_before_recovery() {
        let host = PetWindowRecoveryHost::default();
        let mut watchdog = PetWindowWatchdog::default();
        assert!(!watchdog.should_recover(&host, true, 100_000));
        assert!(!watchdog.should_recover(&host, false, 300_000));
        assert!(!watchdog.should_recover(&host, true, 400_000));
        assert!(!watchdog.should_recover(&host, true, 489_999));
        assert!(watchdog.should_recover(&host, true, 490_000));
    }
}
