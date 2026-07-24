//! Unattended Auto Mode host: start goal+session+grant+job, list/revoke grants.

use super::{
    agent_error, auto_mode_jobs, auto_mode_runner, current_time_ms, default_agent_model,
    default_agent_provider_id, ensure_normal_mode, open_authorization_grant_repository,
    production_agent_tool_allowlist, AppHandle, DesktopError, DesktopState, StartAutoModeJobRequest,
};
use nimora_agent_runtime::{
    AgentBudget, AgentGoal, AgentPlan, AgentPlanStep, AgentTask, AgentTaskOrigin, AgentTaskStatus,
    ApprovalPolicy, AuthorizationGrant, AutoModeCheckpoint, AutoModePauseReason, AutoModePolicy,
    AutoModeSession, DataClassification, GrantLifetime, ModelReasoningPolicy, NetworkPolicy,
    ProviderMessage, ProviderMessageRole, SandboxScope, ToolId,
};
use nimora_agent_workspace_host::{WorkspaceScanPolicy, WorkspaceScanner};
use nimora_persistence_sqlite::{
    SqliteAgentGoalRepository,
    SqliteAutoModeCheckpointRepository, SqliteAutoModeRepository,
    SqliteWorkspaceSnapshotRepository, StoredWorkspaceSnapshot,
};
use nimora_runtime_core::RuntimeMode;
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeSet, HashSet},
    path::{Path, PathBuf},
};
use tauri::State;
use uuid::Uuid;

