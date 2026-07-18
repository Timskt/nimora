use ignore::WalkBuilder;
use nimora_agent_runtime::{TrackedWorkspaceFile, WorkspaceSnapshot};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use std::fs::{self, File, Metadata};
use std::io::{self, Read};
use std::path::{Component, Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use thiserror::Error;
use wait_timeout::ChildExt;

const READ_BUFFER_BYTES: usize = 64 * 1024;
const MAX_GIT_OUTPUT_BYTES: usize = 4 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorkspaceScanPolicy {
    pub max_files: usize,
    pub max_file_bytes: u64,
    pub max_total_bytes: u64,
    pub max_depth: usize,
    pub deadline: Duration,
}

impl Default for WorkspaceScanPolicy {
    fn default() -> Self {
        Self {
            max_files: 20_000,
            max_file_bytes: 64 * 1024 * 1024,
            max_total_bytes: 512 * 1024 * 1024,
            max_depth: 64,
            deadline: Duration::from_secs(15),
        }
    }
}

impl WorkspaceScanPolicy {
    fn validate(&self) -> Result<(), WorkspaceHostError> {
        if self.max_files == 0
            || self.max_files > 20_000
            || self.max_file_bytes == 0
            || self.max_file_bytes > 64 * 1024 * 1024
            || self.max_total_bytes < self.max_file_bytes
            || self.max_depth == 0
            || self.max_depth > 256
            || self.deadline.is_zero()
            || self.deadline > Duration::from_mins(2)
        {
            return Err(WorkspaceHostError::InvalidPolicy);
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
pub struct WorkspaceScanner {
    root: PathBuf,
    policy: WorkspaceScanPolicy,
}

impl WorkspaceScanner {
    /// Opens a canonical workspace root without retaining a renderer-provided path.
    ///
    /// # Errors
    ///
    /// Returns an error when the policy or root is unsafe.
    pub fn open(root: &Path, policy: WorkspaceScanPolicy) -> Result<Self, WorkspaceHostError> {
        policy.validate()?;
        let root = fs::canonicalize(root).map_err(WorkspaceHostError::Io)?;
        if !fs::metadata(&root)
            .map_err(WorkspaceHostError::Io)?
            .is_dir()
        {
            return Err(WorkspaceHostError::InvalidRoot);
        }
        Ok(Self { root, policy })
    }

    /// Produces an immutable domain snapshot using bounded, ignore-aware host reads.
    ///
    /// # Errors
    ///
    /// Fails closed on symlinks, path drift, limits, timeouts, or concurrent file mutation.
    pub fn scan(
        &self,
        revision: u64,
        parent_fingerprint: Option<String>,
        created_at_ms: u64,
    ) -> Result<WorkspaceSnapshot, WorkspaceHostError> {
        let started = Instant::now();
        let mut builder = WalkBuilder::new(&self.root);
        builder
            .follow_links(false)
            .max_depth(Some(self.policy.max_depth))
            .hidden(false)
            .git_ignore(true)
            .git_exclude(true)
            .require_git(false)
            .ignore(true)
            .parents(true)
            .add_custom_ignore_filename(".nimoraignore");

        let mut files = Vec::new();
        let mut total_bytes = 0_u64;
        for entry in builder.build() {
            self.check_deadline(started)?;
            let entry = entry.map_err(|_| WorkspaceHostError::Walk)?;
            if entry.path() == self.root {
                continue;
            }
            let file_type = entry.file_type().ok_or(WorkspaceHostError::Walk)?;
            if file_type.is_symlink() {
                return Err(WorkspaceHostError::SymbolicLink);
            }
            if !file_type.is_file() {
                continue;
            }
            if files.len() >= self.policy.max_files {
                return Err(WorkspaceHostError::FileLimit);
            }
            let relative_path = safe_relative_path(&self.root, entry.path())?;
            let (contents, metadata) = self.read_stable_file(entry.path(), started)?;
            let bytes = u64::try_from(contents.len()).map_err(|_| WorkspaceHostError::ByteLimit)?;
            total_bytes = total_bytes
                .checked_add(bytes)
                .ok_or(WorkspaceHostError::ByteLimit)?;
            if total_bytes > self.policy.max_total_bytes {
                return Err(WorkspaceHostError::ByteLimit);
            }
            files.push(TrackedWorkspaceFile::from_bytes(
                relative_path,
                &contents,
                executable(&metadata),
            )?);
        }
        WorkspaceSnapshot::new(revision, parent_fingerprint, files, created_at_ms)
            .map_err(WorkspaceHostError::Tracking)
    }

    fn read_stable_file(
        &self,
        path: &Path,
        started: Instant,
    ) -> Result<(Vec<u8>, Metadata), WorkspaceHostError> {
        let before = fs::symlink_metadata(path).map_err(WorkspaceHostError::Io)?;
        if !before.is_file() || before.file_type().is_symlink() {
            return Err(WorkspaceHostError::SymbolicLink);
        }
        if before.len() > self.policy.max_file_bytes {
            return Err(WorkspaceHostError::FileTooLarge);
        }
        let mut file = File::open(path).map_err(WorkspaceHostError::Io)?;
        let opened = file.metadata().map_err(WorkspaceHostError::Io)?;
        if !same_file_version(&before, &opened) {
            return Err(WorkspaceHostError::ConcurrentMutation);
        }
        let capacity =
            usize::try_from(opened.len()).map_err(|_| WorkspaceHostError::FileTooLarge)?;
        let mut contents = Vec::with_capacity(capacity);
        let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
        loop {
            self.check_deadline(started)?;
            let count = file.read(&mut buffer).map_err(WorkspaceHostError::Io)?;
            if count == 0 {
                break;
            }
            contents.extend_from_slice(&buffer[..count]);
            if contents.len() as u64 > self.policy.max_file_bytes {
                return Err(WorkspaceHostError::FileTooLarge);
            }
        }
        let after = file.metadata().map_err(WorkspaceHostError::Io)?;
        if !same_file_version(&opened, &after) || after.len() != contents.len() as u64 {
            return Err(WorkspaceHostError::ConcurrentMutation);
        }
        Ok((contents, after))
    }

    fn check_deadline(&self, started: Instant) -> Result<(), WorkspaceHostError> {
        if started.elapsed() > self.policy.deadline {
            Err(WorkspaceHostError::Deadline)
        } else {
            Ok(())
        }
    }

    #[must_use]
    pub fn root_fingerprint(&self) -> String {
        format!(
            "{:x}",
            Sha256::digest(self.root.as_os_str().as_encoded_bytes())
        )
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct GitWorkspaceState {
    pub spec: String,
    pub head_commit: Option<String>,
    pub head_tree: Option<String>,
    pub index_tree: Option<String>,
    pub branch: Option<String>,
    pub ahead: u64,
    pub behind: u64,
    pub changes: BTreeSet<GitWorkspaceChange>,
    pub fingerprint: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GitWorkspaceChange {
    Staged,
    Unstaged,
    Untracked,
    Conflict,
}

#[derive(Debug, Clone)]
pub struct GitWorkspaceAdapter {
    root: PathBuf,
    deadline: Duration,
}

impl GitWorkspaceAdapter {
    /// Creates a bounded Git adapter for a canonical workspace root.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid roots or deadlines.
    pub fn open(root: &Path, deadline: Duration) -> Result<Self, WorkspaceHostError> {
        if deadline.is_zero() || deadline > Duration::from_secs(30) {
            return Err(WorkspaceHostError::InvalidPolicy);
        }
        let root = fs::canonicalize(root).map_err(WorkspaceHostError::Io)?;
        if !root.is_dir() {
            return Err(WorkspaceHostError::InvalidRoot);
        }
        Ok(Self { root, deadline })
    }

    /// Reads HEAD, index, branch, dirty, untracked, and conflict state without shell expansion.
    ///
    /// # Errors
    ///
    /// Returns an error if Git is unavailable, times out, emits excessive data, or the repository is invalid.
    pub fn inspect(&self) -> Result<GitWorkspaceState, WorkspaceHostError> {
        let status = self.git(&["status", "--porcelain=v2", "--branch", "-z"])?;
        let head_commit =
            optional_oid(&self.git_allow_failure(&["rev-parse", "--verify", "HEAD"])?)?;
        let head_tree =
            optional_oid(&self.git_allow_failure(&["rev-parse", "--verify", "HEAD^{tree}"])?)?;
        let index_tree = optional_oid(&self.git_allow_failure(&["write-tree"])?)?;
        parse_git_state(&status, head_commit, head_tree, index_tree)
    }

    fn git(&self, arguments: &[&str]) -> Result<Vec<u8>, WorkspaceHostError> {
        let (success, output) = self.run_git(arguments)?;
        if !success {
            return Err(WorkspaceHostError::GitCommand);
        }
        Ok(output)
    }

    fn git_allow_failure(&self, arguments: &[&str]) -> Result<Vec<u8>, WorkspaceHostError> {
        let (_, output) = self.run_git(arguments)?;
        Ok(output)
    }

    fn run_git(&self, arguments: &[&str]) -> Result<(bool, Vec<u8>), WorkspaceHostError> {
        let mut child = Command::new("git")
            .args(["-c", "core.quotepath=false", "-C"])
            .arg(&self.root)
            .args(arguments)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::null())
            .spawn()
            .map_err(WorkspaceHostError::Io)?;
        let stdout = child.stdout.take().ok_or(WorkspaceHostError::GitCommand)?;
        let reader = std::thread::spawn(move || read_bounded_output(stdout));
        let status = child
            .wait_timeout(self.deadline)
            .map_err(WorkspaceHostError::Io)?;
        if status.is_none() {
            child.kill().map_err(WorkspaceHostError::Io)?;
            child.wait().map_err(WorkspaceHostError::Io)?;
            let _ = reader.join();
            return Err(WorkspaceHostError::GitTimeout);
        }
        let output = reader
            .join()
            .map_err(|_| WorkspaceHostError::GitCommand)??;
        Ok((status.is_some_and(|value| value.success()), output))
    }
}

fn read_bounded_output(mut output: impl Read) -> Result<Vec<u8>, WorkspaceHostError> {
    let mut retained = Vec::new();
    let mut buffer = vec![0_u8; READ_BUFFER_BYTES];
    let mut exceeded = false;
    loop {
        let count = output.read(&mut buffer).map_err(WorkspaceHostError::Io)?;
        if count == 0 {
            break;
        }
        if retained.len().saturating_add(count) <= MAX_GIT_OUTPUT_BYTES {
            retained.extend_from_slice(&buffer[..count]);
        } else {
            exceeded = true;
        }
    }
    if exceeded {
        Err(WorkspaceHostError::GitOutputLimit)
    } else {
        Ok(retained)
    }
}

fn parse_git_state(
    status: &[u8],
    head_commit: Option<String>,
    head_tree: Option<String>,
    index_tree: Option<String>,
) -> Result<GitWorkspaceState, WorkspaceHostError> {
    let mut branch = None;
    let mut ahead = 0;
    let mut behind = 0;
    let mut changes = BTreeSet::new();
    for record in status
        .split(|byte| *byte == 0)
        .filter(|record| !record.is_empty())
    {
        let text = std::str::from_utf8(record).map_err(|_| WorkspaceHostError::GitProtocol)?;
        if let Some(value) = text.strip_prefix("# branch.head ") {
            if value != "(detached)" {
                branch = Some(value.to_owned());
            }
        } else if let Some(value) = text.strip_prefix("# branch.ab ") {
            let mut fields = value.split_ascii_whitespace();
            ahead = parse_count(fields.next(), '+')?;
            behind = parse_count(fields.next(), '-')?;
        } else if text.starts_with("? ") {
            changes.insert(GitWorkspaceChange::Untracked);
        } else if text.starts_with("u ") {
            changes.insert(GitWorkspaceChange::Conflict);
        } else if text.starts_with("1 ") || text.starts_with("2 ") {
            let xy = text
                .as_bytes()
                .get(2..4)
                .ok_or(WorkspaceHostError::GitProtocol)?;
            if xy[0] != b'.' {
                changes.insert(GitWorkspaceChange::Staged);
            }
            if xy[1] != b'.' {
                changes.insert(GitWorkspaceChange::Unstaged);
            }
        }
    }
    let mut state = GitWorkspaceState {
        spec: "nimora.git-workspace-state/1".to_owned(),
        head_commit,
        head_tree,
        index_tree,
        branch,
        ahead,
        behind,
        changes,
        fingerprint: String::new(),
    };
    state.fingerprint = git_fingerprint(&state);
    Ok(state)
}

fn git_fingerprint(state: &GitWorkspaceState) -> String {
    let mut digest = Sha256::new();
    for value in [
        state.head_commit.as_deref(),
        state.head_tree.as_deref(),
        state.index_tree.as_deref(),
        state.branch.as_deref(),
    ] {
        digest.update(value.unwrap_or("-"));
        digest.update([0]);
    }
    digest.update(state.ahead.to_le_bytes());
    digest.update(state.behind.to_le_bytes());
    for change in &state.changes {
        digest.update([*change as u8]);
    }
    format!("{:x}", digest.finalize())
}

fn optional_oid(output: &[u8]) -> Result<Option<String>, WorkspaceHostError> {
    if output.is_empty() {
        return Ok(None);
    }
    let value = std::str::from_utf8(output)
        .map_err(|_| WorkspaceHostError::GitProtocol)?
        .trim();
    if value.len() != 40 && value.len() != 64 || !value.bytes().all(|byte| byte.is_ascii_hexdigit())
    {
        return Err(WorkspaceHostError::GitProtocol);
    }
    Ok(Some(value.to_ascii_lowercase()))
}

fn parse_count(value: Option<&str>, prefix: char) -> Result<u64, WorkspaceHostError> {
    value
        .and_then(|value| value.strip_prefix(prefix))
        .ok_or(WorkspaceHostError::GitProtocol)?
        .parse()
        .map_err(|_| WorkspaceHostError::GitProtocol)
}

fn safe_relative_path(root: &Path, path: &Path) -> Result<String, WorkspaceHostError> {
    let relative = path
        .strip_prefix(root)
        .map_err(|_| WorkspaceHostError::PathEscape)?;
    let mut segments = Vec::new();
    for component in relative.components() {
        match component {
            Component::Normal(value) => segments.push(
                value
                    .to_str()
                    .ok_or(WorkspaceHostError::NonUtf8Path)?
                    .to_owned(),
            ),
            _ => return Err(WorkspaceHostError::PathEscape),
        }
    }
    if segments.is_empty() {
        return Err(WorkspaceHostError::PathEscape);
    }
    Ok(segments.join("/"))
}

fn same_file_version(left: &Metadata, right: &Metadata) -> bool {
    left.len() == right.len()
        && left.modified().ok() == right.modified().ok()
        && platform_file_identity(left) == platform_file_identity(right)
}

#[cfg(unix)]
fn platform_file_identity(metadata: &Metadata) -> (u64, u64) {
    use std::os::unix::fs::MetadataExt;
    (metadata.dev(), metadata.ino())
}

#[cfg(not(unix))]
fn platform_file_identity(metadata: &Metadata) -> (u64, u64) {
    (metadata.len(), 0)
}

#[cfg(unix)]
fn executable(metadata: &Metadata) -> bool {
    use std::os::unix::fs::PermissionsExt;
    metadata.permissions().mode() & 0o111 != 0
}

#[cfg(not(unix))]
fn executable(_: &Metadata) -> bool {
    false
}

#[derive(Debug, Error)]
pub enum WorkspaceHostError {
    #[error("invalid workspace policy")]
    InvalidPolicy,
    #[error("invalid workspace root")]
    InvalidRoot,
    #[error("workspace walk failed")]
    Walk,
    #[error("symbolic links are not accepted")]
    SymbolicLink,
    #[error("workspace path escaped its root")]
    PathEscape,
    #[error("workspace path is not UTF-8")]
    NonUtf8Path,
    #[error("workspace file limit exceeded")]
    FileLimit,
    #[error("workspace byte limit exceeded")]
    ByteLimit,
    #[error("workspace file is too large")]
    FileTooLarge,
    #[error("workspace scan deadline exceeded")]
    Deadline,
    #[error("workspace file changed while being read")]
    ConcurrentMutation,
    #[error("Git command failed")]
    GitCommand,
    #[error("Git command timed out")]
    GitTimeout,
    #[error("Git output limit exceeded")]
    GitOutputLimit,
    #[error("Git output violated the protocol")]
    GitProtocol,
    #[error("workspace tracking rejected the snapshot")]
    Tracking(#[from] nimora_agent_runtime::WorkspaceTrackingError),
    #[error("workspace I/O failed")]
    Io(#[source] io::Error),
}

#[must_use]
pub fn unix_time_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .try_into()
        .unwrap_or(u64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn temporary_directory(label: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "nimora-workspace-{label}-{}-{}",
            std::process::id(),
            unix_time_ms()
        ));
        fs::create_dir_all(&path).expect("temporary directory");
        path
    }

    fn git(root: &Path, arguments: &[&str]) {
        let status = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(arguments)
            .status()
            .expect("git command");
        assert!(status.success());
    }

    #[test]
    fn scan_honors_ignore_rules_and_builds_version_chain() {
        let root = temporary_directory("scan");
        fs::write(root.join("tracked.txt"), b"one").expect("tracked");
        fs::write(root.join("ignored.log"), b"ignored").expect("ignored");
        fs::write(root.join("private.tmp"), b"private").expect("private");
        fs::write(root.join(".gitignore"), b"*.log\n").expect("gitignore");
        fs::write(root.join(".nimoraignore"), b"*.tmp\n").expect("nimoraignore");
        let scanner =
            WorkspaceScanner::open(&root, WorkspaceScanPolicy::default()).expect("scanner");
        let first = scanner.scan(1, None, 1).expect("first snapshot");
        let paths = first
            .files
            .iter()
            .map(|file| file.relative_path.as_str())
            .collect::<Vec<_>>();
        assert!(paths.contains(&"tracked.txt"));
        assert!(!paths.contains(&"ignored.log"));
        assert!(!paths.contains(&"private.tmp"));

        fs::write(root.join("tracked.txt"), b"two").expect("modify");
        let second = scanner
            .scan(2, Some(first.fingerprint.clone()), 2)
            .expect("second snapshot");
        assert_eq!(first.diff(&second).expect("diff").changes.len(), 1);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[cfg(unix)]
    #[test]
    fn scan_rejects_symlink_entries() {
        use std::os::unix::fs::symlink;
        let root = temporary_directory("symlink");
        fs::write(root.join("target"), b"data").expect("target");
        symlink(root.join("target"), root.join("link")).expect("symlink");
        let scanner =
            WorkspaceScanner::open(&root, WorkspaceScanPolicy::default()).expect("scanner");
        assert!(matches!(
            scanner.scan(1, None, 1),
            Err(WorkspaceHostError::SymbolicLink)
        ));
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn git_adapter_reports_head_index_and_worktree_drift() {
        let root = temporary_directory("git");
        git(&root, &["init", "--quiet"]);
        git(&root, &["config", "user.email", "nimora@example.invalid"]);
        git(&root, &["config", "user.name", "Nimora Test"]);
        fs::write(root.join("tracked.txt"), b"one").expect("tracked");
        git(&root, &["add", "tracked.txt"]);
        git(&root, &["commit", "--quiet", "-m", "initial"]);
        fs::write(root.join("tracked.txt"), b"two").expect("modify");
        fs::write(root.join("new.txt"), b"new").expect("untracked");

        let state = GitWorkspaceAdapter::open(&root, Duration::from_secs(5))
            .expect("adapter")
            .inspect()
            .expect("state");
        assert!(state.head_commit.is_some());
        assert!(state.head_tree.is_some());
        assert!(state.index_tree.is_some());
        assert!(state.changes.contains(&GitWorkspaceChange::Unstaged));
        assert!(state.changes.contains(&GitWorkspaceChange::Untracked));
        assert!(!state.changes.contains(&GitWorkspaceChange::Conflict));
        assert_eq!(state.fingerprint.len(), 64);
        fs::remove_dir_all(root).expect("cleanup");
    }

    #[test]
    fn safe_relative_path_rejects_non_normal_components() {
        let root = Path::new("/workspace");
        assert_eq!(
            safe_relative_path(root, Path::new("/workspace/src/lib.rs")).expect("path"),
            "src/lib.rs"
        );
        assert!(safe_relative_path(root, Path::new("/outside/file")).is_err());
    }
}
