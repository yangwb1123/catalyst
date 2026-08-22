use std::collections::HashSet;

use super::{
    ASSEMBLY_MODE, CANONICALIZATION, CONTEXT_PACKAGE_API_VERSION, ContextLanes, ContextOmission,
    ContextPackage, ContextPackageContractError, ContextSnippet, DELIMITER, DeclaredLane,
    DeclaredTrust, MAX_ARRAY_ITEMS, MAX_REFERENCE_BYTES, MAX_SHORT_TEXT_BYTES,
    MAX_SOURCE_CONTENT_BYTES, NORMALIZATION, ProjectedLane, RESULT, RedactionReceipt,
    SelectionReason, SourceClass, invalid, validation,
};

pub(super) fn validate_package_shape(
    package: &ContextPackage,
) -> Result<(), ContextPackageContractError> {
    validation::require_equal(
        &package.api_version,
        CONTEXT_PACKAGE_API_VERSION,
        "context package api_version",
    )?;
    validation::require_equal(&package.assembly_mode, ASSEMBLY_MODE, "assembly_mode")?;
    validation::require_equal(
        &package.canonicalization,
        CANONICALIZATION,
        "context package canonicalization",
    )?;
    validation::require_equal(&package.result, RESULT, "context package result")?;
    validation::validate_budget(&package.budget)?;
    validation::validate_task_binding(&package.task_binding)?;
    validation::validate_source_binding(&package.source_binding)?;
    validate_package_hashes(package)?;
    validate_lanes(&package.lanes)?;
    validate_omissions(&package.omissions)?;
    validate_receipts(&package.redaction_receipts)?;
    validate_accounting_shape(package)
}

fn validate_package_hashes(package: &ContextPackage) -> Result<(), ContextPackageContractError> {
    for (name, digest) in [
        ("cache_key_sha256", package.cache_key_sha256.as_str()),
        ("context_sha256", package.context_sha256.as_str()),
        ("projection_sha256", package.projection_sha256.as_str()),
        ("request_sha256", package.request_sha256.as_str()),
    ] {
        validation::validate_sha256(digest, name)?;
    }
    Ok(())
}

fn validate_lanes(lanes: &ContextLanes) -> Result<(), ContextPackageContractError> {
    let groups = [
        (
            &lanes.instruction_candidates,
            ProjectedLane::InstructionCandidates,
        ),
        (&lanes.trusted_context, ProjectedLane::TrustedContext),
        (&lanes.untrusted_data, ProjectedLane::UntrustedData),
    ];
    let mut source_ids = HashSet::new();
    for (snippets, expected_lane) in groups {
        for snippet in snippets {
            validate_snippet(snippet, expected_lane)?;
            if !source_ids.insert(snippet.source_id.as_str()) {
                return Err(invalid("selected snippet source_id values must be unique"));
            }
        }
    }
    Ok(())
}

fn validate_snippet(
    snippet: &ContextSnippet,
    expected_lane: ProjectedLane,
) -> Result<(), ContextPackageContractError> {
    if snippet.lane != expected_lane || snippet.instruction_allowed {
        return Err(invalid("snippet lane or instruction_allowed is invalid"));
    }
    if projected_lane(snippet) != expected_lane
        || (validation::untrusted_class(snippet.source_class)
            && (snippet.declared_lane != DeclaredLane::UntrustedData
                || snippet.declared_trust != DeclaredTrust::Untrusted))
    {
        return Err(invalid(
            "snippet declared source cannot escalate lane or trust",
        ));
    }
    if (snippet.required && snippet.selection_reason != SelectionReason::RequiredSource)
        || (!snippet.required && snippet.selection_reason != SelectionReason::PrioritySelection)
    {
        return Err(invalid("snippet selection_reason does not match required"));
    }
    validation::require_equal(&snippet.delimiter, DELIMITER, "snippet delimiter")?;
    validation::require_equal(
        &snippet.normalization,
        NORMALIZATION,
        "snippet normalization",
    )?;
    validation::validate_content(&snippet.content)?;
    if snippet.content.is_empty() || snippet.content.len() > MAX_SOURCE_CONTENT_BYTES {
        return Err(invalid(
            "snippet content must contain 1..131072 UTF-8 bytes",
        ));
    }
    validate_snippet_strings(snippet)?;
    validate_snippet_hashes(snippet)?;
    validate_truncation(snippet)
}

