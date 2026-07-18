use std::{fs, io, path::Path};

use nimora_runtime_core::RuntimeMode;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{DesktopError, valid_asset_identifier};

pub(crate) const BUILTIN_CHARACTER_ID: &str = "builtin.aster";
pub(crate) const ACTIVE_CHARACTER_SPEC: &str = "nimora.active-character/1";
pub(crate) const ACTIVE_CHARACTER_FILE: &str = ".active-character.json";
pub(crate) const BUILTIN_THEME_ID: &str = "builtin.nimora";
pub(crate) const ACTIVE_THEME_SPEC: &str = "nimora.active-theme/1";
pub(crate) const ACTIVE_THEME_FILE: &str = ".active-theme.json";
pub(crate) const BUILTIN_VOICE_ID: &str = "builtin.silent";
pub(crate) const ACTIVE_VOICE_SPEC: &str = "nimora.active-voice/1";
pub(crate) const ACTIVE_VOICE_FILE: &str = ".active-voice.json";

pub(crate) const CHARACTER_SELECTION: AssetSelectionPolicy = AssetSelectionPolicy {
    spec: ACTIVE_CHARACTER_SPEC,
    file: ACTIVE_CHARACTER_FILE,
    builtin_id: BUILTIN_CHARACTER_ID,
};
pub(crate) const THEME_SELECTION: AssetSelectionPolicy = AssetSelectionPolicy {
    spec: ACTIVE_THEME_SPEC,
    file: ACTIVE_THEME_FILE,
    builtin_id: BUILTIN_THEME_ID,
};
pub(crate) const VOICE_SELECTION: AssetSelectionPolicy = AssetSelectionPolicy {
    spec: ACTIVE_VOICE_SPEC,
    file: ACTIVE_VOICE_FILE,
    builtin_id: BUILTIN_VOICE_ID,
};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct StoredAssetSelection {
    spec: String,
    asset_id: String,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct AssetSelectionPolicy {
    pub(crate) spec: &'static str,
    pub(crate) file: &'static str,
    pub(crate) builtin_id: &'static str,
}

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum ResolvedAssetSelection {
    BuiltIn { fallback_reason: Option<String> },
    Installed { asset_id: String },
}

pub(crate) fn resolve_asset_selection(
    asset_store: &Path,
    runtime_mode: RuntimeMode,
    policy: AssetSelectionPolicy,
    safe_mode_reason: &str,
) -> Result<ResolvedAssetSelection, DesktopError> {
    if runtime_mode == RuntimeMode::Safe {
        return Ok(ResolvedAssetSelection::BuiltIn {
            fallback_reason: Some(safe_mode_reason.to_owned()),
        });
    }
    let bytes = match fs::read(asset_store.join(policy.file)) {
        Ok(bytes) => bytes,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(ResolvedAssetSelection::BuiltIn {
                fallback_reason: None,
            });
        }
        Err(error) => return Err(error.into()),
    };
    let stored = match serde_json::from_slice::<StoredAssetSelection>(&bytes) {
        Ok(stored) if stored.spec == policy.spec => stored,
        Ok(_) => {
            return Ok(ResolvedAssetSelection::BuiltIn {
                fallback_reason: Some("unknown selection contract".to_owned()),
            });
        }
        Err(_) => {
            return Ok(ResolvedAssetSelection::BuiltIn {
                fallback_reason: Some("selection record is corrupt".to_owned()),
            });
        }
    };
    if stored.asset_id == policy.builtin_id {
        return Ok(ResolvedAssetSelection::BuiltIn {
            fallback_reason: None,
        });
    }
    if !valid_asset_identifier(&stored.asset_id) {
        return Ok(ResolvedAssetSelection::BuiltIn {
            fallback_reason: Some("selection identifier is invalid".to_owned()),
        });
    }
    Ok(ResolvedAssetSelection::Installed {
        asset_id: stored.asset_id,
    })
}

pub(crate) fn persist_asset_selection(
    asset_store: &Path,
    policy: AssetSelectionPolicy,
    asset_id: &str,
) -> Result<(), DesktopError> {
    fs::create_dir_all(asset_store)?;
    let destination = asset_store.join(policy.file);
    let temporary = asset_store.join(format!("{}.{}.tmp", policy.file, Uuid::now_v7()));
    let payload = serde_json::to_vec(&StoredAssetSelection {
        spec: policy.spec.to_owned(),
        asset_id: asset_id.to_owned(),
    })?;
    fs::write(&temporary, payload)?;
    if let Err(error) = fs::rename(&temporary, &destination) {
        let _ = fs::remove_file(&temporary);
        return Err(error.into());
    }
    Ok(())
}
