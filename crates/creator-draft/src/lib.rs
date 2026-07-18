use std::collections::BTreeSet;
use std::path::{Component, Path};

use nimora_asset_installer::{GeneratedThemeMetadata, validate_generated_theme_metadata};
use nimora_automation_runtime::{AutomationDefinition, AutomationEngine};
use nimora_runtime_core::ProfilePolicy;
use nimora_skill_runtime::{SkillManifest, validate_manifest as validate_skill_manifest};
use nimora_user_code_policy::{ProgramManifest, evaluate as evaluate_program_manifest};
use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CREATOR_DRAFT_SPEC: &str = "nimora.creator-draft/1";
pub const CAPABILITY_GAP_SPEC: &str = "nimora.capability-gap/1";
const MAX_REQUIREMENT_BYTES: usize = 16 * 1024;
const MAX_TITLE_BYTES: usize = 128;
const MAX_SUMMARY_BYTES: usize = 2 * 1024;
const MAX_PERMISSION_REASON_BYTES: usize = 512;
const MAX_GAP_ITEMS: usize = 16;
const MAX_GAP_OPERATIONS: usize = 16;
const MAX_GAP_ALTERNATIVES: usize = 8;
const MAX_GAP_SEMANTIC_ITEMS: usize = 32;
const MAX_FILES: usize = 32;
const MAX_FILE_BYTES: usize = 256 * 1024;
const MAX_TOTAL_FILE_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CreatorArtifactKind {
    UserProgram,
    Skill,
    Automation,
    Theme,
    Profile,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratedProfile {
    pub name: String,
    pub policy: ProfilePolicy,
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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityGapItem {
    pub capability: String,
    pub reason: String,
    pub required_operations: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityGapAlternative {
    pub kind: CreatorArtifactKind,
    pub title: String,
    pub tradeoff: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityGap {
    pub spec: String,
    pub title: String,
    pub summary: String,
    pub requested_outcome: String,
    pub missing_capabilities: Vec<CapabilityGapItem>,
    pub available_semantic_inputs: Vec<String>,
    pub required_semantic_outputs: Vec<String>,
    pub closest_alternatives: Vec<CapabilityGapAlternative>,
    pub platform_proposal_required: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum CreatorProposal {
    Draft(Box<CreatorDraft>),
    CapabilityGap(CapabilityGap),
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
    Theme {
        metadata: GeneratedThemeMetadata,
    },
    Profile {
        profile: GeneratedProfile,
    },
}

impl CreatorArtifact {
    #[must_use]
    pub const fn kind(&self) -> CreatorArtifactKind {
        match self {
            Self::UserProgram { .. } => CreatorArtifactKind::UserProgram,
            Self::Skill { .. } => CreatorArtifactKind::Skill,
            Self::Automation { .. } => CreatorArtifactKind::Automation,
            Self::Theme { .. } => CreatorArtifactKind::Theme,
            Self::Profile { .. } => CreatorArtifactKind::Profile,
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
    #[error("creator capability gap is invalid")]
    InvalidCapabilityGap,
}

/// Builds the trusted instruction supplied separately from user requirements.
#[must_use]
pub fn creator_system_instruction(
    kind: CreatorArtifactKind,
    catalog_snapshot: &str,
    composition_graph_snapshot: &str,
) -> String {
    format!(
        "You generate a Nimora {} draft. Return exactly one JSON object. The trusted, read-only Capability Catalog Snapshot is: {catalog_snapshot}. The trusted, implementation-free Semantic Composition Graph is: {composition_graph_snapshot}. Treat only exact IDs in these snapshots as registered facts. When the outcome is fully expressible, use spec '{CREATOR_DRAFT_SPEC}' with title, summary, permissionExplanations, and artifact. The exact artifact contract is: {}. Otherwise use spec '{CAPABILITY_GAP_SPEC}' with title, summary, requestedOutcome, missingCapabilities, availableSemanticInputs, requiredSemanticOutputs, closestAlternatives, and platformProposalRequired. Semantic arrays are bounded candidate mappings, not proof: use only lowercase namespaced semantic IDs represented by the graph or directly inherent in the user's request; never claim preconditions are satisfied. Every missing capability ID must describe one precise namespaced operation absent from the Catalog; never report a present ID as missing or invent commands, APIs, or executable fallback code. Do not return Markdown, paths outside the draft, secrets, network instructions, package-manager commands, or prose outside JSON. A draft artifact kind must be '{}'. Every declared capability must have exactly one permission explanation and no undeclared capability may be explained. Generated source may only use the Nimora sandbox API; it must not access Node, Tauri, process, filesystem, network, databases, or provider objects.",
        artifact_kind_name(kind),
        artifact_contract(kind),
        artifact_kind_name(kind)
    )
}

/// Parses either an installable draft or a non-executable capability gap.
///
/// # Errors
///
/// Returns a stable error for malformed, oversized, or contract-invalid output.
pub fn parse_creator_proposal(
    request: &CreatorDraftRequest,
    model_output: &str,
) -> Result<CreatorProposal, CreatorDraftError> {
    validate_model_output_envelope(request, model_output)?;
    let value = serde_json::from_str::<serde_json::Value>(model_output)
        .map_err(|_| CreatorDraftError::InvalidJson)?;
    match value.get("spec").and_then(serde_json::Value::as_str) {
        Some(CREATOR_DRAFT_SPEC) => {
            let draft = serde_json::from_value::<CreatorDraft>(value)
                .map_err(|_| CreatorDraftError::InvalidJson)?;
            validate_creator_draft(request, &draft)?;
            Ok(CreatorProposal::Draft(Box::new(draft)))
        }
        Some(CAPABILITY_GAP_SPEC) => {
            let gap = serde_json::from_value::<CapabilityGap>(value)
                .map_err(|_| CreatorDraftError::InvalidJson)?;
            validate_capability_gap(&gap)?;
            Ok(CreatorProposal::CapabilityGap(gap))
        }
        _ => Err(CreatorDraftError::InvalidJson),
    }
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
    validate_model_output_envelope(request, model_output)?;
    let draft = serde_json::from_str::<CreatorDraft>(model_output)
        .map_err(|_| CreatorDraftError::InvalidJson)?;
    validate_creator_draft(request, &draft)?;
    Ok(draft)
}

fn validate_model_output_envelope(
    request: &CreatorDraftRequest,
    model_output: &str,
) -> Result<(), CreatorDraftError> {
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
    Ok(())
}

/// Revalidates a structured capability gap received across a trust boundary.
///
/// # Errors
///
/// Rejects empty, oversized, duplicate, control-bearing, or executable-looking entries.
pub fn validate_capability_gap(gap: &CapabilityGap) -> Result<(), CreatorDraftError> {
    if gap.spec != CAPABILITY_GAP_SPEC
        || validate_text(&gap.title, MAX_TITLE_BYTES).is_err()
        || validate_text(&gap.summary, MAX_SUMMARY_BYTES).is_err()
        || validate_text(&gap.requested_outcome, MAX_SUMMARY_BYTES).is_err()
        || gap.missing_capabilities.is_empty()
        || gap.missing_capabilities.len() > MAX_GAP_ITEMS
        || gap.closest_alternatives.len() > MAX_GAP_ALTERNATIVES
        || gap.required_semantic_outputs.is_empty()
        || gap.available_semantic_inputs.len() > MAX_GAP_SEMANTIC_ITEMS
        || gap.required_semantic_outputs.len() > MAX_GAP_SEMANTIC_ITEMS
    {
        return Err(CreatorDraftError::InvalidCapabilityGap);
    }
    for semantic_items in [
        &gap.available_semantic_inputs,
        &gap.required_semantic_outputs,
    ] {
        if semantic_items.windows(2).any(|pair| pair[0] >= pair[1])
            || semantic_items
                .iter()
                .any(|item| !valid_capability_name(item))
        {
            return Err(CreatorDraftError::InvalidCapabilityGap);
        }
    }
    let mut capabilities = BTreeSet::new();
    for item in &gap.missing_capabilities {
        if !valid_capability_name(&item.capability)
            || !capabilities.insert(item.capability.as_str())
            || validate_text(&item.reason, MAX_PERMISSION_REASON_BYTES).is_err()
            || item.required_operations.is_empty()
            || item.required_operations.len() > MAX_GAP_OPERATIONS
        {
            return Err(CreatorDraftError::InvalidCapabilityGap);
        }
        let mut operations = BTreeSet::new();
        for operation in &item.required_operations {
            if validate_text(operation, MAX_PERMISSION_REASON_BYTES).is_err()
                || !operations.insert(operation.as_str())
            {
                return Err(CreatorDraftError::InvalidCapabilityGap);
            }
        }
    }
    for alternative in &gap.closest_alternatives {
        if validate_text(&alternative.title, MAX_TITLE_BYTES).is_err()
            || validate_text(&alternative.tradeoff, MAX_PERMISSION_REASON_BYTES).is_err()
        {
            return Err(CreatorDraftError::InvalidCapabilityGap);
        }
    }
    Ok(())
}

fn valid_capability_name(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').count() >= 2
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
        })
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
        CreatorArtifact::Theme { metadata } => {
            validate_generated_theme_metadata(metadata)
                .map_err(|_| CreatorDraftError::InvalidArtifact)?;
            BTreeSet::new()
        }
        CreatorArtifact::Profile { profile } => {
            validate_generated_profile(profile)?;
            BTreeSet::new()
        }
    };
    validate_permission_explanations(&draft.permission_explanations, &required_capabilities)
}

fn validate_generated_profile(profile: &GeneratedProfile) -> Result<(), CreatorDraftError> {
    if profile.name.trim().is_empty()
        || profile.name.chars().count() > 64
        || profile.name.chars().any(char::is_control)
        || profile
            .policy
            .proactive_frequency
            .is_some_and(|value| value > 100)
    {
        return Err(CreatorDraftError::InvalidArtifact);
    }
    Ok(())
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
        CreatorArtifactKind::Theme => "theme",
        CreatorArtifactKind::Profile => "profile",
    }
}

const fn artifact_contract(kind: CreatorArtifactKind) -> &'static str {
    match kind {
        CreatorArtifactKind::UserProgram => {
            "artifact={kind:'user-program',manifest:<nimora.program/1>,files:[{path,source}]}"
        }
        CreatorArtifactKind::Skill => {
            "artifact={kind:'skill',manifest:<nimora.skill/1>,files:[{path,source}]}"
        }
        CreatorArtifactKind::Automation => {
            "artifact={kind:'automation',definition:<nimora.automation/1>}"
        }
        CreatorArtifactKind::Theme => {
            "artifact={kind:'theme',metadata:{id:'theme.local.<name>',version:<semver>,name:<locale-to-name>,publisher:<namespaced-id>,license:<SPDX-or-LicenseRef>,theme:{spec:'nimora.theme/1',mode:'light'|'dark',colors:{surface,surfaceElevated,text,textMuted,accent,accentSoft,border,success,danger},cornerStyle:'soft'|'rounded'|'compact',motion:'full'|'reduced'}}}; colors must be #RRGGBB or #RRGGBBAA and permissionExplanations must be []"
        }
        CreatorArtifactKind::Profile => {
            "artifact={kind:'profile',profile:{name:'1-64 character display name',policy:{mode:'companion'|'work'|'focus'|'creator'|'developer'|'presentation'|'offline',alwaysOnTop:boolean|null,clickThrough:boolean|null,soundEnabled:boolean|null,proactiveFrequency:integer 0..100|null}}}; permissionExplanations must be []; host creates the UUID and does not activate the profile"
        }
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
                    "policy": { "timeoutMs": 5000, "failure": "stop", "maxConcurrentRuns": 1, "cooldownMs": 0, "dailyCostBudgetMicrounits": 0 }
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

    #[test]
    fn theme_draft_reuses_asset_accessibility_and_namespace_contracts() {
        let value = json!({
            "spec": CREATOR_DRAFT_SPEC,
            "title": "Aurora theme",
            "summary": "Creates a calm accessible local theme.",
            "permissionExplanations": [],
            "artifact": {
                "kind": "theme",
                "metadata": {
                    "id": "theme.local.aurora",
                    "version": "1.0.0",
                    "name": { "zh-CN": "极光" },
                    "publisher": "publisher.local.user",
                    "license": "LicenseRef-Proprietary",
                    "theme": {
                        "spec": "nimora.theme/1",
                        "mode": "light",
                        "colors": {
                            "surface": "#f7f5ef",
                            "surfaceElevated": "#fffdf8",
                            "text": "#30322c",
                            "textMuted": "#77786f",
                            "accent": "#6f61ce",
                            "accentSoft": "#eeeaff",
                            "border": "#deddd6",
                            "success": "#5f875b",
                            "danger": "#a44f45"
                        },
                        "cornerStyle": "soft",
                        "motion": "full"
                    }
                }
            }
        });
        let output = serde_json::to_string(&value).expect("fixture");
        assert!(parse_creator_draft(&request(CreatorArtifactKind::Theme), &output).is_ok());

        let mut inaccessible = value;
        inaccessible["artifact"]["metadata"]["theme"]["colors"]["text"] = json!("#f7f5ef");
        assert!(
            parse_creator_draft(
                &request(CreatorArtifactKind::Theme),
                &serde_json::to_string(&inaccessible).expect("fixture")
            )
            .is_err()
        );
    }

    #[test]
    fn profile_draft_accepts_only_bounded_host_owned_policy() {
        let value = json!({
            "spec": CREATOR_DRAFT_SPEC,
            "title": "Deep focus",
            "summary": "Creates a quiet focus profile without activating it.",
            "permissionExplanations": [],
            "artifact": {
                "kind": "profile",
                "profile": {
                    "name": "深度专注",
                    "policy": {
                        "mode": "focus",
                        "alwaysOnTop": true,
                        "clickThrough": false,
                        "soundEnabled": false,
                        "proactiveFrequency": 5
                    }
                }
            }
        });
        let output = serde_json::to_string(&value).expect("fixture");
        assert!(parse_creator_draft(&request(CreatorArtifactKind::Profile), &output).is_ok());

        let mut invalid = value;
        invalid["artifact"]["profile"]["policy"]["proactiveFrequency"] = json!(101);
        let output = serde_json::to_string(&invalid).expect("fixture");
        assert!(parse_creator_draft(&request(CreatorArtifactKind::Profile), &output).is_err());
    }

    #[test]
    fn parses_a_bounded_non_executable_capability_gap() {
        let output = serde_json::to_string(&json!({
            "spec": CAPABILITY_GAP_SPEC,
            "title": "Missing camera observation capability",
            "summary": "The selected artifact cannot observe camera frames through the current Registry.",
            "requestedOutcome": "React when a user-approved gesture is observed.",
            "missingCapabilities": [{
                "capability": "perception.camera.observe",
                "reason": "No registered Creator capability exposes consent-bound camera observations.",
                "requiredOperations": ["Observe a bounded, user-approved gesture event without retaining frames."]
            }],
            "availableSemanticInputs": ["perception.gesture-request"],
            "requiredSemanticOutputs": ["perception.gesture-event"],
            "closestAlternatives": [{
                "kind": "automation",
                "title": "Use a manual gesture command",
                "tradeoff": "Requires the user to trigger the interaction explicitly."
            }],
            "platformProposalRequired": true
        }))
        .expect("gap");

        let proposal = parse_creator_proposal(&request(CreatorArtifactKind::Automation), &output)
            .expect("proposal");
        assert!(matches!(proposal, CreatorProposal::CapabilityGap(_)));
        assert_eq!(
            parse_creator_draft(&request(CreatorArtifactKind::Automation), &output),
            Err(CreatorDraftError::InvalidJson)
        );
    }

    #[test]
    fn capability_gap_rejects_unknown_fields_duplicates_and_command_like_ids() {
        let base = json!({
            "spec": CAPABILITY_GAP_SPEC,
            "title": "Missing capability",
            "summary": "The Registry cannot express the requested behavior.",
            "requestedOutcome": "Run an unsupported operation.",
            "missingCapabilities": [{
                "capability": "device.robot.control",
                "reason": "No registered device adapter exists.",
                "requiredOperations": ["Send a bounded simulated motion request."]
            }],
            "availableSemanticInputs": ["device.motion-request"],
            "requiredSemanticOutputs": ["device.motion-result"],
            "closestAlternatives": [],
            "platformProposalRequired": true
        });
        for invalid in [
            {
                let mut value = base.clone();
                value["command"] = json!("shell.exec");
                value
            },
            {
                let mut value = base.clone();
                value["missingCapabilities"][0]["capability"] = json!("shell exec");
                value
            },
            {
                let mut value = base.clone();
                value["missingCapabilities"] = json!([
                    value["missingCapabilities"][0].clone(),
                    value["missingCapabilities"][0].clone()
                ]);
                value
            },
        ] {
            let output = serde_json::to_string(&invalid).expect("invalid gap");
            assert!(parse_creator_proposal(&request(CreatorArtifactKind::Skill), &output).is_err());
        }
    }
}
