use forge_runtime_domain::{
    BeginGroupAgentGraphRun, GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
    GROUP_AGENT_GRAPH_RUN_EVENT_DIGEST_DOMAIN, GROUP_AGENT_GRAPH_RUN_VERSION,
    GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION, GroupAgentGraphCorePlan, GroupAgentGraphEdge,
    GroupAgentGraphRunEvent, GroupAgentGraphRunEventKind, GroupAgentGraphRunInspection,
    GroupAgentGraphRunRecord, GroupAgentGraphRunStatus,
};
use serde::Deserialize;

const MANIFEST_SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
const SOURCE_SHA: &str = "abcdef0123456789abcdef0123456789abcdef0123456789abcdef0123456789";
const PLAN_SHA: &str = "e286b16586904bd82bd38c63e453843d36ec6c39cc2fbc139e877f53ba56d0d3";
const EVENT_SHA: &str = "19aff5454dd7594e9019f2d7ab9e7fe9644256aa2df159c0ec5e65963a96e5ca";

#[derive(Deserialize)]
struct GoldenFixture {
    v: u16,
    expected_plan: GroupAgentGraphCorePlan,
    expected_canonical_payload_json: String,
    expected_canonical_plan_json: String,
}

#[test]
fn rust_consumes_the_exact_go_core_plan_golden() {
    let fixture: GoldenFixture = serde_json::from_str(include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/../../../docs/contracts/fixtures/group-agent-graph-core-plan-v1.json"
    )))
    .expect("shared Go fixture");
    let plan = fixture.expected_plan;

    assert_eq!(fixture.v, GROUP_AGENT_GRAPH_CORE_PLAN_VERSION);
    assert_eq!(plan.plan_sha256, PLAN_SHA);
    assert_eq!(plan.expected_sha256().expect("plan digest"), PLAN_SHA);
    assert_eq!(
        plan.canonical_json().expect("canonical plan"),
        fixture.expected_canonical_plan_json
    );
    assert_eq!(
        payload_from_plan(&plan),
        fixture.expected_canonical_payload_json
    );
    plan.validate().expect("valid Go plan");
}

#[test]
fn canonical_plan_keeps_field_order_and_does_not_html_escape() {
    let mut plan = plan();
    plan.graph_id = "graph<fixture&v1".into();
    plan.plan_sha256 = plan.expected_sha256().expect("rehash plan");
    let encoded = plan.canonical_json().expect("canonical plan");

    assert!(encoded.starts_with(
        "{\"v\":1,\"scheduler_protocol_version\":1,\"graph_version\":1,\"graph_id\":"
    ));
    assert!(encoded.contains("\"graph_id\":\"graph<fixture&v1\""));
    assert!(!encoded.ends_with('\n'));
    plan.validate().expect("valid unescaped plan");
}

#[test]
fn plan_validation_rejects_authority_and_topology_drift() {
    let mut authority = plan();
    authority.dispatch_authority_released = true;
    authority.plan_sha256 = authority.expected_sha256().expect("rehash");
    assert!(authority.validate().is_err());

    let mut edge_order = plan();
    edge_order.edges.reverse();
    edge_order.plan_sha256 = edge_order.expected_sha256().expect("rehash");
    assert!(edge_order.validate().is_err());

    let mut node_order = plan();
    node_order.authored_node_ids.swap(0, 1);
    node_order.plan_sha256 = node_order.expected_sha256().expect("rehash");
    assert!(node_order.validate().is_err());

    let mut waves = plan();
    waves.waves[0].swap(0, 1);
    waves.plan_sha256 = waves.expected_sha256().expect("rehash");
    assert!(waves.validate().is_err());
}

