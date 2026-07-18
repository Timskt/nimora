use crate::{AgentBudget, DataClassification, ToolId, valid_principal};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;
use thiserror::Error;
use uuid::Uuid;

const MAX_ROOTS: usize = 32;
const MAX_DOMAINS: usize = 128;
const MAX_TOOLS: usize = 256;
const MAX_MODELS: usize = 64;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SandboxScope {
    ReadOnly,
    WorkspaceWrite,
    SelectedRoots,
    FullDevice,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ApprovalPolicy {
    AlwaysAsk,
    AskRisky,
    AutoReview,
    NeverAskWithinGrant,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case", tag = "kind", content = "domains")]
pub enum NetworkPolicy {
    Offline,
    LoopbackOnly,
    Allowlisted(BTreeSet<String>),
    Unrestricted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GrantLifetime {
    OneAction,
    OneTurn,
    Session,
    UntilTimestamp,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AuthorizationGrant {
    pub spec: String,
    pub id: Uuid,
    pub goal_id: Uuid,
    pub plan_revision: u64,
    pub workspace_fingerprint: String,
    pub sandbox: SandboxScope,
    pub approval: ApprovalPolicy,
    pub network: NetworkPolicy,
    pub selected_roots: BTreeSet<String>,
    pub tool_allowlist: BTreeSet<ToolId>,
    pub provider_allowlist: BTreeSet<String>,
    pub model_allowlist: BTreeSet<String>,
    pub maximum_data_classification: DataClassification,
    pub budget: AgentBudget,
    pub lifetime: GrantLifetime,
    pub issued_at_ms: u64,
    pub expires_at_ms: Option<u64>,
    pub revoked_at_ms: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationRequest<'a> {
    pub goal_id: Uuid,
    pub plan_revision: u64,
    pub workspace_fingerprint: &'a str,
    pub tool_id: &'a ToolId,
    pub provider_id: &'a str,
    pub model: &'a str,
    pub data_classification: DataClassification,
    pub requires_network: bool,
    pub now_ms: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AuthorizationDecision {
    Authorized,
    ApprovalRequired,
}

impl AuthorizationGrant {
    /// Validates an immutable pre-authorization at its trust boundary.
    ///
    /// # Errors
    ///
    /// Returns an error when scope, identity, lifetime, or allowlists are unsafe.
    pub fn validate(&self) -> Result<(), AuthorizationError> {
        let timed = self.lifetime == GrantLifetime::UntilTimestamp;
        if self.spec != "nimora.authorization-grant/1"
            || self.plan_revision == 0
            || !valid_fingerprint(&self.workspace_fingerprint)
            || self.tool_allowlist.is_empty()
            || self.tool_allowlist.len() > MAX_TOOLS
            || self.provider_allowlist.is_empty()
            || self.provider_allowlist.len() > MAX_MODELS
            || self.model_allowlist.is_empty()
            || self.model_allowlist.len() > MAX_MODELS
            || self
                .provider_allowlist
                .iter()
                .any(|value| !valid_principal(value))
            || self
                .model_allowlist
                .iter()
                .any(|value| !valid_principal(value))
            || self.selected_roots.len() > MAX_ROOTS
            || self.selected_roots.iter().any(|root| !valid_root(root))
            || matches!(self.network, NetworkPolicy::Allowlisted(ref domains) if domains.is_empty() || domains.len() > MAX_DOMAINS || domains.iter().any(|domain| !valid_domain(domain)))
            || timed != self.expires_at_ms.is_some()
            || self
                .expires_at_ms
                .is_some_and(|expiry| expiry <= self.issued_at_ms)
            || self
                .revoked_at_ms
                .is_some_and(|revoked| revoked < self.issued_at_ms)
            || (self.sandbox == SandboxScope::SelectedRoots && self.selected_roots.is_empty())
            || (self.sandbox != SandboxScope::SelectedRoots && !self.selected_roots.is_empty())
        {
            return Err(AuthorizationError::InvalidGrant);
        }
        Ok(())
    }

    /// Evaluates exact Goal, Plan, Workspace, Provider, model, tool, data, and network binding.
    ///
    /// # Errors
    ///
    /// Returns a stable reason when a grant is invalid, expired, revoked, or out of scope.
    pub fn authorize(
        &self,
        request: &AuthorizationRequest<'_>,
    ) -> Result<AuthorizationDecision, AuthorizationError> {
        self.validate()?;
        if self.revoked_at_ms.is_some() {
            return Err(AuthorizationError::Revoked);
        }
        if self
            .expires_at_ms
            .is_some_and(|expiry| request.now_ms >= expiry)
        {
            return Err(AuthorizationError::Expired);
        }
        if self.goal_id != request.goal_id
            || self.plan_revision != request.plan_revision
            || self.workspace_fingerprint != request.workspace_fingerprint
        {
            return Err(AuthorizationError::BindingChanged);
        }
        if !self.tool_allowlist.contains(request.tool_id)
            || !self.provider_allowlist.contains(request.provider_id)
            || !self.model_allowlist.contains(request.model)
            || request.data_classification > self.maximum_data_classification
            || (request.requires_network && matches!(self.network, NetworkPolicy::Offline))
        {
            return Err(AuthorizationError::OutOfScope);
        }
        Ok(match self.approval {
            ApprovalPolicy::NeverAskWithinGrant => AuthorizationDecision::Authorized,
            _ => AuthorizationDecision::ApprovalRequired,
        })
    }

    #[must_use]
    /// Returns a canonical fingerprint for the immutable grant.
    ///
    /// # Panics
    ///
    /// Panics only if Serde cannot encode this fixed data-only structure.
    pub fn fingerprint(&self) -> String {
        let encoded = serde_json::to_vec(self).expect("authorization grant is serializable");
        format!("sha256:{:x}", Sha256::digest(encoded))
    }
}

fn valid_fingerprint(value: &str) -> bool {
    value.strip_prefix("sha256:").is_some_and(|digest| {
        digest.len() == 64 && digest.bytes().all(|byte| byte.is_ascii_hexdigit())
    })
}

fn valid_root(value: &str) -> bool {
    !value.trim().is_empty() && value.len() <= 1024 && !value.contains('\0')
}

fn valid_domain(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 253
        && !value.contains('/')
        && value
            .bytes()
            .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'-'))
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum AuthorizationError {
    #[error("authorization grant is invalid")]
    InvalidGrant,
    #[error("authorization grant was revoked")]
    Revoked,
    #[error("authorization grant expired")]
    Expired,
    #[error("authorization binding changed")]
    BindingChanged,
    #[error("operation is outside the authorization grant")]
    OutOfScope,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn grant() -> AuthorizationGrant {
        AuthorizationGrant {
            spec: "nimora.authorization-grant/1".to_owned(),
            id: Uuid::now_v7(),
            goal_id: Uuid::now_v7(),
            plan_revision: 2,
            workspace_fingerprint: format!("sha256:{}", "a".repeat(64)),
            sandbox: SandboxScope::WorkspaceWrite,
            approval: ApprovalPolicy::NeverAskWithinGrant,
            network: NetworkPolicy::Offline,
            selected_roots: BTreeSet::new(),
            tool_allowlist: BTreeSet::from(["core.state.read".parse().expect("tool")]),
            provider_allowlist: BTreeSet::from(["local".to_owned()]),
            model_allowlist: BTreeSet::from(["model".to_owned()]),
            maximum_data_classification: DataClassification::Internal,
            budget: AgentBudget::default(),
            lifetime: GrantLifetime::Session,
            issued_at_ms: 10,
            expires_at_ms: None,
            revoked_at_ms: None,
        }
    }

    #[test]
    fn exact_binding_can_run_without_another_prompt() {
        let grant = grant();
        let tool = "core.state.read".parse().expect("tool");
        let request = AuthorizationRequest {
            goal_id: grant.goal_id,
            plan_revision: grant.plan_revision,
            workspace_fingerprint: &grant.workspace_fingerprint,
            tool_id: &tool,
            provider_id: "local",
            model: "model",
            data_classification: DataClassification::Internal,
            requires_network: false,
            now_ms: 11,
        };
        assert_eq!(
            grant.authorize(&request).expect("authorize"),
            AuthorizationDecision::Authorized
        );
    }

    #[test]
    fn plan_change_invalidates_pre_authorization() {
        let grant = grant();
        let tool = "core.state.read".parse().expect("tool");
        let request = AuthorizationRequest {
            goal_id: grant.goal_id,
            plan_revision: 3,
            workspace_fingerprint: &grant.workspace_fingerprint,
            tool_id: &tool,
            provider_id: "local",
            model: "model",
            data_classification: DataClassification::Internal,
            requires_network: false,
            now_ms: 11,
        };
        assert_eq!(
            grant.authorize(&request),
            Err(AuthorizationError::BindingChanged)
        );
    }
}
