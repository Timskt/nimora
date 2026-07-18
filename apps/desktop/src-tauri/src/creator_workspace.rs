use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Component, Path};

use nimora_creator_draft::{CreatorArtifact, CreatorDraft, CreatorDraftError, CreatorDraftFile};
use serde::Serialize;
use thiserror::Error;

const DRAFTS_DIRECTORY: &str = ".nimora-drafts";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreatorDraftSaveReceipt {
    pub spec: &'static str,
    pub artifact_id: String,
    pub relative_directory: String,
    pub files_written: usize,
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

fn artifact_id(draft: &CreatorDraft) -> String {
    match &draft.artifact {
        CreatorArtifact::UserProgram { manifest, .. } => manifest.id.clone(),
        CreatorArtifact::Skill { manifest, .. } => manifest.id.clone(),
        CreatorArtifact::Automation { definition } => definition.id.clone(),
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
                    "name": "Greeting",
                    "enabled": false,
                    "trigger": { "eventType": "pet.pointer.clicked" },
                    "conditions": [],
                    "actions": [{
                        "id": "greet", "command": "pet.action.play",
                        "arguments": { "action": "pet.click" }, "risk": "safe",
                        "retrySafe": false, "idempotencyKey": null, "compensation": null
                    }],
                    "policy": { "timeoutMs": 5000, "failure": "stop" }
                }
            }
        }))
        .expect("draft")
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
