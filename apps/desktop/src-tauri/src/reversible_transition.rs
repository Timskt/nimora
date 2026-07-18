#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ReversibleTransitionError<NativeError, CommitError> {
    NativeApply(NativeError),
    Commit(CommitError),
    Rollback {
        primary: CommitError,
        rollback: NativeError,
    },
}

pub(crate) fn run_reversible_transition<Policy, Output, NativeError, CommitError>(
    previous: Policy,
    target: Policy,
    mut apply_native: impl FnMut(Policy, Policy) -> Result<(), NativeError>,
    commit: impl FnOnce() -> Result<Output, CommitError>,
) -> Result<Output, ReversibleTransitionError<NativeError, CommitError>>
where
    Policy: Clone,
{
    apply_native(previous.clone(), target.clone())
        .map_err(ReversibleTransitionError::NativeApply)?;
    match commit() {
        Ok(output) => Ok(output),
        Err(primary) => match apply_native(target, previous) {
            Ok(()) => Err(ReversibleTransitionError::Commit(primary)),
            Err(rollback) => Err(ReversibleTransitionError::Rollback { primary, rollback }),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn native_apply_failure_never_commits() {
        let mut committed = false;
        let result = run_reversible_transition(
            "normal",
            "safe",
            |_, _| Err("native unavailable"),
            || {
                committed = true;
                Ok::<_, &str>(())
            },
        );

        assert_eq!(
            result,
            Err(ReversibleTransitionError::NativeApply("native unavailable"))
        );
        assert!(!committed);
    }

    #[test]
    fn commit_failure_rolls_native_policy_back() {
        let mut transitions = Vec::new();
        let result = run_reversible_transition(
            "normal",
            "safe",
            |from, to| {
                transitions.push((from, to));
                Ok::<_, &str>(())
            },
            || Err::<(), _>("commit rejected"),
        );

        assert_eq!(
            result,
            Err(ReversibleTransitionError::Commit("commit rejected"))
        );
        assert_eq!(transitions, [("normal", "safe"), ("safe", "normal")]);
    }

    #[test]
    fn rollback_failure_preserves_primary_and_secondary_causes() {
        let mut attempt = 0;
        let result = run_reversible_transition(
            "normal",
            "safe",
            |_, _| {
                attempt += 1;
                if attempt == 1 {
                    Ok(())
                } else {
                    Err("rollback failed")
                }
            },
            || Err::<(), _>("commit rejected"),
        );

        assert_eq!(
            result,
            Err(ReversibleTransitionError::Rollback {
                primary: "commit rejected",
                rollback: "rollback failed",
            })
        );
    }

    #[test]
    fn successful_commit_does_not_invoke_rollback() {
        let mut transitions = Vec::new();
        let result = run_reversible_transition(
            1,
            2,
            |from, to| {
                transitions.push((from, to));
                Ok::<_, &str>(())
            },
            || Ok::<_, &str>("committed"),
        );

        assert_eq!(result, Ok("committed"));
        assert_eq!(transitions, [(1, 2)]);
    }
}
