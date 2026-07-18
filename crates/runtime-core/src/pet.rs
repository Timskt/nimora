use serde::{Deserialize, Serialize};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct PetId(Uuid);

impl PetId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for PetId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetState {
    Idle,
    Walking,
    Sleeping,
    Dragged,
    Interacting,
    Working,
    Recovering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Emotion {
    Neutral,
    Happy,
    Sad,
    Angry,
    Surprised,
    Focused,
    Sleepy,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetAction {
    Idle,
    Walk,
    Sleep,
    Work,
    Celebrate,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetCareAction {
    Feed,
    Play,
    Groom,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetIntent {
    Observe,
    Explore,
    Rest,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetAutonomyPolicy {
    pub enabled: bool,
    pub quiet: bool,
    pub focus: bool,
    pub idle_delay_ms: u64,
    pub action_duration_ms: u64,
    pub cooldown_ms: u64,
}

impl Default for PetAutonomyPolicy {
    fn default() -> Self {
        Self {
            enabled: true,
            quiet: false,
            focus: false,
            idle_delay_ms: 20_000,
            action_duration_ms: 8_000,
            cooldown_ms: 45_000,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetAutonomyState {
    pub sequence: u64,
    pub next_due_ms: u64,
    pub active_until_ms: Option<u64>,
    pub active_intent: Option<PetIntent>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetAutonomyDecision {
    Noop,
    Schedule {
        next_due_ms: u64,
    },
    Start {
        intent: PetIntent,
        action: PetAction,
    },
    Finish,
    Suppress,
    Interrupt,
}

impl PetAction {
    pub const ALL: [Self; 5] = [
        Self::Idle,
        Self::Walk,
        Self::Sleep,
        Self::Work,
        Self::Celebrate,
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PointerButton {
    Left,
    Middle,
    Right,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
}

impl Position {
    /// Validates a desktop coordinate at an external input boundary.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidPosition`] for NaN or infinite values.
    pub fn validate(self) -> Result<(), PetError> {
        if !self.x.is_finite() || !self.y.is_finite() {
            return Err(PetError::InvalidPosition);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Pet {
    pub id: PetId,
    pub name: String,
    pub state: PetState,
    pub emotion: Emotion,
    pub position: Position,
    pub energy: u8,
    pub mood: u8,
    pub affinity: u8,
    #[serde(default)]
    pub last_vitals_update_ms: u64,
    #[serde(default)]
    pub last_care_ms: u64,
    #[serde(default)]
    pub autonomy: PetAutonomyState,
}

impl Pet {
    const LOW_VITAL_THRESHOLD: u8 = 25;

    fn idle_emotion(&self) -> Emotion {
        if self.energy <= Self::LOW_VITAL_THRESHOLD {
            Emotion::Sleepy
        } else if self.mood <= Self::LOW_VITAL_THRESHOLD {
            Emotion::Sad
        } else {
            Emotion::Neutral
        }
    }

    fn enter_idle(&mut self) {
        self.state = PetState::Idle;
        self.emotion = self.idle_emotion();
    }

    /// Creates a pet with safe default state and vitals.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidName`] when the trimmed name is empty or
    /// longer than 64 Unicode scalar values.
    pub fn new(name: impl Into<String>) -> Result<Self, PetError> {
        let name = name.into().trim().to_owned();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(PetError::InvalidName);
        }
        Ok(Self {
            id: PetId::new(),
            name,
            state: PetState::Idle,
            emotion: Emotion::Neutral,
            position: Position { x: 0.0, y: 0.0 },
            energy: 100,
            mood: 70,
            affinity: 0,
            last_vitals_update_ms: 0,
            last_care_ms: 0,
            autonomy: PetAutonomyState::default(),
        })
    }

    /// Moves the pet to a finite desktop coordinate.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidPosition`] when either coordinate is NaN or
    /// infinite.
    pub fn move_to(&mut self, position: Position) -> Result<(), PetError> {
        position.validate()?;
        self.position = position;
        Ok(())
    }

    /// Enters the semantic pointer-interaction state.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidTransition`] while a drag is active.
    pub fn interact(&mut self) -> Result<(), PetError> {
        if self.state == PetState::Dragged {
            return Err(PetError::InvalidTransition);
        }
        self.state = PetState::Interacting;
        self.emotion = Emotion::Happy;
        self.mood = self.mood.saturating_add(2).min(100);
        self.affinity = self.affinity.saturating_add(1).min(100);
        Ok(())
    }

    /// Returns an active interaction to the neutral idle state.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidTransition`] after a newer state replaced it.
    pub fn finish_interaction(&mut self) -> Result<(), PetError> {
        if self.state != PetState::Interacting {
            return Err(PetError::InvalidTransition);
        }
        self.enter_idle();
        Ok(())
    }

    /// Enters the highest-priority drag state.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidTransition`] when already being dragged.
    pub fn begin_drag(&mut self) -> Result<(), PetError> {
        if self.state == PetState::Dragged {
            return Err(PetError::InvalidTransition);
        }
        self.state = PetState::Dragged;
        self.emotion = Emotion::Surprised;
        Ok(())
    }

    /// Finishes an active drag and atomically applies its final position.
    ///
    /// # Errors
    ///
    /// Returns an error when no drag is active or the final position is invalid.
    pub fn drop_at(&mut self, position: Position) -> Result<(), PetError> {
        if self.state != PetState::Dragged {
            return Err(PetError::InvalidTransition);
        }
        self.move_to(position)?;
        self.enter_idle();
        Ok(())
    }

    /// Validates a pet restored from an external persistence boundary.
    ///
    /// # Errors
    ///
    /// Returns a domain error when the name, position, or vitals violate the
    /// same invariants enforced for newly created pets.
    pub fn validate(&self) -> Result<(), PetError> {
        if self.name.trim().is_empty() || self.name.chars().count() > 64 {
            return Err(PetError::InvalidName);
        }
        if !self.position.x.is_finite() || !self.position.y.is_finite() {
            return Err(PetError::InvalidPosition);
        }
        if self.energy > 100 || self.mood > 100 || self.affinity > 100 {
            return Err(PetError::InvalidVitals);
        }
        Ok(())
    }

    pub fn apply_action(&mut self, action: PetAction) {
        if action == PetAction::Idle {
            self.enter_idle();
            return;
        }
        (self.state, self.emotion) = match action {
            PetAction::Idle => unreachable!("idle action returned above"),
            PetAction::Walk => (PetState::Walking, Emotion::Happy),
            PetAction::Sleep => (PetState::Sleeping, Emotion::Sleepy),
            PetAction::Work => (PetState::Working, Emotion::Focused),
            PetAction::Celebrate => (PetState::Interacting, Emotion::Happy),
        };
    }

    pub fn set_energy(&mut self, energy: i16) {
        self.energy = u8::try_from(energy.clamp(0, 100)).unwrap_or_default();
        if self.state == PetState::Idle {
            self.emotion = self.idle_emotion();
        }
    }

    #[must_use]
    pub fn vitals_update_due(&self, now_ms: u64, interval_ms: u64) -> bool {
        self.last_vitals_update_ms == 0
            || now_ms.saturating_sub(self.last_vitals_update_ms) >= interval_ms
    }

    pub fn update_vitals(&mut self, now_ms: u64, interval_ms: u64, max_intervals: u64) {
        if self.last_vitals_update_ms == 0 {
            self.last_vitals_update_ms = now_ms;
            return;
        }
        let intervals = now_ms
            .saturating_sub(self.last_vitals_update_ms)
            .checked_div(interval_ms.max(1))
            .unwrap_or_default()
            .min(max_intervals);
        if intervals == 0 {
            return;
        }
        let energy_loss = u8::try_from(intervals.min(u64::from(u8::MAX))).unwrap_or(u8::MAX);
        let mood_loss = u8::try_from((intervals / 3).min(u64::from(u8::MAX))).unwrap_or(u8::MAX);
        self.energy = self.energy.saturating_sub(energy_loss);
        self.mood = self.mood.saturating_sub(mood_loss);
        self.last_vitals_update_ms = now_ms;
        if self.state == PetState::Idle {
            self.emotion = self.idle_emotion();
        }
    }

    /// Applies one direct care action while enforcing user-state priority and cooldown.
    ///
    /// # Errors
    ///
    /// Returns an invalid transition while dragged or a cooldown error when care is repeated early.
    pub fn care(
        &mut self,
        action: PetCareAction,
        now_ms: u64,
        cooldown_ms: u64,
    ) -> Result<(), PetError> {
        if self.state == PetState::Dragged {
            return Err(PetError::InvalidTransition);
        }
        if self.last_care_ms != 0 && now_ms.saturating_sub(self.last_care_ms) < cooldown_ms {
            return Err(PetError::CareCooldown);
        }
        match action {
            PetCareAction::Feed => {
                self.energy = self.energy.saturating_add(20).min(100);
                self.mood = self.mood.saturating_add(2).min(100);
                self.affinity = self.affinity.saturating_add(1).min(100);
            }
            PetCareAction::Play => {
                self.energy = self.energy.saturating_sub(5);
                self.mood = self.mood.saturating_add(12).min(100);
                self.affinity = self.affinity.saturating_add(2).min(100);
            }
            PetCareAction::Groom => {
                self.mood = self.mood.saturating_add(6).min(100);
                self.affinity = self.affinity.saturating_add(3).min(100);
            }
        }
        self.state = PetState::Interacting;
        self.emotion = Emotion::Happy;
        self.autonomy.active_intent = None;
        self.autonomy.active_until_ms = None;
        self.last_care_ms = now_ms;
        Ok(())
    }

    pub fn recover_transient_state(&mut self) -> bool {
        if matches!(
            self.state,
            PetState::Dragged | PetState::Interacting | PetState::Recovering
        ) {
            self.enter_idle();
            return true;
        }
        false
    }

    #[must_use]
    pub fn autonomy_decision(&self, policy: PetAutonomyPolicy, now_ms: u64) -> PetAutonomyDecision {
        if let Some(active_until_ms) = self.autonomy.active_until_ms {
            let expected_state = match self.autonomy.active_intent {
                Some(PetIntent::Observe) => PetState::Interacting,
                Some(PetIntent::Explore) => PetState::Walking,
                Some(PetIntent::Rest) => PetState::Sleeping,
                None => return PetAutonomyDecision::Interrupt,
            };
            if self.state != expected_state {
                return PetAutonomyDecision::Interrupt;
            }
            if !policy.enabled || policy.quiet || policy.focus {
                return PetAutonomyDecision::Suppress;
            }
            return if now_ms >= active_until_ms {
                PetAutonomyDecision::Finish
            } else {
                PetAutonomyDecision::Noop
            };
        }
        if !policy.enabled || policy.quiet || policy.focus {
            return PetAutonomyDecision::Noop;
        }
        if self.state != PetState::Idle {
            return PetAutonomyDecision::Noop;
        }
        if self.autonomy.next_due_ms == 0 {
            return PetAutonomyDecision::Schedule {
                next_due_ms: now_ms.saturating_add(policy.idle_delay_ms),
            };
        }
        if now_ms < self.autonomy.next_due_ms {
            return PetAutonomyDecision::Noop;
        }
        let (intent, action) = if self.energy <= Self::LOW_VITAL_THRESHOLD {
            (PetIntent::Rest, PetAction::Sleep)
        } else if self.mood <= Self::LOW_VITAL_THRESHOLD {
            (PetIntent::Observe, PetAction::Celebrate)
        } else {
            match self.autonomy.sequence % 3 {
                0 => (PetIntent::Observe, PetAction::Celebrate),
                1 => (PetIntent::Explore, PetAction::Walk),
                _ => (PetIntent::Rest, PetAction::Sleep),
            }
        };
        PetAutonomyDecision::Start { intent, action }
    }

    pub fn apply_autonomy_decision(
        &mut self,
        decision: PetAutonomyDecision,
        policy: PetAutonomyPolicy,
        now_ms: u64,
    ) {
        match decision {
            PetAutonomyDecision::Noop => {}
            PetAutonomyDecision::Schedule { next_due_ms } => {
                self.autonomy.next_due_ms = next_due_ms;
            }
            PetAutonomyDecision::Start { intent, action } => {
                self.apply_action(action);
                self.autonomy.active_intent = Some(intent);
                self.autonomy.active_until_ms =
                    Some(now_ms.saturating_add(policy.action_duration_ms));
                self.autonomy.sequence = self.autonomy.sequence.saturating_add(1);
            }
            PetAutonomyDecision::Finish => {
                self.apply_action(PetAction::Idle);
                self.autonomy.active_intent = None;
                self.autonomy.active_until_ms = None;
                self.autonomy.next_due_ms = now_ms.saturating_add(policy.cooldown_ms);
            }
            PetAutonomyDecision::Suppress => {
                self.apply_action(PetAction::Idle);
                self.autonomy.active_intent = None;
                self.autonomy.active_until_ms = None;
                self.autonomy.next_due_ms = 0;
            }
            PetAutonomyDecision::Interrupt => {
                self.autonomy.active_intent = None;
                self.autonomy.active_until_ms = None;
                self.autonomy.next_due_ms = now_ms.saturating_add(policy.cooldown_ms);
            }
        }
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PetError {
    #[error("pet name must contain 1 to 64 characters")]
    InvalidName,
    #[error("pet position must be finite")]
    InvalidPosition,
    #[error("pet vitals must be between 0 and 100")]
    InvalidVitals,
    #[error("pet care action is cooling down")]
    CareCooldown,
    #[error("pet state transition is not allowed")]
    InvalidTransition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn autonomy_is_deterministic_and_respects_cooldown() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        let policy = PetAutonomyPolicy {
            idle_delay_ms: 10,
            action_duration_ms: 5,
            cooldown_ms: 20,
            ..PetAutonomyPolicy::default()
        };
        let scheduled = pet.autonomy_decision(policy, 100);
        assert_eq!(
            scheduled,
            PetAutonomyDecision::Schedule { next_due_ms: 110 }
        );
        pet.apply_autonomy_decision(scheduled, policy, 100);
        let started = pet.autonomy_decision(policy, 110);
        assert_eq!(
            started,
            PetAutonomyDecision::Start {
                intent: PetIntent::Observe,
                action: PetAction::Celebrate,
            }
        );
        pet.apply_autonomy_decision(started, policy, 110);
        assert_eq!(pet.state, PetState::Interacting);
        let finished = pet.autonomy_decision(policy, 115);
        assert_eq!(finished, PetAutonomyDecision::Finish);
        pet.apply_autonomy_decision(finished, policy, 115);
        assert_eq!(pet.state, PetState::Idle);
        assert_eq!(pet.autonomy.next_due_ms, 135);
    }

    #[test]
    fn low_vitals_prioritize_rest_then_gentle_attention() {
        let policy = PetAutonomyPolicy::default();
        let mut tired = Pet::new("Aster").expect("valid pet");
        tired.energy = 25;
        tired.mood = 10;
        tired.autonomy.next_due_ms = 100;
        assert_eq!(
            tired.autonomy_decision(policy, 100),
            PetAutonomyDecision::Start {
                intent: PetIntent::Rest,
                action: PetAction::Sleep,
            }
        );

        let mut unhappy = Pet::new("Aster").expect("valid pet");
        unhappy.energy = 80;
        unhappy.mood = 25;
        unhappy.autonomy.next_due_ms = 100;
        assert_eq!(
            unhappy.autonomy_decision(policy, 100),
            PetAutonomyDecision::Start {
                intent: PetIntent::Observe,
                action: PetAction::Celebrate,
            }
        );
    }

    #[test]
    fn idle_emotion_tracks_vitals_without_overwriting_active_states() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.set_energy(25);
        assert_eq!(pet.emotion, Emotion::Sleepy);
        pet.apply_action(PetAction::Work);
        pet.set_energy(10);
        assert_eq!(pet.emotion, Emotion::Focused);
        pet.apply_action(PetAction::Idle);
        assert_eq!(pet.emotion, Emotion::Sleepy);
        pet.energy = 80;
        pet.mood = 20;
        pet.apply_action(PetAction::Idle);
        assert_eq!(pet.emotion, Emotion::Sad);
    }

    #[test]
    fn user_state_interrupts_autonomy_without_being_overwritten() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        let policy = PetAutonomyPolicy::default();
        pet.apply_autonomy_decision(
            PetAutonomyDecision::Start {
                intent: PetIntent::Explore,
                action: PetAction::Walk,
            },
            policy,
            100,
        );
        pet.begin_drag().expect("user drag preempts autonomy");
        let decision = pet.autonomy_decision(policy, 101);
        assert_eq!(decision, PetAutonomyDecision::Interrupt);
        pet.apply_autonomy_decision(decision, policy, 101);
        assert_eq!(pet.state, PetState::Dragged);
        assert_eq!(pet.autonomy.active_intent, None);
    }

    #[test]
    fn suppressed_policy_stops_matching_autonomy_immediately() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        let active_policy = PetAutonomyPolicy::default();
        pet.apply_autonomy_decision(
            PetAutonomyDecision::Start {
                intent: PetIntent::Explore,
                action: PetAction::Walk,
            },
            active_policy,
            100,
        );
        let quiet_policy = PetAutonomyPolicy {
            quiet: true,
            ..active_policy
        };
        let decision = pet.autonomy_decision(quiet_policy, 101);
        assert_eq!(decision, PetAutonomyDecision::Suppress);
        pet.apply_autonomy_decision(decision, quiet_policy, 101);
        assert_eq!(pet.state, PetState::Idle);
        assert_eq!(pet.autonomy.sequence, 1);
        assert_eq!(pet.autonomy.next_due_ms, 0);
        assert_eq!(pet.autonomy.active_until_ms, None);
        assert_eq!(pet.autonomy.active_intent, None);
    }

    #[test]
    fn clamps_vitals_to_domain_range() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.set_energy(-20);
        assert_eq!(pet.energy, 0);
        pet.set_energy(500);
        assert_eq!(pet.energy, 100);
    }

    #[test]
    fn vitals_initialize_then_apply_bounded_offline_decay() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.update_vitals(1_000, 100, 6);
        assert_eq!(pet.energy, 100);
        pet.update_vitals(11_000, 100, 6);
        assert_eq!(pet.energy, 94);
        assert_eq!(pet.mood, 68);
        assert_eq!(pet.last_vitals_update_ms, 11_000);
    }

    #[test]
    fn interaction_improves_mood_and_affinity_without_overflow() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.mood = 99;
        pet.affinity = 100;
        pet.interact().expect("interaction");
        assert_eq!(pet.mood, 100);
        assert_eq!(pet.affinity, 100);
    }

    #[test]
    fn care_actions_apply_distinct_bounded_vital_changes() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.energy = 50;
        pet.mood = 50;
        pet.care(PetCareAction::Feed, 1_000, 30_000).expect("feed");
        assert_eq!((pet.energy, pet.mood, pet.affinity), (70, 52, 1));
        assert_eq!(pet.state, PetState::Interacting);
        assert_eq!(
            pet.care(PetCareAction::Play, 2_000, 30_000),
            Err(PetError::CareCooldown)
        );
        pet.care(PetCareAction::Play, 31_000, 30_000).expect("play");
        assert_eq!((pet.energy, pet.mood, pet.affinity), (65, 64, 3));
    }

    #[test]
    fn care_never_overrides_drag_state() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.begin_drag().expect("drag");
        assert_eq!(
            pet.care(PetCareAction::Groom, 1_000, 30_000),
            Err(PetError::InvalidTransition)
        );
        assert_eq!(pet.state, PetState::Dragged);
    }

    #[test]
    fn rejects_non_finite_positions() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        assert_eq!(
            pet.move_to(Position {
                x: f64::NAN,
                y: 1.0
            }),
            Err(PetError::InvalidPosition)
        );
    }

    #[test]
    fn maps_actions_to_semantic_state() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.apply_action(PetAction::Work);
        assert_eq!(pet.state, PetState::Working);
        assert_eq!(pet.emotion, Emotion::Focused);
    }

    #[test]
    fn drag_preempts_other_states_and_drop_recovers_to_idle() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.apply_action(PetAction::Sleep);
        pet.begin_drag().expect("first drag");
        assert_eq!(pet.state, PetState::Dragged);
        assert_eq!(pet.emotion, Emotion::Surprised);
        assert_eq!(pet.begin_drag(), Err(PetError::InvalidTransition));
        assert_eq!(pet.interact(), Err(PetError::InvalidTransition));
        pet.drop_at(Position { x: 42.0, y: -7.0 })
            .expect("valid drop");
        assert_eq!(pet.state, PetState::Idle);
        assert_eq!(pet.emotion, Emotion::Neutral);
        assert_eq!(pet.position, Position { x: 42.0, y: -7.0 });
    }

    #[test]
    fn drop_requires_an_active_drag() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        assert_eq!(
            pet.drop_at(Position { x: 1.0, y: 2.0 }),
            Err(PetError::InvalidTransition)
        );
    }

    #[test]
    fn interaction_finish_does_not_override_a_newer_state() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.interact().expect("interaction");
        pet.apply_action(PetAction::Work);
        assert_eq!(pet.finish_interaction(), Err(PetError::InvalidTransition));
        assert_eq!(pet.state, PetState::Working);
        assert_eq!(pet.emotion, Emotion::Focused);
    }

    #[test]
    fn transient_state_recovers_to_idle_after_restart() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.begin_drag().expect("drag");
        assert!(pet.recover_transient_state());
        assert_eq!(pet.state, PetState::Idle);
        assert_eq!(pet.emotion, Emotion::Neutral);
        assert!(!pet.recover_transient_state());
    }
}
