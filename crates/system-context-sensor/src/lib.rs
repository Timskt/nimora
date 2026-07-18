//! Host-independent scheduling and health policy for system-context sensors.

use nimora_system_context::{ContextKind, ContextSignal, SensorSource};
use serde::Serialize;
use std::time::Duration;
use thiserror::Error;

pub const SENSOR_HEALTH_SPEC: &str = "nimora.system-context-sensor-health/1";
pub const DEFAULT_SAMPLE_INTERVAL: Duration = Duration::from_secs(5);
pub const DEFAULT_SAMPLE_TIMEOUT: Duration = Duration::from_secs(2);
pub const MAX_RETRY_DELAY: Duration = Duration::from_secs(30);
pub const SIGNAL_LEASE: Duration = Duration::from_secs(15);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensorDescriptor {
    pub kind: ContextKind,
    pub source: SensorSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SensorSchedule {
    pub sample_interval: Duration,
    pub sample_timeout: Duration,
    pub maximum_retry_delay: Duration,
    pub signal_lease: Duration,
}

impl Default for SensorSchedule {
    fn default() -> Self {
        Self {
            sample_interval: DEFAULT_SAMPLE_INTERVAL,
            sample_timeout: DEFAULT_SAMPLE_TIMEOUT,
            maximum_retry_delay: MAX_RETRY_DELAY,
            signal_lease: SIGNAL_LEASE,
        }
    }
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum SensorScheduleError {
    #[error("system context sensor schedule contains a zero duration")]
    ZeroDuration,
    #[error("system context sensor lease must exceed its sample interval")]
    LeaseTooShort,
    #[error("system context sensor lease exceeds the policy limit")]
    LeaseTooLong,
}

impl SensorSchedule {
    /// Validates scheduling values before any platform task starts.
    ///
    /// # Errors
    ///
    /// Returns a stable error when durations cannot provide bounded sampling and renewal.
    pub fn validate(self) -> Result<Self, SensorScheduleError> {
        if self.sample_interval.is_zero()
            || self.sample_timeout.is_zero()
            || self.maximum_retry_delay.is_zero()
            || self.signal_lease.is_zero()
        {
            return Err(SensorScheduleError::ZeroDuration);
        }
        if self.signal_lease <= self.sample_interval {
            return Err(SensorScheduleError::LeaseTooShort);
        }
        if self.signal_lease > Duration::from_secs(30) {
            return Err(SensorScheduleError::LeaseTooLong);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorAvailability {
    Available,
    Degraded,
    Unavailable,
    Stopped,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SensorHealth {
    pub spec: &'static str,
    pub availability: SensorAvailability,
    pub consecutive_failures: u32,
    pub last_success_at_ms: Option<u64>,
    pub last_error_code: Option<String>,
    pub next_sample_at_ms: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct SensorController {
    descriptor: SensorDescriptor,
    schedule: SensorSchedule,
    health: SensorHealth,
}

impl SensorController {
    /// Creates a validated sensor controller.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid scheduling values.
    pub fn new(
        descriptor: SensorDescriptor,
        schedule: SensorSchedule,
        now_ms: u64,
    ) -> Result<Self, SensorScheduleError> {
        let schedule = schedule.validate()?;
        Ok(Self {
            descriptor,
            schedule,
            health: SensorHealth {
                spec: SENSOR_HEALTH_SPEC,
                availability: SensorAvailability::Unavailable,
                consecutive_failures: 0,
                last_success_at_ms: None,
                last_error_code: None,
                next_sample_at_ms: Some(now_ms),
            },
        })
    }

    #[must_use]
    pub const fn health(&self) -> &SensorHealth {
        &self.health
    }

    #[must_use]
    pub fn is_due(&self, now_ms: u64) -> bool {
        self.health
            .next_sample_at_ms
            .is_some_and(|next| next <= now_ms)
    }

    /// Records a successful platform observation and creates a bounded policy signal.
    ///
    /// # Errors
    ///
    /// Returns an error only when the validated lease cannot be represented in milliseconds.
    pub fn record_success(
        &mut self,
        active: bool,
        now_ms: u64,
    ) -> Result<ContextSignal, SensorScheduleError> {
        let lease_ms = duration_ms(self.schedule.signal_lease)?;
        let interval_ms = duration_ms(self.schedule.sample_interval)?;
        self.health.availability = SensorAvailability::Available;
        self.health.consecutive_failures = 0;
        self.health.last_success_at_ms = Some(now_ms);
        self.health.last_error_code = None;
        self.health.next_sample_at_ms = Some(now_ms.saturating_add(interval_ms));
        ContextSignal::new(
            self.descriptor.kind,
            self.descriptor.source,
            active,
            now_ms,
            now_ms.saturating_add(lease_ms),
        )
        .map_err(|_| SensorScheduleError::LeaseTooLong)
    }

    pub fn record_failure(&mut self, error_code: impl Into<String>, now_ms: u64) {
        self.health.consecutive_failures = self.health.consecutive_failures.saturating_add(1);
        self.health.availability = if self.health.last_success_at_ms.is_some() {
            SensorAvailability::Degraded
        } else {
            SensorAvailability::Unavailable
        };
        self.health.last_error_code = Some(error_code.into());
        let exponent = self.health.consecutive_failures.saturating_sub(1).min(31);
        let multiplier = 1_u32.checked_shl(exponent).unwrap_or(u32::MAX);
        let retry = self
            .schedule
            .sample_interval
            .saturating_mul(multiplier)
            .min(self.schedule.maximum_retry_delay);
        self.health.next_sample_at_ms =
            Some(now_ms.saturating_add(u64::try_from(retry.as_millis()).unwrap_or(u64::MAX)));
    }

    pub fn stop(&mut self) {
        self.health.availability = SensorAvailability::Stopped;
        self.health.next_sample_at_ms = None;
    }
}

fn duration_ms(duration: Duration) -> Result<u64, SensorScheduleError> {
    duration
        .as_millis()
        .try_into()
        .map_err(|_| SensorScheduleError::LeaseTooLong)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn controller(now_ms: u64) -> SensorController {
        SensorController::new(
            SensorDescriptor {
                kind: ContextKind::Fullscreen,
                source: SensorSource::OperatingSystem,
            },
            SensorSchedule::default(),
            now_ms,
        )
        .unwrap()
    }

    #[test]
    fn success_renews_a_bounded_signal_and_resets_health() {
        let mut controller = controller(100);
        controller.record_failure("sample-timeout", 100);
        let signal = controller.record_success(true, 5_100).unwrap();
        assert!(signal.active);
        assert_eq!(signal.expires_at_ms, 20_100);
        assert_eq!(
            controller.health().availability,
            SensorAvailability::Available
        );
        assert_eq!(controller.health().consecutive_failures, 0);
        assert_eq!(controller.health().next_sample_at_ms, Some(10_100));
    }

    #[test]
    fn failures_back_off_with_a_stable_cap() {
        let mut controller = controller(0);
        for attempt in 0_u64..5 {
            controller.record_failure("platform-unavailable", attempt * 100_000);
        }
        assert_eq!(controller.health().consecutive_failures, 5);
        assert_eq!(controller.health().next_sample_at_ms, Some(430_000));
        assert_eq!(
            controller.health().availability,
            SensorAvailability::Unavailable
        );
    }

    #[test]
    fn stop_is_terminal_for_scheduling() {
        let mut controller = controller(0);
        controller.stop();
        assert!(!controller.is_due(u64::MAX));
        assert_eq!(
            controller.health().availability,
            SensorAvailability::Stopped
        );
    }

    #[test]
    fn schedule_rejects_leases_that_cannot_be_renewed() {
        let schedule = SensorSchedule {
            signal_lease: DEFAULT_SAMPLE_INTERVAL,
            ..SensorSchedule::default()
        };
        assert_eq!(schedule.validate(), Err(SensorScheduleError::LeaseTooShort));
    }
}
