//! Auto Mode → structured pet directive mapping.
//!
//! Pure mapping mirrors FE `agentCompanion.ts` offline speech / action tokens so
//! native Auto Mode jobs drive the desktop pet through `nimora.pet_directive/1`.
//! Host wrappers prefer domain factories from `nimora_runtime_core` and only
//! layer tier-specific Chinese speech / body-language overrides where the domain
//! enum is coarser.

use nimora_runtime_core::{
    agent_status_directive, auto_mode_directive, automation_directive, grant_directive,
    AgentCompanionPhase, AttentionFocus, AutoModePetEvent, AutomationPhase, GrantPetEvent,
    MoodDelta, PetDirectiveAction, StructuredPetDirective,
};
use tauri::AppHandle;

use super::{auto_mode_jobs::AutoModeJobStatus, DesktopState};

const DIRECTIVE_SPEC_V1: &str = "nimora.pet_directive/1";

/// Companion presentation phase used for de-duplicating emits within a job run.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CompanionPhase {
    RunningWork,
    WaitingForConfirmation,
    Completed,
    Failed,
    Cancelled,
}

/// Maps Auto Mode job status (+ optional pause reason) onto a companion phase.
#[must_use]
pub(crate) fn auto_mode_companion_status(
    status: AutoModeJobStatus,
    pause_reason: Option<&str>,
) -> Option<CompanionPhase> {
    match status {
        AutoModeJobStatus::Starting
        | AutoModeJobStatus::Running
        | AutoModeJobStatus::Pausing
        | AutoModeJobStatus::Cancelling => Some(CompanionPhase::RunningWork),
        AutoModeJobStatus::Paused => {
            // confirmation_required / unsafe_effect / budget / other pause reasons
            // all surface as waiting_for_confirmation for de-dupe; richer speech
            // is chosen via [`auto_mode_pet_event_for_status`].
            let _ = pause_reason;
            Some(CompanionPhase::WaitingForConfirmation)
        }
        AutoModeJobStatus::Completed => Some(CompanionPhase::Completed),
        AutoModeJobStatus::Failed | AutoModeJobStatus::Indeterminate => Some(CompanionPhase::Failed),
        AutoModeJobStatus::Cancelled => Some(CompanionPhase::Cancelled),
    }
}

/// Maps job status (+ pause/error context) onto a domain Auto Mode pet event when
/// the richer auto-mode vocabulary applies (start / pause / budget / terminal).
#[must_use]
pub(crate) fn auto_mode_pet_event_for_status(
    status: AutoModeJobStatus,
    pause_reason: Option<&str>,
    error_code: Option<&str>,
) -> Option<AutoModePetEvent> {
    match status {
        AutoModeJobStatus::Starting
        | AutoModeJobStatus::Running
        | AutoModeJobStatus::Pausing
        | AutoModeJobStatus::Cancelling => Some(AutoModePetEvent::Started),
        AutoModeJobStatus::Paused if pause_reason_is_budget(pause_reason) => {
            Some(AutoModePetEvent::Budget)
        }
        AutoModeJobStatus::Paused => Some(AutoModePetEvent::StepPaused),
        AutoModeJobStatus::Completed => Some(AutoModePetEvent::Completed),
        AutoModeJobStatus::Failed | AutoModeJobStatus::Indeterminate
            if error_code_is_crash_like(error_code) =>
        {
            Some(AutoModePetEvent::Crashed)
        }
        AutoModeJobStatus::Failed | AutoModeJobStatus::Indeterminate => {
            // Generic failures still petize as a soft crash in the auto-mode vocabulary.
            Some(AutoModePetEvent::Crashed)
        }
        AutoModeJobStatus::Cancelled => None,
    }
}

#[allow(dead_code)] // documented confirmation-like codes; covered by unit tests
fn pause_reason_is_confirmation(pause_reason: Option<&str>) -> bool {
    matches!(
        pause_reason,
        Some("confirmation_required") | Some("unsafe_effect")
    )
}

/// True when the pause reason looks budget-related (`budget_exhausted`, etc.).
#[must_use]
pub(crate) fn pause_reason_is_budget(pause_reason: Option<&str>) -> bool {
    matches!(
        pause_reason,
        Some(reason) if reason.contains("budget")
    )
}

