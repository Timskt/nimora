use std::path::{Path, PathBuf};

use nimora_asset_installer::{
    inspect_asset_package, read_verified_asset_image, read_verified_asset_model,
};
use nimora_runtime_core::RuntimeMode;

use crate::asset_selection::{
    CHARACTER_SELECTION, ResolvedAssetSelection, resolve_asset_selection,
};
use crate::valid_asset_identifier;

const PET_WINDOW_LABEL: &str = "pet";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum AssetProtocolStatus {
    Ok,
    BadRequest,
    Forbidden,
    NotFound,
    UnsupportedMediaType,
    ServiceUnavailable,
}

impl AssetProtocolStatus {
    pub(crate) const fn reason(self) -> &'static str {
        match self {
            Self::Ok => "OK",
            Self::BadRequest => "Bad Request",
            Self::Forbidden => "Forbidden",
            Self::NotFound => "Not Found",
            Self::UnsupportedMediaType => "Unsupported Media Type",
            Self::ServiceUnavailable => "Service Unavailable",
        }
    }
}

#[derive(Debug)]
pub(crate) struct AssetProtocolRequest<'a> {
    pub(crate) runtime_mode: RuntimeMode,
    pub(crate) webview_label: &'a str,
    pub(crate) method: &'a str,
    pub(crate) host: Option<&'a str>,
    pub(crate) path: &'a str,
    pub(crate) has_query: bool,
}

#[derive(Debug)]
pub(crate) struct AssetProtocolResult {
    pub(crate) status: AssetProtocolStatus,
    pub(crate) media_type: &'static str,
    pub(crate) body: Vec<u8>,
}

pub(crate) fn serve_asset(
    asset_store: &Path,
    request: &AssetProtocolRequest<'_>,
) -> AssetProtocolResult {
    if request.webview_label != PET_WINDOW_LABEL {
        return error(AssetProtocolStatus::Forbidden);
    }
    if request.method != "GET" || request.has_query {
        return error(AssetProtocolStatus::BadRequest);
    }
    if !matches!(request.host, Some("localhost" | "nimora-asset.localhost")) {
        return error(AssetProtocolStatus::BadRequest);
    }
    let Some((asset_id, relative_path)) = parse_asset_protocol_path(request.path) else {
        return error(AssetProtocolStatus::BadRequest);
    };
    let Ok(selection) = resolve_asset_selection(
        asset_store,
        request.runtime_mode,
        CHARACTER_SELECTION,
        "safe mode uses the built-in character",
    ) else {
        return error(AssetProtocolStatus::ServiceUnavailable);
    };
    let ResolvedAssetSelection::Installed {
        asset_id: active_asset_id,
    } = selection
    else {
        return error(AssetProtocolStatus::Forbidden);
    };
    if active_asset_id != asset_id {
        return error(AssetProtocolStatus::Forbidden);
    }
    let package_root = asset_store.join(&asset_id);
    if !matches!(
        inspect_asset_package(&package_root),
        Ok(asset) if asset.id == asset_id && asset.asset_type == "character"
    ) {
        return error(AssetProtocolStatus::Forbidden);
    }
    match read_verified_asset_image(&package_root, &relative_path) {
        Ok((body, media_type)) => {
            let media_type = match media_type.as_str() {
                "image/png" => "image/png",
                "image/webp" => "image/webp",
                "image/jpeg" => "image/jpeg",
                "image/gif" => "image/gif",
                _ => return error(AssetProtocolStatus::UnsupportedMediaType),
            };
            AssetProtocolResult {
                status: AssetProtocolStatus::Ok,
                media_type,
                body,
            }
        }
        Err(_) => match read_verified_asset_model(&package_root, &relative_path) {
            Ok(body) => AssetProtocolResult {
                status: AssetProtocolStatus::Ok,
                media_type: "model/gltf-binary",
                body,
            },
            Err(_) => error(AssetProtocolStatus::NotFound),
        },
    }
}

pub(crate) fn parse_asset_protocol_path(path: &str) -> Option<(String, PathBuf)> {
    let decoded = percent_decode_path(path.as_bytes())?;
    let decoded = std::str::from_utf8(&decoded).ok()?;
    let mut segments = decoded.strip_prefix('/')?.split('/');
    let asset_id = segments.next()?;
    if !valid_asset_identifier(asset_id) {
        return None;
    }
    let remaining = segments.collect::<Vec<_>>();
    if remaining.is_empty()
        || remaining.iter().any(|segment| {
            segment.is_empty() || *segment == "." || *segment == ".." || segment.contains('\\')
        })
    {
        return None;
    }
    Some((asset_id.to_owned(), PathBuf::from(remaining.join("/"))))
}

fn percent_decode_path(input: &[u8]) -> Option<Vec<u8>> {
    let mut decoded = Vec::with_capacity(input.len());
    let mut index = 0;
    while index < input.len() {
        if input[index] == b'%' {
            let high = *input.get(index + 1)?;
            let low = *input.get(index + 2)?;
            let byte = hex_value(high)?
                .checked_mul(16)?
                .checked_add(hex_value(low)?)?;
            if byte == 0 {
                return None;
            }
            decoded.push(byte);
            index += 3;
        } else {
            decoded.push(input[index]);
            index += 1;
        }
    }
    Some(decoded)
}

fn hex_value(value: u8) -> Option<u8> {
    match value {
        b'0'..=b'9' => Some(value - b'0'),
        b'a'..=b'f' => Some(value - b'a' + 10),
        b'A'..=b'F' => Some(value - b'A' + 10),
        _ => None,
    }
}

fn error(status: AssetProtocolStatus) -> AssetProtocolResult {
    AssetProtocolResult {
        status,
        media_type: "text/plain; charset=utf-8",
        body: status.reason().as_bytes().to_vec(),
    }
}
