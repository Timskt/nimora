use std::collections::{BTreeMap, BTreeSet};

use nimora_agent_runtime::{ToolDescriptor, ToolEffect};
use nimora_capability_contract::{
    CapabilityDataClass, CapabilityEffect, CapabilitySemanticContract, valid_precondition_id,
    valid_semantic_type_id, validate_capability_semantic_contract,
};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use thiserror::Error;

pub const CAPABILITY_CATALOG_SPEC: &str = "nimora.capability-catalog-snapshot/1";
pub const COMPOSITION_PLAN_SPEC: &str = "nimora.capability-composition-plan/1";
pub const COMPOSITION_GRAPH_SPEC: &str = "nimora.capability-composition-graph/1";
pub const SEMANTIC_PLAN_SPEC: &str = "nimora.capability-semantic-plan/1";
const MAX_CAPABILITIES: usize = 256;
const MAX_REQUESTED_CAPABILITIES: usize = 32;
const MAX_GRAPH_DEPTH: usize = 8;
const MAX_EXPANDED_STATES: usize = 2_048;
const MAX_SEMANTIC_REQUEST_ITEMS: usize = 32;

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

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilityCompositionGraph {
    pub spec: String,
    pub digest: String,
    pub contracts: Vec<CapabilitySemanticContract>,
}

