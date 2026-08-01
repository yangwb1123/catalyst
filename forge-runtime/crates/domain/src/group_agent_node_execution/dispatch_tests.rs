use super::*;

const DIGEST_A: &str = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
const DIGEST_B: &str = "bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb";
const DIGEST_C: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";
const DIGEST_D: &str = "dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd";
const DIGEST_E: &str = "eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee";
const BODY_SHA256: &str = "8075abac4fd1ae16f654882594d9583a28c769020a4ed6a5251fe9b6f570b460";
const DESTINATION_SHA256: &str = "704e2556a22620cc2998d8660efb73039429b775bd5df56ecd0e0af77ca19bba";
const DISPATCH_SHA256: &str = "e4769b048d815a5d982d01ce16f8e2de5024346bf506bb3a94b9eb4f4e77a852";
const DISPATCH_ID: &str =
    "node-dispatch-request-e4769b048d815a5d982d01ce16f8e2de5024346bf506bb3a94b9eb4f4e77a852";
const DESTINATION_JSON: &str = r#"{"v":1,"provider_kind":"openai_responses","endpoint":"https://api.openai.com/v1/responses","model":"gpt-test"}"#;
const DISPATCH_PAYLOAD_JSON: &str = r#"{"v":1,"codec_protocol_version":1,"graph_run_id":"graph-run-test","contract_id":"node-contract-aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","contract_sha256":"aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa","expected_last_event_seq":2,"expected_last_event_sha256":"bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb","node_id":"node-a","attempt":1,"project_lane_sha256":"cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc","provider_kind":"openai_responses","endpoint":"https://api.openai.com/v1/responses","model":"gpt-test","destination_sha256":"704e2556a22620cc2998d8660efb73039429b775bd5df56ecd0e0af77ca19bba","logical_request_sha256":"dddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddddd","pricing_snapshot_sha256":"eeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeeee","request_body_bytes":2,"request_body_sha256":"8075abac4fd1ae16f654882594d9583a28c769020a4ed6a5251fe9b6f570b460"}"#;

#[test]
fn destination_identity_locks_exact_canonical_json_and_digest() {
    let payload = DestinationDigestPayload {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        provider_kind: GroupAgentNodeProviderKind::OpenAiResponses,
        endpoint: "https://api.openai.com/v1/responses",
        model: "gpt-test",
    };
    let encoded = serde_json::to_string(&payload).expect("encode destination payload");

    assert_eq!(encoded, DESTINATION_JSON);
    assert!(!encoded.ends_with('\n'));
    assert_eq!(
        group_agent_node_destination_sha256(
            GroupAgentNodeProviderKind::OpenAiResponses,
            payload.endpoint,
            payload.model,
        ),
        DESTINATION_SHA256
    );
}

