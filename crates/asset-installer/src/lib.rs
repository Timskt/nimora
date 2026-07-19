use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    fs,
    io::{self, Read},
    path::{Component, Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use thiserror::Error;
use zip::{CompressionMethod, DateTime, ZipArchive, ZipWriter, write::SimpleFileOptions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallFile {
    pub relative_path: PathBuf,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GeneratedInstallFile {
    pub relative_path: PathBuf,
    pub contents: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GltfCharacterMetadata {
    pub id: String,
    pub version: String,
    pub name: String,
    pub publisher: String,
    pub license: String,
    pub animation_map: BTreeMap<String, ModelAnimationBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GeneratedThemeMetadata {
    pub id: String,
    pub version: String,
    pub name: BTreeMap<String, String>,
    pub publisher: String,
    pub license: String,
    pub theme: ThemeDescriptor,
}

const MAX_FILES: usize = 10_000;
const MAX_TOTAL_BYTES: u64 = 512 * 1024 * 1024;
const MAX_METADATA_BYTES: u64 = 1024 * 1024;
const MAX_PREVIEW_IMAGE_BYTES: u64 = 2 * 1024 * 1024;
const MAX_PREVIEW_IMAGE_EDGE: u32 = 4096;
const MAX_ARCHIVE_COMPRESSION_RATIO: u64 = 200;
const MANIFEST_FILE: &str = "manifest.json";
const VOICE_V1_CUES: &[&str] = &[
    "pet.celebrate",
    "pet.click",
    "pet.idle",
    "pet.observe",
    "pet.perch",
    "pet.climb",
    "pet.peek",
    "pet.stretch",
    "pet.sleep",
    "pet.wake",
    "pet.work",
];

#[derive(Debug, Error)]
pub enum InstallError {
    #[error("asset source is not a directory")]
    SourceNotDirectory,
    #[error("asset source must be an expanded directory or a .nimora package")]
    UnsupportedSource,
    #[error("asset archive is invalid: {0}")]
    InvalidArchive(String),
    #[error("asset export destination must be an absolute .nimora file outside the source")]
    InvalidExportDestination,
    #[error("asset path escapes package root: {0}")]
    UnsafePath(PathBuf),
    #[error("asset file is missing: {0}")]
    MissingFile(PathBuf),
    #[error("asset file resolves outside package root: {0}")]
    EscapedSource(PathBuf),
    #[error("asset inventory exceeds installation budget")]
    BudgetExceeded,
    #[error("asset metadata is invalid: {0}")]
    InvalidMetadata(String),
    #[error("asset SHA-256 is malformed: {0}")]
    InvalidHash(PathBuf),
    #[error("asset size does not match inventory: {0}")]
    SizeMismatch(PathBuf),
    #[error("asset SHA-256 does not match inventory: {0}")]
    HashMismatch(PathBuf),
    #[error("no previous asset version is available")]
    BackupUnavailable,
    #[error("filesystem operation failed: {0}")]
    Io(#[from] io::Error),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstallResult {
    pub active_path: PathBuf,
    pub backup_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RollbackResult {
    pub active_path: PathBuf,
    pub quarantined_path: Option<PathBuf>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetPackageInstallResult {
    pub asset_id: String,
    pub version: String,
    pub install: InstallResult,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPackageSummary {
    pub id: String,
    pub asset_type: String,
    pub version: String,
    pub name: BTreeMap<String, String>,
    pub publisher: String,
    pub license: String,
    pub renderer_backend: Option<String>,
    pub file_count: usize,
    pub total_bytes: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPreviewImage {
    pub media_type: String,
    pub bytes: Vec<u8>,
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPreviewReport {
    pub summary: AssetPackageSummary,
    pub poster: Option<AssetPreviewImage>,
    pub theme: Option<ThemeDescriptor>,
    pub voice: Option<VoiceDescriptor>,
    pub voice_preview: Option<AssetPreviewAudio>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetPreviewAudio {
    pub cue: String,
    pub media_type: String,
    pub bytes: Vec<u8>,
    pub captions: BTreeMap<String, String>,
    pub gain_db: f64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ThemeDescriptor {
    pub spec: String,
    pub mode: ThemeMode,
    pub colors: BTreeMap<String, String>,
    pub corner_style: ThemeCornerStyle,
    pub motion: ThemeMotion,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMode {
    Light,
    Dark,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeCornerStyle {
    Soft,
    Rounded,
    Compact,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ThemeMotion {
    Full,
    Reduced,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceDescriptor {
    pub spec: String,
    pub clips: BTreeMap<String, VoiceClipDescriptor>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VoiceClipDescriptor {
    pub file: PathBuf,
    pub captions: BTreeMap<String, String>,
    pub gain_db: f64,
}

#[derive(Debug)]
struct PreparedAssetSource {
    root: PathBuf,
    temporary: bool,
}

impl Drop for PreparedAssetSource {
    fn drop(&mut self) {
        if self.temporary {
            let _ = fs::remove_dir_all(&self.root);
        }
    }
}

#[derive(Debug)]
struct ValidatedAssetPackage {
    summary: AssetPackageSummary,
    renderer: Option<AssetRendererDescriptor>,
    files: Vec<InstallFile>,
    media_types: BTreeMap<PathBuf, String>,
    preview_poster: Option<PathBuf>,
    theme: Option<ThemeDescriptor>,
    voice: Option<VoiceDescriptor>,
    integrity_path: PathBuf,
    integrity_bytes: Vec<u8>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetManifestHeader {
    spec: String,
    id: String,
    #[serde(rename = "type")]
    asset_type: String,
    version: String,
    name: BTreeMap<String, String>,
    publisher: String,
    license: String,
    engines: serde_json::Value,
    #[serde(default)]
    render: Option<AssetRenderHeader>,
    #[serde(default)]
    entrypoints: Option<AssetEntrypoints>,
    #[serde(default)]
    capabilities: Vec<String>,
    #[serde(default)]
    fallbacks: BTreeMap<String, String>,
    #[serde(default)]
    locales: Vec<String>,
    integrity: AssetIntegrityReference,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetRenderHeader {
    backend: String,
    canvas: RenderCanvas,
    anchor: RenderAnchor,
    default_scale: f64,
    pixel_art: bool,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetEntrypoints {
    animation_graph: Option<PathBuf>,
    clips: Option<PathBuf>,
    model: Option<PathBuf>,
    hitboxes: Option<PathBuf>,
    preview_poster: Option<PathBuf>,
    theme: Option<PathBuf>,
    voice: Option<PathBuf>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenderCanvas {
    pub width: u32,
    pub height: u32,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct RenderAnchor {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AssetRendererDescriptor {
    pub backend: String,
    pub canvas: RenderCanvas,
    pub anchor: RenderAnchor,
    pub default_scale: f64,
    pub pixel_art: bool,
    pub fallbacks: BTreeMap<String, String>,
    pub clips: Option<SpriteClips>,
    pub model: Option<PathBuf>,
    pub animation_map: Option<ModelAnimationMap>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelAnimationMap {
    pub spec: String,
    pub clips: BTreeMap<String, ModelAnimationBinding>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ModelAnimationBinding {
    pub animation: String,
    pub looped: bool,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(tag = "backend", rename_all = "kebab-case", deny_unknown_fields)]
pub enum SpriteClips {
    SpriteSequence {
        spec: String,
        clips: BTreeMap<String, SpriteSequenceClip>,
    },
    SpriteAtlas {
        spec: String,
        image: PathBuf,
        clips: BTreeMap<String, SpriteAtlasClip>,
    },
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteSequenceClip {
    pub r#loop: bool,
    pub frames: Vec<SpriteSequenceFrame>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteSequenceFrame {
    pub file: PathBuf,
    pub duration_ms: u64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteAtlasClip {
    pub r#loop: bool,
    pub frames: Vec<SpriteAtlasFrame>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SpriteAtlasFrame {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
    pub duration_ms: u64,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct AssetIntegrityReference {
    algorithm: String,
    files: PathBuf,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetIntegrityDocument {
    files: Vec<AssetIntegrityFile>,
    total_bytes: u64,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct AssetIntegrityFile {
    path: PathBuf,
    sha256: String,
    bytes: u64,
    media_type: String,
}

/// Loads package-owned metadata, verifies the declared inventory, and atomically
/// activates the package under the manifest's own asset identifier.
///
/// # Errors
///
/// Returns an error when metadata is missing, malformed, unsafe, inconsistent,
/// or when an inventory file fails validation.
pub fn install_asset_package(
    source_root: &Path,
    asset_store: &Path,
) -> Result<AssetPackageInstallResult, InstallError> {
    let package = load_asset_package(source_root)?;
    let active_path = asset_store.join(&package.summary.id);
    let install = install_atomically_with_generated(
        source_root,
        &active_path,
        &package.files,
        &[GeneratedInstallFile {
            relative_path: package.integrity_path,
            contents: package.integrity_bytes,
        }],
    )?;
    Ok(AssetPackageInstallResult {
        asset_id: package.summary.id,
        version: package.summary.version,
        install,
    })
}

/// Opens either an expanded package directory or a `.nimora` archive, fully
/// verifies it, and atomically activates the package.
///
/// # Errors
///
/// Returns an error before activation when the source, archive, metadata, or
/// declared inventory violates any package safety constraint.
pub fn install_asset_source(
    source: &Path,
    asset_store: &Path,
) -> Result<AssetPackageInstallResult, InstallError> {
    let prepared = prepare_asset_source(source)?;
    install_asset_package(&prepared.root, asset_store)
}

/// Normalizes one already-probed GLB into the current Character package schema
/// and atomically installs it through the same verified package pipeline.
///
/// # Errors
///
/// Returns an error when metadata is invalid, the staged model is not a regular
/// GLB file, package generation fails, or the generated package cannot be installed.
pub fn install_gltf_character(
    staged_glb: &Path,
    asset_store: &Path,
    metadata: &GltfCharacterMetadata,
) -> Result<AssetPackageInstallResult, InstallError> {
    if !metadata.id.starts_with("character.local.") {
        return Err(InstallError::InvalidMetadata(
            "locally generated characters require the character.local namespace".to_owned(),
        ));
    }
    validate_model_animation_bindings(&metadata.animation_map)?;
    let source_metadata = fs::symlink_metadata(staged_glb)?;
    if !source_metadata.file_type().is_file()
        || staged_glb.extension().and_then(|value| value.to_str()) != Some("glb")
    {
        return Err(InstallError::InvalidMetadata(
            "normalized model source must be a regular GLB file".to_owned(),
        ));
    }
    let package_root = unique_sibling(
        &std::env::temp_dir().join("nimora-model-package"),
        "staging",
    );
    let package = PreparedAssetSource {
        root: package_root,
        temporary: true,
    };
    fs::create_dir_all(package.root.join("models"))?;
    let model_path = package.root.join("models/character.glb");
    let copied = fs::copy(staged_glb, &model_path)?;
    if copied != source_metadata.len() {
        return Err(InstallError::SizeMismatch(PathBuf::from(
            "models/character.glb",
        )));
    }
    fs::File::open(&model_path)?.sync_all()?;

    write_gltf_character_package_files(&package.root, &model_path, metadata)?;
    install_asset_package(&package.root, asset_store)
}

/// Builds and atomically installs a generated theme through the verified asset pipeline.
///
/// # Errors
///
/// Rejects non-local identities, invalid metadata, inaccessible color combinations, or
/// any package generation or installation failure.
pub fn install_generated_theme(
    asset_store: &Path,
    metadata: &GeneratedThemeMetadata,
) -> Result<AssetPackageInstallResult, InstallError> {
    validate_generated_theme_metadata(metadata)?;
    let package = PreparedAssetSource {
        root: unique_sibling(
            &std::env::temp_dir().join("nimora-theme-package"),
            "staging",
        ),
        temporary: true,
    };
    fs::create_dir_all(package.root.join("theme"))?;
    write_generated_theme_package(&package.root, metadata)?;
    install_asset_package(&package.root, asset_store)
}

/// Validates the complete host-owned metadata contract for an AI-generated theme.
///
/// # Errors
///
/// Rejects identities outside the local namespace and metadata that cannot form a valid
/// `nimora.asset/1` theme package.
pub fn validate_generated_theme_metadata(
    metadata: &GeneratedThemeMetadata,
) -> Result<(), InstallError> {
    if !metadata.id.starts_with("theme.local.")
        || !valid_asset_identifier(&metadata.id)
        || !valid_semver(&metadata.version)
        || !valid_asset_identifier(&metadata.publisher)
        || metadata.name.is_empty()
        || metadata.name.len() > 16
        || metadata.name.iter().any(|(locale, name)| {
            !valid_locale(locale)
                || name.trim().is_empty()
                || name.chars().count() > 128
                || name.chars().any(char::is_control)
        })
        || metadata.license.trim().is_empty()
        || metadata.license.len() > 128
        || metadata.license.chars().any(char::is_control)
    {
        return Err(InstallError::InvalidMetadata(
            "generated theme metadata is invalid".to_owned(),
        ));
    }
    validate_theme_descriptor(&metadata.theme)
}

fn write_generated_theme_package(
    package_root: &Path,
    metadata: &GeneratedThemeMetadata,
) -> Result<(), InstallError> {
    let manifest = serde_json::to_vec_pretty(&serde_json::json!({
        "spec": "nimora.asset/1",
        "id": metadata.id,
        "type": "theme",
        "version": metadata.version,
        "name": metadata.name,
        "publisher": metadata.publisher,
        "license": metadata.license,
        "engines": { "nimora": ">=0.1.0" },
        "entrypoints": { "theme": "theme/theme.json" },
        "capabilities": [],
        "fallbacks": {},
        "locales": metadata.name.keys().collect::<Vec<_>>(),
        "integrity": { "algorithm": "sha256", "files": "integrity.json" }
    }))
    .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    let theme = serde_json::to_vec_pretty(&metadata.theme)
        .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    fs::write(package_root.join(MANIFEST_FILE), &manifest)?;
    fs::write(package_root.join("theme/theme.json"), &theme)?;
    let total_bytes = manifest.len() + theme.len();
    let integrity = serde_json::to_vec_pretty(&serde_json::json!({
        "files": [
            {
                "path": MANIFEST_FILE,
                "sha256": format!("{:x}", Sha256::digest(&manifest)),
                "bytes": manifest.len(),
                "mediaType": "application/json"
            },
            {
                "path": "theme/theme.json",
                "sha256": format!("{:x}", Sha256::digest(&theme)),
                "bytes": theme.len(),
                "mediaType": "application/json"
            }
        ],
        "totalBytes": total_bytes
    }))
    .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    fs::write(package_root.join("integrity.json"), integrity)?;
    Ok(())
}

fn write_gltf_character_package_files(
    package_root: &Path,
    model_path: &Path,
    metadata: &GltfCharacterMetadata,
) -> Result<(), InstallError> {
    let mut entrypoints = serde_json::Map::from_iter([(
        "model".to_owned(),
        serde_json::Value::String("models/character.glb".to_owned()),
    )]);
    if !metadata.animation_map.is_empty() {
        entrypoints.insert(
            "animationGraph".to_owned(),
            serde_json::Value::String("animations/actions.json".to_owned()),
        );
    }
    let manifest = serde_json::to_vec_pretty(&serde_json::json!({
        "spec": "nimora.asset/1",
        "id": metadata.id,
        "type": "character",
        "version": metadata.version,
        "name": { "en": metadata.name },
        "publisher": metadata.publisher,
        "license": metadata.license,
        "engines": { "nimora": ">=0.1.0" },
        "render": {
            "backend": "gltf",
            "canvas": { "width": 512, "height": 512 },
            "anchor": { "x": 0.5, "y": 1.0 },
            "defaultScale": 1.0,
            "pixelArt": false
        },
        "entrypoints": entrypoints,
        "capabilities": [],
        "fallbacks": {},
        "locales": ["en"],
        "integrity": { "algorithm": "sha256", "files": "integrity.json" }
    }))
    .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    fs::write(package_root.join(MANIFEST_FILE), &manifest)?;
    let model_bytes = fs::read(model_path)?;
    let animation_graph = (!metadata.animation_map.is_empty())
        .then(|| {
            serde_json::to_vec_pretty(&ModelAnimationMap {
                spec: "nimora.animation-map/1".to_owned(),
                clips: metadata.animation_map.clone(),
            })
        })
        .transpose()
        .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    if let Some(bytes) = &animation_graph {
        fs::create_dir_all(package_root.join("animations"))?;
        fs::write(package_root.join("animations/actions.json"), bytes)?;
    }
    let mut files = vec![
        serde_json::json!({
            "path": MANIFEST_FILE,
            "sha256": format!("{:x}", Sha256::digest(&manifest)),
            "bytes": manifest.len(),
            "mediaType": "application/json"
        }),
        serde_json::json!({
            "path": "models/character.glb",
            "sha256": format!("{:x}", Sha256::digest(&model_bytes)),
            "bytes": model_bytes.len(),
            "mediaType": "model/gltf-binary"
        }),
    ];
    if let Some(bytes) = &animation_graph {
        files.push(serde_json::json!({
            "path": "animations/actions.json",
            "sha256": format!("{:x}", Sha256::digest(bytes)),
            "bytes": bytes.len(),
            "mediaType": "application/json"
        }));
    }
    let total_bytes =
        manifest.len() + model_bytes.len() + animation_graph.as_ref().map_or(0, Vec::len);
    let integrity = serde_json::to_vec_pretty(&serde_json::json!({
        "files": files,
        "totalBytes": total_bytes
    }))
    .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    fs::write(package_root.join("integrity.json"), integrity)?;
    Ok(())
}

/// Validates the renderer-independent action-to-animation contract.
///
/// # Errors
///
/// Returns an error when a non-empty map omits `pet.idle`, exceeds its budget,
/// or contains an invalid action identifier or animation name.
pub fn validate_model_animation_bindings(
    animation_map: &BTreeMap<String, ModelAnimationBinding>,
) -> Result<(), InstallError> {
    if animation_map.is_empty() {
        return Ok(());
    }
    if !animation_map.contains_key("pet.idle")
        || animation_map.len() > 64
        || animation_map.iter().any(|(action, clip)| {
            !valid_action_id(action)
                || clip.animation.trim().is_empty()
                || clip.animation.len() > 256
                || clip.animation.contains(['\0', '\r', '\n'])
        })
    {
        return Err(InstallError::InvalidMetadata(
            "model animation map is invalid".to_owned(),
        ));
    }
    Ok(())
}

/// Verifies an expanded package without changing the filesystem.
///
/// # Errors
///
/// Returns an error when metadata or any declared file violates the package contract.
pub fn inspect_asset_package(source_root: &Path) -> Result<AssetPackageSummary, InstallError> {
    Ok(load_asset_package(source_root)?.summary)
}

/// Returns a verified Theme descriptor without exposing package paths.
///
/// # Errors
///
/// Returns an error when the package is not a Theme or its descriptor is invalid.
pub fn inspect_asset_theme(source_root: &Path) -> Result<ThemeDescriptor, InstallError> {
    let package = load_asset_package(source_root)?;
    package.theme.ok_or_else(|| {
        InstallError::InvalidMetadata("asset package is not a usable theme".to_owned())
    })
}

/// Opens and verifies an expanded package directory or a `.nimora` archive
/// without changing the asset store.
///
/// # Errors
///
/// Returns an error when the source or package violates the archive and asset contracts.
pub fn inspect_asset_source(source: &Path) -> Result<AssetPackageSummary, InstallError> {
    let prepared = prepare_asset_source(source)?;
    inspect_asset_package(&prepared.root)
}

/// Opens and verifies a package, then reads its explicitly declared preview
/// poster while the isolated source is still available.
///
/// # Errors
///
/// Returns an error when the package or declared preview violates its contract.
pub fn inspect_asset_source_preview(source: &Path) -> Result<AssetPreviewReport, InstallError> {
    let prepared = prepare_asset_source(source)?;
    let package = load_asset_package(&prepared.root)?;
    let poster = package
        .preview_poster
        .as_deref()
        .map(|path| read_preview_image(&prepared.root, path, &package.media_types))
        .transpose()?;
    let voice_preview = package
        .voice
        .as_ref()
        .and_then(|voice| voice.clips.first_key_value())
        .map(|(cue, clip)| read_voice_audio(&prepared.root, cue, clip, &package.media_types))
        .transpose()?;
    Ok(AssetPreviewReport {
        summary: package.summary,
        poster,
        theme: package.theme,
        voice: package.voice,
        voice_preview,
    })
}

/// Reopens and verifies an installed voice package before returning one bounded clip.
///
/// # Errors
///
/// Returns an error when the package, cue, inventory, media header, or clip budget is invalid.
pub fn read_asset_voice_clip(
    source_root: &Path,
    cue: &str,
) -> Result<AssetPreviewAudio, InstallError> {
    let package = load_asset_package(source_root)?;
    let voice = package
        .voice
        .as_ref()
        .ok_or_else(|| InstallError::InvalidMetadata("asset is not a voice package".to_owned()))?;
    let clip = voice
        .clips
        .get(cue)
        .ok_or_else(|| InstallError::InvalidMetadata("voice cue is not declared".to_owned()))?;
    read_voice_audio(source_root, cue, clip, &package.media_types)
}

/// Reopens and verifies a voice package before returning its path-free descriptor.
///
/// # Errors
///
/// Returns an error when the package is not a valid Voice asset.
pub fn inspect_asset_voice(source_root: &Path) -> Result<VoiceDescriptor, InstallError> {
    load_asset_package(source_root)?
        .voice
        .ok_or_else(|| InstallError::InvalidMetadata("asset is not a voice package".to_owned()))
}

/// Verifies an expanded package directory and writes a deterministic `.nimora`
/// archive using only files owned by the authoritative inventory.
///
/// # Errors
///
/// Returns an error without replacing the destination when the source package
/// is invalid, the destination is unsafe, or archive creation fails.
pub fn export_asset_package(
    source_root: &Path,
    destination: &Path,
) -> Result<AssetPackageSummary, InstallError> {
    let package = load_asset_package(source_root)?;
    validate_export_destination(source_root, destination)?;
    let staging = unique_sibling(destination, "staging");
    let result = write_asset_archive(source_root, &package, &staging)
        .and_then(|()| replace_export_atomically(&staging, destination));
    if result.is_err() {
        let _ = fs::remove_file(&staging);
    }
    result?;
    Ok(package.summary)
}

fn validate_export_destination(source_root: &Path, destination: &Path) -> Result<(), InstallError> {
    if !destination.is_absolute()
        || destination
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("nimora"))
    {
        return Err(InstallError::InvalidExportDestination);
    }
    let source_root = source_root.canonicalize()?;
    let parent = destination
        .parent()
        .ok_or(InstallError::InvalidExportDestination)?
        .canonicalize()?;
    if !parent.is_dir() || parent.starts_with(source_root) {
        return Err(InstallError::InvalidExportDestination);
    }
    Ok(())
}

fn write_asset_archive(
    source_root: &Path,
    package: &ValidatedAssetPackage,
    destination: &Path,
) -> Result<(), InstallError> {
    let file = fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(destination)?;
    let mut archive = ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(CompressionMethod::Stored)
        .last_modified_time(DateTime::default())
        .unix_permissions(0o644);
    let mut paths = package
        .files
        .iter()
        .map(|file| file.relative_path.clone())
        .collect::<Vec<_>>();
    paths.push(package.integrity_path.clone());
    paths.sort_unstable();
    for relative_path in paths {
        let relative_path = safe_relative_path(&relative_path)?;
        let archive_name = relative_path
            .to_str()
            .ok_or_else(|| InstallError::UnsafePath(relative_path.to_path_buf()))?
            .replace('\\', "/");
        archive
            .start_file(archive_name, options)
            .map_err(|error| InstallError::InvalidArchive(error.to_string()))?;
        let mut source = fs::File::open(source_root.join(relative_path))?;
        io::copy(&mut source, &mut archive)?;
    }
    let output = archive
        .finish()
        .map_err(|error| InstallError::InvalidArchive(error.to_string()))?;
    output.sync_all()?;
    Ok(())
}

fn replace_export_atomically(staging: &Path, destination: &Path) -> Result<(), InstallError> {
    if !destination.exists() {
        fs::rename(staging, destination)?;
        return Ok(());
    }
    let metadata = fs::symlink_metadata(destination)?;
    if !metadata.file_type().is_file() {
        return Err(InstallError::InvalidExportDestination);
    }
    let backup = unique_sibling(destination, "backup");
    fs::rename(destination, &backup)?;
    if let Err(error) = fs::rename(staging, destination) {
        let _ = fs::rename(&backup, destination);
        return Err(InstallError::Io(error));
    }
    let _ = fs::remove_file(backup);
    Ok(())
}

fn prepare_asset_source(source: &Path) -> Result<PreparedAssetSource, InstallError> {
    if source.is_dir() {
        return Ok(PreparedAssetSource {
            root: source.to_path_buf(),
            temporary: false,
        });
    }
    if !source.is_file()
        || source
            .extension()
            .and_then(std::ffi::OsStr::to_str)
            .is_none_or(|extension| !extension.eq_ignore_ascii_case("nimora"))
    {
        return Err(InstallError::UnsupportedSource);
    }
    extract_asset_archive(source)
}

fn extract_asset_archive(source: &Path) -> Result<PreparedAssetSource, InstallError> {
    let archive_file = fs::File::open(source)?;
    let mut archive = ZipArchive::new(archive_file)
        .map_err(|error| InstallError::InvalidArchive(error.to_string()))?;
    if archive.len() > MAX_FILES {
        return Err(InstallError::BudgetExceeded);
    }
    let root = unique_sibling(&std::env::temp_dir().join("nimora-asset-import"), "extract");
    fs::create_dir(&root)?;
    let prepared = PreparedAssetSource {
        root,
        temporary: true,
    };
    let mut extracted_files = std::collections::HashSet::with_capacity(archive.len());
    let mut total_bytes = 0_u64;
    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|error| InstallError::InvalidArchive(error.to_string()))?;
        let raw_name = entry.name();
        if raw_name.contains(['\\', '\0']) {
            return Err(InstallError::InvalidArchive(
                "entry name contains a forbidden character".to_owned(),
            ));
        }
        let relative_path = entry
            .enclosed_name()
            .ok_or_else(|| {
                InstallError::InvalidArchive("entry path escapes package root".to_owned())
            })?
            .clone();
        safe_relative_path(&relative_path)?;
        validate_archive_entry_type(entry.unix_mode(), entry.is_dir())?;
        let output = prepared.root.join(&relative_path);
        if entry.is_dir() {
            fs::create_dir_all(output)?;
            continue;
        }
        if is_nested_archive(&relative_path) {
            return Err(InstallError::InvalidArchive(format!(
                "nested archive is forbidden: {}",
                relative_path.display()
            )));
        }
        if !extracted_files.insert(relative_path.clone()) {
            return Err(InstallError::InvalidArchive(format!(
                "duplicate file entry: {}",
                relative_path.display()
            )));
        }
        let declared_size = entry.size();
        total_bytes = total_bytes
            .checked_add(declared_size)
            .ok_or(InstallError::BudgetExceeded)?;
        if total_bytes > MAX_TOTAL_BYTES
            || archive_ratio_exceeded(declared_size, entry.compressed_size())
        {
            return Err(InstallError::BudgetExceeded);
        }
        if let Some(parent) = output.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut destination = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        let copied = io::copy(
            &mut entry.by_ref().take(declared_size.saturating_add(1)),
            &mut destination,
        )?;
        if copied != declared_size {
            return Err(InstallError::InvalidArchive(format!(
                "entry size changed while extracting: {}",
                relative_path.display()
            )));
        }
        destination.sync_all()?;
    }
    Ok(prepared)
}

fn validate_archive_entry_type(mode: Option<u32>, directory: bool) -> Result<(), InstallError> {
    let Some(mode) = mode else {
        return Ok(());
    };
    let file_type = mode & 0o170_000;
    if file_type == 0
        || (directory && file_type == 0o040_000)
        || (!directory && file_type == 0o100_000)
    {
        return Ok(());
    }
    Err(InstallError::InvalidArchive(
        "links and special filesystem entries are forbidden".to_owned(),
    ))
}

fn archive_ratio_exceeded(uncompressed: u64, compressed: u64) -> bool {
    uncompressed
        > compressed
            .max(1)
            .saturating_mul(MAX_ARCHIVE_COMPRESSION_RATIO)
}

fn is_nested_archive(path: &Path) -> bool {
    path.extension()
        .and_then(std::ffi::OsStr::to_str)
        .is_some_and(|extension| {
            ["zip", "nimora", "7z", "rar", "tar", "gz", "bz2", "xz"]
                .iter()
                .any(|candidate| extension.eq_ignore_ascii_case(candidate))
        })
}

/// Verifies a package and returns its host-authoritative sprite renderer contract.
///
/// # Errors
///
/// Returns an error for unsupported renderers or any invalid package metadata.
pub fn inspect_asset_renderer(source_root: &Path) -> Result<AssetRendererDescriptor, InstallError> {
    load_asset_package(source_root)?
        .renderer
        .ok_or_else(|| InstallError::InvalidMetadata("asset has no supported renderer".to_owned()))
}

/// Revalidates an installed package and reads one inventory-owned image.
///
/// # Errors
///
/// Returns an error when the package changed, the path is unsafe or absent,
/// or the inventory media type is not an allowed image type.
pub fn read_verified_asset_image(
    source_root: &Path,
    relative_path: &Path,
) -> Result<(Vec<u8>, String), InstallError> {
    let package = load_asset_package(source_root)?;
    let relative_path = safe_relative_path(relative_path)?;
    let media_type = package
        .media_types
        .get(relative_path)
        .ok_or_else(|| invalid_sprite("resource is outside the verified inventory"))?;
    if !["image/png", "image/webp", "image/jpeg", "image/gif"].contains(&media_type.as_str()) {
        return Err(invalid_sprite("resource is not an allowed image"));
    }
    require_media_extension(relative_path, media_type)?;
    let root = source_root.canonicalize()?;
    let path = root.join(relative_path);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() {
        return Err(InstallError::MissingFile(relative_path.to_path_buf()));
    }
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&root) {
        return Err(InstallError::EscapedSource(relative_path.to_path_buf()));
    }
    Ok((fs::read(canonical)?, media_type.clone()))
}

/// Revalidates an installed GLB character and reads only its declared model entrypoint.
///
/// # Errors
///
/// Returns an error when the package changed, the requested path is not the
/// authoritative model entrypoint, or the inventory media type is not GLB.
pub fn read_verified_asset_model(
    source_root: &Path,
    relative_path: &Path,
) -> Result<Vec<u8>, InstallError> {
    let package = load_asset_package(source_root)?;
    let relative_path = safe_relative_path(relative_path)?;
    let declared = package
        .renderer
        .as_ref()
        .and_then(|renderer| renderer.model.as_deref())
        .ok_or_else(|| InstallError::InvalidMetadata("asset has no GLB entrypoint".to_owned()))?;
    if declared != relative_path {
        return Err(InstallError::InvalidMetadata(
            "requested model is not the renderer entrypoint".to_owned(),
        ));
    }
    let media_type = package.media_types.get(relative_path).ok_or_else(|| {
        InstallError::InvalidMetadata("model is outside the verified inventory".to_owned())
    })?;
    let extension = relative_path.extension().and_then(|value| value.to_str());
    if media_type != "model/gltf-binary" || !matches!(extension, Some("glb" | "vrm")) {
        return Err(InstallError::InvalidMetadata(
            "model entrypoint must be GLB or VRM".to_owned(),
        ));
    }
    let root = source_root.canonicalize()?;
    let path = root.join(relative_path);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() {
        return Err(InstallError::MissingFile(relative_path.to_path_buf()));
    }
    let canonical = path.canonicalize()?;
    if !canonical.starts_with(&root) {
        return Err(InstallError::EscapedSource(relative_path.to_path_buf()));
    }
    Ok(fs::read(canonical)?)
}

fn read_preview_image(
    source_root: &Path,
    relative_path: &Path,
    media_types: &BTreeMap<PathBuf, String>,
) -> Result<AssetPreviewImage, InstallError> {
    let relative_path = safe_relative_path(relative_path)?;
    let media_type = media_types.get(relative_path).ok_or_else(|| {
        InstallError::InvalidMetadata("preview poster is outside the verified inventory".to_owned())
    })?;
    if !["image/png", "image/webp"].contains(&media_type.as_str()) {
        return Err(InstallError::InvalidMetadata(
            "preview poster must be PNG or WebP".to_owned(),
        ));
    }
    require_media_extension(relative_path, media_type)?;
    let bytes = fs::read(source_root.join(relative_path))?;
    if bytes.len() as u64 > MAX_PREVIEW_IMAGE_BYTES {
        return Err(InstallError::BudgetExceeded);
    }
    let (width, height) = preview_image_dimensions(&bytes, media_type)?;
    if width == 0
        || height == 0
        || width > MAX_PREVIEW_IMAGE_EDGE
        || height > MAX_PREVIEW_IMAGE_EDGE
    {
        return Err(InstallError::BudgetExceeded);
    }
    Ok(AssetPreviewImage {
        media_type: media_type.clone(),
        bytes,
        width,
        height,
    })
}

fn preview_image_dimensions(bytes: &[u8], media_type: &str) -> Result<(u32, u32), InstallError> {
    let dimensions = match media_type {
        "image/png" if bytes.len() >= 24 && bytes.starts_with(b"\x89PNG\r\n\x1a\n") => Some((
            u32::from_be_bytes(bytes[16..20].try_into().expect("fixed PNG width")),
            u32::from_be_bytes(bytes[20..24].try_into().expect("fixed PNG height")),
        )),
        "image/webp"
            if bytes.len() >= 30
                && bytes.starts_with(b"RIFF")
                && bytes.get(8..12) == Some(b"WEBP") =>
        {
            webp_dimensions(bytes)
        }
        _ => None,
    };
    dimensions.ok_or_else(|| {
        InstallError::InvalidMetadata("preview poster image header is invalid".to_owned())
    })
}

fn webp_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    match bytes.get(12..16)? {
        b"VP8X" if bytes.len() >= 30 => Some((
            1 + u32::from_le_bytes([bytes[24], bytes[25], bytes[26], 0]),
            1 + u32::from_le_bytes([bytes[27], bytes[28], bytes[29], 0]),
        )),
        b"VP8L" if bytes.len() >= 25 && bytes[20] == 0x2f => Some((
            1 + u32::from(bytes[21]) + ((u32::from(bytes[22]) & 0x3f) << 8),
            1 + (u32::from(bytes[22]) >> 6)
                + (u32::from(bytes[23]) << 2)
                + ((u32::from(bytes[24]) & 0x0f) << 10),
        )),
        b"VP8 " if bytes.len() >= 30 && bytes.get(23..26) == Some(&[0x9d, 0x01, 0x2a]) => Some((
            u32::from(u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3fff),
            u32::from(u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3fff),
        )),
        _ => None,
    }
}

fn load_asset_package(source_root: &Path) -> Result<ValidatedAssetPackage, InstallError> {
    let manifest_bytes = read_metadata(source_root, Path::new(MANIFEST_FILE))?;
    let manifest: AssetManifestHeader = serde_json::from_slice(&manifest_bytes)
        .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    validate_manifest_header(&manifest)?;
    let integrity_path = safe_relative_path(&manifest.integrity.files)?;
    if integrity_path == Path::new(MANIFEST_FILE) {
        return Err(InstallError::InvalidMetadata(
            "integrity inventory cannot replace manifest.json".to_owned(),
        ));
    }
    let integrity_bytes = read_metadata(source_root, integrity_path)?;
    let integrity: AssetIntegrityDocument = serde_json::from_slice(&integrity_bytes)
        .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    let renderer = load_asset_renderer(source_root, &manifest, &integrity.files)?;
    let theme = load_theme_descriptor(source_root, &manifest, &integrity.files)?;
    let voice = load_voice_descriptor(source_root, &manifest, &integrity.files)?;
    let preview_poster = manifest
        .entrypoints
        .as_ref()
        .and_then(|entrypoints| entrypoints.preview_poster.clone());
    let media_types = integrity
        .files
        .iter()
        .map(|file| (file.path.clone(), file.media_type.clone()))
        .collect();
    let file_count = integrity.files.len();
    let total_bytes = integrity.total_bytes;
    let files = integrity
        .files
        .into_iter()
        .map(|file| {
            if file.media_type.trim().is_empty() {
                return Err(InstallError::InvalidMetadata(
                    "inventory mediaType cannot be empty".to_owned(),
                ));
            }
            Ok(InstallFile {
                relative_path: file.path,
                bytes: file.bytes,
                sha256: file.sha256,
            })
        })
        .collect::<Result<Vec<_>, _>>()?;
    validate_integrity_document(&files, integrity.total_bytes, integrity_path)?;
    validate_package_tree(source_root, &files, integrity_path)?;
    for file in &files {
        validate_file(
            &source_root.join(safe_relative_path(&file.relative_path)?),
            file,
        )?;
    }
    Ok(ValidatedAssetPackage {
        summary: AssetPackageSummary {
            id: manifest.id,
            asset_type: manifest.asset_type,
            version: manifest.version,
            name: manifest.name,
            publisher: manifest.publisher,
            license: manifest.license,
            renderer_backend: manifest.render.map(|render| render.backend),
            file_count,
            total_bytes,
        },
        renderer,
        files,
        media_types,
        preview_poster,
        theme,
        voice,
        integrity_path: integrity_path.to_path_buf(),
        integrity_bytes,
    })
}

fn validate_package_tree(
    source_root: &Path,
    files: &[InstallFile],
    integrity_path: &Path,
) -> Result<(), InstallError> {
    let canonical_root = source_root.canonicalize()?;
    let mut expected = files
        .iter()
        .map(|file| safe_relative_path(&file.relative_path).map(Path::to_path_buf))
        .collect::<Result<std::collections::HashSet<_>, _>>()?;
    expected.insert(integrity_path.to_path_buf());
    let mut discovered = std::collections::HashSet::with_capacity(expected.len());
    let mut directories = vec![canonical_root.clone()];
    while let Some(directory) = directories.pop() {
        for entry in fs::read_dir(directory)? {
            let entry = entry?;
            let metadata = fs::symlink_metadata(entry.path())?;
            let relative = entry
                .path()
                .strip_prefix(&canonical_root)
                .map_err(|_| InstallError::EscapedSource(entry.path()))?
                .to_path_buf();
            if metadata.file_type().is_symlink() {
                return Err(InstallError::EscapedSource(relative));
            }
            if metadata.is_dir() {
                directories.push(entry.path());
            } else if metadata.is_file() {
                discovered.insert(relative);
            } else {
                return Err(InstallError::InvalidMetadata(format!(
                    "unsupported package entry: {}",
                    relative.display()
                )));
            }
        }
    }
    if discovered != expected {
        return Err(InstallError::InvalidMetadata(
            "package tree does not exactly match the integrity inventory".to_owned(),
        ));
    }
    Ok(())
}

fn read_metadata(source_root: &Path, relative_path: &Path) -> Result<Vec<u8>, InstallError> {
    if !source_root.is_dir() {
        return Err(InstallError::SourceNotDirectory);
    }
    let relative_path = safe_relative_path(relative_path)?;
    let source_root = source_root.canonicalize()?;
    let path = source_root.join(relative_path);
    let metadata = fs::symlink_metadata(&path)?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_METADATA_BYTES {
        return Err(InstallError::InvalidMetadata(
            relative_path.display().to_string(),
        ));
    }
    let canonical_path = path.canonicalize()?;
    if !canonical_path.starts_with(&source_root) {
        return Err(InstallError::EscapedSource(relative_path.to_path_buf()));
    }
    fs::read(canonical_path).map_err(InstallError::from)
}

fn load_asset_renderer(
    source_root: &Path,
    manifest: &AssetManifestHeader,
    inventory: &[AssetIntegrityFile],
) -> Result<Option<AssetRendererDescriptor>, InstallError> {
    let Some(render) = manifest.render.as_ref() else {
        return Ok(None);
    };
    if ["gltf", "vrm"].contains(&render.backend.as_str()) {
        let model = manifest
            .entrypoints
            .as_ref()
            .and_then(|entrypoints| entrypoints.model.as_ref())
            .ok_or_else(|| {
                InstallError::InvalidMetadata(
                    "model renderer requires entrypoints.model".to_owned(),
                )
            })?;
        let model = safe_relative_path(model)?;
        require_inventory_media(inventory, model, &["model/gltf-binary"])?;
        let expected_extension = if render.backend == "vrm" {
            "vrm"
        } else {
            "glb"
        };
        if model.extension().and_then(|value| value.to_str()) != Some(expected_extension) {
            return Err(InstallError::InvalidMetadata(format!(
                "{} renderer model must use .{expected_extension}",
                render.backend
            )));
        }
        let report = nimora_model_importer::probe_model_bytes(&fs::read(source_root.join(model))?)
            .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
        let expected_format = if render.backend == "gltf" {
            "glb"
        } else {
            "vrm"
        };
        if report.format != expected_format {
            return Err(InstallError::InvalidMetadata(format!(
                "model content is {}, not {}",
                report.format, render.backend
            )));
        }
        return Ok(Some(AssetRendererDescriptor {
            backend: render.backend.clone(),
            canvas: render.canvas.clone(),
            anchor: render.anchor.clone(),
            default_scale: render.default_scale,
            pixel_art: render.pixel_art,
            fallbacks: manifest.fallbacks.clone(),
            clips: None,
            model: Some(model.to_path_buf()),
            animation_map: load_model_animation_map(source_root, manifest, inventory)?,
        }));
    }
    if !["sprite-sequence", "sprite-atlas"].contains(&render.backend.as_str()) {
        if manifest
            .entrypoints
            .as_ref()
            .and_then(|entrypoints| entrypoints.clips.as_ref())
            .is_some()
        {
            return Err(InstallError::InvalidMetadata(
                "entrypoints.clips is only valid for sprite renderers".to_owned(),
            ));
        }
        return Ok(None);
    }
    let clips_path = manifest
        .entrypoints
        .as_ref()
        .and_then(|entrypoints| entrypoints.clips.as_ref())
        .ok_or_else(|| {
            InstallError::InvalidMetadata(
                "sprite characters and skins require entrypoints.clips".to_owned(),
            )
        })?;
    let clips_path = safe_relative_path(clips_path)?;
    require_inventory_media(inventory, clips_path, &["application/json"])?;
    let clips: SpriteClips = serde_json::from_slice(&read_metadata(source_root, clips_path)?)
        .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    validate_sprite_clips(&clips, &render.backend, inventory)?;
    Ok(Some(AssetRendererDescriptor {
        backend: render.backend.clone(),
        canvas: render.canvas.clone(),
        anchor: render.anchor.clone(),
        default_scale: render.default_scale,
        pixel_art: render.pixel_art,
        fallbacks: manifest.fallbacks.clone(),
        clips: Some(clips),
        model: None,
        animation_map: None,
    }))
}

fn load_model_animation_map(
    source_root: &Path,
    manifest: &AssetManifestHeader,
    inventory: &[AssetIntegrityFile],
) -> Result<Option<ModelAnimationMap>, InstallError> {
    let Some(path) = manifest
        .entrypoints
        .as_ref()
        .and_then(|entrypoints| entrypoints.animation_graph.as_ref())
    else {
        return Ok(None);
    };
    let path = safe_relative_path(path)?;
    require_inventory_media(inventory, path, &["application/json"])?;
    let animation_map: ModelAnimationMap =
        serde_json::from_slice(&read_metadata(source_root, path)?)
            .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    if animation_map.spec != "nimora.animation-map/1" {
        return Err(InstallError::InvalidMetadata(
            "model animation map is invalid".to_owned(),
        ));
    }
    validate_model_animation_bindings(&animation_map.clips)?;
    Ok(Some(animation_map))
}

fn load_theme_descriptor(
    source_root: &Path,
    manifest: &AssetManifestHeader,
    inventory: &[AssetIntegrityFile],
) -> Result<Option<ThemeDescriptor>, InstallError> {
    let path = manifest
        .entrypoints
        .as_ref()
        .and_then(|entrypoints| entrypoints.theme.as_ref());
    if manifest.asset_type != "theme" {
        if path.is_some() {
            return Err(InstallError::InvalidMetadata(
                "only theme assets may declare a theme entrypoint".to_owned(),
            ));
        }
        return Ok(None);
    }
    let path = path.ok_or_else(|| {
        InstallError::InvalidMetadata("theme asset requires entrypoints.theme".to_owned())
    })?;
    let path = safe_relative_path(path)?;
    require_inventory_media(inventory, path, &["application/json"])?;
    let descriptor: ThemeDescriptor = serde_json::from_slice(&read_metadata(source_root, path)?)
        .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    validate_theme_descriptor(&descriptor)?;
    Ok(Some(descriptor))
}

fn validate_theme_descriptor(theme: &ThemeDescriptor) -> Result<(), InstallError> {
    const REQUIRED: [&str; 9] = [
        "surface",
        "surfaceElevated",
        "text",
        "textMuted",
        "accent",
        "accentSoft",
        "border",
        "success",
        "danger",
    ];
    let valid_color = |value: &str| {
        matches!(value.len(), 7 | 9)
            && value.starts_with('#')
            && value[1..].bytes().all(|byte| byte.is_ascii_hexdigit())
    };
    if theme.spec != "nimora.theme/1"
        || theme.colors.len() != REQUIRED.len()
        || REQUIRED.iter().any(|token| {
            !theme
                .colors
                .get(*token)
                .is_some_and(|value| valid_color(value))
        })
    {
        return Err(InstallError::InvalidMetadata(
            "theme descriptor violates nimora.theme/1".to_owned(),
        ));
    }
    validate_theme_contrast(theme)?;
    Ok(())
}

fn load_voice_descriptor(
    source_root: &Path,
    manifest: &AssetManifestHeader,
    inventory: &[AssetIntegrityFile],
) -> Result<Option<VoiceDescriptor>, InstallError> {
    let path = manifest
        .entrypoints
        .as_ref()
        .and_then(|entrypoints| entrypoints.voice.as_ref());
    if manifest.asset_type != "voice" {
        if path.is_some() {
            return Err(InstallError::InvalidMetadata(
                "only voice assets may declare a voice entrypoint".to_owned(),
            ));
        }
        return Ok(None);
    }
    let path = safe_relative_path(path.ok_or_else(|| {
        InstallError::InvalidMetadata("voice asset requires entrypoints.voice".to_owned())
    })?)?;
    require_inventory_media(inventory, path, &["application/json"])?;
    let descriptor: VoiceDescriptor = serde_json::from_slice(&read_metadata(source_root, path)?)
        .map_err(|error| InstallError::InvalidMetadata(error.to_string()))?;
    validate_voice_descriptor(&descriptor, inventory)?;
    Ok(Some(descriptor))
}

fn validate_voice_descriptor(
    voice: &VoiceDescriptor,
    inventory: &[AssetIntegrityFile],
) -> Result<(), InstallError> {
    if voice.spec != "nimora.voice/1" || voice.clips.is_empty() || voice.clips.len() > 32 {
        return Err(InstallError::InvalidMetadata(
            "voice descriptor violates nimora.voice/1".to_owned(),
        ));
    }
    for (cue, clip) in &voice.clips {
        if !valid_action_id(cue)
            || !VOICE_V1_CUES.contains(&cue.as_str())
            || !clip.gain_db.is_finite()
            || !(-24.0..=6.0).contains(&clip.gain_db)
            || clip.captions.is_empty()
            || clip.captions.len() > 16
            || clip.captions.iter().any(|(locale, caption)| {
                !valid_locale(locale)
                    || caption.trim().is_empty()
                    || caption.chars().count() > 160
                    || caption.chars().any(char::is_control)
            })
        {
            return Err(InstallError::InvalidMetadata(
                "voice clip metadata is invalid".to_owned(),
            ));
        }
        let path = safe_relative_path(&clip.file)?;
        require_inventory_media(inventory, path, &["audio/wav", "audio/ogg"])?;
        let file = inventory
            .iter()
            .find(|file| file.path == path)
            .ok_or_else(|| {
                InstallError::InvalidMetadata("voice clip is absent from inventory".to_owned())
            })?;
        if file.bytes == 0 || file.bytes > 2 * 1_024 * 1_024 {
            return Err(InstallError::InvalidMetadata(
                "voice clip exceeds the 2 MiB budget".to_owned(),
            ));
        }
    }
    Ok(())
}

fn read_voice_audio(
    source_root: &Path,
    cue: &str,
    clip: &VoiceClipDescriptor,
    media_types: &BTreeMap<PathBuf, String>,
) -> Result<AssetPreviewAudio, InstallError> {
    let path = safe_relative_path(&clip.file)?;
    let media_type = media_types.get(path).ok_or_else(|| {
        InstallError::InvalidMetadata("voice clip is absent from inventory".to_owned())
    })?;
    let bytes = fs::read(source_root.join(path))?;
    let valid_header = match media_type.as_str() {
        "audio/wav" => bytes.len() >= 12 && &bytes[..4] == b"RIFF" && &bytes[8..12] == b"WAVE",
        "audio/ogg" => bytes.len() >= 4 && &bytes[..4] == b"OggS",
        _ => false,
    };
    if !valid_header || bytes.len() > 2 * 1_024 * 1_024 {
        return Err(InstallError::InvalidMetadata(
            "voice clip header or budget is invalid".to_owned(),
        ));
    }
    Ok(AssetPreviewAudio {
        cue: cue.to_owned(),
        media_type: media_type.clone(),
        bytes,
        captions: clip.captions.clone(),
        gain_db: clip.gain_db,
    })
}

fn validate_theme_contrast(theme: &ThemeDescriptor) -> Result<(), InstallError> {
    let backdrop = match theme.mode {
        ThemeMode::Light => [1.0, 1.0, 1.0],
        ThemeMode::Dark => [0.0, 0.0, 0.0],
    };
    let color = |token: &str| {
        let rgba = parse_theme_color(&theme.colors[token]);
        composite_rgb(rgba, backdrop)
    };
    let surface = color("surface");
    let elevated = composite_rgb(parse_theme_color(&theme.colors["surfaceElevated"]), surface);
    let checks = [
        ("text/surface", color("text"), surface, 4.5),
        ("text/elevated", color("text"), elevated, 4.5),
        ("textMuted/surface", color("textMuted"), surface, 3.0),
        ("accent/elevated", color("accent"), elevated, 3.0),
        ("success/elevated", color("success"), elevated, 3.0),
        ("danger/elevated", color("danger"), elevated, 3.0),
    ];
    if let Some((pair, _, _, minimum)) =
        checks
            .into_iter()
            .find(|(_, foreground, background, minimum)| {
                contrast_ratio(*foreground, *background) < *minimum
            })
    {
        return Err(InstallError::InvalidMetadata(format!(
            "theme contrast for {pair} is below {minimum}:1"
        )));
    }
    Ok(())
}

fn parse_theme_color(value: &str) -> [f64; 4] {
    let channel =
        |start| f64::from(u8::from_str_radix(&value[start..start + 2], 16).unwrap_or(0)) / 255.0;
    [
        channel(1),
        channel(3),
        channel(5),
        if value.len() == 9 { channel(7) } else { 1.0 },
    ]
}

fn composite_rgb(foreground: [f64; 4], background: [f64; 3]) -> [f64; 3] {
    [0, 1, 2]
        .map(|index| foreground[index] * foreground[3] + background[index] * (1.0 - foreground[3]))
}

fn contrast_ratio(left: [f64; 3], right: [f64; 3]) -> f64 {
    let luminance = |color: [f64; 3]| {
        let linear = color.map(|channel| {
            if channel <= 0.04045 {
                channel / 12.92
            } else {
                ((channel + 0.055) / 1.055).powf(2.4)
            }
        });
        0.2126 * linear[0] + 0.7152 * linear[1] + 0.0722 * linear[2]
    };
    let left = luminance(left);
    let right = luminance(right);
    (left.max(right) + 0.05) / (left.min(right) + 0.05)
}

fn valid_action_id(value: &str) -> bool {
    value.split('.').count() >= 2
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn validate_sprite_clips(
    document: &SpriteClips,
    expected_backend: &str,
    inventory: &[AssetIntegrityFile],
) -> Result<(), InstallError> {
    let image_media = ["image/png", "image/webp", "image/jpeg", "image/gif"];
    let (spec, backend, has_idle) = match document {
        SpriteClips::SpriteSequence { spec, clips } => {
            validate_clip_map(clips, |clip| {
                for frame in &clip.frames {
                    if !(16..=60_000).contains(&frame.duration_ms) {
                        return Err(invalid_sprite("frame duration is outside 16..60000ms"));
                    }
                    require_inventory_media(
                        inventory,
                        safe_relative_path(&frame.file)?,
                        &image_media,
                    )?;
                }
                Ok(())
            })?;
            (spec, "sprite-sequence", clips.contains_key("pet.idle"))
        }
        SpriteClips::SpriteAtlas { spec, image, clips } => {
            require_inventory_media(inventory, safe_relative_path(image)?, &image_media)?;
            validate_clip_map(clips, |clip| {
                for frame in &clip.frames {
                    if frame.width == 0
                        || frame.height == 0
                        || frame.x > 16_384
                        || frame.y > 16_384
                        || frame.width > 4_096
                        || frame.height > 4_096
                        || !(16..=60_000).contains(&frame.duration_ms)
                    {
                        return Err(invalid_sprite("atlas frame exceeds renderer bounds"));
                    }
                }
                Ok(())
            })?;
            (spec, "sprite-atlas", clips.contains_key("pet.idle"))
        }
    };
    if spec != "nimora.sprite-clips/1" || backend != expected_backend || !has_idle {
        return Err(invalid_sprite(
            "clips spec, backend, or required pet.idle action is invalid",
        ));
    }
    Ok(())
}

fn validate_clip_map<T>(
    clips: &BTreeMap<String, T>,
    mut validate: impl FnMut(&T) -> Result<(), InstallError>,
) -> Result<(), InstallError>
where
    T: ClipFrames,
{
    for (action, clip) in clips {
        if !valid_asset_identifier(action) || !(1..=1_000).contains(&clip.frame_count()) {
            return Err(invalid_sprite("action id or frame count is invalid"));
        }
        validate(clip)?;
    }
    Ok(())
}

trait ClipFrames {
    fn frame_count(&self) -> usize;
}

impl ClipFrames for SpriteSequenceClip {
    fn frame_count(&self) -> usize {
        self.frames.len()
    }
}

impl ClipFrames for SpriteAtlasClip {
    fn frame_count(&self) -> usize {
        self.frames.len()
    }
}

fn require_inventory_media(
    inventory: &[AssetIntegrityFile],
    path: &Path,
    allowed: &[&str],
) -> Result<(), InstallError> {
    let file = inventory
        .iter()
        .find(|file| file.path == path)
        .ok_or_else(|| {
            InstallError::InvalidMetadata(
                "asset resource is outside the verified inventory".to_owned(),
            )
        })?;
    if !allowed.contains(&file.media_type.as_str()) {
        return Err(InstallError::InvalidMetadata(
            "asset resource has a disallowed media type".to_owned(),
        ));
    }
    require_media_extension(path, &file.media_type)
}

fn require_media_extension(path: &Path, media_type: &str) -> Result<(), InstallError> {
    let expected_extension = match media_type {
        "application/json" => &["json"][..],
        "image/png" => &["png"][..],
        "image/webp" => &["webp"][..],
        "image/jpeg" => &["jpg", "jpeg"][..],
        "image/gif" => &["gif"][..],
        "model/gltf-binary" => &["glb", "vrm"][..],
        "audio/wav" => &["wav"][..],
        "audio/ogg" => &["ogg"][..],
        _ => &[][..],
    };
    let extension = path.extension().and_then(std::ffi::OsStr::to_str);
    if extension.is_none_or(|extension| !expected_extension.contains(&extension)) {
        return Err(InstallError::InvalidMetadata(
            "asset resource extension does not match its media type".to_owned(),
        ));
    }
    Ok(())
}

fn invalid_sprite(message: &str) -> InstallError {
    InstallError::InvalidMetadata(format!("invalid sprite renderer: {message}"))
}

fn validate_manifest_header(manifest: &AssetManifestHeader) -> Result<(), InstallError> {
    let supported_types = [
        "character",
        "skin",
        "theme",
        "behavior",
        "voice",
        "interaction",
        "bundle",
    ];
    if manifest.spec != "nimora.asset/1"
        || !valid_asset_identifier(&manifest.id)
        || !valid_asset_identifier(&manifest.publisher)
        || !supported_types.contains(&manifest.asset_type.as_str())
        || (["character", "skin"].contains(&manifest.asset_type.as_str())
            && manifest.render.is_none())
        || manifest.render.as_ref().is_some_and(|render| {
            !["sprite-sequence", "sprite-atlas", "vrm", "gltf"].contains(&render.backend.as_str())
                || render.canvas.width == 0
                || render.canvas.height == 0
                || render.canvas.width > 4_096
                || render.canvas.height > 4_096
                || !render.anchor.x.is_finite()
                || !render.anchor.y.is_finite()
                || !(0.0..=1.0).contains(&render.anchor.x)
                || !(0.0..=1.0).contains(&render.anchor.y)
                || !render.default_scale.is_finite()
                || render.default_scale <= 0.0
                || render.default_scale > 8.0
        })
        || !valid_semver(&manifest.version)
        || manifest.license.trim().is_empty()
        || manifest.name.is_empty()
        || !manifest
            .name
            .iter()
            .all(|(locale, text)| valid_locale(locale) && !text.trim().is_empty())
        || manifest
            .engines
            .get("nimora")
            .and_then(serde_json::Value::as_str)
            .is_none_or(str::is_empty)
        || manifest.integrity.algorithm != "sha256"
        || manifest.capabilities.len() > 64
        || !manifest
            .capabilities
            .iter()
            .all(|capability| valid_asset_identifier(capability))
        || manifest.locales.len() > 32
        || !manifest.locales.iter().all(|locale| valid_locale(locale))
        || !manifest.fallbacks.iter().all(|(action, fallback)| {
            valid_asset_identifier(action) && valid_asset_identifier(fallback)
        })
    {
        return Err(InstallError::InvalidMetadata(
            "manifest header violates nimora.asset/1".to_owned(),
        ));
    }
    if let Some(entrypoints) = &manifest.entrypoints {
        for path in [
            entrypoints.animation_graph.as_ref(),
            entrypoints.clips.as_ref(),
            entrypoints.model.as_ref(),
            entrypoints.hitboxes.as_ref(),
            entrypoints.preview_poster.as_ref(),
            entrypoints.theme.as_ref(),
            entrypoints.voice.as_ref(),
        ]
        .into_iter()
        .flatten()
        {
            safe_relative_path(path)?;
        }
    }
    let backend = manifest
        .render
        .as_ref()
        .map(|render| render.backend.as_str());
    let clips = manifest
        .entrypoints
        .as_ref()
        .and_then(|entrypoints| entrypoints.clips.as_ref());
    let model = manifest
        .entrypoints
        .as_ref()
        .and_then(|entrypoints| entrypoints.model.as_ref());
    if matches!(backend, Some("sprite-sequence" | "sprite-atlas")) != clips.is_some()
        || matches!(backend, Some("live2d" | "vrm" | "gltf")) != model.is_some()
    {
        return Err(InstallError::InvalidMetadata(
            "renderer entrypoint does not match its backend".to_owned(),
        ));
    }
    Ok(())
}

fn valid_locale(value: &str) -> bool {
    let mut parts = value.split('-');
    let language = parts.next().unwrap_or_default();
    let region = parts.next();
    parts.next().is_none()
        && language.len() == 2
        && language.bytes().all(|byte| byte.is_ascii_lowercase())
        && region.is_none_or(|region| {
            region.len() == 2 && region.bytes().all(|byte| byte.is_ascii_uppercase())
        })
}

fn validate_integrity_document(
    files: &[InstallFile],
    declared_total: u64,
    integrity_path: &Path,
) -> Result<(), InstallError> {
    validate_budget(files)?;
    let mut paths = std::collections::HashSet::with_capacity(files.len());
    let mut total = 0_u64;
    for file in files {
        let path = safe_relative_path(&file.relative_path)?;
        if !paths.insert(path.to_path_buf()) {
            return Err(InstallError::InvalidMetadata(format!(
                "duplicate inventory path: {}",
                path.display()
            )));
        }
        if path == integrity_path {
            return Err(InstallError::InvalidMetadata(
                "integrity inventory cannot hash itself".to_owned(),
            ));
        }
        total = total
            .checked_add(file.bytes)
            .ok_or(InstallError::BudgetExceeded)?;
    }
    if !paths.contains(Path::new(MANIFEST_FILE)) || total != declared_total {
        return Err(InstallError::InvalidMetadata(
            "inventory must include manifest.json and match totalBytes".to_owned(),
        ));
    }
    Ok(())
}

fn valid_asset_identifier(value: &str) -> bool {
    value.len() <= 128
        && value.split('.').count() >= 2
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn valid_semver(value: &str) -> bool {
    let core = value.split_once('-').map_or(value, |(core, _)| core);
    let mut segments = core.split('.');
    segments.clone().count() == 3
        && segments
            .all(|segment| !segment.is_empty() && segment.bytes().all(|b| b.is_ascii_digit()))
}

/// Copies a validated inventory into a staging directory and activates it atomically.
///
/// # Errors
///
/// Returns an error when the source is invalid, an inventory path escapes the
/// package root, a listed file is missing, or a filesystem operation fails.
pub fn install_atomically(
    source_root: &Path,
    active_path: &Path,
    files: &[InstallFile],
) -> Result<InstallResult, InstallError> {
    install_atomically_with_generated(source_root, active_path, files, &[])
}

/// Copies a validated inventory plus trusted generated files and activates it atomically.
///
/// # Errors
///
/// Returns an error when either inventory is unsafe, budgets are exceeded, a
/// generated path overlaps a source path, or a filesystem operation fails.
pub fn install_atomically_with_generated(
    source_root: &Path,
    active_path: &Path,
    files: &[InstallFile],
    generated_files: &[GeneratedInstallFile],
) -> Result<InstallResult, InstallError> {
    if !source_root.is_dir() {
        return Err(InstallError::SourceNotDirectory);
    }
    validate_budget(files)?;
    validate_generated_files(files, generated_files)?;
    let canonical_source_root = source_root.canonicalize()?;
    let parent = active_path
        .parent()
        .ok_or_else(|| InstallError::UnsafePath(active_path.to_path_buf()))?;
    fs::create_dir_all(parent)?;
    let staging = unique_sibling(active_path, "staging");
    fs::create_dir(&staging)?;
    let result = (|| {
        for file in files {
            let relative = safe_relative_path(&file.relative_path)?;
            let source = source_root.join(relative);
            if !source.is_file() {
                return Err(InstallError::MissingFile(file.relative_path.clone()));
            }
            let canonical_source = source.canonicalize()?;
            if !canonical_source.starts_with(&canonical_source_root) {
                return Err(InstallError::EscapedSource(file.relative_path.clone()));
            }
            validate_file(&canonical_source, file)?;
            let destination = staging.join(relative);
            if let Some(destination_parent) = destination.parent() {
                fs::create_dir_all(destination_parent)?;
            }
            fs::copy(source, destination)?;
        }
        for file in generated_files {
            let relative = safe_relative_path(&file.relative_path)?;
            let destination = staging.join(relative);
            if let Some(destination_parent) = destination.parent() {
                fs::create_dir_all(destination_parent)?;
            }
            fs::write(destination, &file.contents)?;
        }
        validate_inventory(&staging, files)?;
        let backup = if active_path.exists() {
            let backup = unique_sibling(active_path, "backup");
            fs::rename(active_path, &backup)?;
            Some(backup)
        } else {
            None
        };
        if let Err(error) = fs::rename(&staging, active_path) {
            if let Some(backup_path) = &backup {
                fs::rename(backup_path, active_path)?;
            }
            return Err(InstallError::Io(error));
        }
        Ok(InstallResult {
            active_path: active_path.to_path_buf(),
            backup_path: backup,
        })
    })();
    if staging.exists() {
        fs::remove_dir_all(&staging)?;
    }
    result
}

fn validate_generated_files(
    files: &[InstallFile],
    generated_files: &[GeneratedInstallFile],
) -> Result<(), InstallError> {
    let mut paths = std::collections::HashSet::with_capacity(files.len() + generated_files.len());
    for file in files {
        paths.insert(safe_relative_path(&file.relative_path)?.to_path_buf());
    }
    let mut total_bytes = files.iter().try_fold(0_u64, |total, file| {
        total
            .checked_add(file.bytes)
            .ok_or(InstallError::BudgetExceeded)
    })?;
    for file in generated_files {
        let relative = safe_relative_path(&file.relative_path)?.to_path_buf();
        if !paths.insert(relative.clone()) {
            return Err(InstallError::UnsafePath(relative));
        }
        total_bytes = total_bytes
            .checked_add(
                u64::try_from(file.contents.len()).map_err(|_| InstallError::BudgetExceeded)?,
            )
            .ok_or(InstallError::BudgetExceeded)?;
    }
    if paths.len() > MAX_FILES || total_bytes > MAX_TOTAL_BYTES {
        return Err(InstallError::BudgetExceeded);
    }
    Ok(())
}

/// Restores the newest backup next to an active asset directory.
///
/// # Errors
///
/// Returns an error when no backup exists or a filesystem operation fails.
pub fn rollback_latest(active_path: &Path) -> Result<RollbackResult, InstallError> {
    let backup = latest_backup(active_path)?.ok_or(InstallError::BackupUnavailable)?;
    let quarantine = active_path
        .exists()
        .then(|| unique_sibling(active_path, "failed"));
    if let Some(quarantine_path) = &quarantine {
        fs::rename(active_path, quarantine_path)?;
    }
    if let Err(error) = fs::rename(&backup, active_path) {
        if let Some(quarantine_path) = &quarantine {
            fs::rename(quarantine_path, active_path)?;
        }
        return Err(InstallError::Io(error));
    }
    Ok(RollbackResult {
        active_path: active_path.to_path_buf(),
        quarantined_path: quarantine,
    })
}

fn validate_inventory(root: &Path, files: &[InstallFile]) -> Result<(), InstallError> {
    for file in files {
        validate_file(&root.join(safe_relative_path(&file.relative_path)?), file)?;
    }
    Ok(())
}

fn latest_backup(active_path: &Path) -> Result<Option<PathBuf>, InstallError> {
    let parent = active_path
        .parent()
        .ok_or_else(|| InstallError::UnsafePath(active_path.to_path_buf()))?;
    let prefix = format!(
        "{}.backup.",
        active_path
            .file_name()
            .ok_or_else(|| InstallError::UnsafePath(active_path.to_path_buf()))?
            .to_string_lossy()
    );
    let mut backups = fs::read_dir(parent)?
        .filter_map(Result::ok)
        .filter(|entry| entry.file_name().to_string_lossy().starts_with(&prefix))
        .map(|entry| entry.path())
        .collect::<Vec<_>>();
    backups.sort_unstable();
    Ok(backups.pop())
}

fn validate_budget(files: &[InstallFile]) -> Result<(), InstallError> {
    if files.is_empty() || files.len() > MAX_FILES {
        return Err(InstallError::BudgetExceeded);
    }
    let total = files
        .iter()
        .try_fold(0_u64, |total, file| total.checked_add(file.bytes));
    if total.is_none_or(|total| total > MAX_TOTAL_BYTES) {
        return Err(InstallError::BudgetExceeded);
    }
    Ok(())
}

fn validate_file(path: &Path, expected: &InstallFile) -> Result<(), InstallError> {
    if expected.sha256.len() != 64
        || !expected
            .sha256
            .bytes()
            .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    {
        return Err(InstallError::InvalidHash(expected.relative_path.clone()));
    }
    if fs::metadata(path)?.len() != expected.bytes {
        return Err(InstallError::SizeMismatch(expected.relative_path.clone()));
    }
    let mut source = fs::File::open(path)?;
    let mut digest = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024].into_boxed_slice();
    loop {
        let read = source.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        digest.update(&buffer[..read]);
    }
    if format!("{:x}", digest.finalize()) != expected.sha256 {
        return Err(InstallError::HashMismatch(expected.relative_path.clone()));
    }
    Ok(())
}

fn safe_relative_path(path: &Path) -> Result<&Path, InstallError> {
    if path.as_os_str().is_empty()
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(InstallError::UnsafePath(path.to_path_buf()));
    }
    Ok(path)
}

fn unique_sibling(active_path: &Path, suffix: &str) -> PathBuf {
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos();
    active_path.with_file_name(format!(
        "{}.{}.{}",
        active_path
            .file_name()
            .unwrap_or_default()
            .to_string_lossy(),
        suffix,
        stamp
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn sha256(contents: &[u8]) -> String {
        format!("{:x}", Sha256::digest(contents))
    }

    fn model_glb(document: &serde_json::Value) -> Vec<u8> {
        let mut json = serde_json::to_vec(document).unwrap();
        while !json.len().is_multiple_of(4) {
            json.push(b' ');
        }
        let length = 20 + json.len();
        let mut bytes = Vec::with_capacity(length);
        bytes.extend_from_slice(b"glTF");
        bytes.extend_from_slice(&2_u32.to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(length).unwrap().to_le_bytes());
        bytes.extend_from_slice(&u32::try_from(json.len()).unwrap().to_le_bytes());
        bytes.extend_from_slice(&0x4e4f_534a_u32.to_le_bytes());
        bytes.extend_from_slice(&json);
        bytes
    }

    fn minimal_glb() -> Vec<u8> {
        model_glb(&serde_json::json!({ "asset": { "version": "2.0" } }))
    }

    fn write_vrm_package(root: &Path, model: &[u8]) {
        fs::create_dir_all(root.join("models")).unwrap();
        let manifest = serde_json::to_vec(&serde_json::json!({
            "spec": "nimora.asset/1", "id": "character.example.vrm", "type": "character",
            "version": "1.0.0", "name": { "en": "VRM" }, "publisher": "publisher.example",
            "license": "CC0-1.0", "engines": { "nimora": ">=0.1.0" },
            "render": { "backend": "vrm", "canvas": { "width": 512, "height": 512 },
                "anchor": { "x": 0.5, "y": 1.0 }, "defaultScale": 1.0, "pixelArt": false },
            "entrypoints": { "model": "models/character.vrm" }, "capabilities": [],
            "fallbacks": {}, "locales": ["en"],
            "integrity": { "algorithm": "sha256", "files": "integrity.json" }
        }))
        .unwrap();
        fs::write(root.join(MANIFEST_FILE), &manifest).unwrap();
        fs::write(root.join("models/character.vrm"), model).unwrap();
        let integrity = serde_json::to_vec(&serde_json::json!({
            "files": [
                { "path": MANIFEST_FILE, "sha256": sha256(&manifest), "bytes": manifest.len(), "mediaType": "application/json" },
                { "path": "models/character.vrm", "sha256": sha256(model), "bytes": model.len(), "mediaType": "model/gltf-binary" }
            ],
            "totalBytes": manifest.len() + model.len()
        })).unwrap();
        fs::write(root.join("integrity.json"), integrity).unwrap();
    }

    const ONE_PIXEL_PNG: &[u8] = &[
        137, 80, 78, 71, 13, 10, 26, 10, 0, 0, 0, 13, 73, 72, 68, 82, 0, 0, 0, 1, 0, 0, 0, 1, 8, 6,
        0, 0, 0, 31, 21, 196, 137, 0, 0, 0, 13, 73, 68, 65, 84, 8, 215, 99, 248, 207, 192, 240, 31,
        0, 5, 0, 1, 255, 137, 153, 61, 29, 0, 0, 0, 0, 73, 69, 78, 68, 174, 66, 96, 130,
    ];

    const SILENT_WAV: &[u8] = b"RIFF\x24\x00\x00\x00WAVEfmt \x10\x00\x00\x00\x01\x00\x01\x00\x40\x1f\x00\x00\x80\x3e\x00\x00\x02\x00\x10\x00data\x00\x00\x00\x00";

    fn valid_voice_descriptor() -> VoiceDescriptor {
        VoiceDescriptor {
            spec: "nimora.voice/1".to_owned(),
            clips: BTreeMap::from([(
                "pet.click".to_owned(),
                VoiceClipDescriptor {
                    file: "audio/click.wav".into(),
                    captions: BTreeMap::from([
                        ("zh-CN".to_owned(), "轻快的点击声".to_owned()),
                        ("en".to_owned(), "A light click".to_owned()),
                    ]),
                    gain_db: -3.0,
                },
            )]),
        }
    }

    fn valid_voice_inventory(bytes: u64) -> Vec<AssetIntegrityFile> {
        vec![AssetIntegrityFile {
            path: "audio/click.wav".into(),
            sha256: "0".repeat(64),
            bytes,
            media_type: "audio/wav".to_owned(),
        }]
    }

    fn write_voice_package(root: &Path, audio: &[u8], media_type: &str) {
        fs::create_dir_all(root.join("audio")).unwrap();
        fs::create_dir_all(root.join("voices")).unwrap();
        let voice = serde_json::to_vec(&serde_json::json!({
            "spec": "nimora.voice/1",
            "clips": {
                "pet.click": {
                    "file": "audio/click.wav",
                    "captions": { "zh-CN": "轻快的点击声", "en": "A light click" },
                    "gainDb": -3.0
                }
            }
        }))
        .unwrap();
        let manifest = serde_json::to_vec(&serde_json::json!({
            "spec": "nimora.asset/1", "id": "voice.example.chime", "type": "voice",
            "version": "1.0.0", "name": { "en": "Chime" }, "publisher": "publisher.example",
            "license": "MIT", "engines": { "nimora": ">=0.1.0" },
            "entrypoints": { "voice": "voices/voice.json" }, "capabilities": [], "fallbacks": {},
            "locales": ["zh-CN", "en"], "integrity": { "algorithm": "sha256", "files": "integrity.json" }
        })).unwrap();
        fs::write(root.join(MANIFEST_FILE), &manifest).unwrap();
        fs::write(root.join("voices/voice.json"), &voice).unwrap();
        fs::write(root.join("audio/click.wav"), audio).unwrap();
        let integrity = serde_json::to_vec(&serde_json::json!({
            "files": [
                { "path": MANIFEST_FILE, "sha256": sha256(&manifest), "bytes": manifest.len(), "mediaType": "application/json" },
                { "path": "voices/voice.json", "sha256": sha256(&voice), "bytes": voice.len(), "mediaType": "application/json" },
                { "path": "audio/click.wav", "sha256": sha256(audio), "bytes": audio.len(), "mediaType": media_type }
            ],
            "totalBytes": manifest.len() + voice.len() + audio.len()
        })).unwrap();
        fs::write(root.join("integrity.json"), integrity).unwrap();
    }

    fn write_preview_package(root: &Path, poster: &[u8]) {
        fs::create_dir_all(root.join("preview")).unwrap();
        fs::create_dir_all(root.join("themes")).unwrap();
        let theme = serde_json::to_vec(&serde_json::json!({
            "spec": "nimora.theme/1",
            "mode": "light",
            "colors": {
                "surface": "#f7f5ef", "surfaceElevated": "#fffdf8", "text": "#30322c",
                "textMuted": "#77786f", "accent": "#6f61ce", "accentSoft": "#eeeaff",
                "border": "#deddd6", "success": "#5f875b", "danger": "#a44f45"
            },
            "cornerStyle": "soft",
            "motion": "full"
        }))
        .unwrap();
        let manifest = serde_json::to_vec(&serde_json::json!({
            "spec": "nimora.asset/1",
            "id": "theme.example.preview",
            "type": "theme",
            "version": "1.0.0",
            "name": { "en": "Preview" },
            "publisher": "publisher.example",
            "license": "MIT",
            "engines": { "nimora": ">=0.1.0" },
            "entrypoints": { "previewPoster": "preview/poster.png", "theme": "themes/theme.json" },
            "capabilities": [],
            "fallbacks": {},
            "locales": ["en"],
            "integrity": { "algorithm": "sha256", "files": "integrity.json" }
        }))
        .unwrap();
        fs::write(root.join(MANIFEST_FILE), &manifest).unwrap();
        fs::write(root.join("preview/poster.png"), poster).unwrap();
        fs::write(root.join("themes/theme.json"), &theme).unwrap();
        let integrity = serde_json::to_vec(&serde_json::json!({
            "files": [
                {
                    "path": MANIFEST_FILE,
                    "sha256": sha256(&manifest),
                    "bytes": manifest.len(),
                    "mediaType": "application/json"
                },
                {
                    "path": "preview/poster.png",
                    "sha256": sha256(poster),
                    "bytes": poster.len(),
                    "mediaType": "image/png"
                },
                {
                    "path": "themes/theme.json",
                    "sha256": sha256(&theme),
                    "bytes": theme.len(),
                    "mediaType": "application/json"
                }
            ],
            "totalBytes": manifest.len() + poster.len() + theme.len()
        }))
        .unwrap();
        fs::write(root.join("integrity.json"), integrity).unwrap();
    }

    fn write_asset_package(root: &Path, asset_id: &str) {
        fs::create_dir_all(root.join("animations")).unwrap();
        fs::create_dir_all(root.join("sprites")).unwrap();
        let clips = serde_json::to_vec(&serde_json::json!({
            "spec": "nimora.sprite-clips/1",
            "backend": "sprite-atlas",
            "image": "sprites/atlas.webp",
            "clips": {
                "pet.idle": {
                    "loop": true,
                    "frames": [{ "x": 0, "y": 0, "width": 256, "height": 256, "durationMs": 100 }]
                }
            }
        }))
        .unwrap();
        let atlas = b"test-webp";
        let manifest = serde_json::to_vec(&serde_json::json!({
            "spec": "nimora.asset/1",
            "id": asset_id,
            "type": "character",
            "version": "1.0.0",
            "name": { "en": "Mochi" },
            "publisher": "publisher.example",
            "license": "MIT",
            "engines": { "nimora": ">=0.1.0" },
            "render": {
                "backend": "sprite-atlas",
                "canvas": { "width": 512, "height": 512 },
                "anchor": { "x": 0.5, "y": 1.0 },
                "defaultScale": 1.0,
                "pixelArt": false
            },
            "entrypoints": { "clips": "animations/clips.json" },
            "capabilities": [],
            "fallbacks": { "pet.sleep": "pet.idle" },
            "locales": ["en"],
            "integrity": { "algorithm": "sha256", "files": "integrity.json" }
        }))
        .unwrap();
        fs::write(root.join(MANIFEST_FILE), &manifest).unwrap();
        fs::write(root.join("animations/clips.json"), &clips).unwrap();
        fs::write(root.join("sprites/atlas.webp"), atlas).unwrap();
        let integrity = serde_json::to_vec(&serde_json::json!({
            "files": [
                {
                    "path": MANIFEST_FILE,
                    "sha256": sha256(&manifest),
                    "bytes": manifest.len(),
                    "mediaType": "application/json"
                },
                {
                    "path": "animations/clips.json",
                    "sha256": sha256(&clips),
                    "bytes": clips.len(),
                    "mediaType": "application/json"
                },
                {
                    "path": "sprites/atlas.webp",
                    "sha256": sha256(atlas),
                    "bytes": atlas.len(),
                    "mediaType": "image/webp"
                }
            ],
            "totalBytes": manifest.len() + clips.len() + atlas.len()
        }))
        .unwrap();
        fs::write(root.join("integrity.json"), integrity).unwrap();
    }

    fn write_asset_archive(source: &Path, archive_path: &Path) {
        let archive_file = fs::File::create(archive_path).unwrap();
        let mut archive = ZipWriter::new(archive_file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(0o100_644);
        let mut directories = vec![source.to_path_buf()];
        while let Some(directory) = directories.pop() {
            for entry in fs::read_dir(directory).unwrap() {
                let entry = entry.unwrap();
                if entry.file_type().unwrap().is_dir() {
                    directories.push(entry.path());
                    continue;
                }
                let relative = entry.path().strip_prefix(source).unwrap().to_path_buf();
                archive
                    .start_file(relative.to_string_lossy().replace('\\', "/"), options)
                    .unwrap();
                archive.write_all(&fs::read(entry.path()).unwrap()).unwrap();
            }
        }
        archive.finish().unwrap();
    }

    fn write_single_entry_archive(
        archive_path: &Path,
        name: &str,
        contents: &[u8],
        permissions: u32,
    ) {
        let archive_file = fs::File::create(archive_path).unwrap();
        let mut archive = ZipWriter::new(archive_file);
        let options = SimpleFileOptions::default()
            .compression_method(CompressionMethod::Deflated)
            .unix_permissions(permissions);
        archive.start_file(name, options).unwrap();
        archive.write_all(contents).unwrap();
        archive.finish().unwrap();
    }

    #[test]
    fn previews_an_inventory_verified_static_poster() {
        let root = std::env::temp_dir().join("nimora-installer-preview-poster");
        let _ = fs::remove_dir_all(&root);
        write_preview_package(&root, ONE_PIXEL_PNG);

        let report = inspect_asset_source_preview(&root).unwrap();
        let poster = report.poster.unwrap();
        assert_eq!(report.summary.id, "theme.example.preview");
        assert_eq!(poster.media_type, "image/png");
        assert_eq!((poster.width, poster.height), (1, 1));
        assert_eq!(poster.bytes, ONE_PIXEL_PNG);
        let theme = inspect_asset_theme(&root).unwrap();
        assert_eq!(theme.spec, "nimora.theme/1");
        assert_eq!(theme.colors["accent"], "#6f61ce");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_theme_css_injection_and_unknown_tokens() {
        let mut colors = BTreeMap::from([
            ("surface".to_owned(), "#f7f5ef".to_owned()),
            ("surfaceElevated".to_owned(), "#fffdf8".to_owned()),
            ("text".to_owned(), "#30322c".to_owned()),
            ("textMuted".to_owned(), "#77786f".to_owned()),
            ("accent".to_owned(), "url(file:///secret)".to_owned()),
            ("accentSoft".to_owned(), "#eeeaff".to_owned()),
            ("border".to_owned(), "#deddd6".to_owned()),
            ("success".to_owned(), "#5f875b".to_owned()),
            ("danger".to_owned(), "#a44f45".to_owned()),
        ]);
        let theme = ThemeDescriptor {
            spec: "nimora.theme/1".to_owned(),
            mode: ThemeMode::Light,
            colors: colors.clone(),
            corner_style: ThemeCornerStyle::Soft,
            motion: ThemeMotion::Full,
        };
        assert!(validate_theme_descriptor(&theme).is_err());
        colors.insert("customCss".to_owned(), "#000000".to_owned());
        let unknown = ThemeDescriptor { colors, ..theme };
        assert!(validate_theme_descriptor(&unknown).is_err());
    }

    #[test]
    fn rejects_theme_text_and_semantic_colors_without_minimum_contrast() {
        let mut colors = BTreeMap::from([
            ("surface".to_owned(), "#ffffff".to_owned()),
            ("surfaceElevated".to_owned(), "#ffffff".to_owned()),
            ("text".to_owned(), "#fefefe".to_owned()),
            ("textMuted".to_owned(), "#777777".to_owned()),
            ("accent".to_owned(), "#666666".to_owned()),
            ("accentSoft".to_owned(), "#eeeeee".to_owned()),
            ("border".to_owned(), "#dddddd".to_owned()),
            ("success".to_owned(), "#557755".to_owned()),
            ("danger".to_owned(), "#995555".to_owned()),
        ]);
        let theme = ThemeDescriptor {
            spec: "nimora.theme/1".to_owned(),
            mode: ThemeMode::Light,
            colors: colors.clone(),
            corner_style: ThemeCornerStyle::Soft,
            motion: ThemeMotion::Full,
        };
        assert!(validate_theme_descriptor(&theme).is_err());
        colors.insert("text".to_owned(), "#111111".to_owned());
        colors.insert("danger".to_owned(), "#fdfdfd".to_owned());
        let semantic_failure = ThemeDescriptor { colors, ..theme };
        assert!(validate_theme_descriptor(&semantic_failure).is_err());
    }

    #[test]
    fn previews_and_reopens_a_verified_captioned_voice_clip() {
        let root = std::env::temp_dir().join("nimora-voice-preview");
        let _ = fs::remove_dir_all(&root);
        write_voice_package(&root, SILENT_WAV, "audio/wav");
        let report = inspect_asset_source_preview(&root).unwrap();
        let voice = report.voice.unwrap();
        assert_eq!(voice.spec, "nimora.voice/1");
        assert!((voice.clips["pet.click"].gain_db + 3.0).abs() < f64::EPSILON);
        let preview = report.voice_preview.unwrap();
        assert_eq!(preview.cue, "pet.click");
        assert_eq!(preview.media_type, "audio/wav");
        assert_eq!(preview.captions["zh-CN"], "轻快的点击声");
        assert_eq!(
            read_asset_voice_clip(&root, "pet.click").unwrap().bytes,
            SILENT_WAV
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_voice_spoofed_media_headers_and_invalid_metadata() {
        let root = std::env::temp_dir().join("nimora-voice-invalid-header");
        let _ = fs::remove_dir_all(&root);
        write_voice_package(&root, b"not-wave", "audio/wav");
        assert!(inspect_asset_source_preview(&root).is_err());
        fs::remove_dir_all(&root).unwrap();

        write_voice_package(&root, SILENT_WAV, "application/octet-stream");
        assert!(inspect_asset_source_preview(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_voice_descriptor_contract_boundaries() {
        let inventory = valid_voice_inventory(SILENT_WAV.len() as u64);

        let mut voice = valid_voice_descriptor();
        voice.spec = "nimora.voice/2".to_owned();
        assert!(validate_voice_descriptor(&voice, &inventory).is_err());

        let mut voice = valid_voice_descriptor();
        voice.clips.clear();
        assert!(validate_voice_descriptor(&voice, &inventory).is_err());

        let mut voice = valid_voice_descriptor();
        voice.clips.get_mut("pet.click").unwrap().gain_db = f64::NAN;
        assert!(validate_voice_descriptor(&voice, &inventory).is_err());

        let mut voice = valid_voice_descriptor();
        voice.clips.get_mut("pet.click").unwrap().gain_db = 6.1;
        assert!(validate_voice_descriptor(&voice, &inventory).is_err());

        let mut voice = valid_voice_descriptor();
        voice.clips.get_mut("pet.click").unwrap().captions.clear();
        assert!(validate_voice_descriptor(&voice, &inventory).is_err());

        let mut voice = valid_voice_descriptor();
        voice
            .clips
            .get_mut("pet.click")
            .unwrap()
            .captions
            .insert("bad locale!".to_owned(), "caption".to_owned());
        assert!(validate_voice_descriptor(&voice, &inventory).is_err());

        let mut voice = valid_voice_descriptor();
        voice.clips.get_mut("pet.click").unwrap().captions =
            BTreeMap::from([("en".to_owned(), "x".repeat(161))]);
        assert!(validate_voice_descriptor(&voice, &inventory).is_err());

        let mut voice = valid_voice_descriptor();
        let clip = voice.clips.remove("pet.click").unwrap();
        voice.clips.insert("platform.permission".to_owned(), clip);
        assert!(validate_voice_descriptor(&voice, &inventory).is_err());

        assert!(
            validate_voice_descriptor(
                &valid_voice_descriptor(),
                &valid_voice_inventory(2 * 1_024 * 1_024 + 1),
            )
            .is_err()
        );
    }

    #[test]
    fn rejects_voice_extension_mismatch_and_undeclared_cues() {
        let mut voice = valid_voice_descriptor();
        voice.clips.get_mut("pet.click").unwrap().file = "audio/click.ogg".into();
        let mut inventory = valid_voice_inventory(SILENT_WAV.len() as u64);
        inventory[0].path = "audio/click.ogg".into();
        assert!(validate_voice_descriptor(&voice, &inventory).is_err());

        let root = std::env::temp_dir().join("nimora-voice-undeclared-cue");
        let _ = fs::remove_dir_all(&root);
        write_voice_package(&root, SILENT_WAV, "audio/wav");
        assert!(read_asset_voice_clip(&root, "pet.missing").is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn voice_archive_round_trip_preserves_verified_preview() {
        let root = std::env::temp_dir().join("nimora-voice-archive-round-trip");
        let source = root.join("source");
        let archive = root.join("voice.nimora");
        let store = root.join("store");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        write_voice_package(&source, SILENT_WAV, "audio/wav");

        export_asset_package(&source, &archive).unwrap();
        let preview = inspect_asset_source_preview(&archive).unwrap();
        assert_eq!(preview.voice.unwrap().spec, "nimora.voice/1");
        assert_eq!(preview.voice_preview.unwrap().bytes, SILENT_WAV);

        let installed = install_asset_source(&archive, &store).unwrap();
        let reopened = read_asset_voice_clip(&installed.install.active_path, "pet.click").unwrap();
        assert_eq!(reopened.bytes, SILENT_WAV);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_a_preview_poster_with_an_invalid_image_header() {
        let root = std::env::temp_dir().join("nimora-installer-preview-invalid");
        let _ = fs::remove_dir_all(&root);
        write_preview_package(&root, b"not-a-png");

        let error = inspect_asset_source_preview(&root).unwrap_err();
        assert!(
            matches!(error, InstallError::InvalidMetadata(message) if message.contains("image header"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_a_preview_poster_above_the_dimension_budget() {
        let root = std::env::temp_dir().join("nimora-installer-preview-dimensions");
        let _ = fs::remove_dir_all(&root);
        let mut poster = ONE_PIXEL_PNG.to_vec();
        poster[16..20].copy_from_slice(&4097_u32.to_be_bytes());
        write_preview_package(&root, &poster);

        assert!(matches!(
            inspect_asset_source_preview(&root),
            Err(InstallError::BudgetExceeded)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installs_package_using_manifest_owned_identity_and_inventory() {
        let root = std::env::temp_dir().join("nimora-package-authority");
        let source = root.join("source");
        let store = root.join("store");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&source, "character.example.mochi");
        let result = install_asset_package(&source, &store).unwrap();
        assert_eq!(result.asset_id, "character.example.mochi");
        assert_eq!(result.version, "1.0.0");
        assert!(
            store
                .join("character.example.mochi/manifest.json")
                .is_file()
        );
        assert!(
            store
                .join("character.example.mochi/integrity.json")
                .is_file()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exposes_verified_sprite_renderer_without_filesystem_paths() {
        let root = std::env::temp_dir().join("nimora-package-renderer");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&root, "character.example.mochi");
        let renderer = inspect_asset_renderer(&root).unwrap();
        assert_eq!(renderer.backend, "sprite-atlas");
        assert_eq!(renderer.canvas.width, 512);
        assert_eq!(
            renderer.fallbacks.get("pet.sleep").map(String::as_str),
            Some("pet.idle")
        );
        assert!(renderer.model.is_none());
        match renderer.clips.expect("sprite renderer clips") {
            SpriteClips::SpriteAtlas { image, clips, .. } => {
                assert_eq!(image, Path::new("sprites/atlas.webp"));
                assert!(clips.contains_key("pet.idle"));
            }
            SpriteClips::SpriteSequence { .. } => panic!("expected atlas"),
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inspection_reports_verified_package_identity_and_budget() {
        let root = std::env::temp_dir().join("nimora-package-preview");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&root, "character.example.mochi");
        let summary = inspect_asset_package(&root).unwrap();
        assert_eq!(summary.publisher, "publisher.example");
        assert_eq!(summary.license, "MIT");
        assert_eq!(summary.renderer_backend.as_deref(), Some("sprite-atlas"));
        assert_eq!(summary.file_count, 3);
        assert!(summary.total_bytes > 0);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn previews_and_installs_a_bounded_nimora_archive() {
        let root = std::env::temp_dir().join("nimora-archive-install");
        let source = root.join("source");
        let archive = root.join("mochi.nimora");
        let store = root.join("store");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        write_asset_package(&source, "character.example.mochi");
        write_asset_archive(&source, &archive);
        let summary = inspect_asset_source(&archive).unwrap();
        assert_eq!(summary.id, "character.example.mochi");
        let result = install_asset_source(&archive, &store).unwrap();
        assert_eq!(result.asset_id, summary.id);
        assert!(
            store
                .join("character.example.mochi/manifest.json")
                .is_file()
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn exports_deterministic_archives_that_round_trip() {
        let root = std::env::temp_dir().join("nimora-archive-export");
        let source = root.join("source");
        let output = root.join("output");
        let first = output.join("first.nimora");
        let second = output.join("second.nimora");
        let store = root.join("store");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&output).unwrap();
        write_asset_package(&source, "character.example.mochi");
        let summary = export_asset_package(&source, &first).unwrap();
        export_asset_package(&source, &second).unwrap();
        assert_eq!(fs::read(&first).unwrap(), fs::read(&second).unwrap());
        assert_eq!(inspect_asset_source(&first).unwrap(), summary);
        let installed = install_asset_source(&first, &store).unwrap();
        assert_eq!(installed.asset_id, summary.id);
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn invalid_export_never_replaces_an_existing_package() {
        let root = std::env::temp_dir().join("nimora-invalid-export");
        let source = root.join("source");
        let output = root.join("output");
        let destination = output.join("asset.nimora");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&output).unwrap();
        write_asset_package(&source, "character.example.mochi");
        fs::write(&destination, b"existing package").unwrap();
        fs::write(source.join("untracked.txt"), b"invalid").unwrap();
        assert!(export_asset_package(&source, &destination).is_err());
        assert_eq!(fs::read(&destination).unwrap(), b"existing package");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn export_destination_must_be_outside_the_source_tree() {
        let root = std::env::temp_dir().join("nimora-export-destination");
        let source = root.join("source");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&source, "character.example.mochi");
        let destination = source.join("asset.nimora");
        assert!(matches!(
            export_asset_package(&source, &destination),
            Err(InstallError::InvalidExportDestination)
        ));
        assert!(!destination.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_nested_archives_and_symbolic_link_entries() {
        let root = std::env::temp_dir().join("nimora-archive-entry-policy");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let nested = root.join("nested.nimora");
        write_single_entry_archive(&nested, "payload.zip", b"not-a-zip", 0o100_644);
        assert!(matches!(
            inspect_asset_source(&nested),
            Err(InstallError::InvalidArchive(_))
        ));
        let link = root.join("link.nimora");
        let archive_file = fs::File::create(&link).unwrap();
        let mut archive = ZipWriter::new(archive_file);
        archive
            .add_symlink(
                "manifest.json",
                "target",
                SimpleFileOptions::default().unix_permissions(0o777),
            )
            .unwrap();
        archive.finish().unwrap();
        assert!(matches!(
            inspect_asset_source(&link),
            Err(InstallError::InvalidArchive(_))
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_archive_path_escape_and_duplicate_files() {
        let root = std::env::temp_dir().join("nimora-archive-path-policy");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let escape = root.join("escape.nimora");
        write_single_entry_archive(&escape, "../outside.json", b"escape", 0o100_644);
        assert!(inspect_asset_source(&escape).is_err());

        let duplicate = root.join("duplicate.nimora");
        let archive_file = fs::File::create(&duplicate).unwrap();
        let mut archive = ZipWriter::new(archive_file);
        let options = SimpleFileOptions::default().unix_permissions(0o100_644);
        archive.start_file("manifest1.json", options).unwrap();
        archive.write_all(b"first").unwrap();
        archive.start_file("manifest2.json", options).unwrap();
        archive.write_all(b"second").unwrap();
        archive.finish().unwrap();
        let mut bytes = fs::read(&duplicate).unwrap();
        for offset in 0..=bytes.len() - b"manifest2.json".len() {
            if &bytes[offset..offset + b"manifest2.json".len()] == b"manifest2.json" {
                bytes[offset..offset + b"manifest1.json".len()].copy_from_slice(b"manifest1.json");
            }
        }
        fs::write(&duplicate, bytes).unwrap();
        assert!(inspect_asset_source(&duplicate).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_archive_bombs_before_package_validation() {
        let root = std::env::temp_dir().join("nimora-archive-ratio-budget");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let archive = root.join("bomb.nimora");
        write_single_entry_archive(&archive, "manifest.json", &vec![0; 1024 * 1024], 0o100_644);
        assert!(matches!(
            inspect_asset_source(&archive),
            Err(InstallError::BudgetExceeded)
        ));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn install_reopens_archive_and_preserves_active_asset_after_tampering() {
        let root = std::env::temp_dir().join("nimora-archive-tamper");
        let source = root.join("source");
        let archive = root.join("mochi.nimora");
        let store = root.join("store");
        let active = store.join("character.example.mochi");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        write_asset_package(&source, "character.example.mochi");
        write_asset_archive(&source, &archive);
        inspect_asset_source(&archive).unwrap();
        fs::create_dir_all(&active).unwrap();
        fs::write(active.join("sentinel.txt"), b"current").unwrap();
        fs::write(&archive, b"replaced after preview").unwrap();
        assert!(install_asset_source(&archive, &store).is_err());
        assert_eq!(fs::read(active.join("sentinel.txt")).unwrap(), b"current");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn reads_only_verified_inventory_images() {
        let root = std::env::temp_dir().join("nimora-package-image-reader");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&root, "character.example.mochi");
        let (image, media_type) =
            read_verified_asset_image(&root, Path::new("sprites/atlas.webp")).unwrap();
        assert_eq!(image, b"test-webp");
        assert_eq!(media_type, "image/webp");
        assert!(read_verified_asset_image(&root, Path::new("manifest.json")).is_err());
        assert!(read_verified_asset_image(&root, Path::new("sprites/missing.webp")).is_err());
        assert!(read_verified_asset_image(&root, Path::new("../outside.webp")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_sprite_backend_mismatch() {
        let root = std::env::temp_dir().join("nimora-package-renderer-mismatch");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&root, "character.example.mochi");
        let clips_path = root.join("animations/clips.json");
        let mut clips: serde_json::Value =
            serde_json::from_slice(&fs::read(&clips_path).unwrap()).unwrap();
        clips["backend"] = serde_json::Value::String("sprite-sequence".to_owned());
        fs::write(&clips_path, serde_json::to_vec(&clips).unwrap()).unwrap();
        assert!(inspect_asset_renderer(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_self_referential_integrity_inventory() {
        let root = std::env::temp_dir().join("nimora-package-self-integrity");
        let source = root.join("source");
        let store = root.join("store");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&source, "character.example.mochi");
        let integrity = fs::read(source.join("integrity.json")).unwrap();
        fs::write(
            source.join("integrity.json"),
            serde_json::to_vec(&serde_json::json!({
                "files": [{
                    "path": "integrity.json",
                    "sha256": sha256(&integrity),
                    "bytes": integrity.len(),
                    "mediaType": "application/json"
                }],
                "totalBytes": integrity.len()
            }))
            .unwrap(),
        )
        .unwrap();
        let error = install_asset_package(&source, &store).unwrap_err();
        assert!(matches!(error, InstallError::InvalidMetadata(_)));
        assert!(!store.exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn inspection_rejects_untracked_files() {
        let root = std::env::temp_dir().join("nimora-package-untracked");
        let source = root.join("source");
        let _ = fs::remove_dir_all(&root);
        write_asset_package(&source, "character.example.mochi");
        fs::write(source.join("injected.js"), b"unexpected").unwrap();
        let error = inspect_asset_package(&source).unwrap_err();
        assert!(matches!(error, InstallError::InvalidMetadata(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installs_files_and_preserves_previous_directory() {
        let root = std::env::temp_dir().join(format!(
            "nimora-installer-{}",
            unique_sibling(Path::new("x"), "test").display()
        ));
        let source = root.join("source");
        let active = root.join("active");
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("manifest.json"), b"new").unwrap();
        fs::create_dir_all(&active).unwrap();
        fs::write(active.join("old.txt"), b"old").unwrap();
        let result = install_atomically(
            &source,
            &active,
            &[InstallFile {
                relative_path: "manifest.json".into(),
                bytes: 3,
                sha256: "11507a0e2f5e69d5dfa40a62a1bd7b6ee57e6bcd85c67c9b8431b36fff21c437".into(),
            }],
        )
        .unwrap();
        assert_eq!(fs::read(active.join("manifest.json")).unwrap(), b"new");
        assert_eq!(
            fs::read(result.backup_path.unwrap().join("old.txt")).unwrap(),
            b"old"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_path_escape_before_copying() {
        let root = std::env::temp_dir().join("nimora-installer-escape");
        fs::create_dir_all(&root).unwrap();
        let error = install_atomically(
            &root,
            &root.join("active"),
            &[InstallFile {
                relative_path: "../secret".into(),
                bytes: 0,
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(error, InstallError::UnsafePath(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[cfg(unix)]
    #[test]
    fn rejects_symlink_escape_from_source_root() {
        use std::os::unix::fs::symlink;

        let root = std::env::temp_dir().join("nimora-installer-symlink");
        let source = root.join("source");
        fs::create_dir_all(&source).unwrap();
        fs::write(root.join("secret"), b"secret").unwrap();
        symlink(root.join("secret"), source.join("linked")).unwrap();
        let error = install_atomically(
            &source,
            &root.join("active"),
            &[InstallFile {
                relative_path: "linked".into(),
                bytes: 6,
                sha256: "2bb80d537b1da3e38bd30361aa855686bde0eacd7162fef6a25fe97bf527a25b".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(error, InstallError::EscapedSource(_)));
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_inventory_hash_mismatch_without_replacing_active() {
        let root = std::env::temp_dir().join("nimora-installer-hash");
        let source = root.join("source");
        let active = root.join("active");
        fs::create_dir_all(&source).unwrap();
        fs::create_dir_all(&active).unwrap();
        fs::write(source.join("manifest.json"), b"new").unwrap();
        fs::write(active.join("old.txt"), b"old").unwrap();
        let error = install_atomically(
            &source,
            &active,
            &[InstallFile {
                relative_path: "manifest.json".into(),
                bytes: 3,
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            }],
        )
        .unwrap_err();
        assert!(matches!(error, InstallError::HashMismatch(_)));
        assert_eq!(fs::read(active.join("old.txt")).unwrap(), b"old");
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn installs_trusted_generated_files_in_the_atomic_activation() {
        let root = std::env::temp_dir().join("nimora-installer-generated");
        let source = root.join("source");
        let active = root.join("active");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&source).unwrap();
        fs::write(source.join("manifest.json"), b"new").unwrap();
        install_atomically_with_generated(
            &source,
            &active,
            &[InstallFile {
                relative_path: "manifest.json".into(),
                bytes: 3,
                sha256: "11507a0e2f5e69d5dfa40a62a1bd7b6ee57e6bcd85c67c9b8431b36fff21c437".into(),
            }],
            &[GeneratedInstallFile {
                relative_path: ".integrity.json".into(),
                contents: b"trusted".to_vec(),
            }],
        )
        .unwrap();
        assert_eq!(
            fs::read(active.join(".integrity.json")).unwrap(),
            b"trusted"
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn normalizes_a_probed_glb_into_a_verified_character_package() {
        let root = std::env::temp_dir().join("nimora-installer-gltf-character");
        let staged = root.join("staged/character.glb");
        let store = root.join("assets");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(staged.parent().unwrap()).unwrap();
        let model = minimal_glb();
        fs::write(&staged, &model).unwrap();

        let result = install_gltf_character(
            &staged,
            &store,
            &GltfCharacterMetadata {
                id: "character.local.aurora".to_owned(),
                version: "1.0.0".to_owned(),
                name: "Aurora".to_owned(),
                publisher: "publisher.local".to_owned(),
                license: "LicenseRef-Proprietary".to_owned(),
                animation_map: BTreeMap::from([
                    (
                        "pet.idle".to_owned(),
                        ModelAnimationBinding {
                            animation: "Idle".to_owned(),
                            looped: true,
                        },
                    ),
                    (
                        "pet.walk".to_owned(),
                        ModelAnimationBinding {
                            animation: "Walk".to_owned(),
                            looped: true,
                        },
                    ),
                ]),
            },
        )
        .unwrap();

        assert_eq!(result.asset_id, "character.local.aurora");
        let active = store.join("character.local.aurora");
        assert_eq!(
            fs::read(active.join("models/character.glb")).unwrap(),
            model
        );
        let summary = inspect_asset_package(&active).unwrap();
        assert_eq!(summary.renderer_backend.as_deref(), Some("gltf"));
        assert_eq!(summary.file_count, 3);
        let renderer = inspect_asset_renderer(&active).unwrap();
        assert_eq!(renderer.backend, "gltf");
        assert!(renderer.clips.is_none());
        assert_eq!(
            renderer.model.as_deref(),
            Some(Path::new("models/character.glb"))
        );
        assert_eq!(
            renderer
                .animation_map
                .as_ref()
                .and_then(|map| map.clips.get("pet.walk"))
                .map(|binding| binding.animation.as_str()),
            Some("Walk")
        );
        assert_eq!(
            read_verified_asset_model(&active, Path::new("models/character.glb")).unwrap(),
            minimal_glb()
        );
        for forbidden in [
            Path::new("manifest.json"),
            Path::new(".integrity.json"),
            Path::new("models/other.glb"),
        ] {
            assert!(read_verified_asset_model(&active, forbidden).is_err());
        }
        let manifest: serde_json::Value =
            serde_json::from_slice(&fs::read(active.join(MANIFEST_FILE)).unwrap()).unwrap();
        assert_eq!(
            manifest
                .pointer("/entrypoints/model")
                .and_then(serde_json::Value::as_str),
            Some("models/character.glb")
        );
        fs::write(active.join("models/character.glb"), b"tampered").unwrap();
        assert!(read_verified_asset_model(&active, Path::new("models/character.glb")).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn local_model_import_cannot_replace_a_publisher_namespace() {
        let root = std::env::temp_dir().join("nimora-installer-gltf-namespace");
        let staged = root.join("character.glb");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(&staged, minimal_glb()).unwrap();
        let error = install_gltf_character(
            &staged,
            &root.join("assets"),
            &GltfCharacterMetadata {
                id: "character.publisher.aurora".to_owned(),
                version: "1.0.0".to_owned(),
                name: "Aurora".to_owned(),
                publisher: "publisher.local".to_owned(),
                license: "LicenseRef-Proprietary".to_owned(),
                animation_map: BTreeMap::new(),
            },
        )
        .unwrap_err();
        assert!(matches!(error, InstallError::InvalidMetadata(_)));
        assert!(!root.join("assets/character.publisher.aurora").exists());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_model_rejects_invalid_animation_maps() {
        let valid_binding = || ModelAnimationBinding {
            animation: "Idle".to_owned(),
            looped: true,
        };
        for animation_map in [
            BTreeMap::from([("pet.walk".to_owned(), valid_binding())]),
            BTreeMap::from([("Pet.Idle".to_owned(), valid_binding())]),
            BTreeMap::from([(
                "pet.idle".to_owned(),
                ModelAnimationBinding {
                    animation: " \n".to_owned(),
                    looped: true,
                },
            )]),
            BTreeMap::from([(
                "pet.idle".to_owned(),
                ModelAnimationBinding {
                    animation: "a".repeat(257),
                    looped: true,
                },
            )]),
        ] {
            assert!(validate_model_animation_bindings(&animation_map).is_err());
        }
    }

    #[test]
    fn static_generated_model_omits_animation_graph_entrypoint() {
        let root = std::env::temp_dir().join("nimora-installer-static-gltf");
        let staged = root.join("character.glb");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        fs::write(&staged, minimal_glb()).unwrap();
        install_gltf_character(
            &staged,
            &root.join("assets"),
            &GltfCharacterMetadata {
                id: "character.local.static".to_owned(),
                version: "1.0.0".to_owned(),
                name: "Static".to_owned(),
                publisher: "publisher.local".to_owned(),
                license: "LicenseRef-Proprietary".to_owned(),
                animation_map: BTreeMap::new(),
            },
        )
        .unwrap();
        let manifest: serde_json::Value = serde_json::from_slice(
            &fs::read(root.join("assets/character.local.static/manifest.json")).unwrap(),
        )
        .unwrap();
        assert!(manifest.pointer("/entrypoints/animationGraph").is_none());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn accepts_verified_vrm_one_and_rejects_disguised_glb() {
        let root = std::env::temp_dir().join("nimora-installer-vrm");
        let _ = fs::remove_dir_all(&root);
        let valid = model_glb(&serde_json::json!({
            "asset": { "version": "2.0" },
            "extensionsUsed": ["VRMC_vrm"],
            "extensions": { "VRMC_vrm": { "specVersion": "1.0", "meta": {}, "humanoid": {} } }
        }));
        write_vrm_package(&root, &valid);
        let renderer = inspect_asset_renderer(&root).unwrap();
        assert_eq!(renderer.backend, "vrm");
        assert_eq!(
            renderer.model.as_deref(),
            Some(Path::new("models/character.vrm"))
        );

        fs::remove_dir_all(&root).unwrap();
        write_vrm_package(&root, &minimal_glb());
        assert!(inspect_asset_renderer(&root).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn generated_theme_uses_verified_asset_pipeline_and_local_namespace() {
        let root = std::env::temp_dir().join("nimora-installer-generated-theme");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).unwrap();
        let metadata = GeneratedThemeMetadata {
            id: "theme.local.aurora".to_owned(),
            version: "1.0.0".to_owned(),
            name: BTreeMap::from([("zh-CN".to_owned(), "极光".to_owned())]),
            publisher: "publisher.local.user".to_owned(),
            license: "LicenseRef-Proprietary".to_owned(),
            theme: ThemeDescriptor {
                spec: "nimora.theme/1".to_owned(),
                mode: ThemeMode::Light,
                colors: BTreeMap::from([
                    ("surface".to_owned(), "#f7f5ef".to_owned()),
                    ("surfaceElevated".to_owned(), "#fffdf8".to_owned()),
                    ("text".to_owned(), "#30322c".to_owned()),
                    ("textMuted".to_owned(), "#77786f".to_owned()),
                    ("accent".to_owned(), "#6f61ce".to_owned()),
                    ("accentSoft".to_owned(), "#eeeaff".to_owned()),
                    ("border".to_owned(), "#deddd6".to_owned()),
                    ("success".to_owned(), "#5f875b".to_owned()),
                    ("danger".to_owned(), "#a44f45".to_owned()),
                ]),
                corner_style: ThemeCornerStyle::Soft,
                motion: ThemeMotion::Full,
            },
        };
        let installed = install_generated_theme(&root, &metadata).unwrap();
        assert_eq!(installed.asset_id, metadata.id);
        assert_eq!(
            inspect_asset_theme(&installed.install.active_path).unwrap(),
            metadata.theme
        );

        let mut invalid = metadata;
        invalid.id = "theme.publisher.aurora".to_owned();
        assert!(validate_generated_theme_metadata(&invalid).is_err());
        invalid.id = "theme.local.aurora".to_owned();
        invalid.name.insert("en".to_owned(), "x".repeat(129));
        assert!(validate_generated_theme_metadata(&invalid).is_err());
        invalid.name.insert("en".to_owned(), "Aurora".to_owned());
        invalid.license = "LicenseRef-Proprietary\nInjected".to_owned();
        assert!(validate_generated_theme_metadata(&invalid).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn rejects_generated_files_that_overlap_source_inventory() {
        let root = std::env::temp_dir().join("nimora-installer-generated-overlap");
        let source = root.join("source");
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&source).unwrap();
        let error = install_atomically_with_generated(
            &source,
            &root.join("active"),
            &[InstallFile {
                relative_path: "manifest.json".into(),
                bytes: 0,
                sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
            }],
            &[GeneratedInstallFile {
                relative_path: "manifest.json".into(),
                contents: Vec::new(),
            }],
        )
        .unwrap_err();
        assert!(
            matches!(error, InstallError::UnsafePath(path) if path == Path::new("manifest.json"))
        );
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn restores_latest_backup_and_quarantines_failed_version() {
        let root = std::env::temp_dir().join("nimora-installer-rollback");
        let active = root.join("character.example.mochi");
        let older = root.join("character.example.mochi.backup.1");
        let latest = root.join("character.example.mochi.backup.2");
        fs::create_dir_all(&active).unwrap();
        fs::create_dir_all(&older).unwrap();
        fs::create_dir_all(&latest).unwrap();
        fs::write(active.join("version"), b"broken").unwrap();
        fs::write(older.join("version"), b"one").unwrap();
        fs::write(latest.join("version"), b"two").unwrap();
        let result = rollback_latest(&active).unwrap();
        assert_eq!(fs::read(active.join("version")).unwrap(), b"two");
        assert_eq!(
            fs::read(result.quarantined_path.unwrap().join("version")).unwrap(),
            b"broken"
        );
        assert!(older.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
