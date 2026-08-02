use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::{
    GroupAgentNodeDispatchAuthorization, GroupAgentNodeProviderKind,
    MAX_GROUP_AGENT_NODE_MODEL_BYTES, MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS,
    group_agent_node_destination_sha256,
};

pub const GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION: u16 = 1;
pub const GROUP_AGENT_NODE_PRICING_SNAPSHOT_DIGEST_DOMAIN: &[u8] =
    b"forge.group-agent-node-pricing-snapshot.v1\0";
pub const GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT: &str =
    "https://api.openai.com/v1/responses";
pub const GROUP_AGENT_NODE_PRICING_CURRENCY: &str = "usd_micros";
pub const GROUP_AGENT_NODE_PRICING_TOKEN_UNIT: u64 = 1_000_000;
pub const GROUP_AGENT_NODE_PRICING_COST_ALGORITHM: &str = "ceil_each_token_component_v1";
pub const GROUP_AGENT_NODE_PRICING_PROVENANCE: &str = "operator_asserted";
pub const MAX_GROUP_AGENT_NODE_PRICING_RATE_USD_MICROS: u64 = 1_000_000_000_000;
pub const MAX_GROUP_AGENT_NODE_PRICING_INPUT_TOKENS: u64 = 1_000_000_000;
pub const MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES: usize = 16 * 1024;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct GroupAgentNodePricingSnapshot {
    pub v: u16,
    pub pricing_protocol_version: u16,
    pub provider_kind: GroupAgentNodeProviderKind,
    pub endpoint: String,
    pub model: String,
    pub destination_sha256: String,
    pub currency: String,
    pub token_unit: u64,
    pub input_usd_micros_per_token_unit: u64,
    pub output_usd_micros_per_token_unit: u64,
    pub max_input_tokens: u64,
    pub cost_algorithm: String,
    pub provenance: String,
    pub vendor_attestation_present: bool,
    pub pricing_snapshot_sha256: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodePricingQuote {
    pub pricing_snapshot_sha256: String,
    pub destination_sha256: String,
    pub max_input_tokens: u64,
    pub max_output_tokens: u32,
    pub max_cost_usd_micros: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct GroupAgentNodePricingValidationError {
    pub message: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum GroupAgentNodeDestinationRegistryError {
    Rejected,
}

pub trait GroupAgentNodeDestinationRegistry: Send + Sync {
    /// Resolves one exact authorized destination and its conservative quote.
    ///
    /// This port is effect-free: implementations must not inspect credentials,
    /// construct a provider, perform a health check, or issue network requests.
    ///
    /// # Errors
    ///
    /// Returns a controlled rejection when the destination is not registered
    /// or its pricing and authorization bindings are not acceptable.
    fn resolve(
        &self,
        authorization: &GroupAgentNodeDispatchAuthorization,
        pricing: &GroupAgentNodePricingSnapshot,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodeDestinationRegistryError>;
}

impl GroupAgentNodePricingSnapshot {
    /// Decodes and validates one exact canonical pricing snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for malformed, non-canonical, oversized, unsupported,
    /// or self-inconsistent input. Input bytes are never included in errors.
    pub fn decode_exact(json: &str) -> Result<Self, GroupAgentNodePricingValidationError> {
        decode_exact_json(json)
    }

    /// Fully validates this snapshot and its content identity.
    ///
    /// # Errors
    ///
    /// Returns an error for unsupported values, unsafe bounds, or digest drift.
    pub fn validate(&self) -> Result<(), GroupAgentNodePricingValidationError> {
        validate_snapshot(self)
    }

    /// Encodes the complete snapshot as compact canonical JSON.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed snapshot cannot be encoded.
    pub fn canonical_json(&self) -> Result<String, GroupAgentNodePricingValidationError> {
        encode_canonical_json(self)
    }

    /// Encodes the exact identity payload without `pricing_snapshot_sha256`.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed snapshot payload cannot be encoded.
    pub fn canonical_payload_json(&self) -> Result<String, GroupAgentNodePricingValidationError> {
        encode_payload_json(self)
    }

    /// Computes the domain-separated snapshot identity without its digest field.
    ///
    /// # Errors
    ///
    /// Returns an error if the fixed digest payload cannot be encoded.
    pub fn expected_sha256(&self) -> Result<String, GroupAgentNodePricingValidationError> {
        snapshot_digest(self)
    }

    /// Computes the conservative maximum cost without consulting authorization.
    ///
    /// # Errors
    ///
    /// Returns an error when the snapshot or output-token ceiling is outside
    /// protocol bounds, or when checked arithmetic cannot represent the result.
    pub fn maximum_cost_usd_micros(
        &self,
        max_output_tokens: u32,
    ) -> Result<u64, GroupAgentNodePricingValidationError> {
        self.validate()?;
        if !(1..=MAX_GROUP_AGENT_NODE_OUTPUT_TOKENS).contains(&max_output_tokens) {
            return Err(invalid("output token ceiling is outside protocol bounds"));
        }
        maximum_cost(self, max_output_tokens)
    }

    /// Verifies an exact Dispatch Authorization and computes its conservative
    /// maximum cost under this immutable pricing snapshot.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid authorization, destination or pricing drift,
    /// arithmetic overflow, or a maximum cost above the authorized budget.
    pub fn verify_authorization(
        &self,
        authorization: &GroupAgentNodeDispatchAuthorization,
    ) -> Result<GroupAgentNodePricingQuote, GroupAgentNodePricingValidationError> {
        verify_pricing_authorization(self, authorization)
    }

    /// Computes exact observed usage cost under one immutable authorization.
    ///
    /// Each input and output component is independently rounded up using wide,
    /// checked arithmetic. Both token counts must be observed and nonzero.
    ///
    /// # Errors
    ///
    /// Returns an error for invalid bindings, token bounds, arithmetic overflow,
    /// or an actual cost above the authorized budget.
    pub fn actual_cost_usd_micros(
        &self,
        input_tokens: u64,
        output_tokens: u64,
        authorization: &GroupAgentNodeDispatchAuthorization,
    ) -> Result<u64, GroupAgentNodePricingValidationError> {
        verify_actual_cost(self, input_tokens, output_tokens, authorization)
    }
}

impl std::fmt::Display for GroupAgentNodePricingValidationError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for GroupAgentNodePricingValidationError {}

impl std::fmt::Display for GroupAgentNodeDestinationRegistryError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Group Agent Node destination registry rejected the request")
    }
}

impl std::error::Error for GroupAgentNodeDestinationRegistryError {}

#[derive(Serialize)]
struct PricingSnapshotPayload<'a> {
    v: u16,
    pricing_protocol_version: u16,
    provider_kind: GroupAgentNodeProviderKind,
    endpoint: &'a str,
    model: &'a str,
    destination_sha256: &'a str,
    currency: &'a str,
    token_unit: u64,
    input_usd_micros_per_token_unit: u64,
    output_usd_micros_per_token_unit: u64,
    max_input_tokens: u64,
    cost_algorithm: &'a str,
    provenance: &'a str,
    vendor_attestation_present: bool,
}

impl<'a> From<&'a GroupAgentNodePricingSnapshot> for PricingSnapshotPayload<'a> {
    fn from(value: &'a GroupAgentNodePricingSnapshot) -> Self {
        Self {
            v: value.v,
            pricing_protocol_version: value.pricing_protocol_version,
            provider_kind: value.provider_kind,
            endpoint: &value.endpoint,
            model: &value.model,
            destination_sha256: &value.destination_sha256,
            currency: &value.currency,
            token_unit: value.token_unit,
            input_usd_micros_per_token_unit: value.input_usd_micros_per_token_unit,
            output_usd_micros_per_token_unit: value.output_usd_micros_per_token_unit,
            max_input_tokens: value.max_input_tokens,
            cost_algorithm: &value.cost_algorithm,
            provenance: &value.provenance,
            vendor_attestation_present: value.vendor_attestation_present,
        }
    }
}