impl CapabilityCompositionGraph {
    /// Builds an immutable graph from host-validated semantic contracts.
    ///
    /// # Errors
    ///
    /// Rejects empty, oversized, invalid, or duplicate capability declarations.
    pub fn new(
        contracts: impl IntoIterator<Item = CapabilitySemanticContract>,
    ) -> Result<Self, CompositionError> {
        let mut contracts = contracts.into_iter().collect::<Vec<_>>();
        contracts.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
        if contracts.is_empty()
            || contracts.len() > MAX_CAPABILITIES
            || contracts
                .windows(2)
                .any(|pair| pair[0].capability_id == pair[1].capability_id)
            || contracts
                .iter()
                .any(|contract| validate_capability_semantic_contract(contract).is_err())
        {
            return Err(CompositionError::InvalidGraph);
        }
        let canonical =
            serde_json::to_vec(&contracts).map_err(|_| CompositionError::InvalidGraph)?;
        Ok(Self {
            spec: COMPOSITION_GRAPH_SPEC.to_owned(),
            digest: format!("sha256:{:x}", Sha256::digest(canonical)),
            contracts,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SemanticCompositionRequest {
    pub available_inputs: BTreeSet<String>,
    pub required_outputs: BTreeSet<String>,
    pub satisfied_preconditions: BTreeSet<String>,
    pub maximum_data_class: CapabilityDataClass,
    pub maximum_effect: CapabilityEffect,
    pub maximum_cost_units: u32,
    pub offline_only: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SemanticCompositionPlan {
    pub spec: String,
    pub graph_digest: String,
    pub capability_path: Vec<String>,
    pub available_outputs: Vec<String>,
    pub missing_outputs: Vec<String>,
    pub total_cost_units: u32,
    pub fully_resolved: bool,
    pub expanded_states: usize,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct SearchState {
    available: BTreeSet<String>,
    path: Vec<String>,
    cost: u32,
}

/// Finds the lowest-cost deterministic capability path without executing any capability.
///
/// # Errors
///
/// Rejects malformed requests or graphs whose digest no longer matches their contracts.
pub fn plan_semantic_composition(
    graph: &CapabilityCompositionGraph,
    request: &SemanticCompositionRequest,
) -> Result<SemanticCompositionPlan, CompositionError> {
    validate_graph(graph)?;
    validate_semantic_request(request)?;
    let initial = SearchState {
        available: request.available_inputs.clone(),
        path: Vec::new(),
        cost: 0,
    };
    let mut frontier = vec![initial.clone()];
    let mut best_partial = initial;
    let mut visited = BTreeMap::from([(state_key(&best_partial.available), 0_u32)]);
    let mut expanded_states = 0;

    while !frontier.is_empty() && expanded_states < MAX_EXPANDED_STATES {
        frontier.sort_by(|left, right| search_key(left).cmp(&search_key(right)));
        let state = frontier.remove(0);
        expanded_states += 1;
        if request.required_outputs.is_subset(&state.available) {
            return Ok(build_semantic_plan(graph, request, state, expanded_states));
        }
        if better_partial(&state, &best_partial, &request.required_outputs) {
            best_partial = state.clone();
        }
        if state.path.len() >= MAX_GRAPH_DEPTH {
            continue;
        }
        for contract in &graph.contracts {
            if state.path.contains(&contract.capability_id)
                || !contract
                    .requires
                    .iter()
                    .all(|item| state.available.contains(item))
                || !contract
                    .preconditions
                    .iter()
                    .all(|item| request.satisfied_preconditions.contains(item))
                || contract
                    .data_classes
                    .iter()
                    .any(|class| *class > request.maximum_data_class)
                || contract.effect > request.maximum_effect
                || (request.offline_only && !contract.offline_available)
            {
                continue;
            }
            let Some(cost) = state.cost.checked_add(contract.cost_units) else {
                continue;
            };
            if cost > request.maximum_cost_units {
                continue;
            }
            let mut available = state.available.clone();
            available.extend(contract.produces.iter().cloned());
            if available == state.available {
                continue;
            }
            let key = state_key(&available);
            if visited.get(&key).is_some_and(|known| *known <= cost) {
                continue;
            }
            visited.insert(key, cost);
            let mut path = state.path.clone();
            path.push(contract.capability_id.clone());
            frontier.push(SearchState {
                available,
                path,
                cost,
            });
        }
    }
    Ok(build_semantic_plan(
        graph,
        request,
        best_partial,
        expanded_states,
    ))
}

fn validate_graph(graph: &CapabilityCompositionGraph) -> Result<(), CompositionError> {
    if graph.spec != COMPOSITION_GRAPH_SPEC {
        return Err(CompositionError::InvalidGraph);
    }
    let rebuilt = CapabilityCompositionGraph::new(graph.contracts.clone())?;
    (rebuilt.digest == graph.digest)
        .then_some(())
        .ok_or(CompositionError::InvalidGraph)
}

fn validate_semantic_request(request: &SemanticCompositionRequest) -> Result<(), CompositionError> {
    if request.required_outputs.is_empty()
        || request.available_inputs.len() > MAX_SEMANTIC_REQUEST_ITEMS
        || request.required_outputs.len() > MAX_SEMANTIC_REQUEST_ITEMS
        || request.satisfied_preconditions.len() > MAX_SEMANTIC_REQUEST_ITEMS
        || request.maximum_cost_units == 0
        || request
            .available_inputs
            .iter()
            .chain(&request.required_outputs)
            .any(|item| !valid_semantic_type_id(item))
        || request
            .satisfied_preconditions
            .iter()
            .any(|item| !valid_precondition_id(item))
    {
        return Err(CompositionError::InvalidRequest);
    }
    Ok(())
}

fn build_semantic_plan(
    graph: &CapabilityCompositionGraph,
    request: &SemanticCompositionRequest,
    state: SearchState,
    expanded_states: usize,
) -> SemanticCompositionPlan {
    let available_outputs = request
        .required_outputs
        .intersection(&state.available)
        .cloned()
        .collect::<Vec<_>>();
    let missing_outputs = request
        .required_outputs
        .difference(&state.available)
        .cloned()
        .collect::<Vec<_>>();
    SemanticCompositionPlan {
        spec: SEMANTIC_PLAN_SPEC.to_owned(),
        graph_digest: graph.digest.clone(),
        capability_path: state.path,
        available_outputs,
        fully_resolved: missing_outputs.is_empty(),
        missing_outputs,
        total_cost_units: state.cost,
        expanded_states,
    }
}

fn search_key(state: &SearchState) -> (u32, usize, &[String]) {
    (state.cost, state.path.len(), &state.path)
}

fn state_key(available: &BTreeSet<String>) -> String {
    available.iter().cloned().collect::<Vec<_>>().join("\u{1f}")
}

fn better_partial(
    candidate: &SearchState,
    current: &SearchState,
    required: &BTreeSet<String>,
) -> bool {
    let candidate_count = candidate.available.intersection(required).count();
    let current_count = current.available.intersection(required).count();
    candidate_count > current_count
        || (candidate_count == current_count && search_key(candidate) < search_key(current))
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
    #[error("capability composition graph is invalid")]
    InvalidGraph,
}

#[cfg(test)]
mod tests {
    use super::*;
    use nimora_capability_contract::CapabilitySemanticDeclaration;
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

    fn semantic_contract(
        id: &str,
        requires: &[&str],
        produces: &[&str],
        effect: CapabilityEffect,
        cost_units: u32,
        offline_available: bool,
    ) -> CapabilitySemanticContract {
        CapabilitySemanticContract::new(
            id,
            CapabilitySemanticDeclaration {
                requires: requires.iter().map(|value| (*value).to_owned()).collect(),
                produces: produces.iter().map(|value| (*value).to_owned()).collect(),
                preconditions: Vec::new(),
                data_classes: vec![CapabilityDataClass::Internal],
                effect,
                cost_units,
                offline_available,
            },
        )
        .expect("semantic contract")
    }

    fn semantic_request(output: &str) -> SemanticCompositionRequest {
        SemanticCompositionRequest {
            available_inputs: BTreeSet::new(),
            required_outputs: BTreeSet::from([output.to_owned()]),
            satisfied_preconditions: BTreeSet::new(),
            maximum_data_class: CapabilityDataClass::Internal,
            maximum_effect: CapabilityEffect::ReversibleWrite,
            maximum_cost_units: 100,
            offline_only: false,
        }
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

    #[test]
    fn semantic_planner_finds_lowest_cost_deterministic_path() {
        let graph = CapabilityCompositionGraph::new([
            semantic_contract(
                "asset.catalog.read",
                &[],
                &["character.asset-id"],
                CapabilityEffect::ReadOnly,
                10,
                true,
            ),
            semantic_contract(
                "character.active.switch",
                &["character.asset-id"],
                &["character.active-state"],
                CapabilityEffect::ReversibleWrite,
                15,
                true,
            ),
            semantic_contract(
                "character.default.activate",
                &[],
                &["character.active-state"],
                CapabilityEffect::ReversibleWrite,
                40,
                true,
            ),
        ])
        .expect("graph");
        let plan = plan_semantic_composition(&graph, &semantic_request("character.active-state"))
            .expect("plan");
        assert!(plan.fully_resolved);
        assert_eq!(
            plan.capability_path,
            ["asset.catalog.read", "character.active.switch"]
        );
        assert_eq!(plan.total_cost_units, 25);
    }

    #[test]
    fn semantic_planner_enforces_offline_effect_and_cost_limits() {
        let graph = CapabilityCompositionGraph::new([
            semantic_contract(
                "cloud.summary.create",
                &[],
                &["document.summary"],
                CapabilityEffect::ExternalSideEffect,
                10,
                false,
            ),
            semantic_contract(
                "local.summary.create",
                &[],
                &["document.summary"],
                CapabilityEffect::ReadOnly,
                50,
                true,
            ),
        ])
        .expect("graph");
        let mut request = semantic_request("document.summary");
        request.offline_only = true;
        request.maximum_effect = CapabilityEffect::ReadOnly;
        request.maximum_cost_units = 40;
        let plan = plan_semantic_composition(&graph, &request).expect("bounded plan");
        assert!(!plan.fully_resolved);
        assert_eq!(plan.missing_outputs, ["document.summary"]);
        request.maximum_cost_units = 50;
        let plan = plan_semantic_composition(&graph, &request).expect("offline plan");
        assert_eq!(plan.capability_path, ["local.summary.create"]);
    }

    #[test]
    fn semantic_planner_requires_declared_preconditions_and_rejects_tampering() {
        let mut contract = semantic_contract(
            "character.active.switch",
            &["character.asset-id"],
            &["character.active-state"],
            CapabilityEffect::ReversibleWrite,
            10,
            true,
        );
        contract.preconditions = vec!["asset.installed".to_owned()];
        let mut graph = CapabilityCompositionGraph::new([contract]).expect("graph");
        let mut request = semantic_request("character.active-state");
        request
            .available_inputs
            .insert("character.asset-id".to_owned());
        let plan = plan_semantic_composition(&graph, &request).expect("missing precondition");
        assert!(!plan.fully_resolved);
        request
            .satisfied_preconditions
            .insert("asset.installed".to_owned());
        assert!(
            plan_semantic_composition(&graph, &request)
                .expect("satisfied precondition")
                .fully_resolved
        );
        graph.digest.push('0');
        assert_eq!(
            plan_semantic_composition(&graph, &request),
            Err(CompositionError::InvalidGraph)
        );
    }
}
