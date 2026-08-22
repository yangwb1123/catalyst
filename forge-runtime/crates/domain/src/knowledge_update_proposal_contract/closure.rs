use std::collections::{BTreeMap, BTreeSet};

use crate::{
    governance_contract::{GovernanceRecord, KnowledgeClaim},
    validate_governance_semantic_transition,
};

use super::{
    ClaimRef, KnowledgeMutation, KnowledgeUpdateProposal, KnowledgeUpdateProposalContractError,
    MutationOperation, invalid,
};

pub(super) fn validate(
    proposal: &KnowledgeUpdateProposal,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    let records = index_records(&proposal.records);
    validate_mutation_targets(proposal, &records)?;
    validate_exact_closure(proposal, &records)
}

fn index_records(records: &[GovernanceRecord]) -> BTreeMap<&str, &GovernanceRecord> {
    records
        .iter()
        .map(|record| (record.metadata().record_id.as_str(), record))
        .collect()
}

fn validate_mutation_targets(
    proposal: &KnowledgeUpdateProposal,
    records: &BTreeMap<&str, &GovernanceRecord>,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    let mut before_ids = BTreeSet::new();
    let mut after_ids = BTreeSet::new();
    for mutation in &proposal.mutations {
        let after = claim_for(&mutation.after_claim_ref, records, "after claim")?;
        if !after_ids.insert(after.metadata.record_id.as_str()) {
            return Err(invalid("mutation after_claim_ref values must be unique"));
        }
        validate_after_binding(proposal, mutation, after)?;
        match (&mutation.operation, &mutation.before_claim_ref) {
            (MutationOperation::Create, None) => validate_create(after)?,
            (MutationOperation::Supersede, Some(reference)) => {
                let before = claim_for(reference, records, "before claim")?;
                if !before_ids.insert(before.metadata.record_id.as_str()) {
                    return Err(invalid("mutation before_claim_ref values must be unique"));
                }
                validate_supersede(mutation, before, after)?;
            }
            (MutationOperation::Create, Some(_)) => {
                return Err(invalid("create mutation must not declare before_claim_ref"));
            }
            (MutationOperation::Supersede, None) => {
                return Err(invalid("supersede mutation requires before_claim_ref"));
            }
        }
    }
    Ok(())
}

fn claim_for<'a>(
    reference: &ClaimRef,
    records: &BTreeMap<&str, &'a GovernanceRecord>,
    label: &str,
) -> Result<&'a KnowledgeClaim, KnowledgeUpdateProposalContractError> {
    let record = records
        .get(reference.record_id.as_str())
        .ok_or_else(|| invalid(format!("{label} does not resolve in records")))?;
    if record.integrity().canonical_sha256 != reference.canonical_sha256 {
        return Err(invalid(format!(
            "{label} digest does not match exact record"
        )));
    }
    let GovernanceRecord::Claim(claim) = record else {
        return Err(invalid(format!("{label} targets an EvidenceRecord")));
    };
    Ok(claim)
}

fn validate_after_binding(
    proposal: &KnowledgeUpdateProposal,
    mutation: &KnowledgeMutation,
    after: &KnowledgeClaim,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    let metadata = &after.metadata;
    let principal_matches = metadata.created_by.authority_domain
        == proposal.proposer.authority_domain
        && metadata.created_by.principal_id == proposal.proposer.principal_id
        && principal_type_matches(
            metadata.created_by.principal_type,
            proposal.proposer.principal_type,
        )
        && metadata.created_by.role == proposal.task_binding.role
        && metadata.created_by.run_id == proposal.task_binding.run_id;
    let bindings_match = metadata.aggregate_id == mutation.target_aggregate_id
        && metadata.context_sha256 == proposal.bindings.context_sha256
        && metadata.policy_sha256 == proposal.bindings.policy_sha256
        && metadata.source_revision == proposal.bindings.source_revision
        && metadata.source_tree_sha256 == proposal.bindings.source_tree_sha256
        && metadata.created_at_unix_ms <= proposal.submitted_at_unix_ms;
    if principal_matches && bindings_match {
        Ok(())
    } else {
        Err(invalid(
            "after claim does not match proposer, task, source, context, policy, or submission",
        ))
    }
}

fn principal_type_matches(
    record: crate::governance_contract::PrincipalType,
    proposer: crate::capability_grant_contract::PrincipalType,
) -> bool {
    matches!(
        (record, proposer),
        (
            crate::governance_contract::PrincipalType::Agent,
            crate::capability_grant_contract::PrincipalType::Agent
        ) | (
            crate::governance_contract::PrincipalType::Human,
            crate::capability_grant_contract::PrincipalType::Human
        ) | (
            crate::governance_contract::PrincipalType::Operator,
            crate::capability_grant_contract::PrincipalType::Operator
        ) | (
            crate::governance_contract::PrincipalType::Service,
            crate::capability_grant_contract::PrincipalType::Service
        )
    )
}

fn validate_create(after: &KnowledgeClaim) -> Result<(), KnowledgeUpdateProposalContractError> {
    if after.metadata.sequence != 1 || !after.metadata.supersedes_record_ids.is_empty() {
        return Err(invalid(
            "create after claim must start at sequence one without supersession",
        ));
    }
    validate_governance_semantic_transition(None, &GovernanceRecord::Claim(after.clone()))
        .map_err(|error| invalid(format!("create lifecycle: {}", error.message)))
}

fn validate_supersede(
    mutation: &KnowledgeMutation,
    before: &KnowledgeClaim,
    after: &KnowledgeClaim,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    let structural = before.metadata.aggregate_id == mutation.target_aggregate_id
        && after.metadata.aggregate_id == mutation.target_aggregate_id
        && after.metadata.sequence == before.metadata.sequence + 1
        && after
            .metadata
            .supersedes_record_ids
            .binary_search_by(|value| value.as_str().cmp(&before.metadata.record_id))
            .is_ok();
    if !structural {
        return Err(invalid(
            "supersede mutation does not name the exact immediate predecessor",
        ));
    }
    validate_governance_semantic_transition(
        Some(&GovernanceRecord::Claim(before.clone())),
        &GovernanceRecord::Claim(after.clone()),
    )
    .map_err(|error| invalid(format!("supersede lifecycle: {}", error.message)))
}

fn validate_exact_closure(
    proposal: &KnowledgeUpdateProposal,
    records: &BTreeMap<&str, &GovernanceRecord>,
) -> Result<(), KnowledgeUpdateProposalContractError> {
    let mut pending: Vec<&str> = proposal
        .mutations
        .iter()
        .map(|mutation| mutation.after_claim_ref.record_id.as_str())
        .collect();
    let mut reached = BTreeSet::new();
    while let Some(record_id) = pending.pop() {
        if !reached.insert(record_id) {
            continue;
        }
        let record = records
            .get(record_id)
            .ok_or_else(|| invalid("proposal closure contains an unresolved record"))?;
        pending.extend(
            record
                .metadata()
                .supersedes_record_ids
                .iter()
                .map(String::as_str),
        );
        if let GovernanceRecord::Claim(claim) = record {
            pending.extend(
                claim
                    .spec
                    .supporting_evidence_record_ids
                    .iter()
                    .chain(&claim.spec.contradicting_evidence_record_ids)
                    .chain(&claim.spec.derived_from_claim_record_ids)
                    .map(String::as_str),
            );
        }
    }
    if reached.len() == records.len() {
        Ok(())
    } else {
        Err(invalid(
            "records must be the exact mutation-root reference closure without orphans",
        ))
    }
}
