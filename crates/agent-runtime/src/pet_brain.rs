//! Structured "brain" output for the desktop lifeform's AI.
//!
//! Milestone 4 ("emergent intelligence") asks the AI to stop being a plain-text
//! chatbot and instead drive the pet as a subject: given the desktop context and
//! the pet's state, the model emits a structured *instruction* — what to say,
//! what mood to adopt, and which action to perform — rather than prose the
//! renderer would have to guess at.
//!
//! This module owns the two host-side halves of that contract that must be
//! deterministic and safe:
//!
//! 1. Parsing and validating the model's JSON into a [`PetBrainInstruction`],
//!    fail-closed: unknown actions, over-long speech, or malformed JSON are
//!    rejected rather than fed to the pet.
//! 2. Rendering the pet's [`PetPersonality`] into a system-prompt fragment so
//!    the model speaks in-character.
//!
//! It is pure and free of any provider, network, or Tauri dependency — it
//! operates on strings and the authoritative [`PetAction`] vocabulary from
//! `runtime-core` — so it unit-tests in isolation and stays within the
//! workspace architecture boundary.

use nimora_runtime_core::PetAction;
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Longest speech line the pet will voice, in bytes. Keeps a bubble readable and
/// bounds untrusted model output before it reaches the UI.
const MAX_SPEECH_BYTES: usize = 240;

/// The emotional register the pet adopts for an instruction. A closed set so an
/// unknown mood from the model fails closed rather than driving an undefined
/// expression.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PetMood {
    Neutral,
    Happy,
    Caring,
    Curious,
    Sleepy,
    Anxious,
    Proud,
}

/// A validated instruction from the pet's "brain": what to say, the mood to show,
/// and the action to perform. This is what the renderer consumes instead of raw
/// model text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PetBrainInstruction {
    /// The (validated, bounded) line the pet says. May be empty for a silent act.
    pub speech: String,
    /// The mood the pet adopts.
    pub mood: PetMood,
    /// The action the pet performs, from the authoritative `PetAction` vocabulary.
    pub action: PetAction,
}

/// The pet's personality dimensions, each `0..=100`. Injected into the system
/// prompt so the model's tone matches the character. Mirrors the behavior-engine
/// personality parameters (energy / curiosity / laziness / pride).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct PetPersonality {
    pub energy: u8,
    pub curiosity: u8,
    pub laziness: u8,
    pub pride: u8,
}

