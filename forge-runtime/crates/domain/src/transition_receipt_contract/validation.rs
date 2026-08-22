use crate::capability_grant_contract::{ApprovalRef, GrantTaskBinding, Principal};

use super::{
    ASSESSMENT_REQUEST_API_VERSION, ApplicabilityDecision, ApplicabilityDeclaration,
    CANONICALIZATION, CapabilityGrantRef, EvidenceRef, MAX_ASSESSMENT_REQUEST_BYTES,
    MAX_DECLARED_TARGET_BYTES, MAX_RECEIPT_BYTES, MAX_TOTAL_EVIDENCE_REFS, MAX_TOTAL_REASON_CODES,
    MAX_VOCABULARY_BYTES, RECEIPT_API_VERSION, RECEIPT_KIND, TRANSITION_VOCABULARY_SHA256,
    TransitionArtifact, TransitionAssessmentRequest, TransitionBindings, TransitionDeclaration,
    TransitionDeclaredTarget, TransitionPrecondition, TransitionReceipt,
    TransitionReceiptContractError, TransitionState, TransitionStateVocabulary,
    VOCABULARY_API_VERSION, VOCABULARY_KIND, WaiverRef, codec, invalid, primitives, vocabulary,
};

pub(super) fn validate_vocabulary(
    value: &TransitionStateVocabulary,
    allow_empty_digest: bool,
) -> Result<(), TransitionReceiptContractError> {
    let envelope = value.api_version == VOCABULARY_API_VERSION
        && value.canonicalization == CANONICALIZATION
        && value.kind == VOCABULARY_KIND;
    let authored = value.states == vocabulary::STATES
        && value.terminal_states == vocabulary::TERMINAL_STATES
        && value.rework_targets == vocabulary::REWORK_TARGETS
        && value.edges == vocabulary::authored_edges();
    if !envelope || !authored {
        return Err(invalid(
            "Transition vocabulary differs from the authored graph",
        ));
    }
    codec::bounded(value, MAX_VOCABULARY_BYTES, "Transition vocabulary")?;
    if allow_empty_digest && value.vocabulary_sha256.is_empty() {
        return Ok(());
    }
    primitives::sha256(&value.vocabulary_sha256, "vocabulary_sha256")?;
    if codec::vocabulary_sha256_unchecked(value)? == TRANSITION_VOCABULARY_SHA256
        && value.vocabulary_sha256 == TRANSITION_VOCABULARY_SHA256
    {
        Ok(())
    } else {
        Err(invalid("Transition vocabulary self digest does not match"))
    }
}

struct Core<'a> {
    actor: &'a Principal,
    applicability: &'a ApplicabilityDeclaration,
    approval_refs: &'a [ApprovalRef],
    bindings: &'a TransitionBindings,
    capability_grant_ref: &'a CapabilityGrantRef,
    declared_controller: &'a Principal,
    preconditions: &'a [TransitionPrecondition],
    previous_receipt_id: &'a Option<String>,
    previous_receipt_sha256: &'a Option<String>,
    reason_codes: &'a [String],
    sequence: i64,
    task_binding: &'a GrantTaskBinding,
    transition: &'a TransitionDeclaration,
    transition_vocabulary_sha256: &'a str,
    waiver_refs: &'a [WaiverRef],
    work_id: &'a str,
}

impl<'a> From<&'a TransitionReceipt> for Core<'a> {
    fn from(value: &'a TransitionReceipt) -> Self {
        Self {
            actor: &value.actor,
            applicability: &value.applicability,
            approval_refs: &value.approval_refs,
            bindings: &value.bindings,
            capability_grant_ref: &value.capability_grant_ref,
            declared_controller: &value.declared_controller,
            preconditions: &value.preconditions,
            previous_receipt_id: &value.previous_receipt_id,
            previous_receipt_sha256: &value.previous_receipt_sha256,
            reason_codes: &value.reason_codes,
            sequence: value.sequence,
            task_binding: &value.task_binding,
            transition: &value.transition,
            transition_vocabulary_sha256: &value.transition_vocabulary_sha256,
            waiver_refs: &value.waiver_refs,
            work_id: &value.work_id,
        }
    }
}

