use super::{
    ActorRef, ActorType, ArtifactRef, CANONICALIZATION, MAX_EVIDENCE_REFS, MAX_RECEIPT_BYTES,
    MAX_UNIX_MILLISECONDS, MAX_VERIFICATION_CHECKS, PlatformCoreContractError, RECEIPT_VERSION,
    RejectionCode, VerificationApplicability, VerificationCheckRequest, VerificationCheckResult,
    VerificationReceipt, VerificationRequest, VerificationStatus, artifact, codec,
    execution_receipt, identity, references, reject, wire,
};

/// Validates one supplied immutable-input verification request.
///
/// # Errors
/// Returns a stable coded error for invalid declarations, references, or bounds.
pub fn validate_verification_request(
    value: &VerificationRequest,
) -> Result<(), PlatformCoreContractError> {
    validate_request_values(value)?;
    wire::canonical_typed(value, MAX_RECEIPT_BYTES)?;
    validate_input_references(&value.scope_ref, &value.input_artifact_ref)?;
    validate_request_relations(value)
}

fn validate_request_values(value: &VerificationRequest) -> Result<(), PlatformCoreContractError> {
    if value.canonicalization != CANONICALIZATION
        || value.verification_request_version != RECEIPT_VERSION
    {
        return Err(reject(
            RejectionCode::ValueInvalid,
            "verification request version or canonicalization is unsupported",
        ));
    }
    identity::validate_typed_id(&value.verification_id, "ver", "verification_id")?;
    references::validate_actor_ref(&value.requested_by)?;
    validate_unix_ms(value.requested_at_unix_ms, "requested_at_unix_ms")?;
    references::validate_scope_values(&value.scope_ref)?;
    artifact::validate_artifact_values(&value.input_artifact_ref)?;
    validate_check_requests(&value.checks)
}

fn validate_request_relations(
    value: &VerificationRequest,
) -> Result<(), PlatformCoreContractError> {
    artifact::validate_artifact_relations(&value.input_artifact_ref)?;
    if value.input_artifact_ref.created_at_unix_ms > value.requested_at_unix_ms {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "verification input cannot postdate its request",
        ));
    }
    Ok(())
}

/// Validates supplied verification observations without declaring product completion.
///
/// # Errors
/// Returns a stable coded error for invalid declarations, references, state, or relations.
pub fn validate_verification_receipt(
    value: &VerificationReceipt,
) -> Result<(), PlatformCoreContractError> {
    validate_receipt_values(value)?;
    wire::canonical_typed(value, MAX_RECEIPT_BYTES)?;
    validate_input_references(&value.scope_ref, &value.input_artifact_ref)?;
    validate_receipt_state(value)?;
    validate_receipt_relations(value)
}

fn validate_receipt_values(value: &VerificationReceipt) -> Result<(), PlatformCoreContractError> {
    validate_receipt_header(value)?;
    validate_producer(&value.produced_by)?;
    references::validate_scope_values(&value.scope_ref)?;
    artifact::validate_artifact_values(&value.input_artifact_ref)?;
    validate_unix_ms(value.started_at_unix_ms, "started_at_unix_ms")?;
    validate_unix_ms(value.ended_at_unix_ms, "ended_at_unix_ms")?;
    validate_check_result_values(&value.results)
}

/// Compares one exact request and receipt without resolving referenced evidence.
///
/// # Errors
/// Returns `pc_relation_mismatch` when the two valid records do not bind exactly.
pub fn validate_verification_exchange(
    request: &VerificationRequest,
    receipt: &VerificationReceipt,
) -> Result<(), PlatformCoreContractError> {
    wire::canonical_typed(request, MAX_RECEIPT_BYTES)?;
    wire::canonical_typed(receipt, MAX_RECEIPT_BYTES)?;
    validate_request_values(request)?;
    validate_receipt_values(receipt)?;
    validate_input_references(&request.scope_ref, &request.input_artifact_ref)?;
    validate_input_references(&receipt.scope_ref, &receipt.input_artifact_ref)?;
    validate_receipt_state(receipt)?;
    validate_request_relations(request)?;
    validate_receipt_relations(receipt)?;
    validate_exchange_relations(request, receipt)
}

fn validate_exchange_relations(
    request: &VerificationRequest,
    receipt: &VerificationReceipt,
) -> Result<(), PlatformCoreContractError> {
    let digest = codec::verification_request_sha256(request)?;
    if request.verification_id != receipt.verification_id || receipt.request_sha256 != digest {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "verification receipt does not bind the exact request",
        ));
    }
    if request.scope_ref != receipt.scope_ref
        || request.input_artifact_ref != receipt.input_artifact_ref
    {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "verification request and receipt scope or input differs",
        ));
    }
    if receipt.started_at_unix_ms < request.requested_at_unix_ms {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "verification cannot start before its request",
        ));
    }
    compare_checks(&request.checks, &receipt.results)
}