impl Default for PetPersonality {
    fn default() -> Self {
        Self {
            energy: 60,
            curiosity: 60,
            laziness: 40,
            pride: 50,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PetBrainError {
    #[error("brain output was not valid JSON for the instruction schema")]
    InvalidJson,
    #[error("brain speech exceeded the maximum length")]
    SpeechTooLong,
    #[error("brain personality value was out of the 0..=100 range")]
    PersonalityOutOfRange,
}

/// Parses and validates a model's JSON output into a [`PetBrainInstruction`].
///
/// The JSON must match the exact schema (`{ "speech", "mood", "action" }`) with
/// no unknown fields; `mood` and `action` must be members of their closed
/// vocabularies. Speech is trimmed and length-checked. Anything else fails
/// closed so malformed or adversarial model output never drives the pet.
///
/// # Errors
///
/// Returns [`PetBrainError::InvalidJson`] for malformed JSON, an unknown mood or
/// action, or unknown fields, and [`PetBrainError::SpeechTooLong`] when the
/// (trimmed) speech exceeds [`MAX_SPEECH_BYTES`].
pub fn parse_brain_instruction(raw: &str) -> Result<PetBrainInstruction, PetBrainError> {
    let mut instruction: PetBrainInstruction =
        serde_json::from_str(raw).map_err(|_| PetBrainError::InvalidJson)?;
    instruction.speech = instruction.speech.trim().to_owned();
    if instruction.speech.len() > MAX_SPEECH_BYTES {
        return Err(PetBrainError::SpeechTooLong);
    }
    Ok(instruction)
}

/// Renders a system-prompt fragment that locks the model into the pet's persona.
///
/// The fragment names each personality dimension with a coarse qualitative band
/// (rather than a raw number, which models follow poorly) and states the output
/// contract, so the model both speaks in-character and returns the structured
/// instruction this module can parse.
///
/// # Errors
///
/// Returns [`PetBrainError::PersonalityOutOfRange`] if any dimension exceeds 100
/// (values are `u8`, so only the upper bound can be violated).
pub fn personality_system_prompt(
    personality: &PetPersonality,
) -> Result<String, PetBrainError> {
    for value in [
        personality.energy,
        personality.curiosity,
        personality.laziness,
        personality.pride,
    ] {
        if value > 100 {
            return Err(PetBrainError::PersonalityOutOfRange);
        }
    }
    let energy = band(personality.energy);
    let curiosity = band(personality.curiosity);
    let laziness = band(personality.laziness);
    let pride = band(personality.pride);
    Ok(format!(
        "你是用户桌面上的一只桌面生命体宠物，不是助手。请始终以宠物的口吻和意图行动。\
         你的性格：精力{energy}、好奇心{curiosity}、懒惰{laziness}、自尊{pride}。\
         每次只输出一个 JSON 对象，形如 {{\"speech\": string, \"mood\": mood, \"action\": action}}，\
         不要输出多余文本。mood 取值：neutral/happy/caring/curious/sleepy/anxious/proud。\
         action 取值：idle/observe/walk/play/perch/climb/peek/stretch/sleep/work/celebrate。"
    ))
}

/// Maps a `0..=100` dimension to a coarse qualitative band models follow well.
fn band(value: u8) -> &'static str {
    match value {
        0..=19 => "很低",
        20..=39 => "偏低",
        40..=59 => "中等",
        60..=79 => "偏高",
        _ => "很高",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_well_formed_instruction() {
        let raw = r#"{"speech":"主人，该喝水了哦","mood":"caring","action":"walk"}"#;
        let instruction = parse_brain_instruction(raw).expect("valid instruction");
        assert_eq!(instruction.mood, PetMood::Caring);
        assert_eq!(instruction.action, PetAction::Walk);
        assert_eq!(instruction.speech, "主人，该喝水了哦");
    }

    #[test]
    fn trims_surrounding_whitespace_from_speech() {
        let raw = r#"{"speech":"  你好  ","mood":"happy","action":"celebrate"}"#;
        let instruction = parse_brain_instruction(raw).expect("valid instruction");
        assert_eq!(instruction.speech, "你好");
    }

    #[test]
    fn allows_empty_speech_for_a_silent_action() {
        let raw = r#"{"speech":"","mood":"sleepy","action":"sleep"}"#;
        let instruction = parse_brain_instruction(raw).expect("valid instruction");
        assert_eq!(instruction.action, PetAction::Sleep);
        assert!(instruction.speech.is_empty());
    }

    #[test]
    fn rejects_an_unknown_action() {
        let raw = r#"{"speech":"hi","mood":"happy","action":"teleport"}"#;
        assert_eq!(
            parse_brain_instruction(raw),
            Err(PetBrainError::InvalidJson)
        );
    }

    #[test]
    fn rejects_an_unknown_mood() {
        let raw = r#"{"speech":"hi","mood":"furious","action":"idle"}"#;
        assert_eq!(
            parse_brain_instruction(raw),
            Err(PetBrainError::InvalidJson)
        );
    }

    #[test]
    fn rejects_unknown_fields() {
        let raw = r#"{"speech":"hi","mood":"happy","action":"idle","tool":"shell"}"#;
        assert_eq!(
            parse_brain_instruction(raw),
            Err(PetBrainError::InvalidJson)
        );
    }

    #[test]
    fn rejects_malformed_json() {
        assert_eq!(
            parse_brain_instruction("not json"),
            Err(PetBrainError::InvalidJson)
        );
        assert_eq!(
            parse_brain_instruction(""),
            Err(PetBrainError::InvalidJson)
        );
    }

    #[test]
    fn rejects_over_long_speech() {
        let long = "字".repeat(200); // 200 chars * 3 bytes = 600 bytes > 240
        let raw = format!(r#"{{"speech":"{long}","mood":"happy","action":"idle"}}"#);
        assert_eq!(
            parse_brain_instruction(&raw),
            Err(PetBrainError::SpeechTooLong)
        );
    }

    #[test]
    fn instruction_round_trips_through_json() {
        let instruction = PetBrainInstruction {
            speech: "在这里".to_owned(),
            mood: PetMood::Proud,
            action: PetAction::Perch,
        };
        let json = serde_json::to_string(&instruction).expect("serialize");
        let parsed = parse_brain_instruction(&json).expect("round trip");
        assert_eq!(parsed, instruction);
    }

    #[test]
    fn personality_prompt_names_dimensions_and_output_contract() {
        let prompt = personality_system_prompt(&PetPersonality {
            energy: 90,
            curiosity: 10,
            laziness: 50,
            pride: 70,
        })
        .expect("valid personality");
        // Bands are rendered, not raw numbers.
        assert!(prompt.contains("精力很高"));
        assert!(prompt.contains("好奇心很低"));
        assert!(prompt.contains("懒惰中等"));
        assert!(prompt.contains("自尊偏高"));
        // The output contract is stated so the model returns parseable JSON.
        assert!(prompt.contains("speech"));
        assert!(prompt.contains("mood"));
        assert!(prompt.contains("action"));
    }

    #[test]
    fn default_personality_is_balanced_and_produces_a_prompt() {
        let prompt = personality_system_prompt(&PetPersonality::default()).expect("valid");
        assert!(!prompt.is_empty());
    }

    #[test]
    fn band_covers_the_full_range() {
        assert_eq!(band(0), "很低");
        assert_eq!(band(19), "很低");
        assert_eq!(band(20), "偏低");
        assert_eq!(band(50), "中等");
        assert_eq!(band(70), "偏高");
        assert_eq!(band(100), "很高");
    }
}
