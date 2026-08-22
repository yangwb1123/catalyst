use crate::{
    kernel_decision_contract::{AtomRef, KernelDecisionReferenceClosure},
    kernel_operational_contract::{
        ArtifactReceiptRef, ArtifactRef, CapabilityInvocationRef, ExecutionReceiptRef,
        InteractionEventRef,
    },
};
use sha2::{Digest, Sha256};

use super::{
    CANONICALIZATION, DecisionCapsuleContractError, DecisionClosureRef, DecisionTransactionRef,
    MANIFEST_API, MANIFEST_KIND, MANIFEST_MODE, MAX_CLOSURE_BYTES, OperationalClosureRef,
    ReplayAttestations, StructuralReplayManifest, invalid, wire,
};

fn artifact_receipt_refs(closure: &KernelDecisionReferenceClosure) -> Vec<ArtifactReceiptRef> {
    closure
        .operational_closure
        .artifact_receipts
        .iter()
        .map(|item| ArtifactReceiptRef {
            artifact_receipt_id: item.artifact_receipt_id.clone(),
            artifact_receipt_sha256: item.artifact_receipt_sha256.clone(),
        })
        .collect()
}

fn capability_invocation_refs(
    closure: &KernelDecisionReferenceClosure,
) -> Vec<CapabilityInvocationRef> {
    closure
        .operational_closure
        .capability_invocations
        .iter()
        .map(|item| CapabilityInvocationRef {
            invocation_id: item.invocation_id.clone(),
            invocation_sha256: item.invocation_sha256.clone(),
        })
        .collect()
}

fn execution_receipt_refs(closure: &KernelDecisionReferenceClosure) -> Vec<ExecutionReceiptRef> {
    closure
        .operational_closure
        .execution_receipts
        .iter()
        .map(|item| ExecutionReceiptRef {
            execution_receipt_id: item.execution_receipt_id.clone(),
            execution_receipt_sha256: item.execution_receipt_sha256.clone(),
        })
        .collect()
}

fn interaction_event_refs(closure: &KernelDecisionReferenceClosure) -> Vec<InteractionEventRef> {
    closure
        .operational_closure
        .interaction_events
        .iter()
        .map(|item| InteractionEventRef {
            event_id: item.event_id.clone(),
            event_sha256: item.event_sha256.clone(),
        })
        .collect()
}

fn atom_refs(closure: &KernelDecisionReferenceClosure, phase: &str) -> Vec<AtomRef> {
    closure
        .cognitive_atoms
        .iter()
        .filter(|atom| atom.source.source_phase == phase)
        .map(|atom| AtomRef {
            atom_id: atom.atom_id.clone(),
            atom_sha256: atom.atom_sha256.clone(),
        })
        .collect()
}

pub(super) fn project_manifest(
    closure: &KernelDecisionReferenceClosure,
) -> StructuralReplayManifest {
    let operational = &closure.operational_closure;
    let transaction = &closure.decision_transaction;
    StructuralReplayManifest {
        api_version: MANIFEST_API.to_owned(),
        artifact_receipt_refs: artifact_receipt_refs(closure),
        artifact_refs: operational.artifacts.clone(),
        attestations: ReplayAttestations::default(),
        canonicalization: CANONICALIZATION.to_owned(),
        capability_invocation_refs: capability_invocation_refs(closure),
        decision_closure_ref: DecisionClosureRef {
            closure_id: closure.closure_id.clone(),
            closure_sha256: closure.closure_sha256.clone(),
        },
        decision_transaction_ref: DecisionTransactionRef {
            decision_transaction_id: transaction.decision_transaction_id.clone(),
            decision_transaction_sha256: transaction.decision_transaction_sha256.clone(),
        },
        effect_replay_allowed: false,
        execution_receipt_refs: execution_receipt_refs(closure),
        history_rewrite_allowed: false,
        interaction_event_refs: interaction_event_refs(closure),
        kind: MANIFEST_KIND.to_owned(),
        manifest_id: String::new(),
        manifest_sha256: String::new(),
        operational_closure_ref: OperationalClosureRef {
            closure_id: operational.closure_id.clone(),
            closure_sha256: operational.closure_sha256.clone(),
        },
        postdecision_atom_refs: atom_refs(closure, "postdecision"),
        predecision_atom_refs: atom_refs(closure, "predecision"),
        replay_mode: MANIFEST_MODE.to_owned(),
    }
}