fn validate_receipt_header(value: &VerificationReceipt) -> Result<(), PlatformCoreContractError> {
    if value.canonicalization != CANONICALIZATION
        || value.verification_receipt_version != RECEIPT_VERSION
    {
        return Err(reject(
            RejectionCode::ValueInvalid,
            "verification receipt version or canonicalization is unsupported",
        ));
    }
    identity::validate_typed_id(&value.receipt_id, "rcp", "receipt_id")?;
    identity::validate_typed_id(&value.verification_id, "ver", "verification_id")?;
    wire::validate_hash(&value.request_sha256, "request_sha256")
}

fn validate_producer(value: &ActorRef) -> Result<(), PlatformCoreContractError> {
    references::validate_actor_ref(value)?;
    if value.actor_type == ActorType::Harness {
        Ok(())
    } else {
        Err(reject(
            RejectionCode::ValueInvalid,
            "verification receipt producer must declare harness actor_type",
        ))
    }
}

fn validate_input_references(
    scope: &super::ScopeRef,
    input: &ArtifactRef,
) -> Result<(), PlatformCoreContractError> {
    references::validate_scope_references(scope)?;
    artifact::validate_artifact_references(input)?;
    let (Some(snapshot), Some(attempt)) = (
        scope.project_snapshot_id.as_deref(),
        scope.attempt_id.as_deref(),
    ) else {
        return Err(reject(
            RejectionCode::ReferenceMismatch,
            "verification scope requires project snapshot and attempt",
        ));
    };
    if input.source_snapshot_ref.entity_id != snapshot || input.producer_attempt_id != attempt {
        return Err(reject(
            RejectionCode::ReferenceMismatch,
            "verification input must match scoped snapshot and attempt",
        ));
    }
    Ok(())
}

fn validate_check_requests(
    values: &[VerificationCheckRequest],
) -> Result<(), PlatformCoreContractError> {
    if values.is_empty() || values.len() > MAX_VERIFICATION_CHECKS {
        return Err(reject(
            RejectionCode::ValueInvalid,
            format!("checks cardinality must be 1..{MAX_VERIFICATION_CHECKS}"),
        ));
    }
    let mut previous = "";
    for value in values {
        validate_check_id(&value.check_id)?;
        if value.check_id.as_str() <= previous {
            return Err(reject(
                RejectionCode::ValueInvalid,
                "checks must be strictly sorted and unique by check_id",
            ));
        }
        wire::validate_schema_name(&value.check_name, "check_name")?;
        previous = &value.check_id;
    }
    Ok(())
}

fn validate_receipt_relations(
    value: &VerificationReceipt,
) -> Result<(), PlatformCoreContractError> {
    artifact::validate_artifact_relations(&value.input_artifact_ref)?;
    if value.ended_at_unix_ms < value.started_at_unix_ms
        || value.input_artifact_ref.created_at_unix_ms > value.started_at_unix_ms
    {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "verification interval or input timing is invalid",
        ));
    }
    for result in &value.results {
        validate_check_result_relations(result)?;
    }
    let derived = derive_verification_status(&value.results);
    if value.overall_status != derived {
        return Err(reject(
            RejectionCode::RelationMismatch,
            format!("overall_status must be derived as {derived:?}"),
        ));
    }
    Ok(())
}

fn validate_receipt_state(value: &VerificationReceipt) -> Result<(), PlatformCoreContractError> {
    validate_status(&value.overall_status, "overall_status")?;
    for result in &value.results {
        validate_status(&result.status, "verification status")?;
    }
    Ok(())
}

fn validate_check_result_values(
    values: &[VerificationCheckResult],
) -> Result<(), PlatformCoreContractError> {
    if values.is_empty() || values.len() > MAX_VERIFICATION_CHECKS {
        return Err(reject(
            RejectionCode::ValueInvalid,
            format!("results cardinality must be 1..{MAX_VERIFICATION_CHECKS}"),
        ));
    }
    let mut previous = "";
    for value in values {
        validate_check_result_value(value)?;
        if value.check_id.as_str() <= previous {
            return Err(reject(
                RejectionCode::ValueInvalid,
                "results must be strictly sorted and unique by check_id",
            ));
        }
        previous = &value.check_id;
    }
    Ok(())
}

fn validate_check_result_value(
    value: &VerificationCheckResult,
) -> Result<(), PlatformCoreContractError> {
    validate_check_id(&value.check_id)?;
    validate_applicability_value(&value.applicability)?;
    if let Some(reason) = value.applicability_reason.as_deref() {
        wire::validate_text(reason, "applicability_reason", 512, true)?;
    }
    execution_receipt::validate_reason_codes(&value.reason_codes, "result.reason_codes")?;
    validate_evidence_refs(&value.evidence_refs)
}

