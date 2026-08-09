use crate::governance_contract::{
    API_VERSION as GOVERNANCE_API_VERSION, CANONICALIZATION as GOVERNANCE_CANONICALIZATION,
    Collector, CollectorType, ContentRole, Directness, EvidenceLocator, EvidenceRecord,
    EvidenceRecordKind, EvidenceSpec, EvidenceState, EvidenceStatus, EvidenceType,
    GovernanceRecord, Integrity, LocatorType, Principal, PrincipalType, RecordMetadata,
    SnapshotType, SourceSnapshot, SourceTrust, decode_canonical_record,
};

use super::{
    ADAPTED_SHADOW, ArtifactEvidenceAdaptation, ArtifactEvidenceContractError,
    ArtifactEvidenceRequest, artifact_source_sha256, canonical_request_json,
    decode_canonical_request, invalid, request_sha256,
};

const ADAPTER_PRINCIPAL_ID: &str = "forgeos.artifact-evidence-adapter";
const ADAPTER_ROLE: &str = "evidence-adapter";
const ADAPTER_VERSION: &str = "v1";

/// Deterministically adapts one exact canonical artifact request into `EvidenceRecord` v1.
///
/// The function is pure and does not read a repository, use an ambient clock, append
/// persistence, create claims or atoms, grant authority, or perform an effect.
///
/// # Errors
///
/// Returns an error for invalid input, mapping drift, or failed strict governance
/// revalidation of the final exact bytes.
pub fn adapt_canonical_request(
    request_bytes: &[u8],
) -> Result<ArtifactEvidenceAdaptation, ArtifactEvidenceContractError> {
    let request = decode_canonical_request(request_bytes)?;
    let canonical_request_json = canonical_request_json(&request)?;
    let request_sha256 = request_sha256(&request)?;
    let source_sha256 = artifact_source_sha256(&request.artifact)?;
    let observed_at = super::timestamp::unix_millis_floor(&request.artifact.created_at)?;
    let evidence = build_evidence(&request, &request_sha256, &source_sha256, observed_at);
    seal_and_revalidate(
        evidence,
        canonical_request_json,
        request_sha256,
        source_sha256,
    )
}

/// Re-adapts the exact source request and requires byte-for-byte `EvidenceRecord` parity.
///
/// # Errors
///
/// Returns an error when either input is invalid or the supplied `EvidenceRecord` is not
/// the exact deterministic adapter output.
pub fn validate_adaptation(
    request_bytes: &[u8],
    evidence_bytes: &[u8],
) -> Result<ArtifactEvidenceAdaptation, ArtifactEvidenceContractError> {
    let supplied = decode_canonical_record(evidence_bytes)
        .map_err(|error| invalid(format!("supplied EvidenceRecord: {error}")))?;
    if !matches!(supplied, GovernanceRecord::Evidence(_)) {
        return Err(invalid("supplied governance record is not EvidenceRecord"));
    }
    let adapted = adapt_canonical_request(request_bytes)?;
    if adapted.canonical_evidence_json.as_bytes() != evidence_bytes {
        return Err(invalid(
            "EvidenceRecord is not the exact deterministic artifact adaptation",
        ));
    }
    Ok(adapted)
}

fn build_evidence(
    request: &ArtifactEvidenceRequest,
    request_digest: &str,
    source_digest: &str,
    observed_at: i64,
) -> EvidenceRecord {
    EvidenceRecord {
        api_version: GOVERNANCE_API_VERSION.to_owned(),
        integrity: Integrity {
            canonical_sha256: String::new(),
            canonicalization: GOVERNANCE_CANONICALIZATION.to_owned(),
        },
        kind: EvidenceRecordKind::EvidenceRecord,
        metadata: build_metadata(request, request_digest, observed_at),
        spec: build_spec(request, request_digest, source_digest, observed_at),
        status: EvidenceStatus {
            reason_codes: Vec::new(),
            state: EvidenceState::Valid,
            valid_from_unix_ms: observed_at,
            valid_until_unix_ms: None,
        },
    }
}

