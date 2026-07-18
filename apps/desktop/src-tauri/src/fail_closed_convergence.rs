#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SafeModeConvergenceStep {
    AutoMode,
    UserPrograms,
    UserProgramEvents,
    SkillEvents,
    AutomationEvents,
    AgentTools,
    RememberWindowPolicy,
    CacheSafeWindowPolicy,
    RendererNotification,
    DiagnosticEvent,
}

impl SafeModeConvergenceStep {
    pub(crate) const fn code(self) -> &'static str {
        match self {
            Self::AutoMode => "auto-mode",
            Self::UserPrograms => "user-programs",
            Self::UserProgramEvents => "user-program-events",
            Self::SkillEvents => "skill-events",
            Self::AutomationEvents => "automation-events",
            Self::AgentTools => "agent-tools",
            Self::RememberWindowPolicy => "remember-window-policy",
            Self::CacheSafeWindowPolicy => "cache-safe-window-policy",
            Self::RendererNotification => "renderer-notification",
            Self::DiagnosticEvent => "diagnostic-event",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct SafeModeConvergenceFailure {
    failed_steps: Vec<SafeModeConvergenceStep>,
}

impl SafeModeConvergenceFailure {
    pub(crate) fn failed_step_codes(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.failed_steps.iter().map(|step| step.code())
    }
}

pub(crate) trait SafeModeConvergenceOperations {
    type Error;

    fn quiesce_auto_mode(&mut self) -> Result<(), Self::Error>;
    fn cancel_user_programs(&mut self) -> Result<(), Self::Error>;
    fn cancel_user_program_events(&mut self) -> Result<(), Self::Error>;
    fn stop_skill_events(&mut self) -> Result<(), Self::Error>;
    fn stop_automation_events(&mut self) -> Result<(), Self::Error>;
    fn cancel_agent_tools(&mut self) -> Result<(), Self::Error>;
    fn remember_window_policy(&mut self) -> Result<(), Self::Error>;
    fn cache_safe_window_policy(&mut self) -> Result<(), Self::Error>;
    fn notify_renderer(&mut self) -> Result<(), Self::Error>;
    fn record_convergence_failure(&mut self) -> Result<(), Self::Error>;
}

pub(crate) fn converge_safe_mode(
    operations: &mut impl SafeModeConvergenceOperations,
) -> Result<(), SafeModeConvergenceFailure> {
    let mut failed_steps = Vec::new();

    attempt(&mut failed_steps, SafeModeConvergenceStep::AutoMode, || {
        operations.quiesce_auto_mode()
    });
    attempt(
        &mut failed_steps,
        SafeModeConvergenceStep::UserPrograms,
        || operations.cancel_user_programs(),
    );
    attempt(
        &mut failed_steps,
        SafeModeConvergenceStep::UserProgramEvents,
        || operations.cancel_user_program_events(),
    );
    attempt(
        &mut failed_steps,
        SafeModeConvergenceStep::SkillEvents,
        || operations.stop_skill_events(),
    );
    attempt(
        &mut failed_steps,
        SafeModeConvergenceStep::AutomationEvents,
        || operations.stop_automation_events(),
    );
    attempt(
        &mut failed_steps,
        SafeModeConvergenceStep::AgentTools,
        || operations.cancel_agent_tools(),
    );
    attempt(
        &mut failed_steps,
        SafeModeConvergenceStep::RememberWindowPolicy,
        || operations.remember_window_policy(),
    );
    attempt(
        &mut failed_steps,
        SafeModeConvergenceStep::CacheSafeWindowPolicy,
        || operations.cache_safe_window_policy(),
    );
    attempt(
        &mut failed_steps,
        SafeModeConvergenceStep::RendererNotification,
        || operations.notify_renderer(),
    );

    if failed_steps.is_empty() {
        return Ok(());
    }

    attempt(
        &mut failed_steps,
        SafeModeConvergenceStep::DiagnosticEvent,
        || operations.record_convergence_failure(),
    );
    Err(SafeModeConvergenceFailure { failed_steps })
}

fn attempt<Error>(
    failed_steps: &mut Vec<SafeModeConvergenceStep>,
    step: SafeModeConvergenceStep,
    operation: impl FnOnce() -> Result<(), Error>,
) {
    if operation().is_err() {
        failed_steps.push(step);
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::*;

    #[derive(Default)]
    struct Operations {
        calls: Vec<SafeModeConvergenceStep>,
        failures: BTreeSet<&'static str>,
    }

    impl Operations {
        fn run(&mut self, step: SafeModeConvergenceStep) -> Result<(), &'static str> {
            self.calls.push(step);
            if self.failures.contains(step.code()) {
                Err("secret host error containing /private/path")
            } else {
                Ok(())
            }
        }
    }

    impl SafeModeConvergenceOperations for Operations {
        type Error = &'static str;

        fn quiesce_auto_mode(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::AutoMode)
        }

        fn cancel_user_programs(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::UserPrograms)
        }

        fn cancel_user_program_events(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::UserProgramEvents)
        }

        fn stop_skill_events(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::SkillEvents)
        }

        fn stop_automation_events(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::AutomationEvents)
        }

        fn cancel_agent_tools(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::AgentTools)
        }

        fn remember_window_policy(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::RememberWindowPolicy)
        }

        fn cache_safe_window_policy(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::CacheSafeWindowPolicy)
        }

        fn notify_renderer(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::RendererNotification)
        }

        fn record_convergence_failure(&mut self) -> Result<(), Self::Error> {
            self.run(SafeModeConvergenceStep::DiagnosticEvent)
        }
    }

    #[test]
    fn successful_convergence_runs_every_isolation_step_without_diagnostic() {
        let mut operations = Operations::default();

        assert_eq!(converge_safe_mode(&mut operations), Ok(()));
        assert_eq!(
            operations.calls,
            [
                SafeModeConvergenceStep::AutoMode,
                SafeModeConvergenceStep::UserPrograms,
                SafeModeConvergenceStep::UserProgramEvents,
                SafeModeConvergenceStep::SkillEvents,
                SafeModeConvergenceStep::AutomationEvents,
                SafeModeConvergenceStep::AgentTools,
                SafeModeConvergenceStep::RememberWindowPolicy,
                SafeModeConvergenceStep::CacheSafeWindowPolicy,
                SafeModeConvergenceStep::RendererNotification,
            ]
        );
    }

    #[test]
    fn early_failure_does_not_skip_later_isolation_or_diagnostic() {
        let mut operations = Operations {
            failures: BTreeSet::from(["auto-mode"]),
            ..Operations::default()
        };

        let failure = converge_safe_mode(&mut operations).expect_err("must fail closed");

        assert_eq!(operations.calls.len(), 10);
        assert_eq!(
            operations.calls[9],
            SafeModeConvergenceStep::DiagnosticEvent
        );
        assert_eq!(
            failure.failed_step_codes().collect::<Vec<_>>(),
            ["auto-mode"]
        );
    }

    #[test]
    fn multiple_failures_are_reported_in_stable_execution_order() {
        let mut operations = Operations {
            failures: BTreeSet::from([
                "user-programs",
                "agent-tools",
                "renderer-notification",
                "diagnostic-event",
            ]),
            ..Operations::default()
        };

        let failure = converge_safe_mode(&mut operations).expect_err("must fail closed");

        assert_eq!(
            failure.failed_step_codes().collect::<Vec<_>>(),
            [
                "user-programs",
                "agent-tools",
                "renderer-notification",
                "diagnostic-event"
            ]
        );
        assert!(!format!("{failure:?}").contains("private/path"));
    }
}
