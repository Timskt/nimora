use super::{
    companion_directive::{
        auto_mode_companion_status, CompanionPhase, CompanionPhaseTracker,
    },
    AppHandle, AutoModeExecutionError, AutoModeExecutionService, AutoModeHostControl,
    AutoModeHostControlService, AutoModeJobControl, AutoModeJobStatus, AutoModeLoopRequest,
    AutoModeLoopService, AutoModeLoopStop, AutoModePauseReason, ContextCachePolicy,
    ContextCompactionPolicy, DataClassification, DesktopState, Duration, ProviderExecutionContext,
    StartAutoModeJobRequest, Uuid, WorkspaceScanPolicy, authorization_grant_key, auto_mode_jobs,
    context_cache_key, current_time_ms, desktop_provider_registry, desktop_tool_backend,
    desktop_tool_registry, provider_credential_reference, resolve_provider_reasoning,
};
use nimora_agent_auto_host::{AutoModeRecoveryService, RecoveredAutoModeTurn};
use nimora_agent_runtime::AuthorizationGrant;
use nimora_persistence_sqlite::SqliteAuthorizationGrantRepository;
use tauri::Manager;

pub(super) fn run(
    app: &AppHandle,
    job_id: Uuid,
    request: &StartAutoModeJobRequest,
    control: &auto_mode_jobs::AutoModeJobControlHandle,
) {
    let state = app.state::<DesktopState>();
    let mut companion = CompanionPhaseTracker::default();
    let result = run_inner(app, &state, job_id, request, control, &mut companion);
    if let Err((status, error_code)) = result {
        let now_ms = current_time_ms()
            .or_else(|_| {
                state
                    .auto_mode_jobs
                    .snapshot(job_id)
                    .map(|job| job.updated_at_ms)
            })
            .unwrap_or_default();
        let _ = state
            .auto_mode_jobs
            .finish(job_id, status, None, Some(error_code.clone()), now_ms);
        if matches!(
            status,
            AutoModeJobStatus::Failed | AutoModeJobStatus::Indeterminate
        ) {
            companion.apply_failed_if_changed(app, &state, Some(error_code.as_str()));
        } else if let Some(phase) = auto_mode_companion_status(status, None) {
            companion.apply_if_changed(app, &state, phase);
        }
    }
}

fn run_inner(
    app: &AppHandle,
    state: &DesktopState,
    job_id: Uuid,
    request: &StartAutoModeJobRequest,
    control: &auto_mode_jobs::AutoModeJobControlHandle,
    companion: &mut CompanionPhaseTracker,
) -> Result<(), (AutoModeJobStatus, String)> {
    let database_path = state.database_path.as_ref().ok_or_else(|| {
        (
            AutoModeJobStatus::Failed,
            "persistence-unavailable".to_owned(),
        )
    })?;
    let now_ms = current_time_ms().map_err(|_| time_failure())?;
    mark_job_running(app, state, job_id, control, now_ms, companion)?;
    let mut turn = recover_turn(database_path, request, now_ms)?;
    let reasoning = resolve_auto_mode_reasoning(state, request, &turn)?;
    let credential_reference = provider_credential_reference(state, &turn.task.provider_id)
        .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))?;
    let providers = desktop_provider_registry(state)
        .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))?;
    let tools = desktop_tool_registry(state)
        .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))?;
    let backend = desktop_tool_backend(state, turn.task.id, turn.task.trace_id)
        .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))?;
    let execution = AutoModeExecutionService::new(
        database_path,
        WorkspaceScanPolicy::default(),
        context_cache_policy()?,
        context_compaction_policy(),
        24 * 60 * 60 * 1_000,
        context_cache_key(state).map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))?,
    )
    .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))?;
    let loop_service = AutoModeLoopService::new(execution);
    let host_control = AutoModeHostControlService::new(database_path);
    loop {
        if control.requested() != AutoModeJobControl::Continue {
            return commit_requested_control(
                app,
                state,
                job_id,
                turn,
                control,
                &host_control,
                companion,
            );
        }
        let mut logical_now = current_time_ms().map_err(|_| time_failure())?;
        let goal_id = turn.session.goal_id;
        let authorization_grant =
            load_active_authorization_grant(state, database_path, goal_id, logical_now);
        let result = loop_service.run(
            &providers,
            &tools,
            &backend,
            AutoModeLoopRequest {
                turn,
                workspace_root: request.workspace_root.clone(),
                constraints: request.constraints.clone(),
                max_output_tokens: request.max_output_tokens,
                reasoning: reasoning.clone(),
                provider_context: ProviderExecutionContext {
                    timeout: Duration::from_mins(2),
                    cancellation: control.cancellation(),
                    credential_reference: credential_reference.clone(),
                },
                offline: request.offline,
                data_classification: DataClassification::Personal,
                maximum_data_classification: DataClassification::Personal,
                max_turns: request.max_turns_per_batch,
                authorization_grant,
            },
            || {
                logical_now = current_time_ms().map_or_else(
                    |_| logical_now.saturating_add(1),
                    |now| now.max(logical_now),
                );
                logical_now
            },
        );
        let result = result.map_err(loop_failure)?;
        let checkpoint_sequence = checkpoint_sequence(&result.stop);
        state
            .auto_mode_jobs
            .record_batch(
                job_id,
                result.turns_executed,
                result.cache_hits,
                checkpoint_sequence,
                current_time_ms().map_err(|_| time_failure())?,
            )
            .map_err(|_| registry_failure())?;
        match result.stop {
            AutoModeLoopStop::Yielded(next) => {
                // Re-assert work_busy only when phase actually changes (tracker de-dupes).
                companion.apply_if_changed(app, state, CompanionPhase::RunningWork);
                // Throttled domain StepOk so pet observes progress without speech spam.
                if result.turns_executed > 0 {
                    companion.apply_step_ok_throttled(app, state);
                }
                turn = *next;
            }
            AutoModeLoopStop::Paused(paused) => {
                let pause_reason = paused.session.pause_reason.map(pause_reason_code);
                return finish(
                    app,
                    state,
                    job_id,
                    AutoModeJobStatus::Paused,
                    pause_reason,
                    companion,
                    None,
                );
            }
            AutoModeLoopStop::Completed(_) => {
                return finish(
                    app,
                    state,
                    job_id,
                    AutoModeJobStatus::Completed,
                    None,
                    companion,
                    None,
                );
            }
            AutoModeLoopStop::WorkspaceDrift { .. } => {
                return finish(
                    app,
                    state,
                    job_id,
                    AutoModeJobStatus::Paused,
                    Some("workspace_changed".to_owned()),
                    companion,
                    None,
                );
            }
        }
    }
}


