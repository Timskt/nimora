//! Host-independent system context policy for desktop presence.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use thiserror::Error;

pub const SYSTEM_CONTEXT_SPEC: &str = "nimora.system-context/1";
pub const SYSTEM_CONTEXT_DECISION_SPEC: &str = "nimora.system-context-decision/1";
pub const MAX_SIGNAL_LIFETIME_MS: u64 = 30_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextKind {
    DoNotDisturb,
    Fullscreen,
    Game,
    ScreenShare,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SensorSource {
    OperatingSystem,
    ScreenCapture,
    ForegroundApplication,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PresenceOverride {
    Automatic,
    ForceVisible,
    ForceHidden,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    BasePolicy,
    UserForcedVisible,
    UserForcedHidden,
    SafeModeRecovery,
    DoNotDisturb,
    Fullscreen,
    Game,
    ScreenSharePrivacy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextSignal {
    pub spec: &'static str,
    pub kind: ContextKind,
    pub source: SensorSource,
    pub active: bool,
    pub observed_at_ms: u64,
    pub expires_at_ms: u64,
}

impl ContextSignal {
    /// Creates a bounded sensor observation.
    ///
    /// # Errors
    ///
    /// Returns an error for zero-length, inverted, or overly long observations.
    pub fn new(
        kind: ContextKind,
        source: SensorSource,
        active: bool,
        observed_at_ms: u64,
        expires_at_ms: u64,
    ) -> Result<Self, ContextPolicyError> {
        let lifetime = expires_at_ms
            .checked_sub(observed_at_ms)
            .ok_or(ContextPolicyError::InvalidLifetime)?;
        if lifetime == 0 || lifetime > MAX_SIGNAL_LIFETIME_MS {
            return Err(ContextPolicyError::InvalidLifetime);
        }
        Ok(Self {
            spec: SYSTEM_CONTEXT_SPEC,
            kind,
            source,
            active,
            observed_at_ms,
            expires_at_ms,
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresenceDecision {
    pub spec: &'static str,
    pub visible: bool,
    pub suppress_autonomy: bool,
    pub reason: DecisionReason,
    pub decided_at_ms: u64,
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum ContextPolicyError {
    #[error("system context signal lifetime is invalid")]
    InvalidLifetime,
    #[error("system context signal moved backwards for its source and kind")]
    StaleObservation,
}

#[derive(Debug, Clone, Default)]
pub struct SystemContextPolicy {
    signals: BTreeMap<(ContextKind, SensorSource), ContextSignal>,
}

impl SystemContextPolicy {
    /// Records the latest observation for a sensor and context kind.
    ///
    /// # Errors
    ///
    /// Returns an error when a sensor attempts to replace its observation with older data.
    pub fn observe(&mut self, signal: ContextSignal) -> Result<(), ContextPolicyError> {
        let key = (signal.kind, signal.source);
        if self
            .signals
            .get(&key)
            .is_some_and(|current| signal.observed_at_ms < current.observed_at_ms)
        {
            return Err(ContextPolicyError::StaleObservation);
        }
        self.signals.insert(key, signal);
        Ok(())
    }

    pub fn prune_expired(&mut self, now_ms: u64) {
        self.signals
            .retain(|_, signal| signal.expires_at_ms > now_ms);
    }

    #[must_use]
    pub fn decide(
        &self,
        base_visible: bool,
        presence_override: PresenceOverride,
        safe_mode: bool,
        now_ms: u64,
    ) -> PresenceDecision {
        if safe_mode {
            return decision(true, false, DecisionReason::SafeModeRecovery, now_ms);
        }
        if presence_override == PresenceOverride::ForceHidden {
            return decision(false, true, DecisionReason::UserForcedHidden, now_ms);
        }
        let active = |kind| {
            self.signals
                .values()
                .any(|signal| signal.kind == kind && signal.active && signal.expires_at_ms > now_ms)
        };
        if active(ContextKind::ScreenShare) {
            return decision(false, true, DecisionReason::ScreenSharePrivacy, now_ms);
        }
        if presence_override == PresenceOverride::ForceVisible {
            return decision(true, false, DecisionReason::UserForcedVisible, now_ms);
        }
        for (kind, reason) in [
            (ContextKind::Game, DecisionReason::Game),
            (ContextKind::Fullscreen, DecisionReason::Fullscreen),
            (ContextKind::DoNotDisturb, DecisionReason::DoNotDisturb),
        ] {
            if active(kind) {
                return decision(false, true, reason, now_ms);
            }
        }
        decision(
            base_visible,
            !base_visible,
            DecisionReason::BasePolicy,
            now_ms,
        )
    }
}

const fn decision(
    visible: bool,
    suppress_autonomy: bool,
    reason: DecisionReason,
    decided_at_ms: u64,
) -> PresenceDecision {
    PresenceDecision {
        spec: SYSTEM_CONTEXT_DECISION_SPEC,
        visible,
        suppress_autonomy,
        reason,
        decided_at_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signal(kind: ContextKind, source: SensorSource, active: bool, at: u64) -> ContextSignal {
        ContextSignal::new(kind, source, active, at, at + 5_000).unwrap()
    }

    #[test]
    fn privacy_signal_overrides_force_visible() {
        let mut policy = SystemContextPolicy::default();
        policy
            .observe(signal(
                ContextKind::ScreenShare,
                SensorSource::ScreenCapture,
                true,
                100,
            ))
            .unwrap();
        let decision = policy.decide(true, PresenceOverride::ForceVisible, false, 101);
        assert!(!decision.visible);
        assert_eq!(decision.reason, DecisionReason::ScreenSharePrivacy);
    }

    #[test]
    fn explicit_visible_overrides_non_privacy_disturbance() {
        let mut policy = SystemContextPolicy::default();
        policy
            .observe(signal(
                ContextKind::Game,
                SensorSource::ForegroundApplication,
                true,
                100,
            ))
            .unwrap();
        let decision = policy.decide(true, PresenceOverride::ForceVisible, false, 101);
        assert!(decision.visible);
        assert_eq!(decision.reason, DecisionReason::UserForcedVisible);
    }

    #[test]
    fn safe_mode_is_always_visible_and_interactive() {
        let policy = SystemContextPolicy::default();
        let decision = policy.decide(false, PresenceOverride::ForceHidden, true, 100);
        assert!(decision.visible);
        assert!(!decision.suppress_autonomy);
        assert_eq!(decision.reason, DecisionReason::SafeModeRecovery);
    }

    #[test]
    fn expired_and_negative_observations_do_not_hide_pet() {
        let mut policy = SystemContextPolicy::default();
        policy
            .observe(signal(
                ContextKind::Fullscreen,
                SensorSource::OperatingSystem,
                true,
                100,
            ))
            .unwrap();
        policy
            .observe(signal(
                ContextKind::DoNotDisturb,
                SensorSource::OperatingSystem,
                false,
                200,
            ))
            .unwrap();
        let decision = policy.decide(true, PresenceOverride::Automatic, false, 5_101);
        assert!(decision.visible);
        assert_eq!(decision.reason, DecisionReason::BasePolicy);
    }

    #[test]
    fn rejects_unbounded_and_backwards_sensor_data() {
        assert_eq!(
            ContextSignal::new(
                ContextKind::Game,
                SensorSource::ForegroundApplication,
                true,
                1,
                30_002
            ),
            Err(ContextPolicyError::InvalidLifetime)
        );
        let mut policy = SystemContextPolicy::default();
        policy
            .observe(signal(
                ContextKind::Game,
                SensorSource::ForegroundApplication,
                true,
                200,
            ))
            .unwrap();
        assert_eq!(
            policy.observe(signal(
                ContextKind::Game,
                SensorSource::ForegroundApplication,
                false,
                199
            )),
            Err(ContextPolicyError::StaleObservation)
        );
    }
}
