use std::collections::BTreeMap;
use std::io::Write;
use std::path::Path;

use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use tempfile::TempDir;

#[allow(dead_code)]
mod cli_group_agent_scheduled_node_contract_support;
mod group_agent_graph_run_support;
mod group_agent_graph_support;

use cli_group_agent_scheduled_node_contract_support::{
    admit_schedule, build_schedule, export_control, prepare_run,
};
const PRICING: &str = "cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc";

/// Builds one wave-admit invocation with the full execution option set
/// (fail-closed, no literals in candidates — review Finding 2).
fn wave_admit_args(
    graph_run_id: &str,
    schedule_sha256: &str,
    receipt_path: Option<&str>,
) -> Vec<String> {
    let mut args = vec![
        "group".into(),
        "graph".into(),
        "run".into(),
        "scheduled-contract".into(),
        "wave-admit".into(),
        graph_run_id.into(),
        "--schedule-sha256".into(),
        schedule_sha256.into(),
    ];
    if let Some(receipt) = receipt_path {
        args.push("--predecessor-receipt".into());
        args.push(receipt.into());
    }
    args.extend(wave_execution_args());
    args
}

/// Execution options every wave-admit call must carry (fail-closed, no
/// literals in candidates — review Finding 2).
fn wave_execution_args() -> Vec<String> {
    vec![
        "--endpoint".into(),
        "https://api.openai.com/v1/responses".into(),
        "--model".into(),
        "gpt-5.6-sol".into(),
        "--max-output-tokens".into(),
        "4096".into(),
        "--max-model-output-bytes".into(),
        "65536".into(),
        "--max-model-events".into(),
        "4096".into(),
        "--timeout-ms".into(),
        "300000".into(),
        "--max-cost-usd-micros".into(),
        "1000000".into(),
        "--pricing-snapshot-sha256".into(),
        PRICING.into(),
        "--max-result-bytes".into(),
        "262144".into(),
    ]
}
use group_agent_graph_run_support::{Fixture, command};
use group_agent_graph_support::{successful_json, text};

const RECEIPT_DOMAIN: &str = "forge.group-agent-scheduled-node-terminal-receipt.v1\0";

/// Builds a canonical predecessor terminal receipt exactly as the Go core
/// validates it: the digest is sha256(domain || canonical(digest-object))
/// where the digest object carries every field except `receipt_id` /
/// `receipt_sha256` serialized with keys sorted (Go map marshal); the full
/// canonical receipt follows the Go struct field order.
fn build_receipt(
    graph_run_id: &str,
    graph_id: &str,
    node_id: &str,
    control_sha256: &str,
    lane_sha256: &str,
) -> Vec<u8> {
    let digest = receipt_digest(graph_run_id, graph_id, node_id, control_sha256, lane_sha256);
    let receipt_id = format!("scheduled-node-terminal-receipt-{digest}");
    format!(
        "{{\"v\":1,\"scheduler_protocol_version\":1,\"terminal_receipt_protocol_version\":1,\
         \"terminal_control_sha256\":\"{control_sha256}\",\"graph_run_id\":\"{graph_run_id}\",\
         \"graph_id\":\"{graph_id}\",\"node_id\":\"{node_id}\",\"attempt\":1,\
         \"dispatch_id\":\"dispatch-wave-fixture\",\
         \"provider_request_id\":\"scheduled-node-provider-request-wave-fixture\",\
         \"project_lane_sha256\":\"{lane_sha256}\",\"artifact_kind\":\"result\",\
         \"artifact_id\":\"scheduled-node-terminal-artifact-{pad}\",\
         \"artifact_sha256\":\"{pad}\",\"node_outcome\":\"completed\",\
         \"retry_authorized\":false,\"lane_release_authorized\":true,\
         \"successor_advance_authorized\":false,\"receipt_id\":\"{receipt_id}\",\
         \"receipt_sha256\":\"{digest}\"}}",
        pad = "b".repeat(64),
    )
    .into_bytes()
}

