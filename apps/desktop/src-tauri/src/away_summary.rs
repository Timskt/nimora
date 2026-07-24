//! Away Summary: aggregates unattended Auto Mode activity for the FE Control Center panel.
//!
//! # Tauri command
//!
//! ```ignore
//! #[tauri::command]
//! fn get_away_summary(
//!     goal_id: Option<Uuid>,
//!     state: State<'_, DesktopState>,
//! ) -> Result<Option<AwaySummary>, DesktopError>
//! ```
//!
//! Frontend invoke: `invoke("get_away_summary", { goalId: string | null })`
//! Returns `null` when persistence is unavailable; otherwise a camelCase `AwaySummary`.

use super::{
    agent_error, authorization_grant_key, current_time_ms, DesktopError, DesktopState,
};
use nimora_agent_runtime::{
    AgentGoal, AgentGoalStatus, AgentPlan, AgentPlanStepStatus, ApprovalPolicy, AuthorizationGrant,
    AutoModeCheckpoint, AutoModePauseReason, AutoModeSession, AutoModeStatus, NetworkPolicy,
    ProviderMessageRole, SandboxScope,
};
use nimora_persistence_sqlite::{
    AuthorizationGrantKey, SqliteAgentGoalRepository, SqliteAuthorizationGrantRepository,
    SqliteAutoModeCheckpointRepository, SqliteAutoModeRepository,
};
use serde::Serialize;
use std::{
    collections::HashSet,
    path::Path,
};
use tauri::State;
use uuid::Uuid;

/// Wire-format spec for FE `AwaySummary`.
pub const AWAY_SUMMARY_SPEC: &str = "nimora.away-summary/1";

/// FE-facing Away Summary (`apps/desktop/src/components/AgentWorkspace.tsx`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AwaySummary {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub spec: Option<&'static str>,
    pub away_started_at_ms: Option<u64>,
    pub away_ended_at_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub completed_goals: u32,
    pub failed_goals: u32,
    pub pending_confirmations: u32,
    pub grants_revoked: u32,
    pub companion_moments: u32,
    pub highlights: Vec<String>,
    /// Active high-risk / NeverAskWithinGrant callouts (CLI-aligned `riskNotes`).
    pub risk_notes: Vec<String>,
    /// Active grant IDs safe to one-click revoke from the Away Summary surface.
    pub revoke_grant_ids: Vec<Uuid>,
    pub generated_at_ms: u64,
}

/// One Goal row used by the pure aggregator (avoids host/DB coupling in unit tests).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwayGoalFact {
    pub id: Uuid,
    pub title: String,
    pub status: AgentGoalStatus,
    pub completed_steps: usize,
    pub total_steps: usize,
    pub next_step_text: Option<String>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub completed_at_ms: Option<u64>,
}

/// One Auto Mode session row for aggregation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwaySessionFact {
    pub goal_id: Uuid,
    pub status: AutoModeStatus,
    pub pause_reason: Option<AutoModePauseReason>,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub cycles: u32,
    pub tool_calls: u32,
    pub last_speech: Option<String>,
}

/// One authorization grant row for aggregation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AwayGrantFact {
    pub grant_id: Uuid,
    pub goal_id: Uuid,
    pub revoked_at_ms: Option<u64>,
    pub issued_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub sandbox: SandboxScope,
    pub approval: ApprovalPolicy,
    pub network: NetworkPolicy,
}

/// Pure inputs for [`build_away_summary`].
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AwaySummaryInputs {
    pub goals: Vec<AwayGoalFact>,
    pub sessions: Vec<AwaySessionFact>,
    pub grants: Vec<AwayGrantFact>,
    /// In-memory tool confirmations waiting on the host (not persisted).
    pub pending_tool_confirmations: u32,
    pub now_ms: u64,
}

