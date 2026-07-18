//! Application use cases and persistence ports for the `Nimora` runtime.

use nimora_runtime_core::{
    Command, CommandError, CommandRisk, Event, EventError, EventSource, Pet, PetAction,
    PetAutonomyDecision, PetAutonomyPolicy, PetCareAction, PetError, PointerButton, Position,
    Profile, ProfileError, ProfileId, ProfilePolicy, RuntimeMode, SafeModeReason, SafetySnapshot,
};
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, HashSet, VecDeque},
    sync::{Arc, Mutex},
};
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

    /// Atomically stores state and its resulting event in a durable outbox.
    ///
    /// # Errors
    ///
    /// Returns a storage error without committing either record.
    fn save_with_event(&self, pet: &Pet, event: &Event) -> Result<(), RepositoryError>;
}

pub trait ProfileRepository: Send + Sync + std::fmt::Debug {
    /// Loads the profile collection, or `None` on first launch.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the snapshot cannot be read or validated.
    fn load(&self) -> Result<Option<ProfileSnapshot>, RepositoryError>;

    /// Atomically stores the complete profile collection.
    ///
    /// # Errors
    ///
    /// Returns a storage error when the snapshot cannot be committed.
    fn save(&self, snapshot: &ProfileSnapshot) -> Result<(), RepositoryError>;

    /// Atomically stores state and its resulting event in a durable outbox.
    ///
    /// # Errors
    ///
    /// Returns a storage error without committing either record.
    fn save_with_event(
        &self,
        snapshot: &ProfileSnapshot,
        event: &Event,
    ) -> Result<(), RepositoryError>;
}

#[derive(Debug, Error, Clone, PartialEq, Eq)]
#[error("runtime repository error: {message}")]
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
    events: RuntimeEventBus,
}

const EVENT_BUFFER_CAPACITY: usize = 256;
const MAX_EVENT_SUBSCRIPTION_CAPACITY: usize = 256;

#[derive(Debug, Clone)]
pub struct RuntimeEventBus {
    state: Arc<Mutex<RuntimeEventBusState>>,
}

#[derive(Debug)]
struct RuntimeEventBusState {
    events: VecDeque<Event>,
    subscriptions: HashMap<u64, EventSubscriptionState>,
    next_subscription_id: u64,
}

#[derive(Debug)]
struct EventSubscriptionState {
    event_types: HashSet<String>,
    events: VecDeque<Event>,
    capacity: usize,
    dropped: u64,
}

#[derive(Debug)]
pub struct RuntimeEventSubscription {
    id: u64,
    bus: Arc<Mutex<RuntimeEventBusState>>,
}

#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeEventBatch {
    pub events: Vec<Event>,
    pub dropped: u64,
}

impl Default for RuntimeEventBus {
    fn default() -> Self {
        Self {
            state: Arc::new(Mutex::new(RuntimeEventBusState {
                events: VecDeque::with_capacity(EVENT_BUFFER_CAPACITY),
                subscriptions: HashMap::new(),
                next_subscription_id: 1,
            })),
        }
    }
}

impl RuntimeEventBus {
    /// Removes and returns all currently buffered runtime events in order.
    ///
    /// # Errors
    ///
    /// Returns an error if another thread panicked while holding the buffer.
    pub fn drain(&self) -> Result<Vec<Event>, RuntimeError> {
        Ok(self
            .state
            .lock()
            .map_err(|_| RuntimeError::StatePoisoned)?
            .events
            .drain(..)
            .collect())
    }

    /// Creates an independent bounded subscription for trusted runtime events.
    ///
    /// # Errors
    ///
    /// Returns an error for an empty filter, an invalid capacity, identifier
    /// exhaustion, or a poisoned runtime state.
    pub fn subscribe(
        &self,
        event_types: impl IntoIterator<Item = String>,
        capacity: usize,
    ) -> Result<RuntimeEventSubscription, RuntimeError> {
        let event_types = event_types.into_iter().collect::<HashSet<_>>();
        if event_types.is_empty() {
            return Err(RuntimeError::EmptyEventSubscription);
        }
        if capacity == 0 || capacity > MAX_EVENT_SUBSCRIPTION_CAPACITY {
            return Err(RuntimeError::InvalidEventSubscriptionCapacity);
        }
        let mut state = self.lock()?;
        let id = state.next_subscription_id;
        state.next_subscription_id = state
            .next_subscription_id
            .checked_add(1)
            .ok_or(RuntimeError::EventSubscriptionExhausted)?;
        state.subscriptions.insert(
            id,
            EventSubscriptionState {
                event_types,
                events: VecDeque::with_capacity(capacity),
                capacity,
                dropped: 0,
            },
        );
        Ok(RuntimeEventSubscription {
            id,
            bus: Arc::clone(&self.state),
        })
    }

    /// Publishes one event using the shared bounded runtime buffer.
    ///
    /// # Errors
    ///
    /// Returns an error if another thread panicked while holding the buffer.
    pub fn publish(&self, event: Event) -> Result<(), RuntimeError> {
        let mut state = self.lock()?;
        Self::publish_locked(&mut state, event);
        Ok(())
    }

    fn publish_locked(state: &mut RuntimeEventBusState, event: Event) {
        if state.events.len() == EVENT_BUFFER_CAPACITY {
            state.events.pop_front();
        }
        for subscription in state.subscriptions.values_mut() {
            if !subscription.event_types.contains(&event.event_type) {
                continue;
            }
            if subscription.events.len() == subscription.capacity {
                subscription.events.pop_front();
                subscription.dropped = subscription.dropped.saturating_add(1);
            }
            subscription.events.push_back(event.clone());
        }
        state.events.push_back(event);
    }