fn validate_snippet_strings(snippet: &ContextSnippet) -> Result<(), ContextPackageContractError> {
    validation::validate_ordinary(
        &snippet.source_id,
        MAX_SHORT_TEXT_BYTES,
        "snippet source_id",
    )?;
    validation::validate_ordinary(
        &snippet.source_ref,
        MAX_REFERENCE_BYTES,
        "snippet source_ref",
    )?;
    validation::validate_ordinary(
        &snippet.source_revision,
        MAX_SHORT_TEXT_BYTES,
        "snippet source_revision",
    )
}

fn validate_snippet_hashes(snippet: &ContextSnippet) -> Result<(), ContextPackageContractError> {
    for (name, digest) in [
        (
            "projected_content_sha256",
            snippet.projected_content_sha256.as_str(),
        ),
        ("snippet_sha256", snippet.snippet_sha256.as_str()),
        (
            "source_content_sha256",
            snippet.source_content_sha256.as_str(),
        ),
    ] {
        validation::validate_sha256(digest, name)?;
    }
    Ok(())
}

fn validate_truncation(snippet: &ContextSnippet) -> Result<(), ContextPackageContractError> {
    if let Some(truncation) = &snippet.truncation {
        validation::require_equal(&truncation.reason, "source_max_bytes", "truncation reason")?;
        if truncation.original_redacted_bytes > 524_288
            || truncation.retained_bytes != snippet.content.len() as u64
            || truncation.retained_bytes >= truncation.original_redacted_bytes
            || snippet.required
        {
            return Err(invalid("snippet truncation byte counts are invalid"));
        }
    }
    Ok(())
}

fn validate_omissions(omissions: &[ContextOmission]) -> Result<(), ContextPackageContractError> {
    if omissions.len() > 64 {
        return Err(invalid("omissions exceed the source limit"));
    }
    let mut previous_id: Option<&str> = None;
    for omission in omissions {
        validation::validate_ordinary(
            &omission.source_id,
            MAX_SHORT_TEXT_BYTES,
            "omission source_id",
        )?;
        validation::validate_ordinary(
            &omission.source_ref,
            MAX_REFERENCE_BYTES,
            "omission source_ref",
        )?;
        if previous_id.is_some_and(|value| value.as_bytes() >= omission.source_id.as_bytes()) {
            return Err(invalid(
                "omission source_id values are not strictly ordered",
            ));
        }
        previous_id = Some(&omission.source_id);
    }
    Ok(())
}

fn validate_receipts(receipts: &[RedactionReceipt]) -> Result<(), ContextPackageContractError> {
    if receipts.len() > 64 {
        return Err(invalid("redaction receipts exceed the source limit"));
    }
    let mut previous_id: Option<&str> = None;
    let mut total_ranges = 0_usize;
    for receipt in receipts {
        validation::validate_ordinary(
            &receipt.source_id,
            MAX_SHORT_TEXT_BYTES,
            "receipt source_id",
        )?;
        if previous_id.is_some_and(|value| value.as_bytes() >= receipt.source_id.as_bytes()) {
            return Err(invalid("redaction receipts are not strictly ordered"));
        }
        validate_receipt_ranges(receipt)?;
        total_ranges += receipt.ranges.len();
        previous_id = Some(&receipt.source_id);
    }
    if total_ranges > MAX_ARRAY_ITEMS {
        return Err(invalid("redaction receipts exceed 256 total ranges"));
    }
    Ok(())
}