fn decode_exact_json(
    json: &str,
) -> Result<GroupAgentNodePricingSnapshot, GroupAgentNodePricingValidationError> {
    if json.is_empty() || json.len() > MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES {
        return Err(invalid("pricing snapshot input is outside its byte bound"));
    }
    let snapshot: GroupAgentNodePricingSnapshot = serde_json::from_str(json)
        .map_err(|_| invalid("pricing snapshot input is invalid JSON"))?;
    snapshot.validate()?;
    if snapshot.canonical_json()?.as_bytes() != json.as_bytes() {
        return Err(invalid(
            "pricing snapshot input is not exact canonical JSON",
        ));
    }
    Ok(snapshot)
}

fn validate_snapshot(
    snapshot: &GroupAgentNodePricingSnapshot,
) -> Result<(), GroupAgentNodePricingValidationError> {
    if !valid_header(snapshot) || !valid_policy(snapshot) {
        return Err(invalid("invalid Group Agent Node pricing snapshot"));
    }
    if snapshot.destination_sha256
        != group_agent_node_destination_sha256(
            snapshot.provider_kind,
            &snapshot.endpoint,
            &snapshot.model,
        )
    {
        return Err(invalid("pricing snapshot destination identity disagrees"));
    }
    if snapshot.expected_sha256()? != snapshot.pricing_snapshot_sha256 {
        return Err(invalid("pricing snapshot content identity disagrees"));
    }
    let bytes = snapshot.canonical_json()?.len();
    if !(1..=MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES).contains(&bytes) {
        return Err(invalid("pricing snapshot exceeds its byte bound"));
    }
    Ok(())
}

