//! Application use cases and persistence ports for the `AsterPet` runtime.

use asterpet_runtime_core::{
    Command, CommandError, CommandRisk, Event, EventError, EventSource, Pet, PetAction, PetError,
    Position,
};
use std::{collections::VecDeque, sync::Mutex};
use thiserror::Error;

pub trait PetRepository: Send + Sync + std::fmt::Debug {
    /// Loads the current pet snapshot, or `None` on first launch.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the snapshot cannot be read or validated.
    fn load(&self) -> Result<Option<Pet>, RepositoryError>;

    /// Atomically stores the current pet snapshot.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the snapshot cannot be committed.
    fn save(&self, pet: &Pet) -> Result<(), RepositoryError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("pet repository error: {message}")]
pub struct RepositoryError {
    message: String,
}

impl RepositoryError {
    #[must_use]
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }
}

#[derive(Debug)]
pub struct RuntimeService<R> {
    repository: R,
    pet: Mutex<Pet>,
    events: Mutex<VecDeque<Event>>,
}

const EVENT_BUFFER_CAPACITY: usize = 256;

impl<R: PetRepository> RuntimeService<R> {
    /// Restores persisted state or creates and persists the first pet.
    ///
    /// # Errors
    ///
    /// Returns a domain or repository error when initialization cannot finish
    /// without a valid durable snapshot.
    pub fn initialize(repository: R, default_name: &str) -> Result<Self, RuntimeError> {
        let pet = if let Some(pet) = repository.load()? {
            pet
        } else {
            let pet = Pet::new(default_name)?;
            repository.save(&pet)?;
            pet
        };
        Ok(Self {
            repository,
            pet: Mutex::new(pet),
            events: Mutex::new(VecDeque::with_capacity(EVENT_BUFFER_CAPACITY)),
        })
    }

    /// Returns a consistent copy of the current pet state.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::StatePoisoned`] after an unrecoverable panic
    /// while the state lock was held.
    pub fn snapshot(&self) -> Result<Pet, RuntimeError> {
        Ok(self
            .pet
            .lock()
            .map_err(|_| RuntimeError::StatePoisoned)?
            .clone())
    }

    /// Moves and durably stores the pet before publishing the new state.
    ///
    /// # Errors
    ///
    /// Returns an error when validation, command creation, or persistence
    /// fails. The in-memory state remains unchanged on failure.
    pub fn move_pet(&self, position: Position) -> Result<Command, RuntimeError> {
        self.update(
            |pet| pet.move_to(position),
            || {
                Command::new(
                    "pet.window.move",
                    serde_json::json!({ "x": position.x, "y": position.y }),
                    CommandRisk::Safe,
                )
            },
            "pet.position.changed",
            |before, after| {
                serde_json::json!({ "before": before.position, "after": after.position })
            },
        )
    }

    /// Applies and durably stores a semantic pet action.
    ///
    /// # Errors
    ///
    /// Returns an error when command creation or persistence fails. The
    /// in-memory state remains unchanged on failure.
    pub fn play_action(&self, action: PetAction) -> Result<Command, RuntimeError> {
        self.update(
            |pet| {
                pet.apply_action(action);
                Ok(())
            },
            || {
                Command::new(
                    "pet.animation.play",
                    serde_json::json!({ "action": action }),
                    CommandRisk::Safe,
                )
            },
            "pet.action.played",
            |before, after| {
                serde_json::json!({
                    "action": action,
                    "before": { "state": before.state, "emotion": before.emotion },
                    "after": { "state": after.state, "emotion": after.emotion },
                })
            },
        )
    }

    /// Removes and returns all currently buffered runtime events in order.
    ///
    /// # Errors
    ///
    /// Returns [`RuntimeError::StatePoisoned`] if the event buffer is unavailable.
    pub fn drain_events(&self) -> Result<Vec<Event>, RuntimeError> {
        Ok(self
            .events
            .lock()
            .map_err(|_| RuntimeError::StatePoisoned)?
            .drain(..)
            .collect())
    }

    fn update(
        &self,
        mutate: impl FnOnce(&mut Pet) -> Result<(), PetError>,
        command: impl FnOnce() -> Result<Command, CommandError>,
        event_type: &'static str,
        event_data: impl FnOnce(&Pet, &Pet) -> serde_json::Value,
    ) -> Result<Command, RuntimeError> {
        let mut current = self.pet.lock().map_err(|_| RuntimeError::StatePoisoned)?;
        let mut candidate = current.clone();
        mutate(&mut candidate)?;
        candidate.validate()?;
        let command = command()?;
        let event = Event::with_trace_id(
            event_type,
            EventSource::Core,
            command.trace_id,
            event_data(&current, &candidate),
        )?;
        let mut events = self
            .events
            .lock()
            .map_err(|_| RuntimeError::StatePoisoned)?;
        self.repository.save(&candidate)?;
        *current = candidate;
        if events.len() == EVENT_BUFFER_CAPACITY {
            events.pop_front();
        }
        events.push_back(event);
        Ok(command)
    }
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime state is unavailable")]
    StatePoisoned,
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Pet(#[from] PetError),
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Event(#[from] EventError),
}

#[cfg(test)]
mod tests {
    use super::*;
    use asterpet_runtime_core::PetState;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Debug, Default)]
    struct MemoryRepository {
        pet: Mutex<Option<Pet>>,
        fail_save: AtomicBool,
    }

    impl PetRepository for MemoryRepository {
        fn load(&self) -> Result<Option<Pet>, RepositoryError> {
            Ok(self.pet.lock().expect("test lock").clone())
        }

        fn save(&self, pet: &Pet) -> Result<(), RepositoryError> {
            if self.fail_save.load(Ordering::Relaxed) {
                return Err(RepositoryError::new("injected failure"));
            }
            *self.pet.lock().expect("test lock") = Some(pet.clone());
            Ok(())
        }
    }

    #[test]
    fn persists_default_pet_on_first_launch() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("initializes");
        assert_eq!(service.snapshot().expect("snapshot").name, "Aster");
    }

    #[test]
    fn does_not_publish_state_when_persistence_fails() {
        let repository = MemoryRepository::default();
        let service = RuntimeService::initialize(repository, "Aster").expect("initializes");
        service.repository.fail_save.store(true, Ordering::Relaxed);
        assert!(service.play_action(PetAction::Sleep).is_err());
        assert_eq!(service.snapshot().expect("snapshot").state, PetState::Idle);
        assert!(service.drain_events().expect("events").is_empty());
    }

    #[test]
    fn publishes_a_correlated_event_after_persistence() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        let command = service
            .play_action(PetAction::Sleep)
            .expect("persisted action");
        let events = service.drain_events().expect("events");
        assert_eq!(events.len(), 1);
        assert_eq!(events[0].event_type, "pet.action.played");
        assert_eq!(events[0].trace_id, command.trace_id);
    }
}
