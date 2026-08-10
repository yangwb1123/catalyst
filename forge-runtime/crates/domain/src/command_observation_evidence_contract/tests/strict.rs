use super::*;

#[test]
fn decoder_rejects_unknown_duplicate_noncanonical_float_and_overflow() {
    let canonical = fixture().expected.canonical_request_json;
    let duplicate = canonical.replacen(
        "{\"api_version\":",
        "{\"api_version\":\"forgeos.governance.command-observation-evidence-adapter/v1\",\"api_version\":",
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
    request.observation.command.argv.push("报告".into());
    canonical_request_json(&request).expect("safe Unicode argument");
    request.observation.command.argv.push("bad\u{202e}".into());
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn argv_cwd_stdin_timeout_and_digest_rules_are_strict() {
    let mut request = fixture().request;
    request.observation.command.argv[0].clear();
    assert!(canonical_request_json(&request).is_err());
    let mut request = fixture().request;
    request.observation.command.argv.push(String::new());
    canonical_request_json(&request).expect("empty non-executable argument is exact");

    for cwd in [
        "/tmp",
        "C:/secret",
        "work\\tree",
        "work//tree",
        "work/./tree",
        "work/../tree",
        "work/tree/",
    ] {
        let mut request = fixture().request;
        request.observation.command.cwd = cwd.into();
        assert!(canonical_request_json(&request).is_err(), "accepted {cwd}");
    }
    let mut request = fixture().request;
    request.observation.command.stdin_sha256 = "0".repeat(64);
    assert!(canonical_request_json(&request).is_err());
    let mut request = fixture().request;
    request.observation.command.timeout_ms = Some(0);
    assert!(canonical_request_json(&request).is_err());
    let mut request = fixture().request;
    request.observation.command.timeout_ms = Some(86_400_001);
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn timeout_and_cancel_are_valid_observations_but_not_projectable_requests() {
    for kind in [
        CommandTerminationKind::TimedOut,
        CommandTerminationKind::Cancelled,
    ] {
        let mut request = fixture().request;
        request.observation.termination.kind = kind;
        request.observation.termination.exit_code = None;
        canonical_observation_json(&request.observation).expect("valid capture state");
        assert!(canonical_request_json(&request).is_err());
        assert!(adapt_canonical_request(&raw_json(&request)).is_err());
    }

    let mut request = fixture().request;
    request.observation.termination.exit_code = Some(-1);
    assert!(canonical_request_json(&request).is_err());
    request.observation.termination.exit_code = Some(2_147_483_648);
    assert!(canonical_request_json(&request).is_err());
}

#[test]
fn signaled_and_spawn_failed_cannot_be_reclassified_as_v1_termination() {
    let canonical = fixture().expected.canonical_request_json;
    for forbidden in ["signaled", "spawn_failed"] {
        let raw = canonical.replacen(
            "\"kind\":\"exited\"",
            &format!("\"kind\":\"{forbidden}\""),
            1,
        );
        assert!(decode_canonical_request(raw.as_bytes()).is_err());
    }
}

#[test]
fn stream_counts_empty_hashes_and_retention_fail_closed() {
    let mutations: [fn(&mut CommandObservationEvidenceRequest); 6] = [
        |r| r.observation.streams.combined.bytes = 12,
        |r| r.observation.streams.stdout.retained_bytes = 9,
        |r| r.observation.streams.stderr.retained_sha256 = "0".repeat(64),
        |r| r.observation.streams.stderr.sha256 = "0".repeat(64),
        |r| r.observation.streams.stdout.retained_bytes = -1,
        |r| r.observation.streams.stdout.bytes = -1,
    ];
    for mutate in mutations {
        let mut request = fixture().request;
        mutate(&mut request);
        assert!(canonical_request_json(&request).is_err());
    }

    let mut overflow = fixture().request;
    overflow.observation.streams.stdout.bytes = i64::MAX;
    overflow.observation.streams.stdout.retained_bytes = 5;
    assert!(canonical_request_json(&overflow).is_err());
}

#[test]
fn exported_digest_helpers_reject_invalid_complete_requests() {
    let mut request = fixture().request;
    request.binding.subjects = vec!["z".into(), "a".into()];
    assert!(command_sha256(&request).is_err());
    assert!(source_snapshot_sha256(&request).is_err());
    assert!(request_sha256(&request).is_err());
}
