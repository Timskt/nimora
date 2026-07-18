use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

const MAX_PROVIDER_VALUE_BYTES: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningEffort {
    Auto,
    Minimal,
    Low,
    Medium,
    High,
    VeryHigh,
    Maximum,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningStrategy {
    Adaptive,
    QualityFirst,
    CostSaver,
    Fixed,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelReasoningPolicy {
    pub strategy: ReasoningStrategy,
    pub requested: ReasoningEffort,
    pub allow_automatic_downgrade: bool,
}

impl Default for ModelReasoningPolicy {
    fn default() -> Self {
        Self {
            strategy: ReasoningStrategy::Adaptive,
            requested: ReasoningEffort::Auto,
            allow_automatic_downgrade: true,
        }
    }
}

impl ModelReasoningPolicy {
    /// Resolves a provider-neutral request against capabilities reported by an adapter.
    ///
    /// # Errors
    ///
    /// Returns an error when an explicit effort is unsupported and downgrade is forbidden.
    pub fn resolve(
        &self,
        supported: &BTreeSet<ReasoningEffort>,
        adaptive_recommendation: ReasoningEffort,
    ) -> Result<ReasoningEffort, ReasoningPolicyError> {
        if supported.is_empty() {
            return Err(ReasoningPolicyError::Unsupported);
        }
        let target = match self.strategy {
            ReasoningStrategy::Adaptive if self.requested == ReasoningEffort::Auto => {
                adaptive_recommendation
            }
            ReasoningStrategy::QualityFirst if self.requested == ReasoningEffort::Auto => supported
                .iter()
                .next_back()
                .copied()
                .ok_or(ReasoningPolicyError::Unsupported)?,
            ReasoningStrategy::CostSaver if self.requested == ReasoningEffort::Auto => supported
                .iter()
                .next()
                .copied()
                .ok_or(ReasoningPolicyError::Unsupported)?,
            _ if self.requested == ReasoningEffort::Auto => adaptive_recommendation,
            _ => self.requested,
        };
        if supported.contains(&target) {
            return Ok(target);
        }
        if !self.allow_automatic_downgrade || self.requested != ReasoningEffort::Auto {
            return Err(ReasoningPolicyError::Unsupported);
        }
        supported
            .range(..=target)
            .next_back()
            .or_else(|| supported.iter().next())
            .copied()
            .ok_or(ReasoningPolicyError::Unsupported)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReasoningMapping {
    pub requested: ReasoningEffort,
    pub actual: ReasoningEffort,
    pub provider_value: String,
    pub downgraded: bool,
}

impl ReasoningMapping {
    /// Creates the auditable output of a Provider Adapter mapping.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty/oversized provider value or a false downgrade claim.
    pub fn new(
        requested: ReasoningEffort,
        actual: ReasoningEffort,
        provider_value: impl Into<String>,
    ) -> Result<Self, ReasoningPolicyError> {
        let provider_value = provider_value.into();
        if provider_value.trim().is_empty() || provider_value.len() > MAX_PROVIDER_VALUE_BYTES {
            return Err(ReasoningPolicyError::InvalidMapping);
        }
        Ok(Self {
            requested,
            actual,
            provider_value,
            downgraded: requested != ReasoningEffort::Auto && actual < requested,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ReasoningPolicyError {
    #[error("requested reasoning effort is unsupported")]
    Unsupported,
    #[error("provider reasoning mapping is invalid")]
    InvalidMapping,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adaptive_auto_uses_recommendation() {
        let supported = BTreeSet::from([
            ReasoningEffort::Low,
            ReasoningEffort::Medium,
            ReasoningEffort::High,
        ]);
        assert_eq!(
            ModelReasoningPolicy::default()
                .resolve(&supported, ReasoningEffort::Medium)
                .expect("resolve"),
            ReasoningEffort::Medium
        );
    }

    #[test]
    fn explicit_unsupported_effort_fails_closed() {
        let policy = ModelReasoningPolicy {
            strategy: ReasoningStrategy::Fixed,
            requested: ReasoningEffort::Maximum,
            allow_automatic_downgrade: true,
        };
        assert_eq!(
            policy.resolve(
                &BTreeSet::from([ReasoningEffort::High]),
                ReasoningEffort::High
            ),
            Err(ReasoningPolicyError::Unsupported)
        );
    }
}
