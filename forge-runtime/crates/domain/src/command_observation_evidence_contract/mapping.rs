use crate::governance_contract::{
    API_VERSION as GOVERNANCE_API_VERSION, CANONICALIZATION as GOVERNANCE_CANONICALIZATION,
    Collector, CollectorType, ContentRole, Directness, EvidenceLocator, EvidenceRecord,
    EvidenceRecordKind, EvidenceSpec, EvidenceState, EvidenceStatus, EvidenceType,
    GovernanceRecord, Integrity, LocatorType, Principal, PrincipalType, RecordMetadata,
    SnapshotType, SourceSnapshot, SourceTrust, decode_canonical_record,
};

use super::{
    ADAPTED_SHADOW, CommandEvidenceType, CommandObservationEvidenceAdaptation,
    CommandObservationEvidenceContractError, CommandObservationEvidenceRequest, CommandProducer,
    CommandProducerType, canonical_command_json, canonical_observation_json,
    canonical_request_json, command_sha256, decode_canonical_request, invalid, request_sha256,
    source_snapshot_sha256,
};

const ADAPTER_PRINCIPAL_ID: &str = "forgeos.command-observation-evidence-adapter";
const ADAPTER_ROLE: &str = "evidence-adapter";

struct AdaptationInputs {
    canonical_command_json: String,
    canonical_observation_json: String,
    canonical_request_json: String,
    command_sha256: String,
    request_sha256: String,
    source_snapshot_sha256: String,
}

/// Deterministically adapts exact canonical command observation bytes.
///
/// This pure function does not execute a command, read ambient state, persist a
/// record, or attest pass, completion, truth, authority, identity, or effects.
///
/// # Errors
///
/// Returns an error for invalid input, mapping drift, or failed strict governance
/// revalidation of the final exact bytes.
pub fn adapt_canonical_request(
    request_bytes: &[u8],
) -> Result<CommandObservationEvidenceAdaptation, CommandObservationEvidenceContractError> {
    let request = decode_canonical_request(request_bytes)?;
    let inputs = AdaptationInputs {
        canonical_command_json: canonical_command_json(&request.observation.command)?,
        canonical_observation_json: canonical_observation_json(&request.observation)?,
        canonical_request_json: canonical_request_json(&request)?,
        command_sha256: command_sha256(&request)?,
        request_sha256: request_sha256(&request)?,
        source_snapshot_sha256: source_snapshot_sha256(&request)?,
    };
    let evidence = build_evidence(
        &request,
        &inputs.command_sha256,
        &inputs.source_snapshot_sha256,
        &inputs.request_sha256,
    )?;
    seal_and_revalidate(evidence, inputs)
}

/// Requires supplied Evidence bytes to equal exact deterministic re-adaptation.
///
/// # Errors
///
/// Returns an error when either input is invalid or the supplied record drifts.
pub fn validate_adaptation(
    request_bytes: &[u8],
    evidence_bytes: &[u8],
) -> Result<CommandObservationEvidenceAdaptation, CommandObservationEvidenceContractError> {
    let supplied = decode_canonical_record(evidence_bytes)
        .map_err(|error| invalid(format!("supplied EvidenceRecord: {error}")))?;
    if !matches!(supplied, GovernanceRecord::Evidence(_)) {
        return Err(invalid("supplied governance record is not EvidenceRecord"));
    }
    let adapted = adapt_canonical_request(request_bytes)?;
    if adapted.canonical_evidence_json.as_bytes() != evidence_bytes {
        return Err(invalid(
            "EvidenceRecord is not the exact deterministic command observation adaptation",
        ));
    }
    Ok(adapted)
}

fn build_evidence(
    request: &CommandObservationEvidenceRequest,
    command_digest: &str,
    source_digest: &str,
    request_digest: &str,
) -> Result<EvidenceRecord, CommandObservationEvidenceContractError> {
    let observed_at = request.observation.ended_at_unix_ms;
    let exit_code = request
        .observation
        .termination
        .exit_code
        .ok_or_else(|| invalid("projectable observation is missing exit code"))?;
    Ok(EvidenceRecord {
        api_version: GOVERNANCE_API_VERSION.to_owned(),
        integrity: Integrity {
            canonical_sha256: String::new(),
            canonicalization: GOVERNANCE_CANONICALIZATION.to_owned(),
        },
        kind: EvidenceRecordKind::EvidenceRecord,
        metadata: build_metadata(request, request_digest, observed_at),
        spec: build_spec(
            request,
            command_digest,
            source_digest,
            observed_at,
            exit_code,
        ),
        status: EvidenceStatus {
            reason_codes: Vec::new(),
            state: EvidenceState::Valid,
            valid_from_unix_ms: observed_at,
            valid_until_unix_ms: None,
        },
    })
}

