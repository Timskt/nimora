use serde::{Deserialize, Serialize};
use thiserror::Error;
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProfileMode {
    Companion,
    Work,
    Focus,
    Creator,
    Developer,
    Presentation,
    Offline,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CareNeedsMode {
    Full,
    Simple,
    Off,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfilePolicy {
    pub mode: ProfileMode,
    pub always_on_top: Option<bool>,
    pub click_through: Option<bool>,
    #[serde(default)]
    pub edge_snap: Option<bool>,
    pub sound_enabled: Option<bool>,
    pub proactive_frequency: Option<u8>,
    #[serde(default)]
    pub care_needs_mode: Option<CareNeedsMode>,
}

impl ProfilePolicy {
    #[must_use]
    pub const fn standard() -> Self {
        Self {
            mode: ProfileMode::Companion,
            always_on_top: Some(true),
            click_through: Some(false),
            edge_snap: Some(true),
            sound_enabled: Some(true),
            proactive_frequency: Some(25),
            care_needs_mode: Some(CareNeedsMode::Full),
        }
    }

    #[must_use]
    pub fn merge(defaults: &Self, override_policy: &Self) -> Self {
        Self {
            mode: override_policy.mode,
            always_on_top: override_policy.always_on_top.or(defaults.always_on_top),
            click_through: override_policy.click_through.or(defaults.click_through),
            edge_snap: override_policy.edge_snap.or(defaults.edge_snap),
            sound_enabled: override_policy.sound_enabled.or(defaults.sound_enabled),
            proactive_frequency: override_policy
                .proactive_frequency
                .or(defaults.proactive_frequency)
                .map(|value| value.min(100)),
            care_needs_mode: override_policy.care_needs_mode.or(defaults.care_needs_mode),
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

impl Profile {
    /// Creates a validated profile with a stable generated identifier.
    ///
    /// # Errors
    ///
    /// Returns [`ProfileError::InvalidName`] when the trimmed name is empty or
    /// longer than 64 Unicode scalar values.
    pub fn new(name: impl Into<String>, policy: ProfilePolicy) -> Result<Self, ProfileError> {
        let profile = Self {
            id: ProfileId::new(),
            name: name.into().trim().to_owned(),
            policy,
        };
        profile.validate()?;
        Ok(profile)
    }

    /// Validates a profile crossing an external persistence boundary.
    ///
    /// # Errors
    ///
    /// Returns a domain error when persisted values violate current invariants.
    pub fn validate(&self) -> Result<(), ProfileError> {
        if self.name.trim().is_empty() || self.name.chars().count() > 64 {
            return Err(ProfileError::InvalidName);
        }
        if self
            .policy
            .proactive_frequency
            .is_some_and(|value| value > 100)
        {
            return Err(ProfileError::InvalidProactiveFrequency);
        }
        Ok(())
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum ProfileError {
    #[error("profile name must contain 1 to 64 characters")]
    InvalidName,
    #[error("profile proactive frequency must be between 0 and 100")]
    InvalidProactiveFrequency,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn override_policy_wins_without_copying_defaults() {
        let defaults = ProfilePolicy {
            mode: ProfileMode::Companion,
            always_on_top: Some(true),
            click_through: Some(false),
            edge_snap: Some(true),
            sound_enabled: Some(true),
            proactive_frequency: Some(25),
            care_needs_mode: Some(CareNeedsMode::Full),
        };
        let overrides = ProfilePolicy {
            mode: ProfileMode::Work,
            always_on_top: None,
            click_through: Some(true),
            edge_snap: None,
            sound_enabled: None,
            proactive_frequency: Some(200),
            care_needs_mode: Some(CareNeedsMode::Simple),
        };
        let merged = ProfilePolicy::merge(&defaults, &overrides);
        assert_eq!(merged.always_on_top, Some(true));
        assert_eq!(merged.mode, ProfileMode::Work);
        assert_eq!(merged.click_through, Some(true));
        assert_eq!(merged.edge_snap, Some(true));
        assert_eq!(merged.proactive_frequency, Some(100));
        assert_eq!(merged.care_needs_mode, Some(CareNeedsMode::Simple));
    }

    #[test]
    fn validates_profile_boundaries() {
        assert_eq!(
            Profile::new(" ", ProfilePolicy::standard()),
            Err(ProfileError::InvalidName)
        );
        let mut policy = ProfilePolicy::standard();
        policy.proactive_frequency = Some(101);
        assert_eq!(
            Profile::new("Work", policy),
            Err(ProfileError::InvalidProactiveFrequency)
        );
    }

    #[test]
    fn legacy_policy_without_care_mode_defaults_to_full_when_resolved() {
        let policy: ProfilePolicy = serde_json::from_value(serde_json::json!({
            "mode": "companion",
            "alwaysOnTop": true,
            "clickThrough": false,
            "edgeSnap": true,
            "soundEnabled": true,
            "proactiveFrequency": 25
        }))
        .expect("legacy profile remains readable");
        let resolved = ProfilePolicy::merge(&ProfilePolicy::standard(), &policy);
        assert_eq!(resolved.care_needs_mode, Some(CareNeedsMode::Full));
    }
}
