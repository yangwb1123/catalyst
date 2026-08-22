use std::{fs, path::PathBuf};

use serde::Deserialize;

use crate::{capability_grant_contract::ScopeResource, governance_contract::GovernanceRecord};

use super::super::*;

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct Golden {
    pub(super) assessment_request: KnowledgeUpdateAssessmentRequest,
    pub(super) expected_artifact_resources: Vec<ScopeResource>,
    pub(super) expected_assessment: KnowledgeUpdateDeclaredAssessment,
    pub(super) expected_capability_grant_ref: CapabilityGrantRef,
    pub(super) knowledge_update_proposal: KnowledgeUpdateProposal,
}

pub(super) fn fixture() -> Golden {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../docs/contracts/fixtures/knowledge-update-proposal-v1.json");
    let bytes = fs::read(path).expect("read KnowledgeUpdateProposal golden");
    assert!(bytes.len() <= MAX_GOLDEN_BYTES);
    serde_json::from_slice(&bytes).expect("decode KnowledgeUpdateProposal golden")
}

pub(super) fn reseal_record(record: &mut GovernanceRecord) {
    match record {
        GovernanceRecord::Evidence(value) => value.integrity.canonical_sha256.clear(),
        GovernanceRecord::Claim(value) => value.integrity.canonical_sha256.clear(),
    }
    let digest = record.expected_sha256().expect("record digest");
    match record {
        GovernanceRecord::Evidence(value) => value.integrity.canonical_sha256 = digest,
        GovernanceRecord::Claim(value) => value.integrity.canonical_sha256 = digest,
    }
}

pub(super) fn reseal_proposal(
    proposal: &KnowledgeUpdateProposal,
) -> Result<KnowledgeUpdateProposal, KnowledgeUpdateProposalContractError> {
    let mut candidate = proposal.clone();
    candidate.record_set_sha256 = record_set_sha256(&candidate)?;
    candidate.proposal_id.clear();
    candidate.proposal_sha256.clear();
    seal_proposal(&candidate)
}

pub(super) fn reseal_request(request: &mut KnowledgeUpdateAssessmentRequest) {
    request.expected_target_sha256 =
        declared_target_sha256(&request.expected_target).expect("target digest");
    request.request_sha256.clear();
    request.request_sha256 =
        super::super::codec::assessment_request_sha256_unchecked(request).expect("request digest");
}