/// Builds a FE-ready Away Summary from already-loaded host facts.
///
/// Pure-ish: no I/O. Chinese-friendly highlights when activity exists.
#[must_use]
pub fn build_away_summary(inputs: &AwaySummaryInputs) -> AwaySummary {
    let mut completed_goals = 0_u32;
    let mut failed_goals = 0_u32;
    for goal in &inputs.goals {
        match goal.status {
            AgentGoalStatus::Completed => completed_goals = completed_goals.saturating_add(1),
            AgentGoalStatus::Cancelled => failed_goals = failed_goals.saturating_add(1),
            AgentGoalStatus::Active | AgentGoalStatus::Paused => {}
        }
    }

    // Sessions cancelled without a cancelled goal still count as failures.
    for session in &inputs.sessions {
        if session.status == AutoModeStatus::Cancelled {
            let goal_failed = inputs
                .goals
                .iter()
                .find(|goal| goal.id == session.goal_id)
                .is_some_and(|goal| goal.status == AgentGoalStatus::Cancelled);
            if !goal_failed {
                failed_goals = failed_goals.saturating_add(1);
            }
        }
    }

    let mut pending_confirmations = inputs.pending_tool_confirmations;
    for session in &inputs.sessions {
        if session.status == AutoModeStatus::Paused
            && session.pause_reason == Some(AutoModePauseReason::ConfirmationRequired)
        {
            pending_confirmations = pending_confirmations.saturating_add(1);
        }
    }

    let grants_revoked = inputs
        .grants
        .iter()
        .filter(|grant| grant.revoked_at_ms.is_some())
        .count() as u32;

    let mut companion_moments = 0_u32;
    for session in &inputs.sessions {
        if session.cycles > 0 || session.tool_calls > 0 {
            companion_moments = companion_moments.saturating_add(1);
        }
        if session
            .last_speech
            .as_ref()
            .is_some_and(|text| !text.trim().is_empty())
        {
            companion_moments = companion_moments.saturating_add(1);
        }
    }

    let away_started_at_ms = inputs
        .sessions
        .iter()
        .map(|session| session.created_at_ms)
        .chain(inputs.grants.iter().map(|grant| grant.issued_at_ms))
        .chain(inputs.goals.iter().map(|goal| goal.created_at_ms))
        .min();

    let all_terminal = !inputs.sessions.is_empty()
        && inputs.sessions.iter().all(|session| {
            matches!(
                session.status,
                AutoModeStatus::Completed | AutoModeStatus::Cancelled
            )
        });
    let away_ended_at_ms = if all_terminal {
        inputs
            .sessions
            .iter()
            .map(|session| session.updated_at_ms)
            .max()
    } else if away_started_at_ms.is_some() {
        Some(inputs.now_ms)
    } else {
        None
    };

    let duration_ms = match (away_started_at_ms, away_ended_at_ms) {
        (Some(start), Some(end)) if end >= start => Some(end - start),
        _ => None,
    };

    let highlights = build_highlights(inputs);
    let risk_notes = build_risk_notes(inputs);
    let revoke_grant_ids = build_revoke_grant_ids(inputs);

    AwaySummary {
        spec: Some(AWAY_SUMMARY_SPEC),
        away_started_at_ms,
        away_ended_at_ms,
        duration_ms,
        completed_goals,
        failed_goals,
        pending_confirmations,
        grants_revoked,
        companion_moments,
        highlights,
        risk_notes,
        revoke_grant_ids,
        generated_at_ms: inputs.now_ms,
    }
}

fn format_duration_zh(duration_ms: u64) -> String {
    let total_secs = duration_ms / 1_000;
    let hours = total_secs / 3_600;
    let minutes = (total_secs % 3_600) / 60;
    let seconds = total_secs % 60;
    if hours > 0 {
        format!("离开约 {hours} 小时 {minutes} 分钟")
    } else if minutes > 0 {
        format!("离开约 {minutes} 分钟")
    } else {
        format!("离开约 {seconds} 秒")
    }
}

fn grant_is_active(grant: &AwayGrantFact, now_ms: u64) -> bool {
    grant.revoked_at_ms.is_none()
        && !grant
            .expires_at_ms
            .is_some_and(|expires| now_ms >= expires)
}

fn grant_id_short(grant_id: Uuid) -> String {
    grant_id.to_string().chars().take(8).collect()
}

/// Active NeverAsk / elevated-sandbox / unrestricted-network risk notes (CLI-aligned).
#[must_use]
pub fn build_risk_notes(inputs: &AwaySummaryInputs) -> Vec<String> {
    let mut notes = Vec::new();
    for grant in &inputs.grants {
        if !grant_is_active(grant, inputs.now_ms) {
            continue;
        }
        let short = grant_id_short(grant.grant_id);
        match (grant.sandbox, grant.approval) {
            (SandboxScope::SelectedRoots, ApprovalPolicy::NeverAskWithinGrant) => {
                notes.push(format!(
                    "授权 {short} 为 unattended（SelectedRoots + NeverAskWithinGrant），离开期间可能自动推进可逆写入"
                ));
            }
            (SandboxScope::FullDevice, ApprovalPolicy::NeverAskWithinGrant) => {
                notes.push(format!(
                    "授权 {short} 为 full_device，可影响本机文件、命令与联网；请确认后一键撤销"
                ));
            }
            (SandboxScope::WorkspaceWrite, ApprovalPolicy::NeverAskWithinGrant) => {
                notes.push(format!(
                    "授权 {short} 为 trusted_workspace，工作区内写入在 Grant 有效期内可不询问"
                ));
            }
            _ => {}
        }
        if matches!(grant.network, NetworkPolicy::Unrestricted) {
            notes.push(format!(
                "授权 {short} 允许 unrestricted 网络，数据可能离开设备"
            ));
        }
    }
    notes.truncate(8);
    notes
}

