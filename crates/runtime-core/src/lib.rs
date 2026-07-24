//! Pure domain types and policies for `Nimora`.

mod behavior;
mod command;
mod event;
mod pet;
mod profile;
mod safety;

pub use behavior::{
    connector_sensory_directive,
    lifeform_speech_for,
    select_autonomous_intent,
    skill_worker_busy_directive,
    skill_worker_done_directive,
    AttentionFocus,
    AutonomousBehaviorIntent,
    BehaviorError,
    ConnectorSenseKind,
    CrowdingLevel,
    DesktopBehaviorHints,
    MoodAxis,
    MoodDelta,
    PersonalityProfile,
    PetDirectiveAction,
    PetVitalsSnapshot,
    StructuredPetDirective,
    ANIMATION_TOKEN_WHITELIST,
    MAX_ANIMATION_TOKEN_LEN,
    MAX_DIRECTIVE_SPEECH_CHARS,
    MAX_MOOD_DELTA_ABS,
    PERSONALITY_TRAIT_MAX,
    AgentCompanionPhase,
    AutoModePetEvent,
    UserCodePhase,
    AutomationPhase,
    GrantPetEvent,
    agent_status_directive,
    auto_mode_directive,
    user_code_directive,
    automation_directive,
    battery_directive,
    idle_user_directive,
    meeting_sensory_directive,
    notification_sensory_directive,
    grant_directive,
};
pub use command::{Command, CommandError, CommandId, CommandRisk, CommandStatus};
pub use event::{Event, EventError, EventId, EventSource};
pub use pet::{
    Emotion, Pet, PetAction, PetAutonomyDecision, PetAutonomyPolicy, PetAutonomyState,
    PetCareAction, PetError, PetId, PetIntent, PetInventoryStack, PetItemId, PetKeepsake,
    PetRelationshipSnapshot, PetRelationshipStage, PetState, PetVitalsPolicy, PointerButton,
    Position,
};
pub use profile::{
    CareNeedsMode, Profile, ProfileError, ProfileId, ProfileMode, ProfilePolicy, QuietHours,
};
pub use safety::{RuntimeMode, SafeModeReason, SafetySnapshot};
