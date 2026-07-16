use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};
use thiserror::Error;
use uuid::Uuid;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct CommandId(String);

impl FromStr for CommandId {
    type Err = CommandError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let valid = value.split('.').count() >= 3
            && value.split('.').all(|segment| {
                !segment.is_empty()
                    && segment.chars().all(|character| {
                        character.is_ascii_lowercase()
                            || character.is_ascii_digit()
                            || character == '-'
                    })
            });
        valid
            .then(|| Self(value.to_owned()))
            .ok_or_else(|| CommandError::InvalidId(value.to_owned()))
    }
}

impl fmt::Display for CommandId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandRisk {
    Safe,
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CommandStatus {
    Pending,
    Running,
    Succeeded,
    Failed,
    Cancelled,
    TimedOut,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Command {
    pub spec: String,
    pub execution_id: Uuid,
    pub command_id: CommandId,
    pub trace_id: Uuid,
    pub arguments: Value,
    pub risk: CommandRisk,
    pub status: CommandStatus,
    pub idempotency_key: Option<String>,
}

impl Command {
    /// Creates a pending command execution request.
    ///
    /// # Errors
    ///
    /// Returns [`CommandError::InvalidId`] when the command identifier is not
    /// a lowercase, dot-separated identifier with at least three segments.
    pub fn new(
        command_id: impl AsRef<str>,
        arguments: Value,
        risk: CommandRisk,
    ) -> Result<Self, CommandError> {
        Ok(Self {
            spec: "asterpet.command/1".to_owned(),
            execution_id: Uuid::now_v7(),
            command_id: command_id.as_ref().parse()?,
            trace_id: Uuid::now_v7(),
            arguments,
            risk,
            status: CommandStatus::Pending,
            idempotency_key: None,
        })
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum CommandError {
    #[error("invalid command id: {0}")]
    InvalidId(String),
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn command_uses_versioned_contract() {
        let command = Command::new(
            "pet.animation.play",
            json!({"action": "idle"}),
            CommandRisk::Safe,
        )
        .expect("command is valid");
        assert_eq!(command.spec, "asterpet.command/1");
        assert_eq!(command.status, CommandStatus::Pending);
    }

    #[test]
    fn rejects_unqualified_command_id() {
        assert_eq!(
            "play".parse::<CommandId>(),
            Err(CommandError::InvalidId("play".to_owned()))
        );
    }
}
