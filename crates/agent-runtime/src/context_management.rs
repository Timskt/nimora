use crate::{
    ProviderMessage, ProviderMessageRole, ProviderRequest, ReasoningMapping, ToolDescriptor,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use thiserror::Error;

const MAX_ANCHOR_TEXT_BYTES: usize = 64 * 1024;
const MAX_ANCHOR_ITEMS: usize = 256;
const MAX_CACHE_ENTRIES: usize = 256;
const MAX_CACHE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextAnchor {
    pub goal: String,
    pub constraints: Vec<String>,
    pub pending_steps: Vec<String>,
    pub evidence: Vec<String>,
    pub workspace_fingerprint: String,
    pub plan_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningMapping>,
}

impl ContextAnchor {
    fn validate(&self) -> Result<(), ContextManagementError> {
        let items = self
            .constraints
            .len()
            .saturating_add(self.pending_steps.len())
            .saturating_add(self.evidence.len());
        let bytes = self
            .constraints
            .iter()
            .chain(&self.pending_steps)
            .chain(&self.evidence)
            .try_fold(
                self.goal.len() + self.workspace_fingerprint.len(),
                |total, item| total.checked_add(item.len()),
            );
        if self.goal.trim().is_empty()
            || self.workspace_fingerprint.trim().is_empty()
            || self.plan_revision == 0
            || items > MAX_ANCHOR_ITEMS
            || bytes.is_none_or(|bytes| bytes > MAX_ANCHOR_TEXT_BYTES)
            || self
                .constraints
                .iter()
                .chain(&self.pending_steps)
                .chain(&self.evidence)
                .any(|item| item.trim().is_empty())
        {
            return Err(ContextManagementError::InvalidAnchor);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextCompactionPolicy {
    pub max_messages: usize,
    pub max_content_bytes: usize,
    pub retain_recent_units: usize,
}

impl ContextCompactionPolicy {
    fn validate(self) -> Result<Self, ContextManagementError> {
        if self.max_messages < 2
            || self.max_messages > 256
            || self.max_content_bytes < 1_024
            || self.max_content_bytes > 256 * 1024
            || self.retain_recent_units == 0
            || self.retain_recent_units > 128
        {
            return Err(ContextManagementError::InvalidPolicy);
        }
        Ok(self)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactedContext {
    pub spec: String,
    pub cache_key: String,
    pub source_digest: String,
    pub provider_id: String,
    pub model: String,
    pub workspace_fingerprint: String,
    pub plan_revision: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reasoning: Option<ReasoningMapping>,
    pub messages: Vec<ProviderMessage>,
    pub source_message_count: usize,
    pub retained_message_count: usize,
    pub dropped_message_count: usize,
    pub created_at_ms: u64,
}

impl CompactedContext {
    /// Revalidates the content-addressed identity and protocol accounting.
    ///
    /// # Errors
    ///
    /// Returns an error when persisted or transported context was modified.
    pub fn validate(&self) -> Result<(), ContextManagementError> {
        let expected_key = context_cache_key(
            &self.provider_id,
            &self.model,
            &self.workspace_fingerprint,
            self.plan_revision,
            self.reasoning.as_ref(),
            &self.messages,
        )?;
        if self.spec != "nimora.compacted-context/1"
            || self.cache_key != expected_key
            || !valid_digest(&self.source_digest)
            || self.messages.len() != self.retained_message_count
            || self.source_message_count
                != self
                    .dropped_message_count
                    .saturating_add(self.retained_message_count.saturating_sub(1))
        {
            return Err(ContextManagementError::InvalidCacheEntry);
        }
        Ok(())
    }
}

#[derive(Debug, Default)]
pub struct ContextCompactor;

impl ContextCompactor {
    /// Compacts complete Provider protocol units while preserving trusted system instructions.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid source protocol, anchors, policies, or an insufficient budget.
    #[allow(clippy::too_many_arguments)]
    pub fn compact(
        &self,
        task_id: uuid::Uuid,
        trace_id: uuid::Uuid,
        provider_id: &str,
        model: &str,
        source: &[ProviderMessage],
        tools: &[ToolDescriptor],
        anchor: &ContextAnchor,
        policy: ContextCompactionPolicy,
        created_at_ms: u64,
    ) -> Result<CompactedContext, ContextManagementError> {
        anchor.validate()?;
        let policy = policy.validate()?;
        ProviderRequest::new(
            task_id,
            trace_id,
            provider_id,
            model,
            source.to_vec(),
            tools.to_vec(),
            1,
        )
        .map_err(|_| ContextManagementError::InvalidSourceProtocol)?;
        let source_digest = digest_json(source)?;
        let anchor_message = ProviderMessage::text(
            ProviderMessageRole::System,
            serde_json::to_string(&serde_json::json!({
                "spec": "nimora.context-anchor/1",
                "goal": anchor.goal,
                "constraints": anchor.constraints,
                "pendingSteps": anchor.pending_steps,
                "evidence": anchor.evidence,
                "workspaceFingerprint": anchor.workspace_fingerprint,
                "planRevision": anchor.plan_revision,
                "sourceDigest": source_digest,
            }))
            .map_err(|_| ContextManagementError::InvalidAnchor)?,
            crate::DataClassification::Internal,
            true,
        );
        let system_messages = source
            .iter()
            .filter(|message| message.role == ProviderMessageRole::System)
            .cloned()
            .collect::<Vec<_>>();
        let units = protocol_units(source)?;
        let mut selected = Vec::<Vec<ProviderMessage>>::new();
        let mut retained_units = 0_usize;
        for unit in units.into_iter().rev() {
            if unit[0].role == ProviderMessageRole::System {
                continue;
            }
            if retained_units >= policy.retain_recent_units {
                break;
            }
            selected.push(unit);
            retained_units = retained_units.saturating_add(1);
        }
        selected.reverse();
        loop {
            let mut messages = system_messages.clone();
            messages.push(anchor_message.clone());
            messages.extend(selected.iter().flatten().cloned());
            if within_policy(&messages, policy)
                && ProviderRequest::new(
                    task_id,
                    trace_id,
                    provider_id,
                    model,
                    messages.clone(),
                    tools.to_vec(),
                    1,
                )
                .is_ok()
            {
                let cache_key = context_cache_key(
                    provider_id,
                    model,
                    &anchor.workspace_fingerprint,
                    anchor.plan_revision,
                    anchor.reasoning.as_ref(),
                    &messages,
                )?;
                return Ok(CompactedContext {
                    spec: "nimora.compacted-context/1".to_owned(),
                    cache_key,
                    source_digest,
                    provider_id: provider_id.to_owned(),
                    model: model.to_owned(),
                    workspace_fingerprint: anchor.workspace_fingerprint.clone(),
                    plan_revision: anchor.plan_revision,
                    reasoning: anchor.reasoning.clone(),
                    source_message_count: source.len(),
                    retained_message_count: messages.len(),
                    dropped_message_count: source
                        .len()
                        .saturating_sub(messages.len().saturating_sub(1)),
                    messages,
                    created_at_ms,
                });
            }
            if selected.is_empty() {
                return Err(ContextManagementError::InsufficientBudget);
            }
            selected.remove(0);
        }
    }
}

fn protocol_units(
    messages: &[ProviderMessage],
) -> Result<Vec<Vec<ProviderMessage>>, ContextManagementError> {
    let mut units = Vec::new();
    let mut index = 0_usize;
    while index < messages.len() {
        let message = &messages[index];
        if message.role == ProviderMessageRole::Assistant && !message.tool_calls.is_empty() {
            let end = index
                .checked_add(message.tool_calls.len())
                .and_then(|value| value.checked_add(1))
                .ok_or(ContextManagementError::InvalidSourceProtocol)?;
            if end > messages.len()
                || messages[index + 1..end]
                    .iter()
                    .any(|result| result.role != ProviderMessageRole::Tool)
            {
                return Err(ContextManagementError::InvalidSourceProtocol);
            }
            units.push(messages[index..end].to_vec());
            index = end;
        } else if message.role == ProviderMessageRole::Tool {
            return Err(ContextManagementError::InvalidSourceProtocol);
        } else {
            units.push(vec![message.clone()]);
            index = index.saturating_add(1);
        }
    }
    Ok(units)
}

fn within_policy(messages: &[ProviderMessage], policy: ContextCompactionPolicy) -> bool {
    messages.len() <= policy.max_messages
        && messages
            .iter()
            .try_fold(0_usize, |total, message| {
                total.checked_add(message.content.len())
            })
            .is_some_and(|bytes| bytes <= policy.max_content_bytes)
}

fn context_cache_key(
    provider_id: &str,
    model: &str,
    workspace_fingerprint: &str,
    plan_revision: u64,
    reasoning: Option<&ReasoningMapping>,
    messages: &[ProviderMessage],
) -> Result<String, ContextManagementError> {
    digest_json(&(
        provider_id,
        model,
        workspace_fingerprint,
        plan_revision,
        reasoning,
        messages,
    ))
}

fn digest_json<T: Serialize + ?Sized>(value: &T) -> Result<String, ContextManagementError> {
    let encoded =
        serde_json::to_vec(value).map_err(|_| ContextManagementError::InvalidSourceProtocol)?;
    Ok(format!("sha256:{:x}", Sha256::digest(encoded)))
}

fn valid_digest(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64
            && digest
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

#[derive(Debug)]
struct CachedContext {
    value: CompactedContext,
    expires_at_ms: u64,
    bytes: usize,
    last_access: u64,
}

#[derive(Debug)]
pub struct ContextCache {
    max_entries: usize,
    max_bytes: usize,
    current_bytes: usize,
    access_clock: u64,
    entries: BTreeMap<String, CachedContext>,
}

impl ContextCache {
    /// Creates a bounded in-memory content-addressed cache.
    ///
    /// # Errors
    ///
    /// Returns an error for zero or excessive limits.
    pub fn new(max_entries: usize, max_bytes: usize) -> Result<Self, ContextManagementError> {
        if max_entries == 0
            || max_entries > MAX_CACHE_ENTRIES
            || !(1_024..=MAX_CACHE_BYTES).contains(&max_bytes)
        {
            return Err(ContextManagementError::InvalidCachePolicy);
        }
        Ok(Self {
            max_entries,
            max_bytes,
            current_bytes: 0,
            access_clock: 0,
            entries: BTreeMap::new(),
        })
    }

    /// Inserts one immutable context and evicts least-recently-used entries as needed.
    ///
    /// # Errors
    ///
    /// Returns an error for expired or oversized entries.
    pub fn insert(
        &mut self,
        value: CompactedContext,
        expires_at_ms: u64,
    ) -> Result<(), ContextManagementError> {
        value.validate()?;
        if expires_at_ms <= value.created_at_ms {
            return Err(ContextManagementError::InvalidCacheEntry);
        }
        let bytes = serde_json::to_vec(&value)
            .map_err(|_| ContextManagementError::InvalidCacheEntry)?
            .len();
        if bytes > self.max_bytes {
            return Err(ContextManagementError::InvalidCacheEntry);
        }
        if let Some(previous) = self.entries.remove(&value.cache_key) {
            self.current_bytes = self.current_bytes.saturating_sub(previous.bytes);
        }
        self.access_clock = self.access_clock.saturating_add(1);
        self.current_bytes = self.current_bytes.saturating_add(bytes);
        self.entries.insert(
            value.cache_key.clone(),
            CachedContext {
                value,
                expires_at_ms,
                bytes,
                last_access: self.access_clock,
            },
        );
        self.evict();
        Ok(())
    }

    #[must_use]
    pub fn get(&mut self, key: &str, now_ms: u64) -> Option<&CompactedContext> {
        if self
            .entries
            .get(key)
            .is_some_and(|entry| entry.expires_at_ms <= now_ms)
        {
            let removed = self.entries.remove(key)?;
            self.current_bytes = self.current_bytes.saturating_sub(removed.bytes);
            return None;
        }
        self.access_clock = self.access_clock.saturating_add(1);
        let entry = self.entries.get_mut(key)?;
        entry.last_access = self.access_clock;
        Some(&entry.value)
    }

    pub fn invalidate(&mut self, key: &str) -> bool {
        let Some(entry) = self.entries.remove(key) else {
            return false;
        };
        self.current_bytes = self.current_bytes.saturating_sub(entry.bytes);
        true
    }

    fn evict(&mut self) {
        while self.entries.len() > self.max_entries || self.current_bytes > self.max_bytes {
            let Some(key) = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_access)
                .map(|(key, _)| key.clone())
            else {
                break;
            };
            let _ = self.invalidate(&key);
        }
    }
}

#[derive(Debug, Error)]
pub enum ContextManagementError {
    #[error("context anchor is invalid")]
    InvalidAnchor,
    #[error("context compaction policy is invalid")]
    InvalidPolicy,
    #[error("source Provider message protocol is invalid")]
    InvalidSourceProtocol,
    #[error("context budget cannot preserve mandatory instructions and anchor")]
    InsufficientBudget,
    #[error("context cache policy is invalid")]
    InvalidCachePolicy,
    #[error("context cache entry is invalid")]
    InvalidCacheEntry,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        DataClassification, ProviderFinishReason, ProviderResponse, ProviderToolCall,
        ProviderUsage, ToolId,
    };
    use serde_json::json;
    use uuid::Uuid;

    fn anchor() -> ContextAnchor {
        ContextAnchor {
            goal: "finish safely".to_owned(),
            constraints: vec!["never bypass approval".to_owned()],
            pending_steps: vec!["run tests".to_owned()],
            evidence: vec!["compile passed".to_owned()],
            workspace_fingerprint: "sha256:workspace".to_owned(),
            plan_revision: 2,
            reasoning: None,
        }
    }

    #[test]
    fn preserves_system_messages_and_atomic_tool_pairs() {
        let call = ProviderToolCall {
            id: "call:1".to_owned(),
            tool_id: "core.state.read".parse::<ToolId>().expect("tool"),
            arguments: json!({}),
        };
        let response = ProviderResponse {
            spec: "nimora.agent-provider-response/1".to_owned(),
            request_id: Uuid::now_v7(),
            content: String::new(),
            tool_calls: vec![call.clone()],
            finish_reason: ProviderFinishReason::ToolCalls,
            usage: ProviderUsage {
                input_tokens: 1,
                output_tokens: 1,
                cost_microunits: 0,
            },
        };
        let source = vec![
            ProviderMessage::text(
                ProviderMessageRole::System,
                "system",
                DataClassification::Internal,
                true,
            ),
            ProviderMessage::text(
                ProviderMessageRole::User,
                "old",
                DataClassification::Personal,
                true,
            ),
            ProviderMessage::assistant_tool_calls(&response),
            ProviderMessage::tool_result(&call, &json!({"ok": true})).expect("result"),
            ProviderMessage::text(
                ProviderMessageRole::Assistant,
                "latest",
                DataClassification::Personal,
                false,
            ),
        ];
        let tool = ToolDescriptor::new(
            "core.state.read",
            "Read",
            "Reads state",
            json!({"type": "object"}),
            json!({"type": "object"}),
            nimora_runtime_core::CommandRisk::Safe,
            crate::ToolEffect::ReadOnly,
        )
        .expect("descriptor");
        let compacted = ContextCompactor
            .compact(
                Uuid::now_v7(),
                Uuid::now_v7(),
                "provider:local",
                "model:local",
                &source,
                &[tool],
                &anchor(),
                ContextCompactionPolicy {
                    max_messages: 5,
                    max_content_bytes: 16 * 1024,
                    retain_recent_units: 2,
                },
                1_000,
            )
            .expect("compact");
        assert_eq!(compacted.messages[0].role, ProviderMessageRole::System);
        assert_eq!(compacted.messages[1].role, ProviderMessageRole::System);
        assert_eq!(compacted.messages[2].role, ProviderMessageRole::Assistant);
        assert_eq!(compacted.messages[3].role, ProviderMessageRole::Tool);
        assert_eq!(compacted.messages[4].content, "latest");
    }

    #[test]
    fn cache_is_content_addressed_expires_and_evicts_lru() {
        let make = |suffix: &str| {
            let messages = vec![ProviderMessage::text(
                ProviderMessageRole::System,
                suffix,
                DataClassification::Internal,
                true,
            )];
            let cache_key = context_cache_key(
                "provider:local",
                "model:local",
                "sha256:workspace",
                1,
                None,
                &messages,
            )
            .expect("key");
            CompactedContext {
                spec: "nimora.compacted-context/1".to_owned(),
                cache_key,
                source_digest: format!("sha256:{}", "a".repeat(64)),
                provider_id: "provider:local".to_owned(),
                model: "model:local".to_owned(),
                workspace_fingerprint: "sha256:workspace".to_owned(),
                plan_revision: 1,
                reasoning: None,
                messages,
                source_message_count: 0,
                retained_message_count: 1,
                dropped_message_count: 0,
                created_at_ms: 1_000,
            }
        };
        let mut cache = ContextCache::new(2, 16 * 1024).expect("cache");
        let one = make("one");
        let one_key = one.cache_key.clone();
        let two = make("two");
        let two_key = two.cache_key.clone();
        let three = make("three");
        let mut forged = make("forged");
        forged.cache_key = format!("sha256:{}", "0".repeat(64));
        assert!(cache.insert(forged, 2_000).is_err());
        cache.insert(one, 2_000).expect("one");
        cache.insert(two, 2_000).expect("two");
        assert!(cache.get(&one_key, 1_500).is_some());
        cache.insert(three, 2_000).expect("three");
        assert!(cache.get(&two_key, 1_500).is_none());
        assert!(cache.get(&one_key, 2_000).is_none());
    }

    #[test]
    fn cache_identity_separates_reasoning_effort_and_mapping_version() {
        let source = vec![ProviderMessage::text(
            ProviderMessageRole::User,
            "Inspect the workspace",
            DataClassification::Personal,
            false,
        )];
        let policy = ContextCompactionPolicy {
            max_messages: 8,
            max_content_bytes: 8 * 1024,
            retain_recent_units: 2,
        };
        let mut low_anchor = anchor();
        low_anchor.reasoning = Some(
            ReasoningMapping::new(
                crate::ReasoningEffort::Low,
                crate::ReasoningEffort::Low,
                "low",
                "mock-mapping/1",
            )
            .expect("low mapping"),
        );
        let mut high_anchor = low_anchor.clone();
        high_anchor.reasoning = Some(
            ReasoningMapping::new(
                crate::ReasoningEffort::High,
                crate::ReasoningEffort::High,
                "high",
                "mock-mapping/1",
            )
            .expect("high mapping"),
        );
        let mut version_anchor = low_anchor.clone();
        version_anchor.reasoning = Some(
            ReasoningMapping::new(
                crate::ReasoningEffort::Low,
                crate::ReasoningEffort::Low,
                "low",
                "mock-mapping/2",
            )
            .expect("versioned mapping"),
        );
        let compact = |anchor: &ContextAnchor| {
            ContextCompactor
                .compact(
                    Uuid::now_v7(),
                    Uuid::now_v7(),
                    "provider:local",
                    "model:local",
                    &source,
                    &[],
                    anchor,
                    policy,
                    1_000,
                )
                .expect("compact")
        };
        let low = compact(&low_anchor);
        let high = compact(&high_anchor);
        let versioned = compact(&version_anchor);
        assert_ne!(low.cache_key, high.cache_key);
        assert_ne!(low.cache_key, versioned.cache_key);
        low.validate().expect("low context remains valid");
    }
}
