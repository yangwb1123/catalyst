use std::collections::HashMap;

use serde::Serialize;

use super::{
    ContextBudget, ContextLanes, ContextOmission, ContextPackage, ContextPackageBuildRequest,
    ContextPackageContractError, ContextSnippet, ContextSource, DELIMITER, DeclaredLane,
    InjectionRisk, NORMALIZATION, OmissionReason, PROJECTED_CONTENT_DIGEST_DOMAIN, ProjectedLane,
    REDACTION_REPLACEMENT, RedactionRange, SelectionReason, SnippetTruncation, SourceAvailability,
    SourceClass, SourceDisposition, SourceFreshness, SourceTruncationPolicy, TokenCounter, codec,
    invalid, validation,
};

struct Candidate<'a> {
    source: &'a ContextSource,
    snippet: ContextSnippet,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct Projection<'a> {
    instruction_candidates: Vec<ProjectionItem<'a>>,
    trusted_context: Vec<ProjectionItem<'a>>,
    untrusted_data: Vec<ProjectionItem<'a>>,
}

#[derive(Serialize)]
#[serde(deny_unknown_fields)]
struct ProjectionItem<'a> {
    content: &'a str,
    instruction_allowed: bool,
    source_id: &'a str,
}

pub(super) struct Selection {
    pub(super) actual_tokens: u64,
    pub(super) content_bytes: u64,
    pub(super) lanes: ContextLanes,
    pub(super) omissions: Vec<ContextOmission>,
}

/// Assembles one deterministic authority-free `ContextPackage` projection.
///
/// # Errors
///
/// Returns an error when input validation, redaction, required-source admission,
/// pinned token counting, a required budget, or canonical encoding fails closed.
pub fn assemble(
    request: &ContextPackageBuildRequest,
    counter: &impl TokenCounter,
) -> Result<ContextPackage, ContextPackageContractError> {
    validation::validate_request(request)?;
    validation::validate_tokenizer_identity(&request.budget, &counter.identity())?;
    let request_json = codec::canonical_request_json(request)?;
    let (candidates, omissions) = collect_candidates(request)?;
    let selection = select_candidates(&candidates, omissions, &request.budget, counter)?;
    super::package::build_package(request, request_json.as_bytes(), selection)
}

/// Reassembles a package and demands exact equality with every derived field.
///
/// # Errors
///
/// Returns an error when either input is invalid or the package differs from reassembly.
pub fn validate_package(
    request: &ContextPackageBuildRequest,
    package: &ContextPackage,
    counter: &impl TokenCounter,
) -> Result<(), ContextPackageContractError> {
    super::package_validation::validate_package_shape(package)?;
    let expected = assemble(request, counter)?;
    let actual_json = codec::canonical_package_json(package)?;
    let expected_json = codec::canonical_package_json(&expected)?;
    if actual_json != expected_json {
        return Err(invalid(
            "context package does not exactly match deterministic reassembly",
        ));
    }
    Ok(())
}

/// Binds a cached package to the recomputed request key before full reassembly.
///
/// # Errors
///
/// Returns an error before token counting when the request is invalid or the
/// stored cache key does not exactly match the request-derived key.
pub fn validate_cache_hit(
    request: &ContextPackageBuildRequest,
    package: &ContextPackage,
    counter: &impl TokenCounter,
) -> Result<(), ContextPackageContractError> {
    let expected_key = codec::cache_key_sha256(request)?;
    if package.cache_key_sha256 != expected_key {
        return Err(invalid("cached context package key does not match request"));
    }
    validate_package(request, package, counter)
}

fn collect_candidates(
    request: &ContextPackageBuildRequest,
) -> Result<(Vec<Candidate<'_>>, Vec<ContextOmission>), ContextPackageContractError> {
    let plans: HashMap<_, _> = request
        .redactions
        .iter()
        .map(|plan| (plan.source_id.as_str(), plan.ranges.as_slice()))
        .collect();
    let mut ordered: Vec<_> = request.sources.iter().collect();
    ordered.sort_by(|left, right| candidate_order(left, right));
    let mut candidates = Vec::new();
    let mut omissions = Vec::new();
    for source in ordered {
        let ranges = plans.get(source.source_id.as_str()).copied().unwrap_or(&[]);
        let redacted = redact_available(source, ranges)?;
        if let Some(reason) = ineligible_reason(source, request.source_binding.as_of_unix_ms) {
            reject_or_omit(source, reason, &mut omissions)?;
            continue;
        }
        match prepare_candidate(source, redacted.as_deref().unwrap_or_default())? {
            Some(candidate) => candidates.push(candidate),
            None => omissions.push(omission(source, OmissionReason::SourceLimitExceeded)),
        }
    }
    Ok((candidates, omissions))
}

fn candidate_order(left: &ContextSource, right: &ContextSource) -> std::cmp::Ordering {
    left.category
        .cmp(&right.category)
        .then_with(|| right.priority.cmp(&left.priority))
        .then_with(|| left.source_id.as_bytes().cmp(right.source_id.as_bytes()))
}

