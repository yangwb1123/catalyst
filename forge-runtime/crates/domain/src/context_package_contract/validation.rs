use std::collections::{HashMap, HashSet};

use super::{
    BUILD_REQUEST_API_VERSION, CANONICALIZATION, ContextBudget, ContextPackageBuildRequest,
    ContextPackageContractError, ContextSource, DeclaredLane, DeclaredTrust, MAX_ARRAY_ITEMS,
    MAX_REFERENCE_BYTES, MAX_SHORT_TEXT_BYTES, MAX_SOURCE_CONTENT_BYTES, RedactionRange,
    SourceAvailability, SourceBinding, SourceClass, SourceRedaction, TaskBinding,
    TokenizerIdentity, codec, invalid,
};

pub(super) fn validate_request(
    request: &ContextPackageBuildRequest,
) -> Result<(), ContextPackageContractError> {
    require_equal(
        &request.api_version,
        BUILD_REQUEST_API_VERSION,
        "build request api_version",
    )?;
    require_equal(
        &request.canonicalization,
        CANONICALIZATION,
        "build request canonicalization",
    )?;
    validate_budget(&request.budget)?;
    validate_task_binding(&request.task_binding)?;
    validate_source_binding(&request.source_binding)?;
    validate_sources(&request.sources)?;
    validate_redactions(&request.redactions, &request.sources)
}

pub(super) fn validate_tokenizer_identity(
    budget: &ContextBudget,
    identity: &TokenizerIdentity,
) -> Result<(), ContextPackageContractError> {
    validate_ordinary(
        &identity.tokenizer_id,
        MAX_SHORT_TEXT_BYTES,
        "tokenizer identity id",
    )?;
    validate_sha256(&identity.tokenizer_sha256, "tokenizer identity digest")?;
    if budget.tokenizer_id != identity.tokenizer_id
        || budget.tokenizer_sha256 != identity.tokenizer_sha256
    {
        return Err(invalid(
            "token counter identity does not match the pinned budget",
        ));
    }
    Ok(())
}

pub(super) fn validate_budget(budget: &ContextBudget) -> Result<(), ContextPackageContractError> {
    require_range(budget.max_content_bytes, 1, 524_288, "max_content_bytes")?;
    require_range(budget.max_snippets, 1, 24, "max_snippets")?;
    require_range(budget.max_tokens, 1, 1_000_000, "max_tokens")?;
    validate_ordinary(&budget.tokenizer_id, MAX_SHORT_TEXT_BYTES, "tokenizer_id")?;
    validate_sha256(&budget.tokenizer_sha256, "tokenizer_sha256")
}

pub(super) fn validate_task_binding(
    binding: &TaskBinding,
) -> Result<(), ContextPackageContractError> {
    for (name, value) in [
        ("change_id", binding.change_id.as_str()),
        ("node_id", binding.node_id.as_str()),
        ("phase", binding.phase.as_str()),
        ("project_id", binding.project_id.as_str()),
        ("role", binding.role.as_str()),
        ("run_id", binding.run_id.as_str()),
        ("task_id", binding.task_id.as_str()),
    ] {
        validate_ordinary(value, MAX_SHORT_TEXT_BYTES, name)?;
    }
    Ok(())
}

pub(super) fn validate_source_binding(
    binding: &SourceBinding,
) -> Result<(), ContextPackageContractError> {
    if binding.as_of_unix_ms < 0 {
        return Err(invalid("as_of_unix_ms must be nonnegative"));
    }
    validate_sha256(&binding.policy_sha256, "policy_sha256")?;
    validate_sha256(&binding.routes_sha256, "routes_sha256")?;
    validate_ordinary(
        &binding.source_revision,
        MAX_SHORT_TEXT_BYTES,
        "source_revision",
    )?;
    validate_sha256(&binding.source_tree_sha256, "source_tree_sha256")
}

