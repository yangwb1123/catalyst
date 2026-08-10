//! Spec-consistency tests: every constant below must match the authority
//! `docs/contracts/scheduled-successor-protocol.md` (single source of truth
//! shared with forge-core's Go implementation). Drift fails `forge accept`.

use std::process::Command;

use forge_runtime_domain::{
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_CONTRACT_DIGEST_DOMAIN, GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_EXECUTION_PROTOCOL_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_REQUEST_DIGEST_DOMAIN, GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION,
    MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES,
    MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES,
};

fn spec_value(table: &str, key: &str) -> String {
    let manifest = std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir");
    let script = format!("{manifest}/../../../harness/spec_check.py");
    let output = Command::new("python3")
        .args([&script, "--table", table, "--key", key])
        .output()
        .expect("run spec_check.py");
    assert!(output.status.success(), "spec_check {table}/{key} failed");
    String::from_utf8(output.stdout)
        .expect("spec value UTF-8")
        .trim()
        .to_owned()
}

fn spec_uint(table: &str, key: &str) -> u64 {
    spec_value(table, key).parse().expect("spec integer")
}

fn spec_domain(key: &str) -> Vec<u8> {
    // spec md 用 \\x00 字面表示域分隔符;unescape 成真实字节。
    let raw = spec_value("digests", key);
    let escaped = raw.replace("\\x00", "\0");
    escaped.into_bytes()
}

#[test]
fn spec_versions_match_rust_constants() {
    assert_eq!(
        u64::from(GROUP_AGENT_SCHEDULED_NODE_CONTRACT_VERSION),
        spec_uint("versions", "candidate.v"),
    );
    assert_eq!(
        u64::from(GROUP_AGENT_SCHEDULED_NODE_REQUEST_VERSION),
        spec_uint("versions", "request.v"),
    );
    assert_eq!(
        u64::from(GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION),
        spec_uint("versions", "scheduler_protocol_version"),
    );
    assert_eq!(
        u64::from(GROUP_AGENT_SCHEDULED_NODE_EXECUTION_PROTOCOL_VERSION),
        spec_uint("versions", "node_execution_protocol_version"),
    );
}

#[test]
fn spec_digest_domains_match_rust_constants() {
    assert_eq!(
        GROUP_AGENT_SCHEDULED_NODE_CONTRACT_DIGEST_DOMAIN,
        spec_domain("contract_digest_domain").as_slice(),
    );
    assert_eq!(
        GROUP_AGENT_SCHEDULED_NODE_REQUEST_DIGEST_DOMAIN,
        spec_domain("request_digest_domain").as_slice(),
    );
}

#[test]
fn spec_bounds_match_rust_constants() {
    assert_eq!(
        MAX_GROUP_AGENT_SCHEDULED_NODE_CONTRACT_BYTES as u64,
        spec_uint("bounds", "contract_bytes.max"),
    );
    assert_eq!(
        MAX_GROUP_AGENT_SCHEDULED_NODE_PREDECESSOR_OUTPUT_BYTES as u64,
        spec_uint("bounds", "predecessor_output_bytes.max"),
    );
}