fn valid_header(snapshot: &GroupAgentNodePricingSnapshot) -> bool {
    snapshot.v == GROUP_AGENT_NODE_PRICING_SNAPSHOT_VERSION
        && snapshot.pricing_protocol_version == GROUP_AGENT_NODE_PRICING_PROTOCOL_VERSION
        && snapshot.provider_kind == GroupAgentNodeProviderKind::OpenAiResponses
        && snapshot.endpoint == GROUP_AGENT_NODE_OFFICIAL_OPENAI_RESPONSES_ENDPOINT
        && valid_text(&snapshot.model, MAX_GROUP_AGENT_NODE_MODEL_BYTES)
        && is_digest(&snapshot.destination_sha256)
        && is_digest(&snapshot.pricing_snapshot_sha256)
}

fn valid_policy(snapshot: &GroupAgentNodePricingSnapshot) -> bool {
    snapshot.currency == GROUP_AGENT_NODE_PRICING_CURRENCY
        && snapshot.token_unit == GROUP_AGENT_NODE_PRICING_TOKEN_UNIT
        && (1..=MAX_GROUP_AGENT_NODE_PRICING_RATE_USD_MICROS)
            .contains(&snapshot.input_usd_micros_per_token_unit)
        && (1..=MAX_GROUP_AGENT_NODE_PRICING_RATE_USD_MICROS)
            .contains(&snapshot.output_usd_micros_per_token_unit)
        && (1..=MAX_GROUP_AGENT_NODE_PRICING_INPUT_TOKENS).contains(&snapshot.max_input_tokens)
        && snapshot.cost_algorithm == GROUP_AGENT_NODE_PRICING_COST_ALGORITHM
        && snapshot.provenance == GROUP_AGENT_NODE_PRICING_PROVENANCE
        && !snapshot.vendor_attestation_present
}

fn verify_pricing_authorization(
    snapshot: &GroupAgentNodePricingSnapshot,
    authorization: &GroupAgentNodeDispatchAuthorization,
) -> Result<GroupAgentNodePricingQuote, GroupAgentNodePricingValidationError> {
    snapshot.validate()?;
    authorization
        .validate()
        .map_err(|_| invalid("Node Dispatch Authorization is invalid for pricing"))?;
    validate_authorization_bindings(snapshot, authorization)?;
    let maximum = maximum_cost(snapshot, authorization.budgets.max_output_tokens)?;
    if maximum > authorization.budgets.max_cost_usd_micros {
        return Err(invalid(
            "authorized Node Dispatch cost budget is insufficient",
        ));
    }
    Ok(GroupAgentNodePricingQuote {
        pricing_snapshot_sha256: snapshot.pricing_snapshot_sha256.clone(),
        destination_sha256: snapshot.destination_sha256.clone(),
        max_input_tokens: snapshot.max_input_tokens,
        max_output_tokens: authorization.budgets.max_output_tokens,
        max_cost_usd_micros: maximum,
    })
}

fn verify_actual_cost(
    snapshot: &GroupAgentNodePricingSnapshot,
    input_tokens: u64,
    output_tokens: u64,
    authorization: &GroupAgentNodeDispatchAuthorization,
) -> Result<u64, GroupAgentNodePricingValidationError> {
    verify_pricing_authorization(snapshot, authorization)?;
    let valid_tokens = (1..=snapshot.max_input_tokens).contains(&input_tokens)
        && (1..=u64::from(authorization.budgets.max_output_tokens)).contains(&output_tokens);
    if !valid_tokens {
        return Err(invalid("observed usage is outside its authorized bounds"));
    }
    let actual = maximum_cost_components(
        input_tokens,
        output_tokens,
        snapshot.input_usd_micros_per_token_unit,
        snapshot.output_usd_micros_per_token_unit,
        snapshot.token_unit,
    )?;
    (actual <= authorization.budgets.max_cost_usd_micros)
        .then_some(actual)
        .ok_or_else(|| invalid("observed usage cost exceeds its authorized budget"))
}

