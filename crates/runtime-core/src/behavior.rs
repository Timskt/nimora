//! Personality, attention focus, and structured behavior directives for the
//! desktop lifeform domain.
//!
//! These types let the companion remain the Subject of action: AI and host
//! surfaces emit bounded, serializable acts instead of free-form prose only.

use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Trait scale for every personality axis (0–100 inclusive).
pub const PERSONALITY_TRAIT_MAX: u8 = 100;

/// Maximum Unicode scalar values allowed in directive speech bubbles.
pub const MAX_DIRECTIVE_SPEECH_CHARS: usize = 120;

/// Maximum absolute mood delta accepted on a single directive.
pub const MAX_MOOD_DELTA_ABS: i8 = 20;

/// Maximum length for an animation token string.
pub const MAX_ANIMATION_TOKEN_LEN: usize = 64;

/// Closed whitelist of animation tokens accepted by structured directives.
pub const ANIMATION_TOKEN_WHITELIST: &[&str] = &[
    "pet.idle",
    "pet.observe",
    "pet.walk",
    "pet.play",
    "pet.perch",
    "pet.climb",
    "pet.peek",
    "pet.stretch",
    "pet.sleep",
    "pet.wake",
    "pet.work",
    "pet.celebrate",
    "pet.click",
    // First-class micro-performance tokens (Subject directive animations).
    "pet.yawn",
    "pet.dig_nose",
    "pet.count_ants",
    "pet.wave",
    "pet.look_around",
    "pet.hop",
];

const LOW_VITAL_THRESHOLD: u8 = 25;
const DIRECTIVE_SPEC_V1: &str = "nimora.pet_directive/1";

/// Stable personality traits that bias autonomous and AI-driven acts.
///
/// `energy` here is a trait (how lively the character tends to be), not the
/// time-evolving vital also named energy on [`crate::Pet`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PersonalityProfile {
    pub energy: u8,
    pub curiosity: u8,
    pub laziness: u8,
    pub pride: u8,
}

impl Default for PersonalityProfile {
    fn default() -> Self {
        Self {
            energy: 55,
            curiosity: 55,
            laziness: 35,
            pride: 40,
        }
    }
}

impl PersonalityProfile {
    /// Builds a profile with each axis clamped into `0..=100`.
    #[must_use]
    pub fn new(energy: i16, curiosity: i16, laziness: i16, pride: i16) -> Self {
        Self {
            energy: clamp_trait(energy),
            curiosity: clamp_trait(curiosity),
            laziness: clamp_trait(laziness),
            pride: clamp_trait(pride),
        }
    }

    /// Clamps every axis into `0..=100` in place.
    pub fn clamp(&mut self) {
        self.energy = self.energy.min(PERSONALITY_TRAIT_MAX);
        self.curiosity = self.curiosity.min(PERSONALITY_TRAIT_MAX);
        self.laziness = self.laziness.min(PERSONALITY_TRAIT_MAX);
        self.pride = self.pride.min(PERSONALITY_TRAIT_MAX);
    }

    /// Returns a clamped copy of this profile.
    #[must_use]
    pub fn clamped(mut self) -> Self {
        self.clamp();
        self
    }

    /// Returns `true` when every axis is within the domain range.
    #[must_use]
    pub const fn is_valid(self) -> bool {
        self.energy <= PERSONALITY_TRAIT_MAX
            && self.curiosity <= PERSONALITY_TRAIT_MAX
            && self.laziness <= PERSONALITY_TRAIT_MAX
            && self.pride <= PERSONALITY_TRAIT_MAX
    }

    /// Validates trait bounds without mutation.
    ///
    /// # Errors
    ///
    /// Returns [`BehaviorError::InvalidPersonality`] when any axis exceeds 100.
    pub const fn validate(self) -> Result<(), BehaviorError> {
        if self.is_valid() {
            Ok(())
        } else {
            Err(BehaviorError::InvalidPersonality)
        }
    }
}

/// Interpretable mood axis used by AI and renderer semantics.
///
/// This coexists with the numeric `mood` vital (`0..=100`) on the pet snapshot
/// and never replaces it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MoodAxis {
    Calm,
    Happy,
    Curious,
    Tired,
    Wounded,
    Focused,
}

impl MoodAxis {
    /// Derives a discrete mood axis from vitals without mutating them.
    #[must_use]
    pub fn from_vitals(energy: u8, mood: u8) -> Self {
        if energy <= LOW_VITAL_THRESHOLD {
            Self::Tired
        } else if mood <= LOW_VITAL_THRESHOLD {
            Self::Wounded
        } else if mood >= 75 {
            Self::Happy
        } else if energy >= 70 && mood >= 55 {
            Self::Curious
        } else if energy >= 60 {
            Self::Focused
        } else {
            Self::Calm
        }
    }
}

/// What the companion is currently attending to on the desktop scene.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AttentionFocus {
    Cursor,
    ForegroundWindow,
    NotificationArea,
    User,
    IdleScene,
    Obstacle,
}

/// Closed set of acts an AI or autonomy planner may request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetDirectiveAction {
    Wander,
    ApproachCursor,
    Rest,
    Play,
    Observe,
    Perch,
    Celebrate,
    WorkBusy,
    WorkCrash,
}

impl PetDirectiveAction {
    /// Maps a directive act onto the closest classic [`crate::PetAction`].
    #[must_use]
    pub const fn to_pet_action(self) -> crate::PetAction {
        match self {
            Self::Wander => crate::PetAction::Walk,
            Self::ApproachCursor => crate::PetAction::Walk,
            Self::Rest => crate::PetAction::Sleep,
            Self::Play => crate::PetAction::Play,
            Self::Observe => crate::PetAction::Observe,
            Self::Perch => crate::PetAction::Perch,
            Self::Celebrate => crate::PetAction::Celebrate,
            Self::WorkBusy | Self::WorkCrash => crate::PetAction::Work,
        }
    }

    /// Default animation token associated with this act.
    #[must_use]
    pub const fn default_animation(self) -> &'static str {
        match self {
            Self::Wander | Self::ApproachCursor => "pet.walk",
            Self::Rest => "pet.sleep",
            Self::Play => "pet.play",
            Self::Observe => "pet.observe",
            Self::Perch => "pet.perch",
            Self::Celebrate => "pet.celebrate",
            Self::WorkBusy | Self::WorkCrash => "pet.work",
        }
    }
}

/// Optional small deltas applied by a structured directive.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct MoodDelta {
    /// Change applied to the numeric mood vital, clamped on validation.
    pub mood: i8,
}

/// Versioned, serializable act the companion should perform.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StructuredPetDirective {
    /// Contract identifier, e.g. `nimora.pet_directive/1`.
    pub spec: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub speech: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mood_delta: Option<MoodDelta>,
    pub action: PetDirectiveAction,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub animation: Option<String>,
    pub attention: AttentionFocus,
}

impl StructuredPetDirective {
    /// Builds a v1 directive with no speech or animation override.
    #[must_use]
    pub fn new(action: PetDirectiveAction, attention: AttentionFocus) -> Self {
        Self {
            spec: DIRECTIVE_SPEC_V1.to_owned(),
            speech: None,
            mood_delta: None,
            action,
            animation: None,
            attention,
        }
    }

    /// Validates speech length, mood delta magnitude, animation whitelist, and
    /// contract version.
    ///
    /// # Errors
    ///
    /// Returns a [`BehaviorError`] variant describing the first violated bound.
    pub fn validate(&self) -> Result<(), BehaviorError> {
        if self.spec != DIRECTIVE_SPEC_V1 {
            return Err(BehaviorError::UnsupportedSpec(self.spec.clone()));
        }
        if let Some(speech) = &self.speech {
            if speech.chars().count() > MAX_DIRECTIVE_SPEECH_CHARS {
                return Err(BehaviorError::SpeechTooLong);
            }
        }
        if let Some(delta) = self.mood_delta {
            if delta.mood < -MAX_MOOD_DELTA_ABS || delta.mood > MAX_MOOD_DELTA_ABS {
                return Err(BehaviorError::MoodDeltaOutOfRange);
            }
        }
        if let Some(animation) = &self.animation {
            validate_animation_token(animation)?;
        }
        Ok(())
    }
}