#[test]
fn plan_validation_rejects_bad_digest_cycle_and_unknown_json_fields() {
    let mut digest = plan();
    digest.plan_sha256 = "0".repeat(64);
    assert!(digest.validate().is_err());

    let mut cycle = plan();
    cycle.edges.push(edge("sso", "frontend"));
    cycle.edges.sort();
    cycle.plan_sha256 = cycle.expected_sha256().expect("rehash");
    assert!(cycle.validate().is_err());

    let encoded = plan().canonical_json().expect("canonical plan");
    let with_unknown = encoded.replacen("\"v\":1", "\"v\":1,\"unknown\":true", 1);
    assert!(serde_json::from_str::<GroupAgentGraphCorePlan>(&with_unknown).is_err());
}

#[test]
fn prepared_event_has_exact_canonical_bytes_and_domain_digest() {
    let event = event(&plan());
    let expected = concat!(
        "{\"v\":1,\"graph_run_id\":\"graph-run-1\",\"seq\":1,",
        "\"type\":\"graph_run_prepared\",\"graph_id\":\"graph-fixture-v1\",",
        "\"graph_manifest_sha256\":\"0123456789abcdef0123456789abcdef",
        "0123456789abcdef0123456789abcdef\",\"plan_sha256\":\"e286b16586904bd8",
        "2bd38c63e453843d36ec6c39cc2fbc139e877f53ba56d0d3\",",
        "\"scheduler_protocol_version\":1,\"prepared_at_ms\":73}"
    );

    assert_eq!(
        GROUP_AGENT_GRAPH_RUN_EVENT_DIGEST_DOMAIN,
        b"forge.group-agent-graph-run-event.v1\0"
    );
    assert_eq!(event.canonical_json().expect("canonical event"), expected);
    assert_eq!(event.expected_sha256().expect("event digest"), EVENT_SHA);
    assert_eq!(
        serde_json::from_str::<GroupAgentGraphRunEvent>(expected).expect("decode event"),
        event
    );
    event.validate().expect("valid event");
}

#[test]
fn event_decoder_is_strict_without_flatten_tag_ambiguity() {
    let encoded = event(&plan()).canonical_json().expect("canonical event");
    let unknown = encoded.replacen("\"seq\":1", "\"seq\":1,\"unknown\":true", 1);
    let duplicate = encoded.replacen("\"seq\":1", "\"seq\":1,\"seq\":1", 1);

    assert!(serde_json::from_str::<GroupAgentGraphRunEvent>(&unknown).is_err());
    assert!(serde_json::from_str::<GroupAgentGraphRunEvent>(&duplicate).is_err());
    assert_eq!(
        serde_json::from_str::<GroupAgentGraphRunEvent>(&encoded).expect("round trip"),
        event(&plan())
    );
}

#[test]
fn begin_request_requires_exact_plan_event_and_cross_bindings() {
    let request = begin_request();
    request.validate().expect("valid begin request");

    let mut plan_whitespace = request.clone();
    plan_whitespace.plan_json.push('\n');
    assert!(plan_whitespace.validate().is_err());

    let mut event_whitespace = request.clone();
    event_whitespace.event_json.push(' ');
    assert!(event_whitespace.validate().is_err());

    let mut wrong_time = request;
    let GroupAgentGraphRunEventKind::GraphRunPrepared { prepared_at_ms, .. } =
        &mut wrong_time.event.kind
    else {
        panic!("prepared fixture event");
    };
    *prepared_at_ms += 1;
    wrong_time.event_json = wrong_time.event.canonical_json().expect("canonical event");
    assert!(wrong_time.validate().is_err());
}

#[test]
fn inspection_revalidates_record_plan_event_bytes_and_bindings() {
    let inspection = inspection();
    inspection.validate().expect("valid inspection");

    let mut wrong_count = inspection.clone();
    wrong_count.run.node_count -= 1;
    assert!(wrong_count.validate().is_err());

    let mut wrong_graph = inspection.clone();
    wrong_graph.run.graph_id = "other-graph".into();
    assert!(wrong_graph.validate().is_err());

    let mut wrong_event = inspection;
    wrong_event.events[0].seq = 2;
    wrong_event.event_jsons[0] = wrong_event.events[0]
        .canonical_json()
        .expect("canonical event");
    wrong_event.run.journal_bytes = wrong_event.event_jsons[0].len();
    assert!(wrong_event.validate().is_err());
}

