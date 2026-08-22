use std::cell::Cell;

use serde::Deserialize;

use super::*;

const FIXTURE: &str =
    include_str!("../../../../../docs/contracts/fixtures/context-package-v1.json");
const BYTE_COUNTER_ID: &str = "forgeos.token-counter.utf8-bytes/v1";
const BYTE_COUNTER_SHA256: &str =
    "44799f99769528ecb46bcad483faf2d8ff4ab086bf32b2fe692a18f0eebea3cf";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GoldenFixture {
    expected_package: ContextPackage,
    request: ContextPackageBuildRequest,
}

struct ByteCounter;

impl TokenCounter for ByteCounter {
    fn identity(&self) -> TokenizerIdentity {
        TokenizerIdentity {
            tokenizer_id: BYTE_COUNTER_ID.into(),
            tokenizer_sha256: BYTE_COUNTER_SHA256.into(),
        }
    }

    fn count(&self, projection: &[u8]) -> Result<u64, ContextPackageContractError> {
        Ok(projection.len() as u64)
    }
}

struct WrongCounter;

impl TokenCounter for WrongCounter {
    fn identity(&self) -> TokenizerIdentity {
        TokenizerIdentity {
            tokenizer_id: "wrong-counter".into(),
            tokenizer_sha256: "0".repeat(64),
        }
    }

    fn count(&self, projection: &[u8]) -> Result<u64, ContextPackageContractError> {
        Ok(projection.len() as u64)
    }
}

struct TrackingCounter {
    calls: Cell<usize>,
}

impl TokenCounter for TrackingCounter {
    fn identity(&self) -> TokenizerIdentity {
        TokenizerIdentity {
            tokenizer_id: BYTE_COUNTER_ID.into(),
            tokenizer_sha256: BYTE_COUNTER_SHA256.into(),
        }
    }

    fn count(&self, projection: &[u8]) -> Result<u64, ContextPackageContractError> {
        self.calls.set(self.calls.get() + 1);
        Ok(projection.len() as u64)
    }
}

fn fixture() -> GoldenFixture {
    serde_json::from_str(FIXTURE).expect("ContextPackage golden fixture")
}

fn reseal_source(source: &mut ContextSource, content: &str) {
    source.content = Some(content.into());
    source.content_sha256 = Some(codec::raw_sha256(content.as_bytes()));
}

#[test]
fn golden_request_assembles_exact_package_and_digests() {
    let fixture = fixture();
    let actual = assemble(&fixture.request, &ByteCounter).expect("assemble golden package");
    assert_eq!(actual, fixture.expected_package);
    assert_eq!(actual.accounting.actual_tokens, 404);
    assert_eq!(actual.accounting.candidate_count, 5);
    validate_package(&fixture.request, &actual, &ByteCounter).expect("validate golden package");
}

#[test]
fn canonical_request_and_package_round_trip_exactly() {
    let fixture = fixture();
    let request_json = canonical_request_json(&fixture.request).expect("canonical request");
    assert_eq!(
        decode_canonical_request(request_json.as_bytes()).expect("decode request"),
        fixture.request
    );
    let package_json =
        canonical_package_json(&fixture.expected_package).expect("canonical package");
    let package = decode_canonical_package(package_json.as_bytes()).expect("decode package");
    assert_eq!(package, fixture.expected_package);
    validate_package(&fixture.request, &package, &ByteCounter).expect("revalidate package");
}

#[test]
fn duplicate_unknown_float_noncanonical_and_oversized_requests_fail() {
    let canonical = canonical_request_json(&fixture().request).expect("canonical request");
    let duplicate = canonical.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"forgeos.context-package-build-request/v1\",\"api_version\":",
        1,
    );
    assert!(decode_canonical_request(duplicate.as_bytes()).is_err());
    let unknown = canonical.replacen("{\"api_version\":", "{\"added\":1,\"api_version\":", 1);
    assert!(decode_canonical_request(unknown.as_bytes()).is_err());
    let float = canonical.replacen("\"max_tokens\":2000", "\"max_tokens\":1.5", 1);
    assert!(decode_canonical_request(float.as_bytes()).is_err());
    assert!(decode_canonical_request(format!(" {canonical}").as_bytes()).is_err());
    assert!(decode_canonical_request(&vec![b' '; MAX_REQUEST_BYTES + 1]).is_err());
}