const GRANT_SPEC: &str = "nimora.authorization-grant/1";
const GRANT_SUMMARY_SPEC: &str = "nimora.authorization-grant-summary/1";
const UNATTENDED_REQUESTER: &str = "desktop:unattended-auto";
const EIGHT_HOURS_MS: u64 = 8 * 60 * 60 * 1_000;
const FOUR_HOURS_MS: u64 = 4 * 60 * 60 * 1_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AuthorizationTier {
    Observe,
    Workspace,
    TrustedWorkspace,
    Unattended,
    FullDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub(super) enum AuthorizationGrantStatus {
    Active,
    Revoked,
    Expired,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(super) struct StartUnattendedAutoModeRequest {
    title: String,
    objective: String,
    steps: Vec<String>,
    workspace_root: String,
    tier: AuthorizationTier,
    #[serde(default = "default_true")]
    offline: bool,
    #[serde(default = "default_auto_mode_batch_turns")]
    max_turns_per_batch: u16,
    #[serde(default)]
    reasoning_policy: Option<ModelReasoningPolicy>,
    #[serde(default)]
    max_output_tokens: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct StartUnattendedAutoModeResult {
    session_id: Uuid,
    job_id: Uuid,
    grant_id: Uuid,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct AuthorizationGrantSummary {
    spec: &'static str,
    grant_id: Uuid,
    goal_id: Uuid,
    tier: AuthorizationTier,
    status: AuthorizationGrantStatus,
    workspace_root: Option<String>,
    issued_at_ms: u64,
    expires_at_ms: Option<u64>,
    revoked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(super) struct RevokeAuthorizationGrantResult {
    grant_id: Uuid,
    revoked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TierPolicy {
    pub sandbox: SandboxScope,
    pub approval: ApprovalPolicy,
    pub network: NetworkPolicy,
    pub selected_roots: BTreeSet<String>,
    pub lifetime: GrantLifetime,
    pub expires_at_ms: Option<u64>,
}

const fn default_true() -> bool {
    true
}

const fn default_auto_mode_batch_turns() -> u16 {
    8
}

const fn default_auto_mode_output_tokens() -> u64 {
    512
}

fn generous_budget() -> AgentBudget {
    AgentBudget {
        max_steps: 64,
        max_tool_calls: 64,
        max_elapsed_ms: EIGHT_HOURS_MS,
        max_input_tokens: 500_000,
        max_output_tokens: 128_000,
        max_cost_microunits: 50_000_000,
    }
}

/// Maps an authorization tier onto sandbox / approval / network / lifetime constraints.
///
/// # `NeverAskWithinGrant` (sleep-safe)
///
/// `TrustedWorkspace`, `Unattended`, and `FullDevice` set
/// [`ApprovalPolicy::NeverAskWithinGrant`]. While the grant is still active
/// (not revoked/expired) and the request binds exactly to Goal / Plan revision /
/// workspace fingerprint / tool / provider / model / data class / network policy,
/// Auto Mode authorizes without pausing for confirmation. This is **not** a bypass
/// of hard denylists, budgets, or binding drift — only of in-grant confirmation UX.
///
/// - Observe / Workspace → `AskRisky` (still pauses on risky effects)
/// - TrustedWorkspace → `WorkspaceWrite` + `NeverAskWithinGrant` + session lifetime
/// - Unattended → `SelectedRoots` (= workspace root) + `NeverAskWithinGrant` + 8h
/// - FullDevice → `FullDevice` + `NeverAskWithinGrant` + 4h (online → unrestricted net)
#[must_use]
pub(super) fn tier_policy(
    tier: AuthorizationTier,
    offline: bool,
    workspace_root: &str,
    now_ms: u64,
) -> TierPolicy {
    let restricted_network = if offline {
        NetworkPolicy::Offline
    } else {
        NetworkPolicy::LoopbackOnly
    };
    match tier {
        AuthorizationTier::Observe => TierPolicy {
            sandbox: SandboxScope::ReadOnly,
            approval: ApprovalPolicy::AskRisky,
            network: restricted_network,
            selected_roots: BTreeSet::new(),
            lifetime: GrantLifetime::Session,
            expires_at_ms: None,
        },
        AuthorizationTier::Workspace => TierPolicy {
            sandbox: SandboxScope::WorkspaceWrite,
            approval: ApprovalPolicy::AskRisky,
            network: restricted_network,
            selected_roots: BTreeSet::new(),
            lifetime: GrantLifetime::Session,
            expires_at_ms: None,
        },
        AuthorizationTier::TrustedWorkspace => TierPolicy {
            sandbox: SandboxScope::WorkspaceWrite,
            approval: ApprovalPolicy::NeverAskWithinGrant,
            network: restricted_network,
            selected_roots: BTreeSet::new(),
            lifetime: GrantLifetime::Session,
            expires_at_ms: None,
        },
        AuthorizationTier::Unattended => TierPolicy {
            sandbox: SandboxScope::SelectedRoots,
            approval: ApprovalPolicy::NeverAskWithinGrant,
            network: restricted_network,
            selected_roots: BTreeSet::from([workspace_root.to_owned()]),
            lifetime: GrantLifetime::UntilTimestamp,
            expires_at_ms: Some(now_ms.saturating_add(EIGHT_HOURS_MS)),
        },
        AuthorizationTier::FullDevice => TierPolicy {
            sandbox: SandboxScope::FullDevice,
            approval: ApprovalPolicy::NeverAskWithinGrant,
            network: if offline {
                NetworkPolicy::Offline
            } else {
                NetworkPolicy::Unrestricted
            },
            selected_roots: BTreeSet::new(),
            lifetime: GrantLifetime::UntilTimestamp,
            expires_at_ms: Some(now_ms.saturating_add(FOUR_HOURS_MS)),
        },
    }
}

/// Derives authorization tier from persisted grant sandbox + approval.
#[must_use]
pub(super) fn infer_tier(grant: &AuthorizationGrant) -> AuthorizationTier {
    match (grant.sandbox, grant.approval) {
        (SandboxScope::ReadOnly, ApprovalPolicy::AlwaysAsk | ApprovalPolicy::AskRisky) => {
            AuthorizationTier::Observe
        }
        (SandboxScope::WorkspaceWrite, ApprovalPolicy::AskRisky | ApprovalPolicy::AlwaysAsk) => {
            AuthorizationTier::Workspace
        }
        (SandboxScope::WorkspaceWrite, ApprovalPolicy::NeverAskWithinGrant) => {
            AuthorizationTier::TrustedWorkspace
        }
        (SandboxScope::SelectedRoots, ApprovalPolicy::NeverAskWithinGrant) => {
            AuthorizationTier::Unattended
        }
        (SandboxScope::FullDevice, ApprovalPolicy::NeverAskWithinGrant) => {
            AuthorizationTier::FullDevice
        }
        (SandboxScope::ReadOnly, _) => AuthorizationTier::Observe,
        (SandboxScope::WorkspaceWrite, _) => AuthorizationTier::Workspace,
        (SandboxScope::SelectedRoots, _) => AuthorizationTier::Unattended,
        (SandboxScope::FullDevice, _) => AuthorizationTier::FullDevice,
    }
}


/// Returns true when the tier uses sleep-safe `NeverAskWithinGrant` approval.
///
/// Sleep-safe means Auto Mode may authorize risky/effectful tool calls without
/// pausing for interactive confirmation **while the grant remains valid and
/// binding-exact**. This does **not** widen sandbox scope: Unattended still
/// pins SelectedRoots; only FullDevice is unrestricted FS.
#[must_use]
pub(super) const fn is_sleep_safe_tier(tier: AuthorizationTier) -> bool {
    matches!(
        tier,
        AuthorizationTier::TrustedWorkspace
            | AuthorizationTier::Unattended
            | AuthorizationTier::FullDevice
    )
}

/// True when an approval policy is sleep-safe unattended (`NeverAskWithinGrant`).
#[must_use]
pub(super) const fn is_sleep_safe_approval(approval: ApprovalPolicy) -> bool {
    matches!(approval, ApprovalPolicy::NeverAskWithinGrant)
}

/// Workspace fingerprints must be `sha256:` + exactly 64 ASCII hex digits
/// (mirrors agent-runtime `AuthorizationGrant::validate` / `valid_fingerprint`).
#[must_use]
pub(super) fn valid_workspace_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

/// Builds a FE-facing grant summary, optionally carrying a workspace root path.
#[must_use]
pub(super) fn grant_summary(
    grant: &AuthorizationGrant,
    workspace_root: Option<String>,
    now_ms: u64,
) -> AuthorizationGrantSummary {
    let status = if grant.revoked_at_ms.is_some() {
        AuthorizationGrantStatus::Revoked
    } else if grant
        .expires_at_ms
        .is_some_and(|expires_at_ms| now_ms >= expires_at_ms)
    {
        AuthorizationGrantStatus::Expired
    } else {
        AuthorizationGrantStatus::Active
    };
    let workspace_root = workspace_root.or_else(|| {
        grant
            .selected_roots
            .iter()
            .next()
            .cloned()
            .filter(|_| grant.sandbox == SandboxScope::SelectedRoots)
    });
    AuthorizationGrantSummary {
        spec: GRANT_SUMMARY_SPEC,
        grant_id: grant.id,
        goal_id: grant.goal_id,
        tier: infer_tier(grant),
        status,
        workspace_root,
        issued_at_ms: grant.issued_at_ms,
        expires_at_ms: grant.expires_at_ms,
        revoked_at_ms: grant.revoked_at_ms,
    }
}

fn parse_tool_allowlist(tools: &BTreeSet<String>) -> Result<BTreeSet<ToolId>, DesktopError> {
    tools
        .iter()
        .map(|tool| {
            tool.parse::<ToolId>().map_err(|_| {
                DesktopError::Agent(format!("invalid production tool allowlist entry: {tool}"))
            })
        })
        .collect()
}

fn build_authorization_grant(
    tier: AuthorizationTier,
    offline: bool,
    goal_id: Uuid,
    plan_revision: u64,
    workspace_fingerprint: &str,
    workspace_root: &str,
    tools: &BTreeSet<String>,
    provider_id: &str,
    model: &str,
    now_ms: u64,
) -> Result<AuthorizationGrant, DesktopError> {
    if !valid_workspace_fingerprint(workspace_fingerprint) {
        return Err(DesktopError::Agent(
            "authorization grant workspace fingerprint must be sha256: + 64 hex digits".to_owned(),
        ));
    }
    let policy = tier_policy(tier, offline, workspace_root, now_ms);
    let tool_allowlist = parse_tool_allowlist(tools)?;
    if tool_allowlist.is_empty() {
        return Err(DesktopError::Agent(
            "authorization grant requires a non-empty tool allowlist".to_owned(),
        ));
    }
    let grant = AuthorizationGrant {
        spec: GRANT_SPEC.to_owned(),
        id: Uuid::now_v7(),
        goal_id,
        plan_revision,
        workspace_fingerprint: workspace_fingerprint.to_owned(),
        sandbox: policy.sandbox,
        approval: policy.approval,
        network: policy.network,
        selected_roots: policy.selected_roots,
        tool_allowlist,
        provider_allowlist: BTreeSet::from([provider_id.to_owned()]),
        model_allowlist: BTreeSet::from([model.to_owned()]),
        maximum_data_classification: DataClassification::Personal,
        budget: generous_budget(),
        lifetime: policy.lifetime,
        issued_at_ms: now_ms,
        expires_at_ms: policy.expires_at_ms,
        revoked_at_ms: None,
    };
    grant
        .validate()
        .map_err(|error| agent_error(error))?;
    Ok(grant)
}

fn validate_start_request(request: &StartUnattendedAutoModeRequest) -> Result<u64, DesktopError> {
    if request.title.trim().is_empty() {
        return Err(DesktopError::InvalidRequest(
            "unattended Auto Mode title must be non-empty".to_owned(),
        ));
    }
    if request.objective.trim().is_empty() {
        return Err(DesktopError::InvalidRequest(
            "unattended Auto Mode objective must be non-empty".to_owned(),
        ));
    }
    if request.steps.is_empty() || request.steps.len() > 32 {
        return Err(DesktopError::InvalidRequest(
            "unattended Auto Mode steps must contain between 1 and 32 entries".to_owned(),
        ));
    }
    if request.steps.iter().any(|step| step.trim().is_empty()) {
        return Err(DesktopError::InvalidRequest(
            "unattended Auto Mode steps must be non-empty".to_owned(),
        ));
    }
    let workspace_root = Path::new(&request.workspace_root);
    if !workspace_root.is_absolute() {
        return Err(DesktopError::InvalidRequest(
            "unattended Auto Mode workspaceRoot must be an absolute path".to_owned(),
        ));
    }
    let max_output_tokens = request
        .max_output_tokens
        .unwrap_or_else(default_auto_mode_output_tokens);
    if max_output_tokens == 0 || max_output_tokens > 16_384 {
        return Err(DesktopError::Agent(
            "Auto Mode output tokens must be between 1 and 16384".to_owned(),
        ));
    }
    if request.max_turns_per_batch == 0 || request.max_turns_per_batch > 256 {
        return Err(DesktopError::Agent(
            "Auto Mode batch turns must be between 1 and 256".to_owned(),
        ));
    }
    Ok(max_output_tokens)
}

fn require_database_path(state: &DesktopState) -> Result<&Path, DesktopError> {
    state.database_path.as_deref().ok_or_else(|| {
        DesktopError::Agent("persistence is unavailable for unattended Auto Mode".to_owned())
    })
}

fn absolute_workspace_root(path: &Path) -> Result<String, DesktopError> {
    let canonical = std::fs::canonicalize(path).map_err(DesktopError::Io)?;
    Ok(canonical.to_string_lossy().into_owned())
}

/// Starts an unattended Auto Mode session: goal, paused session, checkpoint, grant, and job.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) fn start_unattended_auto_mode(
    request: StartUnattendedAutoModeRequest,
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<StartUnattendedAutoModeResult, DesktopError> {
    ensure_normal_mode(&state)?;
    if state.safety.snapshot()?.mode == RuntimeMode::Safe {
        return Err(DesktopError::SafeModeActive);
    }
    let max_output_tokens = validate_start_request(&request)?;
    let database_path = require_database_path(&state)?.to_path_buf();
    let now_ms = current_time_ms()?;
    let provider_id = default_agent_provider_id();
    let model = default_agent_model();
    let tools = production_agent_tool_allowlist(&state)?;
    if tools.is_empty() {
        return Err(DesktopError::Agent(
            "production tool allowlist is empty".to_owned(),
        ));
    }

    let workspace_path = PathBuf::from(&request.workspace_root);
    let scanner = WorkspaceScanner::open(&workspace_path, WorkspaceScanPolicy::default())
        .map_err(agent_error)?;
    let workspace = scanner
        .scan(1, None, now_ms)
        .map_err(agent_error)?;
    let workspace_root = absolute_workspace_root(&workspace_path).unwrap_or_else(|_| {
        request.workspace_root.clone()
    });

    let goal_id = Uuid::now_v7();
    let plan_steps = request
        .steps
        .iter()
        .map(AgentPlanStep::new)
        .collect::<Result<Vec<_>, _>>()
        .map_err(agent_error)?;
    let plan = AgentPlan::new(
        goal_id,
        plan_steps,
        "Unattended Auto Mode plan",
        now_ms,
    )
    .map_err(agent_error)?;
    let goal = AgentGoal::new(&request.title, &request.objective, &plan, now_ms)
        .map_err(agent_error)?;

    let policy = AutoModePolicy::new(
        64,
        1,
        generous_budget(),
        DataClassification::Personal,
        tools.iter().cloned(),
        workspace.fingerprint.clone(),
    )
    .map_err(agent_error)?;
    let mut session = AutoModeSession::start(&goal, &plan, policy, now_ms).map_err(agent_error)?;
    session
        .pause(AutoModePauseReason::UserRequested, now_ms.saturating_add(1))
        .map_err(agent_error)?;

    let mut task = AgentTask::new(
        AgentTaskOrigin::Desktop,
        UNATTENDED_REQUESTER,
        provider_id.clone(),
        generous_budget(),
        now_ms,
    )
    .map_err(agent_error)?;
    task.transition(AgentTaskStatus::Planning, now_ms.saturating_add(1))
        .map_err(agent_error)?;

    let checkpoint = AutoModeCheckpoint::new(
        session.id,
        goal.id,
        plan.revision,
        1,
        task,
        model.clone(),
        vec![ProviderMessage::text(
            ProviderMessageRole::User,
            request.objective.clone(),
            DataClassification::Personal,
            true,
        )],
        workspace.fingerprint.clone(),
        session.policy.fingerprint(),
        now_ms,
        now_ms.saturating_add(1),
    )
    .map_err(agent_error)?;

    let stored_workspace = StoredWorkspaceSnapshot::new(
        session.id,
        scanner.root_fingerprint(),
        workspace.clone(),
    )?;

    let grant = build_authorization_grant(
        request.tier,
        request.offline,
        goal.id,
        plan.revision,
        &workspace.fingerprint,
        &workspace_root,
        &tools,
        &provider_id,
        &model,
        now_ms,
    )?;

    SqliteAgentGoalRepository::open(&database_path)?.create(&goal, &plan)?;
    SqliteAutoModeRepository::open(&database_path)?.create(&session)?;
    SqliteAutoModeCheckpointRepository::open(&database_path)?.create(&checkpoint)?;
    SqliteWorkspaceSnapshotRepository::open(&database_path)?.create(&stored_workspace)?;
    open_authorization_grant_repository(&state)?.issue(&grant)?;

    let job_request = StartAutoModeJobRequest {
        session_id: session.id,
        workspace_root: workspace_path,
        constraints: Vec::new(),
        max_output_tokens,
        offline: request.offline,
        reasoning_policy: request.reasoning_policy,
        max_turns_per_batch: request.max_turns_per_batch,
    };
    let (snapshot, control) = state
        .auto_mode_jobs
        .start(session.id, now_ms)
        .map_err(agent_error)?;
    let job_id = snapshot.job_id;
    // Petization: grant issuance becomes body language + speech (before runner takes app).
    let tier_key = match request.tier {
        AuthorizationTier::Observe => "observe",
        AuthorizationTier::Workspace => "workspace",
        AuthorizationTier::TrustedWorkspace => "trusted_workspace",
        AuthorizationTier::Unattended => "unattended",
        AuthorizationTier::FullDevice => "full_device",
    };
    crate::companion_directive::apply_grant_event(
        &app,
        crate::companion_directive::grant_event_for_tier(tier_key),
    );

    std::thread::Builder::new()
        .name(format!("nimora-auto-mode-{job_id}"))
        .spawn(move || auto_mode_runner::run(&app, job_id, &job_request, &control))
        .map_err(|error| {
            let _ = state.auto_mode_jobs.finish(
                job_id,
                auto_mode_jobs::AutoModeJobStatus::Failed,
                None,
                Some("runner-spawn-failed".to_owned()),
                current_time_ms().unwrap_or(snapshot.updated_at_ms),
            );
            DesktopError::Io(error)
        })?;

    Ok(StartUnattendedAutoModeResult {
        session_id: session.id,
        job_id,
        grant_id: grant.id,
    })
}

/// Revokes one authorization grant by id.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) fn revoke_authorization_grant(
    grant_id: Uuid,
    app: AppHandle,
    state: State<'_, DesktopState>,
) -> Result<RevokeAuthorizationGrantResult, DesktopError> {
    ensure_normal_mode(&state)?;
    let now_ms = current_time_ms()?;
    let repository = open_authorization_grant_repository(&state)?;
    let result = match repository.revoke(grant_id, now_ms) {
        Ok(_) => Ok(RevokeAuthorizationGrantResult {
            grant_id,
            revoked: true,
        }),
        Err(nimora_persistence_sqlite::SqlitePersistenceError::AuthorizationGrantAlreadyRevoked) => {
            Ok(RevokeAuthorizationGrantResult {
                grant_id,
                revoked: true,
            })
        }
        Err(error) => Err(error.into()),
    };
    if result.is_ok() {
        crate::companion_directive::apply_grant_event(
            &app,
            crate::companion_directive::GrantCompanionEvent::Revoked,
        );
    }
    result
}

/// Lists authorization grants for one goal, or recent goals when `goal_id` is null.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) fn list_authorization_grants(
    goal_id: Option<Uuid>,
    state: State<'_, DesktopState>,
) -> Result<Vec<AuthorizationGrantSummary>, DesktopError> {
    ensure_normal_mode(&state)?;
    let database_path = require_database_path(&state)?;
    let now_ms = current_time_ms()?;
    let grants_repo = open_authorization_grant_repository(&state)?;

    let mut grants = Vec::new();
    if let Some(goal_id) = goal_id {
        grants.extend(grants_repo.list_for_goal(goal_id, 64)?);
    } else {
        let mut goal_ids = HashSet::new();
        for goal in SqliteAgentGoalRepository::open(database_path)?.list(100)? {
            goal_ids.insert(goal.id);
        }
        let sessions = SqliteAutoModeRepository::open(database_path)?;
        for job in state.auto_mode_jobs.snapshots().map_err(agent_error)? {
            if let Some(session) = sessions.get(job.session_id)? {
                goal_ids.insert(session.goal_id);
            }
        }
        for goal_id in goal_ids {
            grants.extend(grants_repo.list_for_goal(goal_id, 32)?);
        }
    }

    grants.sort_unstable_by(|left, right| {
        right
            .issued_at_ms
            .cmp(&left.issued_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });
    grants.dedup_by_key(|grant| grant.id);

    Ok(grants
        .iter()
        .map(|grant| grant_summary(grant, None, now_ms))
        .collect())
}

#[cfg(test)]
mod tests {
    use super::{
        build_authorization_grant, grant_summary, infer_tier, is_sleep_safe_approval,
        is_sleep_safe_tier, tier_policy, valid_workspace_fingerprint, AuthorizationGrantStatus,
        AuthorizationTier, EIGHT_HOURS_MS, FOUR_HOURS_MS,
    };
    use nimora_agent_runtime::{
        AgentBudget, ApprovalPolicy, AuthorizationGrant, DataClassification, GrantLifetime,
        NetworkPolicy, SandboxScope,
    };
    use std::collections::BTreeSet;
    use uuid::Uuid;

    fn sample_tools() -> BTreeSet<String> {
        BTreeSet::from(["pet.test.read".to_owned()])
    }

    fn fingerprint() -> String {
        format!("sha256:{}", "a".repeat(64))
    }

    #[test]
    fn tier_policy_observe_is_readonly_ask_risky() {
        let policy = tier_policy(AuthorizationTier::Observe, true, "/tmp/ws", 1_000);
        assert_eq!(policy.sandbox, SandboxScope::ReadOnly);
        assert_eq!(policy.approval, ApprovalPolicy::AskRisky);
        assert_eq!(policy.network, NetworkPolicy::Offline);
        assert!(policy.selected_roots.is_empty());
        assert_eq!(policy.lifetime, GrantLifetime::Session);
        assert_eq!(policy.expires_at_ms, None);
    }

    #[test]
    fn tier_policy_workspace_and_trusted() {
        let workspace = tier_policy(AuthorizationTier::Workspace, false, "/tmp/ws", 1_000);
        assert_eq!(workspace.sandbox, SandboxScope::WorkspaceWrite);
        assert_eq!(workspace.approval, ApprovalPolicy::AskRisky);
        assert_eq!(workspace.network, NetworkPolicy::LoopbackOnly);

        let trusted = tier_policy(AuthorizationTier::TrustedWorkspace, false, "/tmp/ws", 1_000);
        assert_eq!(trusted.sandbox, SandboxScope::WorkspaceWrite);
        assert_eq!(trusted.approval, ApprovalPolicy::NeverAskWithinGrant);
        assert!(trusted.selected_roots.is_empty());
    }

    #[test]
    fn tier_policy_unattended_uses_selected_roots_and_expiry() {
        let policy = tier_policy(AuthorizationTier::Unattended, true, "/tmp/ws", 1_000);
        assert_eq!(policy.sandbox, SandboxScope::SelectedRoots);
        assert_eq!(policy.approval, ApprovalPolicy::NeverAskWithinGrant);
        assert_eq!(
            policy.selected_roots,
            BTreeSet::from(["/tmp/ws".to_owned()])
        );
        assert_eq!(policy.lifetime, GrantLifetime::UntilTimestamp);
        assert_eq!(policy.expires_at_ms, Some(1_000 + EIGHT_HOURS_MS));
    }

    #[test]
    fn tier_policy_full_device_uses_unrestricted_when_online() {
        let policy = tier_policy(AuthorizationTier::FullDevice, false, "/tmp/ws", 2_000);
        assert_eq!(policy.sandbox, SandboxScope::FullDevice);
        assert_eq!(policy.approval, ApprovalPolicy::NeverAskWithinGrant);
        assert_eq!(policy.network, NetworkPolicy::Unrestricted);
        assert!(policy.selected_roots.is_empty());
        assert_eq!(policy.expires_at_ms, Some(2_000 + FOUR_HOURS_MS));
    }

    #[test]
    fn infer_tier_round_trips_mapped_grants() {
        let tools = sample_tools();
        for tier in [
            AuthorizationTier::Observe,
            AuthorizationTier::Workspace,
            AuthorizationTier::TrustedWorkspace,
            AuthorizationTier::Unattended,
            AuthorizationTier::FullDevice,
        ] {
            let grant = build_authorization_grant(
                tier,
                true,
                Uuid::now_v7(),
                1,
                &fingerprint(),
                "/tmp/ws",
                &tools,
                "provider:deterministic-local",
                "model:echo-v1",
                1_000,
            )
            .expect("grant");
            assert_eq!(infer_tier(&grant), tier);
            grant.validate().expect("valid grant");
        }
    }

    #[test]
    fn grant_summary_reports_status_and_selected_root() {
        let tools = sample_tools();
        let mut grant = build_authorization_grant(
            AuthorizationTier::Unattended,
            true,
            Uuid::now_v7(),
            1,
            &fingerprint(),
            "/tmp/ws",
            &tools,
            "provider:deterministic-local",
            "model:echo-v1",
            1_000,
        )
        .expect("grant");

        let active = grant_summary(&grant, None, 1_500);
        assert_eq!(active.status, AuthorizationGrantStatus::Active);
        assert_eq!(active.tier, AuthorizationTier::Unattended);
        assert_eq!(active.workspace_root.as_deref(), Some("/tmp/ws"));

        let expired = grant_summary(&grant, None, grant.expires_at_ms.unwrap());
        assert_eq!(expired.status, AuthorizationGrantStatus::Expired);

        grant.revoked_at_ms = Some(1_200);
        let revoked = grant_summary(&grant, Some("/custom".to_owned()), 1_500);
        assert_eq!(revoked.status, AuthorizationGrantStatus::Revoked);
        assert_eq!(revoked.workspace_root.as_deref(), Some("/custom"));
    }

    #[test]
    fn workspace_tier_summary_has_null_workspace_root() {
        let grant = AuthorizationGrant {
            spec: "nimora.authorization-grant/1".to_owned(),
            id: Uuid::now_v7(),
            goal_id: Uuid::now_v7(),
            plan_revision: 1,
            workspace_fingerprint: fingerprint(),
            sandbox: SandboxScope::WorkspaceWrite,
            approval: ApprovalPolicy::AskRisky,
            network: NetworkPolicy::Offline,
            selected_roots: BTreeSet::new(),
            tool_allowlist: BTreeSet::from(["pet.test.read".parse().expect("tool")]),
            provider_allowlist: BTreeSet::from(["provider:local".to_owned()]),
            model_allowlist: BTreeSet::from(["model:local".to_owned()]),
            maximum_data_classification: DataClassification::Personal,
            budget: AgentBudget::default(),
            lifetime: GrantLifetime::Session,
            issued_at_ms: 1_000,
            expires_at_ms: None,
            revoked_at_ms: None,
        };
        let summary = grant_summary(&grant, None, 1_500);
        assert_eq!(summary.tier, AuthorizationTier::Workspace);
        assert_eq!(summary.workspace_root, None);
        assert_eq!(summary.status, AuthorizationGrantStatus::Active);
    }

    #[test]
    fn workspace_fingerprint_requires_sha256_prefix_and_64_hex() {
        assert!(valid_workspace_fingerprint(&fingerprint()));
        assert!(valid_workspace_fingerprint(&format!(
            "sha256:{}",
            "A1b2".repeat(16)
        )));
        assert!(!valid_workspace_fingerprint("sha256:abc"));
        assert!(!valid_workspace_fingerprint(&"a".repeat(64)));
        assert!(!valid_workspace_fingerprint(&format!(
            "sha256:{}",
            "g".repeat(64)
        )));
        assert!(!valid_workspace_fingerprint(""));
        assert!(!valid_workspace_fingerprint("md5:deadbeef"));
    }

    #[test]
    fn build_grant_rejects_invalid_fingerprint() {
        let tools = sample_tools();
        let err = build_authorization_grant(
            AuthorizationTier::Workspace,
            true,
            Uuid::now_v7(),
            1,
            "not-a-fingerprint",
            "/tmp/ws",
            &tools,
            "provider:deterministic-local",
            "model:echo-v1",
            1_000,
        )
        .expect_err("invalid fingerprint");
        let message = err.to_string();
        assert!(
            message.contains("fingerprint") || message.contains("sha256"),
            "unexpected error: {message}"
        );
    }

    #[test]
    fn never_ask_within_grant_tiers() {
        for tier in [
            AuthorizationTier::TrustedWorkspace,
            AuthorizationTier::Unattended,
            AuthorizationTier::FullDevice,
        ] {
            let policy = tier_policy(tier, true, "/tmp/ws", 1_000);
            assert_eq!(
                policy.approval,
                ApprovalPolicy::NeverAskWithinGrant,
                "{tier:?} must be sleep-safe NeverAskWithinGrant"
            );
            assert!(is_sleep_safe_tier(tier), "{tier:?}");
            assert!(is_sleep_safe_approval(policy.approval));
        }
        for tier in [AuthorizationTier::Observe, AuthorizationTier::Workspace] {
            let policy = tier_policy(tier, true, "/tmp/ws", 1_000);
            assert_eq!(policy.approval, ApprovalPolicy::AskRisky);
            assert!(!is_sleep_safe_tier(tier), "{tier:?} is interactive");
            assert!(!is_sleep_safe_approval(policy.approval));
        }
    }

    #[test]
    fn sleep_safe_unattended_keeps_selected_roots_not_full_device() {
        // Sleep-safe ≠ Full Device: unattended pins SelectedRoots + 8h expiry.
        let policy = tier_policy(AuthorizationTier::Unattended, true, "/tmp/ws", 1_000);
        assert!(is_sleep_safe_tier(AuthorizationTier::Unattended));
        assert_eq!(policy.sandbox, SandboxScope::SelectedRoots);
        assert_eq!(policy.approval, ApprovalPolicy::NeverAskWithinGrant);
        assert_eq!(policy.selected_roots, BTreeSet::from(["/tmp/ws".to_owned()]));
        assert_eq!(policy.expires_at_ms, Some(1_000 + EIGHT_HOURS_MS));
        // Offline stays offline under sleep-safe unattended.
        assert_eq!(policy.network, NetworkPolicy::Offline);
    }

    #[test]
    fn five_tiers_cover_full_authorization_ladder() {
        let tiers = [
            AuthorizationTier::Observe,
            AuthorizationTier::Workspace,
            AuthorizationTier::TrustedWorkspace,
            AuthorizationTier::Unattended,
            AuthorizationTier::FullDevice,
        ];
        assert_eq!(tiers.len(), 5);
        let sandboxes: Vec<_> = tiers
            .iter()
            .map(|tier| tier_policy(*tier, false, "/tmp/ws", 0).sandbox)
            .collect();
        assert!(sandboxes.contains(&SandboxScope::ReadOnly));
        assert!(sandboxes.contains(&SandboxScope::WorkspaceWrite));
        assert!(sandboxes.contains(&SandboxScope::SelectedRoots));
        assert!(sandboxes.contains(&SandboxScope::FullDevice));
    }

    #[test]
    fn infer_tier_fallback_by_sandbox_only() {
        // SelectedRoots without NeverAsk still maps Unattended (sandbox-dominant).
        let grant = AuthorizationGrant {
            spec: "nimora.authorization-grant/1".to_owned(),
            id: Uuid::now_v7(),
            goal_id: Uuid::now_v7(),
            plan_revision: 1,
            workspace_fingerprint: fingerprint(),
            sandbox: SandboxScope::SelectedRoots,
            approval: ApprovalPolicy::AskRisky,
            network: NetworkPolicy::Offline,
            selected_roots: BTreeSet::from(["/tmp/ws".to_owned()]),
            tool_allowlist: BTreeSet::from(["pet.test.read".parse().expect("tool")]),
            provider_allowlist: BTreeSet::from(["provider:local".to_owned()]),
            model_allowlist: BTreeSet::from(["model:local".to_owned()]),
            maximum_data_classification: DataClassification::Personal,
            budget: AgentBudget::default(),
            lifetime: GrantLifetime::Session,
            issued_at_ms: 1_000,
            expires_at_ms: None,
            revoked_at_ms: None,
        };
        assert_eq!(infer_tier(&grant), AuthorizationTier::Unattended);
    }
}
