#![cfg(target_os = "linux")]

use std::{
    fs,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Output},
};

use rusqlite::Connection;
use serde_json::Value;
use sha2::{Digest, Sha256};
use tempfile::TempDir;

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
use group_agent_graph_run_support::{Fixture, command};
use group_agent_graph_support::{path_text, successful_json, text};
use scheduled_graph_reconcile_cli_support::shared_core;

const PRICING: &str = "48f3531a7d71015453dc27a71bd0f17efbaf68ddfcff04461bd5d01b52cade8d";
const MODEL: &str = "private-scheduled-contract-model";

#[test]
fn core_refusal_quarantine_reports_cleanup_failure_and_reentry_never_resends() {
    let fixture = ExecuteFixture::new();

    let first = fixture.invoke("offline-test-key");
    let first = successful_json(&first);
    assert_quarantined_invocation(&first, &fixture.provider_request_id);
    assert_durable_quarantine(fixture.graph.state.path(), &fixture.provider_request_id);
    assert!(
        fixture.network_guard_marker.is_file(),
        "network guard did not intercept the provider path"
    );

    let decision_marker = fs::read(&fixture.core.marker).expect("Core refusal marker");
    let network_marker = fs::read(&fixture.network_guard_marker).expect("network guard marker");
    fs::remove_file(&fixture.authorization_path).expect("remove authorization source");
    fs::remove_file(&fixture.pricing_path).expect("remove pricing source");
    fs::remove_file(&fixture.core.path).expect("remove Core source");
    let replay =
        successful_json(&fixture.invoke("reentry-must-not-read-this\r\nx-private: rejected"));
    assert_quarantined_reentry(&replay, &fixture.provider_request_id);
    assert_eq!(
        fs::read(&fixture.core.marker).expect("reread Core refusal marker"),
        decision_marker,
        "re-entry invoked Core or resent the provider request"
    );
    assert_eq!(
        fs::read(&fixture.network_guard_marker).expect("reread network guard marker"),
        network_marker,
        "re-entry attempted a second network operation"
    );
    assert_durable_quarantine(fixture.graph.state.path(), &fixture.provider_request_id);
    fixture.graph.assert_workspace_unchanged();
}

struct ExecuteFixture {
    graph: Fixture,
    provider_request_id: String,
    authorization_path: PathBuf,
    pricing_path: PathBuf,
    core: RefusingCore,
    _network_guard: TempDir,
    network_guard_path: PathBuf,
    network_guard_marker: PathBuf,
}

impl ExecuteFixture {
    fn new() -> Self {
        let graph = Fixture::new();
        let graph_run_id = prepare_run(&graph, "structured-quarantine-source-run");
        let control = export_control(&graph, &graph_run_id);
        let schedule = build_schedule(&control);
        admit_schedule(&graph, &graph_run_id, &schedule);
        let schedule_sha256 = text(&json(&schedule)["schedule_sha256"]);
        let candidate = build_candidate(&control, &schedule_sha256);
        let admitted = admit_candidate(
            &graph,
            &graph_run_id,
            "structured-quarantine-candidate",
            &candidate,
        );
        let contract_id = text(&admitted["inspection"]["record"]["contract_id"]);
        let provider_request_id =
            prepare_provider_request(&graph, &contract_id, "structured-quarantine-request");
        let release = export_scheduled_release_control(&graph, &provider_request_id);
        let authorization = authorize_scheduled_with_core(&release);
        let authorization_path = graph.cwd.path().join("scheduled-authorization.json");
        let pricing_path = graph.cwd.path().join("scheduled-pricing.json");
        fs::write(&authorization_path, authorization).expect("write authorization source");
        fs::write(&pricing_path, build_pricing()).expect("write pricing source");
        let core = RefusingCore::new(graph.state.path(), graph.cwd.path());
        let (network_guard, network_guard_path, network_guard_marker) = compile_network_guard();
        Self {
            graph,
            provider_request_id,
            authorization_path,
            pricing_path,
            core,
            _network_guard: network_guard,
            network_guard_path,
            network_guard_marker,
        }
    }

    fn invoke(&self, credential: &str) -> Output {
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
                "execute",
                &self.provider_request_id,
                "--authorization",
                path_text(&self.authorization_path),
                "--pricing",
                path_text(&self.pricing_path),
                "--core-bin",
                path_text(&self.core.path),
                "--core-bin-sha256",
                &self.core.sha256,
                "--confirm-off-machine",
            ],
        )
        .env("OPENAI_API_KEY", credential)
        .env("LD_PRELOAD", &self.network_guard_path)
        .env("FORGE_NETWORK_GUARD_MARKER", &self.network_guard_marker)
        .env_remove("OPENAI_BASE_URL")
        .env_remove("HTTP_PROXY")
        .env_remove("HTTPS_PROXY")
        .env_remove("ALL_PROXY")
        .env_remove("NO_PROXY")
        .output()
        .expect("run scheduled dispatch execute")
    }
}

struct RefusingCore {
    path: PathBuf,
    sha256: String,
    marker: PathBuf,
}

impl RefusingCore {
    fn new(state: &Path, directory: &Path) -> Self {
        let path = directory.join("refusing-scheduled-core");
        let marker = directory.join("refusing-scheduled-core-decisions");
        let owners = state.join("scheduled-executor-owners");
        let script = refusing_core_script(&owners, &marker);
        fs::write(&path, script.as_bytes()).expect("write refusing Core");
        fs::set_permissions(&path, fs::Permissions::from_mode(0o700))
            .expect("make refusing Core executable");
        let path = path.canonicalize().expect("canonical refusing Core");
        let sha256 = format!("{:x}", Sha256::digest(script.as_bytes()));
        Self {
            path,
            sha256,
            marker,
        }
    }
}

