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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct QuietHours {
    pub enabled: bool,
    pub start_minute: u16,
    pub end_minute: u16,
}

impl QuietHours {
    #[must_use]
    pub const fn contains(self, minute_of_day: u16) -> bool {
        if !self.enabled || self.start_minute == self.end_minute || minute_of_day >= 1_440 {
            return false;
        }
        if self.start_minute < self.end_minute {
            minute_of_day >= self.start_minute && minute_of_day < self.end_minute
        } else {
            minute_of_day >= self.start_minute || minute_of_day < self.end_minute
        }
    }
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
    pub cursor_approach_enabled: Option<bool>,
    #[serde(default)]
    pub care_needs_mode: Option<CareNeedsMode>,
    #[serde(default)]
    pub quiet_hours: Option<QuietHours>,
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
            cursor_approach_enabled: Some(true),
            care_needs_mode: Some(CareNeedsMode::Full),
            quiet_hours: None,
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
            cursor_approach_enabled: override_policy
                .cursor_approach_enabled
                .or(defaults.cursor_approach_enabled),
            care_needs_mode: override_policy.care_needs_mode.or(defaults.care_needs_mode),
            quiet_hours: override_policy.quiet_hours.or(defaults.quiet_hours),
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
        if self.policy.quiet_hours.is_some_and(|quiet| {
            quiet.start_minute >= 1_440
                || quiet.end_minute >= 1_440
                || (quiet.enabled && quiet.start_minute == quiet.end_minute)
        }) {
            return Err(ProfileError::InvalidQuietHours);
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
    #[error("profile quiet-hour minutes must be between 0 and 1439")]
    InvalidQuietHours,
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
            cursor_approach_enabled: Some(true),
            care_needs_mode: Some(CareNeedsMode::Full),
            quiet_hours: None,
        };
        let overrides = ProfilePolicy {
            mode: ProfileMode::Work,
            always_on_top: None,
            click_through: Some(true),
            edge_snap: None,
            sound_enabled: None,
            proactive_frequency: Some(200),
            cursor_approach_enabled: Some(false),
            care_needs_mode: Some(CareNeedsMode::Simple),
            quiet_hours: Some(QuietHours {
                enabled: true,
                start_minute: 1_320,
                end_minute: 420,
            }),
        };
        let merged = ProfilePolicy::merge(&defaults, &overrides);
        assert_eq!(merged.always_on_top, Some(true));
        assert_eq!(merged.mode, ProfileMode::Work);
        assert_eq!(merged.click_through, Some(true));
        assert_eq!(merged.edge_snap, Some(true));
        assert_eq!(merged.proactive_frequency, Some(100));
        assert_eq!(merged.cursor_approach_enabled, Some(false));
        assert_eq!(merged.care_needs_mode, Some(CareNeedsMode::Simple));
        assert_eq!(merged.quiet_hours, overrides.quiet_hours);
    }

    #[test]
    fn quiet_hours_support_daytime_and_cross_midnight_windows() {
        let daytime = QuietHours {
            enabled: true,
            start_minute: 540,
            end_minute: 1_020,
        };
        assert!(daytime.contains(600));
        assert!(!daytime.contains(1_100));
        let overnight = QuietHours {
            enabled: true,
            start_minute: 1_320,
            end_minute: 420,
        };
        assert!(overnight.contains(1_400));
        assert!(overnight.contains(300));
        assert!(!overnight.contains(800));
        assert!(
            !QuietHours {
                enabled: true,
                start_minute: 600,
                end_minute: 600
            }
            .contains(600)
        );
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
        let mut policy = ProfilePolicy::standard();
        policy.quiet_hours = Some(QuietHours {
            enabled: true,
            start_minute: 600,
            end_minute: 600,
        });
        assert_eq!(
            Profile::new("Quiet", policy),
            Err(ProfileError::InvalidQuietHours)
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
        assert_eq!(resolved.cursor_approach_enabled, Some(true));
    }
}
