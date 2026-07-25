use nimora_runtime_core::CommandRisk;
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;
use thiserror::Error;

const MAX_PROVIDER_VALUE_BYTES: usize = 64;
const MAX_MAPPING_VERSION_BYTES: usize = 64;

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

/// Broad category of the work an Agent step is about to perform.
///
/// Mirrors the task-type input in `MODEL_REASONING_POLICY.md` §4 without leaking any
/// prompt content: the caller classifies intent, not the untrusted model output.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ReasoningTaskKind {
    /// Formatting, stable structured extraction, short summaries, deterministic orchestration.
    MechanicalEdit,
    /// General question answering, ordinary code edits, and test fixes.
    GeneralEdit,
    /// Architecture, complex debugging, migrations, and security or permission decisions.
    ComplexReasoning,
    /// User-demanded critical planning, independent review, or hard reasoning.
    CriticalReasoning,
}

/// How much of the model context window the pending step is expected to consume.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextPressure {
    Low,
    Moderate,
    High,
}

/// The user's standing quality-versus-cost preference for the Goal.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QualityPreference {
    Economy,
    Balanced,
    Quality,
}

/// Remaining headroom for a bounded resource (cost or latency).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BudgetPressure {
    Ample,
    Tight,
    Critical,
}

/// Bounded, prompt-free inputs the Adaptive strategy uses to recommend an effort.
///
/// Every field is host-derived signal, never model output, so a compromised Provider
/// response cannot raise reasoning effort past organization limits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AdaptiveReasoningSignals {
    pub task: ReasoningTaskKind,
    pub max_command_risk: CommandRisk,
    pub has_external_side_effects: bool,
    pub recent_failures: u32,
    pub context_pressure: ContextPressure,
    pub quality_preference: QualityPreference,
    pub cost_pressure: BudgetPressure,
    pub latency_pressure: BudgetPressure,
}

impl Default for AdaptiveReasoningSignals {
    fn default() -> Self {
        Self {
            task: ReasoningTaskKind::GeneralEdit,
            max_command_risk: CommandRisk::Safe,
            has_external_side_effects: false,
            recent_failures: 0,
            context_pressure: ContextPressure::Low,
            quality_preference: QualityPreference::Balanced,
            cost_pressure: BudgetPressure::Ample,
            latency_pressure: BudgetPressure::Ample,
        }
    }
}

/// Concrete effort ladder the recommender maps onto, excluding `Auto`.
const ADAPTIVE_LADDER: [ReasoningEffort; 6] = [
    ReasoningEffort::Minimal,
    ReasoningEffort::Low,
    ReasoningEffort::Medium,
    ReasoningEffort::High,
    ReasoningEffort::VeryHigh,
    ReasoningEffort::Maximum,
];