#[test]
fn dispatch_identity_locks_exact_payload_digest_and_direct_id() {
    let record = record();

    assert_eq!(
        record.canonical_payload_json().expect("canonical payload"),
        DISPATCH_PAYLOAD_JSON
    );
    assert!(!DISPATCH_PAYLOAD_JSON.ends_with('\n'));
    assert!(DISPATCH_PAYLOAD_JSON.contains(&format!(r#""logical_request_sha256":"{DIGEST_D}""#)));
    assert!(!DISPATCH_PAYLOAD_JSON.contains(r#""request_sha256""#));
    assert_eq!(
        record.expected_sha256().expect("dispatch digest"),
        DISPATCH_SHA256
    );
    assert_eq!(record.dispatch_request_sha256, DISPATCH_SHA256);
    assert_eq!(
        group_agent_node_dispatch_request_id(DISPATCH_SHA256),
        DISPATCH_ID
    );
    assert_eq!(record.dispatch_request_id, DISPATCH_ID);
    record.validate().expect("valid golden dispatch record");
}

#[test]
fn every_payload_field_is_bound_but_creation_metadata_is_not() {
    let original = record();
    let original_digest = original.expected_sha256().expect("original digest");
    for (name, mutation) in identity_mutations().into_iter().chain(request_mutations()) {
        let mut changed = original.clone();
        mutation(&mut changed);
        assert_ne!(
            changed.expected_sha256().expect("mutated digest"),
            original_digest,
            "{name} must be content-addressed"
        );
        assert!(
            changed.validate().is_err(),
            "{name} with a stale digest and ID must fail closed"
        );
    }

    let mut later = original.clone();
    later.created_at_ms += 1;
    assert_eq!(
        later.expected_sha256().expect("non-semantic time"),
        original_digest
    );
}

type RecordMutation = (&'static str, fn(&mut GroupAgentNodeDispatchRequestRecord));

fn identity_mutations() -> Vec<RecordMutation> {
    vec![
        ("v", |record| record.v += 1),
        ("codec protocol", |record| {
            record.codec_protocol_version += 1;
        }),
        ("Graph Run", |record| record.graph_run_id.push('x')),
        ("contract ID", |record| record.contract_id.push('x')),
        ("contract digest", |record| {
            record.contract_sha256 = DIGEST_B.into();
        }),
        ("expected seq", |record| record.expected_last_event_seq += 1),
        ("expected head", |record| {
            record.expected_last_event_sha256 = DIGEST_C.into();
        }),
        ("node", |record| record.node_id.push('x')),
        ("attempt", |record| record.attempt += 1),
    ]
}

fn request_mutations() -> Vec<RecordMutation> {
    vec![
        ("project lane", |record| {
            record.project_lane_sha256 = DIGEST_D.into();
        }),
        ("endpoint", |record| record.endpoint.push('/')),
        ("model", |record| record.model.push('x')),
        ("destination", |record| {
            record.destination_sha256 = DIGEST_E.into();
        }),
        ("logical request", |record| {
            record.request_sha256 = DIGEST_E.into();
        }),
        ("pricing", |record| {
            record.pricing_snapshot_sha256 = DIGEST_D.into();
        }),
        ("body bytes", |record| record.provider_request_bytes += 1),
        ("body digest", |record| {
            record.provider_request_sha256 = DIGEST_A.into();
        }),
    ]
}

#[test]
fn body_identity_is_domain_separated_and_bound_by_exact_bytes() {
    assert_eq!(group_agent_node_provider_request_sha256(b"{}"), BODY_SHA256);
    assert_ne!(
        group_agent_node_provider_request_sha256(b"{}\n"),
        BODY_SHA256
    );
}

fn record() -> GroupAgentNodeDispatchRequestRecord {
    GroupAgentNodeDispatchRequestRecord {
        v: GROUP_AGENT_NODE_DISPATCH_REQUEST_VERSION,
        dispatch_request_id: DISPATCH_ID.into(),
        graph_run_id: "graph-run-test".into(),
        contract_id: format!("node-contract-{DIGEST_A}"),
        node_id: "node-a".into(),
        attempt: 1,
        contract_sha256: DIGEST_A.into(),
        request_sha256: DIGEST_D.into(),
        project_lane_sha256: DIGEST_C.into(),
        provider: GroupAgentNodeProviderKind::OpenAiResponses,
        endpoint: "https://api.openai.com/v1/responses".into(),
        model: "gpt-test".into(),
        pricing_snapshot_sha256: DIGEST_E.into(),
        provider_request_sha256: BODY_SHA256.into(),
        provider_request_bytes: 2,
        destination_sha256: DESTINATION_SHA256.into(),
        dispatch_request_sha256: DISPATCH_SHA256.into(),
        codec_protocol_version: GROUP_AGENT_NODE_DISPATCH_CODEC_VERSION,
        expected_last_event_seq: 2,
        expected_last_event_sha256: DIGEST_B.into(),
        created_at_ms: 7,
    }
}