fn load_active_authorization_grant(
    state: &DesktopState,
    database_path: &std::path::Path,
    goal_id: Uuid,
    now_ms: u64,
) -> Option<AuthorizationGrant> {
    let key = authorization_grant_key(state).ok()?;
    SqliteAuthorizationGrantRepository::open_with_key(database_path, key)
        .ok()
        .and_then(|repository| repository.get_active_for_goal(goal_id, now_ms).ok())
        .flatten()
}

fn recover_turn(
    database_path: &std::path::Path,
    request: &StartAutoModeJobRequest,
    now_ms: u64,
) -> Result<RecoveredAutoModeTurn, (AutoModeJobStatus, String)> {
    let recovery = AutoModeRecoveryService::new(database_path, WorkspaceScanPolicy::default());
    let recovered = recovery
        .recover(request.session_id, &request.workspace_root, now_ms)
        .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))?;
    recovery
        .commit_resume(recovered, now_ms)
        .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))
}

fn mark_job_running(
    app: &AppHandle,
    state: &DesktopState,
    job_id: Uuid,
    control: &auto_mode_jobs::AutoModeJobControlHandle,
    now_ms: u64,
    companion: &mut CompanionPhaseTracker,
) -> Result<(), (AutoModeJobStatus, String)> {
    if control.requested() == AutoModeJobControl::Continue {
        state
            .auto_mode_jobs
            .mark_running(job_id, now_ms)
            .map_err(|_| registry_failure())?;
        companion.apply_if_changed(app, state, CompanionPhase::RunningWork);
    }
    Ok(())
}

fn resolve_auto_mode_reasoning(
    state: &DesktopState,
    request: &StartAutoModeJobRequest,
    turn: &RecoveredAutoModeTurn,
) -> Result<Option<nimora_agent_runtime::ReasoningMapping>, (AutoModeJobStatus, String)> {
    resolve_provider_reasoning(
        state,
        &turn.task.provider_id,
        request.reasoning_policy.as_ref(),
    )
    .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))
}

fn context_cache_policy() -> Result<ContextCachePolicy, (AutoModeJobStatus, String)> {
    ContextCachePolicy::new(256, 64 * 1024 * 1024)
        .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))
}

fn context_compaction_policy() -> ContextCompactionPolicy {
    ContextCompactionPolicy {
        max_messages: 128,
        max_content_bytes: 128 * 1024,
        retain_recent_units: 32,
    }
}

fn loop_failure(error: nimora_agent_auto_host::AutoModeLoopError) -> (AutoModeJobStatus, String) {
    match error {
        nimora_agent_auto_host::AutoModeLoopError::Execution(
            AutoModeExecutionError::ExecutionIndeterminate { .. },
        ) => (
            AutoModeJobStatus::Indeterminate,
            "execution-indeterminate".to_owned(),
        ),
        other => (AutoModeJobStatus::Failed, other.to_string()),
    }
}

