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
}

impl Pet {
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
        self.state = PetState::Idle;
        self.emotion = Emotion::Neutral;
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
        self.state = PetState::Idle;
        self.emotion = Emotion::Neutral;
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
        (self.state, self.emotion) = match action {
            PetAction::Idle => (PetState::Idle, Emotion::Neutral),
            PetAction::Walk => (PetState::Walking, Emotion::Happy),
            PetAction::Sleep => (PetState::Sleeping, Emotion::Sleepy),
            PetAction::Work => (PetState::Working, Emotion::Focused),
            PetAction::Celebrate => (PetState::Interacting, Emotion::Happy),
        };
    }

    pub fn set_energy(&mut self, energy: i16) {
        self.energy = u8::try_from(energy.clamp(0, 100)).unwrap_or_default();
    }

    pub fn recover_transient_state(&mut self) -> bool {
        if matches!(
            self.state,
            PetState::Dragged | PetState::Interacting | PetState::Recovering
        ) {
            self.state = PetState::Idle;
            self.emotion = Emotion::Neutral;
            return true;
        }
        false
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
    #[error("pet state transition is not allowed")]
    InvalidTransition,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clamps_vitals_to_domain_range() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.set_energy(-20);
        assert_eq!(pet.energy, 0);
        pet.set_energy(500);
        assert_eq!(pet.energy, 100);
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
