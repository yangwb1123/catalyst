use super::{
    API_VERSION, ArtifactEvidenceBinding, ArtifactEvidenceContractError, ArtifactEvidenceRequest,
    ArtifactProvenanceRecord, CANONICALIZATION, invalid,
};

const ARTIFACT_FORMAT: &str = "forgeos.artifact.v1";
const MAX_IDENTIFIER_BYTES: usize = 160;

pub(super) fn validate_request(
    request: &ArtifactEvidenceRequest,
) -> Result<(), ArtifactEvidenceContractError> {
    if request.api_version != API_VERSION || request.canonicalization != CANONICALIZATION {
        return Err(invalid("unsupported adapter API or canonicalization"));
    }
    validate_artifact(&request.artifact)?;
    validate_binding(&request.binding)
}

pub(super) fn validate_artifact(
    artifact: &ArtifactProvenanceRecord,
) -> Result<(), ArtifactEvidenceContractError> {
    if artifact.format != ARTIFACT_FORMAT {
        return Err(invalid("artifact _format must be forgeos.artifact.v1"));
    }
    let required = [
        artifact.agent.as_str(),
        artifact.model.as_str(),
        artifact.path.as_str(),
        artifact.phase.as_str(),
        artifact.workflow.as_str(),
    ];
    if !required.into_iter().all(bounded_text) || artifact.size <= 0 {
        return Err(invalid("artifact required fields or size are invalid"));
    }
    if !digest(&artifact.sha256) || !digest(&artifact.prompt_sha256) {
        return Err(invalid("artifact SHA-256 fields are invalid"));
    }
    if !identifier(&artifact.run_id) || !safe_repo_path(&artifact.path) {
        return Err(invalid("artifact run_id or path is invalid"));
    }
    super::timestamp::unix_millis_floor(&artifact.created_at).map(|_| ())
}

fn validate_binding(
    binding: &ArtifactEvidenceBinding,
) -> Result<(), ArtifactEvidenceContractError> {
    let identifiers = [
        binding.aggregate_id.as_str(),
        binding.project_id.as_str(),
        binding.scope.as_str(),
        binding.source_revision.as_str(),
    ];
    if !identifiers.into_iter().all(identifier)
        || binding.sequence < 1
        || !digest(&binding.context_sha256)
        || !digest(&binding.policy_sha256)
        || !digest(&binding.source_tree_sha256)
        || binding.subjects.is_empty()
        || !sorted_unique_identifiers(&binding.subjects)
        || !sorted_unique_identifiers(&binding.supersedes_record_ids)
    {
        return Err(invalid("artifact evidence binding is invalid"));
    }
    Ok(())
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

fn sorted_unique_identifiers(values: &[String]) -> bool {
    values.iter().all(|value| identifier(value))
        && values
            .windows(2)
            .all(|pair| pair[0].as_bytes() < pair[1].as_bytes())
}

fn bounded_text(value: &str) -> bool {
    let scalar_count = value.chars().count();
    (1..=4096).contains(&scalar_count) && !value.trim().is_empty()
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