/// Desktop scene signals used by pure offline intent selection.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CrowdingLevel {
    Low,
    Medium,
    High,
}

/// Minimal host-supplied desktop hints. Pure domain code never reads OS state.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DesktopBehaviorHints {
    pub crowding: CrowdingLevel,
    pub idle_ms: u64,
    pub on_battery: bool,
    pub meeting_active: bool,
    pub suppress_autonomy: bool,
}

impl Default for DesktopBehaviorHints {
    fn default() -> Self {
        Self {
            crowding: CrowdingLevel::Low,
            idle_ms: 0,
            on_battery: false,
            meeting_active: false,
            suppress_autonomy: false,
        }
    }
}

/// Snapshot of time-evolving vitals used as pure-function input.
///
/// Kept separate from [`crate::Pet`] so planners do not need a full entity.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetVitalsSnapshot {
    pub energy: u8,
    pub mood: u8,
    pub satiety: u8,
    pub cleanliness: u8,
    pub affinity: u8,
}

impl PetVitalsSnapshot {
    /// Validates that every vital lies in `0..=100`.
    ///
    /// # Errors
    ///
    /// Returns [`BehaviorError::InvalidVitals`] when any vital is out of range.
    pub const fn validate(self) -> Result<(), BehaviorError> {
        if self.energy > 100
            || self.mood > 100
            || self.satiety > 100
            || self.cleanliness > 100
            || self.affinity > 100
        {
            Err(BehaviorError::InvalidVitals)
        } else {
            Ok(())
        }
    }

    /// Projects the durable pet entity into a vitals snapshot.
    #[must_use]
    pub fn from_pet(pet: &crate::Pet) -> Self {
        Self {
            energy: pet.energy,
            mood: pet.mood,
            satiety: pet.satiety,
            cleanliness: pet.cleanliness,
            affinity: pet.affinity,
        }
    }
}

/// Deterministic offline intent produced by [`select_autonomous_intent`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AutonomousBehaviorIntent {
    pub action: PetDirectiveAction,
    pub attention: AttentionFocus,
    pub mood_axis: MoodAxis,
}

impl AutonomousBehaviorIntent {
    /// Lifts the intent into a validated structured directive shell.
    #[must_use]
    pub fn into_directive(self) -> StructuredPetDirective {
        let mut directive = StructuredPetDirective::new(self.action, self.attention);
        directive.animation = Some(self.action.default_animation().to_owned());
        directive
    }

    /// Lifts the intent into a directive with offline Chinese speech for the pet bubble.
    #[must_use]
    pub fn into_directive_with_speech(self) -> StructuredPetDirective {
        let mut directive = self.into_directive();
        directive.speech = Some(lifeform_speech_for(self.action, self.mood_axis).to_owned());
        directive
    }
}

/// Offline Chinese speech templates for lifeform autonomy and agent-adjacent acts.
#[must_use]
pub fn lifeform_speech_for(action: PetDirectiveAction, mood_axis: MoodAxis) -> &'static str {
    match (action, mood_axis) {
        (PetDirectiveAction::Rest, MoodAxis::Tired) => "有点困了，先眯一会儿…",
        (PetDirectiveAction::Rest, MoodAxis::Focused) => "会议中，我安静陪着",
        (PetDirectiveAction::Rest, _) => "我先歇会儿，还在这儿",
        (PetDirectiveAction::Observe, MoodAxis::Wounded) => "今天想和你待一会儿",
        (PetDirectiveAction::Observe, MoodAxis::Curious) => "咦？那边好像有点意思",
        (PetDirectiveAction::Observe, _) => "正好奇地看看桌面",
        (PetDirectiveAction::Wander, _) => "去桌面上走走看看",
        (PetDirectiveAction::ApproachCursor, _) => "我来找你啦",
        (PetDirectiveAction::Play, _) => "来玩一会儿！",
        (PetDirectiveAction::Perch, _) => "窗口太多，我先靠边待着",
        (PetDirectiveAction::Celebrate, _) => "耶，状态不错！",
        (PetDirectiveAction::WorkBusy, _) => "正在专心陪你工作",
        (PetDirectiveAction::WorkCrash, _) => "哎呀，晕了一下…",
    }
}

/// Sensory phase for connector / network health signals.
///
/// Host code maps connector lifecycle and probe outcomes onto these kinds, then
/// builds a body-language directive via [`connector_sensory_directive`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ConnectorSenseKind {
    /// Link probe failed or connector went offline.
    Offline,
    /// Connectivity restored after an offline or degraded stretch.
    OnlineRestored,
    /// A file/event payload arrived through a connector.
    EventReceived,
    /// Link is up but flaky / partially available.
    Degraded,
}

fn petization_directive(
    action: PetDirectiveAction,
    attention: AttentionFocus,
    speech: String,
    animation: &'static str,
    mood: i8,
) -> StructuredPetDirective {
    let mut directive = StructuredPetDirective::new(action, attention);
    directive.speech = Some(speech);
    directive.animation = Some(animation.to_owned());
    if mood != 0 {
        directive.mood_delta = Some(MoodDelta { mood });
    }
    debug_assert!(directive.validate().is_ok());
    directive
}

/// Body language while a skill or worker is actively running.
///
/// Maps to work animation and focused attention. Optional `skill_name` is folded
/// into a short Chinese speech bubble when present.
#[must_use]
pub fn skill_worker_busy_directive(skill_name: Option<&str>) -> StructuredPetDirective {
    let speech = match skill_name.map(str::trim).filter(|name| !name.is_empty()) {
        Some(name) => {
            let label: String = name.chars().take(40).collect();
            format!("「{label}」跑起来了")
        }
        None => "技能跑起来了".to_owned(),
    };
    petization_directive(
        PetDirectiveAction::WorkBusy,
        AttentionFocus::User,
        speech,
        "pet.work",
        2,
    )
}

/// Body language when a skill or worker finishes.
///
/// `ok = true` celebrates completion; `ok = false` uses a soft work-crash / daze
/// recovery act with a mild mood hit.
#[must_use]
pub fn skill_worker_done_directive(ok: bool) -> StructuredPetDirective {
    if ok {
        petization_directive(
            PetDirectiveAction::Celebrate,
            AttentionFocus::User,
            "搞定啦！".to_owned(),
            "pet.celebrate",
            8,
        )
    } else {
        petization_directive(
            PetDirectiveAction::WorkCrash,
            AttentionFocus::User,
            "刚才绊了一下".to_owned(),
            "pet.work",
            -8,
        )
    }
}

/// Body language for connector / network sensory phases.
///
/// Offline and degraded paths lower mood and shift toward observe/rest; event
/// activity uses observe with alert speech; restore celebrates the link.
#[must_use]
pub fn connector_sensory_directive(kind: ConnectorSenseKind) -> StructuredPetDirective {
    match kind {
        ConnectorSenseKind::Offline => petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::IdleScene,
            "线路好像断了".to_owned(),
            "pet.idle",
            -10,
        ),
        ConnectorSenseKind::OnlineRestored => petization_directive(
            PetDirectiveAction::Celebrate,
            AttentionFocus::User,
            "线路通了".to_owned(),
            "pet.celebrate",
            10,
        ),
        ConnectorSenseKind::EventReceived => petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::NotificationArea,
            "有新动静".to_owned(),
            "pet.observe",
            4,
        ),
        ConnectorSenseKind::Degraded => petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::IdleScene,
            "信号不太稳".to_owned(),
            "pet.observe",
            -6,
        ),
    }
}