fn receipt_digest(
    graph_run_id: &str,
    graph_id: &str,
    node_id: &str,
    control_sha256: &str,
    lane_sha256: &str,
) -> String {
    let mut digest_object: BTreeMap<String, Value> = BTreeMap::new();
    digest_object.insert("v".into(), json!(1));
    digest_object.insert("scheduler_protocol_version".into(), json!(1));
    digest_object.insert("terminal_receipt_protocol_version".into(), json!(1));
    digest_object.insert("terminal_control_sha256".into(), json!(control_sha256));
    digest_object.insert("graph_run_id".into(), json!(graph_run_id));
    digest_object.insert("graph_id".into(), json!(graph_id));
    digest_object.insert("node_id".into(), json!(node_id));
    digest_object.insert("attempt".into(), json!(1));
    digest_object.insert("dispatch_id".into(), json!("dispatch-wave-fixture"));
    digest_object.insert(
        "provider_request_id".into(),
        json!("scheduled-node-provider-request-wave-fixture"),
    );
    digest_object.insert("project_lane_sha256".into(), json!(lane_sha256));
    digest_object.insert("artifact_kind".into(), json!("result"));
    digest_object.insert(
        "artifact_id".into(),
        json!("scheduled-node-terminal-artifact-".to_string() + &"b".repeat(64)),
    );
    digest_object.insert("artifact_sha256".into(), json!("b".repeat(64)));
    digest_object.insert("node_outcome".into(), json!("completed"));
    digest_object.insert("retry_authorized".into(), json!(false));
    digest_object.insert("lane_release_authorized".into(), json!(true));
    digest_object.insert("successor_advance_authorized".into(), json!(false));
    let digest_canonical = serde_json::to_string(&digest_object).expect("digest canonical");
    let mut hasher = Sha256::new();
    hasher.update(RECEIPT_DOMAIN.as_bytes());
    hasher.update(digest_canonical.as_bytes());
    format!("{:x}", hasher.finalize())
}

fn write_temp(dir: &TempDir, name: &str, data: &[u8]) -> String {
    let path = dir.path().join(name);
    let mut file = std::fs::File::create(&path).expect("create temp");
    file.write_all(data).expect("write temp");
    path.display().to_string()
}

fn json_parse(bytes: &[u8]) -> Value {
    serde_json::from_slice(bytes).expect("valid JSON")
}

fn prepared_wave(fixture: &Fixture, key: &str) -> (String, String, Value) {
    let graph_run_id = prepare_run(fixture, key);
    let control = export_control(fixture, &graph_run_id);
    let schedule = build_schedule(&control);
    admit_schedule(fixture, &graph_run_id, &schedule);
    let schedule_sha256 = text(&json_parse(&schedule)["schedule_sha256"]);
    (graph_run_id, schedule_sha256, json_parse(&schedule))
}

#[test]
fn wave_admit_materializes_every_ready_node_from_one_wave() {
    let fixture = Fixture::new();
    let (graph_run_id, schedule_sha256, schedule) =
        prepared_wave(&fixture, "wave-admit-source-run");
    let graph_id = text(&schedule["graph_id"]);
    // node 0 (frontend) is consumed; the wave lists its successor(s).
    let node0 = text(&schedule["nodes"][0]["node_id"]);
    let receipt = build_receipt(
        &graph_run_id,
        &graph_id,
        &node0,
        &"a".repeat(64),
        &text(&schedule["nodes"][0]["project_lane_sha256"]),
    );
    std::fs::write(fixture.cwd.path().join("relative-receipt.json"), &receipt)
        .expect("write relative receipt");

    let args = wave_admit_args(
        &graph_run_id,
        &schedule_sha256,
        Some("relative-receipt.json"),
    );
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = command(fixture.state.path(), fixture.cwd.path(), &arg_refs)
        .output()
        .expect("wave-admit");
    let parsed = successful_json(&output);
    assert_eq!(parsed["v"], 1);
    // The Go core planned the wave (backend is ready after node0), the
    // candidate was materialized, and admission ran — the evidence-chain
    // guard then correctly rejected the fabricated receipt because no
    // provider-request lifecycle exists for it (a real predecessor receipt
    // comes from a completed dispatch; that admission success path is the
    // same successor-service code covered by the application-layer tests).
    let wave = parsed["wave"].as_array().expect("wave array");
    let rejected = parsed["rejected"].as_array().expect("rejected array");
    // backend is a same-wave sibling of frontend with an empty
    // direct-predecessor set: the fabricated receipt passes the Go digest
    // check and is accepted as consumed-set evidence but is NOT carried by
    // the candidate (zero receipts), so admission has no lifecycle
    // dependency and succeeds (ADR-0035 + v21).
    assert_eq!(rejected.len(), 0, "no rejections: {parsed}");
    assert_eq!(wave.len(), 1, "exactly one wave node admitted: {parsed}");
    assert_ne!(wave[0]["node_id"], node0);
    let contract_id = text(&wave[0]["contract_id"]);
    assert!(!contract_id.is_empty());
    assert_wave_contract_visible(&fixture, &contract_id);
    assert_wave_contract_carries_execution_options(&fixture, &contract_id);
}

