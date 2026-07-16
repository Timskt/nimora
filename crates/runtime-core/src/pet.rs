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

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Position {
    pub x: f64,
    pub y: f64,
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
        if !position.x.is_finite() || !position.y.is_finite() {
            return Err(PetError::InvalidPosition);
        }
        self.position = position;
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
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum PetError {
    #[error("pet name must contain 1 to 64 characters")]
    InvalidName,
    #[error("pet position must be finite")]
    InvalidPosition,
    #[error("pet vitals must be between 0 and 100")]
    InvalidVitals,
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
}
