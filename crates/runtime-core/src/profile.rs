use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct ProfileId(Uuid);

impl ProfileId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for ProfileId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePolicy {
    pub always_on_top: Option<bool>,
    pub click_through: Option<bool>,
    pub sound_enabled: Option<bool>,
    pub proactive_frequency: Option<u8>,
}

impl ProfilePolicy {
    #[must_use]
    pub fn merge(defaults: &Self, override_policy: &Self) -> Self {
        Self {
            always_on_top: override_policy.always_on_top.or(defaults.always_on_top),
            click_through: override_policy.click_through.or(defaults.click_through),
            sound_enabled: override_policy.sound_enabled.or(defaults.sound_enabled),
            proactive_frequency: override_policy
                .proactive_frequency
                .or(defaults.proactive_frequency)
                .map(|value| value.min(100)),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    pub id: ProfileId,
    pub name: String,
    pub policy: ProfilePolicy,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_policy_wins_without_copying_defaults() {
        let defaults = ProfilePolicy {
            always_on_top: Some(true),
            click_through: Some(false),
            sound_enabled: Some(true),
            proactive_frequency: Some(25),
        };
        let overrides = ProfilePolicy {
            always_on_top: None,
            click_through: Some(true),
            sound_enabled: None,
            proactive_frequency: Some(200),
        };
        let merged = ProfilePolicy::merge(&defaults, &overrides);
        assert_eq!(merged.always_on_top, Some(true));
        assert_eq!(merged.click_through, Some(true));
        assert_eq!(merged.proactive_frequency, Some(100));
    }
}