impl<'a> From<&'a TransitionDeclaredTarget> for Core<'a> {
    fn from(value: &'a TransitionDeclaredTarget) -> Self {
        Self {
            actor: &value.actor,
            applicability: &value.applicability,
            approval_refs: &value.approval_refs,
            bindings: &value.bindings,
            capability_grant_ref: &value.capability_grant_ref,
            declared_controller: &value.declared_controller,
            preconditions: &value.preconditions,
            previous_receipt_id: &value.previous_receipt_id,
            previous_receipt_sha256: &value.previous_receipt_sha256,
            reason_codes: &value.reason_codes,
            sequence: value.sequence,
            task_binding: &value.task_binding,
            transition: &value.transition,
            transition_vocabulary_sha256: &value.transition_vocabulary_sha256,
            waiver_refs: &value.waiver_refs,
            work_id: &value.work_id,
        }
    }
}

pub(super) fn validate_receipt(
    value: &TransitionReceipt,
    allow_empty_identity: bool,
) -> Result<(), TransitionReceiptContractError> {
    if value.api_version != RECEIPT_API_VERSION
        || value.canonicalization != CANONICALIZATION
        || value.kind != RECEIPT_KIND
    {
        return Err(invalid(
            "TransitionReceipt envelope drifted; aliases are rejected",
        ));
    }
    validate_core(&Core::from(value), "TransitionReceipt")?;
    codec::bounded(value, MAX_RECEIPT_BYTES, "TransitionReceipt")?;
    if allow_empty_identity && value.receipt_id.is_empty() && value.receipt_sha256.is_empty() {
        return Ok(());
    }
    primitives::sha256(&value.receipt_sha256, "receipt_sha256")?;
    let expected_id = format!("transition-receipt-{}", value.receipt_sha256);
    if value.receipt_id != expected_id
        || codec::receipt_sha256_unchecked(value)? != value.receipt_sha256
    {
        Err(invalid(
            "TransitionReceipt identity or self digest does not match",
        ))
    } else {
        Ok(())
    }
}

pub(super) fn validate_target(
    value: &TransitionDeclaredTarget,
) -> Result<(), TransitionReceiptContractError> {
    validate_core(&Core::from(value), "Transition declared target")?;
    codec::bounded(
        value,
        MAX_DECLARED_TARGET_BYTES,
        "Transition declared target",
    )?;
    Ok(())
}

fn validate_core(value: &Core<'_>, label: &str) -> Result<(), TransitionReceiptContractError> {
    primitives::principal(value.actor, &format!("{label}.actor"))?;
    primitives::controller(
        value.declared_controller,
        &format!("{label}.declared_controller"),
    )?;
    primitives::task_binding(value.task_binding, &format!("{label}.task_binding"))?;
    validate_bindings(value.bindings, &format!("{label}.bindings"))?;
    validate_grant_ref(
        value.capability_grant_ref,
        &format!("{label}.capability_grant_ref"),
    )?;
    validate_approval_refs(value.approval_refs, &format!("{label}.approval_refs"))?;
    validate_waiver_refs(value.waiver_refs, &format!("{label}.waiver_refs"))?;
    let (pre_reasons, pre_evidence) =
        validate_preconditions(value.preconditions, &format!("{label}.preconditions"))?;
    let (app_reasons, app_evidence) = validate_applicability(
        value.applicability,
        value.transition.to_state,
        &format!("{label}.applicability"),
    )?;
    validate_transition(value.transition, label)?;
    validate_core_limits(
        value,
        pre_reasons + app_reasons,
        pre_evidence + app_evidence,
        label,
    )
}

fn validate_core_limits(
    value: &Core<'_>,
    nested_reasons: usize,
    evidence: usize,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    primitives::sorted_reasons(value.reason_codes, 256, &format!("{label}.reason_codes"))?;
    if value.reason_codes.len() + nested_reasons > MAX_TOTAL_REASON_CODES
        || evidence > MAX_TOTAL_EVIDENCE_REFS
    {
        return Err(invalid(format!(
            "{label} exceeds aggregate declaration limits"
        )));
    }
    validate_predecessor(value, label)?;
    primitives::short_text(value.work_id, &format!("{label}.work_id"))?;
    if value.transition_vocabulary_sha256 == TRANSITION_VOCABULARY_SHA256 {
        Ok(())
    } else {
        Err(invalid(format!(
            "{label} does not bind the frozen vocabulary"
        )))
    }
}

