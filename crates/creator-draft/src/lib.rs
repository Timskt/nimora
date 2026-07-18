use std::collections::BTreeSet;
use std::path::{Component, Path};

use nimora_automation_runtime::{AutomationDefinition, AutomationEngine};
use nimora_skill_runtime::{SkillManifest, validate_manifest as validate_skill_manifest};
use nimora_user_code_policy::{ProgramManifest, evaluate as evaluate_program_manifest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CREATOR_DRAFT_SPEC: &str = "nimora.creator-draft/1";
const MAX_REQUIREMENT_BYTES: usize = 16 * 1024;
const MAX_TITLE_BYTES: usize = 128;
const MAX_SUMMARY_BYTES: usize = 2 * 1024;
const MAX_PERMISSION_REASON_BYTES: usize = 512;
const MAX_FILES: usize = 32;
const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_TOTAL_FILE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CreatorArtifactKind {
    UserProgram,
    Skill,
    Automation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatorDraftRequest {
    pub spec: String,
    pub kind: CreatorArtifactKind,
    pub requirement: String,
}

impl CreatorDraftRequest {
    /// Creates a bounded natural-language request for an AI draft.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, or control-character-bearing requirements.
    pub fn new(
        kind: CreatorArtifactKind,
        requirement: impl Into<String>,
    ) -> Result<Self, CreatorDraftError> {
        let requirement = requirement.into();
        validate_text(&requirement, MAX_REQUIREMENT_BYTES)?;
        Ok(Self {
            spec: CREATOR_DRAFT_SPEC.to_owned(),
            kind,
            requirement,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PermissionExplanation {
    pub capability: String,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatorDraftFile {
    pub path: String,
    pub source: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CreatorDraft {
    pub spec: String,
    pub title: String,
    pub summary: String,
    pub permission_explanations: Vec<PermissionExplanation>,
    pub artifact: CreatorArtifact,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case", deny_unknown_fields)]
pub enum CreatorArtifact {
    UserProgram {
        manifest: ProgramManifest,
        files: Vec<CreatorDraftFile>,
    },
    Skill {
        manifest: SkillManifest,
        files: Vec<CreatorDraftFile>,
    },
    Automation {
        definition: AutomationDefinition,
    },
}

impl CreatorArtifact {
    #[must_use]
    pub const fn kind(&self) -> CreatorArtifactKind {
        match self {
            Self::UserProgram { .. } => CreatorArtifactKind::UserProgram,
            Self::Skill { .. } => CreatorArtifactKind::Skill,
            Self::Automation { .. } => CreatorArtifactKind::Automation,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CreatorDraftError {
    #[error("creator draft request is invalid")]
    InvalidRequest,
    #[error("creator draft output is not strict JSON")]
    InvalidJson,
    #[error("creator draft metadata is invalid")]
    InvalidMetadata,
    #[error("creator draft kind does not match the request")]
    KindMismatch,
    #[error("creator draft permissions do not exactly match the artifact")]
    PermissionMismatch,
    #[error("creator draft files are unsafe or exceed the budget")]
    InvalidFiles,
    #[error("creator draft artifact failed its production contract")]
    InvalidArtifact,
}

/// Builds the trusted instruction supplied separately from user requirements.
#[must_use]
pub fn creator_system_instruction(kind: CreatorArtifactKind) -> String {
    format!(
        "You generate a Nimora {} draft. Return exactly one JSON object with spec '{CREATOR_DRAFT_SPEC}', title, summary, permissionExplanations, and artifact. Do not return Markdown, commands, paths outside the draft, secrets, network instructions, package-manager commands, or prose outside JSON. The artifact kind must be '{}'. Every declared capability must have exactly one permission explanation and no undeclared capability may be explained. Generated source may only use the Nimora sandbox API; it must not access Node, Tauri, process, filesystem, network, databases, or provider objects.",
        artifact_kind_name(kind),
        artifact_kind_name(kind)
    )
}

/// Parses untrusted model output and validates it against production contracts.
///
/// # Errors
///
/// Returns a stable error without including model output or parser details.
pub fn parse_creator_draft(
    request: &CreatorDraftRequest,
    model_output: &str,
) -> Result<CreatorDraft, CreatorDraftError> {
    if request.spec != CREATOR_DRAFT_SPEC {
        return Err(CreatorDraftError::InvalidRequest);
    }
    validate_text(&request.requirement, MAX_REQUIREMENT_BYTES)?;
    if model_output.len() > MAX_TOTAL_FILE_BYTES + 128 * 1024
        || model_output.trim() != model_output
        || !model_output.starts_with('{')
        || !model_output.ends_with('}')
    {
        return Err(CreatorDraftError::InvalidJson);
    }
    let draft = serde_json::from_str::<CreatorDraft>(model_output)
        .map_err(|_| CreatorDraftError::InvalidJson)?;
    validate_creator_draft(request, &draft)?;
    Ok(draft)
}

/// Revalidates a structured draft received across a trust boundary.
///
/// # Errors
///
/// Rejects metadata, artifact, file, or permission-contract drift.
pub fn validate_creator_draft(
    request: &CreatorDraftRequest,
    draft: &CreatorDraft,
) -> Result<(), CreatorDraftError> {
    if draft.spec != CREATOR_DRAFT_SPEC
        || validate_text(&draft.title, MAX_TITLE_BYTES).is_err()
        || validate_text(&draft.summary, MAX_SUMMARY_BYTES).is_err()
    {
        return Err(CreatorDraftError::InvalidMetadata);
    }
    if draft.artifact.kind() != request.kind {
        return Err(CreatorDraftError::KindMismatch);
    }

    let required_capabilities = match &draft.artifact {
        CreatorArtifact::UserProgram { manifest, files } => {
            evaluate_program_manifest(manifest.clone())
                .map_err(|_| CreatorDraftError::InvalidArtifact)?;
            validate_files(files, "main.js")?;
            serialized_names(&manifest.capabilities)?
        }
        CreatorArtifact::Skill { manifest, files } => {
            validate_skill_manifest(manifest.clone())
                .map_err(|_| CreatorDraftError::InvalidArtifact)?;
            validate_files(files, &manifest.entrypoint)?;
            serialized_names(&manifest.capabilities)?
        }
        CreatorArtifact::Automation { definition } => {
            AutomationEngine::validate(definition)
                .map_err(|_| CreatorDraftError::InvalidArtifact)?;
            BTreeSet::new()
        }
    };
    validate_permission_explanations(&draft.permission_explanations, &required_capabilities)
}

fn serialized_names<'a, T: Serialize + 'a>(
    values: impl IntoIterator<Item = &'a T>,
) -> Result<BTreeSet<String>, CreatorDraftError> {
    values
        .into_iter()
        .map(|value| {
            serde_json::to_value(value)
                .ok()
                .and_then(|value| value.as_str().map(ToOwned::to_owned))
                .ok_or(CreatorDraftError::InvalidArtifact)
        })
        .collect()
}

fn validate_permission_explanations(
    explanations: &[PermissionExplanation],
    required: &BTreeSet<String>,
) -> Result<(), CreatorDraftError> {
    let mut explained = BTreeSet::new();
    for explanation in explanations {
        if !required.contains(&explanation.capability)
            || !explained.insert(explanation.capability.clone())
            || validate_text(&explanation.reason, MAX_PERMISSION_REASON_BYTES).is_err()
        {
            return Err(CreatorDraftError::PermissionMismatch);
        }
    }
    if &explained != required {
        return Err(CreatorDraftError::PermissionMismatch);
    }
    Ok(())
}

fn validate_files(
    files: &[CreatorDraftFile],
    required_entrypoint: &str,
) -> Result<(), CreatorDraftError> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(CreatorDraftError::InvalidFiles);
    }
    let mut paths = BTreeSet::new();
    let mut total_bytes = 0usize;
    for file in files {
        let path = Path::new(&file.path);
        if file.path.is_empty()
            || file.path.contains('\\')
            || path.is_absolute()
            || path
                .components()
                .any(|component| !matches!(component, Component::Normal(_)))
            || !paths.insert(file.path.as_str())
            || file.source.is_empty()
            || file.source.len() > MAX_FILE_BYTES
            || file.source.contains('\0')
        {
            return Err(CreatorDraftError::InvalidFiles);
        }
        total_bytes = total_bytes.saturating_add(file.source.len());
    }
    if total_bytes > MAX_TOTAL_FILE_BYTES || !paths.contains(required_entrypoint) {
        return Err(CreatorDraftError::InvalidFiles);
    }
    Ok(())
}

fn validate_text(value: &str, max_bytes: usize) -> Result<(), CreatorDraftError> {
    if value.trim().is_empty()
        || value.len() > max_bytes
        || value
            .chars()
            .any(|character| character.is_control() && character != '\n')
    {
        return Err(CreatorDraftError::InvalidRequest);
    }
    Ok(())
}

const fn artifact_kind_name(kind: CreatorArtifactKind) -> &'static str {
    match kind {
        CreatorArtifactKind::UserProgram => "user-program",
        CreatorArtifactKind::Skill => "skill",
        CreatorArtifactKind::Automation => "automation",
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    fn request(kind: CreatorArtifactKind) -> CreatorDraftRequest {
        CreatorDraftRequest::new(kind, "Create a focus reminder").expect("request")
    }

    fn program_json() -> serde_json::Value {
        json!({
            "spec": CREATOR_DRAFT_SPEC,
            "title": "Focus reminder",
            "summary": "Shows a local reminder after a runtime event.",
            "permissionExplanations": [{
                "capability": "subscribe-events",
                "reason": "Receives the explicitly selected focus event."
            }],
            "artifact": {
                "kind": "user-program",
                "manifest": {
                    "id": "studio.local.focus-reminder",
                    "version": "1.0.0",
                    "capabilities": ["subscribe-events"],
                    "subscriptions": ["focus.timer.completed"],
                    "eventConcurrency": "serial",
                    "eventQueueCapacity": 8,
                    "commands": [],
                    "timeoutMs": 5000,
                    "memoryBytes": 8_388_608
                },
                "files": [{ "path": "main.js", "source": "({ agentTasks: [] })" }]
            }
        })
    }

    #[test]
    fn accepts_a_strict_validated_user_program_draft() {
        let output = serde_json::to_string(&program_json()).expect("fixture");
        let draft = parse_creator_draft(&request(CreatorArtifactKind::UserProgram), &output)
            .expect("valid draft");
        assert_eq!(draft.artifact.kind(), CreatorArtifactKind::UserProgram);
    }

    #[test]
    fn rejects_markdown_wrapped_or_mismatched_output() {
        let output = serde_json::to_string(&program_json()).expect("fixture");
        assert_eq!(
            parse_creator_draft(
                &request(CreatorArtifactKind::UserProgram),
                &format!("```json\n{output}\n```")
            ),
            Err(CreatorDraftError::InvalidJson)
        );
        assert_eq!(
            parse_creator_draft(&request(CreatorArtifactKind::Skill), &output),
            Err(CreatorDraftError::KindMismatch)
        );
    }

    #[test]
    fn rejects_missing_extra_or_duplicate_permission_explanations() {
        for explanations in [
            json!([]),
            json!([{
                "capability": "store-local-data",
                "reason": "Not declared."
            }]),
            json!([{
                "capability": "subscribe-events",
                "reason": "First."
            }, {
                "capability": "subscribe-events",
                "reason": "Duplicate."
            }]),
        ] {
            let mut value = program_json();
            value["permissionExplanations"] = explanations;
            let output = serde_json::to_string(&value).expect("fixture");
            assert_eq!(
                parse_creator_draft(&request(CreatorArtifactKind::UserProgram), &output),
                Err(CreatorDraftError::PermissionMismatch)
            );
        }
    }

    #[test]
    fn rejects_path_escape_missing_entrypoint_and_invalid_manifest() {
        for (path, id) in [
            ("../main.js", "studio.local.focus-reminder"),
            ("helper.js", "studio.local.focus-reminder"),
            ("main.js", "invalid"),
        ] {
            let mut value = program_json();
            value["artifact"]["files"][0]["path"] = json!(path);
            value["artifact"]["manifest"]["id"] = json!(id);
            let output = serde_json::to_string(&value).expect("fixture");
            assert!(
                parse_creator_draft(&request(CreatorArtifactKind::UserProgram), &output).is_err()
            );
        }
    }

    #[test]
    fn automation_requires_no_permission_explanations_and_validates_contract() {
        let value = json!({
            "spec": CREATOR_DRAFT_SPEC,
            "title": "Greeting",
            "summary": "Plays a safe greeting action.",
            "permissionExplanations": [],
            "artifact": {
                "kind": "automation",
                "definition": {
                    "spec": "nimora.automation/1",
                    "id": "automation.local.greeting",
                    "version": "1.0.0",
                    "name": "Greeting",
                    "enabled": false,
                    "trigger": { "eventType": "pet.pointer.clicked" },
                    "conditions": [],
                    "actions": [{
                        "id": "greet",
                        "command": "pet.action.play",
                        "arguments": { "action": "pet.click" },
                        "risk": "safe",
                        "retrySafe": false,
                        "idempotencyKey": null,
                        "compensation": null
                    }],
                    "policy": { "timeoutMs": 5000, "failure": "stop" }
                }
            }
        });
        let output = serde_json::to_string(&value).expect("fixture");
        assert!(parse_creator_draft(&request(CreatorArtifactKind::Automation), &output).is_ok());
    }

    #[test]
    fn skill_draft_reuses_manifest_entrypoint_and_permission_contracts() {
        let value = json!({
            "spec": CREATOR_DRAFT_SPEC,
            "title": "Focus skill",
            "summary": "Adds a local focus command.",
            "permissionExplanations": [{
                "capability": "invoke-commands",
                "reason": "Invokes the explicitly allowlisted safe reminder command."
            }],
            "artifact": {
                "kind": "skill",
                "manifest": {
                    "spec": "nimora.skill/1",
                    "id": "studio.local.focus",
                    "version": "1.0.0",
                    "publisher": "publisher.local.user",
                    "entrypoint": "src/main.js",
                    "capabilities": ["invoke-commands"],
                    "activationEvents": [],
                    "commandAllowlist": ["safe.notification.show"],
                    "contributions": {
                        "commands": [{ "id": "studio.local.focus.remind", "title": "Focus reminder" }],
                        "agentTools": [],
                        "agentTasks": false
                    }
                },
                "files": [{ "path": "src/main.js", "source": "({ commands: [] })" }]
            }
        });
        let output = serde_json::to_string(&value).expect("fixture");
        assert!(parse_creator_draft(&request(CreatorArtifactKind::Skill), &output).is_ok());
    }
}