fn redact_available(
    source: &ContextSource,
    ranges: &[RedactionRange],
) -> Result<Option<String>, ContextPackageContractError> {
    let Some(content) = source.content.as_deref() else {
        return Ok(None);
    };
    let mut output = Vec::with_capacity(content.len());
    let mut cursor = 0_usize;
    for range in ranges {
        let start = usize::try_from(range.start_byte)
            .map_err(|_| invalid("redaction start_byte is too large"))?;
        let end = usize::try_from(range.end_byte)
            .map_err(|_| invalid("redaction end_byte is too large"))?;
        output.extend_from_slice(&content.as_bytes()[cursor..start]);
        output.extend_from_slice(REDACTION_REPLACEMENT.as_bytes());
        cursor = end;
    }
    output.extend_from_slice(&content.as_bytes()[cursor..]);
    String::from_utf8(output)
        .map(Some)
        .map_err(|_| invalid("redaction produced invalid UTF-8"))
}

fn ineligible_reason(source: &ContextSource, as_of: i64) -> Option<OmissionReason> {
    if source.availability == SourceAvailability::Missing {
        Some(OmissionReason::Missing)
    } else if source.disposition == SourceDisposition::Deny {
        Some(OmissionReason::Denied)
    } else if source.freshness == SourceFreshness::Stale {
        Some(OmissionReason::Stale)
    } else if source.freshness == SourceFreshness::Contested {
        Some(OmissionReason::Contested)
    } else if source.freshness == SourceFreshness::Unknown {
        Some(OmissionReason::UnknownFreshness)
    } else if source
        .expires_at_unix_ms
        .is_some_and(|expires| as_of >= expires)
    {
        Some(OmissionReason::Expired)
    } else if source.injection_risk == InjectionRisk::Suspected {
        Some(OmissionReason::QuarantinedPromptInjection)
    } else {
        None
    }
}

fn reject_or_omit(
    source: &ContextSource,
    reason: OmissionReason,
    omissions: &mut Vec<ContextOmission>,
) -> Result<(), ContextPackageContractError> {
    if source.required {
        return Err(invalid(format!(
            "required source {} is ineligible: {reason:?}",
            source.source_id
        )));
    }
    omissions.push(omission(source, reason));
    Ok(())
}

fn prepare_candidate<'a>(
    source: &'a ContextSource,
    redacted: &str,
) -> Result<Option<Candidate<'a>>, ContextPackageContractError> {
    let maximum =
        usize::try_from(source.max_bytes).map_err(|_| invalid("source max_bytes is too large"))?;
    let (content, truncation) = if redacted.len() <= maximum {
        (redacted.to_owned(), None)
    } else if source.required {
        return Err(invalid(format!(
            "required source {} exceeds source max_bytes",
            source.source_id
        )));
    } else if source.truncation == SourceTruncationPolicy::Forbidden {
        return Ok(None);
    } else {
        let prefix = utf8_prefix(redacted, maximum);
        if prefix.is_empty() {
            return Ok(None);
        }
        let receipt = SnippetTruncation {
            original_redacted_bytes: redacted.len() as u64,
            reason: "source_max_bytes".into(),
            retained_bytes: prefix.len() as u64,
        };
        (prefix.to_owned(), Some(receipt))
    };
    Ok(Some(Candidate {
        source,
        snippet: make_snippet(source, content, truncation)?,
    }))
}

fn utf8_prefix(content: &str, maximum: usize) -> &str {
    let mut retained = maximum.min(content.len());
    while retained > 0 && !content.is_char_boundary(retained) {
        retained -= 1;
    }
    &content[..retained]
}

fn make_snippet(
    source: &ContextSource,
    content: String,
    truncation: Option<SnippetTruncation>,
) -> Result<ContextSnippet, ContextPackageContractError> {
    let lane = projected_lane(source);
    let mut snippet = ContextSnippet {
        category: source.category,
        projected_content_sha256: codec::domain_sha256(
            PROJECTED_CONTENT_DIGEST_DOMAIN,
            content.as_bytes(),
        ),
        content,
        declared_lane: source.declared_lane,
        declared_trust: source.declared_trust,
        delimiter: DELIMITER.into(),
        instruction_allowed: false,
        lane,
        normalization: NORMALIZATION.into(),
        required: source.required,
        selection_reason: if source.required {
            SelectionReason::RequiredSource
        } else {
            SelectionReason::PrioritySelection
        },
        snippet_sha256: String::new(),
        source_class: source.source_class,
        source_content_sha256: source.content_sha256.clone().unwrap_or_default(),
        source_id: source.source_id.clone(),
        source_ref: source.source_ref.clone(),
        source_revision: source.source_revision.clone(),
        truncation,
    };
    snippet.snippet_sha256 = codec::snippet_sha256(&snippet)?;
    Ok(snippet)
}

fn projected_lane(source: &ContextSource) -> ProjectedLane {
    if matches!(
        source.source_class,
        SourceClass::SystemPolicy | SourceClass::UserInstruction
    ) && source.declared_lane == DeclaredLane::Instruction
    {
        ProjectedLane::InstructionCandidates
    } else if source.declared_lane == DeclaredLane::UntrustedData {
        ProjectedLane::UntrustedData
    } else {
        ProjectedLane::TrustedContext
    }
}

