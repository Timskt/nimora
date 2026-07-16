use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::{fmt, str::FromStr};
use thiserror::Error;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct EventId(Uuid);

impl EventId {
    #[must_use]
    pub fn new() -> Self {
        Self(Uuid::now_v7())
    }
}

impl Default for EventId {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub enum EventSource {
    Core,
    Skill(String),
    Automation(String),
    Agent(String),
    Connector(String),
    Gateway(String),
    System(String),
}

impl fmt::Display for EventSource {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Core => formatter.write_str("core"),
            Self::Skill(id) => write!(formatter, "skill:{id}"),
            Self::Automation(id) => write!(formatter, "automation:{id}"),
            Self::Agent(id) => write!(formatter, "agent:{id}"),
            Self::Connector(id) => write!(formatter, "connector:{id}"),
            Self::Gateway(id) => write!(formatter, "gateway:{id}"),
            Self::System(id) => write!(formatter, "system:{id}"),
        }
    }
}

impl From<EventSource> for String {
    fn from(source: EventSource) -> Self {
        source.to_string()
    }
}

impl TryFrom<String> for EventSource {
    type Error = EventError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::from_str(&value)
    }
}

impl FromStr for EventSource {
    type Err = EventError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        if value == "core" {
            return Ok(Self::Core);
        }

        let (namespace, id) = value
            .split_once(':')
            .ok_or_else(|| EventError::InvalidSource(value.to_owned()))?;
        if id.is_empty() || id.contains(':') {
            return Err(EventError::InvalidSource(value.to_owned()));
        }

        match namespace {
            "skill" => Ok(Self::Skill(id.to_owned())),
            "automation" => Ok(Self::Automation(id.to_owned())),
            "agent" => Ok(Self::Agent(id.to_owned())),
            "connector" => Ok(Self::Connector(id.to_owned())),
            "gateway" => Ok(Self::Gateway(id.to_owned())),
            "system" => Ok(Self::System(id.to_owned())),
            _ => Err(EventError::InvalidSource(value.to_owned())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Event {
    pub spec: String,
    pub id: EventId,
    pub event_type: String,
    pub source: EventSource,
    #[serde(with = "time::serde::rfc3339")]
    pub timestamp: OffsetDateTime,
    pub trace_id: Uuid,
    pub data: Value,
}

impl Event {
    /// Creates a versioned domain event with fresh event and trace identifiers.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::InvalidType`] when the event type is not a
    /// lowercase, dot-separated identifier with at least three segments.
    pub fn new(
        event_type: impl Into<String>,
        source: EventSource,
        data: Value,
    ) -> Result<Self, EventError> {
        Self::with_trace_id(event_type, source, Uuid::now_v7(), data)
    }

    /// Creates a versioned event correlated with an existing execution trace.
    ///
    /// # Errors
    ///
    /// Returns [`EventError::InvalidType`] when the event type is invalid or
    /// [`EventError::InvalidTraceId`] when the supplied trace identifier is nil.
    pub fn with_trace_id(
        event_type: impl Into<String>,
        source: EventSource,
        trace_id: Uuid,
        data: Value,
    ) -> Result<Self, EventError> {
        let event_type = event_type.into();
        validate_event_type(&event_type)?;
        if trace_id.is_nil() {
            return Err(EventError::InvalidTraceId);
        }
        Ok(Self {
            spec: "asterpet.event/1".to_owned(),
            id: EventId::new(),
            event_type,
            source,
            timestamp: OffsetDateTime::now_utc(),
            trace_id,
            data,
        })
    }
}

fn validate_event_type(event_type: &str) -> Result<(), EventError> {
    let valid = !event_type.is_empty()
        && event_type.split('.').count() >= 3
        && event_type.split('.').all(|segment| {
            !segment.is_empty()
                && segment.chars().all(|character| {
                    character.is_ascii_lowercase() || character.is_ascii_digit() || character == '-'
                })
        });
    if valid {
        Ok(())
    } else {
        Err(EventError::InvalidType(event_type.to_owned()))
    }
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum EventError {
    #[error("invalid event source: {0}")]
    InvalidSource(String),
    #[error("invalid event type: {0}")]
    InvalidType(String),
    #[error("event trace id must not be nil")]
    InvalidTraceId,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn source_round_trips_through_wire_format() {
        let source = EventSource::Skill("dev.asterpet.timer".to_owned());
        let json = serde_json::to_string(&source).expect("source serializes");
        assert_eq!(json, r#""skill:dev.asterpet.timer""#);
        let decoded = serde_json::from_str::<EventSource>(&json).expect("source deserializes");
        assert_eq!(decoded, source);
    }

    #[test]
    fn rejects_unknown_or_nested_source_namespaces() {
        assert!("plugin:test".parse::<EventSource>().is_err());
        assert!("skill:a:b".parse::<EventSource>().is_err());
    }

    #[test]
    fn event_has_versioned_spec_and_trace() {
        let event = Event::new(
            "pet.state.changed",
            EventSource::Core,
            json!({"state": "idle"}),
        )
        .expect("event is valid");
        assert_eq!(event.spec, "asterpet.event/1");
        assert_eq!(event.event_type, "pet.state.changed");
        assert!(!event.trace_id.is_nil());
    }

    #[test]
    fn rejects_non_namespaced_event_type() {
        assert_eq!(
            Event::new("clicked", EventSource::Core, Value::Null),
            Err(EventError::InvalidType("clicked".to_owned()))
        );
    }

    #[test]
    fn preserves_an_existing_trace_id() {
        let trace_id = Uuid::now_v7();
        let event = Event::with_trace_id(
            "pet.position.changed",
            EventSource::Core,
            trace_id,
            Value::Null,
        )
        .expect("event");
        assert_eq!(event.trace_id, trace_id);
    }
}