fn checkpoint_sequence(stop: &AutoModeLoopStop) -> u64 {
    match stop {
        AutoModeLoopStop::Yielded(turn)
        | AutoModeLoopStop::Paused(turn)
        | AutoModeLoopStop::Completed(turn) => turn.checkpoint_sequence,
        AutoModeLoopStop::WorkspaceDrift {
            checkpoint_sequence,
            ..
        } => *checkpoint_sequence,
    }
}

fn commit_requested_control(
    app: &AppHandle,
    state: &DesktopState,
    job_id: Uuid,
    turn: RecoveredAutoModeTurn,
    control: &auto_mode_jobs::AutoModeJobControlHandle,
    host_control: &AutoModeHostControlService,
    companion: &mut CompanionPhaseTracker,
) -> Result<(), (AutoModeJobStatus, String)> {
    let requested = control.requested();
    let controlled = host_control
        .commit(
            turn,
            match requested {
                AutoModeJobControl::Pause => AutoModeHostControl::Pause,
                AutoModeJobControl::Cancel => AutoModeHostControl::Cancel,
                AutoModeJobControl::Continue => unreachable!(),
            },
            current_time_ms().map_err(|_| time_failure())?,
        )
        .map_err(|error| (AutoModeJobStatus::Failed, error.to_string()))?;
    let (status, checkpoint_sequence) = match controlled {
        nimora_agent_auto_host::HostControlledAutoModeTurn::Paused(turn) => {
            (AutoModeJobStatus::Paused, turn.checkpoint_sequence)
        }
        nimora_agent_auto_host::HostControlledAutoModeTurn::Cancelled(turn) => {
            (AutoModeJobStatus::Cancelled, turn.checkpoint_sequence)
        }
    };
    state
        .auto_mode_jobs
        .record_batch(
            job_id,
            0,
            0,
            checkpoint_sequence,
            current_time_ms().map_err(|_| time_failure())?,
        )
        .map_err(|_| registry_failure())?;
    let pause_reason =
        (status == AutoModeJobStatus::Paused).then(|| "user_requested".to_owned());
    state
        .auto_mode_jobs
        .finish(
            job_id,
            status,
            pause_reason.clone(),
            None,
            current_time_ms().map_err(|_| time_failure())?,
        )
        .map_err(|_| registry_failure())?;
    apply_finish_companion(app, state, status, pause_reason.as_deref(), None, companion);
    Ok(())
}

fn finish(
    app: &AppHandle,
    state: &DesktopState,
    job_id: Uuid,
    status: AutoModeJobStatus,
    pause_reason: Option<String>,
    companion: &mut CompanionPhaseTracker,
    error_code: Option<&str>,
) -> Result<(), (AutoModeJobStatus, String)> {
    state
        .auto_mode_jobs
        .finish(
            job_id,
            status,
            pause_reason.clone(),
            error_code.map(str::to_owned),
            current_time_ms().map_err(|_| time_failure())?,
        )
        .map_err(|_| registry_failure())?;
    apply_finish_companion(
        app,
        state,
        status,
        pause_reason.as_deref(),
        error_code,
        companion,
    );
    Ok(())
}

fn apply_finish_companion(
    app: &AppHandle,
    state: &DesktopState,
    status: AutoModeJobStatus,
    pause_reason: Option<&str>,
    error_code: Option<&str>,
    companion: &mut CompanionPhaseTracker,
) {
    match status {
        AutoModeJobStatus::Failed | AutoModeJobStatus::Indeterminate => {
            companion.apply_failed_if_changed(app, state, error_code);
        }
        AutoModeJobStatus::Paused
            if matches!(
                pause_reason,
                Some("budget_exhausted") | Some("budget") | Some("token_budget") | Some("cost_budget")
            ) =>
        {
            companion.apply_budget_pause(app, state);
        }
        other => {
            if let Some(phase) = auto_mode_companion_status(other, pause_reason) {
                companion.apply_if_changed(app, state, phase);
            }
        }
    }
}

fn time_failure() -> (AutoModeJobStatus, String) {
    (AutoModeJobStatus::Failed, "clock-unavailable".to_owned())
}

fn registry_failure() -> (AutoModeJobStatus, String) {
    (AutoModeJobStatus::Failed, "job-registry-failure".to_owned())
}

fn pause_reason_code(reason: AutoModePauseReason) -> String {
    match reason {
        AutoModePauseReason::ConfirmationRequired => "confirmation_required",
        AutoModePauseReason::BudgetExhausted => "budget_exhausted",
        AutoModePauseReason::GoalChanged => "goal_changed",
        AutoModePauseReason::WorkspaceChanged => "workspace_changed",
        AutoModePauseReason::ProviderUnavailable => "provider_unavailable",
        AutoModePauseReason::Restarted => "restarted",
        AutoModePauseReason::UnsafeEffect => "unsafe_effect",
        AutoModePauseReason::UserRequested => "user_requested",
    }
    .to_owned()
}