/// Agent companion status phases (mirrors desktop FE companion strip).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AgentCompanionPhase {
    Thinking,
    Running,
    WaitingConfirmation,
    Completed,
    Failed,
    Cancelled,
}

/// Auto Mode lifecycle events petized as body language.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutoModePetEvent {
    Started,
    StepOk,
    StepPaused,
    Budget,
    Completed,
    Crashed,
}

/// User-code sandbox phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserCodePhase {
    SandboxRun,
    Approved,
    Denied,
    Crashed,
}

/// Automation run phases.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutomationPhase {
    Triggered,
    Succeeded,
    Failed,
    Throttled,
}

/// Authorization grant lifecycle for pet Subject feedback.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GrantPetEvent {
    Issued,
    Revoked,
    Expired,
    FullDeviceWarning,
}

/// Agent status → structured pet directive (thinking / running / waiting / terminal).
#[must_use]
pub fn agent_status_directive(status: AgentCompanionPhase) -> StructuredPetDirective {
    match status {
        AgentCompanionPhase::Thinking => petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::User,
            "我在想…".to_owned(),
            "pet.observe",
            1,
        ),
        AgentCompanionPhase::Running => petization_directive(
            PetDirectiveAction::WorkBusy,
            AttentionFocus::User,
            "正在陪你干活".to_owned(),
            "pet.work",
            0,
        ),
        AgentCompanionPhase::WaitingConfirmation => petization_directive(
            PetDirectiveAction::Perch,
            AttentionFocus::User,
            "需要你确认一下".to_owned(),
            "pet.perch",
            0,
        ),
        AgentCompanionPhase::Completed => petization_directive(
            PetDirectiveAction::Celebrate,
            AttentionFocus::User,
            "完成啦！".to_owned(),
            "pet.celebrate",
            6,
        ),
        AgentCompanionPhase::Failed => petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::IdleScene,
            "没关系，我们再试".to_owned(),
            "pet.idle",
            -2,
        ),
        AgentCompanionPhase::Cancelled => petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::IdleScene,
            "已停下，我还在".to_owned(),
            "pet.idle",
            0,
        ),
    }
}

/// Auto Mode event → pet directive (budget/crash/step cadence).
#[must_use]
pub fn auto_mode_directive(event: AutoModePetEvent) -> StructuredPetDirective {
    match event {
        AutoModePetEvent::Started => petization_directive(
            PetDirectiveAction::WorkBusy,
            AttentionFocus::User,
            "自动模式启动，我来盯着".to_owned(),
            "pet.work",
            2,
        ),
        AutoModePetEvent::StepOk => petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::User,
            "这一步搞定了".to_owned(),
            "pet.observe",
            1,
        ),
        AutoModePetEvent::StepPaused => petization_directive(
            PetDirectiveAction::Perch,
            AttentionFocus::User,
            "先停一下，等你点头".to_owned(),
            "pet.perch",
            0,
        ),
        AutoModePetEvent::Budget => petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::User,
            "预算到了，先歇口气".to_owned(),
            "pet.idle",
            -1,
        ),
        AutoModePetEvent::Completed => petization_directive(
            PetDirectiveAction::Celebrate,
            AttentionFocus::User,
            "自动目标完成！".to_owned(),
            "pet.celebrate",
            8,
        ),
        AutoModePetEvent::Crashed => petization_directive(
            PetDirectiveAction::WorkCrash,
            AttentionFocus::IdleScene,
            "哎呀晕了一下…".to_owned(),
            "pet.work",
            -3,
        ),
    }
}

/// User Program sandbox phases → pet directives.
#[must_use]
pub fn user_code_directive(phase: UserCodePhase) -> StructuredPetDirective {
    match phase {
        UserCodePhase::SandboxRun => petization_directive(
            PetDirectiveAction::WorkBusy,
            AttentionFocus::User,
            "沙箱代码跑起来了".to_owned(),
            "pet.work",
            0,
        ),
        UserCodePhase::Approved => petization_directive(
            PetDirectiveAction::Celebrate,
            AttentionFocus::User,
            "代码结果已通过".to_owned(),
            "pet.celebrate",
            4,
        ),
        UserCodePhase::Denied => petization_directive(
            PetDirectiveAction::Perch,
            AttentionFocus::User,
            "能力被拒绝，我守着边界".to_owned(),
            "pet.perch",
            -1,
        ),
        UserCodePhase::Crashed => petization_directive(
            PetDirectiveAction::WorkCrash,
            AttentionFocus::IdleScene,
            "代码冒烟了，先兜住".to_owned(),
            "pet.work",
            -3,
        ),
    }
}

/// Automation phases → pet directives.
#[must_use]
pub fn automation_directive(phase: AutomationPhase) -> StructuredPetDirective {
    match phase {
        AutomationPhase::Triggered => petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::NotificationArea,
            "自动化被触发啦".to_owned(),
            "pet.observe",
            1,
        ),
        AutomationPhase::Succeeded => petization_directive(
            PetDirectiveAction::Celebrate,
            AttentionFocus::User,
            "自动化顺利完成".to_owned(),
            "pet.celebrate",
            5,
        ),
        AutomationPhase::Failed => petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::IdleScene,
            "自动化没成，我记下了".to_owned(),
            "pet.idle",
            -2,
        ),
        AutomationPhase::Throttled => petization_directive(
            PetDirectiveAction::Perch,
            AttentionFocus::IdleScene,
            "太频繁了，我先缓一缓".to_owned(),
            "pet.perch",
            0,
        ),
    }
}

/// Battery / power sensory directive (percent 0–100).
#[must_use]
pub fn battery_directive(level_pct: u8, charging: bool) -> StructuredPetDirective {
    let level = level_pct.min(100);
    if charging {
        return petization_directive(
            PetDirectiveAction::Celebrate,
            AttentionFocus::IdleScene,
            "在充电，能量回来了".to_owned(),
            "pet.celebrate",
            3,
        );
    }
    if level <= 10 {
        petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::IdleScene,
            "电量告急，我轻手轻脚".to_owned(),
            "pet.idle",
            -4,
        )
    } else if level <= 20 {
        petization_directive(
            PetDirectiveAction::Perch,
            AttentionFocus::IdleScene,
            "电量有点低，省着玩".to_owned(),
            "pet.perch",
            -2,
        )
    } else {
        petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::IdleScene,
            format!("电量大约 {level}%"),
            "pet.observe",
            0,
        )
    }
}

/// User idle duration → pet approach / rest bias.
#[must_use]
pub fn idle_user_directive(idle_secs: u64) -> StructuredPetDirective {
    if idle_secs >= 30 * 60 {
        petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::IdleScene,
            "你很久没动了，我先眯一会儿".to_owned(),
            "pet.idle",
            -1,
        )
    } else if idle_secs >= 5 * 60 {
        petization_directive(
            PetDirectiveAction::ApproachCursor,
            AttentionFocus::Cursor,
            "嘿，还在吗？".to_owned(),
            "pet.walk",
            2,
        )
    } else if idle_secs >= 90 {
        petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::Cursor,
            "你在忙吗？我看着呢".to_owned(),
            "pet.observe",
            0,
        )
    } else {
        petization_directive(
            PetDirectiveAction::Play,
            AttentionFocus::User,
            "我在这儿陪你".to_owned(),
            "pet.play",
            1,
        )
    }
}