/// Active grants eligible for one-click revoke from the Away Summary surface.
#[must_use]
pub fn build_revoke_grant_ids(inputs: &AwaySummaryInputs) -> Vec<Uuid> {
    inputs
        .grants
        .iter()
        .filter(|grant| grant_is_active(grant, inputs.now_ms))
        .map(|grant| grant.grant_id)
        .collect()
}

fn build_highlights(inputs: &AwaySummaryInputs) -> Vec<String> {
    let mut highlights = Vec::new();
    if inputs.goals.is_empty() && inputs.sessions.is_empty() && inputs.grants.is_empty() {
        return highlights;
    }

    // Approximate away window from the same facts as build_away_summary.
    let away_started_at_ms = inputs
        .sessions
        .iter()
        .map(|session| session.created_at_ms)
        .chain(inputs.grants.iter().map(|grant| grant.issued_at_ms))
        .chain(inputs.goals.iter().map(|goal| goal.created_at_ms))
        .min();
    let all_terminal = !inputs.sessions.is_empty()
        && inputs.sessions.iter().all(|session| {
            matches!(
                session.status,
                AutoModeStatus::Completed | AutoModeStatus::Cancelled
            )
        });
    let away_ended_at_ms = if all_terminal {
        inputs
            .sessions
            .iter()
            .map(|session| session.updated_at_ms)
            .max()
    } else if away_started_at_ms.is_some() {
        Some(inputs.now_ms)
    } else {
        None
    };
    if let (Some(start), Some(end)) = (away_started_at_ms, away_ended_at_ms) {
        if end >= start {
            highlights.push(format_duration_zh(end - start));
        }
    }

    for goal in inputs.goals.iter().take(4) {
        highlights.push(format!(
            "目标「{}」状态为{}，计划进度 {}/{}",
            goal.title,
            goal_status_zh(goal.status),
            goal.completed_steps,
            goal.total_steps
        ));
        if let Some(step) = goal.next_step_text.as_ref() {
            if !step.trim().is_empty() {
                highlights.push(format!("下一步：{step}"));
            }
        }
    }

    let failed_goal_titles: Vec<&str> = inputs
        .goals
        .iter()
        .filter(|goal| goal.status == AgentGoalStatus::Cancelled)
        .map(|goal| goal.title.as_str())
        .take(3)
        .collect();
    if !failed_goal_titles.is_empty() {
        highlights.push(format!(
            "失败/取消目标：{}",
            failed_goal_titles.join("、")
        ));
    }

    let cancelled_sessions = inputs
        .sessions
        .iter()
        .filter(|session| session.status == AutoModeStatus::Cancelled)
        .count();
    if cancelled_sessions > 0 {
        highlights.push(format!("离开期间取消会话 {cancelled_sessions} 个"));
    }

    let session_count = inputs.sessions.len();
    let active_grants = inputs
        .grants
        .iter()
        .filter(|grant| grant.revoked_at_ms.is_none())
        .count();
    let issued_grants = inputs.grants.len();
    if session_count > 0 || active_grants > 0 || issued_grants > 0 {
        highlights.push(format!(
            "关联 Auto Mode 会话 {session_count} 个，签发授权 {issued_grants} 个，未撤销 {active_grants} 个"
        ));
    }

    // Pause reasons (budget / confirmation / other), prefer budget visibility.
    let budget_pauses = inputs
        .sessions
        .iter()
        .filter(|session| {
            session.status == AutoModeStatus::Paused
                && session.pause_reason == Some(AutoModePauseReason::BudgetExhausted)
        })
        .count();
    if budget_pauses > 0 {
        highlights.push(format!("预算暂停 {budget_pauses} 次，需回来后放行或调预算"));
    }

    let confirmation_pauses = inputs
        .sessions
        .iter()
        .filter(|session| {
            session.status == AutoModeStatus::Paused
                && session.pause_reason == Some(AutoModePauseReason::ConfirmationRequired)
        })
        .count();
    if confirmation_pauses > 0 {
        highlights.push(format!("等待确认暂停 {confirmation_pauses} 次（需要确认）"));
    }

    if let Some(reason) = inputs.sessions.iter().find_map(|session| {
        if session.status == AutoModeStatus::Paused {
            session.pause_reason
        } else {
            None
        }
    }) {
        // Avoid duplicating the dedicated budget/confirmation lines when already covered.
        let skip = matches!(
            reason,
            AutoModePauseReason::BudgetExhausted | AutoModePauseReason::ConfirmationRequired
        );
        if !skip {
            highlights.push(format!("当前暂停原因：{}", pause_reason_zh(reason)));
        } else if budget_pauses == 0 && confirmation_pauses == 0 {
            highlights.push(format!("当前暂停原因：{}", pause_reason_zh(reason)));
        }
    }

    // Batch / usage: cycles ≈ batch turns progressed; tools = tool calls.
    let total_cycles: u64 = inputs.sessions.iter().map(|s| u64::from(s.cycles)).sum();
    let total_tools: u64 = inputs.sessions.iter().map(|s| u64::from(s.tool_calls)).sum();
    let batch_sessions = inputs
        .sessions
        .iter()
        .filter(|session| session.cycles > 0 || session.tool_calls > 0)
        .count();
    if total_cycles > 0 || total_tools > 0 {
        highlights.push(format!(
            "累计批次用量：会话 {batch_sessions} 个，推进 {total_cycles} 轮，工具调用 {total_tools} 次"
        ));
    }

    if let Some(speech) = inputs.sessions.iter().find_map(|s| s.last_speech.as_ref()) {
        let snippet: String = speech.chars().take(80).collect();
        if !snippet.trim().is_empty() {
            highlights.push(format!("伙伴留言：{snippet}"));
        }
    }

    let revoked = inputs
        .grants
        .iter()
        .filter(|grant| grant.revoked_at_ms.is_some())
        .count();
    if revoked > 0 {
        highlights.push(format!("离开期间已撤销授权 {revoked} 个"));
    }

    // High-risk active grants: NeverAskWithinGrant + elevated sandbox.
    for grant in inputs.grants.iter().filter(|g| g.revoked_at_ms.is_none()) {
        let expired = grant
            .expires_at_ms
            .is_some_and(|expires| inputs.now_ms >= expires);
        if expired {
            continue;
        }
        match (grant.sandbox, grant.approval) {
            (SandboxScope::SelectedRoots, ApprovalPolicy::NeverAskWithinGrant) => {
                highlights.push(
                    "活跃 unattended 授权：SelectedRoots + Grant 内不询问，离开期间可自动推进".to_owned(),
                );
            }
            (SandboxScope::FullDevice, ApprovalPolicy::NeverAskWithinGrant) => {
                highlights.push(
                    "活跃 full_device 授权：高风险，可影响本机文件与联网；确认后请一键撤销".to_owned(),
                );
            }
            (SandboxScope::WorkspaceWrite, ApprovalPolicy::NeverAskWithinGrant) => {
                highlights.push(
                    "活跃 trusted_workspace 授权：工作区内写入在 Grant 有效期内可不询问".to_owned(),
                );
            }
            _ => {}
        }
    }

    highlights.truncate(12);
    highlights
}