#[test]
fn source_order_and_reference_identity_are_strict() {
    let mut request = fixture().request;
    request.sources.swap(0, 1);
    assert!(canonical_request_json(&request).is_err());

    let mut request = fixture().request;
    request.sources[1].source_ref = request.sources[0].source_ref.clone();
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn untrusted_sources_cannot_upgrade_lane_or_trust() {
    let mut request = fixture().request;
    request.sources[2].declared_lane = DeclaredLane::Instruction;
    assert!(canonical_request_json(&request).is_err());

    let mut request = fixture().request;
    request.sources[2].declared_trust = DeclaredTrust::ProjectGovernance;
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn forbidden_content_and_empty_available_content_fail() {
    let mut request = fixture().request;
    reseal_source(&mut request.sources[1], "line\rcommand");
    assert!(canonical_request_json(&request).is_err());

    let mut request = fixture().request;
    reseal_source(&mut request.sources[1], "bad\u{202e}text");
    assert!(canonical_request_json(&request).is_err());

    let mut request = fixture().request;
    reseal_source(&mut request.sources[1], "");
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn redactions_require_utf8_boundaries_and_nonoverlap() {
    let mut request = fixture().request;
    reseal_source(&mut request.sources[0], "Never α secret");
    request.redactions[0].ranges = vec![RedactionRange {
        end_byte: 7,
        rule_id: "inside-rune".into(),
        start_byte: 6,
    }];
    assert!(canonical_request_json(&request).is_err());

    let mut request = fixture().request;
    let range = request.redactions[0].ranges[0].clone();
    request.redactions[0].ranges.push(range);
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn required_sources_fail_closed_for_eligibility_and_budgets() {
    let mut request = fixture().request;
    request.sources[0].freshness = SourceFreshness::Stale;
    assert!(assemble(&request, &ByteCounter).is_err());

    let mut request = fixture().request;
    request.budget.max_content_bytes = 1;
    assert!(assemble(&request, &ByteCounter).is_err());

    let mut request = fixture().request;
    request.sources[0].max_bytes = 1;
    assert!(assemble(&request, &ByteCounter).is_err());
}

#[test]
fn optional_utf8_prefix_that_cannot_retain_a_scalar_is_omitted() {
    let mut request = fixture().request;
    reseal_source(&mut request.sources[2], "α");
    request.sources[2].max_bytes = 1;
    let package = assemble(&request, &ByteCounter).expect("assemble with optional omission");
    assert!(package.omissions.iter().any(|omission| {
        omission.source_id == "source-03-repository"
            && omission.reason == OmissionReason::SourceLimitExceeded
    }));
    assert_eq!(
        package.accounting.candidate_count,
        package.accounting.selected_snippet_count + package.accounting.omitted_source_count
    );
}

#[test]
fn tokenizer_identity_and_package_tampering_fail() {
    let fixture = fixture();
    assert!(assemble(&fixture.request, &WrongCounter).is_err());

    let mut package = fixture.expected_package;
    package.accounting.actual_tokens += 1;
    assert!(validate_package(&fixture.request, &package, &ByteCounter).is_err());
}

#[test]
fn package_decoder_rejects_lane_and_resource_escalation() {
    let mut package = fixture().expected_package;
    let mut snippet = package
        .lanes
        .untrusted_data
        .pop()
        .expect("fixture untrusted snippet");
    snippet.lane = ProjectedLane::TrustedContext;
    package.lanes.trusted_context.push(snippet);
    let encoded = canonical_package_json(&package).expect("encode mutated package");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());

    let mut package = fixture().expected_package;
    package.redaction_receipts[0].ranges[0].end_byte = 131_073;
    let encoded = canonical_package_json(&package).expect("encode oversized receipt range");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());

    let mut package = fixture().expected_package;
    package.lanes.untrusted_data[0]
        .truncation
        .as_mut()
        .expect("fixture truncation")
        .retained_bytes += 1;
    let encoded = canonical_package_json(&package).expect("encode invalid truncation");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());

    let mut package = fixture().expected_package;
    package.accounting.content_bytes += 1;
    let encoded = canonical_package_json(&package).expect("encode accounting drift");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());

    let mut package = fixture().expected_package;
    package.omissions[0].source_id = "source-01-policy".into();
    package.omissions[0].source_ref = "fixture://source-01-policy".into();
    let encoded = canonical_package_json(&package).expect("encode overlapping partition");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());

    let mut package = fixture().expected_package;
    package.lanes.instruction_candidates[0].selection_reason = SelectionReason::PrioritySelection;
    let encoded = canonical_package_json(&package).expect("encode selection reason drift");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());
}