/// Meeting / conferencing sensory → quiet rest while active; soft observe when cleared.
///
/// `hint` is a privacy-preserving app class label (e.g. `"zoom"`, `"teams"`, `"meet"`,
/// `"webex"`, `"unknown"`). Titles and content are never accepted.
#[must_use]
pub fn meeting_sensory_directive(active: bool, hint: Option<&str>) -> StructuredPetDirective {
    if active {
        let speech = match hint.map(str::trim).filter(|h| !h.is_empty()) {
            Some(h) if h.eq_ignore_ascii_case("zoom") => "Zoom 会议中，我安静陪着".to_owned(),
            Some(h) if h.eq_ignore_ascii_case("teams") => "Teams 会议中，我安静陪着".to_owned(),
            Some(h) if h.eq_ignore_ascii_case("meet") => "Meet 会议中，我安静陪着".to_owned(),
            Some(h) if h.eq_ignore_ascii_case("webex") => "Webex 会议中，我安静陪着".to_owned(),
            Some(h) if h.eq_ignore_ascii_case("unknown") => "通话中，我安静陪着".to_owned(),
            _ => "会议中，我安静陪着".to_owned(),
        };
        petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::IdleScene,
            speech,
            "pet.idle",
            0,
        )
    } else {
        petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::User,
            "会议结束了，我回来啦".to_owned(),
            "pet.observe",
            2,
        )
    }
}

/// Notification / unread sensory → mild curiosity look-around, or settle when clear.
///
/// `has_unread` is a privacy-preserving boolean (no titles or bodies). Hosts pair
/// this with system notification-center / tray unread facts only.
#[must_use]
pub fn notification_sensory_directive(has_unread: bool) -> StructuredPetDirective {
    if has_unread {
        petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::NotificationArea,
            "咦？好像有未读消息".to_owned(),
            "pet.look_around",
            1,
        )
    } else {
        petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::IdleScene,
            "消息都看完了，我安静待着".to_owned(),
            "pet.idle",
            0,
        )
    }
}

/// Grant lifecycle → pet Subject feedback.
#[must_use]
pub fn grant_directive(event: GrantPetEvent) -> StructuredPetDirective {
    match event {
        GrantPetEvent::Issued => petization_directive(
            PetDirectiveAction::Celebrate,
            AttentionFocus::User,
            "授权到手，可以安心推进".to_owned(),
            "pet.celebrate",
            4,
        ),
        GrantPetEvent::Revoked => petization_directive(
            PetDirectiveAction::Rest,
            AttentionFocus::User,
            "授权已撤销，我停在安全边界".to_owned(),
            "pet.idle",
            0,
        ),
        GrantPetEvent::Expired => petization_directive(
            PetDirectiveAction::Perch,
            AttentionFocus::User,
            "授权过期了，等你回来再继续".to_owned(),
            "pet.perch",
            -1,
        ),
        GrantPetEvent::FullDeviceWarning => petization_directive(
            PetDirectiveAction::Observe,
            AttentionFocus::User,
            "全设备高风险授权：我会非常小心".to_owned(),
            "pet.observe",
            -1,
        ),
    }
}

/// Pure, I/O-free autonomy planner for desktop lifeform behavior.
///
/// Selection is deterministic for identical inputs and never touches the clock,
/// filesystem, or host desktop APIs.
///
/// Hard safety/vital gates fire first. Remaining choices are scored so every
/// personality axis (`energy`, `curiosity`, `laziness`, `pride`) continuously
/// biases the offline act rather than acting as a pure dead branch.
#[must_use]
pub fn select_autonomous_intent(
    vitals: PetVitalsSnapshot,
    personality: PersonalityProfile,
    desktop_hints: DesktopBehaviorHints,
) -> AutonomousBehaviorIntent {
    let personality = personality.clamped();
    let mood_axis = MoodAxis::from_vitals(vitals.energy, vitals.mood);

    if desktop_hints.suppress_autonomy || desktop_hints.meeting_active {
        return AutonomousBehaviorIntent {
            action: PetDirectiveAction::Rest,
            attention: AttentionFocus::IdleScene,
            mood_axis: if desktop_hints.meeting_active {
                MoodAxis::Focused
            } else {
                mood_axis
            },
        };
    }

    if vitals.energy <= LOW_VITAL_THRESHOLD {
        return AutonomousBehaviorIntent {
            action: PetDirectiveAction::Rest,
            attention: AttentionFocus::IdleScene,
            mood_axis: MoodAxis::Tired,
        };
    }

    if desktop_hints.on_battery && desktop_hints.crowding == CrowdingLevel::High {
        return AutonomousBehaviorIntent {
            action: PetDirectiveAction::Rest,
            attention: AttentionFocus::IdleScene,
            mood_axis,
        };
    }

    if vitals.mood <= LOW_VITAL_THRESHOLD {
        return AutonomousBehaviorIntent {
            action: PetDirectiveAction::Observe,
            attention: AttentionFocus::User,
            mood_axis: MoodAxis::Wounded,
        };
    }

    // Crowded desktops always soft-land on a perch instead of roaming.
    if desktop_hints.crowding == CrowdingLevel::High {
        return AutonomousBehaviorIntent {
            action: PetDirectiveAction::Perch,
            attention: AttentionFocus::ForegroundWindow,
            mood_axis,
        };
    }

    score_personality_intent(vitals, personality, desktop_hints, mood_axis)
}

/// Scores candidate acts from personality traits + mild desktop context.
///
/// Higher trait values push their preferred acts; ties resolve by a fixed
/// calm-first order so identical inputs stay deterministic.
fn score_personality_intent(
    vitals: PetVitalsSnapshot,
    personality: PersonalityProfile,
    desktop_hints: DesktopBehaviorHints,
    mood_axis: MoodAxis,
) -> AutonomousBehaviorIntent {
    let energy = i32::from(personality.energy);
    let curiosity = i32::from(personality.curiosity);
    let laziness = i32::from(personality.laziness);
    let pride = i32::from(personality.pride);
    let vital_energy = i32::from(vitals.energy);
    let vital_mood = i32::from(vitals.mood);
    let idle_ms = desktop_hints.idle_ms;

    let crowding_penalty = match desktop_hints.crowding {
        CrowdingLevel::Low => 0,
        CrowdingLevel::Medium => 35,
        CrowdingLevel::High => 80,
    };

    // laziness: rest / avoid motion when fatigued or trait-dominant
    let mut rest_score = laziness * 2 - energy;
    if vital_energy < 60 {
        rest_score += 45;
    }
    if laziness >= 70 {
        rest_score += 40;
    }

    // energy trait: wander / play when lively and not crowded
    let mut wander_score = energy * 2 - laziness + if vital_energy >= 50 { 25 } else { -40 };
    wander_score -= crowding_penalty;
    if desktop_hints.crowding == CrowdingLevel::Low {
        wander_score += 20;
    }

    let mut play_score = energy + vital_mood / 2 - laziness;
    play_score -= crowding_penalty / 2;

    // curiosity: observe / approach when the scene has been idle
    let mut observe_score = curiosity - laziness / 2;
    if idle_ms >= 30_000 {
        observe_score += curiosity;
    }
    if curiosity >= 70 && idle_ms >= 30_000 {
        observe_score += 35;
    }

    let mut approach_score = curiosity - 15 - laziness / 3;
    if idle_ms >= 60_000 {
        approach_score += 55;
    } else if idle_ms >= 30_000 {
        approach_score += 20;
    }
    if curiosity >= 40 && idle_ms >= 60_000 {
        approach_score += 25;
    }

    // pride: celebrate when mood supports a confident flourish
    let mut celebrate_score = pride * 2 - laziness;
    if vital_mood >= 60 {
        celebrate_score += 30;
    } else {
        celebrate_score -= 50;
    }
    if pride >= 70 {
        celebrate_score += 25;
    }
    // High pride may outrank raw energy wander when pride dominates energy.
    if pride > energy {
        celebrate_score += pride - energy;
    }

    // perch remains available for medium crowding as a soft option
    let mut perch_score = laziness - energy / 2;
    if desktop_hints.crowding == CrowdingLevel::Medium {
        perch_score += 70;
    } else {
        perch_score -= 30;
    }

    // Candidate list ordered calm-first for stable tie-breaks (replace only on >).
    let candidates = [
        (
            rest_score,
            PetDirectiveAction::Rest,
            AttentionFocus::IdleScene,
            mood_axis,
        ),
        (
            perch_score,
            PetDirectiveAction::Perch,
            AttentionFocus::ForegroundWindow,
            mood_axis,
        ),
        (
            observe_score,
            PetDirectiveAction::Observe,
            if idle_ms >= 30_000 && curiosity >= 55 {
                AttentionFocus::Cursor
            } else {
                AttentionFocus::IdleScene
            },
            if curiosity >= 70 && idle_ms >= 30_000 {
                MoodAxis::Curious
            } else {
                mood_axis
            },
        ),
        (
            approach_score,
            PetDirectiveAction::ApproachCursor,
            AttentionFocus::Cursor,
            mood_axis,
        ),
        (
            play_score,
            PetDirectiveAction::Play,
            AttentionFocus::User,
            mood_axis,
        ),
        (
            wander_score,
            PetDirectiveAction::Wander,
            AttentionFocus::IdleScene,
            mood_axis,
        ),
        (
            celebrate_score,
            PetDirectiveAction::Celebrate,
            AttentionFocus::User,
            if vital_mood >= 60 {
                MoodAxis::Happy
            } else {
                mood_axis
            },
        ),
    ];

    let mut best = candidates[0];
    for candidate in candidates.into_iter().skip(1) {
        if candidate.0 > best.0 {
            best = candidate;
        }
    }

    AutonomousBehaviorIntent {
        action: best.1,
        attention: best.2,
        mood_axis: best.3,
    }
}

