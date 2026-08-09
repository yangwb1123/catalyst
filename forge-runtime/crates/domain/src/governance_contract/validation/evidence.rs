use super::super::{
    API_VERSION, CollectorType, ContentRole, Directness, EvidenceRecord, EvidenceState,
    EvidenceType, GovernanceContractError, LocatorType, SourceTrust, invalid,
};
use super::common::{
    is_digest, is_identifier, nonempty_text, sorted_unique_identifiers, validate_interval,
};

pub(super) fn validate(record: &EvidenceRecord) -> Result<(), GovernanceContractError> {
    if record.api_version != API_VERSION {
        return Err(invalid("evidence api_version is invalid"));
    }
    validate_spec(record)?;
    validate_status(record)?;
    Ok(())
}

fn validate_spec(record: &EvidenceRecord) -> Result<(), GovernanceContractError> {
    let spec = &record.spec;
    let identifiers = [
        spec.collector.collector_id.as_str(),
        spec.collector.collector_version.as_str(),
        spec.collector.run_id.as_str(),
        spec.source_snapshot.snapshot_id.as_str(),
    ];
    if !identifiers.into_iter().all(is_identifier)
        || !is_digest(&spec.collector.parameters_sha256)
        || !is_digest(&spec.source_snapshot.snapshot_sha256)
        || !is_digest(&spec.locator.content_sha256)
        || spec
            .artifact_sha256
            .as_deref()
            .is_some_and(|value| !is_digest(value))
        || spec.subjects.is_empty()
        || !sorted_unique_identifiers(&spec.subjects)
        || !nonempty_text(&spec.locator.locator_ref, 4096)
    {
        return Err(invalid("evidence spec fields are invalid"));
    }
    validate_shadow_source(record)?;
    validate_locator(record)?;
    validate_directness(record)
}

fn validate_shadow_source(record: &EvidenceRecord) -> Result<(), GovernanceContractError> {
    let spec = &record.spec;
    let trust_allowed = matches!(
        spec.source_trust,
        SourceTrust::Untrusted | SourceTrust::Observed
    );
    if spec.content_role == ContentRole::UntrustedData && trust_allowed {
        Ok(())
    } else {
        Err(invalid(
            "shadow evidence cannot assert trusted control or elevated source trust",
        ))
    }
}

fn validate_locator(record: &EvidenceRecord) -> Result<(), GovernanceContractError> {
    let locator = &record.spec.locator;
    if locator.locator_type != expected_locator(record.spec.evidence_type) {
        return Err(invalid("evidence type and locator type do not match"));
    }
    match locator.locator_type {
        LocatorType::Repo => validate_repo_locator(record),
        LocatorType::Command => validate_command_locator(record),
        _ if locator.exit_code.is_none()
            && locator.line_start.is_none()
            && locator.line_end.is_none() =>
        {
            Ok(())
        }
        _ => Err(invalid(
            "non-repository locator has unsupported position fields",
        )),
    }
}

fn validate_repo_locator(record: &EvidenceRecord) -> Result<(), GovernanceContractError> {
    let locator = &record.spec.locator;
    let lines_valid = match (locator.line_start, locator.line_end) {
        (None, None) => true,
        (Some(start), Some(end)) => start > 0 && end >= start,
        _ => false,
    };
    if safe_repo_path(&locator.locator_ref) && lines_valid && locator.exit_code.is_none() {
        Ok(())
    } else {
        Err(invalid(
            "repository locator is not a safe normalized source range",
        ))
    }
}

fn validate_command_locator(record: &EvidenceRecord) -> Result<(), GovernanceContractError> {
    let locator = &record.spec.locator;
    if locator.exit_code.is_some() && locator.line_start.is_none() && locator.line_end.is_none() {
        Ok(())
    } else {
        Err(invalid("command locator must contain only an exit code"))
    }
}

fn validate_directness(record: &EvidenceRecord) -> Result<(), GovernanceContractError> {
    let spec = &record.spec;
    if spec.evidence_type == EvidenceType::HumanAttestation {
        let collector = matches!(
            spec.collector.collector_type,
            CollectorType::Human | CollectorType::Operator
        );
        if spec.directness != Directness::Attested || !collector {
            return Err(invalid(
                "human attestation requires attested human or operator collection",
            ));
        }
    }
    if spec.evidence_type == EvidenceType::ExternalSource && spec.directness != Directness::Derived
    {
        return Err(invalid(
            "external source evidence is derived in the shadow contract",
        ));
    }
    if spec.directness == Directness::Direct && !direct_observation_allowed(record) {
        return Err(invalid(
            "direct evidence has an unsupported type or collector",
        ));
    }
    Ok(())
}

fn direct_observation_allowed(record: &EvidenceRecord) -> bool {
    let spec = &record.spec;
    let direct_type = matches!(
        spec.evidence_type,
        EvidenceType::Artifact
            | EvidenceType::GateResult
            | EvidenceType::RepoLocator
            | EvidenceType::RuntimeMetric
            | EvidenceType::TestRun
    );
    let collector = matches!(
        spec.collector.collector_type,
        CollectorType::Service | CollectorType::Tool
    );
    direct_type && collector
}

fn validate_status(record: &EvidenceRecord) -> Result<(), GovernanceContractError> {
    let status = &record.status;
    validate_interval(status.valid_from_unix_ms, status.valid_until_unix_ms)?;
    if record.spec.observed_at_unix_ms < 0
        || record.spec.observed_at_unix_ms > record.metadata.created_at_unix_ms
        || status.valid_from_unix_ms < record.spec.observed_at_unix_ms
        || !sorted_unique_identifiers(&status.reason_codes)
    {
        return Err(invalid(
            "evidence observation or status metadata is invalid",
        ));
    }
    let artifact = record.spec.artifact_sha256.is_some();
    let reasons = !status.reason_codes.is_empty();
    let shape = match status.state {
        EvidenceState::Valid => artifact && !reasons,
        EvidenceState::Unavailable => !artifact && reasons,
        EvidenceState::Invalid => artifact && reasons,
        EvidenceState::Expired => artifact && reasons && expired_by_creation(record),
    };
    shape
        .then_some(())
        .ok_or_else(|| invalid("evidence state, artifact, and reasons disagree"))
}

fn expired_by_creation(record: &EvidenceRecord) -> bool {
    record
        .status
        .valid_until_unix_ms
        .is_some_and(|until| until <= record.metadata.created_at_unix_ms)
}

fn expected_locator(evidence_type: EvidenceType) -> LocatorType {
    match evidence_type {
        EvidenceType::Artifact => LocatorType::Artifact,
        EvidenceType::ExternalSource => LocatorType::External,
        EvidenceType::GateResult | EvidenceType::TestRun => LocatorType::Command,
        EvidenceType::HumanAttestation => LocatorType::Attestation,
        EvidenceType::RepoLocator => LocatorType::Repo,
        EvidenceType::RuntimeMetric => LocatorType::Metric,
    }
}

fn safe_repo_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    !(path.is_empty()
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':')
        && path
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