fn build_metadata(
    request: &ArtifactEvidenceRequest,
    request_digest: &str,
    observed_at: i64,
) -> RecordMetadata {
    let binding = &request.binding;
    RecordMetadata {
        aggregate_id: binding.aggregate_id.clone(),
        context_sha256: binding.context_sha256.clone(),
        created_at_unix_ms: observed_at,
        created_by: fixed_principal(&request.artifact.run_id),
        policy_sha256: binding.policy_sha256.clone(),
        project_id: binding.project_id.clone(),
        record_id: format!("artifact-evidence-{request_digest}"),
        scope: binding.scope.clone(),
        sequence: binding.sequence,
        source_revision: binding.source_revision.clone(),
        source_tree_sha256: binding.source_tree_sha256.clone(),
        supersedes_record_ids: binding.supersedes_record_ids.clone(),
    }
}

fn build_spec(
    request: &ArtifactEvidenceRequest,
    request_digest: &str,
    source_digest: &str,
    observed_at: i64,
) -> EvidenceSpec {
    EvidenceSpec {
        artifact_sha256: Some(request.artifact.sha256.clone()),
        collector: fixed_collector(&request.artifact.run_id, request_digest),
        content_role: ContentRole::UntrustedData,
        directness: Directness::Direct,
        evidence_type: EvidenceType::Artifact,
        locator: artifact_locator(request),
        observed_at_unix_ms: observed_at,
        sensitivity: request.binding.sensitivity,
        source_snapshot: SourceSnapshot {
            snapshot_id: format!("artifact-snapshot-{source_digest}"),
            snapshot_sha256: source_digest.to_owned(),
            snapshot_type: SnapshotType::Artifact,
        },
        source_trust: SourceTrust::Observed,
        subjects: request.binding.subjects.clone(),
    }
}

fn fixed_principal(run_id: &str) -> Principal {
    Principal {
        authority_domain: "shadow".to_owned(),
        principal_id: ADAPTER_PRINCIPAL_ID.to_owned(),
        principal_type: PrincipalType::Tool,
        role: ADAPTER_ROLE.to_owned(),
        run_id: run_id.to_owned(),
    }
}

fn fixed_collector(run_id: &str, request_digest: &str) -> Collector {
    Collector {
        collector_id: ADAPTER_PRINCIPAL_ID.to_owned(),
        collector_type: CollectorType::Tool,
        collector_version: ADAPTER_VERSION.to_owned(),
        parameters_sha256: request_digest.to_owned(),
        run_id: run_id.to_owned(),
    }
}

fn artifact_locator(request: &ArtifactEvidenceRequest) -> EvidenceLocator {
    EvidenceLocator {
        content_sha256: request.artifact.sha256.clone(),
        exit_code: None,
        line_end: None,
        line_start: None,
        locator_ref: request.artifact.path.clone(),
        locator_type: LocatorType::Artifact,
    }
}

fn seal_and_revalidate(
    evidence: EvidenceRecord,
    canonical_request_json: String,
    request_sha256: String,
    source_sha256: String,
) -> Result<ArtifactEvidenceAdaptation, ArtifactEvidenceContractError> {
    let mut record = GovernanceRecord::Evidence(evidence);
    let digest = record
        .expected_sha256()
        .map_err(|error| invalid(format!("seal EvidenceRecord: {error}")))?;
    let GovernanceRecord::Evidence(evidence) = &mut record else {
        unreachable!("adapter only constructs EvidenceRecord")
    };
    evidence.integrity.canonical_sha256 = digest;
    let canonical_evidence_json = record
        .canonical_record_json()
        .map_err(|error| invalid(format!("encode EvidenceRecord: {error}")))?;
    let decoded = decode_canonical_record(canonical_evidence_json.as_bytes())
        .map_err(|error| invalid(format!("strict EvidenceRecord revalidation: {error}")))?;
    let GovernanceRecord::Evidence(evidence) = decoded else {
        return Err(invalid("strict revalidation changed EvidenceRecord kind"));
    };
    Ok(ArtifactEvidenceAdaptation {
        canonical_evidence_json,
        canonical_request_json,
        evidence,
        request_sha256,
        result: ADAPTED_SHADOW,
        source_sha256,
    })
}
