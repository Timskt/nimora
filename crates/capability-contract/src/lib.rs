use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};
use thiserror::Error;

pub const CAPABILITY_SEMANTIC_CONTRACT_SPEC: &str = "nimora.capability-semantic-contract/1";
pub const MAX_SEMANTIC_TYPES: usize = 32;
pub const MAX_PRECONDITIONS: usize = 32;
pub const MAX_COST_UNITS: u32 = 1_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityEffect {
    ReadOnly,
    ReversibleWrite,
    IrreversibleWrite,
    ExternalSideEffect,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CapabilityDataClass {
    Public,
    Internal,
    Personal,
    Sensitive,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilitySemanticContract {
    pub spec: String,
    pub capability_id: String,
    pub requires: Vec<String>,
    pub produces: Vec<String>,
    pub preconditions: Vec<String>,
    pub data_classes: Vec<CapabilityDataClass>,
    pub effect: CapabilityEffect,
    pub cost_units: u32,
    pub offline_available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CapabilitySemanticDeclaration {
    pub requires: Vec<String>,
    pub produces: Vec<String>,
    pub preconditions: Vec<String>,
    pub data_classes: Vec<CapabilityDataClass>,
    pub effect: CapabilityEffect,
    pub cost_units: u32,
    pub offline_available: bool,
}

impl CapabilitySemanticContract {
    /// Builds and validates a canonical semantic contract.
    ///
    /// # Errors
    ///
    /// Returns a stable validation error for malformed, duplicate, unsorted, or unbounded fields.
    pub fn new(
        capability_id: impl Into<String>,
        declaration: CapabilitySemanticDeclaration,
    ) -> Result<Self, CapabilityContractError> {
        let contract = Self {
            spec: CAPABILITY_SEMANTIC_CONTRACT_SPEC.to_owned(),
            capability_id: capability_id.into(),
            requires: declaration.requires,
            produces: declaration.produces,
            preconditions: declaration.preconditions,
            data_classes: declaration.data_classes,
            effect: declaration.effect,
            cost_units: declaration.cost_units,
            offline_available: declaration.offline_available,
        };
        validate_capability_semantic_contract(&contract)?;
        Ok(contract)
    }
}

/// Validates one canonical, implementation-free semantic capability contract.
///
/// # Errors
///
/// Rejects unsupported specs, malformed identifiers, duplicate or unsorted lists, empty outputs,
/// and costs outside the host budget.
pub fn validate_capability_semantic_contract(
    contract: &CapabilitySemanticContract,
) -> Result<(), CapabilityContractError> {
    if contract.spec != CAPABILITY_SEMANTIC_CONTRACT_SPEC {
        return Err(CapabilityContractError::UnsupportedSpec);
    }
    if !valid_capability_id(&contract.capability_id) {
        return Err(CapabilityContractError::InvalidCapabilityId);
    }
    if contract.produces.is_empty()
        || contract.requires.len() > MAX_SEMANTIC_TYPES
        || contract.produces.len() > MAX_SEMANTIC_TYPES
        || contract.preconditions.len() > MAX_PRECONDITIONS
        || !canonical_ids(&contract.requires, valid_semantic_type_id)
        || !canonical_ids(&contract.produces, valid_semantic_type_id)
        || !canonical_ids(&contract.preconditions, valid_precondition_id)
        || !canonical_values(&contract.data_classes)
    {
        return Err(CapabilityContractError::InvalidSemantics);
    }
    if contract.cost_units == 0 || contract.cost_units > MAX_COST_UNITS {
        return Err(CapabilityContractError::InvalidCost);
    }
    Ok(())
}

#[must_use]
pub fn valid_capability_id(value: &str) -> bool {
    valid_namespaced_id(value, 3)
}

#[must_use]
pub fn valid_semantic_type_id(value: &str) -> bool {
    valid_namespaced_id(value, 2)
}

#[must_use]
pub fn valid_precondition_id(value: &str) -> bool {
    valid_namespaced_id(value, 2)
}

fn valid_namespaced_id(value: &str, minimum_segments: usize) -> bool {
    !value.is_empty()
        && value.len() <= 128
        && value.split('.').count() >= minimum_segments
        && value.split('.').all(|segment| {
            !segment.is_empty()
                && segment.len() <= 48
                && segment
                    .bytes()
                    .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        })
}

fn canonical_ids(values: &[String], validator: fn(&str) -> bool) -> bool {
    values.iter().all(|value| validator(value)) && values.windows(2).all(|pair| pair[0] < pair[1])
}

fn canonical_values<T: Ord>(values: &[T]) -> bool {
    values.iter().collect::<BTreeSet<_>>().len() == values.len()
        && values.windows(2).all(|pair| pair[0] < pair[1])
}

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CapabilityContractError {
    #[error("capability semantic contract spec is unsupported")]
    UnsupportedSpec,
    #[error("capability identifier is invalid")]
    InvalidCapabilityId,
    #[error("capability semantic declaration is invalid")]
    InvalidSemantics,
    #[error("capability cost is invalid")]
    InvalidCost,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contract() -> CapabilitySemanticContract {
        CapabilitySemanticContract::new(
            "character.active.switch",
            CapabilitySemanticDeclaration {
                requires: vec!["character.asset-id".to_owned()],
                produces: vec!["character.active-state".to_owned()],
                preconditions: vec![
                    "asset.installed".to_owned(),
                    "asset.integrity-verified".to_owned(),
                ],
                data_classes: vec![CapabilityDataClass::Internal],
                effect: CapabilityEffect::ReversibleWrite,
                cost_units: 20,
                offline_available: true,
            },
        )
        .expect("contract")
    }

    #[test]
    fn accepts_canonical_bounded_contract() {
        let value = contract();
        assert_eq!(value.spec, CAPABILITY_SEMANTIC_CONTRACT_SPEC);
        assert!(validate_capability_semantic_contract(&value).is_ok());
    }

    #[test]
    fn rejects_unsorted_or_duplicate_semantics() {
        let mut value = contract();
        value.preconditions.reverse();
        assert_eq!(
            validate_capability_semantic_contract(&value),
            Err(CapabilityContractError::InvalidSemantics)
        );
        value = contract();
        value.produces.push("character.active-state".to_owned());
        assert_eq!(
            validate_capability_semantic_contract(&value),
            Err(CapabilityContractError::InvalidSemantics)
        );
    }

    #[test]
    fn rejects_unknown_fields_and_invalid_cost() {
        let json = serde_json::json!({
            "spec": CAPABILITY_SEMANTIC_CONTRACT_SPEC,
            "capabilityId": "pet.state.read",
            "requires": [],
            "produces": ["pet.state"],
            "preconditions": [],
            "dataClasses": ["internal"],
            "effect": "read-only",
            "costUnits": 0,
            "offlineAvailable": true,
            "backend": "forbidden"
        });
        assert!(serde_json::from_value::<CapabilitySemanticContract>(json).is_err());
        let mut value = contract();
        value.cost_units = 0;
        assert_eq!(
            validate_capability_semantic_contract(&value),
            Err(CapabilityContractError::InvalidCost)
        );
    }
}
