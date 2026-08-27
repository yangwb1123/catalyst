use std::{
    io::ErrorKind,
    net::TcpListener,
    path::Path,
    process::{Command, Output},
};

use serde_json::Value;

use super::{
    group_agent_graph_run_support::command,
    group_agent_graph_support::{path_text, successful_json, text},
    scheduled_graph_reconcile_cli_support::{
        CREDENTIAL_POISON, PinnedGoCore, ReconcileFixture, TASK_SECRET, WORKSPACE_SECRET,
    },
};

pub(super) const MODEL: &str = "private-controller-model";
pub(super) const OFFICIAL_ENDPOINT: &str = "https://api.openai.com/v1/responses";
pub(super) const PRICING_SHA256: &str =
    "4444444444444444444444444444444444444444444444444444444444444444";

pub(super) struct ControllerFixture {
    pub(super) source: ReconcileFixture,
    listener: TcpListener,
    proxy: String,
    schedule_sha256: String,
}

impl ControllerFixture {
    pub(super) fn new(core: &PinnedGoCore) -> Self {
        let source = ReconcileFixture::new(core);
        let schedule_sha256 = admitted_schedule_sha256(&source);
        let (listener, proxy) = loopback_proxy_sentinel();
        Self {
            source,
            listener,
            proxy,
            schedule_sha256,
        }
    }

    pub(super) fn start(&self, core: &Path, digest: &str) -> Output {
        self.start_with_pricing(core, digest, PRICING_SHA256)
    }

    pub(super) fn start_with_pricing(
        &self,
        core: &Path,
        digest: &str,
        pricing_sha256: &str,
    ) -> Output {
        self.secure_command(&self.start_args(core, digest, pricing_sha256))
            .output()
            .expect("run controller start")
    }

    pub(super) fn advance_command(&self, core: &Path, digest: &str) -> Command {
        self.secure_command(&[
            "group",
            "graph",
            "run",
            "controller",
            "advance",
            &self.source.graph_run_id,
            "--core-bin",
            path_text(core),
            "--core-bin-sha256",
            digest,
        ])
    }

    pub(super) fn advance(&self, core: &Path, digest: &str) -> Output {
        self.advance_command(core, digest)
            .output()
            .expect("run controller advance")
    }

    pub(super) fn show(&self) -> Output {
        self.secure_command(&[
            "group",
            "graph",
            "run",
            "controller",
            "show",
            &self.source.graph_run_id,
        ])
        .output()
        .expect("run controller show")
    }

    pub(super) fn step_command(
        &self,
        core: &Path,
        digest: &str,
        awaiting: &str,
        provider_request: &str,
        authorization: &str,
        pricing: &Path,
    ) -> Command {
        self.secure_command(&[
            "group",
            "graph",
            "run",
            "controller",
            "step",
            &self.source.graph_run_id,
            "--expected-awaiting-event-sha256",
            awaiting,
            "--expected-provider-request-id",
            provider_request,
            "--expected-authorization-sha256",
            authorization,
            "--pricing",
            path_text(pricing),
            "--core-bin",
            path_text(core),
            "--core-bin-sha256",
            digest,
            "--confirm-off-machine",
        ])
    }

    pub(super) fn assert_no_network(&self) {
        let error = self
            .listener
            .accept()
            .expect_err("unexpected provider connection");
        assert_eq!(error.kind(), ErrorKind::WouldBlock);
    }

    pub(super) fn assert_private(&self, output: &Output, extra: &[&str]) {
        let combined = [&output.stdout[..], &output.stderr[..]].concat();
        let rendered = String::from_utf8_lossy(&combined);
        for private in [
            CREDENTIAL_POISON,
            TASK_SECRET,
            WORKSPACE_SECRET,
            OFFICIAL_ENDPOINT,
            MODEL,
            &self.proxy,
        ]
        .into_iter()
        .chain(extra.iter().copied())
        {
            assert!(!rendered.contains(private), "output leaked private source");
        }
    }

    pub(super) fn assert_workspace_unchanged(&self) {
        self.source.assert_workspace_unchanged();
    }

    pub(super) fn schedule_sha256(&self) -> &str {
        &self.schedule_sha256
    }

    fn secure_command(&self, args: &[&str]) -> Command {
        let mut process = command(self.source.state.path(), self.source.cwd.path(), args);
        process
            .env("OPENAI_API_KEY", CREDENTIAL_POISON)
            .env("HTTP_PROXY", &self.proxy)
            .env("HTTPS_PROXY", &self.proxy)
            .env("http_proxy", &self.proxy)
            .env("https_proxy", &self.proxy)
            .env("NO_PROXY", "")
            .env("no_proxy", "")
            .env_remove("ALL_PROXY")
            .env_remove("all_proxy");
        process
    }

    fn start_args<'a>(
        &'a self,
        core: &'a Path,
        digest: &'a str,
        pricing_sha256: &'a str,
    ) -> Vec<&'a str> {
        vec![
            "group",
            "graph",
            "run",
            "controller",
            "start",
            &self.source.graph_run_id,
            "--expected-schedule-sha256",
            &self.schedule_sha256,
            "--core-bin",
            path_text(core),
            "--core-bin-sha256",
            digest,
            "--endpoint",
            OFFICIAL_ENDPOINT,
            "--model",
            MODEL,
            "--max-output-tokens",
            "32",
            "--max-model-output-bytes",
            "4096",
            "--max-model-events",
            "64",
            "--timeout-ms",
            "5000",
            "--max-cost-usd-micros",
            "10000",
            "--pricing-snapshot-sha256",
            pricing_sha256,
            "--max-result-bytes",
            "4096",
            "--max-effectful-steps",
            "2",
            "--max-total-cost-usd-micros",
            "20000",
        ]
    }
}

pub(super) fn assert_event_chain(value: &Value) {
    let events = value["events"].as_array().expect("show event chain");
    assert_eq!(
        events.len(),
        usize::try_from(value["head_sequence"].as_u64().unwrap()).unwrap()
    );
    let mut previous = None;
    for (index, event) in events.iter().enumerate() {
        assert_eq!(event["sequence"], u64::try_from(index + 1).unwrap());
        assert_eq!(event["previous_event_sha256"].as_str(), previous.as_deref());
        let digest = text(&event["event_sha256"]);
        assert_digest(&digest);
        previous = Some(digest);
    }
    assert_eq!(
        events.last().unwrap()["event_sha256"],
        value["head_event_sha256"]
    );
}

pub(super) fn assert_digest(value: &str) {
    assert_eq!(value.len(), 64);
    assert!(
        value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    );
}

fn admitted_schedule_sha256(fixture: &ReconcileFixture) -> String {
    let output = command(
        fixture.state.path(),
        fixture.cwd.path(),
        &[
            "group",
            "graph",
            "run",
            "schedule",
            "list",
            &fixture.graph_run_id,
        ],
    )
    .output()
    .expect("list admitted schedule");
    let value = successful_json(&output);
    assert_eq!(value["schedules"].as_array().map(Vec::len), Some(1));
    text(&value["schedules"][0]["schedule_sha256"])
}

fn loopback_proxy_sentinel() -> (TcpListener, String) {
    let listener = TcpListener::bind("127.0.0.1:0").expect("loopback proxy sentinel");
    listener
        .set_nonblocking(true)
        .expect("nonblocking proxy sentinel");
    let port = listener
        .local_addr()
        .expect("proxy sentinel address")
        .port();
    (listener, format!("http://127.0.0.1:{port}"))
}