fn goal_status_zh(status: AgentGoalStatus) -> &'static str {
    match status {
        AgentGoalStatus::Active => "进行中",
        AgentGoalStatus::Paused => "已暂停",
        AgentGoalStatus::Completed => "已完成",
        AgentGoalStatus::Cancelled => "已取消",
    }
}

fn pause_reason_zh(reason: AutoModePauseReason) -> &'static str {
    match reason {
        AutoModePauseReason::ConfirmationRequired => "需要确认",
        AutoModePauseReason::BudgetExhausted => "预算耗尽",
        AutoModePauseReason::GoalChanged => "目标已变更",
        AutoModePauseReason::WorkspaceChanged => "工作区已变更",
        AutoModePauseReason::ProviderUnavailable => "模型不可用",
        AutoModePauseReason::Restarted => "进程重启后暂停",
        AutoModePauseReason::UnsafeEffect => "不安全副作用",
        AutoModePauseReason::UserRequested => "用户请求暂停",
    }
}

fn goal_to_fact(goal: &AgentGoal, plan: &AgentPlan) -> AwayGoalFact {
    let completed_steps = plan
        .steps
        .iter()
        .filter(|step| step.status == AgentPlanStepStatus::Completed)
        .count();
    let next_step_text = plan
        .steps
        .iter()
        .find(|step| {
            matches!(
                step.status,
                AgentPlanStepStatus::Pending | AgentPlanStepStatus::InProgress
            )
        })
        .map(|step| step.text.clone());
    AwayGoalFact {
        id: goal.id,
        title: goal.title.clone(),
        status: goal.status,
        completed_steps,
        total_steps: plan.steps.len(),
        next_step_text,
        created_at_ms: goal.created_at_ms,
        updated_at_ms: goal.updated_at_ms,
        completed_at_ms: goal.completed_at_ms,
    }
}

