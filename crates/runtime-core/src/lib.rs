//! Pure domain types and policies for `Nimora`.

mod command;
mod event;
mod pet;
mod profile;
mod safety;

pub use command::{Command, CommandError, CommandId, CommandRisk, CommandStatus};
pub use event::{Event, EventError, EventId, EventSource};
pub use pet::{
    Emotion, Pet, PetAction, PetAutonomyDecision, PetAutonomyPolicy, PetAutonomyState, PetError,
    PetId, PetIntent, PetState, PointerButton, Position,
};
pub use profile::{Profile, ProfileError, ProfileId, ProfileMode, ProfilePolicy};
pub use safety::{RuntimeMode, SafeModeReason, SafetySnapshot};
