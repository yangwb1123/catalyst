mod atom;
mod closure;
mod graph;
mod model;
mod primitives;
mod source;
mod transaction;
mod wire;

use std::fmt;

pub use atom::{decode_cognitive_atom, seal_cognitive_atom, validate_cognitive_atom};
pub use closure::{decode_closure, seal_closure, validate_closure};
pub use model::*;
pub use transaction::{
    decode_decision_transaction, seal_decision_transaction, validate_decision_transaction,
};
pub use wire::canonical_json;

pub const SUCCESS_MARKER: &str = "STRUCTURALLY_VALID_KERNEL_DECISION_REFERENCE_CLOSURE_V1 (exact caller-supplied cognitive, transaction and operational reference relations only; declared authority and hardness are ineffective; all twenty-two attestations are false: no Approval, principal, Grant or binding authentication; no source resolution, authority, authorization, CAS, completion, content provenance, effect, event append, execution, hard guard, instruction, outcome, permission, persistence, transition, truth, usage measurement or verifier independence attestation)";

pub const MAX_ATOM_BYTES: usize = 131_072;
pub const MAX_ATOM_SET_BYTES: usize = 1_048_576;
pub const MAX_TRANSACTION_BYTES: usize = 1_048_576;
pub const MAX_CLOSURE_BYTES: usize = 20_971_520;

pub(super) const CANONICALIZATION: &str = "forgeos.canonical-json/v1";
pub(super) const ATOM_API: &str = "forgeos.aadm.cognitive-atom/v2";
pub(super) const ATOM_KIND: &str = "CognitiveAtom";
pub(super) const ATOM_PREFIX: &str = "cognitive-atom-";
pub(super) const ATOM_DOMAIN: &[u8] = b"forgeos.aadm.cognitive-atom.v2\0";
pub(super) const TRANSACTION_API: &str = "forgeos.aadm.decision-transaction/v1";
pub(super) const TRANSACTION_KIND: &str = "DecisionTransaction";
pub(super) const TRANSACTION_PREFIX: &str = "decision-transaction-";
pub(super) const TRANSACTION_DOMAIN: &[u8] = b"forgeos.aadm.decision-transaction.v1\0";
pub(super) const CLOSURE_API: &str = "forgeos.kernel-decision-reference-closure/v1";
pub(super) const CLOSURE_KIND: &str = "KernelDecisionReferenceClosure";
pub(super) const CLOSURE_PREFIX: &str = "kernel-decision-reference-closure-";
pub(super) const CLOSURE_DOMAIN: &[u8] = b"forgeos.kernel-decision-reference-closure.v1\0";

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct KernelDecisionContractError {
    pub message: String,
}

impl fmt::Display for KernelDecisionContractError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl std::error::Error for KernelDecisionContractError {}

pub(super) fn invalid(message: impl Into<String>) -> KernelDecisionContractError {
    KernelDecisionContractError {
        message: message.into(),
    }
}

#[cfg(test)]
mod tests;
