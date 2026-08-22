use std::collections::HashSet;

use super::{
    ASSEMBLY_MODE, CACHE_KEY_DIGEST_DOMAIN, CANONICALIZATION, CONTEXT_PACKAGE_API_VERSION,
    ContextAccounting, ContextFreshness, ContextLanes, ContextPackage, ContextPackageBuildRequest,
    ContextPackageContractError, ContextSnippet, PROJECTION_DIGEST_DOMAIN, REQUEST_DIGEST_DOMAIN,
    RESULT, RedactionReceipt, codec, package_validation,
};
use crate::context_package_contract::assembly::{Selection, projection_json};

pub(super) fn build_package(
    request: &ContextPackageBuildRequest,
    request_bytes: &[u8],
    selection: Selection,
) -> Result<ContextPackage, ContextPackageContractError> {
    let projection = projection_json(&selection.lanes)?;
    let selected_ids: HashSet<_> = all_snippets(&selection.lanes)
        .iter()
        .map(|snippet| snippet.source_id.as_str())
        .collect();
    let expiry = request
        .sources
        .iter()
        .filter(|source| selected_ids.contains(source.source_id.as_str()))
        .filter_map(|source| source.expires_at_unix_ms)
        .min();
    let mut package = package_body(request, request_bytes, selection, &projection, expiry);
    package.context_sha256 = codec::context_sha256(&package)?;
    package_validation::validate_package_shape(&package)?;
    Ok(package)
}

fn package_body(
    request: &ContextPackageBuildRequest,
    request_bytes: &[u8],
    selection: Selection,
    projection: &str,
    expires_at_unix_ms: Option<i64>,
) -> ContextPackage {
    let accounting = accounting_for(request, &selection);
    ContextPackage {
        accounting,
        api_version: CONTEXT_PACKAGE_API_VERSION.into(),
        assembly_mode: ASSEMBLY_MODE.into(),
        budget: request.budget.clone(),
        cache_key_sha256: codec::domain_sha256(CACHE_KEY_DIGEST_DOMAIN, request_bytes),
        canonicalization: CANONICALIZATION.into(),
        context_sha256: String::new(),
        freshness: ContextFreshness {
            evaluated_at_unix_ms: request.source_binding.as_of_unix_ms,
            expires_at_unix_ms,
        },
        lanes: selection.lanes,
        omissions: selection.omissions,
        projection_sha256: codec::domain_sha256(PROJECTION_DIGEST_DOMAIN, projection.as_bytes()),
        redaction_receipts: receipts_for(request),
        request_sha256: codec::domain_sha256(REQUEST_DIGEST_DOMAIN, request_bytes),
        result: RESULT.into(),
        source_binding: request.source_binding.clone(),
        task_binding: request.task_binding.clone(),
    }
}

fn accounting_for(
    request: &ContextPackageBuildRequest,
    selection: &Selection,
) -> ContextAccounting {
    let snippets = all_snippets(&selection.lanes);
    ContextAccounting {
        actual_tokens: selection.actual_tokens,
        candidate_count: request.sources.len() as u64,
        content_bytes: selection.content_bytes,
        omitted_source_count: selection.omissions.len() as u64,
        redacted_range_count: request
            .redactions
            .iter()
            .map(|receipt| receipt.ranges.len() as u64)
            .sum(),
        selected_snippet_count: snippets.len() as u64,
        truncated_snippet_count: snippets
            .iter()
            .filter(|snippet| snippet.truncation.is_some())
            .count() as u64,
    }
}

fn receipts_for(request: &ContextPackageBuildRequest) -> Vec<RedactionReceipt> {
    request
        .redactions
        .iter()
        .map(|plan| RedactionReceipt {
            ranges: plan.ranges.clone(),
            source_id: plan.source_id.clone(),
        })
        .collect()
}

fn all_snippets(lanes: &ContextLanes) -> Vec<&ContextSnippet> {
    lanes
        .instruction_candidates
        .iter()
        .chain(&lanes.trusted_context)
        .chain(&lanes.untrusted_data)
        .collect()
}