fn with_host_spec(mut directive: StructuredPetDirective) -> StructuredPetDirective {
    directive.spec = DIRECTIVE_SPEC_V1.to_owned();
    directive
}

/// Builds the structured directive for a companion phase (FE `agentCompanionDirective`).
///
/// Delegates to domain [`agent_status_directive`] so host and runtime-core stay aligned.
#[must_use]
pub(crate) fn companion_phase_directive(phase: CompanionPhase) -> StructuredPetDirective {
    let domain = match phase {
        CompanionPhase::RunningWork => AgentCompanionPhase::Running,
        CompanionPhase::WaitingForConfirmation => AgentCompanionPhase::WaitingConfirmation,
        CompanionPhase::Completed => AgentCompanionPhase::Completed,
        CompanionPhase::Failed => AgentCompanionPhase::Failed,
        CompanionPhase::Cancelled => AgentCompanionPhase::Cancelled,
    };
    with_host_spec(agent_status_directive(domain))
}

/// Auto Mode lifecycle → structured pet directive via domain [`auto_mode_directive`].
#[must_use]
pub(crate) fn auto_mode_lifecycle_directive(event: AutoModePetEvent) -> StructuredPetDirective {
    with_host_spec(auto_mode_directive(event))
}

/// Automation phases → structured pet directive via domain [`automation_directive`].
///
/// Prefer this helper from host automation entry points (see module docs / parent
/// paste snippets for `execute_admitted_automation_event`).
#[must_use]
pub(crate) fn automation_phase_directive(phase: AutomationPhase) -> StructuredPetDirective {
    with_host_spec(automation_directive(phase))
}

/// Grant lifecycle → pet body language (issued / revoked / high-risk warning).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum GrantCompanionEvent {
    IssuedObserve,
    IssuedWorkspace,
    IssuedTrusted,
    IssuedUnattended,
    IssuedFullDevice,
    Revoked,
    Expired,
}

/// Maps host grant tiers onto domain [`GrantPetEvent`] (coarser vocabulary).
#[must_use]
pub(crate) fn grant_pet_event_for(event: GrantCompanionEvent) -> GrantPetEvent {
    match event {
        GrantCompanionEvent::IssuedFullDevice => GrantPetEvent::FullDeviceWarning,
        GrantCompanionEvent::Revoked => GrantPetEvent::Revoked,
        GrantCompanionEvent::Expired => GrantPetEvent::Expired,
        GrantCompanionEvent::IssuedObserve
        | GrantCompanionEvent::IssuedWorkspace
        | GrantCompanionEvent::IssuedTrusted
        | GrantCompanionEvent::IssuedUnattended => GrantPetEvent::Issued,
    }
}

/// Builds grant companion directive by cloning domain [`grant_directive`] action /
/// animation / mood, then applying tier-specific Chinese speech (and body-language
/// overrides where domain `Issued` is coarser than host tiers).
#[must_use]
pub(crate) fn grant_event_directive(event: GrantCompanionEvent) -> StructuredPetDirective {
    let mut directive = with_host_spec(grant_directive(grant_pet_event_for(event)));
    match event {
        GrantCompanionEvent::IssuedObserve => {
            directive.speech = Some("观察模式，我先安静看着".to_owned());
            directive.mood_delta = None;
            directive.action = PetDirectiveAction::Observe;
            directive.animation = Some("pet.observe".to_owned());
            directive.attention = AttentionFocus::User;
        }
        GrantCompanionEvent::IssuedWorkspace => {
            directive.speech = Some("工作区授权到手，可以开工啦".to_owned());
            directive.mood_delta = Some(MoodDelta { mood: 3 });
            directive.action = PetDirectiveAction::Celebrate;
            directive.animation = Some("pet.celebrate".to_owned());
            directive.attention = AttentionFocus::User;
        }
        GrantCompanionEvent::IssuedTrusted => {
            directive.speech = Some("信任工作区：我会在边界内自动推进".to_owned());
            directive.mood_delta = Some(MoodDelta { mood: 4 });
            directive.action = PetDirectiveAction::WorkBusy;
            directive.animation = Some("pet.work".to_owned());
            directive.attention = AttentionFocus::User;
        }
        GrantCompanionEvent::IssuedUnattended => {
            directive.speech = Some("无人值守启动：安心睡，我守着".to_owned());
            directive.mood_delta = Some(MoodDelta { mood: 5 });
            directive.action = PetDirectiveAction::WorkBusy;
            directive.animation = Some("pet.work".to_owned());
            directive.attention = AttentionFocus::User;
        }
        // FullDeviceWarning / Revoked / Expired: domain speech + body language already match.
        GrantCompanionEvent::IssuedFullDevice
        | GrantCompanionEvent::Revoked
        | GrantCompanionEvent::Expired => {}
    }
    directive
}

