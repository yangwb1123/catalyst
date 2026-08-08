use super::dispatch_execution_adapters::{read_authorization_bounded, read_pricing_bounded};
use super::*;

#[test]
fn release_control_writes_exact_canonical_bytes_irrespective_of_json_mode() {
    let canonical = r#"{"v":1,"private":"exact"}"#;
    let output = GroupAgentGraphRunDispatchCommandCliOutput::ReleaseControl(canonical.into());
    for json in [false, true] {
        let mut bytes = Vec::new();
        write_output(&output, json, &mut bytes).expect("write release control");
        assert_eq!(bytes, canonical.as_bytes());
        assert!(!bytes.ends_with(b"\n"));
    }
}

#[test]
fn authorization_reader_rejects_bytes_past_the_public_bound() {
    let bytes = vec![
        b'x';
        crate::runtime_domain::MAX_GROUP_AGENT_NODE_DISPATCH_AUTHORIZATION_BYTES + 1
    ];
    let error = read_authorization_bounded(bytes.as_slice()).expect_err("oversize input fails");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("exceeds its byte limit"));
}

#[test]
fn pricing_reader_rejects_bytes_past_the_public_bound() {
    let bytes = vec![b'x'; crate::runtime_domain::MAX_GROUP_AGENT_NODE_PRICING_SNAPSHOT_BYTES + 1];
    let error = read_pricing_bounded(bytes.as_slice()).expect_err("oversize input fails");
    assert_eq!(error.kind(), io::ErrorKind::InvalidInput);
    assert!(error.to_string().contains("exceeds its byte limit"));
}
