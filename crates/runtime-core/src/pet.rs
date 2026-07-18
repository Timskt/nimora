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

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetKeepsake {
    FirstHello,
    CaringHands,
    TrustedCompanion,
    HundredMoments,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetRelationshipStage {
    NewlyMet,
    Familiar,
    Trusted,
    Kindred,
    Lifelong,
}

impl PetRelationshipStage {
    pub const ALL: [(Self, u64); 5] = [
        (Self::NewlyMet, 0),
        (Self::Familiar, 25),
        (Self::Trusted, 100),
        (Self::Kindred, 300),
        (Self::Lifelong, 1_000),
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PetRelationshipSnapshot {
    pub bond_points: u64,
    pub affinity: u8,
    pub level: u64,
    pub level_progress: u64,
    pub points_per_level: u64,
    pub stage: PetRelationshipStage,
    pub next_stage: Option<PetRelationshipStage>,
    pub next_stage_at: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetItemId {
    BerryBite,
    StarBall,
    BubbleSoap,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PetInventoryStack {
    pub item_id: PetItemId,
    pub quantity: u16,
}

fn starter_inventory() -> Vec<PetInventoryStack> {
    vec![
        PetInventoryStack {
            item_id: PetItemId::BerryBite,
            quantity: 3,
        },
        PetInventoryStack {
            item_id: PetItemId::StarBall,
            quantity: 3,
        },
        PetInventoryStack {
            item_id: PetItemId::BubbleSoap,
            quantity: 3,
        },
    ]
}

impl PetKeepsake {
    pub const ALL: [(Self, u64); 4] = [
        (Self::FirstHello, 1),
        (Self::CaringHands, 25),
        (Self::TrustedCompanion, 50),
        (Self::HundredMoments, 100),
    ];
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PetVitalsPolicy {
    Full,
    Simple,
    Off,
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
    #[serde(default)]
    pub home_position: Option<Position>,
    pub energy: u8,
    pub mood: u8,
    #[serde(default = "default_need_level")]
    pub satiety: u8,
    #[serde(default = "default_need_level")]
    pub cleanliness: u8,
    pub affinity: u8,
    #[serde(default)]
    pub bond_points: u64,
    #[serde(default)]
    pub keepsakes: Vec<PetKeepsake>,
    #[serde(default = "starter_inventory")]
    pub inventory: Vec<PetInventoryStack>,
    #[serde(default)]
    pub last_vitals_update_ms: u64,
    #[serde(default)]
    pub last_care_ms: u64,
    #[serde(default)]
    pub last_item_use_ms: u64,
    #[serde(default)]
    pub autonomy: PetAutonomyState,
}

const fn default_need_level() -> u8 {
    100
}

impl Pet {
    const LOW_VITAL_THRESHOLD: u8 = 25;
    const AUTONOMY_REST_ENERGY_GAIN: u8 = 8;
    const SLEEP_ENERGY_GAIN_PER_INTERVAL: u64 = 2;
    pub const MAX_BOND_POINTS: u64 = 9_007_199_254_740_991;
    pub const BOND_POINTS_PER_LEVEL: u64 = 50;

    fn idle_emotion(&self) -> Emotion {
        if self.energy <= Self::LOW_VITAL_THRESHOLD {
            Emotion::Sleepy
        } else if self.mood <= Self::LOW_VITAL_THRESHOLD
            || self.satiety <= Self::LOW_VITAL_THRESHOLD
            || self.cleanliness <= Self::LOW_VITAL_THRESHOLD
        {
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
        let name = Self::normalize_name(name)?;
        Ok(Self {
            id: PetId::new(),
            name,
            state: PetState::Idle,
            emotion: Emotion::Neutral,
            position: Position { x: 0.0, y: 0.0 },
            home_position: Some(Position { x: 0.0, y: 0.0 }),
            energy: 100,
            mood: 70,
            satiety: 100,
            cleanliness: 100,
            affinity: 0,
            bond_points: 0,
            keepsakes: Vec::new(),
            inventory: starter_inventory(),
            last_vitals_update_ms: 0,
            last_care_ms: 0,
            last_item_use_ms: 0,
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

    /// Migrates a legacy pet by treating its last durable position as home.
    pub fn ensure_home_position(&mut self) -> bool {
        if self.home_position.is_some() {
            return false;
        }
        self.home_position = Some(self.position);
        true
    }

    /// Saves a finite desktop coordinate as the companion's home.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidPosition`] when either coordinate is NaN or infinite.
    pub fn set_home(&mut self, position: Position) -> Result<(), PetError> {
        position.validate()?;
        self.home_position = Some(position);
        Ok(())
    }

    /// Moves the companion to a host-validated visible position while preserving its home anchor.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidPosition`] for a non-finite position and
    /// [`PetError::InvalidTransition`] while the companion is being dragged.
    pub fn return_home_to(&mut self, position: Position) -> Result<(), PetError> {
        if self.state == PetState::Dragged {
            return Err(PetError::InvalidTransition);
        }
        self.move_to(position)?;
        self.enter_idle();
        Ok(())
    }

    /// Replaces the companion name after applying the same normalization as creation.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidName`] when the normalized name is empty or exceeds 64
    /// Unicode scalar values.
    pub fn rename(&mut self, name: impl Into<String>) -> Result<(), PetError> {
        self.name = Self::normalize_name(name)?;
        Ok(())
    }

    /// Normalizes and validates a companion name for domain and native host surfaces.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidName`] when the trimmed name is empty or exceeds 64
    /// Unicode scalar values.
    pub fn normalize_name(name: impl Into<String>) -> Result<String, PetError> {
        let name = name.into().trim().to_owned();
        if name.is_empty() || name.chars().count() > 64 {
            return Err(PetError::InvalidName);
        }
        Ok(name)
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
        self.add_bond_points(1);
        self.affinity = self.affinity.saturating_add(1).min(100);
        Ok(())
    }

    /// Applies one deliberate, host-recognized petting gesture.
    ///
    /// # Errors
    ///
    /// Returns [`PetError::InvalidTransition`] while a drag is active.
    pub fn stroke(&mut self) -> Result<(), PetError> {
        if self.state == PetState::Dragged {
            return Err(PetError::InvalidTransition);
        }
        self.state = PetState::Interacting;
        self.emotion = Emotion::Happy;
        self.mood = self.mood.saturating_add(4).min(100);
        self.add_bond_points(2);
        self.affinity = self.affinity.saturating_add(2).min(100);
        Ok(())
    }

    #[must_use]
    pub fn effective_bond_points(&self) -> u64 {
        self.bond_points.max(u64::from(self.affinity))
    }

    #[must_use]
    pub fn relationship_level(&self) -> u64 {
        self.effective_bond_points()
            .saturating_div(Self::BOND_POINTS_PER_LEVEL)
            .saturating_add(1)
    }

    #[must_use]
    pub fn relationship_level_progress(&self) -> u64 {
        self.effective_bond_points() % Self::BOND_POINTS_PER_LEVEL
    }

    #[must_use]
    pub fn relationship(&self) -> PetRelationshipSnapshot {
        let bond_points = self.effective_bond_points();
        let stage_index = PetRelationshipStage::ALL
            .partition_point(|(_, threshold)| *threshold <= bond_points)
            .saturating_sub(1);
        let stage = PetRelationshipStage::ALL[stage_index].0;
        let next = PetRelationshipStage::ALL.get(stage_index + 1).copied();
        PetRelationshipSnapshot {
            bond_points,
            affinity: self.affinity,
            level: self.relationship_level(),
            level_progress: self.relationship_level_progress(),
            points_per_level: Self::BOND_POINTS_PER_LEVEL,
            stage,
            next_stage: next.map(|(stage, _)| stage),
            next_stage_at: next.map(|(_, threshold)| threshold),
        }
    }

    fn add_bond_points(&mut self, points: u64) {
        self.bond_points = self
            .effective_bond_points()
            .saturating_add(points)
            .min(Self::MAX_BOND_POINTS);
        self.unlock_keepsakes();
    }

    fn unlock_keepsakes(&mut self) {
        let bond_points = self.effective_bond_points();
        for (keepsake, threshold) in PetKeepsake::ALL {
            if bond_points >= threshold && !self.keepsakes.contains(&keepsake) {
                self.keepsakes.push(keepsake);
            }
        }
        self.keepsakes.sort_unstable();
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
        if let Some(home_position) = self.home_position {
            home_position.validate()?;
        }
        if self.energy > 100
            || self.mood > 100
            || self.satiety > 100
            || self.cleanliness > 100
            || self.affinity > 100
            || self.bond_points > Self::MAX_BOND_POINTS
        {
            return Err(PetError::InvalidVitals);
        }
        if self.keepsakes.windows(2).any(|pair| pair[0] >= pair[1]) {
            return Err(PetError::InvalidCollection);
        }
        if self
            .inventory
            .iter()
            .any(|stack| stack.quantity == 0 || stack.quantity > 999)
            || self
                .inventory
                .windows(2)
                .any(|pair| pair[0].item_id >= pair[1].item_id)
        {
            return Err(PetError::InvalidInventory);
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

    pub fn update_vitals(
        &mut self,
        policy: PetVitalsPolicy,
        now_ms: u64,
        interval_ms: u64,
        max_intervals: u64,
    ) {
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
        if policy == PetVitalsPolicy::Off {
            self.last_vitals_update_ms = now_ms;
            return;
        }
        let energy_delta = if self.state == PetState::Sleeping {
            intervals.saturating_mul(Self::SLEEP_ENERGY_GAIN_PER_INTERVAL)
        } else {
            intervals
        };
        let energy_delta = u8::try_from(energy_delta.min(u64::from(u8::MAX))).unwrap_or(u8::MAX);
        let mood_loss = u8::try_from((intervals / 3).min(u64::from(u8::MAX))).unwrap_or(u8::MAX);
        let satiety_loss = u8::try_from((intervals / 2).min(u64::from(u8::MAX))).unwrap_or(u8::MAX);
        let cleanliness_loss =
            u8::try_from((intervals / 4).min(u64::from(u8::MAX))).unwrap_or(u8::MAX);
        self.energy = if self.state == PetState::Sleeping {
            self.energy.saturating_add(energy_delta).min(100)
        } else {
            self.energy.saturating_sub(energy_delta)
        };
        self.mood = self.mood.saturating_sub(mood_loss);
        if policy == PetVitalsPolicy::Full {
            self.satiety = self.satiety.saturating_sub(satiety_loss);
            self.cleanliness = self.cleanliness.saturating_sub(cleanliness_loss);
        }
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
                self.satiety = self.satiety.saturating_add(25).min(100);
                self.mood = self.mood.saturating_add(2).min(100);
                self.add_bond_points(1);
                self.affinity = self.affinity.saturating_add(1).min(100);
            }
            PetCareAction::Play => {
                self.energy = self.energy.saturating_sub(5);
                self.satiety = self.satiety.saturating_sub(3);
                self.cleanliness = self.cleanliness.saturating_sub(2);
                self.mood = self.mood.saturating_add(12).min(100);
                self.add_bond_points(2);
                self.affinity = self.affinity.saturating_add(2).min(100);
            }
            PetCareAction::Groom => {
                self.cleanliness = self.cleanliness.saturating_add(30).min(100);
                self.mood = self.mood.saturating_add(6).min(100);
                self.add_bond_points(3);
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

    /// Consumes one owned item and applies its bounded local effect.
    ///
    /// # Errors
    ///
    /// Returns an error while dragged, during cooldown, or when the item is not owned.
    pub fn use_item(
        &mut self,
        item_id: PetItemId,
        now_ms: u64,
        cooldown_ms: u64,
    ) -> Result<(), PetError> {
        if self.state == PetState::Dragged {
            return Err(PetError::InvalidTransition);
        }
        if self.last_item_use_ms != 0 && now_ms.saturating_sub(self.last_item_use_ms) < cooldown_ms
        {
            return Err(PetError::ItemCooldown);
        }
        let index = self
            .inventory
            .binary_search_by_key(&item_id, |stack| stack.item_id)
            .map_err(|_| PetError::ItemUnavailable)?;
        match item_id {
            PetItemId::BerryBite => {
                self.energy = self.energy.saturating_add(30).min(100);
                self.satiety = self.satiety.saturating_add(35).min(100);
                self.mood = self.mood.saturating_add(3).min(100);
                self.add_bond_points(1);
                self.affinity = self.affinity.saturating_add(1).min(100);
            }
            PetItemId::StarBall => {
                self.energy = self.energy.saturating_sub(3);
                self.satiety = self.satiety.saturating_sub(2);
                self.cleanliness = self.cleanliness.saturating_sub(2);
                self.mood = self.mood.saturating_add(18).min(100);
                self.add_bond_points(3);
                self.affinity = self.affinity.saturating_add(3).min(100);
            }
            PetItemId::BubbleSoap => {
                self.cleanliness = self.cleanliness.saturating_add(45).min(100);
                self.mood = self.mood.saturating_add(8).min(100);
                self.add_bond_points(3);
                self.affinity = self.affinity.saturating_add(3).min(100);
            }
        }
        self.inventory[index].quantity -= 1;
        if self.inventory[index].quantity == 0 {
            self.inventory.remove(index);
        }
        self.last_item_use_ms = now_ms;
        self.state = PetState::Interacting;
        self.emotion = Emotion::Happy;
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
        } else if self.mood <= Self::LOW_VITAL_THRESHOLD
            || self.satiety <= Self::LOW_VITAL_THRESHOLD
            || self.cleanliness <= Self::LOW_VITAL_THRESHOLD
        {
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
                if self.autonomy.active_intent == Some(PetIntent::Rest) {
                    self.energy = self
                        .energy
                        .saturating_add(Self::AUTONOMY_REST_ENERGY_GAIN)
                        .min(100);
                }
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
    #[error("pet keepsake collection must be sorted and unique")]
    InvalidCollection,
    #[error("pet inventory must be sorted, unique, and contain quantities between 1 and 999")]
    InvalidInventory,
    #[error("pet item is not available")]
    ItemUnavailable,
    #[error("pet item use is cooling down")]
    ItemCooldown,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rename_normalizes_and_rejects_invalid_names_without_mutation() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.rename("  Mochi  ").expect("rename");
        assert_eq!(pet.name, "Mochi");
        assert_eq!(pet.rename("   "), Err(PetError::InvalidName));
        assert_eq!(pet.name, "Mochi");
        assert_eq!(pet.rename("灵".repeat(65)), Err(PetError::InvalidName));
        assert_eq!(pet.name, "Mochi");
    }

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
        pet.update_vitals(PetVitalsPolicy::Full, 1_000, 100, 6);
        assert_eq!(pet.energy, 100);
        pet.update_vitals(PetVitalsPolicy::Full, 11_000, 100, 6);
        assert_eq!(pet.energy, 94);
        assert_eq!(pet.mood, 68);
        assert_eq!(pet.satiety, 97);
        assert_eq!(pet.cleanliness, 99);
        assert_eq!(pet.last_vitals_update_ms, 11_000);
    }

    #[test]
    fn manual_sleep_restores_energy_while_other_needs_keep_evolving() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.energy = 40;
        pet.update_vitals(PetVitalsPolicy::Full, 1_000, 100, 6);
        pet.apply_action(PetAction::Sleep);
        pet.update_vitals(PetVitalsPolicy::Full, 11_000, 100, 6);
        assert_eq!(pet.energy, 52);
        assert_eq!((pet.mood, pet.satiety, pet.cleanliness), (68, 97, 99));

        pet.energy = 99;
        pet.update_vitals(PetVitalsPolicy::Full, 21_000, 100, 6);
        assert_eq!(pet.energy, 100);
    }

    #[test]
    fn simplified_and_disabled_vitals_preserve_user_choice() {
        let mut simple = Pet::new("Aster").expect("valid pet");
        simple.update_vitals(PetVitalsPolicy::Simple, 1_000, 100, 6);
        simple.update_vitals(PetVitalsPolicy::Simple, 11_000, 100, 6);
        assert_eq!((simple.energy, simple.mood), (94, 68));
        assert_eq!((simple.satiety, simple.cleanliness), (100, 100));

        let mut disabled = Pet::new("Aster").expect("valid pet");
        disabled.update_vitals(PetVitalsPolicy::Off, 1_000, 100, 6);
        disabled.update_vitals(PetVitalsPolicy::Off, 11_000, 100, 6);
        assert_eq!(
            (
                disabled.energy,
                disabled.mood,
                disabled.satiety,
                disabled.cleanliness
            ),
            (100, 70, 100, 100)
        );
        assert_eq!(disabled.last_vitals_update_ms, 11_000);
        disabled.update_vitals(PetVitalsPolicy::Full, 11_100, 100, 6);
        assert_eq!(disabled.energy, 99);
    }

    #[test]
    fn completed_autonomous_rest_has_a_bounded_domain_effect() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.energy = 20;
        let policy = PetAutonomyPolicy::default();
        pet.apply_autonomy_decision(
            PetAutonomyDecision::Start {
                intent: PetIntent::Rest,
                action: PetAction::Sleep,
            },
            policy,
            100,
        );
        pet.apply_autonomy_decision(PetAutonomyDecision::Finish, policy, 200);
        assert_eq!(pet.energy, 28);
        assert_eq!(pet.state, PetState::Idle);

        pet.energy = 98;
        pet.apply_autonomy_decision(
            PetAutonomyDecision::Start {
                intent: PetIntent::Rest,
                action: PetAction::Sleep,
            },
            policy,
            300,
        );
        pet.apply_autonomy_decision(PetAutonomyDecision::Finish, policy, 400);
        assert_eq!(pet.energy, 100);
    }

    #[test]
    fn care_actions_restore_real_needs_without_punitive_overflow() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.energy = 70;
        pet.satiety = 20;
        pet.cleanliness = 20;
        pet.care(PetCareAction::Feed, 1_000, 0).expect("feed");
        assert_eq!((pet.energy, pet.satiety), (90, 45));
        pet.care(PetCareAction::Groom, 2_000, 0).expect("groom");
        assert_eq!(pet.cleanliness, 50);
        pet.care(PetCareAction::Play, 3_000, 0).expect("play");
        assert_eq!((pet.energy, pet.satiety, pet.cleanliness), (85, 42, 48));
    }

    #[test]
    fn legacy_snapshot_defaults_new_care_needs_to_full() {
        let pet = Pet::new("Aster").expect("valid pet");
        let mut value = serde_json::to_value(pet).expect("serialize pet");
        let object = value.as_object_mut().expect("pet object");
        object.remove("satiety");
        object.remove("cleanliness");
        let restored: Pet = serde_json::from_value(value).expect("legacy pet");
        assert_eq!((restored.satiety, restored.cleanliness), (100, 100));
    }

    #[test]
    fn inventory_starts_with_and_migrates_to_the_local_starter_pack() {
        let pet = Pet::new("Aster").expect("valid pet");
        assert_eq!(pet.inventory, starter_inventory());

        let mut value = serde_json::to_value(pet).expect("serialize pet");
        let object = value.as_object_mut().expect("pet object");
        object.remove("inventory");
        object.remove("lastItemUseMs");
        let restored: Pet = serde_json::from_value(value).expect("legacy pet");
        assert_eq!(restored.inventory, starter_inventory());
        assert_eq!(restored.last_item_use_ms, 0);
    }

    #[test]
    fn owned_items_apply_distinct_bounded_effects() {
        let mut berry = Pet::new("Aster").expect("valid pet");
        berry.energy = 60;
        berry.satiety = 50;
        berry
            .use_item(PetItemId::BerryBite, 1_000, 0)
            .expect("berry");
        assert_eq!(
            (
                berry.energy,
                berry.satiety,
                berry.mood,
                berry.affinity,
                berry.bond_points
            ),
            (90, 85, 73, 1, 1)
        );
        assert_eq!(berry.inventory[0].quantity, 2);

        let mut ball = Pet::new("Aster").expect("valid pet");
        ball.use_item(PetItemId::StarBall, 1_000, 0).expect("ball");
        assert_eq!(
            (
                ball.energy,
                ball.satiety,
                ball.cleanliness,
                ball.mood,
                ball.affinity,
                ball.bond_points
            ),
            (97, 98, 98, 88, 3, 3)
        );

        let mut soap = Pet::new("Aster").expect("valid pet");
        soap.cleanliness = 40;
        soap.use_item(PetItemId::BubbleSoap, 1_000, 0)
            .expect("soap");
        assert_eq!(
            (soap.cleanliness, soap.mood, soap.affinity, soap.bond_points),
            (85, 78, 3, 3)
        );
    }

    #[test]
    fn item_exhaustion_cooldown_and_drag_rejections_preserve_inventory() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.inventory[0].quantity = 1;
        pet.use_item(PetItemId::BerryBite, 1_000, 5_000)
            .expect("first use");
        assert!(
            !pet.inventory
                .iter()
                .any(|stack| stack.item_id == PetItemId::BerryBite)
        );
        let inventory = pet.inventory.clone();
        assert_eq!(
            pet.use_item(PetItemId::StarBall, 2_000, 5_000),
            Err(PetError::ItemCooldown)
        );
        assert_eq!(pet.inventory, inventory);
        assert_eq!(
            pet.use_item(PetItemId::BerryBite, 7_000, 5_000),
            Err(PetError::ItemUnavailable)
        );

        pet.begin_drag().expect("drag");
        let inventory = pet.inventory.clone();
        assert_eq!(
            pet.use_item(PetItemId::StarBall, 8_000, 0),
            Err(PetError::InvalidTransition)
        );
        assert_eq!(pet.inventory, inventory);
    }

    #[test]
    fn inventory_validation_rejects_invalid_quantity_order_and_duplicates() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.inventory[0].quantity = 0;
        assert_eq!(pet.validate(), Err(PetError::InvalidInventory));
        pet.inventory[0].quantity = 1_000;
        assert_eq!(pet.validate(), Err(PetError::InvalidInventory));

        pet.inventory = starter_inventory();
        pet.inventory.swap(0, 1);
        assert_eq!(pet.validate(), Err(PetError::InvalidInventory));
        pet.inventory = vec![
            PetInventoryStack {
                item_id: PetItemId::BerryBite,
                quantity: 1,
            },
            PetInventoryStack {
                item_id: PetItemId::BerryBite,
                quantity: 2,
            },
        ];
        assert_eq!(pet.validate(), Err(PetError::InvalidInventory));
    }

    #[test]
    fn interaction_improves_mood_and_affinity_without_overflow() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.mood = 99;
        pet.affinity = 100;
        pet.interact().expect("interaction");
        assert_eq!(pet.mood, 100);
        assert_eq!(pet.affinity, 100);
        assert_eq!(pet.bond_points, 101);
    }

    #[test]
    fn stroke_is_a_distinct_bounded_companionship_interaction() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.mood = 98;
        pet.stroke().expect("stroke");
        assert_eq!((pet.mood, pet.affinity, pet.bond_points), (100, 2, 2));

        pet.begin_drag().expect("drag");
        assert_eq!(pet.stroke(), Err(PetError::InvalidTransition));
        assert_eq!(pet.bond_points, 2);
    }

    #[test]
    fn companionship_growth_is_durable_unbounded_and_migration_safe() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        assert_eq!((pet.bond_points, pet.relationship_level()), (0, 1));

        pet.affinity = 34;
        pet.interact().expect("interaction");
        assert_eq!(pet.bond_points, 35);

        pet.affinity = 100;
        pet.bond_points = 149;
        pet.interact().expect("interaction");
        assert_eq!(pet.bond_points, 150);
        assert_eq!(pet.relationship_level(), 4);
        assert_eq!(pet.relationship_level_progress(), 0);

        pet.bond_points = Pet::MAX_BOND_POINTS;
        pet.interact().expect("saturating interaction");
        assert_eq!(pet.bond_points, Pet::MAX_BOND_POINTS);
        assert_eq!(pet.relationship_level(), Pet::MAX_BOND_POINTS / 50 + 1);
    }

    #[test]
    fn relationship_level_uses_fifty_point_boundaries() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        for (points, level, progress) in [(0, 1, 0), (49, 1, 49), (50, 2, 0)] {
            pet.bond_points = points;
            assert_eq!(pet.relationship_level(), level);
            assert_eq!(pet.relationship_level_progress(), progress);
        }
    }

    #[test]
    fn relationship_stage_uses_stable_bond_boundaries() {
        let mut pet = Pet::new("Mori").expect("pet");
        for (points, stage, next_stage, next_stage_at) in [
            (
                0,
                PetRelationshipStage::NewlyMet,
                Some(PetRelationshipStage::Familiar),
                Some(25),
            ),
            (
                24,
                PetRelationshipStage::NewlyMet,
                Some(PetRelationshipStage::Familiar),
                Some(25),
            ),
            (
                25,
                PetRelationshipStage::Familiar,
                Some(PetRelationshipStage::Trusted),
                Some(100),
            ),
            (
                99,
                PetRelationshipStage::Familiar,
                Some(PetRelationshipStage::Trusted),
                Some(100),
            ),
            (
                100,
                PetRelationshipStage::Trusted,
                Some(PetRelationshipStage::Kindred),
                Some(300),
            ),
            (
                299,
                PetRelationshipStage::Trusted,
                Some(PetRelationshipStage::Kindred),
                Some(300),
            ),
            (
                300,
                PetRelationshipStage::Kindred,
                Some(PetRelationshipStage::Lifelong),
                Some(1_000),
            ),
            (
                999,
                PetRelationshipStage::Kindred,
                Some(PetRelationshipStage::Lifelong),
                Some(1_000),
            ),
            (1_000, PetRelationshipStage::Lifelong, None, None),
            (
                Pet::MAX_BOND_POINTS,
                PetRelationshipStage::Lifelong,
                None,
                None,
            ),
        ] {
            pet.bond_points = points;
            let relationship = pet.relationship();
            assert_eq!(
                (
                    relationship.stage,
                    relationship.next_stage,
                    relationship.next_stage_at
                ),
                (stage, next_stage, next_stage_at)
            );
        }
    }

    #[test]
    fn relationship_projection_preserves_legacy_affinity_baseline() {
        let mut pet = Pet::new("Mori").expect("pet");
        pet.affinity = 84;
        let relationship = pet.relationship();
        assert_eq!(
            (
                relationship.bond_points,
                relationship.level,
                relationship.level_progress
            ),
            (84, 2, 34)
        );
        assert_eq!(relationship.stage, PetRelationshipStage::Familiar);
    }

    #[test]
    fn companionship_unlocks_ordered_keepsakes_across_thresholds() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.interact().expect("first hello");
        assert_eq!(pet.keepsakes, [PetKeepsake::FirstHello]);

        pet.bond_points = 24;
        pet.interact().expect("care milestone");
        assert_eq!(
            pet.keepsakes,
            [PetKeepsake::FirstHello, PetKeepsake::CaringHands]
        );

        pet.bond_points = 99;
        pet.interact().expect("hundred moments");
        assert_eq!(
            pet.keepsakes,
            PetKeepsake::ALL.map(|(keepsake, _)| keepsake)
        );
    }

    #[test]
    fn legacy_collection_defaults_empty_and_rejects_duplicate_ownership() {
        let pet = Pet::new("Aster").expect("valid pet");
        let mut value = serde_json::to_value(&pet).expect("serialize pet");
        value
            .as_object_mut()
            .expect("pet object")
            .remove("keepsakes");
        let restored: Pet = serde_json::from_value(value).expect("legacy pet remains readable");
        assert!(restored.keepsakes.is_empty());

        let mut corrupt = pet;
        corrupt.keepsakes = vec![PetKeepsake::FirstHello, PetKeepsake::FirstHello];
        assert_eq!(corrupt.validate(), Err(PetError::InvalidCollection));
    }

    #[test]
    fn legacy_snapshot_defaults_bond_points_to_affinity_baseline() {
        let pet = Pet::new("Aster").expect("valid pet");
        let mut value = serde_json::to_value(pet).expect("serialize pet");
        value
            .as_object_mut()
            .expect("pet object")
            .remove("bondPoints");
        value["affinity"] = serde_json::json!(34);

        let restored: Pet = serde_json::from_value(value).expect("legacy snapshot");
        assert_eq!(restored.bond_points, 0);
        assert_eq!(restored.effective_bond_points(), 34);
        assert_eq!(restored.relationship_level(), 1);
    }

    #[test]
    fn care_actions_apply_distinct_bounded_vital_changes() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.energy = 50;
        pet.mood = 50;
        pet.care(PetCareAction::Feed, 1_000, 30_000).expect("feed");
        assert_eq!((pet.energy, pet.mood, pet.affinity), (70, 52, 1));
        assert_eq!(pet.bond_points, 1);
        assert_eq!(pet.state, PetState::Interacting);
        assert_eq!(
            pet.care(PetCareAction::Play, 2_000, 30_000),
            Err(PetError::CareCooldown)
        );
        pet.care(PetCareAction::Play, 31_000, 30_000).expect("play");
        assert_eq!((pet.energy, pet.mood, pet.affinity), (65, 64, 3));
        assert_eq!(pet.bond_points, 3);
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
    fn home_anchor_is_independent_from_current_position_and_rejects_drag_return() {
        let mut pet = Pet::new("Aster").expect("valid pet");
        pet.set_home(Position { x: 120.0, y: 80.0 })
            .expect("set home");
        pet.move_to(Position { x: 480.0, y: 320.0 })
            .expect("move away");
        assert_eq!(pet.home_position, Some(Position { x: 120.0, y: 80.0 }));
        pet.return_home_to(Position { x: 120.0, y: 80.0 })
            .expect("return home");
        assert_eq!(pet.position, Position { x: 120.0, y: 80.0 });

        pet.begin_drag().expect("drag");
        assert_eq!(
            pet.return_home_to(Position { x: 120.0, y: 80.0 }),
            Err(PetError::InvalidTransition)
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
