use std::collections::{BTreeMap, BTreeSet};
use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path};

use nimora_creator_composition::{
    COMPOSITION_PLAN_SPEC, CapabilityCompositionPlan, SEMANTIC_PLAN_SPEC, SemanticCompositionPlan,
};
use nimora_creator_draft::{
    CapabilityGap, CreatorArtifact, CreatorDraft, CreatorDraftError, CreatorDraftFile,
    validate_capability_gap,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

const DRAFTS_DIRECTORY: &str = ".nimora-drafts";
const PROPOSALS_DIRECTORY: &str = ".nimora-proposals";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorDraftSaveReceipt {
    pub spec: &'static str,
    pub artifact_id: String,
    pub relative_directory: String,
    pub files_written: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityGapSaveReceipt {
    pub spec: &'static str,
    pub report_id: String,
    pub relative_file: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProposalReceipt {
    pub spec: &'static str,
    pub proposal_id: String,
    pub relative_file: String,
    pub status: &'static str,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct PersistedCapabilityGapReport<'a> {
    spec: &'static str,
    gap: &'a CapabilityGap,
    composition_plan: &'a CapabilityCompositionPlan,
    semantic_composition_plan: &'a SemanticCompositionPlan,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityProposalStatus {
    PendingReview,
    Accepted,
    Rejected,
    Duplicate,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityProposalReview {
    pub status: CapabilityProposalStatus,
    pub reason: String,
    pub reviewed_at_ms: u64,
    pub duplicate_of_proposal_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityProposalRecord {
    pub spec: String,
    pub proposal_id: String,
    pub status: CapabilityProposalStatus,
    pub submitted_at_ms: u64,
    pub gap: CapabilityGap,
    pub composition_plan: CapabilityCompositionPlan,
    pub semantic_composition_plan: SemanticCompositionPlan,
    pub review: Option<CapabilityProposalReview>,
    pub integrity_digest: String,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct CapabilityProposalIntegrityPayload<'a> {
    spec: &'a str,
    proposal_id: &'a str,
    status: CapabilityProposalStatus,
    submitted_at_ms: u64,
    gap: &'a CapabilityGap,
    composition_plan: &'a CapabilityCompositionPlan,
    semantic_composition_plan: &'a SemanticCompositionPlan,
    review: &'a Option<CapabilityProposalReview>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityProposalTriagePriority {
    Normal,
    Elevated,
    High,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProposalCluster {
    pub cluster_key: String,
    pub occurrence_count: usize,
    pub canonical_proposal_id: String,
    pub related_proposal_ids: Vec<String>,
    pub triage_priority: CapabilityProposalTriagePriority,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CapabilityProposalGovernanceItem {
    pub record: CapabilityProposalRecord,
    pub cluster: CapabilityProposalCluster,
}

#[derive(Debug, Error)]
pub enum CreatorWorkspaceError {
    #[error("creator workspace root is invalid")]
    InvalidRoot,
    #[error("creator artifact identifier is unsafe")]
    InvalidArtifactId,
    #[error("creator draft destination already exists")]
    AlreadyExists,
    #[error("creator workspace contains an unsafe symbolic link")]
    SymbolicLink,
    #[error("creator draft persistence failed")]
    Io,
    #[error("creator capability gap proof is invalid")]
    InvalidGapProof,
    #[error("creator capability proposal is invalid")]
    InvalidProposal,
    #[error(transparent)]
    Draft(#[from] CreatorDraftError),
}

pub fn save_creator_draft(
    workspace_root: &Path,
    draft: &CreatorDraft,
    operation_id: &str,
) -> Result<CreatorDraftSaveReceipt, CreatorWorkspaceError> {
    let root = fs::canonicalize(workspace_root).map_err(|_| CreatorWorkspaceError::InvalidRoot)?;
    if !root.is_dir()
        || fs::symlink_metadata(&root)
            .map_err(|_| CreatorWorkspaceError::InvalidRoot)?
            .file_type()
            .is_symlink()
    {
        return Err(CreatorWorkspaceError::InvalidRoot);
    }
    let artifact_id = artifact_id(draft);
    validate_segment(&artifact_id)?;
    validate_segment(operation_id)?;
    let drafts_root = root.join(DRAFTS_DIRECTORY);
    create_or_verify_directory(&drafts_root)?;
    let destination = drafts_root.join(&artifact_id);
    if destination.exists() {
        return Err(CreatorWorkspaceError::AlreadyExists);
    }
    let staging = drafts_root.join(format!(".{artifact_id}.{operation_id}.staging"));
    fs::create_dir(&staging).map_err(|_| CreatorWorkspaceError::Io)?;
    let result = write_draft(&staging, draft).and_then(|count| {
        fs::rename(&staging, &destination).map_err(|_| CreatorWorkspaceError::Io)?;
        Ok(count)
    });
    if result.is_err() {
        let _ = fs::remove_dir_all(&staging);
    }
    let files_written = result?;
    Ok(CreatorDraftSaveReceipt {
        spec: "nimora.creator-draft-save/1",
        artifact_id: artifact_id.clone(),
        relative_directory: format!("{DRAFTS_DIRECTORY}/{artifact_id}"),
        files_written,
    })
}

pub fn save_capability_gap(
    workspace_root: &Path,
    gap: &CapabilityGap,
    composition_plan: &CapabilityCompositionPlan,
    semantic_composition_plan: &SemanticCompositionPlan,
    operation_id: &str,
) -> Result<CapabilityGapSaveReceipt, CreatorWorkspaceError> {
    validate_gap_proof(gap, composition_plan, semantic_composition_plan)?;
    validate_segment(operation_id)?;
    let root = validated_workspace_root(workspace_root)?;
    let drafts_root = root.join(DRAFTS_DIRECTORY);
    create_or_verify_directory(&drafts_root)?;
    let report_id = format!("capability-gap-{operation_id}");
    let file_name = format!("{report_id}.json");
    let destination = drafts_root.join(&file_name);
    let bytes = serde_json::to_vec_pretty(&PersistedCapabilityGapReport {
        spec: "nimora.persisted-capability-gap/2",
        gap,
        composition_plan,
        semantic_composition_plan,
    })
    .map_err(|_| CreatorWorkspaceError::Io)?;
    write_new(&destination, &bytes)?;
    Ok(CapabilityGapSaveReceipt {
        spec: "nimora.capability-gap-save/1",
        report_id,
        relative_file: format!("{DRAFTS_DIRECTORY}/{file_name}"),
    })
}

pub fn submit_capability_proposal(
    workspace_root: &Path,
    gap: &CapabilityGap,
    composition_plan: &CapabilityCompositionPlan,
    semantic_composition_plan: &SemanticCompositionPlan,
    operation_id: &str,
    submitted_at_ms: u64,
) -> Result<CapabilityProposalReceipt, CreatorWorkspaceError> {
    validate_gap_proof(gap, composition_plan, semantic_composition_plan)?;
    if !gap.platform_proposal_required {
        return Err(CreatorWorkspaceError::InvalidGapProof);
    }
    validate_segment(operation_id)?;
    let root = validated_workspace_root(workspace_root)?;
    let proposals_root = root.join(PROPOSALS_DIRECTORY);
    create_or_verify_directory(&proposals_root)?;
    let proposal_id = format!("capability-proposal-{operation_id}");
    let file_name = format!("{proposal_id}.json");
    let mut record = CapabilityProposalRecord {
        spec: "nimora.capability-proposal/1".to_owned(),
        proposal_id: proposal_id.clone(),
        status: CapabilityProposalStatus::PendingReview,
        submitted_at_ms,
        gap: gap.clone(),
        composition_plan: composition_plan.clone(),
        semantic_composition_plan: semantic_composition_plan.clone(),
        review: None,
        integrity_digest: String::new(),
    };
    record.integrity_digest = capability_proposal_digest(&record)?;
    let bytes = serde_json::to_vec_pretty(&record).map_err(|_| CreatorWorkspaceError::Io)?;
    write_new(&proposals_root.join(&file_name), &bytes)?;
    Ok(CapabilityProposalReceipt {
        spec: "nimora.capability-proposal-receipt/1",
        proposal_id,
        relative_file: format!("{PROPOSALS_DIRECTORY}/{file_name}"),
        status: "pending-review",
    })
}

pub fn list_capability_proposals(
    workspace_root: &Path,
) -> Result<Vec<CapabilityProposalRecord>, CreatorWorkspaceError> {
    let root = validated_workspace_root(workspace_root)?;
    let proposals_root = root.join(PROPOSALS_DIRECTORY);
    if !proposals_root.exists() {
        return Ok(Vec::new());
    }
    create_or_verify_directory(&proposals_root)?;
    let mut records = Vec::new();
    for entry in fs::read_dir(&proposals_root).map_err(|_| CreatorWorkspaceError::Io)? {
        if records.len() >= 256 {
            return Err(CreatorWorkspaceError::InvalidProposal);
        }
        let entry = entry.map_err(|_| CreatorWorkspaceError::Io)?;
        let metadata = entry.metadata().map_err(|_| CreatorWorkspaceError::Io)?;
        if !metadata.is_file()
            || entry
                .file_type()
                .map_err(|_| CreatorWorkspaceError::Io)?
                .is_symlink()
        {
            return Err(CreatorWorkspaceError::InvalidProposal);
        }
        let record = read_capability_proposal(&entry.path())?;
        let expected_file_name = format!("{}.json", record.proposal_id);
        if entry.file_name().to_str() != Some(expected_file_name.as_str()) {
            return Err(CreatorWorkspaceError::InvalidProposal);
        }
        records.push(record);
    }
    let records_by_id = records
        .iter()
        .map(|record| (record.proposal_id.as_str(), record))
        .collect::<BTreeMap<_, _>>();
    for record in &records {
        if let Some(target_id) = record
            .review
            .as_ref()
            .and_then(|review| review.duplicate_of_proposal_id.as_deref())
        {
            let target = records_by_id
                .get(target_id)
                .ok_or(CreatorWorkspaceError::InvalidProposal)?;
            if capability_proposal_cluster_key(record)? != capability_proposal_cluster_key(target)?
            {
                return Err(CreatorWorkspaceError::InvalidProposal);
            }
        }
    }
    records.sort_by(|left, right| {
        right
            .submitted_at_ms
            .cmp(&left.submitted_at_ms)
            .then_with(|| left.proposal_id.cmp(&right.proposal_id))
    });
    Ok(records)
}

pub fn capability_proposal_governance(
    workspace_root: &Path,
) -> Result<Vec<CapabilityProposalGovernanceItem>, CreatorWorkspaceError> {
    let records = list_capability_proposals(workspace_root)?;
    let mut clusters = BTreeMap::<String, Vec<(String, u64)>>::new();
    for record in &records {
        clusters
            .entry(capability_proposal_cluster_key(record)?)
            .or_default()
            .push((record.proposal_id.clone(), record.submitted_at_ms));
    }
    let mut items = Vec::with_capacity(records.len());
    for record in records {
        let cluster_key = capability_proposal_cluster_key(&record)?;
        let mut related = clusters
            .get(&cluster_key)
            .ok_or(CreatorWorkspaceError::InvalidProposal)?
            .iter()
            .map(|(proposal_id, _)| proposal_id.clone())
            .collect::<Vec<_>>();
        related.sort();
        let occurrence_count = related.len();
        let canonical_proposal_id = clusters[&cluster_key]
            .iter()
            .min_by(|left, right| left.1.cmp(&right.1).then_with(|| left.0.cmp(&right.0)))
            .ok_or(CreatorWorkspaceError::InvalidProposal)?
            .0
            .clone();
        items.push(CapabilityProposalGovernanceItem {
            record,
            cluster: CapabilityProposalCluster {
                cluster_key,
                occurrence_count,
                canonical_proposal_id,
                related_proposal_ids: related,
                triage_priority: match occurrence_count {
                    4.. => CapabilityProposalTriagePriority::High,
                    2..=3 => CapabilityProposalTriagePriority::Elevated,
                    _ => CapabilityProposalTriagePriority::Normal,
                },
            },
        });
    }
    items.sort_by(|left, right| {
        right
            .cluster
            .occurrence_count
            .cmp(&left.cluster.occurrence_count)
            .then_with(|| {
                right
                    .record
                    .submitted_at_ms
                    .cmp(&left.record.submitted_at_ms)
            })
            .then_with(|| left.record.proposal_id.cmp(&right.record.proposal_id))
    });
    Ok(items)
}

pub fn review_capability_proposal(
    workspace_root: &Path,
    proposal_id: &str,
    status: CapabilityProposalStatus,
    reason: &str,
    duplicate_of_proposal_id: Option<&str>,
    reviewed_at_ms: u64,
    operation_id: &str,
) -> Result<CapabilityProposalRecord, CreatorWorkspaceError> {
    validate_segment(proposal_id)?;
    validate_segment(operation_id)?;
    if status == CapabilityProposalStatus::PendingReview
        || reason.trim() != reason
        || reason.is_empty()
        || reason.len() > 1_024
        || reason.chars().any(char::is_control)
    {
        return Err(CreatorWorkspaceError::InvalidProposal);
    }
    let root = validated_workspace_root(workspace_root)?;
    let proposals_root = root.join(PROPOSALS_DIRECTORY);
    create_or_verify_directory(&proposals_root)?;
    let destination = proposals_root.join(format!("{proposal_id}.json"));
    let mut record = read_capability_proposal(&destination)?;
    if record.proposal_id != proposal_id || record.status != CapabilityProposalStatus::PendingReview
    {
        return Err(CreatorWorkspaceError::InvalidProposal);
    }
    let duplicate_of_proposal_id = match (status, duplicate_of_proposal_id) {
        (CapabilityProposalStatus::Duplicate, Some(target_id)) => {
            validate_segment(target_id)?;
            if target_id == proposal_id {
                return Err(CreatorWorkspaceError::InvalidProposal);
            }
            let target =
                read_capability_proposal(&proposals_root.join(format!("{target_id}.json")))?;
            if capability_proposal_cluster_key(&record)?
                != capability_proposal_cluster_key(&target)?
            {
                return Err(CreatorWorkspaceError::InvalidProposal);
            }
            Some(target_id.to_owned())
        }
        (CapabilityProposalStatus::Duplicate, None) | (_, Some(_)) => {
            return Err(CreatorWorkspaceError::InvalidProposal);
        }
        (_, None) => None,
    };
    record.status = status;
    record.review = Some(CapabilityProposalReview {
        status,
        reason: reason.to_owned(),
        reviewed_at_ms,
        duplicate_of_proposal_id,
    });
    record.integrity_digest = capability_proposal_digest(&record)?;
    let bytes = serde_json::to_vec_pretty(&record).map_err(|_| CreatorWorkspaceError::Io)?;
    write_replace(&destination, &bytes, operation_id)?;
    Ok(record)
}

fn read_capability_proposal(
    path: &Path,
) -> Result<CapabilityProposalRecord, CreatorWorkspaceError> {
    let bytes = fs::read(path).map_err(|_| CreatorWorkspaceError::InvalidProposal)?;
    if bytes.len() > 1024 * 1024 {
        return Err(CreatorWorkspaceError::InvalidProposal);
    }
    let record: CapabilityProposalRecord =
        serde_json::from_slice(&bytes).map_err(|_| CreatorWorkspaceError::InvalidProposal)?;
    validate_capability_proposal(&record)?;
    Ok(record)
}

fn validate_capability_proposal(
    record: &CapabilityProposalRecord,
) -> Result<(), CreatorWorkspaceError> {
    if record.integrity_digest != capability_proposal_digest(record)? {
        return Err(CreatorWorkspaceError::InvalidProposal);
    }
    validate_segment(&record.proposal_id)?;
    validate_gap_proof(
        &record.gap,
        &record.composition_plan,
        &record.semantic_composition_plan,
    )?;
    let review_matches = match (&record.status, &record.review) {
        (CapabilityProposalStatus::PendingReview, None) => true,
        (status, Some(review)) => {
            *status == review.status
                && *status != CapabilityProposalStatus::PendingReview
                && !review.reason.is_empty()
                && review.reason.len() <= 1_024
                && review.reason.trim() == review.reason
                && !review.reason.chars().any(char::is_control)
                && match status {
                    CapabilityProposalStatus::Duplicate => review
                        .duplicate_of_proposal_id
                        .as_deref()
                        .is_some_and(|target| {
                            target != record.proposal_id && validate_segment(target).is_ok()
                        }),
                    _ => review.duplicate_of_proposal_id.is_none(),
                }
        }
        _ => false,
    };
    if record.spec != "nimora.capability-proposal/1"
        || !record.proposal_id.starts_with("capability-proposal-")
        || !record.gap.platform_proposal_required
        || !review_matches
    {
        return Err(CreatorWorkspaceError::InvalidProposal);
    }
    Ok(())
}

fn capability_proposal_cluster_key(
    record: &CapabilityProposalRecord,
) -> Result<String, CreatorWorkspaceError> {
    #[derive(Serialize)]
    #[serde(rename_all = "camelCase")]
    struct ClusterPayload<'a> {
        missing_capabilities: &'a [String],
        missing_semantic_outputs: &'a [String],
    }

    let mut missing_capabilities = record.composition_plan.missing_capabilities.clone();
    let mut missing_semantic_outputs = record.semantic_composition_plan.missing_outputs.clone();
    missing_capabilities.sort();
    missing_semantic_outputs.sort();
    let bytes = serde_json::to_vec(&ClusterPayload {
        missing_capabilities: &missing_capabilities,
        missing_semantic_outputs: &missing_semantic_outputs,
    })
    .map_err(|_| CreatorWorkspaceError::InvalidProposal)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn capability_proposal_digest(
    record: &CapabilityProposalRecord,
) -> Result<String, CreatorWorkspaceError> {
    let payload = CapabilityProposalIntegrityPayload {
        spec: &record.spec,
        proposal_id: &record.proposal_id,
        status: record.status,
        submitted_at_ms: record.submitted_at_ms,
        gap: &record.gap,
        composition_plan: &record.composition_plan,
        semantic_composition_plan: &record.semantic_composition_plan,
        review: &record.review,
    };
    let bytes = serde_json::to_vec(&payload).map_err(|_| CreatorWorkspaceError::InvalidProposal)?;
    Ok(format!("sha256:{:x}", Sha256::digest(bytes)))
}

fn validate_gap_proof(
    gap: &CapabilityGap,
    composition_plan: &CapabilityCompositionPlan,
    semantic_composition_plan: &SemanticCompositionPlan,
) -> Result<(), CreatorWorkspaceError> {
    validate_capability_gap(gap)?;
    let requested = gap
        .missing_capabilities
        .iter()
        .map(|item| item.capability.as_str())
        .collect::<Vec<_>>();
    let semantic_partition = semantic_composition_plan
        .available_outputs
        .iter()
        .chain(&semantic_composition_plan.missing_outputs)
        .cloned()
        .collect::<BTreeSet<_>>();
    let required_outputs = gap
        .required_semantic_outputs
        .iter()
        .cloned()
        .collect::<BTreeSet<_>>();
    if composition_plan.spec != COMPOSITION_PLAN_SPEC
        || !valid_sha256_digest(&composition_plan.catalog_digest)
        || !composition_plan.resolved_capabilities.is_empty()
        || composition_plan.fully_resolved
        || composition_plan
            .requested_capabilities
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != requested
        || semantic_composition_plan.spec != SEMANTIC_PLAN_SPEC
        || !valid_sha256_digest(&semantic_composition_plan.graph_digest)
        || semantic_composition_plan.fully_resolved
        || semantic_composition_plan.missing_outputs.is_empty()
        || semantic_partition != required_outputs
        || semantic_composition_plan
            .available_outputs
            .iter()
            .any(|output| semantic_composition_plan.missing_outputs.contains(output))
        || composition_plan
            .missing_capabilities
            .iter()
            .map(String::as_str)
            .collect::<Vec<_>>()
            != requested
    {
        return Err(CreatorWorkspaceError::InvalidGapProof);
    }
    Ok(())
}

fn validated_workspace_root(
    workspace_root: &Path,
) -> Result<std::path::PathBuf, CreatorWorkspaceError> {
    let root = fs::canonicalize(workspace_root).map_err(|_| CreatorWorkspaceError::InvalidRoot)?;
    if !root.is_dir()
        || fs::symlink_metadata(&root)
            .map_err(|_| CreatorWorkspaceError::InvalidRoot)?
            .file_type()
            .is_symlink()
    {
        return Err(CreatorWorkspaceError::InvalidRoot);
    }
    Ok(root)
}

fn valid_sha256_digest(value: &str) -> bool {
    value.len() == 71
        && value.starts_with("sha256:")
        && value[7..].bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn artifact_id(draft: &CreatorDraft) -> String {
    match &draft.artifact {
        CreatorArtifact::UserProgram { manifest, .. } => manifest.id.clone(),
        CreatorArtifact::Skill { manifest, .. } => manifest.id.clone(),
        CreatorArtifact::Automation { definition } => definition.id.clone(),
        CreatorArtifact::Theme { metadata } => metadata.id.clone(),
        CreatorArtifact::Profile { profile } => {
            let bytes = serde_json::to_vec(profile).expect("serializable generated profile");
            let digest = Sha256::digest(bytes);
            format!("profile-draft-{}", &format!("{digest:x}")[..16])
        }
    }
}

fn write_draft(root: &Path, draft: &CreatorDraft) -> Result<usize, CreatorWorkspaceError> {
    let metadata = serde_json::to_vec_pretty(draft).map_err(|_| CreatorWorkspaceError::Io)?;
    write_new(&root.join("nimora-draft.json"), &metadata)?;
    let mut count = 1;
    match &draft.artifact {
        CreatorArtifact::UserProgram { files, .. } | CreatorArtifact::Skill { files, .. } => {
            for file in files {
                write_source(root, file)?;
                count += 1;
            }
        }
        CreatorArtifact::Automation { definition } => {
            let bytes =
                serde_json::to_vec_pretty(definition).map_err(|_| CreatorWorkspaceError::Io)?;
            write_new(&root.join("automation.json"), &bytes)?;
            count += 1;
        }
        CreatorArtifact::Theme { metadata } => {
            let bytes =
                serde_json::to_vec_pretty(metadata).map_err(|_| CreatorWorkspaceError::Io)?;
            write_new(&root.join("theme.json"), &bytes)?;
            count += 1;
        }
        CreatorArtifact::Profile { profile } => {
            let bytes =
                serde_json::to_vec_pretty(profile).map_err(|_| CreatorWorkspaceError::Io)?;
            write_new(&root.join("profile.json"), &bytes)?;
            count += 1;
        }
    }
    Ok(count)
}

fn write_source(root: &Path, file: &CreatorDraftFile) -> Result<(), CreatorWorkspaceError> {
    let relative = Path::new(&file.path);
    if relative
        .components()
        .any(|item| !matches!(item, Component::Normal(_)))
    {
        return Err(CreatorWorkspaceError::Io);
    }
    let destination = root.join(relative);
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent).map_err(|_| CreatorWorkspaceError::Io)?;
    }
    write_new(&destination, file.source.as_bytes())
}

fn write_new(path: &Path, bytes: &[u8]) -> Result<(), CreatorWorkspaceError> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|_| CreatorWorkspaceError::Io)?;
    file.write_all(bytes)
        .and_then(|()| file.sync_all())
        .map_err(|_| CreatorWorkspaceError::Io)
}

fn write_replace(
    destination: &Path,
    bytes: &[u8],
    operation_id: &str,
) -> Result<(), CreatorWorkspaceError> {
    let metadata =
        fs::symlink_metadata(destination).map_err(|_| CreatorWorkspaceError::InvalidProposal)?;
    if !metadata.is_file() || metadata.file_type().is_symlink() {
        return Err(CreatorWorkspaceError::InvalidProposal);
    }
    let parent = destination.parent().ok_or(CreatorWorkspaceError::Io)?;
    let staging = parent.join(format!(".{operation_id}.review.staging"));
    write_new(&staging, bytes)?;
    fs::rename(&staging, destination).map_err(|_| {
        let _ = fs::remove_file(&staging);
        CreatorWorkspaceError::Io
    })
}

fn create_or_verify_directory(path: &Path) -> Result<(), CreatorWorkspaceError> {
    match fs::symlink_metadata(path) {
        Ok(metadata) if metadata.file_type().is_symlink() => {
            Err(CreatorWorkspaceError::SymbolicLink)
        }
        Ok(metadata) if metadata.is_dir() => Ok(()),
        Ok(_) => Err(CreatorWorkspaceError::InvalidRoot),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            fs::create_dir(path).map_err(|_| CreatorWorkspaceError::Io)
        }
        Err(_) => Err(CreatorWorkspaceError::Io),
    }
}

fn validate_segment(value: &str) -> Result<(), CreatorWorkspaceError> {
    if value.is_empty()
        || value.len() > 128
        || value.starts_with('.')
        || value.chars().any(|character| {
            !(character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || matches!(character, '.' | '-'))
        })
    {
        return Err(CreatorWorkspaceError::InvalidArtifactId);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicU64, Ordering};

    use nimora_agent_runtime::{ToolDescriptor, ToolEffect};
    use nimora_creator_composition::{CapabilityCatalogSnapshot, plan_exact_capabilities};
    use nimora_runtime_core::CommandRisk;
    use serde_json::json;

    use super::*;

    static FIXTURE_SEQUENCE: AtomicU64 = AtomicU64::new(1);

    fn fixture_root() -> PathBuf {
        let root = std::env::temp_dir().join(format!(
            "nimora-creator-workspace-{}-{}",
            std::process::id(),
            FIXTURE_SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&root).expect("fixture root");
        root
    }

    fn automation_draft() -> CreatorDraft {
        serde_json::from_value(json!({
            "spec": "nimora.creator-draft/1",
            "title": "Greeting",
            "summary": "A validated automation draft.",
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
                        "id": "greet", "command": "pet.action.play",
                        "arguments": { "action": "pet.click" }, "risk": "safe",
                        "retrySafe": false, "idempotencyKey": null, "compensation": null
                    }],
                    "policy": { "timeoutMs": 5000, "failure": "stop", "maxConcurrentRuns": 1, "cooldownMs": 0, "dailyCostBudgetMicrounits": 0 }
                }
            }
        }))
        .expect("draft")
    }

    fn theme_draft() -> CreatorDraft {
        serde_json::from_value(json!({
            "spec": "nimora.creator-draft/1",
            "title": "Aurora theme",
            "summary": "A validated accessible theme draft.",
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
                        "spec": "nimora.theme/1", "mode": "light",
                        "colors": {
                            "surface": "#f7f5ef", "surfaceElevated": "#fffdf8",
                            "text": "#30322c", "textMuted": "#77786f",
                            "accent": "#6f61ce", "accentSoft": "#eeeaff",
                            "border": "#deddd6", "success": "#5f875b",
                            "danger": "#a44f45"
                        },
                        "cornerStyle": "soft", "motion": "full"
                    }
                }
            }
        }))
        .expect("theme draft")
    }

    fn profile_draft() -> CreatorDraft {
        serde_json::from_value(json!({
            "spec": "nimora.creator-draft/1",
            "title": "Deep focus",
            "summary": "A validated profile draft.",
            "permissionExplanations": [],
            "artifact": {
                "kind": "profile",
                "profile": {
                    "name": "深度专注",
                    "policy": {
                        "mode": "focus", "alwaysOnTop": true,
                        "clickThrough": false, "soundEnabled": false,
                        "proactiveFrequency": 5
                    }
                }
            }
        }))
        .expect("profile draft")
    }

    fn gap_proof_fixture() -> (
        CapabilityGap,
        CapabilityCatalogSnapshot,
        CapabilityCompositionPlan,
        SemanticCompositionPlan,
    ) {
        let gap = serde_json::from_value(json!({
            "spec": "nimora.capability-gap/1",
            "title": "Missing camera capability",
            "summary": "The Registry cannot express this outcome.",
            "requestedOutcome": "Observe a user-approved gesture.",
            "missingCapabilities": [{
                "capability": "perception.camera.observe",
                "reason": "No consent-bound observation capability is registered.",
                "requiredOperations": ["Produce a bounded gesture event without retaining frames."]
            }],
            "availableSemanticInputs": ["perception.gesture-request"],
            "requiredSemanticOutputs": ["perception.gesture-event"],
            "closestAlternatives": [],
            "platformProposalRequired": true
        }))
        .expect("gap");
        let descriptor = ToolDescriptor::new(
            "pet.state.read",
            "Read pet state",
            "Reads bounded pet state.",
            json!({"type": "object"}),
            json!({"type": "object"}),
            CommandRisk::Safe,
            ToolEffect::ReadOnly,
        )
        .expect("descriptor");
        let catalog =
            CapabilityCatalogSnapshot::from_tool_descriptors([descriptor]).expect("catalog");
        let plan = plan_exact_capabilities(&catalog, ["perception.camera.observe".to_owned()])
            .expect("plan");
        let semantic_plan = SemanticCompositionPlan {
            spec: SEMANTIC_PLAN_SPEC.to_owned(),
            graph_digest: format!("sha256:{}", "1".repeat(64)),
            capability_path: Vec::new(),
            available_outputs: Vec::new(),
            missing_outputs: vec!["perception.gesture-event".to_owned()],
            total_cost_units: 0,
            fully_resolved: false,
            expanded_states: 1,
        };
        (gap, catalog, plan, semantic_plan)
    }

    #[test]
    fn atomically_creates_a_non_overwriting_draft_directory() {
        let root = fixture_root();
        let draft = automation_draft();
        let receipt = save_creator_draft(&root, &draft, "1").expect("save");
        assert_eq!(receipt.files_written, 2);
        let destination = root.join(&receipt.relative_directory);
        assert!(destination.join("nimora-draft.json").is_file());
        assert!(destination.join("automation.json").is_file());
        assert_eq!(
            save_creator_draft(&root, &draft, "2")
                .unwrap_err()
                .to_string(),
            CreatorWorkspaceError::AlreadyExists.to_string()
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn atomically_saves_theme_metadata_with_the_creator_envelope() {
        let root = fixture_root();
        let receipt = save_creator_draft(&root, &theme_draft(), "theme-save").expect("save");
        assert_eq!(receipt.files_written, 2);
        let destination = root.join(&receipt.relative_directory);
        assert!(destination.join("nimora-draft.json").is_file());
        let metadata: serde_json::Value = serde_json::from_slice(
            &fs::read(destination.join("theme.json")).expect("theme metadata"),
        )
        .expect("theme metadata JSON");
        assert_eq!(metadata["id"], "theme.local.aurora");
        assert_eq!(metadata["theme"]["spec"], "nimora.theme/1");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn atomically_saves_profile_policy_with_a_content_bound_directory() {
        let root = fixture_root();
        let receipt = save_creator_draft(&root, &profile_draft(), "profile-save").expect("save");
        assert_eq!(receipt.files_written, 2);
        assert!(receipt.artifact_id.starts_with("profile-draft-"));
        let destination = root.join(&receipt.relative_directory);
        assert!(destination.join("nimora-draft.json").is_file());
        let profile: serde_json::Value = serde_json::from_slice(
            &fs::read(destination.join("profile.json")).expect("profile policy"),
        )
        .expect("profile policy JSON");
        assert_eq!(profile["name"], "深度专注");
        assert_eq!(profile["policy"]["mode"], "focus");
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn saves_a_validated_capability_gap_as_an_inert_report() {
        let root = fixture_root();
        let (gap, catalog, plan, semantic_plan) = gap_proof_fixture();
        let receipt = save_capability_gap(
            &root,
            &gap,
            &plan,
            &semantic_plan,
            "018f0000-0000-7000-8000-000000000032",
        )
        .expect("save");
        assert_eq!(receipt.spec, "nimora.capability-gap-save/1");
        let report: serde_json::Value =
            serde_json::from_slice(&fs::read(root.join(&receipt.relative_file)).expect("report"))
                .expect("JSON report");
        assert_eq!(report["spec"], "nimora.persisted-capability-gap/2");
        assert_eq!(report["compositionPlan"]["catalogDigest"], catalog.digest);
        assert_eq!(
            save_capability_gap(
                &root,
                &gap,
                &plan,
                &semantic_plan,
                "018f0000-0000-7000-8000-000000000032",
            )
            .unwrap_err()
            .to_string(),
            CreatorWorkspaceError::Io.to_string()
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn submits_only_required_gaps_to_an_inert_proposal_queue() {
        let root = fixture_root();
        let (gap, _, plan, semantic_plan) = gap_proof_fixture();
        let proposal = submit_capability_proposal(
            &root,
            &gap,
            &plan,
            &semantic_plan,
            "018f0000-0000-7000-8000-000000000033",
            1_726_000_000_000,
        )
        .expect("proposal");
        assert_eq!(proposal.status, "pending-review");
        let proposal_report: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join(&proposal.relative_file)).expect("proposal report"),
        )
        .expect("proposal JSON");
        assert_eq!(proposal_report["spec"], "nimora.capability-proposal/1");
        assert_eq!(proposal_report["status"], "pending-review");
        assert!(
            proposal_report["integrityDigest"]
                .as_str()
                .is_some_and(|digest| digest.starts_with("sha256:"))
        );
        assert!(proposal_report.get("approvalId").is_none());
        assert!(proposal_report.get("executable").is_none());
        let mut no_proposal_gap = gap.clone();
        no_proposal_gap.platform_proposal_required = false;
        assert_eq!(
            submit_capability_proposal(
                &root,
                &no_proposal_gap,
                &plan,
                &semantic_plan,
                "018f0000-0000-7000-8000-000000000034",
                1_726_000_000_001,
            )
            .unwrap_err()
            .to_string(),
            CreatorWorkspaceError::InvalidGapProof.to_string()
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn reviews_proposals_once_and_rejects_tampered_queue_records() {
        let root = fixture_root();
        let (gap, _, plan, semantic_plan) = gap_proof_fixture();
        assert!(
            list_capability_proposals(&root)
                .expect("empty queue")
                .is_empty()
        );
        let receipt = submit_capability_proposal(
            &root,
            &gap,
            &plan,
            &semantic_plan,
            "018f0000-0000-7000-8000-000000000035",
            1_726_000_000_100,
        )
        .expect("proposal");
        let queue = list_capability_proposals(&root).expect("queue");
        assert_eq!(queue.len(), 1);
        assert_eq!(queue[0].status, CapabilityProposalStatus::PendingReview);
        let reviewed = review_capability_proposal(
            &root,
            &receipt.proposal_id,
            CapabilityProposalStatus::Accepted,
            "Accepted for platform feasibility analysis.",
            None,
            1_726_000_000_200,
            "018f0000-0000-7000-8000-000000000036",
        )
        .expect("review");
        assert_eq!(reviewed.status, CapabilityProposalStatus::Accepted);
        assert_eq!(
            review_capability_proposal(
                &root,
                &receipt.proposal_id,
                CapabilityProposalStatus::Rejected,
                "A terminal review cannot be replaced.",
                None,
                1_726_000_000_300,
                "018f0000-0000-7000-8000-000000000037",
            )
            .unwrap_err()
            .to_string(),
            CreatorWorkspaceError::InvalidProposal.to_string()
        );
        let path = root.join(&receipt.relative_file);
        let mut value: serde_json::Value =
            serde_json::from_slice(&fs::read(&path).expect("record")).expect("record JSON");
        value["compositionPlan"]["catalogDigest"] = json!(format!("sha256:{}", "0".repeat(64)));
        fs::write(
            &path,
            serde_json::to_vec_pretty(&value).expect("tampered JSON"),
        )
        .expect("tamper");
        assert_eq!(
            list_capability_proposals(&root).unwrap_err().to_string(),
            CreatorWorkspaceError::InvalidProposal.to_string()
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn rejects_invalid_review_reasons_and_renamed_queue_records() {
        let root = fixture_root();
        let (gap, _, plan, semantic_plan) = gap_proof_fixture();
        let receipt = submit_capability_proposal(
            &root,
            &gap,
            &plan,
            &semantic_plan,
            "018f0000-0000-7000-8000-000000000038",
            1_726_000_000_400,
        )
        .expect("proposal");
        for reason in ["", " padded ", "contains\ncontrol"] {
            assert!(matches!(
                review_capability_proposal(
                    &root,
                    &receipt.proposal_id,
                    CapabilityProposalStatus::Rejected,
                    reason,
                    None,
                    1_726_000_000_500,
                    "018f0000-0000-7000-8000-000000000039",
                ),
                Err(CreatorWorkspaceError::InvalidProposal)
            ));
        }
        let original = root.join(&receipt.relative_file);
        let renamed = original.with_file_name("capability-proposal-renamed.json");
        fs::rename(original, renamed).expect("rename record");
        assert!(matches!(
            list_capability_proposals(&root),
            Err(CreatorWorkspaceError::InvalidProposal)
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn clusters_repeated_gaps_and_binds_duplicates_to_the_canonical_record() {
        let root = fixture_root();
        let (gap, _, plan, semantic_plan) = gap_proof_fixture();
        let canonical = submit_capability_proposal(
            &root,
            &gap,
            &plan,
            &semantic_plan,
            "018f0000-0000-7000-8000-000000000040",
            1_726_000_000_600,
        )
        .expect("canonical proposal");
        let initial_queue = capability_proposal_governance(&root).expect("initial queue");
        assert_eq!(
            initial_queue[0].cluster.triage_priority,
            CapabilityProposalTriagePriority::Normal
        );
        let mut retitled_gap = gap.clone();
        retitled_gap.title = "A different user-facing title".to_owned();
        let duplicate = submit_capability_proposal(
            &root,
            &retitled_gap,
            &plan,
            &semantic_plan,
            "018f0000-0000-7000-8000-000000000041",
            1_726_000_000_700,
        )
        .expect("duplicate proposal");

        let queue = capability_proposal_governance(&root).expect("governance queue");
        assert_eq!(queue.len(), 2);
        assert!(queue.iter().all(|item| item.cluster.occurrence_count == 2));
        assert!(queue.iter().all(|item| {
            item.cluster.triage_priority == CapabilityProposalTriagePriority::Elevated
                && item.cluster.canonical_proposal_id == canonical.proposal_id
        }));
        assert_eq!(queue[0].record.proposal_id, duplicate.proposal_id);

        for (operation_id, submitted_at_ms) in [
            ("018f0000-0000-7000-8000-000000000047", 1_726_000_000_710),
            ("018f0000-0000-7000-8000-000000000048", 1_726_000_000_720),
        ] {
            submit_capability_proposal(
                &root,
                &gap,
                &plan,
                &semantic_plan,
                operation_id,
                submitted_at_ms,
            )
            .expect("repeated proposal");
        }
        let high_priority_queue =
            capability_proposal_governance(&root).expect("high priority queue");
        assert!(high_priority_queue.iter().all(|item| {
            item.cluster.occurrence_count == 4
                && item.cluster.triage_priority == CapabilityProposalTriagePriority::High
        }));

        let reviewed = review_capability_proposal(
            &root,
            &duplicate.proposal_id,
            CapabilityProposalStatus::Duplicate,
            "Same deterministic capability and semantic gap.",
            Some(&canonical.proposal_id),
            1_726_000_000_800,
            "018f0000-0000-7000-8000-000000000042",
        )
        .expect("duplicate review");
        assert_eq!(
            reviewed
                .review
                .expect("review")
                .duplicate_of_proposal_id
                .as_deref(),
            Some(canonical.proposal_id.as_str())
        );
        assert_invalid_duplicate_targets(&root, &gap, &plan, &semantic_plan, &canonical);
        fs::remove_dir_all(root).expect("cleanup");
    }

    fn assert_invalid_duplicate_targets(
        root: &Path,
        gap: &CapabilityGap,
        plan: &CapabilityCompositionPlan,
        semantic_plan: &SemanticCompositionPlan,
        canonical: &CapabilityProposalReceipt,
    ) {
        assert!(matches!(
            review_capability_proposal(
                root,
                &canonical.proposal_id,
                CapabilityProposalStatus::Duplicate,
                "Cannot target itself.",
                Some(&canonical.proposal_id),
                1_726_000_000_900,
                "018f0000-0000-7000-8000-000000000043",
            ),
            Err(CreatorWorkspaceError::InvalidProposal)
        ));
        let mut unrelated_gap = gap.clone();
        unrelated_gap.missing_capabilities[0].capability =
            "perception.microphone.observe".to_owned();
        let mut unrelated_plan = plan.clone();
        unrelated_plan.requested_capabilities = vec!["perception.microphone.observe".to_owned()];
        unrelated_plan.missing_capabilities = vec!["perception.microphone.observe".to_owned()];
        let unrelated = submit_capability_proposal(
            root,
            &unrelated_gap,
            &unrelated_plan,
            semantic_plan,
            "018f0000-0000-7000-8000-000000000044",
            1_726_000_001_000,
        )
        .expect("unrelated proposal");
        assert!(matches!(
            review_capability_proposal(
                root,
                &canonical.proposal_id,
                CapabilityProposalStatus::Duplicate,
                "Different cluster cannot be linked.",
                Some(&unrelated.proposal_id),
                1_726_000_001_100,
                "018f0000-0000-7000-8000-000000000045",
            ),
            Err(CreatorWorkspaceError::InvalidProposal)
        ));
        assert!(matches!(
            review_capability_proposal(
                root,
                &canonical.proposal_id,
                CapabilityProposalStatus::Duplicate,
                "Missing target cannot be linked.",
                Some("capability-proposal-missing"),
                1_726_000_001_200,
                "018f0000-0000-7000-8000-000000000046",
            ),
            Err(CreatorWorkspaceError::InvalidProposal)
        ));
        fs::remove_file(root.join(&canonical.relative_file)).expect("remove canonical record");
        assert!(matches!(
            list_capability_proposals(root),
            Err(CreatorWorkspaceError::InvalidProposal)
        ));
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symbolic_links_inside_the_proposal_queue() {
        use std::os::unix::fs::symlink;

        let root = fixture_root();
        let outside = fixture_root();
        fs::create_dir(root.join(PROPOSALS_DIRECTORY)).expect("proposal directory");
        let outside_record = outside.join("record.json");
        fs::write(&outside_record, b"{}").expect("outside record");
        symlink(
            outside_record,
            root.join(PROPOSALS_DIRECTORY)
                .join("capability-proposal-link.json"),
        )
        .expect("symlink");
        assert!(matches!(
            list_capability_proposals(&root),
            Err(CreatorWorkspaceError::InvalidProposal)
        ));
        fs::remove_dir_all(root).expect("cleanup root");
        fs::remove_dir_all(outside).expect("cleanup outside");
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symbolic_linked_drafts_root() {
        use std::os::unix::fs::symlink;

        let root = fixture_root();
        let outside = fixture_root();
        symlink(&outside, root.join(DRAFTS_DIRECTORY)).expect("symlink");
        assert!(matches!(
            save_creator_draft(&root, &automation_draft(), "1"),
            Err(CreatorWorkspaceError::SymbolicLink)
        ));
        fs::remove_dir_all(root).expect("cleanup root");
        fs::remove_dir_all(outside).expect("cleanup outside");
    }
}
