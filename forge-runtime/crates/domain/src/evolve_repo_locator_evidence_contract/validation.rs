use super::{
    API_VERSION, CANONICALIZATION, EvolveLocatorRelation, EvolveProducer, EvolveRepoLocator,
    EvolveRepoLocatorEvidenceBinding, EvolveRepoLocatorEvidenceContractError,
    EvolveRepoLocatorEvidenceRequest, EvolveRepoLocatorObservation, SCAN_CONTRACT, invalid,
};
use crate::governance_contract::MAX_ARRAY_ITEMS;

pub(super) const MAX_CONTENT_BYTES: i64 = 1_048_576;
const MAX_IDENTIFIER_BYTES: usize = 160;
const MAX_TEXT_CHARS: usize = 4096;
const MAX_DETAIL_BYTES: usize = 512;

pub(super) fn validate_request(
    request: &EvolveRepoLocatorEvidenceRequest,
) -> Result<(), EvolveRepoLocatorEvidenceContractError> {
    if request.api_version != API_VERSION || request.canonicalization != CANONICALIZATION {
        return Err(invalid("unsupported adapter API or canonicalization"));
    }
    validate_binding(&request.binding)?;
    validate_observation(&request.observation)
}

pub(super) fn validate_observation(
    observation: &EvolveRepoLocatorObservation,
) -> Result<(), EvolveRepoLocatorEvidenceContractError> {
    if observation.api_version != super::OBSERVATION_API_VERSION
        || observation.canonicalization != CANONICALIZATION
    {
        return Err(invalid(
            "unsupported Evolve locator observation API or canonicalization",
        ));
    }
    if !(1..=MAX_CONTENT_BYTES).contains(&observation.content.bytes)
        || !digest(&observation.content.sha256)
    {
        return Err(invalid("Evolve locator observed content is invalid"));
    }
    validate_locator(&observation.locator)?;
    if observation.observed_at_unix_ms < 0 {
        return Err(invalid("Evolve locator observation time is invalid"));
    }
    validate_producer(&observation.producer)?;
    validate_scan_context(observation)?;
    if !identifier(&observation.source.source_revision)
        || !digest(&observation.source.source_tree_sha256)
    {
        return Err(invalid("Evolve locator observation source is invalid"));
    }
    Ok(())
}

pub(super) fn validate_locator(
    locator: &EvolveRepoLocator,
) -> Result<(), EvolveRepoLocatorEvidenceContractError> {
    if locator.detail.trim().is_empty()
        || locator.detail.chars().any(char::is_control)
        || locator.detail.len() > MAX_DETAIL_BYTES
        || locator.line < 0
        || !safe_repository_path(&locator.path)
    {
        return Err(invalid("Evolve repository locator is invalid"));
    }
    Ok(())
}

fn validate_binding(
    binding: &EvolveRepoLocatorEvidenceBinding,
) -> Result<(), EvolveRepoLocatorEvidenceContractError> {
    if ![
        binding.aggregate_id.as_str(),
        binding.project_id.as_str(),
        binding.scope.as_str(),
    ]
    .into_iter()
    .all(identifier)
        || !digest(&binding.context_sha256)
        || !digest(&binding.policy_sha256)
        || binding.sequence < 1
        || binding.subjects.is_empty()
        || !sorted_unique_identifiers(&binding.subjects)
        || !sorted_unique_identifiers(&binding.supersedes_record_ids)
    {
        return Err(invalid("Evolve locator Evidence binding is invalid"));
    }
    Ok(())
}

fn validate_producer(
    producer: &EvolveProducer,
) -> Result<(), EvolveRepoLocatorEvidenceContractError> {
    if [
        producer.producer_id.as_str(),
        producer.producer_version.as_str(),
        producer.run_id.as_str(),
    ]
    .into_iter()
    .all(identifier)
        && digest(&producer.parameters_sha256)
    {
        Ok(())
    } else {
        Err(invalid("Evolve locator observation producer is invalid"))
    }
}

fn validate_scan_context(
    observation: &EvolveRepoLocatorObservation,
) -> Result<(), EvolveRepoLocatorEvidenceContractError> {
    let context = &observation.scan_context;
    if context.contract != SCAN_CONTRACT || !digest(&context.report_sha256) {
        return Err(invalid("Evolve locator scan context is invalid"));
    }
    let opportunity_is_valid = match context.relation {
        EvolveLocatorRelation::Opportunity => context
            .opportunity_id
            .as_deref()
            .is_some_and(evolve_identifier),
        EvolveLocatorRelation::Clear | EvolveLocatorRelation::Finding => {
            context.opportunity_id.is_none()
        }
    };
    opportunity_is_valid
        .then_some(())
        .ok_or_else(|| invalid("Evolve locator relation/opportunity binding is invalid"))
}

fn digest(value: &str) -> bool {
    value.len() == 64
        && value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
}

fn identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= MAX_IDENTIFIER_BYTES
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase()
                || byte.is_ascii_digit()
                || matches!(byte, b'.' | b'_' | b':' | b'/' | b'-')
        })
}

fn evolve_identifier(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 64
        && (bytes[0].is_ascii_lowercase() || bytes[0].is_ascii_digit())
        && bytes.iter().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'_' | b'-')
        })
}

fn sorted_unique_identifiers(values: &[String]) -> bool {
    values.len() <= MAX_ARRAY_ITEMS
        && values.iter().all(|value| identifier(value))
        && values
            .windows(2)
            .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
}

fn safe_repository_path(path: &str) -> bool {
    let bytes = path.as_bytes();
    if path.trim().is_empty()
        || path.chars().count() > MAX_TEXT_CHARS
        || path.chars().any(char::is_control)
        || path.starts_with('/')
        || path.ends_with('/')
        || path.contains('\\')
        || bytes.len() >= 2 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':'
    {
        return false;
    }
    let mut segments = path.split('/');
    let Some(first) = segments.next() else {
        return false;
    };
    if first.eq_ignore_ascii_case(".git") || first.eq_ignore_ascii_case(".forge") {
        return false;
    }
    !first.is_empty()
        && first != "."
        && first != ".."
        && segments.all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