fn validate_receipt_ranges(receipt: &RedactionReceipt) -> Result<(), ContextPackageContractError> {
    if receipt.ranges.is_empty() {
        return Err(invalid("redaction receipt ranges cannot be empty"));
    }
    let mut previous_end = 0_u64;
    for (index, range) in receipt.ranges.iter().enumerate() {
        validation::validate_ordinary(&range.rule_id, MAX_SHORT_TEXT_BYTES, "receipt rule_id")?;
        if range.start_byte >= range.end_byte
            || range.start_byte > 131_071
            || range.end_byte > 131_072
            || (index > 0 && range.start_byte < previous_end)
        {
            return Err(invalid("redaction receipt range is invalid"));
        }
        previous_end = range.end_byte;
    }
    Ok(())
}

fn validate_accounting_shape(package: &ContextPackage) -> Result<(), ContextPackageContractError> {
    let selected = selected_count(&package.lanes);
    let redactions: u64 = package
        .redaction_receipts
        .iter()
        .map(|receipt| receipt.ranges.len() as u64)
        .sum();
    if package.freshness.evaluated_at_unix_ms != package.source_binding.as_of_unix_ms
        || package
            .freshness
            .expires_at_unix_ms
            .is_some_and(|value| value <= package.freshness.evaluated_at_unix_ms)
        || package.accounting.selected_snippet_count != selected
        || package.accounting.omitted_source_count != package.omissions.len() as u64
        || package.accounting.redacted_range_count != redactions
        || package.accounting.content_bytes != selected_content_bytes(&package.lanes)
        || package.accounting.candidate_count
            != package.accounting.selected_snippet_count + package.accounting.omitted_source_count
        || exceeds_budget(package)
    {
        return Err(invalid(
            "context package accounting cardinalities are invalid",
        ));
    }
    validate_source_partition(package)?;
    validate_truncated_count(package)
}

fn exceeds_budget(package: &ContextPackage) -> bool {
    package.accounting.candidate_count == 0
        || package.accounting.candidate_count > 64
        || package.accounting.actual_tokens > package.budget.max_tokens
        || package.accounting.content_bytes > package.budget.max_content_bytes
        || package.accounting.selected_snippet_count > package.budget.max_snippets
}

fn validate_truncated_count(package: &ContextPackage) -> Result<(), ContextPackageContractError> {
    let truncated = package
        .lanes
        .instruction_candidates
        .iter()
        .chain(&package.lanes.trusted_context)
        .chain(&package.lanes.untrusted_data)
        .filter(|snippet| snippet.truncation.is_some())
        .count() as u64;
    if package.accounting.truncated_snippet_count != truncated {
        return Err(invalid("context package truncated count is invalid"));
    }
    Ok(())
}

fn selected_count(lanes: &ContextLanes) -> u64 {
    (lanes.instruction_candidates.len() + lanes.trusted_context.len() + lanes.untrusted_data.len())
        as u64
}

fn selected_content_bytes(lanes: &ContextLanes) -> u64 {
    lanes
        .instruction_candidates
        .iter()
        .chain(&lanes.trusted_context)
        .chain(&lanes.untrusted_data)
        .map(|snippet| snippet.content.len() as u64)
        .sum()
}

fn validate_source_partition(package: &ContextPackage) -> Result<(), ContextPackageContractError> {
    let mut source_ids = HashSet::new();
    for snippet in package
        .lanes
        .instruction_candidates
        .iter()
        .chain(&package.lanes.trusted_context)
        .chain(&package.lanes.untrusted_data)
    {
        source_ids.insert(snippet.source_id.as_str());
    }
    for omission in &package.omissions {
        if !source_ids.insert(omission.source_id.as_str()) {
            return Err(invalid("a package source cannot be selected and omitted"));
        }
    }
    Ok(())
}

fn projected_lane(snippet: &ContextSnippet) -> ProjectedLane {
    if matches!(
        snippet.source_class,
        SourceClass::SystemPolicy | SourceClass::UserInstruction
    ) && snippet.declared_lane == DeclaredLane::Instruction
    {
        ProjectedLane::InstructionCandidates
    } else if snippet.declared_lane == DeclaredLane::UntrustedData {
        ProjectedLane::UntrustedData
    } else {
        ProjectedLane::TrustedContext
    }
}
