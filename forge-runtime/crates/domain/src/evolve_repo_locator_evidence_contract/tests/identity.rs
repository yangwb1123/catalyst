use super::*;

#[test]
fn locator_observation_and_binding_have_separate_identity_domains() {
    let base = adapt_fixture();

    let mut locator = fixture().request;
    locator.observation.locator.detail.push_str(" Changed.");
    let locator_variant =
        adapt_canonical_request(canonical_request(&locator).as_bytes()).expect("locator variant");
    assert_ne!(locator_variant.locator_sha256, base.locator_sha256);
    assert_ne!(
        locator_variant.source_snapshot_sha256,
        base.source_snapshot_sha256
    );
    assert_ne!(locator_variant.request_sha256, base.request_sha256);

    let mut observation = fixture().request;
    observation.observation.content.bytes += 1;
    let observation_variant = adapt_canonical_request(canonical_request(&observation).as_bytes())
        .expect("observation variant");
    assert_eq!(observation_variant.locator_sha256, base.locator_sha256);
    assert_ne!(
        observation_variant.source_snapshot_sha256,
        base.source_snapshot_sha256
    );
    assert_ne!(observation_variant.request_sha256, base.request_sha256);

    let mut binding = fixture().request;
    binding.binding.sequence = 2;
    let binding_variant =
        adapt_canonical_request(canonical_request(&binding).as_bytes()).expect("binding variant");
    assert_eq!(binding_variant.locator_sha256, base.locator_sha256);
    assert_eq!(
        binding_variant.source_snapshot_sha256,
        base.source_snapshot_sha256
    );
    assert_ne!(binding_variant.request_sha256, base.request_sha256);
}

#[test]
fn every_declared_observation_surface_changes_source_identity() {
    let base = adapt_fixture();
    let mutations: [fn(&mut EvolveRepoLocatorEvidenceRequest); 7] = [
        |r| r.observation.observed_at_unix_ms += 1,
        |r| r.observation.producer.run_id = "run-evolve-0051".into(),
        |r| r.observation.scan_context.depth = EvolveScanDepth::Standard,
        |r| r.observation.scan_context.dimension = EvolveDimension::Security,
        |r| r.observation.scan_context.report_sha256 = "1".repeat(64),
        |r| r.observation.source.source_revision = "fixture-revision-0051".into(),
        |r| r.observation.content.sha256 = "2".repeat(64),
    ];
    for mutate in mutations {
        let mut request = fixture().request;
        mutate(&mut request);
        let adapted = adapt_canonical_request(canonical_request(&request).as_bytes())
            .expect("valid observation identity variant");
        assert_eq!(adapted.locator_sha256, base.locator_sha256);
        assert_ne!(adapted.source_snapshot_sha256, base.source_snapshot_sha256);
        assert_ne!(adapted.request_sha256, base.request_sha256);
        assert_ne!(
            adapted.evidence.integrity.canonical_sha256,
            base.evidence.integrity.canonical_sha256
        );
    }
}

#[test]
fn exported_digest_helpers_reject_invalid_complete_requests() {
    let mut request = fixture().request;
    request.binding.subjects = vec!["z".into(), "a".into()];
    assert!(locator_sha256(&request).is_err());
    assert!(source_snapshot_sha256(&request).is_err());
    assert!(request_sha256(&request).is_err());

    let mut oversized = fixture().request;
    oversized.binding.subjects = (0..=256).map(|index| format!("s{index:03}")).collect();
    assert!(locator_sha256(&oversized).is_err());
    assert!(source_snapshot_sha256(&oversized).is_err());
    assert!(request_sha256(&oversized).is_err());
}
