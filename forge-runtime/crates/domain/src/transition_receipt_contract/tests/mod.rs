use std::path::PathBuf;

use serde::Deserialize;

use crate::capability_grant_contract::ApprovalRef;

use super::{
    ApplicabilityDecision, CapabilityGrantRef, TransitionAssessmentRequest,
    TransitionDeclaredAssessment, TransitionReceipt, TransitionState, TransitionStateVocabulary,
    assessment_request_sha256, declared_target_sha256, receipt_sha256,
};

const FIXTURE: &str =
    include_str!("../../../../../../docs/contracts/fixtures/transition-receipt-v1.json");

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenFixture {
    assessment_request: TransitionAssessmentRequest,
    expected_approval_refs: Vec<ApprovalRef>,
    expected_assessment: TransitionDeclaredAssessment,
    expected_capability_grant_ref: CapabilityGrantRef,
    transition_receipt: TransitionReceipt,
    transition_vocabulary: TransitionStateVocabulary,
}

fn fixture() -> GoldenFixture {
    serde_json::from_str(FIXTURE).expect("decode TransitionReceipt golden")
}

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../..")
}

fn reseal_receipt(value: &mut TransitionReceipt) {
    value.receipt_id.clear();
    value.receipt_sha256.clear();
    value.receipt_sha256 = receipt_sha256(value).expect("receipt digest");
    value.receipt_id = format!("transition-receipt-{}", value.receipt_sha256);
}

fn reseal_request(value: &mut TransitionAssessmentRequest) {
    value.expected_target_sha256 =
        declared_target_sha256(&value.expected_target).expect("target digest");
    value.request_sha256.clear();
    value.request_sha256 = assessment_request_sha256(value).expect("request digest");
}

fn prior_entering(target: TransitionState) -> TransitionReceipt {
    let mut receipt = fixture().transition_receipt;
    receipt.transition.to_state = target;
    receipt.transition.resume_state = match target {
        TransitionState::NeedsInfo | TransitionState::Blocked => Some(TransitionState::Draft),
        _ => None,
    };
    receipt.transition.rework_target =
        (target == TransitionState::ChangesRequested).then_some(TransitionState::Verifying);
    receipt.applicability.stage_id = target.as_str().into();
    reseal_receipt(&mut receipt);
    receipt
}

fn successor(previous: &TransitionReceipt, target: TransitionState) -> TransitionReceipt {
    let mut receipt = fixture().transition_receipt;
    let source = previous.transition.to_state;
    receipt.sequence = previous.sequence + 1;
    receipt.previous_receipt_id = Some(previous.receipt_id.clone());
    receipt.previous_receipt_sha256 = Some(previous.receipt_sha256.clone());
    receipt.transition.declared_at_unix_ms = previous.transition.declared_at_unix_ms + 1;
    receipt.transition.from_state = source;
    receipt.transition.to_state = target;
    receipt.transition.rework_target =
        (target == TransitionState::ChangesRequested).then_some(TransitionState::Verifying);
    receipt.transition.resume_state = resume_for(previous, target);
    receipt.applicability.stage_id = target.as_str().into();
    receipt.applicability.decision = ApplicabilityDecision::Applicable;
    receipt.applicability.reason_codes.clear();
    reseal_receipt(&mut receipt);
    receipt
}

fn resume_for(previous: &TransitionReceipt, target: TransitionState) -> Option<TransitionState> {
    let source = previous.transition.to_state;
    if !matches!(
        target,
        TransitionState::NeedsInfo | TransitionState::Blocked
    ) {
        return None;
    }
    if source == TransitionState::NeedsInfo && target == TransitionState::Blocked {
        previous.transition.resume_state
    } else {
        Some(source)
    }
}

mod bounds;
mod compatibility;
mod evaluator;
mod golden;
mod strict;