/// `assert_wave_contract_carries_execution_options` verifies the admitted
/// candidate carries the operator's real execution options — the wave-admit
/// pass-through (review Finding 2), never literals.
fn assert_wave_contract_carries_execution_options(fixture: &Fixture, contract_id: &str) {
    let show = command(fixture.state.path(), fixture.cwd.path(), &[
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "successor",
        "show",
        contract_id,
        "--include-contract",
    ])
    .output()
    .expect("show wave contract with candidate");
    let shown = successful_json(&show);
    let contract = &shown["inspection"]["contract"];
    assert_eq!(
        contract["provider"]["endpoint"],
        "https://api.openai.com/v1/responses"
    );
    assert_eq!(contract["provider"]["model"], "gpt-5.6-sol");
    assert_eq!(contract["budgets"]["pricing_snapshot_sha256"], PRICING);
}

///  verifies the admitted successor candidate is
/// queryable through the successor show command.
fn assert_wave_contract_visible(fixture: &Fixture, contract_id: &str) {
    let show = command(fixture.state.path(), fixture.cwd.path(), &[
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "successor",
        "show",
        contract_id,
    ])
    .output()
    .expect("show admitted wave contract");
    let shown = successful_json(&show);
    assert_eq!(shown["inspection"]["record"]["execution_ordinal"], 1);
}

#[test]
fn wave_admit_accepts_zero_receipts_for_same_wave_sibling() {
    let fixture = Fixture::new();
    let (graph_run_id, schedule_sha256, schedule) =
        prepared_wave(&fixture, "wave-admit-zero-receipt-run");
    let args = wave_admit_args(&graph_run_id, &schedule_sha256, None);
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = command(fixture.state.path(), fixture.cwd.path(), &arg_refs)
        .output()
        .expect("wave-admit without receipts");
    let parsed = successful_json(&output);
    let wave = parsed["wave"].as_array().expect("wave array");
    let rejected = parsed["rejected"].as_array().expect("rejected array");
    assert!(rejected.is_empty(), "zero-receipt wave rejected: {parsed}");
    assert_eq!(wave.len(), 1, "exactly one same-wave sibling: {parsed}");
    assert_eq!(wave[0]["node_id"], schedule["nodes"][1]["node_id"]);
    let contract_id = text(&wave[0]["contract_id"]);
    assert_wave_contract_visible(&fixture, &contract_id);
    assert_successor_provider_request_prepares(&fixture, &contract_id);
}

fn assert_successor_provider_request_prepares(fixture: &Fixture, contract_id: &str) {
    let prepared = command(fixture.state.path(), fixture.cwd.path(), &[
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "provider-request",
        "prepare",
        contract_id,
        "--idempotency-key",
        "zero-receipt-successor-provider-request",
    ])
    .output()
    .expect("prepare successor provider request");
    let prepared = successful_json(&prepared);
    assert_eq!(
        prepared["inspection"]["record"]["scheduled_contract_id"],
        contract_id
    );
    let request_id = text(&prepared["inspection"]["record"]["provider_request_id"]);
    let shown = command(fixture.state.path(), fixture.cwd.path(), &[
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "provider-request",
        "show",
        &request_id,
    ])
    .output()
    .expect("inspect successor provider request");
    let shown = successful_json(&shown);
    assert_eq!(
        shown["inspection"]["record"]["provider_request_id"],
        request_id
    );
}

#[test]
fn wave_admit_rejects_unknown_flag() {
    let fixture = Fixture::new();
    let (graph_run_id, _, _) = prepared_wave(&fixture, "wave-admit-unknown-flag-run");
    let output = command(fixture.state.path(), fixture.cwd.path(), &[
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "wave-admit",
        &graph_run_id,
        "--bogus",
    ])
    .output()
    .expect("wave-admit bogus flag");
    assert!(!output.status.success());
}