fn validate_authorization_bindings(
    snapshot: &GroupAgentNodePricingSnapshot,
    authorization: &GroupAgentNodeDispatchAuthorization,
) -> Result<(), GroupAgentNodePricingValidationError> {
    let exact = authorization.provider_kind == snapshot.provider_kind
        && authorization.endpoint == snapshot.endpoint
        && authorization.model == snapshot.model
        && authorization.destination_sha256 == snapshot.destination_sha256
        && authorization.pricing_snapshot_sha256 == snapshot.pricing_snapshot_sha256
        && authorization.budgets.pricing_snapshot_sha256 == snapshot.pricing_snapshot_sha256;
    exact
        .then_some(())
        .ok_or_else(|| invalid("pricing snapshot and Node Dispatch Authorization disagree"))
}

fn maximum_cost(
    snapshot: &GroupAgentNodePricingSnapshot,
    max_output_tokens: u32,
) -> Result<u64, GroupAgentNodePricingValidationError> {
    maximum_cost_components(
        snapshot.max_input_tokens,
        u64::from(max_output_tokens),
        snapshot.input_usd_micros_per_token_unit,
        snapshot.output_usd_micros_per_token_unit,
        snapshot.token_unit,
    )
}

fn maximum_cost_components(
    input_tokens: u64,
    output_tokens: u64,
    input_rate: u64,
    output_rate: u64,
    unit: u64,
) -> Result<u64, GroupAgentNodePricingValidationError> {
    let input = ceiling_token_cost(input_tokens, input_rate, unit)?;
    let output = ceiling_token_cost(output_tokens, output_rate, unit)?;
    let total = input
        .checked_add(output)
        .ok_or_else(|| invalid("pricing cost arithmetic overflowed"))?;
    u64::try_from(total).map_err(|_| invalid("pricing cost exceeds its integer bound"))
}

fn ceiling_token_cost(
    tokens: u64,
    rate: u64,
    unit: u64,
) -> Result<u128, GroupAgentNodePricingValidationError> {
    let product = u128::from(tokens)
        .checked_mul(u128::from(rate))
        .ok_or_else(|| invalid("pricing cost multiplication overflowed"))?;
    let unit = u128::from(unit);
    let quotient = product / unit;
    quotient
        .checked_add(u128::from(product % unit != 0))
        .ok_or_else(|| invalid("pricing cost rounding overflowed"))
}

fn encode_canonical_json(
    value: &impl Serialize,
) -> Result<String, GroupAgentNodePricingValidationError> {
    serde_json::to_string(value).map_err(|_| invalid("pricing snapshot cannot be encoded"))
}

fn encode_payload_json(
    snapshot: &GroupAgentNodePricingSnapshot,
) -> Result<String, GroupAgentNodePricingValidationError> {
    serde_json::to_string(&PricingSnapshotPayload::from(snapshot))
        .map_err(|_| invalid("pricing snapshot identity cannot be encoded"))
}

fn snapshot_digest(
    snapshot: &GroupAgentNodePricingSnapshot,
) -> Result<String, GroupAgentNodePricingValidationError> {
    let bytes = encode_payload_json(snapshot)?.into_bytes();
    let mut digest = Sha256::new();
    digest.update(GROUP_AGENT_NODE_PRICING_SNAPSHOT_DIGEST_DOMAIN);
    digest.update(bytes);
    Ok(format!("{:x}", digest.finalize()))
}

fn valid_text(value: &str, maximum: usize) -> bool {
    !value.trim().is_empty()
        && value.len() <= maximum
        && !value.chars().any(|character| {
            character.is_control()
                || matches!(
                    character,
                    '\u{061c}'
                        | '\u{200e}'
                        | '\u{200f}'
                        | '\u{2028}'..='\u{202e}'
                        | '\u{2066}'..='\u{2069}'
                )
        })
}

fn is_digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn invalid(message: &str) -> GroupAgentNodePricingValidationError {
    GroupAgentNodePricingValidationError {
        message: message.into(),
    }
}

#[cfg(test)]
mod arithmetic_tests {
    use super::*;

    #[test]
    fn two_u64_factors_always_fit_in_the_u128_wide_product() {
        let maximum = u128::from(u64::MAX);
        let product = maximum
            .checked_mul(maximum)
            .expect("two u64 factors always fit in u128");
        assert_eq!(ceiling_token_cost(u64::MAX, u64::MAX, 1), Ok(product));
    }

    #[test]
    fn out_of_protocol_addition_and_narrowing_fail_closed() {
        let addition = maximum_cost_components(u64::MAX, u64::MAX, u64::MAX, u64::MAX, 1)
            .expect_err("two maximum components overflow their u128 sum");
        assert!(addition.message.contains("arithmetic overflowed"));

        let narrowing = maximum_cost_components(u64::MAX, 1, u64::MAX, 1, 1)
            .expect_err("one maximum product exceeds the u64 result bound");
        assert!(narrowing.message.contains("integer bound"));
    }
}