fn refusing_core_script(owners: &Path, marker: &Path) -> String {
    format!(
        "#!/bin/sh\n\
         if [ \"$2\" = \"--protocol-version\" ]; then printf 1; exit 0; fi\n\
         set -- {owners}/scheduled-executor-owner-*.json\n\
         [ \"$#\" -eq 1 ] || exit 2\n\
         owner=\"$1\"\n\
         [ -f \"$owner\" ] || exit 3\n\
         mv \"$owner\" \"$owner.original\" || exit 4\n\
         printf replacement > \"$owner\" || exit 5\n\
         printf refused >> {marker}\n\
         printf invalid\n",
        owners = shell_quote(owners),
        marker = shell_quote(marker),
    )
}

fn shell_quote(path: &Path) -> String {
    format!("'{}'", path_text(path).replace('\'', "'\"'\"'"))
}

fn build_pricing() -> Vec<u8> {
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
        .expect("build exact scheduled pricing");
    assert!(
        output.status.success(),
        "pricing failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    let value: Value = serde_json::from_slice(&output.stdout).expect("pricing JSON");
    assert_eq!(value["pricing_snapshot_sha256"], PRICING);
    output.stdout
}

fn compile_network_guard() -> (TempDir, PathBuf, PathBuf) {
    let directory = TempDir::new().expect("network guard directory");
    let source = directory.path().join("deny-network.c");
    let library = directory.path().join("deny-network.so");
    let marker = directory.path().join("network-blocked");
    fs::write(&source, NETWORK_GUARD_SOURCE).expect("write network guard source");
    let output = Command::new("cc")
        .args(["-shared", "-fPIC", "-O2", "-o"])
        .arg(&library)
        .arg(&source)
        .output()
        .expect("compile network guard");
    assert!(
        output.status.success(),
        "network guard compile failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (directory, library, marker)
}

const NETWORK_GUARD_SOURCE: &[u8] = br#"
#include <errno.h>
#include <fcntl.h>
#include <netdb.h>
#include <stdlib.h>
#include <sys/socket.h>
#include <unistd.h>

static void mark_blocked(void) {
    const char *path = getenv("FORGE_NETWORK_GUARD_MARKER");
    if (path == NULL) return;
    int descriptor = open(path, O_WRONLY | O_CREAT | O_APPEND, 0600);
    if (descriptor < 0) return;
    (void)write(descriptor, "blocked\n", 8);
    (void)close(descriptor);
}

int getaddrinfo(const char *node, const char *service,
                const struct addrinfo *hints, struct addrinfo **result) {
    (void)node; (void)service; (void)hints; (void)result;
    mark_blocked();
    return EAI_AGAIN;
}

int connect(int socket, const struct sockaddr *address, socklen_t length) {
    (void)socket; (void)address; (void)length;
    mark_blocked();
    errno = ENETUNREACH;
    return -1;
}
"#;

fn assert_quarantined_invocation(value: &Value, request: &str) {
    assert_eq!(value["status"], "quarantined");
    assert_eq!(value["provider_request_id"], request);
    assert_eq!(value["provider_poll_started"], true);
    assert_eq!(value["remote_provider_request_observation"], "not_attested");
    assert_eq!(value["lane_active"], false);
    assert_eq!(value["retry_authorized"], false);
    assert_eq!(value["lane_release_authorized"], false);
    assert_eq!(value["successor_advance_authorized"], false);
    assert_eq!(value["dispatch_performed_this_invocation"], true);
    assert_eq!(value["database_written_this_invocation"], true);
    assert_eq!(value["owner_sidecar_cleanup_observation"], "failed");
    assert!(value["owner_sidecar_left_active_by_this_invocation"].is_null());
    assert!(
        value["outcome"].is_null(),
        "Core refusal produced a receipt"
    );
}

fn assert_quarantined_reentry(value: &Value, request: &str) {
    assert_eq!(value["status"], "quarantined");
    assert_eq!(value["provider_request_id"], request);
    assert_eq!(value["provider_poll_started"], true);
    assert_eq!(value["retry_authorized"], false);
    assert_eq!(value["dispatch_performed_this_invocation"], false);
    assert_eq!(value["database_written_this_invocation"], false);
    assert_eq!(value["owner_sidecar_cleanup_observation"], "not_applicable");
    assert_eq!(value["owner_sidecar_left_active_by_this_invocation"], false);
}

fn assert_durable_quarantine(state: &Path, request: &str) {
    let facts: (String, i64, i64, i64, i64) = Connection::open(state.join("hub.sqlite3"))
        .expect("open Hub")
        .query_row(
            "SELECT status,lane_active,artifact_json IS NOT NULL,\
             terminal_control_json IS NULL,terminal_receipt_json IS NULL FROM \
             group_agent_graph_scheduled_node_dispatch_lifecycles \
             WHERE provider_request_id=?1",
            [request],
            |row| {
                Ok((
                    row.get(0)?,
                    row.get(1)?,
                    row.get(2)?,
                    row.get(3)?,
                    row.get(4)?,
                ))
            },
        )
        .expect("inspect durable quarantine");
    assert_eq!(facts, ("quarantined".into(), 0, 1, 1, 1));
}