/// Maps unattended authorization tier onto a grant companion event.
#[must_use]
pub(crate) fn grant_event_for_tier(tier: &str) -> GrantCompanionEvent {
    match tier {
        "observe" => GrantCompanionEvent::IssuedObserve,
        "workspace" => GrantCompanionEvent::IssuedWorkspace,
        "trusted_workspace" => GrantCompanionEvent::IssuedTrusted,
        "unattended" => GrantCompanionEvent::IssuedUnattended,
        "full_device" => GrantCompanionEvent::IssuedFullDevice,
        _ => GrantCompanionEvent::IssuedWorkspace,
    }
}

/// Failed/indeterminate path: prefer domain auto-mode crash for crash-like codes.
#[must_use]
pub(crate) fn failed_directive_for_error(error_code: Option<&str>) -> StructuredPetDirective {
    if error_code_is_crash_like(error_code) {
        auto_mode_lifecycle_directive(AutoModePetEvent::Crashed)
    } else {
        companion_phase_directive(CompanionPhase::Failed)
    }
}

fn error_code_is_crash_like(error_code: Option<&str>) -> bool {
    matches!(
        error_code,
        Some(code)
            if code.contains("crash")
                || code.contains("panic")
                || code.contains("indeterminate")
                || code == "execution-indeterminate"
    )
}

/// Applies a companion phase via the host helper (no wander).
///
/// Prefer richer Auto Mode vocabulary for start / pause / completed; keep agent
/// cancelled / generic failed paths on [`companion_phase_directive`].
pub(crate) fn apply_companion_phase(
    app: &AppHandle,
    _state: &DesktopState,
    phase: CompanionPhase,
) {
    let directive = match phase {
        CompanionPhase::RunningWork => auto_mode_lifecycle_directive(AutoModePetEvent::Started),
        CompanionPhase::WaitingForConfirmation => {
            auto_mode_lifecycle_directive(AutoModePetEvent::StepPaused)
        }
        CompanionPhase::Completed => auto_mode_lifecycle_directive(AutoModePetEvent::Completed),
        CompanionPhase::Failed | CompanionPhase::Cancelled => companion_phase_directive(phase),
    };
    let _ = super::apply_lifeform_directive_from_host(app, directive);
}

/// Applies a domain auto-mode event (budget / crash / etc.) without wander.
pub(crate) fn apply_auto_mode_event(
    app: &AppHandle,
    _state: &DesktopState,
    event: AutoModePetEvent,
) {
    let _ = super::apply_lifeform_directive_from_host(app, auto_mode_lifecycle_directive(event));
}

/// Applies a terminal failed directive, optionally using crash-like mapping.
pub(crate) fn apply_failed_companion(
    app: &AppHandle,
    _state: &DesktopState,
    error_code: Option<&str>,
) {
    let _ = super::apply_lifeform_directive_from_host(app, failed_directive_for_error(error_code));
}

/// Applies an automation phase directive when a native app handle is available.
pub(crate) fn apply_automation_phase(app: &AppHandle, phase: AutomationPhase) {
    let _ = super::apply_lifeform_directive_from_host(app, automation_phase_directive(phase));
}

/// Returns true when the Nth successful batch should surface a StepOk pet moment.
///
/// Host Auto Mode calls this cadence after every yielded batch with `turns_executed > 0`.
#[must_use]
pub(crate) fn should_emit_step_ok(step_ok_count: u32) -> bool {
    step_ok_count > 0 && step_ok_count % 3 == 0
}

