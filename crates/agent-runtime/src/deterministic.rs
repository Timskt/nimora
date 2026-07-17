use super::{
    ProviderAdapter, ProviderCapabilities, ProviderCapability, ProviderDescriptor, ProviderError,
    ProviderExecutionContext, ProviderFinishReason, ProviderLocality, ProviderMessageRole,
    ProviderRequest, ProviderResponse, ProviderUsage,
};
use std::collections::BTreeSet;

#[derive(Debug)]
pub struct DeterministicLocalProvider {
    descriptor: ProviderDescriptor,
}

impl DeterministicLocalProvider {
    /// Creates the built-in credential-free Provider used for offline diagnostics and tests.
    ///
    /// # Errors
    ///
    /// Returns an error only if the built-in descriptor violates the Provider contract.
    pub fn new() -> Result<Self, ProviderError> {
        Ok(Self {
            descriptor: ProviderDescriptor::new(
                "provider:deterministic-local",
                "Deterministic Local",
                ProviderLocality::Local,
                8_192,
                2_048,
                ProviderCapabilities {
                    supported: BTreeSet::from([
                        ProviderCapability::Cancellation,
                        ProviderCapability::UsageReporting,
                    ]),
                },
            )?,
        })
    }
}

impl ProviderAdapter for DeterministicLocalProvider {
    fn descriptor(&self) -> &ProviderDescriptor {
        &self.descriptor
    }

    fn complete(
        &self,
        request: &ProviderRequest,
        context: &ProviderExecutionContext,
    ) -> Result<ProviderResponse, ProviderError> {
        if context.cancellation.is_cancelled() {
            return Err(ProviderError::new(
                super::ProviderErrorKind::Cancelled,
                "provider request was cancelled",
            ));
        }
        let source_text = request
            .messages
            .iter()
            .rev()
            .find(|message| message.role == ProviderMessageRole::User)
            .map_or_else(String::new, |message| message.content.clone());
        let max_characters =
            usize::try_from(request.max_output_tokens.saturating_mul(4)).unwrap_or(usize::MAX);
        let response_text = source_text.chars().take(max_characters).collect::<String>();
        let finish_reason = if response_text.chars().count() < source_text.chars().count() {
            ProviderFinishReason::Length
        } else {
            ProviderFinishReason::Completed
        };
        let input_tokens = request
            .messages
            .iter()
            .map(|message| estimate_tokens(&message.content))
            .sum();
        let output_tokens = estimate_tokens(&response_text).min(request.max_output_tokens);
        Ok(ProviderResponse {
            spec: "nimora.agent-provider-response/1".to_owned(),
            request_id: request.request_id,
            content: response_text,
            tool_calls: Vec::new(),
            finish_reason,
            usage: ProviderUsage {
                input_tokens,
                output_tokens,
                cost_microunits: 0,
            },
        })
    }
}

fn estimate_tokens(content: &str) -> u64 {
    u64::try_from(content.chars().count())
        .unwrap_or(u64::MAX)
        .saturating_add(3)
        / 4
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CancellationFlag, DataClassification, ProviderMessage};
    use std::time::Duration;
    use uuid::Uuid;

    #[test]
    fn echoes_latest_user_message_without_credentials_or_network() {
        let provider = DeterministicLocalProvider::new().expect("provider");
        let request = ProviderRequest::new(
            Uuid::now_v7(),
            Uuid::now_v7(),
            provider.descriptor().id.clone(),
            "model:echo-v1",
            vec![ProviderMessage {
                role: ProviderMessageRole::User,
                content: "offline check".to_owned(),
                classification: DataClassification::Internal,
                trusted: true,
            }],
            Vec::new(),
            64,
        )
        .expect("request");
        let response = provider
            .complete(
                &request,
                &ProviderExecutionContext {
                    timeout: Duration::from_secs(1),
                    cancellation: CancellationFlag::default(),
                    credential_reference: None,
                },
            )
            .expect("response");
        assert_eq!(response.content, "offline check");
        assert_eq!(response.usage.cost_microunits, 0);
    }
}