fn build_metadata(
    request: &CommandObservationEvidenceRequest,
    request_digest: &str,
    observed_at: i64,
) -> RecordMetadata {
    let binding = &request.binding;
    let source = &request.observation.source;
    RecordMetadata {
        aggregate_id: binding.aggregate_id.clone(),
        context_sha256: binding.context_sha256.clone(),
        created_at_unix_ms: observed_at,
        created_by: fixed_principal(request_digest),
        policy_sha256: binding.policy_sha256.clone(),
        project_id: binding.project_id.clone(),
        record_id: format!("command-evidence-{request_digest}"),
        scope: binding.scope.clone(),
        sequence: binding.sequence,
        source_revision: source.source_revision.clone(),
        source_tree_sha256: source.source_tree_sha256.clone(),
        supersedes_record_ids: binding.supersedes_record_ids.clone(),
    }
}

fn build_spec(
    request: &CommandObservationEvidenceRequest,
    command_digest: &str,
    source_digest: &str,
    observed_at: i64,
    exit_code: i64,
) -> EvidenceSpec {
    let observation = &request.observation;
    EvidenceSpec {
        artifact_sha256: Some(source_digest.to_owned()),
        collector: observed_collector(&observation.producer, command_digest),
        content_role: ContentRole::UntrustedData,
        directness: Directness::Direct,
        evidence_type: mapped_evidence_type(observation.evidence_type),
        locator: EvidenceLocator {
            content_sha256: observation.streams.combined.sha256.clone(),
            exit_code: Some(exit_code),
            line_end: None,
            line_start: None,
            locator_ref: format!("command-observation:{source_digest}"),
            locator_type: LocatorType::Command,
        },
        observed_at_unix_ms: observed_at,
        sensitivity: request.binding.sensitivity,
        source_snapshot: SourceSnapshot {
            snapshot_id: format!("command-observation-{source_digest}"),
            snapshot_sha256: source_digest.to_owned(),
            snapshot_type: SnapshotType::Runtime,
        },
        source_trust: SourceTrust::Observed,
        subjects: request.binding.subjects.clone(),
    }
}

fn fixed_principal(request_digest: &str) -> Principal {
    Principal {
        authority_domain: "shadow".to_owned(),
        principal_id: ADAPTER_PRINCIPAL_ID.to_owned(),
        principal_type: PrincipalType::Tool,
        role: ADAPTER_ROLE.to_owned(),
        run_id: format!("command-adaptation-{request_digest}"),
    }
}

fn observed_collector(producer: &CommandProducer, command_digest: &str) -> Collector {
    Collector {
        collector_id: producer.producer_id.clone(),
        collector_type: match producer.producer_type {
            CommandProducerType::Service => CollectorType::Service,
            CommandProducerType::Tool => CollectorType::Tool,
        },
        collector_version: producer.producer_version.clone(),
        parameters_sha256: command_digest.to_owned(),
        run_id: producer.run_id.clone(),
    }
}

fn mapped_evidence_type(value: CommandEvidenceType) -> EvidenceType {
    match value {
        CommandEvidenceType::GateResult => EvidenceType::GateResult,
        CommandEvidenceType::TestRun => EvidenceType::TestRun,
    }
}

fn seal_and_revalidate(
    evidence: EvidenceRecord,
    inputs: AdaptationInputs,
) -> Result<CommandObservationEvidenceAdaptation, CommandObservationEvidenceContractError> {
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
    Ok(CommandObservationEvidenceAdaptation {
        canonical_command_json: inputs.canonical_command_json,
        canonical_evidence_json,
        canonical_observation_json: inputs.canonical_observation_json,
        canonical_request_json: inputs.canonical_request_json,
        command_sha256: inputs.command_sha256,
        evidence,
        request_sha256: inputs.request_sha256,
        result: ADAPTED_SHADOW,
        source_snapshot_sha256: inputs.source_snapshot_sha256,
    })
}