fn validate_check_result_relations(
    value: &VerificationCheckResult,
) -> Result<(), PlatformCoreContractError> {
    validate_applicability(value)?;
    if (value.status == VerificationStatus::Pass) != value.reason_codes.is_empty() {
        return Err(reject(
            RejectionCode::RelationMismatch,
            "pass requires no reasons; other statuses require reasons",
        ));
    }
    Ok(())
}

fn derive_verification_status(values: &[VerificationCheckResult]) -> VerificationStatus {
    let mut overall = VerificationStatus::Pass;
    let mut applicable = false;
    for value in values {
        if value.applicability == VerificationApplicability::Applicable {
            applicable = true;
            overall = more_severe(overall, &value.status);
        }
    }
    if applicable {
        overall
    } else {
        VerificationStatus::NotExecuted
    }
}

fn validate_applicability(
    value: &VerificationCheckResult,
) -> Result<(), PlatformCoreContractError> {
    match &value.applicability {
        VerificationApplicability::Applicable if value.applicability_reason.is_none() => Ok(()),
        VerificationApplicability::NotApplicable
            if value.status == VerificationStatus::NotExecuted
                && value.applicability_reason.is_some() =>
        {
            wire::validate_text(
                value.applicability_reason.as_deref().unwrap_or_default(),
                "applicability_reason",
                512,
                true,
            )
        }
        VerificationApplicability::Applicable => Err(reject(
            RejectionCode::RelationMismatch,
            "applicable result requires null applicability_reason",
        )),
        VerificationApplicability::NotApplicable => Err(reject(
            RejectionCode::RelationMismatch,
            "not_applicable requires not_executed and a reason",
        )),
        VerificationApplicability::Unknown(_) => Err(reject(
            RejectionCode::ValueInvalid,
            "applicability is unsupported",
        )),
    }
}

fn validate_evidence_refs(values: &[super::RecordRef]) -> Result<(), PlatformCoreContractError> {
    if values.len() > MAX_EVIDENCE_REFS {
        return Err(reject(
            RejectionCode::ValueInvalid,
            format!("evidence_refs exceeds {MAX_EVIDENCE_REFS} items"),
        ));
    }
    let mut previous = "";
    for value in values {
        references::validate_record_ref(value, "evidence_ref")?;
        if value.record_id.as_str() <= previous {
            return Err(reject(
                RejectionCode::ValueInvalid,
                "evidence_refs must be strictly sorted and unique",
            ));
        }
        previous = &value.record_id;
    }
    Ok(())
}

fn validate_check_id(value: &str) -> Result<(), PlatformCoreContractError> {
    if wire::lower_token(value) && value.len() <= 64 {
        Ok(())
    } else {
        Err(reject(
            RejectionCode::ValueInvalid,
            "check_id must be a bounded lowercase token",
        ))
    }
}

fn more_severe(left: VerificationStatus, right: &VerificationStatus) -> VerificationStatus {
    if status_severity(right) > status_severity(&left) {
        right.clone()
    } else {
        left
    }
}

fn status_severity(value: &VerificationStatus) -> u8 {
    match value {
        VerificationStatus::Pass | VerificationStatus::Unknown(_) => 0,
        VerificationStatus::NotExecuted => 1,
        VerificationStatus::Inconclusive => 2,
        VerificationStatus::Fail => 3,
    }
}

fn validate_status(
    value: &VerificationStatus,
    label: &str,
) -> Result<(), PlatformCoreContractError> {
    if matches!(value, VerificationStatus::Unknown(_)) {
        Err(reject(
            RejectionCode::StateInvalid,
            format!("{label} is unsupported"),
        ))
    } else {
        Ok(())
    }
}

fn validate_applicability_value(
    value: &VerificationApplicability,
) -> Result<(), PlatformCoreContractError> {
    if matches!(value, VerificationApplicability::Unknown(_)) {
        Err(reject(
            RejectionCode::ValueInvalid,
            "applicability is unsupported",
        ))
    } else {
        Ok(())
    }
}

fn compare_checks(
    requests: &[VerificationCheckRequest],
    results: &[VerificationCheckResult],
) -> Result<(), PlatformCoreContractError> {
    if requests.len() == results.len()
        && requests
            .iter()
            .zip(results)
            .all(|(request, result)| request.check_id == result.check_id)
    {
        Ok(())
    } else {
        Err(reject(
            RejectionCode::RelationMismatch,
            "verification result set must exactly cover requested checks",
        ))
    }
}

fn validate_unix_ms(value: i64, label: &str) -> Result<(), PlatformCoreContractError> {
    if (1..=MAX_UNIX_MILLISECONDS).contains(&value) {
        Ok(())
    } else {
        Err(reject(
            RejectionCode::ValueInvalid,
            format!("{label} is outside the supported range"),
        ))
    }
}
