use super::{
    AppHandle, AutoModeExecutionError, AutoModeExecutionService, AutoModeHostControl,
    AutoModeHostControlService, AutoModeJobControl, AutoModeJobStatus, AutoModeLoopRequest,
    AutoModeLoopService, AutoModeLoopStop, AutoModePauseReason, ContextCachePolicy,
    ContextCompactionPolicy, DataClassification, DesktopState, Duration, ProviderExecutionContext,
    StartAutoModeJobRequest, Uuid, WorkspaceScanPolicy, auto_mode_jobs, context_cache_key,
    current_time_ms, desktop_provider_registry, desktop_tool_backend, desktop_tool_registry,
    provider_credential_reference, resolve_provider_reasoning,
};
use nimora_agent_auto_host::{AutoModeRecoveryService, RecoveredAutoModeTurn};
use tauri::Manager;

pub(super) fn run(
    app: &AppHandle,
    job_id: Uuid,
    request: &StartAutoModeJobRequest,
    control: &auto_mode_jobs::AutoModeJobControlHandle,
) {
    let state = app.state::<DesktopState>();
    let result = run_inner(&state, job_id, request, control);
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
            .finish(job_id, status, None, Some(error_code), now_ms);
    }
}

fn run_inner(
    state: &DesktopState,
    job_id: Uuid,
    request: &StartAutoModeJobRequest,
    control: &auto_mode_jobs::AutoModeJobControlHandle,
) -> Result<(), (AutoModeJobStatus, String)> {
    let database_path = state.database_path.as_ref().ok_or_else(|| {
        (
            AutoModeJobStatus::Failed,
            "persistence-unavailable".to_owned(),
        )
    })?;
    let now_ms = current_time_ms().map_err(|_| time_failure())?;
    mark_job_running(state, job_id, control, now_ms)?;
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
            return commit_requested_control(state, job_id, turn, control, &host_control);
        }
        let mut logical_now = current_time_ms().map_err(|_| time_failure())?;
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
            AutoModeLoopStop::Yielded(next) => turn = *next,
            AutoModeLoopStop::Paused(paused) => {
                return finish(
                    state,
                    job_id,
                    AutoModeJobStatus::Paused,
                    paused.session.pause_reason.map(pause_reason_code),
                );
            }
            AutoModeLoopStop::Completed(_) => {
                return finish(state, job_id, AutoModeJobStatus::Completed, None);
            }
            AutoModeLoopStop::WorkspaceDrift { .. } => {
                return finish(
                    state,
                    job_id,
                    AutoModeJobStatus::Paused,
                    Some("workspace_changed".to_owned()),
                );
            }
        }
    }
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
    state: &DesktopState,
    job_id: Uuid,
    control: &auto_mode_jobs::AutoModeJobControlHandle,
    now_ms: u64,
) -> Result<(), (AutoModeJobStatus, String)> {
    if control.requested() == AutoModeJobControl::Continue {
        state
            .auto_mode_jobs
            .mark_running(job_id, now_ms)
            .map_err(|_| registry_failure())?;
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
    state: &DesktopState,
    job_id: Uuid,
    turn: RecoveredAutoModeTurn,
    control: &auto_mode_jobs::AutoModeJobControlHandle,
    host_control: &AutoModeHostControlService,
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
    state
        .auto_mode_jobs
        .finish(
            job_id,
            status,
            (status == AutoModeJobStatus::Paused).then(|| "user_requested".to_owned()),
            None,
            current_time_ms().map_err(|_| time_failure())?,
        )
        .map_err(|_| registry_failure())?;
    Ok(())
}

fn finish(
    state: &DesktopState,
    job_id: Uuid,
    status: AutoModeJobStatus,
    pause_reason: Option<String>,
) -> Result<(), (AutoModeJobStatus, String)> {
    state
        .auto_mode_jobs
        .finish(
            job_id,
            status,
            pause_reason,
            None,
            current_time_ms().map_err(|_| time_failure())?,
        )
        .map_err(|_| registry_failure())?;
    Ok(())
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