fn validate_bindings(
    value: &TransitionBindings,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    if value.artifacts.len() > 32 {
        return Err(invalid(format!("{label}.artifacts exceeds 32 items")));
    }
    for artifact in &value.artifacts {
        validate_artifact(artifact, label)?;
    }
    primitives::sorted_nodes(&value.artifacts, &format!("{label}.artifacts"))?;
    for (field, digest) in [
        ("context_sha256", value.context_sha256.as_str()),
        ("policy_sha256", value.policy_sha256.as_str()),
        ("source_tree_sha256", value.source_tree_sha256.as_str()),
    ] {
        primitives::sha256(digest, &format!("{label}.{field}"))?;
    }
    primitives::optional_sha256(
        value.impact_sha256.as_deref(),
        &format!("{label}.impact_sha256"),
    )?;
    primitives::optional_sha256(
        value.plan_sha256.as_deref(),
        &format!("{label}.plan_sha256"),
    )?;
    primitives::optional_sha256(
        value.risk_sha256.as_deref(),
        &format!("{label}.risk_sha256"),
    )?;
    primitives::short_text(&value.source_revision, &format!("{label}.source_revision"))
}

fn validate_artifact(
    value: &TransitionArtifact,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    primitives::short_text(&value.artifact_kind, &format!("{label}.artifact_kind"))?;
    primitives::reference_text(&value.artifact_ref, &format!("{label}.artifact_ref"))?;
    primitives::sha256(&value.artifact_sha256, &format!("{label}.artifact_sha256"))
}

fn validate_grant_ref(
    value: &CapabilityGrantRef,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    primitives::short_text(
        &value.authority_domain,
        &format!("{label}.authority_domain"),
    )?;
    primitives::sha256(&value.grant_sha256, &format!("{label}.grant_sha256"))?;
    if value.grant_id == format!("capability-grant-{}", value.grant_sha256) {
        Ok(())
    } else {
        Err(invalid(format!("{label} identity is inconsistent")))
    }
}

fn validate_approval_refs(
    values: &[ApprovalRef],
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    if values.len() > 32 {
        return Err(invalid(format!("{label} exceeds 32 items")));
    }
    for value in values {
        primitives::approval_ref(value, label)?;
    }
    primitives::sorted_nodes(values, label)
}

fn validate_waiver_refs(
    values: &[WaiverRef],
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    if values.len() > 32 {
        return Err(invalid(format!("{label} exceeds 32 items")));
    }
    for value in values {
        primitives::short_text(&value.authority_domain, label)?;
        primitives::short_text(&value.waiver_id, label)?;
        primitives::sha256(&value.waiver_sha256, label)?;
    }
    primitives::sorted_nodes(values, label)
}

fn validate_preconditions(
    values: &[TransitionPrecondition],
    label: &str,
) -> Result<(usize, usize), TransitionReceiptContractError> {
    if values.is_empty() || values.len() > 64 {
        return Err(invalid(format!("{label} item count must be 1..64")));
    }
    let mut reasons = 0;
    let mut evidence = 0;
    for value in values {
        primitives::short_text(&value.precondition_id, label)?;
        primitives::sorted_reasons(&value.reason_codes, 16, label)?;
        validate_evidence_refs(&value.evidence_refs, label)?;
        reasons += value.reason_codes.len();
        evidence += value.evidence_refs.len();
    }
    primitives::sorted_nodes(values, label)?;
    Ok((reasons, evidence))
}

fn validate_applicability(
    value: &ApplicabilityDeclaration,
    target: TransitionState,
    label: &str,
) -> Result<(usize, usize), TransitionReceiptContractError> {
    primitives::short_text(&value.stage_id, &format!("{label}.stage_id"))?;
    primitives::sorted_reasons(&value.reason_codes, 16, &format!("{label}.reason_codes"))?;
    validate_evidence_refs(&value.evidence_refs, &format!("{label}.evidence_refs"))?;
    let consistent = value.stage_id == target.as_str()
        && match value.decision {
            ApplicabilityDecision::Applicable => value.reason_codes.is_empty(),
            ApplicabilityDecision::NotApplicable => {
                !value.reason_codes.is_empty() && !value.evidence_refs.is_empty()
            }
        };
    if consistent {
        Ok((value.reason_codes.len(), value.evidence_refs.len()))
    } else {
        Err(invalid(format!("{label} is internally inconsistent")))
    }
}