#[test]
fn package_decoder_rejects_invalid_freshness_binding() {
    let mut package = fixture().expected_package;
    package.freshness.evaluated_at_unix_ms += 1;
    let encoded = canonical_package_json(&package).expect("encode divergent evaluation time");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());

    let mut package = fixture().expected_package;
    package.freshness.expires_at_unix_ms = Some(package.freshness.evaluated_at_unix_ms);
    let encoded = canonical_package_json(&package).expect("encode nonfuture expiry");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());
}

#[test]
fn package_decoder_rejects_required_snippet_truncation() {
    let mut package = fixture().expected_package;
    let snippet = &mut package.lanes.instruction_candidates[0];
    snippet.truncation = Some(SnippetTruncation {
        original_redacted_bytes: snippet.content.len() as u64 + 1,
        reason: "source_max_bytes".into(),
        retained_bytes: snippet.content.len() as u64,
    });
    let encoded = canonical_package_json(&package).expect("encode required truncation");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());
}

#[test]
fn package_decoder_rejects_an_empty_candidate_partition() {
    let mut package = fixture().expected_package;
    package.lanes.instruction_candidates.clear();
    package.lanes.trusted_context.clear();
    package.lanes.untrusted_data.clear();
    package.omissions.clear();
    package.accounting.actual_tokens = 0;
    package.accounting.candidate_count = 0;
    package.accounting.content_bytes = 0;
    package.accounting.omitted_source_count = 0;
    package.accounting.selected_snippet_count = 0;
    package.accounting.truncated_snippet_count = 0;
    let encoded = canonical_package_json(&package).expect("encode empty candidate package");
    assert!(decode_canonical_package(encoded.as_bytes()).is_err());
}

#[test]
fn cache_hit_rejects_key_before_token_counting() {
    let fixture = fixture();
    let mut package = fixture.expected_package;
    package.cache_key_sha256.replace_range(..1, "0");
    let counter = TrackingCounter {
        calls: Cell::new(0),
    };
    assert!(validate_cache_hit(&fixture.request, &package, &counter).is_err());
    assert_eq!(counter.calls.get(), 0);
}

#[test]
fn optional_omissions_are_sorted_and_instruction_is_never_enabled() {
    let package = assemble(&fixture().request, &ByteCounter).expect("assemble fixture");
    assert!(
        package
            .omissions
            .windows(2)
            .all(|pair| pair[0].source_id.as_bytes() < pair[1].source_id.as_bytes())
    );
    assert!(
        package
            .lanes
            .instruction_candidates
            .iter()
            .chain(&package.lanes.trusted_context)
            .chain(&package.lanes.untrusted_data)
            .all(|snippet| !snippet.instruction_allowed)
    );
}
