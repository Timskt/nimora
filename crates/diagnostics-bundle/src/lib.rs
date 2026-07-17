use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

const REPORT_PATH: &str = "report.json";
const MANIFEST_PATH: &str = "manifest.json";
const MAX_REPORT_BYTES: usize = 256 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticReport {
    pub spec: String,
    pub generated_at_ms: u64,
    pub application: ApplicationSummary,
    pub system: SystemSummary,
    pub runtime: RuntimeSummary,
    pub data_protection: DataProtectionSummary,
    pub privacy: PrivacySummary,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ApplicationSummary {
    pub name: String,
    pub version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SystemSummary {
    pub os: String,
    pub architecture: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuntimeSummary {
    pub startup_mode: String,
    pub startup_reason: Option<String>,
    pub safety_mode: String,
    pub outbox_pending: u64,
    pub outbox_dead_letter: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DataProtectionSummary {
    pub database_schema: u32,
    pub backup_count: u64,
    pub latest_backup_at_ms: Option<u64>,
    pub pending_restore: bool,
    pub last_backup_error: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[allow(clippy::struct_excessive_bools)]
pub struct PrivacySummary {
    pub includes_logs: bool,
    pub includes_user_content: bool,
    pub includes_secrets: bool,
    pub includes_file_paths: bool,
    pub automatically_uploaded: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticBundleReceipt {
    pub spec: String,
    pub bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BundleManifest {
    spec: String,
    files: Vec<BundleFile>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct BundleFile {
    path: String,
    bytes: u64,
    sha256: String,
}

#[derive(Debug, thiserror::Error)]
pub enum DiagnosticBundleError {
    #[error("diagnostic destination must be an unused absolute zip path")]
    InvalidDestination,
    #[error("diagnostic report exceeds the size budget")]
    ReportTooLarge,
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error(transparent)]
    Json(#[from] serde_json::Error),
    #[error(transparent)]
    Zip(#[from] zip::result::ZipError),
}

/// Writes a redacted diagnostic report and its integrity manifest to a new ZIP archive.
///
/// # Errors
///
/// Returns [`DiagnosticBundleError::InvalidDestination`] unless `destination` is an unused,
/// absolute path ending in `.nimora-diagnostics.zip`. Serialization, archive creation, file-system
/// persistence, and integrity calculation errors are returned through their corresponding error
/// variants. Reports larger than the fixed safety budget return
/// [`DiagnosticBundleError::ReportTooLarge`].
pub fn export_diagnostic_bundle(
    report: &DiagnosticReport,
    destination: &Path,
) -> Result<DiagnosticBundleReceipt, DiagnosticBundleError> {
    validate_destination(destination)?;
    let report_bytes = serde_json::to_vec_pretty(report)?;
    if report_bytes.len() > MAX_REPORT_BYTES {
        return Err(DiagnosticBundleError::ReportTooLarge);
    }
    let manifest = BundleManifest {
        spec: "nimora.diagnostic-bundle-manifest/1".to_owned(),
        files: vec![BundleFile {
            path: REPORT_PATH.to_owned(),
            bytes: report_bytes.len() as u64,
            sha256: sha256_hex(&report_bytes),
        }],
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let partial = partial_path(destination);
    let result = write_bundle(&partial, &report_bytes, &manifest_bytes).and_then(|()| {
        let file = File::open(&partial)?;
        file.sync_all()?;
        fs::rename(&partial, destination)?;
        sync_parent(destination)
    });
    if let Err(error) = result {
        let _ = fs::remove_file(&partial);
        return Err(error);
    }
    let bytes = fs::metadata(destination)?.len();
    let sha256 = sha256_file(destination)?;
    Ok(DiagnosticBundleReceipt {
        spec: "nimora.diagnostic-bundle-receipt/1".to_owned(),
        bytes,
        sha256,
    })
}

fn validate_destination(destination: &Path) -> Result<(), DiagnosticBundleError> {
    let valid_name = destination
        .file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".nimora-diagnostics.zip"));
    let valid_parent = destination.parent().is_some_and(Path::is_dir);
    if !destination.is_absolute() || destination.exists() || !valid_name || !valid_parent {
        return Err(DiagnosticBundleError::InvalidDestination);
    }
    Ok(())
}

fn write_bundle(
    destination: &Path,
    report: &[u8],
    manifest: &[u8],
) -> Result<(), DiagnosticBundleError> {
    let file = File::create(destination)?;
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);
    archive.start_file(REPORT_PATH, options)?;
    archive.write_all(report)?;
    archive.start_file(MANIFEST_PATH, options)?;
    archive.write_all(manifest)?;
    archive.finish()?.sync_all()?;
    Ok(())
}

fn partial_path(destination: &Path) -> PathBuf {
    destination.with_file_name(format!(
        ".{}.partial",
        destination
            .file_name()
            .and_then(|name| name.to_str())
            .unwrap_or("nimora-diagnostics.zip")
    ))
}

#[cfg(unix)]
fn sync_parent(path: &Path) -> Result<(), DiagnosticBundleError> {
    if let Some(parent) = path.parent() {
        File::open(parent)?.sync_all()?;
    }
    Ok(())
}

#[cfg(not(unix))]
fn sync_parent(_path: &Path) -> Result<(), DiagnosticBundleError> {
    Ok(())
}

fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

fn sha256_file(path: &Path) -> Result<String, DiagnosticBundleError> {
    let mut file = File::open(path)?;
    let mut hasher = Sha256::new();
    let mut buffer = [0_u8; 16 * 1024];
    loop {
        let read = file.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report() -> DiagnosticReport {
        DiagnosticReport {
            spec: "nimora.diagnostic-report/1".to_owned(),
            generated_at_ms: 1_700_000_000_000,
            application: ApplicationSummary {
                name: "Nimora".to_owned(),
                version: "0.1.0".to_owned(),
            },
            system: SystemSummary {
                os: "test-os".to_owned(),
                architecture: "test-arch".to_owned(),
            },
            runtime: RuntimeSummary {
                startup_mode: "recovery".to_owned(),
                startup_reason: Some("database-unavailable".to_owned()),
                safety_mode: "normal".to_owned(),
                outbox_pending: 0,
                outbox_dead_letter: 0,
            },
            data_protection: DataProtectionSummary {
                database_schema: 1,
                backup_count: 2,
                latest_backup_at_ms: Some(1_699_000_000_000),
                pending_restore: false,
                last_backup_error: false,
            },
            privacy: PrivacySummary {
                includes_logs: false,
                includes_user_content: false,
                includes_secrets: false,
                includes_file_paths: false,
                automatically_uploaded: false,
            },
        }
    }

    #[test]
    fn exports_a_verified_private_bundle_without_sensitive_fields() {
        let root =
            std::env::temp_dir().join(format!("nimora-diagnostic-bundle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture");
        let destination = root.join("support.nimora-diagnostics.zip");
        let receipt = export_diagnostic_bundle(&report(), &destination).expect("export");
        assert_eq!(receipt.sha256.len(), 64);
        let file = File::open(&destination).expect("bundle");
        let mut archive = zip::ZipArchive::new(file).expect("archive");
        assert_eq!(archive.len(), 2);
        let report: DiagnosticReport =
            serde_json::from_reader(archive.by_name(REPORT_PATH).expect("report"))
                .expect("report json");
        assert!(!report.privacy.includes_secrets);
        assert!(!report.privacy.includes_user_content);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn refuses_overwrite_and_ambiguous_extensions() {
        let root = std::env::temp_dir().join(format!(
            "nimora-diagnostic-destination-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture");
        let existing = root.join("existing.nimora-diagnostics.zip");
        fs::write(&existing, b"keep").expect("existing");
        assert!(matches!(
            export_diagnostic_bundle(&report(), &existing),
            Err(DiagnosticBundleError::InvalidDestination)
        ));
        assert_eq!(fs::read(&existing).expect("preserved"), b"keep");
        assert!(export_diagnostic_bundle(&report(), &root.join("support.zip")).is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }
}