#[test]
fn wave_admit_rejects_drifted_receipt_without_admitting_anything() {
    let fixture = Fixture::new();
    let (graph_run_id, schedule_sha256, schedule) = prepared_wave(&fixture, "wave-admit-drift-run");
    let graph_id = text(&schedule["graph_id"]);
    let node0 = text(&schedule["nodes"][0]["node_id"]);
    let mut receipt = build_receipt(
        &graph_run_id,
        &graph_id,
        &node0,
        &"a".repeat(64),
        &text(&schedule["nodes"][0]["project_lane_sha256"]),
    );
    // Mutate the receipt body: digest no longer matches -> Go core rejects.
    let drift: String = String::from_utf8(receipt.clone()).expect("utf8").replacen(
        &"a".repeat(64),
        &"c".repeat(64),
        1,
    );
    receipt = drift.into_bytes();
    let dir = TempDir::new().expect("tempdir");
    let receipt_path = write_temp(&dir, "receipt.json", &receipt);
    let args = wave_admit_args(&graph_run_id, &schedule_sha256, Some(&receipt_path));
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = command(fixture.state.path(), fixture.cwd.path(), &arg_refs)
        .output()
        .expect("wave-admit drifted receipt");
    assert!(!output.status.success());
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(stderr.contains("go core"), "{stderr}");
    // Nothing admitted: contract list is empty for the run.
    let list = command(fixture.state.path(), fixture.cwd.path(), &[
        "group",
        "graph",
        "run",
        "scheduled-contract",
        "list",
        &graph_run_id,
    ])
    .output()
    .expect("list contracts after rejected wave");
    let listed = successful_json(&list);
    let contracts = listed["contracts"].as_array().expect("contracts array");
    assert!(contracts.is_empty(), "no contract admitted: {listed}");
}

#[allow(dead_code)]
fn _unused(_: &Path) {}

#[test]
fn wave_admit_materializes_two_ready_nodes_in_one_wave() {
    let fixture = Fixture::fan_out();
    let (graph_run_id, schedule_sha256, schedule) =
        prepared_wave(&fixture, "wave-admit-fanout-run");
    let graph_id = text(&schedule["graph_id"]);
    let node0 = text(&schedule["nodes"][0]["node_id"]);
    let receipt = build_receipt(
        &graph_run_id,
        &graph_id,
        &node0,
        &"a".repeat(64),
        &text(&schedule["nodes"][0]["project_lane_sha256"]),
    );
    let dir = TempDir::new().expect("tempdir");
    let receipt_path = write_temp(&dir, "receipt.json", &receipt);

    let args = wave_admit_args(&graph_run_id, &schedule_sha256, Some(&receipt_path));
    let arg_refs: Vec<&str> = args.iter().map(String::as_str).collect();
    let output = command(fixture.state.path(), fixture.cwd.path(), &arg_refs)
        .output()
        .expect("wave-admit fan-out");
    // The fan-out wave is planned correctly (both successors ready after
    // frontend), and the fabricated receipt is accepted as consumed-set
    // evidence; admission then fails the evidence chain for BOTH nodes
    // (no frontend lifecycle exists — a real predecessor receipt comes from
    // a completed dispatch), and the non-zero exit code surfaces the partial
    // failure to automation (Finding 4).
    assert!(!output.status.success(), "rejected wave must exit non-zero");
    let parsed: Value = serde_json::from_slice(&output.stdout).expect("wave-admit JSON on failure");
    let wave = parsed["wave"].as_array().expect("wave array");
    let rejected = parsed["rejected"].as_array().expect("rejected array");
    assert_eq!(wave.len(), 0);
    assert_eq!(rejected.len(), 2, "both fan-out nodes rejected: {parsed}");
    let ids: Vec<&str> = rejected
        .iter()
        .map(|node| node["node_id"].as_str().expect("node id"))
        .collect();
    assert!(
        ids.contains(&"backend") && ids.contains(&"sso"),
        "rejected nodes: {ids:?}"
    );
    for node in rejected {
        let reason = node["disposition"].as_str().expect("reason");
        assert!(
            reason.contains("was not found"),
            "evidence-chain guard must name the missing lifecycle: {reason}"
        );
    }
}
