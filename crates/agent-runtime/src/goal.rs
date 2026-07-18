use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

const MAX_TITLE_BYTES: usize = 256;
const MAX_OBJECTIVE_BYTES: usize = 32 * 1024;
const MAX_PLAN_STEPS: usize = 128;
const MAX_STEP_TEXT_BYTES: usize = 1024;
const MAX_EVIDENCE_PER_STEP: usize = 32;
const MAX_EVIDENCE_BYTES: usize = 2048;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentGoalStatus {
    Active,
    Paused,
    Completed,
    Cancelled,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AgentPlanStepStatus {
    Pending,
    InProgress,
    Completed,
    Blocked,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentPlanStep {
    pub id: Uuid,
    pub text: String,
    pub status: AgentPlanStepStatus,
    pub evidence: Vec<String>,
}

impl AgentPlanStep {
    /// Creates a bounded pending plan step.
    ///
    /// # Errors
    ///
    /// Returns an error when the step text is empty or exceeds its byte budget.
    pub fn new(text: impl Into<String>) -> Result<Self, AgentGoalError> {
        let text = text.into();
        let text = bounded_text(&text, MAX_STEP_TEXT_BYTES)?;
        Ok(Self {
            id: Uuid::now_v7(),
            text,
            status: AgentPlanStepStatus::Pending,
            evidence: Vec::new(),
        })
    }

    /// Replaces step status and evidence while preserving step identity.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid evidence or completed steps without evidence.
    pub fn update(
        &mut self,
        status: AgentPlanStepStatus,
        evidence: Vec<String>,
    ) -> Result<(), AgentGoalError> {
        validate_evidence(&evidence)?;
        if status == AgentPlanStepStatus::Completed && evidence.is_empty() {
            return Err(AgentGoalError::CompletionEvidenceRequired);
        }
        self.status = status;
        self.evidence = evidence;
        Ok(())
    }

    fn validate(&self) -> Result<(), AgentGoalError> {
        bounded_text(&self.text, MAX_STEP_TEXT_BYTES)?;
        validate_evidence(&self.evidence)?;
        if self.status == AgentPlanStepStatus::Completed && self.evidence.is_empty() {
            return Err(AgentGoalError::CompletionEvidenceRequired);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentPlan {
    pub spec: String,
    pub goal_id: Uuid,
    pub revision: u64,
    pub steps: Vec<AgentPlanStep>,
    pub rationale: String,
    pub created_at_ms: u64,
}

impl AgentPlan {
    /// Creates the first plan revision for a Goal.
    ///
    /// # Errors
    ///
    /// Returns an error for empty, duplicate, or oversized steps.
    pub fn new(
        goal_id: Uuid,
        steps: Vec<AgentPlanStep>,
        rationale: impl Into<String>,
        now_ms: u64,
    ) -> Result<Self, AgentGoalError> {
        let rationale = rationale.into();
        let plan = Self {
            spec: "nimora.agent-plan/1".to_owned(),
            goal_id,
            revision: 1,
            steps,
            rationale: bounded_text(&rationale, MAX_STEP_TEXT_BYTES)?,
            created_at_ms: now_ms,
        };
        plan.validate()?;
        Ok(plan)
    }

    /// Creates a new immutable plan revision.
    ///
    /// # Errors
    ///
    /// Returns an error for revision overflow or invalid plan contents.
    pub fn revise(
        &self,
        steps: Vec<AgentPlanStep>,
        rationale: impl Into<String>,
        now_ms: u64,
    ) -> Result<Self, AgentGoalError> {
        let rationale = rationale.into();
        let plan = Self {
            spec: self.spec.clone(),
            goal_id: self.goal_id,
            revision: self
                .revision
                .checked_add(1)
                .ok_or(AgentGoalError::PlanRevisionOverflow)?,
            steps,
            rationale: bounded_text(&rationale, MAX_STEP_TEXT_BYTES)?,
            created_at_ms: now_ms,
        };
        plan.validate()?;
        Ok(plan)
    }

    #[must_use]
    pub fn proves_completion(&self) -> bool {
        !self.steps.is_empty()
            && self.steps.iter().all(|step| {
                step.status == AgentPlanStepStatus::Completed && !step.evidence.is_empty()
            })
    }

    /// Validates a plan restored across a persistence or process boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when plan metadata, steps, or evidence violate the contract.
    pub fn validate(&self) -> Result<(), AgentGoalError> {
        if self.spec != "nimora.agent-plan/1"
            || self.revision == 0
            || self.steps.is_empty()
            || self.steps.len() > MAX_PLAN_STEPS
        {
            return Err(AgentGoalError::InvalidPlan);
        }
        bounded_text(&self.rationale, MAX_STEP_TEXT_BYTES)?;
        let mut ids = self.steps.iter().map(|step| step.id).collect::<Vec<_>>();
        ids.sort_unstable();
        ids.dedup();
        if ids.len() != self.steps.len() {
            return Err(AgentGoalError::InvalidPlan);
        }
        for step in &self.steps {
            step.validate()?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AgentGoal {
    pub spec: String,
    pub id: Uuid,
    pub title: String,
    pub objective: String,
    pub status: AgentGoalStatus,
    pub current_plan_revision: u64,
    pub created_at_ms: u64,
    pub updated_at_ms: u64,
    pub completed_at_ms: Option<u64>,
}

impl AgentGoal {
    /// Creates an active persistent Goal bound to its first plan revision.
    ///
    /// # Errors
    ///
    /// Returns an error when identity, text, timestamps, or plan binding is invalid.
    pub fn new(
        title: impl Into<String>,
        objective: impl Into<String>,
        plan: &AgentPlan,
        now_ms: u64,
    ) -> Result<Self, AgentGoalError> {
        if plan.revision != 1 || plan.created_at_ms != now_ms {
            return Err(AgentGoalError::InvalidPlanBinding);
        }
        let title = title.into();
        let objective = objective.into();
        let goal = Self {
            spec: "nimora.agent-goal/1".to_owned(),
            id: plan.goal_id,
            title: bounded_text(&title, MAX_TITLE_BYTES)?,
            objective: bounded_text(&objective, MAX_OBJECTIVE_BYTES)?,
            status: AgentGoalStatus::Active,
            current_plan_revision: plan.revision,
            created_at_ms: now_ms,
            updated_at_ms: now_ms,
            completed_at_ms: None,
        };
        goal.validate()?;
        Ok(goal)
    }

    /// Binds the Goal to a newer plan revision.
    ///
    /// # Errors
    ///
    /// Returns an error for terminal Goals, mismatched Goals, or non-sequential revisions.
    pub fn adopt_plan(&mut self, plan: &AgentPlan, now_ms: u64) -> Result<(), AgentGoalError> {
        if matches!(
            self.status,
            AgentGoalStatus::Completed | AgentGoalStatus::Cancelled
        ) {
            return Err(AgentGoalError::TerminalGoal);
        }
        if plan.goal_id != self.id
            || plan.revision != self.current_plan_revision.saturating_add(1)
            || now_ms < self.updated_at_ms
        {
            return Err(AgentGoalError::InvalidPlanBinding);
        }
        plan.validate()?;
        self.current_plan_revision = plan.revision;
        self.updated_at_ms = now_ms;
        Ok(())
    }

    /// Changes Goal lifecycle while enforcing evidence-backed completion.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid transitions or completion without current-plan evidence.
    pub fn transition(
        &mut self,
        next: AgentGoalStatus,
        current_plan: &AgentPlan,
        now_ms: u64,
    ) -> Result<(), AgentGoalError> {
        if current_plan.goal_id != self.id
            || current_plan.revision != self.current_plan_revision
            || now_ms < self.updated_at_ms
        {
            return Err(AgentGoalError::InvalidPlanBinding);
        }
        let allowed = matches!(
            (self.status, next),
            (AgentGoalStatus::Active, AgentGoalStatus::Paused)
                | (AgentGoalStatus::Paused, AgentGoalStatus::Active)
                | (
                    AgentGoalStatus::Active | AgentGoalStatus::Paused,
                    AgentGoalStatus::Completed | AgentGoalStatus::Cancelled
                )
        );
        if !allowed {
            return Err(AgentGoalError::InvalidGoalTransition);
        }
        if next == AgentGoalStatus::Completed && !current_plan.proves_completion() {
            return Err(AgentGoalError::CompletionEvidenceRequired);
        }
        self.status = next;
        self.updated_at_ms = now_ms;
        self.completed_at_ms = (next == AgentGoalStatus::Completed).then_some(now_ms);
        Ok(())
    }

    /// Validates a Goal restored across a persistence or process boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when metadata, lifecycle, timestamps, or text violate the contract.
    pub fn validate(&self) -> Result<(), AgentGoalError> {
        if self.spec != "nimora.agent-goal/1"
            || self.current_plan_revision == 0
            || self.updated_at_ms < self.created_at_ms
            || (self.status == AgentGoalStatus::Completed) != self.completed_at_ms.is_some()
        {
            return Err(AgentGoalError::InvalidGoal);
        }
        bounded_text(&self.title, MAX_TITLE_BYTES)?;
        bounded_text(&self.objective, MAX_OBJECTIVE_BYTES)?;
        Ok(())
    }
}

fn bounded_text(value: &str, max_bytes: usize) -> Result<String, AgentGoalError> {
    let value = value.trim().to_owned();
    if value.is_empty() || value.len() > max_bytes {
        return Err(AgentGoalError::InvalidText);
    }
    Ok(value)
}

fn validate_evidence(evidence: &[String]) -> Result<(), AgentGoalError> {
    if evidence.len() > MAX_EVIDENCE_PER_STEP
        || evidence
            .iter()
            .any(|item| item.trim().is_empty() || item.len() > MAX_EVIDENCE_BYTES)
    {
        return Err(AgentGoalError::InvalidEvidence);
    }
    Ok(())
}

#[derive(Debug, Error, Clone, Copy, PartialEq, Eq)]
pub enum AgentGoalError {
    #[error("Goal text is invalid")]
    InvalidText,
    #[error("Goal is invalid")]
    InvalidGoal,
    #[error("plan is invalid")]
    InvalidPlan,
    #[error("plan binding is invalid")]
    InvalidPlanBinding,
    #[error("plan revision overflowed")]
    PlanRevisionOverflow,
    #[error("Goal transition is invalid")]
    InvalidGoalTransition,
    #[error("terminal Goal cannot be changed")]
    TerminalGoal,
    #[error("completion requires evidence for every plan step")]
    CompletionEvidenceRequired,
    #[error("plan evidence is invalid")]
    InvalidEvidence,
}

#[cfg(test)]
mod tests {
    use super::{AgentGoal, AgentGoalStatus, AgentPlan, AgentPlanStep, AgentPlanStepStatus};
    use uuid::Uuid;

    fn fixture() -> (AgentGoal, AgentPlan) {
        let goal_id = Uuid::now_v7();
        let plan = AgentPlan::new(
            goal_id,
            vec![AgentPlanStep::new("Implement storage").expect("step")],
            "Initial plan",
            1_000,
        )
        .expect("plan");
        let goal =
            AgentGoal::new("Persistent Goals", "Persist Goals safely", &plan, 1_000).expect("Goal");
        (goal, plan)
    }

    #[test]
    fn completion_requires_current_plan_evidence() {
        let (mut goal, mut plan) = fixture();
        assert!(
            goal.transition(AgentGoalStatus::Completed, &plan, 1_001)
                .is_err()
        );
        plan.steps[0]
            .update(
                AgentPlanStepStatus::Completed,
                vec!["cargo test --workspace passed".to_owned()],
            )
            .expect("evidence");
        goal.transition(AgentGoalStatus::Completed, &plan, 1_002)
            .expect("complete");
        assert_eq!(goal.completed_at_ms, Some(1_002));
    }

    #[test]
    fn plan_revision_is_sequential_and_goal_scoped() {
        let (mut goal, plan) = fixture();
        let revised = plan
            .revise(
                vec![AgentPlanStep::new("Implement CLI").expect("step")],
                "Storage completed",
                1_010,
            )
            .expect("revision");
        goal.adopt_plan(&revised, 1_010).expect("adopt plan");
        assert_eq!(goal.current_plan_revision, 2);
    }
}