impl AdaptiveReasoningSignals {
    /// Computes a concrete recommended effort from bounded host signals.
    ///
    /// The result is never `Auto`; it is a starting recommendation for
    /// [`ModelReasoningPolicy::resolve`], which still enforces the supported capability
    /// set, allowlists, and downgrade policy. Cost and latency pressure can only lower
    /// the recommendation, never raise it past what quality signals justify.
    #[must_use]
    pub fn recommend(&self) -> ReasoningEffort {
        let base: i32 = match self.task {
            ReasoningTaskKind::MechanicalEdit => 1,
            ReasoningTaskKind::GeneralEdit => 2,
            ReasoningTaskKind::ComplexReasoning => 3,
            ReasoningTaskKind::CriticalReasoning => 4,
        };
        let mut level = base;
        level += match self.max_command_risk {
            CommandRisk::Safe | CommandRisk::Low => 0,
            CommandRisk::Medium => 1,
            CommandRisk::High | CommandRisk::Critical => 2,
        };
        level += i32::from(self.has_external_side_effects);
        level += match self.recent_failures {
            0 => 0,
            1..=2 => 1,
            _ => 2,
        };
        level += match self.context_pressure {
            ContextPressure::Low | ContextPressure::Moderate => 0,
            ContextPressure::High => 1,
        };
        level += match self.quality_preference {
            QualityPreference::Economy => -1,
            QualityPreference::Balanced => 0,
            QualityPreference::Quality => 1,
        };
        level += match self.cost_pressure {
            BudgetPressure::Ample => 0,
            BudgetPressure::Tight => -1,
            BudgetPressure::Critical => -2,
        };
        level += match self.latency_pressure {
            BudgetPressure::Ample => 0,
            BudgetPressure::Tight => -1,
            BudgetPressure::Critical => -2,
        };
        let index = usize::try_from(level.max(0)).unwrap_or(0);
        ADAPTIVE_LADDER[index.min(ADAPTIVE_LADDER.len() - 1)]
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReasoningMapping {
    pub requested: ReasoningEffort,
    pub actual: ReasoningEffort,
    pub provider_value: String,
    pub mapping_version: String,
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
        mapping_version: impl Into<String>,
    ) -> Result<Self, ReasoningPolicyError> {
        let provider_value = provider_value.into();
        let mapping_version = mapping_version.into();
        if provider_value.trim().is_empty()
            || provider_value.len() > MAX_PROVIDER_VALUE_BYTES
            || mapping_version.trim().is_empty()
            || mapping_version.len() > MAX_MAPPING_VERSION_BYTES
        {
            return Err(ReasoningPolicyError::InvalidMapping);
        }
        Ok(Self {
            requested,
            actual,
            provider_value,
            mapping_version,
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
    fn adaptive_signals_lift_effort_for_risky_complex_work() {
        let signals = AdaptiveReasoningSignals {
            task: ReasoningTaskKind::ComplexReasoning,
            max_command_risk: CommandRisk::High,
            has_external_side_effects: true,
            recent_failures: 3,
            context_pressure: ContextPressure::High,
            quality_preference: QualityPreference::Quality,
            cost_pressure: BudgetPressure::Ample,
            latency_pressure: BudgetPressure::Ample,
        };
        assert_eq!(signals.recommend(), ReasoningEffort::Maximum);
    }

    #[test]
    fn adaptive_signals_lower_effort_for_cheap_mechanical_work() {
        let signals = AdaptiveReasoningSignals {
            task: ReasoningTaskKind::MechanicalEdit,
            quality_preference: QualityPreference::Economy,
            cost_pressure: BudgetPressure::Critical,
            ..AdaptiveReasoningSignals::default()
        };
        assert_eq!(signals.recommend(), ReasoningEffort::Minimal);
    }

    #[test]
    fn adaptive_default_is_medium_matching_prior_host_baseline() {
        assert_eq!(
            AdaptiveReasoningSignals::default().recommend(),
            ReasoningEffort::Medium
        );
    }

    #[test]
    fn adaptive_recommendation_feeds_policy_resolution() {
        let signals = AdaptiveReasoningSignals {
            task: ReasoningTaskKind::CriticalReasoning,
            quality_preference: QualityPreference::Quality,
            ..AdaptiveReasoningSignals::default()
        };
        let recommended = signals.recommend();
        assert_eq!(recommended, ReasoningEffort::Maximum);
        // Adaptive still fails closed onto the supported capability set.
        let supported = BTreeSet::from([ReasoningEffort::Low, ReasoningEffort::Medium]);
        assert_eq!(
            ModelReasoningPolicy::default()
                .resolve(&supported, recommended)
                .expect("resolve"),
            ReasoningEffort::Medium
        );
    }

    #[test]
    fn budget_pressure_never_raises_effort_above_quality_signal() {
        let calm = AdaptiveReasoningSignals {
            task: ReasoningTaskKind::GeneralEdit,
            ..AdaptiveReasoningSignals::default()
        };
        let pressured = AdaptiveReasoningSignals {
            cost_pressure: BudgetPressure::Critical,
            latency_pressure: BudgetPressure::Critical,
            ..calm
        };
        assert!(pressured.recommend() <= calm.recommend());
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
