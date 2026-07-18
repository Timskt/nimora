use std::collections::{BTreeMap, BTreeSet};

use nimora_agent_runtime::{ToolDescriptor, ToolEffect};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const CAPABILITY_CATALOG_SPEC: &str = "nimora.capability-catalog-snapshot/1";
pub const COMPOSITION_PLAN_SPEC: &str = "nimora.capability-composition-plan/1";
const MAX_CAPABILITIES: usize = 256;
const MAX_REQUESTED_CAPABILITIES: usize = 32;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CatalogCapabilityEffect {
    ReadOnly,
    ReversibleWrite,
    IrreversibleWrite,
    ExternalSideEffect,
}

impl From<ToolEffect> for CatalogCapabilityEffect {
    fn from(value: ToolEffect) -> Self {
        match value {
            ToolEffect::ReadOnly => Self::ReadOnly,
            ToolEffect::ReversibleWrite => Self::ReversibleWrite,
            ToolEffect::IrreversibleWrite => Self::IrreversibleWrite,
            ToolEffect::ExternalSideEffect => Self::ExternalSideEffect,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CatalogCapability {
    pub id: String,
    pub effect: CatalogCapabilityEffect,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityCatalogSnapshot {
    pub spec: String,
    pub digest: String,
    pub capabilities: Vec<CatalogCapability>,
}

impl CapabilityCatalogSnapshot {
    /// Projects a bounded, implementation-free Creator catalog from the live Tool Registry.
    ///
    /// # Errors
    ///
    /// Rejects duplicate identifiers or an oversized catalog.
    pub fn from_tool_descriptors(
        descriptors: impl IntoIterator<Item = ToolDescriptor>,
    ) -> Result<Self, CompositionError> {
        let mut capabilities = descriptors
            .into_iter()
            .map(|descriptor| CatalogCapability {
                id: descriptor.id.to_string(),
                effect: descriptor.effect.into(),
            })
            .collect::<Vec<_>>();
        capabilities.sort_by(|left, right| left.id.cmp(&right.id));
        if capabilities.is_empty() || capabilities.len() > MAX_CAPABILITIES {
            return Err(CompositionError::InvalidCatalog);
        }
        if capabilities.windows(2).any(|pair| pair[0].id == pair[1].id) {
            return Err(CompositionError::InvalidCatalog);
        }
        let canonical =
            serde_json::to_vec(&capabilities).map_err(|_| CompositionError::InvalidCatalog)?;
        let digest = format!("sha256:{:x}", Sha256::digest(canonical));
        Ok(Self {
            spec: CAPABILITY_CATALOG_SPEC.to_owned(),
            digest,
            capabilities,
        })
    }

    /// Serializes the implementation-free snapshot for a trusted Provider system message.
    ///
    /// # Errors
    ///
    /// Returns a stable catalog error if serialization fails.
    pub fn compact_prompt_slice(&self) -> Result<String, CompositionError> {
        serde_json::to_string(self).map_err(|_| CompositionError::InvalidCatalog)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityCompositionPlan {
    pub spec: String,
    pub catalog_digest: String,
    pub requested_capabilities: Vec<String>,
    pub resolved_capabilities: Vec<String>,
    pub missing_capabilities: Vec<String>,
    pub fully_resolved: bool,
}

/// Deterministically resolves exact capability identifiers against one immutable snapshot.
///
/// # Errors
///
/// Rejects empty, duplicate, malformed, or oversized requests.
pub fn plan_exact_capabilities(
    snapshot: &CapabilityCatalogSnapshot,
    requested: impl IntoIterator<Item = String>,
) -> Result<CapabilityCompositionPlan, CompositionError> {
    validate_snapshot(snapshot)?;
    let requested_capabilities = requested.into_iter().collect::<Vec<_>>();
    if requested_capabilities.is_empty()
        || requested_capabilities.len() > MAX_REQUESTED_CAPABILITIES
        || requested_capabilities
            .iter()
            .any(|id| !valid_capability_id(id))
        || requested_capabilities.iter().collect::<BTreeSet<_>>().len()
            != requested_capabilities.len()
    {
        return Err(CompositionError::InvalidRequest);
    }
    let catalog = snapshot
        .capabilities
        .iter()
        .map(|capability| (capability.id.as_str(), capability))
        .collect::<BTreeMap<_, _>>();
    let mut resolved_capabilities = Vec::new();
    let mut missing_capabilities = Vec::new();
    for capability in &requested_capabilities {
        if catalog.contains_key(capability.as_str()) {
            resolved_capabilities.push(capability.clone());
        } else {
            missing_capabilities.push(capability.clone());
        }
    }
    Ok(CapabilityCompositionPlan {
        spec: COMPOSITION_PLAN_SPEC.to_owned(),
        catalog_digest: snapshot.digest.clone(),
        requested_capabilities,
        fully_resolved: missing_capabilities.is_empty(),
        resolved_capabilities,
        missing_capabilities,
    })
}

fn validate_snapshot(snapshot: &CapabilityCatalogSnapshot) -> Result<(), CompositionError> {
    if snapshot.spec != CAPABILITY_CATALOG_SPEC
        || snapshot.capabilities.is_empty()
        || snapshot.capabilities.len() > MAX_CAPABILITIES
        || snapshot
            .capabilities
            .windows(2)
            .any(|pair| pair[0].id >= pair[1].id)
        || snapshot
            .capabilities
            .iter()
            .any(|capability| !valid_capability_id(&capability.id))
    {
        return Err(CompositionError::InvalidCatalog);
    }
    let canonical =
        serde_json::to_vec(&snapshot.capabilities).map_err(|_| CompositionError::InvalidCatalog)?;
    let digest = format!("sha256:{:x}", Sha256::digest(canonical));
    (snapshot.digest == digest)
        .then_some(())
        .ok_or(CompositionError::InvalidCatalog)
}

fn valid_capability_id(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').count() >= 3
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CompositionError {
    #[error("capability catalog snapshot is invalid")]
    InvalidCatalog,
    #[error("capability composition request is invalid")]
    InvalidRequest,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_runtime_core::CommandRisk;
    use serde_json::json;

    fn descriptor(id: &str, effect: ToolEffect) -> ToolDescriptor {
        ToolDescriptor::new(
            id,
            id,
            format!("Capability {id}"),
            json!({"type": "object"}),
            json!({"type": "object"}),
            CommandRisk::Safe,
            effect,
        )
        .expect("descriptor")
    }

    #[test]
    fn snapshot_is_sorted_bounded_and_stable() {
        let snapshot = CapabilityCatalogSnapshot::from_tool_descriptors([
            descriptor("pet.position.move", ToolEffect::ReversibleWrite),
            descriptor("pet.state.read", ToolEffect::ReadOnly),
        ])
        .expect("snapshot");
        assert_eq!(snapshot.capabilities[0].id, "pet.position.move");
        assert_eq!(snapshot.capabilities[1].id, "pet.state.read");
        assert!(snapshot.digest.starts_with("sha256:"));
        assert!(
            !snapshot
                .compact_prompt_slice()
                .expect("prompt slice")
                .contains("inputSchema")
        );
        assert!(
            !snapshot
                .compact_prompt_slice()
                .expect("prompt slice")
                .contains("Capability pet.state.read")
        );
    }

    #[test]
    fn planner_separates_registered_and_missing_exact_ids() {
        let snapshot = CapabilityCatalogSnapshot::from_tool_descriptors([
            descriptor("pet.state.read", ToolEffect::ReadOnly),
            descriptor("pet.animation.play", ToolEffect::ReversibleWrite),
        ])
        .expect("snapshot");
        let plan = plan_exact_capabilities(
            &snapshot,
            [
                "pet.state.read".to_owned(),
                "perception.camera.observe".to_owned(),
            ],
        )
        .expect("plan");
        assert_eq!(plan.resolved_capabilities, ["pet.state.read"]);
        assert_eq!(plan.missing_capabilities, ["perception.camera.observe"]);
        assert!(!plan.fully_resolved);
    }

    #[test]
    fn planner_rejects_duplicate_or_tampered_inputs() {
        let mut snapshot = CapabilityCatalogSnapshot::from_tool_descriptors([descriptor(
            "pet.state.read",
            ToolEffect::ReadOnly,
        )])
        .expect("snapshot");
        assert_eq!(
            plan_exact_capabilities(
                &snapshot,
                ["pet.state.read".to_owned(), "pet.state.read".to_owned()]
            ),
            Err(CompositionError::InvalidRequest)
        );
        snapshot.digest.push('0');
        assert_eq!(
            plan_exact_capabilities(&snapshot, ["pet.state.read".to_owned()]),
            Err(CompositionError::InvalidCatalog)
        );
    }
}