fn session_to_fact(session: &AutoModeSession, last_speech: Option<String>) -> AwaySessionFact {
    AwaySessionFact {
        goal_id: session.goal_id,
        status: session.status,
        pause_reason: session.pause_reason,
        created_at_ms: session.created_at_ms,
        updated_at_ms: session.updated_at_ms,
        cycles: session.usage.cycles,
        tool_calls: session.usage.tool_calls,
        last_speech,
    }
}

fn grant_to_fact(grant: &AuthorizationGrant) -> AwayGrantFact {
    AwayGrantFact {
        grant_id: grant.id,
        goal_id: grant.goal_id,
        revoked_at_ms: grant.revoked_at_ms,
        issued_at_ms: grant.issued_at_ms,
        expires_at_ms: grant.expires_at_ms,
        sandbox: grant.sandbox,
        approval: grant.approval,
        network: grant.network.clone(),
    }
}

fn extract_last_assistant_text(checkpoint: &AutoModeCheckpoint) -> Option<String> {
    checkpoint
        .messages
        .iter()
        .rev()
        .find(|message| message.role == ProviderMessageRole::Assistant)
        .map(|message| message.content.chars().take(200).collect::<String>())
        .filter(|content| !content.trim().is_empty())
}

/// Loads host facts from SQLite + in-memory job registry and builds an Away Summary.
///
/// # Errors
///
/// Returns storage or agent errors when repositories cannot be opened or read.
pub fn load_away_summary(
    database_path: &Path,
    grant_key: AuthorizationGrantKey,
    goal_id: Option<Uuid>,
    job_session_ids: &[Uuid],
    pending_tool_confirmations: u32,
    now_ms: u64,
) -> Result<AwaySummary, DesktopError> {
    let goals_repo = SqliteAgentGoalRepository::open(database_path)?;
    let grants_repo =
        SqliteAuthorizationGrantRepository::open_with_key(database_path, grant_key)?;
    let sessions_repo = SqliteAutoModeRepository::open(database_path)?;
    let checkpoints_repo = SqliteAutoModeCheckpointRepository::open(database_path)?;

    let mut goal_ids: HashSet<Uuid> = HashSet::new();
    if let Some(goal_id) = goal_id {
        goal_ids.insert(goal_id);
    } else {
        for goal in goals_repo.list(100)? {
            goal_ids.insert(goal.id);
        }
    }

    // Discover sessions via job registry + per-goal list + restart-recoverable rows.
    let mut session_ids: HashSet<Uuid> = job_session_ids.iter().copied().collect();
    if let Ok(recoverable) = sessions_repo.list_recoverable(256) {
        for session in recoverable {
            session_ids.insert(session.id);
        }
    }
    for id in goal_ids.clone() {
        for session in sessions_repo.list_for_goal(id, 64)? {
            session_ids.insert(session.id);
        }
    }

    let mut sessions: Vec<AutoModeSession> = Vec::new();
    let mut seen: HashSet<Uuid> = HashSet::new();
    for session_id in session_ids {
        if !seen.insert(session_id) {
            continue;
        }
        if let Some(session) = sessions_repo.get(session_id)? {
            if goal_id.map(|id| session.goal_id == id).unwrap_or(true) {
                goal_ids.insert(session.goal_id);
                sessions.push(session);
            }
        }
    }
    sessions.sort_unstable_by(|left, right| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });

    // Prefer goals that still exist; skip orphan goal ids silently.
    let mut goal_facts = Vec::new();
    let mut ordered_goal_ids: Vec<Uuid> = goal_ids.into_iter().collect();
    ordered_goal_ids.sort_unstable();
    for id in ordered_goal_ids {
        if let Some(snapshot) = goals_repo.get(id)? {
            goal_facts.push(goal_to_fact(&snapshot.goal, &snapshot.current_plan));
        }
    }
    goal_facts.sort_unstable_by(|left, right| {
        right
            .updated_at_ms
            .cmp(&left.updated_at_ms)
            .then_with(|| right.id.cmp(&left.id))
    });

    let mut grant_facts = Vec::new();
    for fact in &goal_facts {
        for grant in grants_repo.list_for_goal(fact.id, 32)? {
            grant_facts.push(grant_to_fact(&grant));
        }
    }
    grant_facts.sort_unstable_by(|left, right| {
        right
            .issued_at_ms
            .cmp(&left.issued_at_ms)
            .then_with(|| right.goal_id.cmp(&left.goal_id))
    });

    let mut session_facts = Vec::with_capacity(sessions.len());
    for session in &sessions {
        let last_speech = checkpoints_repo
            .get(session.id)?
            .as_ref()
            .and_then(extract_last_assistant_text);
        session_facts.push(session_to_fact(session, last_speech));
    }

    Ok(build_away_summary(&AwaySummaryInputs {
        goals: goal_facts,
        sessions: session_facts,
        grants: grant_facts,
        pending_tool_confirmations,
        now_ms,
    }))
}