fn validate_sources(sources: &[ContextSource]) -> Result<(), ContextPackageContractError> {
    if sources.is_empty() || sources.len() > 64 {
        return Err(invalid("sources must contain between 1 and 64 entries"));
    }
    let mut previous_id: Option<&str> = None;
    let mut references = HashSet::new();
    for source in sources {
        validate_source(source)?;
        if previous_id.is_some_and(|value| value.as_bytes() >= source.source_id.as_bytes()) {
            return Err(invalid(
                "sources must be unique and strictly ordered by source_id bytes",
            ));
        }
        if !references.insert(source.source_ref.as_str()) {
            return Err(invalid("source_ref values must be unique"));
        }
        previous_id = Some(&source.source_id);
    }
    Ok(())
}

fn validate_source(source: &ContextSource) -> Result<(), ContextPackageContractError> {
    validate_ordinary(&source.source_id, MAX_SHORT_TEXT_BYTES, "source_id")?;
    validate_ordinary(&source.source_ref, MAX_REFERENCE_BYTES, "source_ref")?;
    validate_ordinary(
        &source.source_revision,
        MAX_SHORT_TEXT_BYTES,
        "source source_revision",
    )?;
    require_range(source.max_bytes, 1, 131_072, "source max_bytes")?;
    require_range(source.priority, 0, 1_000, "source priority")?;
    if source.expires_at_unix_ms.is_some_and(|value| value < 0) {
        return Err(invalid("source expires_at_unix_ms must be nonnegative"));
    }
    validate_source_availability(source)?;
    if untrusted_class(source.source_class)
        && (source.declared_lane != DeclaredLane::UntrustedData
            || source.declared_trust != DeclaredTrust::Untrusted)
    {
        return Err(invalid(
            "untrusted source classes require untrusted trust and the untrusted_data lane",
        ));
    }
    Ok(())
}

fn validate_source_availability(source: &ContextSource) -> Result<(), ContextPackageContractError> {
    match (source.availability, &source.content, &source.content_sha256) {
        (SourceAvailability::Available, Some(content), Some(digest)) => {
            if content.is_empty() || content.len() > MAX_SOURCE_CONTENT_BYTES {
                return Err(invalid("source content must contain 1..131072 UTF-8 bytes"));
            }
            validate_content(content)?;
            validate_sha256(digest, "source content_sha256")?;
            if codec::raw_sha256(content.as_bytes()) != *digest {
                return Err(invalid("source content_sha256 does not match content"));
            }
        }
        (SourceAvailability::Missing, None, None) => {}
        _ => {
            return Err(invalid(
                "available sources require content and digest; missing sources require nulls",
            ));
        }
    }
    Ok(())
}

fn validate_redactions(
    redactions: &[SourceRedaction],
    sources: &[ContextSource],
) -> Result<(), ContextPackageContractError> {
    if redactions.len() > 64 {
        return Err(invalid("redactions exceed the source limit"));
    }
    let by_id: HashMap<_, _> = sources
        .iter()
        .map(|source| (source.source_id.as_str(), source))
        .collect();
    let mut previous_id: Option<&str> = None;
    let mut total_ranges = 0_usize;
    for redaction in redactions {
        validate_ordinary(
            &redaction.source_id,
            MAX_SHORT_TEXT_BYTES,
            "redaction source_id",
        )?;
        if previous_id.is_some_and(|value| value.as_bytes() >= redaction.source_id.as_bytes()) {
            return Err(invalid(
                "redactions must be unique and strictly ordered by source_id bytes",
            ));
        }
        let source = by_id
            .get(redaction.source_id.as_str())
            .ok_or_else(|| invalid("redaction refers to an unknown source_id"))?;
        let content = source
            .content
            .as_deref()
            .ok_or_else(|| invalid("redaction cannot target a missing source"))?;
        validate_ranges(&redaction.ranges, content)?;
        total_ranges += redaction.ranges.len();
        if total_ranges > MAX_ARRAY_ITEMS {
            return Err(invalid("redactions exceed 256 total ranges"));
        }
        previous_id = Some(&redaction.source_id);
    }
    Ok(())
}