    fn lock(&self) -> Result<std::sync::MutexGuard<'_, RuntimeEventBusState>, RuntimeError> {
        self.state.lock().map_err(|_| RuntimeError::StatePoisoned)
    }
}

impl RuntimeEventSubscription {
    /// Drains this subscription without consuming the bus or other subscribers.
    ///
    /// # Errors
    ///
    /// Returns an error after cancellation or when the runtime state is poisoned.
    pub fn drain(&self) -> Result<RuntimeEventBatch, RuntimeError> {
        let mut state = self.bus.lock().map_err(|_| RuntimeError::StatePoisoned)?;
        let subscription = state
            .subscriptions
            .get_mut(&self.id)
            .ok_or(RuntimeError::EventSubscriptionCancelled)?;
        let events = subscription.events.drain(..).collect();
        let dropped = std::mem::take(&mut subscription.dropped);
        Ok(RuntimeEventBatch { events, dropped })
    }

    /// Removes at most one oldest event while preserving the remaining queue.
    ///
    /// # Errors
    ///
    /// Returns an error after cancellation or when the runtime state is poisoned.
    pub fn pop(&self) -> Result<RuntimeEventBatch, RuntimeError> {
        let mut state = self.bus.lock().map_err(|_| RuntimeError::StatePoisoned)?;
        let subscription = state
            .subscriptions
            .get_mut(&self.id)
            .ok_or(RuntimeError::EventSubscriptionCancelled)?;
        let events = subscription.events.pop_front().into_iter().collect();
        let dropped = std::mem::take(&mut subscription.dropped);
        Ok(RuntimeEventBatch { events, dropped })
    }

    /// Cancels this subscription and releases its buffered events.
    ///
    /// # Errors
    ///
    /// Returns an error when the runtime state is poisoned.
    pub fn cancel(&self) -> Result<(), RuntimeError> {
        self.bus
            .lock()
            .map_err(|_| RuntimeError::StatePoisoned)?
            .subscriptions
            .remove(&self.id);
        Ok(())
    }
}

impl Drop for RuntimeEventSubscription {
    fn drop(&mut self) {
        if let Ok(mut state) = self.bus.lock() {
            state.subscriptions.remove(&self.id);
        }
    }
}

#[derive(Debug)]
pub struct SafetyService {
    snapshot: Mutex<SafetySnapshot>,
    events: RuntimeEventBus,
}

impl SafetyService {
    #[must_use]
    pub const fn new(events: RuntimeEventBus) -> Self {
        Self {
            snapshot: Mutex::new(SafetySnapshot::normal()),
            events,
        }
    }

    /// Returns the current process-local operational safety state.
    ///
    /// # Errors
    ///
    /// Returns an error if another thread panicked while holding safety state.
    pub fn snapshot(&self) -> Result<SafetySnapshot, SafetyServiceError> {
        Ok(*self
            .snapshot
            .lock()
            .map_err(|_| SafetyServiceError::StatePoisoned)?)
    }

    /// Enters safe mode and publishes a correlated runtime event.
    ///
    /// # Errors
    ///
    /// Returns an error when already in safe mode or when command/event state
    /// cannot be created or published.
    pub fn enter(&self, reason: SafeModeReason) -> Result<Command, SafetyServiceError> {
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| SafetyServiceError::StatePoisoned)?;
        if current.mode == RuntimeMode::Safe {
            return Err(SafetyServiceError::AlreadySafe);
        }
        let command = Command::new(
            "runtime.safety.enter",
            serde_json::json!({ "reason": reason }),
            CommandRisk::Safe,
        )?;
        let event = Event::with_trace_id(
            "runtime.safety.entered",
            EventSource::Core,
            command.trace_id,
            serde_json::json!({ "reason": reason }),
        )?;
        self.events.publish(event)?;
        *current = SafetySnapshot::safe(reason);
        Ok(command)
    }

    /// Returns to normal mode after an explicit user action.
    ///
    /// # Errors
    ///
    /// Returns an error when already normal or when command/event state cannot
    /// be created or published.
    pub fn exit(&self) -> Result<Command, SafetyServiceError> {
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| SafetyServiceError::StatePoisoned)?;
        if current.mode == RuntimeMode::Normal {
            return Err(SafetyServiceError::AlreadyNormal);
        }
        let previous_reason = current.reason;
        let command = Command::new(
            "runtime.safety.exit",
            serde_json::json!({ "previousReason": previous_reason }),
            CommandRisk::Low,
        )?;
        let event = Event::with_trace_id(
            "runtime.safety.exited",
            EventSource::Core,
            command.trace_id,
            serde_json::json!({ "previousReason": previous_reason }),
        )?;
        self.events.publish(event)?;
        *current = SafetySnapshot::normal();
        Ok(command)
    }
}