/// Tauri command: returns structured Away Summary for the Control Center panel.
///
/// `goal_id` filters to one Goal when present; otherwise aggregates recent goals/sessions.
#[tauri::command]
#[allow(clippy::needless_pass_by_value)]
pub(super) fn get_away_summary(
    goal_id: Option<Uuid>,
    state: State<'_, DesktopState>,
) -> Result<Option<AwaySummary>, DesktopError> {
    let Some(database_path) = state.database_path.as_ref() else {
        return Ok(None);
    };
    let now_ms = current_time_ms()?;
    let job_session_ids = state
        .auto_mode_jobs
        .snapshots()
        .map_err(agent_error)?
        .into_iter()
        .map(|job| job.session_id)
        .collect::<Vec<_>>();
    let pending_tool_confirmations = state
        .pending_agent_tools
        .lock()
        .map_err(|_| DesktopError::StatePoisoned)?
        .len() as u32;
    let grant_key = authorization_grant_key(&state)?;
    let summary = load_away_summary(
        database_path,
        grant_key,
        goal_id,
        &job_session_ids,
        pending_tool_confirmations,
        now_ms,
    )?;
    Ok(Some(summary))
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_agent_runtime::AutoModePauseReason;

    fn goal(
        id: Uuid,
        title: &str,
        status: AgentGoalStatus,
        completed: usize,
        total: usize,
    ) -> AwayGoalFact {
        AwayGoalFact {
            id,
            title: title.to_owned(),
            status,
            completed_steps: completed,
            total_steps: total,
            next_step_text: Some("运行测试".to_owned()),
            created_at_ms: 1_000,
            updated_at_ms: 2_000,
            completed_at_ms: matches!(status, AgentGoalStatus::Completed).then_some(2_000),
        }
    }

    fn grant(
        goal_id: Uuid,
        sandbox: SandboxScope,
        approval: ApprovalPolicy,
        revoked_at_ms: Option<u64>,
        issued_at_ms: u64,
        expires_at_ms: Option<u64>,
    ) -> AwayGrantFact {
        AwayGrantFact {
            grant_id: Uuid::now_v7(),
            goal_id,
            revoked_at_ms,
            issued_at_ms,
            expires_at_ms,
            sandbox,
            approval,
            network: NetworkPolicy::Offline,
        }
    }

    #[test]
    fn empty_inputs_produce_empty_activity_summary() {
        let summary = build_away_summary(&AwaySummaryInputs {
            now_ms: 10_000,
            ..AwaySummaryInputs::default()
        });
        assert_eq!(summary.completed_goals, 0);
        assert_eq!(summary.failed_goals, 0);
        assert_eq!(summary.pending_confirmations, 0);
        assert_eq!(summary.grants_revoked, 0);
        assert_eq!(summary.companion_moments, 0);
        assert!(summary.highlights.is_empty());
        assert!(summary.risk_notes.is_empty());
        assert!(summary.revoke_grant_ids.is_empty());
        assert_eq!(summary.duration_ms, None);
        assert_eq!(summary.generated_at_ms, 10_000);
        assert_eq!(summary.spec, Some(AWAY_SUMMARY_SPEC));
    }

    #[test]
    fn sessions_and_goals_aggregate_chinese_highlights() {
        let goal_id = Uuid::now_v7();
        let summary = build_away_summary(&AwaySummaryInputs {
            goals: vec![goal(
                goal_id,
                "夜间回归",
                AgentGoalStatus::Completed,
                2,
                3,
            )],
            sessions: vec![AwaySessionFact {
                goal_id,
                status: AutoModeStatus::Completed,
                pause_reason: None,
                created_at_ms: 1_000,
                updated_at_ms: 61_000,
                cycles: 4,
                tool_calls: 2,
                last_speech: Some("测试都绿了".to_owned()),
            }],
            grants: vec![
                grant(
                goal_id,
                SandboxScope::SelectedRoots,
                ApprovalPolicy::NeverAskWithinGrant,
                None,
                1_000,
                Some(1_000 + 8 * 60 * 60 * 1_000),
            ),
                grant(
                goal_id,
                SandboxScope::WorkspaceWrite,
                ApprovalPolicy::AskRisky,
                Some(50_000),
                2_000,
                None,
            ),
            ],
            pending_tool_confirmations: 1,
            now_ms: 70_000,
        });

        assert_eq!(summary.completed_goals, 1);
        assert_eq!(summary.failed_goals, 0);
        assert_eq!(summary.grants_revoked, 1);
        assert_eq!(summary.pending_confirmations, 1);
        assert!(summary.companion_moments >= 2);
        assert_eq!(summary.away_started_at_ms, Some(1_000));
        assert_eq!(summary.away_ended_at_ms, Some(61_000));
        assert_eq!(summary.duration_ms, Some(60_000));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("夜间回归") && line.contains("已完成")));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("伙伴留言")));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("撤销授权")));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("unattended") || line.contains("SelectedRoots")));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("推进") && line.contains("轮")));
        assert_eq!(summary.revoke_grant_ids.len(), 1);
        assert!(summary
            .risk_notes
            .iter()
            .any(|line| line.contains("unattended")));
    }

    #[test]
    fn confirmation_pause_and_cancelled_session_count() {
        let goal_id = Uuid::now_v7();
        let summary = build_away_summary(&AwaySummaryInputs {
            goals: vec![goal(goal_id, "写补丁", AgentGoalStatus::Active, 0, 2)],
            sessions: vec![
                AwaySessionFact {
                    goal_id,
                    status: AutoModeStatus::Paused,
                    pause_reason: Some(AutoModePauseReason::ConfirmationRequired),
                    created_at_ms: 5_000,
                    updated_at_ms: 8_000,
                    cycles: 1,
                    tool_calls: 0,
                    last_speech: None,
                },
                AwaySessionFact {
                    goal_id,
                    status: AutoModeStatus::Cancelled,
                    pause_reason: None,
                    created_at_ms: 1_000,
                    updated_at_ms: 2_000,
                    cycles: 0,
                    tool_calls: 0,
                    last_speech: None,
                },
            ],
            grants: Vec::new(),
            pending_tool_confirmations: 0,
            now_ms: 9_000,
        });
        assert_eq!(summary.pending_confirmations, 1);
        assert_eq!(summary.failed_goals, 1);
        assert_eq!(summary.away_ended_at_ms, Some(9_000));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("需要确认")));
    }

    #[test]
    fn budget_pause_and_batch_usage_highlights() {
        let goal_id = Uuid::now_v7();
        let summary = build_away_summary(&AwaySummaryInputs {
            goals: vec![goal(goal_id, "夜间构建", AgentGoalStatus::Active, 1, 4)],
            sessions: vec![AwaySessionFact {
                goal_id,
                status: AutoModeStatus::Paused,
                pause_reason: Some(AutoModePauseReason::BudgetExhausted),
                created_at_ms: 10_000,
                updated_at_ms: 40_000,
                cycles: 8,
                tool_calls: 12,
                last_speech: Some("预算到了，先歇口气".to_owned()),
            }],
            grants: vec![grant(
                goal_id,
                SandboxScope::SelectedRoots,
                ApprovalPolicy::NeverAskWithinGrant,
                None,
                10_000,
                Some(10_000 + 8 * 60 * 60 * 1_000),
            )],
            pending_tool_confirmations: 0,
            now_ms: 50_000,
        });
        assert_eq!(summary.pending_confirmations, 0);
        assert!(summary.companion_moments >= 2);
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("预算暂停")));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("推进 8 轮") && line.contains("工具调用 12 次")));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("伙伴留言")));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("unattended")));
    }

    #[test]
    fn failures_and_full_device_grant_highlights() {
        let goal_id = Uuid::now_v7();
        let other = Uuid::now_v7();
        let summary = build_away_summary(&AwaySummaryInputs {
            goals: vec![
                goal(goal_id, "坏掉的迁移", AgentGoalStatus::Cancelled, 0, 2),
                goal(other, "旁路观察", AgentGoalStatus::Active, 0, 1),
            ],
            sessions: vec![
                AwaySessionFact {
                    goal_id,
                    status: AutoModeStatus::Cancelled,
                    pause_reason: None,
                    created_at_ms: 1_000,
                    updated_at_ms: 2_000,
                    cycles: 0,
                    tool_calls: 0,
                    last_speech: None,
                },
                AwaySessionFact {
                    goal_id: other,
                    status: AutoModeStatus::Running,
                    pause_reason: None,
                    created_at_ms: 1_500,
                    updated_at_ms: 3_000,
                    cycles: 2,
                    tool_calls: 1,
                    last_speech: None,
                },
            ],
            grants: vec![grant(
                other,
                SandboxScope::FullDevice,
                ApprovalPolicy::NeverAskWithinGrant,
                None,
                1_500,
                Some(1_500 + 4 * 60 * 60 * 1_000),
            )],
            pending_tool_confirmations: 0,
            now_ms: 4_000,
        });
        // Cancelled goal already counts as failed; cancelled session with cancelled goal not double-counted.
        assert_eq!(summary.failed_goals, 1);
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("失败/取消") && line.contains("坏掉的迁移")));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("取消会话")));
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("full_device")));
        assert!(summary
            .risk_notes
            .iter()
            .any(|line| line.contains("full_device")));
        assert_eq!(summary.revoke_grant_ids.len(), 1);
        assert!(summary.companion_moments >= 1);
        assert_eq!(summary.away_ended_at_ms, Some(4_000)); // non-terminal session still open
    }

    #[test]
    fn grants_issued_and_expired_do_not_risk_note() {
        let goal_id = Uuid::now_v7();
        let summary = build_away_summary(&AwaySummaryInputs {
            goals: vec![goal(goal_id, "短任务", AgentGoalStatus::Completed, 1, 1)],
            sessions: vec![AwaySessionFact {
                goal_id,
                status: AutoModeStatus::Completed,
                pause_reason: None,
                created_at_ms: 1_000,
                updated_at_ms: 2_000,
                cycles: 1,
                tool_calls: 0,
                last_speech: None,
            }],
            grants: vec![grant(
                goal_id,
                SandboxScope::FullDevice,
                ApprovalPolicy::NeverAskWithinGrant,
                None,
                1_000,
                Some(1_500),
            )],
            pending_tool_confirmations: 0,
            now_ms: 3_000,
        });
        assert!(summary
            .highlights
            .iter()
            .any(|line| line.contains("签发授权 1")));
        assert!(!summary
            .highlights
            .iter()
            .any(|line| line.contains("full_device")));
        assert!(summary.risk_notes.is_empty());
        assert!(summary.revoke_grant_ids.is_empty());
    }

    #[test]
    fn unrestricted_network_adds_risk_note() {
        let goal_id = Uuid::now_v7();
        let mut fact = grant(
            goal_id,
            SandboxScope::FullDevice,
            ApprovalPolicy::NeverAskWithinGrant,
            None,
            1_000,
            Some(10_000 + 4 * 60 * 60 * 1_000),
        );
        fact.network = NetworkPolicy::Unrestricted;
        let summary = build_away_summary(&AwaySummaryInputs {
            goals: vec![goal(goal_id, "联网任务", AgentGoalStatus::Active, 0, 1)],
            sessions: vec![AwaySessionFact {
                goal_id,
                status: AutoModeStatus::Running,
                pause_reason: None,
                created_at_ms: 1_000,
                updated_at_ms: 2_000,
                cycles: 1,
                tool_calls: 0,
                last_speech: None,
            }],
            grants: vec![fact],
            pending_tool_confirmations: 0,
            now_ms: 3_000,
        });
        assert!(summary
            .risk_notes
            .iter()
            .any(|line| line.contains("unrestricted") || line.contains("网络")));
        assert_eq!(summary.revoke_grant_ids.len(), 1);
    }
}