/// Applies a grant companion event when a native app handle is available.
pub(crate) fn apply_grant_event(app: &AppHandle, event: GrantCompanionEvent) {
    let _ = super::apply_lifeform_directive_from_host(app, grant_event_directive(event));
}

/// Tracks last-emitted phase so auto-mode reasserts only on change.
#[derive(Debug, Default)]
pub(crate) struct CompanionPhaseTracker {
    last: Option<CompanionPhase>,
    last_auto_event: Option<AutoModePetEvent>,
    step_ok_count: u32,
}

impl CompanionPhaseTracker {
    /// Applies `phase` only when it differs from the last applied phase.
    pub(crate) fn apply_if_changed(
        &mut self,
        app: &AppHandle,
        state: &DesktopState,
        phase: CompanionPhase,
    ) {
        if self.last == Some(phase) {
            return;
        }
        apply_companion_phase(app, state, phase);
        self.last = Some(phase);
        self.last_auto_event = match phase {
            CompanionPhase::RunningWork => Some(AutoModePetEvent::Started),
            CompanionPhase::WaitingForConfirmation => Some(AutoModePetEvent::StepPaused),
            CompanionPhase::Completed => Some(AutoModePetEvent::Completed),
            CompanionPhase::Failed => Some(AutoModePetEvent::Crashed),
            CompanionPhase::Cancelled => None,
        };
    }

    /// Applies a domain auto-mode event, de-duped against the companion phase key.
    pub(crate) fn apply_auto_mode_if_changed(
        &mut self,
        app: &AppHandle,
        state: &DesktopState,
        event: AutoModePetEvent,
        phase: CompanionPhase,
    ) {
        if self.last == Some(phase) && self.last_auto_event == Some(event) {
            return;
        }
        apply_auto_mode_event(app, state, event);
        self.last = Some(phase);
        self.last_auto_event = Some(event);
    }

    /// Throttled StepOk observation after successful turn batches (every 3rd).
    ///
    /// Cadence is modulo-only: successive StepOk emits are allowed once every three
    /// successful batches so progress stays visible without speech spam.
    pub(crate) fn apply_step_ok_throttled(
        &mut self,
        app: &AppHandle,
        state: &DesktopState,
    ) {
        self.step_ok_count = self.step_ok_count.saturating_add(1);
        if !should_emit_step_ok(self.step_ok_count) {
            return;
        }
        apply_auto_mode_event(app, state, AutoModePetEvent::StepOk);
        self.last_auto_event = Some(AutoModePetEvent::StepOk);
        self.last = Some(CompanionPhase::RunningWork);
    }

    /// Pure inspection helpers for unit tests / host diagnostics.
    #[must_use]
    pub(crate) fn last_phase(&self) -> Option<CompanionPhase> {
        self.last
    }

    #[must_use]
    pub(crate) fn last_auto_event(&self) -> Option<AutoModePetEvent> {
        self.last_auto_event
    }

    #[must_use]
    pub(crate) fn step_ok_count(&self) -> u32 {
        self.step_ok_count
    }

    /// Budget pause uses domain Budget body language.
    pub(crate) fn apply_budget_pause(
        &mut self,
        app: &AppHandle,
        state: &DesktopState,
    ) {
        self.apply_auto_mode_if_changed(
            app,
            state,
            AutoModePetEvent::Budget,
            CompanionPhase::WaitingForConfirmation,
        );
    }