#[derive(Debug, Error)]
pub enum SafetyServiceError {
    #[error("safety state is unavailable")]
    StatePoisoned,
    #[error("runtime is already in safe mode")]
    AlreadySafe,
    #[error("runtime is already in normal mode")]
    AlreadyNormal,
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Event(#[from] EventError),
    #[error(transparent)]
    Runtime(#[from] RuntimeError),
}

impl<R: PetRepository> RuntimeService<R> {
    /// Restores persisted state or creates and persists the first pet.
    ///
    /// # Errors
    ///
    /// Returns a domain or repository error when initialization cannot finish
    /// without a valid durable snapshot.
    pub fn initialize(repository: R, default_name: &str) -> Result<Self, RuntimeError> {
        Self::initialize_with_event_bus(repository, default_name, RuntimeEventBus::default())
    }

    /// Restores state while publishing future events to a shared runtime bus.
    ///
    /// # Errors
    ///
    /// Returns a domain or repository error when initialization cannot produce
    /// a valid durable pet snapshot.
    pub fn initialize_with_event_bus(
        repository: R,
        default_name: &str,
        events: RuntimeEventBus,
    ) -> Result<Self, RuntimeError> {
        let pet = if let Some(mut pet) = repository.load()? {
            if pet.recover_transient_state() {
                repository.save(&pet)?;
            }
            pet
        } else {
            let pet = Pet::new(default_name)?;
            repository.save(&pet)?;
            pet
        };
        Ok(Self {
            repository,
            pet: Mutex::new(pet),
            events,
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

    /// Advances the deterministic offline autonomy state machine once.
    ///
    /// # Errors
    ///
    /// Returns an error when the resulting transition cannot be persisted atomically.
    pub fn tick_autonomy(
        &self,
        policy: PetAutonomyPolicy,
        now_ms: u64,
    ) -> Result<Option<Command>, RuntimeError> {
        let decision = self.snapshot()?.autonomy_decision(policy, now_ms);
        if decision == PetAutonomyDecision::Noop {
            return Ok(None);
        }
        self.update(
            |pet| {
                pet.apply_autonomy_decision(decision, policy, now_ms);
                Ok(())
            },
            || {
                Command::new(
                    "pet.autonomy.tick",
                    serde_json::json!({ "nowMs": now_ms }),
                    CommandRisk::Safe,
                )
            },
            "pet.autonomy.transitioned",
            |before, after| {
                serde_json::json!({
                    "decision": format!("{decision:?}"),
                    "beforeState": before.state,
                    "afterState": after.state,
                    "beforeEnergy": before.energy,
                    "afterEnergy": after.energy,
                    "intent": after.autonomy.active_intent,
                    "nextDueMs": after.autonomy.next_due_ms,
                })
            },
        )
        .map(Some)
    }

    /// Advances durable, provider-independent pet vitals when the interval is due.
    ///
    /// # Errors
    ///
    /// Returns an error when state access, command creation, or atomic persistence fails.
    pub fn tick_vitals(
        &self,
        policy: nimora_runtime_core::PetVitalsPolicy,
        now_ms: u64,
        interval_ms: u64,
        max_intervals: u64,
    ) -> Result<Option<Command>, RuntimeError> {
        if !self.snapshot()?.vitals_update_due(now_ms, interval_ms) {
            return Ok(None);
        }
        self.update(
            |pet| {
                pet.update_vitals(policy, now_ms, interval_ms, max_intervals);
                Ok(())
            },
            || {
                Command::new(
                    "pet.vitals.tick",
                    serde_json::json!({ "nowMs": now_ms, "policy": format!("{policy:?}") }),
                    CommandRisk::Safe,
                )
            },
            "pet.vitals.changed",
            |before, after| {
                serde_json::json!({
                    "before": { "energy": before.energy, "mood": before.mood, "satiety": before.satiety, "cleanliness": before.cleanliness },
                    "after": { "energy": after.energy, "mood": after.mood, "satiety": after.satiety, "cleanliness": after.cleanliness },
                    "affinity": after.affinity,
                })
            },
        )
        .map(Some)
    }

    /// Applies a direct care action and atomically publishes its vital changes.
    ///
    /// # Errors
    ///
    /// Returns an error when care is invalid, cooling down, or cannot be persisted.
    pub fn care_pet(
        &self,
        action: PetCareAction,
        now_ms: u64,
        cooldown_ms: u64,
    ) -> Result<Command, RuntimeError> {
        self.update(
            |pet| pet.care(action, now_ms, cooldown_ms),
            || {
                Command::new(
                    "pet.care.perform",
                    serde_json::json!({ "action": action }),
                    CommandRisk::Safe,
                )
            },
            "pet.care.performed",
            |before, after| {
                serde_json::json!({
                    "action": action,
                    "before": {
                        "energy": before.energy,
                        "mood": before.mood,
                        "satiety": before.satiety,
                        "cleanliness": before.cleanliness,
                        "affinity": before.affinity,
                        "bondPoints": before.effective_bond_points(),
                        "relationshipLevel": before.relationship_level(),
                    },
                    "after": {
                        "energy": after.energy,
                        "mood": after.mood,
                        "satiety": after.satiety,
                        "cleanliness": after.cleanliness,
                        "affinity": after.affinity,
                        "bondPoints": after.effective_bond_points(),
                        "relationshipLevel": after.relationship_level(),
                    },
                })
            },
        )
    }

    /// Records a semantic pointer interaction with the pet.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid coordinates, forbidden transitions, or a
    /// failed durable update.
    pub fn click_pet(
        &self,
        position: Position,
        button: PointerButton,
    ) -> Result<Command, RuntimeError> {
        self.update(
            |pet| {
                position.validate()?;
                pet.interact()
            },
            || {
                Command::new(
                    "pet.interaction.click",
                    serde_json::json!({ "position": position, "button": button }),
                    CommandRisk::Safe,
                )
            },
            "pet.interaction.clicked",
            |before, after| {
                serde_json::json!({
                    "position": position,
                    "button": button,
                    "before": {
                        "state": before.state,
                        "emotion": before.emotion,
                        "mood": before.mood,
                        "affinity": before.affinity,
                        "bondPoints": before.effective_bond_points(),
                        "relationshipLevel": before.relationship_level(),
                    },
                    "after": {
                        "state": after.state,
                        "emotion": after.emotion,
                        "mood": after.mood,
                        "affinity": after.affinity,
                        "bondPoints": after.effective_bond_points(),
                        "relationshipLevel": after.relationship_level(),
                    },
                })
            },
        )
    }

    /// Records a deliberate host-recognized petting gesture.
    ///
    /// # Errors
    ///
    /// Returns an error for forbidden transitions or a failed durable update.
    pub fn stroke_pet(
        &self,
        distance_px: f64,
        duration_ms: u64,
        reversals: u8,
    ) -> Result<Command, RuntimeError> {
        self.update(
            Pet::stroke,
            || {
                Command::new(
                    "pet.interaction.stroke",
                    serde_json::json!({
                        "distancePx": distance_px,
                        "durationMs": duration_ms,
                        "reversals": reversals,
                    }),
                    CommandRisk::Safe,
                )
            },
            "pet.interaction.stroked",
            |before, after| {
                serde_json::json!({
                    "gesture": {
                        "distancePx": distance_px,
                        "durationMs": duration_ms,
                        "reversals": reversals,
                    },
                    "before": {
                        "mood": before.mood,
                        "affinity": before.affinity,
                        "bondPoints": before.effective_bond_points(),
                        "relationshipLevel": before.relationship_level(),
                    },
                    "after": {
                        "mood": after.mood,
                        "affinity": after.affinity,
                        "bondPoints": after.effective_bond_points(),
                        "relationshipLevel": after.relationship_level(),
                    },
                })
            },
        )
    }

    /// Returns an interaction animation to idle without overriding newer states.
    ///
    /// # Errors
    ///
    /// Returns an error when the pet is no longer interacting or persistence fails.
    pub fn finish_interaction(&self) -> Result<Command, RuntimeError> {
        self.update(
            Pet::finish_interaction,
            || {
                Command::new(
                    "pet.interaction.finish",
                    serde_json::json!({}),
                    CommandRisk::Safe,
                )
            },
            "pet.interaction.finished",
            |before, after| {
                serde_json::json!({
                    "beforeState": before.state,
                    "afterState": after.state,
                })
            },
        )
    }

    /// Starts the highest-priority drag state.
    ///
    /// # Errors
    ///
    /// Returns an error when command creation or persistence fails.
    pub fn begin_drag(&self) -> Result<Command, RuntimeError> {
        self.update(
            Pet::begin_drag,
            || {
                Command::new(
                    "pet.window.drag.begin",
                    serde_json::json!({}),
                    CommandRisk::Safe,
                )
            },
            "pet.window.drag.started",
            |before, after| {
                serde_json::json!({
                    "from": before.position,
                    "beforeState": before.state,
                    "afterState": after.state,
                })
            },
        )
    }

    /// Drops a dragged pet at its final desktop position.
    ///
    /// # Errors
    ///
    /// Returns an error when no drag is active, the position is invalid, or
    /// the durable update fails.
    pub fn drop_pet(&self, position: Position) -> Result<Command, RuntimeError> {
        self.update(
            |pet| pet.drop_at(position),
            || {
                Command::new(
                    "pet.window.drag.end",
                    serde_json::json!({ "position": position }),
                    CommandRisk::Safe,
                )
            },
            "pet.window.dragged",
            |before, after| {
                serde_json::json!({
                    "from": before.position,
                    "to": after.position,
                    "beforeState": before.state,
                    "afterState": after.state,
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
        self.events.drain()
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
        let mut events = self.events.lock()?;
        self.repository.save_with_event(&candidate, &event)?;
        *current = candidate;
        RuntimeEventBus::publish_locked(&mut events, event);
        Ok(command)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileSnapshot {
    pub schema_version: u32,
    pub active_profile_id: ProfileId,
    pub profiles: Vec<Profile>,
}

impl ProfileSnapshot {
    pub const SCHEMA_VERSION: u32 = 1;

    /// Validates the version, profile identities, active reference, and domain values.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted profile state violates any collection or
    /// domain invariant.
    pub fn validate(&self) -> Result<(), ProfileServiceError> {
        if self.schema_version != Self::SCHEMA_VERSION {
            return Err(ProfileServiceError::UnsupportedSnapshotVersion(
                self.schema_version,
            ));
        }
        if self.profiles.is_empty() {
            return Err(ProfileServiceError::EmptyProfiles);
        }
        for (index, profile) in self.profiles.iter().enumerate() {
            profile.validate()?;
            if self.profiles[..index]
                .iter()
                .any(|candidate| candidate.id == profile.id)
            {
                return Err(ProfileServiceError::DuplicateProfileId);
            }
        }
        if !self
            .profiles
            .iter()
            .any(|profile| profile.id == self.active_profile_id)
        {
            return Err(ProfileServiceError::ActiveProfileMissing);
        }
        Ok(())
    }
}

#[derive(Debug)]
pub struct ProfileService<R> {
    repository: R,
    snapshot: Mutex<ProfileSnapshot>,
    events: RuntimeEventBus,
}

impl<R: ProfileRepository> ProfileService<R> {
    /// Restores profiles or creates the first local default profile.
    ///
    /// # Errors
    ///
    /// Returns an error when loading, validation, default creation, or the
    /// first durable save fails.
    pub fn initialize(repository: R, events: RuntimeEventBus) -> Result<Self, ProfileServiceError> {
        let snapshot = if let Some(snapshot) = repository.load()? {
            snapshot.validate()?;
            snapshot
        } else {
            let profile = Profile::new("Default", ProfilePolicy::standard())?;
            let snapshot = ProfileSnapshot {
                schema_version: ProfileSnapshot::SCHEMA_VERSION,
                active_profile_id: profile.id,
                profiles: vec![profile],
            };
            repository.save(&snapshot)?;
            snapshot
        };
        Ok(Self {
            repository,
            snapshot: Mutex::new(snapshot),
            events,
        })
    }

    /// Returns a consistent copy of the profile collection.
    ///
    /// # Errors
    ///
    /// Returns an error if another thread panicked while holding profile state.
    pub fn snapshot(&self) -> Result<ProfileSnapshot, ProfileServiceError> {
        Ok(self
            .snapshot
            .lock()
            .map_err(|_| ProfileServiceError::StatePoisoned)?
            .clone())
    }

    /// Creates and durably stores a new composable profile.
    ///
    /// # Errors
    ///
    /// Returns an error when domain validation, event creation, locking, or
    /// persistence fails. No state or event is published before persistence.
    pub fn create_profile(
        &self,
        name: impl Into<String>,
        policy: ProfilePolicy,
    ) -> Result<Command, ProfileServiceError> {
        let profile = Profile::new(name, policy)?;
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| ProfileServiceError::StatePoisoned)?;
        let mut candidate = current.clone();
        candidate.profiles.push(profile.clone());
        candidate.validate()?;
        let command = Command::new(
            "profile.collection.create",
            serde_json::json!({ "profile": profile }),
            CommandRisk::Safe,
        )?;
        let event = Event::with_trace_id(
            "profile.collection.created",
            EventSource::Core,
            command.trace_id,
            serde_json::json!({ "profile": profile }),
        )?;
        let mut events = self.events.lock().map_err(ProfileServiceError::Runtime)?;
        self.repository.save_with_event(&candidate, &event)?;
        *current = candidate;
        RuntimeEventBus::publish_locked(&mut events, event);
        Ok(command)
    }

    /// Activates a profile only after the complete snapshot is durably stored.
    ///
    /// # Errors
    ///
    /// Returns an error for an unknown profile or when validation, event
    /// creation, locking, or persistence fails. No state or event is published
    /// before persistence succeeds.
    pub fn switch_active(&self, profile_id: ProfileId) -> Result<Command, ProfileServiceError> {
        let mut current = self
            .snapshot
            .lock()
            .map_err(|_| ProfileServiceError::StatePoisoned)?;
        if !current
            .profiles
            .iter()
            .any(|profile| profile.id == profile_id)
        {
            return Err(ProfileServiceError::ProfileNotFound);
        }
        if current.active_profile_id == profile_id {
            return Err(ProfileServiceError::ProfileAlreadyActive);
        }
        let mut candidate = current.clone();
        let previous_profile_id = candidate.active_profile_id;
        candidate.active_profile_id = profile_id;
        candidate.validate()?;
        let command = Command::new(
            "profile.active.switch",
            serde_json::json!({ "profileId": profile_id }),
            CommandRisk::Safe,
        )?;
        let event = Event::with_trace_id(
            "profile.active.changed",
            EventSource::Core,
            command.trace_id,
            serde_json::json!({
                "beforeProfileId": previous_profile_id,
                "afterProfileId": profile_id,
            }),
        )?;
        let mut events = self.events.lock().map_err(ProfileServiceError::Runtime)?;
        self.repository.save_with_event(&candidate, &event)?;
        *current = candidate;
        RuntimeEventBus::publish_locked(&mut events, event);
        Ok(command)
    }
}

#[derive(Debug, Error)]
pub enum ProfileServiceError {
    #[error("profile state is unavailable")]
    StatePoisoned,
    #[error("profile collection must not be empty")]
    EmptyProfiles,
    #[error("active profile does not exist")]
    ActiveProfileMissing,
    #[error("profile identifiers must be unique")]
    DuplicateProfileId,
    #[error("profile was not found")]
    ProfileNotFound,
    #[error("profile is already active")]
    ProfileAlreadyActive,
    #[error("profile snapshot version {0} is unsupported")]
    UnsupportedSnapshotVersion(u32),
    #[error(transparent)]
    Repository(#[from] RepositoryError),
    #[error(transparent)]
    Profile(#[from] ProfileError),
    #[error(transparent)]
    Command(#[from] CommandError),
    #[error(transparent)]
    Event(#[from] EventError),
    #[error(transparent)]
    Runtime(RuntimeError),
}

#[derive(Debug, Error)]
pub enum RuntimeError {
    #[error("runtime state is unavailable")]
    StatePoisoned,
    #[error("event subscriptions require at least one event type")]
    EmptyEventSubscription,
    #[error("event subscription capacity must be between 1 and 256")]
    InvalidEventSubscriptionCapacity,
    #[error("event subscription identifiers are exhausted")]
    EventSubscriptionExhausted,
    #[error("event subscription was cancelled")]
    EventSubscriptionCancelled,
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
    use nimora_runtime_core::PetState;
    use std::sync::atomic::{AtomicBool, Ordering};

    #[derive(Debug, Default)]
    struct MemoryRepository {
        pet: Mutex<Option<Pet>>,
        outbox: Mutex<Vec<Event>>,
        fail_save: AtomicBool,
    }

    #[derive(Debug, Default)]
    struct MemoryProfileRepository {
        snapshot: Mutex<Option<ProfileSnapshot>>,
        outbox: Mutex<Vec<Event>>,
        fail_save: AtomicBool,
    }

    impl ProfileRepository for MemoryProfileRepository {
        fn load(&self) -> Result<Option<ProfileSnapshot>, RepositoryError> {
            Ok(self.snapshot.lock().expect("test lock").clone())
        }

        fn save(&self, snapshot: &ProfileSnapshot) -> Result<(), RepositoryError> {
            if self.fail_save.load(Ordering::Relaxed) {
                return Err(RepositoryError::new("injected failure"));
            }
            *self.snapshot.lock().expect("test lock") = Some(snapshot.clone());
            Ok(())
        }

        fn save_with_event(
            &self,
            snapshot: &ProfileSnapshot,
            event: &Event,
        ) -> Result<(), RepositoryError> {
            if self.fail_save.load(Ordering::Relaxed) {
                return Err(RepositoryError::new("injected failure"));
            }
            *self.snapshot.lock().expect("test lock") = Some(snapshot.clone());
            self.outbox.lock().expect("test lock").push(event.clone());
            Ok(())
        }
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

        fn save_with_event(&self, pet: &Pet, event: &Event) -> Result<(), RepositoryError> {
            if self.fail_save.load(Ordering::Relaxed) {
                return Err(RepositoryError::new("injected failure"));
            }
            *self.pet.lock().expect("test lock") = Some(pet.clone());
            self.outbox.lock().expect("test lock").push(event.clone());
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
    fn initialization_recovers_a_persisted_transient_state() {
        let mut pet = Pet::new("Aster").expect("pet");
        pet.begin_drag().expect("drag");
        let repository = MemoryRepository {
            pet: Mutex::new(Some(pet)),
            outbox: Mutex::new(Vec::new()),
            fail_save: AtomicBool::new(false),
        };
        let service = RuntimeService::initialize(repository, "Aster").expect("runtime");
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(snapshot.state, PetState::Idle);
        assert_eq!(snapshot.emotion, nimora_runtime_core::Emotion::Neutral);
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

    #[test]
    fn vitals_tick_is_due_bounded_and_publishes_after_persistence() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        let initialized = service
            .tick_vitals(nimora_runtime_core::PetVitalsPolicy::Full, 1_000, 100, 6)
            .expect("baseline")
            .expect("baseline command");
        assert!(
            service
                .tick_vitals(nimora_runtime_core::PetVitalsPolicy::Full, 1_050, 100, 6)
                .expect("not due")
                .is_none()
        );
        let updated = service
            .tick_vitals(nimora_runtime_core::PetVitalsPolicy::Full, 11_000, 100, 6)
            .expect("offline catchup")
            .expect("update command");
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(snapshot.energy, 94);
        assert_eq!(snapshot.mood, 68);
        assert_eq!((snapshot.satiety, snapshot.cleanliness), (97, 99));
        let events = service.drain_events().expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].trace_id, initialized.trace_id);
        assert_eq!(events[1].trace_id, updated.trace_id);
        assert_eq!(events[1].event_type, "pet.vitals.changed");
        assert_eq!(events[1].data["after"]["satiety"], 97);
        assert_eq!(events[1].data["after"]["cleanliness"], 99);
    }

    #[test]
    fn sleeping_vitals_recovery_is_persisted_with_standard_event() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        service
            .tick_vitals(nimora_runtime_core::PetVitalsPolicy::Full, 1_000, 100, 6)
            .expect("baseline")
            .expect("command");
        service.play_action(PetAction::Sleep).expect("manual sleep");
        service.snapshot().expect("snapshot");
        let command = service
            .tick_vitals(nimora_runtime_core::PetVitalsPolicy::Full, 11_000, 100, 6)
            .expect("sleep recovery")
            .expect("command");
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(snapshot.energy, 100);
        let event = service
            .drain_events()
            .expect("events")
            .into_iter()
            .find(|event| event.trace_id == command.trace_id)
            .expect("vitals event");
        assert_eq!(event.event_type, "pet.vitals.changed");
        assert_eq!(event.data["before"]["energy"], 100);
        assert_eq!(event.data["after"]["energy"], 100);
        assert_eq!(event.data["after"]["satiety"], 97);
    }

    #[test]
    fn failed_vitals_persistence_does_not_publish_or_mutate_memory() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        service.repository.fail_save.store(true, Ordering::Relaxed);
        assert!(
            service
                .tick_vitals(nimora_runtime_core::PetVitalsPolicy::Full, 1_000, 100, 6)
                .is_err()
        );
        assert_eq!(
            service.snapshot().expect("snapshot").last_vitals_update_ms,
            0
        );
        assert!(service.drain_events().expect("events").is_empty());
    }

    #[test]
    fn care_is_persisted_with_correlated_vital_event_and_cooldown() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        let command = service
            .care_pet(PetCareAction::Play, 1_000, 30_000)
            .expect("care");
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(
            (
                snapshot.energy,
                snapshot.mood,
                snapshot.satiety,
                snapshot.cleanliness,
                snapshot.affinity
            ),
            (95, 82, 97, 98, 2)
        );
        assert!(matches!(
            service.care_pet(PetCareAction::Feed, 2_000, 30_000),
            Err(RuntimeError::Pet(PetError::CareCooldown))
        ));
        let event = service
            .drain_events()
            .expect("events")
            .pop()
            .expect("event");
        assert_eq!(event.event_type, "pet.care.performed");
        assert_eq!(event.trace_id, command.trace_id);
        assert_eq!(event.data["action"], "play");
        assert_eq!(event.data["before"]["bondPoints"], 0);
        assert_eq!(event.data["after"]["bondPoints"], 2);
        assert_eq!(event.data["after"]["satiety"], 97);
        assert_eq!(event.data["after"]["cleanliness"], 98);
        assert_eq!(event.data["after"]["relationshipLevel"], 1);
    }

    #[test]
    fn failed_care_persistence_preserves_vitals_and_cooldown() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        service.repository.fail_save.store(true, Ordering::Relaxed);
        assert!(
            service
                .care_pet(PetCareAction::Feed, 1_000, 30_000)
                .is_err()
        );
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(
            (snapshot.energy, snapshot.mood, snapshot.affinity),
            (100, 70, 0)
        );
        assert_eq!(snapshot.bond_points, 0);
        assert_eq!(snapshot.last_care_ms, 0);
        assert!(service.drain_events().expect("events").is_empty());
    }

    #[test]
    fn click_publishes_standard_event_with_pointer_context() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        let command = service
            .click_pet(Position { x: 12.0, y: 24.0 }, PointerButton::Left)
            .expect("click");
        assert_eq!(
            service.snapshot().expect("snapshot").state,
            PetState::Interacting
        );
        let event = service
            .drain_events()
            .expect("events")
            .pop()
            .expect("event");
        assert_eq!(event.event_type, "pet.interaction.clicked");
        assert_eq!(event.trace_id, command.trace_id);
        assert_eq!(event.data["position"]["x"], 12.0);
        assert_eq!(event.data["button"], "left");
        assert_eq!(event.data["before"]["bondPoints"], 0);
        assert_eq!(event.data["after"]["bondPoints"], 1);
        assert_eq!(event.data["after"]["relationshipLevel"], 1);
    }

    #[test]
    fn stroke_publishes_growth_and_gesture_evidence_atomically() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        let command = service.stroke_pet(42.0, 240, 3).expect("stroke");
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(
            (snapshot.mood, snapshot.affinity, snapshot.bond_points),
            (74, 2, 2)
        );
        let event = service
            .drain_events()
            .expect("events")
            .pop()
            .expect("event");
        assert_eq!(event.event_type, "pet.interaction.stroked");
        assert_eq!(event.trace_id, command.trace_id);
        assert_eq!(event.data["gesture"]["reversals"], 3);
        assert_eq!(event.data["after"]["bondPoints"], 2);
    }

    #[test]
    fn failed_stroke_persistence_does_not_publish_or_apply_growth() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        service.repository.fail_save.store(true, Ordering::Relaxed);
        assert!(service.stroke_pet(42.0, 240, 3).is_err());
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(
            (snapshot.mood, snapshot.affinity, snapshot.bond_points),
            (70, 0, 0)
        );
        assert!(service.drain_events().expect("events").is_empty());
    }

    #[test]
    fn drag_and_drop_publish_correlated_state_transitions() {
        let service =
            RuntimeService::initialize(MemoryRepository::default(), "Aster").expect("runtime");
        let begin = service.begin_drag().expect("begin drag");
        assert_eq!(
            service.snapshot().expect("snapshot").state,
            PetState::Dragged
        );
        let drop = service
            .drop_pet(Position { x: -40.0, y: 80.0 })
            .expect("drop");
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(snapshot.state, PetState::Idle);
        assert_eq!(snapshot.position, Position { x: -40.0, y: 80.0 });
        let events = service.drain_events().expect("events");
        assert_eq!(events[0].event_type, "pet.window.drag.started");
        assert_eq!(events[0].trace_id, begin.trace_id);
        assert_eq!(events[1].event_type, "pet.window.dragged");
        assert_eq!(events[1].trace_id, drop.trace_id);
    }

    #[test]
    fn persists_the_default_profile_on_first_launch() {
        let bus = RuntimeEventBus::default();
        let service =
            ProfileService::initialize(MemoryProfileRepository::default(), bus).expect("profiles");
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(snapshot.profiles.len(), 1);
        assert_eq!(snapshot.profiles[0].name, "Default");
        assert_eq!(snapshot.active_profile_id, snapshot.profiles[0].id);
    }

    #[test]
    fn profile_switch_is_durable_and_correlated() {
        let first = Profile::new("Default", ProfilePolicy::standard()).expect("profile");
        let second = Profile::new("Focus", ProfilePolicy::standard()).expect("profile");
        let repository = MemoryProfileRepository {
            snapshot: Mutex::new(Some(ProfileSnapshot {
                schema_version: ProfileSnapshot::SCHEMA_VERSION,
                active_profile_id: first.id,
                profiles: vec![first, second.clone()],
            })),
            outbox: Mutex::new(Vec::new()),
            fail_save: AtomicBool::new(false),
        };
        let bus = RuntimeEventBus::default();
        let service = ProfileService::initialize(repository, bus.clone()).expect("profiles");
        let command = service.switch_active(second.id).expect("switch");
        assert_eq!(
            service.snapshot().expect("snapshot").active_profile_id,
            second.id
        );
        let events = bus.drain().expect("events");
        assert_eq!(events[0].event_type, "profile.active.changed");
        assert_eq!(events[0].trace_id, command.trace_id);
    }

    #[test]
    fn creates_a_valid_profile_without_activating_it() {
        let bus = RuntimeEventBus::default();
        let service = ProfileService::initialize(MemoryProfileRepository::default(), bus.clone())
            .expect("profiles");
        let active = service.snapshot().expect("snapshot").active_profile_id;
        let command = service
            .create_profile("Focus", ProfilePolicy::standard())
            .expect("create");
        let snapshot = service.snapshot().expect("snapshot");
        assert_eq!(snapshot.profiles.len(), 2);
        assert_eq!(snapshot.active_profile_id, active);
        let event = bus.drain().expect("events").pop().expect("event");
        assert_eq!(event.event_type, "profile.collection.created");
        assert_eq!(event.trace_id, command.trace_id);
    }

    #[test]
    fn failed_profile_save_does_not_publish_state_or_event() {
        let bus = RuntimeEventBus::default();
        let repository = MemoryProfileRepository::default();
        let service = ProfileService::initialize(repository, bus.clone()).expect("profiles");
        let before = service.snapshot().expect("snapshot");
        service.repository.fail_save.store(true, Ordering::Relaxed);
        let second = Profile::new("Focus", ProfilePolicy::standard()).expect("profile");
        service
            .create_profile(second.name, second.policy)
            .expect_err("save fails");
        assert_eq!(service.snapshot().expect("snapshot"), before);
        assert!(bus.drain().expect("events").is_empty());
    }

    #[test]
    fn safety_transitions_publish_correlated_events() {
        let bus = RuntimeEventBus::default();
        let service = SafetyService::new(bus.clone());
        let enter = service
            .enter(SafeModeReason::Manual)
            .expect("enter safe mode");
        assert_eq!(
            service.snapshot().expect("snapshot"),
            SafetySnapshot::safe(SafeModeReason::Manual)
        );
        let exit = service.exit().expect("exit safe mode");
        assert_eq!(
            service.snapshot().expect("snapshot"),
            SafetySnapshot::normal()
        );
        let events = bus.drain().expect("events");
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event_type, "runtime.safety.entered");
        assert_eq!(events[0].trace_id, enter.trace_id);
        assert_eq!(events[1].event_type, "runtime.safety.exited");
        assert_eq!(events[1].trace_id, exit.trace_id);
    }

    #[test]
    fn safety_rejects_duplicate_transitions_without_false_events() {
        let bus = RuntimeEventBus::default();
        let service = SafetyService::new(bus.clone());
        service
            .enter(SafeModeReason::Manual)
            .expect("enter safe mode");
        assert!(matches!(
            service.enter(SafeModeReason::CrashLoop),
            Err(SafetyServiceError::AlreadySafe)
        ));
        assert_eq!(bus.drain().expect("events").len(), 1);
        service.exit().expect("exit safe mode");
        assert!(matches!(
            service.exit(),
            Err(SafetyServiceError::AlreadyNormal)
        ));
        assert_eq!(bus.drain().expect("events").len(), 1);
    }

    #[test]
    fn event_subscriptions_are_filtered_and_do_not_consume_the_main_buffer() {
        let bus = RuntimeEventBus::default();
        let subscription = bus
            .subscribe(["pet.example.clicked".to_owned()], 4)
            .expect("subscribe");
        bus.publish(
            Event::new(
                "pet.example.clicked",
                EventSource::Core,
                serde_json::json!({"sequence": 1}),
            )
            .expect("event"),
        )
        .expect("publish");
        bus.publish(
            Event::new(
                "profile.example.changed",
                EventSource::Core,
                serde_json::json!({"sequence": 2}),
            )
            .expect("event"),
        )
        .expect("publish");
        let batch = subscription.drain().expect("subscription events");
        assert_eq!(batch.events.len(), 1);
        assert_eq!(batch.events[0].event_type, "pet.example.clicked");
        assert_eq!(batch.dropped, 0);
        assert_eq!(bus.drain().expect("main events").len(), 2);
    }

    #[test]
    fn slow_event_subscriptions_drop_oldest_events_with_accounting() {
        let bus = RuntimeEventBus::default();
        let subscription = bus
            .subscribe(["pet.example.clicked".to_owned()], 2)
            .expect("subscribe");
        for sequence in 1..=4 {
            bus.publish(
                Event::new(
                    "pet.example.clicked",
                    EventSource::Core,
                    serde_json::json!({"sequence": sequence}),
                )
                .expect("event"),
            )
            .expect("publish");
        }
        let batch = subscription.drain().expect("subscription events");
        assert_eq!(batch.dropped, 2);
        assert_eq!(batch.events.len(), 2);
        assert_eq!(batch.events[0].data["sequence"], 3);
        assert_eq!(batch.events[1].data["sequence"], 4);
        assert_eq!(subscription.drain().expect("empty batch").dropped, 0);
    }

    #[test]
    fn popping_one_subscription_event_preserves_the_remaining_queue() {
        let bus = RuntimeEventBus::default();
        let subscription = bus
            .subscribe(["pet.example.clicked".to_owned()], 4)
            .expect("subscribe");
        for sequence in 1..=2 {
            bus.publish(
                Event::new(
                    "pet.example.clicked",
                    EventSource::Core,
                    serde_json::json!({"sequence": sequence}),
                )
                .expect("event"),
            )
            .expect("publish");
        }
        let first = subscription.pop().expect("first event");
        assert_eq!(first.events[0].data["sequence"], 1);
        let second = subscription.pop().expect("second event");
        assert_eq!(second.events[0].data["sequence"], 2);
        assert!(subscription.pop().expect("empty").events.is_empty());
    }

    #[test]
    fn cancelled_event_subscriptions_release_their_queue() {
        let bus = RuntimeEventBus::default();
        let subscription = bus
            .subscribe(["pet.example.clicked".to_owned()], 2)
            .expect("subscribe");
        subscription.cancel().expect("cancel");
        assert!(matches!(
            subscription.drain(),
            Err(RuntimeError::EventSubscriptionCancelled)
        ));
    }
}
