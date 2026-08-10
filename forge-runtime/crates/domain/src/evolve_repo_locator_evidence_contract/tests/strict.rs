use super::*;

#[test]
fn decoder_rejects_unknown_duplicate_noncanonical_float_and_overflow() {
    let canonical = fixture().expected.canonical_request_json;
    let duplicate = canonical.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"forgeos.governance.evolve-repo-locator-evidence-adapter/v1\",\"api_version\":",
        1,
    );
    assert!(decode_canonical_request(duplicate.as_bytes()).is_err());
    let unknown = canonical.replacen("{\"api_version\":", "{\"alien\":null,\"api_version\":", 1);
    assert!(decode_canonical_request(unknown.as_bytes()).is_err());
    assert!(decode_canonical_request(format!(" {canonical}").as_bytes()).is_err());
    let float = canonical.replacen("\"sequence\":1", "\"sequence\":1.0", 1);
    assert!(decode_canonical_request(float.as_bytes()).is_err());
    let overflow = canonical.replacen("\"sequence\":1", "\"sequence\":9223372036854775808", 1);
    assert!(decode_canonical_request(overflow.as_bytes()).is_err());
}

#[test]
fn unicode_spelling_and_forbidden_scalars_fail_closed() {
    let canonical = fixture().expected.canonical_request_json;
    let escaped = canonical.replacen("fixture-revision", "fixture\\u002drevision", 1);
    assert!(decode_canonical_request(escaped.as_bytes()).is_err());
    let bidi = canonical.replacen("fixture-revision", "fixture\\u202erevision", 1);
    assert!(decode_canonical_request(bidi.as_bytes()).is_err());

    let mut request = fixture().request;
    request.observation.locator.detail = "仓库证据".into();
    canonical_request_json(&request).expect("safe Unicode detail");
    request.observation.locator.detail.push('\u{202e}');
    assert!(canonical_request_json(&request).is_err());
    request.observation.locator.detail = "bad\u{0085}detail".into();
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn repository_path_rules_reject_escape_drive_and_casefolded_control_roots() {
    for path in [
        "",
        "   ",
        "/tmp/file",
        "C:file",
        "c:/file",
        "work\\file",
        "work//file",
        "work/./file",
        "work/../file",
        "work/file/",
        ".git/config",
        ".GIT/config",
        ".forge/state",
        ".FoRgE/state",
        "dir/bad\u{0085}path",
    ] {
        let mut request = fixture().request;
        request.observation.locator.path = path.into();
        assert!(
            canonical_request_json(&request).is_err(),
            "accepted {path:?}"
        );
    }
    let mut request = fixture().request;
    request.observation.locator.path = "a".repeat(4097);
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn content_line_detail_and_hash_limits_fail_closed() {
    let mutations: [fn(&mut EvolveRepoLocatorEvidenceRequest); 7] = [
        |r| r.observation.content.bytes = 0,
        |r| r.observation.content.bytes = 1_048_577,
        |r| r.observation.content.sha256 = "0".repeat(63),
        |r| r.observation.locator.line = -1,
        |r| r.observation.locator.detail.clear(),
        |r| r.observation.locator.detail = " ".into(),
        |r| r.observation.locator.detail = "x".repeat(513),
    ];
    for mutate in mutations {
        let mut request = fixture().request;
        mutate(&mut request);
        assert!(canonical_request_json(&request).is_err());
    }
}

#[test]
fn opportunity_id_preserves_the_evolve_scan_v1_vocabulary() {
    for value in ["x:y".into(), "x/y".into(), "a".repeat(65)] {
        let mut request = fixture().request;
        request.observation.scan_context.opportunity_id = Some(value);
        assert!(canonical_request_json(&request).is_err());
    }
}

#[test]
fn relation_and_opportunity_id_form_a_strict_union() {
    let mut finding_with_id = fixture().request;
    finding_with_id.observation.scan_context.relation = EvolveLocatorRelation::Finding;
    assert!(canonical_request_json(&finding_with_id).is_err());

    let mut opportunity_without_id = fixture().request;
    opportunity_without_id
        .observation
        .scan_context
        .opportunity_id = None;
    assert!(canonical_request_json(&opportunity_without_id).is_err());

    let mut clear = fixture().request;
    clear.observation.scan_context.relation = EvolveLocatorRelation::Clear;
    clear.observation.scan_context.opportunity_id = None;
    canonical_request_json(&clear).expect("clear locator without opportunity id");
}

#[test]
fn identity_hash_and_sorted_list_rules_are_strict() {
    let mutations: [fn(&mut EvolveRepoLocatorEvidenceRequest); 6] = [
        |r| r.observation.producer.producer_id = "Bad-ID".into(),
        |r| r.observation.producer.parameters_sha256 = "A".repeat(64),
        |r| r.observation.source.source_tree_sha256 = "0".repeat(63),
        |r| r.observation.scan_context.report_sha256 = "g".repeat(64),
        |r| r.binding.subjects = vec!["z".into(), "a".into()],
        |r| r.binding.supersedes_record_ids = vec!["same".into(), "same".into()],
    ];
    for mutate in mutations {
        let mut request = fixture().request;
        mutate(&mut request);
        assert!(canonical_request_json(&request).is_err());
    }
}

#[test]
fn typed_decoder_rejects_invalid_enums_and_adapter_version() {
    let canonical = fixture().expected.canonical_request_json;
    for (from, to) in [
        ("\"depth\":\"thorough\"", "\"depth\":\"deep\""),
        (
            "\"dimension\":\"architecture_drift\"",
            "\"dimension\":\"database\"",
        ),
        ("\"producer_type\":\"tool\"", "\"producer_type\":\"human\""),
        (
            "\"relation\":\"opportunity\"",
            "\"relation\":\"unavailable\"",
        ),
    ] {
        assert!(decode_canonical_request(canonical.replacen(from, to, 1).as_bytes()).is_err());
    }
    let mut request = fixture().request;
    request.observation.scan_context.contract = "evolve_scan_v2".into();
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn exported_observation_and_locator_helpers_validate_their_own_inputs() {
    let mut request = fixture().request;
    request.observation.locator.path = ".GiT/config".into();
    assert!(canonical_locator_json(&request.observation.locator).is_err());
    assert!(canonical_observation_json(&request.observation).is_err());

    let mut request = fixture().request;
    request.observation.producer.parameters_sha256 = "0".repeat(63);
    assert!(canonical_observation_json(&request.observation).is_err());
}

#[test]
fn raw_noncanonical_requests_do_not_become_valid_by_typed_roundtrip() {
    let mut request = fixture().request;
    request.binding.sequence = 0;
    assert!(adapt_canonical_request(&raw_json(&request)).is_err());
}
