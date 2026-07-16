use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeMode {
    Normal,
    Safe,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SafeModeReason {
    Manual,
    CrashLoop,
    DataRecovery,
    PolicyViolation,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SafetySnapshot {
    pub mode: RuntimeMode,
    pub reason: Option<SafeModeReason>,
}

impl SafetySnapshot {
    #[must_use]
    pub const fn normal() -> Self {
        Self {
            mode: RuntimeMode::Normal,
            reason: None,
        }
    }

    #[must_use]
    pub const fn safe(reason: SafeModeReason) -> Self {
        Self {
            mode: RuntimeMode::Safe,
            reason: Some(reason),
        }
    }
}

impl Default for SafetySnapshot {
    fn default() -> Self {
        Self::normal()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn safe_mode_always_records_a_reason() {
        assert_eq!(
            SafetySnapshot::safe(SafeModeReason::Manual),
            SafetySnapshot {
                mode: RuntimeMode::Safe,
                reason: Some(SafeModeReason::Manual),
            }
        );
        assert_eq!(SafetySnapshot::normal().reason, None);
    }
}
