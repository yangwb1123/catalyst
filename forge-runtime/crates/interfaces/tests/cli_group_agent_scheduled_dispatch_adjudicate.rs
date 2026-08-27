#![cfg(target_os = "linux")]

use std::{
    fs::{self, OpenOptions},
    io::{ErrorKind, Write},
    net::TcpListener,
    os::unix::fs::{OpenOptionsExt, PermissionsExt},
    path::{Path, PathBuf},
    process::{Command, Output},
    time::{SystemTime, UNIX_EPOCH},
};

use forge_runtime_domain::{
    ClaimGroupAgentScheduledNodeDispatch, ClaimGroupAgentScheduledNodeDispatchResult,
    GROUP_AGENT_SCHEDULED_NODE_ACTIVE_LANE_VERSION, GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION,
    GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION, GroupAgentNodePricingSnapshot,
    GroupAgentScheduledNodeActiveLane, GroupAgentScheduledNodeDispatchAuthorization,
    GroupAgentScheduledNodeDispatchClaim, GroupAgentScheduledNodeDispatchClaimEvent,
    GroupAgentScheduledNodeDispatchReleaseControl, GroupAgentScheduledNodeLifecycleStore,
};
use forge_runtime_infrastructure::SqliteHubStore;
use rusqlite::Connection;
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};

#[allow(dead_code, clippy::duplicate_mod)]
mod cli_group_agent_scheduled_node_contract_support;
#[allow(dead_code, clippy::duplicate_mod)]
mod cli_group_agent_scheduled_node_provider_request_support;
#[allow(dead_code, clippy::duplicate_mod)]
mod group_agent_graph_run_support;
#[allow(dead_code, clippy::duplicate_mod)]
mod group_agent_graph_support;
#[allow(dead_code, clippy::duplicate_mod)]
mod scheduled_graph_reconcile_cli_support;

use cli_group_agent_scheduled_node_contract_support::{
    admit_candidate, admit_schedule, build_candidate, build_schedule, export_control, json,
    prepare_run,
};
use cli_group_agent_scheduled_node_provider_request_support::{
    authorize_scheduled_with_core, export_scheduled_release_control, prepare_provider_request,
};
use group_agent_graph_run_support::{Fixture as GraphFixture, command};
use group_agent_graph_support::text;
use scheduled_graph_reconcile_cli_support::shared_core;

const PRICING: &str = "48f3531a7d71015453dc27a71bd0f17efbaf68ddfcff04461bd5d01b52cade8d";
const MODEL: &str = "private-scheduled-contract-model";
const OWNER: &str = "public-cli-dead-scheduled-owner";
const CREDENTIAL_POISON: &str =
    "adjudication-must-not-read-this-credential\r\nx-private-header: rejected";
const OWNER_PATH_DOMAIN: &[u8] = b"forge.scheduled-executor-sidecar-owner.v1\0";