fn select_candidates(
    candidates: &[Candidate<'_>],
    mut omissions: Vec<ContextOmission>,
    budget: &ContextBudget,
    counter: &impl TokenCounter,
) -> Result<Selection, ContextPackageContractError> {
    let mut lanes = empty_lanes();
    let mut actual_tokens = count_projection(&lanes, budget, counter)?;
    if actual_tokens > budget.max_tokens {
        return Err(invalid(
            "token budget cannot represent the empty projection",
        ));
    }
    let mut content_bytes = 0_u64;
    let ordered = candidates
        .iter()
        .filter(|candidate| candidate.source.required)
        .chain(
            candidates
                .iter()
                .filter(|candidate| !candidate.source.required),
        );
    for candidate in ordered {
        select_candidate(
            &mut lanes,
            candidate,
            &mut content_bytes,
            &mut actual_tokens,
            &mut omissions,
            budget,
            counter,
        )?;
    }
    omissions.sort_by(|left, right| left.source_id.as_bytes().cmp(right.source_id.as_bytes()));
    Ok(Selection {
        actual_tokens,
        content_bytes,
        lanes,
        omissions,
    })
}

fn select_candidate(
    lanes: &mut ContextLanes,
    candidate: &Candidate<'_>,
    content_bytes: &mut u64,
    actual_tokens: &mut u64,
    omissions: &mut Vec<ContextOmission>,
    budget: &ContextBudget,
    counter: &impl TokenCounter,
) -> Result<(), ContextPackageContractError> {
    match try_select(lanes, &candidate.snippet, *content_bytes, budget, counter)? {
        Ok(tokens) => {
            *content_bytes += candidate.snippet.content.len() as u64;
            *actual_tokens = tokens;
        }
        Err(reason) if candidate.source.required => {
            return Err(invalid(format!(
                "required source {} exceeds {reason:?}",
                candidate.source.source_id
            )));
        }
        Err(reason) => omissions.push(omission(candidate.source, reason)),
    }
    Ok(())
}

fn try_select(
    lanes: &mut ContextLanes,
    snippet: &ContextSnippet,
    content_bytes: u64,
    budget: &ContextBudget,
    counter: &impl TokenCounter,
) -> Result<Result<u64, OmissionReason>, ContextPackageContractError> {
    if selected_count(lanes) + 1 > budget.max_snippets {
        return Ok(Err(OmissionReason::SnippetBudgetExceeded));
    }
    if content_bytes + snippet.content.len() as u64 > budget.max_content_bytes {
        return Ok(Err(OmissionReason::ContentBudgetExceeded));
    }
    lane_mut(lanes, snippet.lane).push(snippet.clone());
    let tokens = count_projection(lanes, budget, counter)?;
    if tokens > budget.max_tokens {
        lane_mut(lanes, snippet.lane).pop();
        Ok(Err(OmissionReason::TokenBudgetExceeded))
    } else {
        Ok(Ok(tokens))
    }
}

fn count_projection(
    lanes: &ContextLanes,
    budget: &ContextBudget,
    counter: &impl TokenCounter,
) -> Result<u64, ContextPackageContractError> {
    validation::validate_tokenizer_identity(budget, &counter.identity())?;
    let projection = projection_json(lanes)?;
    counter.count(projection.as_bytes())
}

pub(super) fn projection_json(lanes: &ContextLanes) -> Result<String, ContextPackageContractError> {
    codec::canonical_json(&Projection {
        instruction_candidates: projection_items(&lanes.instruction_candidates),
        trusted_context: projection_items(&lanes.trusted_context),
        untrusted_data: projection_items(&lanes.untrusted_data),
    })
}

fn projection_items(snippets: &[ContextSnippet]) -> Vec<ProjectionItem<'_>> {
    snippets
        .iter()
        .map(|snippet| ProjectionItem {
            content: &snippet.content,
            instruction_allowed: false,
            source_id: &snippet.source_id,
        })
        .collect()
}

fn omission(source: &ContextSource, reason: OmissionReason) -> ContextOmission {
    ContextOmission {
        reason,
        source_id: source.source_id.clone(),
        source_ref: source.source_ref.clone(),
    }
}

fn empty_lanes() -> ContextLanes {
    ContextLanes {
        instruction_candidates: Vec::new(),
        trusted_context: Vec::new(),
        untrusted_data: Vec::new(),
    }
}

fn lane_mut(lanes: &mut ContextLanes, lane: ProjectedLane) -> &mut Vec<ContextSnippet> {
    match lane {
        ProjectedLane::InstructionCandidates => &mut lanes.instruction_candidates,
        ProjectedLane::TrustedContext => &mut lanes.trusted_context,
        ProjectedLane::UntrustedData => &mut lanes.untrusted_data,
    }
}

fn selected_count(lanes: &ContextLanes) -> u64 {
    (lanes.instruction_candidates.len() + lanes.trusted_context.len() + lanes.untrusted_data.len())
        as u64
}