fn payload_from_plan(plan: &GroupAgentGraphCorePlan) -> String {
    let encoded = plan.canonical_json().expect("canonical plan");
    let suffix = format!(",\"plan_sha256\":\"{}\"}}", plan.plan_sha256);
    format!(
        "{}}}",
        encoded
            .strip_suffix(&suffix)
            .expect("plan digest is final field")
    )
}

fn plan() -> GroupAgentGraphCorePlan {
    GroupAgentGraphCorePlan {
        v: GROUP_AGENT_GRAPH_CORE_PLAN_VERSION,
        scheduler_protocol_version: GROUP_AGENT_GRAPH_SCHEDULER_PROTOCOL_VERSION,
        graph_version: 1,
        graph_id: "graph-fixture-v1".into(),
        graph_manifest_sha256: MANIFEST_SHA.into(),
        authored_node_ids: vec!["frontend".into(), "backend".into(), "sso".into()],
        edges: vec![edge("backend", "sso"), edge("frontend", "sso")],
        waves: vec![
            vec!["frontend".into(), "backend".into()],
            vec!["sso".into()],
        ],
        execution_contract_present: false,
        dispatch_authority_released: false,
        plan_sha256: PLAN_SHA.into(),
    }
}

fn edge(from: &str, to: &str) -> GroupAgentGraphEdge {
    GroupAgentGraphEdge {
        from_node_id: from.into(),
        to_node_id: to.into(),
    }
}

fn event(plan: &GroupAgentGraphCorePlan) -> GroupAgentGraphRunEvent {
    GroupAgentGraphRunEvent {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: "graph-run-1".into(),
        seq: 1,
        kind: GroupAgentGraphRunEventKind::GraphRunPrepared {
            graph_id: plan.graph_id.clone(),
            graph_manifest_sha256: plan.graph_manifest_sha256.clone(),
            plan_sha256: plan.plan_sha256.clone(),
            scheduler_protocol_version: plan.scheduler_protocol_version,
            prepared_at_ms: 73,
        },
    }
}

fn begin_request() -> BeginGroupAgentGraphRun {
    let plan = plan();
    let event = event(&plan);
    BeginGroupAgentGraphRun {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        graph_run_id: event.graph_run_id.clone(),
        graph_id: plan.graph_id.clone(),
        source_snapshot_sha256: SOURCE_SHA.into(),
        graph_manifest_sha256: plan.graph_manifest_sha256.clone(),
        plan_json: plan.canonical_json().expect("canonical plan"),
        event_json: event.canonical_json().expect("canonical event"),
        plan,
        event,
        idempotency_key: "graph-run-key".into(),
        created_at_ms: 73,
    }
}

fn inspection() -> GroupAgentGraphRunInspection {
    let request = begin_request();
    GroupAgentGraphRunInspection {
        v: GROUP_AGENT_GRAPH_RUN_VERSION,
        run: GroupAgentGraphRunRecord {
            v: GROUP_AGENT_GRAPH_RUN_VERSION,
            graph_run_id: request.graph_run_id,
            graph_id: request.graph_id,
            status: GroupAgentGraphRunStatus::AwaitingExecutionContract,
            source_snapshot_sha256: request.source_snapshot_sha256,
            graph_manifest_sha256: request.graph_manifest_sha256,
            scheduler_protocol_version: request.plan.scheduler_protocol_version,
            plan_sha256: request.plan.plan_sha256.clone(),
            plan_bytes: request.plan_json.len(),
            node_count: request.plan.authored_node_ids.len(),
            wave_count: request.plan.waves.len(),
            execution_contract_present: false,
            dispatch_authority_released: false,
            last_event_seq: 1,
            journal_bytes: request.event_json.len(),
            created_at_ms: request.created_at_ms,
        },
        plan_json: request.plan_json,
        plan: request.plan,
        event_jsons: vec![request.event_json],
        events: vec![request.event],
    }
}