#[test]
fn public_adjudication_reports_no_dispatch_and_one_database_write() {
    let fixture = AdjudicationFixture::new();
    fixture.assert_claimed_without_terminal_evidence();
    let (listener, sentinel) = loopback_sentinel();

    let output = fixture.adjudicate(&sentinel);
    assert!(
        output.status.success(),
        "adjudication failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    assert!(output.stderr.is_empty());
    assert_adjudication_output(&output, &fixture.provider_request_id);
    fixture.assert_adjudicated_without_terminal_evidence();
    assert!(
        !fixture.owner_path.exists(),
        "owner sidecar was not removed"
    );
    fixture.graph.assert_workspace_unchanged();
    assert_no_local_send(&listener);
}

struct AdjudicationFixture {
    graph: GraphFixture,
    provider_request_id: String,
    owner_path: PathBuf,
}

impl AdjudicationFixture {
    fn new() -> Self {
        let graph = GraphFixture::new();
        let artifacts = prepare_claim_artifacts(&graph);
        let request = claim_request(&artifacts);
        let database = graph.state.path().join("hub.sqlite3");
        let store = SqliteHubStore::open(&database).expect("open current Hub");
        let result = store
            .claim_group_agent_scheduled_node_dispatch(&request)
            .expect("commit exact scheduled claim");
        assert!(matches!(
            result,
            ClaimGroupAgentScheduledNodeDispatchResult::Claimed { .. }
        ));
        let owner_path = write_dead_owner(graph.state.path(), &artifacts.request_id, OWNER);
        Self {
            graph,
            provider_request_id: artifacts.request_id,
            owner_path,
        }
    }

    fn adjudicate(&self, sentinel: &str) -> Output {
        command(
            self.graph.state.path(),
            self.graph.cwd.path(),
            &[
                "group",
                "graph",
                "run",
                "scheduled-contract",
                "provider-request",
                "dispatch",
                "adjudicate",
                &self.provider_request_id,
            ],
        )
        .env("OPENAI_API_KEY", CREDENTIAL_POISON)
        .env("OPENAI_BASE_URL", sentinel)
        .env("HTTP_PROXY", sentinel)
        .env("HTTPS_PROXY", sentinel)
        .env("ALL_PROXY", sentinel)
        .env("NO_PROXY", "")
        .output()
        .expect("run public scheduled adjudication CLI")
    }

    fn assert_claimed_without_terminal_evidence(&self) {
        let (status, lane_active, adjudicated_at, terminal_evidence) = self.lifecycle_state();
        assert_eq!(status, "claimed");
        assert_eq!(lane_active, 1);
        assert_eq!(adjudicated_at, None);
        assert_eq!(terminal_evidence, 0);
        assert!(self.owner_path.is_file());
    }

    fn assert_adjudicated_without_terminal_evidence(&self) {
        let (status, lane_active, adjudicated_at, terminal_evidence) = self.lifecycle_state();
        assert_eq!(status, "adjudicated");
        assert_eq!(lane_active, 0);
        assert!(adjudicated_at.is_some());
        assert_eq!(terminal_evidence, 0);
    }

    fn lifecycle_state(&self) -> (String, i64, Option<i64>, i64) {
        Connection::open(self.graph.state.path().join("hub.sqlite3"))
            .expect("open Hub inspection")
            .query_row(
                "SELECT status,lane_active,adjudicated_at_ms,\
                 artifact_json IS NOT NULL OR terminal_control_json IS NOT NULL OR \
                 terminal_receipt_json IS NOT NULL FROM \
                 group_agent_graph_scheduled_node_dispatch_lifecycles \
                 WHERE provider_request_id=?1",
                [&self.provider_request_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .expect("inspect scheduled lifecycle row")
    }
}

struct ClaimArtifacts {
    request_id: String,
    release_json: String,
    authorization_json: String,
    pricing_json: String,
}

fn prepare_claim_artifacts(graph: &GraphFixture) -> ClaimArtifacts {
    let graph_run_id = prepare_run(graph, "public-adjudication-source-run");
    let control = export_control(graph, &graph_run_id);
    let schedule = build_schedule(&control);
    admit_schedule(graph, &graph_run_id, &schedule);
    let schedule_sha256 = text(&json(&schedule)["schedule_sha256"]);
    let candidate = build_candidate(&control, &schedule_sha256);
    let admitted = admit_candidate(
        graph,
        &graph_run_id,
        "public-adjudication-candidate",
        &candidate,
    );
    let contract_id = text(&admitted["inspection"]["record"]["contract_id"]);
    let request_id = prepare_provider_request(graph, &contract_id, "public-adjudication-request");
    let release = export_scheduled_release_control(graph, &request_id);
    let authorization = authorize_scheduled_with_core(&release);
    ClaimArtifacts {
        request_id,
        release_json: String::from_utf8(release).expect("release control UTF-8"),
        authorization_json: String::from_utf8(authorization).expect("authorization UTF-8"),
        pricing_json: build_pricing(),
    }
}

fn build_pricing() -> String {
    let core = shared_core();
    let output = Command::new(&core.path)
        .args([
            "graph-node-pricing-snapshot",
            "--model",
            MODEL,
            "--input-usd-micros-per-token-unit",
            "1000000",
            "--output-usd-micros-per-token-unit",
            "976561523",
            "--max-input-tokens",
            "1",
        ])
        .env_clear()
        .output()
        .expect("build exact pricing with pinned Core");
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("pricing JSON");
    assert_eq!(value["pricing_snapshot_sha256"], PRICING);
    String::from_utf8(output.stdout).expect("pricing UTF-8")
}

fn claim_request(artifacts: &ClaimArtifacts) -> ClaimGroupAgentScheduledNodeDispatch {
    let release =
        GroupAgentScheduledNodeDispatchReleaseControl::decode_exact(&artifacts.release_json)
            .expect("exact release control");
    let authorization =
        GroupAgentScheduledNodeDispatchAuthorization::decode_exact(&artifacts.authorization_json)
            .expect("exact authorization");
    let pricing = GroupAgentNodePricingSnapshot::decode_exact(&artifacts.pricing_json)
        .expect("exact pricing");
    let mut claim = dispatch_claim(&authorization);
    claim.claim_event_sha256 = unsealed_event(&claim)
        .expected_sha256()
        .expect("claim event digest");
    let active_lane = active_lane(&claim);
    let claim_event = sealed_event(&claim);
    let request = ClaimGroupAgentScheduledNodeDispatch {
        v: GROUP_AGENT_SCHEDULED_NODE_LIFECYCLE_VERSION,
        release_control_json: artifacts.release_json.clone(),
        authorization_json: artifacts.authorization_json.clone(),
        pricing_json: artifacts.pricing_json.clone(),
        provider_request: release.provider_request.clone(),
        provider_request_body: release.provider_request_json.as_bytes().to_vec(),
        claim_json: claim.canonical_json().expect("claim JSON"),
        active_lane_json: active_lane.canonical_json().expect("lane JSON"),
        claim_event_json: claim_event.canonical_json().expect("claim event JSON"),
        release_control: release,
        authorization,
        pricing,
        claim,
        active_lane,
        claim_event,
    };
    request.validate().expect("valid scheduled claim request");
    request
}

fn dispatch_claim(
    authorization: &GroupAgentScheduledNodeDispatchAuthorization,
) -> GroupAgentScheduledNodeDispatchClaim {
    GroupAgentScheduledNodeDispatchClaim {
        v: GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION,
        graph_run_id: authorization.graph_run_id.clone(),
        provider_request_id: authorization.scheduled_provider_request_id.clone(),
        dispatch_id: format!(
            "scheduled-node-dispatch-{}",
            authorization.authorization_sha256
        ),
        authorization_id: authorization.authorization_id.clone(),
        authorization_sha256: authorization.authorization_sha256.clone(),
        provider_request_sha256: authorization.scheduled_provider_request_sha256.clone(),
        request_body_sha256: authorization.request_body_sha256.clone(),
        request_body_bytes: authorization.request_body_bytes,
        pricing_snapshot_sha256: authorization.pricing_snapshot_sha256.clone(),
        node_id: authorization.node_id.clone(),
        attempt: authorization.attempt,
        max_cost_usd_micros: authorization.budgets.max_cost_usd_micros,
        lane_ownership_id: OWNER.into(),
        project_lane_sha256: authorization.project_lane_sha256.clone(),
        expected_last_event_seq: authorization.expected_last_event_seq,
        expected_last_event_sha256: authorization.expected_last_event_sha256.clone(),
        claim_event_sha256: String::new(),
        released_at_ms: unix_time_millis(),
    }
}

fn active_lane(claim: &GroupAgentScheduledNodeDispatchClaim) -> GroupAgentScheduledNodeActiveLane {
    GroupAgentScheduledNodeActiveLane {
        v: GROUP_AGENT_SCHEDULED_NODE_ACTIVE_LANE_VERSION,
        project_lane_sha256: claim.project_lane_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        graph_run_id: claim.graph_run_id.clone(),
        provider_request_id: claim.provider_request_id.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        dispatch_id: claim.dispatch_id.clone(),
        claim_event_sha256: claim.claim_event_sha256.clone(),
        claimed_at_ms: claim.released_at_ms,
    }
}

fn sealed_event(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> GroupAgentScheduledNodeDispatchClaimEvent {
    let mut event = unsealed_event(claim);
    event.event_sha256 = event.expected_sha256().expect("event digest");
    event
}

fn unsealed_event(
    claim: &GroupAgentScheduledNodeDispatchClaim,
) -> GroupAgentScheduledNodeDispatchClaimEvent {
    GroupAgentScheduledNodeDispatchClaimEvent {
        v: GROUP_AGENT_SCHEDULED_NODE_CLAIM_VERSION,
        graph_run_id: claim.graph_run_id.clone(),
        provider_request_id: claim.provider_request_id.clone(),
        dispatch_id: claim.dispatch_id.clone(),
        authorization_id: claim.authorization_id.clone(),
        authorization_sha256: claim.authorization_sha256.clone(),
        provider_request_sha256: claim.provider_request_sha256.clone(),
        project_lane_sha256: claim.project_lane_sha256.clone(),
        node_id: claim.node_id.clone(),
        attempt: claim.attempt,
        expected_last_event_seq: claim.expected_last_event_seq,
        expected_last_event_sha256: claim.expected_last_event_sha256.clone(),
        lane_ownership_id: claim.lane_ownership_id.clone(),
        released_at_ms: claim.released_at_ms,
        event_sha256: String::new(),
    }
}

#[derive(Serialize)]
struct DeadOwnerDocument<'a> {
    v: u16,
    provider_request_id: &'a str,
    lane_ownership_id: &'a str,
    pid: u32,
    linux_machine_id: String,
    linux_boot_id: String,
    linux_pid_namespace_id: String,
    linux_time_namespace_id: String,
    proc_start_ticks: u64,
}

fn write_dead_owner(state: &Path, request_id: &str, owner: &str) -> PathBuf {
    let directory = state.join("scheduled-executor-owners");
    fs::create_dir(&directory).expect("create owner directory");
    fs::set_permissions(&directory, fs::Permissions::from_mode(0o700))
        .expect("set owner directory mode");
    let document = DeadOwnerDocument {
        v: 1,
        provider_request_id: request_id,
        lane_ownership_id: owner,
        pid: u32::MAX,
        linux_machine_id: read_trimmed("/etc/machine-id"),
        linux_boot_id: read_trimmed("/proc/sys/kernel/random/boot_id"),
        linux_pid_namespace_id: fs::read_link("/proc/self/ns/pid")
            .expect("read PID namespace")
            .to_string_lossy()
            .into_owned(),
        linux_time_namespace_id: fs::read_link("/proc/self/ns/time")
            .expect("read time namespace")
            .to_string_lossy()
            .into_owned(),
        proc_start_ticks: 1,
    };
    assert!(!Path::new(&format!("/proc/{}", document.pid)).exists());
    let path = owner_path(&directory, request_id, owner);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .mode(0o600)
        .open(&path)
        .expect("create exact owner sidecar");
    serde_json::to_writer(&mut file, &document).expect("write owner document");
    file.flush().expect("flush owner sidecar");
    file.sync_all().expect("sync owner sidecar");
    path
}

fn owner_path(directory: &Path, request_id: &str, owner: &str) -> PathBuf {
    let mut hasher = Sha256::new();
    hasher.update(OWNER_PATH_DOMAIN);
    hasher.update(request_id.as_bytes());
    hasher.update([0]);
    hasher.update(owner.as_bytes());
    directory.join(format!(
        "scheduled-executor-owner-{:x}.json",
        hasher.finalize()
    ))
}

fn read_trimmed(path: &str) -> String {
    fs::read_to_string(path)
        .expect("read Linux identity")
        .trim()
        .to_owned()
}

fn unix_time_millis() -> u64 {
    let millis = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .expect("system time after Unix epoch")
        .as_millis();
    u64::try_from(millis).expect("time fits u64")
}

fn assert_adjudication_output(output: &Output, request_id: &str) {
    let value: Value = serde_json::from_slice(&output.stdout).expect("adjudication JSON");
    assert_eq!(
        value["type"],
        "group_agent_scheduled_node_dispatch_execution"
    );
    assert_eq!(value["status"], "adjudicated");
    assert_eq!(value["provider_request_id"], request_id);
    assert_eq!(value["lane_active"], false);
    assert_eq!(value["provider_poll_started"], false);
    assert_eq!(value["dispatch_performed_this_invocation"], false);
    assert_eq!(value["database_written_this_invocation"], true);
    assert_eq!(value["metadata_only"], true);
    let rendered = String::from_utf8_lossy(&output.stdout);
    assert!(!rendered.contains(CREDENTIAL_POISON));
}

fn loopback_sentinel() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("bind no-send sentinel");
    listener
        .set_nonblocking(true)
        .expect("nonblocking sentinel");
    let address = listener.local_addr().expect("sentinel address");
    (listener, format!("http://{address}"))
}

fn assert_no_local_send(listener: &TcpListener) {
    let error = listener
        .accept()
        .expect_err("unexpected provider connection");
    assert_eq!(error.kind(), ErrorKind::WouldBlock);
}