pub(super) fn attestations(value: &ReplayAttestations) -> Result<(), DecisionCapsuleContractError> {
    if value == &ReplayAttestations::default() {
        Ok(())
    } else {
        Err(invalid("all thirty-two replay attestations must be false"))
    }
}

fn valid_hash(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

pub(super) fn identity(
    identity: &str,
    digest: &str,
    prefix: &str,
    label: &str,
    allow_blank: bool,
) -> Result<(), DecisionCapsuleContractError> {
    if allow_blank && identity.is_empty() && digest.is_empty() {
        return Ok(());
    }
    if valid_hash(digest) && identity == format!("{prefix}{digest}") {
        Ok(())
    } else {
        Err(invalid(format!("{label} identity must bind its SHA-256")))
    }
}

pub(super) fn reseal_decision_closure(
    value: &KernelDecisionReferenceClosure,
) -> Result<KernelDecisionReferenceClosure, DecisionCapsuleContractError> {
    wire::premeasure_only(value, crate::kernel_decision_contract::MAX_CLOSURE_BYTES)?;
    crate::kernel_decision_contract::validate_closure(value)
        .map_err(|error| invalid(format!("decision_closure: {}", error.message)))?;
    let mut blank = wire::bounded_clone(value, crate::kernel_decision_contract::MAX_CLOSURE_BYTES)?;
    blank.closure_id.clear();
    blank.closure_sha256.clear();
    let canonical = crate::kernel_decision_contract::canonical_json(&blank)
        .map_err(|error| invalid(format!("decision_closure encode: {}", error.message)))?;
    let mut hasher = Sha256::new();
    hasher.update(b"forgeos.kernel-decision-reference-closure.v1\0");
    hasher.update(canonical.as_bytes());
    let digest = crate::governance_contract::codec::lower_hex(&hasher.finalize());
    blank.closure_id = format!("kernel-decision-reference-closure-{digest}");
    blank.closure_sha256 = digest;
    if &blank == value {
        Ok(blank)
    } else {
        Err(invalid(
            "embedded ADR-0090 closure differs after exact reseal",
        ))
    }
}

pub(super) fn validate_artifact(value: &ArtifactRef) -> Result<(), DecisionCapsuleContractError> {
    if value.artifact_kind.is_empty() || value.artifact_kind.len() > 160 {
        return Err(invalid(
            "ArtifactRef artifact_kind is outside 1..=160 bytes",
        ));
    }
    if value.artifact_ref.is_empty() || value.artifact_ref.len() > 4_096 {
        return Err(invalid(
            "ArtifactRef artifact_ref is outside 1..=4096 bytes",
        ));
    }
    if !valid_hash(&value.artifact_sha256) {
        return Err(invalid(
            "ArtifactRef artifact_sha256 is not lowercase SHA-256",
        ));
    }
    Ok(())
}

pub(super) fn reflection_refs(values: &[ArtifactRef]) -> Result<(), DecisionCapsuleContractError> {
    if values.len() > 32 {
        return Err(invalid("reflection report ArtifactRefs exceed 32"));
    }
    let mut encoded = Vec::with_capacity(values.len());
    for value in values {
        validate_artifact(value)?;
        if value.artifact_kind != "reflection_report" {
            return Err(invalid("reflection report ArtifactRef kind differs"));
        }
        encoded.push(wire::canonical_with_max(value, MAX_CLOSURE_BYTES)?);
    }
    if encoded.windows(2).all(|pair| pair[0] < pair[1]) {
        Ok(())
    } else {
        Err(invalid(
            "reflection report ArtifactRefs must be sorted and unique",
        ))
    }
}
