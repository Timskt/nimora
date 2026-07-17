use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet, VecDeque};
use std::ffi::OsStr;
use std::path::Path;
use thiserror::Error;

pub const SKILL_SPEC: &str = "nimora.skill/1";
const MAX_CAPABILITIES: usize = 64;
const MAX_ACTIVATION_EVENTS: usize = 64;
const MAX_COMMANDS: usize = 128;
const CRASH_WINDOW_MS: u64 = 5 * 60 * 1_000;
const CRASH_QUARANTINE_THRESHOLD: usize = 3;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SkillCapability {
    InvokeAgentTasks,
    InvokeCommands,
    StoreLocalData,
    SubscribeEvents,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillManifest {
    pub spec: String,
    pub id: String,
    pub version: String,
    pub publisher: String,
    pub entrypoint: String,
    #[serde(default)]
    pub capabilities: BTreeSet<SkillCapability>,
    #[serde(default)]
    pub activation_events: BTreeSet<String>,
    #[serde(default)]
    pub contributions: SkillContributions,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillContributions {
    #[serde(default)]
    pub commands: Vec<SkillCommandContribution>,
    #[serde(default)]
    pub agent_tasks: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SkillCommandContribution {
    pub id: String,
    pub title: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ValidatedSkillManifest(SkillManifest);

impl ValidatedSkillManifest {
    #[must_use]
    pub const fn manifest(&self) -> &SkillManifest {
        &self.0
    }

    #[must_use]
    pub fn into_manifest(self) -> SkillManifest {
        self.0
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillGrant {
    pub skill_id: String,
    pub version: String,
    pub capabilities: BTreeSet<SkillCapability>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SkillStatus {
    PermissionRequired,
    Authorized,
    Activated,
    Suspended,
    Crashed,
    Quarantined,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ActiveSkill {
    pub id: String,
    pub version: String,
    pub commands: Vec<SkillCommandContribution>,
    pub can_invoke_agent_tasks: bool,
}

#[derive(Debug, Clone)]
struct SkillRecord {
    manifest: ValidatedSkillManifest,
    grant: Option<SkillGrant>,
    status: SkillStatus,
    crashes: VecDeque<u64>,
}

#[derive(Debug, Default)]
pub struct SkillHost {
    records: BTreeMap<String, SkillRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SkillError {
    #[error("skill manifest spec is unsupported")]
    UnsupportedSpec,
    #[error("skill identity is invalid")]
    InvalidIdentity,
    #[error("skill version is invalid")]
    InvalidVersion,
    #[error("skill entrypoint is invalid")]
    InvalidEntrypoint,
    #[error("skill manifest exceeds contribution limits")]
    LimitExceeded,
    #[error("skill activation event is invalid")]
    InvalidActivationEvent,
    #[error("skill contribution is invalid")]
    InvalidContribution,
    #[error("skill contribution requires an undeclared capability")]
    MissingCapability,
    #[error("skill is already installed")]
    AlreadyInstalled,
    #[error("skill is not installed")]
    NotInstalled,
    #[error("skill grant does not exactly match the installed manifest")]
    GrantMismatch,
    #[error("skill cannot activate from its current state")]
    InvalidLifecycle,
    #[error("skill is quarantined")]
    Quarantined,
    #[error("skill has no active agent task contribution")]
    AgentTasksUnavailable,
}

/// Validates a Skill package manifest before installation or authorization.
///
/// # Errors
///
/// Returns a stable error when identity, limits, activation, contribution, or capability rules fail.
pub fn validate_manifest(manifest: SkillManifest) -> Result<ValidatedSkillManifest, SkillError> {
    if manifest.spec != SKILL_SPEC {
        return Err(SkillError::UnsupportedSpec);
    }
    if !valid_qualified_id(&manifest.id) || !valid_qualified_id(&manifest.publisher) {
        return Err(SkillError::InvalidIdentity);
    }
    if !valid_version(&manifest.version) {
        return Err(SkillError::InvalidVersion);
    }
    if !valid_entrypoint(&manifest.entrypoint) {
        return Err(SkillError::InvalidEntrypoint);
    }
    if manifest.capabilities.len() > MAX_CAPABILITIES
        || manifest.activation_events.len() > MAX_ACTIVATION_EVENTS
        || manifest.contributions.commands.len() > MAX_COMMANDS
    {
        return Err(SkillError::LimitExceeded);
    }
    if manifest
        .activation_events
        .iter()
        .any(|event| !valid_activation_event(event, &manifest.id))
    {
        return Err(SkillError::InvalidActivationEvent);
    }
    let mut command_ids = BTreeSet::new();
    for command in &manifest.contributions.commands {
        if !command.id.starts_with(&format!("{}.", manifest.id))
            || !valid_qualified_id(&command.id)
            || command.title.trim().is_empty()
            || command.title.len() > 128
            || !command_ids.insert(command.id.as_str())
        {
            return Err(SkillError::InvalidContribution);
        }
    }
    if (!manifest.contributions.commands.is_empty()
        && !manifest
            .capabilities
            .contains(&SkillCapability::InvokeCommands))
        || (manifest.contributions.agent_tasks
            && !manifest
                .capabilities
                .contains(&SkillCapability::InvokeAgentTasks))
    {
        return Err(SkillError::MissingCapability);
    }
    Ok(ValidatedSkillManifest(manifest))
}

impl SkillHost {
    /// Registers one validated, inactive Skill.
    ///
    /// # Errors
    ///
    /// Returns an error when the identity is already installed.
    pub fn install(&mut self, manifest: ValidatedSkillManifest) -> Result<(), SkillError> {
        let id = manifest.manifest().id.clone();
        if self.records.contains_key(&id) {
            return Err(SkillError::AlreadyInstalled);
        }
        self.records.insert(
            id,
            SkillRecord {
                status: if manifest.manifest().capabilities.is_empty() {
                    SkillStatus::Authorized
                } else {
                    SkillStatus::PermissionRequired
                },
                manifest,
                grant: None,
                crashes: VecDeque::new(),
            },
        );
        Ok(())
    }

    /// Applies an exact-version, exact-capability grant.
    ///
    /// # Errors
    ///
    /// Returns an error when identity, version, or capabilities differ from the installed manifest.
    pub fn authorize(&mut self, grant: SkillGrant) -> Result<(), SkillError> {
        let record = self
            .records
            .get_mut(&grant.skill_id)
            .ok_or(SkillError::NotInstalled)?;
        let manifest = record.manifest.manifest();
        if grant.version != manifest.version || grant.capabilities != manifest.capabilities {
            return Err(SkillError::GrantMismatch);
        }
        record.grant = Some(grant);
        record.status = SkillStatus::Authorized;
        Ok(())
    }

    /// Activates contributions only after exact authorization.
    ///
    /// # Errors
    ///
    /// Returns an error for missing authorization, invalid lifecycle, or quarantine.
    pub fn activate(&mut self, skill_id: &str) -> Result<ActiveSkill, SkillError> {
        let record = self
            .records
            .get_mut(skill_id)
            .ok_or(SkillError::NotInstalled)?;
        if record.status == SkillStatus::Quarantined {
            return Err(SkillError::Quarantined);
        }
        if !matches!(
            record.status,
            SkillStatus::Authorized | SkillStatus::Suspended
        ) {
            return Err(SkillError::InvalidLifecycle);
        }
        let manifest = record.manifest.manifest();
        if !manifest.capabilities.is_empty() && record.grant.is_none() {
            return Err(SkillError::InvalidLifecycle);
        }
        record.status = SkillStatus::Activated;
        Ok(active_skill(record))
    }

    /// Suspends a Skill and atomically removes all active contributions.
    ///
    /// # Errors
    ///
    /// Returns an error unless the Skill is currently activated.
    pub fn suspend(&mut self, skill_id: &str) -> Result<(), SkillError> {
        let record = self
            .records
            .get_mut(skill_id)
            .ok_or(SkillError::NotInstalled)?;
        if record.status != SkillStatus::Activated {
            return Err(SkillError::InvalidLifecycle);
        }
        record.status = SkillStatus::Suspended;
        Ok(())
    }

    /// Records a host crash and quarantines repeated failures within five minutes.
    ///
    /// # Errors
    ///
    /// Returns an error unless the Skill was active or already crashed.
    pub fn record_crash(&mut self, skill_id: &str, now_ms: u64) -> Result<SkillStatus, SkillError> {
        let record = self
            .records
            .get_mut(skill_id)
            .ok_or(SkillError::NotInstalled)?;
        if !matches!(record.status, SkillStatus::Activated | SkillStatus::Crashed) {
            return Err(SkillError::InvalidLifecycle);
        }
        while record
            .crashes
            .front()
            .is_some_and(|occurred| now_ms.saturating_sub(*occurred) > CRASH_WINDOW_MS)
        {
            record.crashes.pop_front();
        }
        record.crashes.push_back(now_ms);
        record.status = if record.crashes.len() >= CRASH_QUARANTINE_THRESHOLD {
            SkillStatus::Quarantined
        } else {
            SkillStatus::Crashed
        };
        Ok(record.status)
    }

    /// Makes one non-quarantined crashed Skill eligible for an explicit restart.
    ///
    /// # Errors
    ///
    /// Returns an error unless the Skill is currently crashed.
    pub fn recover_crashed(&mut self, skill_id: &str) -> Result<(), SkillError> {
        let record = self
            .records
            .get_mut(skill_id)
            .ok_or(SkillError::NotInstalled)?;
        if record.status != SkillStatus::Crashed {
            return Err(SkillError::InvalidLifecycle);
        }
        record.status = SkillStatus::Authorized;
        Ok(())
    }

    /// Clears quarantine after an explicit user recovery action.
    ///
    /// # Errors
    ///
    /// Returns an error unless the Skill is quarantined.
    pub fn reset_quarantine(&mut self, skill_id: &str) -> Result<(), SkillError> {
        let record = self
            .records
            .get_mut(skill_id)
            .ok_or(SkillError::NotInstalled)?;
        if record.status != SkillStatus::Quarantined {
            return Err(SkillError::InvalidLifecycle);
        }
        record.crashes.clear();
        record.status =
            if record.manifest.manifest().capabilities.is_empty() || record.grant.is_some() {
                SkillStatus::Authorized
            } else {
                SkillStatus::PermissionRequired
            };
        Ok(())
    }

    /// Removes an inactive Skill and all persisted lifecycle state.
    ///
    /// # Errors
    ///
    /// Returns an error when the Skill is active or not installed.
    pub fn uninstall(&mut self, skill_id: &str) -> Result<(), SkillError> {
        let status = self.status(skill_id).ok_or(SkillError::NotInstalled)?;
        if status == SkillStatus::Activated {
            return Err(SkillError::InvalidLifecycle);
        }
        self.records.remove(skill_id);
        Ok(())
    }

    /// Returns the contribution snapshot currently visible to host registries.
    #[must_use]
    pub fn active_contributions(&self) -> Vec<ActiveSkill> {
        self.records
            .values()
            .filter(|record| record.status == SkillStatus::Activated)
            .map(active_skill)
            .collect()
    }

    /// Issues a trusted module requester identity for the shared Agent adapter.
    ///
    /// # Errors
    ///
    /// Returns an error unless the Skill is active and explicitly contributes Agent tasks.
    pub fn module_agent_identity(&self, skill_id: &str) -> Result<String, SkillError> {
        let record = self.records.get(skill_id).ok_or(SkillError::NotInstalled)?;
        if record.status != SkillStatus::Activated
            || !record.manifest.manifest().contributions.agent_tasks
        {
            return Err(SkillError::AgentTasksUnavailable);
        }
        Ok(format!("skill:{skill_id}"))
    }

    #[must_use]
    pub fn status(&self, skill_id: &str) -> Option<SkillStatus> {
        self.records.get(skill_id).map(|record| record.status)
    }
}

fn active_skill(record: &SkillRecord) -> ActiveSkill {
    let manifest = record.manifest.manifest();
    ActiveSkill {
        id: manifest.id.clone(),
        version: manifest.version.clone(),
        commands: manifest.contributions.commands.clone(),
        can_invoke_agent_tasks: manifest.contributions.agent_tasks,
    }
}

fn valid_qualified_id(value: &str) -> bool {
    value.len() <= 128
        && value.split('.').count() >= 2
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.len() <= 63
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn valid_version(value: &str) -> bool {
    let parts = value.split('.').collect::<Vec<_>>();
    parts.len() == 3
        && parts
            .iter()
            .all(|part| !part.is_empty() && part.bytes().all(|byte| byte.is_ascii_digit()))
}

fn valid_entrypoint(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 256
        && !value.starts_with('/')
        && !value.contains('\\')
        && value.split('/').all(|segment| {
            !segment.is_empty() && segment != "." && segment != ".." && !segment.starts_with('.')
        })
        && Path::new(value).extension() == Some(OsStr::new("js"))
}

fn valid_activation_event(value: &str, skill_id: &str) -> bool {
    value == "onStartup"
        || value
            .strip_prefix("onCommand:")
            .is_some_and(|id| id.starts_with(&format!("{skill_id}.")) && valid_qualified_id(id))
        || value
            .strip_prefix("onEvent:")
            .is_some_and(valid_qualified_id)
}

#[cfg(test)]
mod tests {
    use super::{
        SkillCapability, SkillCommandContribution, SkillContributions, SkillError, SkillGrant,
        SkillHost, SkillManifest, SkillStatus, validate_manifest,
    };
    use std::collections::BTreeSet;

    fn manifest() -> SkillManifest {
        SkillManifest {
            spec: "nimora.skill/1".to_owned(),
            id: "studio.example.focus".to_owned(),
            version: "1.0.0".to_owned(),
            publisher: "studio.example".to_owned(),
            entrypoint: "dist/main.js".to_owned(),
            capabilities: BTreeSet::from([
                SkillCapability::InvokeAgentTasks,
                SkillCapability::InvokeCommands,
            ]),
            activation_events: BTreeSet::from([
                "onStartup".to_owned(),
                "onCommand:studio.example.focus.start".to_owned(),
            ]),
            contributions: SkillContributions {
                commands: vec![SkillCommandContribution {
                    id: "studio.example.focus.start".to_owned(),
                    title: "Start focus".to_owned(),
                }],
                agent_tasks: true,
            },
        }
    }

    #[test]
    fn validates_namespaced_contributions_and_required_capabilities() {
        assert!(validate_manifest(manifest()).is_ok());
        let mut invalid = manifest();
        invalid
            .capabilities
            .remove(&SkillCapability::InvokeAgentTasks);
        assert_eq!(
            validate_manifest(invalid),
            Err(SkillError::MissingCapability)
        );
    }

    #[test]
    fn rejects_path_escape_and_foreign_command_activation() {
        let mut invalid = manifest();
        invalid.entrypoint = "../main.js".to_owned();
        assert_eq!(
            validate_manifest(invalid),
            Err(SkillError::InvalidEntrypoint)
        );
        let mut foreign = manifest();
        foreign.activation_events = BTreeSet::from(["onCommand:other.skill.run".to_owned()]);
        assert_eq!(
            validate_manifest(foreign),
            Err(SkillError::InvalidActivationEvent)
        );
    }

    #[test]
    fn requires_exact_grant_before_contributions_activate() {
        let validated = validate_manifest(manifest()).unwrap();
        let mut host = SkillHost::default();
        host.install(validated).unwrap();
        assert_eq!(
            host.status("studio.example.focus"),
            Some(SkillStatus::PermissionRequired)
        );
        let mut capabilities = manifest().capabilities;
        capabilities.remove(&SkillCapability::InvokeAgentTasks);
        assert_eq!(
            host.authorize(SkillGrant {
                skill_id: "studio.example.focus".to_owned(),
                version: "1.0.0".to_owned(),
                capabilities,
            }),
            Err(SkillError::GrantMismatch)
        );
        host.authorize(SkillGrant {
            skill_id: "studio.example.focus".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: manifest().capabilities,
        })
        .unwrap();
        let active = host.activate("studio.example.focus").unwrap();
        assert_eq!(active.commands.len(), 1);
        assert!(active.can_invoke_agent_tasks);
        host.suspend("studio.example.focus").unwrap();
        assert_eq!(
            host.status("studio.example.focus"),
            Some(SkillStatus::Suspended)
        );
    }

    #[test]
    fn quarantines_three_crashes_within_five_minutes() {
        let mut host = SkillHost::default();
        host.install(validate_manifest(manifest()).unwrap())
            .unwrap();
        host.authorize(SkillGrant {
            skill_id: "studio.example.focus".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: manifest().capabilities,
        })
        .unwrap();
        host.activate("studio.example.focus").unwrap();
        assert_eq!(
            host.record_crash("studio.example.focus", 1_000).unwrap(),
            SkillStatus::Crashed
        );
        host.recover_crashed("studio.example.focus").unwrap();
        host.activate("studio.example.focus").unwrap();
        host.record_crash("studio.example.focus", 2_000).unwrap();
        host.recover_crashed("studio.example.focus").unwrap();
        host.activate("studio.example.focus").unwrap();
        assert_eq!(
            host.record_crash("studio.example.focus", 3_000).unwrap(),
            SkillStatus::Quarantined
        );
        assert_eq!(
            host.activate("studio.example.focus"),
            Err(SkillError::Quarantined)
        );
        assert!(host.active_contributions().is_empty());
        assert_eq!(
            host.module_agent_identity("studio.example.focus"),
            Err(SkillError::AgentTasksUnavailable)
        );
        host.reset_quarantine("studio.example.focus").unwrap();
        host.activate("studio.example.focus").unwrap();
        assert_eq!(
            host.module_agent_identity("studio.example.focus").unwrap(),
            "skill:studio.example.focus"
        );
    }

    #[test]
    fn revokes_active_contributions_and_agent_identity_on_suspend() {
        let mut host = SkillHost::default();
        host.install(validate_manifest(manifest()).unwrap())
            .unwrap();
        host.authorize(SkillGrant {
            skill_id: "studio.example.focus".to_owned(),
            version: "1.0.0".to_owned(),
            capabilities: manifest().capabilities,
        })
        .unwrap();
        assert_eq!(
            host.module_agent_identity("studio.example.focus"),
            Err(SkillError::AgentTasksUnavailable)
        );
        host.activate("studio.example.focus").unwrap();
        assert_eq!(host.active_contributions().len(), 1);
        host.suspend("studio.example.focus").unwrap();
        assert!(host.active_contributions().is_empty());
        assert_eq!(
            host.module_agent_identity("studio.example.focus"),
            Err(SkillError::AgentTasksUnavailable)
        );
        host.uninstall("studio.example.focus").unwrap();
        assert_eq!(host.status("studio.example.focus"), None);
    }
}