fn validate_ranges(
    ranges: &[RedactionRange],
    content: &str,
) -> Result<(), ContextPackageContractError> {
    if ranges.is_empty() || ranges.len() > MAX_ARRAY_ITEMS {
        return Err(invalid(
            "redaction ranges must contain between 1 and 256 entries",
        ));
    }
    let mut previous_end = 0_usize;
    for (index, range) in ranges.iter().enumerate() {
        validate_ordinary(&range.rule_id, MAX_SHORT_TEXT_BYTES, "redaction rule_id")?;
        let start = usize::try_from(range.start_byte)
            .map_err(|_| invalid("redaction start_byte is too large"))?;
        let end = usize::try_from(range.end_byte)
            .map_err(|_| invalid("redaction end_byte is too large"))?;
        if start >= end
            || end > content.len()
            || !content.is_char_boundary(start)
            || !content.is_char_boundary(end)
        {
            return Err(invalid("redaction range is not a valid UTF-8 byte range"));
        }
        if index > 0 && start < previous_end {
            return Err(invalid("redaction ranges overlap or are not ordered"));
        }
        previous_end = end;
    }
    Ok(())
}

pub(super) fn validate_content(value: &str) -> Result<(), ContextPackageContractError> {
    if value.chars().any(forbidden_content_scalar) {
        return Err(invalid("content contains a forbidden Unicode scalar"));
    }
    Ok(())
}

pub(super) fn validate_ordinary(
    value: &str,
    max_bytes: usize,
    name: &str,
) -> Result<(), ContextPackageContractError> {
    if value.is_empty() || value.len() > max_bytes || value.chars().any(forbidden_ordinary_scalar) {
        return Err(invalid(format!(
            "{name} is empty, oversized, or contains forbidden text"
        )));
    }
    Ok(())
}

pub(super) fn validate_sha256(value: &str, name: &str) -> Result<(), ContextPackageContractError> {
    if value.len() != 64
        || !value
            .as_bytes()
            .iter()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(byte))
    {
        return Err(invalid(format!("{name} is not lowercase SHA-256 hex")));
    }
    Ok(())
}

fn require_range(
    value: u64,
    minimum: u64,
    maximum: u64,
    name: &str,
) -> Result<(), ContextPackageContractError> {
    if (minimum..=maximum).contains(&value) {
        Ok(())
    } else {
        Err(invalid(format!("{name} is outside its admitted range")))
    }
}

pub(super) fn require_equal(
    value: &str,
    expected: &str,
    name: &str,
) -> Result<(), ContextPackageContractError> {
    if value == expected {
        Ok(())
    } else {
        Err(invalid(format!(
            "{name} does not match the frozen contract"
        )))
    }
}

pub(super) fn untrusted_class(source_class: SourceClass) -> bool {
    matches!(
        source_class,
        SourceClass::Repository
            | SourceClass::Web
            | SourceClass::Log
            | SourceClass::Issue
            | SourceClass::ToolOutput
            | SourceClass::Artifact
            | SourceClass::Other
    )
}

fn forbidden_ordinary_scalar(value: char) -> bool {
    matches!(value, '\u{0000}'..='\u{001f}') || forbidden_shared_scalar(value)
}

pub(super) fn forbidden_content_scalar(value: char) -> bool {
    (matches!(value, '\u{0000}'..='\u{001f}') && !matches!(value, '\t' | '\n'))
        || forbidden_shared_scalar(value)
}

fn forbidden_shared_scalar(value: char) -> bool {
    matches!(
        value,
        '\u{007f}'
            | '\u{061c}'
            | '\u{200e}'
            | '\u{200f}'
            | '\u{2028}'..='\u{202e}'
            | '\u{2066}'..='\u{2069}'
    )
}