fn validate_evidence_refs(
    values: &[EvidenceRef],
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    if values.len() > 32 {
        return Err(invalid(format!("{label} exceeds 32 evidence refs")));
    }
    for value in values {
        primitives::sha256(&value.canonical_sha256, label)?;
        primitives::short_text(&value.record_id, label)?;
    }
    primitives::sorted_nodes(values, label)
}

fn validate_transition(
    value: &TransitionDeclaration,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    if value.declared_at_unix_ms < 0 {
        return Err(invalid(format!("{label}.declared_at_unix_ms is negative")));
    }
    if let Some(gate_id) = &value.gate_id {
        primitives::short_text(gate_id, &format!("{label}.gate_id"))?;
    }
    let rework_ok = (value.rework_target.is_some()
        == (value.to_state == TransitionState::ChangesRequested))
        && value
            .rework_target
            .is_none_or(|state| vocabulary::REWORK_TARGETS.contains(&state));
    let suspended = matches!(
        value.to_state,
        TransitionState::NeedsInfo | TransitionState::Blocked
    );
    let resume_ok = value.resume_state.is_some() == suspended
        && (value.resume_state.is_none()
            || (value.from_state == TransitionState::NeedsInfo
                && value.to_state == TransitionState::Blocked)
            || value.resume_state == Some(value.from_state));
    if rework_ok && resume_ok {
        Ok(())
    } else {
        Err(invalid(format!("{label} recovery fields are inconsistent")))
    }
}

fn validate_predecessor(
    value: &Core<'_>,
    label: &str,
) -> Result<(), TransitionReceiptContractError> {
    if value.sequence < 1 {
        return Err(invalid(format!("{label}.sequence must be positive")));
    }
    match (
        value.sequence,
        value.previous_receipt_id.as_deref(),
        value.previous_receipt_sha256.as_deref(),
    ) {
        (1, None, None) if value.transition.from_state == TransitionState::Draft => Ok(()),
        (sequence, Some(identifier), Some(digest)) if sequence > 1 => {
            primitives::sha256(digest, &format!("{label}.previous_receipt_sha256"))?;
            if identifier == format!("transition-receipt-{digest}") {
                Ok(())
            } else {
                Err(invalid(format!(
                    "{label} predecessor identity is inconsistent"
                )))
            }
        }
        _ => Err(invalid(format!(
            "{label} predecessor declaration is inconsistent"
        ))),
    }
}

pub(super) fn validate_request(
    value: &TransitionAssessmentRequest,
    allow_empty_digest: bool,
) -> Result<(), TransitionReceiptContractError> {
    if value.api_version != ASSESSMENT_REQUEST_API_VERSION
        || value.canonicalization != CANONICALIZATION
        || value.evaluated_at_unix_ms < 0
    {
        return Err(invalid("Transition assessment request envelope drifted"));
    }
    validate_receipt(&value.transition_receipt, false)?;
    if let Some(previous) = &value.previous_receipt {
        validate_receipt(previous, false)?;
    }
    validate_target(&value.expected_target)?;
    primitives::sha256(&value.expected_target_sha256, "expected_target_sha256")?;
    if codec::declared_target_sha256(&value.expected_target)? != value.expected_target_sha256 {
        return Err(invalid("expected target digest does not match"));
    }
    validate_request_digest(value, allow_empty_digest)?;
    codec::bounded(
        value,
        MAX_ASSESSMENT_REQUEST_BYTES,
        "Transition assessment request",
    )?;
    Ok(())
}

fn validate_request_digest(
    value: &TransitionAssessmentRequest,
    allow_empty: bool,
) -> Result<(), TransitionReceiptContractError> {
    if allow_empty && value.request_sha256.is_empty() {
        return Ok(());
    }
    primitives::sha256(&value.request_sha256, "request_sha256")?;
    if codec::assessment_request_sha256_unchecked(value)? == value.request_sha256 {
        Ok(())
    } else {
        Err(invalid(
            "Transition assessment request self digest does not match",
        ))
    }
}
