use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::VecDeque;
use std::fs::{self, File, OpenOptions};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::{Path, PathBuf};
use zip::write::SimpleFileOptions;

const REPORT_PATH: &str = "report.json";
const MANIFEST_PATH: &str = "manifest.json";
const EVENTS_PATH: &str = "events.json";
const MAX_REPORT_BYTES: usize = 256 * 1024;
const MAX_EVENTS_BYTES: usize = 256 * 1024;
pub const MAX_DIAGNOSTIC_EVENTS: usize = 256;
const MILLIS_PER_DAY: u64 = 86_400_000;
const MAX_JOURNAL_FILES: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticReport {
    pub spec: String,
    pub generated_at_ms: u64,
    pub application: ApplicationSummary,
    pub system: SystemSummary,
    pub runtime: RuntimeSummary,
    pub data_protection: DataProtectionSummary,
    pub sources: DiagnosticSourcesSummary,
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
pub struct DiagnosticSourcesSummary {
    pub event_count: u64,
    pub event_retention_days: u64,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticSeverity {
    Info,
    Warning,
    Error,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticComponent {
    Application,
    Persistence,
    Backup,
    Security,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DiagnosticEventCode {
    ApplicationStarted,
    RecoveryModeStarted,
    ScheduledBackupCompleted,
    ScheduledBackupFailed,
    ContextAdmissionRejected,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticContextAdmissionAudit {
    pub reason: String,
    pub source_categories: Vec<String>,
    pub segment_count: u64,
    pub total_bytes: u64,
    pub trace_id: String,
    pub run_id: Option<String>,
    pub automation_id: Option<String>,
    pub action_id: Option<String>,
    pub command_execution_id: Option<String>,
    pub module_id: Option<String>,
    pub module_execution_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticEvent {
    pub occurred_at_ms: u64,
    pub severity: DiagnosticSeverity,
    pub component: DiagnosticComponent,
    pub code: DiagnosticEventCode,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub context_admission: Option<DiagnosticContextAdmissionAudit>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DiagnosticEventLog {
    pub spec: String,
    pub entries: Vec<DiagnosticEvent>,
}

#[derive(Debug, Clone)]
pub struct DiagnosticJournal {
    entries: VecDeque<DiagnosticEvent>,
}

impl Default for DiagnosticJournal {
    fn default() -> Self {
        Self {
            entries: VecDeque::with_capacity(MAX_DIAGNOSTIC_EVENTS),
        }
    }
}

impl DiagnosticJournal {
    pub fn record(&mut self, event: DiagnosticEvent) {
        if self.entries.len() == MAX_DIAGNOSTIC_EVENTS {
            self.entries.pop_front();
        }
        self.entries.push_back(event);
    }

    #[must_use]
    pub fn snapshot(&self) -> DiagnosticEventLog {
        DiagnosticEventLog {
            spec: "nimora.diagnostic-events/1".to_owned(),
            entries: self.entries.iter().cloned().collect(),
        }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DiagnosticJournalPolicy {
    pub retention_days: u64,
    pub max_segment_bytes: u64,
}

impl Default for DiagnosticJournalPolicy {
    fn default() -> Self {
        Self {
            retention_days: 14,
            max_segment_bytes: 1024 * 1024,
        }
    }
}

#[derive(Debug)]
pub struct PersistentDiagnosticJournal {
    memory: DiagnosticJournal,
    directory: Option<PathBuf>,
    policy: DiagnosticJournalPolicy,
    segment: Option<File>,
    segment_path: Option<PathBuf>,
    segment_bytes: u64,
    segment_day: u64,
    segment_sequence: u32,
}

impl Default for PersistentDiagnosticJournal {
    fn default() -> Self {
        Self::in_memory()
    }
}

impl PersistentDiagnosticJournal {
    #[must_use]
    pub fn in_memory() -> Self {
        Self {
            memory: DiagnosticJournal::default(),
            directory: None,
            policy: DiagnosticJournalPolicy::default(),
            segment: None,
            segment_path: None,
            segment_bytes: 0,
            segment_day: 0,
            segment_sequence: 0,
        }
    }

    /// Opens a persistent structured-event journal and loads its newest valid entries.
    ///
    /// # Errors
    ///
    /// Returns an I/O error when the journal directory cannot be created, is not a real directory,
    /// cannot be cleaned, or a new non-overwriting segment cannot be created.
    pub fn open(
        directory: &Path,
        policy: DiagnosticJournalPolicy,
        now_ms: u64,
    ) -> Result<Self, DiagnosticBundleError> {
        validate_journal_policy(policy)?;
        fs::create_dir_all(directory)?;
        let metadata = fs::symlink_metadata(directory)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(DiagnosticBundleError::InvalidJournalPolicy);
        }
        let current_day = now_ms / MILLIS_PER_DAY;
        let files = journal_files(directory)?;
        cleanup_journal_files(&files, current_day, policy.retention_days, None)?;
        let retained = journal_files(directory)?;
        let memory = load_journal(&retained)?;
        let (segment, segment_path, segment_sequence) =
            create_journal_segment(directory, current_day, now_ms)?;
        Ok(Self {
            memory,
            directory: Some(directory.to_owned()),
            policy,
            segment: Some(segment),
            segment_path: Some(segment_path),
            segment_bytes: 0,
            segment_day: current_day,
            segment_sequence,
        })
    }

    /// Persists one bounded structured event and retains it in the in-memory snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error when serialization, segment rotation, or durable storage fails.
    pub fn record(&mut self, event: DiagnosticEvent) -> Result<(), DiagnosticBundleError> {
        let serialized = serde_json::to_vec(&event)?;
        let serialized_bytes = u64::try_from(serialized.len())
            .map_err(|_| DiagnosticBundleError::InvalidJournalPolicy)?
            .saturating_add(1);
        if serialized_bytes > self.policy.max_segment_bytes {
            return Err(DiagnosticBundleError::InvalidJournalPolicy);
        }
        self.memory.record(event);
        if self.directory.is_none() {
            return Ok(());
        }
        let event_day = self
            .memory
            .entries
            .back()
            .map_or(self.segment_day, |entry| {
                entry.occurred_at_ms / MILLIS_PER_DAY
            });
        if event_day != self.segment_day
            || self.segment_bytes.saturating_add(serialized_bytes) > self.policy.max_segment_bytes
        {
            self.rotate(event_day)?;
        }
        let segment = self
            .segment
            .as_mut()
            .ok_or(DiagnosticBundleError::InvalidJournalPolicy)?;
        segment.write_all(&serialized)?;
        segment.write_all(b"\n")?;
        segment.sync_data()?;
        self.segment_bytes = self.segment_bytes.saturating_add(serialized_bytes);
        Ok(())
    }

    #[must_use]
    pub fn snapshot(&self) -> DiagnosticEventLog {
        self.memory.snapshot()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.memory.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.memory.is_empty()
    }

    fn rotate(&mut self, day: u64) -> Result<(), DiagnosticBundleError> {
        let directory = self
            .directory
            .as_deref()
            .ok_or(DiagnosticBundleError::InvalidJournalPolicy)?;
        self.segment_sequence = self.segment_sequence.saturating_add(1);
        let path = journal_segment_path(directory, day, std::process::id(), self.segment_sequence);
        self.segment = Some(open_new_private_file(&path)?);
        self.segment_path = Some(path.clone());
        self.segment_bytes = 0;
        self.segment_day = day;
        let files = journal_files(directory)?;
        cleanup_journal_files(
            &files,
            day,
            self.policy.retention_days,
            Some(path.as_path()),
        )?;
        Ok(())
    }
}

#[derive(Debug)]
struct JournalFile {
    path: PathBuf,
    day: u64,
}

fn validate_journal_policy(policy: DiagnosticJournalPolicy) -> Result<(), DiagnosticBundleError> {
    if policy.retention_days == 0 || policy.max_segment_bytes < 1024 {
        return Err(DiagnosticBundleError::InvalidJournalPolicy);
    }
    Ok(())
}

fn journal_files(directory: &Path) -> Result<Vec<JournalFile>, DiagnosticBundleError> {
    let mut files = Vec::new();
    for entry in fs::read_dir(directory)? {
        let entry = entry?;
        let metadata = entry.metadata()?;
        if !metadata.is_file() || entry.file_type()?.is_symlink() {
            continue;
        }
        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
            continue;
        };
        let Some(day) = parse_journal_day(&name) else {
            continue;
        };
        files.push(JournalFile {
            path: entry.path(),
            day,
        });
    }
    files.sort_by(|left, right| {
        left.day
            .cmp(&right.day)
            .then_with(|| left.path.cmp(&right.path))
    });
    Ok(files)
}

fn parse_journal_day(name: &str) -> Option<u64> {
    let rest = name.strip_prefix("events-")?;
    let (day, suffix) = rest.split_once('-')?;
    if Path::new(suffix).extension()?.to_str()? != "jsonl" {
        return None;
    }
    day.parse().ok()
}

fn cleanup_journal_files(
    files: &[JournalFile],
    current_day: u64,
    retention_days: u64,
    protected: Option<&Path>,
) -> Result<(), DiagnosticBundleError> {
    let first_retained_day = current_day.saturating_sub(retention_days.saturating_sub(1));
    let mut remaining = files.len();
    for file in files {
        if protected.is_some_and(|path| path == file.path) {
            continue;
        }
        if file.day < first_retained_day || remaining > MAX_JOURNAL_FILES {
            fs::remove_file(&file.path)?;
            remaining = remaining.saturating_sub(1);
        }
    }
    Ok(())
}

fn load_journal(files: &[JournalFile]) -> Result<DiagnosticJournal, DiagnosticBundleError> {
    let mut journal = DiagnosticJournal::default();
    for file in files {
        let reader = BufReader::new(File::open(&file.path)?);
        for line in reader.lines() {
            let Ok(line) = line else { continue };
            let Ok(event) = serde_json::from_str::<DiagnosticEvent>(&line) else {
                continue;
            };
            journal.record(event);
        }
    }
    Ok(journal)
}

fn create_journal_segment(
    directory: &Path,
    day: u64,
    now_ms: u64,
) -> Result<(File, PathBuf, u32), DiagnosticBundleError> {
    let seed = u32::try_from(now_ms % u64::from(u32::MAX)).unwrap_or(0);
    for offset in 0..1024_u32 {
        let sequence = seed.saturating_add(offset);
        let path = journal_segment_path(directory, day, std::process::id(), sequence);
        match open_new_private_file(&path) {
            Ok(file) => return Ok((file, path, sequence)),
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error.into()),
        }
    }
    Err(DiagnosticBundleError::InvalidJournalPolicy)
}

fn journal_segment_path(directory: &Path, day: u64, process_id: u32, sequence: u32) -> PathBuf {
    directory.join(format!("events-{day}-{process_id}-{sequence}.jsonl"))
}

fn open_new_private_file(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.create_new(true).append(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DiagnosticBundleSelection {
    pub include_events: bool,
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
    #[error("diagnostic events exceed the size budget")]
    EventsTooLarge,
    #[error("diagnostic journal policy or storage is invalid")]
    InvalidJournalPolicy,
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
    events: &DiagnosticEventLog,
    selection: DiagnosticBundleSelection,
    destination: &Path,
) -> Result<DiagnosticBundleReceipt, DiagnosticBundleError> {
    validate_destination(destination)?;
    let report_bytes = serde_json::to_vec_pretty(report)?;
    if report_bytes.len() > MAX_REPORT_BYTES {
        return Err(DiagnosticBundleError::ReportTooLarge);
    }
    let events_bytes = if selection.include_events {
        let bytes = serde_json::to_vec_pretty(events)?;
        if bytes.len() > MAX_EVENTS_BYTES {
            return Err(DiagnosticBundleError::EventsTooLarge);
        }
        Some(bytes)
    } else {
        None
    };
    let mut files = vec![BundleFile {
        path: REPORT_PATH.to_owned(),
        bytes: report_bytes.len() as u64,
        sha256: sha256_hex(&report_bytes),
    }];
    if let Some(bytes) = &events_bytes {
        files.push(BundleFile {
            path: EVENTS_PATH.to_owned(),
            bytes: bytes.len() as u64,
            sha256: sha256_hex(bytes),
        });
    }
    let manifest = BundleManifest {
        spec: "nimora.diagnostic-bundle-manifest/1".to_owned(),
        files,
    };
    let manifest_bytes = serde_json::to_vec_pretty(&manifest)?;
    let partial = partial_path(destination);
    let result = write_bundle(
        &partial,
        &report_bytes,
        events_bytes.as_deref(),
        &manifest_bytes,
    )
    .and_then(|()| {
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
    events: Option<&[u8]>,
    manifest: &[u8],
) -> Result<(), DiagnosticBundleError> {
    let file = File::create(destination)?;
    let mut archive = zip::ZipWriter::new(file);
    let options = SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated)
        .unix_permissions(0o600);
    archive.start_file(REPORT_PATH, options)?;
    archive.write_all(report)?;
    if let Some(events) = events {
        archive.start_file(EVENTS_PATH, options)?;
        archive.write_all(events)?;
    }
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
            sources: DiagnosticSourcesSummary {
                event_count: 1,
                event_retention_days: 14,
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

    fn events() -> DiagnosticEventLog {
        DiagnosticEventLog {
            spec: "nimora.diagnostic-events/1".to_owned(),
            entries: vec![DiagnosticEvent {
                occurred_at_ms: 1_700_000_000_001,
                severity: DiagnosticSeverity::Warning,
                component: DiagnosticComponent::Persistence,
                code: DiagnosticEventCode::RecoveryModeStarted,
                context_admission: None,
            }],
        }
    }

    fn event(occurred_at_ms: u64) -> DiagnosticEvent {
        DiagnosticEvent {
            occurred_at_ms,
            severity: DiagnosticSeverity::Info,
            component: DiagnosticComponent::Application,
            code: DiagnosticEventCode::ApplicationStarted,
            context_admission: None,
        }
    }

    #[test]
    fn exports_a_verified_private_bundle_without_sensitive_fields() {
        let root =
            std::env::temp_dir().join(format!("nimora-diagnostic-bundle-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture");
        let destination = root.join("support.nimora-diagnostics.zip");
        let receipt = export_diagnostic_bundle(
            &report(),
            &events(),
            DiagnosticBundleSelection {
                include_events: true,
            },
            &destination,
        )
        .expect("export");
        assert_eq!(receipt.sha256.len(), 64);
        let file = File::open(&destination).expect("bundle");
        let mut archive = zip::ZipArchive::new(file).expect("archive");
        assert_eq!(archive.len(), 3);
        let report: DiagnosticReport =
            serde_json::from_reader(archive.by_name(REPORT_PATH).expect("report"))
                .expect("report json");
        assert!(!report.privacy.includes_secrets);
        assert!(!report.privacy.includes_user_content);
        let event_log: DiagnosticEventLog =
            serde_json::from_reader(archive.by_name(EVENTS_PATH).expect("events"))
                .expect("events json");
        assert_eq!(event_log.entries.len(), 1);
        let event_bytes = serde_json::to_vec_pretty(&event_log).expect("event bytes");
        let manifest: BundleManifest =
            serde_json::from_reader(archive.by_name(MANIFEST_PATH).expect("manifest"))
                .expect("manifest json");
        let event_inventory = manifest
            .files
            .iter()
            .find(|file| file.path == EVENTS_PATH)
            .expect("event inventory");
        assert_eq!(event_inventory.bytes, event_bytes.len() as u64);
        assert_eq!(event_inventory.sha256, sha256_hex(&event_bytes));
        let serialized = String::from_utf8(event_bytes).expect("utf8 event log");
        for forbidden in ["message", "path", "secret", "content", "username"] {
            assert!(!serialized.to_lowercase().contains(forbidden));
        }
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
            export_diagnostic_bundle(
                &report(),
                &events(),
                DiagnosticBundleSelection::default(),
                &existing
            ),
            Err(DiagnosticBundleError::InvalidDestination)
        ));
        assert_eq!(fs::read(&existing).expect("preserved"), b"keep");
        assert!(
            export_diagnostic_bundle(
                &report(),
                &events(),
                DiagnosticBundleSelection::default(),
                &root.join("support.zip")
            )
            .is_err()
        );
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn journal_is_bounded_and_discards_oldest_entries() {
        let mut journal = DiagnosticJournal::default();
        for occurred_at_ms in 0..=MAX_DIAGNOSTIC_EVENTS as u64 {
            journal.record(event(occurred_at_ms));
        }
        let snapshot = journal.snapshot();
        assert_eq!(snapshot.entries.len(), MAX_DIAGNOSTIC_EVENTS);
        assert_eq!(snapshot.entries[0].occurred_at_ms, 1);
    }

    #[test]
    fn excludes_optional_events_when_user_cancels_them() {
        let root = std::env::temp_dir().join(format!(
            "nimora-diagnostic-selection-{}",
            std::process::id()
        ));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture");
        let destination = root.join("summary.nimora-diagnostics.zip");
        export_diagnostic_bundle(
            &report(),
            &events(),
            DiagnosticBundleSelection::default(),
            &destination,
        )
        .expect("export");
        let file = File::open(&destination).expect("bundle");
        let mut archive = zip::ZipArchive::new(file).expect("archive");
        assert_eq!(archive.len(), 2);
        assert!(archive.by_name(EVENTS_PATH).is_err());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn persistent_journal_rotates_and_recovers_valid_entries() {
        let root =
            std::env::temp_dir().join(format!("nimora-persistent-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        let now_ms = 100 * MILLIS_PER_DAY;
        let policy = DiagnosticJournalPolicy {
            retention_days: 14,
            max_segment_bytes: 1024,
        };
        let mut journal =
            PersistentDiagnosticJournal::open(&root, policy, now_ms).expect("open journal");
        for offset in 0..40 {
            journal.record(event(now_ms + offset)).expect("record");
        }
        assert!(journal_files(&root).expect("segments").len() > 1);
        drop(journal);

        let reopened =
            PersistentDiagnosticJournal::open(&root, policy, now_ms + 100).expect("reopen");
        let snapshot = reopened.snapshot();
        assert_eq!(snapshot.entries.len(), 40);
        assert_eq!(snapshot.entries[0].occurred_at_ms, now_ms);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn persistent_journal_ignores_corrupt_lines_and_prunes_expired_segments() {
        let root =
            std::env::temp_dir().join(format!("nimora-corrupt-journal-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(&root).expect("fixture");
        let now_ms = 100 * MILLIS_PER_DAY;
        let valid = serde_json::to_string(&event(now_ms)).expect("valid event");
        fs::write(
            root.join("events-100-1-1.jsonl"),
            format!("{valid}\n{{truncated"),
        )
        .expect("current fixture");
        fs::write(root.join("events-1-1-1.jsonl"), format!("{valid}\n")).expect("expired fixture");

        let journal =
            PersistentDiagnosticJournal::open(&root, DiagnosticJournalPolicy::default(), now_ms)
                .expect("open journal");
        assert_eq!(journal.len(), 1);
        assert!(!root.join("events-1-1-1.jsonl").exists());
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn persistent_journal_rejects_a_symbolic_link_directory() {
        use std::os::unix::fs::symlink;

        let root =
            std::env::temp_dir().join(format!("nimora-journal-symlink-{}", std::process::id()));
        let _ = fs::remove_dir_all(&root);
        fs::create_dir_all(root.join("real")).expect("real directory");
        symlink(root.join("real"), root.join("linked")).expect("symlink");
        assert!(matches!(
            PersistentDiagnosticJournal::open(
                &root.join("linked"),
                DiagnosticJournalPolicy::default(),
                100 * MILLIS_PER_DAY
            ),
            Err(DiagnosticBundleError::InvalidJournalPolicy)
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }
}