    /// Applies failed with optional crash mapping, de-duped against Failed phase.
    pub(crate) fn apply_failed_if_changed(
        &mut self,
        app: &AppHandle,
        state: &DesktopState,
        error_code: Option<&str>,
    ) {
        if self.last == Some(CompanionPhase::Failed) {
            return;
        }
        apply_failed_companion(app, state, error_code);
        self.last = Some(CompanionPhase::Failed);
        self.last_auto_event = Some(AutoModePetEvent::Crashed);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_active_statuses_to_running_work() {
        for status in [
            AutoModeJobStatus::Starting,
            AutoModeJobStatus::Running,
            AutoModeJobStatus::Pausing,
            AutoModeJobStatus::Cancelling,
        ] {
            assert_eq!(
                auto_mode_companion_status(status, None),
                Some(CompanionPhase::RunningWork)
            );
            assert_eq!(
                auto_mode_pet_event_for_status(status, None, None),
                Some(AutoModePetEvent::Started)
            );
        }
    }

    #[test]
    fn maps_paused_confirmation_and_other_to_waiting() {
        assert_eq!(
            auto_mode_companion_status(
                AutoModeJobStatus::Paused,
                Some("confirmation_required")
            ),
            Some(CompanionPhase::WaitingForConfirmation)
        );
        assert!(pause_reason_is_confirmation(Some("confirmation_required")));
        assert!(pause_reason_is_confirmation(Some("unsafe_effect")));
        assert!(!pause_reason_is_confirmation(Some("user_requested")));
        assert_eq!(
            auto_mode_companion_status(AutoModeJobStatus::Paused, Some("user_requested")),
            Some(CompanionPhase::WaitingForConfirmation)
        );
        assert_eq!(
            auto_mode_companion_status(AutoModeJobStatus::Paused, Some("workspace_changed")),
            Some(CompanionPhase::WaitingForConfirmation)
        );
        assert_eq!(
            auto_mode_pet_event_for_status(
                AutoModeJobStatus::Paused,
                Some("confirmation_required"),
                None
            ),
            Some(AutoModePetEvent::StepPaused)
        );
    }

    #[test]
    fn maps_budget_pause_to_budget_event() {
        assert!(pause_reason_is_budget(Some("budget_exhausted")));
        assert!(!pause_reason_is_budget(Some("user_requested")));
        assert_eq!(
            auto_mode_pet_event_for_status(
                AutoModeJobStatus::Paused,
                Some("budget_exhausted"),
                None
            ),
            Some(AutoModePetEvent::Budget)
        );
        let d = auto_mode_lifecycle_directive(AutoModePetEvent::Budget);
        assert_eq!(d.action, PetDirectiveAction::Rest);
        assert!(d.speech.as_deref().unwrap_or("").contains("预算"));
    }

    #[test]
    fn maps_terminal_statuses() {
        assert_eq!(
            auto_mode_companion_status(AutoModeJobStatus::Completed, None),
            Some(CompanionPhase::Completed)
        );
        assert_eq!(
            auto_mode_companion_status(AutoModeJobStatus::Failed, None),
            Some(CompanionPhase::Failed)
        );
        assert_eq!(
            auto_mode_companion_status(AutoModeJobStatus::Indeterminate, None),
            Some(CompanionPhase::Failed)
        );
        assert_eq!(
            auto_mode_companion_status(AutoModeJobStatus::Cancelled, None),
            Some(CompanionPhase::Cancelled)
        );
        assert_eq!(
            auto_mode_pet_event_for_status(AutoModeJobStatus::Completed, None, None),
            Some(AutoModePetEvent::Completed)
        );
        assert_eq!(
            auto_mode_pet_event_for_status(
                AutoModeJobStatus::Indeterminate,
                None,
                Some("execution-indeterminate")
            ),
            Some(AutoModePetEvent::Crashed)
        );
        assert_eq!(
            auto_mode_pet_event_for_status(AutoModeJobStatus::Cancelled, None, None),
            None
        );
    }

    #[test]
    fn running_directive_matches_fe_agent_companion() {
        let d = companion_phase_directive(CompanionPhase::RunningWork);
        assert_eq!(d.spec, "nimora.pet_directive/1");
        assert_eq!(d.speech.as_deref(), Some("正在陪你干活"));
        assert_eq!(d.action, PetDirectiveAction::WorkBusy);
        assert_eq!(d.animation.as_deref(), Some("pet.work"));
        assert_eq!(d.attention, AttentionFocus::User);
        assert!(d.mood_delta.is_none());
    }

    #[test]
    fn auto_mode_started_is_richer_than_agent_running() {
        let d = auto_mode_lifecycle_directive(AutoModePetEvent::Started);
        assert_eq!(d.action, PetDirectiveAction::WorkBusy);
        assert_eq!(d.animation.as_deref(), Some("pet.work"));
        assert_eq!(d.speech.as_deref(), Some("自动模式启动，我来盯着"));
        assert_eq!(d.mood_delta, Some(MoodDelta { mood: 2 }));
    }

    #[test]
    fn waiting_directive_matches_fe() {
        let d = companion_phase_directive(CompanionPhase::WaitingForConfirmation);
        assert_eq!(d.speech.as_deref(), Some("需要你确认一下"));
        assert_eq!(d.action, PetDirectiveAction::Perch);
        assert_eq!(d.animation.as_deref(), Some("pet.perch"));
        assert_eq!(d.attention, AttentionFocus::User);
    }

    #[test]
    fn completed_directive_matches_fe() {
        let d = companion_phase_directive(CompanionPhase::Completed);
        assert_eq!(d.speech.as_deref(), Some("完成啦！"));
        assert_eq!(d.action, PetDirectiveAction::Celebrate);
        assert_eq!(d.animation.as_deref(), Some("pet.celebrate"));
        assert_eq!(d.mood_delta, Some(MoodDelta { mood: 6 }));
        assert_eq!(d.attention, AttentionFocus::User);
    }

    #[test]
    fn auto_mode_completed_is_richer() {
        let d = auto_mode_lifecycle_directive(AutoModePetEvent::Completed);
        assert_eq!(d.speech.as_deref(), Some("自动目标完成！"));
        assert_eq!(d.action, PetDirectiveAction::Celebrate);
        assert_eq!(d.mood_delta, Some(MoodDelta { mood: 8 }));
    }

    #[test]
    fn failed_directive_matches_fe() {
        let d = companion_phase_directive(CompanionPhase::Failed);
        assert_eq!(d.speech.as_deref(), Some("没关系，我们再试"));
        assert_eq!(d.action, PetDirectiveAction::Rest);
        assert_eq!(d.animation.as_deref(), Some("pet.idle"));
        assert_eq!(d.mood_delta, Some(MoodDelta { mood: -2 }));
        assert_eq!(d.attention, AttentionFocus::IdleScene);
    }

    #[test]
    fn cancelled_directive_matches_fe() {
        let d = companion_phase_directive(CompanionPhase::Cancelled);
        assert_eq!(d.speech.as_deref(), Some("已停下，我还在"));
        assert_eq!(d.action, PetDirectiveAction::Rest);
        assert_eq!(d.animation.as_deref(), Some("pet.idle"));
        assert!(d.mood_delta.is_none());
        assert_eq!(d.attention, AttentionFocus::IdleScene);
    }

    #[test]
    fn crash_like_error_uses_work_crash() {
        let d = failed_directive_for_error(Some("execution-indeterminate"));
        assert_eq!(d.action, PetDirectiveAction::WorkCrash);
        assert_eq!(d.animation.as_deref(), Some("pet.work"));
        assert_eq!(d.speech.as_deref(), Some("哎呀晕了一下…"));
        assert_eq!(d.mood_delta, Some(MoodDelta { mood: -3 }));

        let generic = failed_directive_for_error(Some("persistence-unavailable"));
        assert_eq!(generic.action, PetDirectiveAction::Rest);
        assert_eq!(generic.animation.as_deref(), Some("pet.idle"));
    }

    #[test]
    fn directives_validate() {
        for phase in [
            CompanionPhase::RunningWork,
            CompanionPhase::WaitingForConfirmation,
            CompanionPhase::Completed,
            CompanionPhase::Failed,
            CompanionPhase::Cancelled,
        ] {
            companion_phase_directive(phase)
                .validate()
                .expect("directive should validate");
        }

        for event in [
            AutoModePetEvent::Started,
            AutoModePetEvent::StepOk,
            AutoModePetEvent::StepPaused,
            AutoModePetEvent::Budget,
            AutoModePetEvent::Completed,
            AutoModePetEvent::Crashed,
        ] {
            auto_mode_lifecycle_directive(event)
                .validate()
                .expect("auto mode directive should validate");
        }

        for phase in [
            AutomationPhase::Triggered,
            AutomationPhase::Succeeded,
            AutomationPhase::Failed,
            AutomationPhase::Throttled,
        ] {
            automation_phase_directive(phase)
                .validate()
                .expect("automation directive should validate");
        }

        grant_event_directive(GrantCompanionEvent::IssuedFullDevice)
            .validate()
            .expect("full device grant directive");
        grant_event_directive(GrantCompanionEvent::IssuedUnattended)
            .validate()
            .expect("unattended grant directive");
        grant_event_directive(GrantCompanionEvent::Revoked)
            .validate()
            .expect("revoked grant directive");
        grant_event_directive(GrantCompanionEvent::Expired)
            .validate()
            .expect("expired grant directive");
        grant_event_directive(GrantCompanionEvent::IssuedObserve)
            .validate()
            .expect("observe grant directive");
    }

    #[test]
    fn grant_full_device_is_cautious() {
        let d = grant_event_directive(GrantCompanionEvent::IssuedFullDevice);
        assert_eq!(d.action, PetDirectiveAction::Observe);
        assert!(d.speech.as_deref().unwrap_or("").contains("高风险"));
        assert_eq!(
            grant_pet_event_for(GrantCompanionEvent::IssuedFullDevice),
            GrantPetEvent::FullDeviceWarning
        );
    }

    #[test]
    fn grant_unattended_is_work_busy() {
        let d = grant_event_directive(GrantCompanionEvent::IssuedUnattended);
        assert_eq!(d.action, PetDirectiveAction::WorkBusy);
        assert_eq!(
            grant_event_for_tier("unattended"),
            GrantCompanionEvent::IssuedUnattended
        );
        assert_eq!(
            grant_event_for_tier("full_device"),
            GrantCompanionEvent::IssuedFullDevice
        );
        assert_eq!(
            grant_pet_event_for(GrantCompanionEvent::IssuedUnattended),
            GrantPetEvent::Issued
        );
    }

    #[test]
    fn grant_revoked_and_expired_delegate_domain_speech() {
        let revoked = grant_event_directive(GrantCompanionEvent::Revoked);
        assert_eq!(revoked.action, PetDirectiveAction::Rest);
        assert_eq!(
            revoked.speech.as_deref(),
            Some("授权已撤销，我停在安全边界")
        );
        let expired = grant_event_directive(GrantCompanionEvent::Expired);
        assert_eq!(expired.action, PetDirectiveAction::Perch);
        assert_eq!(
            expired.speech.as_deref(),
            Some("授权过期了，等你回来再继续")
        );
    }

    #[test]
    fn automation_triggered_observe() {
        let d = automation_phase_directive(AutomationPhase::Triggered);
        assert_eq!(d.action, PetDirectiveAction::Observe);
        assert_eq!(d.attention, AttentionFocus::NotificationArea);
        assert!(d.speech.as_deref().unwrap_or("").contains("自动化"));
    }

    #[test]
    fn step_ok_throttle_every_third_batch() {
        assert!(!should_emit_step_ok(0));
        assert!(!should_emit_step_ok(1));
        assert!(!should_emit_step_ok(2));
        assert!(should_emit_step_ok(3));
        assert!(!should_emit_step_ok(4));
        assert!(!should_emit_step_ok(5));
        assert!(should_emit_step_ok(6));
        assert!(should_emit_step_ok(9));
    }

    #[test]
    fn tracker_defaults_and_step_ok_helper_align() {
        let tracker = CompanionPhaseTracker::default();
        assert_eq!(tracker.step_ok_count(), 0);
        assert_eq!(tracker.last_phase(), None);
        assert_eq!(tracker.last_auto_event(), None);
        // Pure cadence is host-callable without AppHandle.
        assert!(!should_emit_step_ok(1));
        assert!(should_emit_step_ok(3));
        // Sixth batch may re-emit StepOk (modulo throttle only — not de-duped forever).
        assert!(should_emit_step_ok(6));
    }

    #[test]
    fn budget_and_pause_reason_helpers() {
        assert!(pause_reason_is_budget(Some("budget_exhausted")));
        assert!(pause_reason_is_budget(Some("agent_budget")));
        assert!(!pause_reason_is_budget(Some("confirmation_required")));
        assert!(!pause_reason_is_budget(None));
        assert_eq!(
            auto_mode_pet_event_for_status(
                AutoModeJobStatus::Paused,
                Some("budget_exhausted"),
                None
            ),
            Some(AutoModePetEvent::Budget)
        );
    }

    #[test]
    fn grant_event_for_all_tiers() {
        assert_eq!(
            grant_event_for_tier("observe"),
            GrantCompanionEvent::IssuedObserve
        );
        assert_eq!(
            grant_event_for_tier("workspace"),
            GrantCompanionEvent::IssuedWorkspace
        );
        assert_eq!(
            grant_event_for_tier("trusted_workspace"),
            GrantCompanionEvent::IssuedTrusted
        );
        assert_eq!(
            grant_event_for_tier("unknown_tier"),
            GrantCompanionEvent::IssuedWorkspace
        );
        let observe = grant_event_directive(GrantCompanionEvent::IssuedObserve);
        assert_eq!(observe.action, PetDirectiveAction::Observe);
        let trusted = grant_event_directive(GrantCompanionEvent::IssuedTrusted);
        assert_eq!(trusted.action, PetDirectiveAction::WorkBusy);
        assert!(trusted.speech.as_deref().unwrap_or("").contains("信任"));
    }

    #[test]
    fn automation_phase_directives_cover_all() {
        let succeeded = automation_phase_directive(AutomationPhase::Succeeded);
        assert_eq!(succeeded.action, PetDirectiveAction::Celebrate);
        let failed = automation_phase_directive(AutomationPhase::Failed);
        assert!(matches!(
            failed.action,
            PetDirectiveAction::Rest | PetDirectiveAction::WorkCrash | PetDirectiveAction::Observe
        ));
        let throttled = automation_phase_directive(AutomationPhase::Throttled);
        assert_eq!(throttled.attention, AttentionFocus::IdleScene);
    }

    #[test]
    fn host_spec_is_pet_directive_v1() {
        let d = auto_mode_lifecycle_directive(AutoModePetEvent::StepOk);
        assert_eq!(d.spec, DIRECTIVE_SPEC_V1);
        assert_eq!(d.speech.as_deref(), Some("这一步搞定了"));
    }

    #[test]
    fn agent_and_automation_chinese_speech_cover_all() {
        for phase in [
            CompanionPhase::RunningWork,
            CompanionPhase::WaitingForConfirmation,
            CompanionPhase::Completed,
            CompanionPhase::Failed,
            CompanionPhase::Cancelled,
        ] {
            let d = companion_phase_directive(phase);
            let speech = d.speech.as_deref().unwrap_or("");
            assert!(!speech.is_empty(), "{phase:?} missing speech");
            // Chinese speech: contains CJK ideographs.
            assert!(
                speech.chars().any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
                "{phase:?} speech not Chinese: {speech}"
            );
            assert_eq!(d.spec, DIRECTIVE_SPEC_V1);
        }

        for event in [
            AutoModePetEvent::Started,
            AutoModePetEvent::StepOk,
            AutoModePetEvent::StepPaused,
            AutoModePetEvent::Budget,
            AutoModePetEvent::Completed,
            AutoModePetEvent::Crashed,
        ] {
            let d = auto_mode_lifecycle_directive(event);
            let speech = d.speech.as_deref().unwrap_or("");
            assert!(
                speech.chars().any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
                "{event:?} speech not Chinese: {speech}"
            );
        }

        for phase in [
            AutomationPhase::Triggered,
            AutomationPhase::Succeeded,
            AutomationPhase::Failed,
            AutomationPhase::Throttled,
        ] {
            let d = automation_phase_directive(phase);
            let speech = d.speech.as_deref().unwrap_or("");
            assert!(
                speech.chars().any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
                "{phase:?} speech not Chinese: {speech}"
            );
        }

        for event in [
            GrantCompanionEvent::IssuedObserve,
            GrantCompanionEvent::IssuedWorkspace,
            GrantCompanionEvent::IssuedTrusted,
            GrantCompanionEvent::IssuedUnattended,
            GrantCompanionEvent::IssuedFullDevice,
            GrantCompanionEvent::Revoked,
            GrantCompanionEvent::Expired,
        ] {
            let d = grant_event_directive(event);
            let speech = d.speech.as_deref().unwrap_or("");
            assert!(
                speech.chars().any(|ch| ('\u{4e00}'..='\u{9fff}').contains(&ch)),
                "{event:?} speech not Chinese: {speech}"
            );
        }
    }
}