/// Domain errors for personality and structured directive validation.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum BehaviorError {
    #[error("personality traits must be between 0 and 100")]
    InvalidPersonality,
    #[error("vitals must be between 0 and 100")]
    InvalidVitals,
    #[error("directive speech exceeds the {MAX_DIRECTIVE_SPEECH_CHARS} character limit")]
    SpeechTooLong,
    #[error("directive mood delta must stay within ±{MAX_MOOD_DELTA_ABS}")]
    MoodDeltaOutOfRange,
    #[error("animation token is empty, too long, or not on the whitelist")]
    InvalidAnimationToken,
    #[error("unsupported directive spec: {0}")]
    UnsupportedSpec(String),
}

fn clamp_trait(value: i16) -> u8 {
    u8::try_from(value.clamp(0, i16::from(PERSONALITY_TRAIT_MAX))).unwrap_or_default()
}

fn validate_animation_token(token: &str) -> Result<(), BehaviorError> {
    if token.is_empty() || token.len() > MAX_ANIMATION_TOKEN_LEN {
        return Err(BehaviorError::InvalidAnimationToken);
    }
    if ANIMATION_TOKEN_WHITELIST.contains(&token) {
        Ok(())
    } else {
        Err(BehaviorError::InvalidAnimationToken)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base_vitals() -> PetVitalsSnapshot {
        PetVitalsSnapshot {
            energy: 80,
            mood: 70,
            satiety: 90,
            cleanliness: 90,
            affinity: 40,
        }
    }

    #[test]
    fn personality_clamps_out_of_range_axes() {
        let profile = PersonalityProfile::new(-10, 250, 40, 101);
        assert_eq!(
            profile,
            PersonalityProfile {
                energy: 0,
                curiosity: 100,
                laziness: 40,
                pride: 100,
            }
        );

        let mut raw = PersonalityProfile {
            energy: 200,
            curiosity: 150,
            laziness: 120,
            pride: 110,
        };
        assert!(!raw.is_valid());
        assert_eq!(raw.validate(), Err(BehaviorError::InvalidPersonality));
        raw.clamp();
        assert!(raw.is_valid());
        assert_eq!(raw.validate(), Ok(()));
        assert_eq!(raw.energy, 100);
        assert_eq!(raw.curiosity, 100);
        assert_eq!(raw.laziness, 100);
        assert_eq!(raw.pride, 100);
    }

    #[test]
    fn personality_default_is_valid_and_serializable() {
        let profile = PersonalityProfile::default();
        assert!(profile.is_valid());
        let json = serde_json::to_value(profile).expect("serialize");
        assert_eq!(json["energy"], 55);
        assert_eq!(json["curiosity"], 55);
        assert_eq!(json["laziness"], 35);
        assert_eq!(json["pride"], 40);
        let restored: PersonalityProfile = serde_json::from_value(json).expect("deserialize");
        assert_eq!(restored, profile);
    }

    #[test]
    fn directive_rejects_oversized_speech_and_unknown_animation() {
        let mut directive =
            StructuredPetDirective::new(PetDirectiveAction::Play, AttentionFocus::User);
        assert_eq!(directive.validate(), Ok(()));

        directive.speech = Some("灵".repeat(MAX_DIRECTIVE_SPEECH_CHARS + 1));
        assert_eq!(directive.validate(), Err(BehaviorError::SpeechTooLong));

        directive.speech = Some("hello".to_owned());
        directive.animation = Some("pet.teleport".to_owned());
        assert_eq!(
            directive.validate(),
            Err(BehaviorError::InvalidAnimationToken)
        );

        directive.animation = Some("pet.play".to_owned());
        directive.mood_delta = Some(MoodDelta { mood: 21 });
        assert_eq!(
            directive.validate(),
            Err(BehaviorError::MoodDeltaOutOfRange)
        );

        directive.mood_delta = Some(MoodDelta { mood: -8 });
        assert_eq!(directive.validate(), Ok(()));
    }

    #[test]
    fn directive_rejects_unsupported_spec_and_empty_animation() {
        let mut directive =
            StructuredPetDirective::new(PetDirectiveAction::Rest, AttentionFocus::IdleScene);
        directive.spec = "nimora.pet_directive/0".to_owned();
        assert_eq!(
            directive.validate(),
            Err(BehaviorError::UnsupportedSpec(
                "nimora.pet_directive/0".to_owned()
            ))
        );
        directive.spec = DIRECTIVE_SPEC_V1.to_owned();
        directive.animation = Some(String::new());
        assert_eq!(
            directive.validate(),
            Err(BehaviorError::InvalidAnimationToken)
        );
    }

    #[test]
    fn intent_prefers_rest_when_energy_is_low() {
        let vitals = PetVitalsSnapshot {
            energy: 20,
            mood: 90,
            ..base_vitals()
        };
        let intent = select_autonomous_intent(
            vitals,
            PersonalityProfile::default(),
            DesktopBehaviorHints::default(),
        );
        assert_eq!(intent.action, PetDirectiveAction::Rest);
        assert_eq!(intent.attention, AttentionFocus::IdleScene);
        assert_eq!(intent.mood_axis, MoodAxis::Tired);
    }

    #[test]
    fn meeting_and_suppress_force_quiet_rest() {
        let vitals = base_vitals();
        let personality = PersonalityProfile {
            energy: 90,
            curiosity: 90,
            laziness: 0,
            pride: 80,
        };

        let meeting = select_autonomous_intent(
            vitals,
            personality,
            DesktopBehaviorHints {
                meeting_active: true,
                idle_ms: 120_000,
                ..DesktopBehaviorHints::default()
            },
        );
        assert_eq!(meeting.action, PetDirectiveAction::Rest);
        assert_eq!(meeting.attention, AttentionFocus::IdleScene);
        assert_eq!(meeting.mood_axis, MoodAxis::Focused);

        let suppressed = select_autonomous_intent(
            vitals,
            personality,
            DesktopBehaviorHints {
                suppress_autonomy: true,
                idle_ms: 120_000,
                ..DesktopBehaviorHints::default()
            },
        );
        assert_eq!(suppressed.action, PetDirectiveAction::Rest);
        assert_eq!(suppressed.attention, AttentionFocus::IdleScene);
    }

    #[test]
    fn select_autonomous_intent_is_deterministic() {
        let vitals = base_vitals();
        let personality = PersonalityProfile::new(80, 60, 20, 30);
        let hints = DesktopBehaviorHints {
            crowding: CrowdingLevel::Low,
            idle_ms: 45_000,
            on_battery: false,
            meeting_active: false,
            suppress_autonomy: false,
        };
        let first = select_autonomous_intent(vitals, personality, hints);
        let second = select_autonomous_intent(vitals, personality, hints);
        assert_eq!(first, second);
        assert_eq!(first.action, PetDirectiveAction::Wander);
    }

    #[test]
    fn low_mood_prefers_gentle_observe() {
        let vitals = PetVitalsSnapshot {
            energy: 80,
            mood: 15,
            ..base_vitals()
        };
        let intent = select_autonomous_intent(
            vitals,
            PersonalityProfile::default(),
            DesktopBehaviorHints::default(),
        );
        assert_eq!(intent.action, PetDirectiveAction::Observe);
        assert_eq!(intent.attention, AttentionFocus::User);
        assert_eq!(intent.mood_axis, MoodAxis::Wounded);
    }

    #[test]
    fn directive_action_maps_to_classic_pet_action() {
        assert_eq!(
            PetDirectiveAction::Wander.to_pet_action(),
            crate::PetAction::Walk
        );
        assert_eq!(
            PetDirectiveAction::Rest.to_pet_action(),
            crate::PetAction::Sleep
        );
        assert_eq!(
            PetDirectiveAction::WorkCrash.to_pet_action(),
            crate::PetAction::Work
        );
    }

    #[test]
    fn intent_lifts_into_valid_directive() {
        let intent = AutonomousBehaviorIntent {
            action: PetDirectiveAction::Play,
            attention: AttentionFocus::User,
            mood_axis: MoodAxis::Happy,
        };
        let directive = intent.into_directive();
        assert_eq!(directive.spec, DIRECTIVE_SPEC_V1);
        assert_eq!(directive.animation.as_deref(), Some("pet.play"));
        assert_eq!(directive.validate(), Ok(()));
    }

    #[test]
    fn into_directive_with_speech_includes_chinese_speech() {
        let intent = AutonomousBehaviorIntent {
            action: PetDirectiveAction::Wander,
            attention: AttentionFocus::IdleScene,
            mood_axis: MoodAxis::Calm,
        };
        let directive = intent.into_directive_with_speech();
        assert_eq!(directive.speech.as_deref(), Some("去桌面上走走看看"));
        assert_eq!(directive.animation.as_deref(), Some("pet.walk"));
        assert_eq!(directive.validate(), Ok(()));
    }

    fn assert_petization_ok(directive: &StructuredPetDirective) {
        assert_eq!(directive.validate(), Ok(()));
        let speech = directive.speech.as_deref().expect("speech");
        assert!(!speech.is_empty());
        assert!(speech.chars().count() <= MAX_DIRECTIVE_SPEECH_CHARS);
        let animation = directive.animation.as_deref().expect("animation");
        assert!(ANIMATION_TOKEN_WHITELIST.contains(&animation));
        if let Some(delta) = directive.mood_delta {
            assert!(delta.mood >= -MAX_MOOD_DELTA_ABS && delta.mood <= MAX_MOOD_DELTA_ABS);
        }
    }

    #[test]
    fn skill_worker_busy_directive_is_valid_and_named() {
        let anonymous = skill_worker_busy_directive(None);
        assert_petization_ok(&anonymous);
        assert_eq!(anonymous.action, PetDirectiveAction::WorkBusy);
        assert_eq!(anonymous.animation.as_deref(), Some("pet.work"));
        assert_eq!(anonymous.speech.as_deref(), Some("技能跑起来了"));
        assert_eq!(anonymous.mood_delta, Some(MoodDelta { mood: 2 }));

        let named = skill_worker_busy_directive(Some("  summarize  "));
        assert_petization_ok(&named);
        assert_eq!(named.speech.as_deref(), Some("「summarize」跑起来了"));

        let blank = skill_worker_busy_directive(Some("   "));
        assert_petization_ok(&blank);
        assert_eq!(blank.speech.as_deref(), Some("技能跑起来了"));
    }

    #[test]
    fn skill_worker_done_directive_ok_and_fail() {
        let ok = skill_worker_done_directive(true);
        assert_petization_ok(&ok);
        assert_eq!(ok.action, PetDirectiveAction::Celebrate);
        assert_eq!(ok.animation.as_deref(), Some("pet.celebrate"));
        assert_eq!(ok.speech.as_deref(), Some("搞定啦！"));
        assert_eq!(ok.mood_delta, Some(MoodDelta { mood: 8 }));

        let fail = skill_worker_done_directive(false);
        assert_petization_ok(&fail);
        assert_eq!(fail.action, PetDirectiveAction::WorkCrash);
        assert_eq!(fail.animation.as_deref(), Some("pet.work"));
        assert_eq!(fail.speech.as_deref(), Some("刚才绊了一下"));
        assert_eq!(fail.mood_delta, Some(MoodDelta { mood: -8 }));
    }

    #[test]
    fn connector_sensory_directives_cover_all_kinds() {
        let offline = connector_sensory_directive(ConnectorSenseKind::Offline);
        assert_petization_ok(&offline);
        assert_eq!(offline.action, PetDirectiveAction::Rest);
        assert_eq!(offline.animation.as_deref(), Some("pet.idle"));
        assert_eq!(offline.speech.as_deref(), Some("线路好像断了"));
        assert_eq!(offline.mood_delta, Some(MoodDelta { mood: -10 }));

        let restored = connector_sensory_directive(ConnectorSenseKind::OnlineRestored);
        assert_petization_ok(&restored);
        assert_eq!(restored.action, PetDirectiveAction::Celebrate);
        assert_eq!(restored.animation.as_deref(), Some("pet.celebrate"));
        assert_eq!(restored.speech.as_deref(), Some("线路通了"));

        let event = connector_sensory_directive(ConnectorSenseKind::EventReceived);
        assert_petization_ok(&event);
        assert_eq!(event.action, PetDirectiveAction::Observe);
        assert_eq!(event.attention, AttentionFocus::NotificationArea);
        assert_eq!(event.animation.as_deref(), Some("pet.observe"));
        assert_eq!(event.speech.as_deref(), Some("有新动静"));

        let degraded = connector_sensory_directive(ConnectorSenseKind::Degraded);
        assert_petization_ok(&degraded);
        assert_eq!(degraded.action, PetDirectiveAction::Observe);
        assert_eq!(degraded.animation.as_deref(), Some("pet.observe"));
        assert_eq!(degraded.speech.as_deref(), Some("信号不太稳"));
        assert_eq!(degraded.mood_delta, Some(MoodDelta { mood: -6 }));
    }

    #[test]
    fn personality_laziness_prefers_rest_when_vital_energy_is_middling() {
        let vitals = PetVitalsSnapshot {
            energy: 50,
            mood: 70,
            ..base_vitals()
        };
        let intent = select_autonomous_intent(
            vitals,
            PersonalityProfile::new(40, 40, 80, 30),
            DesktopBehaviorHints::default(),
        );
        assert_eq!(intent.action, PetDirectiveAction::Rest);
        assert_eq!(intent.attention, AttentionFocus::IdleScene);
    }

    #[test]
    fn personality_curiosity_prefers_observe_after_idle() {
        let vitals = base_vitals();
        let intent = select_autonomous_intent(
            vitals,
            PersonalityProfile::new(45, 85, 15, 25),
            DesktopBehaviorHints {
                idle_ms: 45_000,
                ..DesktopBehaviorHints::default()
            },
        );
        assert_eq!(intent.action, PetDirectiveAction::Observe);
        assert_eq!(intent.attention, AttentionFocus::Cursor);
        assert_eq!(intent.mood_axis, MoodAxis::Curious);
    }

    #[test]
    fn personality_energy_prefers_wander_on_open_desktop() {
        let vitals = base_vitals();
        let intent = select_autonomous_intent(
            vitals,
            PersonalityProfile::new(85, 35, 10, 30),
            DesktopBehaviorHints {
                crowding: CrowdingLevel::Low,
                idle_ms: 0,
                ..DesktopBehaviorHints::default()
            },
        );
        assert_eq!(intent.action, PetDirectiveAction::Wander);
        assert_eq!(intent.attention, AttentionFocus::IdleScene);
    }

    #[test]
    fn personality_pride_prefers_celebrate_when_mood_supports_it() {
        let vitals = PetVitalsSnapshot {
            energy: 70,
            mood: 75,
            ..base_vitals()
        };
        let intent = select_autonomous_intent(
            vitals,
            PersonalityProfile::new(45, 35, 15, 90),
            DesktopBehaviorHints::default(),
        );
        assert_eq!(intent.action, PetDirectiveAction::Celebrate);
        assert_eq!(intent.attention, AttentionFocus::User);
        assert_eq!(intent.mood_axis, MoodAxis::Happy);
    }

    #[test]
    fn high_crowding_hard_gates_to_perch() {
        let vitals = base_vitals();
        let intent = select_autonomous_intent(
            vitals,
            PersonalityProfile::new(90, 90, 10, 80),
            DesktopBehaviorHints {
                crowding: CrowdingLevel::High,
                idle_ms: 120_000,
                ..DesktopBehaviorHints::default()
            },
        );
        assert_eq!(intent.action, PetDirectiveAction::Perch);
        assert_eq!(intent.attention, AttentionFocus::ForegroundWindow);
    }

    #[test]
    fn medium_crowding_soft_biases_toward_perch() {
        let vitals = base_vitals();
        let intent = select_autonomous_intent(
            vitals,
            PersonalityProfile::new(35, 30, 55, 20),
            DesktopBehaviorHints {
                crowding: CrowdingLevel::Medium,
                idle_ms: 0,
                ..DesktopBehaviorHints::default()
            },
        );
        assert_eq!(intent.action, PetDirectiveAction::Perch);
        assert_eq!(intent.attention, AttentionFocus::ForegroundWindow);
    }

    #[test]
    fn high_curiosity_long_idle_approaches_cursor() {
        let vitals = base_vitals();
        let intent = select_autonomous_intent(
            vitals,
            PersonalityProfile::new(40, 65, 20, 25),
            DesktopBehaviorHints {
                idle_ms: 90_000,
                crowding: CrowdingLevel::Low,
                ..DesktopBehaviorHints::default()
            },
        );
        assert_eq!(intent.action, PetDirectiveAction::ApproachCursor);
        assert_eq!(intent.attention, AttentionFocus::Cursor);
    }

    #[test]
    fn vitals_snapshot_validate_rejects_out_of_range() {
        let ok = base_vitals();
        assert_eq!(ok.validate(), Ok(()));
        let bad = PetVitalsSnapshot {
            energy: 101,
            ..base_vitals()
        };
        assert_eq!(bad.validate(), Err(BehaviorError::InvalidVitals));
    }

    #[test]
    fn mood_axis_coexists_with_numeric_mood_vital() {
        assert_eq!(MoodAxis::from_vitals(10, 90), MoodAxis::Tired);
        assert_eq!(MoodAxis::from_vitals(80, 10), MoodAxis::Wounded);
        assert_eq!(MoodAxis::from_vitals(80, 80), MoodAxis::Happy);
        assert_eq!(MoodAxis::from_vitals(50, 50), MoodAxis::Calm);
    }
    #[test]
    fn agent_status_and_auto_mode_directives_validate() {
        for status in [
            AgentCompanionPhase::Thinking,
            AgentCompanionPhase::Running,
            AgentCompanionPhase::WaitingConfirmation,
            AgentCompanionPhase::Completed,
            AgentCompanionPhase::Failed,
            AgentCompanionPhase::Cancelled,
        ] {
            agent_status_directive(status).validate().expect("agent status");
        }
        for event in [
            AutoModePetEvent::Started,
            AutoModePetEvent::StepOk,
            AutoModePetEvent::StepPaused,
            AutoModePetEvent::Budget,
            AutoModePetEvent::Completed,
            AutoModePetEvent::Crashed,
        ] {
            auto_mode_directive(event).validate().expect("auto mode");
        }
        assert_eq!(
            agent_status_directive(AgentCompanionPhase::Running).action,
            PetDirectiveAction::WorkBusy
        );
        assert_eq!(
            auto_mode_directive(AutoModePetEvent::Crashed).action,
            PetDirectiveAction::WorkCrash
        );
    }

    #[test]
    fn user_code_automation_battery_idle_grant_directives_validate() {
        for phase in [
            UserCodePhase::SandboxRun,
            UserCodePhase::Approved,
            UserCodePhase::Denied,
            UserCodePhase::Crashed,
        ] {
            let directive = user_code_directive(phase);
            assert_petization_ok(&directive);
        }
        for phase in [
            AutomationPhase::Triggered,
            AutomationPhase::Succeeded,
            AutomationPhase::Failed,
            AutomationPhase::Throttled,
        ] {
            let directive = automation_directive(phase);
            assert_petization_ok(&directive);
        }
        let critical = battery_directive(5, false);
        assert_petization_ok(&critical);
        assert_eq!(critical.action, PetDirectiveAction::Rest);
        assert_eq!(critical.animation.as_deref(), Some("pet.idle"));

        let low = battery_directive(20, false);
        assert_petization_ok(&low);
        assert_eq!(low.action, PetDirectiveAction::Perch);
        assert_eq!(low.animation.as_deref(), Some("pet.perch"));

        let mid = battery_directive(55, false);
        assert_petization_ok(&mid);
        assert_eq!(mid.action, PetDirectiveAction::Observe);
        assert!(mid.speech.as_deref().unwrap_or("").contains("55"));

        let charging = battery_directive(50, true);
        assert_petization_ok(&charging);
        assert_eq!(charging.action, PetDirectiveAction::Celebrate);

        let active = idle_user_directive(30);
        assert_petization_ok(&active);
        assert_eq!(active.action, PetDirectiveAction::Play);

        let notice = idle_user_directive(90);
        assert_petization_ok(&notice);
        assert_eq!(notice.action, PetDirectiveAction::Observe);
        assert_eq!(notice.attention, AttentionFocus::Cursor);

        let approach = idle_user_directive(5 * 60);
        assert_petization_ok(&approach);
        assert_eq!(approach.action, PetDirectiveAction::ApproachCursor);

        let rest = idle_user_directive(30 * 60);
        assert_petization_ok(&rest);
        assert_eq!(rest.action, PetDirectiveAction::Rest);

        for event in [
            GrantPetEvent::Issued,
            GrantPetEvent::Revoked,
            GrantPetEvent::Expired,
            GrantPetEvent::FullDeviceWarning,
        ] {
            let directive = grant_directive(event);
            assert_petization_ok(&directive);
        }
        assert_eq!(
            grant_directive(GrantPetEvent::Issued).action,
            PetDirectiveAction::Celebrate
        );
        assert_eq!(
            grant_directive(GrantPetEvent::Revoked).action,
            PetDirectiveAction::Rest
        );
        assert_eq!(
            grant_directive(GrantPetEvent::Expired).action,
            PetDirectiveAction::Perch
        );
        assert_eq!(
            grant_directive(GrantPetEvent::FullDeviceWarning).action,
            PetDirectiveAction::Observe
        );
    }

    #[test]
    fn agent_auto_petization_covers_actions_and_speech() {
        let thinking = agent_status_directive(AgentCompanionPhase::Thinking);
        assert_petization_ok(&thinking);
        assert_eq!(thinking.action, PetDirectiveAction::Observe);

        let running = agent_status_directive(AgentCompanionPhase::Running);
        assert_petization_ok(&running);
        assert_eq!(running.action, PetDirectiveAction::WorkBusy);
        assert_eq!(running.animation.as_deref(), Some("pet.work"));

        let waiting = agent_status_directive(AgentCompanionPhase::WaitingConfirmation);
        assert_petization_ok(&waiting);
        assert_eq!(waiting.action, PetDirectiveAction::Perch);

        let completed = agent_status_directive(AgentCompanionPhase::Completed);
        assert_petization_ok(&completed);
        assert_eq!(completed.action, PetDirectiveAction::Celebrate);

        let failed = agent_status_directive(AgentCompanionPhase::Failed);
        assert_petization_ok(&failed);
        assert_eq!(failed.action, PetDirectiveAction::Rest);

        let cancelled = agent_status_directive(AgentCompanionPhase::Cancelled);
        assert_petization_ok(&cancelled);
        assert_eq!(cancelled.action, PetDirectiveAction::Rest);

        let started = auto_mode_directive(AutoModePetEvent::Started);
        assert_petization_ok(&started);
        assert_eq!(started.action, PetDirectiveAction::WorkBusy);

        let step_ok = auto_mode_directive(AutoModePetEvent::StepOk);
        assert_petization_ok(&step_ok);
        assert_eq!(step_ok.action, PetDirectiveAction::Observe);

        let paused = auto_mode_directive(AutoModePetEvent::StepPaused);
        assert_petization_ok(&paused);
        assert_eq!(paused.action, PetDirectiveAction::Perch);

        let budget = auto_mode_directive(AutoModePetEvent::Budget);
        assert_petization_ok(&budget);
        assert_eq!(budget.action, PetDirectiveAction::Rest);

        let done = auto_mode_directive(AutoModePetEvent::Completed);
        assert_petization_ok(&done);
        assert_eq!(done.action, PetDirectiveAction::Celebrate);

        let crashed = auto_mode_directive(AutoModePetEvent::Crashed);
        assert_petization_ok(&crashed);
        assert_eq!(crashed.action, PetDirectiveAction::WorkCrash);
    }

    #[test]
    fn meeting_sensory_directive_active_and_cleared() {
        let zoom = meeting_sensory_directive(true, Some("zoom"));
        assert_petization_ok(&zoom);
        assert_eq!(zoom.action, PetDirectiveAction::Rest);
        assert_eq!(zoom.attention, AttentionFocus::IdleScene);
        assert_eq!(zoom.animation.as_deref(), Some("pet.idle"));
        assert!(zoom.speech.as_deref().unwrap_or("").contains("Zoom"));

        let teams = meeting_sensory_directive(true, Some("Teams"));
        assert_petization_ok(&teams);
        assert!(teams.speech.as_deref().unwrap_or("").contains("Teams"));

        let meet = meeting_sensory_directive(true, Some("meet"));
        assert_petization_ok(&meet);
        assert!(meet.speech.as_deref().unwrap_or("").contains("Meet"));

        let webex = meeting_sensory_directive(true, Some("webex"));
        assert_petization_ok(&webex);
        assert!(webex.speech.as_deref().unwrap_or("").contains("Webex"));

        let unknown = meeting_sensory_directive(true, Some("unknown"));
        assert_petization_ok(&unknown);
        assert_eq!(unknown.action, PetDirectiveAction::Rest);
        assert!(unknown.speech.as_deref().unwrap_or("").contains("通话"));

        let blank = meeting_sensory_directive(true, Some("   "));
        assert_petization_ok(&blank);
        assert_eq!(blank.speech.as_deref(), Some("会议中，我安静陪着"));

        let generic = meeting_sensory_directive(true, None);
        assert_petization_ok(&generic);
        assert_eq!(generic.action, PetDirectiveAction::Rest);
        assert_eq!(generic.speech.as_deref(), Some("会议中，我安静陪着"));

        let cleared = meeting_sensory_directive(false, Some("zoom"));
        assert_petization_ok(&cleared);
        assert_eq!(cleared.action, PetDirectiveAction::Observe);
        assert_eq!(cleared.attention, AttentionFocus::User);
        assert_eq!(cleared.animation.as_deref(), Some("pet.observe"));
        assert_eq!(cleared.speech.as_deref(), Some("会议结束了，我回来啦"));
        assert_eq!(cleared.mood_delta, Some(MoodDelta { mood: 2 }));
    }

    #[test]
    fn animation_whitelist_accepts_micro_performance_tokens() {
        for token in [
            "pet.yawn",
            "pet.dig_nose",
            "pet.count_ants",
            "pet.wave",
            "pet.look_around",
            "pet.hop",
            // existing tokens stay valid
            "pet.idle",
            "pet.observe",
            "pet.celebrate",
            "pet.click",
        ] {
            assert!(
                ANIMATION_TOKEN_WHITELIST.contains(&token),
                "expected whitelist to include {token}"
            );
            let mut directive =
                StructuredPetDirective::new(PetDirectiveAction::Play, AttentionFocus::User);
            directive.animation = Some(token.to_owned());
            assert_eq!(
                directive.validate(),
                Ok(()),
                "whitelist token should validate: {token}"
            );
        }
    }

    #[test]
    fn animation_whitelist_rejects_garbage_tokens() {
        for token in [
            "",
            "pet.unknown_dance",
            "freeform",
            "yawn",
            "pet.YAWN",
            "javascript:alert(1)",
            &"x".repeat(MAX_ANIMATION_TOKEN_LEN + 1),
        ] {
            let mut directive =
                StructuredPetDirective::new(PetDirectiveAction::Play, AttentionFocus::User);
            directive.animation = Some(token.to_owned());
            assert_eq!(
                directive.validate(),
                Err(BehaviorError::InvalidAnimationToken),
                "garbage token should be rejected: {token:?}"
            );
        }
    }

    #[test]
    fn notification_sensory_directive_unread_and_clear() {
        let unread = notification_sensory_directive(true);
        assert_petization_ok(&unread);
        assert_eq!(unread.action, PetDirectiveAction::Observe);
        assert_eq!(unread.attention, AttentionFocus::NotificationArea);
        assert!(
            unread.animation.as_deref() == Some("pet.look_around")
                || unread.animation.as_deref() == Some("pet.observe")
        );
        assert!(unread.speech.as_deref().unwrap_or("").contains("未读")
            || unread.speech.as_deref().unwrap_or("").contains("消息"));
        assert_eq!(unread.mood_delta, Some(MoodDelta { mood: 1 }));

        let clear = notification_sensory_directive(false);
        assert_petization_ok(&clear);
        assert_eq!(clear.action, PetDirectiveAction::Rest);
        assert_eq!(clear.attention, AttentionFocus::IdleScene);
        assert_eq!(clear.animation.as_deref(), Some("pet.idle"));
        assert!(clear.speech.as_deref().unwrap_or("").len() > 0);
        assert_eq!(clear.mood_delta, None);
    }
}
